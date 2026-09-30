# Perps HTTP (public info routes) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `polyoxide-perps` crate that reads all 21 public `GET /v1/info/*` routes on `api.perpetuals.polymarket.com`, with spec, wire and live tests, a measured rate-limit table, and the crate wired into the workspace.

**Architecture:** One new crate on `polyoxide-core`'s `HttpClient`/`Request`, shaped like `polyoxide-data`: a `Perps` client with four namespaces (`health`, `exchange`, `market`, `public`), request builders ending in `.send().await?`, and a `PerpsError` that recognises the venue's `{status:"err", error}` body by shape. Rate rows are measured by a soak example before they are written into `RateLimiter::perps_default()`. Plan 2 (WebSocket) builds on this crate behind a `ws` feature.

**Tech Stack:** Rust 1.91, `polyoxide-core`, `serde`/`serde_json`, `rust_decimal` (string serde), `thiserror`, `mockito` for mocks, `tracing-subscriber` in the soak, Python 3 (`urllib`) for fixture capture.

**Spec:** `docs/superpowers/specs/2026-09-30-perps-public-design.md`. Two corrections landed with this plan: REST `book` depth is 10/100/500/1000 (the socket's 20/50 is a separate enum in plan 2), and the soak routes are `klines`, `trades`, `portfolio`, `bbo` (see Task 11 for why `instruments` cannot be soaked).

**Conventions that apply to every task:**
- Commit messages are conventional commits scoped `perps` (`feat(perps): …`, `test(perps): …`, `docs(perps): …`), ending with the attribution lines from the session's system reminder.
- Run `cargo fmt --all` before every commit. Clippy runs with `-D warnings`. `cargo doc` runs with `RUSTDOCFLAGS="-D warnings"`: a doc comment on a `pub` item may not `[`link`]` a `pub(crate)` item.
- Wire field names are snake_case exactly as the spec spells them. Prices and quantities are `Decimal` with `#[serde(with = "rust_decimal::serde::str")]`. Timestamps are `u64` milliseconds.
- Response structs are `#[non_exhaustive]` and derive `Debug, Clone, PartialEq, Serialize, Deserialize`. Serialize is needed by the agreement tests.

---

## File structure

```
polyoxide-perps/
  Cargo.toml
  README.md                     doctest (no_run examples)
  src/lib.rs                    re-exports + README doctest hook
  src/client.rs                 Perps, PerpsBuilder, DEFAULT_BASE_URL
  src/error.rs                  PerpsError, VenueError
  src/types.rs                  InstrumentId, enums, Kline, MarkPoint, Level
  src/api/mod.rs                Fetch<T>, setter! macro
  src/api/health.rs             ping, time
  src/api/exchange.rs           exchange, assets, instruments, fees, limit_tiers
  src/api/market.rs             tickers, statistics, exchange_stats, klines, mark_history, bbo, book, index, trades, funding
  src/api/public.rs             portfolio, position_fills, leaderboard, invite
  examples/info_soak.rs         rate-limit ramp + validation harness
  tests/mock_api.rs             mockito tests per builder
  tests/spec_agreement.rs       types/builders vs openapi.json
  tests/wire_agreement.rs       types vs captured fixtures
  tests/live_api.rs             #[ignore] live tests
  tests/fixtures/*.json         captured bodies + PROVENANCE.md
scripts/capture_perps_fixtures.py
docs/specs/perps/OBSERVED.md
polyoxide-core/src/rate_limit.rs   perps_default() + documented_perps_limits tests
Cargo.toml, polyoxide/Cargo.toml, polyoxide/src/lib.rs
.github/workflows/{release,nightly-behavioral}.yml
docs/specs/INDEX.md, docs/specs/perps/INDEX.md, CLAUDE.md
```

---

### Task 1: Crate skeleton, error type, client builder

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Create: `polyoxide-perps/Cargo.toml`, `polyoxide-perps/README.md`, `polyoxide-perps/src/lib.rs`, `polyoxide-perps/src/error.rs`, `polyoxide-perps/src/client.rs`, `polyoxide-perps/src/api/mod.rs`, `polyoxide-perps/src/types.rs` (empty module for now)

- [ ] **Step 1: Register the crate in the workspace**

In the root `Cargo.toml`, add `"polyoxide-perps",` to `members` after `"polyoxide-gamma",`, and under `[workspace.dependencies]` add after the `polyoxide-gamma` line:

```toml
polyoxide-perps = { path = "polyoxide-perps", version = "0.33.1" }
```

- [ ] **Step 2: Write `polyoxide-perps/Cargo.toml`**

```toml
[package]
name = "polyoxide-perps"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Rust client library for the Polymarket Perps API"
keywords = ["polymarket", "perpetuals", "trading"]
categories = ["api-bindings", "web-programming::http-client"]

[dependencies]
polyoxide-core = { workspace = true }
reqwest = { workspace = true }
rust_decimal = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
url = { workspace = true }

[dev-dependencies]
mockito = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time"] }
tracing-subscriber = { workspace = true }
```

- [ ] **Step 3: Write `polyoxide-perps/src/error.rs` with its tests**

```rust
//! Error types for the Perps API.

use std::time::Duration;

use polyoxide_core::{retry_after_header, ApiError, RequestError};
use serde::Deserialize;
use thiserror::Error;

/// Error type for Perps API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PerpsError {
    /// Transport, decoding, or an error body in some other shape.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// The venue answered with its `{status: "err", error}` body.
    #[error(transparent)]
    Venue(#[from] VenueError),
}

/// An error the Perps host produced, carrying its stable identifier.
///
/// Upstream calls `error` a machine-readable snake_case identifier that is
/// part of the API contract for domain and transport rejections
/// (`ip_rate_limited`, `not_found`). On a 400 it is a human-readable
/// validation message instead. It stays a `String` because the catalogue is
/// long and grows.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[error("perps API {status}: {code}")]
pub struct VenueError {
    /// HTTP status.
    pub status: u16,
    /// The wire `error` field.
    pub code: String,
    /// The wire `ref` field, a gateway trace id (`g-…`), sent on validation
    /// failures and absent on `not_found`. Quote it when reporting a failure.
    pub reference: Option<String>,
    /// `Retry-After`, whole seconds, sent only on token-bucket rejections.
    pub retry_after: Option<Duration>,
}

#[derive(Deserialize)]
struct Body {
    status: String,
    error: String,
    #[serde(default, rename = "ref")]
    reference: Option<String>,
}

impl VenueError {
    /// Parses a venue error body; `None` when the body is some other shape
    /// (a CDN error page, or a body without `status: "err"`).
    pub(crate) fn from_parts(status: u16, retry_after: Option<&str>, body: &str) -> Option<Self> {
        let body: Body = serde_json::from_str(body).ok()?;
        if body.status != "err" {
            return None;
        }
        Some(Self {
            status,
            code: body.error,
            reference: body.reference,
            retry_after: retry_after
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs),
        })
    }

    /// Whether re-sending the same request could plausibly succeed: a 429 or
    /// any 5xx.
    pub fn is_retriable(&self) -> bool {
        self.status == 429 || self.status >= 500
    }
}

impl PerpsError {
    /// Whether re-sending the same request could plausibly succeed.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => err.is_retriable(),
            Self::Venue(err) => err.is_retriable(),
        }
    }

    /// The venue's error identifier, for venue errors.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Venue(err) => Some(&err.code),
            Self::Api(_) => None,
        }
    }

    /// The `Retry-After` delay, for venue errors that carried one.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Venue(err) => err.retry_after,
            Self::Api(_) => None,
        }
    }
}

impl RequestError for PerpsError {
    async fn from_response(response: reqwest::Response) -> Self {
        let status = response.status().as_u16();
        let retry_after = retry_after_header(&response);
        let body = response.text().await.unwrap_or_default();

        // Told apart by body shape, not by path or status.
        match VenueError::from_parts(status, retry_after.as_deref(), &body) {
            Some(err) => Self::Venue(err),
            None => Self::Api(ApiError::from_status_and_body(status, &body)),
        }
    }
}

polyoxide_core::impl_api_error_conversions!(PerpsError);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_404_body_becomes_a_venue_error_with_its_identifier() {
        let err = VenueError::from_parts(404, None, r#"{"status":"err","error":"not_found"}"#)
            .expect("venue shape");
        assert_eq!(err.code, "not_found");
        assert_eq!(err.reference, None);
        assert!(!err.is_retriable());
    }

    #[test]
    fn a_400_body_keeps_the_gateway_reference() {
        // Captured 2026-09-30: validation failures carry `arts`, `ts` and `ref`
        // beyond the schema's two fields.
        let body = r#"{"status":"err","error":"invalid query parameters: missing field `instrument_id`","arts":1790758475821,"ts":1790758475821,"ref":"g-1224ed1744735"}"#;
        let err = VenueError::from_parts(400, None, body).expect("venue shape");
        assert_eq!(err.reference.as_deref(), Some("g-1224ed1744735"));
        assert!(err.code.starts_with("invalid query parameters"));
    }

    #[test]
    fn a_429_is_retriable_and_reads_retry_after_as_whole_seconds() {
        let err = PerpsError::Venue(
            VenueError::from_parts(429, Some("2"), r#"{"status":"err","error":"ip_rate_limited"}"#)
                .unwrap(),
        );
        assert!(err.is_retriable());
        assert_eq!(err.code(), Some("ip_rate_limited"));
        assert_eq!(err.retry_after(), Some(Duration::from_secs(2)));
    }

    #[test]
    fn a_body_without_status_err_is_not_a_venue_error() {
        assert_eq!(VenueError::from_parts(200, None, r#"{"status":"ok"}"#), None);
        assert_eq!(VenueError::from_parts(502, None, "<html>bad gateway</html>"), None);
    }

    #[test]
    fn api_errors_have_no_code_or_retry_after() {
        let err = PerpsError::from(ApiError::Timeout);
        assert!(err.is_retriable());
        assert_eq!(err.code(), None);
        assert_eq!(err.retry_after(), None);
    }

    #[test]
    fn perps_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PerpsError>();
    }
}
```

- [ ] **Step 4: Write `polyoxide-perps/src/client.rs`**

```rust
//! The `Perps` client and its builder.

use polyoxide_core::{
    HttpClient, HttpClientBuilder, RateLimiter, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::error::PerpsError;

/// Production Perps HTTP API host.
pub const DEFAULT_BASE_URL: &str = "https://api.perpetuals.polymarket.com";

/// In-flight requests the client allows by default, matching the sibling
/// read-only crates.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for the public Perps HTTP API. No credentials are needed.
#[derive(Clone)]
pub struct Perps {
    pub(crate) http_client: HttpClient,
}

impl Perps {
    /// A client with default settings.
    pub fn new() -> Result<Self, PerpsError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> PerpsBuilder {
        PerpsBuilder::new()
    }
}

/// Builder for [`Perps`].
pub struct PerpsBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    rate_limiter: Option<RateLimiter>,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
}

impl PerpsBuilder {
    fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            rate_limiter: None,
            retry_config: None,
            max_concurrent: None,
        }
    }

    /// Override the host, for example to point at a mock server.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Request timeout in milliseconds.
    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    /// Idle connections kept per host.
    pub fn pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Replace the rate limiter.
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Replace the retry policy.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Maximum in-flight requests (default 4).
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Perps, PerpsError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(DEFAULT_MAX_CONCURRENT));
        if let Some(limiter) = self.rate_limiter {
            builder = builder.with_rate_limiter(limiter);
        }
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        Ok(Perps {
            http_client: builder.build()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_targets_the_production_host() {
        let perps = Perps::new().expect("client builds");
        assert_eq!(
            perps.http_client.base_url.as_str(),
            "https://api.perpetuals.polymarket.com/"
        );
    }

    #[test]
    fn a_bad_base_url_is_a_url_error() {
        let err = Perps::builder().base_url("not a url").build().err().expect("fails");
        assert!(matches!(err, PerpsError::Api(polyoxide_core::ApiError::Url(_))));
    }
}
```

- [ ] **Step 5: Write `polyoxide-perps/src/api/mod.rs`, `src/types.rs`, `src/lib.rs`, `README.md`**

`src/api/mod.rs`:

```rust
//! API namespaces, one module per group of routes.

use polyoxide_core::{HttpClient, Request};
use serde::de::DeserializeOwned;

use crate::error::PerpsError;

/// A route with nothing left to set. Call [`Fetch::send`].
pub struct Fetch<T> {
    pub(crate) request: Request<T, PerpsError>,
}

impl<T: DeserializeOwned> Fetch<T> {
    /// Execute the request.
    pub async fn send(self) -> Result<T, PerpsError> {
        self.request.send().await
    }
}

pub(crate) fn fetch<T>(http_client: &HttpClient, path: &str) -> Fetch<T> {
    Fetch {
        request: Request::new(http_client.clone(), path),
    }
}

/// A chained query-parameter setter on a request builder that holds its
/// `Request` in a field named `request`.
macro_rules! setter {
    ($(#[$meta:meta])* $name:ident => $key:literal) => {
        $(#[$meta])*
        pub fn $name(mut self, value: impl ToString) -> Self {
            self.request = self.request.query($key, value);
            self
        }
    };
}
pub(crate) use setter;
```

`src/types.rs` for now:

```rust
//! Vocabulary shared by every namespace.
```

`src/lib.rs`:

```rust
//! Rust client for the Polymarket Perps HTTP API (`api.perpetuals.polymarket.com`).
//!
//! This crate covers the public `/v1/info/*` routes, which need no credentials.
//!
//! ```no_run
//! use polyoxide_perps::Perps;
//!
//! # async fn example() -> Result<(), polyoxide_perps::PerpsError> {
//! let perps = Perps::new()?;
//! let latency = perps.health().ping().await?;
//! println!("perps host answered in {latency:?}");
//! # Ok(())
//! # }
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod api;
pub mod client;
pub mod error;
pub mod types;

pub use client::{Perps, PerpsBuilder, DEFAULT_BASE_URL};
pub use error::{PerpsError, VenueError};
```

The `health()` call in the lib doc does not exist until Task 3; write the doc comment now but with the example as a plain `text` block, and switch it to `no_run` in Task 3. Concretely, use ```` ```text ```` here.

`README.md`:

````markdown
# polyoxide-perps

Rust client library for the Polymarket Perps API (perpetual futures).

Public market data: exchange and instrument reference data, tickers, order
books, klines, trades, funding, fees and leaderboards. No authentication
required for anything in this crate.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-perps/).

## Installation

```toml
[dependencies]
polyoxide-perps = "0.33"
```

## Usage

```text
use polyoxide_perps::Perps;

let perps = Perps::new()?;
let instruments = perps.exchange().instruments().send().await?;
```

The usage block becomes a `no_run` doctest in Task 4, once `exchange()` exists.
````

- [ ] **Step 6: Build and run the unit tests**

Run: `cargo test -p polyoxide-perps`
Expected: 8 tests pass (6 in `error`, 2 in `client`), zero warnings.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add Cargo.toml polyoxide-perps
git commit -m "feat(perps): crate skeleton with PerpsError and the Perps builder"
```

---

### Task 2: Vocabulary types

**Files:**
- Modify: `polyoxide-perps/src/types.rs`

- [ ] **Step 1: Write the failing tests at the bottom of `types.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_enum_serializes_to_the_wire_spelling_and_displays_the_same() {
        fn check<T: Serialize + Copy + std::fmt::Display + FromStr + PartialEq + std::fmt::Debug>(
            all: &[T],
            wire: &[&str],
        ) where
            <T as FromStr>::Err: std::fmt::Debug,
        {
            assert_eq!(all.len(), wire.len());
            for (variant, expected) in all.iter().zip(wire) {
                assert_eq!(serde_json::to_string(variant).unwrap(), format!("\"{expected}\""));
                assert_eq!(variant.to_string(), *expected);
                assert_eq!(expected.parse::<T>().unwrap(), *variant);
            }
        }
        check(
            Interval::ALL,
            &["1s", "1m", "5m", "15m", "30m", "1h", "4h", "6h", "12h", "1d", "1w"],
        );
        check(Side::ALL, &["long", "short"]);
        check(InstrumentType::ALL, &["perpetual"]);
        check(InstrumentCategory::ALL, &["equity", "commodity", "index", "crypto"]);
        check(LeaderboardWindow::ALL, &["day", "week", "month", "all"]);
        check(LeaderboardSort::ALL, &["pnl", "notional", "account_value"]);
        check(SortOrder::ALL, &["desc", "asc"]);
    }

    #[test]
    fn an_unknown_spelling_names_the_type_and_the_value() {
        let err = "2m".parse::<Interval>().unwrap_err();
        assert_eq!(err.to_string(), "\"2m\" is not a valid Interval");
    }

    #[test]
    fn book_depth_displays_its_level_count() {
        assert_eq!(BookDepth::Ten.to_string(), "10");
        assert_eq!(BookDepth::Thousand.levels(), 1000);
        assert_eq!(BookDepth::ALL.len(), 4);
    }

    #[test]
    fn instrument_id_is_a_transparent_integer() {
        let id: InstrumentId = serde_json::from_str("7").unwrap();
        assert_eq!(id, InstrumentId(7));
        assert_eq!(serde_json::to_string(&id).unwrap(), "7");
        assert_eq!(id.to_string(), "7");
    }

    #[test]
    fn a_kline_round_trips_through_its_positional_wire_form() {
        // Captured 2026-09-30 from /v1/info/klines.
        let wire = r#"[1790758080000,"7689.7","7689.7","7689.6","7689.6","1.47861",3]"#;
        let kline: Kline = serde_json::from_str(wire).unwrap();
        assert_eq!(kline.open_time, 1790758080000);
        assert_eq!(kline.high, Decimal::new(76897, 1));
        assert_eq!(kline.trades, 3);
        assert_eq!(serde_json::to_string(&kline).unwrap(), wire);
    }

    #[test]
    fn a_kline_with_the_wrong_arity_is_rejected() {
        assert!(serde_json::from_str::<Kline>(r#"[1,"2","3"]"#).is_err());
    }

    #[test]
    fn a_mark_point_and_a_level_round_trip() {
        let point: MarkPoint = serde_json::from_str(r#"[1790668800000,"7684.7"]"#).unwrap();
        assert_eq!(point.mark_price, Decimal::new(76847, 1));
        assert_eq!(serde_json::to_string(&point).unwrap(), r#"[1790668800000,"7684.7"]"#);

        let level: Level = serde_json::from_str(r#"["7688.5","0.31605"]"#).unwrap();
        assert_eq!(level.quantity, Decimal::new(31605, 5));
        assert_eq!(serde_json::to_string(&level).unwrap(), r#"["7688.5","0.31605"]"#);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p polyoxide-perps types`
Expected: compile errors, the types do not exist.

- [ ] **Step 3: Write the types above the tests**

```rust
//! Vocabulary shared by every namespace: identifiers, closed sets the spec
//! enumerates, and the positional rows the host sends as bare arrays.

use std::{fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A perps instrument id. One newtype so a REST parameter, a WebSocket channel
/// name and, later, a signed op cannot be handed a bare integer for the wrong
/// market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstrumentId(pub u64);

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for InstrumentId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// A string that is not one of a closed set's wire spellings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{value:?} is not a valid {type_name}")]
pub struct UnknownVariant {
    /// The Rust type being parsed.
    pub type_name: &'static str,
    /// The offending input.
    pub value: String,
}

/// A closed set with one wire spelling per variant. Generates serde renames,
/// `Display`, `FromStr` and an `ALL` table, so every spelling lives in one
/// place and the agreement test can walk them.
macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( #[serde(rename = $wire)] $variant, )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$name] = &[$( $name::$variant, )+];

            /// The wire spelling.
            pub fn as_str(self) -> &'static str {
                match self { $( $name::$variant => $wire, )+ }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownVariant;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $wire => Ok($name::$variant), )+
                    _ => Err(UnknownVariant { type_name: stringify!($name), value: s.to_owned() }),
                }
            }
        }
    };
}

wire_enum! {
    /// Kline and mark-history bucket width. Also the `klines` channel suffix.
    Interval {
        S1 => "1s", M1 => "1m", M5 => "5m", M15 => "15m", M30 => "30m",
        H1 => "1h", H4 => "4h", H6 => "6h", H12 => "12h", D1 => "1d", W1 => "1w",
    }
}

wire_enum! {
    /// Side of a trade or position.
    Side { Long => "long", Short => "short" }
}

wire_enum! {
    /// Instrument type. Only perpetuals are listed today.
    InstrumentType { Perpetual => "perpetual" }
}

wire_enum! {
    /// Instrument category.
    InstrumentCategory { Equity => "equity", Commodity => "commodity", Index => "index", Crypto => "crypto" }
}

wire_enum! {
    /// Leaderboard window.
    LeaderboardWindow { Day => "day", Week => "week", Month => "month", All => "all" }
}

wire_enum! {
    /// Leaderboard ranking key.
    LeaderboardSort { Pnl => "pnl", Notional => "notional", AccountValue => "account_value" }
}

wire_enum! {
    /// Sort direction for paged history.
    SortOrder { Desc => "desc", Asc => "asc" }
}

/// Levels per side that `GET /v1/info/book` can return. The WebSocket `book`
/// channel takes a different set (20 or 50) and has its own type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BookDepth {
    /// Ten levels per side.
    Ten,
    /// One hundred levels per side, the server default.
    Hundred,
    /// Five hundred levels per side.
    FiveHundred,
    /// One thousand levels per side.
    Thousand,
}

impl BookDepth {
    /// Every variant, in ascending order.
    pub const ALL: &'static [BookDepth] = &[
        BookDepth::Ten,
        BookDepth::Hundred,
        BookDepth::FiveHundred,
        BookDepth::Thousand,
    ];

    /// The number of levels per side.
    pub fn levels(self) -> u32 {
        match self {
            BookDepth::Ten => 10,
            BookDepth::Hundred => 100,
            BookDepth::FiveHundred => 500,
            BookDepth::Thousand => 1000,
        }
    }
}

impl fmt::Display for BookDepth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.levels())
    }
}

fn parse_decimal<E: serde::de::Error>(s: &str) -> Result<Decimal, E> {
    s.parse::<Decimal>().map_err(E::custom)
}

/// One candle. On the wire this is a positional array:
/// `[open_time, open, high, low, close, volume, trades]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Kline {
    /// Bucket open time, Unix milliseconds.
    pub open_time: u64,
    /// Open price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Close price.
    pub close: Decimal,
    /// Volume in contracts.
    pub volume: Decimal,
    /// Number of trades in the bucket.
    pub trades: u64,
}

#[derive(Serialize, Deserialize)]
struct KlineWire(u64, String, String, String, String, String, u64);

impl<'de> Deserialize<'de> for Kline {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let KlineWire(open_time, open, high, low, close, volume, trades) =
            KlineWire::deserialize(deserializer)?;
        Ok(Self {
            open_time,
            open: parse_decimal(&open)?,
            high: parse_decimal(&high)?,
            low: parse_decimal(&low)?,
            close: parse_decimal(&close)?,
            volume: parse_decimal(&volume)?,
            trades,
        })
    }
}

impl Serialize for Kline {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        KlineWire(
            self.open_time,
            self.open.to_string(),
            self.high.to_string(),
            self.low.to_string(),
            self.close.to_string(),
            self.volume.to_string(),
            self.trades,
        )
        .serialize(serializer)
    }
}

/// One mark-price sample. On the wire: `[time, mark_price]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MarkPoint {
    /// Bucket open time, Unix milliseconds.
    pub time: u64,
    /// Last mark price in the bucket.
    pub mark_price: Decimal,
}

impl<'de> Deserialize<'de> for MarkPoint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (time, mark_price): (u64, String) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            time,
            mark_price: parse_decimal(&mark_price)?,
        })
    }
}

impl Serialize for MarkPoint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.time, self.mark_price.to_string()).serialize(serializer)
    }
}

/// One order-book level. On the wire: `[price, quantity]`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Level {
    /// Price.
    pub price: Decimal,
    /// Quantity in contracts.
    pub quantity: Decimal,
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (price, quantity): (String, String) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            price: parse_decimal(&price)?,
            quantity: parse_decimal(&quantity)?,
        })
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.price.to_string(), self.quantity.to_string()).serialize(serializer)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p polyoxide-perps types`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-perps/src/types.rs
git commit -m "feat(perps): vocabulary types, closed-set enums and positional rows"
```

---

### Task 3: Health namespace (`ping`, `time`) and the mock test harness

**Files:**
- Create: `polyoxide-perps/src/api/health.rs`, `polyoxide-perps/tests/mock_api.rs`
- Modify: `polyoxide-perps/src/api/mod.rs`, `polyoxide-perps/src/client.rs`, `polyoxide-perps/src/lib.rs`

- [ ] **Step 1: Write the failing mock tests in `tests/mock_api.rs`**

```rust
//! Mock-server tests: one per builder, checking the path, the query keys the
//! builder sends, and the decoding of a representative body.

use mockito::{Matcher, Server, ServerGuard};
use polyoxide_perps::{Perps, PerpsError};

fn test_perps(server: &ServerGuard) -> Perps {
    Perps::builder().base_url(server.url()).build().unwrap()
}

// ── health ──────────────────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency_when_the_host_says_ok() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/ping")
        .with_status(200)
        .with_body(r#"{"status":"ok"}"#)
        .create_async()
        .await;

    let latency = test_perps(&server).health().ping().await.expect("ping");
    mock.assert_async().await;
    assert!(latency < std::time::Duration::from_secs(5));
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/time")
        .with_status(200)
        .with_body(r#"{"time":1790758431064}"#)
        .create_async()
        .await;

    let time = test_perps(&server).health().time().send().await.expect("time");
    mock.assert_async().await;
    assert_eq!(time.time, 1790758431064);
}

#[tokio::test]
async fn a_venue_error_body_maps_to_perps_error_venue() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .with_status(404)
        .with_body(r#"{"status":"err","error":"not_found"}"#)
        .create_async()
        .await;

    let err = test_perps(&server).health().time().send().await.unwrap_err();
    assert!(matches!(&err, PerpsError::Venue(v) if v.code == "not_found" && v.status == 404));
}

#[tokio::test]
async fn a_non_venue_error_body_stays_an_api_error() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/v1/info/time")
        .with_status(502)
        .with_body("<html>bad gateway</html>")
        .create_async()
        .await;

    let err = test_perps(&server).health().time().send().await.unwrap_err();
    assert!(matches!(err, PerpsError::Api(_)));
}
```

`Matcher` is unused until Task 4; keep the import, Task 4 uses it.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p polyoxide-perps --test mock_api`
Expected: compile error, `health` not found on `Perps`.

- [ ] **Step 3: Write `src/api/health.rs`**

```rust
//! Liveness routes: `/v1/info/ping` and `/v1/info/time`.

use std::time::{Duration, Instant};

use polyoxide_core::{ApiError, HttpClient, Request};
use serde::{Deserialize, Serialize};

use crate::{
    api::{fetch, Fetch},
    error::PerpsError,
};

/// Health namespace.
#[derive(Clone)]
pub struct Health {
    pub(crate) http_client: HttpClient,
}

#[derive(Deserialize)]
struct Ping {
    status: String,
}

/// `GET /v1/info/time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Time {
    /// Server clock, Unix milliseconds.
    pub time: u64,
}

impl Health {
    /// Round-trip time to the host, via `GET /v1/info/ping`.
    ///
    /// Goes through the same rate limiter and concurrency budget as every
    /// other route.
    pub async fn ping(&self) -> Result<Duration, PerpsError> {
        let start = Instant::now();
        let ping: Ping = Request::<Ping, PerpsError>::new(self.http_client.clone(), "/v1/info/ping")
            .send()
            .await?;
        if ping.status != "ok" {
            return Err(ApiError::Api {
                status: 200,
                message: format!("ping answered status {:?}", ping.status),
            }
            .into());
        }
        Ok(start.elapsed())
    }

    /// Server time, via `GET /v1/info/time`.
    pub fn time(&self) -> Fetch<Time> {
        fetch(&self.http_client, "/v1/info/time")
    }
}
```

- [ ] **Step 4: Wire the module and the accessor**

In `src/api/mod.rs`, add `pub mod health;` at the top (after the module doc comment).

In `src/client.rs`, add `use crate::api::health::Health;` and, inside `impl Perps` after `builder()`:

```rust
    /// Liveness: `ping`, `time`.
    pub fn health(&self) -> Health {
        Health {
            http_client: self.http_client.clone(),
        }
    }
```

In `src/lib.rs`, change the crate doc example's fence from ```` ```text ```` to ```` ```no_run ````.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p polyoxide-perps`
Expected: all unit tests plus 4 mock tests pass; the lib doctest compiles.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add polyoxide-perps
git commit -m "feat(perps): health namespace with ping and time"
```

---

### Task 4: Exchange namespace

**Files:**
- Create: `polyoxide-perps/src/api/exchange.rs`
- Modify: `polyoxide-perps/src/api/mod.rs`, `polyoxide-perps/src/client.rs`, `polyoxide-perps/tests/mock_api.rs`, `polyoxide-perps/README.md`

- [ ] **Step 1: Append the failing mock tests to `tests/mock_api.rs`**

```rust
// ── exchange ────────────────────────────────────────────────────

use polyoxide_perps::types::{InstrumentCategory, InstrumentId, InstrumentType};

#[tokio::test]
async fn instruments_sends_every_filter_and_decodes_undocumented_fields() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/instruments")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("instrument_type".into(), "perpetual".into()),
            Matcher::UrlEncoded("category".into(), "index".into()),
        ]))
        .with_status(200)
        .with_body(
            r#"[{"instrument_id":1,"instrument_type":"perpetual","category":"index","isolated_only":false,"symbol":"SP500-USD","display_symbol":"USA500-USD","close_only":false,"base_asset":"SP500","quote_asset":"pUSD","funding_interval":"1h","quantity_decimals":5,"price_decimals":1,"price_bounds":"0.02","liquidation_fee":"0.005","max_order_count":200,"min_notional":"10","max_market_notional":"1000000","max_limit_notional":"5000000","max_leverage":50,"risk_tiers":[{"lower_bound":"0","max_leverage":50}],"ui_live_time":1790000000000,"logo":"https://example/x.png"}]"#,
        )
        .create_async()
        .await;

    let rows = test_perps(&server)
        .exchange()
        .instruments()
        .instrument_id(InstrumentId(1))
        .instrument_type(InstrumentType::Perpetual)
        .category(InstrumentCategory::Index)
        .send()
        .await
        .expect("instruments");
    mock.assert_async().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].symbol, "SP500-USD");
    assert_eq!(rows[0].display_symbol.as_deref(), Some("USA500-USD"));
    assert_eq!(rows[0].close_only, Some(false));
    assert_eq!(rows[0].risk_tiers[0].max_leverage, 50);
}

#[tokio::test]
async fn exchange_assets_fees_and_limit_tiers_decode() {
    let mut server = Server::new_async().await;
    let exchange = server
        .mock("GET", "/v1/info/exchange")
        .with_body(r#"{"name":"Polymarket","version":"1","chain_id":137,"contract":"0xDCa4af75705dbB50f62437045afF9921947917d2","cancel_only":false,"engine_version":"0.0.7"}"#)
        .create_async()
        .await;
    let assets = server
        .mock("GET", "/v1/info/assets")
        .with_body(r#"[{"asset":"pUSD","address":"0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB","decimals":6,"collateral_ratio":"1.00","withdrawal_fee":"0.000000"}]"#)
        .create_async()
        .await;
    let fees = server
        .mock("GET", "/v1/info/fees")
        .with_body(r#"{"fee_schedule":[{"instrument_type":"perpetual","category":"equity","taker_fee_rate":"0.0004","maker_fee_rate":"0.000125","tiers":[{"min_volume_30d":"0","taker_fee_rate":"0.0004","maker_fee_rate":"0.000125"}]}]}"#)
        .create_async()
        .await;
    let tiers = server
        .mock("GET", "/v1/info/limit-tiers")
        .with_body(r#"[{"min_volume_14d":"0","rate_per_minute_limit":750,"rate_burst_limit":500,"actions_per_minute_limit":2000,"actions_burst_limit":500,"open_orders_limit":200,"messages_per_minute":1000,"connects_per_minute_limit":60,"max_connections":10,"ws_messages_burst_limit":100,"ws_messages_per_minute_limit":1200}]"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let x = perps.exchange().exchange().send().await.expect("exchange");
    assert_eq!(x.chain_id, 137);
    let a = perps.exchange().assets().send().await.expect("assets");
    assert_eq!(a[0].decimals, 6);
    let f = perps.exchange().fees().send().await.expect("fees");
    assert_eq!(f.fee_schedule[0].tiers.len(), 1);
    let t = perps.exchange().limit_tiers().send().await.expect("limit tiers");
    assert_eq!(t[0].rate_per_minute_limit, 750);
    assert_eq!(t[0].ws_messages_per_minute_limit, Some(1200));
    for m in [exchange, assets, fees, tiers] {
        m.assert_async().await;
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p polyoxide-perps --test mock_api`
Expected: compile error, `exchange` not found on `Perps`.

- [ ] **Step 3: Write `src/api/exchange.rs`**

```rust
//! Reference data: `/v1/info/{exchange,assets,instruments,fees,limit-tiers}`.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    api::{fetch, setter, Fetch},
    error::PerpsError,
    types::{InstrumentCategory, InstrumentId, InstrumentType},
};

/// Exchange namespace: static reference data.
#[derive(Clone)]
pub struct ExchangeApi {
    pub(crate) http_client: HttpClient,
}

impl ExchangeApi {
    /// `GET /v1/info/exchange`: the EIP-712 domain and maintenance state.
    pub fn exchange(&self) -> Fetch<Exchange> {
        fetch(&self.http_client, "/v1/info/exchange")
    }

    /// `GET /v1/info/assets`: collateral assets.
    pub fn assets(&self) -> Fetch<Vec<Asset>> {
        fetch(&self.http_client, "/v1/info/assets")
    }

    /// `GET /v1/info/instruments`: listed instruments, optionally filtered.
    pub fn instruments(&self) -> ListInstruments {
        ListInstruments {
            request: Request::new(self.http_client.clone(), "/v1/info/instruments"),
        }
    }

    /// `GET /v1/info/fees`: the tiered maker/taker schedule.
    pub fn fees(&self) -> Fetch<FeesInfo> {
        fetch(&self.http_client, "/v1/info/fees")
    }

    /// `GET /v1/info/limit-tiers`: volume-based rate-limit tiers.
    pub fn limit_tiers(&self) -> Fetch<Vec<LimitTier>> {
        fetch(&self.http_client, "/v1/info/limit-tiers")
    }
}

/// Request builder for `GET /v1/info/instruments`.
pub struct ListInstruments {
    request: Request<Vec<Instrument>, PerpsError>,
}

impl ListInstruments {
    setter! {
        /// Restrict to one instrument.
        instrument_id => "instrument_id"
    }
    setter! {
        /// Restrict to one instrument type.
        instrument_type => "instrument_type"
    }
    setter! {
        /// Restrict to one category.
        category => "category"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Instrument>, PerpsError> {
        self.request.send().await
    }
}

/// `GET /v1/info/exchange`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Exchange {
    /// Exchange name used in the EIP-712 domain.
    pub name: String,
    /// Exchange version used in the EIP-712 domain.
    pub version: String,
    /// Chain the exchange is deployed on.
    pub chain_id: u64,
    /// Verifying contract of the EIP-712 domain.
    pub contract: String,
    /// True while the exchange is in cancel-only (maintenance) mode.
    pub cancel_only: bool,
    /// Engine release serving this response.
    pub engine_version: String,
}

/// A collateral asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Asset {
    /// Asset name.
    pub asset: String,
    /// Token address.
    pub address: String,
    /// Token decimals.
    pub decimals: u32,
    /// Collateral ratio.
    #[serde(with = "rust_decimal::serde::str")]
    pub collateral_ratio: Decimal,
    /// Withdrawal fee in decimalised asset units.
    #[serde(with = "rust_decimal::serde::str")]
    pub withdrawal_fee: Decimal,
}

/// A listed instrument.
///
/// `display_symbol`, `close_only` and `logo` are on the wire and not in the
/// published schema (`docs/specs/perps/OBSERVED.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Instrument {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Instrument type.
    pub instrument_type: InstrumentType,
    /// Category.
    pub category: InstrumentCategory,
    /// Whether only isolated margin is supported.
    pub isolated_only: bool,
    /// Symbol, e.g. `SP500-USD`.
    pub symbol: String,
    /// Symbol first-party interfaces show, when it differs. Undocumented.
    pub display_symbol: Option<String>,
    /// Whether new positions are refused. Undocumented.
    pub close_only: Option<bool>,
    /// Base asset name.
    pub base_asset: String,
    /// Quote asset name.
    pub quote_asset: String,
    /// Funding interval, e.g. `1h`.
    pub funding_interval: String,
    /// Decimal places for quantity.
    pub quantity_decimals: u32,
    /// Decimal places for price.
    pub price_decimals: u32,
    /// Price bounds as a fraction.
    #[serde(with = "rust_decimal::serde::str")]
    pub price_bounds: Decimal,
    /// Liquidation fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub liquidation_fee: Decimal,
    /// Maximum open orders.
    pub max_order_count: u32,
    /// Minimum order notional in USD.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_notional: Decimal,
    /// Maximum market-order notional in USD.
    #[serde(with = "rust_decimal::serde::str")]
    pub max_market_notional: Decimal,
    /// Maximum limit-order notional in USD.
    #[serde(with = "rust_decimal::serde::str")]
    pub max_limit_notional: Decimal,
    /// Maximum leverage.
    pub max_leverage: u32,
    /// Leverage caps by position size.
    pub risk_tiers: Vec<RiskTier>,
    /// When first-party interfaces may show the instrument, Unix ms.
    pub ui_live_time: u64,
    /// Logo URL. Undocumented.
    pub logo: Option<String>,
}

/// One leverage tier by position size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RiskTier {
    /// Position size lower bound.
    #[serde(with = "rust_decimal::serde::str")]
    pub lower_bound: Decimal,
    /// Maximum leverage at and above the bound.
    pub max_leverage: u32,
}

/// `GET /v1/info/fees`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FeesInfo {
    /// One entry per instrument type and category.
    pub fee_schedule: Vec<FeeScheduleEntry>,
}

/// Fees for one instrument type and category.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FeeScheduleEntry {
    /// Instrument type.
    pub instrument_type: InstrumentType,
    /// Category.
    pub category: InstrumentCategory,
    /// Base taker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub taker_fee_rate: Decimal,
    /// Base maker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub maker_fee_rate: Decimal,
    /// Volume tiers.
    pub tiers: Vec<FeeTier>,
}

/// One volume tier of the fee schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FeeTier {
    /// 30-day volume at which the tier starts.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_volume_30d: Decimal,
    /// Taker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub taker_fee_rate: Decimal,
    /// Maker fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub maker_fee_rate: Decimal,
}

/// One volume-based rate-limit tier.
///
/// The four `Option` fields are on the wire and not in the published schema
/// (`docs/specs/perps/OBSERVED.md`); they describe the WebSocket budget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LimitTier {
    /// 14-day volume at which the tier starts.
    #[serde(with = "rust_decimal::serde::str")]
    pub min_volume_14d: Decimal,
    /// Sustained request rate per minute.
    pub rate_per_minute_limit: u64,
    /// Request burst allowance.
    pub rate_burst_limit: u64,
    /// Sustained order actions per minute.
    pub actions_per_minute_limit: u64,
    /// Order-action burst allowance.
    pub actions_burst_limit: u64,
    /// Resting open-order cap.
    pub open_orders_limit: u64,
    /// Display-only messages-per-minute figure.
    pub messages_per_minute: u64,
    /// WebSocket connects per minute. Undocumented.
    pub connects_per_minute_limit: Option<u64>,
    /// Concurrent WebSocket connections. Undocumented.
    pub max_connections: Option<u64>,
    /// WebSocket inbound-message burst allowance. Undocumented.
    pub ws_messages_burst_limit: Option<u64>,
    /// WebSocket inbound messages per minute. Undocumented.
    pub ws_messages_per_minute_limit: Option<u64>,
}
```

- [ ] **Step 4: Wire the module, the accessor and the README**

`src/api/mod.rs`: add `pub mod exchange;`.

`src/client.rs`: add `use crate::api::exchange::ExchangeApi;` and

```rust
    /// Reference data: exchange, assets, instruments, fees, limit tiers.
    pub fn exchange(&self) -> ExchangeApi {
        ExchangeApi {
            http_client: self.http_client.clone(),
        }
    }
```

`README.md`: replace the `text` usage block with a real doctest:

````markdown
```no_run
use polyoxide_perps::Perps;

# async fn example() -> Result<(), polyoxide_perps::PerpsError> {
let perps = Perps::new()?;
let instruments = perps.exchange().instruments().send().await?;
for instrument in &instruments {
    println!("{} {}", instrument.instrument_id, instrument.symbol);
}
# Ok(())
# }
```
````

and delete the sentence about Task 4.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p polyoxide-perps`
Expected: all pass, including the README doctest.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add polyoxide-perps
git commit -m "feat(perps): exchange namespace"
```

---

### Task 5: Market namespace

**Files:**
- Create: `polyoxide-perps/src/api/market.rs`
- Modify: `polyoxide-perps/src/api/mod.rs`, `polyoxide-perps/src/client.rs`, `polyoxide-perps/tests/mock_api.rs`

- [ ] **Step 1: Append the failing mock tests to `tests/mock_api.rs`**

```rust
// ── market ──────────────────────────────────────────────────────

use polyoxide_perps::types::{BookDepth, Interval, Side};
use rust_decimal::Decimal;

#[tokio::test]
async fn klines_sends_required_and_optional_params_and_decodes_rows() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/klines")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("interval".into(), "1m".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "100".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "200".into()),
        ]))
        .with_body(r#"{"data":[[1790758080000,"7689.7","7689.7","7689.6","7689.6","1.47861",3]],"more":false}"#)
        .create_async()
        .await;

    let klines = test_perps(&server)
        .market()
        .klines(InstrumentId(1), Interval::M1, 100)
        .end(200)
        .send()
        .await
        .expect("klines");
    mock.assert_async().await;
    assert_eq!(klines.data[0].trades, 3);
    assert!(!klines.more);
}

#[tokio::test]
async fn book_sends_depth_and_decodes_levels() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/book")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("depth".into(), "10".into()),
        ]))
        .with_body(r#"{"instrument_id":1,"bids":[["7688.5","0.31605"]],"asks":[["7688.6","0.31358"]],"timestamp":1790758475031,"sequence":58737731190}"#)
        .create_async()
        .await;

    let book = test_perps(&server)
        .market()
        .book(InstrumentId(1))
        .depth(BookDepth::Ten)
        .send()
        .await
        .expect("book");
    mock.assert_async().await;
    assert_eq!(book.bids[0].price, Decimal::new(76885, 1));
    assert_eq!(book.sequence, 58737731190);
}

#[tokio::test]
async fn tickers_statistics_bbo_and_index_decode() {
    let mut server = Server::new_async().await;
    let tickers = server
        .mock("GET", "/v1/info/tickers")
        .match_query(Matcher::UrlEncoded("instrument_id".into(), "1".into()))
        .with_body(r#"[{"instrument_id":1,"symbol":"SP500-USD","index_price":"7686.8","mark_price":"7687.8","last_price":"7689.9","mid_price":"7687.8","open_interest":"1000","funding_rate":"0.00000625","next_funding":1790762400000,"timestamp":1790758485077}]"#)
        .create_async()
        .await;
    let statistics = server
        .mock("GET", "/v1/info/statistics")
        .match_query(Matcher::Any)
        .with_body(r#"[{"instrument_id":1,"symbol":"SP500-USD","volume":"1255218.444899","open_price":"7682.7","klines":[[1790668800000,"7682.7","7683","7682.7","7682.9","0.06996",4]]}]"#)
        .create_async()
        .await;
    let bbo = server
        .mock("GET", "/v1/info/bbo")
        .match_query(Matcher::Any)
        .with_body(r#"[{"instrument_id":1,"bid_price":"7687.8","bid_quantity":"6.85437","ask_price":"7687.9","ask_quantity":"0.31358","timestamp":1790758485077}]"#)
        .create_async()
        .await;
    let index = server
        .mock("GET", "/v1/info/index")
        .match_query(Matcher::UrlEncoded("asset".into(), "BTC".into()))
        .with_body(r#"{"asset":"BTC","index_price":"83019","constituents":[{"source":"binance","symbol":"BTCUSDT","weight":"0.5","price":"83020"}],"ts":1790758492229}"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let t = perps.market().tickers().instrument_id(InstrumentId(1)).send().await.expect("tickers");
    assert_eq!(t[0].next_funding, 1790762400000);
    let s = perps.market().statistics().send().await.expect("statistics");
    assert_eq!(s[0].klines[0].trades, 4);
    let b = perps.market().bbo().send().await.expect("bbo");
    assert_eq!(b[0].ask_quantity, Decimal::new(31358, 5));
    let i = perps.market().index("BTC").send().await.expect("index");
    assert_eq!(i.constituents[0].source, "binance");
    for m in [tickers, statistics, bbo, index] {
        m.assert_async().await;
    }
}

#[tokio::test]
async fn exchange_stats_mark_history_trades_and_funding_decode() {
    let mut server = Server::new_async().await;
    let stats = server
        .mock("GET", "/v1/info/exchange-stats")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"start_timestamp":1,"end_timestamp":2,"volume":"65715149.726359","open_interest":"75573217.100902647081712288","open_interest_timestamp":2,"fees":"1000.5"}"#)
        .create_async()
        .await;
    let marks = server
        .mock("GET", "/v1/info/mark-history")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("interval".into(), "1h".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"data":[[1790668800000,"7684.7"]],"more":false}"#)
        .create_async()
        .await;
    let trades = server
        .mock("GET", "/v1/info/trades")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"data":[{"trade_id":1736331004042335,"instrument_id":1,"side":"long","price":"7689.9","quantity":"2.28875","settlement":false,"timestamp":1790758400000,"hash":"0xabc"}],"more":true}"#)
        .create_async()
        .await;
    let funding = server
        .mock("GET", "/v1/info/funding")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("start_timestamp".into(), "1".into()),
            Matcher::UrlEncoded("end_timestamp".into(), "2".into()),
        ]))
        .with_body(r#"{"data":[{"funding_rate":"0.00000625","timestamp":1790755200032}],"more":false}"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let x = perps.market().exchange_stats(1, 2).send().await.expect("exchange stats");
    // 32 significant digits on the wire; Decimal keeps 28 and rounds.
    assert!(x.open_interest > Decimal::new(75_573_217, 0));
    let m = perps
        .market()
        .mark_history(InstrumentId(1), Interval::H1, 1)
        .end(2)
        .send()
        .await
        .expect("mark history");
    assert_eq!(m.data[0].mark_price, Decimal::new(76847, 1));
    let t = perps
        .market()
        .trades(InstrumentId(1))
        .start(1)
        .end(2)
        .send()
        .await
        .expect("trades");
    assert_eq!(t.data[0].side, Side::Long);
    assert_eq!(t.data[0].settlement, Some(false));
    assert!(t.more);
    let f = perps
        .market()
        .funding(InstrumentId(1))
        .start(1)
        .end(2)
        .send()
        .await
        .expect("funding");
    assert_eq!(f.data[0].funding_rate, Decimal::new(625, 8));
    for m in [stats, marks, trades, funding] {
        m.assert_async().await;
    }
}
```

Add `rust_decimal = { workspace = true }` to `[dev-dependencies]` in `polyoxide-perps/Cargo.toml` (it is already a normal dependency; tests use it directly so list it there too for clarity, or rely on the normal dependency: either compiles).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p polyoxide-perps --test mock_api`
Expected: compile error, `market` not found on `Perps`.

- [ ] **Step 3: Write `src/api/market.rs`**

```rust
//! Market data keyed by instrument: tickers, statistics, klines, mark history,
//! BBO, book, index, trades, funding, and exchange-wide statistics.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    api::{fetch, setter, Fetch},
    error::PerpsError,
    types::{InstrumentId, Interval, Kline, Level, MarkPoint, Side},
};

/// Market namespace.
#[derive(Clone)]
pub struct MarketApi {
    pub(crate) http_client: HttpClient,
}

impl MarketApi {
    /// `GET /v1/info/tickers`: mark, index, last and mid prices per instrument.
    pub fn tickers(&self) -> ListTickers {
        ListTickers {
            request: Request::new(self.http_client.clone(), "/v1/info/tickers"),
        }
    }

    /// `GET /v1/info/statistics`: 24-hour volume, open and hourly klines.
    pub fn statistics(&self) -> ListStatistics {
        ListStatistics {
            request: Request::new(self.http_client.clone(), "/v1/info/statistics"),
        }
    }

    /// `GET /v1/info/exchange-stats`: exchange-wide volume, open interest and
    /// fees over `[start, end]` (Unix ms).
    pub fn exchange_stats(&self, start_timestamp: u64, end_timestamp: u64) -> Fetch<ExchangeStatistics> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/exchange-stats")
                .query("start_timestamp", start_timestamp)
                .query("end_timestamp", end_timestamp),
        }
    }

    /// `GET /v1/info/klines`: candles from `start_timestamp` (Unix ms).
    pub fn klines(&self, instrument_id: InstrumentId, interval: Interval, start_timestamp: u64) -> GetKlines {
        GetKlines {
            request: Request::new(self.http_client.clone(), "/v1/info/klines")
                .query("instrument_id", instrument_id)
                .query("interval", interval)
                .query("start_timestamp", start_timestamp),
        }
    }

    /// `GET /v1/info/mark-history`: mark-price samples from `start_timestamp`.
    pub fn mark_history(&self, instrument_id: InstrumentId, interval: Interval, start_timestamp: u64) -> GetMarkHistory {
        GetMarkHistory {
            request: Request::new(self.http_client.clone(), "/v1/info/mark-history")
                .query("instrument_id", instrument_id)
                .query("interval", interval)
                .query("start_timestamp", start_timestamp),
        }
    }

    /// `GET /v1/info/bbo`: best bid and offer per instrument.
    pub fn bbo(&self) -> ListBbo {
        ListBbo {
            request: Request::new(self.http_client.clone(), "/v1/info/bbo"),
        }
    }

    /// `GET /v1/info/book`: the order book for one instrument.
    ///
    /// An instrument the host does not know answers 200 with empty sides,
    /// not 404 (`docs/specs/perps/OBSERVED.md`).
    pub fn book(&self, instrument_id: InstrumentId) -> GetBook {
        GetBook {
            request: Request::new(self.http_client.clone(), "/v1/info/book")
                .query("instrument_id", instrument_id),
        }
    }

    /// `GET /v1/info/index`: the index price and its constituents for a base
    /// asset name such as `BTC`.
    pub fn index(&self, asset: impl ToString) -> Fetch<Index> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/index").query("asset", asset),
        }
    }

    /// `GET /v1/info/trades`: recent public trades.
    pub fn trades(&self, instrument_id: InstrumentId) -> ListTrades {
        ListTrades {
            request: Request::new(self.http_client.clone(), "/v1/info/trades")
                .query("instrument_id", instrument_id),
        }
    }

    /// `GET /v1/info/funding`: historical funding rates.
    pub fn funding(&self, instrument_id: InstrumentId) -> GetFunding {
        GetFunding {
            request: Request::new(self.http_client.clone(), "/v1/info/funding")
                .query("instrument_id", instrument_id),
        }
    }
}

/// Request builder for `GET /v1/info/tickers`.
pub struct ListTickers {
    request: Request<Vec<Ticker>, PerpsError>,
}

impl ListTickers {
    setter! {
        /// Restrict to one instrument.
        instrument_id => "instrument_id"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Ticker>, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/statistics`.
pub struct ListStatistics {
    request: Request<Vec<Statistic>, PerpsError>,
}

impl ListStatistics {
    setter! {
        /// Restrict to one instrument.
        instrument_id => "instrument_id"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Statistic>, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/klines`.
pub struct GetKlines {
    request: Request<Klines, PerpsError>,
}

impl GetKlines {
    setter! {
        /// End of the range, Unix ms. Defaults to now.
        end => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Klines, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/mark-history`.
pub struct GetMarkHistory {
    request: Request<MarkHistory, PerpsError>,
}

impl GetMarkHistory {
    setter! {
        /// End of the range, Unix ms. Defaults to now.
        end => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<MarkHistory, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/bbo`.
pub struct ListBbo {
    request: Request<Vec<Bbo>, PerpsError>,
}

impl ListBbo {
    setter! {
        /// Restrict to one instrument.
        instrument_id => "instrument_id"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Bbo>, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/book`.
pub struct GetBook {
    request: Request<Book, PerpsError>,
}

impl GetBook {
    setter! {
        /// Levels per side. The server default is 100.
        depth => "depth"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Book, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/trades`.
pub struct ListTrades {
    request: Request<Trades, PerpsError>,
}

impl ListTrades {
    setter! {
        /// Start of the range, Unix ms.
        start => "start_timestamp"
    }
    setter! {
        /// End of the range, Unix ms.
        end => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Trades, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/funding`.
pub struct GetFunding {
    request: Request<FundingHistory, PerpsError>,
}

impl GetFunding {
    setter! {
        /// Start of the range, Unix ms.
        start => "start_timestamp"
    }
    setter! {
        /// End of the range, Unix ms.
        end => "end_timestamp"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<FundingHistory, PerpsError> {
        self.request.send().await
    }
}

/// One instrument's ticker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ticker {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Symbol.
    pub symbol: String,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Last traded price.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Mid price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mid_price: Decimal,
    /// Open interest in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
    /// Current funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Next funding time, Unix ms.
    pub next_funding: u64,
    /// Sample time, Unix ms.
    pub timestamp: u64,
}

/// One instrument's 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Statistic {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Symbol.
    pub symbol: String,
    /// 24-hour volume in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Price 24 hours ago.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Hourly candles for the last 24 hours.
    pub klines: Vec<Kline>,
}

/// `GET /v1/info/exchange-stats`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExchangeStatistics {
    /// Range start, Unix ms.
    pub start_timestamp: u64,
    /// Range end, Unix ms.
    pub end_timestamp: u64,
    /// Volume over the range.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Open interest at `open_interest_timestamp`. The wire carries more
    /// digits than `Decimal` holds; the value is rounded to 28 significant.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
    /// When `open_interest` was sampled, Unix ms.
    pub open_interest_timestamp: u64,
    /// Fees collected over the range.
    #[serde(with = "rust_decimal::serde::str")]
    pub fees: Decimal,
}

/// `GET /v1/info/klines`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Klines {
    /// Candles, at most 1000.
    pub data: Vec<Kline>,
    /// Whether more candles exist past the last one returned.
    pub more: bool,
}

/// `GET /v1/info/mark-history`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarkHistory {
    /// Samples, at most 1000.
    pub data: Vec<MarkPoint>,
    /// Whether more samples exist past the last one returned.
    pub more: bool,
}

/// Best bid and offer for one instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Bbo {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Best bid price.
    #[serde(with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Quantity at the best bid.
    #[serde(with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Quantity at the best ask.
    #[serde(with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
    /// Sample time, Unix ms.
    pub timestamp: u64,
}

/// `GET /v1/info/book`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Book {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Bid levels, best first.
    pub bids: Vec<Level>,
    /// Ask levels, best first.
    pub asks: Vec<Level>,
    /// Snapshot time, Unix ms.
    pub timestamp: u64,
    /// Book sequence number.
    pub sequence: u64,
}

/// `GET /v1/info/index`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Index {
    /// Base asset name.
    pub asset: String,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Sources the index is computed from. Empty for some assets.
    pub constituents: Vec<IndexConstituent>,
    /// Sample time, Unix ms.
    pub ts: u64,
}

/// One source of an index price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IndexConstituent {
    /// Venue.
    pub source: String,
    /// Symbol at the venue.
    pub symbol: String,
    /// Weight in the index.
    #[serde(with = "rust_decimal::serde::str")]
    pub weight: Decimal,
    /// Price at the venue.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
}

/// `GET /v1/info/trades`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Trades {
    /// Trades, newest first.
    pub data: Vec<Trade>,
    /// Whether more trades exist past the last one returned.
    pub more: bool,
}

/// One public trade.
///
/// `settlement` is on the wire and not in the published schema
/// (`docs/specs/perps/OBSERVED.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Trade {
    /// Trade id.
    pub trade_id: u64,
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Taker side.
    pub side: Side,
    /// Price.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Whether this was a settlement trade. Undocumented.
    pub settlement: Option<bool>,
    /// Trade time, Unix ms.
    pub timestamp: u64,
    /// Transaction hash.
    pub hash: String,
}

/// `GET /v1/info/funding`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FundingHistory {
    /// Funding rates, newest first.
    pub data: Vec<FundingRate>,
    /// Whether more rates exist past the last one returned.
    pub more: bool,
}

/// One funding settlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FundingRate {
    /// Rate applied.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Settlement time, Unix ms.
    pub timestamp: u64,
}
```

- [ ] **Step 4: Wire the module and the accessor**

`src/api/mod.rs`: add `pub mod market;`.

`src/client.rs`: add `use crate::api::market::MarketApi;` and

```rust
    /// Market data keyed by instrument.
    pub fn market(&self) -> MarketApi {
        MarketApi {
            http_client: self.http_client.clone(),
        }
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p polyoxide-perps`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add polyoxide-perps
git commit -m "feat(perps): market namespace"
```

---

### Task 6: Public namespace

**Files:**
- Create: `polyoxide-perps/src/api/public.rs`
- Modify: `polyoxide-perps/src/api/mod.rs`, `polyoxide-perps/src/client.rs`, `polyoxide-perps/tests/mock_api.rs`

- [ ] **Step 1: Append the failing mock tests to `tests/mock_api.rs`**

```rust
// ── public ──────────────────────────────────────────────────────

use polyoxide_perps::types::{LeaderboardSort, LeaderboardWindow, SortOrder};

#[tokio::test]
async fn leaderboard_sends_every_setter_and_decodes_the_optional_account() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/info/leaderboard")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("window".into(), "week".into()),
            Matcher::UrlEncoded("sort_by".into(), "account_value".into()),
            Matcher::UrlEncoded("limit".into(), "3".into()),
            Matcher::UrlEncoded("offset".into(), "6".into()),
            Matcher::UrlEncoded("address".into(), "0xabc".into()),
        ]))
        .with_body(r#"{"window":"week","sort_by":"account_value","timestamp":1790758440134,"total":7751,"entries":[{"rank":1,"account":"0x65c8","pnl":"1","notional":"2","account_value":"3"}],"account":{"account":"0xabc","pnl":"0","notional":"0","account_value":"0"}}"#)
        .create_async()
        .await;

    let board = test_perps(&server)
        .public()
        .leaderboard()
        .window(LeaderboardWindow::Week)
        .sort_by(LeaderboardSort::AccountValue)
        .limit(3)
        .offset(6)
        .address("0xabc")
        .send()
        .await
        .expect("leaderboard");
    mock.assert_async().await;
    assert_eq!(board.total, 7751);
    assert_eq!(board.entries[0].rank, 1);
    let account = board.account.expect("account echoed");
    assert_eq!(account.rank, None);
}

#[tokio::test]
async fn portfolio_position_fills_and_invite_decode() {
    let mut server = Server::new_async().await;
    let portfolio = server
        .mock("GET", "/v1/info/portfolio")
        .match_query(Matcher::UrlEncoded("address".into(), "0xabc".into()))
        .with_body(r#"{"positions":[{"instrument_id":32,"symbol":"ZEC-USD","size":"-253.4562","entry_price":"1446.1","unrealized_pnl":"10642.35","return_on_equity":"0.1"}],"equity":"100","timestamp":1790758440134}"#)
        .create_async()
        .await;
    let fills = server
        .mock("GET", "/v1/info/position-fills")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), "0xabc".into()),
            Matcher::UrlEncoded("instrument_id".into(), "1".into()),
            Matcher::UrlEncoded("cursor".into(), "c1".into()),
            Matcher::UrlEncoded("sort".into(), "asc".into()),
        ]))
        .with_body(r#"{"data":[{"trade_id":1,"order_id":2,"instrument_id":1,"side":"short","price":"1","quantity":"2","taker":true,"fee":"0.1","fee_asset":"pUSD","previous_size":"0","previous_entry_price":"0","pnl":"0","liquidation":false,"adl":false,"timestamp":1,"hash":"0x"}],"more":true,"cursor":"c2"}"#)
        .create_async()
        .await;
    let invite = server
        .mock("GET", "/v1/info/invite")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("code".into(), "nope".into()),
            Matcher::UrlEncoded("address".into(), "0xabc".into()),
        ]))
        .with_body(r#"{"valid":false,"error":"code not found"}"#)
        .create_async()
        .await;

    let perps = test_perps(&server);
    let p = perps.public().portfolio("0xabc").send().await.expect("portfolio");
    assert_eq!(p.positions[0].size, Decimal::new(-2534562, 4));
    let f = perps
        .public()
        .position_fills("0xabc", InstrumentId(1))
        .cursor("c1")
        .sort(SortOrder::Asc)
        .send()
        .await
        .expect("position fills");
    assert_eq!(f.cursor.as_deref(), Some("c2"));
    assert!(f.data[0].taker);
    let i = perps.public().invite("nope").address("0xabc").send().await.expect("invite");
    assert!(!i.valid);
    assert_eq!(i.error.as_deref(), Some("code not found"));
    for m in [portfolio, fills, invite] {
        m.assert_async().await;
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p polyoxide-perps --test mock_api`
Expected: compile error, `public` not found on `Perps`.

- [ ] **Step 3: Write `src/api/public.rs`**

```rust
//! Public-by-address lookups and the invite check:
//! `/v1/info/{portfolio,position-fills,leaderboard,invite}`.

use polyoxide_core::{HttpClient, QueryBuilder, Request};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    api::{setter, Fetch},
    error::PerpsError,
    types::{InstrumentId, LeaderboardSort, LeaderboardWindow, Side},
};

/// Public namespace.
#[derive(Clone)]
pub struct PublicApi {
    pub(crate) http_client: HttpClient,
}

impl PublicApi {
    /// `GET /v1/info/portfolio`: an account's open positions and equity.
    pub fn portfolio(&self, address: impl ToString) -> Fetch<PublicPortfolio> {
        Fetch {
            request: Request::new(self.http_client.clone(), "/v1/info/portfolio")
                .query("address", address),
        }
    }

    /// `GET /v1/info/position-fills`: the fills behind an account's current
    /// position in one instrument, cursor-paged.
    pub fn position_fills(&self, address: impl ToString, instrument_id: InstrumentId) -> ListPositionFills {
        ListPositionFills {
            request: Request::new(self.http_client.clone(), "/v1/info/position-fills")
                .query("address", address)
                .query("instrument_id", instrument_id),
        }
    }

    /// `GET /v1/info/leaderboard`.
    pub fn leaderboard(&self) -> GetLeaderboard {
        GetLeaderboard {
            request: Request::new(self.http_client.clone(), "/v1/info/leaderboard"),
        }
    }

    /// `GET /v1/info/invite`: whether an invite code is valid.
    pub fn invite(&self, code: impl ToString) -> CheckInvite {
        CheckInvite {
            request: Request::new(self.http_client.clone(), "/v1/info/invite").query("code", code),
        }
    }
}

/// Request builder for `GET /v1/info/position-fills`.
pub struct ListPositionFills {
    request: Request<PositionFills, PerpsError>,
}

impl ListPositionFills {
    setter! {
        /// Resume from a previous page's `cursor`.
        cursor => "cursor"
    }
    setter! {
        /// Sort order. The server default is descending.
        sort => "sort"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<PositionFills, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/leaderboard`.
pub struct GetLeaderboard {
    request: Request<Leaderboard, PerpsError>,
}

impl GetLeaderboard {
    setter! {
        /// Window. The server default is `day`.
        window => "window"
    }
    setter! {
        /// Ranking key. The server default is `pnl`.
        sort_by => "sort_by"
    }
    setter! {
        /// Page size.
        limit => "limit"
    }
    setter! {
        /// Page offset.
        offset => "offset"
    }
    setter! {
        /// Also return this account's own standing as `account`.
        address => "address"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Leaderboard, PerpsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /v1/info/invite`.
pub struct CheckInvite {
    request: Request<InviteCheck, PerpsError>,
}

impl CheckInvite {
    setter! {
        /// The address that would redeem the code.
        address => "address"
    }

    /// Execute the request.
    pub async fn send(self) -> Result<InviteCheck, PerpsError> {
        self.request.send().await
    }
}

/// `GET /v1/info/portfolio`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PublicPortfolio {
    /// Open positions.
    pub positions: Vec<PublicPortfolioPosition>,
    /// Account equity.
    #[serde(with = "rust_decimal::serde::str")]
    pub equity: Decimal,
    /// Snapshot time, Unix ms.
    pub timestamp: u64,
}

/// One open position on a public portfolio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PublicPortfolioPosition {
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Symbol.
    pub symbol: String,
    /// Signed size in contracts; negative is short.
    #[serde(with = "rust_decimal::serde::str")]
    pub size: Decimal,
    /// Average entry price.
    #[serde(with = "rust_decimal::serde::str")]
    pub entry_price: Decimal,
    /// Unrealised PnL.
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    /// Return on equity as a fraction.
    #[serde(with = "rust_decimal::serde::str")]
    pub return_on_equity: Decimal,
}

/// `GET /v1/info/position-fills`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionFills {
    /// Fills.
    pub data: Vec<PositionFill>,
    /// Whether another page exists.
    pub more: bool,
    /// Cursor for the next page; absent on the last page.
    pub cursor: Option<String>,
}

/// One fill behind a current position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PositionFill {
    /// Trade id.
    pub trade_id: u64,
    /// Order id.
    pub order_id: u64,
    /// Instrument id.
    pub instrument_id: InstrumentId,
    /// Side.
    pub side: Side,
    /// Price.
    #[serde(with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Whether the account was the taker.
    pub taker: bool,
    /// Fee paid.
    #[serde(with = "rust_decimal::serde::str")]
    pub fee: Decimal,
    /// Asset the fee was paid in.
    pub fee_asset: String,
    /// Position size before the fill.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_size: Decimal,
    /// Entry price before the fill.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_entry_price: Decimal,
    /// Realised PnL from the fill.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Whether the fill was a liquidation.
    pub liquidation: bool,
    /// Whether the fill was auto-deleveraging.
    pub adl: bool,
    /// Fill time, Unix ms.
    pub timestamp: u64,
    /// Transaction hash.
    pub hash: String,
}

/// `GET /v1/info/leaderboard`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Leaderboard {
    /// Window the board covers.
    pub window: LeaderboardWindow,
    /// Ranking key.
    pub sort_by: LeaderboardSort,
    /// Snapshot time, Unix ms.
    pub timestamp: u64,
    /// Total ranked accounts.
    pub total: u64,
    /// This page of entries.
    pub entries: Vec<LeaderboardEntry>,
    /// The standing of the `address` the request named; absent otherwise.
    pub account: Option<LeaderboardAccount>,
}

/// One ranked account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LeaderboardEntry {
    /// Rank, 1-based.
    pub rank: u64,
    /// Account address.
    pub account: String,
    /// PnL over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Notional traded over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub notional: Decimal,
    /// Account value.
    #[serde(with = "rust_decimal::serde::str")]
    pub account_value: Decimal,
}

/// The requesting account's own standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct LeaderboardAccount {
    /// Rank, absent when the account is unranked.
    pub rank: Option<u64>,
    /// Account address.
    pub account: String,
    /// PnL over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub pnl: Decimal,
    /// Notional traded over the window.
    #[serde(with = "rust_decimal::serde::str")]
    pub notional: Decimal,
    /// Account value.
    #[serde(with = "rust_decimal::serde::str")]
    pub account_value: Decimal,
}

/// `GET /v1/info/invite`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct InviteCheck {
    /// Whether the code can be redeemed.
    pub valid: bool,
    /// Why not, when `valid` is false.
    pub error: Option<String>,
}
```

- [ ] **Step 4: Wire the module and the accessor**

`src/api/mod.rs`: add `pub mod public;`.

`src/client.rs`: add `use crate::api::public::PublicApi;` and

```rust
    /// Public-by-address lookups: portfolio, position fills, leaderboard, invite.
    pub fn public(&self) -> PublicApi {
        PublicApi {
            http_client: self.http_client.clone(),
        }
    }
```

- [ ] **Step 5: Run everything, then clippy and docs**

Run:
```bash
cargo test -p polyoxide-perps
cargo clippy -p polyoxide-perps --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p polyoxide-perps --no-deps
```
Expected: all pass, zero warnings.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add polyoxide-perps
git commit -m "feat(perps): public namespace; all 20 info routes covered"
```

---

### Task 7: Fixture capture script and captured fixtures

**Files:**
- Create: `scripts/capture_perps_fixtures.py`, `polyoxide-perps/tests/fixtures/*.json`, `polyoxide-perps/tests/fixtures/PROVENANCE.md`

- [ ] **Step 1: Write `scripts/capture_perps_fixtures.py`**

```python
#!/usr/bin/env python3
"""Capture one live response per public Perps route as a test fixture.

Inputs are chosen live: the first instrument from /v1/info/instruments, an
address from the weekly leaderboard, and the first base asset whose index has
constituents. Requests are spaced out.

Usage:
    python3 scripts/capture_perps_fixtures.py polyoxide-perps/tests/fixtures

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

HOST = "https://api.perpetuals.polymarket.com"
PAUSE_SECONDS = 0.5
DAY_MS = 24 * 60 * 60 * 1000


def get(path, **params):
    query = urllib.parse.urlencode({k: v for k, v in params.items() if v is not None})
    url = f"{HOST}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(url, headers={"user-agent": "polyoxide-fixture-capture"})
    time.sleep(PAUSE_SECONDS)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return url, response.status, json.loads(response.read())
    except urllib.error.HTTPError as err:
        return url, err.code, json.loads(err.read())


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    captured = []

    def save(name, path, **params):
        url, status, body = get(path, **params)
        if status != 200:
            raise SystemExit(f"{name}: HTTP {status} from {url}: {body}")
        (out / f"{name}.json").write_text(json.dumps(body, indent=2, ensure_ascii=False) + "\n")
        captured.append((name, url))
        return body

    now = int(time.time() * 1000)
    day_ago = now - DAY_MS

    save("ping", "/v1/info/ping")
    save("time", "/v1/info/time")
    save("exchange", "/v1/info/exchange")
    save("assets", "/v1/info/assets")
    instruments = save("instruments", "/v1/info/instruments")
    iid = instruments[0]["instrument_id"]
    save("fees", "/v1/info/fees")
    save("limit_tiers", "/v1/info/limit-tiers")

    save("tickers", "/v1/info/tickers", instrument_id=iid)
    save("statistics", "/v1/info/statistics", instrument_id=iid)
    save("exchange_stats", "/v1/info/exchange-stats", start_timestamp=day_ago, end_timestamp=now)
    save("klines", "/v1/info/klines", instrument_id=iid, interval="1h", start_timestamp=day_ago)
    save("mark_history", "/v1/info/mark-history", instrument_id=iid, interval="1h", start_timestamp=day_ago)
    save("bbo", "/v1/info/bbo", instrument_id=iid)
    save("book", "/v1/info/book", instrument_id=iid, depth=10)
    save("trades", "/v1/info/trades", instrument_id=iid)
    save("funding", "/v1/info/funding", instrument_id=iid)

    # Prefer an index with constituents so IndexConstituent is on the wire.
    chosen = None
    for instrument in instruments:
        url, status, body = get("/v1/info/index", asset=instrument["base_asset"])
        if status == 200 and body.get("constituents"):
            chosen = instrument["base_asset"]
            break
    save("index", "/v1/info/index", asset=chosen or instruments[0]["base_asset"])

    board = save("leaderboard", "/v1/info/leaderboard", window="week", limit=3)
    address = board["entries"][0]["account"]
    save("leaderboard_account", "/v1/info/leaderboard", window="week", limit=1, address=address)
    portfolio = save("portfolio", "/v1/info/portfolio", address=address)
    fill_iid = portfolio["positions"][0]["instrument_id"] if portfolio["positions"] else iid
    save("position_fills", "/v1/info/position-fills", address=address, instrument_id=fill_iid)
    save("invite", "/v1/info/invite", code="polyoxide-fixture")

    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines = [
        "# Perps fixtures",
        "",
        f"Captured {stamp} by `scripts/capture_perps_fixtures.py` from the live host.",
        "Each file is the complete response body, pretty-printed. Inputs were chosen live",
        "(see the script), so the instrument and address here are whatever was active then.",
        "",
        "| Fixture | Request |",
        "|---------|---------|",
    ]
    lines += [f"| `{name}.json` | `{url}` |" for name, url in captured]
    (out / "PROVENANCE.md").write_text("\n".join(lines) + "\n")
    print(f"captured {len(captured)} fixtures to {out}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it**

Run: `python3 scripts/capture_perps_fixtures.py polyoxide-perps/tests/fixtures`
Expected: `captured 23 fixtures to polyoxide-perps/tests/fixtures`. Open `position_fills.json`; if `data` is empty, pick a different leaderboard entry by hand (edit the `entries[0]` index in the script to `entries[1]`, re-run) until a fill row is captured, because the wire test needs `PositionFill` on the wire. Open `index.json` and confirm `constituents` is non-empty.

- [ ] **Step 3: Commit**

```bash
git add scripts/capture_perps_fixtures.py polyoxide-perps/tests/fixtures
git commit -m "test(perps): capture one live fixture per public route"
```

---

### Task 8: Spec agreement test

**Files:**
- Create: `polyoxide-perps/tests/spec_agreement.rs`

The oracle is `docs/specs/perps/openapi.json`. Three checks, adapted from `polyoxide-data/tests/v2_spec_agreement.rs`:

1. **Types.** For every schema reachable from a `/v1/info/*` response: only required fields present must deserialize; each strictly required field missing must fail; each optional field set to `null` must deserialize; a fully populated value must serialize back to the spec's property set plus the wire-only fields listed in `OBSERVED_EXTRA`.
2. **Coverage.** Every reachable object schema is in the table or excused.
3. **Query keys.** Every builder, called with every argument and setter, sends exactly the spec's parameter names, and its `send()` decodes that route's fixture.

- [ ] **Step 1: Write `tests/spec_agreement.rs`**

```rust
//! Agreement between the types and builders and `docs/specs/perps/openapi.json`.
//!
//! Only schemas reachable from the `/v1/info/*` routes are checked; the other
//! 300-odd belong to routes later slices implement. Wire-only fields are
//! allowed through `OBSERVED_EXTRA`, each recorded in
//! `docs/specs/perps/OBSERVED.md`.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use mockito::{Matcher, Server};
use polyoxide_perps::{
    api::{exchange::*, health::*, market::*, public::*},
    types::*,
    Perps, PerpsError,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/perps/openapi.json");

fn spec() -> Value {
    serde_json::from_str(SPEC).expect("spec parses")
}

fn schemas() -> Map<String, Value> {
    spec()["components"]["schemas"]
        .as_object()
        .expect("components.schemas")
        .clone()
}

fn ref_name(r: &str) -> &str {
    r.rsplit('/').next().unwrap()
}

/// Whether a property admits `null`. The perps schema puts `nullable: true`
/// on the `$ref` target (`exchange_open_interest`, `ui_live_time`), not on
/// the property, so the reference is followed.
fn is_nullable(schemas: &Map<String, Value>, prop: &Value) -> bool {
    if let Some(r) = prop["$ref"].as_str() {
        return is_nullable(schemas, &schemas[ref_name(r)]);
    }
    if let Some(types) = prop["type"].as_array() {
        return types.iter().any(|t| t == "null");
    }
    prop["nullable"] == true
        || prop["oneOf"]
            .as_array()
            .is_some_and(|arms| arms.iter().any(|a| a["type"] == "null"))
}

/// A value of the property's type. `full` also fills non-required properties
/// of any object it descends into. An array schema without `items` (the
/// positional `kline` and `mark_point` rows) is taken from its `example`.
fn synth(schemas: &Map<String, Value>, prop: &Value, full: bool) -> Value {
    if let Some(r) = prop["$ref"].as_str() {
        let target = &schemas[ref_name(r)];
        if target["type"] == "object" || target.get("allOf").is_some() || target.get("properties").is_some() {
            return synth_object(schemas, ref_name(r), full);
        }
        return synth(schemas, target, full);
    }
    if let Some(arms) = prop["oneOf"].as_array() {
        let arm = arms.iter().find(|a| a["type"] != "null").expect("non-null arm");
        return synth(schemas, arm, full);
    }
    if let Some(e) = prop["enum"].as_array() {
        return e[0].clone();
    }
    let ty = match &prop["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(ts) => ts.iter().filter_map(Value::as_str).find(|t| *t != "null").unwrap(),
        other => panic!("unsupported type {other} in {prop}"),
    };
    match ty {
        "string" => Value::from("x"),
        "integer" => Value::from(1),
        "number" => Value::from(1.5),
        "boolean" => Value::from(true),
        "array" => match prop.get("items") {
            Some(items) => Value::Array(vec![synth(schemas, items, full)]),
            None => prop["example"].clone(),
        },
        "object" => synth_object_inline(schemas, prop, full),
        other => panic!("unsupported type {other}"),
    }
}

/// An object schema's properties and required names, flattening `allOf`.
fn fields(schemas: &Map<String, Value>, schema: &Value) -> (Map<String, Value>, BTreeSet<String>) {
    if let Some(r) = schema["$ref"].as_str() {
        return fields(schemas, &schemas[ref_name(r)]);
    }
    let own = schema["properties"].as_object();
    let arms = schema["allOf"].as_array();
    assert!(own.is_some() || arms.is_some(), "neither properties nor allOf: {schema}");
    let mut props = own.cloned().unwrap_or_default();
    let mut required: BTreeSet<String> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    for arm in arms.into_iter().flatten() {
        let (arm_props, arm_required) = fields(schemas, arm);
        for (key, prop) in arm_props {
            props.insert(key, prop);
        }
        required.extend(arm_required);
    }
    (props, required)
}

fn synth_object_inline(schemas: &Map<String, Value>, schema: &Value, full: bool) -> Value {
    let (props, required) = fields(schemas, schema);
    let mut out = Map::new();
    for (key, prop) in &props {
        if full || (required.contains(key) && !is_nullable(schemas, prop)) {
            out.insert(key.clone(), synth(schemas, prop, full));
        }
    }
    Value::Object(out)
}

fn synth_object(schemas: &Map<String, Value>, name: &str, full: bool) -> Value {
    synth_object_inline(schemas, &schemas[name], full)
}

/// `(schema, field)` pairs on the wire but not in the spec. Each must be an
/// `Option` on the type and have an entry in OBSERVED.md.
const OBSERVED_EXTRA: &[(&str, &str)] = &[
    ("Instrument", "display_symbol"),
    ("Instrument", "close_only"),
    ("Instrument", "logo"),
    ("TradeData", "settlement"),
    ("LimitTier", "connects_per_minute_limit"),
    ("LimitTier", "max_connections"),
    ("LimitTier", "ws_messages_burst_limit"),
    ("LimitTier", "ws_messages_per_minute_limit"),
];

fn check<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    let (props, required) = fields(schemas, &schemas[name]);

    let minimal = synth_object(schemas, name, false);
    if let Err(e) = serde_json::from_value::<T>(minimal.clone()) {
        panic!("{name}: only required fields present should deserialize: {e}");
    }

    for (key, prop) in &props {
        if required.contains(key) && !is_nullable(schemas, prop) {
            let mut without = minimal.clone();
            without.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<T>(without).is_err(),
                "{name}.{key} is required and non-nullable in the spec but the type accepts it missing"
            );
        } else {
            let mut with_null = minimal.clone();
            with_null.as_object_mut().unwrap().insert(key.clone(), Value::Null);
            if let Err(e) = serde_json::from_value::<T>(with_null) {
                panic!("{name}.{key} is optional or nullable in the spec but the type rejects null: {e}");
            }
        }
    }

    let full = synth_object(schemas, name, true);
    let parsed: T = serde_json::from_value(full)
        .unwrap_or_else(|e| panic!("{name}: every field present should deserialize: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();
    let emitted: BTreeSet<&str> = emitted.as_object().unwrap().keys().map(String::as_str).collect();
    let mut documented: BTreeSet<&str> = props.keys().map(String::as_str).collect();
    documented.extend(
        OBSERVED_EXTRA
            .iter()
            .filter(|(schema, _)| *schema == name)
            .map(|(_, field)| *field),
    );
    assert_eq!(emitted, documented, "{name}: emitted keys differ from the spec's properties");
}

macro_rules! agreement {
    ($($schema:literal => $ty:ty),+ $(,)?) => {
        const MODELLED: &[&str] = &[$($schema),+];
        #[test]
        fn every_modelled_schema_agrees_with_the_spec() {
            let schemas = schemas();
            $( check::<$ty>(&schemas, $schema); )+
        }
    };
}

agreement! {
    "Time" => Time,
    "Exchange" => Exchange,
    "Asset" => Asset,
    "Instrument" => Instrument,
    "RiskTier" => RiskTier,
    "FeesInfo" => FeesInfo,
    "FeeScheduleEntry" => FeeScheduleEntry,
    "FeeTier" => FeeTier,
    "LimitTier" => LimitTier,
    "Ticker" => Ticker,
    "Statistic" => Statistic,
    "ExchangeStatistics" => ExchangeStatistics,
    "KlinesResponse" => Klines,
    "MarkHistoryResponse" => MarkHistory,
    "BBO" => Bbo,
    "Book" => Book,
    "Index" => Index,
    "IndexConstituent" => IndexConstituent,
    "Trades" => Trades,
    "TradeData" => Trade,
    "FundingHistory" => FundingHistory,
    "FundingRate" => FundingRate,
    "PublicPortfolio" => PublicPortfolio,
    "PublicPortfolioPosition" => PublicPortfolioPosition,
    "AccountTrades" => PositionFills,
    "AccountTradeData" => PositionFill,
    "Leaderboard" => Leaderboard,
    "LeaderboardEntry" => LeaderboardEntry,
    "LeaderboardAccount" => LeaderboardAccount,
    "InviteCheckResponse" => InviteCheck,
}

/// Reachable object schemas deliberately not in the table, each with a reason.
const NOT_MODELLED: &[(&str, &str)] = &[
    ("TickerData", "the allOf base of Ticker; no route serves it bare"),
];

/// Every named schema reachable by `$ref` from the `/v1/info/*` responses and
/// parameters.
fn reachable_from_info_routes() -> BTreeSet<String> {
    fn walk(schemas: &Map<String, Value>, node: &Value, seen: &mut BTreeSet<String>) {
        match node {
            Value::Object(map) => {
                if let Some(r) = map.get("$ref").and_then(Value::as_str) {
                    let name = ref_name(r).to_owned();
                    if seen.insert(name.clone()) {
                        walk(schemas, &schemas[&name], seen);
                    }
                }
                for v in map.values() {
                    walk(schemas, v, seen);
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(schemas, v, seen)),
            _ => {}
        }
    }
    let spec = spec();
    let schemas = schemas();
    let mut seen = BTreeSet::new();
    for (path, ops) in spec["paths"].as_object().unwrap() {
        if path.starts_with("/v1/info/") {
            walk(&schemas, &ops["get"]["responses"]["200"], &mut seen);
            walk(&schemas, &ops["get"]["parameters"], &mut seen);
        }
    }
    seen
}

#[test]
fn every_reachable_object_schema_is_modelled_or_excused() {
    let schemas = schemas();
    let excused: BTreeSet<&str> = NOT_MODELLED.iter().map(|(n, _)| *n).collect();
    let modelled: BTreeSet<&str> = MODELLED.iter().copied().collect();
    let unaccounted: Vec<String> = reachable_from_info_routes()
        .into_iter()
        .filter(|n| {
            let s = &schemas[n];
            s["type"] == "object" || s.get("allOf").is_some() || s.get("properties").is_some()
        })
        .filter(|n| !modelled.contains(n.as_str()) && !excused.contains(n.as_str()))
        .collect();
    assert!(unaccounted.is_empty(), "schemas neither modelled nor excused: {unaccounted:?}");
}

#[test]
fn every_enum_matches_the_spec() {
    let schemas = schemas();
    fn wire<T: Serialize>(all: &[T]) -> BTreeSet<String> {
        all.iter()
            .map(|v| serde_json::to_value(v).unwrap().as_str().unwrap().to_owned())
            .collect()
    }
    fn documented(schemas: &Map<String, Value>, name: &str) -> BTreeSet<String> {
        schemas[name]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} has no enum"))
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    }
    assert_eq!(wire(Interval::ALL), documented(&schemas, "interval"));
    assert_eq!(wire(Side::ALL), documented(&schemas, "side"));
    assert_eq!(wire(InstrumentType::ALL), documented(&schemas, "instrument_type"));
    assert_eq!(wire(InstrumentCategory::ALL), documented(&schemas, "category"));
    assert_eq!(wire(LeaderboardWindow::ALL), documented(&schemas, "window"));
    assert_eq!(wire(LeaderboardSort::ALL), documented(&schemas, "sort_by"));
    assert_eq!(wire(SortOrder::ALL), documented(&schemas, "sort"));

    let depths: BTreeSet<u64> = schemas["depth"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(
        BookDepth::ALL.iter().map(|d| u64::from(d.levels())).collect::<BTreeSet<_>>(),
        depths
    );
}

// ── Query parameters ────────────────────────────────────────────────

type Fire = fn(Perps) -> Pin<Box<dyn Future<Output = Result<(), PerpsError>> + Send>>;

/// Sends one request through `fire`, requires its response to decode, and
/// returns the query keys it carried.
async fn query_keys_sent(path: &str, fixture: &str, fire: Fire) -> BTreeSet<String> {
    let body_path = format!("{}/tests/fixtures/{fixture}.json", env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(&body_path).unwrap_or_else(|e| panic!("{body_path}: {e}"));
    let mut server = Server::new_async().await;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&seen);
    let mock = server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock().unwrap().push(request.path_and_query().to_owned());
            true
        })
        .with_status(200)
        .with_body(body)
        .create_async()
        .await;

    let decoded = fire(Perps::builder().base_url(server.url()).build().unwrap()).await;
    mock.assert_async().await;
    if let Err(e) = decoded {
        panic!("{path}: the builder did not decode `{fixture}.json`: {e}");
    }

    let seen = seen.lock().unwrap();
    let url = url::Url::parse(&format!("http://mock{}", seen.last().unwrap())).unwrap();
    url.query_pairs().map(|(key, _)| key.into_owned()).collect()
}

/// One entry per builder: its path, the fixture it must decode, and a call
/// using every argument and setter it has.
const ROUTES: &[(&str, &str, Fire)] = &[
    ("/v1/info/time", "time", |p| Box::pin(async move { p.health().time().send().await.map(|_| ()) })),
    ("/v1/info/exchange", "exchange", |p| Box::pin(async move { p.exchange().exchange().send().await.map(|_| ()) })),
    ("/v1/info/assets", "assets", |p| Box::pin(async move { p.exchange().assets().send().await.map(|_| ()) })),
    ("/v1/info/instruments", "instruments", |p| {
        Box::pin(async move {
            p.exchange()
                .instruments()
                .instrument_id(InstrumentId(1))
                .instrument_type(InstrumentType::Perpetual)
                .category(InstrumentCategory::Index)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/fees", "fees", |p| Box::pin(async move { p.exchange().fees().send().await.map(|_| ()) })),
    ("/v1/info/limit-tiers", "limit_tiers", |p| Box::pin(async move { p.exchange().limit_tiers().send().await.map(|_| ()) })),
    ("/v1/info/tickers", "tickers", |p| {
        Box::pin(async move { p.market().tickers().instrument_id(InstrumentId(1)).send().await.map(|_| ()) })
    }),
    ("/v1/info/statistics", "statistics", |p| {
        Box::pin(async move { p.market().statistics().instrument_id(InstrumentId(1)).send().await.map(|_| ()) })
    }),
    ("/v1/info/exchange-stats", "exchange_stats", |p| {
        Box::pin(async move { p.market().exchange_stats(1, 2).send().await.map(|_| ()) })
    }),
    ("/v1/info/klines", "klines", |p| {
        Box::pin(async move { p.market().klines(InstrumentId(1), Interval::H1, 1).end(2).send().await.map(|_| ()) })
    }),
    ("/v1/info/mark-history", "mark_history", |p| {
        Box::pin(async move { p.market().mark_history(InstrumentId(1), Interval::H1, 1).end(2).send().await.map(|_| ()) })
    }),
    ("/v1/info/bbo", "bbo", |p| {
        Box::pin(async move { p.market().bbo().instrument_id(InstrumentId(1)).send().await.map(|_| ()) })
    }),
    ("/v1/info/book", "book", |p| {
        Box::pin(async move { p.market().book(InstrumentId(1)).depth(BookDepth::Ten).send().await.map(|_| ()) })
    }),
    ("/v1/info/index", "index", |p| Box::pin(async move { p.market().index("BTC").send().await.map(|_| ()) })),
    ("/v1/info/trades", "trades", |p| {
        Box::pin(async move { p.market().trades(InstrumentId(1)).start(1).end(2).send().await.map(|_| ()) })
    }),
    ("/v1/info/funding", "funding", |p| {
        Box::pin(async move { p.market().funding(InstrumentId(1)).start(1).end(2).send().await.map(|_| ()) })
    }),
    ("/v1/info/portfolio", "portfolio", |p| {
        Box::pin(async move { p.public().portfolio("0xabc").send().await.map(|_| ()) })
    }),
    ("/v1/info/position-fills", "position_fills", |p| {
        Box::pin(async move {
            p.public()
                .position_fills("0xabc", InstrumentId(1))
                .cursor("c")
                .sort(SortOrder::Asc)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/leaderboard", "leaderboard_account", |p| {
        Box::pin(async move {
            p.public()
                .leaderboard()
                .window(LeaderboardWindow::Week)
                .sort_by(LeaderboardSort::Pnl)
                .limit(1)
                .offset(0)
                .address("0xabc")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/invite", "invite", |p| {
        Box::pin(async move { p.public().invite("code").address("0xabc").send().await.map(|_| ()) })
    }),
];

#[tokio::test]
async fn every_builder_sends_exactly_the_documented_query_keys() {
    let spec = spec();
    let mut by_path: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (path, fixture, fire) in ROUTES {
        by_path.entry(path).or_default().extend(query_keys_sent(path, fixture, *fire).await);
    }
    for (path, sent) in by_path {
        let documented: BTreeSet<String> = spec["paths"][path]["get"]["parameters"]
            .as_array()
            .map(|ps| ps.iter().map(|p| p["name"].as_str().unwrap().to_owned()).collect())
            .unwrap_or_default();
        assert_eq!(sent, documented, "{path}: query keys sent differ from the spec's parameters");
    }
}

#[test]
fn every_info_route_has_a_builder_entry() {
    let spec = spec();
    let routes: BTreeSet<&str> = ROUTES.iter().map(|(p, _, _)| *p).collect();
    let missing: Vec<&str> = spec["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|p| p.starts_with("/v1/info/") && *p != "/v1/info/ping")
        .filter(|p| !routes.contains(p))
        .collect();
    assert!(missing.is_empty(), "info routes with no builder entry: {missing:?}");
}
```

`/v1/info/ping` is excluded from `ROUTES` because `ping()` returns a `Duration`, not a decoded body; the mock test in Task 3 covers it. Add `url = { workspace = true }` to `[dev-dependencies]` if the test cannot see `url` (it is a normal dependency already, so it resolves).

- [ ] **Step 2: Run it**

Run: `cargo test -p polyoxide-perps --test spec_agreement`
Expected: 5 tests pass. If `every_modelled_schema_agrees_with_the_spec` fails on a field, the type is wrong, not the test: fix the type. If `every_reachable_object_schema_is_modelled_or_excused` names a schema, add it to the table or excuse it with a reason.

- [ ] **Step 3: Prove the test bites**

Temporarily change `pub ui_live_time: Option<u64>` on `Instrument` to `pub ui_live_time: u64` and run the test: expected failure `Instrument.ui_live_time is optional or nullable in the spec but the type rejects null`. Revert. (`ui_live_time`, `ExchangeStatistics.open_interest` and `ExchangeStatistics.open_interest_timestamp` are `nullable` on their `$ref` targets, which is why the types carry `Option` and why `is_nullable` follows references.)

- [ ] **Step 4: Commit**

```bash
cargo fmt --all
git add polyoxide-perps/tests/spec_agreement.rs
git commit -m "test(perps): agreement with the published OpenAPI schema"
```

---

### Task 9: Wire agreement test

**Files:**
- Create: `polyoxide-perps/tests/wire_agreement.rs`

- [ ] **Step 1: Write `tests/wire_agreement.rs`**

```rust
//! Agreement between the types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/PROVENANCE.md`.
//!
//! Each fixture is deserialized into its type and serialized back, and the two
//! key-path sets are compared:
//!
//! 1. **Nothing unmodelled.** Every path the server sent is emitted by the type,
//!    unless listed in `IGNORED` with a reason.
//! 2. **Nothing invented.** Every path the type emits was sent by the server,
//!    unless listed in `EXPECTED_ABSENT` with a reason. Every `Option`
//!    serializes as `null`, so a field modelled from the spec but absent from
//!    the wire shows up here instead of hiding.
//!
//! An entry in either list that no fixture needs fails the test too.

use std::collections::BTreeSet;

use polyoxide_perps::api::{exchange::*, health::*, market::*, public::*};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately do not model.
const IGNORED: &[(&str, &str, &str)] = &[];

/// `(fixture, path, reason)` a type emits that this capture did not contain.
const EXPECTED_ABSENT: &[(&str, &str, &str)] = &[
    ("leaderboard", "/account", "sent only when the request names an address"),
    ("position_fills", "/cursor", "sent only when another page exists"),
    ("invite", "/error", "sent only when the code is invalid; an unknown code was captured, so drop this row if the capture says valid=false"),
];

fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let p = format!("{prefix}/{k}");
                out.insert(p.clone());
                key_paths(v, &p, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                key_paths(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

fn check<T: DeserializeOwned + Serialize>(fixture: &str, used: &mut BTreeSet<(&'static str, &'static str)>) {
    let path = format!("{}/tests/fixtures/{fixture}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let wire: Value = serde_json::from_str(&text).unwrap();
    let parsed: T = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{fixture}: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();

    let mut sent = BTreeSet::new();
    key_paths(&wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(&emitted, "", &mut modelled);

    for p in sent.difference(&modelled) {
        match IGNORED.iter().find(|(f, ip, _)| *f == fixture && *ip == p) {
            Some((f, ip, _)) => {
                used.insert((f, ip));
            }
            None => panic!("{fixture}: server sent {p}, which the type does not model"),
        }
    }
    for p in modelled.difference(&sent) {
        match EXPECTED_ABSENT.iter().find(|(f, ap, _)| *f == fixture && *ap == p) {
            Some((f, ap, _)) => {
                used.insert((f, ap));
            }
            None => panic!("{fixture}: the type emits {p}, which the server did not send"),
        }
    }
}

#[test]
fn every_fixture_agrees_with_its_type() {
    let mut used = BTreeSet::new();
    check::<Time>("time", &mut used);
    check::<Exchange>("exchange", &mut used);
    check::<Vec<Asset>>("assets", &mut used);
    check::<Vec<Instrument>>("instruments", &mut used);
    check::<FeesInfo>("fees", &mut used);
    check::<Vec<LimitTier>>("limit_tiers", &mut used);
    check::<Vec<Ticker>>("tickers", &mut used);
    check::<Vec<Statistic>>("statistics", &mut used);
    check::<ExchangeStatistics>("exchange_stats", &mut used);
    check::<Klines>("klines", &mut used);
    check::<MarkHistory>("mark_history", &mut used);
    check::<Vec<Bbo>>("bbo", &mut used);
    check::<Book>("book", &mut used);
    check::<Index>("index", &mut used);
    check::<Trades>("trades", &mut used);
    check::<FundingHistory>("funding", &mut used);
    check::<PublicPortfolio>("portfolio", &mut used);
    check::<PositionFills>("position_fills", &mut used);
    check::<Leaderboard>("leaderboard", &mut used);
    check::<Leaderboard>("leaderboard_account", &mut used);
    check::<InviteCheck>("invite", &mut used);

    let listed: BTreeSet<(&str, &str)> = IGNORED
        .iter()
        .chain(EXPECTED_ABSENT.iter())
        .map(|(f, p, _)| (*f, *p))
        .collect();
    let stale: Vec<_> = listed.difference(&used).collect();
    assert!(stale.is_empty(), "allowance entries no fixture needs: {stale:?}");
}
```

- [ ] **Step 2: Run it and reconcile**

Run: `cargo test -p polyoxide-perps --test wire_agreement`

Expected on first run: possibly failures naming paths. Reconcile each one the same way:
- A path the server sent that no type models: add the field as `Option<T>`, add it to `OBSERVED_EXTRA` in `spec_agreement.rs`, and record it in Task 10's `OBSERVED.md`.
- A path the type emits that the server did not send: if it is documented as required, the capture is the odd one; add an `EXPECTED_ABSENT` row with the reason. If it is an `Option` the capture simply lacks, same.
- The `invite` row: the captured code is invalid, so `error` is on the wire and the `EXPECTED_ABSENT` row for it will be reported stale. Delete that row. It is listed above only so the reconciliation is explicit.

Re-run until green.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-perps
git commit -m "test(perps): agreement with captured wire payloads"
```

---

### Task 10: `docs/specs/perps/OBSERVED.md`

**Files:**
- Create: `docs/specs/perps/OBSERVED.md`
- Modify: `docs/specs/perps/INDEX.md` (add a link line under the machine-readable schema line)

- [ ] **Step 1: Write `OBSERVED.md`**

Every entry below was observed on 2026-09-30 against the live host during brainstorming; refresh the dates from the fixture capture in Task 7 and add anything Task 9's reconciliation surfaced.

```markdown
# Perps API — observed behaviour

Where the live host disagrees with, or goes beyond, [openapi.json](openapi.json).
The mirror stays byte-faithful so the nightly drift check can compare it with
upstream; what the server actually does is recorded here instead.

Each entry names its evidence. Re-check before relying on an old one.

## Fields on the wire that the schema omits

Captured 2026-09-30 (`polyoxide-perps/tests/fixtures/`). Each is modelled as an
`Option` and allowed through `OBSERVED_EXTRA` in `tests/spec_agreement.rs`.

| Schema | Field | Seen as | Note |
|--------|-------|---------|------|
| `Instrument` | `display_symbol` | `"USA500-USD"` | on some rows only |
| `Instrument` | `close_only` | `false` | on every row |
| `Instrument` | `logo` | URL | on every row |
| `TradeData` | `settlement` | `false` | on every row |
| `LimitTier` | `connects_per_minute_limit` | integer | WebSocket budget |
| `LimitTier` | `max_connections` | integer | WebSocket budget |
| `LimitTier` | `ws_messages_burst_limit` | integer | WebSocket budget |
| `LimitTier` | `ws_messages_per_minute_limit` | integer | WebSocket budget |

The four `LimitTier` extras are the only published figures for the WebSocket
inbound budget the AsyncAPI calls "weighted"; plan 2 reads them.

## Error bodies carry more than `{status, error}`

A 400 (`GET /v1/info/book` with no `instrument_id`, 2026-09-30):

```json
{"status":"err","error":"invalid query parameters: missing field `instrument_id`","arts":1790758475821,"ts":1790758475821,"ref":"g-1224ed1744735"}
```

`ref` is a gateway trace id; `VenueError::reference` carries it. A 404
(`GET /v1/info/nope`) is the bare `{"status":"err","error":"not_found"}`. On a
400 `error` is a human-readable message, as the spec says, not an identifier.

## An unknown instrument is an empty book, not a 404

`GET /v1/info/book?instrument_id=999999` answers 200 with
`{"instrument_id":999999,"bids":[],"asks":[],…}` (2026-09-30). Callers cannot
tell "no liquidity" from "no such instrument" on this route; use
`/v1/info/instruments` for existence.

## Responses are served through CloudFront

Every response carries `x-cache: Hit from cloudfront` or `Miss from cloudfront`
and `cache-control: public, max-age=0` (`instruments`) or `max-age=1` (`book`).
A repeated URL can be answered without reaching the origin, so a rate-limit
soak must vary the URL (`examples/info_soak.rs`), and `/v1/info/instruments`
cannot be soaked at all: it has only 88 × 5 distinct parameterisations.

## Long decimals and `Decimal` precision

`GET /v1/info/portfolio` sent `"unrealized_pnl":"10642.357770000000000000000018"`
(24 fractional places) and `GET /v1/info/exchange-stats` sent
`"open_interest":"75573217.100902647081712288"` (18), both 2026-09-30.
`rust_decimal::Decimal` holds up to 28 fractional places on a 96-bit mantissa,
so both fit exactly; a longer value would be rounded on decode, since the
crate's string serde uses `Decimal::from_str`, not `from_str_exact`. No capture
has exceeded the limit yet.

## Leaderboard `account` is request-dependent

`account` appears only when the request names an `address`; otherwise the key
is absent. Modelled as `Option<LeaderboardAccount>`.

## Rate limits

Nothing numeric is published for the public routes. See the soak runs recorded
below by Task 12 of `docs/superpowers/plans/2026-09-30-perps-http.md`.
```

- [ ] **Step 2: Link it from `docs/specs/perps/INDEX.md`**

After the line beginning `Machine-readable schema:` add:

```markdown
Observed behaviour the schema does not describe: [OBSERVED.md](OBSERVED.md).
```

- [ ] **Step 3: Commit**

```bash
git add docs/specs/perps/OBSERVED.md docs/specs/perps/INDEX.md
git commit -m "docs(perps): OBSERVED.md for wire behaviour the schema omits"
```

---

### Task 11: Live tests

**Files:**
- Create: `polyoxide-perps/tests/live_api.rs`

- [ ] **Step 1: Write `tests/live_api.rs`**

```rust
//! Live integration tests against the Polymarket Perps API.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-perps --test live_api -- --ignored
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use polyoxide_perps::{
    api::exchange::Instrument,
    types::{BookDepth, InstrumentId, Interval, LeaderboardWindow},
    Perps,
};

fn client() -> Perps {
    Perps::new().expect("perps client")
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

/// An instrument that is currently quoting, so book and bbo assertions have
/// something to look at. Selects on the precondition it asserts on.
async fn a_quoting_instrument(perps: &Perps) -> Instrument {
    let instruments = perps.exchange().instruments().send().await.expect("instruments");
    for instrument in instruments {
        let book = perps.market().book(instrument.instrument_id).depth(BookDepth::Ten).send().await;
        if let Ok(book) = book {
            if !book.bids.is_empty() && !book.asks.is_empty() {
                return instrument;
            }
        }
    }
    panic!("no suitable market: no instrument has a two-sided book right now");
}

#[tokio::test]
#[ignore]
async fn live_ping_and_time() {
    let perps = client();
    let latency = perps.health().ping().await.expect("ping");
    assert!(latency < Duration::from_secs(10), "latency too high: {latency:?}");
    let time = perps.health().time().send().await.expect("time");
    let skew = time.time.abs_diff(now_ms());
    assert!(skew < 60_000, "server clock differs from ours by {skew} ms");
}

#[tokio::test]
#[ignore]
async fn live_reference_data() {
    let perps = client();
    let exchange = perps.exchange().exchange().send().await.expect("exchange");
    assert_eq!(exchange.chain_id, 137);
    let assets = perps.exchange().assets().send().await.expect("assets");
    assert!(assets.iter().any(|a| a.asset == "pUSD"));
    let fees = perps.exchange().fees().send().await.expect("fees");
    assert!(!fees.fee_schedule.is_empty());
    let tiers = perps.exchange().limit_tiers().send().await.expect("limit tiers");
    assert!(!tiers.is_empty());
}

#[tokio::test]
#[ignore]
async fn live_market_data_for_a_quoting_instrument() {
    let perps = client();
    let instrument = a_quoting_instrument(&perps).await;
    let iid = instrument.instrument_id;

    let tickers = perps.market().tickers().instrument_id(iid).send().await.expect("tickers");
    assert_eq!(tickers.len(), 1);
    assert_eq!(tickers[0].instrument_id, iid);

    let bbo = perps.market().bbo().instrument_id(iid).send().await.expect("bbo");
    assert!(bbo[0].bid_price < bbo[0].ask_price);

    let start = now_ms() - 6 * 60 * 60 * 1000;
    let klines = perps.market().klines(iid, Interval::H1, start).send().await.expect("klines");
    assert!(!klines.data.is_empty());

    let marks = perps.market().mark_history(iid, Interval::H1, start).send().await.expect("mark history");
    assert!(!marks.data.is_empty());

    let trades = perps.market().trades(iid).send().await.expect("trades");
    assert!(trades.data.iter().all(|t| t.instrument_id == iid));

    let funding = perps.market().funding(iid).send().await.expect("funding");
    assert!(!funding.data.is_empty());

    let index = perps.market().index(&instrument.base_asset).send().await.expect("index");
    assert_eq!(index.asset, instrument.base_asset);

    let stats = perps.market().exchange_stats(start, now_ms()).send().await.expect("exchange stats");
    assert!(stats.end_timestamp >= stats.start_timestamp);
}

#[tokio::test]
#[ignore]
async fn live_public_lookups() {
    let perps = client();
    let board = perps
        .public()
        .leaderboard()
        .window(LeaderboardWindow::Week)
        .limit(3)
        .send()
        .await
        .expect("leaderboard");
    assert!(!board.entries.is_empty());
    let address = &board.entries[0].account;

    let portfolio = perps.public().portfolio(address).send().await.expect("portfolio");
    let iid = portfolio
        .positions
        .first()
        .map(|p| p.instrument_id)
        .unwrap_or(InstrumentId(1));
    let fills = perps.public().position_fills(address, iid).send().await.expect("position fills");
    assert!(fills.data.iter().all(|f| f.instrument_id == iid));

    let invite = perps.public().invite("polyoxide-live-test").send().await.expect("invite");
    assert!(!invite.valid);
}
```

`a_quoting_instrument` panics with the `no suitable market` phrasing that `.github/scripts/classify_failures.py` classifies as environmental.

- [ ] **Step 2: Run them**

Run: `cargo test -p polyoxide-perps --test live_api -- --ignored`
Expected: 4 passed. A venue error on a live route is not provoked here: no builder can send a malformed request, and `Url::join` with an absolute path discards any prefix on the base URL, so the `Venue` mapping is covered by the mock tests in Task 3 only.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-perps/tests/live_api.rs
git commit -m "test(perps): live tests for every public route"
```

---

### Task 12: The rate-limit soak harness

**Files:**
- Create: `polyoxide-perps/examples/info_soak.rs`
- Modify: `polyoxide-perps/Cargo.toml`

Why these routes: a soak has to send a distinct URL per request, because CloudFront answers a repeated one without reaching the origin (`OBSERVED.md`). `klines` and `trades` vary `start_timestamp`, so they have unbounded URL space. `portfolio` varies `address` over leaderboard entries (thousands). `bbo` has only 88 distinct URLs (one per instrument) and is kept as the small-pool case: the harness counts cache hits and reports what fraction of the stage actually reached the origin. `instruments` has 88 × 5 parameterisations and is not soaked.

Why raw requests: a ramp drives rates above what the shipped client would allow, so it sends with `reqwest` directly. Validation (`--pace client`) goes through `Perps` with the pinned limiter and detects a retried-away 429 through `tracing`, since the retry loop returns `Ok` after a 429 it survives.

- [ ] **Step 1: Register the example so its unit tests run in CI**

Append to `polyoxide-perps/Cargo.toml`:

```toml
# `cargo test` builds examples but does not run their unit tests unless
# asked; the verdict rules that pin `RateLimiter::perps_default` live here.
[[example]]
name = "info_soak"
path = "examples/info_soak.rs"
test = true
```

- [ ] **Step 2: Write `examples/info_soak.rs`**

```rust
//! Measures, then validates, the rate limit for public Perps routes.
//!
//! Upstream publishes no figure for `/v1/info/*`. This harness ramps a fixed
//! rate against one route, stepping up until the host pushes back, and
//! prints the count to pin (the highest clean rate, per 10 seconds).
//!
//! ```sh
//! # Ramp one route.
//! cargo run --release -p polyoxide-perps --example info_soak -- --route klines
//!
//! # Validate: pace every route by the shipped limiter and require zero 429s.
//! cargo run --release -p polyoxide-perps --example info_soak -- --route all --pace client
//! ```
//!
//! Every URL in a ramp is distinct, because CloudFront fronts the host and a
//! repeated URL is answered from cache (`docs/specs/perps/OBSERVED.md`). A
//! stage whose origin-served share falls below `MIN_ORIGIN_SHARE` is reported
//! as saturated rather than clean.

use std::{
    process::ExitCode,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use polyoxide_perps::{types::InstrumentId, Perps};
use tracing::field::{Field, Visit};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

const DEFAULT_BASE_URL: &str = "https://api.perpetuals.polymarket.com";
const DEFAULT_STAGES: [f64; 5] = [5.0, 10.0, 15.0, 20.0, 30.0];
const CEILING_RPS: f64 = 40.0;
/// Below this share of origin-served replies a stage measured the CDN, not
/// the host.
const MIN_ORIGIN_SHARE: f64 = 0.9;

// ── Routes ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Klines,
    Trades,
    Portfolio,
    Bbo,
}

impl Route {
    const ALL: [Route; 4] = [Route::Klines, Route::Trades, Route::Portfolio, Route::Bbo];

    fn name(self) -> &'static str {
        match self {
            Route::Klines => "klines",
            Route::Trades => "trades",
            Route::Portfolio => "portfolio",
            Route::Bbo => "bbo",
        }
    }

    fn path(self) -> &'static str {
        match self {
            Route::Klines => "/v1/info/klines",
            Route::Trades => "/v1/info/trades",
            Route::Portfolio => "/v1/info/portfolio",
            Route::Bbo => "/v1/info/bbo",
        }
    }
}

fn parse_routes(raw: &str) -> Result<Vec<Route>, String> {
    if raw == "all" {
        return Ok(Route::ALL.to_vec());
    }
    raw.split(',')
        .map(|s| match s.trim() {
            "klines" => Ok(Route::Klines),
            "trades" => Ok(Route::Trades),
            "portfolio" => Ok(Route::Portfolio),
            "bbo" => Ok(Route::Bbo),
            other => Err(format!("unknown route {other:?}")),
        })
        .collect()
}

/// Distinct URLs for a route, drawn from live inputs.
struct Probes {
    instruments: Vec<InstrumentId>,
    addresses: Vec<String>,
    counter: AtomicU64,
}

impl Probes {
    fn next(&self, route: Route, base_url: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        let iid = self.instruments[(n as usize) % self.instruments.len()];
        match route {
            // Each request asks for a different window, so the URL never repeats.
            Route::Klines => format!(
                "{base_url}/v1/info/klines?instrument_id={iid}&interval=1m&start_timestamp={}",
                1_700_000_000_000u64 + n * 60_000
            ),
            Route::Trades => format!(
                "{base_url}/v1/info/trades?instrument_id={iid}&start_timestamp={}",
                1_700_000_000_000u64 + n * 1_000
            ),
            Route::Portfolio => format!(
                "{base_url}/v1/info/portfolio?address={}",
                self.addresses[(n as usize) % self.addresses.len()]
            ),
            Route::Bbo => format!("{base_url}/v1/info/bbo?instrument_id={iid}"),
        }
    }
}

async fn load_probes(perps: &Perps, addresses: usize) -> Probes {
    let instruments = perps
        .exchange()
        .instruments()
        .send()
        .await
        .expect("instruments")
        .into_iter()
        .map(|i| i.instrument_id)
        .collect::<Vec<_>>();
    let mut found = Vec::new();
    let mut offset = 0u64;
    while found.len() < addresses {
        let page = perps
            .public()
            .leaderboard()
            .window(polyoxide_perps::types::LeaderboardWindow::Month)
            .limit(100)
            .offset(offset)
            .send()
            .await
            .expect("leaderboard");
        if page.entries.is_empty() {
            break;
        }
        offset += page.entries.len() as u64;
        found.extend(page.entries.into_iter().map(|e| e.account));
    }
    assert!(!instruments.is_empty() && !found.is_empty(), "no probes");
    Probes {
        instruments,
        addresses: found,
        counter: AtomicU64::new(0),
    }
}

// ── Verdicts (pure, unit-tested) ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum Reply {
    Ok,
    CacheHit,
    Throttled { code: String, retry_after: Option<u64> },
    Error(u16),
}

fn classify(status: u16, x_cache: Option<&str>, retry_after: Option<&str>, body: &str) -> Reply {
    if status == 429 {
        let code = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("error").and_then(|c| c.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        return Reply::Throttled {
            code,
            retry_after: retry_after.and_then(|v| v.trim().parse().ok()),
        };
    }
    if x_cache.is_some_and(|v| v.to_ascii_lowercase().starts_with("hit")) {
        return Reply::CacheHit;
    }
    if (200..300).contains(&status) {
        Reply::Ok
    } else {
        Reply::Error(status)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Verdict {
    Clean,
    Throttled { after: Duration, code: String },
    Saturated { origin_share: f64 },
    Invalid { errors: usize },
}

fn judge(replies: &[(Duration, Reply)]) -> Verdict {
    if let Some((at, Reply::Throttled { code, .. })) =
        replies.iter().find(|(_, r)| matches!(r, Reply::Throttled { .. }))
    {
        return Verdict::Throttled {
            after: *at,
            code: code.clone(),
        };
    }
    let errors = replies.iter().filter(|(_, r)| matches!(r, Reply::Error(_))).count();
    if errors > 0 {
        return Verdict::Invalid { errors };
    }
    let origin = replies.iter().filter(|(_, r)| *r == Reply::Ok).count();
    let share = if replies.is_empty() {
        0.0
    } else {
        origin as f64 / replies.len() as f64
    };
    if share < MIN_ORIGIN_SHARE {
        return Verdict::Saturated { origin_share: share };
    }
    Verdict::Clean
}

/// The count to pin for a 10-second window: the highest clean stage rate.
fn pin(stages: &[(f64, Verdict)]) -> Option<u32> {
    stages
        .iter()
        .take_while(|(_, v)| *v == Verdict::Clean)
        .last()
        .map(|(rate, _)| (rate * 10.0).floor() as u32)
}

// ── Pacing ──────────────────────────────────────────────────────

struct Pacer {
    interval: Duration,
    next: Mutex<Option<Instant>>,
}

impl Pacer {
    fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    /// The next slot, never in the past: an idle pacer must not bank credit
    /// and release it as a burst.
    fn reserve(&self, now: Instant) -> Instant {
        let mut next = self.next.lock().unwrap();
        let slot = next.map_or(now, |claimed| claimed.max(now));
        *next = Some(slot + self.interval);
        slot
    }

    async fn wait(&self) {
        let slot = self.reserve(Instant::now());
        tokio::time::sleep_until(tokio::time::Instant::from_std(slot)).await;
    }
}

// ── Throttle observer for validation mode ───────────────────────

#[derive(Default)]
struct MessageVisitor(Option<String>);

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = Some(format!("{value:?}"));
        }
    }
}

struct ThrottleLayer(Arc<AtomicU64>);

impl<S: tracing::Subscriber> Layer<S> for ThrottleLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let meta = event.metadata();
        if !meta.target().starts_with("polyoxide_core") || *meta.level() != tracing::Level::WARN {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        if visitor.0.is_some_and(|m| m.contains("Retriable status 429")) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// ── Drivers ─────────────────────────────────────────────────────

async fn ramp_stage(
    client: &reqwest::Client,
    probes: &Probes,
    route: Route,
    base_url: &str,
    rate: f64,
    secs: u64,
    concurrency: usize,
) -> Vec<(Duration, Reply)> {
    let pacer = Arc::new(Pacer::new(Duration::from_secs_f64(1.0 / rate)));
    let replies = Arc::new(Mutex::new(Vec::new()));
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    let mut workers = Vec::new();
    for _ in 0..concurrency {
        let client = client.clone();
        let pacer = Arc::clone(&pacer);
        let replies = Arc::clone(&replies);
        let urls: Vec<String> = (0..(rate * secs as f64 / concurrency as f64).ceil() as u64 + 1)
            .map(|_| probes.next(route, base_url))
            .collect();
        workers.push(tokio::spawn(async move {
            for url in urls {
                if Instant::now() >= deadline {
                    break;
                }
                pacer.wait().await;
                let response = client.get(&url).send().await;
                let reply = match response {
                    Ok(r) => {
                        let status = r.status().as_u16();
                        let x_cache = r.headers().get("x-cache").and_then(|v| v.to_str().ok()).map(str::to_owned);
                        let retry_after = r.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(str::to_owned);
                        let body = r.text().await.unwrap_or_default();
                        classify(status, x_cache.as_deref(), retry_after.as_deref(), &body)
                    }
                    Err(_) => Reply::Error(0),
                };
                let stop = matches!(reply, Reply::Throttled { .. });
                replies.lock().unwrap().push((start.elapsed(), reply));
                if stop {
                    break;
                }
            }
        }));
    }
    for w in workers {
        let _ = w.await;
    }
    let mut out = replies.lock().unwrap().clone();
    out.sort_by_key(|(at, _)| *at);
    out
}

async fn run_ramp(cfg: &Config, route: Route) -> ExitCode {
    let perps = Perps::builder().base_url(&cfg.base_url).build().unwrap();
    let probes = load_probes(&perps, cfg.addresses).await;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let mut stages = Vec::new();
    for &rate in &cfg.stages {
        eprintln!("== {} at {rate:.1} req/s for {}s", route.name(), cfg.stage_secs);
        let replies = ramp_stage(&client, &probes, route, &cfg.base_url, rate, cfg.stage_secs, cfg.concurrency).await;
        let verdict = judge(&replies);
        eprintln!("   {} replies, verdict {verdict:?}", replies.len());
        let stop = verdict != Verdict::Clean;
        stages.push((rate, verdict));
        if stop {
            break;
        }
        eprintln!("   cooling down {}s", cfg.cooldown_secs);
        tokio::time::sleep(Duration::from_secs(cfg.cooldown_secs)).await;
    }
    match pin(&stages) {
        Some(count) => {
            println!("{}: pin {count} per 10s ({stages:?})", route.name());
            ExitCode::SUCCESS
        }
        None => {
            println!("{}: even the first stage was not clean ({stages:?})", route.name());
            ExitCode::from(1)
        }
    }
}

async fn run_validation(cfg: &Config) -> ExitCode {
    let throttles = Arc::new(AtomicU64::new(0));
    tracing_subscriber::registry()
        .with(ThrottleLayer(Arc::clone(&throttles)))
        .init();
    let perps = Perps::builder().base_url(&cfg.base_url).build().unwrap();
    let probes = Arc::new(load_probes(&perps, cfg.addresses).await);
    let deadline = Instant::now() + Duration::from_secs(cfg.stage_secs);
    let sent = Arc::new(AtomicU64::new(0));
    let mut workers = Vec::new();
    for route in &cfg.routes {
        for _ in 0..cfg.concurrency {
            let perps = perps.clone();
            let probes = Arc::clone(&probes);
            let sent = Arc::clone(&sent);
            let route = *route;
            workers.push(tokio::spawn(async move {
                while Instant::now() < deadline {
                    let n = probes.counter.fetch_add(1, Ordering::Relaxed);
                    let iid = probes.instruments[(n as usize) % probes.instruments.len()];
                    let result = match route {
                        Route::Klines => perps
                            .market()
                            .klines(iid, polyoxide_perps::types::Interval::M1, 1_700_000_000_000 + n * 60_000)
                            .send()
                            .await
                            .map(|_| ()),
                        Route::Trades => perps
                            .market()
                            .trades(iid)
                            .start(1_700_000_000_000 + n * 1_000)
                            .send()
                            .await
                            .map(|_| ()),
                        Route::Portfolio => perps
                            .public()
                            .portfolio(&probes.addresses[(n as usize) % probes.addresses.len()])
                            .send()
                            .await
                            .map(|_| ()),
                        Route::Bbo => perps.market().bbo().instrument_id(iid).send().await.map(|_| ()),
                    };
                    sent.fetch_add(1, Ordering::Relaxed);
                    if let Err(e) = result {
                        eprintln!("{}: {e}", route.name());
                    }
                }
            }));
        }
    }
    for w in workers {
        let _ = w.await;
    }
    let sent = sent.load(Ordering::Relaxed);
    let throttled = throttles.load(Ordering::Relaxed);
    println!("validation: {sent} requests over {}s, {throttled} throttled", cfg.stage_secs);
    if throttled == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

// ── Configuration ───────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
struct Config {
    routes: Vec<Route>,
    stages: Vec<f64>,
    client_paced: bool,
    stage_secs: u64,
    cooldown_secs: u64,
    concurrency: usize,
    addresses: usize,
    base_url: String,
}

const USAGE: &str = "\
Measure or validate the rate limit for public Perps routes.

Usage: info_soak --route <routes> [options]

  --route <routes>        klines | trades | portfolio | bbo. A ramp takes one;
                          --pace client takes a comma list or `all`
  --stages <r1,r2,...>    Ramp rates in req/s, ascending, at most 40
                          (default: 5,10,15,20,30)
  --pace client           Validate instead: pace by the shipped limiter and
                          require zero 429s
  --stage-secs <n>        Seconds per stage (default: 60 ramp, 120 validation)
  --cooldown-secs <n>     Idle seconds between ramp stages (default: 120)
  --concurrency <n>       In-flight requests per route (default: 8 ramp, 4 validation)
  --addresses <n>         Leaderboard addresses to draw portfolio probes from (default: 500)
  --base-url <url>        Override the host
  -h, --help              Show this message";

fn parse_stages(raw: &str) -> Result<Vec<f64>, String> {
    let stages: Vec<f64> = raw
        .split(',')
        .map(|s| s.trim().parse::<f64>().map_err(|_| format!("bad stage rate: {s:?}")))
        .collect::<Result<_, _>>()?;
    if stages.is_empty() || stages.iter().any(|r| !r.is_finite() || *r <= 0.0 || *r > CEILING_RPS) {
        return Err(format!("stages must be positive and at most {CEILING_RPS}"));
    }
    if stages.windows(2).any(|w| w[1] <= w[0]) {
        return Err("--stages must be strictly ascending".into());
    }
    Ok(stages)
}

impl Config {
    fn from_args(args: impl Iterator<Item = String>) -> Result<Option<Self>, String> {
        let mut routes = None;
        let mut stages = None;
        let mut client_paced = false;
        let mut stage_secs = None;
        let mut cooldown_secs = 120;
        let mut concurrency = None;
        let mut addresses = 500;
        let mut base_url = DEFAULT_BASE_URL.to_owned();
        let mut args = args.peekable();
        while let Some(flag) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{flag} requires a value"));
            match flag.as_str() {
                "-h" | "--help" => return Ok(None),
                "--route" => routes = Some(parse_routes(&value()?)?),
                "--stages" => stages = Some(parse_stages(&value()?)?),
                "--pace" => match value()?.as_str() {
                    "client" => client_paced = true,
                    other => return Err(format!("--pace takes only `client`, got {other:?}")),
                },
                "--stage-secs" => stage_secs = Some(value()?.parse().map_err(|_| "bad --stage-secs")?),
                "--cooldown-secs" => cooldown_secs = value()?.parse().map_err(|_| "bad --cooldown-secs")?,
                "--concurrency" => concurrency = Some(value()?.parse().map_err(|_| "bad --concurrency")?),
                "--addresses" => addresses = value()?.parse().map_err(|_| "bad --addresses")?,
                "--base-url" => base_url = value()?.trim_end_matches('/').to_owned(),
                other => return Err(format!("unknown argument: {other}")),
            }
        }
        let routes = routes.ok_or("--route is required")?;
        if !client_paced && routes.len() != 1 {
            return Err("a ramp measures one route at a time; list several only with --pace client".into());
        }
        Ok(Some(Self {
            routes,
            stages: stages.unwrap_or_else(|| DEFAULT_STAGES.to_vec()),
            client_paced,
            stage_secs: stage_secs.unwrap_or(if client_paced { 120 } else { 60 }),
            cooldown_secs,
            concurrency: concurrency.unwrap_or(if client_paced { 4 } else { 8 }),
            addresses,
            base_url,
        }))
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cfg = match Config::from_args(std::env::args().skip(1)) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if cfg.client_paced {
        run_validation(&cfg).await
    } else {
        run_ramp(&cfg, cfg.routes[0]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(at: u64) -> (Duration, Reply) {
        (Duration::from_secs(at), Reply::Ok)
    }

    #[test]
    fn a_429_is_throttled_with_its_identifier_and_retry_after() {
        assert_eq!(
            classify(429, None, Some("2"), r#"{"status":"err","error":"ip_rate_limited"}"#),
            Reply::Throttled {
                code: "ip_rate_limited".into(),
                retry_after: Some(2)
            }
        );
    }

    #[test]
    fn a_cache_hit_is_not_an_origin_reply() {
        assert_eq!(classify(200, Some("Hit from cloudfront"), None, "[]"), Reply::CacheHit);
        assert_eq!(classify(200, Some("Miss from cloudfront"), None, "[]"), Reply::Ok);
    }

    #[test]
    fn a_stage_with_any_throttle_is_throttled_at_its_first() {
        let replies = vec![
            ok(1),
            (Duration::from_secs(7), Reply::Throttled { code: "ip_rate_limited".into(), retry_after: None }),
            ok(9),
        ];
        assert_eq!(
            judge(&replies),
            Verdict::Throttled { after: Duration::from_secs(7), code: "ip_rate_limited".into() }
        );
    }

    #[test]
    fn a_stage_mostly_served_from_cache_is_saturated_not_clean() {
        let replies: Vec<_> = (0..10)
            .map(|i| if i < 5 { ok(i) } else { (Duration::from_secs(i), Reply::CacheHit) })
            .collect();
        assert_eq!(judge(&replies), Verdict::Saturated { origin_share: 0.5 });
    }

    #[test]
    fn pin_is_the_last_clean_stage_before_the_first_unclean_one() {
        let stages = vec![
            (5.0, Verdict::Clean),
            (10.0, Verdict::Clean),
            (15.0, Verdict::Throttled { after: Duration::from_secs(3), code: "x".into() }),
            (20.0, Verdict::Clean),
        ];
        assert_eq!(pin(&stages), Some(100));
        assert_eq!(pin(&[(5.0, Verdict::Invalid { errors: 1 })]), None);
    }

    #[test]
    fn pacer_never_hands_out_a_slot_in_the_past() {
        let pacer = Pacer::new(Duration::from_millis(10));
        let start = Instant::now();
        pacer.reserve(start);
        let later = start + Duration::from_secs(10);
        assert_eq!(pacer.reserve(later), later);
        assert_eq!(pacer.reserve(later), later + Duration::from_millis(10));
    }

    #[test]
    fn a_ramp_takes_exactly_one_route() {
        let err = Config::from_args(["--route", "klines,trades"].map(String::from).into_iter()).unwrap_err();
        assert!(err.contains("one route at a time"));
        let cfg = Config::from_args(["--route", "all", "--pace", "client"].map(String::from).into_iter())
            .unwrap()
            .unwrap();
        assert_eq!(cfg.routes.len(), 4);
        assert_eq!(cfg.stage_secs, 120);
    }

    #[test]
    fn stages_must_ascend_and_stay_under_the_ceiling() {
        assert!(parse_stages("10,5").is_err());
        assert!(parse_stages("10,50").is_err());
        assert_eq!(parse_stages("5,10").unwrap(), vec![5.0, 10.0]);
    }
}
```

- [ ] **Step 3: Build and run the unit tests**

Run: `cargo test -p polyoxide-perps --example info_soak`
Expected: 8 tests pass. The URL list is built before each worker is spawned, so no worker borrows `probes`.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all
git add polyoxide-perps/examples/info_soak.rs polyoxide-perps/Cargo.toml
git commit -m "feat(perps): info_soak rate-limit ramp and validation harness"
```

---

### Task 13: Run the soak and pin `RateLimiter::perps_default()`

**Files:**
- Modify: `polyoxide-core/src/rate_limit.rs`, `polyoxide-perps/src/client.rs`, `docs/specs/perps/OBSERVED.md`

- [ ] **Step 1: Ramp each route**

Run, one at a time, from a machine whose IP is not otherwise talking to the host (each run is roughly 5 stages × 60 s plus 4 × 120 s cooldown, about 13 minutes):

```bash
cargo run --release -p polyoxide-perps --example info_soak -- --route klines
cargo run --release -p polyoxide-perps --example info_soak -- --route trades
cargo run --release -p polyoxide-perps --example info_soak -- --route portfolio
cargo run --release -p polyoxide-perps --example info_soak -- --route bbo
```

Each prints `<route>: pin N per 10s (…stages…)`. Wait at least 5 minutes between routes so one ramp's tail does not throttle the next's first stage. If every stage of a route is clean, re-run it with `--stages 30,35,40`; if the first stage is throttled, re-run with `--stages 1,2,3,4`. Keep the four `pin` lines and the stage lists.

- [ ] **Step 2: Add `perps_default()` to core**

In `polyoxide-core/src/rate_limit.rs`, after `relay_default()` inside `impl RateLimiter`, add, substituting each `PIN_*` constant with the count its ramp printed (the four `KLINES`/`TRADES`/`PORTFOLIO`/`BBO` constants in Step 3 get the same numbers, transcribed separately from OBSERVED.md):

```rust
    /// Perps API rate limits.
    ///
    /// Upstream publishes no figure for the public `/v1/info/*` routes, only
    /// that a per-IP token bucket exists. Each count is the highest clean
    /// ramp stage measured by `polyoxide-perps/examples/info_soak.rs` and
    /// recorded in `docs/specs/perps/OBSERVED.md`; `quota()` reserves a
    /// tenth. Routes that were not soaked fall to the general bucket, which
    /// is set to the lowest measured route so an unmeasured route cannot be
    /// driven harder than any measured one.
    pub fn perps_default() -> Self {
        let ten_sec = Duration::from_secs(10);
        const PIN_KLINES: u32 = 0;
        const PIN_TRADES: u32 = 0;
        const PIN_PORTFOLIO: u32 = 0;
        const PIN_BBO: u32 = 0;
        let general = PIN_KLINES.min(PIN_TRADES).min(PIN_PORTFOLIO).min(PIN_BBO);

        Self {
            inner: Arc::new(RateLimiterInner {
                default: DirectLimiter::direct(quota(general, ten_sec)),
                cooldown_until: Mutex::new(None),
                limits: vec![
                    simple_limit("/v1/info/klines", None, PIN_KLINES, ten_sec),
                    simple_limit("/v1/info/trades", None, PIN_TRADES, ten_sec),
                    simple_limit("/v1/info/portfolio", None, PIN_PORTFOLIO, ten_sec),
                    simple_limit("/v1/info/bbo", None, PIN_BBO, ten_sec),
                ],
            }),
        }
    }
```

The four `0` values are what the ramps replace; a `0` that survives fails the agreement test below (`quota` clamps it to 2 per window, which no row is).

- [ ] **Step 3: Add the agreement tests**

After the `documented_gamma_limits` module in the same file, add:

```rust
#[cfg(test)]
mod documented_perps_limits {
    //! Agreement tests for the Perps table. Nothing is published, so the
    //! rows are the measured figures in `docs/specs/perps/OBSERVED.md`.

    use super::agreement::*;
    use super::*;

    /// The measured table, transcribed by hand from the OBSERVED.md runs.
    /// This is the golden vector: a second transcription, separate from the
    /// constants inside `perps_default()`, so a typo in either is caught.
    const KLINES: u32 = 0;
    const TRADES: u32 = 0;
    const PORTFOLIO: u32 = 0;
    const BBO: u32 = 0;

    fn measured() -> Vec<DocumentedRule> {
        vec![
            ("/v1/info/klines", Some(Method::GET), vec![(KLINES, 10)]),
            ("/v1/info/trades", Some(Method::GET), vec![(TRADES, 10)]),
            ("/v1/info/portfolio", Some(Method::GET), vec![(PORTFOLIO, 10)]),
            ("/v1/info/bbo", Some(Method::GET), vec![(BBO, 10)]),
        ]
    }

    #[test]
    fn every_soaked_route_has_its_own_row() {
        for (path, _, specs) in measured() {
            assert!(specs[0].0 >= 10, "{path} expected {} per 10s: the ramp result was not written in", specs[0].0);
        }
        assert_matches_published(&RateLimiter::perps_default(), measured(), u32::MAX);
    }

    #[test]
    fn an_unsoaked_info_route_is_held_to_the_general_bucket_not_left_unlimited() {
        let rl = RateLimiter::perps_default();
        assert!(rl.resolve_specs("/v1/info/instruments", Some(&Method::GET)).is_empty());
        // The general bucket is the lowest measured row. Drain the general
        // bucket through an unsoaked route and the next call has to wait.
        assert_unconfigured(&rl, "/v1/info/nope");
    }

    #[tokio::test]
    async fn the_klines_row_actually_paces() {
        let rl = RateLimiter::perps_default();
        let count = rl.resolve_specs("/v1/info/klines", Some(&Method::GET))[0].count;
        assert_paced_by_its_own_quota(&rl, "/v1/info/klines", count, Duration::from_secs(10)).await;
    }
}
```

`assert_matches_published`'s `general` argument only feeds an error message; `u32::MAX` keeps the division safe.

- [ ] **Step 4: Make the builder use it**

In `polyoxide-perps/src/client.rs`, `PerpsBuilder::new()`: change `rate_limiter: None,` to `rate_limiter: Some(RateLimiter::perps_default()),`. The limiter is a private field of `HttpClient`, so there is no unit test for this; the validation run in Step 7 is what proves the default client is paced.

- [ ] **Step 5: Mock test for the 429 path through the client**

Append to `polyoxide-perps/tests/mock_api.rs`:

```rust
// ── rate limiting ───────────────────────────────────────────────

#[tokio::test]
async fn a_429_is_retried_and_retry_after_zero_does_not_shorten_the_backoff() {
    use std::time::Instant;
    let mut server = Server::new_async().await;
    let throttled = server
        .mock("GET", "/v1/info/time")
        .with_status(429)
        .with_header("retry-after", "0")
        .with_body(r#"{"status":"err","error":"ip_rate_limited"}"#)
        .expect(1)
        .create_async()
        .await;
    let ok = server
        .mock("GET", "/v1/info/time")
        .with_status(200)
        .with_body(r#"{"time":1}"#)
        .expect(1)
        .create_async()
        .await;

    let start = Instant::now();
    let time = test_perps(&server).health().time().send().await.expect("retried to success");
    throttled.assert_async().await;
    ok.assert_async().await;
    assert_eq!(time.time, 1);
    // The client's own first backoff is the floor; a Retry-After of zero may
    // not pull the retry forward (the Cloudflare lesson in CLAUDE.md).
    assert!(
        start.elapsed() >= std::time::Duration::from_millis(100),
        "retry landed after {:?}: Retry-After: 0 shortened the backoff",
        start.elapsed()
    );
}
```

mockito serves mocks in creation order for the same path, so the 429 is answered first. If `RetryConfig::default()`'s first backoff is below 100 ms, read the actual value from `polyoxide-core/src/rate_limit.rs` (`RetryConfig` / `backoff`) and assert against that figure instead.

Run: `cargo test -p polyoxide-perps --test mock_api`
Expected: 13 mock tests pass.

- [ ] **Step 6: Run the core tests**

Run: `cargo test -p polyoxide-core documented_perps_limits`
Expected: 3 tests pass.

- [ ] **Step 7: Validate the pinned table against the host**

Run: `cargo run --release -p polyoxide-perps --example info_soak -- --route all --pace client`
Expected: `validation: N requests over 120s, 0 throttled` and exit code 0. If it reports throttles, lower the offending row by one ramp step and re-run.

- [ ] **Step 8: Record the runs in `OBSERVED.md`**

Replace the `## Rate limits` section with the measured results:

```markdown
## Rate limits

Nothing numeric is published for the public routes. Measured with
`polyoxide-perps/examples/info_soak.rs` on <date>, one route per run, 60 s
stages, 120 s cooldowns, 8 in-flight, distinct URLs throughout:

| Route | Stages (req/s → verdict) | Pinned (per 10 s) |
|-------|--------------------------|-------------------|
| `/v1/info/klines` | … | … |
| `/v1/info/trades` | … | … |
| `/v1/info/portfolio` | … | … |
| `/v1/info/bbo` | … | … |

Validation (`--route all --pace client`, 120 s, 4 in-flight per route):
<N> requests, 0 throttled. A 429 body seen during the ramps was
`{"status":"err","error":"ip_rate_limited"}` with `Retry-After: <n>`.

The WebSocket budget was not soaked; `LimitTier` carries the only published
figures for it (see the wire-only fields above).
```

Fill every cell from the run output; the table is the golden vector the core test module refers to.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all
git add polyoxide-core/src/rate_limit.rs polyoxide-perps/src/client.rs polyoxide-perps/tests/mock_api.rs docs/specs/perps/OBSERVED.md
git commit -m "feat(perps): measured rate-limit table for the public routes"
```

---

### Task 14: Workspace wiring and docs

**Files:**
- Modify: `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `.github/workflows/release.yml`, `.github/workflows/nightly-behavioral.yml`, `docs/specs/INDEX.md`, `docs/specs/perps/INDEX.md`, `CLAUDE.md`

- [ ] **Step 1: Unified crate feature**

`polyoxide/Cargo.toml`: under `[features]` add `perps = ["dep:polyoxide-perps"]` after the `rtds` line, and change `full` to `full = ["clob", "gamma", "data", "ws", "rtds", "perps"]`. Under `[dependencies]` add `polyoxide-perps = { workspace = true, optional = true }`. Update `description` to `"Unified Rust client for Polymarket APIs (CLOB, Gamma, Data, RTDS and Perps)"`.

`polyoxide/src/lib.rs`: after `pub use polyoxide_rtds;` add

```rust
#[cfg(feature = "perps")]
pub use polyoxide_perps;
```

In `pub mod prelude`, after the `rtds` block add

```rust
    #[cfg(feature = "perps")]
    pub use polyoxide_perps::{Perps, PerpsError};
```

In `PolymarketError`, after the `Gamma` variant add

```rust
    /// Perps API error
    #[cfg(feature = "perps")]
    #[error("Perps error: {0}")]
    Perps(#[from] polyoxide_perps::PerpsError),
```

- [ ] **Step 2: Release order**

`.github/workflows/release.yml`: change the comment to `# Publish in dependency order: core -> rtds -> perps -> relay -> gamma -> data -> clob -> polyoxide` and the array to

```bash
CRATES=("polyoxide-core" "polyoxide-rtds" "polyoxide-perps" "polyoxide-relay" "polyoxide-gamma" "polyoxide-data" "polyoxide-clob" "polyoxide")
```

- [ ] **Step 3: Nightly row**

`.github/workflows/nightly-behavioral.yml`: after the `polyoxide-rtds` row add

```yaml
          - { crate: polyoxide-perps, suite: live,         timeout: 15, flags: "--test live_api" }
```

- [ ] **Step 4: Spec index**

`docs/specs/INDEX.md`: move the Perps row out of the "not implemented" table into the implemented table with a crate column:

```markdown
| [Perps](perps/INDEX.md) | `https://api.perpetuals.polymarket.com` | Perpetual futures: market info implemented; accounts and orders pending | `polyoxide-perps` (public `/v1/info/*`) |
```

and change the AsyncAPI row to

```markdown
| [perps/asyncapi.json](perps/asyncapi.json) | Perps WebSocket (27 channels) — public channels planned for `polyoxide-perps` (`ws` feature); see [perps/OBSERVED.md](perps/OBSERVED.md) |
```

`docs/specs/perps/INDEX.md`: replace the "Not implemented by polyoxide" blockquote with

```markdown
> **Partially implemented.** `polyoxide-perps` covers the 21 public
> `/v1/info/*` routes. Credentials (`POST /v1/account/proxy`), the
> header-authenticated `/v1/account/*` reads, the signed `/v1/trade/*` routes,
> funds and BLP are not yet implemented; the API's own `POLYMARKET-PROXY` /
> `POLYMARKET-SECRET` auth (below) is separate from the CLOB's L1/L2 layers.
```

- [ ] **Step 5: CLAUDE.md**

- In the workspace diagram, add `├── polyoxide-perps     (perpetual futures: public market data; auth and trading pending)` under `polyoxide-core`, and add `polyoxide-perps` to the list of component crates in the sentence that begins "Note: `polyoxide-cli` does **not** depend" only if the CLI gains a dependency (it does not in this plan, so leave that sentence).
- In the "Not yet implemented" paragraph under "API Specs", change the Perps entry to say the public `/v1/info/*` routes are implemented by `polyoxide-perps` and the rest is pending, and point at `docs/specs/perps/OBSERVED.md`.
- Add a short paragraph after it:

```markdown
**Perps public routes** are implemented by `polyoxide-perps` (`Perps::new()`,
namespaces `health()`, `exchange()`, `market()`, `public()`). Three test files
hold it in place on the pattern of Data v2: `tests/spec_agreement.rs` (types,
enums and query keys against `docs/specs/perps/openapi.json`, restricted to
schemas reachable from `/v1/info/*`), `tests/wire_agreement.rs` (against
`tests/fixtures/`, refreshed by `scripts/capture_perps_fixtures.py`) and
`tests/live_api.rs`. Wire-only fields are allowed through `OBSERVED_EXTRA` and
recorded in `docs/specs/perps/OBSERVED.md`. Klines and mark points are
positional arrays on the wire and have hand-written serde. The host is fronted
by CloudFront, so the rate rows in `RateLimiter::perps_default` were measured
with `polyoxide-perps/examples/info_soak.rs` over distinct URLs.
```

- In "Publishing Order", change to `core → rtds → perps → relay → gamma → data → clob → polyoxide` and note that `polyoxide-perps` depends only on core.
- In the `polyoxide` feature-flags sentence, add `perps`.

- [ ] **Step 6: Build the unified crate with and without the feature**

Run:
```bash
cargo build -p polyoxide
cargo build -p polyoxide --features perps
cargo test -p polyoxide --features full
```
Expected: all three succeed.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add polyoxide/Cargo.toml polyoxide/src/lib.rs .github/workflows docs/specs/INDEX.md docs/specs/perps/INDEX.md CLAUDE.md Cargo.lock
git commit -m "feat(polyoxide): perps feature; wire polyoxide-perps into release, nightly and docs"
```

---

### Task 15: Full verification

- [ ] **Step 1: Run the CI gates locally**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

Expected: every command exits 0. The doc build is the gate CLAUDE.md warns about: a red one silently withholds the release tag.

- [ ] **Step 2: Run the crate's live suite once more**

Run: `cargo test -p polyoxide-perps --test live_api -- --ignored`
Expected: 4 passed.

- [ ] **Step 3: Report**

State which soak counts were pinned, which fixtures needed reconciliation in Task 9, and anything left out. Plan 2 (WebSocket) starts from this commit.
