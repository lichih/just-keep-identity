use clap::{Parser, Subcommand, CommandFactory};
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
use anyhow::{Context, anyhow};

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
    /// Manage the jki-agent background process
    #[command(subcommand)]
    Agent(AgentCommands),
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
    /// Manage the system keychain
    #[command(subcommand)]
    Keychain(KeychainCommands),
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
    /// Generate shell completions
    Completions {
        /// The shell to generate completions for
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum AgentCommands {
    /// Start the background agent
    Start,
    /// Stop the background agent
    Stop,
    /// Reload the agent (clear cached secrets)
    Reload,
}

#[derive(Subcommand)]
enum MasterKeyCommands {
    /// Save a new master key to disk (0600)
    Set {
        /// Force overwrite without confirmation
        #[arg(short, long)]
        force: bool,
        /// Store the key in the system keychain (default: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::SetTrue)]
        keychain: bool,
        /// Do not store the key in the system keychain
        #[arg(long, overrides_with = "keychain", action = clap::ArgAction::SetFalse)]
        no_keychain: bool,
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

#[derive(Subcommand)]
enum KeychainCommands {
    /// Store a master key directly into the keychain (prompts for input in CLI)
    Set,
    /// Remove the master key from the keychain
    Remove,
    /// Copy the local master.key file INTO the system keychain
    Push,
    /// Copy the master key FROM the system keychain to the local master.key file
    Pull,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct MetadataFile {
    accounts: Vec<Account>,
    version: u32,
}

fn handle_export(output: &Option<PathBuf>, auth: AuthSource, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    if !meta_path.exists() {
        return Err(anyhow!("Metadata not found. Run 'jkim init' first."));
    }

    let master_key = acquire_master_key(auth, interactor, None).map_err(|e| anyhow!("Authentication failed: {}", e))?;

    let meta_content = fs::read_to_string(&meta_path).context("Failed to read metadata")?;
    let metadata: MetadataFile = serde_json::from_str(&meta_content).context("Failed to parse metadata")?;

    let secrets_map: HashMap<String, AccountSecret> = if dec_path.exists() {
        let content = fs::read(&dec_path).context("Failed to read plaintext secrets")?;
        serde_json::from_slice(&content).context("Failed to parse plaintext secrets")?
    } else if sec_path.exists() {
        let encrypted = fs::read(&sec_path).context("Failed to read secrets file")?;
        let decrypted = decrypt_with_master_key(&encrypted, &master_key).map_err(|e| anyhow!(e))?;
        serde_json::from_slice(&decrypted).context("Failed to parse existing secrets JSON")?
    } else {
        return Err(anyhow!("Secrets not found."));
    };

    let (integrated, missing) = jki_core::integrate_accounts(metadata.accounts, &secrets_map);
    if !missing.is_empty() {
        eprintln!("Warning: Some accounts are missing secrets: {:?}", missing);
    }

    let export_pass = interactor.prompt_password("Enter EXPORT Password (for ZIP encryption)").map_err(|e| anyhow!(e))?;
    let export_pass_confirm = interactor.prompt_password("Confirm EXPORT Password").map_err(|e| anyhow!(e))?;
    if export_pass.expose_secret() != export_pass_confirm.expose_secret() {
        return Err(anyhow!("Passwords do not match."));
    }

    let output_path = output.clone().unwrap_or_else(|| {
        let now = chrono::Local::now();
        PathBuf::from(format!("export_{}.zip", now.format("%Y%m%d_%H%M")))
    });

    let file = fs::File::create(&output_path).context("Failed to create export file")?;
    let mut zip = zip::ZipWriter::new(file);

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .with_aes_encryption(AesMode::Aes256, export_pass.expose_secret());

    zip.start_file("accounts.txt", options).context("Failed to start file in ZIP")?;
    
    for acc in integrated {
        let uri = acc.to_otpauth_uri();
        zip.write_all(uri.as_bytes()).context("Failed to write to ZIP")?;
        zip.write_all(b"\n").context("Failed to write newline to ZIP")?;
    }

    zip.finish().context("Failed to finalize ZIP")?;
    println!("Export completed successfully: {:?}", output_path);
    Ok(())
}

fn handle_agent(cmd: &AgentCommands) -> anyhow::Result<()> {
    use jki_core::agent::AgentClient;
    match cmd {
        AgentCommands::Start => {
            if AgentClient::ping() {
                println!("jki-agent is already running.");
            } else {
                if jki_core::ensure_agent_running(false) {
                    println!("jki-agent started successfully.");
                } else {
                    return Err(anyhow!("Failed to start jki-agent."));
                }
            }
        }
        AgentCommands::Stop => {
            if AgentClient::ping() {
                AgentClient::shutdown().map_err(|e| anyhow!("Failed to shut down jki-agent: {}", e))?;
                println!("jki-agent shut down successfully.");
            } else {
                println!("jki-agent is not running.");
            }
        }
        AgentCommands::Reload => {
            if AgentClient::ping() {
                AgentClient::reload().map_err(|e| anyhow!("Failed to reload jki-agent: {}", e))?;
                println!("jki-agent reloaded.");
            } else {
                println!("jki-agent is not running.");
            }
        }
    }
    Ok(())
}

fn handle_status() -> anyhow::Result<()> {
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
    Ok(())
}

fn handle_master_key(cmd: &MasterKeyCommands, auth: AuthSource, default_flag: bool, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let key_path = JkiPath::master_key_path();
    let sec_path = JkiPath::secrets_path();

    match cmd {
        MasterKeyCommands::Set { force, keychain, no_keychain: _ } => {
            if !*force && key_path.exists() {
                if !default_flag && !interactor.confirm(&format!("Warning: master.key already exists at {:?}", key_path), false) { return Ok(()); }
            }
            if !*force && sec_path.exists() {
                println!("CRITICAL WARNING: vault.secrets.bin.age already exists.");
                println!("If the new key doesn't match the one used to encrypt it, you will LOSE ACCESS to your secrets.");
                if !default_flag && !interactor.confirm("Proceed anyway?", false) { return Ok(()); }
            }

            let p1 = interactor.prompt_password("Enter new Master Key").map_err(|e| anyhow!(e))?;
            let p2 = interactor.prompt_password("Confirm Master Key").map_err(|e| anyhow!(e))?;
            if p1.expose_secret() != p2.expose_secret() {
                return Err(anyhow!("Passwords do not match."));
            }

            fs::write(&key_path, p1.expose_secret()).context("Failed to write key")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).context("Failed to set key permissions")?;
            }
            println!("Master Key saved to {:?}", key_path);

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
                    if !default_flag && !interactor.confirm("Warning: Removing master.key from disk. Are you sure?", false) { return Ok(()); }
                }
                fs::remove_file(&key_path).context("Failed to remove key")?;
                println!("Master Key removed from disk.");
                removed = true;
            }

            if *keychain {
                if !*force && !removed {
                    if !default_flag && !interactor.confirm("Warning: Removing master_key from system keychain. Are you sure?", false) { return Ok(()); }
                }
                if let Err(e) = KeyringStore.delete_secret("jki", "master_key") {
                    eprintln!("Warning: Failed to remove from system keychain: {}", e);
                } else {
                    println!("Master Key removed from system keychain.");
                }
            }
        }
        MasterKeyCommands::Change { commit } => {
            let mut current_key = acquire_master_key(auth, interactor, None)
                .unwrap_or_else(|_| interactor.prompt_password("Enter current Master Key").expect("Input failed"));
            
            let mut secrets_data = None;
            if sec_path.exists() {
                let encrypted = fs::read(&sec_path).context("Failed to read secrets")?;
                match decrypt_with_master_key(&encrypted, &current_key) {
                    Ok(d) => secrets_data = Some(d),
                    Err(_) => {
                        println!("Stored Master Key failed to decrypt vault.");
                        current_key = interactor.prompt_password("Enter CORRECT current Master Key").map_err(|e| anyhow!(e))?;
                        secrets_data = Some(decrypt_with_master_key(&encrypted, &current_key).map_err(|e| anyhow!("Authentication failed: {}", e))?);
                    }
                }
            } else {
                println!("No existing vault found. This is equivalent to 'set'.");
            }

            let p1 = interactor.prompt_password("Enter NEW Master Key").map_err(|e| anyhow!(e))?;
            let p2 = interactor.prompt_password("Confirm NEW Master Key").map_err(|e| anyhow!(e))?;
            if p1.expose_secret() != p2.expose_secret() {
                return Err(anyhow!("Passwords do not match."));
            }

            let key_tmp = key_path.with_extension("tmp");
            let sec_tmp = sec_path.with_extension("tmp");

            fs::write(&key_tmp, p1.expose_secret()).context("Failed to write temp key")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&key_tmp, fs::Permissions::from_mode(0o600)).context("Failed to set temp key permissions")?;
            }

            if let Some(data) = secrets_data {
                let encrypted = encrypt_with_master_key(&data, &p1).map_err(|e| anyhow!("Encryption failed: {}", e))?;
                fs::write(&sec_tmp, encrypted).context("Failed to write temp secrets")?;
            }

            if sec_tmp.exists() { fs::rename(&sec_tmp, &sec_path).context("Failed to replace secrets")?; }
            fs::rename(&key_tmp, &key_path).context("Failed to replace key")?;

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
                let _ = git::add_all(&config_dir);
                let _ = git::commit(&config_dir, "jki: master key rotation");
                println!("Changes committed to Git.");
            } else {
                println!("Note: You may want to run 'jkim sync' to backup your new encrypted vault.");
            }
        }
    }
    Ok(())
}

