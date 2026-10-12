//! Re-export of shared HMAC signer from polyoxide-core
//!
//! This module provides a thin wrapper around the unified Signer implementation
//! to maintain backward compatibility with the CLOB API client.

pub use polyoxide_polymarket::shared::{Base64Format, Signer as CoreSigner};

use crate::error::ClobError;

/// HMAC signer for API authentication
///
/// This is a thin wrapper around the shared `polyoxide_polymarket::shared::Signer` that provides
/// CLOB-specific error handling.
#[derive(Clone, Debug)]
pub struct Signer {
    inner: CoreSigner,
}

impl Signer {
    /// Create a new signer from base64-encoded secret (supports multiple formats)
    pub fn new(secret: &str) -> Self {
        Self {
            inner: CoreSigner::new(secret),
        }
    }

    /// Sign a message with HMAC-SHA256, using URL-safe base64 encoding
    pub fn sign(&self, message: &str) -> Result<String, ClobError> {
        self.inner
            .sign(message, Base64Format::UrlSafe)
            .map_err(ClobError::Crypto)
    }

    /// Create signature message for API request
    pub fn create_message(timestamp: u64, method: &str, path: &str, body: Option<&str>) -> String {
        CoreSigner::create_message(timestamp, method, path, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign() {
        // Test secret (base64)
        let secret = "c2VjcmV0"; // "secret" in base64
        let signer = Signer::new(secret);

        let message = Signer::create_message(1234567890, "GET", "/api/test", None);
        let signature = signer.sign(&message).unwrap();

        // Signature should be URL-safe base64
        assert!(!signature.contains('+'));
        assert!(!signature.contains('/'));
    }
}
