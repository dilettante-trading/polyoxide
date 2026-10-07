# polyoxide-binance HTTP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `polyoxide-binance` crate that reads Binance USDⓈ-M futures public market data from `fapi.binance.com`. Every request goes through a request-weight budget, money is `Decimal`, errors are typed, and the crate is held in place by offline, wire-agreement and live tests.

**Architecture:** One new crate, depending only on `polyoxide-core`. Every route is a `WeightedRequest<T>` that charges a crate-local `WeightBudget` before it sends through core's `HttpClient`. The budget has a per-minute weight and a separate bucket for the weightless funding routes. Every response's `X-MBX-USED-WEIGHT-1M` header raises the budget's count, and a `429` or `418` becomes a client-wide cooldown. Core gains one switch, `HttpClientBuilder::gzip`, off by default. This is plan 1 of 2: `2026-10-07-polyoxide-binance-ws.md` adds the sockets and `polyoxide ws binance`, and the 0.37.0 release follows it.

**Tech Stack:** Rust 2021 (MSRV 1.91), tokio, reqwest 0.12 (rustls, plus the `gzip` feature this plan enables), serde and serde_json, rust_decimal, thiserror, mockito. The capture script is Python 3, stdlib only.

**Spec:** `docs/superpowers/specs/2026-10-07-polyoxide-binance-design.md`. The REST names follow prader-rs's consumer contract (its plan at prader-rs 42a13ea05), which the spec adopted in eebe5fa.

---

## Before you start

