use rkyv::{Archive, Deserialize, Serialize};
use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
use secrecy::SecretString;
use std::io::{Read, Write};

pub mod import;
pub mod paths;
pub mod keychain;

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone, PartialEq)]
#[archive(check_bytes)]
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

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone, PartialEq)]
#[archive(check_bytes)]
pub enum AccountType {
    Standard,
    Steam,
    Blizzard,
}

#[derive(Archive, Deserialize, Serialize, SerdeDeserialize, SerdeSerialize, Debug, Clone)]
#[archive(check_bytes)]
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

pub fn acquire_master_key(force_interactive: bool, interactor: &dyn Interactor) -> Result<SecretString, String> {
    use crate::paths::JkiPath;

    if !force_interactive {
        let key_path = JkiPath::master_key_path();
        if key_path.exists() {
            if JkiPath::check_secure_permissions(&key_path).is_ok() {
                let content = std::fs::read_to_string(key_path).map_err(|e| e.to_string())?;
                return Ok(SecretString::from(content.trim().to_string()));
            }
        }
    }

    interactor.prompt_password("Enter Master Key")
}

pub mod agent {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Request {
        Ping,
        Unlock { master_key: String },
        GetOTP { account_id: String },
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Response {
        Pong,
        Unlocked,
        OTP(String),
        Error(String),
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

    #[test]
    fn test_fuzzy_match() {
        assert!(fuzzy_match("gg", "Google"));
        assert!(fuzzy_match("goog", "Google"));
        assert!(fuzzy_match("gle", "Google"));
        assert!(fuzzy_match("G", "Google"));
        assert!(!fuzzy_match("ga", "Google"));
    }

    #[test]
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
    fn test_crypto_roundtrip() {
        let master_key = SecretString::from("correct horse battery staple".to_string());
        let data = b"sensitive data";
        
        let encrypted = encrypt_with_master_key(data, &master_key).unwrap();
        let decrypted = decrypt_with_master_key(&encrypted, &master_key).unwrap();
        
        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_crypto_wrong_password() {
        let master_key = SecretString::from("correct horse battery staple".to_string());
        let wrong_key = SecretString::from("wrong password".to_string());
        let data = b"sensitive data";
        
        let encrypted = encrypt_with_master_key(data, &master_key).unwrap();
        let decrypted = decrypt_with_master_key(&encrypted, &wrong_key);
        
        assert!(decrypted.is_err());
    }

    #[test]
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
    fn test_generate_otp() {
        let acc = Account {
            id: "1".to_string(),
            name: "Test".to_string(),
            issuer: None,
            account_type: AccountType::Standard,
            secret: "JBSWY3DPEHPK3PXP".to_string(), // base32 for "Hello!"
            digits: 6,
            algorithm: "SHA1".to_string(),
        };
        let otp = generate_otp(&acc).unwrap();
        assert_eq!(otp.len(), 6);
        assert!(otp.chars().all(|c| c.is_ascii_digit()));
    }
}