fn handle_keychain(cmd: &KeychainCommands, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let key_path = JkiPath::master_key_path();

    match cmd {
        KeychainCommands::Set => {
            let p1 = interactor.prompt_password("Enter Master Key to store in keychain").map_err(|e| anyhow!(e))?;
            let p2 = interactor.prompt_password("Confirm Master Key").map_err(|e| anyhow!(e))?;
            
            if p1.expose_secret() != p2.expose_secret() {
                return Err(anyhow!("Passwords do not match."));
            }

            KeyringStore.set_secret("jki", "master_key", p1.expose_secret()).map_err(|e| anyhow!("Failed to save to system keychain: {}", e))?;
            println!("Master Key saved to system keychain successfully.");
        }
        KeychainCommands::Remove => {
            KeyringStore.delete_secret("jki", "master_key").map_err(|e| anyhow!("Failed to remove from system keychain: {}", e))?;
            println!("Master Key removed from system keychain successfully.");
        }
        KeychainCommands::Push => {
            if !key_path.exists() {
                return Err(anyhow!("Error: master.key file not found at {:?}.", key_path));
            }
            let content = fs::read_to_string(&key_path).context("Failed to read master.key")?;
            KeyringStore.set_secret("jki", "master_key", content.trim()).map_err(|e| anyhow!("Failed to push to system keychain: {}", e))?;
            println!("Master Key pushed from {:?} to system keychain successfully.", key_path);
        }
        KeychainCommands::Pull => {
            match KeyringStore.get_secret("jki", "master_key") {
                Ok(secret) => {
                    fs::write(&key_path, secret.expose_secret()).context("Failed to write master.key")?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).context("Failed to set key permissions")?;
                    }
                    println!("Master Key pulled from system keychain to {:?} successfully.", key_path);
                }
                Err(e) => {
                    return Err(anyhow!("Error: Failed to pull from system keychain: {}", e));
                }
            }
        }
    }
    Ok(())
}

