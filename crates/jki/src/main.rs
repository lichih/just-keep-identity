use clap::{Parser, Subcommand};
use jki_core::{
    agent::AgentClient,
    generate_otp, paths::JkiPath,
    Account, AccountSecret, acquire_master_key, decrypt_with_master_key, search_accounts,
    TerminalInteractor, keychain::KeyringStore, ensure_agent_running,
};
use std::fs;
use std::process;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Search patterns (used if no subcommand is provided)
    pub patterns: Vec<String>,

    /// Force interactive master key input, ignoring master.key file
    #[arg(short = 'I', long)]
    pub interactive: bool,

    #[arg(short, long)]
    pub list: bool,
    #[arg(short, long)]
    pub otp: bool,
    #[arg(short, long)]
    pub quiet: bool,
    #[arg(short = 's', long = "stdout")]
    pub stdout: bool,

    /// Force using agent or local decryption, bypassing plaintext vault
    #[arg(long)]
    pub force_agent: bool,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Interact with the JKI background agent
    Agent {
        #[command(subcommand)]
        cmd: AgentCommands,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum AgentCommands {
    /// Check if the agent is alive
    Ping,
    /// Unlock the agent with master key
    Unlock,
    /// Get an OTP via the agent
    Get { id: String },
}

#[derive(Deserialize, Serialize)]
struct MetadataFile {
    accounts: Vec<Account>,
    version: u32,
}

fn handle_agent(cmd: &AgentCommands) {
    match cmd {
        AgentCommands::Ping => {
            if AgentClient::ping() { println!("Agent is alive (Pong)"); }
            else { 
                eprintln!("Agent is not responding. Is jki-agent running?");
                process::exit(1);
            }
        }
        AgentCommands::Unlock => {
            let interactor = TerminalInteractor;
            let master_key = match acquire_master_key(false, &interactor, Some(&KeyringStore)) {
                Ok(k) => k,
                Err(e) => { eprintln!("Authentication failed: {}", e); process::exit(1); }
            };
            match AgentClient::unlock(&master_key) {
                Ok(source) => println!("Agent unlocked successfully using {}", source),
                Err(e) => { eprintln!("Unlock failed: {}", e); process::exit(1); }
            }
        }
        AgentCommands::Get { id } => {
            match AgentClient::get_otp(id) {
                Ok(otp) => println!("{}", otp),
                Err(e) => { eprintln!("Error: {}", e); process::exit(1); }
            }
        }
    }
}

fn handle_otp_output(otp: String, label: String, source: &str, stdout_flag: bool, quiet: bool) {
    if !quiet { eprintln!("[{}] Selected: {}", source, label); }
    if stdout_flag { println!("{}", otp); }
    else {
        use copypasta::{ClipboardContext, ClipboardProvider};
        let mut ctx = ClipboardContext::new().expect("Failed to open clipboard");
        ctx.set_contents(otp).expect("Failed to set clipboard content");
        if !quiet {
            eprintln!("Copied OTP to clipboard.");
            use notify_rust::Notification;
            let _ = Notification::new().summary("jki: OTP Copied").body(&format!("Account: {}", label)).show();
        }
    }
}

fn run(cli: Cli) -> Result<(), i32> {
    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Agent { cmd } => {
                handle_agent(cmd);
                return Ok(());
            }
        }
    }

    let mut patterns = cli.patterns.clone();
    let mut stdout_flag = cli.stdout;

    if patterns.contains(&"-".to_string()) {
        stdout_flag = true;
        patterns.retain(|x| x != "-");
    }

    let meta_path = JkiPath::metadata_path();
    if !meta_path.exists() {
        if !cli.quiet { eprintln!("Error: Metadata not found at {:?}", meta_path); }
        return Err(100);
    }

    let meta_content = fs::read_to_string(&meta_path).expect("Failed to read metadata");
    let meta_data: MetadataFile = serde_json::from_str(&meta_content).expect("Metadata parse error");

    let mut search_terms = patterns;
    let mut index_selection: Option<usize> = None;
    if search_terms.len() > 1 && search_terms.last().unwrap().chars().all(|c| c.is_ascii_digit()) {
        index_selection = search_terms.pop().and_then(|s| s.parse().ok());
    }

    let initial_results = if search_terms.is_empty() {
        meta_data.accounts.clone()
    } else {
        search_accounts(&meta_data.accounts, &search_terms)
    };

    if initial_results.is_empty() {
        if !cli.quiet { eprintln!("No matches found."); }
        return Err(1);
    }

    let target_acc = if initial_results.len() == 1 && !cli.list {
        Some(&initial_results[0])
    } else if let Some(idx) = index_selection {
        if idx >= 1 && idx <= initial_results.len() { Some(&initial_results[idx - 1]) }
        else { return Err(2); }
    } else {
        if !cli.quiet { eprintln!("{}:", if cli.list { "Matches" } else { "Ambiguous results" }); }
        for (i, acc) in initial_results.iter().enumerate() {
            println!("{:2}) {}{}", i + 1, acc.issuer.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default(), acc.name);
        }
        return Err(2);
    };

    if let Some(acc) = target_acc {
        let label = format!("{}{}", acc.issuer.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default(), acc.name);
        
        // 1. Plaintext Path: Check vault.secrets.json first (Fastest)
        if !cli.force_agent {
            let decrypted_path = JkiPath::decrypted_secrets_path();
            if decrypted_path.exists() {
                if let Ok(content) = fs::read(&decrypted_path) {
                    if let Ok(secrets_map) = serde_json::from_slice::<HashMap<String, AccountSecret>>(&content) {
                        if let Some(s) = secrets_map.get(&acc.id) {
                            let mut full_acc = acc.clone();
                            full_acc.secret = s.secret.clone();
                            full_acc.digits = s.digits;
                            full_acc.algorithm = s.algorithm.clone();
                            if let Ok(otp) = generate_otp(&full_acc) {
                                handle_otp_output(otp, label, "Plaintext", stdout_flag, cli.quiet);
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        // 2. Agent Path: Connect to jki-agent
        if ensure_agent_running(cli.quiet) {
            match AgentClient::get_otp(&acc.id) {
                Ok(otp) => {
                    handle_otp_output(otp, label, "Agent", stdout_flag, cli.quiet);
                    return Ok(());
                }
                Err(e) if e.contains("Agent is locked") => {
                    if !cli.quiet { eprintln!("Agent is locked. Attempting to unlock..."); }
                    let interactor = TerminalInteractor;
                    if let Ok(master_key) = acquire_master_key(cli.interactive, &interactor, Some(&KeyringStore)) {
                        if let Ok(_) = AgentClient::unlock(&master_key) {
                            if let Ok(otp) = AgentClient::get_otp(&acc.id) {
                                handle_otp_output(otp, label, "Agent", stdout_flag, cli.quiet);
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    if !cli.quiet { eprintln!("Error: Agent failed: {}", e); }
                }
            }
        }

        // 3 & 4. Static Key Path & Interactive Path (Fallback)
        if !cli.quiet { eprintln!("Falling back to local decryption..."); }
        let sec_path = JkiPath::secrets_path();
        let interactor = TerminalInteractor;
        
        if !sec_path.exists() {
            eprintln!("Error: Secrets file missing at {:?}. Please run jkim init or restore from backup.", sec_path);
            return Err(1);
        }

        let master_key = acquire_master_key(cli.interactive, &interactor, Some(&KeyringStore)).unwrap_or_else(|e| {
            eprintln!("Authentication failed: {}", e);
            process::exit(101);
        });

        // Lazy Unlock: Sync to Agent
        if ensure_agent_running(true) {
            let _ = AgentClient::unlock(&master_key);
        }

        let sec_encrypted = fs::read(&sec_path).map_err(|e| {
            eprintln!("Error: Failed to read secrets file: {}", e);
            1
        })?;
        let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key).expect("Decryption failed");
        let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).expect("Secrets parse error");

        if let Some(s) = secrets_map.get(&acc.id) {
            let mut full_acc = acc.clone();
            full_acc.secret = s.secret.clone();
            full_acc.digits = s.digits;
            full_acc.algorithm = s.algorithm.clone();
            
            let otp = generate_otp(&full_acc).unwrap_or_else(|e| {
                eprintln!("OTP generation failed: {}", e);
                process::exit(102);
            });
            handle_otp_output(otp, label, "Local", stdout_flag, cli.quiet);
        } else {
            eprintln!("Error: Secret not found for account {}", acc.id);
            return Err(1);
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    if let Err(code) = run(cli) {
        process::exit(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use serial_test::serial;
    use tempfile::tempdir;
    use std::env;
    use jki_core::{AccountType, encrypt_with_master_key};
    use secrecy::SecretString;

    #[test]
    fn test_args_parsing() {
        let cli = Cli::try_parse_from(["jki", "google", "gmail", "-l", "-o"]).unwrap();
        assert_eq!(cli.patterns, vec!["google", "gmail"]);
        assert!(cli.list);
        assert!(cli.otp);
        assert!(!cli.quiet);
    }

    #[test]
    fn test_args_stdout_short() {
        let cli = Cli::try_parse_from(["jki", "google", "-s"]).unwrap();
        assert!(cli.stdout);
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_run_full_flow() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let master_key_val = "testpass";
        let master_key = SecretString::from(master_key_val.to_string());
        
        let key_path = home.join("master.key");
        fs::write(&key_path, master_key_val).unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let acc_id = "test-id";
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
        fs::write(home.join("vault.metadata.json"), serde_json::to_string(&metadata).unwrap()).unwrap();

        let mut secrets_map = HashMap::new();
        secrets_map.insert(acc_id.to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        let sec_json = serde_json::to_vec(&secrets_map).unwrap();
        let encrypted = encrypt_with_master_key(&sec_json, &master_key).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();

        let cli = Cli {
            command: None,
            patterns: vec!["google".to_string()],
            interactive: false,
            list: false,
            otp: false,
            quiet: true,
            stdout: true,
            force_agent: false,
        };
        
        let result = run(cli);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_run_force_agent_skips_plaintext() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let master_key_val = "testpass";
        let master_key = SecretString::from(master_key_val.to_string());
        
        let key_path = home.join("master.key");
        fs::write(&key_path, master_key_val).unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let acc_id = "test-id";
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
        fs::write(home.join("vault.metadata.json"), serde_json::to_string(&metadata).unwrap()).unwrap();

        // Write plaintext vault
        let mut plaintext_map = HashMap::new();
        plaintext_map.insert(acc_id.to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&plaintext_map).unwrap()).unwrap();

        // Write encrypted vault
        let encrypted = encrypt_with_master_key(&serde_json::to_vec(&plaintext_map).unwrap(), &master_key).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();

        // Run without force_agent -> should use plaintext
        let cli_no_force = Cli {
            command: None,
            patterns: vec!["google".to_string()],
            interactive: false,
            list: false,
            otp: false,
            quiet: false, // Show output to verify manually if needed, but here we just check result
            stdout: true,
            force_agent: false,
        };
        assert!(run(cli_no_force).is_ok());

        // Run with force_agent -> should skip plaintext and use local (since no agent)
        let cli_force = Cli {
            command: None,
            patterns: vec!["google".to_string()],
            interactive: false,
            list: false,
            otp: false,
            quiet: false,
            stdout: true,
            force_agent: true,
        };
        assert!(run(cli_force).is_ok());
    }
}