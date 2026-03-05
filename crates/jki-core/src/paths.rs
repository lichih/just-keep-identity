use std::path::{Path, PathBuf};
use std::env;

pub trait JkiPathExt {
    fn to_jki_metadata(&self) -> PathBuf;
    fn to_jki_secrets(&self) -> PathBuf;
    fn to_jki_decrypted_secrets(&self) -> PathBuf;
    fn to_jki_master_key(&self) -> PathBuf;
    fn to_jki_agent_socket(&self) -> PathBuf;
    fn check_secure_permissions(&self) -> crate::Result<()>;
}

impl JkiPathExt for Path {
    fn to_jki_metadata(&self) -> PathBuf { self.join("vault.metadata.json") }
    fn to_jki_secrets(&self) -> PathBuf { self.join("vault.secrets.bin.age") }
    fn to_jki_decrypted_secrets(&self) -> PathBuf { self.join("vault.secrets.json") }
    fn to_jki_master_key(&self) -> PathBuf { self.join("master.key") }
    fn to_jki_agent_socket(&self) -> PathBuf { self.join("jki.sock") }

    fn check_secure_permissions(&self) -> crate::Result<()> {
        if !self.exists() { return Err(crate::JkiCoreError::Path("File does not exist".to_string())); }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(self)?.permissions().mode() & 0o777;
            if mode != 0o600 {
                return Err(crate::JkiCoreError::Path(format!("Insecure permissions: {:o}. Expected 0600.", mode)));
            }
        }
        Ok(())
    }
}

pub struct JkiPath;

impl JkiPath {
    /// 獲取 JKI 根目錄 (JKI_HOME)
    pub fn home_dir() -> PathBuf {
        if let Ok(h) = env::var("JKI_HOME") {
            let p = PathBuf::from(&h);
            return p.canonicalize().unwrap_or_else(|_| p);
        }

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
            .unwrap_or_else(|_| Self::home_dir().to_jki_metadata())
    }

    pub fn secrets_path() -> PathBuf {
        env::var("JKI_SECRETS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().to_jki_secrets())
    }

    pub fn decrypted_secrets_path() -> PathBuf {
        env::var("JKI_DECRYPTED_SECRETS_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().to_jki_decrypted_secrets())
    }

    pub fn master_key_path() -> PathBuf {
        env::var("JKI_MASTER_KEY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().to_jki_master_key())
    }

    pub fn agent_socket_path() -> PathBuf {
        env::var("JKI_AGENT_SOCKET_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| Self::home_dir().to_jki_agent_socket())
    }

    /// Legacy support
    pub fn check_secure_permissions(path: &Path) -> crate::Result<()> {
        path.check_secure_permissions()
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
    fn test_extension_trait() {
        let p = PathBuf::from("/tmp/jki_test");
        assert_eq!(p.to_jki_metadata(), p.join("vault.metadata.json"));
        assert_eq!(p.to_jki_secrets(), p.join("vault.secrets.bin.age"));
        assert_eq!(p.to_jki_master_key(), p.join("master.key"));
    }

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
    fn test_check_secure_permissions_trait() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("test.key");
        
        fs::write(&file_path, "secret").unwrap();
        
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(file_path.check_secure_permissions().is_err());
        
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(file_path.check_secure_permissions().is_ok());
    }
}
