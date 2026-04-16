//! OS keychain credential storage.
//!
//! Provides `get` and `set` functions for storing and retrieving individual
//! credential strings from the platform keychain (macOS Keychain, Windows
//! Credential Manager, Linux Secret Service).
//!
//! Gated behind the `keychain` feature flag.

use keyring::Entry;

/// Error type for keychain operations.
#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    /// The requested keychain entry does not exist.
    #[error("Keychain entry not found: service={service}, key={key}")]
    NotFound {
        /// The service name that was queried.
        service: String,
        /// The key name that was queried.
        key: String,
    },

    /// An error from the underlying keychain backend.
    #[error("Keychain error: {0}")]
    Backend(#[from] keyring::Error),
}

/// Retrieve a credential from the OS keychain.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-clob"`)
/// * `key` - The key name (e.g., `"api_key"`)
pub fn get(service: &str, key: &str) -> Result<String, KeychainError> {
    let entry = Entry::new(service, key)?;
    entry.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => KeychainError::NotFound {
            service: service.to_string(),
            key: key.to_string(),
        },
        other => KeychainError::Backend(other),
    })
}

/// Store a credential in the OS keychain.
///
/// Creates or overwrites the entry for the given `(service, key)` pair.
///
/// # Arguments
///
/// * `service` - The service name (e.g., `"polyoxide-clob"`)
/// * `key` - The key name (e.g., `"api_key"`)
/// * `value` - The credential value to store
pub fn set(service: &str, key: &str, value: &str) -> Result<(), KeychainError> {
    let entry = Entry::new(service, key)?;
    entry.set_password(value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_error_includes_service_and_key() {
        let err = KeychainError::NotFound {
            service: "test-svc".to_string(),
            key: "test-key".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("test-svc"), "missing service: {msg}");
        assert!(msg.contains("test-key"), "missing key: {msg}");
    }

    #[test]
    fn backend_error_display() {
        let err = KeychainError::Backend(keyring::Error::NoEntry);
        let msg = err.to_string();
        assert!(msg.contains("Keychain error"), "unexpected: {msg}");
    }
}
