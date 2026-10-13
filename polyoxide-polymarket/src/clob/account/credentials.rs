use std::fmt;

use serde::{Deserialize, Serialize};

/// API credentials for L2 authentication
#[derive(Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("key", &"<redacted>")
            .field("secret", &"<redacted>")
            .field("passphrase", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_all_fields() {
        let creds = Credentials {
            key: "my-api-key-12345".to_string(),
            secret: "my-super-secret".to_string(),
            passphrase: "my-passphrase-xyz".to_string(),
        };
        let debug = format!("{:?}", creds);

        assert!(
            !debug.contains("my-api-key-12345"),
            "Debug must not leak key: {}",
            debug
        );
        assert!(
            !debug.contains("my-super-secret"),
            "Debug must not leak secret: {}",
            debug
        );
        assert!(
            !debug.contains("my-passphrase-xyz"),
            "Debug must not leak passphrase: {}",
            debug
        );
        assert!(
            debug.contains("<redacted>"),
            "Debug should show <redacted>: {}",
            debug
        );
    }

    #[test]
    fn credentials_serde_roundtrip() {
        let original = Credentials {
            key: "k".to_string(),
            secret: "s".to_string(),
            passphrase: "p".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Credentials = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.key, "k");
        assert_eq!(deserialized.secret, "s");
        assert_eq!(deserialized.passphrase, "p");
    }

    #[test]
    fn credentials_clone() {
        let original = Credentials {
            key: "k".to_string(),
            secret: "s".to_string(),
            passphrase: "p".to_string(),
        };
        let cloned = original.clone();
        assert_eq!(cloned.key, original.key);
        assert_eq!(cloned.secret, original.secret);
        assert_eq!(cloned.passphrase, original.passphrase);
    }
}
