use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
use secrecy::SecretString;
use std::io::{Read, Write};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

pub mod import;
pub mod paths;
pub use paths::JkiPathExt;
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

use thiserror::Error;

#[derive(Error, Debug)]
pub enum JkiCoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Encryption error: {0}")]
    Encrypt(String),
    #[error("Decryption error: {0}")]
    Decrypt(String),
    #[error("OTP generation error: {0}")]
    Otp(String),
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Agent error: {0}")]
    Agent(String),
    #[error("Git error: {0}")]
    Git(String),
    #[error("Path error: {0}")]
    Path(String),
    #[error("Keyring error: {0}")]
    Keyring(String),
}

pub type Result<T> = std::result::Result<T, JkiCoreError>;

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

pub fn generate_otp(acc: &Account) -> Result<String> {
    let secret_str = acc.secret.trim().replace(" ", "").to_uppercase();
    let secret = Secret::Encoded(secret_str).to_bytes().map_err(|e| JkiCoreError::Otp(e.to_string()))?;
    
    let algo = match acc.algorithm.to_uppercase().as_str() {
        "SHA1" => Algorithm::SHA1,
        "SHA256" => Algorithm::SHA256,
        "SHA512" => Algorithm::SHA512,
        _ => Algorithm::SHA1, // Default fallback
    };

    // 使用 new_unchecked 繞過 RFC 對長度的強硬要求 (128 bits)
    let totp = TOTP::new_unchecked(
        algo, 
        acc.digits as usize, 
        1, 
        30, 
        secret
    );
    
    totp.generate_current().map_err(|e| JkiCoreError::Otp(e.to_string()))
}

// --- 加解密核心 ---

pub fn encrypt_with_master_key(data: &[u8], master_key: &SecretString) -> Result<Vec<u8>> {
    let encryptor = age::Encryptor::with_user_passphrase(master_key.clone());
    let mut encrypted = vec![];
    let mut writer = encryptor.wrap_output(&mut encrypted).map_err(|e| JkiCoreError::Encrypt(e.to_string()))?;
    writer.write_all(data)?;
    writer.finish().map_err(|e| JkiCoreError::Encrypt(e.to_string()))?;
    Ok(encrypted)
}

pub fn decrypt_with_master_key(encrypted_data: &[u8], master_key: &SecretString) -> Result<Vec<u8>> {
    let decryptor = match age::Decryptor::new(encrypted_data).map_err(|e| JkiCoreError::Decrypt(e.to_string()))? {
        age::Decryptor::Passphrase(d) => d,
        _ => return Err(JkiCoreError::Decrypt("Expected passphrase-encrypted data".to_string())),
    };
    let mut reader = decryptor.decrypt(master_key, None).map_err(|e| JkiCoreError::Decrypt(e.to_string()))?;
    let mut decrypted = vec![];
    reader.read_to_end(&mut decrypted)?;
    Ok(decrypted)
}

// --- 搜尋邏輯 ---

#[derive(Debug, Clone, PartialEq)]
pub struct MatchedAccount {
    pub account: Account,
    pub score: i64,
    pub issuer_indices: Vec<usize>,
    pub name_indices: Vec<usize>,
}

pub fn search_accounts(accounts: &[Account], patterns: &[String]) -> Vec<MatchedAccount> {
    let matcher = SkimMatcherV2::default();

    accounts.iter()
        .filter_map(|acc| {
            let issuer = acc.issuer.as_deref().unwrap_or_default();
            let name = &acc.name;

            let mut total_score = 0;
            let mut all_issuer_indices = Vec::new();
            let mut all_name_indices = Vec::new();

            for p in patterns {
                let issuer_res = matcher.fuzzy_indices(issuer, p);
                let name_res = matcher.fuzzy_indices(name, p);

                match (issuer_res, name_res) {
                    (Some((s1, mut i1)), Some((s2, mut i2))) => {
                        // 智慧加權：Issuer 命中權重更高，且優先考慮前綴匹配
                        let s1_weighted = adjust_score(s1, issuer, p, true);
                        let s2_weighted = adjust_score(s2, name, p, false);

                        total_score += s1_weighted.max(s2_weighted);
                        all_issuer_indices.append(&mut i1);
                        all_name_indices.append(&mut i2);
                    }
                    (Some((s, mut i)), None) => {
                        total_score += adjust_score(s, issuer, p, true);
                        all_issuer_indices.append(&mut i);
                    }
                    (None, Some((s, mut i))) => {
                        total_score += adjust_score(s, name, p, false);
                        all_name_indices.append(&mut i);
                    }
                    (None, None) => return None,
                }
            }

            all_issuer_indices.sort_unstable();
            all_issuer_indices.dedup();
            all_name_indices.sort_unstable();
            all_name_indices.dedup();

            Some(MatchedAccount {
                account: acc.clone(),
                score: total_score,
                issuer_indices: all_issuer_indices,
                name_indices: all_name_indices,
            })
        })
        .collect()
}