fn handle_sync(default_flag: bool, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let config_dir = JkiPath::home_dir();
    println!("Syncing JKI Home at {:?}...", config_dir);

    let status = match git::check_status(&config_dir) {
        Some(s) => s,
        None => {
            return Err(anyhow!("Error: Not a git repository. Run 'jkim init' first."));
        }
    };

    println!("  - Stage changes...");
    git::add_all(&config_dir).context("Failed to add files")?;

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
                        git::checkout_theirs(&config_dir, &files).context("Failed to resolve")?;
                        git::add(&config_dir, &files).context("Failed to add resolved files")?;
                        git::rebase_continue(&config_dir).context("Failed to continue rebase")?;
                        println!("  - Conflicts resolved and rebase completed.");
                    }
                    Ok(_) => {
                        return Err(anyhow!("Pull failed but no conflicts detected. Resolve manually."));
                    }
                    Err(e) => {
                        return Err(anyhow!("Error: Failed to get conflicting files: {}", e));
                    }
                }
            } else {
                return Err(anyhow!("Manual resolution required. Run 'git status' to see conflicts."));
            }
        }

        println!("  - Push...");
        git::push(&config_dir).context("Push failed")?;
        println!("Sync completed successfully!");
        let _ = jki_core::agent::AgentClient::reload();
    } else {
        println!("No remote configured. Local backup complete.");
    }
    Ok(())
}

