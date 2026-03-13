use clap::{Parser, Subcommand};
use jki_core::{
    agent::AgentClient,
    generate_otp, paths::JkiPath,
    Account, AccountSecret, acquire_master_key, decrypt_with_master_key, search_accounts,
    TerminalInteractor, keychain::KeyringStore, AuthSource,
    JkiCoreError, MatchedAccount, MetadataFile,
};
use std::fs;
use std::process;
use std::collections::HashMap;
use anyhow::{Context, anyhow};
use console::style;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Search patterns (used if no subcommand is provided)
    pub patterns: Vec<String>,

    /// Authentication and data source
    #[arg(short = 'A', long, default_value = "auto")]
    pub auth: AuthSource,

    /// Force interactive master key input (alias for --auth interactive)
    #[arg(short = 'I', long)]
    pub interactive: bool,

    #[arg(short, long)]
    pub list: bool,
    #[arg(short, long)]
    pub otp: bool,
    #[arg(short = 'S', long = "show-secret")]
    pub show_secret: bool,
    #[arg(short = 'U', long = "uri")]
    pub show_uri: bool,
    #[arg(short, long)]
    pub quiet: bool,
    #[arg(short = 's', long = "stdout")]
    pub stdout: bool,
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
    /// Force agent to reload data from disk
    Reload,
    /// Get an OTP via the agent
    Get { id: String },
}

fn handle_agent(cmd: &AgentCommands, _auth: AuthSource, _quiet: bool) -> anyhow::Result<()> {
    match cmd {
        AgentCommands::Ping => {
            if AgentClient::ping() { println!("Agent is alive (Pong)"); }
            else {
                return Err(anyhow!("Agent is not responding. [Tip] Start it with 'jkim agent start'"));
            }
        }
        AgentCommands::Unlock => {
            if !AgentClient::ping() {
                return Err(anyhow!("Agent is not running. [Tip] Start it with 'jkim agent start'"));
            }
            let res = if _auth == AuthSource::Biometric {
                AgentClient::unlock_biometric()
            } else {
                let interactor = TerminalInteractor;
                let master_key = acquire_master_key(_auth, &interactor, Some(&KeyringStore))
                    .map_err(|e| anyhow!("Authentication failed: {}", e))?;
                AgentClient::unlock(&master_key)
            };

            match res {
                Ok(source) => println!("Agent unlocked successfully using {}", source),
                Err(e) => return Err(anyhow!("Unlock failed: {}", e)),
            }
        }
        AgentCommands::Reload => {
            if !AgentClient::ping() {
                return Err(anyhow!("Agent is not running."));
            }
            AgentClient::reload().map_err(|e| anyhow!("Reload failed: {}", e))?;
            println!("Agent reloaded successfully.");
        }
        AgentCommands::Get { id } => {
            if !AgentClient::ping() {
                return Err(anyhow!("Agent is not running. [Tip] Start it with 'jkim agent start'"));
            }
            match AgentClient::get_otp(id) {
                Ok(otp) => println!("{}", otp),
                Err(e) => return Err(anyhow!("Error: {}", e)),
            }
        }
    }
    Ok(())
}

fn handle_output(data: String, label: String, source: &str, data_type: &str, stdout_flag: bool, quiet: bool) {
    if !quiet {
        eprintln!("{} {}: {}", style(format!("[{}/{}]", source, data_type)).green().bold(), style("Selected").bold(), style(label.clone()).cyan());
    }
    if stdout_flag { println!("{}", data); }
    else {
        use copypasta::{ClipboardContext, ClipboardProvider};
        let mut ctx = ClipboardContext::new().expect("Failed to open clipboard");
        ctx.set_contents(data).expect("Failed to set clipboard content");
        if !quiet {
            eprintln!("Copied {} to clipboard.", data_type);
            use notify_rust::Notification;
            let _ = Notification::new().summary(&format!("jki: {} Copied", data_type)).body(&format!("Account: {}", label)).show();
        }
    }
}

fn render_highlighted(text: &str, indices: &[usize]) -> String {
    let mut result = String::new();
    let indices_set: std::collections::HashSet<_> = indices.iter().collect();
    for (i, c) in text.chars().enumerate() {
        if indices_set.contains(&i) {
            result.push_str(&style(c.to_string()).cyan().bold().to_string());
        } else {
            result.push(c);
        }
    }
    result
}