pub struct MatchedSubcommand {
    pub name: String,
    pub score: i64,
}

pub fn resolve_subcommand(input: &str, candidates: &[String]) -> Option<String> {
    let matcher = SkimMatcherV2::default();
    let mut matches: Vec<MatchedSubcommand> = candidates.iter()
        .filter_map(|name| {
            matcher.fuzzy_match(name, input).map(|score| {
                // 這裡複用之前的加權邏輯：前綴匹配加分
                let weighted_score = if name.to_lowercase().starts_with(&input.to_lowercase()) {
                    score + 100
                } else {
                    score
                };
                MatchedSubcommand { name: name.clone(), score: weighted_score }
            })
        })
        .collect();

    if matches.is_empty() { return None; }

    matches.sort_by(|a, b| b.score.cmp(&a.score));

    // 壓倒性優勢判定：第一名比第二名高 40 分，或只有一個結果且分數足夠高
    if matches.len() == 1 && matches[0].score > 30 {
        Some(matches[0].name.clone())
    } else if matches.len() > 1 && (matches[0].score - matches[1].score) >= 40 {
        Some(matches[0].name.clone())
    } else {
        // 分數太近，不自動判定，讓呼叫端處理 Did you mean?
        None
    }
}

pub fn get_subcommand_suggestions(input: &str, candidates: &[String]) -> Vec<String> {
    let matcher = SkimMatcherV2::default();
    let mut matches: Vec<(String, i64)> = candidates.iter()
        .filter_map(|name| {
            matcher.fuzzy_match(name, input).map(|score| (name.clone(), score))
        })
        .collect();
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.into_iter().take(3).map(|m| m.0).collect()
}

fn adjust_score(base_score: i64, target: &str, pattern: &str, is_issuer: bool) -> i64 {
    let mut score = base_score;

    // 1. 欄位權重：Issuer 比 Name 更具識別性
    if is_issuer {
        score += 20;
    }

    // 2. 前綴獎勵：開頭匹配是強烈意圖的表現
    let target_lc = target.to_lowercase();
    let pattern_lc = pattern.to_lowercase();
    if target_lc.starts_with(&pattern_lc) {
        score += 50;
        if is_issuer {
            score += 50; // Issuer 前綴最為關鍵
        }
    } else if target_lc.contains(&pattern_lc) {
        score += 20; // 包含但不一定是開頭
    }

    score
}

// --- 互動抽象 (用於 Mock 測試) ---

pub trait Interactor {
    fn prompt(&self, prompt: &str) -> Result<String>;
    fn prompt_password(&self, prompt: &str) -> Result<SecretString>;
    fn confirm(&self, prompt: &str, default: bool) -> bool;
}

pub struct TerminalInteractor;

impl Interactor for TerminalInteractor {
    fn prompt(&self, prompt: &str) -> Result<String> {
        use std::io::{self, Write};
        print!("{}: ", prompt);
        let _ = io::stdout().flush();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }

