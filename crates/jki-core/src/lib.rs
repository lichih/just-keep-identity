use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
use secrecy::SecretString;
use std::io::{Read, Write};

pub mod import;
pub mod paths;
pub mod keychain;

#[derive(SerdeDeserialize, SerdeSerialize, Debug, Clone, PartialEq)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub issuer: Option<String>,
    pub account_type: AccountType,
    
    // 這些欄位僅在記憶體整合後存在，不應出現在 metadata.json 中
    #[serde(skip_serializing, default)]
    pub secret: String,
    #[serde(skip_serializing, default = "default_digits")]
    pub digits: u32,
    #[serde(skip_serializing, default = "default_algorithm")]
    pub algorithm: String,
}

fn default_digits() -> u32 { 6 }
fn default_algorithm() -> String { "SHA1".to_string() }

impl Account {
    pub fn to_otpauth_uri(&self) -> String {
        use urlencoding::encode;
        let label = if let Some(ref issuer) = self.issuer {
            format!("{}:{}", encode(issuer), encode(&self.name))
        } else {
            encode(&self.name).into_owned()
        };

        let mut uri = format!(
            "otpauth://totp/{}?secret={}&digits={}&algorithm={}",
            label,
            self.secret,
            self.digits,
            self.algorithm.to_uppercase()
        );

        if let Some(ref issuer) = self.issuer {
            uri.push_str(&format!("&issuer={}", encode(issuer)));
        }

        uri
    }
}

#[derive(SerdeDeserialize, SerdeSerialize, Debug, Clone, PartialEq)]
pub enum AccountType {
    Standard,
    Steam,
    Blizzard,
}

#[derive(SerdeDeserialize, SerdeSerialize, Debug, Clone)]
pub struct AccountSecret {
    pub secret: String,
    pub digits: u32,
    pub algorithm: String,
}

use std::collections::HashMap;

pub fn integrate_accounts(metadata: Vec<Account>, secrets: &HashMap<String, AccountSecret>) -> (Vec<Account>, Vec<String>) {
    let mut integrated = Vec::new();
    let mut missing = Vec::new();
    for mut acc in metadata {
        if let Some(s) = secrets.get(&acc.id) {
            acc.secret = s.secret.clone();
            acc.digits = s.digits;
            acc.algorithm = s.algorithm.clone();
            integrated.push(acc);
        } else {
            missing.push(acc.name.clone());
        }
    }
    (integrated, missing)
}

use totp_rs::{Algorithm, TOTP, Secret};

pub fn generate_otp(acc: &Account) -> Result<String, String> {
    let secret_str = acc.secret.trim().replace(" ", "");
    let secret = Secret::Encoded(secret_str).to_bytes().map_err(|e| e.to_string())?;
    
    // 使用 new_unchecked 繞過 RFC 對長度的強硬要求 (128 bits)
    let totp = TOTP::new_unchecked(
        Algorithm::SHA1, 
        acc.digits as usize, 
        1, 
        30, 
        secret
    );
    
    Ok(totp.generate_current().unwrap())
}

// --- 加解密核心 ---

pub fn encrypt_with_master_key(data: &[u8], master_key: &SecretString) -> Result<Vec<u8>, String> {
    let encryptor = age::Encryptor::with_user_passphrase(master_key.clone());
    let mut encrypted = vec![];
    let mut writer = encryptor.wrap_output(&mut encrypted).map_err(|e| e.to_string())?;
    writer.write_all(data).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(encrypted)
}

pub fn decrypt_with_master_key(encrypted_data: &[u8], master_key: &SecretString) -> Result<Vec<u8>, String> {
    let decryptor = match age::Decryptor::new(encrypted_data).map_err(|e| e.to_string())? {
        age::Decryptor::Passphrase(d) => d,
        _ => return Err("Expected passphrase-encrypted data".to_string()),
    };
    let mut reader = decryptor.decrypt(master_key, None).map_err(|e| e.to_string())?;
    let mut decrypted = vec![];
    reader.read_to_end(&mut decrypted).map_err(|e| e.to_string())?;
    Ok(decrypted)
}

// --- 搜尋邏輯 ---

pub fn fuzzy_match(pattern: &str, target: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let target = target.to_lowercase();
    let mut target_chars = target.chars();
    for p in pattern.chars() {
        match target_chars.by_ref().find(|&t| t == p) {
            Some(_) => continue,
            None => return false,
        }
    }
    true
}