fn handle_edit() -> anyhow::Result<()> {
    let meta_path = JkiPath::metadata_path();
    if !meta_path.exists() {
        return Err(anyhow!("Error: Metadata not found. Run 'jkim init' first."));
    }

    let mut temp_file = tempfile::Builder::new()
        .prefix("jki-metadata-")
        .suffix(".tmp.json")
        .tempfile()
        .context("Failed to create temporary file")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(temp_file.path()).unwrap().permissions();
        perms.set_mode(0o600);
        fs::set_permissions(temp_file.path(), perms).context("Failed to set temp file permissions")?;
    }

    let content = fs::read_to_string(&meta_path).context("Failed to read metadata")?;
    temp_file.write_all(content.as_bytes()).context("Failed to write to temporary file")?;
    temp_file.flush().context("Failed to flush temporary file")?;

    let editor = env::var("EDITOR").unwrap_or_else(|_| {
        if cfg!(windows) { "notepad.exe".to_string() } else { "vi".to_string() }
    });

    println!("Opening metadata with {}...", editor);
    let status = Command::new(&editor)
        .arg(temp_file.path())
        .status()
        .context(format!("Failed to launch editor '{}'", editor))?;

    if status.success() {
        let mut new_content = String::new();
        temp_file.reopen().context("Failed to reopen temp file")?
            .read_to_string(&mut new_content).context("Failed to read back metadata from temp file")?;

        match serde_json::from_str::<MetadataFile>(&new_content) {
            Ok(_) => {
                fs::write(&meta_path, &new_content).context("Failed to write back metadata")?;
                println!("Metadata updated and validated successfully.");
                let _ = jki_core::agent::AgentClient::reload();
            }
            Err(e) => {
                eprintln!("\nERROR: Metadata contains JSON syntax errors: {}", e);
                eprintln!("The changes have NOT been applied.");
                eprintln!("Your edited content is preserved at: {:?}", temp_file.path());
                let (file, path) = temp_file.keep().context("Failed to preserve temp file")?;
                drop(file);
                drop(path);
            }
        }
    } else {
        return Err(anyhow!("Editor exited with error: {}", status));
    }
    Ok(())
}

