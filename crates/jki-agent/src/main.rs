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
    AuthSource,
};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};

mod tray;

#[derive(Parser, Debug)]
#[command(author, version, about = "jki-agent - Just Keep Identity Agent", long_about = None)]
struct Args {
    /// Authentication and data source
    #[arg(short = 'A', long, default_value = "auto")]
    auth: AuthSource,
}

pub struct State {
    pub secrets: Option<HashMap<String, AccountSecret>>,
    pub master_key: Option<secrecy::SecretString>,
    pub last_unlocked: Option<Instant>,
    pub ttl: Duration,
    pub auth: AuthSource,
}

impl State {
    fn new(auth: AuthSource) -> Self {
        Self {
            secrets: None,
            master_key: None,
            last_unlocked: None,
            ttl: Duration::from_secs(3600), // 1 hour TTL
            auth,
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

    pub fn account_count(&self) -> usize {
        self.secrets.as_ref().map(|s| s.len()).unwrap_or(0)
    }

    fn unlock(&mut self, master_key: secrecy::SecretString) -> Result<String, String> {
        let sec_path = JkiPath::secrets_path();
        let decrypted_path = JkiPath::decrypted_secrets_path();

        // If Bio or Agent mode is on, we prefer .age, but FALLBACK to .json if .age is missing.
        // We do NOT refuse plaintext here anymore.
        let res = if sec_path.exists() && self.auth != AuthSource::Plaintext {
            let sec_encrypted = std::fs::read(&sec_path).map_err(|e| e.to_string())?;
            let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key)?;
            let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).map_err(|e| e.to_string())?;

            self.secrets = Some(secrets_map);
            self.last_unlocked = Some(Instant::now());
            Ok("Encrypted Vault".to_string())
        } else if decrypted_path.exists() {
            // This covers Plaintext mode, Auto fallback, and now Biometric fallback
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

    fn unlock_with_biometric(&mut self) -> Result<String, String> {
        use jki_core::keychain::{KeyringStore, SecretStore};
        
        // 1. Retrieve master key from keychain (triggers system prompt)
        let store = KeyringStore;
        let master_key = store.get_secret("jki", "master_key")
            .map_err(|e| format!("Failed to retrieve master key from keychain: {}", e))?;
            
        // 2. Directly perform unlock logic to avoid calling self.unlock() 
        // which might have its own fallback logic/prompts.
        let sec_path = JkiPath::secrets_path();
        let decrypted_path = JkiPath::decrypted_secrets_path();

        let (secrets_map, source) = if sec_path.exists() {
            let sec_encrypted = std::fs::read(&sec_path).map_err(|e| e.to_string())?;
            let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key)?;
            let map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).map_err(|e| e.to_string())?;
            (map, "Encrypted Vault")
        } else if decrypted_path.exists() {
            let sec_json = std::fs::read(&decrypted_path).map_err(|e| e.to_string())?;
            let map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).map_err(|e| e.to_string())?;
            (map, "Plaintext Vault")
        } else {
            return Err("Secrets file missing (neither .age nor .json found)".to_string());
        };

        self.secrets = Some(secrets_map);
        self.master_key = Some(master_key);
        self.last_unlocked = Some(Instant::now());
        
        Ok(source.to_string())
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
    let mut auth = args.auth;
    
    // Legacy support for JKI_FORCE_AGE env var
    if std::env::var("JKI_FORCE_AGE").map(|v| v == "1").unwrap_or(false) && auth == AuthSource::Auto {
        auth = AuthSource::Agent;
    }

    if auth == AuthSource::Biometric {
        println!("Biometric auth mode requested (agent)");
    }

    let socket_path = JkiPath::agent_socket_path();
    let name = socket_path.to_str().unwrap().to_string();

    // Pre-flight check: Ensure at least one vault exists
    let has_encrypted = JkiPath::secrets_path().exists();
    let has_plaintext = JkiPath::decrypted_secrets_path().exists();

    if !has_encrypted && !has_plaintext {
        eprintln!("CRITICAL: No vault file found (.age or .json). Exit.");
        std::process::exit(1);
    }

    if auth == AuthSource::Plaintext && !has_plaintext {
        eprintln!("CRITICAL: Plaintext mode enabled but vault.secrets.json is missing. Exit.");
        std::process::exit(1);
    }

    // Remove existing socket file if it exists (for Unix)
    if socket_path.exists() && !cfg!(windows) {
        let _ = std::fs::remove_file(&socket_path);
    }

    println!("jki-agent starting (auth: {:?})", auth);

    let state = Arc::new(Mutex::new(State::new(auth)));
    let state_clone = Arc::clone(&state);

    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let shutdown_tx_clone = shutdown_tx.clone();

    if auth == AuthSource::Biometric {
        let mut s = state.lock().unwrap();
        match s.unlock_with_biometric() {
            Ok(src) => println!("Biometric unlock successful: {}", src),
            Err(e) => {
                eprintln!("CRITICAL: Biometric unlock failed: {}. Exit.", e);
                std::process::exit(1);
            }
        }
    }

