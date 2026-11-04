//! Authentication utilities for Polymarket API clients
//!
//! This module provides shared authentication functionality used across CLOB and Relay clients.

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE},
    prelude::BASE64_URL_SAFE_NO_PAD,
    Engine,
};
use hmac::{Hmac, Mac};
use polyoxide_rt::{SystemTime, UNIX_EPOCH};
use sha2::Sha256;

/// Get current Unix timestamp in seconds.
///
/// # Panics
///
/// Panics if the system clock is set before the Unix epoch (1970-01-01).
pub fn current_timestamp() -> u64 {
    let now = SystemTime::now(); // runtime-gate: the wall clock is never injected
    now.duration_since(UNIX_EPOCH)
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
        self.sign_bytes(message.as_bytes(), format)
    }

    /// Sign a request: `timestamp + method + path + body`, the body read as the
    /// bytes it is sent as, so a body that is not UTF-8 signs too. For a text
    /// body this is `sign(&create_message(..))`.
    pub(crate) fn sign_request(
        &self,
        timestamp: u64,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        format: Base64Format,
    ) -> Result<String, String> {
        let mut message = format!("{timestamp}{method}{path}").into_bytes();
        message.extend_from_slice(body.unwrap_or_default());
        self.sign_bytes(&message, format)
    }

    /// HMAC-SHA256 over `message`, encoded in `format`.
    fn sign_bytes(&self, message: &[u8], format: Base64Format) -> Result<String, String> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret)
            .map_err(|e| format!("Failed to create HMAC: {}", e))?;

        mac.update(message);
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
    fn current_timestamp_sits_between_two_reads_of_the_runtime_crates_clock() {
        let read = || {
            polyoxide_rt::SystemTime::now()
                .duration_since(polyoxide_rt::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs())
        };
        let before = read();
        let ts = current_timestamp();
        let after = read();
        assert!(before <= ts && ts <= after, "{before} <= {ts} <= {after}");
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

    /// A gzip member's first bytes, which are not UTF-8 (issue #87).
    const BINARY_BODY: &[u8] = &[0x1f, 0x8b, 0x08, 0x00, 0xff, 0xfe, 0x00, 0x80];

    // The vectors are Python's, not this crate's:
    // `hmac.new(b"secret", b"1234567890POST/upload" + BINARY_BODY, hashlib.sha256).digest()`,
    // then `base64.urlsafe_b64encode` and `base64.b64encode`.
    #[test]
    fn sign_request_signs_a_body_that_is_not_utf8_over_its_bytes() {
        let signer = Signer::new("c2VjcmV0");
        let sign = |format| {
            signer
                .sign_request(1234567890, "POST", "/upload", Some(BINARY_BODY), format)
                .unwrap()
        };
        assert_eq!(
            sign(Base64Format::UrlSafe),
            "5sgudeEzgOyrOwl5xZavhZxSDKzDroXsjw_nZCbiQC4="
        );
        assert_eq!(
            sign(Base64Format::Standard),
            "5sgudeEzgOyrOwl5xZavhZxSDKzDroXsjw/nZCbiQC4="
        );
    }

    // `hmac.new(b"secret", b'1234567890POST/order{"size":1}', hashlib.sha256)`, url-safe.
    #[test]
    fn sign_request_over_a_text_body_is_the_text_signature() {
        let signer = Signer::new("c2VjcmV0");
        let body = r#"{"size":1}"#;
        for format in [Base64Format::UrlSafe, Base64Format::Standard] {
            for body in [None, Some(body)] {
                let message = Signer::create_message(1234567890, "POST", "/order", body);
                assert_eq!(
                    signer
                        .sign_request(
                            1234567890,
                            "POST",
                            "/order",
                            body.map(str::as_bytes),
                            format
                        )
                        .unwrap(),
                    signer.sign(&message, format).unwrap(),
                    "{format:?}, {body:?}"
                );
            }
        }
        assert_eq!(
            signer
                .sign_request(
                    1234567890,
                    "POST",
                    "/order",
                    Some(body.as_bytes()),
                    Base64Format::UrlSafe
                )
                .unwrap(),
            "K67J2JqKjD1RUDODfz42YEX-L6xDqgIdt6Ig2ScA13c="
        );
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
