use keyring::{Entry, Error as KeyringError};
use secrecy::SecretString;

/// Trait representing a secure storage for secrets.
pub trait SecretStore {
    fn set_secret(&self, service: &str, user: &str, secret: &str) -> Result<(), String>;
    fn get_secret(&self, service: &str, user: &str) -> Result<SecretString, String>;
    fn delete_secret(&self, service: &str, user: &str) -> Result<(), String>;
}

/// Implementation of `SecretStore` using the system's native keychain via the `keyring` crate.
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn set_secret(&self, service: &str, user: &str, secret: &str) -> Result<(), String> {
        let entry = Entry::new(service, user).map_err(|e| e.to_string())?;
        entry.set_password(secret).map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
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
}
