use clap::{Parser, Subcommand};
use jki_core::{
    paths::JkiPath, 
    git, 
    Account, 
    AccountSecret,
    acquire_master_key, 
    encrypt_with_master_key,
    decrypt_with_master_key,
    import::parse_otpauth_uri,
    Interactor,
    TerminalInteractor,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::env;
use std::io::{Read, Write};

#[derive(Parser)]
#[command(name = "jkim", version, about = "JK Suite Management Hub")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Force interactive master key input, ignoring master.key file
    #[arg(short = 'I', long, global = true)]
    pub interactive: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Check the health and status of the JKI system
    Status,
    /// Initialize the JKI home directory and Git repository
    Init {
        /// Force reset by deleting existing vault data
        #[arg(short, long)]
        force: bool,
    },
    /// Sync changes to Git (add, commit, pull --rebase, push)
    Sync,
    /// Edit metadata manually using your default editor
    Edit,
    /// Manage the Master Key
    #[command(subcommand)]
    MasterKey(MasterKeyCommands),
    /// Import accounts from a WinAuth decrypted text file
    ImportWinauth {
        /// Path to the decrypted WinAuth .txt file
        file: PathBuf,
        /// Overwrite existing accounts if name+issuer matches
        #[arg(short, long)]
        overwrite: bool,
    },
}

