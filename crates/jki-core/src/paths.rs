use std::path::PathBuf;
use std::env;

pub struct JkiPath;

impl JkiPath {
    /// 獲取 JKI 根目錄 (JKI_HOME)
    pub fn home_dir() -> PathBuf {
        if let Ok(h) = env::var("JKI_HOME") {
            let p = PathBuf::from(&h);
            // 嘗試規範化為絕對路徑，若失敗則保留原始值
            return p.canonicalize().unwrap_or_else(|_| p);
        }

        // 預設路徑處理
        let mut path = if cfg!(windows) {
            dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
        } else {
            dirs::home_dir().map(|h| h.join(".config")).unwrap_or_else(|| PathBuf::from("."))
        };

        path.push("jki");
        path
    }

    pub fn metadata_path() -> PathBuf {
        env::var("JKI_METADATA_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().join("vault.metadata.json"))
    }

    pub fn secrets_path() -> PathBuf {
        env::var("JKI_SECRETS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().join("vault.secrets.bin.age"))
    }

    pub fn master_key_path() -> PathBuf {
        env::var("JKI_MASTER_KEY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().join("master.key"))
    }

    pub fn check_secure_permissions(path: &PathBuf) -> Result<(), String> {
        if !path.exists() { return Err("File does not exist".to_string()); }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path).map_err(|e| e.to_string())?.permissions().mode() & 0o777;
            if mode != 0o600 {
                return Err(format!("Insecure permissions: {:o}. Expected 0600.", mode));
            }
        }
        Ok(())
    }
}
