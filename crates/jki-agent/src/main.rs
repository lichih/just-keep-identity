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
use anyhow::{Context, anyhow};

mod tray;

#[derive(Parser, Debug)]
#[command(author, version, about = "jki-agent - Just Keep Identity Agent", long_about = None)]
struct Args {
    /// Authentication and data source
    #[arg(short = 'A', long, default_value = "auto")]
    pub auth: AuthSource,
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

    fn unlock(&mut self, master_key: secrecy::SecretString) -> anyhow::Result<String> {
        let sec_path = JkiPath::secrets_path();
        let decrypted_path = JkiPath::decrypted_secrets_path();

        let source = if sec_path.exists() && self.auth != AuthSource::Plaintext {
            let sec_encrypted = std::fs::read(&sec_path).context("Failed to read encrypted vault")?;
            let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key).map_err(|e| anyhow!(e))?;
            let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).context("Failed to parse vault secrets")?;

            self.secrets = Some(secrets_map);
            self.last_unlocked = Some(Instant::now());
            "Encrypted Vault".to_string()
        } else if decrypted_path.exists() {
            let sec_json = std::fs::read(&decrypted_path).context("Failed to read plaintext vault")?;
            let secrets_map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).context("Failed to parse plaintext secrets")?;

            self.secrets = Some(secrets_map);
            self.last_unlocked = Some(Instant::now());
            "Plaintext Vault".to_string()
        } else {
            return Err(anyhow!("Secrets file missing (neither .age nor .json found)"));
        };

        self.master_key = Some(master_key);
        Ok(source)
    }

    fn unlock_with_biometric(&mut self) -> anyhow::Result<String> {
        use jki_core::keychain::{KeyringStore, SecretStore};
        
        let store = KeyringStore;
        let master_key = store.get_secret("jki", "master_key")
            .map_err(|e| anyhow!("Failed to retrieve master key from keychain: {}", e))?;
            
        let sec_path = JkiPath::secrets_path();
        let decrypted_path = JkiPath::decrypted_secrets_path();

        let (secrets_map, source) = if sec_path.exists() {
            let sec_encrypted = std::fs::read(&sec_path).context("Failed to read encrypted vault")?;
            let sec_json = decrypt_with_master_key(&sec_encrypted, &master_key).map_err(|e| anyhow!(e))?;
            let map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).context("Failed to parse vault secrets")?;
            (map, "Encrypted Vault")
        } else if decrypted_path.exists() {
            let sec_json = std::fs::read(&decrypted_path).context("Failed to read plaintext vault")?;
            let map: HashMap<String, AccountSecret> = serde_json::from_slice(&sec_json).context("Failed to parse plaintext secrets")?;
            (map, "Plaintext Vault")
        } else {
            return Err(anyhow!("Secrets file missing (neither .age nor .json found)"));
        };

        self.secrets = Some(secrets_map);
        self.master_key = Some(master_key);
        self.last_unlocked = Some(Instant::now());
        
        Ok(source.to_string())
    }

    fn get_otp(&mut self, account_id: &str) -> anyhow::Result<String> {
        self.check_ttl();
        
        if self.secrets.is_none() {
            if let Some(key) = self.master_key.clone() {
                let _ = self.unlock(key)?;
            }
        }

        let secrets = self.secrets.as_ref().ok_or_else(|| anyhow!("Agent is locked"))?;
        let secret = secrets.get(account_id).ok_or_else(|| anyhow!("Account not found"))?;
        
        let acc = Account {
            id: account_id.to_string(),
            name: "".to_string(),
            issuer: None,
            account_type: AccountType::Standard,
            secret: secret.secret.clone(),
            digits: secret.digits,
            algorithm: secret.algorithm.clone(),
        };

        generate_otp(&acc).map_err(|e| anyhow!(e))
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut auth = args.auth;
    
    if std::env::var("JKI_FORCE_AGE").map(|v| v == "1").unwrap_or(false) && auth == AuthSource::Auto {
        auth = AuthSource::Agent;
    }

    if auth == AuthSource::Biometric {
        println!("Biometric auth mode requested (agent)");
    }

    let socket_path = JkiPath::agent_socket_path();
    let name = socket_path.to_str().ok_or_else(|| anyhow!("Invalid socket path"))?.to_string();

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

    if socket_path.exists() && !cfg!(windows) {
        let _ = std::fs::remove_file(&socket_path);
    }

    println!("jki-agent starting (auth: {:?})", auth);

    let state = Arc::new(Mutex::new(State::new(auth)));
    let state_clone = Arc::clone(&state);

    let (shutdown_tx, shutdown_rx) = std::sync::mpsc::channel();
    let shutdown_tx_clone = shutdown_tx.clone();

    if auth == AuthSource::Biometric {
        let mut s = state.lock().map_err(|_| anyhow!("Failed to lock state"))?;
        match s.unlock_with_biometric() {
            Ok(src) => println!("Biometric unlock successful: {}", src),
            Err(e) => {
                eprintln!("CRITICAL: Biometric unlock failed: {}. Exit.", e);
                std::process::exit(1);
            }
        }
    }

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
    
    {
        let s = state.lock().map_err(|_| anyhow!("Failed to lock state"))?;
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
            let s = state.lock().unwrap();
            tray_handler.update_status(&s);
        }

        match event {
            Event::MainEventsCleared => {
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
                let _ = s.write_all(format!("{}\n", serde_json::to_string(&resp).unwrap()).as_bytes());
                let _ = s.flush();
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
        let _ = s.write_all(format!("{}\n", resp_json).as_bytes());
        let _ = s.flush();

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
        
        let sec_path = home.join("vault.secrets.bin.age");
        env::set_var("JKI_HOME", home.to_str().unwrap());
        env::set_var("JKI_SECRETS_PATH", sec_path.to_str().unwrap());

        let master_key_val = "testpass";
        let master_key = SecretString::from(master_key_val.to_string());
        
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

        let unlock_req = Request::Unlock { master_key: master_key_val.to_string() };
        let mut input_data = serde_json::to_vec(&unlock_req).unwrap();
        input_data.push(b'\n');

        let otp_req = Request::GetOTP { account_id: acc_id.to_string() };
        input_data.extend(serde_json::to_vec(&otp_req).unwrap());
        input_data.push(b'\n');

        let mut stream = MockStream { input: Cursor::new(input_data), output: Vec::new() };
        let (tx, _) = std::sync::mpsc::channel();
        handle_client_io(&mut stream, state, tx).unwrap();

        let resp_output = String::from_utf8(stream.output).unwrap();
        let mut resps = resp_output.lines().map(|l| serde_json::from_str::<Response>(l).unwrap());

        match resps.next().unwrap() {
            Response::Unlocked(source) => {
                assert!(source.contains("Vault"));
            },
            resp => panic!("Expected Unlocked, got {:?}", resp),
        }

        env::remove_var("JKI_HOME");
        env::remove_var("JKI_SECRETS_PATH");
    }

}
