//! Authentication utilities for Polymarket API clients
//!
//! This module provides shared authentication functionality used across CLOB and Relay clients.

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE},
    prelude::BASE64_URL_SAFE_NO_PAD,
    Engine,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp in seconds.
///
/// # Panics
///
/// Panics if the system clock is set before the Unix epoch (1970-01-01).
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must not be before Unix epoch")
        .as_secs()
}

/// Base64 encoding format for HMAC signatures
#[derive(Clone, Copy, Debug)]
pub enum Base64Format {
    /// URL-safe base64 (replaces + with - and / with _)
    UrlSafe,
    /// Standard base64
    Standard,
}

/// HMAC signer for API authentication
///
/// Supports both base64-encoded and raw string secrets, with configurable output format.
#[derive(Clone)]
pub struct Signer {
    secret: Vec<u8>,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

impl Signer {
    /// Create a new signer from base64-encoded secret
    ///
    /// Attempts to decode the secret using multiple base64 formats in order:
    /// 1. URL-safe without padding (most common for API keys)
    /// 2. URL-safe with padding
    /// 3. Standard base64
    /// 4. Falls back to raw bytes if all decoding attempts fail
    pub fn new(secret: &str) -> Self {
        let decoded = BASE64_URL_SAFE_NO_PAD
            .decode(secret)
            .or_else(|_| URL_SAFE.decode(secret))
            .or_else(|_| STANDARD.decode(secret))
            .unwrap_or_else(|_| secret.as_bytes().to_vec());

        Self { secret: decoded }
    }

    /// Create a new signer from raw string secret (no base64 decoding)
    pub fn from_raw(secret: &str) -> Self {
        Self {
            secret: secret.as_bytes().to_vec(),
        }
    }

    /// Sign a message with HMAC-SHA256
    ///
    /// # Arguments
    /// * `message` - The message to sign
    /// * `format` - The base64 encoding format for the output signature
    pub fn sign(&self, message: &str, format: Base64Format) -> Result<String, String> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret)
            .map_err(|e| format!("Failed to create HMAC: {}", e))?;

        mac.update(message.as_bytes());
        let result = mac.finalize();

        let signature = match format {
            Base64Format::UrlSafe => {
                // Encode with standard base64 then convert to URL-safe
                let sig = STANDARD.encode(result.into_bytes());
                sig.replace('+', "-").replace('/', "_")
            }
            Base64Format::Standard => STANDARD.encode(result.into_bytes()),
        };

        Ok(signature)
    }

    /// Create signature message for API request
    ///
    /// Format: timestamp + method + path + body
    pub fn create_message(timestamp: u64, method: &str, path: &str, body: Option<&str>) -> String {
        format!("{}{}{}{}", timestamp, method, path, body.unwrap_or(""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        // Should be a reasonable Unix timestamp (after 2020)
        assert!(ts > 1_600_000_000);
    }

    #[test]
    fn test_signer_new() {
        // Test with base64-encoded secret
        let secret = "c2VjcmV0"; // "secret" in base64
        let signer = Signer::new(secret);
        assert_eq!(signer.secret, b"secret");
    }

    #[test]
    fn test_signer_from_raw() {
        let signer = Signer::from_raw("secret");
        assert_eq!(signer.secret, b"secret");
    }

    #[test]
    fn test_sign_url_safe() {
        let secret = "c2VjcmV0"; // "secret" in base64
        let signer = Signer::new(secret);

        let message = Signer::create_message(1234567890, "GET", "/api/test", None);
        let signature = signer.sign(&message, Base64Format::UrlSafe).unwrap();

        // Signature should be URL-safe base64 (no + or /)
        assert!(!signature.contains('+'));
        assert!(!signature.contains('/'));
    }

    #[test]
    fn test_sign_standard() {
        let secret = "c2VjcmV0"; // "secret" in base64
        let signer = Signer::new(secret);

        let message = Signer::create_message(1234567890, "GET", "/api/test", None);
        let signature = signer.sign(&message, Base64Format::Standard).unwrap();

        // Standard base64 may contain + or /
        assert!(!signature.is_empty());
    }

    #[test]
    fn test_create_message() {
        let msg = Signer::create_message(1234567890, "GET", "/api/test", None);
        assert_eq!(msg, "1234567890GET/api/test");

        let msg_with_body =
            Signer::create_message(1234567890, "POST", "/api/test", Some(r#"{"key":"value"}"#));
        assert_eq!(msg_with_body, r#"1234567890POST/api/test{"key":"value"}"#);
    }

    #[test]
    fn test_signer_debug_redacts_secret() {
        // "c2VjcmV0" is "secret" in base64, decoded bytes are [115, 101, 99, 114, 101, 116]
        let signer = Signer::new("c2VjcmV0");
        let debug_output = format!("{:?}", signer);
        assert!(
            debug_output.contains("[REDACTED]"),
            "Debug output should contain [REDACTED]: {}",
            debug_output
        );
        // Should not contain the base64 secret or the decoded bytes
        assert!(
            !debug_output.contains("c2VjcmV0"),
            "Debug output should not contain the base64 secret: {}",
            debug_output
        );
        assert!(
            !debug_output.contains("115, 101"),
            "Debug output should not contain decoded secret bytes: {}",
            debug_output
        );
    }
}
