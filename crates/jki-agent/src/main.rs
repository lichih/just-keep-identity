use clap::Parser;
use interprocess::local_socket::LocalSocketListener;
use jki_core::{
    agent::{Request, Response},
    paths::JkiPath,
    decrypt_with_master_key,
    AccountSecret,
    generate_otp,
    Account,
    AccountType,
};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(author, version, about = "jki-agent - Just Keep Identity Agent", long_about = None)]
struct Args {
    /// Force loading from encrypted .age files only
    #[arg(long)]
    force_age: bool,
}

struct State {
    secrets: Option<HashMap<String, AccountSecret>>,
    master_key: Option<secrecy::SecretString>,
    last_unlocked: Option<Instant>,
    ttl: Duration,
    force_age: bool,
}

impl State {
    fn new(force_age: bool) -> Self {
        Self {
            secrets: None,
            master_key: None,
            last_unlocked: None,
            ttl: Duration::from_secs(3600), // 1 hour TTL
            force_age,
        }
    }

    fn check_ttl(&mut self) {
        if let Some(last) = self.last_unlocked {
            if last.elapsed() > self.ttl {
                self.secrets = None;
                self.master_key = None;
                self.last_unlocked = None;
            }
        }
    }

    fn unlock(&mut self, master_key: secrecy::SecretString) -> Result<String, String> {
        let sec_path = JkiPath::secrets_path();
        let decrypted_path = JkiPath::decrypted_secrets_path();

        let res = if sec_path.exists() {
            let sec_encrypted = std::fs::read(&sec_path).map_err(|e| e.to_string())?;
            let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key)?;
            let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).map_err(|e| e.to_string())?;

            self.secrets = Some(secrets_map);
            self.last_unlocked = Some(Instant::now());
            Ok("Encrypted Vault".to_string())
        } else if self.force_age {
            Err("Force-age mode enabled: Encrypted vault missing. Refusing to load plaintext.".to_string())
        } else if decrypted_path.exists() {
            let sec_json = std::fs::read(&decrypted_path).map_err(|e| e.to_string())?;
            let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).map_err(|e| e.to_string())?;

            self.secrets = Some(secrets_map);
            self.last_unlocked = Some(Instant::now());
            Ok("Plaintext Vault".to_string())
        } else {
            Err("Secrets file missing (neither .age nor .json found)".to_string())
        };

        if res.is_ok() {
            self.master_key = Some(master_key);
        }
        res
    }

    fn get_otp(&mut self, account_id: &str) -> Result<String, String> {
        self.check_ttl();
        
        if self.secrets.is_none() {
            if let Some(key) = self.master_key.clone() {
                let _ = self.unlock(key)?;
            }
        }

        let secrets = self.secrets.as_ref().ok_or("Agent is locked")?;
        let secret = secrets.get(account_id).ok_or("Account not found")?;
        
        // Construct a temporary Account object for generate_otp
        let acc = Account {
            id: account_id.to_string(),
            name: "".to_string(), // Not needed for OTP
            issuer: None,
            account_type: AccountType::Standard,
            secret: secret.secret.clone(),
            digits: secret.digits,
            algorithm: secret.algorithm.clone(),
        };

        generate_otp(&acc)
    }
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let mut force_age = args.force_age;
    if std::env::var("JKI_FORCE_AGE").map(|v| v == "1").unwrap_or(false) {
        force_age = true;
    }

    let socket_path = JkiPath::agent_socket_path();
    let name = socket_path.to_str().unwrap();

    // Pre-flight check
    if force_age && !JkiPath::secrets_path().exists() {
        eprintln!("CRITICAL: Force-age mode enabled but encrypted vault (.age) is missing. Exit.");
        std::process::exit(1);
    }

    // Remove existing socket file if it exists (for Unix)
    if socket_path.exists() && !cfg!(windows) {
        let _ = std::fs::remove_file(&socket_path);
    }

    let listener = LocalSocketListener::bind(name)?;
    println!("jki-agent listening on {:?} (force_age: {})", socket_path, force_age);

    let state = Arc::new(Mutex::new(State::new(force_age)));

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(e) = handle_client(s, state) {
                        eprintln!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
    Ok(())
}

