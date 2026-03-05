#[cfg(feature = "keychain")]
use keyring::{Entry, Error as KeyringError};
use secrecy::SecretString;

/// Trait representing a secure storage for secrets.
pub trait SecretStore {
    fn set_secret(&self, service: &str, user: &str, secret: &str) -> Result<(), String>;
    fn get_secret(&self, service: &str, user: &str) -> Result<SecretString, String>;
    fn delete_secret(&self, service: &str, user: &str) -> Result<(), String>;
}

/// Implementation of `SecretStore` using the system's native keychain via the `keyring` crate.
#[cfg(feature = "keychain")]
pub struct KeyringStore;

#[cfg(feature = "keychain")]
impl SecretStore for KeyringStore {
    fn set_secret(&self, service: &str, user: &str, secret: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            // On macOS, we use the 'security' command directly to handle ACL (Access Control List).
            // This allows us to authorize both jkim and jki-agent simultaneously at creation time,
            // preventing the annoying "App B wants to access App A's item" prompt.
            use std::process::Command;

            let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let mut agent_exe = current_exe.clone();
            agent_exe.pop();
            agent_exe.push("jki-agent");

            // 1. Delete existing item to ensure we start with a clean ACL
            let _ = Command::new("security")
                .arg("delete-generic-password")
                .arg("-a").arg(user)
                .arg("-s").arg(service)
                .output();

            // 2. Add new item with explicit trusted applications (-T)
            // -T allows the specified applications to access the item without prompt.
            let output = Command::new("security")
                .arg("add-generic-password")
                .arg("-a").arg(user)
                .arg("-s").arg(service)
                .arg("-w").arg(secret)
                .arg("-T").arg(current_exe.to_string_lossy().to_string())
                .arg("-T").arg(agent_exe.to_string_lossy().to_string())
                .output()
                .map_err(|e| e.to_string())?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Security command failed: {}", err));
            }
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let entry = Entry::new(service, user).map_err(|e| e.to_string())?;
            entry.set_password(secret).map_err(|e| e.to_string())
        }
    }

    fn get_secret(&self, service: &str, user: &str) -> Result<SecretString, String> {
        let entry = Entry::new(service, user).map_err(|e| e.to_string())?;
        let password = entry.get_password().map_err(|e| {
            match e {
                KeyringError::NoEntry => "Secret not found".to_string(),
                _ => e.to_string(),
            }
        })?;
        Ok(SecretString::from(password))
    }

    fn delete_secret(&self, service: &str, user: &str) -> Result<(), String> {
        let entry = Entry::new(service, user).map_err(|e| e.to_string())?;
        entry.delete_credential().map_err(|e| e.to_string())
    }
}

/// Fallback implementation of `KeyringStore` when the `keychain` feature is disabled.
/// This prevents build errors in lightweight binaries like `jki`.
#[cfg(not(feature = "keychain"))]
pub struct KeyringStore;

#[cfg(not(feature = "keychain"))]
impl SecretStore for KeyringStore {
    fn set_secret(&self, _service: &str, _user: &str, _secret: &str) -> Result<(), String> {
        Err("Keychain support not compiled in".to_string())
    }

    fn get_secret(&self, _service: &str, _user: &str) -> Result<SecretString, String> {
        Err("Keychain support not compiled in".to_string())
    }

    fn delete_secret(&self, _service: &str, _user: &str) -> Result<(), String> {
        Err("Keychain support not compiled in".to_string())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use secrecy::ExposeSecret;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// A simple mock store for unit testing components that depend on `SecretStore`.
    pub struct MockSecretStore {
        storage: Mutex<HashMap<String, String>>,
    }

    impl MockSecretStore {
        pub fn new() -> Self {
            Self {
                storage: Mutex::new(HashMap::new()),
            }
        }

        fn key(service: &str, user: &str) -> String {
            format!("{}:{}", service, user)
        }
    }

    impl SecretStore for MockSecretStore {
        fn set_secret(&self, service: &str, user: &str, secret: &str) -> Result<(), String> {
            self.storage.lock().unwrap().insert(Self::key(service, user), secret.to_string());
            Ok(())
        }

        fn get_secret(&self, service: &str, user: &str) -> Result<SecretString, String> {
            self.storage.lock().unwrap()
                .get(&Self::key(service, user))
                .cloned()
                .map(SecretString::from)
                .ok_or_else(|| "Secret not found".to_string())
        }

        fn delete_secret(&self, service: &str, user: &str) -> Result<(), String> {
            self.storage.lock().unwrap()
                .remove(&Self::key(service, user))
                .map(|_| ())
                .ok_or_else(|| "Secret not found".to_string())
        }
    }

    #[test]
    fn test_mock_secret_store() {
        let store = MockSecretStore::new();
        let service = "test-service";
        let user = "test-user";
        let secret = "test-secret";

        // 1. Set
        store.set_secret(service, user, secret).unwrap();

        // 2. Get
        let retrieved = store.get_secret(service, user).unwrap();
        assert_eq!(retrieved.expose_secret(), secret);

        // 3. Delete
        store.delete_secret(service, user).unwrap();

        // 4. Verify deletion
        let result = store.get_secret(service, user);
        assert!(result.is_err());
        }

        #[test]
        #[cfg(not(feature = "keychain"))]
        fn test_keyring_store_fallback() {
        let store = KeyringStore;
        assert!(store.set_secret("s", "u", "p").is_err());
        assert!(store.get_secret("s", "u").is_err());
        assert!(store.delete_secret("s", "u").is_err());
        }
        }

