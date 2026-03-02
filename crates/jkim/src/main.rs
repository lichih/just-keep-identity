use clap::{Parser, Subcommand};
use jki_core::{
    paths::JkiPath, 
    git, 
    Account, 
    AccountSecret,
    acquire_master_key, 
    encrypt_with_master_key,
    decrypt_with_master_key,
    import::parse_otpauth_uri
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser)]
#[command(name = "jkim", version, about = "JK Suite Management Hub")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check the health and status of the JKI system
    Status,
    /// Initialize the JKI home directory and Git repository
    Init,
    /// Import accounts from a WinAuth decrypted text file
    ImportWinauth {
        /// Path to the decrypted WinAuth .txt file
        file: PathBuf,
        /// Overwrite existing accounts if name+issuer matches
        #[arg(short, long)]
        overwrite: bool,
    },
}

#[derive(Serialize, Deserialize)]
struct MetadataFile {
    accounts: Vec<Account>,
    version: u32,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Status => {
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
                println!("  - Working Tree    : Clean");
            } else {
                println!("  - Git Repository  : Not initialized");
            }

            println!("\n[Paths]");
            println!("  - Metadata Path   : {:?}", JkiPath::metadata_path());
            println!("  - Secrets Path    : {:?}", JkiPath::secrets_path());
        }

        Commands::Init => {
            let config_dir = JkiPath::home_dir();
            println!("Initializing JKI Home at {:?}...", config_dir);
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
                println!("  - Directory created.");
            }
            if !config_dir.join(".git").exists() {
                let status = Command::new("git").args(["init", "-b", "main"]).current_dir(&config_dir).status().expect("Failed to git init");
                if status.success() { println!("  - Git initialized."); }
            }
            let gitignore_path = config_dir.join(".gitignore");
            fs::write(gitignore_path, "# JKI\nmaster.key\nvault.json\n*.txt\n*.bin\n").ok();
            let gitattrs_path = config_dir.join(".gitattributes");
            fs::write(gitattrs_path, "vault.secrets.bin.age binary\nvault.metadata.json filter=age\n").ok();
            println!("\nInitialization complete!");
        }

        Commands::ImportWinauth { file, overwrite } => {
            if !file.exists() { eprintln!("Error: File not found."); return; }

            let meta_path = JkiPath::metadata_path();
            let sec_path = JkiPath::secrets_path();

            // 1. Acquire Master Key EARLIER (We need it to load existing secrets)
            println!("Please unlock your vault to perform import.");
            let master_key = acquire_master_key().unwrap_or_else(|e| {
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
                        
                        if !*overwrite {
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
    }
}
