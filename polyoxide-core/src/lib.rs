//! # polyoxide-core
//!
//! Core utilities and shared types for the polyoxide venue clients.
//!
//! This crate provides the venue-neutral machinery every venue crate builds
//! its HTTP clients on, and names no venue:
//! - Shared error types and error handling
//! - HTTP client configuration
//! - The transport seam, [`HttpTransport`], with the [`Response`] it answers
//!   with and the [`TransportError`] it fails with, so the send loop names no
//!   HTTP library
//! - Request builder utilities
//! - One send loop, [`HttpClient::send`], with the hooks a venue supplies
//!   ([`Throttle`], [`RetryPolicy`], [`Authenticator`]), and one health
//!   ping on it, [`HttpClient::health`]
//! - Per-endpoint window quotas ([`WindowQuotaTable`], which builds a
//!   [`RateLimiter`]), token-cost buckets ([`CapacityBucket`]), the
//!   [`Hold`] their layers share, and the retry schedule ([`RetryConfig`])
//! - Optional OS keychain credential storage (behind the `keychain` feature)
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
//! An unsuccessful response is [`ApiError::Response`], carrying an
//! [`ErrorResponse`]: its status, headers, body, message and `Retry-After`.
//! A venue's error type wraps [`ApiError`], and its `From<ApiError>` is its
//! one decode, so a `?` on core's error anywhere reads the venue's body.

// Compile the crate README's `rust` code fences as doctests so broken examples
// fail CI. `#[cfg(doctest)]` keeps this out of normal builds and `cargo doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[macro_use]
pub mod macros;

pub mod capacity;
pub mod client;
pub mod config;
pub mod error;
mod fetch;
pub mod health;
pub mod hold;
pub mod hooks;
mod query;
pub mod rate_limit;
pub mod request;
pub mod response;
pub mod send;
pub mod transport;

#[cfg(feature = "keychain")]
pub mod keychain;

#[cfg(test)]
mod macro_tests;

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

/// The request vocabulary core's HTTP client speaks: `Method`, `StatusCode`,
/// `HeaderMap`, `HeaderValue` and `header::*`.
///
/// Venue code, tests and examples name these through this path, never through
/// reqwest. They are the very types reqwest re-exports, so moving a path here
/// changes no type.
pub use http;

/// The runtime seam, for the macros of this crate to name: a crate invoking
/// `client_config_setters!` needs no `polyoxide-rt` dependency of its own.
#[doc(hidden)]
pub use polyoxide_rt as __rt;

/// The HTTP client core sends with, re-exported whole.
///
/// Venue crates name its client types (`Client`, `Response`, `RequestBuilder`)
/// through this path and never depend on reqwest themselves, so core alone
/// decides its features and every client sends the same headers (AD-18). CI's
/// dependency fence fails when any other member declares it.
#[cfg(not(target_arch = "wasm32"))]
pub use reqwest;

pub use capacity::CapacityBucket;
pub use client::{
    retry_after_header, HttpClient, HttpClientBuilder, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};
pub use config::ClientConfig;
pub use error::{ApiError, ErrorResponse};
#[cfg(target_arch = "wasm32")]
pub use fetch::FetchTransport;
pub use health::Pong;
pub use hold::Hold;
pub use hooks::{
    AttemptInfo, Authenticator, Charge, Cost, Decision, DefaultRetryPolicy, DynAuthenticator,
    DynRetryPolicy, DynThrottle, LayerCharge, LayerId, NoThrottle, Outcome, Refused, RequestMeta,
    RequestParts, ResponseMeta, RetryPolicy, Throttle,
};
pub use query::csv;
pub use rate_limit::{
    BucketId, EffectiveQuota, Matching, QuotaRow, RateLimiter, RetryConfig, WindowQuotaTable,
};
pub use request::{QueryBuilder, Request, RequestError};
pub use response::Response;
pub use send::decode_json;
#[cfg(not(target_arch = "wasm32"))]
pub use transport::ReqwestTransport;
pub use transport::{HttpRequest, HttpTransport, TransportError, TransportKind};

#[cfg(feature = "keychain")]
pub use keychain::KeychainError;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<ApiError>();
    is::<Refused>();
    is::<TransportError>();
    #[cfg(feature = "keychain")]
    is::<KeychainError>();
};

#[cfg(test)]
mod tests {
    use super::*;

    /// `polyoxide_core::http` names the types reqwest sends with: each value
    /// below is assigned to the reqwest-typed binding with no conversion, so a
    /// re-export of any other version of the crate would not compile. Every
    /// path starts at the crate root, which the extern prelude does not serve,
    /// so deleting either `pub use` fails to compile here.
    #[test]
    fn the_http_vocabulary_is_the_type_reqwest_sends() {
        let method: crate::reqwest::Method = crate::http::Method::POST;
        let status: crate::reqwest::StatusCode = crate::http::StatusCode::TOO_MANY_REQUESTS;
        let mut headers: crate::reqwest::header::HeaderMap = crate::http::HeaderMap::new();
        headers.insert(
            crate::http::header::RETRY_AFTER,
            crate::http::HeaderValue::from_static("3"),
        );

        assert_eq!(method, crate::reqwest::Method::POST);
        assert_eq!(status.as_u16(), 429);
        assert_eq!(
            headers[crate::reqwest::header::RETRY_AFTER]
                .to_str()
                .unwrap(),
            "3"
        );
    }

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