    fn prompt_password(&self, prompt: &str) -> Result<SecretString> {
        use crossterm::{
            event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
            terminal::{disable_raw_mode, enable_raw_mode},
            execute, cursor, style::Print,
        };
        use std::io::{self, Write};

        if !atty::is(atty::Stream::Stdin) {
            let mut line = String::new();
            io::stdin().read_line(&mut line)?;
            return Ok(SecretString::from(line.trim().to_string()));
        }

        enable_raw_mode()?;
        let mut stderr = io::stderr();
        execute!(stderr, Print(format!("{}: [ ", prompt)), cursor::SavePosition, Print("_ ]"), cursor::RestorePosition).ok();
        let _ = stderr.flush();

        let mut password = String::new();
        let mut toggle = false;

        let result = loop {
            match event::read() {
                Ok(Event::Key(KeyEvent { code, modifiers, .. })) => {
                    match code {
                        KeyCode::Enter => {
                            execute!(stderr, cursor::RestorePosition, cursor::MoveRight(2), Print("\r\n")).ok();
                            break Ok(SecretString::from(password));
                        }
                        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                            execute!(stderr, cursor::RestorePosition, cursor::MoveRight(2), Print("\r\nCancelled\r\n")).ok();
                            break Err(JkiCoreError::Auth("Interrupted".to_string()));
                        }
                        KeyCode::Char(c) => { password.push(c); toggle = !toggle; }
                        KeyCode::Backspace => { if !password.is_empty() { password.pop(); toggle = !toggle; } }
                        _ => continue,
                    }
                    let symbol = if password.is_empty() { "_" } else if toggle { "*" } else { "x" };
                    execute!(stderr, cursor::RestorePosition, Print(symbol), cursor::RestorePosition).ok();
                    let _ = stderr.flush();
                }
                Err(e) => break Err(JkiCoreError::Io(e)),
                _ => continue,
            }
        };
        let _ = disable_raw_mode();
        result
    }

    fn confirm(&self, prompt: &str, default: bool) -> bool {
        use std::io::{self, Write};
        let options = if default { "[Y/n]" } else { "[y/N]" };
        print!("{} {}: ", prompt, options);
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return default;
        }
        let input = input.trim().to_lowercase();
        if input.is_empty() {
            return default;
        }
        input == "y"
    }
}

pub struct MockInteractor {
    pub prompts: std::cell::RefCell<Vec<String>>,
    pub passwords: std::cell::RefCell<Vec<String>>,
    pub confirms: std::cell::RefCell<Vec<bool>>,
}

impl Interactor for MockInteractor {
    fn prompt(&self, _prompt: &str) -> Result<String> {
        if self.prompts.borrow().is_empty() {
            return Err(JkiCoreError::Auth("No mock prompt provided".to_string()));
        }
        Ok(self.prompts.borrow_mut().remove(0))
    }