fn handle_decrypt(force: bool, keep: bool, remove_key: bool, default_flag: bool, auth: AuthSource, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();
    let key_path = JkiPath::master_key_path();

    if !sec_path.exists() {
        return Err(anyhow!("Error: Encrypted vault not found at {:?}", sec_path));
    }

    if dec_path.exists() && !force {
        if !default_flag && !interactor.confirm(&format!("Warning: Plaintext vault already exists at {:?}. Overwrite?", dec_path), false) {
            return Ok(());
        }
    }

    let master_key = acquire_master_key(auth, interactor, None).map_err(|e| anyhow!("Authentication failed: {}", e))?;
    let encrypted = fs::read(&sec_path).context("Failed to read secrets")?;
    let decrypted = decrypt_with_master_key(&encrypted, &master_key).map_err(|e| anyhow!("Decryption failed: {}", e))?;

    fs::write(&dec_path, &decrypted).context("Failed to write plaintext vault")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dec_path, fs::Permissions::from_mode(0o600)).context("Failed to set vault permissions")?;
    }
    println!("Vault decrypted to plaintext at {:?}", dec_path);
    println!("Note: jki will now use this for zero-latency lookups.");

    if !keep {
        if default_flag || interactor.confirm("Delete encrypted source (.age)?", true) {
            fs::remove_file(&sec_path).context("Failed to delete encrypted source")?;
            println!("Encrypted source deleted.");
        }
    }

    if remove_key {
        if key_path.exists() {
            fs::remove_file(&key_path).context("Failed to delete master.key")?;
            println!("Master Key file removed.");
        }
    } else if key_path.exists() && !default_flag {
        if interactor.confirm("Delete master key file?", false) {
            fs::remove_file(&key_path).context("Failed to delete master.key")?;
            println!("Master Key file removed.");
        }
    }
    Ok(())
}

fn handle_encrypt(force: bool, default_flag: bool, auth: AuthSource, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    if !dec_path.exists() {
        return Err(anyhow!("Error: Plaintext vault not found at {:?}", dec_path));
    }

    if sec_path.exists() && !force {
        if !default_flag && !interactor.confirm(&format!("Warning: Encrypted vault already exists at {:?}. Overwrite?", sec_path), false) {
            return Ok(());
        }
    }

    let master_key = acquire_master_key(auth, interactor, None).map_err(|e| anyhow!("Authentication failed: {}", e))?;
    let decrypted = fs::read(&dec_path).context("Failed to read plaintext secrets")?;
    let encrypted = encrypt_with_master_key(&decrypted, &master_key).map_err(|e| anyhow!("Encryption failed: {}", e))?;

    fs::write(&sec_path, encrypted).context("Failed to write encrypted vault")?;
    fs::remove_file(&dec_path).context("Failed to delete plaintext vault after encryption")?;
    println!("Vault encrypted to {:?}", sec_path);
    println!("Plaintext vault physically deleted.");
    Ok(())
}

fn handle_init(force: bool) -> anyhow::Result<()> {
    let config_dir = JkiPath::home_dir();
    println!("Initializing JKI Home at {:?}...", config_dir);

    if force {
        println!("\n[Force Reset]");
        let meta = JkiPath::metadata_path();
        let sec = JkiPath::secrets_path();
        if meta.exists() { let _ = fs::remove_file(&meta); println!("  - Metadata: Deleted."); }
        if sec.exists() { let _ = fs::remove_file(&sec); println!("  - Secrets:  Deleted."); }
    }

    print!("  - Directory: ");
    if !config_dir.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).recursive(true).create(&config_dir).context("Failed to create config directory")?;
        }
        #[cfg(windows)]
        {
            fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        }
        println!("Created.");
    } else {
        println!("Already exists. (Skipped)");
    }

    print!("  - Git Repo:  ");
    if !config_dir.join(".git").exists() {
        let status = Command::new("git").args(["init", "-b", "main"]).current_dir(&config_dir).status().context("Failed to init git")?;
        if status.success() { println!("Initialized."); } else { println!("FAILED to initialize."); }
    } else {
        println!("Already initialized. (Skipped)");
    }

    print!("  - Config:    ");
    let gitignore_path = config_dir.join(".gitignore");
    let _ = fs::write(gitignore_path, "# JKI\nmaster.key\nvault.json\n*.txt\n*.bin\n");
    let gitattrs_path = config_dir.join(".gitattributes");
    let _ = fs::write(gitattrs_path, "vault.secrets.bin.age binary\nvault.metadata.json filter=age\n");
    println!(".gitignore and .gitattributes written. (Updated)");

    let meta_exists = JkiPath::metadata_path().exists();
    let sec_exists = JkiPath::secrets_path().exists();
    if meta_exists || sec_exists {
        println!("\n[Data Warning]");
        if sec_exists { println!("  - Existing vault data (vault.secrets.bin.age) detected."); }
        println!("  - Subsequent imports will attempt to MERGE using your Master Key.");
        println!("  - To start fresh, use 'jkim init --force' or delete vault.* manually.");
    }

    println!("\nInitialization complete!");
    Ok(())
}

