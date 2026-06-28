# polyoxide-core

Core infrastructure crate for the [polyoxide](https://crates.io/crates/polyoxide) workspace. Provides the shared HTTP client, error types, authentication primitives, rate limiting, request building, and macros used by `polyoxide-clob`, `polyoxide-gamma`, `polyoxide-data`, and `polyoxide-relay`.

This is an **internal** crate. End users should depend on [`polyoxide`](https://crates.io/crates/polyoxide) instead.

## What's inside

| Module | Purpose |
|---|---|
| `client` | `HttpClient` / `HttpClientBuilder` -- configurable reqwest wrapper with base URL, connection pooling, concurrency limiting, and 429 retry logic |
| `error` | `ApiError` -- shared error enum (Api, Authentication, Validation, RateLimit, Timeout, Network, Serialization, Url) with `from_response()` for automatic status-code mapping |
| `auth` | `Signer` (HMAC-SHA256), `Base64Format`, `current_timestamp()` -- authentication building blocks for L2 API credentials |
| `request` | `Request<T, E>`, `QueryBuilder` trait, `RequestError` trait -- generic GET request builder with automatic deserialization, retry, and rate-limit integration |
| `rate_limit` | `RateLimiter` with per-endpoint burst/sustained windows and `RetryConfig` for exponential backoff with jitter. Factory methods: `clob_default()`, `gamma_default()`, `data_default()`, `relay_default()` |
| `macros` | `impl_api_error_conversions!` -- generates `From<reqwest::Error>` and `From<url::ParseError>` for crate-specific error wrappers |
| `keychain` | OS credential storage via `keyring` -- `get`, `set`, `delete` helpers and `KeychainError` (feature-gated behind `keychain`) |

## Error hierarchy

Each downstream crate defines its own error enum (e.g. `ClobError`, `GammaError`) with an `Api(ApiError)` variant. The `impl_api_error_conversions!` macro wires up the `From` conversions so `reqwest::Error` and `url::ParseError` flow through `ApiError` automatically:

```rust
use polyoxide_core::{ApiError, impl_api_error_conversions};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyCrateError {
    #[error(transparent)]
    Api(#[from] ApiError),
}

impl_api_error_conversions!(MyCrateError);
```

## Key exports

```rust
# #![allow(unused_imports)]
// HTTP client
use polyoxide_core::{HttpClient, HttpClientBuilder, DEFAULT_TIMEOUT_MS, DEFAULT_POOL_SIZE};

// Errors
use polyoxide_core::ApiError;

// Auth
use polyoxide_core::{Signer, Base64Format, current_timestamp};

// Request building
use polyoxide_core::{Request, QueryBuilder, RequestError};

// Rate limiting & retry
use polyoxide_core::{RateLimiter, RetryConfig};
```

## Installation

```toml
[dependencies]
polyoxide-core = "0.17"
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