    fn prompt_password(&self, _prompt: &str) -> Result<SecretString> {
        if self.passwords.borrow().is_empty() {
            return Err(JkiCoreError::Auth("No mock password provided".to_string()));
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
) -> Result<SecretString> {
    use crate::paths::JkiPath;

    match source {
        AuthSource::Auto => {
            // 1. Try Agent first (Session aware)
            if let Ok(key) = agent::AgentClient::get_master_key() {
                return Ok(key);
            }

            // 2. Try Secret Store (Keychain/Keyring)
            #[cfg(feature = "keychain")]
            if let Some(store) = secret_store {
                if let Ok(key) = store.get_secret("jki", "master_key") {
                    return Ok(key);
                }
            }

            // 3. Try master.key file
            let key_path = JkiPath::master_key_path();
            if key_path.exists() {
                if key_path.check_secure_permissions().is_ok() {
                    let content = std::fs::read_to_string(key_path)?;
                    return Ok(SecretString::from(content.trim().to_string()));
                }
            }

            // 4. Fallback to interactive prompt
            interactor.prompt_password("Enter Master Key")
        }
        AuthSource::Agent => {
            agent::AgentClient::get_master_key().map_err(|e| JkiCoreError::Auth(format!("Agent auth failed: {}", e)))
        }
        AuthSource::Interactive => {
            interactor.prompt_password("Enter Master Key")
        }
        AuthSource::Keyfile => {
            let key_path = JkiPath::master_key_path();
            if key_path.exists() {
                if let Err(e) = key_path.check_secure_permissions() {
                     return Err(JkiCoreError::Auth(format!("Keyfile auth failed: {}", e)));
                }
                let content = std::fs::read_to_string(key_path)?;
                return Ok(SecretString::from(content.trim().to_string()));
            }
            Err(JkiCoreError::Auth("Keyfile auth failed: File missing".to_string()))
        }
        AuthSource::Keychain => {
            #[cfg(feature = "keychain")]
            {
                if let Some(store) = secret_store {
                    store.get_secret("jki", "master_key").map_err(|e| JkiCoreError::Auth(format!("Keychain auth failed: {}", e)))
                } else {
                    Err(JkiCoreError::Auth("Keychain auth failed: Store not provided".to_string()))
                }
            }
            #[cfg(not(feature = "keychain"))]
            {
                Err(JkiCoreError::Auth("Keychain support not compiled in".to_string()))
            }
        }
        AuthSource::Biometric => {
            // For biometric, we always try agent first as it's the primary gateway.
            agent::AgentClient::get_master_key().map_err(|e| JkiCoreError::Auth(format!("Biometric (Agent) auth failed: {}", e)))
        }
        AuthSource::Plaintext => {
            Err(JkiCoreError::Auth("Plaintext auth source not applicable for master key acquisition".to_string()))
        }
    }
}

pub fn ensure_agent_running(quiet: bool) -> bool {
    use crate::paths::JkiPath;
    use interprocess::local_socket::LocalSocketStream;
    use std::process;

    let socket_path = JkiPath::agent_socket_path();
    if socket_path.exists() {
        if let Ok(_) = LocalSocketStream::connect(socket_path.to_str().unwrap_or_default()) {
            return true;
        }
        if !cfg!(windows) {
            let _ = std::fs::remove_file(&socket_path);
        }
    }

    if !quiet { eprintln!("Starting jki-agent..."); }
    
    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(_) => return false,
    };
    let mut agent_exe = match current_exe.parent() {
        Some(p) => p.join("jki-agent"),
        None => return false,
    };
    
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
    use crate::JkiCoreError;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Request {
        Ping,
        Unlock { master_key: String },
        UnlockBiometric,
        GetOTP { account_id: String },
        GetMasterKey,
        Reload,
        Shutdown,
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
        fn connect() -> Result<LocalSocketStream, JkiCoreError> {
            let socket_path = JkiPath::agent_socket_path();
            let name = socket_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid socket path".to_string()))?;
            LocalSocketStream::connect(name).map_err(|e| JkiCoreError::Agent(e.to_string()))
        }

        fn call(req: Request) -> Result<Response, JkiCoreError> {
            let mut stream = Self::connect()?;
            let req_json = serde_json::to_string(&req)?;
            stream.write_all(format!("{}\n", req_json).as_bytes())?;
            stream.flush()?;

            let mut line = String::new();
            let mut reader = BufReader::new(stream);
            reader.read_line(&mut line)?;
            serde_json::from_str(&line).map_err(|e| JkiCoreError::SerdeJson(e))
        }

        pub fn ping() -> bool {
            match Self::call(Request::Ping) {
                Ok(Response::Pong) => true,
                _ => false,
            }
        }

        pub fn shutdown() -> Result<(), JkiCoreError> {
            match Self::call(Request::Shutdown) {
                Ok(Response::Success) => Ok(()),
                Ok(Response::Error(e)) => Err(JkiCoreError::Agent(e)),
                Err(e) => Err(e),
                _ => Err(JkiCoreError::Agent("Unexpected response".to_string())),
            }
        }

        pub fn unlock(master_key: &SecretString) -> Result<String, JkiCoreError> {
            match Self::call(Request::Unlock { master_key: master_key.expose_secret().clone() }) {
                Ok(Response::Unlocked(source)) => Ok(source),
                Ok(Response::Error(e)) => Err(JkiCoreError::Agent(e)),
                _ => Err(JkiCoreError::Agent("Invalid agent response".to_string())),
            }
        }

        pub fn unlock_biometric() -> Result<String, JkiCoreError> {
            match Self::call(Request::UnlockBiometric) {
                Ok(Response::Unlocked(source)) => Ok(source),
                Ok(Response::Error(e)) => Err(JkiCoreError::Agent(e)),
                _ => Err(JkiCoreError::Agent("Invalid agent response".to_string())),
            }
        }

        pub fn get_otp(account_id: &str) -> Result<String, JkiCoreError> {
            match Self::call(Request::GetOTP { account_id: account_id.to_string() }) {
                Ok(Response::OTP(otp)) => Ok(otp),
                Ok(Response::Error(e)) => Err(JkiCoreError::Agent(e)),
                _ => Err(JkiCoreError::Agent("Invalid agent response".to_string())),
            }
        }

        pub fn get_master_key() -> Result<SecretString, JkiCoreError> {
            match Self::call(Request::GetMasterKey) {
                Ok(Response::MasterKey(key)) => Ok(SecretString::from(key)),
                Ok(Response::Error(e)) => Err(JkiCoreError::Agent(e)),
                _ => Err(JkiCoreError::Agent("Agent is locked".to_string())),
            }
        }

        pub fn reload() -> Result<(), JkiCoreError> {
            match Self::call(Request::Reload) {
                Ok(Response::Success) => Ok(()),
                Ok(Response::Error(e)) => Err(JkiCoreError::Agent(e)),
                _ => Err(JkiCoreError::Agent("Invalid agent response".to_string())),
            }
        }
    }
}

pub mod git {
    use std::process::Command;
    use std::path::Path;
    use crate::JkiCoreError;

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

