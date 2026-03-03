use clap::{Parser, Subcommand};
use jki_core::{
    agent::{Request, Response},
    generate_otp, paths::JkiPath,
    Account, AccountSecret, acquire_master_key, decrypt_with_master_key, search_accounts,
    TerminalInteractor
};
use interprocess::local_socket::LocalSocketStream;
use std::io::{BufRead, BufReader, Read, Write};
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
    let socket_path = JkiPath::agent_socket_path();
    let name = socket_path.to_str().expect("Invalid socket path");

    let stream = match LocalSocketStream::connect(name) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error connecting to agent: {}. Is jki-agent running?", e);
            process::exit(1);
        }
    };
    
    if let Err(e) = handle_agent_with_stream(cmd, stream) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn handle_agent_with_stream<S: Read + Write>(cmd: &AgentCommands, mut stream: S) -> Result<(), String> {
    let req = match cmd {
        AgentCommands::Ping => Request::Ping,
        AgentCommands::Unlock => {
            let interactor = TerminalInteractor;
            let master_key = acquire_master_key(false, &interactor)?;
            use secrecy::ExposeSecret;
            Request::Unlock { master_key: master_key.expose_secret().clone() }
        }
        AgentCommands::Get { id } => Request::GetOTP { account_id: id.clone() },
    };

    let req_json = serde_json::to_string(&req).expect("Failed to serialize request");
    stream.write_all(format!("{}\n", req_json).as_bytes()).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let resp: Response = serde_json::from_str(&line).map_err(|e| format!("Failed to parse agent response: {}", e))?;

    match resp {
        Response::Pong => println!("Agent is alive (Pong)"),
        Response::Unlocked => println!("Agent unlocked successfully"),
        Response::OTP(otp) => println!("{}", otp),
        Response::Error(e) => return Err(format!("Agent error: {}", e)),
    }
    Ok(())
}

fn ensure_agent_running(quiet: bool) -> bool {
    let socket_path = JkiPath::agent_socket_path();
    if socket_path.exists() {
        if let Ok(_) = LocalSocketStream::connect(socket_path.to_str().unwrap()) {
            return true;
        }
        if !cfg!(windows) {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    if !quiet { eprintln!("Starting jki-agent..."); }
    
    let current_exe = std::env::current_exe().expect("Failed to get current exe");
    let agent_exe = current_exe.parent().unwrap().join("jki-agent");
    
    let child = process::Command::new(agent_exe)
        .stdin(process::Stdio::null())
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .spawn();

    match child {
        Ok(_) => {
            std::thread::sleep(std::time::Duration::from_millis(200));
            true
        }
        Err(e) => {
            if !quiet { eprintln!("Failed to start jki-agent: {}", e); }
            false
        }
    }
}

fn handle_otp_output(otp: String, label: String, stdout_flag: bool, quiet: bool) {
    if !quiet { eprintln!("Selected: {}", label); }
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
        
        if ensure_agent_running(cli.quiet) {
            let socket_path = JkiPath::agent_socket_path();
            if let Ok(mut stream) = LocalSocketStream::connect(socket_path.to_str().unwrap()) {
                let req = Request::GetOTP { account_id: acc.id.clone() };
                let req_json = serde_json::to_string(&req).unwrap();
                let _ = stream.write_all(format!("{}\n", req_json).as_bytes());
                let _ = stream.flush();

                let mut line = String::new();
                let mut reader = BufReader::new(stream);
                if let Ok(_) = reader.read_line(&mut line) {
                    if let Ok(resp) = serde_json::from_str::<Response>(&line) {
                        match resp {
                            Response::OTP(otp) => {
                                handle_otp_output(otp, label, stdout_flag, cli.quiet);
                                return Ok(());
                            }
                            Response::Error(e) if e.contains("Agent is locked") => {
                                if !cli.quiet { eprintln!("Agent is locked. Attempting to unlock..."); }
                                let interactor = TerminalInteractor;
                                if let Ok(master_key) = acquire_master_key(cli.interactive, &interactor) {
                                    use secrecy::ExposeSecret;
                                    let unlock_req = Request::Unlock { master_key: master_key.expose_secret().clone() };
                                    let mut s = reader.into_inner();
                                    let _ = s.write_all(format!("{}\n", serde_json::to_string(&unlock_req).unwrap()).as_bytes());
                                    let _ = s.flush();
                                    
                                    let mut line = String::new();
                                    let mut reader = BufReader::new(s);
                                    if let Ok(_) = reader.read_line(&mut line) {
                                        if let Ok(Response::Unlocked) = serde_json::from_str(&line) {
                                            let req = Request::GetOTP { account_id: acc.id.clone() };
                                            let mut s = reader.into_inner();
                                            let _ = s.write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes());
                                            let _ = s.flush();
                                            
                                            let mut line = String::new();
                                            let mut reader = BufReader::new(s);
                                            if let Ok(_) = reader.read_line(&mut line) {
                                                if let Ok(Response::OTP(otp)) = serde_json::from_str(&line) {
                                                    handle_otp_output(otp, label, stdout_flag, cli.quiet);
                                                    return Ok(());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if !cli.quiet { eprintln!("Falling back to local decryption..."); }
        let sec_path = JkiPath::secrets_path();
        let interactor = TerminalInteractor;
        let master_key = acquire_master_key(cli.interactive, &interactor).unwrap_or_else(|e| {
            eprintln!("Authentication failed: {}", e);
            process::exit(101);
        });

        let sec_encrypted = fs::read(&sec_path).expect("Secrets file missing");
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
            handle_otp_output(otp, label, stdout_flag, cli.quiet);
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
    fn test_handle_agent_with_stream() {
        use std::io::Cursor;
        let cmd = AgentCommands::Ping;
        let mut _output: Vec<u8> = Vec::new();
        
        struct MockStream {
            input: Cursor<Vec<u8>>,
            output: Vec<u8>,
        }
        impl std::io::Read for MockStream {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.input.read(buf) }
        }
        impl std::io::Write for MockStream {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> { self.output.write(buf) }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }

        let resp = Response::Pong;
        let mut input = serde_json::to_vec(&resp).unwrap();
        input.push(b'\n');

        let mut stream = MockStream { input: Cursor::new(input), output: Vec::new() };
        handle_agent_with_stream(&cmd, &mut stream).unwrap();

        let req_str = String::from_utf8(stream.output).unwrap();
        let req: Request = serde_json::from_str(&req_str).unwrap();
        match req {
            Request::Ping => {},
            _ => panic!("Expected Ping request"),
        }
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
        };
        
        let result = run(cli);
        assert!(result.is_ok());
    }
}