pub fn search_accounts(accounts: &[Account], patterns: &[String]) -> Vec<Account> {
    accounts.iter()
        .filter(|acc| {
            let issuer = acc.issuer.as_deref().unwrap_or_default();
            let name = &acc.name;
            
            // AND 邏輯：每個關鍵字都必須在任一欄位中找到匹配
            patterns.iter().all(|p| {
                fuzzy_match(p, issuer) || fuzzy_match(p, name)
            })
        })
        .cloned()
        .collect()
}

// --- 互動抽象 (用於 Mock 測試) ---

pub trait Interactor {
    fn prompt_password(&self, prompt: &str) -> Result<SecretString, String>;
    fn confirm(&self, prompt: &str, default: bool) -> bool;
}

pub struct TerminalInteractor;

impl Interactor for TerminalInteractor {
    fn prompt_password(&self, prompt: &str) -> Result<SecretString, String> {
        use crossterm::{
            event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
            terminal::{disable_raw_mode, enable_raw_mode},
            execute, cursor, style::Print,
        };
        use std::io::{self, Write};

        if !atty::is(atty::Stream::Stdin) {
            let mut line = String::new();
            io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
            return Ok(SecretString::from(line.trim().to_string()));
        }

        enable_raw_mode().map_err(|e| e.to_string())?;
        let mut stderr = io::stderr();
        execute!(stderr, Print(format!("{}: [ ", prompt)), cursor::SavePosition, Print("_ ]"), cursor::RestorePosition).ok();
        stderr.flush().ok();

        let mut password = String::new();
        let mut toggle = false;

        let result = loop {
            if let Ok(Event::Key(KeyEvent { code, modifiers, .. })) = event::read() {
                match code {
                    KeyCode::Enter => {
                        execute!(stderr, cursor::RestorePosition, cursor::MoveRight(2), Print("\r\n")).ok();
                        break Ok(SecretString::from(password));
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        execute!(stderr, cursor::RestorePosition, cursor::MoveRight(2), Print("\r\nCancelled\r\n")).ok();
                        break Err("Interrupted".to_string());
                    }                    KeyCode::Char(c) => { password.push(c); toggle = !toggle; }
                    KeyCode::Backspace => { if !password.is_empty() { password.pop(); toggle = !toggle; } }
                    _ => continue,
                }
                let symbol = if password.is_empty() { "_" } else if toggle { "*" } else { "x" };
                execute!(stderr, cursor::RestorePosition, Print(symbol), cursor::RestorePosition).ok();
                stderr.flush().ok();
            }
        };
        disable_raw_mode().ok();
        result
    }

    fn confirm(&self, prompt: &str, default: bool) -> bool {
        use std::io::{self, Write};
        let options = if default { "[Y/n]" } else { "[y/N]" };
        print!("{} {}: ", prompt, options);
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();
        if input.is_empty() {
            return default;
        }
        input == "y"
    }
}

pub struct MockInteractor {
    pub passwords: std::cell::RefCell<Vec<String>>,
    pub confirms: std::cell::RefCell<Vec<bool>>,
}

impl Interactor for MockInteractor {
    fn prompt_password(&self, _prompt: &str) -> Result<SecretString, String> {
        if self.passwords.borrow().is_empty() {
            return Err("No mock password provided".to_string());
        }
        Ok(SecretString::from(self.passwords.borrow_mut().remove(0)))
    }

    fn confirm(&self, _prompt: &str, default: bool) -> bool {
        if self.confirms.borrow().is_empty() {
            return default;
        }
        self.confirms.borrow_mut().remove(0)
    }
}

#[derive(clap::ValueEnum, serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthSource {
    #[default]
    Auto,
    Agent,
    Interactive,
    Keyfile,
    Keychain,
    Plaintext,
    Biometric,
}

