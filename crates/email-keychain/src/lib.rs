use email_core::error::{EmailError, Result};
use keyring::Entry;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::RwLock;

pub const SERVICE_NAME: &str = "com.rustmail.emailapp";

pub trait CredentialStore: Send + Sync {
    fn set_credential(&self, key: &str, secret: &str) -> Result<()>;
    fn get_credential(&self, key: &str) -> Result<String>;
    fn delete_credential(&self, key: &str) -> Result<()>;
    fn is_available(&self) -> bool;
}

/// OS Native Keyring (Keychain / Secret Service / Credential Manager)
pub struct NativeKeyringStore {
    service_name: String,
    fallback_cache: RwLock<HashMap<String, String>>,
}

impl NativeKeyringStore {
    pub fn new() -> Self {
        Self {
            service_name: SERVICE_NAME.to_string(),
            fallback_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_service(service: &str) -> Self {
        Self {
            service_name: service.to_string(),
            fallback_cache: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for NativeKeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for NativeKeyringStore {
    fn set_credential(&self, key: &str, secret: &str) -> Result<()> {
        match Entry::new(&self.service_name, key) {
            Ok(entry) => match entry.set_password(secret) {
                Ok(_) => {
                    info!("Successfully stored credential for key: {}", key);
                    // Also update cache as fallback
                    if let Ok(mut cache) = self.fallback_cache.write() {
                        cache.insert(key.to_string(), secret.to_string());
                    }
                    Ok(())
                }
                Err(e) => {
                    warn!(
                        "OS keyring set_password failed ({}), using volatile memory cache fallback",
                        e
                    );
                    if let Ok(mut cache) = self.fallback_cache.write() {
                        cache.insert(key.to_string(), secret.to_string());
                        Ok(())
                    } else {
                        Err(EmailError::Keyring(format!("Failed to store secret: {}", e)))
                    }
                }
            },
            Err(e) => {
                warn!("Failed to create keyring entry ({}). Using fallback memory cache", e);
                if let Ok(mut cache) = self.fallback_cache.write() {
                    cache.insert(key.to_string(), secret.to_string());
                    Ok(())
                } else {
                    Err(EmailError::Keyring(format!("Keyring init failed: {}", e)))
                }
            }
        }
    }

    fn get_credential(&self, key: &str) -> Result<String> {
        // 1. Check in-memory cache first to avoid blocking D-Bus RPCs to OS keyring daemon
        if let Ok(cache) = self.fallback_cache.read() {
            if let Some(secret) = cache.get(key) {
                return Ok(secret.clone());
            }
        }

        // 2. Query OS Native Keyring
        match Entry::new(&self.service_name, key) {
            Ok(entry) => match entry.get_password() {
                Ok(secret) => {
                    // Populate memory cache for future instant lookups
                    if let Ok(mut cache) = self.fallback_cache.write() {
                        cache.insert(key.to_string(), secret.clone());
                    }
                    Ok(secret)
                }
                Err(keyring::Error::NoEntry) => {
                    // Try fallback legacy service names
                    let fallback_services = ["com.rustmail.emailapp", "AT-mail-rs-credential-store"];
                    for svc in fallback_services {
                        if svc != self.service_name {
                            if let Ok(legacy_entry) = Entry::new(svc, key) {
                                if let Ok(secret) = legacy_entry.get_password() {
                                    if let Ok(mut cache) = self.fallback_cache.write() {
                                        cache.insert(key.to_string(), secret.clone());
                                    }
                                    return Ok(secret);
                                }
                            }
                        }
                    }
                    Err(EmailError::Keyring(format!(
                        "No credential found for key: {}",
                        key
                    )))
                }
                Err(e) => {
                    Err(EmailError::Keyring(format!(
                        "Keyring lookup error for {}: {}",
                        key, e
                    )))
                }
            },
            Err(e) => {
                Err(EmailError::Keyring(format!("Keyring init error: {}", e)))
            }
        }
    }

    fn delete_credential(&self, key: &str) -> Result<()> {
        if let Ok(mut cache) = self.fallback_cache.write() {
            cache.remove(key);
        }
        if let Ok(entry) = Entry::new(&self.service_name, key) {
            let _ = entry.delete_credential();
        }
        Ok(())
    }

    fn is_available(&self) -> bool {
        // Quick probe test
        if let Ok(entry) = Entry::new(&self.service_name, "__probe_test__") {
            let _ = entry.get_password();
            true
        } else {
            false
        }
    }
}

/// In-memory mock store for testing
pub struct MockKeyringStore {
    store: RwLock<HashMap<String, String>>,
}

impl MockKeyringStore {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MockKeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for MockKeyringStore {
    fn set_credential(&self, key: &str, secret: &str) -> Result<()> {
        let mut store = self.store.write().map_err(|e| EmailError::Keyring(e.to_string()))?;
        store.insert(key.to_string(), secret.to_string());
        Ok(())
    }

    fn get_credential(&self, key: &str) -> Result<String> {
        let store = self.store.read().map_err(|e| EmailError::Keyring(e.to_string()))?;
        store.get(key).cloned().ok_or_else(|| {
            EmailError::Keyring(format!("Credential not found for key: {}", key))
        })
    }

    fn delete_credential(&self, key: &str) -> Result<()> {
        let mut store = self.store.write().map_err(|e| EmailError::Keyring(e.to_string()))?;
        store.remove(key);
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_keyring_store_crud() {
        let store = MockKeyringStore::new();
        assert!(store.set_credential("user1", "pass123").is_ok());
        assert_eq!(store.get_credential("user1").unwrap(), "pass123");
        assert!(store.delete_credential("user1").is_ok());
        assert!(store.get_credential("user1").is_err());
    }

    #[test]
    fn test_native_keyring_store_cache_fallback() {
        let store = NativeKeyringStore::with_service("com.test.testservice");
        // Pre-populate cache directly
        if let Ok(mut cache) = store.fallback_cache.write() {
            cache.insert("cached_acc".to_string(), "cached_pass".to_string());
        }
        // get_credential should return cached password immediately
        assert_eq!(store.get_credential("cached_acc").unwrap(), "cached_pass");
    }
}