    pub fn add_all(repo_path: &Path) -> Result<(), JkiCoreError> {
        let status = Command::new("git")
            .args(["-C", repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?, "add", "."])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git add failed".to_string()))
        }
    }

    pub fn add(repo_path: &Path, files: &[String]) -> Result<(), JkiCoreError> {
        if files.is_empty() { return Ok(()); }
        let mut args = vec!["-C", repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?, "add", "--"];
        for f in files {
            args.push(f);
        }
        let status = Command::new("git")
            .args(args)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git add failed".to_string()))
        }
    }

    pub fn commit(repo_path: &Path, message: &str) -> Result<bool, JkiCoreError> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?,
                "commit",
                "-m",
                message,
            ])
            .status()?;
        Ok(status.success())
    }

    pub fn pull_rebase(repo_path: &Path) -> Result<(), JkiCoreError> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?,
                "pull",
                "--rebase",
            ])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git pull --rebase failed".to_string()))
        }
    }

    pub fn get_conflicting_files(repo_path: &Path) -> Result<Vec<String>, JkiCoreError> {
        let output = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?,
                "diff",
                "--name-only",
                "--diff-filter=U",
            ])
            .output()?;
        let s = String::from_utf8_lossy(&output.stdout);
        Ok(s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
    }

    pub fn checkout_theirs(repo_path: &Path, files: &[String]) -> Result<(), JkiCoreError> {
        if files.is_empty() { return Ok(()); }
        let mut args = vec!["-C", repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?, "checkout", "--theirs", "--"];
        for f in files {
            args.push(f);
        }
        let status = Command::new("git")
            .args(args)
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git checkout --theirs failed".to_string()))
        }
    }

    pub fn rebase_continue(repo_path: &Path) -> Result<(), JkiCoreError> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?,
                "rebase",
                "--continue",
            ])
            .env("GIT_EDITOR", "true") // Skip editor for commit message
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git rebase --continue failed".to_string()))
        }
    }

    pub fn rebase_abort(repo_path: &Path) -> Result<(), JkiCoreError> {
        let status = Command::new("git")
            .args([
                "-C",
                repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?,
                "rebase",
                "--abort",
            ])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git rebase --abort failed".to_string()))
        }
    }

    pub fn push(repo_path: &Path) -> Result<(), JkiCoreError> {
        let status = Command::new("git")
            .args(["-C", repo_path.to_str().ok_or_else(|| JkiCoreError::Path("Invalid path".to_string()))?, "push"])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(JkiCoreError::Git("git push failed".to_string()))
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
        match decrypted.unwrap_err() {
            JkiCoreError::Decrypt(_) => {},
            e => panic!("Expected Decrypt error, got {:?}", e),
        }
    }

    #[test]
    #[serial]
    fn test_crypto_decrypt_invalid_format() {
        let key = SecretString::from("pass".to_string());
        let res = decrypt_with_master_key(b"not an age file", &key);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Decryption error"));
    }

    #[test]
    #[serial]
    fn test_generate_otp_invalid_secret() {
        let acc = Account {
            id: "1".into(),
            name: "test".into(),
            issuer: None,
            account_type: AccountType::Standard,
            secret: "!!! INVALID BASE32 !!!".into(),
            digits: 6,
            algorithm: "SHA1".into(),
        };
        let res = generate_otp(&acc);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("OTP generation error"));
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

        // Test add specific files
        std::fs::write(repo_path.join("file2"), "content").unwrap();
        git::add(repo_path, &vec!["file2".to_string()]).unwrap();
        let s = git::check_status(repo_path).unwrap();
        assert!(!s.is_clean);

        // Test conflict helpers (mocking conflict)
        std::fs::write(repo_path.join("file2"), "conflict").unwrap();
        // Just verify the porcelain output logic doesn't crash
        let _ = git::get_conflicting_files(repo_path);
    }

    #[test]
    #[serial]
    fn test_git_error_paths() {
        use tempfile::tempdir;
        use std::process::Command;
        let dir = tempdir().unwrap();
        let repo_path = dir.path();

        // 1. Git add on non-git dir
        let res = git::add_all(repo_path);
        assert!(res.is_err());

        // 2. Git push on non-git dir
        let res = git::push(repo_path);
        assert!(res.is_err());

        // 3. Pull rebase failure (no remote)
        Command::new("git").args(["init"]).current_dir(repo_path).output().unwrap();
        let res = git::pull_rebase(repo_path);
        assert!(res.is_err());
        match res.unwrap_err() {
            JkiCoreError::Git(_) => {},
            e => panic!("Expected Git error, got {:?}", e),
        }
    }

    #[test]
    #[serial]
    fn test_agent_client_error_handling() {
        // Since we can't easily mock the local socket server here without a lot of boilerplate,
        // we test the Response matching logic which is a large part of the coverage.
        
        let err_resp = agent::Response::Error("test error".to_string());
        let json = serde_json::to_string(&err_resp).unwrap();
        let decoded: agent::Response = serde_json::from_str(&json).unwrap();
        match decoded {
            agent::Response::Error(e) => assert_eq!(e, "test error"),
            _ => panic!("Decode failed"),
        }
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
        #[cfg(feature = "keychain")]
        {
            mock_store.set_secret("jki", "master_key", "keychain_pass").unwrap();
            std::fs::write(&key_path, "file_pass").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
            }

            let key = acquire_master_key(AuthSource::Auto, &interactor, Some(&mock_store)).unwrap();
            assert_eq!(key.expose_secret(), "keychain_pass");
        }

        // 3. File Priority (when keychain is missing or disabled)
        #[cfg(feature = "keychain")]
        mock_store.delete_secret("jki", "master_key").unwrap();
        
        std::fs::write(&key_path, "file_pass").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let key = acquire_master_key(AuthSource::Auto, &interactor, Some(&mock_store)).unwrap();
        assert_eq!(key.expose_secret(), "file_pass");

        // 4. Interactive Fallback (when both missing)
        std::fs::remove_file(&key_path).unwrap();
        interactor.passwords.borrow_mut().push("interactive_fallback".to_string());
        let key = acquire_master_key(AuthSource::Auto, &interactor, Some(&mock_store)).unwrap();
        assert_eq!(key.expose_secret(), "interactive_fallback");
    }

    #[test]
    #[serial]
    fn test_acquire_master_key_fail_fast() -> Result<()> {
        use tempfile::tempdir;
        use std::env;
        use crate::keychain::tests::MockSecretStore;

        let temp = tempdir().unwrap();
        let home = temp.path().join("jki_home_fail_fast");
        std::fs::create_dir_all(&home).unwrap();
        env::set_var("JKI_HOME", &home);

        let interactor = MockInteractor {
            passwords: std::cell::RefCell::new(vec![]),
            confirms: std::cell::RefCell::new(vec![]),
        };
        let mock_store = MockSecretStore::new();

        // Test explicit Keyfile source when file is missing
        let res = acquire_master_key(AuthSource::Keyfile, &interactor, Some(&mock_store));
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("File missing"));

        // Test explicit Keyfile source when permissions are insecure
        let key_path = crate::paths::JkiPath::master_key_path();
        std::fs::write(&key_path, "pass")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o644))?;
            let res = acquire_master_key(AuthSource::Keyfile, &interactor, Some(&mock_store));
            assert!(res.is_err());
            assert!(res.unwrap_err().to_string().contains("Insecure permissions"));
        }

        // Test explicit Keychain source
        let res = acquire_master_key(AuthSource::Keychain, &interactor, None);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        #[cfg(feature = "keychain")]
        assert!(err_msg.contains("Store not provided"));
        #[cfg(not(feature = "keychain"))]
        assert!(err_msg.contains("not compiled in"));
        
        Ok(())
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