pub fn acquire_master_key(
    source: AuthSource,
    interactor: &dyn Interactor,
    secret_store: Option<&dyn keychain::SecretStore>,
) -> Result<SecretString, String> {
    use crate::paths::JkiPath;

    match source {
        AuthSource::Auto => {
            // 1. Try Agent first (Session aware)
            if let Ok(key) = agent::AgentClient::get_master_key() {
                return Ok(key);
            }

            // 2. Try Secret Store (Keychain/Keyring)
            if let Some(store) = secret_store {
                if let Ok(key) = store.get_secret("jki", "master_key") {
                    return Ok(key);
                }
            }

            // 3. Try master.key file
            let key_path = JkiPath::master_key_path();
            if key_path.exists() {
                if JkiPath::check_secure_permissions(&key_path).is_ok() {
                    let content = std::fs::read_to_string(key_path).map_err(|e| e.to_string())?;
                    return Ok(SecretString::from(content.trim().to_string()));
                }
            }

            // 4. Fallback to interactive prompt
            interactor.prompt_password("Enter Master Key")
        }
        AuthSource::Agent => {
            agent::AgentClient::get_master_key().map_err(|e| format!("Agent auth failed: {}", e))
        }
        AuthSource::Interactive => {
            interactor.prompt_password("Enter Master Key")
        }
        AuthSource::Keyfile => {
            let key_path = JkiPath::master_key_path();
            if key_path.exists() {
                if JkiPath::check_secure_permissions(&key_path).is_ok() {
                    let content = std::fs::read_to_string(key_path).map_err(|e| e.to_string())?;
                    return Ok(SecretString::from(content.trim().to_string()));
                } else {
                    return Err("Keyfile auth failed: Insecure permissions".to_string());
                }
            }
            Err("Keyfile auth failed: File missing".to_string())
        }
        AuthSource::Keychain => {
            if let Some(store) = secret_store {
                store.get_secret("jki", "master_key").map_err(|e| format!("Keychain auth failed: {}", e))
            } else {
                Err("Keychain auth failed: Store not provided".to_string())
            }
        }
        AuthSource::Biometric => {
            // For now, biometric is linked to agent which might trigger OS prompt
            agent::AgentClient::get_master_key().map_err(|e| format!("Biometric (Agent) auth failed: {}", e))
        }
        AuthSource::Plaintext => {
            Err("Plaintext auth source not applicable for master key acquisition".to_string())
        }
    }
}