fn resolve_target(
    patterns: &[String],
    has_double_dash: bool,
    accounts: &[Account],
    list_mode: bool,
    _quiet: bool,
) -> (Vec<MatchedAccount>, Option<Account>) {
    if has_double_dash {
        let results = if patterns.is_empty() {
            accounts.iter().map(|acc| MatchedAccount {
                account: acc.clone(),
                score: 0,
                issuer_indices: vec![],
                name_indices: vec![],
            }).collect()
        } else {
            search_accounts(accounts, patterns)
        };
        let target = if results.len() == 1 && !list_mode { Some(results[0].account.clone()) } else { None };
        (results, target)
    } else {
        let search_terms = patterns.to_vec();
        let index_candidate = if !search_terms.is_empty() {
            let last = search_terms.last().unwrap();
            if last.chars().all(|c| c.is_ascii_digit()) {
                last.parse::<usize>().ok()
            } else {
                None
            }
        } else {
            None
        };

        match index_candidate {
            Some(idx) => {
                let mut terms_without_idx = search_terms.clone();
                terms_without_idx.pop();

                let results_without_idx = if terms_without_idx.is_empty() {
                    accounts.iter().map(|acc| MatchedAccount {
                        account: acc.clone(),
                        score: 0,
                        issuer_indices: vec![],
                        name_indices: vec![],
                    }).collect::<Vec<_>>()
                } else {
                    search_accounts(accounts, &terms_without_idx)
                };

                if idx >= 1 && idx <= results_without_idx.len() {
                    let selected = results_without_idx[idx - 1].account.clone();
                    let target = if !list_mode { Some(selected) } else { None };
                    (results_without_idx, target)
                } else {
                    let results = if patterns.is_empty() {
                        accounts.iter().map(|acc| MatchedAccount {
                            account: acc.clone(),
                            score: 0,
                            issuer_indices: vec![],
                            name_indices: vec![],
                        }).collect()
                    } else {
                        search_accounts(accounts, patterns)
                    };
                    let target = if results.len() == 1 && !list_mode { Some(results[0].account.clone()) } else { None };
                    (results, target)
                }
            }
            None => {
                let results = if patterns.is_empty() {
                    accounts.iter().map(|acc| MatchedAccount {
                        account: acc.clone(),
                        score: 0,
                        issuer_indices: vec![],
                        name_indices: vec![],
                    }).collect()
                } else {
                    search_accounts(accounts, patterns)
                };
                let target = if results.len() == 1 && !list_mode { Some(results[0].account.clone()) } else { None };
                (results, target)
            }
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    let mut auth = cli.auth;
    if cli.interactive {
        auth = AuthSource::Interactive;
    }

    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Agent { cmd } => {
                handle_agent(cmd, auth, cli.quiet)?;
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

    let meta_data = MetadataFile::load().context("Failed to load metadata. [Tip] Run 'jkim init' if you haven't.")?;

    let raw_args: Vec<String> = std::env::args().collect();
    let has_double_dash = raw_args.iter().any(|arg| arg == "--");

    let (initial_results, target_acc) = resolve_target(
        &patterns,
        has_double_dash,
        &meta_data.accounts,
        cli.list,
        cli.quiet,
    );

    if initial_results.is_empty() {
        if !cli.quiet { eprintln!("No matches found."); }
        return Err(anyhow!("No matches found"));
    }

    // 智慧命中邏輯 (Dominant Winner)
    let selected_acc = if let Some(acc) = target_acc {
        Some(acc)
    } else {
        // 如果使用者有傳入 patterns 且不是為了列出清單，我們嘗試尋找壓倒性勝出者
        if !patterns.is_empty() && !cli.list {
            let mut sorted = initial_results.clone();
            sorted.sort_by(|a, b| b.score.cmp(&a.score));

            let dominant = if sorted.len() == 1 {
                Some(sorted[0].account.clone())
            } else if sorted.len() > 1 && (sorted[0].score - sorted[1].score) >= 40 {
                // 如果第一名顯著領先第二名，自動命中
                Some(sorted[0].account.clone())
            } else {
                None
            };

            if let Some(acc) = dominant {
                Some(acc)
            } else {
                // 有歧義，列出清單 (維持穩定順序顯示)
                if !cli.quiet {
                    let top_score = sorted[0].score;
                    let second_score = sorted[1].score;
                    let top_id = &sorted[0].account.id;

                    eprintln!("{} (Gap {} < 40):", style("Ambiguous results").yellow().bold(), top_score - second_score);

                    for (i, matched) in initial_results.iter().enumerate() {
                        let acc = &matched.account;
                        let is_top = acc.id == *top_id;

                        let marker = if is_top { style(">").green().bold().to_string() } else { " ".to_string() };
                        let score_tag = if is_top { style(format!(" (Score: {})", matched.score)).dim().to_string() } else { "".to_string() };

                        let issuer_str = if let Some(ref s) = acc.issuer {
                            format!("[{}] ", render_highlighted(s, &matched.issuer_indices))
                        } else {
                            "".to_string()
                        };
                        let name_str = render_highlighted(&acc.name, &matched.name_indices);
                        println!("{} {:2}) {}{}{}", marker, i + 1, issuer_str, name_str, score_tag);
                    }
                    eprintln!("\n[Tip] Be more specific, or use 'jki <pattern> <index>' to select.");
                }
                return Ok(());
            }
        } else if patterns.is_empty() || cli.list {
            // 列出清單 (維持穩定順序)
            if !cli.quiet {
                let header = if patterns.is_empty() { "Accounts" } else { "Matches" };
                eprintln!("{}:", header);
                for (i, matched) in initial_results.iter().enumerate() {
                    let acc = &matched.account;
                    let issuer_str = if let Some(ref s) = acc.issuer {
                        format!("[{}] ", render_highlighted(s, &matched.issuer_indices))
                    } else {
                        "".to_string()
                    };
                    let name_str = render_highlighted(&acc.name, &matched.name_indices);
                    println!("{:2}) {}{}", i + 1, issuer_str, name_str);
                }
            }
            return Ok(());
        } else {
            None
        }
    };

    if let Some(acc) = selected_acc {
        let label = format!("{}{}", acc.issuer.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default(), acc.name);
        
        // Plaintext check
        if auth == AuthSource::Plaintext || auth == AuthSource::Auto {
            let decrypted_path = JkiPath::decrypted_secrets_path();
            if decrypted_path.exists() {
                if let Ok(content) = fs::read(&decrypted_path) {
                    if let Ok(secrets_map) = serde_json::from_slice::<HashMap<String, AccountSecret>>(&content) {
                        if let Some(s) = secrets_map.get(&acc.id) {
                            if cli.show_uri {
                                let mut full_acc = acc.clone();
                                full_acc.secret = s.secret.clone();
                                full_acc.digits = s.digits;
                                full_acc.algorithm = s.algorithm.clone();
                                handle_output(full_acc.to_otpauth_uri(), label, "Plaintext", "URI", stdout_flag, cli.quiet);
                                return Ok(());
                            } else if cli.show_secret {
                                handle_output(s.secret.clone(), label, "Plaintext", "Secret", stdout_flag, cli.quiet);
                                return Ok(());
                            } else {
                                let mut full_acc = acc.clone();
                                full_acc.secret = s.secret.clone();
                                full_acc.digits = s.digits;
                                full_acc.algorithm = s.algorithm.clone();
                                if let Ok(otp) = generate_otp(&full_acc) {
                                    handle_output(otp, label, "Plaintext", "OTP", stdout_flag, cli.quiet);
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            if auth == AuthSource::Plaintext {
                return Err(anyhow!("Plaintext vault missing."));
            }
        }

        // Agent check (only for OTP, unless show_secret/show_uri is true)
        if !cli.show_secret && !cli.show_uri && (auth == AuthSource::Agent || auth == AuthSource::Auto || auth == AuthSource::Biometric) {
            if AgentClient::ping() {
                match AgentClient::get_otp(&acc.id) {
                    Ok(otp) => {
                        handle_output(otp, label, "Agent", "OTP", stdout_flag, cli.quiet);
                        return Ok(());
                    }
                    Err(JkiCoreError::Agent(e)) if e.contains("Agent is locked") => {
                        if !cli.quiet { eprintln!("Agent is locked. Attempting to unlock..."); }

                        let res = if auth == AuthSource::Biometric {
                            AgentClient::unlock_biometric()
                        } else {
                            let interactor = TerminalInteractor;
                            // Fix: Don't use 'auth' (which might be Agent) to unlock the Agent itself.
                            let master_key = acquire_master_key(AuthSource::Auto, &interactor, Some(&KeyringStore))
                                .map_err(|e| anyhow!("Failed to acquire master key: {}", e))?;
                            AgentClient::unlock(&master_key)
                        };

                        match res {
                            Ok(_source) => {
                                if let Ok(otp) = AgentClient::get_otp(&acc.id) {
                                    handle_output(otp, label, "Agent", "OTP", stdout_flag, cli.quiet);
                                    return Ok(());
                                }
                            },
                            Err(e) => {
                                return Err(anyhow!("Unlock failed: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        if auth != AuthSource::Auto && !cli.quiet {
                             return Err(anyhow!("Agent failed: {}", e));
                        }
                        if !cli.quiet { eprintln!("Error: Agent failed: {}", e); }
                    }
                }
            } else if auth == AuthSource::Agent || auth == AuthSource::Biometric {
                if !cli.quiet {
                    eprintln!("[Tip] Start jki-agent with 'jkim agent start' for faster lookups.");
                }
                return Err(anyhow!("Agent not running"));
            }
        }

        // Local Decryption (Fallback or explicit, and ALWAYS used for show_secret/show_uri if not plaintext)
        if auth != AuthSource::Agent && auth != AuthSource::Plaintext && auth != AuthSource::Biometric || cli.show_secret || cli.show_uri {
            if auth == AuthSource::Auto && !cli.quiet && !cli.show_secret && !cli.show_uri { eprintln!("Falling back to local decryption..."); }
            let sec_path = JkiPath::secrets_path();
            let interactor = TerminalInteractor;
            
            if !sec_path.exists() {
                return Err(anyhow!("Secrets file missing at {:?}.", sec_path));
            }

            let master_key = acquire_master_key(auth, &interactor, Some(&KeyringStore))
                .map_err(|e| anyhow!("Authentication failed: {}", e))?;

            if AgentClient::ping() {
                let _ = AgentClient::unlock(&master_key);
            }

            let sec_encrypted = fs::read(&sec_path).context("Failed to read secrets file")?;
            let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key).map_err(|e| anyhow!(e))?;
            let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).context("Secrets parse error")?;

            if let Some(s) = secrets_map.get(&acc.id) {
                if cli.show_uri {
                    let mut full_acc = acc.clone();
                    full_acc.secret = s.secret.clone();
                    full_acc.digits = s.digits;
                    full_acc.algorithm = s.algorithm.clone();
                    handle_output(full_acc.to_otpauth_uri(), label, "Local", "URI", stdout_flag, cli.quiet);
                } else if cli.show_secret {
                    handle_output(s.secret.clone(), label, "Local", "Secret", stdout_flag, cli.quiet);
                } else {
                    let mut full_acc = acc.clone();
                    full_acc.secret = s.secret.clone();
                    full_acc.digits = s.digits;
                    full_acc.algorithm = s.algorithm.clone();
                    
                    let otp = generate_otp(&full_acc).map_err(|e| anyhow!(e))?;
                    handle_output(otp, label, "Local", "OTP", stdout_flag, cli.quiet);
                }
            } else {
                return Err(anyhow!("Secret not found for account {}", acc.id));
            }
        }
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        process::exit(1);
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
    fn test_args_show_secret() {
        let cli = Cli::try_parse_from(["jki", "google", "-S"]).unwrap();
        assert!(cli.show_secret);
        assert_eq!(cli.patterns, vec!["google"]);
    }

    #[test]
    fn test_args_uri() {
        let cli = Cli::try_parse_from(["jki", "google", "-U"]).unwrap();
        assert!(cli.show_uri);
        assert_eq!(cli.patterns, vec!["google"]);
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
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();

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
            auth: AuthSource::Auto,
            interactive: false,
            list: false,
            otp: false,
            show_secret: false,
            show_uri: false,
            quiet: true,
            stdout: true,
        };
        
        let result = run(cli);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_run_show_secret_stdout() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_show_secret");
        fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let master_key_val = "testpass";
        let master_key = SecretString::from(master_key_val.to_string());
        
        let key_path = home.join("master.key");
        fs::write(&key_path, master_key_val).unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let acc_id = "test-id";
        let raw_secret = "JBSWY3DPEHPK3PXP";
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
            secret: raw_secret.to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        let sec_json = serde_json::to_vec(&secrets_map).unwrap();
        let encrypted = encrypt_with_master_key(&sec_json, &master_key).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();

        let cli = Cli {
            command: None,
            patterns: vec!["google".to_string()],
            auth: AuthSource::Auto,
            interactive: false,
            list: false,
            otp: false,
            show_secret: true,
            show_uri: false,
            quiet: true,
            stdout: true,
        };
        
        let result = run(cli);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_run_uri_stdout() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_uri");
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
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();

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
            auth: AuthSource::Auto,
            interactive: false,
            list: false,
            otp: false,
            show_secret: false,
            show_uri: true,
            quiet: true,
            stdout: true,
        };
        
        let result = run(cli);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn test_run_auth_agent_skips_plaintext() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_agent_skip");
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
        fs::write(home.join("vault.metadata.yaml"), serde_yaml::to_string(&metadata).unwrap()).unwrap();

        let mut plaintext_map = HashMap::new();
        plaintext_map.insert(acc_id.to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        fs::write(home.join("vault.secrets.json"), serde_json::to_vec(&plaintext_map).unwrap()).unwrap();

        let encrypted = encrypt_with_master_key(&serde_json::to_vec(&plaintext_map).unwrap(), &master_key).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();

        let cli_force = Cli {
            command: None,
            patterns: vec!["google".to_string()],
            auth: AuthSource::Agent,
            interactive: false,
            list: false,
            otp: false,
            show_secret: false,
            show_uri: false,
            quiet: false,
            stdout: true,
        };
        assert!(run(cli_force).is_err());
    }

    #[test]
    fn test_resolve_target_index_simple() {
        let accs = vec![
            Account { id: "1".into(), name: "A".into(), issuer: None, account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
            Account { id: "2".into(), name: "B".into(), issuer: None, account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
        ];
        let (_results, target) = resolve_target(&["1".to_string()], false, &accs, false, true);
        assert_eq!(target.unwrap().id, "1");
        assert_eq!(_results.len(), 2);
    }

    #[test]
    fn test_resolve_target_double_dash() {
        let accs = vec![
            Account { id: "1".into(), name: "10".into(), issuer: None, account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
            Account { id: "2".into(), name: "B".into(), issuer: None, account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
        ];
        let (_results, target) = resolve_target(&["1".to_string()], true, &accs, false, true);
        assert_eq!(target.unwrap().name, "10");
        assert_eq!(_results.len(), 1);
    }

    #[test]
    fn test_resolve_target_pattern_plus_index() {
        let accs = vec![
            Account { id: "1".into(), name: "Google:A".into(), issuer: Some("Google".into()), account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
            Account { id: "2".into(), name: "Google:B".into(), issuer: Some("Google".into()), account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
            Account { id: "3".into(), name: "Other".into(), issuer: None, account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
        ];
        let (_results, target) = resolve_target(&["google".to_string(), "2".to_string()], false, &accs, false, true);
        assert_eq!(target.unwrap().id, "2");
        assert_eq!(_results.len(), 2);
    }

    #[test]
    fn test_resolve_target_index_out_of_range_fallback() {
        let accs = vec![
            Account { id: "1".into(), name: "999".into(), issuer: None, account_type: AccountType::Standard, secret: "".into(), digits: 6, algorithm: "SHA1".into() },
        ];
        let (_results, target) = resolve_target(&["2".to_string()], false, &accs, false, true);
        assert!(target.is_none());
        assert!(_results.is_empty());

        let (_results, target) = resolve_target(&["999".to_string()], false, &accs, false, true);
        assert_eq!(target.unwrap().name, "999");
    }
}