fn handle_import_winauth(file: &PathBuf, overwrite: bool, auth: AuthSource, default_flag: bool, interactor: &dyn Interactor, force_new_vault: bool) -> anyhow::Result<()> {
    if !file.exists() { return Err(anyhow!("Error: File not found.")); }

    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    let has_meta = meta_path.exists();
    let has_age = sec_path.exists();
    let has_json = dec_path.exists();

    if has_meta && !has_age && !has_json {
        return Err(anyhow!("Error: Vault corrupted: Metadata exists but secrets are missing."));
    }

    let master_key = acquire_master_key(auth, interactor, None).ok();

    let mut metadata = if has_meta {
        let content = fs::read_to_string(&meta_path).unwrap_or_default();
        serde_json::from_str::<MetadataFile>(&content).unwrap_or(MetadataFile { accounts: vec![], version: 1 })
    } else {
        MetadataFile { accounts: vec![], version: 1 }
    };

    let mut secrets_map: HashMap<String, AccountSecret> = match (has_age, has_json) {
        (true, _) => {
            let k = master_key.clone().ok_or_else(|| anyhow!("Authentication required for encrypted vault."))?;
            let encrypted = fs::read(&sec_path).context("Failed to read secrets file")?;
            match decrypt_with_master_key(&encrypted, &k) {
                Ok(decrypted) => serde_json::from_slice(&decrypted).context("Failed to parse existing secrets JSON")?,
                Err(e) => {
                    if force_new_vault {
                        println!("\n[Warning] Decryption failed: {}. --force-new-vault is set, discarding existing data.", e);
                        metadata = MetadataFile { accounts: vec![], version: 1 };
                        HashMap::new()
                    } else {
                        return Err(anyhow!("\n[Error] Master Key incorrect for the existing vault ({}).", e));
                    }
                }
            }
        },
        (false, true) => {
            let content = fs::read(&dec_path).context("Failed to read plaintext secrets")?;
            serde_json::from_slice(&content).context("Failed to parse plaintext secrets")?
        },
        (false, false) => {
            HashMap::new()
        }
    };

    let content = fs::read_to_string(file).context("Failed to read file")?;
    let mut new_count = 0;
    let mut updated_count = 0;
    let mut skip_count = 0;

    let mut secret_to_id: HashMap<String, String> = HashMap::new();
    for (id, sec) in &secrets_map {
        secret_to_id.insert(sec.secret.clone(), id.clone());
    }

    for line in content.lines() {
        if let Some(mut acc) = parse_otpauth_uri(line) {
            let existing_pos = metadata.accounts.iter().position(|m| m.name == acc.name && m.issuer == acc.issuer)
                .or_else(|| {
                    secret_to_id.get(&acc.secret).and_then(|id| {
                        metadata.accounts.iter().position(|m| m.id == *id)
                    })
                });
            
            if let Some(pos) = existing_pos {
                let id = metadata.accounts[pos].id.clone();
                if !overwrite { skip_count += 1; continue; }
                let entry = AccountSecret { secret: acc.secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                acc.id = id.clone();
                acc.secret = "".to_string();
                metadata.accounts[pos] = acc;
                secrets_map.insert(id, entry);
                updated_count += 1;
            } else {
                let id = acc.id.clone();
                let entry = AccountSecret { secret: acc.secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                acc.id = id.clone();
                acc.secret = "".to_string();
                if let Some(_) = secret_to_id.get(&entry.secret) { skip_count += 1; continue; }
                metadata.accounts.push(acc);
                secrets_map.insert(id.clone(), entry.clone());
                secret_to_id.insert(entry.secret, id);
                new_count += 1;
            }
        }
    }

    let secrets_json = serde_json::to_vec(&secrets_map).context("Failed to serialize secrets")?;
    match (has_age, has_json) {
        (true, _) => {
            let k = master_key.ok_or_else(|| anyhow!("Already verified key above"))?;
            let encrypted_data = encrypt_with_master_key(&secrets_json, &k).map_err(|e| anyhow!("Encryption failed: {}", e))?;
            fs::write(&sec_path, encrypted_data).context("Failed to write encrypted vault")?;
            println!("Saved to encrypted vault.");
        },
        (false, true) => {
            if let Some(k) = master_key {
                if !default_flag && interactor.confirm("Master Key detected. Upgrade to Encrypted? [y/N]", false) {
                    let encrypted_data = encrypt_with_master_key(&secrets_json, &k).map_err(|e| anyhow!("Encryption failed: {}", e))?;
                    fs::write(&sec_path, encrypted_data).context("Failed to write encrypted vault")?;
                    let _ = fs::remove_file(&dec_path);
                    println!("Vault upgraded to encrypted and plaintext deleted.");
                } else {
                    fs::write(&dec_path, &secrets_json).context("Failed to write plaintext vault")?;
                    println!("Updated plaintext vault.");
                }
            } else {
                fs::write(&dec_path, &secrets_json).context("Failed to write plaintext vault")?;
                println!("Updated plaintext vault.");
            }
        },
        (false, false) => {
            if let Some(k) = master_key {
                let encrypted_data = encrypt_with_master_key(&secrets_json, &k).map_err(|e| anyhow!("Encryption failed: {}", e))?;
                fs::write(&sec_path, encrypted_data).context("Failed to write encrypted vault")?;
                println!("Created encrypted vault.");
            } else {
                if !default_flag && interactor.confirm("No Key. Create as PLAINTEXT vault? [y/n]", false) {
                    fs::write(&dec_path, &secrets_json).context("Failed to write plaintext vault")?;
                    println!("Created plaintext vault.");
                } else {
                    println!("Import cancelled (No Key provided and declined plaintext).");
                    return Ok(());
                }
            }
        }
    }

    fs::write(&meta_path, serde_json::to_string_pretty(&metadata).unwrap()).context("Failed to write metadata")?;
    println!("\nImport completed successfully!");
    println!("  - New: {}, Updated: {}, Skipped: {}", new_count, updated_count, skip_count);
    let _ = jki_core::agent::AgentClient::reload();
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let interactor = TerminalInteractor;
    let mut auth = cli.auth;
    if cli.interactive { auth = AuthSource::Interactive; }

    match &cli.command {
        Commands::Status => handle_status()?,
        Commands::Agent(a) => handle_agent(a)?,
        Commands::Init { force } => handle_init(*force)?,
        Commands::Sync => handle_sync(cli.default, &interactor)?,
        Commands::Edit => handle_edit()?,
        Commands::Decrypt { force, keep, remove_key } => handle_decrypt(*force, *keep, *remove_key, cli.default, auth, &interactor)?,
        Commands::Encrypt { force } => handle_encrypt(*force, cli.default, auth, &interactor)?,
        Commands::MasterKey(m) => handle_master_key(m, auth, cli.default, &interactor)?,
        Commands::Keychain(k) => handle_keychain(k, &interactor)?,
        Commands::ImportWinauth { file, overwrite, force_new_vault } =>
            handle_import_winauth(file, *overwrite, auth, cli.default, &interactor, *force_new_vault)?,
        Commands::Export { output } => handle_export(output, auth, &interactor)?,
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            clap_complete::generate(*shell, &mut cmd, bin_name, &mut std::io::stdout());
        }
    }
    Ok(())
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

        let cmd = MasterKeyCommands::Set { force: false, keychain: false, no_keychain: true };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec!["newpass".to_string(), "newpass".to_string()]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, AuthSource::Auto, false, &interactor).unwrap();

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

        let cmd = MasterKeyCommands::Change { commit: false };
        let interactor = MockInteractor {
            passwords: RefCell::new(vec!["newpass".to_string(), "newpass".to_string()]),
            confirms: RefCell::new(vec![]),
        };
        handle_master_key(&cmd, AuthSource::Auto, false, &interactor).unwrap();

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
        handle_master_key(&cmd, AuthSource::Auto, false, &interactor).unwrap();

        assert!(!home.join("master.key").exists());
    }

    #[test]
    #[serial]
    fn test_handle_init() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home");
        env::set_var("JKI_HOME", &home);

        handle_init(false).unwrap();

        assert!(home.exists());
        assert!(home.join(".git").exists());
        assert!(home.join(".gitignore").exists());
        assert!(home.join(".gitattributes").exists());
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

        let key_path = home.join("master.key");
        fs::write(&key_path, "testpass").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let import_file = temp.path().join("winauth.txt");
        fs::write(&import_file, "otpauth://totp/Google:test@gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google\n").unwrap();

        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };
        handle_import_winauth(&import_file, false, AuthSource::Auto, true, &interactor, false).unwrap();

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
    #[cfg(unix)]
    fn test_import_hardening_logic() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_hardening");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let import_file = temp.path().join("winauth.txt");
        fs::write(&import_file, "otpauth://totp/Google:test@gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google\n").unwrap();

        let key_path = home.join("master.key");
        fs::write(&key_path, "testpass").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]), 
        };
        handle_import_winauth(&import_file, false, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.bin.age").exists());
        assert!(!home.join("vault.secrets.json").exists());
        fs::remove_file(home.join("vault.secrets.bin.age")).unwrap();
        fs::remove_file(home.join("vault.metadata.json")).unwrap();

        fs::remove_file(&key_path).unwrap();
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![true]), 
        };
        handle_import_winauth(&import_file, false, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.json").exists());
        assert!(!home.join("vault.secrets.bin.age").exists());

        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]), 
        };
        handle_import_winauth(&import_file, true, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.json").exists());

        fs::write(&key_path, "testpass").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![true]), 
        };
        handle_import_winauth(&import_file, true, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.bin.age").exists());
        assert!(!home.join("vault.secrets.json").exists());

        let interactor = MockInteractor {
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]), 
        };
        handle_import_winauth(&import_file, true, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.bin.age").exists());
    }

    #[test]
    #[serial]
    fn test_handle_sync() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_sync");
        env::set_var("JKI_HOME", &home);

        handle_init(false).unwrap();
        
        fs::write(home.join("test.txt"), "content").unwrap();
        
        let interactor = MockInteractor { passwords: std::cell::RefCell::new(vec![]), confirms: std::cell::RefCell::new(vec![]) };
        handle_sync(false, &interactor).unwrap();
        
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
        
        handle_decrypt(false, false, false, false, AuthSource::Auto, &interactor).unwrap();
        assert!(dec_path.exists());
        assert!(!sec_path.exists());
        assert!(key_path.exists());

        handle_encrypt(false, false, AuthSource::Auto, &interactor).unwrap();
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

        handle_edit().unwrap();

        let updated_meta: MetadataFile = serde_json::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        assert_eq!(updated_meta.version, 2);
        assert_eq!(updated_meta.accounts.len(), 1);
    }
}