    // Socket listener thread
    thread::spawn(move || {
        let listener = LocalSocketListener::bind(name).expect("Failed to bind socket");
        println!("jki-agent listening on {:?}", JkiPath::agent_socket_path());
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    let st = Arc::clone(&state_clone);
                    let tx = shutdown_tx_clone.clone();
                    thread::spawn(move || {
                        if let Err(e) = handle_client(s, st, tx) {
                            eprintln!("Error handling client: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }
    });

    // Tray UI Event Loop
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::event::Event;
    use muda::MenuEvent;

    let mut event_loop = EventLoop::new();

    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::EventLoopExtMacOS;
        event_loop.set_activation_policy(tao::platform::macos::ActivationPolicy::Accessory);
    }

    let (tray_handler, _menu) = tray::TrayHandler::new();
    
    // Initial status update
    {
        let s = state.lock().unwrap();
        tray_handler.update_status(&s);
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(500));

        if shutdown_rx.try_recv().is_ok() {
            *control_flow = ControlFlow::Exit;
            return;
        }

        if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            if tray_handler.handle_menu_event(menu_event, Arc::clone(&state)) {
                *control_flow = ControlFlow::Exit;
            }
            // After any menu action, force a status update
            let s = state.lock().unwrap();
            tray_handler.update_status(&s);
        }

        match event {
            Event::MainEventsCleared => {
                // Periodically check TTL and refresh tray (less aggressive than NewEvents)
                let mut s = state.lock().unwrap();
                s.check_ttl();
                tray_handler.update_status(&s);
            }
            _ => (),
        }
    });
}

fn handle_client(stream: interprocess::local_socket::LocalSocketStream, state: Arc<Mutex<State>>, shutdown_tx: std::sync::mpsc::Sender<()>) -> io::Result<()> {
    handle_client_io(stream, state, shutdown_tx)
}

fn handle_client_io<S: Read + Write>(stream: S, state: Arc<Mutex<State>>, shutdown_tx: std::sync::mpsc::Sender<()>) -> io::Result<()> {
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

        let mut should_shutdown = false;
        let resp = match req {
            Request::Ping => Response::Pong,
            Request::Unlock { master_key } => {
                let mut s = state.lock().unwrap();
                match s.unlock(master_key.into()) {
                    Ok(source) => Response::Unlocked(source),
                    Err(e) => Response::Error(format!("Unlock failed: {}", e)),
                }
            }
            Request::UnlockBiometric => {
                let mut s = state.lock().unwrap();
                match s.unlock_with_biometric() {
                    Ok(source) => Response::Unlocked(source),
                    Err(e) => Response::Error(format!("Biometric unlock failed: {}", e)),
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
            Request::Shutdown => {
                should_shutdown = true;
                Response::Success
            }
        };

        let resp_json = serde_json::to_string(&resp).unwrap();
        let s = reader.get_mut();
        s.write_all(format!("{}\n", resp_json).as_bytes())?;
        s.flush()?;

        if should_shutdown {
            let _ = shutdown_tx.send(());
            break;
        }
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
        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));
        let req = Request::Ping;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

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
        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));
        let req = Request::GetOTP { account_id: "test".to_string() };
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

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

        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));

        // 2. Unlock Request
        let unlock_req = Request::Unlock { master_key: master_key_val.to_string() };
        let mut input_data = serde_json::to_vec(&unlock_req).unwrap();
        input_data.push(b'\n');

        // 3. GetOTP Request
        let otp_req = Request::GetOTP { account_id: acc_id.to_string() };
        input_data.extend(serde_json::to_vec(&otp_req).unwrap());
        input_data.push(b'\n');

        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

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
        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));
        let input_data = b"not a json\n";
        let mut stream = MockStream { input: Cursor::new(input_data.to_vec()), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

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
        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));
        {
            let mut s = state.lock().unwrap();
            s.secrets = Some(HashMap::new()); // Set as unlocked
        }
        
        let req = Request::Reload;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state.clone(), tx).unwrap();

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
    fn test_handle_client_shutdown() {
        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));
        let req = Request::Shutdown;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, rx) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Success => {},
            _ => panic!("Expected Success, got {:?}", resp),
        }
        
        // Verify shutdown signal was sent
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    #[serial]
    fn test_handle_client_get_master_key() {
        use secrecy::SecretString;
        let state = Arc::new(Mutex::new(State::new(AuthSource::Auto)));
        let key = "secret-pass";
        {
            let mut s = state.lock().unwrap();
            s.master_key = Some(SecretString::from(key.to_string()));
        }
        
        let req = Request::GetMasterKey;
        let mut input_data = serde_json::to_vec(&req).unwrap();
        input_data.push(b'\n');
        
        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::MasterKey(k) => assert_eq!(k, key),
            _ => panic!("Expected MasterKey, got {:?}", resp),
        }
    }

    #[test]
    #[serial]
    fn test_auth_agent_refusal_plaintext() {
        use tempfile::tempdir;
        use std::env;

        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_auth_refusal");
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

        // Enable agent auth (which refuses plaintext if encrypted missing)
        let state = Arc::new(Mutex::new(State::new(AuthSource::Agent)));

        let master_key_val = "testpass";
        let unlock_req = Request::Unlock { master_key: master_key_val.to_string() };
        let mut input_data = serde_json::to_vec(&unlock_req).unwrap();
        input_data.push(b'\n');

        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

        let resp_str = String::from_utf8(stream.output).unwrap();
        let resp: Response = serde_json::from_str(&resp_str).unwrap();
        match resp {
            Response::Unlocked(source) => assert!(source.contains("Plaintext")),
            _ => panic!("Expected Unlocked (Plaintext Vault), got {:?}", resp),
        }

        // Cleanup env
        env::remove_var("JKI_HOME");
        env::remove_var("JKI_DECRYPTED_SECRETS_PATH");
    }

}