fn handle_client(stream: interprocess::local_socket::LocalSocketStream, state: Arc<Mutex<State>>) -> io::Result<()> {
    handle_client_io(stream, state)
}

fn handle_client_io<S: Read + Write>(stream: S, state: Arc<Mutex<State>>) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let _n = match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        };

        if line.trim().is_empty() { continue; }

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::Error(format!("Invalid request: {}", e));
                let s = reader.get_mut();
                s.write_all(format!("{}\n", serde_json::to_string(&resp).unwrap()).as_bytes())?;
                s.flush()?;
                continue;
            }
        };

        let resp = match req {
            Request::Ping => Response::Pong,
            Request::Unlock { master_key } => {
                let mut s = state.lock().unwrap();
                match s.unlock(master_key.into()) {
                    Ok(source) => Response::Unlocked(source),
                    Err(e) => Response::Error(format!("Unlock failed: {}", e)),
                }
            }
            Request::GetOTP { account_id } => {
                let mut s = state.lock().unwrap();
                match s.get_otp(&account_id) {
                    Ok(otp) => Response::OTP(otp),
                    Err(e) => Response::Error(format!("GetOTP failed: {}", e)),
                }
            }
            Request::GetMasterKey => {
                use secrecy::ExposeSecret;
                let s = state.lock().unwrap();
                match &s.master_key {
                    Some(key) => Response::MasterKey(key.expose_secret().clone()),
                    None => Response::Error("Agent is locked".to_string()),
                }
            }
            Request::Reload => {
                let mut s = state.lock().unwrap();
                s.secrets = None;
                Response::Success
            }
        };

        let resp_json = serde_json::to_string(&resp).unwrap();
        let s = reader.get_mut();
        s.write_all(format!("{}\n", resp_json).as_bytes())?;
        s.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use serial_test::serial;

    struct MockStream {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for MockStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.input.read(buf) }
    }
    impl Write for MockStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.output.write(buf) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    #[test]
    #[serial]
    fn test_handle_client_ping() {
        let state = Arc::new(Mutex::new(State::new(false)));
        let req = Request::Ping;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        handle_client_io(&mut stream, state).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Pong => {},
            _ => panic!("Expected Pong, got {:?}", resp),
        }
    }

    #[test]
    #[serial]
    fn test_handle_client_get_otp_locked() {
        let state = Arc::new(Mutex::new(State::new(false)));
        let req = Request::GetOTP { account_id: "test".to_string() };
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        handle_client_io(&mut stream, state).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Error(msg) => assert!(msg.contains("Agent is locked")),
            _ => panic!("Expected Error (locked), got {:?}", resp),
        }
    }

    #[test]
    #[serial]
    fn test_handle_client_unlock_and_get_otp() {
        use tempfile::tempdir;
        use std::env;
        use jki_core::encrypt_with_master_key;
        use secrecy::SecretString;

        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_agent_test");
        std::fs::create_dir_all(&home).unwrap();
        
        // Explicitly set paths to avoid canonicalization issues in tests
        let sec_path = home.join("vault.secrets.bin.age");
        env::set_var("JKI_HOME", home.to_str().unwrap());
        env::set_var("JKI_SECRETS_PATH", sec_path.to_str().unwrap());

        let master_key_val = "testpass";
        let master_key = SecretString::from(master_key_val.to_string());
        
        // 1. Create Secrets
        let acc_id = "test-id";
        let mut secrets_map = HashMap::new();
        secrets_map.insert(acc_id.to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        let sec_json = serde_json::to_vec(&secrets_map).unwrap();
        let encrypted = encrypt_with_master_key(&sec_json, &master_key).unwrap();
        std::fs::write(&sec_path, encrypted).unwrap();

        let state = Arc::new(Mutex::new(State::new(false)));

        // 2. Unlock Request
        let unlock_req = Request::Unlock { master_key: master_key_val.to_string() };
        let mut input_data = serde_json::to_vec(&unlock_req).unwrap();
        input_data.push(b'\n');

        // 3. GetOTP Request
        let otp_req = Request::GetOTP { account_id: acc_id.to_string() };
        input_data.extend(serde_json::to_vec(&otp_req).unwrap());
        input_data.push(b'\n');

        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        handle_client_io(&mut stream, state).unwrap();

        let resp_output = String::from_utf8(stream.output).unwrap();
        let mut resps = resp_output.lines().map(|l| serde_json::from_str::<Response>(l).unwrap());

        // First response: Unlocked
        match resps.next().unwrap() {
            Response::Unlocked(source) => {
                assert!(source.contains("Vault"));
            },
            resp => panic!("Expected Unlocked, got {:?}", resp),
        }

        // Cleanup env
        env::remove_var("JKI_HOME");
        env::remove_var("JKI_SECRETS_PATH");
    }

    #[test]
    #[serial]
    fn test_handle_client_malformed_json() {
        let state = Arc::new(Mutex::new(State::new(false)));
        let input_data = b"not a json\n";
        let mut stream = MockStream { input: Cursor::new(input_data.to_vec()), output: Vec::new() };
        handle_client_io(&mut stream, state).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Error(msg) => assert!(msg.contains("Invalid request")),
            _ => panic!("Expected Error response, got {:?}", resp),
        }
    }

    #[test]
    #[serial]
    fn test_handle_client_reload() {
        let state = Arc::new(Mutex::new(State::new(false)));
        {
            let mut s = state.lock().unwrap();
            s.secrets = Some(HashMap::new()); // Set as unlocked
        }
        
        let req = Request::Reload;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        handle_client_io(&mut stream, state.clone()).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Success => {},
            _ => panic!("Expected Success, got {:?}", resp),
        }
        
        // Verify secrets cleared but master_key persists if it was there
        let s = state.lock().unwrap();
        assert!(s.secrets.is_none());
    }

    #[test]
    #[serial]
    fn test_handle_client_get_master_key() {
        use secrecy::SecretString;
        let state = Arc::new(Mutex::new(State::new(false)));
        let key = "secret-pass";
        {
            let mut s = state.lock().unwrap();
            s.master_key = Some(SecretString::from(key.to_string()));
        }
        
        let req = Request::GetMasterKey;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        handle_client_io(&mut stream, state).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::MasterKey(k) => assert_eq!(k, key),
            _ => panic!("Expected MasterKey, got {:?}", resp),
        }
    }

    #[test]
    #[serial]
    fn test_force_age_refusal() {
        use tempfile::tempdir;
        use std::env;

        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_force_age");
        std::fs::create_dir_all(&home).unwrap();
        
        let dec_path = home.join("vault.secrets.json");
        env::set_var("JKI_HOME", home.to_str().unwrap());
        env::set_var("JKI_DECRYPTED_SECRETS_PATH", dec_path.to_str().unwrap());
        env::remove_var("JKI_SECRETS_PATH"); // Ensure .age is not found

        // Create plaintext vault only
        let acc_id = "test-id";
        let mut secrets_map = HashMap::new();
        secrets_map.insert(acc_id.to_string(), AccountSecret {
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        });
        let sec_json = serde_json::to_vec(&secrets_map).unwrap();
        std::fs::write(&dec_path, sec_json).unwrap();

        // Enable force_age
        let state = Arc::new(Mutex::new(State::new(true)));

        let master_key_val = "testpass";
        let unlock_req = Request::Unlock { master_key: master_key_val.to_string() };
        let mut input_data = serde_json::to_vec(&unlock_req).unwrap();
        input_data.push(b'\n');

        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        handle_client_io(&mut stream, state).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Error(msg) => assert!(msg.contains("Force-age mode enabled: Encrypted vault missing")),
            _ => panic!("Expected Error (force-age), got {:?}", resp),
        }

        // Cleanup env
        env::remove_var("JKI_HOME");
        env::remove_var("JKI_DECRYPTED_SECRETS_PATH");
    }

}
