use clap::Parser;
use jki_core::{search_accounts, paths::JkiPath, Account, AccountSecret, acquire_master_key, decrypt_with_master_key, integrate_accounts, generate_otp};
use std::fs;
use std::process;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    patterns: Vec<String>,
    #[arg(short, long)]
    list: bool,
    #[arg(short, long)]
    otp: bool,
    #[arg(short, long)]
    quiet: bool,
    #[arg(short = 's', long = "stdout")]
    stdout: bool,
}

#[derive(Deserialize, Serialize)]
struct MetadataFile {
    accounts: Vec<Account>,
    version: u32,
}

fn run(mut args: Args) -> Result<(), i32> {
    if args.patterns.contains(&"-".to_string()) {
        args.stdout = true;
        args.patterns.retain(|x| x != "-");
    }

    let meta_path = JkiPath::metadata_path();
    let sec_path = JkiPath::secrets_path();


    if !meta_path.exists() {
        if !args.quiet { eprintln!("Error: Metadata not found at {:?}", meta_path); }
        return Err(100);
    }

    if !args.quiet { eprintln!("Unlocking vault..."); }
    let master_key = acquire_master_key().unwrap_or_else(|e| {
        eprintln!("Authentication failed: {}", e);
        process::exit(101); // acquire_master_key uses raw terminal mode, exit might be okay if it restores it.
    });

    let meta_content = fs::read_to_string(&meta_path).expect("Failed to read metadata");
    let meta_data: MetadataFile = serde_json::from_str(&meta_content).expect("Metadata parse error");

    let sec_encrypted = fs::read(&sec_path).expect("Secrets file missing");
    let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key).expect("Decryption failed");
    let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).expect("Secrets parse error");

    let (integrated_accounts, missing_ids) = integrate_accounts(meta_data.accounts, &secrets_map);

    if !missing_ids.is_empty() && !args.quiet {
        eprintln!("Data Consistency Warning: Some accounts are missing secrets.");
        for name in &missing_ids { eprintln!("  - {}", name); }
        eprintln!("(Run with -q to suppress this warning)\n");
    }

    let accounts = integrated_accounts;

    let mut search_terms = args.patterns.clone();
    let mut index_selection: Option<usize> = None;
    if search_terms.len() > 1 && search_terms.last().unwrap().chars().all(|c| c.is_ascii_digit()) {
        index_selection = search_terms.pop().and_then(|s| s.parse().ok());
    }

    let results = if search_terms.is_empty() { accounts.clone() } else { search_accounts(&accounts, &search_terms) };

    if results.is_empty() {
        if !args.quiet { eprintln!("No matches found."); }
        return Err(1);
    }

    let target = if results.len() == 1 && !args.list {
        Some(&results[0])
    } else if let Some(idx) = index_selection {
        if idx >= 1 && idx <= results.len() { Some(&results[idx - 1]) }
        else { return Err(2); }
    } else {
        if !args.quiet { eprintln!("{}:", if args.list { "Matches" } else { "Ambiguous results" }); }
        for (i, acc) in results.iter().enumerate() {
            let otp_str = if args.otp { format!("{} - ", generate_otp(acc).unwrap_or("ERROR".to_string())) } else { "".to_string() };
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
        
        if !args.quiet { eprintln!("Selected: {}", label); }
        if args.stdout { println!("{}", otp); }
        else {
            use copypasta::{ClipboardContext, ClipboardProvider};
            let mut ctx = ClipboardContext::new().unwrap();
            ctx.set_contents(otp).unwrap();
            if !args.quiet {
                eprintln!("Copied OTP to clipboard.");
                use notify_rust::Notification;
                let _ = Notification::new().summary("jki: OTP Copied").body(&format!("Account: {}", label)).show();
            }
        }
    }
    Ok(())
}

fn main() {
    let args = Args::parse();
    if let Err(code) = run(args) {
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
        let args = Args::try_parse_from(["jki", "google", "gmail", "-l", "-o"]).unwrap();
        assert_eq!(args.patterns, vec!["google", "gmail"]);
        assert!(args.list);
        assert!(args.otp);
        assert!(!args.quiet);
    }

    #[test]
    fn test_args_stdout_short() {
        let args = Args::try_parse_from(["jki", "google", "-s"]).unwrap();
        assert!(args.stdout);
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
        let args = Args {
            patterns: vec!["google".to_string()],
            list: false,
            otp: false,
            quiet: true,
            stdout: true,
        };
        
        let result = run(args);
        assert!(result.is_ok());
    }
}