pub fn ensure_agent_running(quiet: bool) -> bool {
    use crate::paths::JkiPath;
    use interprocess::local_socket::LocalSocketStream;
    use std::process;

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
    let mut agent_exe = current_exe.parent().unwrap().join("jki-agent");
    
    // Handle cargo test/run where binaries might be in the parent directory of 'deps'
    if !agent_exe.exists() {
        if let Some(parent) = current_exe.parent() {
            if parent.ends_with("deps") {
                if let Some(grandparent) = parent.parent() {
                    let alt_agent_exe = grandparent.join("jki-agent");
                    if alt_agent_exe.exists() {
                        agent_exe = alt_agent_exe;
                    }
                }
            }
        }
    }
    
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

pub mod agent {
    use serde::{Deserialize, Serialize};
    use std::io::{Write, BufReader, BufRead};
    use interprocess::local_socket::LocalSocketStream;
    use crate::paths::JkiPath;
    use secrecy::{SecretString, ExposeSecret};

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Request {
        Ping,
        Unlock { master_key: String },
        GetOTP { account_id: String },
        GetMasterKey,
        Reload,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Response {
        Pong,
        Unlocked(String),
        OTP(String),
        MasterKey(String),
        Success,
        Error(String),
    }

    pub struct AgentClient;

    impl AgentClient {
        fn connect() -> Result<LocalSocketStream, String> {
            let socket_path = JkiPath::agent_socket_path();
            let name = socket_path.to_str().ok_or("Invalid socket path")?;
            LocalSocketStream::connect(name).map_err(|e| e.to_string())
        }

        fn call(req: Request) -> Result<Response, String> {
            let mut stream = Self::connect()?;
            let req_json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
            stream.write_all(format!("{}\n", req_json).as_bytes()).map_err(|e| e.to_string())?;
            stream.flush().map_err(|e| e.to_string())?;

            let mut line = String::new();
            let mut reader = BufReader::new(stream);
            reader.read_line(&mut line).map_err(|e| e.to_string())?;
            serde_json::from_str(&line).map_err(|e| e.to_string())
        }

        pub fn ping() -> bool {
            match Self::call(Request::Ping) {
                Ok(Response::Pong) => true,
                _ => false,
            }
        }

        pub fn unlock(master_key: &SecretString) -> Result<String, String> {
            match Self::call(Request::Unlock { master_key: master_key.expose_secret().clone() }) {
                Ok(Response::Unlocked(source)) => Ok(source),
                Ok(Response::Error(e)) => Err(e),
                _ => Err("Invalid agent response".to_string()),
            }
        }

        pub fn get_otp(account_id: &str) -> Result<String, String> {
            match Self::call(Request::GetOTP { account_id: account_id.to_string() }) {
                Ok(Response::OTP(otp)) => Ok(otp),
                Ok(Response::Error(e)) => Err(e),
                _ => Err("Invalid agent response".to_string()),
            }
        }

        pub fn get_master_key() -> Result<SecretString, String> {
            match Self::call(Request::GetMasterKey) {
                Ok(Response::MasterKey(key)) => Ok(SecretString::from(key)),
                Ok(Response::Error(e)) => Err(e),
                _ => Err("Agent is locked".to_string()),
            }
        }

        pub fn reload() -> Result<(), String> {
            match Self::call(Request::Reload) {
                Ok(Response::Success) => Ok(()),
                Ok(Response::Error(e)) => Err(e),
                _ => Err("Invalid agent response".to_string()),
            }
        }
    }
}

pub mod git {
    use std::process::Command;
    use std::path::Path;

    pub struct GitRepoStatus {
        pub branch: String,
        pub is_clean: bool,
        pub has_remote: bool,
    }

    pub fn check_status(repo_path: &Path) -> Option<GitRepoStatus> {
        if !repo_path.join(".git").exists() {
            return None;
        }
        let b = Command::new("git")
            .args(["-C", repo_path.to_str()?, "rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()?;
        let s = Command::new("git")
            .args(["-C", repo_path.to_str()?, "status", "--porcelain"])
            .output()
            .ok()?;
        let r = Command::new("git")
            .args(["-C", repo_path.to_str()?, "remote"])
            .output()
            .ok()?;
        Some(GitRepoStatus {
            branch: String::from_utf8_lossy(&b.stdout).trim().to_string(),
            is_clean: s.stdout.is_empty(),
            has_remote: !r.stdout.is_empty(),
        })
    }

    pub fn add_all(repo_path: &Path) -> Result<(), String> {
        let status = Command::new("git")
            .args(["-C", repo_path.to_str().ok_or("Invalid path")?, "add", "."])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git add failed".to_string())
        }
    }

    pub fn add(repo_path: &Path, files: &[String]) -> Result<(), String> {
        if files.is_empty() { return Ok(()); }
        let mut args = vec!["-C", repo_path.to_str().ok_or("Invalid path")?, "add", "--"];
        for f in files {
            args.push(f);
        }
        let status = Command::new("git")
            .args(args)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git add failed".to_string())
        }
    }

    pub fn commit(repo_path: &Path, message: &str) -> Result<bool, String> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or("Invalid path")?,
                "commit",
                "-m",
                message,
            ])
            .status()
            .map_err(|e| e.to_string())?;
        Ok(status.success())
    }

    pub fn pull_rebase(repo_path: &Path) -> Result<(), String> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or("Invalid path")?,
                "pull",
                "--rebase",
            ])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git pull --rebase failed".to_string())
        }
    }

    pub fn get_conflicting_files(repo_path: &Path) -> Result<Vec<String>, String> {
        let output = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or("Invalid path")?,
                "diff",
                "--name-only",
                "--diff-filter=U",
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&output.stdout);
        Ok(s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
    }

    pub fn checkout_theirs(repo_path: &Path, files: &[String]) -> Result<(), String> {
        if files.is_empty() { return Ok(()); }
        let mut args = vec!["-C", repo_path.to_str().ok_or("Invalid path")?, "checkout", "--theirs", "--"];
        for f in files {
            args.push(f);
        }
        let status = Command::new("git")
            .args(args)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git checkout --theirs failed".to_string())
        }
    }

    pub fn rebase_continue(repo_path: &Path) -> Result<(), String> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or("Invalid path")?,
                "rebase",
                "--continue",
            ])
            .env("GIT_EDITOR", "true") // Skip editor for commit message
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git rebase --continue failed".to_string())
        }
    }

    pub fn rebase_abort(repo_path: &Path) -> Result<(), String> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or("Invalid path")?,
                "rebase",
                "--abort",
            ])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git rebase --abort failed".to_string())
        }
    }

    pub fn push(repo_path: &Path) -> Result<(), String> {
        let status = Command::new("git")
            .args(["-C", repo_path.to_str().ok_or("Invalid path")?, "push"])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("git push failed".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use crate::keychain::SecretStore;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("gg", "Google"));
        assert!(fuzzy_match("goog", "Google"));
        assert!(fuzzy_match("gle", "Google"));
        assert!(fuzzy_match("G", "Google"));
        assert!(!fuzzy_match("ga", "Google"));
    }

    #[test]
    #[serial]
    fn test_search_accounts() {
        let accounts = vec![
            Account {
                id: "1".to_string(),
                name: "lichihwu@gmail.com".to_string(),
                issuer: Some("Google".to_string()),
                account_type: AccountType::Standard,
                secret: "".to_string(),
                digits: 6,
                algorithm: "SHA1".to_string(),
            },
            Account {
                id: "2".to_string(),
                name: "Facebook".to_string(),
                issuer: None,
                account_type: AccountType::Standard,
                secret: "".to_string(),
                digits: 6,
                algorithm: "SHA1".to_string(),
            },
            Account {
                id: "3".to_string(),
                name: "lichih".to_string(),
                issuer: Some("GitHub".to_string()),
                account_type: AccountType::Standard,
                secret: "".to_string(),
                digits: 6,
                algorithm: "SHA1".to_string(),
            },
        ];

        // 1. 驗證 "gh" 只會匹配 GitHub，不會誤中 Google-lichih (Field Isolation)
        let results = search_accounts(&accounts, &vec!["gh".to_string()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].issuer.as_deref(), Some("GitHub"));

        // 2. 驗證 "g li" AND 邏輯
        // Google-lichihwu: g match Google, li match lichihwu (Match)
        // GitHub-lichih: g match GitHub, li match lichih (Match)
        let results = search_accounts(&accounts, &vec!["g".to_string(), "li".to_string()]);
        assert_eq!(results.len(), 2);

        // 3. 驗證 "li" 匹配兩個 (fuzzy 匹配成功)
        let results = search_accounts(&accounts, &vec!["li".to_string()]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    #[serial]
    fn test_crypto_roundtrip() {
        let master_key = SecretString::from("correct horse battery staple".to_string());
        let data = b"sensitive data";
        
        let encrypted = encrypt_with_master_key(data, &master_key).unwrap();
        let decrypted = decrypt_with_master_key(&encrypted, &master_key).unwrap();
        
        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    #[serial]
    fn test_crypto_wrong_password() {
        let master_key = SecretString::from("correct horse battery staple".to_string());
        let wrong_key = SecretString::from("wrong password".to_string());
        let data = b"sensitive data";
        
        let encrypted = encrypt_with_master_key(data, &master_key).unwrap();
        let decrypted = decrypt_with_master_key(&encrypted, &wrong_key);
        
        assert!(decrypted.is_err());
    }

    #[test]
    #[serial]
    fn test_git_check_status() {
        use std::process::Command;
        use tempfile::tempdir;
        
        let dir = tempdir().unwrap();
        let repo_path = dir.path();
        
        // 1. Not a git repo
        assert!(git::check_status(repo_path).is_none());
        
        // 2. Init git repo
        Command::new("git").args(["init"]).current_dir(repo_path).output().unwrap();
        // Need a commit to have a branch
        Command::new("git").args(["config", "user.email", "you@example.com"]).current_dir(repo_path).output().unwrap();
        Command::new("git").args(["config", "user.name", "Your Name"]).current_dir(repo_path).output().unwrap();
        std::fs::write(repo_path.join("file"), "content").unwrap();
        Command::new("git").args(["add", "."]).current_dir(repo_path).output().unwrap();
        Command::new("git").args(["commit", "-m", "initial"]).current_dir(repo_path).output().unwrap();
        
        let status = git::check_status(repo_path).unwrap();
        assert!(status.branch == "master" || status.branch == "main");
        assert!(status.is_clean);
        assert!(!status.has_remote);
    }

    #[test]
    #[serial]
    fn test_git_operations() {
        use std::process::Command;
        use tempfile::tempdir;
        
        let dir = tempdir().unwrap();
        let repo_path = dir.path();
        
        Command::new("git").args(["init"]).current_dir(repo_path).output().unwrap();
        Command::new("git").args(["config", "user.email", "you@example.com"]).current_dir(repo_path).output().unwrap();
        Command::new("git").args(["config", "user.name", "Your Name"]).current_dir(repo_path).output().unwrap();
        
        // Test add_all
        std::fs::write(repo_path.join("file1"), "content").unwrap();
        git::add_all(repo_path).unwrap();
        let s = git::check_status(repo_path).unwrap();
        assert!(!s.is_clean);
        
        // Test commit
        assert!(git::commit(repo_path, "feat: initial").unwrap());
        let s = git::check_status(repo_path).unwrap();
        assert!(s.is_clean);

        // Test commit no changes
        assert!(!git::commit(repo_path, "no change").unwrap());
    }

    #[test]
    #[serial]
    fn test_agent_ipc_serialization() {
        let req = agent::Request::Ping;
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, "\"Ping\"");

        let req = agent::Request::GetOTP { account_id: "123".to_string() };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("account_id"));
        assert!(json.contains("123"));

        let resp = agent::Response::OTP("123456".to_string());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("OTP"));
        assert!(json.contains("123456"));
    }

    #[test]
    #[serial]
    fn test_integrate_accounts() {
        let metadata = vec![
            Account { id: "1".to_string(), name: "A".to_string(), issuer: None, account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "".to_string() },
            Account { id: "2".to_string(), name: "B".to_string(), issuer: None, account_type: AccountType::Standard, secret: "".to_string(), digits: 6, algorithm: "".to_string() },
        ];
        let mut secrets = HashMap::new();
        secrets.insert("1".to_string(), AccountSecret { secret: "S1".to_string(), digits: 6, algorithm: "SHA1".to_string() });
        
        let (integrated, missing) = integrate_accounts(metadata, &secrets);
        assert_eq!(integrated.len(), 1);
        assert_eq!(integrated[0].id, "1");
        assert_eq!(integrated[0].secret, "S1");
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "B");
    }

    #[test]
    #[serial]
    fn test_acquire_master_key_priority() {
        use tempfile::tempdir;
        use std::env;
        use crate::keychain::tests::MockSecretStore;

        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_priority");
        std::fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let interactor = MockInteractor {
            passwords: std::cell::RefCell::new(vec!["interactive_pass".to_string()]),
            confirms: std::cell::RefCell::new(vec![]),
        };

        let mock_store = MockSecretStore::new();
        let key_path = crate::paths::JkiPath::master_key_path();

        // 1. Force Interactive
        let key = acquire_master_key(AuthSource::Interactive, &interactor, Some(&mock_store)).unwrap();
        assert_eq!(key.expose_secret(), "interactive_pass");

        // 2. Secret Store (Keychain) Priority
        mock_store.set_secret("jki", "master_key", "keychain_pass").unwrap();
        std::fs::write(&key_path, "file_pass").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }

        let key = acquire_master_key(AuthSource::Auto, &interactor, Some(&mock_store)).unwrap();
        assert_eq!(key.expose_secret(), "keychain_pass");

        // 3. File Priority (when keychain is missing)
        mock_store.delete_secret("jki", "master_key").unwrap();
        let key = acquire_master_key(AuthSource::Auto, &interactor, Some(&mock_store)).unwrap();
        assert_eq!(key.expose_secret(), "file_pass");

        // 4. Interactive Fallback (when both missing)
        std::fs::remove_file(&key_path).unwrap();
        interactor.passwords.borrow_mut().push("interactive_fallback".to_string());
        let key = acquire_master_key(AuthSource::Auto, &interactor, Some(&mock_store)).unwrap();
        assert_eq!(key.expose_secret(), "interactive_fallback");
    }

    #[test]
    fn test_to_otpauth_uri() {
        let acc = Account {
            id: "test-id".to_string(),
            name: "user@example.com".to_string(),
            issuer: Some("Google".to_string()),
            account_type: AccountType::Standard,
            secret: "JBSWY3DPEHPK3PXP".to_string(),
            digits: 6,
            algorithm: "SHA1".to_string(),
        };
        let uri = acc.to_otpauth_uri();
        assert_eq!(uri, "otpauth://totp/Google:user%40example.com?secret=JBSWY3DPEHPK3PXP&digits=6&algorithm=SHA1&issuer=Google");

        let acc_no_issuer = Account {
            id: "test-id-2".to_string(),
            name: "standalone".to_string(),
            issuer: None,
            account_type: AccountType::Standard,
            secret: "SECRET123".to_string(),
            digits: 8,
            algorithm: "SHA256".to_string(),
        };
        let uri2 = acc_no_issuer.to_otpauth_uri();
        assert_eq!(uri2, "otpauth://totp/standalone?secret=SECRET123&digits=8&algorithm=SHA256");
    }
}