- Work on branch `aidanb/polyoxide-binance` in its loom worktree. Do not switch branches or create worktrees.
- Every code block below was compiled, rustfmt-formatted, clippy-checked and tested in a scratch copy of this repository on 2026-10-07, with Rust 1.95. CI floats on stable (1.99 on 2026-10-07), which can add lints 1.95 does not have.
- Build with `-j 4`. This machine's OOM reaper sends rustc SIGTERM under memory pressure, and `signal: 15` or `exit status: 254` means the reaper fired, not that the code is broken. Never point `CARGO_TARGET_DIR` at `/tmp`: it is a RAM-backed tmpfs.
- Run `cargo fmt --all` before every commit.
- **Expected warnings in Tasks 4 and 5.** The budget's and the error classifier's crate-private functions are first used by the send loop in Task 6. Until then `cargo build` reports them as `never used`. That is expected; run clippy with `-D warnings` from Task 6 on.
- Live steps (Tasks 3, 7 and 8) call `fapi.binance.com` and spend under 300 of the 2400 weight per minute. A `451` means Binance does not serve your location; stop and ask.
- End every commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN
  ```
- The first edit of a file under `.github/workflows/` can be refused by a security hook. Retry the same edit once, then grep to confirm it landed.

## Facts the code relies on

Each was measured on 2026-10-07; method and detail are in Task 9's `OBSERVED.md`.

| Fact | Where it matters |
|---|---|
| The weight window is the UTC clock minute; the header fell to 1 just after two successive minute boundaries | `WeightBudget::try_charge` |
| `klines` weight bands are inclusive at the top (100 → 1, 500 → 2, 1000 → 5, more → 10); no `limit` costs 5 | `Route::cost` |
| `depth` without `limit` costs 1 and returns 500 levels; 500 costs 10, 1000 costs 20 | `Route::cost`, `MarketApi::depth` |
| `aggTrades` costs 20 at every limit and serves only the last 48 hours | `Route::cost` |
| `exchangeInfo` has a top-level `futuresType` the handover fixture dropped | `ExchangeInfo`, Task 3's capture |
| `fundingInfo`'s `updateTime` can be `null`; it also lists COIN-M symbols (`BTCUSD_PERP`) | `FundingInfo`, rows use `String` symbols |
| `fundingRate`'s `markPrice` is `""` before about 2022 | `FundingRate::mark_price: Option<Decimal>` |
| Quarterly symbols carry `_`; no listed symbol has a lowercase ASCII letter; the host accepts `btcusdt` | `Symbol` |
| `1s` klines are refused (`-1120`) | `Interval` has 15 variants |

## File structure

| File | Responsibility |
|---|---|
| `Cargo.toml` (workspace) | Member, `[workspace.dependencies]` entry, reqwest `gzip` feature |
| `polyoxide-core/src/client.rs` | `HttpClientBuilder::gzip(bool)`, off by default |
| `polyoxide-data/examples/v2_soak/main.rs`, `polyoxide-gamma/examples/{cf_burst_probe,gamma_batch_ceiling}.rs` | Pin `.gzip(false)` so their measured requests do not change |
| `polyoxide-binance/Cargo.toml` | The crate manifest |
| `polyoxide-binance/README.md` | Crate docs; its examples are doctests |
| `polyoxide-binance/src/lib.rs` | Re-exports |
| `polyoxide-binance/src/error.rs` | `BinanceError`, classification by status and body |
| `polyoxide-binance/src/weight.rs` | `Route`, `Cost`, the weight table, `WeightBudget` |
| `polyoxide-binance/src/usdm/mod.rs` | `Usdm`, `UsdmBuilder`, `DEFAULT_BASE_URL` |
| `polyoxide-binance/src/usdm/request.rs` | `WeightedRequest`, the send loop |
| `polyoxide-binance/src/usdm/types.rs` | Vocabulary (`Symbol`, `Interval`, `DepthLimit`, open enums) and response rows |
| `polyoxide-binance/src/usdm/api/{mod,health,exchange,market}.rs` | The three namespaces and their builders |
| `polyoxide-binance/tests/common/mod.rs` | Key-path and value agreement, shared by two test files |
| `polyoxide-binance/tests/wire_agreement.rs` | Types against captured fixtures |
| `polyoxide-binance/tests/mock_api.rs` | Paths, queries, decoding, errors and the budget against mockito |
| `polyoxide-binance/tests/live_api.rs` | The live suite and the host's drift detector |
| `polyoxide-binance/examples/weight_probe.rs` | Re-measures the weight table live |
| `polyoxide-binance/tests/fixtures/rest/*.json`, `PROVENANCE.md` | Refreshed by the capture script |
| `scripts/capture_binance_fixtures.py` | Captures the REST fixtures |
| `docs/specs/binance/{INDEX,OBSERVED}.md` | What the host is and what it does |
| `CLAUDE.md`, `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-schema.yml` | Docs |
| `.github/workflows/release.yml`, `scripts/finish_release.sh`, `.github/workflows/nightly-behavioral.yml` | Publishing order and the nightly row |

---

### Task 1: Core's gzip switch

`exchangeInfo` is 1.15 MB raw and 51 KB gzipped, so the Binance client asks for gzip. That needs reqwest's `gzip` feature, and with the feature on, reqwest asks every server for gzip from every client unless told not to. The switch keeps every other crate's requests unchanged.

**Files:**
- Modify: `Cargo.toml` (the `reqwest` line in `[workspace.dependencies]`)
- Modify: `polyoxide-core/src/client.rs` (`HttpClientBuilder` and its tests)
- Modify: `polyoxide-data/examples/v2_soak/main.rs`, `polyoxide-gamma/examples/cf_burst_probe.rs`, `polyoxide-gamma/examples/gamma_batch_ceiling.rs`

- [ ] **Step 1: Write the test that pins today's behaviour**

In `polyoxide-core/src/client.rs`, inside `mod tests`, add this immediately before `fn with_base_url_retargets_and_shares_transport`:

```rust
    // ── gzip ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn gzip_is_off_unless_asked_for() {
        // The workspace turns on reqwest's `gzip` feature for one host. With the
        // feature on, reqwest asks every server for gzip unless told not to, so
        // this pins that no other crate's requests changed.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/plain")
            .match_header("accept-encoding", mockito::Matcher::Missing)
            .with_body("ok")
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url()).build().unwrap();
        assert_eq!(client.get_bytes("/plain", &[]).await.unwrap(), b"ok");
        mock.assert_async().await;
    }

```

- [ ] **Step 2: Enable the feature and watch the test fail**

In the root `Cargo.toml`, replace

```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
```

with

```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls", "gzip"], default-features = false }
```

Run: `cargo test -j 4 -p polyoxide-core --lib gzip_is_off_unless_asked_for`

Expected: FAIL, panicking in `gzip_is_off_unless_asked_for` with `called \`Result::unwrap()\` on an \`Err\` value: Api { status: 501, ...`. mockito answers 501 when no mock matches, and reqwest now sends `accept-encoding: gzip`.

- [ ] **Step 3: Add the switch**

In `polyoxide-core/src/client.rs`, add a field to `HttpClientBuilder`:

```rust
pub struct HttpClientBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    rate_limiter: Option<RateLimiter>,
    retry_config: RetryConfig,
    max_concurrent: Option<usize>,
    gzip: bool,
}
```

Set it in `HttpClientBuilder::new` and in `impl Default for HttpClientBuilder`. In both, add `gzip: false,` after `max_concurrent: None,`.

Add the method directly above `pub fn build`:

```rust
    /// Ask for gzip-compressed responses and decode them.
    ///
    /// Off by default. The workspace enables reqwest's `gzip` feature for
    /// hosts with large bodies (Binance's `exchangeInfo` is 1.15 MB raw and
    /// 51 KB gzipped), and with the feature on reqwest would otherwise send
    /// `Accept-Encoding: gzip` from every client. Leaving it off keeps every
    /// other crate's requests byte-identical to before the feature existed.
    pub fn gzip(mut self, enabled: bool) -> Self {
        self.gzip = enabled;
        self
    }
```

In `build`, make the switch the first call on reqwest's builder:

```rust
        let client = reqwest::Client::builder()
            .gzip(self.gzip)
            .timeout(Duration::from_millis(self.timeout_ms))
```

- [ ] **Step 4: Run the test**

Run: `cargo test -j 4 -p polyoxide-core --lib gzip_is_off_unless_asked_for`
Expected: PASS.

- [ ] **Step 5: Test the switch's other side**

Add after `gzip_is_off_unless_asked_for`:

```rust
    #[tokio::test]
    async fn gzip_asks_for_and_decodes_a_compressed_body() {
        // `{"serverTime":1}` compressed by python3's `gzip.compress(.., mtime=0)`.
        const GZIPPED: &[u8] = &[
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0x2a, 0x4e,
            0x2d, 0x2a, 0x4b, 0x2d, 0x0a, 0xc9, 0xcc, 0x4d, 0x55, 0xb2, 0x32, 0xac, 0x05, 0x00,
            0xe2, 0x1d, 0x3e, 0x1a, 0x10, 0x00, 0x00, 0x00,
        ];
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/time")
            .match_header("accept-encoding", mockito::Matcher::Regex("gzip".into()))
            .with_header("content-encoding", "gzip")
            .with_body(GZIPPED)
            .create_async()
            .await;

        let client = HttpClientBuilder::new(server.url())
            .gzip(true)
            .build()
            .unwrap();
        let body = client.get_bytes("/time", &[]).await.unwrap();
        assert_eq!(body, br#"{"serverTime":1}"#);
        mock.assert_async().await;
    }
```

Run: `cargo test -j 4 -p polyoxide-core --lib gzip`
Expected: PASS, 2 tests.

- [ ] **Step 6: Keep the four measuring examples' requests as they were**

Four examples build their own `reqwest::Client`, so the feature now turns gzip on for them. Two of them probe rate limits and two soak them, and a CDN may key its cache on `Accept-Encoding`, so pin each one off.

In `polyoxide-data/examples/v2_soak/main.rs`, replace

```rust
    let http = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
```

with

```rust
    let http = match reqwest::Client::builder()
        // The workspace enables reqwest's `gzip` feature for polyoxide-binance;
        // keep this soak's requests as they were measured.
        .gzip(false)
        .timeout(Duration::from_secs(30))
```

In `polyoxide-gamma/examples/cf_burst_probe.rs`, replace

```rust
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
```

with

```rust
    let client = reqwest::Client::builder()
        // The workspace enables reqwest's `gzip` feature for polyoxide-binance;
        // keep this probe's requests as they were measured.
        .gzip(false)
        .timeout(Duration::from_secs(10))
```

In `polyoxide-gamma/examples/gamma_batch_ceiling.rs`, replace

```rust
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
```

with

```rust
    let client = reqwest::Client::builder()
        // The workspace enables reqwest's `gzip` feature for polyoxide-binance;
        // keep this probe's requests as they were measured.
        .gzip(false)
        .timeout(Duration::from_secs(15))
```

In `polyoxide-perps/examples/info_soak.rs`, replace

```rust
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
```

with

```rust
    let client = reqwest::Client::builder()
        // The workspace enables reqwest's `gzip` feature for polyoxide-binance;
        // keep this soak's requests as they were measured.
        .gzip(false)
        .timeout(Duration::from_secs(30))
```

Run: `cargo check -j 4 -p polyoxide-data --example v2_soak -p polyoxide-gamma --example cf_burst_probe --example gamma_batch_ceiling -p polyoxide-perps --example info_soak`
Expected: `Finished`, no warnings.

No other crate builds its own `reqwest::Client` outside tests (`grep -rn 'Client::builder\|Client::new()' --include='*.rs'`). `polyoxide-clob/src/request.rs` builds two in a test, only to read the headers it set, and never sends them.

- [ ] **Step 7: Run core's tests and commit**

Run: `cargo test -j 4 -p polyoxide-core --lib`
Expected: PASS (148 tests).

```bash
cargo fmt --all
git add Cargo.toml Cargo.lock polyoxide-core/src/client.rs polyoxide-data/examples/v2_soak/main.rs polyoxide-gamma/examples/cf_burst_probe.rs polyoxide-gamma/examples/gamma_batch_ceiling.rs polyoxide-perps/examples/info_soak.rs
git commit -m "feat(core): HttpClientBuilder::gzip, off by default

Enables reqwest's gzip feature for the workspace, for Binance's 1.15 MB
exchangeInfo. With the feature on, reqwest would ask every server for
gzip from every client; the switch keeps every other crate's requests
unchanged, and gzip_is_off_unless_asked_for pins that. Four examples
that build their own client and measure rate limits pin gzip(false).

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 2: The crate and its vocabulary

**Files:**
- Modify: `Cargo.toml` (workspace `members` and `[workspace.dependencies]`)
- Create: `polyoxide-binance/Cargo.toml`
- Create: `polyoxide-binance/src/lib.rs`
- Create: `polyoxide-binance/src/usdm/mod.rs`
- Create: `polyoxide-binance/src/usdm/types.rs`

- [ ] **Step 1: Register the crate**

In the root `Cargo.toml`, add `"polyoxide-binance",` to `members` directly after `"polyoxide",`. In `[workspace.dependencies]`, directly after the `polyoxide-core` line, add:

```toml
polyoxide-binance = { path = "polyoxide-binance", version = "0.36.0" }
```

Create `polyoxide-binance/Cargo.toml`. Do not add the `[[example]]` section yet: Cargo checks that a named target's file exists when it parses the manifest, so Task 8 adds it together with the file.

```toml
[package]
name = "polyoxide-binance"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Rust client library for Binance USDⓈ-M futures public market data"
keywords = ["binance", "futures", "perpetuals", "market-data"]
categories = ["api-bindings", "web-programming::http-client"]

[features]
default = []

[dependencies]
polyoxide-core = { workspace = true }
reqwest = { workspace = true }
rust_decimal = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["time", "sync"] }
tracing = { workspace = true }
url = { workspace = true }

[dev-dependencies]
mockito = { workspace = true }
# `test-util` gives the limiter tests a paused clock, so a minute's wait is
# asserted without actually waiting a minute.
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time", "test-util"] }
```

- [ ] **Step 2: Create the module skeleton**

`polyoxide-binance/src/lib.rs`:

```rust
//! Rust client for Binance USDⓈ-M futures public market data
//! (`fapi.binance.com`).

pub mod usdm;
```

`polyoxide-binance/src/usdm/mod.rs`:

```rust
//! Binance USDⓈ-M futures on `fapi.binance.com`.

pub mod types;
```

- [ ] **Step 3: Write the vocabulary's tests**

Create `polyoxide-binance/src/usdm/types.rs` containing only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_take_letters_digits_and_underscores_in_any_script() {
        for good in ["BTCUSDT", "BTCUSDT_261225", "币安人生USDT", "1000PEPEUSDT"] {
            assert_eq!(Symbol::new(good).unwrap().as_str(), good);
        }
        for bad in [
            "",
            "BTC USDT",
            "BTC@USDT",
            "BTC/USDT",
            "BTC-USDT",
            &"A".repeat(33),
        ] {
            assert_eq!(
                Symbol::new(bad),
                Err(InvalidSymbol(bad.to_owned())),
                "{bad:?}"
            );
        }
        assert!(Symbol::new("A".repeat(32)).is_ok());
    }

    #[test]
    fn ascii_letters_are_uppercased_and_nothing_else_changes() {
        assert_eq!(
            Symbol::new("btcusdt").unwrap(),
            Symbol::new("BTCUSDT").unwrap()
        );
        assert_eq!(
            Symbol::new("btcusdt_261225").unwrap().as_str(),
            "BTCUSDT_261225"
        );
        assert_eq!(
            Symbol::new("币安人生usdt").unwrap().as_str(),
            "币安人生USDT"
        );
    }

    #[test]
    fn every_interval_round_trips_its_wire_spelling() {
        assert_eq!(Interval::ALL.len(), 15);
        for interval in Interval::ALL {
            assert_eq!(interval.as_str().parse::<Interval>(), Ok(*interval));
        }
        assert!("1s".parse::<Interval>().is_err(), "this host refuses 1s");
    }

    #[test]
    fn an_unknown_contract_type_is_kept_verbatim() {
        let parsed: ContractType = serde_json::from_str(r#""NEXT_DECADE""#).unwrap();
        assert_eq!(parsed, ContractType::Other("NEXT_DECADE".to_owned()));
        assert_eq!(serde_json::to_string(&parsed).unwrap(), r#""NEXT_DECADE""#);
        let known: ContractType = serde_json::from_str(r#""TRADIFI_PERPETUAL""#).unwrap();
        assert_eq!(known, ContractType::TradifiPerpetual);
    }
}
```

Run: `cargo test -j 4 -p polyoxide-binance --lib`
Expected: FAIL to compile: ``cannot find type `Symbol` in this scope`` and the like.

- [ ] **Step 4: Write the vocabulary**

Put this above the test module in `polyoxide-binance/src/usdm/types.rs`:

```rust
//! Vocabulary and response rows for USDⓈ-M futures.
//!
//! Prices, quantities and rates are [`Decimal`], decoded from the decimal
//! strings Binance sends, so no value passes through an `f64`. Timestamps are
//! Unix milliseconds. Field names are the long forms; the wire's terse keys
//! (`aggTrades`, `depth`) are serde renames.
use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A Binance symbol such as `BTCUSDT`, `BTCUSDT_261225` or `币安人生USDT`.
///
/// [`Symbol::new`] accepts 1 to 32 characters, each a Unicode letter, a digit
/// or `_`. Binance lists Chinese-character symbols, and quarterly contracts
/// carry their delivery date after an underscore; on 2026-10-07 the 924 listed
/// symbols used no other character and the longest had 17. ASCII letters are
/// uppercased: no listed symbol has a lowercase one, stream names spell every
/// symbol in lowercase, and uppercasing is what lets a stream name be parsed
/// back to the symbol that built it.
///
/// Response rows carry symbols as `String`, taken as sent, so a symbol this
/// type would refuse never fails a whole response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

/// A string [`Symbol::new`] refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a Binance symbol: 1 to 32 letters, digits or underscores")]
pub struct InvalidSymbol(pub String);

impl Symbol {
    /// The longest symbol [`Symbol::new`] accepts, in characters.
    pub const MAX_LEN: usize = 32;

    /// Checks a symbol and uppercases its ASCII letters.
    pub fn new(symbol: impl Into<String>) -> Result<Self, InvalidSymbol> {
        let symbol = symbol.into();
        let chars = symbol.chars().count();
        let valid = (1..=Self::MAX_LEN).contains(&chars)
            && symbol.chars().all(|c| c.is_alphanumeric() || c == '_');
        if valid {
            Ok(Self(symbol.to_ascii_uppercase()))
        } else {
            Err(InvalidSymbol(symbol))
        }
    }

    /// The symbol, as REST spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Symbol {
    type Err = InvalidSymbol;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
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

/// A closed set the client sends: one wire spelling per variant, and an `ALL`
/// table so a test can walk every spelling.
macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $( $(#[$vmeta])* #[serde(rename = $wire)] $variant, )+
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

/// A set Binance reports and extends over time: a value this version does not
/// know is kept verbatim in `Other` instead of failing the response.
macro_rules! open_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )+
            /// A value this version of the SDK does not recognise, kept verbatim.
            Other(String),
        }

        impl $name {
            /// Every variant this SDK knows, in declaration order.
            pub const ALL: &'static [Self] = &[$( Self::$variant ),+];

            /// The wire spelling.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )+
                    Self::Other(raw) => raw,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s {
                    $( $wire => Self::$variant, )+
                    other => Self::Other(other.to_owned()),
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                let Ok(value) = raw.parse();
                Ok(value)
            }
        }
    };
}

wire_enum! {
    /// Kline width, shared by REST `klines` and the kline stream. The docs'
    /// list also has `1s`, which this host refuses (`-1120`).
    Interval {
        M1 => "1m", M3 => "3m", M5 => "5m", M15 => "15m", M30 => "30m",
        H1 => "1h", H2 => "2h", H4 => "4h", H6 => "6h", H8 => "8h", H12 => "12h",
        D1 => "1d", D3 => "3d", W1 => "1w",
        /// One calendar month.
        Mo1 => "1M",
    }
}

wire_enum! {
    /// Levels per side `depth` can return. Any other `limit` is refused
    /// (`-4021`).
    DepthLimit {
        Five => "5", Ten => "10", Twenty => "20", Fifty => "50",
        Hundred => "100", FiveHundred => "500", Thousand => "1000",
    }
}

open_enum! {
    /// A contract's type, from `exchangeInfo`.
    ContractType {
        Perpetual => "PERPETUAL",
        /// A perpetual on a traditional-finance underlying (equities, metals, FX).
        TradifiPerpetual => "TRADIFI_PERPETUAL",
        CurrentMonth => "CURRENT_MONTH",
        NextMonth => "NEXT_MONTH",
        CurrentQuarter => "CURRENT_QUARTER",
        NextQuarter => "NEXT_QUARTER",
        PerpetualDelivering => "PERPETUAL_DELIVERING",
    }
}

open_enum! {
    /// A contract's status, from `exchangeInfo`.
    SymbolStatus {
        PendingTrading => "PENDING_TRADING",
        Trading => "TRADING",
        PreDelivering => "PRE_DELIVERING",
        Delivering => "DELIVERING",
        Delivered => "DELIVERED",
        PreSettle => "PRE_SETTLE",
        /// A delisted perpetual. 134 of 924 symbols on 2026-10-07.
        Settling => "SETTLING",
        Close => "CLOSE",
        TradingHalt => "TRADING_HALT",
        TradingCancelOnly => "TRADING_CANCEL_ONLY",
    }
}

open_enum! {
    /// What a contract's underlying is, from `exchangeInfo`. The values seen on
    /// 2026-10-07; Binance's docs give no list.
    UnderlyingType {
        Coin => "COIN",
        Index => "INDEX",
        Premarket => "PREMARKET",
        Commodity => "COMMODITY",
        Equity => "EQUITY",
        CnEquity => "CN_EQUITY",
        HkEquity => "HK_EQUITY",
        KrEquity => "KR_EQUITY",
        Fx => "FX",
    }
}
```

Notes for the reviewer:
- `Symbol` uppercases ASCII letters. No listed symbol has a lowercase one, and the WebSocket plan's stream names spell symbols in lowercase, so this is what lets an echoed stream name parse back to its symbol. `Symbol` has no serde impls: response rows carry symbols as `String` (Task 3).
- `wire_enum!` is for sets the client sends (`Interval`, `DepthLimit`). `open_enum!` is for sets Binance reports and extends; it keeps an unknown value in `Other(String)`, with the same shape as gamma's and data v2's macros.

- [ ] **Step 5: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --lib`
Expected: PASS, 4 tests.

Code review added two tests in a follow-up commit, which pin the wire spellings and `Symbol`'s character counting and ASCII-only uppercasing. From here on the crate has 6 vocabulary tests, and every later count below includes them.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add Cargo.toml Cargo.lock polyoxide-binance/Cargo.toml polyoxide-binance/src
git commit -m "feat(binance): the crate, Symbol and the USDⓈ-M vocabulary

Symbol accepts letters in any script, digits and _ (quarterlies are
BTCUSDT_261225) and uppercases ASCII, so a lowercase stream name parses
back to its symbol. Interval has the host's fifteen (it refuses 1s);
DepthLimit has the seven depth limits; ContractType, SymbolStatus and
UnderlyingType keep unknown values in Other.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 3: Response rows, fixtures and wire agreement

The handover's fixtures were built from chosen keys, and its `exchangeInfo` lacks the top-level `futuresType`. A wire-agreement test against them cannot see a missing field. This task writes the types, shows the test failing on the handover fixture, then re-captures every top-level key.

**Files:**
- Modify: `polyoxide-binance/src/usdm/types.rs`
- Create: `polyoxide-binance/tests/common/mod.rs`
- Create: `polyoxide-binance/tests/wire_agreement.rs`
- Create: `scripts/capture_binance_fixtures.py`
- Replace: `polyoxide-binance/tests/fixtures/rest/*.json`, `polyoxide-binance/tests/fixtures/PROVENANCE.md`

- [ ] **Step 1: Write the agreement machinery and the test**

`polyoxide-binance/tests/common/mod.rs`:

```rust
//! Wire-agreement machinery shared by `wire_agreement.rs` (fixtures) and
//! `live_api.rs` (the live host).

use std::collections::BTreeSet;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// Every key path in a JSON value: `/symbols[]/filters[]/tickSize`. Arrays of
/// scalars contribute no path, so a positional row is compared by value only.
pub fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let path = format!("{prefix}/{key}");
                out.insert(path.clone());
                key_paths(child, &path, out);
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

/// Asserts every scalar present on both sides is equal, recursing objects by
/// key and arrays by index, so a value that rounds or overflows on decode fails
/// here rather than passing on its key alone.
pub fn assert_values_agree(what: &str, path: &str, wire: &Value, emitted: &Value) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            for (key, wv) in w {
                if let Some(ev) = e.get(key) {
                    assert_values_agree(what, &format!("{path}/{key}"), wv, ev);
                }
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            for (i, (wv, ev)) in w.iter().zip(e).enumerate() {
                assert_values_agree(what, &format!("{path}[{i}]"), wv, ev);
            }
        }
        (Value::Object(_) | Value::Array(_), _) | (_, Value::Object(_) | Value::Array(_)) => {
            panic!("{what}: {path} decoded as {emitted} but the wire sent {wire}")
        }
        _ => assert_eq!(
            wire, emitted,
            "{what}: {path} decoded as {emitted} but the wire sent {wire}"
        ),
    }
}

/// Paths the server sent that the type does not emit, and paths the type
/// emits that the server did not send, after checking every shared value.
pub struct Disagreement {
    pub unmodelled: Vec<String>,
    pub invented: Vec<String>,
}

/// Decodes `text` as `T`, re-encodes it, and compares the two.
pub fn compare<T: DeserializeOwned + Serialize>(what: &str, text: &str) -> Disagreement {
    let wire: Value =
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{what}: not JSON: {e}"));
    let parsed: T = serde_json::from_str(text).unwrap_or_else(|e| panic!("{what}: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();
    assert_values_agree(what, "", &wire, &emitted);

    let mut sent = BTreeSet::new();
    key_paths(&wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(&emitted, "", &mut modelled);
    Disagreement {
        unmodelled: sent.difference(&modelled).cloned().collect(),
        invented: modelled.difference(&sent).cloned().collect(),
    }
}
```

`polyoxide-binance/tests/wire_agreement.rs`:

```rust
//! Agreement between the types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/PROVENANCE.md`.
//!
//! Each fixture is decoded into its type and encoded back, and:
//!
//! 1. **Nothing unmodelled.** Every key path the server sent is emitted by the
//!    type.
//! 2. **Nothing invented.** Every key path the type emits was sent by the
//!    server. An `Option` encodes as `null`, so a field modelled but absent from
//!    the wire shows up here instead of hiding.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a
//!    price with more digits than an `f64` keeps survives exactly or fails.
//!
//! `kline`'s twelfth element, which Binance documents as "ignore", is the one
//! thing dropped on purpose; positional arrays are compared by index only as
//! far as the shorter side goes.

mod common;

use polyoxide_binance::usdm::types::{
    AggTrade, Depth, ExchangeInfo, FundingInfo, FundingRate, Kline, OpenInterest, PremiumIndex,
    ServerTime, Ticker24h,
};
use serde::{de::DeserializeOwned, Serialize};

fn check<T: DeserializeOwned + Serialize>(fixture: &str) {
    let path = format!(
        "{}/tests/fixtures/rest/{fixture}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let diff = common::compare::<T>(fixture, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{fixture}: the server sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{fixture}: the type emits {:?}, which the server did not send",
        diff.invented
    );
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    check::<ExchangeInfo>("exchange_info");
    check::<ServerTime>("time");
    check::<Vec<FundingInfo>>("funding_info");
    check::<Vec<Ticker24h>>("ticker_24hr");
    check::<Vec<PremiumIndex>>("premium_index");
    check::<Vec<Kline>>("klines");
    check::<Vec<FundingRate>>("funding_rate");
    check::<Vec<FundingRate>>("funding_rate_2019");
    check::<OpenInterest>("open_interest");
    check::<Vec<AggTrade>>("agg_trades");
    check::<Depth>("depth");
}

#[test]
fn the_fixtures_cover_the_cases_the_types_exist_for() {
    let read = |name: &str| {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/rest/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    };

    let info: ExchangeInfo = serde_json::from_str(&read("exchange_info")).unwrap();
    use polyoxide_binance::usdm::types::{ContractType, SymbolStatus, UnderlyingType};
    let has = |f: &dyn Fn(&polyoxide_binance::usdm::types::SymbolInfo) -> bool| {
        info.symbols.iter().any(f)
    };
    assert!(
        has(&|s| s.contract_type == ContractType::TradifiPerpetual),
        "a TradFi perpetual"
    );
    assert!(
        has(&|s| s.contract_type == ContractType::CurrentQuarter),
        "a quarterly"
    );
    assert!(
        has(&|s| s.status == SymbolStatus::Settling),
        "a delisted contract"
    );
    assert!(
        has(&|s| !s.symbol.as_str().is_ascii()),
        "a Chinese-character symbol"
    );
    assert!(
        has(&|s| s.symbol.as_str().contains('_')),
        "a symbol with an underscore"
    );
    // An unknown value would land in `Other` and decode fine; failing here
    // is how a value Binance adds gets modelled instead of passing silently.
    assert!(
        !has(&|s| matches!(s.contract_type, ContractType::Other(_))
            || matches!(s.status, SymbolStatus::Other(_))
            || matches!(s.underlying_type, UnderlyingType::Other(_))),
        "a contract type, status or underlying type the enums do not name: model it"
    );

    let funding: Vec<FundingInfo> = serde_json::from_str(&read("funding_info")).unwrap();
    assert!(
        funding.iter().any(|f| f.update_time.is_none()),
        "a null updateTime"
    );

    let old: Vec<FundingRate> = serde_json::from_str(&read("funding_rate_2019")).unwrap();
    assert!(old.iter().all(|f| f.mark_price.is_none()), "markPrice \"\"");
}
```

Run: `cargo test -j 4 -p polyoxide-binance --test wire_agreement`
Expected: FAIL to compile: ``unresolved imports `polyoxide_binance::usdm::types::AggTrade` ``, and so on.

- [ ] **Step 2: Write the rows' unit tests**

In `polyoxide-binance/src/usdm/types.rs`, add these at the end of `mod tests`, before its closing `}`:

```rust

    #[test]
    fn an_unseen_filter_type_is_kept_and_the_known_ones_parse() {
        let filters: Vec<Filter> = serde_json::from_str(
            r#"[{"filterType":"PRICE_FILTER","minPrice":"261.10","maxPrice":"809484","tickSize":"0.10"},
                {"filterType":"MAX_NUM_ICEBERG_ORDERS","maxNumIcebergOrders":5}]"#,
        )
        .unwrap();
        assert!(
            matches!(&filters[0], Filter::PriceFilter { tick_size, .. } if *tick_size == Decimal::new(10, 2))
        );
        let Filter::Other { filter_type, raw } = &filters[1] else {
            panic!("an unseen filter type must not fail the response");
        };
        assert_eq!(filter_type, "MAX_NUM_ICEBERG_ORDERS");
        assert_eq!(raw["maxNumIcebergOrders"], 5);
        let again: Vec<Value> =
            serde_json::from_value(serde_json::to_value(&filters).unwrap()).unwrap();
        assert_eq!(again[0]["filterType"], "PRICE_FILTER");
        assert_eq!(again[1]["maxNumIcebergOrders"], 5);
    }

    #[test]
    fn a_known_filter_with_a_missing_field_fails_loudly() {
        let err = serde_json::from_str::<Filter>(r#"{"filterType":"LOT_SIZE","minQty":"0.001"}"#)
            .unwrap_err();
        assert!(err.to_string().contains("maxQty"), "{err}");
    }

    #[test]
    fn a_kline_drops_the_ignored_element_and_tolerates_more() {
        let wire = r#"[1791354000000,"84125.70","84125.70","84099.90","84106.90","214.561",1791354059999,"18046628.85200",2912,"38.078","3202701.86390","0","extra"]"#;
        let kline: Kline = serde_json::from_str(wire).unwrap();
        assert_eq!(kline.close, "84106.90".parse::<Decimal>().unwrap());
        assert_eq!(kline.trade_count, 2912);
        assert_eq!(
            serde_json::to_string(&kline).unwrap(),
            r#"[1791354000000,"84125.70","84125.70","84099.90","84106.90","214.561",1791354059999,"18046628.85200",2912,"38.078","3202701.86390"]"#
        );
        assert!(serde_json::from_str::<Kline>("[1791354000000]").is_err());
    }

    #[test]
    fn a_level_keeps_every_digit() {
        // More significant digits than an f64 holds.
        let level: Level = serde_json::from_str(r#"["84141.123456789012345","14.663"]"#).unwrap();
        assert_eq!(level.price.to_string(), "84141.123456789012345");
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            r#"["84141.123456789012345","14.663"]"#
        );
    }

    #[test]
    fn an_old_funding_rate_has_no_mark_price() {
        // Captured 2026-10-07: `fundingRate?symbol=BTCUSDT&startTime=1568102400000`.
        let old: FundingRate = serde_json::from_str(
            r#"{"symbol":"BTCUSDT","fundingTime":1568102400000,"fundingRate":"0.00010000","markPrice":"","rateType":"Regular"}"#,
        )
        .unwrap();
        assert_eq!(old.mark_price, None);
        assert_eq!(serde_json::to_value(&old).unwrap()["markPrice"], "");

        let new: FundingRate = serde_json::from_str(
            r#"{"symbol":"BTCUSDT","fundingTime":1791273600001,"fundingRate":"-0.00001592","markPrice":"85514.00007496","rateType":"Regular"}"#,
        )
        .unwrap();
        assert_eq!(new.mark_price, Some("85514.00007496".parse().unwrap()));
    }
```

- [ ] **Step 3: Write the rows**

In `polyoxide-binance/src/usdm/types.rs`, replace

```rust
use serde::{Deserialize, Deserializer, Serialize, Serializer};
```

with

```rust
use rust_decimal::Decimal;
use serde::{
    de::{self, DeserializeOwned, IgnoredAny, SeqAccess, Visitor},
    ser::SerializeSeq,
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_json::Value;
```

Insert this between the last `open_enum!` invocation (`UnderlyingType`) and `#[cfg(test)]`:

```rust
/// `serde(with)` for a decimal string that is empty when the value is unknown.
///
/// `fundingRate` sends `"markPrice": ""` for funding events before about 2022,
/// so a plain `Decimal` would fail a backfill on its first old page.
mod decimal_or_empty {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<Decimal>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.collect_str(value),
            None => serializer.serialize_str(""),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Decimal>, D::Error> {
        let raw = String::deserialize(deserializer)?;
        if raw.is_empty() {
            return Ok(None);
        }
        raw.parse().map(Some).map_err(de::Error::custom)
    }
}

/// `GET /fapi/v1/time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ServerTime {
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
}

/// `GET /fapi/v1/exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExchangeInfo {
    /// Always `UTC`.
    pub timezone: String,
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
    /// `U_MARGINED` on this host.
    pub futures_type: String,
    /// The IP's request-weight and order limits.
    pub rate_limits: Vec<RateLimit>,
    /// Exchange-wide filters; empty on 2026-10-07.
    pub exchange_filters: Vec<Filter>,
    /// Margin assets.
    pub assets: Vec<AssetInfo>,
    /// Every listed contract, including delisted (`SETTLING`) ones.
    pub symbols: Vec<SymbolInfo>,
}

/// One row of `exchangeInfo`'s `rateLimits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RateLimit {
    /// `REQUEST_WEIGHT` or `ORDERS`.
    pub rate_limit_type: String,
    /// `MINUTE` or `SECOND`.
    pub interval: String,
    /// Intervals per window.
    pub interval_num: u32,
    /// The limit per window.
    pub limit: u32,
}

/// One margin asset in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AssetInfo {
    /// Asset name, such as `USDT`.
    pub asset: String,
    /// Whether the asset can be margin.
    pub margin_available: bool,
    /// Binance's auto-exchange threshold.
    #[serde(with = "rust_decimal::serde::str")]
    pub auto_asset_exchange: Decimal,
}

/// One contract in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SymbolInfo {
    /// The contract's symbol.
    pub symbol: String,
    /// The underlying pair, `BTCUSDT` for `BTCUSDT_261225`.
    pub pair: String,
    /// Perpetual, TradFi perpetual or a delivery contract.
    pub contract_type: ContractType,
    /// Delivery time; far in the future for a perpetual.
    pub delivery_date: u64,
    /// Listing time.
    pub onboard_date: u64,
    /// Trading, settling (delisted) and so on.
    pub status: SymbolStatus,
    /// Ignore; Binance documents it so.
    #[serde(with = "rust_decimal::serde::str")]
    pub maint_margin_percent: Decimal,
    /// Ignore; Binance documents it so.
    #[serde(with = "rust_decimal::serde::str")]
    pub required_margin_percent: Decimal,
    /// Base asset.
    pub base_asset: String,
    /// Quote asset.
    pub quote_asset: String,
    /// Margin asset.
    pub margin_asset: String,
    /// Decimal places in a price. Use `PRICE_FILTER`'s tick size to round.
    pub price_precision: u32,
    /// Decimal places in a quantity. Use `LOT_SIZE`'s step size to round.
    pub quantity_precision: u32,
    /// Decimal places of the base asset.
    pub base_asset_precision: u32,
    /// Decimal places of the quote asset.
    pub quote_precision: u32,
    /// What the underlying is.
    pub underlying_type: UnderlyingType,
    /// Free-text tags such as `Layer-2` or `TradFi`.
    pub underlying_sub_type: Vec<String>,
    /// Threshold for algo orders with `priceProtect`.
    #[serde(with = "rust_decimal::serde::str")]
    pub trigger_protect: Decimal,
    /// Liquidation fee rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub liquidation_fee: Decimal,
    /// The most a market order may deviate from the mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub market_take_bound: Decimal,
    /// Most orders a move-order request may touch.
    pub max_move_order_limit: u32,
    /// Order filters.
    pub filters: Vec<Filter>,
    /// Order types the contract accepts.
    pub order_types: Vec<String>,
    /// Times in force the contract accepts.
    pub time_in_force: Vec<String>,
    /// Trading products the contract is open to (`GRID`, `COPY`, …).
    pub permission_sets: Vec<String>,
}

/// An `exchangeInfo` filter, named after its wire `filterType`.
///
/// A filter type this version does not know is kept whole in [`Filter::Other`],
/// so a new one never fails the response. A known type whose fields change
/// still fails, which is how that drift is noticed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Filter {
    /// `PRICE_FILTER`.
    PriceFilter {
        /// Lowest price.
        min_price: Decimal,
        /// Highest price.
        max_price: Decimal,
        /// Price increment.
        tick_size: Decimal,
    },
    /// `LOT_SIZE`: limit orders.
    LotSize {
        /// Smallest quantity.
        min_qty: Decimal,
        /// Largest quantity.
        max_qty: Decimal,
        /// Quantity increment.
        step_size: Decimal,
    },
    /// `MARKET_LOT_SIZE`: market orders.
    MarketLotSize {
        /// Smallest quantity.
        min_qty: Decimal,
        /// Largest quantity.
        max_qty: Decimal,
        /// Quantity increment.
        step_size: Decimal,
    },
    /// `MAX_NUM_ORDERS`.
    MaxNumOrders {
        /// Most open orders.
        limit: u32,
    },
    /// `MIN_NOTIONAL`.
    MinNotional {
        /// Smallest order value.
        notional: Decimal,
    },
    /// `PERCENT_PRICE`.
    PercentPrice {
        /// Highest price as a multiple of the mark price.
        multiplier_up: Decimal,
        /// Lowest price as a multiple of the mark price.
        multiplier_down: Decimal,
        /// Decimal places of the multipliers.
        multiplier_decimal: Decimal,
    },
    /// `POSITION_RISK_CONTROL`.
    PositionRiskControl {
        /// Which side's position is controlled, such as `NONE`.
        position_control_side: String,
    },
    /// A filter type this version does not model.
    Other {
        /// The wire `filterType`.
        filter_type: String,
        /// The whole filter object, `filterType` included.
        raw: Value,
    },
}

/// The wire bodies of the known filters, which `Filter`'s serde goes through.
mod filter_wire {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Price {
        #[serde(with = "rust_decimal::serde::str")]
        pub min_price: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub max_price: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub tick_size: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Lot {
        #[serde(with = "rust_decimal::serde::str")]
        pub min_qty: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub max_qty: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub step_size: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MaxNumOrders {
        pub limit: u32,
    }

    #[derive(Serialize, Deserialize)]
    pub struct MinNotional {
        #[serde(with = "rust_decimal::serde::str")]
        pub notional: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PercentPrice {
        #[serde(with = "rust_decimal::serde::str")]
        pub multiplier_up: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub multiplier_down: Decimal,
        #[serde(with = "rust_decimal::serde::str")]
        pub multiplier_decimal: Decimal,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PositionRiskControl {
        pub position_control_side: String,
    }
}

impl<'de> Deserialize<'de> for Filter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use filter_wire as wire;

        fn parse<T: DeserializeOwned, E: de::Error>(raw: Value) -> Result<T, E> {
            serde_json::from_value(raw).map_err(E::custom)
        }

        let raw = Value::deserialize(deserializer)?;
        let filter_type = raw
            .get("filterType")
            .and_then(Value::as_str)
            .ok_or_else(|| de::Error::custom("a filter without a string filterType"))?
            .to_owned();
        Ok(match filter_type.as_str() {
            "PRICE_FILTER" => {
                let f: wire::Price = parse(raw)?;
                Self::PriceFilter {
                    min_price: f.min_price,
                    max_price: f.max_price,
                    tick_size: f.tick_size,
                }
            }
            "LOT_SIZE" => {
                let f: wire::Lot = parse(raw)?;
                Self::LotSize {
                    min_qty: f.min_qty,
                    max_qty: f.max_qty,
                    step_size: f.step_size,
                }
            }
            "MARKET_LOT_SIZE" => {
                let f: wire::Lot = parse(raw)?;
                Self::MarketLotSize {
                    min_qty: f.min_qty,
                    max_qty: f.max_qty,
                    step_size: f.step_size,
                }
            }
            "MAX_NUM_ORDERS" => {
                let f: wire::MaxNumOrders = parse(raw)?;
                Self::MaxNumOrders { limit: f.limit }
            }
            "MIN_NOTIONAL" => {
                let f: wire::MinNotional = parse(raw)?;
                Self::MinNotional {
                    notional: f.notional,
                }
            }
            "PERCENT_PRICE" => {
                let f: wire::PercentPrice = parse(raw)?;
                Self::PercentPrice {
                    multiplier_up: f.multiplier_up,
                    multiplier_down: f.multiplier_down,
                    multiplier_decimal: f.multiplier_decimal,
                }
            }
            "POSITION_RISK_CONTROL" => {
                let f: wire::PositionRiskControl = parse(raw)?;
                Self::PositionRiskControl {
                    position_control_side: f.position_control_side,
                }
            }
            _ => Self::Other { filter_type, raw },
        })
    }
}

impl Serialize for Filter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use filter_wire as wire;

        fn tagged<T: Serialize, E: serde::ser::Error>(
            filter_type: &str,
            body: &T,
        ) -> Result<Value, E> {
            let mut value = serde_json::to_value(body).map_err(E::custom)?;
            if let Value::Object(map) = &mut value {
                map.insert(
                    "filterType".to_owned(),
                    Value::String(filter_type.to_owned()),
                );
            }
            Ok(value)
        }

        let value = match self {
            Self::PriceFilter {
                min_price,
                max_price,
                tick_size,
            } => tagged(
                "PRICE_FILTER",
                &wire::Price {
                    min_price: *min_price,
                    max_price: *max_price,
                    tick_size: *tick_size,
                },
            )?,
            Self::LotSize {
                min_qty,
                max_qty,
                step_size,
            } => tagged(
                "LOT_SIZE",
                &wire::Lot {
                    min_qty: *min_qty,
                    max_qty: *max_qty,
                    step_size: *step_size,
                },
            )?,
            Self::MarketLotSize {
                min_qty,
                max_qty,
                step_size,
            } => tagged(
                "MARKET_LOT_SIZE",
                &wire::Lot {
                    min_qty: *min_qty,
                    max_qty: *max_qty,
                    step_size: *step_size,
                },
            )?,
            Self::MaxNumOrders { limit } => {
                tagged("MAX_NUM_ORDERS", &wire::MaxNumOrders { limit: *limit })?
            }
            Self::MinNotional { notional } => tagged(
                "MIN_NOTIONAL",
                &wire::MinNotional {
                    notional: *notional,
                },
            )?,
            Self::PercentPrice {
                multiplier_up,
                multiplier_down,
                multiplier_decimal,
            } => tagged(
                "PERCENT_PRICE",
                &wire::PercentPrice {
                    multiplier_up: *multiplier_up,
                    multiplier_down: *multiplier_down,
                    multiplier_decimal: *multiplier_decimal,
                },
            )?,
            Self::PositionRiskControl {
                position_control_side,
            } => tagged(
                "POSITION_RISK_CONTROL",
                &wire::PositionRiskControl {
                    position_control_side: position_control_side.clone(),
                },
            )?,
            Self::Other { raw, .. } => raw.clone(),
        };
        value.serialize(serializer)
    }
}

/// One row of `GET /fapi/v1/fundingInfo`: the symbols whose funding cap,
/// floor or interval was adjusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct FundingInfo {
    /// The contract.
    pub symbol: String,
    /// Highest funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub adjusted_funding_rate_cap: Decimal,
    /// Lowest funding rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub adjusted_funding_rate_floor: Decimal,
    /// Hours between funding events.
    pub funding_interval_hours: u32,
    /// Whether Binance shows a disclaimer for the contract.
    pub disclaimer: bool,
    /// When the adjustment was made. `null` on 57 of 805 rows on 2026-10-07,
    /// `BTCUSDT` among them.
    pub update_time: Option<u64>,
}

/// `GET /fapi/v1/ticker/24hr`: rolling 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Ticker24h {
    /// The contract.
    pub symbol: String,
    /// Last price less the open price.
    #[serde(with = "rust_decimal::serde::str")]
    pub price_change: Decimal,
    /// The change as a percentage.
    #[serde(with = "rust_decimal::serde::str")]
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    #[serde(with = "rust_decimal::serde::str")]
    pub weighted_avg_price: Decimal,
    /// Last traded price.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Last traded quantity.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_qty: Decimal,
    /// Price 24 hours ago.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Window start.
    pub open_time: u64,
    /// Window end.
    pub close_time: u64,
    /// First trade id in the window.
    pub first_id: i64,
    /// Last trade id in the window.
    pub last_id: i64,
    /// Trades in the window.
    pub count: u64,
}

/// `GET /fapi/v1/premiumIndex`: mark price, index price and funding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct PremiumIndex {
    /// The contract.
    pub symbol: String,
    /// Mark price.
    #[serde(with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Index price.
    #[serde(with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Estimated settle price, meaningful only in the hour before a settlement.
    #[serde(with = "rust_decimal::serde::str")]
    pub estimated_settle_price: Decimal,
    /// The funding rate of the current period.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_funding_rate: Decimal,
    /// Interest rate.
    #[serde(with = "rust_decimal::serde::str")]
    pub interest_rate: Decimal,
    /// Next funding event.
    pub next_funding_time: u64,
    /// When the values were computed.
    pub time: u64,
}

/// One candle of `GET /fapi/v1/klines`, a positional array on the wire.
///
/// The wire sends a twelfth element Binance documents as "ignore"; it is
/// dropped, and any further elements are tolerated.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Kline {
    /// Candle start.
    pub open_time: u64,
    /// Open price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Close price.
    pub close: Decimal,
    /// Base-asset volume.
    pub volume: Decimal,
    /// Candle end, inclusive.
    pub close_time: u64,
    /// Quote-asset volume.
    pub quote_volume: Decimal,
    /// Trades in the candle.
    pub trade_count: u64,
    /// Base-asset volume bought by takers.
    pub taker_buy_base_volume: Decimal,
    /// Quote-asset volume bought by takers.
    pub taker_buy_quote_volume: Decimal,
}

/// A decimal string inside a positional array.
struct DecimalStr(Decimal);

impl<'de> Deserialize<'de> for DecimalStr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        rust_decimal::serde::str::deserialize(deserializer).map(Self)
    }
}

fn next<'de, T: Deserialize<'de>, A: SeqAccess<'de>>(
    seq: &mut A,
    index: usize,
    what: &str,
) -> Result<T, A::Error> {
    seq.next_element()?
        .ok_or_else(|| de::Error::invalid_length(index, &what))
}

fn drain<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<(), A::Error> {
    while seq.next_element::<IgnoredAny>()?.is_some() {}
    Ok(())
}

impl<'de> Deserialize<'de> for Kline {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KlineVisitor;

        impl<'de> Visitor<'de> for KlineVisitor {
            type Value = Kline;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a kline array of at least 11 elements")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Kline, A::Error> {
                const WHAT: &str = "a kline array of at least 11 elements";
                let kline = Kline {
                    open_time: next(&mut seq, 0, WHAT)?,
                    open: next::<DecimalStr, _>(&mut seq, 1, WHAT)?.0,
                    high: next::<DecimalStr, _>(&mut seq, 2, WHAT)?.0,
                    low: next::<DecimalStr, _>(&mut seq, 3, WHAT)?.0,
                    close: next::<DecimalStr, _>(&mut seq, 4, WHAT)?.0,
                    volume: next::<DecimalStr, _>(&mut seq, 5, WHAT)?.0,
                    close_time: next(&mut seq, 6, WHAT)?,
                    quote_volume: next::<DecimalStr, _>(&mut seq, 7, WHAT)?.0,
                    trade_count: next(&mut seq, 8, WHAT)?,
                    taker_buy_base_volume: next::<DecimalStr, _>(&mut seq, 9, WHAT)?.0,
                    taker_buy_quote_volume: next::<DecimalStr, _>(&mut seq, 10, WHAT)?.0,
                };
                drain(&mut seq)?;
                Ok(kline)
            }
        }

        deserializer.deserialize_seq(KlineVisitor)
    }
}

impl Serialize for Kline {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(11))?;
        seq.serialize_element(&self.open_time)?;
        for value in [&self.open, &self.high, &self.low, &self.close, &self.volume] {
            seq.serialize_element(&value.to_string())?;
        }
        seq.serialize_element(&self.close_time)?;
        seq.serialize_element(&self.quote_volume.to_string())?;
        seq.serialize_element(&self.trade_count)?;
        seq.serialize_element(&self.taker_buy_base_volume.to_string())?;
        seq.serialize_element(&self.taker_buy_quote_volume.to_string())?;
        seq.end()
    }
}

/// One row of `GET /fapi/v1/fundingRate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct FundingRate {
    /// The contract.
    pub symbol: String,
    /// When funding was paid.
    pub funding_time: u64,
    /// The rate paid.
    #[serde(with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// The mark price at funding. `None` for events before about 2022, which
    /// the wire sends as `""`.
    #[serde(with = "decimal_or_empty")]
    pub mark_price: Option<Decimal>,
    /// `Regular` on every row seen on 2026-10-07.
    pub rate_type: String,
}

/// `GET /fapi/v1/openInterest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct OpenInterest {
    /// The contract.
    pub symbol: String,
    /// Open interest in contracts.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
    /// When it was computed.
    pub time: u64,
}

/// One row of `GET /fapi/v1/aggTrades`: trades at one price, taken by one
/// order, merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AggTrade {
    /// Aggregate trade id.
    #[serde(rename = "a")]
    pub id: u64,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Quantity without the trades involving RPI (Retail Price Improvement)
    /// orders, as Binance documents `nq`.
    #[serde(rename = "nq", with = "rust_decimal::serde::str")]
    pub normal_quantity: Decimal,
    /// First trade id merged.
    #[serde(rename = "f")]
    pub first_trade_id: u64,
    /// Last trade id merged.
    #[serde(rename = "l")]
    pub last_trade_id: u64,
    /// Trade time.
    #[serde(rename = "T")]
    pub time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// `GET /fapi/v1/depth`: an order book snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Depth {
    /// Book update id of the snapshot.
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    /// When the message was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// When the book was last changed.
    #[serde(rename = "T")]
    pub transaction_time: u64,
    /// Bids, best first.
    pub bids: Vec<Level>,
    /// Asks, best first.
    pub asks: Vec<Level>,
}

/// A price level, a `[price, quantity]` pair on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Level {
    /// Price.
    pub price: Decimal,
    /// Quantity at the price.
    pub quantity: Decimal,
}

impl Level {
    /// A level from a price and a quantity.
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self { price, quantity }
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LevelVisitor;

        impl<'de> Visitor<'de> for LevelVisitor {
            type Value = Level;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a [price, quantity] array")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Level, A::Error> {
                const WHAT: &str = "a [price, quantity] array";
                let level = Level {
                    price: next::<DecimalStr, _>(&mut seq, 0, WHAT)?.0,
                    quantity: next::<DecimalStr, _>(&mut seq, 1, WHAT)?.0,
                };
                drain(&mut seq)?;
                Ok(level)
            }
        }

        deserializer.deserialize_seq(LevelVisitor)
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&self.price.to_string())?;
        seq.serialize_element(&self.quantity.to_string())?;
        seq.end()
    }
}
```

Notes for the reviewer:
- Every row symbol is `String`, taken as sent: `fundingInfo` lists COIN-M symbols (`BTCUSD_PERP`), and a validated field would fail the whole response.
- `Filter`'s known types are struct variants named after the wire, as prader-rs's contract reads them (`Filter::PriceFilter { tick_size, .. }`). Serde goes through the private `filter_wire` bodies, and an unseen `filterType` lands in `Other`.
- `Kline` drops the twelfth element Binance documents as "ignore" and tolerates more; `Level` is a `[price, quantity]` pair. Both serialise decimals with `to_string`, which keeps the wire's scale (`"84142.00"`).

Run: `cargo test -j 4 -p polyoxide-binance --lib`
Expected: PASS, 11 tests.

- [ ] **Step 4: Watch the handover fixture fail**

Run: `cargo test -j 4 -p polyoxide-binance --test wire_agreement`
Expected: FAIL in both tests. `every_rest_fixture_agrees_with_its_type` panics with ``exchange_info: missing field `futuresType` ``.

- [ ] **Step 5: Write the capture script**

`scripts/capture_binance_fixtures.py`:

```python
#!/usr/bin/env python3
"""Capture Binance USDⓈ-M REST responses as test fixtures for polyoxide-binance.

Usage: python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures

Writes one JSON file per route under OUT_DIR/rest/ and rewrites OUT_DIR/PROVENANCE.md.
Every top-level key of every response is kept; only list lengths are trimmed, so a
wire-agreement test sees each field the server sends. Stdlib only, no credentials.
Costs about 60 request weight, well inside the 2400 per minute.
"""
import datetime
import json
import os
import sys
import urllib.parse
import urllib.request

BASE = "https://fapi.binance.com"


def get(path, **params):
    query = urllib.parse.urlencode(params)
    url = f"{BASE}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(url, headers={"Accept-Encoding": "identity"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read().decode("utf-8"))


def save(out, name, value):
    with open(os.path.join(out, name), "w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=1)
        f.write("\n")


def first(rows, predicate, what):
    for row in rows:
        if predicate(row):
            return row
    sys.exit(f"no {what} listed; pick the fixture rows by hand")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    rest = os.path.join(out, "rest")
    os.makedirs(rest, exist_ok=True)

    info = get("/fapi/v1/exchangeInfo")
    symbols = info["symbols"]
    trading = [s for s in symbols if s["status"] == "TRADING"]
    chinese = first(trading, lambda s: not s["symbol"].isascii(), "Chinese-character symbol")
    tradfi = first(trading, lambda s: s["contractType"] == "TRADIFI_PERPETUAL", "TradFi perpetual")
    settling = first(symbols, lambda s: s["status"] == "SETTLING", "SETTLING contract")
    quarterly = first(trading, lambda s: s["contractType"] == "CURRENT_QUARTER", "quarterly")
    picked = [first(trading, lambda s: s["symbol"] == "BTCUSDT", "BTCUSDT"), tradfi, chinese, settling, quarterly]
    names = [s["symbol"] for s in picked]
    trimmed = dict(info)
    trimmed["assets"] = info["assets"][:2]
    trimmed["symbols"] = picked
    save(rest, "exchange_info.json", trimmed)

    wanted = {"BTCUSDT", tradfi["symbol"], chinese["symbol"]}
    save(rest, "time.json", get("/fapi/v1/time"))
    save(rest, "ticker_24hr.json", [t for t in get("/fapi/v1/ticker/24hr") if t["symbol"] in wanted])
    save(rest, "premium_index.json", [p for p in get("/fapi/v1/premiumIndex") if p["symbol"] in wanted])
    funding_info = get("/fapi/v1/fundingInfo")
    rows = [f for f in funding_info if f["symbol"] in wanted]
    if not any(f["updateTime"] is None for f in rows):
        rows.append(first(funding_info, lambda f: f["updateTime"] is None, "fundingInfo row with null updateTime"))
    save(rest, "funding_info.json", rows)
    save(rest, "klines.json", get("/fapi/v1/klines", symbol="BTCUSDT", interval="1m", limit=2))
    save(rest, "funding_rate.json", get("/fapi/v1/fundingRate", symbol="BTCUSDT", limit=3))
    # The first funding events, from before markPrice was recorded: it is "" on the wire.
    save(rest, "funding_rate_2019.json",
         get("/fapi/v1/fundingRate", symbol="BTCUSDT", startTime=1568102400000, limit=2))
    save(rest, "open_interest.json", get("/fapi/v1/openInterest", symbol="BTCUSDT"))
    save(rest, "agg_trades.json", get("/fapi/v1/aggTrades", symbol="BTCUSDT", limit=3))
    save(rest, "depth.json", get("/fapi/v1/depth", symbol="BTCUSDT", limit=5))

    today = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    with open(os.path.join(out, "PROVENANCE.md"), "w", encoding="utf-8") as f:
        f.write(f"""# Provenance

REST fixtures (`rest/`) captured {today} from `https://fapi.binance.com` by
`scripts/capture_binance_fixtures.py`. No credentials. Every top-level key is kept; only
list lengths are trimmed.

| File | Request | Trimmed to |
|---|---|---|
| `exchange_info.json` | `GET /fapi/v1/exchangeInfo` | `symbols`: {", ".join(f"`{n}`" for n in names)} (BTCUSDT, a TradFi perpetual, a Chinese-character perpetual, the first `SETTLING` contract, the first `CURRENT_QUARTER`); `assets`: the first two |
| `time.json` | `GET /fapi/v1/time` | as fetched |
| `ticker_24hr.json` | `GET /fapi/v1/ticker/24hr` | {", ".join(f"`{n}`" for n in sorted(wanted))} |
| `premium_index.json` | `GET /fapi/v1/premiumIndex` | the same three |
| `funding_info.json` | `GET /fapi/v1/fundingInfo` | the same three, plus a row with `updateTime: null` if none of them has one |
| `klines.json` | `GET /fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=2` | as fetched |
| `funding_rate.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&limit=3` | as fetched |
| `funding_rate_2019.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&startTime=1568102400000&limit=2` | as fetched; `markPrice` is `""` |
| `open_interest.json` | `GET /fapi/v1/openInterest?symbol=BTCUSDT` | as fetched |
| `agg_trades.json` | `GET /fapi/v1/aggTrades?symbol=BTCUSDT&limit=3` | as fetched |
| `depth.json` | `GET /fapi/v1/depth?symbol=BTCUSDT&limit=5` | as fetched |

## Streams (`ws/`)

Captured 2026-10-07 by the stdlib scripts handed over with the design spec
(`docs/specs/binance/probes/capture_ws.py`): one combined-stream envelope
(`{{"stream", "data"}}`) per stream, from `/market/stream` for `!ticker@arr`,
`!markPrice@arr@1s`, `btcusdt@aggTrade`, `btcusdt@kline_1m`, `btcusdt@markPrice@1s` and
`btcusdt@ticker`, and from `/public/stream` for `btcusdt@depth20@100ms` and
`btcusdt@bookTicker`. Arrays are trimmed to two rows and depth sides to three levels.

## Probes

`docs/specs/binance/probes/` holds the scripts behind the design spec's measured facts:
`probe_rest.py` (weights from `X-MBX-USED-WEIGHT-1M` deltas), `probe_ws.py` and
`probe_ws2.py` (acknowledgements, case, the 1024 cap, the message rate),
`probe_ws_ping.py` (server ping cadence), and `wsprobe.py` (the frame reader they share).
""")
    print("captured:", ", ".join(sorted(os.listdir(rest))))


if __name__ == "__main__":
    main()
```

- [ ] **Step 6: Re-capture**

Run: `python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures`
Expected: `captured: agg_trades.json, depth.json, exchange_info.json, funding_info.json, funding_rate.json, funding_rate_2019.json, klines.json, open_interest.json, premium_index.json, ticker_24hr.json, time.json`

It rewrites `PROVENANCE.md` with the symbols it picked. The TradFi perpetual is whichever trading one is listed first (`XAUUSDT` on 2026-10-07). If the script exits with `no ... listed`, the listing changed; pick the row by hand and say so in `PROVENANCE.md`.

- [ ] **Step 7: Run the agreement test**

Run: `cargo test -j 4 -p polyoxide-binance --test wire_agreement`
Expected: PASS, 2 tests. If it fails with `the server sent [...], which the type does not model`, Binance has added a field since 2026-10-07: model it, and add it to Task 9's `OBSERVED.md`.

Code review added a follow-up commit (2638f71). The module header links `[`Decimal`]` plainly, since this task's import makes an explicit target fail rustdoc. `Filter`'s known variants are `#[non_exhaustive]`. The fixture test also fails on any `Filter::Other`, because a misspelled parse arm otherwise passes every test, and both of its `Other` checks print the offending values.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/src/usdm/types.rs polyoxide-binance/tests scripts/capture_binance_fixtures.py
git commit -m "feat(binance): USDⓈ-M response rows, re-captured fixtures, wire agreement

Every route's row, with Decimal money, String symbols (fundingInfo lists
COIN-M symbols too), Option where the wire sends null or \"\", hand
serde for the positional klines and levels, and Filter keeping unseen
types in Other. The handover's exchangeInfo dropped the top-level
futuresType, so the capture script keeps every top-level key and the
fixtures are re-captured; funding_rate_2019 holds markPrice \"\".

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 4: The weight table and the budget

**Files:**
- Create: `polyoxide-binance/src/weight.rs`
- Modify: `polyoxide-binance/src/lib.rs`

- [ ] **Step 1: Register the module**

Replace `polyoxide-binance/src/lib.rs` with:

```rust
//! Rust client for Binance USDⓈ-M futures public market data
//! (`fapi.binance.com`).

pub mod usdm;
pub mod weight;

pub use weight::WeightBudget;
```

- [ ] **Step 2: Write the tests**

Create `polyoxide-binance/src/weight.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-07 08:35:00 UTC, a minute boundary.
    const ON_A_MINUTE: u64 = 1_791_362_100_000;

    fn budget(weight_per_minute: u32, start_ms: u64) -> WeightBudget {
        WeightBudget::with_clock(
            weight_per_minute,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::Tokio {
                start: Instant::now(),
                start_ms,
            },
        )
    }

    #[test]
    fn documented_weights() {
        use DepthLimit::*;
        let w = Cost::Weight;
        let table = [
            (Route::Ping, w(1)),
            (Route::Time, w(1)),
            (Route::ExchangeInfo, w(1)),
            (Route::FundingInfo, Cost::Funding),
            (Route::Ticker24h { all: false }, w(1)),
            (Route::Ticker24h { all: true }, w(40)),
            (Route::PremiumIndex { all: false }, w(1)),
            (Route::PremiumIndex { all: true }, w(10)),
            (Route::Klines { limit: None }, w(5)),
            (Route::Klines { limit: Some(1) }, w(1)),
            (Route::Klines { limit: Some(100) }, w(1)),
            (Route::Klines { limit: Some(101) }, w(2)),
            (Route::Klines { limit: Some(500) }, w(2)),
            (Route::Klines { limit: Some(501) }, w(5)),
            (Route::Klines { limit: Some(1000) }, w(5)),
            (Route::Klines { limit: Some(1001) }, w(10)),
            (Route::Klines { limit: Some(1500) }, w(10)),
            (Route::FundingRate, Cost::Funding),
            (Route::OpenInterest, w(1)),
            (Route::AggTrades, w(20)),
            (Route::Depth { limit: None }, w(1)),
            (Route::Depth { limit: Some(Five) }, w(2)),
            (Route::Depth { limit: Some(Ten) }, w(2)),
            (
                Route::Depth {
                    limit: Some(Twenty),
                },
                w(2),
            ),
            (Route::Depth { limit: Some(Fifty) }, w(2)),
            (
                Route::Depth {
                    limit: Some(Hundred),
                },
                w(5),
            ),
            (
                Route::Depth {
                    limit: Some(FiveHundred),
                },
                w(10),
            ),
            (
                Route::Depth {
                    limit: Some(Thousand),
                },
                w(20),
            ),
        ];
        for (route, cost) in table {
            assert_eq!(route.cost(), cost, "{route:?}");
        }
    }

    #[test]
    fn the_budget_aims_a_tenth_below_the_published_limit() {
        assert_eq!(WeightBudget::new().per_minute(), 2160);
    }

    #[tokio::test(start_paused = true)]
    async fn a_charge_past_the_budget_waits_for_the_next_minute() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE + 15_000);
        for _ in 0..54 {
            budget.acquire(Cost::Weight(40)).await;
        }
        assert_eq!(budget.used(), 2160);

        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(
            start.elapsed(),
            Duration::from_secs(45),
            "held to the boundary"
        );
        assert_eq!(budget.used(), 1, "the new minute starts from zero");
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_raises_the_count_and_never_lowers_it() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        let charge = budget.acquire(Cost::Weight(1)).await;
        budget.record_used(charge, 2000);
        assert_eq!(budget.used(), 2000, "another process on this IP spent 1999");
        budget.record_used(charge, 3);
        assert_eq!(budget.used(), 2000);

        let start = Instant::now();
        budget.acquire(Cost::Weight(200)).await;
        assert_eq!(
            start.elapsed(),
            Duration::from_secs(60),
            "2000 + 200 passes 2160"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_is_applied_only_to_the_minute_its_request_was_charged_in() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE + 59_900);
        let in_flight = budget.acquire(Cost::Weight(5)).await;
        tokio::time::advance(Duration::from_millis(200)).await;
        budget.acquire(Cost::Weight(5)).await;
        // The response to the first request arrives in the new minute carrying
        // the old minute's count. Applying it would hold the new minute for 60 s.
        budget.record_used(in_flight, 2150);
        assert_eq!(budget.used(), 5);
    }

    #[tokio::test(start_paused = true)]
    async fn a_request_heavier_than_the_budget_is_sent_alone() {
        let budget = budget(20, ON_A_MINUTE);
        let start = Instant::now();
        budget.acquire(Cost::Weight(40)).await;
        assert_eq!(start.elapsed(), Duration::ZERO);
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), Duration::from_secs(60));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_holds_both_limits_and_is_only_extended() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.begin_cooldown(Duration::from_secs(10));
        budget.begin_cooldown(Duration::from_secs(2));

        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), Duration::from_secs(10));

        budget.begin_cooldown(Duration::from_secs(3));
        let start = Instant::now();
        budget.acquire(Cost::Funding).await;
        assert_eq!(start.elapsed(), Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_is_clamped_to_the_longest_documented_ban() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        budget.begin_cooldown(Duration::MAX);
        let start = Instant::now();
        budget.acquire(Cost::Weight(1)).await;
        assert_eq!(start.elapsed(), MAX_COOLDOWN);
    }

    #[tokio::test(start_paused = true)]
    async fn the_funding_bucket_admits_450_per_five_minutes() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        let start = Instant::now();
        let mut admitted = 0;
        while start.elapsed() <= FUNDING_PERIOD {
            budget.acquire(Cost::Funding).await;
            if start.elapsed() <= FUNDING_PERIOD {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 450);
    }

    #[tokio::test(start_paused = true)]
    async fn the_funding_bucket_never_bursts() {
        let budget = budget(PUBLISHED_WEIGHT_PER_MINUTE, ON_A_MINUTE);
        let start = Instant::now();
        budget.acquire(Cost::Funding).await;
        budget.acquire(Cost::Funding).await;
        assert!(
            start.elapsed() >= Duration::from_millis(668),
            "{:?}",
            start.elapsed()
        );
    }
}
```

Run: `cargo test -j 4 -p polyoxide-binance --lib weight`
Expected: FAIL to compile: ``cannot find type `WeightBudget` in this scope`` and the like.

- [ ] **Step 3: Write the table and the budget**

Put this above the test module:

```rust
//! Binance's request-weight budget, and the separate limit on the funding routes.
//!
//! Binance charges each REST request a *weight* that depends on its route and
//! its parameters, and refuses an IP whose weight in the current minute passes
//! `exchangeInfo`'s `REQUEST_WEIGHT` limit of 2400. Core's `RateLimiter` counts
//! requests per path, so it cannot model a cost that varies with `limit`;
//! [`WeightBudget`] does. The table is [`Route::cost`]; the measurements behind
//! it are in `docs/specs/binance/OBSERVED.md`.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::time::Instant;

use crate::usdm::types::DepthLimit;

/// `exchangeInfo`'s `REQUEST_WEIGHT` limit per minute, per IP.
pub const PUBLISHED_WEIGHT_PER_MINUTE: u32 = 2400;

/// Requests per IP every five minutes that `fundingRate` and `fundingInfo`
/// share, as documented. Neither route reports a weight.
pub const PUBLISHED_FUNDING_PER_FIVE_MINUTES: u32 = 500;

/// How long a `418` holds every request when it carries no `Retry-After`: the
/// shortest ban Binance documents.
pub const DEFAULT_BAN: Duration = Duration::from_secs(120);

/// The longest a cooldown can last: the longest ban Binance documents. A
/// `Retry-After` beyond it is clamped.
pub const MAX_COOLDOWN: Duration = Duration::from_secs(3 * 24 * 60 * 60);

/// Reciprocal of the share of a published limit the client leaves unused, as
/// in core's `RESERVED_FRACTION`: aiming at a published quota is a bug, because
/// the server's count and the client's never agree exactly.
const RESERVED_FRACTION: u32 = 10;

const MINUTE_MS: u64 = 60_000;
const FUNDING_PERIOD: Duration = Duration::from_secs(300);

fn after_reserve(published: u32) -> u32 {
    published - published.div_ceil(RESERVED_FRACTION)
}

/// What one request costs, and which limit it draws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Request weight, against the per-minute budget.
    Weight(u32),
    /// One request against the funding routes' own limit.
    Funding,
}

/// A REST route, with the parameters its weight depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Route {
    /// `GET /fapi/v1/ping`.
    Ping,
    /// `GET /fapi/v1/time`.
    Time,
    /// `GET /fapi/v1/exchangeInfo`.
    ExchangeInfo,
    /// `GET /fapi/v1/fundingInfo`.
    FundingInfo,
    /// `GET /fapi/v1/ticker/24hr`: one symbol, or every symbol when `all`.
    Ticker24h {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /fapi/v1/premiumIndex`: one symbol, or every symbol when `all`.
    PremiumIndex {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /fapi/v1/klines`.
    Klines {
        /// The `limit` parameter, if sent.
        limit: Option<u32>,
    },
    /// `GET /fapi/v1/fundingRate`.
    FundingRate,
    /// `GET /fapi/v1/openInterest`.
    OpenInterest,
    /// `GET /fapi/v1/aggTrades`.
    AggTrades,
    /// `GET /fapi/v1/depth`.
    Depth {
        /// The `limit` parameter, if sent.
        limit: Option<DepthLimit>,
    },
}

impl Route {
    /// The route's path on `fapi.binance.com`.
    pub fn path(self) -> &'static str {
        match self {
            Self::Ping => "/fapi/v1/ping",
            Self::Time => "/fapi/v1/time",
            Self::ExchangeInfo => "/fapi/v1/exchangeInfo",
            Self::FundingInfo => "/fapi/v1/fundingInfo",
            Self::Ticker24h { .. } => "/fapi/v1/ticker/24hr",
            Self::PremiumIndex { .. } => "/fapi/v1/premiumIndex",
            Self::Klines { .. } => "/fapi/v1/klines",
            Self::FundingRate => "/fapi/v1/fundingRate",
            Self::OpenInterest => "/fapi/v1/openInterest",
            Self::AggTrades => "/fapi/v1/aggTrades",
            Self::Depth { .. } => "/fapi/v1/depth",
        }
    }

    /// What the request costs, as measured from `X-MBX-USED-WEIGHT-1M` deltas
    /// on 2026-10-07.
    ///
    /// Two rows differ from Binance's page. `klines` bands are inclusive at the
    /// top (a limit of 100 costs 1, 500 costs 2, 1000 costs 5), and omitting
    /// `limit` costs 5 although it returns 500 rows. `depth` without `limit`
    /// costs 1 and returns 500 levels, where an explicit 500 costs 10.
    pub fn cost(self) -> Cost {
        match self {
            Self::Ping | Self::Time | Self::ExchangeInfo | Self::OpenInterest => Cost::Weight(1),
            Self::FundingInfo | Self::FundingRate => Cost::Funding,
            Self::Ticker24h { all } => Cost::Weight(if all { 40 } else { 1 }),
            Self::PremiumIndex { all } => Cost::Weight(if all { 10 } else { 1 }),
            Self::Klines { limit: None } => Cost::Weight(5),
            Self::Klines { limit: Some(limit) } => Cost::Weight(match limit {
                0..=100 => 1,
                101..=500 => 2,
                501..=1000 => 5,
                _ => 10,
            }),
            Self::AggTrades => Cost::Weight(20),
            Self::Depth { limit: None } => Cost::Weight(1),
            Self::Depth { limit: Some(limit) } => Cost::Weight(match limit {
                DepthLimit::Five | DepthLimit::Ten | DepthLimit::Twenty | DepthLimit::Fifty => 2,
                DepthLimit::Hundred => 5,
                DepthLimit::FiveHundred => 10,
                DepthLimit::Thousand => 20,
            }),
        }
    }
}

/// The request-weight budget of one IP, shared by every client that clones it.
///
/// Binance limits weight per IP, not per client, so every
/// [`Usdm`](crate::Usdm) in a process should be built with one budget
/// ([`UsdmBuilder::weight_budget`](crate::UsdmBuilder::weight_budget)).
///
/// - **Window.** The UTC clock minute: the server's count fell to 1 just after
///   each minute boundary on 2026-10-07, where a sliding window would have
///   kept counting.
/// - **Budget.** The published 2400 less a tenth, 2160. A request waits while
///   its weight would take the minute past that.
/// - **Server count.** Every response's `X-MBX-USED-WEIGHT-1M` raises the
///   minute's count to at least what the server reports, because other
///   processes on the same IP spend the same budget. The count never falls
///   within a minute, and a response is applied only to the minute its request
///   was charged in.
/// - **Funding.** `fundingRate` and `fundingInfo` carry no weight and share a
///   documented 500 requests per 5 minutes, paced here at 450 with a depth of
///   one, so they never burst.
/// - **Cooldown.** A `429` or `418` holds every request, on both limits, for
///   the delay the server asked for. A cooldown is only ever extended.
#[derive(Debug, Clone)]
pub struct WeightBudget {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    per_minute: u32,
    funding_interval: Duration,
    clock: Clock,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    /// The UTC minute (milliseconds / 60 000) that `used` counts.
    minute: u64,
    used: u32,
    cooldown_until: Option<Instant>,
    next_funding: Option<Instant>,
}

/// The minute a request was charged in, so its response's header is applied to
/// that minute and no other. `None` for the funding routes, which report no
/// weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Charge {
    minute: Option<u64>,
}

/// Where the current UTC minute comes from.
#[derive(Debug, Clone, Copy)]
enum Clock {
    /// The system clock, which is what Binance's minute follows.
    System,
    /// Wall time derived from tokio's clock, so a paused-time test controls it.
    #[cfg(test)]
    Tokio { start: Instant, start_ms: u64 },
}

impl Clock {
    fn now_ms(self) -> u64 {
        match self {
            Self::System => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_millis() as u64),
            #[cfg(test)]
            Self::Tokio { start, start_ms } => start_ms + start.elapsed().as_millis() as u64,
        }
    }
}

impl Default for WeightBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightBudget {
    /// A budget for Binance's published limits, less the reserve.
    pub fn new() -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::System,
        )
    }

    fn with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self {
        // One slot of the funding target is the bucket's depth; the rest is
        // paced, so no five-minute window admits more than the target.
        let funding_slots = after_reserve(funding_per_five_minutes).max(2) - 1;
        Self {
            inner: Arc::new(Inner {
                per_minute: after_reserve(weight_per_minute),
                funding_interval: FUNDING_PERIOD / funding_slots,
                clock,
                state: Mutex::new(State {
                    minute: 0,
                    used: 0,
                    cooldown_until: None,
                    next_funding: None,
                }),
            }),
        }
    }

    /// Whether two handles charge one budget.
    #[cfg(test)]
    pub(crate) fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute
    }

    /// Weight charged or reported in the current UTC minute.
    pub fn used(&self) -> u32 {
        let minute = self.inner.clock.now_ms() / MINUTE_MS;
        let state = self.lock();
        if state.minute == minute {
            state.used
        } else {
            0
        }
    }

    /// Waits until the request can go, then charges it.
    pub(crate) async fn acquire(&self, cost: Cost) -> Charge {
        match cost {
            Cost::Funding => {
                let slot = self.reserve_funding_slot();
                tokio::time::sleep_until(slot).await;
                self.await_cooldown().await;
                Charge { minute: None }
            }
            Cost::Weight(weight) => loop {
                self.await_cooldown().await;
                match self.try_charge(weight) {
                    Ok(charge) => return charge,
                    Err(until_next_minute) => tokio::time::sleep(until_next_minute).await,
                }
            },
        }
    }

    /// Charges `weight` to the current minute, or says how long until the next.
    ///
    /// An empty minute admits any weight, so a request heavier than the whole
    /// budget is sent alone rather than held forever.
    fn try_charge(&self, weight: u32) -> Result<Charge, Duration> {
        let now_ms = self.inner.clock.now_ms();
        let minute = now_ms / MINUTE_MS;
        let mut state = self.lock();
        if state.minute != minute {
            state.minute = minute;
            state.used = 0;
        }
        if state.used == 0 || state.used.saturating_add(weight) <= self.inner.per_minute {
            state.used = state.used.saturating_add(weight);
            Ok(Charge {
                minute: Some(minute),
            })
        } else {
            Err(Duration::from_millis(MINUTE_MS - now_ms % MINUTE_MS))
        }
    }

    /// Raises the minute's count to what the server reported for it.
    pub(crate) fn record_used(&self, charge: Charge, used: u32) {
        let Some(minute) = charge.minute else { return };
        let mut state = self.lock();
        if state.minute == minute && used > state.used {
            state.used = used;
        }
    }

    /// Holds every request for `delay`. Extends a cooldown in force, never
    /// shortens one: concurrent requests see the same `429` milliseconds
    /// apart, and taking the latest delay would release them all early.
    pub(crate) fn begin_cooldown(&self, delay: Duration) {
        let until = Instant::now() + delay.min(MAX_COOLDOWN);
        let mut state = self.lock();
        if state.cooldown_until.is_none_or(|current| until > current) {
            state.cooldown_until = Some(until);
        }
    }

    async fn await_cooldown(&self) {
        loop {
            // Copy the deadline out so the guard is not held across the await.
            let until = self.lock().cooldown_until;
            match until {
                // Loop rather than return: another response may extend the
                // cooldown while this one sleeps.
                Some(until) if until > Instant::now() => tokio::time::sleep_until(until).await,
                _ => return,
            }
        }
    }

    fn reserve_funding_slot(&self) -> Instant {
        let now = Instant::now();
        let mut state = self.lock();
        let slot = state.next_funding.map_or(now, |next| next.max(now));
        state.next_funding = Some(slot + self.inner.funding_interval);
        slot
    }

    /// A poison-tolerant lock: a panic elsewhere must not turn the budget into
    /// a permanent outage.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
```

Notes for the reviewer:
- `Clock::Tokio` exists only under `cfg(test)`. It derives wall time from tokio's paused clock, so a test can start exactly on a minute boundary and assert a 45-second wait without waiting.
- The funding bucket's depth of one is paid for out of its target, as in core's `quota()`: 449 slots plus one of depth is 450 in any five minutes.
- A response's header is applied only if its request was charged in the current minute. Otherwise a request in flight across a boundary would carry the old minute's count into the new one, and could hold it for a whole minute.

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --lib`
Expected: PASS, 21 tests. Dead-code warnings are expected until Task 6 calls the budget: `Charge` is never constructed, `funding_interval`, `cooldown_until` and `next_funding` are never read, and `acquire`, `try_charge`, `record_used`, `begin_cooldown`, `await_cooldown` and `reserve_funding_slot` are never used.

- [ ] **Step 5: Show the tests can fail**

Break each rule, run `cargo test -j 4 -p polyoxide-binance --lib weight`, see the named test fail, then restore the line:

| Change in `weight.rs` | Test that must fail |
|---|---|
| `if state.minute == minute && used > state.used {` → `if used > state.used {` | `a_header_is_applied_only_to_the_minute_its_request_was_charged_in` |
| `if state.cooldown_until.is_none_or(\|current\| until > current) {` → `if true {` | `a_cooldown_holds_both_limits_and_is_only_extended` |
| `let funding_slots = after_reserve(funding_per_five_minutes).max(2) - 1;` → `let funding_slots = after_reserve(funding_per_five_minutes);` | `the_funding_bucket_admits_450_per_five_minutes`, `the_funding_bucket_never_bursts` |
| `published - published.div_ceil(RESERVED_FRACTION)` → `published` | `the_budget_aims_a_tenth_below_the_published_limit` and four more |

Run `git diff --stat polyoxide-binance/src/weight.rs` afterwards; the file must be new and unmodified from Step 3.

Code review added a follow-up commit (2442708) with three changes and three tests. Funding requests wait out a cooldown before taking a slot, and requeue if one begins while they wait, so a cooldown no longer releases them all at once. A cooldown extended mid-wait is now tested. The counted minute only moves forward, so a stale read or a clock stepped back cannot reset the count; a test-only `Clock::Manual` steps the clock back. The doc states the in-flight invariant the header rule relies on. A second follow-up (a821aeb) tests a cooldown that begins during a funding slot wait, and a stale charge's header. The crate then has 25 unit tests, and later counts include them. At this commit the doc gate still fails, on links to `crate::Usdm` and `crate::UsdmBuilder::weight_budget`; Task 6 resolves them.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/src/weight.rs polyoxide-binance/src/lib.rs
git commit -m "feat(binance): the measured weight table and WeightBudget

Route::cost is the table measured from X-MBX-USED-WEIGHT-1M deltas on
2026-10-07: klines bands are inclusive at the top and cost 5 without a
limit, depth costs 1 without one. WeightBudget charges the UTC minute at
2160 of the published 2400, follows the server's count only for the
minute a request was charged in, paces the funding routes at 450 per 5
minutes with a depth of one, and holds every request through a 429 or
418 cooldown that is only ever extended.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 5: The error type

**Files:**
- Create: `polyoxide-binance/src/error.rs`
- Modify: `polyoxide-binance/src/lib.rs`

- [ ] **Step 1: Register the module**

Replace `polyoxide-binance/src/lib.rs` with:

```rust
//! Rust client for Binance USDⓈ-M futures public market data
//! (`fapi.binance.com`).

pub mod error;
pub mod usdm;
pub mod weight;

pub use error::BinanceError;
pub use weight::WeightBudget;
```

- [ ] **Step 2: Write the tests**

Create `polyoxide-binance/src/error.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_symbol_is_a_venue_error_with_its_code() {
        // Captured 2026-10-07 from `GET /fapi/v1/ticker/24hr?symbol=NOTASYMBOLUSDT`.
        let err = BinanceError::from_response_parts(
            400,
            None,
            r#"{"code":-1121,"msg":"Invalid symbol."}"#,
        );
        assert!(
            matches!(&err, BinanceError::Venue { status: 400, code: -1121, msg } if msg == "Invalid symbol.")
        );
        assert_eq!(err.code(), Some(-1121));
        assert!(!err.is_retriable());
    }

    #[test]
    fn statuses_that_decide_the_caller_s_next_step_win_over_the_body() {
        // A 429 carries a {code, msg} body too; it must still be a rate limit,
        // not a venue error a retry policy would treat as permanent.
        let body = r#"{"code":-1003,"msg":"Too many requests."}"#;
        let limited = BinanceError::from_response_parts(429, Some("7"), body);
        assert!(matches!(limited, BinanceError::RateLimited { .. }));
        assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
        assert!(limited.is_retriable());

        let banned = BinanceError::from_response_parts(418, Some("120"), body);
        assert!(matches!(banned, BinanceError::IpBanned { .. }));
        assert_eq!(banned.retry_after(), Some(Duration::from_secs(120)));
        assert!(!banned.is_retriable());

        let region = BinanceError::from_response_parts(451, None, body);
        assert!(matches!(&region, BinanceError::RegionBlocked { msg } if msg.contains("-1003")));
        assert!(!region.is_retriable());

        let firewall = BinanceError::from_response_parts(403, None, "<html>denied</html>");
        assert!(
            matches!(&firewall, BinanceError::Forbidden { msg } if msg == "<html>denied</html>")
        );
        assert!(!firewall.is_retriable());
    }

    #[test]
    fn a_5xx_venue_error_is_retriable_and_a_4xx_is_not() {
        let body = r#"{"code":-1001,"msg":"Internal error; unable to process your request."}"#;
        assert!(BinanceError::from_response_parts(503, None, body).is_retriable());
        assert!(!BinanceError::from_response_parts(400, None, body).is_retriable());
    }

    #[test]
    fn a_body_in_another_shape_is_left_to_core() {
        let err = BinanceError::from_response_parts(502, None, "<html>bad gateway</html>");
        assert!(matches!(
            err,
            BinanceError::Api(ApiError::Api { status: 502, .. })
        ));
        assert!(err.is_retriable());
        assert_eq!(err.code(), None);
    }

    #[test]
    fn bodies_are_clipped_before_they_are_kept() {
        let body = "x".repeat(10_000);
        let BinanceError::Forbidden { msg } = BinanceError::from_response_parts(403, None, &body)
        else {
            panic!("a 403 is Forbidden");
        };
        assert!(msg.len() < 600, "kept {} bytes", msg.len());
    }

    #[test]
    fn retry_after_reads_seconds_and_rejects_what_is_not_a_wait() {
        assert_eq!(retry_after_secs(Some("2")), Some(Duration::from_secs(2)));
        assert_eq!(
            retry_after_secs(Some(" 1.5 ")),
            Some(Duration::from_millis(1500))
        );
        for junk in ["0", "-3", "soon", "NaN", "inf"] {
            assert_eq!(retry_after_secs(Some(junk)), None, "{junk:?}");
        }
        assert_eq!(retry_after_secs(None), None);
        // A week is longer than any documented ban; it is clamped, not trusted.
        assert_eq!(retry_after_secs(Some("604800")), Some(MAX_COOLDOWN));
    }

    #[test]
    fn binance_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BinanceError>();
    }
}
```

Run: `cargo test -j 4 -p polyoxide-binance --lib error`
Expected: FAIL to compile: ``cannot find type `BinanceError` in this scope``.

- [ ] **Step 3: Write the type**

Put this above the test module:

```rust
//! Error types for the Binance API.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError};
use serde::Deserialize;
use thiserror::Error;

use crate::weight::MAX_COOLDOWN;

/// Error type for Binance API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum BinanceError {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// Binance refused the request with its `{"code", "msg"}` body: an unknown
    /// symbol is `400` with code `-1121`.
    #[error("binance answered {status}: {code} {msg}")]
    Venue {
        /// HTTP status.
        status: u16,
        /// Binance's error code, always negative.
        code: i64,
        /// Binance's message, clipped to 512 bytes.
        msg: String,
    },

    /// Still answered `429` after the retry schedule ran out.
    #[error("binance rate limit (429), retry after {retry_after:?}")]
    RateLimited {
        /// The response's `Retry-After`.
        retry_after: Option<Duration>,
    },

    /// `418`: Binance has banned this IP for continuing after a `429`. Every
    /// request on the same [`WeightBudget`](crate::WeightBudget) waits until the
    /// ban lifts.
    #[error("binance has banned this IP (418), retry after {retry_after:?}")]
    IpBanned {
        /// The response's `Retry-After`. The docs give bans of 2 minutes to 3 days.
        retry_after: Option<Duration>,
    },

    /// `451`: Binance does not serve the caller's location.
    #[error("binance does not serve this location (451): {msg}")]
    RegionBlocked {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },

    /// `403`: Binance's web application firewall refused the request.
    #[error("binance's firewall refused the request (403): {msg}")]
    Forbidden {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
}

#[derive(Deserialize)]
struct VenueBody {
    code: i64,
    msg: String,
}

impl BinanceError {
    /// Classifies an unsuccessful response.
    ///
    /// `418`, `429`, `451` and `403` go by status alone: their bodies were not
    /// observed from a test IP, and the status is what decides what a caller
    /// should do. Anything else with Binance's `{code, msg}` body is
    /// [`Venue`](Self::Venue); any other body is left to core.
    pub(crate) fn from_response_parts(status: u16, retry_after: Option<&str>, body: &str) -> Self {
        let retry_after = retry_after_secs(retry_after);
        match status {
            418 => Self::IpBanned { retry_after },
            429 => Self::RateLimited { retry_after },
            451 => Self::RegionBlocked { msg: clip(body) },
            403 => Self::Forbidden { msg: clip(body) },
            _ => match serde_json::from_str::<VenueBody>(body) {
                Ok(venue) => Self::Venue {
                    status,
                    code: venue.code,
                    msg: clip(&venue.msg),
                },
                Err(_) => Self::Api(ApiError::from_status_and_body(status, &clip(body))),
            },
        }
    }

    /// Whether re-sending the same request could plausibly succeed.
    ///
    /// A `429` and a 5xx are; a ban, a region block and a firewall refusal are
    /// not, because sending again does not change them, and sending after a
    /// `418` lengthens the ban.
    pub fn is_retriable(&self) -> bool {
        match self {
            Self::Api(err) => err.is_retriable(),
            Self::Venue { status, .. } => *status >= 500,
            Self::RateLimited { .. } => true,
            Self::IpBanned { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => false,
        }
    }

    /// Binance's error code, for a [`Venue`](Self::Venue) error.
    pub fn code(&self) -> Option<i64> {
        match self {
            Self::Venue { code, .. } => Some(*code),
            _ => None,
        }
    }

    /// The `Retry-After` delay, for a `429` or a `418` that carried one.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimited { retry_after } | Self::IpBanned { retry_after } => *retry_after,
            _ => None,
        }
    }
}

/// Parses `Retry-After` as seconds, whole or fractional. Zero, negative and
/// unparsable values are `None`; anything past [`MAX_COOLDOWN`] is clamped to it.
pub(crate) fn retry_after_secs(value: Option<&str>) -> Option<Duration> {
    let secs = value?.trim().parse::<f64>().ok()?;
    if !secs.is_finite() || secs <= 0.0 {
        return None;
    }
    Some(Duration::from_secs_f64(
        secs.min(MAX_COOLDOWN.as_secs_f64()),
    ))
}

fn clip(text: &str) -> String {
    truncate_for_log(text).into_owned()
}

polyoxide_core::impl_api_error_conversions!(BinanceError);
```

Notes for the reviewer:
- `418`, `429`, `451` and `403` are classified by status before the body is looked at. A `429` carries a `{code, msg}` body too, and it must still be a rate limit, not a venue error a retry policy would treat as permanent.
- Every variant can be built and matched outside the crate; only the enum is `#[non_exhaustive]`. prader-rs constructs them in its tests.

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --lib`
Expected: PASS, 32 tests. `never used` warnings for `from_response_parts` and `retry_after_secs` are expected until Task 6.

Code review added a follow-up commit (e08683a). Core reads a non-Binance body whole before its message is clipped. A `Venue` error with 408 or 425 is retriable, as core says for those statuses, and a test pins the agreement. A `Retry-After` that rounds to zero is `None`. Tests cover every clip site, a 403 with Binance's body, and the 500 boundary. The crate then has 34 unit tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/src/error.rs polyoxide-binance/src/lib.rs
git commit -m "feat(binance): BinanceError

Venue for Binance's {code, msg} bodies; RateLimited, IpBanned,
RegionBlocked and Forbidden decided by status, since the status is what
tells a caller what to do next. Retry-After reads whole or fractional
seconds and is clamped to the longest documented ban.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 6: The client, the send loop and the namespaces

**Files:**
- Create: `polyoxide-binance/tests/mock_api.rs`
- Create: `polyoxide-binance/src/usdm/request.rs`
- Replace: `polyoxide-binance/src/usdm/mod.rs`
- Create: `polyoxide-binance/src/usdm/api/mod.rs`, `health.rs`, `exchange.rs`, `market.rs`
- Replace: `polyoxide-binance/src/lib.rs`
- Create: `polyoxide-binance/README.md`

- [ ] **Step 1: Write the mock tests**

`polyoxide-binance/tests/mock_api.rs`:

```rust
//! Mock-server tests: every route's path and exact query, the decoding of a
//! captured body, error mapping, and the budget's response to the server.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::{
    usdm::types::{DepthLimit, Interval, Symbol},
    weight::Cost,
    BinanceError, Usdm,
};

fn usdm(server: &ServerGuard) -> Usdm {
    Usdm::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/rest/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

/// A route answering `body`, matched on its whole query string so a missing
/// or extra parameter fails the request.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .create_async()
        .await
}

/// Waits out the end of a minute, so a test that holds the budget for a moment
/// cannot see the window roll over underneath it.
async fn clear_of_a_minute_boundary() {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        % 60_000;
    if ms > 57_000 {
        tokio::time::sleep(Duration::from_millis(60_100 - ms)).await;
    }
}

// ── health and exchange ─────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/fapi/v1/ping", "", "{}").await;
    let latency = usdm(&server).health().ping().await.unwrap();
    mock.assert_async().await;
    assert!(latency < Duration::from_secs(5));
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/fapi/v1/time", "", &fixture("time")).await;
    let time = usdm(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert!(time.server_time > 1_790_000_000_000);
}

#[tokio::test]
async fn exchange_info_decodes_the_captured_body() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/exchangeInfo",
        "",
        &fixture("exchange_info"),
    )
    .await;
    let info = usdm(&server)
        .exchange()
        .exchange_info()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(info.futures_type, "U_MARGINED");
    assert_eq!(info.symbols.len(), 5);
}

#[tokio::test]
async fn funding_info_decodes_a_null_update_time() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/fundingInfo",
        "",
        &fixture("funding_info"),
    )
    .await;
    let rows = usdm(&server)
        .exchange()
        .funding_info()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert!(rows.iter().any(|row| row.update_time.is_none()));
}

// ── market ──────────────────────────────────────────────────────

#[tokio::test]
async fn ticker_24h_sends_the_symbol_and_tickers_24h_sends_nothing() {
    let mut server = Server::new_async().await;
    let all = fixture("ticker_24hr");
    let rows: Vec<serde_json::Value> = serde_json::from_str(&all).unwrap();
    let one = rows
        .iter()
        .find(|row| row["symbol"] == "BTCUSDT")
        .unwrap()
        .to_string();
    let single = route(&mut server, "/fapi/v1/ticker/24hr", "symbol=BTCUSDT", &one).await;
    let every = route(&mut server, "/fapi/v1/ticker/24hr", "", &all).await;

    let client = usdm(&server);
    let ticker = client.market().ticker_24h(&btc()).send().await.unwrap();
    assert_eq!(ticker.symbol, "BTCUSDT");
    let tickers = client.market().tickers_24h().send().await.unwrap();
    assert_eq!(tickers.len(), 3);
    single.assert_async().await;
    every.assert_async().await;
}

#[tokio::test]
async fn a_chinese_character_symbol_is_percent_encoded() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "symbol=%E5%B8%81%E5%AE%89%E4%BA%BA%E7%94%9FUSDT",
        r#"{"symbol":"币安人生USDT","markPrice":"0.48683834","indexPrice":"0.48711513","estimatedSettlePrice":"0.48795216","lastFundingRate":"0.00005000","interestRate":"0.00005000","nextFundingTime":1791360000000,"time":1791354113000}"#,
    )
    .await;
    let symbol = Symbol::new("币安人生USDT").unwrap();
    let index = usdm(&server)
        .market()
        .premium_index(&symbol)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(index.symbol, symbol.as_str());
}

#[tokio::test]
async fn premium_indices_sends_no_symbol() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/premiumIndex",
        "",
        &fixture("premium_index"),
    )
    .await;
    let rows = usdm(&server)
        .market()
        .premium_indices()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
async fn klines_send_every_parameter_once() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/klines",
        "symbol=BTCUSDT&interval=1h&startTime=1791350000000&endTime=1791354000000&limit=24",
        &fixture("klines"),
    )
    .await;
    let candles = usdm(&server)
        .market()
        .klines(&btc(), Interval::H1)
        .start_time(1_791_350_000_000)
        .end_time(1_791_354_000_000)
        .limit(100)
        .limit(24)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(candles.len(), 2);
}

#[tokio::test]
async fn funding_rate_sends_its_filters_and_decodes_old_rows() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/fundingRate",
        "symbol=BTCUSDT&startTime=1568102400000&limit=2",
        &fixture("funding_rate_2019"),
    )
    .await;
    let rows = usdm(&server)
        .market()
        .funding_rate()
        .symbol(&btc())
        .start_time(1_568_102_400_000)
        .limit(2)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert!(rows.iter().all(|row| row.mark_price.is_none()));
}

#[tokio::test]
async fn open_interest_sends_the_symbol() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/openInterest",
        "symbol=BTCUSDT",
        &fixture("open_interest"),
    )
    .await;
    let oi = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(oi.symbol, "BTCUSDT");
}

#[tokio::test]
async fn agg_trades_send_their_filters() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/fapi/v1/aggTrades",
        "symbol=BTCUSDT&fromId=3477383749&limit=3",
        &fixture("agg_trades"),
    )
    .await;
    let trades = usdm(&server)
        .market()
        .agg_trades(&btc())
        .from_id(3_477_383_749)
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(trades.len(), 3);
}

#[tokio::test]
async fn depth_sends_a_limit_only_when_asked() {
    let mut server = Server::new_async().await;
    let limited = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT&limit=5",
        &fixture("depth"),
    )
    .await;
    let bare = route(
        &mut server,
        "/fapi/v1/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
    )
    .await;
    let client = usdm(&server);
    let book = client
        .market()
        .depth(&btc())
        .limit(DepthLimit::Five)
        .send()
        .await
        .unwrap();
    assert_eq!(book.bids.len(), 5);
    client.market().depth(&btc()).send().await.unwrap();
    limited.assert_async().await;
    bare.assert_async().await;
}

#[test]
fn each_builder_charges_its_route() {
    let usdm = Usdm::new().unwrap();
    let market = usdm.market();
    assert_eq!(market.klines(&btc(), Interval::M1).cost(), Cost::Weight(5));
    assert_eq!(
        market.klines(&btc(), Interval::M1).limit(1000).cost(),
        Cost::Weight(5)
    );
    assert_eq!(
        market.klines(&btc(), Interval::M1).limit(1001).cost(),
        Cost::Weight(10)
    );
    assert_eq!(market.depth(&btc()).cost(), Cost::Weight(1));
    assert_eq!(
        market.depth(&btc()).limit(DepthLimit::Thousand).cost(),
        Cost::Weight(20)
    );
    assert_eq!(market.agg_trades(&btc()).limit(1).cost(), Cost::Weight(20));
    assert_eq!(market.tickers_24h().cost(), Cost::Weight(40));
    assert_eq!(market.ticker_24h(&btc()).cost(), Cost::Weight(1));
    assert_eq!(market.premium_indices().cost(), Cost::Weight(10));
    assert_eq!(market.funding_rate().cost(), Cost::Funding);
    assert_eq!(usdm.exchange().funding_info().cost(), Cost::Funding);
}

// ── errors ──────────────────────────────────────────────────────

async fn failing(
    server: &mut ServerGuard,
    status: usize,
    headers: &[(&str, &str)],
    body: &str,
) -> Mock {
    let mut mock = server
        .mock("GET", "/fapi/v1/openInterest")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    for (name, value) in headers {
        mock = mock.with_header(*name, value);
    }
    mock.create_async().await
}

#[tokio::test]
async fn a_code_and_msg_body_is_a_venue_error() {
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        400,
        &[],
        r#"{"code":-1121,"msg":"Invalid symbol."}"#,
    )
    .await;
    let err = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            BinanceError::Venue {
                status: 400,
                code: -1121,
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_451_is_region_blocked_and_a_403_forbidden() {
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        451,
        &[],
        "Service unavailable from a restricted location",
    )
    .await;
    let err = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, BinanceError::RegionBlocked { .. }), "{err:?}");

    let mut server = Server::new_async().await;
    let _mock = failing(&mut server, 403, &[], "<html>Request blocked</html>").await;
    let err = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, BinanceError::Forbidden { .. }), "{err:?}");
}

#[tokio::test]
async fn a_418_is_not_retried_and_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let banned = failing(
        &mut server,
        418,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"banned"}"#,
    )
    .await
    .expect(1);
    let client = usdm(&server);
    let err = client
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(err, BinanceError::IpBanned { retry_after: Some(d) } if d == Duration::from_secs(1))
    );
    banned.assert_async().await;

    let start = Instant::now();
    let _ = client.market().open_interest(&btc()).send().await;
    assert!(
        start.elapsed() >= Duration::from_millis(900),
        "{:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn a_429_cools_down_then_succeeds() {
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = route(
        &mut server,
        "/fapi/v1/openInterest",
        "symbol=BTCUSDT",
        &fixture("open_interest"),
    )
    .await;

    let start = Instant::now();
    let oi = usdm(&server)
        .market()
        .open_interest(&btc())
        .send()
        .await
        .unwrap();
    assert_eq!(oi.symbol, "BTCUSDT");
    assert!(
        start.elapsed() >= Duration::from_millis(900),
        "{:?}",
        start.elapsed()
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn a_high_used_weight_header_holds_the_next_request() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/fapi/v1/openInterest")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "2160")
        .with_body(fixture("open_interest"))
        .create_async()
        .await;
    let client = usdm(&server);
    client.market().open_interest(&btc()).send().await.unwrap();
    assert_eq!(
        client.weight_budget().used(),
        2160,
        "another process spent the minute"
    );

    let held = tokio::time::timeout(
        Duration::from_millis(500),
        client.market().open_interest(&btc()).send(),
    )
    .await;
    assert!(
        held.is_err(),
        "the next request must wait for the next minute"
    );
}

#[tokio::test]
async fn a_gzip_body_is_requested_and_decoded() {
    // `{"serverTime":1}` compressed by python3's `gzip.compress(.., mtime=0)`.
    const GZIPPED: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0x2a, 0x4e, 0x2d,
        0x2a, 0x4b, 0x2d, 0x0a, 0xc9, 0xcc, 0x4d, 0x55, 0xb2, 0x32, 0xac, 0x05, 0x00, 0xe2, 0x1d,
        0x3e, 0x1a, 0x10, 0x00, 0x00, 0x00,
    ];
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/fapi/v1/time")
        .match_header("accept-encoding", Matcher::Regex("gzip".into()))
        .with_header("content-encoding", "gzip")
        .with_body(GZIPPED)
        .create_async()
        .await;
    let time = usdm(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert_eq!(time.server_time, 1);
}
```

Run: `cargo test -j 4 -p polyoxide-binance --test mock_api`
Expected: FAIL to compile: ``unresolved import `polyoxide_binance::Usdm` ``.

- [ ] **Step 2: Write the send loop**

`polyoxide-binance/src/usdm/request.rs`:

```rust
//! The send loop every route goes through.

use std::marker::PhantomData;

use polyoxide_core::{retry_after_header, truncate_for_log, ApiError, HttpClient};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::{
    error::{retry_after_secs, BinanceError},
    weight::{Cost, Route, WeightBudget, DEFAULT_BAN},
};

/// The header carrying the IP's weight used in the current minute.
pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";

/// A REST request that knows its weight. Call [`send`](Self::send).
#[must_use = "a request does nothing until it is sent"]
pub struct WeightedRequest<T> {
    http: HttpClient,
    budget: WeightBudget,
    route: Route,
    query: Vec<(&'static str, String)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> WeightedRequest<T> {
    pub(crate) fn new(http: &HttpClient, budget: &WeightBudget, route: Route) -> Self {
        Self {
            http: http.clone(),
            budget: budget.clone(),
            route,
            query: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a query parameter, replacing an earlier value for the same key.
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Replaces the route, for a parameter that changes the weight.
    pub(crate) fn route(mut self, route: Route) -> Self {
        self.route = route;
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.route.cost()
    }
}

impl<T: DeserializeOwned> WeightedRequest<T> {
    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` starts a cooldown every request on the budget waits out, then
    /// this request is retried on core's schedule. A `418` starts a cooldown
    /// for its `Retry-After`, or 2 minutes, and is not retried.
    pub async fn send(self) -> Result<T, BinanceError> {
        let text = self.send_text().await?;
        serde_json::from_str(&text).map_err(|err| {
            tracing::error!(
                "Failed to decode {}: {err}: {}",
                self.route.path(),
                truncate_for_log(&text)
            );
            BinanceError::from(ApiError::from(err))
        })
    }

    async fn send_text(&self) -> Result<String, BinanceError> {
        let path = self.route.path();
        let url = self.http.base_url.join(path)?;
        let cost = self.route.cost();
        let mut attempt = 0u32;

        loop {
            let permit = self.http.acquire_concurrency().await;
            let charge = self.budget.acquire(cost).await;

            let mut request = self.http.client.get(url.clone());
            if !self.query.is_empty() {
                request = request.query(&self.query);
            }
            let response = request.send().await?;
            let status = response.status();
            let retry_after = retry_after_header(&response);

            // Before anything else, and whatever the status: a refused request
            // still spent weight, and the server's count is the truth.
            if let Some(used) = used_weight(&response) {
                self.budget.record_used(charge, used);
            }

            if status == StatusCode::IM_A_TEAPOT {
                let ban = retry_after_secs(retry_after.as_deref()).unwrap_or(DEFAULT_BAN);
                tracing::warn!("418 on {path}: IP banned, every request held {ban:?}");
                self.budget.begin_cooldown(ban);
            } else if status == StatusCode::TOO_MANY_REQUESTS {
                // Retry-After only ever extends the wait, as in core.
                let retry = self
                    .http
                    .should_retry(status, attempt, retry_after.as_deref());
                let asked = retry_after_secs(retry_after.as_deref()).unwrap_or_default();
                let cooldown = retry.unwrap_or_default().max(asked);
                self.budget.begin_cooldown(cooldown);
                if retry.is_some() {
                    attempt += 1;
                    tracing::warn!("429 on {path}, retry {attempt} after {cooldown:?}");
                    drop(permit);
                    continue;
                }
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(BinanceError::from_response_parts(
                    status.as_u16(),
                    retry_after.as_deref(),
                    &body,
                ));
            }
            return Ok(response.text().await?);
        }
    }
}

fn used_weight(response: &Response) -> Option<u32> {
    response
        .headers()
        .get(USED_WEIGHT_HEADER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}
```

Notes for the reviewer:
- The used-weight header is recorded before the status is looked at: a refused request still spent weight.
- On a `429` the cooldown is the larger of core's backoff for this attempt and `Retry-After`, so `Retry-After` only ever extends the wait, as in core. The retry then waits in `acquire`, behind the same cooldown as every other request.
- On a `418` nothing is retried. The cooldown is `Retry-After`, or two minutes, the shortest documented ban.

- [ ] **Step 3: Write the client**

Replace `polyoxide-binance/src/usdm/mod.rs` with:

```rust
//! Binance USDⓈ-M futures on `fapi.binance.com`.

pub mod api;
pub mod request;
pub mod types;

use polyoxide_core::{
    HttpClient, HttpClientBuilder, RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS,
};

use crate::{
    error::BinanceError,
    usdm::api::{exchange::ExchangeApi, health::Health, market::MarketApi},
    weight::WeightBudget,
};

pub use request::WeightedRequest;

/// Production USDⓈ-M futures REST host.
pub const DEFAULT_BASE_URL: &str = "https://fapi.binance.com";

/// In-flight requests the client allows by default, as in the sibling crates.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for USDⓈ-M futures public market data. No credentials are needed.
#[derive(Debug, Clone)]
pub struct Usdm {
    http: HttpClient,
    budget: WeightBudget,
}

impl Usdm {
    /// A client with default settings and its own [`WeightBudget`].
    pub fn new() -> Result<Self, BinanceError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> UsdmBuilder {
        UsdmBuilder::new()
    }

    /// Liveness: `ping`, `time`.
    pub fn health(&self) -> Health {
        Health {
            http: self.http.clone(),
            budget: self.budget.clone(),
        }
    }

    /// Reference data: `exchangeInfo`, `fundingInfo`.
    pub fn exchange(&self) -> ExchangeApi {
        ExchangeApi {
            http: self.http.clone(),
            budget: self.budget.clone(),
        }
    }

    /// Market data: tickers, premium index, klines, funding, open interest,
    /// trades and depth.
    pub fn market(&self) -> MarketApi {
        MarketApi {
            http: self.http.clone(),
            budget: self.budget.clone(),
        }
    }

    /// The budget this client charges.
    pub fn weight_budget(&self) -> &WeightBudget {
        &self.budget
    }
}

/// Builder for [`Usdm`].
#[derive(Debug)]
pub struct UsdmBuilder {
    base_url: String,
    timeout_ms: u64,
    pool_size: usize,
    retry_config: Option<RetryConfig>,
    max_concurrent: Option<usize>,
    budget: Option<WeightBudget>,
}

impl UsdmBuilder {
    fn new() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_owned(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            pool_size: DEFAULT_POOL_SIZE,
            retry_config: None,
            max_concurrent: None,
            budget: None,
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

    /// Replace the retry policy for `429` responses.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = Some(config);
        self
    }

    /// Maximum in-flight requests (default 4).
    pub fn max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Charge this budget instead of a new one. Binance limits weight per IP,
    /// so every client in a process should share one budget.
    pub fn weight_budget(mut self, budget: WeightBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Build the client.
    ///
    /// It asks for gzip, since `exchangeInfo` is 1.15 MB raw and 51 KB
    /// compressed, and has no core `RateLimiter`: the [`WeightBudget`] paces
    /// every request instead.
    pub fn build(self) -> Result<Usdm, BinanceError> {
        let mut builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(DEFAULT_MAX_CONCURRENT))
            .gzip(true);
        if let Some(config) = self.retry_config {
            builder = builder.with_retry_config(config);
        }
        Ok(Usdm {
            http: builder.build()?,
            budget: self.budget.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_targets_the_production_host() {
        let usdm = Usdm::new().expect("client builds");
        assert_eq!(usdm.http.base_url.as_str(), "https://fapi.binance.com/");
    }

    #[test]
    fn a_bad_base_url_is_a_url_error() {
        let err = Usdm::builder().base_url("not a url").build().unwrap_err();
        assert!(matches!(
            err,
            BinanceError::Api(polyoxide_core::ApiError::Url(_))
        ));
    }

    #[test]
    fn clients_given_one_budget_share_it() {
        let budget = WeightBudget::new();
        let a = Usdm::builder()
            .weight_budget(budget.clone())
            .build()
            .unwrap();
        let b = Usdm::builder().weight_budget(budget).build().unwrap();
        assert!(a.weight_budget().is_shared_with(b.weight_budget()));
        let c = Usdm::new().unwrap();
        assert!(!a.weight_budget().is_shared_with(c.weight_budget()));
    }
}
```

- [ ] **Step 4: Write the namespaces**

`polyoxide-binance/src/usdm/api/mod.rs`:

```rust
//! API namespaces, one module per group of routes.

pub mod exchange;
pub mod health;
pub mod market;
```

`polyoxide-binance/src/usdm/api/health.rs`:

```rust
//! Liveness routes: `/fapi/v1/ping` and `/fapi/v1/time`.

use std::time::{Duration, Instant};

use polyoxide_core::HttpClient;
use serde::Deserialize;

use crate::{
    error::BinanceError,
    usdm::{request::WeightedRequest, types::ServerTime},
    weight::{Route, WeightBudget},
};

/// Health namespace.
#[derive(Debug, Clone)]
pub struct Health {
    pub(crate) http: HttpClient,
    pub(crate) budget: WeightBudget,
}

/// `ping`'s body, `{}`.
#[derive(Deserialize)]
struct Empty {}

impl Health {
    /// Round-trip time to the host, via `GET /fapi/v1/ping` (weight 1).
    ///
    /// Includes any wait for the budget, as every route's latency does.
    pub async fn ping(&self) -> Result<Duration, BinanceError> {
        let start = Instant::now();
        WeightedRequest::<Empty>::new(&self.http, &self.budget, Route::Ping)
            .send()
            .await?;
        Ok(start.elapsed())
    }

    /// Server time, via `GET /fapi/v1/time` (weight 1).
    pub fn time(&self) -> WeightedRequest<ServerTime> {
        WeightedRequest::new(&self.http, &self.budget, Route::Time)
    }
}
```

`polyoxide-binance/src/usdm/api/exchange.rs`:

```rust
//! Reference data: `/fapi/v1/exchangeInfo` and `/fapi/v1/fundingInfo`.

use polyoxide_core::HttpClient;

use crate::{
    usdm::{
        request::WeightedRequest,
        types::{ExchangeInfo, FundingInfo},
    },
    weight::{Route, WeightBudget},
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) http: HttpClient,
    pub(crate) budget: WeightBudget,
}

impl ExchangeApi {
    /// `GET /fapi/v1/exchangeInfo` (weight 1): every contract, its filters, and
    /// the IP's limits.
    pub fn exchange_info(&self) -> WeightedRequest<ExchangeInfo> {
        WeightedRequest::new(&self.http, &self.budget, Route::ExchangeInfo)
    }

    /// `GET /fapi/v1/fundingInfo`: the symbols whose funding cap, floor or
    /// interval was adjusted. Paced by the funding limit, not by weight.
    pub fn funding_info(&self) -> WeightedRequest<Vec<FundingInfo>> {
        WeightedRequest::new(&self.http, &self.budget, Route::FundingInfo)
    }
}
```

`polyoxide-binance/src/usdm/api/market.rs`:

```rust
//! Market data: tickers, premium index, klines, funding history, open
//! interest, aggregate trades and depth.

use polyoxide_core::HttpClient;

use crate::{
    error::BinanceError,
    usdm::{
        request::WeightedRequest,
        types::{
            AggTrade, Depth, DepthLimit, FundingRate, Interval, Kline, OpenInterest, PremiumIndex,
            Symbol, Ticker24h,
        },
    },
    weight::{Cost, Route, WeightBudget},
};

/// Market namespace.
#[derive(Debug, Clone)]
pub struct MarketApi {
    pub(crate) http: HttpClient,
    pub(crate) budget: WeightBudget,
}

impl MarketApi {
    fn request<T>(&self, route: Route) -> WeightedRequest<T> {
        WeightedRequest::new(&self.http, &self.budget, route)
    }

    /// `GET /fapi/v1/ticker/24hr` for one symbol (weight 1).
    pub fn ticker_24h(&self, symbol: &Symbol) -> WeightedRequest<Ticker24h> {
        self.request(Route::Ticker24h { all: false })
            .query("symbol", symbol)
    }

    /// `GET /fapi/v1/ticker/24hr` for every trading symbol (weight 40).
    pub fn tickers_24h(&self) -> WeightedRequest<Vec<Ticker24h>> {
        self.request(Route::Ticker24h { all: true })
    }

    /// `GET /fapi/v1/premiumIndex` for one symbol (weight 1): mark price,
    /// index price and funding.
    pub fn premium_index(&self, symbol: &Symbol) -> WeightedRequest<PremiumIndex> {
        self.request(Route::PremiumIndex { all: false })
            .query("symbol", symbol)
    }

    /// `GET /fapi/v1/premiumIndex` for every symbol (weight 10).
    pub fn premium_indices(&self) -> WeightedRequest<Vec<PremiumIndex>> {
        self.request(Route::PremiumIndex { all: true })
    }

    /// `GET /fapi/v1/klines`: candles, newest last. Weight 5 without a limit,
    /// otherwise by limit (see [`Route::cost`]).
    pub fn klines(&self, symbol: &Symbol, interval: Interval) -> GetKlines {
        GetKlines {
            request: self
                .request(Route::Klines { limit: None })
                .query("symbol", symbol)
                .query("interval", interval),
        }
    }

    /// `GET /fapi/v1/fundingRate`: funding history, oldest first. Paced by
    /// the funding limit, not by weight.
    pub fn funding_rate(&self) -> GetFundingRate {
        GetFundingRate {
            request: self.request(Route::FundingRate),
        }
    }

    /// `GET /fapi/v1/openInterest` (weight 1).
    pub fn open_interest(&self, symbol: &Symbol) -> WeightedRequest<OpenInterest> {
        self.request(Route::OpenInterest).query("symbol", symbol)
    }

    /// `GET /fapi/v1/aggTrades` (weight 20 at any limit). Serves only the last
    /// 48 hours; an older window is refused with code `-4166`.
    pub fn agg_trades(&self, symbol: &Symbol) -> GetAggTrades {
        GetAggTrades {
            request: self.request(Route::AggTrades).query("symbol", symbol),
        }
    }

    /// `GET /fapi/v1/depth`: an order book snapshot. Without a limit it
    /// returns 500 levels for weight 1, the cheapest way to get them.
    pub fn depth(&self, symbol: &Symbol) -> GetDepth {
        GetDepth {
            request: self
                .request(Route::Depth { limit: None })
                .query("symbol", symbol),
        }
    }
}

/// Request builder for `GET /fapi/v1/klines`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetKlines {
    request: WeightedRequest<Vec<Kline>>,
}

impl GetKlines {
    /// Earliest candle start, Unix milliseconds.
    pub fn start_time(mut self, ms: u64) -> Self {
        self.request = self.request.query("startTime", ms);
        self
    }

    /// Latest candle start, Unix milliseconds.
    pub fn end_time(mut self, ms: u64) -> Self {
        self.request = self.request.query("endTime", ms);
        self
    }

    /// Candles to return, up to 1500. Sets the weight: up to 100 costs 1, up
    /// to 500 costs 2, up to 1000 costs 5, more costs 10.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self
            .request
            .route(Route::Klines { limit: Some(limit) })
            .query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Kline>, BinanceError> {
        self.request.send().await
    }
}

/// Request builder for `GET /fapi/v1/fundingRate`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetFundingRate {
    request: WeightedRequest<Vec<FundingRate>>,
}

impl GetFundingRate {
    /// Restrict to one symbol. Without it, the latest event of every symbol.
    pub fn symbol(mut self, symbol: &Symbol) -> Self {
        self.request = self.request.query("symbol", symbol);
        self
    }

    /// Earliest funding time, Unix milliseconds.
    pub fn start_time(mut self, ms: u64) -> Self {
        self.request = self.request.query("startTime", ms);
        self
    }

    /// Latest funding time, Unix milliseconds.
    pub fn end_time(mut self, ms: u64) -> Self {
        self.request = self.request.query("endTime", ms);
        self
    }

    /// Rows to return, up to 1000.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<FundingRate>, BinanceError> {
        self.request.send().await
    }
}

/// Request builder for `GET /fapi/v1/aggTrades`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetAggTrades {
    request: WeightedRequest<Vec<AggTrade>>,
}

impl GetAggTrades {
    /// Start from this aggregate trade id, inclusive.
    pub fn from_id(mut self, id: u64) -> Self {
        self.request = self.request.query("fromId", id);
        self
    }

    /// Earliest trade time, Unix milliseconds, within the last 48 hours.
    pub fn start_time(mut self, ms: u64) -> Self {
        self.request = self.request.query("startTime", ms);
        self
    }

    /// Latest trade time, Unix milliseconds.
    pub fn end_time(mut self, ms: u64) -> Self {
        self.request = self.request.query("endTime", ms);
        self
    }

    /// Rows to return, up to 1000. The weight is 20 whatever the limit.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self.request.query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<AggTrade>, BinanceError> {
        self.request.send().await
    }
}

/// Request builder for `GET /fapi/v1/depth`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetDepth {
    request: WeightedRequest<Depth>,
}

impl GetDepth {
    /// Levels per side. Sets the weight: up to 50 costs 2, 100 costs 5, 500
    /// costs 10, 1000 costs 20.
    pub fn limit(mut self, limit: DepthLimit) -> Self {
        self.request = self
            .request
            .route(Route::Depth { limit: Some(limit) })
            .query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Depth, BinanceError> {
        self.request.send().await
    }
}
```

- [ ] **Step 5: Write the crate root and the README**

Replace `polyoxide-binance/src/lib.rs` with:

```rust
//! Rust client for Binance USDⓈ-M futures public market data
//! (`fapi.binance.com`).
//!
//! No credentials are needed. Every request is charged against a
//! [`WeightBudget`], because Binance limits each IP by request *weight*, which
//! varies by route and parameters, rather than by request count.
//!
//! ```no_run
//! use polyoxide_binance::{usdm::types::Symbol, Usdm};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let usdm = Usdm::new()?;
//! let btc = Symbol::new("BTCUSDT")?;
//! let index = usdm.market().premium_index(&btc).send().await?;
//! println!("BTCUSDT mark {} funding {}", index.mark_price, index.last_funding_rate);
//! # Ok(())
//! # }
//! ```

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

pub mod error;
pub mod usdm;
pub mod weight;

pub use error::BinanceError;
pub use usdm::{Usdm, UsdmBuilder};
pub use weight::WeightBudget;
```

`polyoxide-binance/README.md` (its code blocks are doctests):

````markdown
# polyoxide-binance

Rust client library for Binance USDⓈ-M futures public market data
(`fapi.binance.com`): contracts, tickers, the premium index and funding, klines,
open interest, aggregate trades and order book snapshots. No credentials are
needed.

Every request is charged against a `WeightBudget`, because Binance limits each
IP by request *weight*, which varies with the route and its parameters, rather
than by request count. The budget follows the server's own count from the
`X-MBX-USED-WEIGHT-1M` header, and a `429` or `418` holds every request until
the server's wait is over.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-binance/).

## Installation

```toml
[dependencies]
polyoxide-binance = "0.36"
```

## Usage

```no_run
use polyoxide_binance::{
    usdm::types::{Interval, Symbol},
    Usdm,
};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let usdm = Usdm::new()?;
let btc = Symbol::new("BTCUSDT")?;

let index = usdm.market().premium_index(&btc).send().await?;
println!("mark {} index {}", index.mark_price, index.index_price);

let candles = usdm
    .market()
    .klines(&btc, Interval::H1)
    .limit(24)
    .send()
    .await?;
println!("{} hourly candles", candles.len());
# Ok(())
# }
```

Clients in one process should share one budget, since Binance counts weight
per IP:

```no_run
use polyoxide_binance::{Usdm, WeightBudget};

# fn example() -> Result<(), polyoxide_binance::BinanceError> {
let budget = WeightBudget::new();
let a = Usdm::builder().weight_budget(budget.clone()).build()?;
let b = Usdm::builder().weight_budget(budget).build()?;
# Ok(())
# }
```
````

- [ ] **Step 6: Run everything**

Run: `cargo test -j 4 -p polyoxide-binance --all-targets && cargo test -j 4 -p polyoxide-binance --doc`
Expected: PASS: 37 unit tests, 19 in `mock_api`, 2 in `wire_agreement`, 3 doctests. `a_418_is_not_retried_and_holds_the_next_request` and `a_429_cools_down_then_succeeds` each take about a second; `a_high_used_weight_header_holds_the_next_request` may first wait up to three seconds to stay clear of a minute boundary.

Code review added a follow-up commit (781cb7a). A `429` with no retry left and no `Retry-After` started no cooldown, so the next request went out at once; it now holds every request until the next UTC minute (`WeightBudget::hold_until_next_minute`). Core's own floor is private, and `should_retry` gives no delay at `max_retries: 0`, the setting prader-rs uses. A `418` logs a clipped excerpt of its body, which names when the ban ends. Six mock tests pin rules that no test could fail before: a refused request's header is recorded, a retry is charged again, `Retry-After` outlasts a shorter backoff, the out-of-retries path for both a `429` that has a `Retry-After` and one that has none, and each route's own weight. The cooldown tests gain upper bounds. The docs state the in-flight reserve, that `send` has no deadline, and the cost of a funding backfill to the weight routes. `mock_api` then has 25 tests. A second follow-up (2cd2738) adds a paused-clock unit test that the hold lasts until the minute boundary for both limits, logs both `429` waits with their length, and sizes two weighted clients sharing a budget at `max_concurrent(2)`, under the reserve. The crate then has 38 unit tests.

- [ ] **Step 7: Show the send loop's tests can fail**

Break each rule, run `cargo test -j 4 -p polyoxide-binance --test mock_api`, see the named test fail, then restore:

| Change | Test that must fail |
|---|---|
| In `request.rs`, `self.budget.record_used(charge, used);` → `let _ = (charge, used);` | `a_high_used_weight_header_holds_the_next_request` |
| In `request.rs`, `self.budget.begin_cooldown(ban);` → `let _ = ban;` | `a_418_is_not_retried_and_holds_the_next_request` |
| In `request.rs`, `self.budget.begin_cooldown(cooldown);` → `let _ = cooldown;` | `a_429_cools_down_then_succeeds` |
| In `usdm/mod.rs`, `.gzip(true);` → `.gzip(false);` | `a_gzip_body_is_requested_and_decoded` |

- [ ] **Step 8: Lint and the doc gate**

Run: `cargo clippy -j 4 -p polyoxide-binance --all-targets --all-features -- -D warnings`
Expected: no warnings. The Task 4 and 5 `never used` warnings are gone.

Run: `RUSTDOCFLAGS="-D warnings" cargo doc -j 4 --no-deps --all-features -p polyoxide-binance`
Expected: no warnings.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all
git add polyoxide-binance
git commit -m "feat(binance): Usdm client, the weighted send loop and eleven routes

Usdm::new() / Usdm::builder() with health(), exchange() and market().
Every route is a WeightedRequest that charges the budget before sending
and records X-MBX-USED-WEIGHT-1M after; a 429 becomes a cooldown and is
retried on core's schedule, a 418 holds every request and is not
retried. Builders that change the weight (klines and depth limits)
change their route, and cost() says what a request will spend.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 7: The live suite

`live_responses_carry_no_unmodelled_keys` is the host's only drift detector: Binance publishes no schema. The helper chooses its TradFi and Chinese-character symbols from today's `exchangeInfo`, so a delisting cannot break the suite. If none is listed, it panics with `no suitable market`, which `.github/scripts/classify_failures.py` already classifies as environmental.

**Files:**
- Create: `polyoxide-binance/tests/live_api.rs`

- [ ] **Step 1: Write the suite**

```rust
//! Live integration tests against `fapi.binance.com`.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --test live_api -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector:
//! Binance publishes no schema, so nothing else notices a new field.

mod common;

use std::time::Duration;

use polyoxide_binance::{
    usdm::types::{
        AggTrade, ContractType, Depth, DepthLimit, ExchangeInfo, FundingInfo, FundingRate,
        Interval, Kline, OpenInterest, PremiumIndex, ServerTime, Symbol, SymbolStatus, Ticker24h,
    },
    BinanceError, Usdm,
};
use serde::{de::DeserializeOwned, Serialize};

fn client() -> Usdm {
    Usdm::new().expect("binance client")
}

/// BTCUSDT, a trading TradFi perpetual and a trading Chinese-character
/// perpetual, chosen from today's `exchangeInfo` so a delisting cannot break
/// the suite.
async fn three_kinds_of_symbol(usdm: &Usdm) -> Vec<Symbol> {
    let info = usdm
        .exchange()
        .exchange_info()
        .send()
        .await
        .expect("exchangeInfo");
    let trading =
        |s: &&polyoxide_binance::usdm::types::SymbolInfo| s.status == SymbolStatus::Trading;
    let tradfi = info
        .symbols
        .iter()
        .filter(trading)
        .find(|s| s.contract_type == ContractType::TradifiPerpetual)
        .expect("no suitable market: no trading TradFi perpetual is listed");
    let chinese = info
        .symbols
        .iter()
        .filter(trading)
        .find(|s| !s.symbol.as_str().is_ascii())
        .expect("no suitable market: no trading Chinese-character symbol is listed");
    vec![
        Symbol::new("BTCUSDT").unwrap(),
        Symbol::new(&*tradfi.symbol).unwrap(),
        Symbol::new(&*chinese.symbol).unwrap(),
    ]
}

#[tokio::test]
#[ignore]
async fn live_ping_time_and_the_weight_header() {
    let usdm = client();
    let latency = usdm.health().ping().await.expect("ping");
    assert!(latency < Duration::from_secs(10), "latency {latency:?}");
    let time = usdm.health().time().send().await.expect("time");
    assert!(time.server_time > 1_790_000_000_000);
    assert!(
        usdm.weight_budget().used() >= 2,
        "X-MBX-USED-WEIGHT-1M was not recorded"
    );
}

#[tokio::test]
#[ignore]
async fn live_every_market_route_answers_for_three_kinds_of_symbol() {
    let usdm = client();
    for symbol in three_kinds_of_symbol(&usdm).await {
        let market = usdm.market();
        let ticker = market.ticker_24h(&symbol).send().await.expect("ticker");
        assert_eq!(ticker.symbol, symbol.as_str());
        let index = market
            .premium_index(&symbol)
            .send()
            .await
            .expect("premiumIndex");
        assert!(
            index.mark_price > 0.into(),
            "{symbol} mark {}",
            index.mark_price
        );
        let candles = market
            .klines(&symbol, Interval::M1)
            .limit(2)
            .send()
            .await
            .expect("klines");
        assert_eq!(candles.len(), 2, "{symbol}");
        let oi = market
            .open_interest(&symbol)
            .send()
            .await
            .expect("openInterest");
        assert_eq!(oi.symbol, symbol.as_str());
        let trades = market
            .agg_trades(&symbol)
            .limit(2)
            .send()
            .await
            .expect("aggTrades");
        assert!(trades.len() <= 2, "{symbol}");
        let book = market
            .depth(&symbol)
            .limit(DepthLimit::Five)
            .send()
            .await
            .expect("depth");
        assert!(book.bids.len() <= 5 && book.asks.len() <= 5, "{symbol}");
        let funding = market
            .funding_rate()
            .symbol(&symbol)
            .limit(2)
            .send()
            .await
            .expect("fundingRate");
        assert!(funding.iter().all(|row| row.symbol == symbol.as_str()));
    }
}

#[tokio::test]
#[ignore]
async fn live_an_unknown_symbol_is_venue_error_1121() {
    let err = client()
        .market()
        .open_interest(&Symbol::new("NOTASYMBOLUSDT").unwrap())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            BinanceError::Venue {
                status: 400,
                code: -1121,
                ..
            }
        ),
        "{err:?}"
    );
}

async fn raw(path: &str) -> String {
    reqwest::Client::builder()
        .gzip(true)
        .build()
        .unwrap()
        .get(format!("https://fapi.binance.com{path}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .error_for_status()
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .text()
        .await
        .unwrap()
}

fn agrees<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = common::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may
    // be sent only sometimes. A key the type does not model is.
    if !diff.invented.is_empty() {
        eprintln!(
            "{path}: modelled but not sent this time: {:?}",
            diff.invented
        );
    }
    diff.unmodelled
        .into_iter()
        .map(|key| format!("{path}: {key}"))
        .collect()
}

#[tokio::test]
#[ignore]
async fn live_responses_carry_no_unmodelled_keys() {
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/fapi/v1/time";
    check(agrees::<ServerTime>(path, &raw(path).await));
    let path = "/fapi/v1/exchangeInfo";
    check(agrees::<ExchangeInfo>(path, &raw(path).await));
    let path = "/fapi/v1/fundingInfo";
    check(agrees::<Vec<FundingInfo>>(path, &raw(path).await));
    let path = "/fapi/v1/ticker/24hr";
    check(agrees::<Vec<Ticker24h>>(path, &raw(path).await));
    let path = "/fapi/v1/premiumIndex";
    check(agrees::<Vec<PremiumIndex>>(path, &raw(path).await));
    let path = "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=3";
    check(agrees::<Vec<Kline>>(path, &raw(path).await));
    let path = "/fapi/v1/fundingRate?limit=100";
    check(agrees::<Vec<FundingRate>>(path, &raw(path).await));
    let path = "/fapi/v1/openInterest?symbol=BTCUSDT";
    check(agrees::<OpenInterest>(path, &raw(path).await));
    let path = "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=10";
    check(agrees::<Vec<AggTrade>>(path, &raw(path).await));
    let path = "/fapi/v1/depth?symbol=BTCUSDT&limit=5";
    check(agrees::<Depth>(path, &raw(path).await));

    assert!(
        unmodelled.is_empty(),
        "the host sent keys the types do not model; add them and record them in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -j 4 -p polyoxide-binance --test live_api -- --ignored`
Expected: PASS, 4 tests, in about 12 seconds.

Code review added two follow-up commits:

- **f727235.** The weight header check could fail at a minute boundary, and it passed with the header never read, because the client's own two charges made 2. It now uses two clients with their own budgets, so a count of 2 can only come from the server, and a pair that straddles a minute is sent again. `raw` has a 30 s deadline, and it spells a failed status as core does (`API error: 503 Service Unavailable`), which the nightly classifier reads as transient; reqwest's `error_for_status` prose read as real. The drift detector also checks that kline rows have 12 values and book levels 2, since positional rows have no keys.
- **023121b.** The nightly classifier learns `BinanceError`'s retriable arms: `RateLimited { .. }`, and a `Venue` 408, 425 or 5xx in both renderings. Twelve pytest rows pin them, and the non-retriable arms stay real.

Whether a `451` from a US runner is environmental is settled in Task 11.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/tests/live_api.rs
git commit -m "test(binance): live suite and the host's drift detector

Every route for BTCUSDT, a TradFi perpetual and a Chinese-character
symbol chosen from today's listing; an unknown symbol is Venue -1121;
and every route's raw response must carry no key the types do not
model, since Binance publishes no schema to diff.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 8: The weight probe

**Files:**
- Create: `polyoxide-binance/examples/weight_probe.rs`
- Modify: `polyoxide-binance/Cargo.toml`

- [ ] **Step 1: Write the probe**

```rust
//! Measures each route's weight on the live host and compares it with
//! `Route::cost`, the table the client charges.
//!
//! ```sh
//! cargo run -p polyoxide-binance --example weight_probe
//! ```
//!
//! A route's weight is the rise in `X-MBX-USED-WEIGHT-1M` across its request,
//! so every case runs back to back inside one UTC minute; the probe waits for a
//! fresh minute first. Another process spending weight on the same IP makes a
//! row read high. Costs about 150 weight. The funding routes report no header
//! and are not probed. Exits 1 if any row differs from the table.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use polyoxide_binance::{
    usdm::types::DepthLimit,
    weight::{Cost, Route},
};

const BASE: &str = "https://fapi.binance.com";

/// `(route, path and query)`. The query carries exactly the parameters the
/// route names, which `every_case_requests_the_route_it_checks` holds.
fn cases() -> Vec<(Route, String)> {
    let mut cases = vec![
        (Route::Ping, "/fapi/v1/ping".to_owned()),
        (Route::Time, "/fapi/v1/time".to_owned()),
        (Route::ExchangeInfo, "/fapi/v1/exchangeInfo".to_owned()),
        (
            Route::Ticker24h { all: false },
            "/fapi/v1/ticker/24hr?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Ticker24h { all: true },
            "/fapi/v1/ticker/24hr".to_owned(),
        ),
        (
            Route::PremiumIndex { all: false },
            "/fapi/v1/premiumIndex?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::PremiumIndex { all: true },
            "/fapi/v1/premiumIndex".to_owned(),
        ),
        (
            Route::OpenInterest,
            "/fapi/v1/openInterest?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::AggTrades,
            "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=1".to_owned(),
        ),
        (
            Route::Klines { limit: None },
            "/fapi/v1/klines?symbol=BTCUSDT&interval=1m".to_owned(),
        ),
        (
            Route::Depth { limit: None },
            "/fapi/v1/depth?symbol=BTCUSDT".to_owned(),
        ),
    ];
    for limit in [100u32, 101, 500, 501, 1000, 1001] {
        cases.push((
            Route::Klines { limit: Some(limit) },
            format!("/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit={limit}"),
        ));
    }
    for limit in DepthLimit::ALL {
        cases.push((
            Route::Depth {
                limit: Some(*limit),
            },
            format!("/fapi/v1/depth?symbol=BTCUSDT&limit={limit}"),
        ));
    }
    cases
}

async fn used_after(client: &reqwest::Client, path: &str) -> u32 {
    let response = client
        .get(format!("{BASE}{path}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    response
        .headers()
        .get("x-mbx-used-weight-1m")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            panic!(
                "{path} answered {} with no weight header",
                response.status()
            )
        })
}

async fn wait_for_a_fresh_minute() {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        % 60_000;
    if ms > 20_000 {
        let wait = 61_000 - ms;
        eprintln!("waiting {} s for a fresh minute", wait / 1000);
        tokio::time::sleep(Duration::from_millis(wait)).await;
    }
}

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder().gzip(true).build().unwrap();
    wait_for_a_fresh_minute().await;
    let mut previous = used_after(&client, "/fapi/v1/ping").await;
    let mut differing = 0;
    for (route, path) in cases() {
        let now = used_after(&client, &path).await;
        let measured = now.saturating_sub(previous);
        previous = now;
        let Cost::Weight(table) = route.cost() else {
            continue;
        };
        let verdict = if measured == table {
            "ok"
        } else {
            differing += 1;
            "DIFFERS"
        };
        println!("{path:58} table {table:>3}  measured {measured:>3}  {verdict}");
    }
    if differing > 0 {
        eprintln!("{differing} rows differ from Route::cost; update it and docs/specs/binance/OBSERVED.md");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_requests_the_route_it_checks() {
        for (route, path) in cases() {
            assert!(path.starts_with(route.path()), "{path} is not {route:?}");
            let limit = path
                .split(['?', '&'])
                .find_map(|kv| kv.strip_prefix("limit="));
            let one_symbol = path.contains("symbol=");
            match route {
                Route::Klines { limit: expected } => {
                    assert_eq!(limit.map(|l| l.parse::<u32>().unwrap()), expected, "{path}")
                }
                Route::Depth { limit: expected } => {
                    assert_eq!(limit, expected.map(DepthLimit::as_str), "{path}")
                }
                Route::Ticker24h { all } | Route::PremiumIndex { all } => {
                    assert_eq!(one_symbol, !all, "{path}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn every_depth_limit_and_klines_band_edge_is_probed() {
        let routes: Vec<Route> = cases().into_iter().map(|(route, _)| route).collect();
        for limit in DepthLimit::ALL {
            assert!(
                routes.contains(&Route::Depth {
                    limit: Some(*limit)
                }),
                "{limit}"
            );
        }
        for limit in [100, 101, 500, 501, 1000, 1001] {
            assert!(
                routes.contains(&Route::Klines { limit: Some(limit) }),
                "{limit}"
            );
        }
    }
}
```

Append to `polyoxide-binance/Cargo.toml`, after a blank line:

```toml
# `cargo test` builds examples but does not run their unit tests unless
# asked; the checks that the probe measures the routes it names live here.
[[example]]
name = "weight_probe"
path = "examples/weight_probe.rs"
test = true
```

- [ ] **Step 2: Run its unit tests**

Run: `cargo test -j 4 -p polyoxide-binance --example weight_probe`
Expected: PASS, 2 tests.

- [ ] **Step 3: Run it live**

Run: `cargo run -j 4 -q -p polyoxide-binance --example weight_probe`
Expected: up to a minute's wait for a fresh minute, then 24 rows, each ending `ok`, and exit status 0. A `DIFFERS` row means Binance changed a weight, or another process spent weight on this IP during the run: run it again, and if the row still differs, update `Route::cost`, `documented_weights` and `OBSERVED.md`.

Code review added a follow-up commit (d904aae). The probe now stops at the first answer that is not 200. It prints the status, the `Retry-After` and the body, sends nothing more, and exits 2: before, a `429` midway was taken as a measurement, and a refused request printed `ok`. A count that falls mid-run is reported as a minute rollover, where it used to show as a weight of 0. The wait now lands three seconds past the boundary. A busy IP is named before the table, and every request has a 30 s deadline.

- [ ] **Step 4: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/examples/weight_probe.rs polyoxide-binance/Cargo.toml
git commit -m "feat(binance): weight_probe re-measures the weight table live

Runs every row of Route::cost, each klines band edge and depth limit
included, back to back in one fresh UTC minute, and exits 1 on a row
whose X-MBX-USED-WEIGHT-1M delta differs from the table.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 9: What the host is and what it does

**Files:**
- Create: `docs/specs/binance/INDEX.md`
- Create: `docs/specs/binance/OBSERVED.md`

- [ ] **Step 1: Write the index**

`docs/specs/binance/INDEX.md`:

````markdown
# Binance USDⓈ-M futures

Binance is not a Polymarket host. `polyoxide-binance` reads its USDⓈ-M futures public
market data for consumers that trade both venues; it is not part of the `polyoxide`
umbrella crate.

| Surface | Host | Crate |
|---|---|---|
| REST, public market data | `https://fapi.binance.com` | `polyoxide-binance` (`Usdm`) |
| Market streams | `wss://fstream.binance.com/{market,public}/stream` | the WebSocket plan, not yet implemented |

**This directory is not a mirror.** Binance publishes no OpenAPI or AsyncAPI document for
USDⓈ-M futures (`github.com/binance/binance-api-swagger` holds `spot_api.yaml` only), so
there is nothing to vendor and nothing for `nightly-schema.yml` to diff. The sources are
the prose pages and the wire:

- REST pages: `https://developers.binance.com/docs/derivatives/usds-margined-futures/`
- Stream pages: `https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/`
  (`market` and `public`). The old stream URLs under the REST prefix land on a generic
  page as of 2026-10-07.

Where the pages and the wire disagree, the wire wins and [OBSERVED.md](OBSERVED.md)
records it. The drift detector is the live suite:
`polyoxide-binance/tests/live_api.rs::live_responses_carry_no_unmodelled_keys` fails on any
key the types do not model.
A new enum value or filter type decodes as `Other` and is not seen, and a changed weight
goes unseen until `weight_probe` is run by hand.

## Routes covered

| Route | Method | Weight |
|---|---|---|
| `/fapi/v1/ping` | `usdm.health().ping()` | 1 |
| `/fapi/v1/time` | `usdm.health().time()` | 1 |
| `/fapi/v1/exchangeInfo` | `usdm.exchange().exchange_info()` | 1 |
| `/fapi/v1/fundingInfo` | `usdm.exchange().funding_info()` | funding limit, 500 per 5 minutes |
| `/fapi/v1/ticker/24hr` | `usdm.market().ticker_24h(&s)` / `tickers_24h()` | 1 / 40 |
| `/fapi/v1/premiumIndex` | `usdm.market().premium_index(&s)` / `premium_indices()` | 1 / 10 |
| `/fapi/v1/klines` | `usdm.market().klines(&s, interval)` | 1 to 10 by `limit`, 5 without |
| `/fapi/v1/fundingRate` | `usdm.market().funding_rate()` | funding limit, 500 per 5 minutes |
| `/fapi/v1/openInterest` | `usdm.market().open_interest(&s)` | 1 |
| `/fapi/v1/aggTrades` | `usdm.market().agg_trades(&s)` | 20 |
| `/fapi/v1/depth` | `usdm.market().depth(&s)` | 1 without `limit`, 2 to 20 with |

The weight table is `Route::cost` in `polyoxide-binance/src/weight.rs`, pinned by its
`documented_weights` test and re-measured live by
`cargo run -p polyoxide-binance --example weight_probe`.

## Fixtures and probes

- `polyoxide-binance/tests/fixtures/rest/`: refreshed by
  `python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures`.
- `polyoxide-binance/tests/fixtures/ws/`: stream envelopes captured 2026-10-07.
- `probes/`: the stdlib scripts behind most of the design spec's measurements. Its
  `capture.py` is superseded by `scripts/capture_binance_fixtures.py`.
````

- [ ] **Step 2: Write the observations**

`docs/specs/binance/OBSERVED.md`:

````markdown
# Binance USDⓈ-M: what the host does

Measured against `fapi.binance.com` and `fstream.binance.com` on 2026-10-07 unless an
entry says otherwise. The weights and the window name their method; the other entries are
single requests made while writing the design spec,
`docs/superpowers/specs/2026-10-07-polyoxide-binance-design.md`.

## Request weight

Method: the rise in `X-MBX-USED-WEIGHT-1M` across back-to-back requests inside one
minute. `polyoxide-binance/examples/weight_probe.rs` re-measures every weighted row; the
`klines` edges it skips (99, 499, 999, 1500), `aggTrades` at limits 100 and 1000, and
the refusals below were probed on 2026-10-07, with `docs/specs/binance/probes/probe_rest.py`
and single requests.

| Route | Measured 2026-10-07 | The page says |
|---|---|---|
| `ping`, `time`, `exchangeInfo`, `openInterest` | 1 | 1 |
| `ticker/24hr` | 1 with `symbol`, 40 without | the same |
| `premiumIndex` | 1 with `symbol`, 10 without | the same |
| `klines` | 1 up to 100, 2 up to 500, 5 up to 1000, 10 above; **5 without `limit`** | under 100 → 1, 100–499 → 2, 500–1000 → 5, over 1000 → 10 |
| `aggTrades` | 20 at limits 1, 100 and 1000 | 20 |
| `depth` | 2 at 5, 10, 20, 50; 5 at 100; 10 at 500; 20 at 1000; **1 without `limit`** | 2 / 5 / 10 / 20 by limit |
| `fundingInfo`, `fundingRate` | no header | share 500 per 5 minutes per IP |

`klines` was measured at every band edge (99, 100, 101, 499, 500, 501, 999, 1000, 1001,
1500), twice: each band is inclusive at the top, which puts the page one off at the 100
and 500 edges; the 1000 edge agrees. A request without `limit` returns 500 rows for 5
where an explicit `limit=500` costs 2.
`depth` without `limit` returns 500 levels for 1 where an explicit `limit=500` costs 10.
A request refused with `400` still costs weight. An unknown symbol costs its route's
weight: 1 on `premiumIndex` and 20 on `aggTrades`. A `klines` limit of 1501 costs 10,
and a `depth` limit of 7 costs 1.

## The weight window

The UTC clock minute. On 2026-10-07 a `ping` every 3 s read 10 at 08:34:59.5 and 1 at
08:35:02.9, then 16 at 08:35:57.3 and 1 at 08:36:00.9. A sliding 60-second window would
have read 10 or more, not 1, at 08:35:02.9.

## Errors

- An unknown symbol: `400 {"code":-1121,"msg":"Invalid symbol."}`.
- A `depth` limit outside 5, 10, 20, 50, 100, 500, 1000: `400 {"code":-4021,"msg":"7 is not valid depth limit"}`.
- A `klines` limit above 1500: `400 {"code":-1130,"msg":"Data sent for parameter 'limit' is not valid."}`.
- A `klines` interval of `1s`, which the docs list: `400 {"code":-1120,"msg":"Invalid interval."}`.
- `aggTrades` older than 48 hours: `400 {"code":-4166,"msg":"Search window is restricted to recent 2 days only."}`.
  The page documents the 48 hours but not the code.
- `418`, `429`, `451` and `403` were not provoked from a test IP; the client classifies
  them by status.

## Response shapes the page does not give

- `exchangeInfo` has a top-level `futuresType`, `"U_MARGINED"`.
- `fundingInfo`'s `updateTime` is `null` on 57 of 805 rows, `BTCUSDT` among them.
- `fundingInfo` also lists COIN-M perpetuals (`BTCUSD_PERP`, `ETHUSD_PERP`, …) that
  `exchangeInfo` on this host does not, which is one reason row symbols are `String`.
- `fundingRate`'s `markPrice` is `""` for funding events through at least 2022-01-01
  (`startTime=1568102400000` and `startTime=1640995200000` both answered `""`; the
  cutoff was not located).
- `ticker/24hr` without `symbol` lists only `TRADING` contracts (789 of 924);
  `premiumIndex` lists 927 rows.
- The REST host accepts a lowercase symbol (`premiumIndex?symbol=btcusdt`) and answers
  with `BTCUSDT`. No listed symbol has a lowercase ASCII letter.
- Quarterly symbols carry an underscore (`BTCUSDT_261225`). Of the 924 symbols in
  `exchangeInfo` on 2026-10-07 no other symbol used a character other than a letter or
  a digit; five were Chinese-character symbols; the longest was 17 characters.
- `underlyingType` took nine values: `COIN`, `EQUITY`, `HK_EQUITY`, `COMMODITY`,
  `KR_EQUITY`, `PREMARKET`, `INDEX`, `CN_EQUITY`, `FX`. The docs list none.
- `aggTrades` rows carry `nq`, documented as the quantity without trades involving RPI
  orders.

## What the page says and the host did not refuse

- `aggTrades` with both `startTime` and `endTime` "must span less than an hour": a
  two-hour window was answered `200` on 2026-10-07.

## Streams

Recorded in the design spec's venue contract (1024 streams per connection, about 15
requests back to back before a close, every `SUBSCRIBE` acknowledged, server pings about
every 180 s) and moved here by the WebSocket plan.
````

- [ ] **Step 3: Commit**

```bash
git add docs/specs/binance/INDEX.md docs/specs/binance/OBSERVED.md
git commit -m "docs(binance): INDEX and OBSERVED for USDⓈ-M futures

Not a mirror: Binance publishes no spec for USDⓈ-M. OBSERVED.md records
the measured weight table (off by one from the page at every klines
band edge), the UTC-minute window, the error codes seen, and the
response shapes the page omits.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 10: The repository's docs

**Files:**
- Modify: `CLAUDE.md`, `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-schema.yml`

- [ ] **Step 1: CLAUDE.md**

Replace `Eleven crates with this dependency graph:` with `Twelve crates with this dependency graph:`.

After the line `├── polyoxide-perps     (perpetual futures: public market data; auth and trading pending)` add:

```
├── polyoxide-binance   (Binance USDⓈ-M futures public market data; not in the umbrella crate)
```

In the "Nightly API Smoketest" list, replace `perps incl. \`live_ws\`, sports, cli)` with `perps incl. \`live_ws\`, sports, binance, cli)`.

In the same section, replace `doc) and the undocumented \`user-pnl-api\`/\`lb-api\` hosts (nothing to diff).` with `doc), the undocumented \`user-pnl-api\`/\`lb-api\` hosts (nothing to diff), and Binance (\`docs/specs/binance/\`: Binance publishes no spec for USDⓈ-M, and its live suite is the drift check).`

In "Publishing Order", replace `core → rtds → sports → perps → relay → gamma → data → clob → polyoxide.` with `core → rtds → sports → perps → binance → relay → gamma → data → clob → polyoxide.`, and replace `so it only has to follow core and precede \`polyoxide\`.)` with `so it only has to follow core and precede \`polyoxide\`; \`polyoxide-binance\` only has to follow core, since no published crate depends on it.)`.

In "Testing Conventions", replace `They hit the real Polymarket APIs.` with `They hit the real upstream APIs.`

Insert this paragraph directly before the paragraph that begins `**Data API v2**`:

```markdown
**Binance USDⓈ-M futures is not a Polymarket host.** `polyoxide-binance` reads its public
market data on `fapi.binance.com` (`Usdm::new()`, namespaces `health()`, `exchange()`,
`market()`) for consumers that trade both venues, and is deliberately not in the
`polyoxide` umbrella crate or `full`. Binance publishes no OpenAPI or AsyncAPI for
USDⓈ-M, so `docs/specs/binance/` is not a mirror: `OBSERVED.md` records what the host
does, `nightly-schema.yml` has nothing to diff, and
`tests/live_api.rs::live_responses_carry_no_unmodelled_keys` is the drift detector.
Binance limits each IP by request *weight*, which depends on the route and its
parameters, so the crate has its own `WeightBudget` (`src/weight.rs`) instead of core's
`RateLimiter`: the UTC clock minute at 2160 of the published 2400 (the tenth core's
`RESERVED_FRACTION` also reserves), raised by every response's `X-MBX-USED-WEIGHT-1M` but only for the
minute its request was charged in, a separate bucket for the weightless funding routes
(450 per 5 minutes, depth one), and a `429` or `418` held as a client-wide cooldown that
is only ever extended. A `429` with no retry left and no `Retry-After` holds every
request to the next minute: sending into a spent minute is how a `429` becomes a `418`
ban. The weight table, `Route::cost`, is measured, not copied: Binance's page is one off
at the 100 and 500 `klines` edges and does not say that omitting `limit` costs 5 on
`klines` and 1 on `depth`. `cargo run -p polyoxide-binance --example weight_probe`
re-measures it. Response rows carry symbols as `String`; `Symbol` is for what a caller
sends, uppercases ASCII, and accepts `_` for quarterlies (`BTCUSDT_261225`).
`FundingRate::mark_price` is an `Option` because funding events through at least
2022-01-01 send `""`. Core's `HttpClientBuilder::gzip` is off by default and only
this crate turns it on (`exchangeInfo` is 1.15 MB raw, 51 KB gzipped); with reqwest's
`gzip` feature on workspace-wide, any client built without core would ask for gzip,
which is why four rate-limit examples pin `.gzip(false)`.
```

- [ ] **Step 2: README.md**

In the crate table, after the `| [polyoxide](./polyoxide) | ... |` row, add:

```markdown
| [polyoxide-binance](./polyoxide-binance) | Client library for Binance USDⓈ-M futures public market data (not part of the unified crate) |
```

- [ ] **Step 3: docs/specs/INDEX.md**

After the "Mirrored for reference, **not implemented** by any crate" table, before `## Hosts with no upstream spec`, add:

```markdown
## Other venues

Not Polymarket hosts. Read by a polyoxide crate for consumers that trade both venues:

| API | Base URL | Description | Crate |
|-----|----------|-------------|-------|
| [Binance USDⓈ-M](binance/INDEX.md) | `https://fapi.binance.com` | Futures public market data. No published spec, so not a mirror | `polyoxide-binance` |

```

- [ ] **Step 4: SELF-HEALING.md**

In the nightly table, after the `polyoxide-sports` row, add:

```markdown
| polyoxide-binance | `live_api` |
```

Replace `against the real Polymarket APIs,` with `against the real upstream APIs,`.

Under `### Deliberate exclusions`, after the `user-pnl-api` / `lb-api` bullet, add:

```markdown
- **`docs/specs/binance/`** — Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M
  futures, so the directory records observations, not a mirror.
  `polyoxide-binance/tests/live_api.rs` is the drift check.
```

- [ ] **Step 5: nightly-schema.yml's comment**

In `.github/workflows/nightly-schema.yml`, after the comment bullet that ends `#     diff; the fixtures are the drift check.` and before `        include:`, add:

```yaml
        #   - Binance USDⓈ-M (docs/specs/binance/): not a Polymarket host,
        #     and Binance publishes no OpenAPI or AsyncAPI for it. The live
        #     suite in polyoxide-binance is the drift check.
```

Run: `grep -n 'Binance' .github/workflows/nightly-schema.yml CLAUDE.md README.md docs/specs/INDEX.md SELF-HEALING.md`
Expected: each file's new lines.

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md README.md docs/specs/INDEX.md SELF-HEALING.md .github/workflows/nightly-schema.yml
git commit -m "docs: polyoxide-binance in CLAUDE.md, READMEs and the drift notes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 11: Publishing order, the nightly row and the full gate

The crate publishes with the next release, which the WebSocket plan cuts as 0.37.0. Until then the order only has to be right.

**Files:**
- Modify: `.github/workflows/release.yml`, `scripts/finish_release.sh`, `.github/workflows/nightly-behavioral.yml`, `.github/scripts/classify_failures.py`, `.github/scripts/tests/test_classify_failures.py`, `CLAUDE.md`, `SELF-HEALING.md`

- [ ] **Step 1: release.yml**

Replace

```yaml
      # Publish in dependency order: core -> rtds -> sports -> perps -> relay -> gamma -> data -> clob -> polyoxide
```

with

```yaml
      # Publish in dependency order: core -> rtds -> sports -> perps -> binance -> relay -> gamma -> data -> clob -> polyoxide
```

and in the `CRATES=(...)` line, insert `"polyoxide-binance"` after `"polyoxide-perps"`:

```yaml
          CRATES=("polyoxide-core" "polyoxide-rtds" "polyoxide-sports" "polyoxide-perps" "polyoxide-binance" "polyoxide-relay" "polyoxide-gamma" "polyoxide-data" "polyoxide-clob" "polyoxide")
```

- [ ] **Step 2: finish_release.sh**

After the block

```bash
echo "📦 Publishing polyoxide-perps..."
cargo publish -p polyoxide-perps
echo "✅ polyoxide-perps published"
```

add

```bash

echo "📦 Publishing polyoxide-binance..."
cargo publish -p polyoxide-binance
echo "✅ polyoxide-binance published"
```

- [ ] **Step 3: The nightly row**

In `.github/workflows/nightly-behavioral.yml`, after the `polyoxide-perps` row of `include:`, add:

```yaml
          - { crate: polyoxide-binance, suite: live,       timeout: 15, flags: "--test live_api" }
```

Run: `grep -n 'polyoxide-binance' .github/workflows/release.yml .github/workflows/nightly-behavioral.yml scripts/finish_release.sh`
Expected: one line in each workflow, three in the script.

- [ ] **Step 4: A region block is environmental**

Binance answers HTTP 451 to callers in places it does not serve, and US cloud regions are reported to be among them. GitHub's hosted runners run there. As written, a 451 would file a `real` nightly issue every night. The owner decided on 2026-10-07 that a Binance region block is **environmental**: logged and skipped, never filed. The suites still run wherever Binance serves the caller, so if the runners turn out to be served, the drift detector runs nightly.

In `.github/scripts/classify_failures.py`, replace

```python
# Two shapes so far: the sports channel's `legitimately time out`, and the
# order-placing tests refusing to post because no open market's book satisfies
# their price precondition (`no qualifying market` from the selection helper,
# `no suitable market` from a test's own guard). The phrase is required in
# full — a bare `market` would swallow most genuine CLOB failures.
ENVIRONMENTAL_RE = re.compile(
    r"legitimately time out|no (?:qualifying|suitable) market",
    re.IGNORECASE,
)
```

with

```python
# Three shapes so far. The sports channel's `legitimately time out`. The
# order-placing tests refusing to post because no open market's book satisfies
# their price precondition (`no qualifying market` from the selection helper,
# `no suitable market` from a test's own guard); the phrase is required in
# full — a bare `market` would swallow most genuine CLOB failures. And Binance
# refusing the caller's location with HTTP 451: GitHub's hosted runners run in
# US regions, which Binance is reported not to serve, so there its live suites
# can only report the block. It is skipped, not filed, and the suites still run
# wherever Binance serves the caller. The 451 is matched in each spelling a
# panic carries: `BinanceError::RegionBlocked` (Display and Debug), the live
# suite's `API error: 451 Unavailable For Legal Reasons`, a refused stream
# handshake's Display (`HTTP error: 451 Unavailable For Legal Reasons`) and its
# Debug (`Response { status: 451, .. }`).
ENVIRONMENTAL_RE = re.compile(
    r"legitimately time out|no (?:qualifying|suitable) market"
    r"|does not serve this location \(451\)|\bRegionBlocked \{"
    r"|\b451 Unavailable For Legal Reasons\b|\bstatus: 451\b",
    re.IGNORECASE,
)
```

In `.github/scripts/tests/test_classify_failures.py`, directly before `def test_classify_bare_market_word_is_real() -> None:`, add:

```python
# Binance refuses a caller in a place it does not serve with HTTP 451, on REST
# and on the stream handshake. Every spelling a Binance live test's panic can
# carry it in is environmental.
BINANCE_REGION_BLOCKS: list[tuple[str, str]] = [
    (
        "RegionBlocked / Display",
        "live_x: binance does not serve this location (451): "
        "Service unavailable from a restricted location",
    ),
    (
        "RegionBlocked / Debug",
        'live_x: RegionBlocked { msg: "Service unavailable from a restricted location" }',
    ),
    ("raw status", "/fapi/v1/time: API error: 451 Unavailable For Legal Reasons"),
    (
        "handshake / Display",
        "live_x: WebSocket transport error: HTTP error: 451 Unavailable For Legal Reasons",
    ),
    (
        "handshake / Debug",
        "live_x: Connect(Http(Response { status: 451, version: HTTP/1.1, headers: {} }))",
    ),
]


@pytest.mark.parametrize(
    "label,text", BINANCE_REGION_BLOCKS, ids=[a[0] for a in BINANCE_REGION_BLOCKS]
)
def test_a_binance_region_block_is_environmental(label: str, text: str) -> None:
    assert classify(text) == Verdict.ENVIRONMENTAL, f"{label} was not skipped"


def test_other_binance_refusals_are_not_environmental() -> None:
    """A firewall refusal or a ban is about how this client behaved, not where
    it runs, so each still files an issue."""
    for text in (
        "live_x: binance's firewall refused the request (403): <html>",
        "live_x: IpBanned { retry_after: None }",
        "/fapi/v1/time: API error: 403 Forbidden",
    ):
        assert classify(text) == Verdict.REAL, text


```

In `CLAUDE.md`, replace

```
matches `legitimately time out`) — logged and skipped
```

with

```
matches `legitimately time out` — or Binance refusing a US runner's location with a 451) — logged and skipped
```

In `SELF-HEALING.md`, replace

```
(e.g. the sports feed with no live match anywhere at 06:00 UTC) |
```

with

```
(e.g. the sports feed with no live match anywhere at 06:00 UTC), or Binance refuses the runner's location with HTTP 451 |
```

Run: `cd .github/scripts && uv run pytest tests/ -q && cd ../..`
Expected: PASS, 156 tests (150 before, plus five region-block rows and one test that keeps other refusals real).

Then prove the new pattern is what classifies each row. Delete the line `    r"|\b451 Unavailable For Legal Reasons\b|\bstatus: 451\b",` and move its trailing comma onto the line above, so the call still parses, then run the tests again: `raw status`, `handshake / Display` and `handshake / Debug` must fail, and nothing else. Restore the file.

Executed 2026-10-07 (0d12b78): 156 passed; the mutation failed exactly those three rows; the full gate was clean, 1936 workspace tests.

- [ ] **Step 5: The full gate**

Run each; all must be clean:

```bash
cargo fmt --all -- --check
cargo clippy -j 4 --all-targets --all-features -- -D warnings
cargo test -j 4 --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc -j 4 --no-deps --all-features --workspace
(cd .github/scripts && uv run pytest tests/ -q)
```

The workspace test and doc builds are the heavy ones. If they are reaped (`signal: 15`), rerun them with `-j 2`, or run `-p polyoxide-binance -p polyoxide-core -p polyoxide-data -p polyoxide-gamma` and leave the rest to CI.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/release.yml .github/workflows/nightly-behavioral.yml scripts/finish_release.sh \
  .github/scripts/classify_failures.py .github/scripts/tests/test_classify_failures.py CLAUDE.md SELF-HEALING.md
git commit -m "ci: publish polyoxide-binance after perps; nightly live_api row

A Binance region block (HTTP 451) is classified environmental: GitHub's
hosted runners are in US regions, which Binance is reported not to
serve, so a 451 there is skipped, not filed.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

## Done when

- `cargo test -p polyoxide-binance --all-targets` and `--doc` pass offline; the live suite and the weight probe pass against the host.
- The full gate in Task 11 is clean.
- prader-rs's REST contract compiles against the crate unchanged: `Usdm::builder().base_url(..).timeout_ms(..).with_retry_config(..).build()`, the eleven calls, `limit(u32)`, `DepthLimit::Twenty`, `Filter::PriceFilter { tick_size, .. }`, `String` row symbols, `AggTrade::is_buyer_maker`, and the six constructible `BinanceError` variants.
- Nothing is released yet: the WebSocket plan cuts 0.37.0.
