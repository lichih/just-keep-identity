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
    find_duplicate_groups,
    keychain::{KeyringStore, SecretStore},
    AuthSource,
    JkiPathExt,
    MetadataFile,
};
use secrecy::ExposeSecret;
use std::collections::HashMap;
use console::style;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::env;
use std::io::{Read, Write, stdout};
use std::time::Duration;
use crossterm::{
    execute,
    terminal,
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::AesMode;
use anyhow::{Context, anyhow};

pub mod assets;
use assets::AssetId;

#[derive(Parser)]
#[command(name = "jkim", version, about = "JK Suite Management Hub")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Authentication and data source
    #[arg(short = 'A', long, global = true, default_value = "auto")]
    pub auth: AuthSource,

    /// Force interactive master key input (alias for --auth interactive)
    #[arg(short = 'I', long, global = true)]
    pub interactive: bool,

    /// Apply recommended default decisions for all prompts
    #[arg(short, long, global = true)]
    pub default: bool,

    /// Suppress non-critical output
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Check the health and status of the JKI system
    Status,
    /// Add a new account manually
    Add {
        /// Account name (e.g., email or username)
        name: Option<String>,
        /// Issuer name (e.g., Google, GitHub)
        issuer: Option<String>,
        /// Base32 encoded secret
        #[arg(short = 's', long)]
        secret: Option<String>,
        /// Import from an otpauth:// URI
        #[arg(long)]
        uri: Option<String>,
        /// Overwrite if name and issuer already exist
        #[arg(short, long)]
        force: bool,
        /// Show the secret and OTPAuth URI after adding
        #[arg(short = 'S', long)]
        show_secret: bool,
        /// Direct OTP output to stdout only (disables clipboard during handshake)
        #[arg(long)]
        stdout: bool,
    },
    /// Git repository operations (init, sync)
    #[command(subcommand)]
    Git(GitCommands),
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
    /// Configuration and data integrity checks
    #[command(subcommand)]
    Config(ConfigCommands),
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
        /// Path to write the script to. Use "-o -" for stdout.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Display the JKI user manual
    Man,
    /// Sync changes to Git (shortcut for git sync)
    Sync,
    /// Deduplicate accounts by comparing decrypted secrets
    Dedupe {
        /// Keep specific accounts by index (comma-separated, e.g., 1,3,5)
        #[arg(short, long, value_delimiter = ',')]
        keep: Vec<usize>,
        /// Discard specific accounts by index (comma-separated, e.g., 2,4,6)
        #[arg(short, long, value_delimiter = ',')]
        discard: Vec<usize>,
        /// Automatically accept the deletion of shadow entries when using --keep
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum GitCommands {
    /// Initialize the JKI home directory and Git repository
    Init {
        /// Force reset by deleting existing vault data
        #[arg(short, long)]
        force: bool,
    },
    /// Sync changes (add, commit, pull --rebase, push)
    Sync,
}

#[derive(Subcommand)]
pub enum AgentCommands {
    /// Start the background agent
    Start,
    /// Stop the background agent
    Stop,
    /// Reload the agent (clear cached secrets)
    Reload,
}

#[derive(Subcommand)]
pub enum MasterKeyCommands {
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
pub enum KeychainCommands {
    /// Store a master key directly into the keychain (prompts for input in CLI)
    Set,
    /// Remove the master key from the keychain
    Remove,
    /// Copy the local master.key file INTO the system keychain
    Push,
    /// Copy the master key FROM the system keychain to the local master.key file
    Pull,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Check vault configuration and data integrity
    Check,
}

fn handle_config(cmd: &ConfigCommands, auth: AuthSource, interactor: &dyn Interactor) -> anyhow::Result<()> {
    match cmd {
        ConfigCommands::Check => {
            println!("--- JKI Vault Integrity Check ---\n");

            let meta_path = JkiPath::metadata_path();
            let sec_path = JkiPath::secrets_path();
            let dec_path = JkiPath::decrypted_secrets_path();

            let mut has_errors = false;

            // 1. Metadata Verification
            print!("Checking metadata YAML... ");
            let metadata = match MetadataFile::load() {
                Ok(m) => {
                    println!("OK ({} accounts)", m.accounts.len());
                    m
                },
                Err(e) => {
                    println!("ERROR");
                    eprintln!("  -> {}", e);
                    return Err(anyhow!("Metadata validation failed."));
                }
            };

            // 2. Security (Permissions)
            print!("Checking file permissions... ");
            let mut perm_issues = vec![];

            if let Err(e) = meta_path.check_secure_permissions() {
                perm_issues.push(format!("Metadata: {}", e));
            }

            if sec_path.exists() {
                if let Err(e) = sec_path.check_secure_permissions() {
                    perm_issues.push(format!("Encrypted Secrets: {}", e));
                }
            } else if dec_path.exists() {
                if let Err(e) = dec_path.check_secure_permissions() {
                    perm_issues.push(format!("Plaintext Secrets: {}", e));
                }
            }

            if perm_issues.is_empty() {
                println!("OK");
            } else {
                println!("WARNING");
                for issue in perm_issues {
                    eprintln!("  -> {}", issue);
                }
                has_errors = true;
            }

            // 3. Consistency (ID Matching)
            print!("Checking data consistency (ID mapping)... ");

            let master_key = acquire_master_key(auth, interactor, None).ok();

            let secrets_map: Option<HashMap<String, AccountSecret>> = if dec_path.exists() {
                fs::read(&dec_path).ok().and_then(|c| serde_json::from_slice(&c).ok())
            } else if sec_path.exists() {
                if let Some(k) = master_key {
                    if let Ok(encrypted) = fs::read(&sec_path) {
                        if let Ok(decrypted) = decrypt_with_master_key(&encrypted, &k) {
                            serde_json::from_slice(&decrypted).ok()
                        } else { None }
                    } else { None }
                } else { None }
            } else {
                None
            };

            if let Some(secrets) = secrets_map {
                let mut missing_secrets = vec![];
                for acc in &metadata.accounts {
                    if !secrets.contains_key(&acc.id) {
                        missing_secrets.push(format!("{}:{} (ID: {})", acc.issuer.as_deref().unwrap_or("None"), acc.name, acc.id));
                    }
                }

                if missing_secrets.is_empty() {
                    println!("OK");
                } else {
                    println!("ERROR");
                    eprintln!("  -> Found {} orphaned accounts in metadata without a corresponding secret:", missing_secrets.len());
                    for m in missing_secrets {
                        eprintln!("     - {}", m);
                    }
                    has_errors = true;
                }
            } else {
                println!("SKIPPED (Authentication required or secrets unreadable)");
            }

            if has_errors {
                println!("\nCheck completed with ERRORS.");
            } else {
                println!("\nAll checks passed successfully.");
            }
        }
    }
    Ok(())
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
    let metadata: MetadataFile = serde_yaml::from_str(&meta_content).context("Failed to parse metadata")?;

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
        match key_path.check_secure_permissions() {
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

    AssetId::GuideStatus.render();
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
                handle_git(default_flag, interactor)?;
                println!("Changes committed to Git.");
            } else {
                println!("Note: You may want to run 'jkim git sync' to backup your new encrypted vault.");
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

fn handle_git(default_flag: bool, interactor: &dyn Interactor) -> anyhow::Result<()> {
    let config_dir = JkiPath::home_dir();
    println!("--- Git Synchronization ---\n");
    println!("Syncing JKI Home at {:?}...", config_dir);

    let status = match git::check_status(&config_dir) {
        Some(s) => s,
        None => {
            return Err(anyhow!("Error: Not a git repository. Run 'jkim init' first."));
        }
    };

    // 1. Check for plaintext secrets and try auto-encryption
    let plaintext_path = JkiPath::decrypted_secrets_path();
    let encrypted_path = JkiPath::secrets_path();

    if plaintext_path.exists() {
        println!("  - Detected plaintext secrets ({:?})...", plaintext_path.file_name().unwrap_or_default());
        let store = KeyringStore;
        match acquire_master_key(AuthSource::Auto, interactor, Some(&store)) {
            Ok(key) => {
                println!("  - Master key acquired. Auto-encrypting...");
                let data = fs::read(&plaintext_path).context("Failed to read plaintext secrets")?;
                let encrypted = encrypt_with_master_key(&data, &key).context("Encryption failed")?;
                fs::write(&encrypted_path, encrypted).context("Failed to write encrypted secrets")?;
                fs::remove_file(&plaintext_path).context("Failed to remove plaintext secrets")?;
                println!("  - Encrypted and secured vault secrets.");
            }
            Err(_) => {
                eprintln!("{}", style("  ! Warning: Plaintext secrets found but could not be auto-encrypted.").yellow().bold());
                eprintln!("    Only encrypted vault secrets are allowed in synchronization.");
            }
        }
    }

    println!("  - Stage changes...");
    let mut files_to_stage = Vec::new();

    if config_dir.join("vault.metadata.yaml").exists() {
        files_to_stage.push("vault.metadata.yaml".to_string());
    }

    if encrypted_path.exists() {
        files_to_stage.push("vault.secrets.bin.age".to_string());
    }

    if config_dir.join("config.yaml").exists() {
        files_to_stage.push("config.yaml".to_string());
    }

    if config_dir.join(".gitignore").exists() {
        files_to_stage.push(".gitignore".to_string());
    }

    if files_to_stage.is_empty() {
        println!("  - No files to stage.");
    } else {
        git::add(&config_dir, &files_to_stage).context("Failed to stage files")?;
    }

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

        match serde_yaml::from_str::<MetadataFile>(&new_content) {
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
    let _ = fs::write(gitattrs_path, "vault.secrets.bin.age binary\nvault.metadata.yaml filter=age\n");
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
        serde_yaml::from_str::<MetadataFile>(&content).unwrap_or(MetadataFile { accounts: vec![], version: 1 })
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

    let mut secret_to_ids: HashMap<String, Vec<String>> = HashMap::new();
    for (id, sec) in &secrets_map {
        secret_to_ids.entry(sec.secret.clone()).or_default().push(id.clone());
    }

    for line in content.lines() {
        if let Some(mut acc) = parse_otpauth_uri(line) {
            let secret = acc.secret.clone();

            // 1. Exact triplet match (Issuer + Name + Secret)
            let exact_match = metadata.accounts.iter().find(|m|
                m.name == acc.name &&
                m.issuer == acc.issuer &&
                secrets_map.get(&m.id).map(|s| &s.secret) == Some(&secret)
            );

            if exact_match.is_some() {
                skip_count += 1;
                continue;
            }

            let name_match_pos = metadata.accounts.iter().position(|m| m.name == acc.name && m.issuer == acc.issuer);

            if let Some(pos) = name_match_pos {
                if !overwrite {
                    skip_count += 1;
                    continue;
                }
                let id = metadata.accounts[pos].id.clone();
                let entry = AccountSecret { secret: secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                secrets_map.insert(id, entry);
                updated_count += 1;
                continue;
            }

            // 2. Secret exists but labels are different
            if let Some(ids) = secret_to_ids.get(&secret) {
                if ids.len() == 1 {
                    // Unique secret: can update labels
                    if !overwrite {
                        skip_count += 1;
                        continue;
                    }
                    let existing_id = &ids[0];
                    if let Some(m) = metadata.accounts.iter_mut().find(|m| m.id == *existing_id) {
                        m.name = acc.name.clone();
                        m.issuer = acc.issuer.clone();
                    }
                    let entry = AccountSecret { secret: secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                    secrets_map.insert(existing_id.clone(), entry);
                    updated_count += 1;
                } else {
                    // Non-unique secret
                    println!("[Ambiguous] Secret for '{}:{}' exists multiple times. Safely appending as new entry.", acc.issuer.as_deref().unwrap_or("None"), acc.name);
                    let id = acc.id.clone();
                    let entry = AccountSecret { secret: secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                    acc.secret = "".to_string();
                    metadata.accounts.push(acc);
                    secrets_map.insert(id.clone(), entry);
                    secret_to_ids.get_mut(&secret).unwrap().push(id);
                    new_count += 1;
                }
            } else {
                // 3. Completely new secret
                let id = acc.id.clone();
                let entry = AccountSecret { secret: secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
                acc.secret = "".to_string();
                metadata.accounts.push(acc);
                secrets_map.insert(id.clone(), entry);
                secret_to_ids.insert(secret, vec![id]);
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

    fs::write(&meta_path, serde_yaml::to_string(&metadata).unwrap()).context("Failed to write metadata")?;
    println!("\nImport completed successfully!");
    println!("  - New: {}, Updated: {}, Skipped: {}", new_count, updated_count, skip_count);
    let _ = jki_core::agent::AgentClient::reload();
    Ok(())
}

struct TerminalGuard;

impl TerminalGuard {
    fn init() -> anyhow::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), cursor::Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(stdout(), cursor::Show);
        let _ = terminal::disable_raw_mode();
    }
}

fn perform_handshake(acc: &Account, is_authorized: bool, stdout_flag: bool) -> anyhow::Result<bool> {
    let _guard = TerminalGuard::init()?;
    let mut stdout_handle = stdout();

    print!("\r\n[Handshake] Please verify this code on the service provider's website:\r\n");
    print!("  Account : {}{}\r\n", acc.issuer.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default(), acc.name);
    
    let clipboard_status = if stdout_flag { 
        "\x1b[33mDisabled (Stdout mode)\x1b[0m" 
    } else { 
        "\x1b[32mAuto-syncing (Enabled)\x1b[0m" 
    };
    print!("  Clipboard: {}\r\n", clipboard_status);

    if is_authorized {
        print!("  Status  : Pre-authorized. You can press ENTER to save immediately.\r\n");
    }
    print!("--------------------------------------------------\r\n");
    stdout_handle.flush()?;

    let mut last_otp: Option<String> = None;

    let res = (|| -> anyhow::Result<bool> {
        loop {
            let now = chrono::Utc::now().timestamp() as u64;
            let seconds_left = 30 - (now % 30);
            let otp = jki_core::generate_otp(acc).unwrap_or_else(|_| "ERROR".to_string());
            
            // Auto-copy to clipboard if changed and not in stdout mode
            if !stdout_flag && last_otp.as_ref() != Some(&otp) {
                use copypasta::{ClipboardContext, ClipboardProvider};
                if let Ok(mut ctx) = ClipboardContext::new() {
                    let _ = <ClipboardContext as ClipboardProvider>::set_contents(&mut ctx, otp.clone());
                }
                last_otp = Some(otp.clone());
            }

            // Format OTP with space for readability
            let display_otp = if otp.len() == 6 {
                format!("{} {}", &otp[0..3], &otp[3..6])
            } else {
                otp
            };

            let copy_indicator = if !stdout_flag { " (Copied!)" } else { "" };
            print!("\r  CODE    : \x1b[1;32m{}\x1b[0m  (expires in {:2}s){}   ", display_otp, seconds_left, copy_indicator);
            stdout_handle.flush()?;

            // Poll for input (100ms)
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Enter => return Ok(true),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(false);
                            }
                            KeyCode::Esc => return Ok(false),
                            _ => {}
                        }
                    }
                }
            }
        }
    })();

    print!("\r\n--------------------------------------------------\r\n");
    stdout_handle.flush()?;
    res
}

fn handle_dedupe(
    keep: &Vec<usize>,
    discard: &Vec<usize>,
    yes: bool,
    auth: AuthSource,
    interactor: &dyn Interactor,
    quiet: bool,
) -> anyhow::Result<()> {
    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    if !meta_path.exists() {
        return Err(anyhow!("Metadata not found. Run 'jkim init' first."));
    }

    let master_key = acquire_master_key(auth, interactor, None).map_err(|e| anyhow!("Authentication failed: {}", e))?;

    let meta_content = fs::read_to_string(&meta_path).context("Failed to read metadata")?;
    let mut metadata: MetadataFile = serde_yaml::from_str(&meta_content).context("Failed to parse metadata")?;

    let mut secrets_map: HashMap<String, AccountSecret> = if dec_path.exists() {
        let content = fs::read(&dec_path).context("Failed to read plaintext secrets")?;
        serde_json::from_slice(&content).context("Failed to parse plaintext secrets")?
    } else if sec_path.exists() {
        let encrypted = fs::read(&sec_path).context("Failed to read secrets file")?;
        let decrypted = decrypt_with_master_key(&encrypted, &master_key).map_err(|e| anyhow!(e))?;
        serde_json::from_slice(&decrypted).context("Failed to parse existing secrets JSON")?
    } else {
        return Err(anyhow!("Secrets not found."));
    };

    let (integrated, _) = jki_core::integrate_accounts(metadata.accounts.clone(), &secrets_map);
    let groups = find_duplicate_groups(&integrated);

    if groups.is_empty() {
        if !quiet { println!("No duplicates found."); }
        return Ok(());
    }

    // 2. Identifying deletions
    let mut to_delete_ids = std::collections::HashSet::new();

    // Check for conflicts between keep and discard
    for idx in keep {
        if discard.contains(idx) {
            return Err(anyhow!("Conflict: Index {} is in both --keep and --discard.", idx));
        }
    }

    if keep.is_empty() && discard.is_empty() {
        // List mode
        println!("The following duplicate groups were found:\n");
        for group in &groups {
            println!("Group (Secret: {}...):", &group.secret[..std::cmp::min(8, group.secret.len())]);
            for acc in &group.accounts {
                println!("  {:>2}) [{}] {} (ID: {})",
                    acc.global_index,
                    acc.account.issuer.as_deref().unwrap_or("None"),
                    acc.account.name,
                    &acc.account.id[..8]
                );
            }
            println!();
        }
        println!("Run 'jkim dedupe -k <idx>' to keep one item and delete its shadows.");
        println!("Run 'jkim dedupe -d <idx>' to delete specific items.");
        return Ok(());
    }

    // Handle --keep
    for &k_idx in keep {
        let mut found = false;
        for group in &groups {
            if let Some(_) = group.accounts.iter().find(|a| a.global_index == k_idx) {
                found = true;
                // Delete everything else in this group
                for other in &group.accounts {
                    if other.global_index != k_idx {
                        to_delete_ids.insert(other.account.id.clone());
                    }
                }
            }
        }
        if !found {
            return Err(anyhow!("Index {} not found in any duplicate group.", k_idx));
        }
    }

    // Handle --discard
    for &d_idx in discard {
        let mut found = false;
        for group in &groups {
            if let Some(target) = group.accounts.iter().find(|a| a.global_index == d_idx) {
                found = true;
                to_delete_ids.insert(target.account.id.clone());
            }
        }
        if !found {
            return Err(anyhow!("Index {} not found in any duplicate group.", d_idx));
        }
    }

    // 3. Safety Verification
    if to_delete_ids.is_empty() {
        println!("Nothing to delete based on the provided indices.");
        return Ok(());
    }

    println!("!!! WARNING: PERMANENT DELETION !!!");
    println!("The following entries will be removed from Metadata and Secrets vault:");
    for acc in &integrated {
        if to_delete_ids.contains(&acc.id) {
            println!("  - [{}] {} (ID: {})",
                acc.issuer.as_deref().unwrap_or("None"),
                acc.name,
                acc.id
            );
        }
    }
    println!("\nTotal to delete: {} entries.", to_delete_ids.len());

    if !yes && !interactor.confirm("Proceed with deletion?", false) {
        println!("Aborted.");
        return Ok(());
    }

    // 4. Performing the Sweep
    metadata.accounts.retain(|a| !to_delete_ids.contains(&a.id));
    for id in &to_delete_ids {
        secrets_map.remove(id);
    }

    // Save back
    let secrets_json = serde_json::to_vec(&secrets_map).context("Failed to serialize secrets")?;
    if sec_path.exists() {
        let encrypted = encrypt_with_master_key(&secrets_json, &master_key).map_err(|e| anyhow!("Encryption failed: {}", e))?;
        fs::write(&sec_path, encrypted).context("Failed to write encrypted vault")?;
    } else if dec_path.exists() {
        fs::write(&dec_path, &secrets_json).context("Failed to write plaintext vault")?;
    }

    fs::write(&meta_path, serde_yaml::to_string(&metadata).unwrap()).context("Failed to write metadata")?;

    println!("Success: {} entries removed.", to_delete_ids.len());
    let _ = jki_core::agent::AgentClient::reload();

    Ok(())
}

fn handle_add(
    name: &Option<String>,
    issuer: &Option<String>,
    secret: &Option<String>,
    uri: &Option<String>,
    force: bool,
    show_secret: bool,
    stdout_flag: bool,
    auth: AuthSource,
    default_flag: bool,
    quiet: bool,
    interactor: &dyn Interactor,
) -> anyhow::Result<()> {
    let mut stdout_flag = stdout_flag;
    let mut name = name.clone();
    let mut issuer = issuer.clone();

    // Support '-' as positional alias for stdout
    if name.as_deref() == Some("-") {
        stdout_flag = true;
        name = None;
    }
    if issuer.as_deref() == Some("-") {
        stdout_flag = true;
        issuer = None;
    }

    // 1. Data Source Resolution (with Smart URI Detection)
    let mut acc = if let Some(u) = uri {
        parse_otpauth_uri(u).ok_or_else(|| anyhow!("Invalid OTPAuth URI format."))?
    } else if let Some(n_val) = &name {
        if n_val.starts_with("otpauth://") {
            parse_otpauth_uri(n_val).ok_or_else(|| anyhow!("Invalid OTPAuth URI format in positional argument."))?
        } else {
            // Standard manual input path
            let n = n_val.clone();
            let i = if let Some(i_val) = &issuer {
                Some(i_val.clone())
            } else {
                if atty::is(atty::Stream::Stdin) {
                    let input = interactor.prompt("Enter Issuer (Optional, Enter to skip)").map_err(|e| anyhow!(e))?;
                    if input.is_empty() { None } else { Some(input) }
                } else {
                    None
                }
            };
            
            let s = if let Some(s_cli) = secret {
                if !quiet && atty::is(atty::Stream::Stdin) {
                    eprintln!("Warning: Secret provided in CLI might leak into history.");
                }
                s_cli.clone()
            } else {
                interactor.prompt_password("Enter Base32 Secret").map_err(|e| anyhow!(e))?.expose_secret().clone()
            };

            jki_core::Account {
                id: uuid::Uuid::new_v4().to_string(),
                name: n,
                issuer: i,
                account_type: jki_core::AccountType::Standard,
                secret: s,
                digits: 6,
                algorithm: "SHA1".to_string(),
            }
        }
    } else {
        // No arguments provided, prompt for everything
        if !atty::is(atty::Stream::Stdin) { return Err(anyhow!("Account name is required in non-TTY mode.")); }
        let n = interactor.prompt("Enter Account Name").map_err(|e| anyhow!(e))?;
        
        let i = if atty::is(atty::Stream::Stdin) {
            let input = interactor.prompt("Enter Issuer (Optional, Enter to skip)").map_err(|e| anyhow!(e))?;
            if input.is_empty() { None } else { Some(input) }
        } else {
            None
        };

        let s = interactor.prompt_password("Enter Base32 Secret").map_err(|e| anyhow!(e))?.expose_secret().clone();

        jki_core::Account {
            id: uuid::Uuid::new_v4().to_string(),
            name: n,
            issuer: i,
            account_type: jki_core::AccountType::Standard,
            secret: s,
            digits: 6,
            algorithm: "SHA1".to_string(),
        }
    };

    // 2. Secret Cleaning
    acc.secret = acc.secret.trim().replace(" ", "").to_uppercase();
    
    // 3. Base32 Sanity Check
    if base32::decode(base32::Alphabet::RFC4648 { padding: true }, &acc.secret).is_none() {
        if !quiet && !force {
            eprintln!("Warning: Secret does not look like valid Base32.");
            if !default_flag && !interactor.confirm("Proceed anyway?", false) {
                return Err(anyhow!("Aborted due to invalid secret format."));
            }
        }
    }

    // 4. Persistence Logic
    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();
    let dec_path = JkiPath::decrypted_secrets_path();

    if !meta_path.parent().unwrap().exists() {
        return Err(anyhow!("JKI home not initialized. Run 'jkim init' first."));
    }

    let master_key = acquire_master_key(auth, interactor, None).ok();

    let mut metadata = if meta_path.exists() {
        let content = fs::read_to_string(&meta_path).unwrap_or_default();
        serde_yaml::from_str::<MetadataFile>(&content).unwrap_or(MetadataFile { accounts: vec![], version: 1 })
    } else {
        MetadataFile { accounts: vec![], version: 1 }
    };

    let (has_age, has_json) = (sec_path.exists(), dec_path.exists());
    let mut secrets_map: HashMap<String, AccountSecret> = match (has_age, has_json) {
        (true, _) => {
            let k = master_key.clone().ok_or_else(|| anyhow!("Authentication required for encrypted vault."))?;
            let encrypted = fs::read(&sec_path).context("Failed to read secrets file")?;
            let decrypted = decrypt_with_master_key(&encrypted, &k).map_err(|e| anyhow!("Decryption failed: {}", e))?;
            serde_json::from_slice(&decrypted).context("Failed to parse existing secrets JSON")?
        },
        (false, true) => {
            let content = fs::read(&dec_path).context("Failed to read plaintext secrets")?;
            serde_json::from_slice(&content).context("Failed to parse plaintext secrets")?
        },
        (false, false) => HashMap::new(),
    };

    // 5. Conflict Check
    let existing_pos = metadata.accounts.iter().position(|m| m.name == acc.name && m.issuer == acc.issuer);
    if let Some(pos) = existing_pos {
        if !force {
            return Err(anyhow!("Conflict: Account '{}:{}' already exists. Use -f/--force to overwrite.", 
                acc.issuer.as_deref().unwrap_or(""), acc.name));
        }
        if !quiet { eprintln!("Overwriting existing account: {}:{}", acc.issuer.as_deref().unwrap_or(""), acc.name); }
        acc.id = metadata.accounts[pos].id.clone();
    }

    // --- SSoT Handshake Loop ---
    let is_authorized = force || default_flag;
    let should_handshake = atty::is(atty::Stream::Stdin) && !(is_authorized && quiet);

    if should_handshake {
        if !perform_handshake(&acc, is_authorized, stdout_flag)? {
            println!("\nAborted. No changes saved to vault.");
            return Ok(());
        }
    }

    if let Some(pos) = existing_pos {
        metadata.accounts[pos] = acc.clone();
    } else {
        metadata.accounts.push(acc.clone());
    }

    let entry = AccountSecret { secret: acc.secret.clone(), digits: acc.digits, algorithm: acc.algorithm.clone() };
    secrets_map.insert(acc.id.clone(), entry.clone());

    // 6. Write back
    let secrets_json = serde_json::to_vec(&secrets_map).context("Failed to serialize secrets")?;
    if has_age {
        let k = master_key.ok_or_else(|| anyhow!("Key required for encrypted write"))?;
        let encrypted = encrypt_with_master_key(&secrets_json, &k).map_err(|e| anyhow!("Encryption failed: {}", e))?;
        fs::write(&sec_path, encrypted).context("Failed to write encrypted vault")?;
        
        // Anti-Shadowing: If a plaintext vault exists, it's now stale. Delete it.
        if has_json {
            let _ = fs::remove_file(&dec_path);
            if !quiet { eprintln!("Note: Stale plaintext vault deleted to maintain integrity."); }
        }
    } else if has_json {
        fs::write(&dec_path, &secrets_json).context("Failed to write plaintext vault")?;
    } else {
        // New vault
        if let Some(k) = master_key {
            let encrypted = encrypt_with_master_key(&secrets_json, &k).map_err(|e| anyhow!("Encryption failed: {}", e))?;
            fs::write(&sec_path, encrypted).context("Failed to write encrypted vault")?;
        } else {
            fs::write(&dec_path, &secrets_json).context("Failed to write plaintext vault")?;
        }
    }

    fs::write(&meta_path, serde_yaml::to_string(&metadata).unwrap()).context("Failed to write metadata")?;

    if !quiet { println!("Account added successfully: {}:{}", acc.issuer.as_deref().unwrap_or(""), acc.name); }
    
    if show_secret {
        if !quiet { eprintln!("[Secret] Added: {}:{}", acc.issuer.as_deref().unwrap_or(""), acc.name); }
        println!("{}", entry.secret);
        println!("{}", acc.to_otpauth_uri());
    }

    let _ = jki_core::agent::AgentClient::reload();
    Ok(())
}

pub fn run(cli: Cli) -> anyhow::Result<()> {
    let interactor = TerminalInteractor;
    let mut auth = cli.auth;
    if cli.interactive { auth = AuthSource::Interactive; }

    match &cli.command {
        Commands::Status => handle_status()?,
        Commands::Agent(a) => handle_agent(a)?,
        Commands::Git(g) => match g {
            GitCommands::Init { force } => handle_init(*force)?,
            GitCommands::Sync => handle_git(cli.default, &interactor)?,
        },
        Commands::Sync => handle_git(cli.default, &interactor)?,
        Commands::Add { name, issuer, secret, uri, force, show_secret, stdout } =>
            handle_add(name, issuer, secret, uri, *force, *show_secret, *stdout, auth, cli.default, cli.quiet, &interactor)?,
        Commands::Edit => handle_edit()?,
        Commands::Decrypt { force, keep, remove_key } => handle_decrypt(*force, *keep, *remove_key, cli.default, auth, &interactor)?,
        Commands::Encrypt { force } => handle_encrypt(*force, cli.default, auth, &interactor)?,
        Commands::MasterKey(m) => handle_master_key(m, auth, cli.default, &interactor)?,
        Commands::Keychain(k) => handle_keychain(k, &interactor)?,
        Commands::Config(c) => handle_config(c, auth, &interactor)?,
        Commands::ImportWinauth { file, overwrite, force_new_vault } =>
            handle_import_winauth(file, *overwrite, auth, cli.default, &interactor, *force_new_vault)?,
        Commands::Export { output } => handle_export(output, auth, &interactor)?,
        Commands::Completions { shell, output } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();

            match output {
                Some(ref o) if o == "-" => {
                    // Output directly to stdout
                    clap_complete::generate(*shell, &mut cmd, bin_name, &mut std::io::stdout());
                }
                Some(ref path_str) => {
                    // Write to file
                    let path = PathBuf::from(path_str);
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let mut file = fs::File::create(&path).context(format!("Failed to create completion file at {:?}", path))?;
                    clap_complete::generate(*shell, &mut cmd, bin_name, &mut file);
                    eprintln!("{} Completion script for {:?} written to {:?}", style("Success:").green().bold(), shell, path);
                }
                None => {
                    // Guide mode (default): No stdout pollution
                    AssetId::GuideCompletions.render();
                    let shell_name = format!("{:?}", shell).to_lowercase();
                    eprintln!("\n{}", style("Example usage:").yellow().bold());
                    eprintln!("  jkim completions {} -o ~/.jki/jkim_completion.{}", shell_name, match shell {
                        clap_complete::Shell::Bash => "bash",
                        clap_complete::Shell::Zsh => "zsh",
                        clap_complete::Shell::Fish => "fish",
                        clap_complete::Shell::PowerShell => "ps1",
                        _ => "completions"
                    });
                    eprintln!("  jkim completions {} -o - > jkim_completions.{}", shell_name, shell_name);
                }
            }
        }
        Commands::Man => {
            AssetId::GuideMan.render();
        }
        Commands::Dedupe { keep, discard, yes } =>
            handle_dedupe(keep, discard, *yes, auth, &interactor, cli.quiet)?,
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
            prompts: RefCell::new(vec![]),
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
            prompts: RefCell::new(vec![]),
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
            prompts: RefCell::new(vec![]),
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
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };
        handle_import_winauth(&import_file, false, AuthSource::Auto, true, &interactor, false).unwrap();

        let meta_path = home.join("vault.metadata.yaml");
        let sec_path = home.join("vault.secrets.bin.age");
        assert!(meta_path.exists());
        assert!(sec_path.exists());

        let meta_content = fs::read_to_string(meta_path).unwrap();
        let metadata: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();
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
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };

        handle_import_winauth(&import_file, false, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.bin.age").exists());
        assert!(!home.join("vault.secrets.json").exists());
        fs::remove_file(home.join("vault.secrets.bin.age")).unwrap();
        fs::remove_file(home.join("vault.metadata.yaml")).unwrap();

        fs::remove_file(&key_path).unwrap();
        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![true]), 
        };

        handle_import_winauth(&import_file, false, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.json").exists());
        assert!(!home.join("vault.secrets.bin.age").exists());

        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![]),
        };

        handle_import_winauth(&import_file, true, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.json").exists());

        fs::write(&key_path, "testpass").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();
        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec![]),
            confirms: RefCell::new(vec![true]), 
        };

        handle_import_winauth(&import_file, true, AuthSource::Auto, false, &interactor, false).unwrap();
        assert!(home.join("vault.secrets.bin.age").exists());
        assert!(!home.join("vault.secrets.json").exists());

        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
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
        
        let interactor = MockInteractor { prompts: std::cell::RefCell::new(vec![]), passwords: std::cell::RefCell::new(vec![]), confirms: std::cell::RefCell::new(vec![]) };
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
            prompts: RefCell::new(vec![]),
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

        let meta_path = home.join("vault.metadata.yaml");
        let initial_meta = MetadataFile { accounts: vec![], version: 1 };
        fs::write(&meta_path, serde_yaml::to_string(&initial_meta).unwrap()).unwrap();

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
        let new_json = serde_yaml::to_string(&new_meta).unwrap();
        
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

        let updated_meta: MetadataFile = serde_yaml::from_str(&fs::read_to_string(&meta_path).unwrap()).unwrap();
        assert_eq!(updated_meta.version, 2);
        assert_eq!(updated_meta.accounts.len(), 1);
    }

    #[test]
    #[serial]
    fn test_handle_export_success() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_export");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // 1. Setup metadata
        let metadata = MetadataFile {
            version: 1,
            accounts: vec![Account {
                id: "test-id".to_string(),
                name: "test@gmail.com".to_string(),
                issuer: Some("Google".to_string()),
                account_type: AccountType::Standard,
                secret: "".to_string(),
                digits: 6,
                algorithm: "SHA1".to_string(),
            }]
        };
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();

        // 2. Setup secrets (Plaintext for simplicity in test)
        let mut secrets_map = HashMap::new();
        secrets_map.insert("test-id".to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&secrets_map).unwrap()).unwrap();

        // 3. Mock interactions (Master Key, then Export Password x2)
        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec!["master_pass".to_string(), "zippass".to_string(), "zippass".to_string()]),
            confirms: RefCell::new(vec![]),
        };

        let zip_path = temp.path().join("test_export.zip");
        handle_export(&Some(zip_path.clone()), AuthSource::Interactive, &interactor).unwrap();

        assert!(zip_path.exists());
        
        // 4. Verify ZIP content
        let zip_file = fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        assert_eq!(archive.len(), 1);
        
        let mut file = archive.by_index_decrypt(0, b"zippass").expect("Failed to decrypt or find file in ZIP");
        assert_eq!(file.name(), "accounts.txt");
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert!(content.contains("otpauth://totp/Google:test%40gmail.com"));
    }

    #[test]
    #[serial]
    fn test_handle_export_password_mismatch() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_export_fail");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        let metadata = MetadataFile { version: 1, accounts: vec![] };
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();
        fs::write(home.join("vault.secrets.json"), b"{}").unwrap();

        // Master Key, then Mismatched export passwords
        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec!["master_pass".to_string(), "zippass".to_string(), "WRONGpass".to_string()]),
            confirms: RefCell::new(vec![]),
        };

        let res = handle_export(&None, AuthSource::Interactive, &interactor);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Passwords do not match"));
    }

    #[test]
    #[serial]
    fn test_handle_import_winauth_conflict_handling() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_conflict");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // 1. Initial vault with 1 account
        let acc_id = "old-id";
        let metadata = MetadataFile {
            version: 1,
            accounts: vec![Account {
                id: acc_id.to_string(),
                name: "test@gmail.com".to_string(),
                issuer: Some("Google".to_string()),
                account_type: AccountType::Standard,
                secret: "".to_string(),
                digits: 6,
                algorithm: "SHA1".to_string(),
            }]
        };
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();
        let mut secrets_map = HashMap::new();
        secrets_map.insert(acc_id.to_string(), AccountSecret {
            secret: "OLDSECRET".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&secrets_map).unwrap()).unwrap();

        // 2. Import file with same account but NEW secret
        let import_file = temp.path().join("import.txt");
        fs::write(&import_file, "otpauth://totp/Google:test%40gmail.com?secret=NEWSECRET123&issuer=Google").unwrap();

        let interactor = MockInteractor { prompts: RefCell::new(vec![]), passwords: RefCell::new(vec![]), confirms: RefCell::new(vec![]) };

        // Test Skip (overwrite = false)
        handle_import_winauth(&import_file, false, AuthSource::Auto, true, &interactor, false).unwrap();
        let current_secrets: HashMap<String, AccountSecret> = serde_json::from_slice(&fs::read(home.join("vault.secrets.json")).unwrap()).unwrap();
        assert_eq!(current_secrets.get(acc_id).unwrap().secret, "OLDSECRET");

        // Test Overwrite (overwrite = true)
        handle_import_winauth(&import_file, true, AuthSource::Auto, true, &interactor, false).unwrap();
        let current_secrets: HashMap<String, AccountSecret> = serde_json::from_slice(&fs::read(home.join("vault.secrets.json")).unwrap()).unwrap();
        assert_eq!(current_secrets.get(acc_id).unwrap().secret, "NEWSECRET123");
    }

    #[test]
    #[serial]
    fn test_handle_import_winauth_vault_corrupted() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_corrupt");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // Metadata exists, but no secrets files
        fs::write(home.join("vault.metadata.yaml"), "{\"accounts\":[],\"version\":1}").unwrap();

        let import_file = temp.path().join("import.txt");
        fs::write(&import_file, "otpauth://totp/test?secret=S1").unwrap();

        let interactor = MockInteractor { prompts: RefCell::new(vec![]), passwords: RefCell::new(vec![]), confirms: RefCell::new(vec![]) };
        let res = handle_import_winauth(&import_file, false, AuthSource::Auto, true, &interactor, false);
        
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Vault corrupted"));
    }

    #[test]
    #[serial]
    fn test_handle_import_winauth_force_new() {
        use jki_core::encrypt_with_master_key;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_force_new");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // Encrypted vault exists with WRONG key
        let real_key = secrecy::SecretString::from("correct".to_string());
        let encrypted = encrypt_with_master_key(b"{}", &real_key).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();
        fs::write(home.join("vault.metadata.yaml"), "{\"accounts\":[],\"version\":1}").unwrap();

        let import_file = temp.path().join("import.txt");
        fs::write(&import_file, "otpauth://totp/test?secret=S1").unwrap();

        // Mock interaction: provide WRONG key
        let interactor = MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec!["wrong_key".to_string()]),
            confirms: RefCell::new(vec![true]), // Confirm overwrite
        };

        // Should fail without force_new_vault
        let res = handle_import_winauth(&import_file, false, AuthSource::Interactive, false, &interactor, false);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Master Key incorrect"));

        // Should succeed with force_new_vault
        interactor.passwords.borrow_mut().push("wrong_key".to_string());
        handle_import_winauth(&import_file, false, AuthSource::Interactive, false, &interactor, true).unwrap();
        assert!(home.join("vault.secrets.bin.age").exists());
    }

    #[test]
    #[serial]
    fn test_handle_status_smoke() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_status");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // 1. Without Git
        handle_status().unwrap();

        // 2. With Git but no remote
        handle_init(false).unwrap();
        handle_status().unwrap();

        // 3. With Master key
        fs::write(home.join("master.key"), "testpass").unwrap();
        handle_status().unwrap();
    }

    #[test]
    #[serial]
    fn test_handle_add_uri() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_add_uri");
        env::set_var("JKI_HOME", &home);
        handle_init(false).unwrap();

        let uri = "otpauth://totp/Google:test@gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google";
        let interactor = jki_core::MockInteractor { prompts: RefCell::new(vec![]), passwords: RefCell::new(vec![]), confirms: RefCell::new(vec![]) };
        
        handle_add(&None, &None, &None, &Some(uri.to_string()), false, false, false, AuthSource::Auto, true, true, &interactor).unwrap();

        let meta_content = fs::read_to_string(home.join("vault.metadata.yaml")).unwrap();
        let metadata: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();
        assert_eq!(metadata.accounts.len(), 1);
        assert_eq!(metadata.accounts[0].name, "test@gmail.com");
        assert_eq!(metadata.accounts[0].issuer, Some("Google".to_string()));

        let sec_content = fs::read(home.join("vault.secrets.json")).unwrap();
        let secrets: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_content).unwrap();
        let acc_id = &metadata.accounts[0].id;
        assert_eq!(secrets.get(acc_id).unwrap().secret, "JBSWY3DPEHPK3PXP");
    }

    #[test]
    #[serial]
    fn test_handle_add_manual_cleaning() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_add_manual");
        env::set_var("JKI_HOME", &home);
        handle_init(false).unwrap();

        // Secret with spaces and lowercase
        let name = Some("test".to_string());
        let issuer = Some("Service".to_string());
        let secret = Some("jbsw y3dp ehpk 3pxp".to_string()); 
        
        let interactor = jki_core::MockInteractor { prompts: RefCell::new(vec![]), passwords: RefCell::new(vec![]), confirms: RefCell::new(vec![]) };
        handle_add(&name, &issuer, &secret, &None, false, false, false, AuthSource::Auto, true, true, &interactor).unwrap();

        let meta_content = fs::read_to_string(home.join("vault.metadata.yaml")).unwrap();
        let metadata: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();
        let acc_id = &metadata.accounts[0].id;

        let sec_content = fs::read(home.join("vault.secrets.json")).unwrap();
        let secrets: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_content).unwrap();
        // Should be cleaned to uppercase and no spaces
        assert_eq!(secrets.get(acc_id).unwrap().secret, "JBSWY3DPEHPK3PXP");
    }

    #[test]
    #[serial]
    fn test_handle_add_conflict() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_add_conflict");
        env::set_var("JKI_HOME", &home);
        handle_init(false).unwrap();

        let name = Some("test".to_string());
        let issuer = Some("Service".to_string());
        let secret = Some("JBSWY3DPEHPK3PXP".to_string());
        let interactor = jki_core::MockInteractor { prompts: RefCell::new(vec![]), passwords: RefCell::new(vec![]), confirms: RefCell::new(vec![]) };

        // First add
        handle_add(&name, &issuer, &secret, &None, false, false, false, AuthSource::Auto, true, true, &interactor).unwrap();

        // Second add without force -> Conflict
        let res = handle_add(&name, &issuer, &Some("NEWSECRET".to_string()), &None, false, false, false, AuthSource::Auto, true, true, &interactor);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("already exists"));

        // Third add WITH force -> Success
        handle_add(&name, &issuer, &Some("NEWSECRET".to_string()), &None, true, false, false, AuthSource::Auto, true, true, &interactor).unwrap();
        
        let sec_content = fs::read(home.join("vault.secrets.json")).unwrap();
        let secrets: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_content).unwrap();
        let meta_content = fs::read_to_string(home.join("vault.metadata.yaml")).unwrap();
        let metadata: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();
        let acc_id = &metadata.accounts[0].id;
        assert_eq!(secrets.get(acc_id).unwrap().secret, "NEWSECRET");
    }

    #[test]
    #[serial]
    fn test_handle_add_show_secret() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_add_show_secret");
        env::set_var("JKI_HOME", &home);
        handle_init(false).unwrap();

        let name = Some("test".to_string());
        let issuer = Some("Service".to_string());
        let secret = Some("JBSWY3DPEHPK3PXP".to_string());
        let interactor = jki_core::MockInteractor { prompts: RefCell::new(vec![]), passwords: RefCell::new(vec![]), confirms: RefCell::new(vec![]) };

        // Test that it runs with show_secret = true
        handle_add(&name, &issuer, &secret, &None, false, true, false, AuthSource::Auto, true, true, &interactor).unwrap();
        
        let meta_content = fs::read_to_string(home.join("vault.metadata.yaml")).unwrap();
        let metadata: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();
        assert_eq!(metadata.accounts.len(), 1);
    }

    #[test]
    #[serial]
    fn test_handle_dedupe() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_dedupe");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        // 1. Setup metadata with duplicates
        let metadata = MetadataFile {
            version: 1,
            accounts: vec![
                Account { id: "1".to_string(), name: "user1".to_string(), issuer: Some("Google".to_string()), account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
                Account { id: "2".to_string(), name: "user1@gmail.com".to_string(), issuer: Some("Google".to_string()), account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
                Account { id: "3".to_string(), name: "other".to_string(), issuer: None, account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
            ]
        };
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();

        // 2. Setup secrets (1 and 2 are duplicates)
        let mut secrets_map = HashMap::new();
        secrets_map.insert("1".to_string(), AccountSecret { secret: "JBSWY3DPEHPK3PXP".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        secrets_map.insert("2".to_string(), AccountSecret { secret: "JBSWY3DPEHPK3PXP".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        secrets_map.insert("3".to_string(), AccountSecret { secret: "DIFFERENT".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&secrets_map).unwrap()).unwrap();

        let interactor = jki_core::MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec!["testpass".to_string()]),
            confirms: RefCell::new(vec![true]), // Confirm deletion
        };

        // 3. Run dedupe: keep index 2 (user1@gmail.com), index 1 should be removed
        handle_dedupe(&vec![2], &vec![], false, AuthSource::Auto, &interactor, true).unwrap();

        let meta_content = fs::read_to_string(home.join("vault.metadata.yaml")).unwrap();
        let updated_meta: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();

        assert_eq!(updated_meta.accounts.len(), 2);
        assert!(updated_meta.accounts.iter().any(|a| a.id == "2"));
        assert!(updated_meta.accounts.iter().any(|a| a.id == "3"));
        assert!(!updated_meta.accounts.iter().any(|a| a.id == "1"));

        let sec_content = fs::read(home.join("vault.secrets.json")).unwrap();
        let updated_secrets: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_content).unwrap();
        assert_eq!(updated_secrets.len(), 2);
        assert!(!updated_secrets.contains_key("1"));
    }

    #[test]
    #[serial]
    fn test_handle_dedupe_discard() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_dedupe_discard");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        let metadata = MetadataFile {
            version: 1,
            accounts: vec![
                Account { id: "1".to_string(), name: "user1".to_string(), issuer: Some("Google".to_string()), account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
                Account { id: "2".to_string(), name: "user2".to_string(), issuer: Some("Google".to_string()), account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
            ]
        };
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();

        let mut secrets_map = HashMap::new();
        secrets_map.insert("1".to_string(), AccountSecret { secret: "DUP".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        secrets_map.insert("2".to_string(), AccountSecret { secret: "DUP".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&secrets_map).unwrap()).unwrap();

        let interactor = jki_core::MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec!["p".to_string()]),
            confirms: RefCell::new(vec![true]),
        };

        // Discard index 1
        handle_dedupe(&vec![], &vec![1], false, AuthSource::Auto, &interactor, true).unwrap();

        let meta_content = fs::read_to_string(home.join("vault.metadata.yaml")).unwrap();
        let updated_meta: MetadataFile = serde_yaml::from_str(&meta_content).unwrap();
        assert_eq!(updated_meta.accounts.len(), 1);
        assert_eq!(updated_meta.accounts[0].id, "2");
    }

    #[test]
    #[serial]
    fn test_handle_dedupe_conflict() {
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_dedupe_conflict_test");
        env::set_var("JKI_HOME", &home);
        fs::create_dir_all(&home).unwrap();

        let metadata = MetadataFile {
            version: 1,
            accounts: vec![
                Account { id: "1".to_string(), name: "u".to_string(), issuer: None, account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
                Account { id: "2".to_string(), name: "u".to_string(), issuer: None, account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "SHA1".to_string() },
            ]
        };
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();
        let mut secrets_map = HashMap::new();
        secrets_map.insert("1".to_string(), AccountSecret { secret: "S".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        secrets_map.insert("2".to_string(), AccountSecret { secret: "S".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&secrets_map).unwrap()).unwrap();

        let interactor = jki_core::MockInteractor {
            prompts: RefCell::new(vec![]),
            passwords: RefCell::new(vec!["p".to_string()]),
            confirms: RefCell::new(vec![]),
        };

        // Conflict: keep and discard the same index
        let res = handle_dedupe(&vec![1], &vec![1], false, AuthSource::Auto, &interactor, true);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Conflict"));
    }
}
