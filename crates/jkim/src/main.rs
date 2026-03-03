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
    keychain::{KeyringStore, SecretStore},
    AuthSource,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::env;
use std::io::{Read, Write};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::AesMode;

#[derive(Parser)]
#[command(name = "jkim", version, about = "JK Suite Management Hub")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Authentication and data source
    #[arg(short = 'A', long, global = true, default_value = "auto")]
    pub auth: AuthSource,

    /// Force interactive master key input (alias for --auth interactive)
    #[arg(short = 'I', long, global = true)]
    pub interactive: bool,

    /// Apply recommended default decisions for all prompts
    #[arg(short, long, global = true)]
    pub default: bool,
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
    /// Decrypt the vault to plaintext JSON (for zero-latency mode)
    Decrypt {
        /// Force overwrite of existing vault.secrets.json
        #[arg(short, long)]
        force: bool,
        /// Keep the encrypted source file (.age) after decryption
        #[arg(short, long)]
        keep: bool,
        /// Remove the master.key file after decryption
        #[arg(long)]
        remove_key: bool,
    },
    /// Encrypt the plaintext JSON vault back to .age
    Encrypt {
        /// Force overwrite of existing vault.secrets.bin.age
        #[arg(short, long)]
        force: bool,
    },
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
        /// If decryption fails, discard existing vault and create a new one
        #[arg(long)]
        force_new_vault: bool,
    },
    /// Export all accounts to a password-protected ZIP file
    Export {
        /// Optional path for the export file
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum MasterKeyCommands {
    /// Save a new master key to disk (0600)
    Set {
        /// Force overwrite without confirmation
        #[arg(short, long)]
        force: bool,
        /// Store the key in the system keychain (default: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        keychain: bool,
    },
    /// Delete the master key from disk
    Remove {
        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
        /// Remove the key from the system keychain
        #[arg(long)]
        keychain: bool,
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

fn handle_export(output: &Option<PathBuf>, auth: AuthSource, interactor: &dyn Interactor) {
    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    if !meta_path.exists() {
        eprintln!("Error: Metadata not found. Run 'jkim init' first.");
        return;
    }

    // 1. Acquire Master Key
    let master_key = acquire_master_key(auth, interactor, Some(&KeyringStore)).unwrap_or_else(|e| {
        eprintln!("Authentication failed: {}", e);
        std::process::exit(1);
    });

    // 2. Load Metadata
    let meta_content = fs::read_to_string(&meta_path).expect("Failed to read metadata");
    let metadata: MetadataFile = serde_json::from_str(&meta_content).expect("Failed to parse metadata");

    // 3. Load Secrets
    let secrets_map: HashMap<String, AccountSecret> = if dec_path.exists() {
        let content = fs::read(&dec_path).expect("Failed to read plaintext secrets");
        serde_json::from_slice(&content).expect("Failed to parse plaintext secrets")
    } else if sec_path.exists() {
        let encrypted = fs::read(&sec_path).expect("Failed to read secrets file");
        let decrypted = decrypt_with_master_key(&encrypted, &master_key).expect("Decryption failed");
        serde_json::from_slice(&decrypted).expect("Failed to parse existing secrets JSON")
    } else {
        eprintln!("Error: Secrets not found.");
        return;
    };

    // 4. Integrate
    let (integrated, missing) = jki_core::integrate_accounts(metadata.accounts, &secrets_map);
    if !missing.is_empty() {
        eprintln!("Warning: Some accounts are missing secrets: {:?}", missing);
    }

    // 5. Prompt for Export Password
    let export_pass = interactor.prompt_password("Enter EXPORT Password (for ZIP encryption)").expect("Input failed");
    let export_pass_confirm = interactor.prompt_password("Confirm EXPORT Password").expect("Input failed");
    if export_pass.expose_secret() != export_pass_confirm.expose_secret() {
        eprintln!("Error: Passwords do not match.");
        return;
    }

    // 6. Determine output path
    let output_path = output.clone().unwrap_or_else(|| {
        let now = chrono::Local::now();
        PathBuf::from(format!("export_{}.zip", now.format("%Y%m%d_%H%M")))
    });

    // 7. Create ZIP
    let file = fs::File::create(&output_path).expect("Failed to create export file");
    let mut zip = zip::ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .with_aes_encryption(AesMode::Aes256, export_pass.expose_secret());

    zip.start_file("accounts.txt", options).expect("Failed to start file in ZIP");
    
    for acc in integrated {
        let uri = acc.to_otpauth_uri();
        zip.write_all(uri.as_bytes()).expect("Failed to write to ZIP");
        zip.write_all(b"\n").expect("Failed to write newline to ZIP");
    }

    zip.finish().expect("Failed to finalize ZIP");
    println!("Export completed successfully: {:?}", output_path);
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
        println!("  - Master Key File : Not found");
    }

    match KeyringStore.get_secret("jki", "master_key") {
        Ok(_) => println!("  - System Keychain : Found (jki:master_key)"),
        Err(_) => println!("  - System Keychain : Not found"),
    }
    
    let agent_status = if jki_core::agent::AgentClient::ping() {
        if jki_core::agent::AgentClient::get_master_key().is_ok() {
            "Running (Unlocked)"
        } else {
            "Running (Locked)"
        }
    } else {
        "Not running"
    };
    println!("  - jki-agent       : {}", agent_status);

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

fn handle_master_key(cmd: &MasterKeyCommands, auth: AuthSource, default_flag: bool, interactor: &dyn Interactor) {
    let key_path = JkiPath::master_key_path();
    let sec_path = JkiPath::secrets_path();

    match cmd {
        MasterKeyCommands::Set { force, keychain } => {
            if !*force && key_path.exists() {
                if !default_flag && !interactor.confirm(&format!("Warning: master.key already exists at {:?}", key_path), false) { return; }
            }
            if !*force && sec_path.exists() {
                println!("CRITICAL WARNING: vault.secrets.bin.age already exists.");
                println!("If the new key doesn't match the one used to encrypt it, you will LOSE ACCESS to your secrets.");
                if !default_flag && !interactor.confirm("Proceed anyway?", false) { return; }
            }

            let p1 = interactor.prompt_password("Enter new Master Key").expect("Input failed");
            let p2 = interactor.prompt_password("Confirm Master Key").expect("Input failed");
            if p1.expose_secret() != p2.expose_secret() {
                eprintln!("Error: Passwords do not match.");
                return;
            }

            // Save to file
            fs::write(&key_path, p1.expose_secret()).expect("Failed to write key");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            println!("Master Key saved to {:?}", key_path);

            // Save to keychain
            if *keychain {
                if let Err(e) = KeyringStore.set_secret("jki", "master_key", p1.expose_secret()) {
                    eprintln!("Warning: Failed to save to system keychain: {}", e);
                } else {
                    println!("Master Key saved to system keychain.");
                }
            }
        }
        MasterKeyCommands::Remove { force, keychain } => {
            let mut removed = false;
            if key_path.exists() {
                if !*force {
                    if !default_flag && !interactor.confirm("Warning: Removing master.key from disk. Are you sure?", false) { return; }
                }
                fs::remove_file(&key_path).expect("Failed to remove key");
                println!("Master Key removed from disk.");
                removed = true;
            }

            if *keychain {
                if !*force && !removed {
                    if !default_flag && !interactor.confirm("Warning: Removing master_key from system keychain. Are you sure?", false) { return; }
                }
                if let Err(e) = KeyringStore.delete_secret("jki", "master_key") {
                    eprintln!("Warning: Failed to remove from system keychain: {}", e);
                } else {
                    println!("Master Key removed from system keychain.");
                }
            }
        }
        MasterKeyCommands::Change { commit } => {
            // 1. Try to get current key to decrypt
            let mut current_key = acquire_master_key(auth, interactor, Some(&KeyringStore))
                .unwrap_or_else(|_| interactor.prompt_password("Enter current Master Key").expect("Input failed"));
            
            let mut secrets_data = None;
            if sec_path.exists() {
                let encrypted = fs::read(&sec_path).expect("Failed to read secrets");
                match decrypt_with_master_key(&encrypted, &current_key) {
                    Ok(d) => secrets_data = Some(d),
                    Err(_) => {
                        // If stored key failed, try prompting once
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

            // Write new key to file
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

            // Update Keychain
            if let Ok(_) = KeyringStore.get_secret("jki", "master_key") {
                if let Err(e) = KeyringStore.set_secret("jki", "master_key", p1.expose_secret()) {
                    eprintln!("Warning: Failed to update system keychain: {}", e);
                } else {
                    println!("Master Key updated in system keychain.");
                }
            }

            println!("Master Key changed successfully.");
            let _ = jki_core::agent::AgentClient::unlock(&p1);
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

fn handle_sync(default_flag: bool, interactor: &dyn Interactor) {
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
            eprintln!("  - Pull failed: {}.", e);
            
            let should_resolve = if default_flag {
                println!("  - Conflict detected. Using recommended path (--default).");
                true
            } else {
                interactor.confirm("Conflict detected. Automatically backup and resolve using local changes?", true)
            };

            if should_resolve {
                match git::get_conflicting_files(&config_dir) {
                    Ok(files) if !files.is_empty() => {
                        for f in &files {
                            let src = config_dir.join(f);
                            let dst = config_dir.join(format!("{}.conflict", f));
                            if let Err(e) = fs::copy(&src, &dst) {
                                eprintln!("  - Warning: Failed to backup {}: {}", f, e);
                            } else {
                                println!("  - Backed up conflicting file to {:?}", dst);
                            }
                        }
                        println!("  - Resolving conflicts using local changes (prefer local)...");
                        if let Err(e) = git::checkout_theirs(&config_dir, &files) {
                            eprintln!("  - Error: Failed to resolve: {}", e);
                            return;
                        }
                        if let Err(e) = git::add(&config_dir, &files) {
                            eprintln!("  - Error: Failed to add resolved files: {}", e);
                            return;
                        }
                        if let Err(e) = git::rebase_continue(&config_dir) {
                            eprintln!("  - Error: Failed to continue rebase: {}", e);
                            return;
                        }
                        println!("  - Conflicts resolved and rebase completed.");
                    }
                    Ok(_) => {
                        eprintln!("  - Pull failed but no conflicts detected. Resolve manually.");
                        return;
                    }
                    Err(e) => {
                        eprintln!("  - Error: Failed to get conflicting files: {}", e);
                        return;
                    }
                }
            } else {
                println!("  - Manual resolution required. Run 'git status' to see conflicts.");
                return;
            }
        }

        println!("  - Push...");
        if let Err(e) = git::push(&config_dir) {
            eprintln!("  - Push failed: {}.", e);
            return;
        }
        println!("Sync completed successfully!");
        let _ = jki_core::agent::AgentClient::reload();
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
                    let _ = jki_core::agent::AgentClient::reload();
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

fn handle_decrypt(force: bool, keep: bool, remove_key: bool, default_flag: bool, auth: AuthSource, interactor: &dyn Interactor) {
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();
    let key_path = JkiPath::master_key_path();

    if !sec_path.exists() {
        eprintln!("Error: Encrypted vault not found at {:?}", sec_path);
        return;
    }

    if dec_path.exists() && !force {
        if !default_flag && !interactor.confirm(&format!("Warning: Plaintext vault already exists at {:?}. Overwrite?", dec_path), false) {
            return;
        }
    }

    let master_key = acquire_master_key(auth, interactor, Some(&KeyringStore)).expect("Authentication failed");
    let encrypted = fs::read(&sec_path).expect("Failed to read secrets");
    let decrypted = decrypt_with_master_key(&encrypted, &master_key).expect("Decryption failed");

    fs::write(&dec_path, &decrypted).expect("Failed to write plaintext vault");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dec_path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    println!("Vault decrypted to plaintext at {:?}", dec_path);
    println!("Note: jki will now use this for zero-latency lookups.");

    // 1. Handle .age deletion (Recommended: Yes)
    if !keep {
        if default_flag || interactor.confirm("Delete encrypted source (.age)?", true) {
            fs::remove_file(&sec_path).expect("Failed to delete encrypted source");
            println!("Encrypted source deleted.");
        }
    }

    // 2. Handle master.key deletion (Recommended: No)
    if remove_key {
        if key_path.exists() {
            fs::remove_file(&key_path).expect("Failed to delete master.key");
            println!("Master Key file removed.");
        }
    } else if key_path.exists() && !default_flag {
        if interactor.confirm("Delete master key file?", false) {
            fs::remove_file(&key_path).expect("Failed to delete master.key");
            println!("Master Key file removed.");
        }
    }
}

fn handle_encrypt(force: bool, default_flag: bool, auth: AuthSource, interactor: &dyn Interactor) {
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    if !dec_path.exists() {
        eprintln!("Error: Plaintext vault not found at {:?}", dec_path);
        return;
    }

    if sec_path.exists() && !force {
        if !default_flag && !interactor.confirm(&format!("Warning: Encrypted vault already exists at {:?}. Overwrite?", sec_path), false) {
            return;
        }
    }

    let master_key = acquire_master_key(auth, interactor, Some(&KeyringStore)).expect("Authentication failed");
    let decrypted = fs::read(&dec_path).expect("Failed to read plaintext secrets");
    let encrypted = encrypt_with_master_key(&decrypted, &master_key).expect("Encryption failed");

    fs::write(&sec_path, encrypted).expect("Failed to write encrypted vault");
    fs::remove_file(&dec_path).expect("Failed to delete plaintext vault after encryption");
    println!("Vault encrypted to {:?}", sec_path);
    println!("Plaintext vault physically deleted.");
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

fn handle_import_winauth(file: &PathBuf, overwrite: bool, auth: AuthSource, default_flag: bool, interactor: &dyn Interactor, force_new_vault: bool) {
    if !file.exists() { eprintln!("Error: File not found."); return; }

    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();
    let key_path = JkiPath::master_key_path();

    // 1. Detect State
    let is_plaintext = dec_path.exists();
    let is_encrypted = sec_path.exists();
    let has_master_key = key_path.exists();

    // 2. Acquire Master Key
    println!("Please unlock your vault to perform import.");
    let master_key = acquire_master_key(auth, interactor, Some(&KeyringStore)).unwrap_or_else(|e| {
        eprintln!("Authentication failed: {}", e);
        std::process::exit(1);
    });

    // 3. Load existing Metadata
    let mut metadata = if meta_path.exists() {
        let content = fs::read_to_string(&meta_path).unwrap_or_default();
        serde_json::from_str::<MetadataFile>(&content).unwrap_or(MetadataFile { accounts: vec![], version: 1 })
    } else {
        MetadataFile { accounts: vec![], version: 1 }
    };

    // 4. Load Secrets (Prefer Plaintext if it exists, otherwise Encrypted)
    let mut secrets_map: HashMap<String, AccountSecret> = if is_plaintext {
        let content = fs::read(&dec_path).expect("Failed to read plaintext secrets");
        serde_json::from_slice(&content).expect("Failed to parse plaintext secrets")
    } else if is_encrypted {
        let encrypted = fs::read(&sec_path).expect("Failed to read secrets file");
        match decrypt_with_master_key(&encrypted, &master_key) {
            Ok(decrypted) => serde_json::from_slice(&decrypted).expect("Failed to parse existing secrets JSON"),
            Err(e) => {
                if force_new_vault {
                    println!("\n[Warning] Decryption failed: {}. --force-new-vault is set, discarding existing data.", e);
                    metadata = MetadataFile { accounts: vec![], version: 1 };
                    HashMap::new()
                } else {
                    eprintln!("\n[Error] Master Key incorrect for the existing vault ({}).", e);
                    std::process::exit(101);
                }
            }
        }
    } else {
        HashMap::new()
    };

    // 5. Process Import
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

    // 6. Save back
    let secrets_json = serde_json::to_vec(&secrets_map).unwrap();
    
    // Hybrid state logic: if master_key exists, always encrypt to .age
    if has_master_key {
        let mut should_seal = true;
        if !default_flag && is_plaintext {
            should_seal = interactor.confirm("Master Key exists. Encrypt and delete plaintext vault?", true);
        }
        
        if should_seal {
            let encrypted_data = encrypt_with_master_key(&secrets_json, &master_key).expect("Encryption failed");
            fs::write(&sec_path, encrypted_data).unwrap();
            if dec_path.exists() { let _ = fs::remove_file(&dec_path); }
            if is_plaintext { println!("Vault encrypted and plaintext deleted."); }
            else { println!("Saved to encrypted vault."); }
        } else {
            fs::write(&dec_path, &secrets_json).unwrap();
            println!("Saved to plaintext vault as requested.");
        }
    } else if is_plaintext {
        fs::write(&dec_path, &secrets_json).unwrap();
        println!("Saved to plaintext vault.");
    } else {
        let encrypted_data = encrypt_with_master_key(&secrets_json, &master_key).expect("Encryption failed");
        fs::write(&sec_path, encrypted_data).unwrap();
        println!("Saved to encrypted vault.");
    }

    fs::write(&meta_path, serde_json::to_string_pretty(&metadata).unwrap()).unwrap();

    println!("\nImport completed successfully!");
    println!("  - New: {}, Updated: {}, Skipped: {}", new_count, updated_count, skip_count);
    let _ = jki_core::agent::AgentClient::reload();
}

fn main() {
    let cli = Cli::parse();
    let interactor = TerminalInteractor;
    let mut auth = cli.auth;
    if cli.interactive {
        auth = AuthSource::Interactive;
    }

    match &cli.command {
        Commands::Status => handle_status(),
        Commands::Init { force } => handle_init(*force),
        Commands::Sync => handle_sync(cli.default, &interactor),
        Commands::Edit => handle_edit(),
        Commands::Decrypt { force, keep, remove_key } => handle_decrypt(*force, *keep, *remove_key, cli.default, auth, &interactor),
        Commands::Encrypt { force } => handle_encrypt(*force, cli.default, auth, &interactor),
        Commands::MasterKey(m) => handle_master_key(m, auth, cli.default, &interactor),
        Commands::ImportWinauth { file, overwrite, force_new_vault } =>
            handle_import_winauth(file, *overwrite, auth, cli.default, &interactor, *force_new_vault),
        Commands::Export { output } => handle_export(output, auth, &interactor),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use jki_core::AccountType;
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

        let cmd = MasterKeyCommands::Set { force: false, keychain: false };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec!["newpass".to_string(), "newpass".to_string()]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, AuthSource::Auto, false, &interactor);

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
        handle_master_key(&cmd, AuthSource::Auto, false, &interactor);

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
        
        let cmd = MasterKeyCommands::Remove { force: true, keychain: false };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, AuthSource::Auto, false, &interactor);

        assert!(!home.join("master.key").exists());
    }

    #[test]
    #[serial]
    fn test_handle_init() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home");
        env::set_var("JKI_HOME", &home);

        handle_init(false);

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
        
        handle_init(false);
        
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
        handle_import_winauth(&import_file, false, AuthSource::Auto, true, &interactor, false);

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
        handle_init(false);
        
        // 2. Add some file
        fs::write(home.join("test.txt"), "content").unwrap();
        
        // 3. Sync (should commit)
        let interactor = MockInteractor { passwords: std::cell::RefCell::new(vec![]), confirms: std::cell::RefCell::new(vec![]) };
        handle_sync(false, &interactor);
        
        // 4. Verify commit
        let output = Command::new("git")
            .args(["-C", home.to_str().unwrap(), "log", "-n", "1"])
            .output()
            .unwrap();
        let log = String::from_utf8_lossy(&output.stdout);
        assert!(log.contains("jki backup:"));
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_handle_decrypt_encrypt() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_de_en");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let key_path = home.join("master.key");
        fs::write(&key_path, "testpass").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let sec_path = home.join("vault.secrets.bin.age");
        let dec_path = home.join("vault.secrets.json");
        let secrets_map: HashMap<String, AccountSecret> = HashMap::new();
        let secrets_map_json = serde_json::to_vec(&secrets_map).unwrap();
        let encrypted = encrypt_with_master_key(&secrets_map_json, &secrecy::SecretString::from("testpass".to_string())).unwrap();
        fs::write(&sec_path, encrypted).unwrap();

        let interactor = MockInteractor {
            passwords: RefCell::new(vec!["testpass".to_string()]),
            confirms: RefCell::new(vec![true, false]),
        };
        
        // 1. Decrypt: Delete .age (true), Keep master.key (false)
        handle_decrypt(false, false, false, false, AuthSource::Auto, &interactor);
        assert!(dec_path.exists());
        assert!(!sec_path.exists());
        assert!(key_path.exists());

        // 2. Encrypt (uses master.key, no prompt needed)
        handle_encrypt(false, false, AuthSource::Auto, &interactor);
        assert!(!dec_path.exists());
        assert!(sec_path.exists());
    }

    #[test]
    #[serial]
    fn test_handle_edit() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_edit");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let meta_path = home.join("vault.metadata.json");
        let initial_meta = MetadataFile { accounts: vec![], version: 1 };
        fs::write(&meta_path, serde_json::to_string(&initial_meta).unwrap()).unwrap();

        // Mock EDITOR: a script that replaces content with a new valid JSON
        let editor_script = temp.path().join("mock_editor.sh");
        let new_meta = MetadataFile { 
            accounts: vec![Account {
                id: "1".to_string(),
                name: "new".to_string(),
                issuer: None,
                account_type: AccountType::Standard,
                secret: "".to_string(),
                digits: 6,
                algorithm: "SHA1".to_string(),
            }],
            version: 2 
        };
        let new_json = serde_json::to_string(&new_meta).unwrap();
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::write(&editor_script, format!("#!/bin/sh\necho '{}' > \"$1\"", new_json)).unwrap();
            fs::set_permissions(&editor_script, fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(windows)]
        {
            fs::write(&editor_script, format!("echo {} > %1", new_json)).unwrap();
        }

        env::set_var("EDITOR", &editor_script);

        handle_edit();

        let updated_meta: MetadataFile = serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        assert_eq!(updated_meta.version, 2);
        assert_eq!(updated_meta.accounts.len(), 1);
    }

    #[test]
    #[serial]
    fn test_handle_sync_conflict_resolve() {
        let temp = tempdir().unwrap();
        let remote_path = temp.path().join("remote");
        let local_path = temp.path().join("local");
        
        // 1. Setup remote
        Command::new("git").args(["init", "--bare", "-b", "main", "remote"]).current_dir(temp.path()).output().unwrap();
        
        // 2. Setup local A (to push initial content)
        let local_a = temp.path().join("local_a");
        fs::create_dir_all(&local_a).unwrap();
        Command::new("git").args(["init", "-b", "main"]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["config", "user.email", "a@example.com"]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["config", "user.name", "A"]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["remote", "add", "origin", remote_path.to_str().unwrap()]).current_dir(&local_a).output().unwrap();
        
        fs::write(local_a.join("vault.metadata.json"), "{\"accounts\":[], \"version\":1}").unwrap();
        Command::new("git").args(["add", "."]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["commit", "-m", "initial"]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["push", "origin", "main"]).current_dir(&local_a).output().unwrap();

        // 3. Setup local B (the one we will test sync with)
        fs::create_dir_all(&local_path).unwrap();
        Command::new("git").args(["clone", remote_path.to_str().unwrap(), "."]).current_dir(&local_path).output().unwrap();
        Command::new("git").args(["config", "user.email", "b@example.com"]).current_dir(&local_path).output().unwrap();
        Command::new("git").args(["config", "user.name", "B"]).current_dir(&local_path).output().unwrap();
        env::set_var("JKI_HOME", &local_path);

        // 4. Create conflict: modify remote (via local A)
        fs::write(local_a.join("vault.metadata.json"), "{\"accounts\":[], \"version\":2, \"note\":\"remote\"}").unwrap();
        Command::new("git").args(["add", "."]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["commit", "-m", "remote change"]).current_dir(&local_a).output().unwrap();
        Command::new("git").args(["push", "origin", "main"]).current_dir(&local_a).output().unwrap();

        // 5. Modify local B
        fs::write(local_path.join("vault.metadata.json"), "{\"accounts\":[], \"version\":2, \"note\":\"local\"}").unwrap();
        // (No commit yet, handle_sync will commit)

        // 6. Run handle_sync with conflict resolution (default=true)
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![true]), // Confirm resolution
        };
        handle_sync(false, &interactor);

        // 7. Verify resolution (should prefer local according to handle_sync logic)
        // handle_sync uses checkout --theirs? 
        // Wait, let's check handle_sync logic:
        // git::checkout_theirs(&config_dir, &files)
        // In a rebase, 'theirs' is the branch being rebased onto (the remote), 
        // and 'ours' is the current branch (local).
        // Wait, in `git pull --rebase`, 'ours' is the upstream, 'theirs' is your local changes.
        // So `checkout --theirs` picks LOCAL changes.
        
        let resolved_content = fs::read_to_string(local_path.join("vault.metadata.json")).unwrap();
        assert!(resolved_content.contains("\"local\""));
        assert!(local_path.join("vault.metadata.json.conflict").exists());
    }
}