#[derive(Subcommand)]
enum MasterKeyCommands {
    /// Save a new master key to disk (0600)
    Set {
        /// Force overwrite without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Delete the master key from disk
    Remove {
        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Re-encrypt the vault with a new master key
    Change {
        /// Automatically commit the change to Git
        #[arg(long)]
        commit: bool,
    },
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct MetadataFile {
    accounts: Vec<Account>,
    version: u32,
}

fn handle_status() {
    println!("--- Just Keep Identity Status ---\n");
    let key_path = JkiPath::master_key_path();
    if key_path.exists() {
        match JkiPath::check_secure_permissions(&key_path) {
            Ok(_) => println!("  - Master Key File : OK ({:?}, 0600)", key_path),
            Err(e) => println!("  - Master Key File : SECURITY ERROR ({})", e),
        }
    } else {
        println!("  - Master Key File : Not found (Standalone mode disabled)");
    }
    println!("  - jki-agent       : Not checked (IPC placeholder)");

    println!("\n[Data & Synchronization]");
    let config_dir = JkiPath::home_dir();
    if let Some(repo_status) = git::check_status(&config_dir) {
        println!("  - Git Repository  : OK ({:?})", config_dir);
        println!("  - Current Branch  : {}", repo_status.branch);
        println!("  - Working Tree    : {}", if repo_status.is_clean { "Clean" } else { "Modified" });
        println!("  - Remote          : {}", if repo_status.has_remote { "Configured" } else { "None" });
    } else {
        println!("  - Git Repository  : Not initialized");
    }

    println!("\n[Paths]");
    println!("  - Metadata Path   : {:?}", JkiPath::metadata_path());
    println!("  - Secrets Path    : {:?}", JkiPath::secrets_path());
}

fn handle_master_key(cmd: &MasterKeyCommands, force_interactive: bool, interactor: &dyn Interactor) {
    let key_path = JkiPath::master_key_path();
    let sec_path = JkiPath::secrets_path();

    match cmd {
        MasterKeyCommands::Set { force } => {
            if !*force && key_path.exists() {
                if !interactor.confirm(&format!("Warning: master.key already exists at {:?}", key_path)) { return; }
            }
            if !*force && sec_path.exists() {
                println!("CRITICAL WARNING: vault.secrets.bin.age already exists.");
                println!("If the new key doesn't match the one used to encrypt it, you will LOSE ACCESS to your secrets.");
                if !interactor.confirm("Proceed anyway?") { return; }
            }

            let p1 = interactor.prompt_password("Enter new Master Key").expect("Input failed");
            let p2 = interactor.prompt_password("Confirm Master Key").expect("Input failed");
            if p1.expose_secret() != p2.expose_secret() {
                eprintln!("Error: Passwords do not match.");
                return;
            }

            fs::write(&key_path, p1.expose_secret()).expect("Failed to write key");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            println!("Master Key saved to {:?}", key_path);
        }
        MasterKeyCommands::Remove { force } => {
            if !key_path.exists() {
                eprintln!("Error: master.key not found.");
                return;
            }
            if !*force {
                if !interactor.confirm("Warning: Removing master.key means you will need to input it manually for every 'jki' command. Are you sure?") { return; }
            }
            fs::remove_file(&key_path).expect("Failed to remove key");
            println!("Master Key removed.");
        }
        MasterKeyCommands::Change { commit } => {
            // 1. Try to get current key to decrypt
            let mut current_key = acquire_master_key(force_interactive, interactor).unwrap_or_else(|_| interactor.prompt_password("Enter current Master Key").expect("Input failed"));
            
            let mut secrets_data = None;
            if sec_path.exists() {
                let encrypted = fs::read(&sec_path).expect("Failed to read secrets");
                match decrypt_with_master_key(&encrypted, &current_key) {
                    Ok(d) => secrets_data = Some(d),
                    Err(_) => {
                        // If file-based key failed, try prompting once
                        println!("Stored Master Key failed to decrypt vault.");
                        current_key = interactor.prompt_password("Enter CORRECT current Master Key").expect("Input failed");
                        secrets_data = Some(decrypt_with_master_key(&encrypted, &current_key).expect("Authentication failed"));
                    }
                }
            } else {
                println!("No existing vault found. This is equivalent to 'set'.");
            }

            // 2. Get new key
            let p1 = interactor.prompt_password("Enter NEW Master Key").expect("Input failed");
            let p2 = interactor.prompt_password("Confirm NEW Master Key").expect("Input failed");
            if p1.expose_secret() != p2.expose_secret() {
                eprintln!("Error: Passwords do not match.");
                return;
            }

            // 3. Atomic Write
            let key_tmp = key_path.with_extension("tmp");
            let sec_tmp = sec_path.with_extension("tmp");

            // Write new key
            fs::write(&key_tmp, p1.expose_secret()).expect("Failed to write temp key");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_tmp, fs::Permissions::from_mode(0o600)).unwrap();
            }

            // Write new vault (if exists)
            if let Some(data) = secrets_data {
                let encrypted = encrypt_with_master_key(&data, &p1).expect("Encryption failed");
                fs::write(&sec_tmp, encrypted).expect("Failed to write temp secrets");
            }

            // Atomic rename
            if sec_tmp.exists() { fs::rename(&sec_tmp, &sec_path).expect("Failed to replace secrets"); }
            fs::rename(&key_tmp, &key_path).expect("Failed to replace key");

            println!("Master Key changed successfully.");
            if *commit {
                let config_dir = JkiPath::home_dir();
                git::add_all(&config_dir).ok();
                git::commit(&config_dir, "jki: master key rotation").ok();
                println!("Changes committed to Git.");
            } else {
                println!("Note: You may want to run 'jkim sync' to backup your new encrypted vault.");
            }
        }
    }
}

fn handle_sync() {
    let config_dir = JkiPath::home_dir();
    println!("Syncing JKI Home at {:?}...", config_dir);

    let status = match git::check_status(&config_dir) {
        Some(s) => s,
        None => {
            eprintln!("Error: Not a git repository. Run 'jkim init' first.");
            return;
        }
    };

    println!("  - Stage changes...");
    git::add_all(&config_dir).expect("Failed to add files");

    println!("  - Commit...");
    let now = chrono::Local::now();
    let msg = format!("jki backup: {}", now.format("%Y-%m-%d %H:%M:%S"));
    match git::commit(&config_dir, &msg) {
        Ok(true) => println!("  - Committed: {}", msg),
        Ok(false) => println!("  - Nothing to commit, working tree clean."),
        Err(e) => eprintln!("  - Commit failed: {}", e),
    }

    if status.has_remote {
        println!("  - Pull --rebase...");
        if let Err(e) = git::pull_rebase(&config_dir) {
            eprintln!("  - Pull failed: {}. Resolve conflicts manually.", e);
            return;
        }

        println!("  - Push...");
        if let Err(e) = git::push(&config_dir) {
            eprintln!("  - Push failed: {}.", e);
            return;
        }
        println!("Sync completed successfully!");
    } else {
        println!("No remote configured. Local backup complete.");
    }
}

fn handle_edit() {
    let meta_path = JkiPath::metadata_path();
    if !meta_path.exists() {
        eprintln!("Error: Metadata not found. Run 'jkim init' first.");
        return;
    }

    // 1. Prepare temporary file with .tmp.json suffix for better syntax highlighting
    let mut temp_file = tempfile::Builder::new()
        .prefix("jki-metadata-")
        .suffix(".tmp.json")
        .tempfile()
        .expect("Failed to create temporary file");

    // 2. Set secure permissions on Unix (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(temp_file.path()).unwrap().permissions();
        perms.set_mode(0o600);
        fs::set_permissions(temp_file.path(), perms).unwrap();
    }

