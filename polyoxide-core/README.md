# polyoxide-core

Core infrastructure crate for the [polyoxide](https://crates.io/crates/polyoxide) workspace. Provides the HTTP client and its one send loop, the hooks a venue gives that loop, error types, rate limiting, request building, and macros that every venue crate builds on. It names no venue: each venue's own rules, such as its retry policy, its rate limit tables and its signing, live in that venue's crate.

This is an **internal** crate. End users should depend on [`polyoxide`](https://crates.io/crates/polyoxide) instead.

## What's inside

| Module | Purpose |
|---|---|
| `client` | `HttpClient` / `HttpClientBuilder` -- configurable reqwest wrapper with base URL, connection pooling, concurrency limiting, and 429 retry logic |
| `error` | `ApiError` -- shared error enum (Response, Validation, Network, Serialization, Url, Refused, Sign), classed through `polyoxide_venue::Classify`; `ErrorResponse` -- an unsuccessful response read whole (status, headers, body, message, `Retry-After`), which `ApiError::Response` carries |
| `hooks` | `Throttle`, `RetryPolicy` and `Authenticator` -- the hooks a venue gives the send loop -- with the types they take (`RequestMeta`, `ResponseMeta`, `AttemptInfo`, `RequestParts`, `Decision`, `Cost`, `Charge`, `Refused`), `NoThrottle` and `DefaultRetryPolicy` |
| `send` | `HttpClient::send`, core's one send loop, and `decode_json`, which logs a decode failure once |
| `hold` | `Hold` -- the extend-only deadline every layer of one throttle waits out, with an optional ceiling |
| `capacity` | `CapacityBucket` -- a token bucket holding a published capacity, charging integer costs, refusing a cost it can never hold, resizable in place |
| `request` | `Request<T, E>`, `QueryBuilder` trait, `RequestError` marker trait (any `From<ApiError> + Debug` type, whose `From` is its decode) -- the one request builder: method, query, JSON body, authenticator and throttle costs, sent on the send loop with automatic deserialization |
| `rate_limit` | `WindowQuotaTable`, which builds a `RateLimiter` from a general bucket and per-endpoint rows (shared buckets, prefix or exact matching, scoped by method), with `effective_quota()` and `rows()` for inspection; `RetryConfig` for exponential backoff with jitter |
| `macros` | `client_config_setters!`, `namespaces!` and `query_setters!` -- a builder's transport setters, a client's namespace accessors, and a request builder's query setters |
| `keychain` | OS credential storage via `keyring` (feature-gated behind `keychain`) -- the `StoredCredential` trait a venue implements for each kind of credential it stores (its service, its `Field`s, which are secret, and its checks), with the generic `store`, `show` and `delete_all` over it, the per-key `get`, `set` and `delete`, and `KeychainError` |

## Error hierarchy

Each venue crate defines its own error enum, one per module, with an `Api(ApiError)` variant. An unsuccessful response reaches it as `ApiError::Response`, which carries the whole response (`ErrorResponse`: status, headers, body, message and `Retry-After`), and the crate's `From<ApiError>` is its one decode: a `?` on core's error anywhere reads the venue's own body shape there. Transport, URL and JSON failures enter the same way, as `ApiError::Network`, `Url` and `Serialization`:

```rust
use polyoxide_core::ApiError;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum MyCrateError {
    #[error(transparent)]
    Api(ApiError),
    #[error("venue refused: {0}")]
    Venue(String),
}

impl From<ApiError> for MyCrateError {
    fn from(err: ApiError) -> Self {
        match err {
            // The venue's own body shape becomes its own variant.
            ApiError::Response(response) if response.body.starts_with(r#"{"venue""#) => {
                Self::Venue(response.message)
            }
            other => Self::Api(other),
        }
    }
}
```

## Key exports

```rust
# #![allow(unused_imports)]
// HTTP client
use polyoxide_core::{HttpClient, HttpClientBuilder, DEFAULT_TIMEOUT_MS, DEFAULT_POOL_SIZE};

// Errors
use polyoxide_core::{ApiError, ErrorResponse};

// Request building
use polyoxide_core::{Request, QueryBuilder, RequestError};

// Rate limiting & retry
use polyoxide_core::{CapacityBucket, Hold, RateLimiter, RetryConfig, WindowQuotaTable};

// The send loop's hooks
use polyoxide_core::{Authenticator, DefaultRetryPolicy, RetryPolicy, Throttle};
```

## Installation

```toml
[dependencies]
polyoxide-core = "0.40"
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
