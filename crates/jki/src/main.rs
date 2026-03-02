use clap::{Parser, Subcommand};
use jki_core::{
    agent::{Request, Response},
    generate_otp, integrate_accounts, paths::JkiPath,
    Account, AccountSecret, acquire_master_key, decrypt_with_master_key, search_accounts,
    TerminalInteractor, Interactor
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
        Response::OTP(otp) => println!("{}", otp),
        Response::Error(e) => return Err(format!("Agent error: {}", e)),
    }
    Ok(())
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
    let sec_path = JkiPath::secrets_path();

    if !meta_path.exists() {
        if !cli.quiet { eprintln!("Error: Metadata not found at {:?}", meta_path); }
        return Err(100);
    }

    if !cli.quiet { eprintln!("Unlocking vault..."); }
    let interactor = TerminalInteractor;
    let master_key = acquire_master_key(cli.interactive, &interactor).unwrap_or_else(|e| {
        eprintln!("Authentication failed: {}", e);
        process::exit(101);
    });

    let meta_content = fs::read_to_string(&meta_path).expect("Failed to read metadata");
    let meta_data: MetadataFile = serde_json::from_str(&meta_content).expect("Metadata parse error");

    let sec_encrypted = fs::read(&sec_path).expect("Secrets file missing");
    let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key).expect("Decryption failed");
    let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).expect("Secrets parse error");

    let (integrated_accounts, missing_ids) = integrate_accounts(meta_data.accounts, &secrets_map);

    if !missing_ids.is_empty() && !cli.quiet {
        eprintln!("Data Consistency Warning: Some accounts are missing secrets.");
        for name in &missing_ids { eprintln!("  - {}", name); }
        eprintln!("(Run with -q to suppress this warning)\n");
    }

    let accounts = integrated_accounts;

    let mut search_terms = patterns;
    let mut index_selection: Option<usize> = None;
    if search_terms.len() > 1 && search_terms.last().unwrap().chars().all(|c| c.is_ascii_digit()) {
        index_selection = search_terms.pop().and_then(|s| s.parse().ok());
    }

    let results = if search_terms.is_empty() { accounts.clone() } else { search_accounts(&accounts, &search_terms) };

    if results.is_empty() {
        if !cli.quiet { eprintln!("No matches found."); }
        return Err(1);
    }

    let target = if results.len() == 1 && !cli.list {
        Some(&results[0])
    } else if let Some(idx) = index_selection {
        if idx >= 1 && idx <= results.len() { Some(&results[idx - 1]) }
        else { return Err(2); }
    } else {
        if !cli.quiet { eprintln!("{}:", if cli.list { "Matches" } else { "Ambiguous results" }); }
        for (i, acc) in results.iter().enumerate() {
            let otp_str = if cli.otp { format!("{} - ", generate_otp(acc).unwrap_or("ERROR".to_string())) } else { "".to_string() };
            println!("{:2}) {}{}{}", i + 1, otp_str, acc.issuer.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default(), acc.name);
        }
        return Err(2);
    };

    if let Some(acc) = target {
        let otp = generate_otp(acc).unwrap_or_else(|e| {
            eprintln!("OTP generation failed: {}", e);
            process::exit(102);
        });
        let label = format!("{}{}", acc.issuer.as_deref().map(|s| format!("[{}] ", s)).unwrap_or_default(), acc.name);
        
        if !cli.quiet { eprintln!("Selected: {}", label); }
        if stdout_flag { println!("{}", otp); }
        else {
            use copypasta::{ClipboardContext, ClipboardProvider};
            let mut ctx = ClipboardContext::new().expect("Failed to open clipboard");
            ctx.set_contents(otp).expect("Failed to set clipboard content");
            if !cli.quiet {
                eprintln!("Copied OTP to clipboard.");
                use notify_rust::Notification;
                let _ = Notification::new().summary("jki: OTP Copied").body(&format!("Account: {}", label)).show();
            }
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
        
        // 1. Create master.key
        let key_path = home.join("master.key");
        fs::write(&key_path, master_key_val).unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        // 2. Create Metadata
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

        // 3. Create Secrets
        let mut secrets_map = HashMap::new();
        secrets_map.insert(acc_id.to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        let sec_json = serde_json::to_vec(&secrets_map).unwrap();
        let encrypted = encrypt_with_master_key(&sec_json, &master_key).unwrap();
        fs::write(home.join("vault.secrets.bin.age"), encrypted).unwrap();

        // 4. Run jki
        let cli = Cli {
            command: None,
            patterns: vec!["google".to_string()],
            list: false,
            otp: false,
            quiet: true,
            stdout: true,
        };
        
        let result = run(cli);
        assert!(result.is_ok());
    }
}