    // 3. Copy content to temp file
    let content = fs::read_to_string(&meta_path).expect("Failed to read metadata");
    temp_file.write_all(content.as_bytes()).expect("Failed to write to temporary file");
    temp_file.flush().expect("Failed to flush temporary file");

    // 4. Launch editor
    let editor = env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) { "notepad.exe".to_string() } else { "vi".to_string() }
    });

    println!("Opening metadata with {}...", editor);
    let status = Command::new(&editor)
        .arg(temp_file.path())
        .status();

    match status {
        Ok(s) if s.success() => {
            // 5. Validate JSON after editing
            let mut new_content = String::new();
            temp_file.reopen().expect("Failed to reopen temp file")
                .read_to_string(&mut new_content).expect("Failed to read back metadata from temp file");

            match serde_json::from_str::<MetadataFile>(&new_content) {
                Ok(_) => {
                    // 6. Write back if valid
                    fs::write(&meta_path, &new_content).expect("Failed to write back metadata");
                    println!("Metadata updated and validated successfully.");
                }
                Err(e) => {
                    eprintln!("\nERROR: Metadata contains JSON syntax errors: {}", e);
                    eprintln!("The changes have NOT been applied.");
                    eprintln!("Your edited content is preserved at: {:?}", temp_file.path());
                    // Keep the temp file by persisting it (don't let it be deleted)
                    let (file, path) = temp_file.keep().expect("Failed to preserve temp file");
                    drop(file);
                    drop(path);
                }
            }
        }
        Ok(s) => eprintln!("Editor exited with error: {}", s),
        Err(e) => eprintln!("Failed to launch editor '{}': {}", editor, e),
    }
}

fn handle_init(force: bool) {
    let config_dir = JkiPath::home_dir();
    println!("Initializing JKI Home at {:?}...", config_dir);

    // 1. Force Reset Logic
    if force {
        println!("\n[Force Reset]");
        let meta = JkiPath::metadata_path();
        let sec = JkiPath::secrets_path();
        if meta.exists() { let _ = fs::remove_file(&meta); println!("  - Metadata: Deleted."); }
        if sec.exists() { let _ = fs::remove_file(&sec); println!("  - Secrets:  Deleted."); }
    }

    // 2. Directory Creation
    print!("  - Directory: ");
    if !config_dir.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).recursive(true).create(&config_dir).expect("Failed to create config directory");
        }
        #[cfg(windows)]
        {
            fs::create_dir_all(&config_dir).expect("Failed to create config directory");
        }
        println!("Created.");
    } else {
        println!("Already exists. (Skipped)");
    }

    // 3. Git Initialization
    print!("  - Git Repo:  ");
    if !config_dir.join(".git").exists() {
        let status = Command::new("git").args(["init", "-b", "main"]).current_dir(&config_dir).status();
        match status {
            Ok(s) if s.success() => println!("Initialized."),
            _ => println!("FAILED to initialize."),
        }
    } else {
        println!("Already initialized. (Skipped)");
    }

    // 4. Config Files
    print!("  - Config:    ");
    let gitignore_path = config_dir.join(".gitignore");
    fs::write(gitignore_path, "# JKI\nmaster.key\nvault.json\n*.txt\n*.bin\n").ok();
    let gitattrs_path = config_dir.join(".gitattributes");
    fs::write(gitattrs_path, "vault.secrets.bin.age binary\nvault.metadata.json filter=age\n").ok();
    println!(".gitignore and .gitattributes written. (Updated)");

    // 5. Data Conflict Detection
    let meta_exists = JkiPath::metadata_path().exists();
    let sec_exists = JkiPath::secrets_path().exists();
    if meta_exists || sec_exists {
        println!("\n[Data Warning]");
        if sec_exists { println!("  - Existing vault data (vault.secrets.bin.age) detected."); }
        println!("  - Subsequent imports will attempt to MERGE using your Master Key.");
        println!("  - To start fresh, use 'jkim init --force' or delete vault.* manually.");
    }

    println!("\nInitialization complete!");
    println!("Next steps:");
    println!("  - Run 'jkim status' to check health.");
    println!("  - Run 'jkim import-winauth <file>' to add accounts.");
}

