//! # polyoxide-core
//!
//! Core utilities and shared types for Polyoxide Polymarket API clients.
//!
//! This crate provides common functionality used across `polyoxide-clob`, `polyoxide-gamma`, and `polyoxide-data`:
//! - Shared error types and error handling
//! - HTTP client configuration
//! - Request builder utilities
//!
//! ## HTTP Client
//!
//! Use [`HttpClientBuilder`] to create configured HTTP clients:
//!
//! ```
//! use polyoxide_core::HttpClientBuilder;
//!
//! let client = HttpClientBuilder::new("https://api.example.com")
//!     .timeout_ms(60_000)
//!     .build()
//!     .unwrap();
//! ```
//!
//! ## Error Handling
//!
//! Use the [`impl_api_error_conversions`] macro to reduce boilerplate in error types.

// Compile the crate README's `rust` code fences as doctests so broken examples
// fail CI. `#[cfg(doctest)]` keeps this out of normal builds and `cargo doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[macro_use]
pub mod macros;

pub mod auth;
pub mod client;
pub mod error;
pub mod rate_limit;
pub mod request;

#[cfg(feature = "keychain")]
pub mod keychain;

/// Maximum number of characters to include in log messages containing response bodies.
const LOG_BODY_MAX_LEN: usize = 512;

/// Truncate a string for safe inclusion in log output.
///
/// Returns the original string if it fits within `LOG_BODY_MAX_LEN`,
/// otherwise truncates at a UTF-8 boundary and appends `... [truncated]`.
pub fn truncate_for_log(s: &str) -> std::borrow::Cow<'_, str> {
    if s.len() <= LOG_BODY_MAX_LEN {
        std::borrow::Cow::Borrowed(s)
    } else {
        let truncated = &s[..s.floor_char_boundary(LOG_BODY_MAX_LEN)];
        std::borrow::Cow::Owned(format!("{}... [truncated]", truncated))
    }
}

pub use auth::{current_timestamp, Base64Format, Signer};
pub use client::{
    retry_after_header, HttpClient, HttpClientBuilder, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};
pub use error::ApiError;
pub use rate_limit::{RateLimiter, RetryConfig};
pub use request::{QueryBuilder, Request, RequestError};

#[cfg(feature = "keychain")]
pub use keychain::KeychainError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_for_log_short_string_unchanged() {
        let short = "hello world";
        let result = truncate_for_log(short);
        assert_eq!(result.as_ref(), short);
    }

    #[test]
    fn test_truncate_for_log_exact_limit_unchanged() {
        let exact = "a".repeat(LOG_BODY_MAX_LEN);
        let result = truncate_for_log(&exact);
        assert_eq!(result.as_ref(), exact.as_str());
    }

    #[test]
    fn test_truncate_for_log_over_limit_truncated() {
        let long = "x".repeat(LOG_BODY_MAX_LEN + 100);
        let result = truncate_for_log(&long);
        assert!(result.ends_with("... [truncated]"));
        assert!(result.len() < long.len());
    }

    #[test]
    fn test_truncate_for_log_multibyte_char_boundary() {
        // Create a string where the 512th byte falls inside a multi-byte char
        let mut s = "a".repeat(LOG_BODY_MAX_LEN - 1);
        s.push('\u{1F600}'); // 4-byte emoji at position 511-514
        s.push_str("overflow");
        let result = truncate_for_log(&s);
        assert!(result.ends_with("... [truncated]"));
        // Should not panic or produce invalid UTF-8
        assert!(result.is_char_boundary(0));
    }
}
