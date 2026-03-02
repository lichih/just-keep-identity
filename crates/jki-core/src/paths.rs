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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    #[serial]
    fn test_home_dir_default() {
        let original_jki_home = env::var("JKI_HOME");
        env::remove_var("JKI_HOME");

        let home = JkiPath::home_dir();
        assert!(home.ends_with("jki"));

        if let Ok(v) = original_jki_home {
            env::set_var("JKI_HOME", v);
        }
    }

    #[test]
    #[serial]
    fn test_home_dir_override() {
        let temp = tempdir().unwrap();
        let temp_path = temp.path().to_str().unwrap();
        let original_jki_home = env::var("JKI_HOME");
        
        env::set_var("JKI_HOME", temp_path);
        let home = JkiPath::home_dir();
        
        // canonicalize might change the path format on some OS, 
        // but it should refer to the same directory.
        assert_eq!(home.canonicalize().unwrap(), fs::canonicalize(temp_path).unwrap());

        if let Ok(v) = original_jki_home {
            env::set_var("JKI_HOME", v);
        } else {
            env::remove_var("JKI_HOME");
        }
    }

    #[test]
    #[serial]
    fn test_path_overrides() {
        env::set_var("JKI_METADATA_PATH", "/tmp/m.json");
        env::set_var("JKI_SECRETS_PATH", "/tmp/s.bin");
        env::set_var("JKI_MASTER_KEY_PATH", "/tmp/k.key");

        assert_eq!(JkiPath::metadata_path(), PathBuf::from("/tmp/m.json"));
        assert_eq!(JkiPath::secrets_path(), PathBuf::from("/tmp/s.bin"));
        assert_eq!(JkiPath::master_key_path(), PathBuf::from("/tmp/k.key"));

        env::remove_var("JKI_METADATA_PATH");
        env::remove_var("JKI_SECRETS_PATH");
        env::remove_var("JKI_MASTER_KEY_PATH");
    }

    #[test]
    #[cfg(unix)]
    fn test_check_secure_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.key");
        
        fs::write(&file_path, "secret").unwrap();
        
        // Test insecure
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(JkiPath::check_secure_permissions(&file_path).is_err());
        
        // Test secure
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(JkiPath::check_secure_permissions(&file_path).is_ok());
    }
}