fn handle_import_winauth(file: &PathBuf, overwrite: bool, force_interactive: bool, interactor: &dyn Interactor) {
    if !file.exists() { eprintln!("Error: File not found."); return; }

    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();

    // 1. Acquire Master Key EARLIER (We need it to load existing secrets)
    println!("Please unlock your vault to perform import.");
    let master_key = acquire_master_key(force_interactive, interactor).unwrap_or_else(|e| {
        eprintln!("Authentication failed: {}", e);
        std::process::exit(1);
    });

    // 2. Load existing Metadata
    let mut metadata = if meta_path.exists() {
        let content = fs::read_to_string(&meta_path).unwrap();
        serde_json::from_str::<MetadataFile>(&content).unwrap()
    } else {
        MetadataFile { accounts: vec![], version: 1 }
    };

    // 3. Load and Decrypt existing Secrets (Merge-aware)
    let mut secrets_map: HashMap<String, AccountSecret> = if sec_path.exists() {
        let encrypted = fs::read(&sec_path).expect("Failed to read secrets");
        let decrypted = decrypt_with_master_key(&encrypted, &master_key).expect("Failed to decrypt existing secrets. Is the master key correct?");
        serde_json::from_slice(&decrypted).expect("Failed to parse secrets JSON")
    } else {
        HashMap::new()
    };

    // 4. Process Import
    let content = fs::read_to_string(file).expect("Failed to read file");
    let mut new_count = 0;
    let mut updated_count = 0;
    let mut skip_count = 0;

    for line in content.lines() {
        if let Some(mut acc) = parse_otpauth_uri(line) {
            let existing_pos = metadata.accounts.iter().position(|m| m.name == acc.name && m.issuer == acc.issuer);
            
            if let Some(pos) = existing_pos {
                let id = metadata.accounts[pos].id.clone();
                
                if !overwrite {
                    // 即使跳過 Metadata 更新，如果 secrets_map 裡已有，也必須保留
                    skip_count += 1;
                    continue;
                }
                
                // Update case
                let entry = AccountSecret { secret: acc.secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                acc.id = id.clone();
                acc.secret = "".to_string();
                metadata.accounts[pos] = acc;
                secrets_map.insert(id, entry);
                updated_count += 1;
            } else {
                // Insert case
                let id = acc.id.clone();
                let entry = AccountSecret { secret: acc.secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                acc.secret = "".to_string();
                metadata.accounts.push(acc);
                secrets_map.insert(id, entry);
                new_count += 1;
            }
        }
    }

    // 5. Encrypt and Save
    let secrets_json = serde_json::to_vec(&secrets_map).unwrap();
    let encrypted_data = encrypt_with_master_key(&secrets_json, &master_key).expect("Encryption failed");

    fs::write(&meta_path, serde_json::to_string_pretty(&metadata).unwrap()).unwrap();
    fs::write(&sec_path, encrypted_data).unwrap();

    println!("\nImport completed successfully!");
    println!("  - New: {}, Updated: {}, Skipped: {}", new_count, updated_count, skip_count);
}

fn main() {
    let cli = Cli::parse();
    let interactor = TerminalInteractor;

    match &cli.command {
        Commands::Status => handle_status(),
        Commands::Init { force } => handle_init(*force),
        Commands::Sync => handle_sync(),
        Commands::Edit => handle_edit(),
        Commands::MasterKey(m) => handle_master_key(m, cli.interactive, &interactor),
        Commands::ImportWinauth { file, overwrite } => handle_import_winauth(file, *overwrite, cli.interactive, &interactor),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;
    use std::env;
    use jki_core::MockInteractor;
    use std::cell::RefCell;

    #[test]
    #[serial]
    fn test_handle_master_key_set() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_set");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        let cmd = MasterKeyCommands::Set { force: false };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec!["newpass".to_string(), "newpass".to_string()]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, false, &interactor);

        assert!(home.join("master.key").exists());
        assert_eq!(fs::read_to_string(home.join("master.key")).unwrap(), "newpass");
    }

    #[test]
    #[serial]
    fn test_handle_master_key_change_rotation() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_rotation");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // 1. Setup initial state
        let old_pass = "oldpass";
        let key_path = home.join("master.key");
        fs::write(&key_path, old_pass).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        
        let secret_data = b"my secret";
        let encrypted = encrypt_with_master_key(secret_data, &secrecy::SecretString::from(old_pass.to_string())).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();

        // 2. Change key
        let cmd = MasterKeyCommands::Change { commit: false };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec!["newpass".to_string(), "newpass".to_string()]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, false, &interactor);

        // 3. Verify
        assert_eq!(fs::read_to_string(home.join("master.key")).unwrap(), "newpass");
        
        let new_encrypted = fs::read(home.join("vault.secrets.bin.age")).unwrap();
        let decrypted = decrypt_with_master_key(&new_encrypted, &secrecy::SecretString::from("newpass".to_string())).unwrap();
        assert_eq!(decrypted, secret_data);
    }

    #[test]
    #[serial]
    fn test_handle_master_key_remove() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_remove");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        fs::write(home.join("master.key"), "todelete").unwrap();
        
        let cmd = MasterKeyCommands::Remove { force: true };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, false, &interactor);

        assert!(!home.join("master.key").exists());
    }

    #[test]
    #[serial]
    fn test_handle_init() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home");
        env::set_var("JKI_HOME", &home);

        handle_init();

        assert!(home.exists());
        assert!(home.join(".git").exists());
        assert!(home.join(".gitignore").exists());
        assert!(home.join(".gitattributes").exists());
    }

    #[test]
    #[serial]
    fn test_handle_status() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_status");
        env::set_var("JKI_HOME", &home);
        
        // Before init
        handle_status();
        
        handle_init();
        
        // After init
        handle_status();
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_handle_import_winauth() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_import");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        // 1. Create master.key
        let key_path = home.join("master.key");
        fs::write(&key_path, "testpass").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        // 2. Create WinAuth export file
        let import_file = temp.path().join("winauth.txt");
        fs::write(&import_file, "otpauth://totp/Google:test@gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google\n").unwrap();

        // 3. Run import
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };
        handle_import_winauth(&import_file, false, false, &interactor);

        // 4. Verify files
        let meta_path = home.join("vault.metadata.json");
        let sec_path = home.join("vault.secrets.bin.age");
        assert!(meta_path.exists());
        assert!(sec_path.exists());

        let meta_content = fs::read_to_string(meta_path).unwrap();
        let metadata: MetadataFile = serde_json::from_str(&meta_content).unwrap();
        assert_eq!(metadata.accounts.len(), 1);
        assert_eq!(metadata.accounts[0].name, "test@gmail.com");
        assert_eq!(metadata.accounts[0].issuer, Some("Google".to_string()));
    }

    #[test]
    #[serial]
    fn test_handle_sync() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_sync");
        env::set_var("JKI_HOME", &home);

        // 1. Init
        handle_init();
        
        // 2. Add some file
        fs::write(home.join("test.txt"), "content").unwrap();
        
        // 3. Sync (should commit)
        handle_sync();
        
        // 4. Verify commit
        let output = Command::new("git")
            .args(["-C", home.to_str().unwrap(), "log", "-n", "1"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stdout);
        assert!(log.contains("jki backup:"));
    }
}
