# polyoxide-rtds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `polyoxide-rtds`, a credential-free WebSocket client for Polymarket's Real-Time Data Service, covering Binance spot, Chainlink spot, and the Chainlink 30s/60s TWAP price topics.

**Architecture:** A leaf crate depending on nothing else in the workspace. Two tiers: `Rtds` is a bare `Stream` that ends on disconnect; `SupervisedRtds` adds keep-alive pings, a staleness watchdog and reconnect-with-resubscribe. Payloads are typed per topic rather than shared, because `full_accuracy_value` is E18 fixed-point on the Chainlink topics and a plain decimal on Binance — two field-identical wire shapes with different meanings.

**Tech Stack:** Rust 1.91, `tokio-tungstenite` 0.26, `rust_decimal` 1.37, `serde`, `thiserror`, `futures-util`.

**Design spec:** `docs/superpowers/specs/2026-09-05-rtds-chainlink-twap-design.md`. Read the "Evidence base" section before starting — every non-obvious decision here traces to observed wire behaviour that contradicts Polymarket's published docs.

**Commit convention:** Every commit message in this plan must end with these two trailers (omitted from the per-task commands below for brevity):

```
Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017AM9cw7G4us4kyU3DN1PL2
```

**Verification gates.** CI runs four jobs. Before any commit, the relevant gate must pass:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p polyoxide-rtds --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features -p polyoxide-rtds
```

The `cargo doc` gate is not optional and is easy to trip: a doc comment on a `pub` item may not use ``[`link`]`` syntax to reference a `pub(crate)` item. Doctests do not catch this. A red doc build silently withholds the release tag.

**Wire the module into `lib.rs` before writing the failing test.** Several tasks below create a new module and say "write the failing test, then add the `pub mod` line to `lib.rs`". Done in that order the test does not fail — it does not *run*. An unreferenced file is never compiled, so `cargo test -p polyoxide-rtds <name>` reports `0 tests, N filtered out` and exits 0, which looks like a pass. Add the `pub mod` and `pub use` lines first, then write the test, then watch it fail to compile with `cannot find type ...`. Same end state, and it is the only ordering that produces the evidence the red-then-green step is asking for. This applies to Tasks 4, 5, 8, 9 and 11.

---

## File Structure

| File | Responsibility |
|---|---|
| `polyoxide-rtds/Cargo.toml` | Manifest. No features; ws deps unconditional. |
| `polyoxide-rtds/src/lib.rs` | Crate docs and re-exports. |
| `polyoxide-rtds/src/topic.rs` | `Topic`, `TwapWindow`, wire-string mapping. |
| `polyoxide-rtds/src/subscription.rs` | `Subscription`, `SubscriptionRequest`, filter encoding. |
| `polyoxide-rtds/src/decode.rs` | `decode_e18`, `decode_plain`. The only place a scale is applied. |
| `polyoxide-rtds/src/payload.rs` | Per-topic update structs, `Snapshot`, `SnapshotPoints`. |
| `polyoxide-rtds/src/event.rs` | `PriceEvent`, `PriceUpdate`, accessors, frame dispatch. |
| `polyoxide-rtds/src/error.rs` | `RtdsError` and the three-way `Recovery` classifier. |
| `polyoxide-rtds/src/client.rs` | `Rtds` — tier 1. |
| `polyoxide-rtds/src/supervisor.rs` | `RtdsBuilder`, `SupervisedRtds` — tier 2. |
| `polyoxide-rtds/src/fixtures.rs` | Frames captured 2026-09-05, behind `test-fixtures` so `tests/` can share them. |
| `polyoxide-rtds/tests/scripted_server.rs` | Local ws harness (dev-only). |
| `polyoxide-rtds/tests/supervision.rs` | Reconnect/staleness tests against the harness. |
| `polyoxide-rtds/tests/live_api.rs` | `#[ignore]` tests against the real host. |
| `polyoxide-rtds/examples/twap_stream.rs` | Runnable example. |

---

### Task 1: Crate skeleton

**Files:**
- Create: `polyoxide-rtds/Cargo.toml`
- Create: `polyoxide-rtds/src/lib.rs`
- Modify: `Cargo.toml` (workspace members list, lines 2-11; workspace dependencies, near line 27)

- [ ] **Step 1: Create the manifest**

`polyoxide-rtds/Cargo.toml`:

```toml
[package]
name = "polyoxide-rtds"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Rust client for Polymarket's Real-Time Data Service (RTDS) crypto price streams"
keywords = ["polymarket", "websocket", "chainlink", "twap", "prices"]
categories = ["api-bindings", "web-programming::websocket"]

[dependencies]
tokio = { workspace = true, features = ["macros", "net", "time", "rt"] }
tokio-tungstenite = { workspace = true }
futures-util = "0.3"
serde = { workspace = true }
serde_json = { workspace = true }
rust_decimal = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
url = { workspace = true }
# Needed only to install a process-default rustls CryptoProvider. See the
# comment on `ensure_crypto_provider` in src/client.rs.
rustls = { version = "0.23", default-features = false, features = [
  "ring",
  "std",
] }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time", "net"] }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 2: Add to the workspace**

In the root `Cargo.toml`, add `"polyoxide-rtds",` to `members` (keep alphabetical — it goes after `"polyoxide-relay",`), and add this line to `[workspace.dependencies]` alongside the other internal crates:

```toml
polyoxide-rtds = { path = "polyoxide-rtds", version = "0.28.1" }
```

- [ ] **Step 3: Create a minimal lib.rs**

`polyoxide-rtds/src/lib.rs`:

```rust
//! # polyoxide-rtds
//!
//! Rust client for Polymarket's Real-Time Data Service (RTDS), served at
//! `wss://ws-live-data.polymarket.com`.
//!
//! RTDS relays reference prices without credentials: Binance spot, Chainlink
//! spot, and Chainlink-computed 30-second and 60-second TWAPs. It is a
//! different protocol from the CLOB WebSocket channels in `polyoxide-clob` —
//! many topics are multiplexed over one connection.
//!
//! # Precision
//!
//! Each update carries both a lossy `display_value: f64` and an exact
//! [`Decimal`](rust_decimal::Decimal) `value`. **Always use `value` for
//! arithmetic.** The wire field it is decoded from, `full_accuracy_value`, is
//! E18 fixed-point on the Chainlink topics and a plain decimal on Binance, so
//! each topic has its own type and the scale is never a runtime decision.

#![warn(missing_docs)]
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build -p polyoxide-rtds`
Expected: `Finished` with no warnings.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml polyoxide-rtds/
git commit -m "feat(rtds): add the polyoxide-rtds crate skeleton"
```

---

### Task 2: Topic and TwapWindow

**Files:**
- Create: `polyoxide-rtds/src/topic.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-rtds/src/topic.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_strings_match_the_venue() {
        assert_eq!(Topic::BinanceSpot.as_wire(), "crypto_prices");
        assert_eq!(Topic::ChainlinkSpot.as_wire(), "crypto_prices_chainlink");
        assert_eq!(
            Topic::ChainlinkTwap(TwapWindow::Thirty).as_wire(),
            "crypto_prices_twap_thirty"
        );
        assert_eq!(
            Topic::ChainlinkTwap(TwapWindow::Sixty).as_wire(),
            "crypto_prices_twap_sixty"
        );
    }

    #[test]
    fn wire_strings_round_trip() {
        for topic in [
            Topic::BinanceSpot,
            Topic::ChainlinkSpot,
            Topic::ChainlinkTwap(TwapWindow::Thirty),
            Topic::ChainlinkTwap(TwapWindow::Sixty),
        ] {
            assert_eq!(Topic::from_wire(topic.as_wire()), Some(topic));
        }
    }

    #[test]
    fn unknown_wire_strings_are_rejected() {
        // Batch poisoning: one unrecognised topic zeroes an entire
        // subscription. An unknown topic must never become a Topic value.
        assert_eq!(Topic::from_wire("equity_prices"), None);
        assert_eq!(Topic::from_wire(""), None);
    }

    #[test]
    fn windows_carry_their_seconds() {
        assert_eq!(TwapWindow::Thirty.seconds(), 30);
        assert_eq!(TwapWindow::Sixty.seconds(), 60);
        assert_eq!(TwapWindow::from_seconds(30), Some(TwapWindow::Thirty));
        assert_eq!(TwapWindow::from_seconds(60), Some(TwapWindow::Sixty));
        assert_eq!(TwapWindow::from_seconds(45), None);
    }

    #[test]
    fn only_twap_topics_have_a_window() {
        assert_eq!(Topic::BinanceSpot.window(), None);
        assert_eq!(Topic::ChainlinkSpot.window(), None);
        assert_eq!(
            Topic::ChainlinkTwap(TwapWindow::Sixty).window(),
            Some(TwapWindow::Sixty)
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds topic`
Expected: FAIL — `cannot find type Topic in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `polyoxide-rtds/src/topic.rs`:

```rust
//! RTDS topic identifiers.
//!
//! [`Topic`] is a closed enum rather than a string on purpose. One
//! unrecognised topic in a subscription batch causes the venue to return zero
//! frames for **every** topic in that batch, answering only with a single
//! error frame. Making an invalid topic unrepresentable removes that failure
//! mode rather than documenting it.

/// Lookback window of a Chainlink TWAP feed.
///
/// These are lookback windows, not publication cadences — both windows publish
/// roughly once per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TwapWindow {
    /// 30-second lookback.
    Thirty,
    /// 60-second lookback.
    Sixty,
}

impl TwapWindow {
    /// The window length in seconds, as it appears in `payload.window_s`.
    pub fn seconds(self) -> u32 {
        match self {
            Self::Thirty => 30,
            Self::Sixty => 60,
        }
    }

    /// Parse a `payload.window_s` value. Returns `None` for any other length.
    pub fn from_seconds(seconds: u32) -> Option<Self> {
        match seconds {
            30 => Some(Self::Thirty),
            60 => Some(Self::Sixty),
            _ => None,
        }
    }
}

/// An RTDS topic.
///
/// Marked `#[non_exhaustive]` because upstream carries topics this crate does
/// not yet model (`equity_prices`, `comments`), and adding one later must not
/// be a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Topic {
    /// Binance spot prices (`crypto_prices`). Symbols look like `btcusdt`.
    ///
    /// On this topic `full_accuracy_value` is a **plain decimal**, unlike
    /// every other topic here.
    BinanceSpot,
    /// Chainlink spot prices (`crypto_prices_chainlink`). Symbols look like
    /// `btc/usd`. `full_accuracy_value` is E18 fixed-point.
    ChainlinkSpot,
    /// Chainlink time-weighted average prices. `full_accuracy_value` is E18
    /// fixed-point and the payload carries `window_s`.
    ChainlinkTwap(TwapWindow),
}

impl Topic {
    /// The exact string the venue expects in a subscription frame.
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::BinanceSpot => "crypto_prices",
            Self::ChainlinkSpot => "crypto_prices_chainlink",
            Self::ChainlinkTwap(TwapWindow::Thirty) => "crypto_prices_twap_thirty",
            Self::ChainlinkTwap(TwapWindow::Sixty) => "crypto_prices_twap_sixty",
        }
    }

    /// Parse a topic from an incoming frame's `topic` field.
    ///
    /// Returns `None` for topics this crate does not model, so a frame from an
    /// unmodelled topic is skipped rather than misparsed.
    pub fn from_wire(wire: &str) -> Option<Self> {
        match wire {
            "crypto_prices" => Some(Self::BinanceSpot),
            "crypto_prices_chainlink" => Some(Self::ChainlinkSpot),
            "crypto_prices_twap_thirty" => Some(Self::ChainlinkTwap(TwapWindow::Thirty)),
            "crypto_prices_twap_sixty" => Some(Self::ChainlinkTwap(TwapWindow::Sixty)),
            _ => None,
        }
    }

    /// The TWAP window, for TWAP topics only.
    pub fn window(self) -> Option<TwapWindow> {
        match self {
            Self::ChainlinkTwap(window) => Some(window),
            _ => None,
        }
    }
}
```

Add to `polyoxide-rtds/src/lib.rs`, after the `#![warn(missing_docs)]` line:

```rust
pub mod topic;

pub use topic::{Topic, TwapWindow};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds topic`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/src/topic.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): add Topic and TwapWindow with wire mappings"
```

---

### Task 3: Subscription and filter encoding

**Files:**
- Create: `polyoxide-rtds/src/subscription.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

This is the task that defuses the whitespace trap. A filter with one stray space — `{"symbol": "btc/usd"}` — still delivers the subscribe backfill and then goes permanently silent on updates, with no error. The public API therefore never accepts a filter string.

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-rtds/src/subscription.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::TwapWindow;

    #[test]
    fn filter_is_compact_json_with_no_spaces() {
        // A single space here makes the venue deliver the snapshot and then
        // fall silent forever. The encoding must be exact.
        let sub = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
            .symbol("btc/usd");
        let request = SubscriptionRequest::new([sub]);
        let json = serde_json::to_string(&request).unwrap();

        assert!(
            json.contains(r#""filters":"{\"symbol\":\"btc/usd\"}""#),
            "filter must be compact JSON, got: {json}"
        );
        assert!(
            !json.contains(r#"{\"symbol\": \"btc/usd\"}"#),
            "a space in the filter silently kills updates: {json}"
        );
    }

    #[test]
    fn request_matches_the_documented_envelope() {
        let sub = Subscription::for_topic(Topic::ChainlinkSpot).symbol("eth/usd");
        let value = serde_json::to_value(SubscriptionRequest::new([sub])).unwrap();

        assert_eq!(value["action"], "subscribe");
        assert_eq!(value["subscriptions"][0]["topic"], "crypto_prices_chainlink");
        assert_eq!(value["subscriptions"][0]["type"], "update");
        assert_eq!(
            value["subscriptions"][0]["filters"],
            r#"{"symbol":"eth/usd"}"#
        );
    }

    #[test]
    fn an_unfiltered_subscription_omits_filters_entirely() {
        let value = serde_json::to_value(SubscriptionRequest::new([
            Subscription::for_topic(Topic::BinanceSpot),
        ]))
        .unwrap();

        assert!(
            value["subscriptions"][0]
                .as_object()
                .unwrap()
                .get("filters")
                .is_none(),
            "omit filters to receive every symbol; null is not the same request"
        );
    }

    #[test]
    fn multiple_symbols_fan_out_into_separate_entries() {
        // `{"symbol":["btc/usd","eth/usd"]}` returns zero frames. One symbol
        // per subscription entry is the only form the venue accepts.
        let subs = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Sixty))
            .symbols(["btc/usd", "eth/usd"]);
        assert_eq!(subs.len(), 2);

        let value = serde_json::to_value(SubscriptionRequest::new(subs)).unwrap();
        let entries = value["subscriptions"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["filters"], r#"{"symbol":"btc/usd"}"#);
        assert_eq!(entries[1]["filters"], r#"{"symbol":"eth/usd"}"#);
        assert_eq!(entries[0]["topic"], "crypto_prices_twap_sixty");
        assert_eq!(entries[1]["topic"], "crypto_prices_twap_sixty");
    }

    #[test]
    fn symbols_with_no_arguments_yields_no_subscriptions() {
        let subs = Subscription::for_topic(Topic::BinanceSpot).symbols(Vec::<String>::new());
        assert!(
            subs.is_empty(),
            "an empty symbol list must not silently become an unfiltered subscription"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds subscription`
Expected: FAIL — `cannot find type Subscription in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `polyoxide-rtds/src/subscription.rs`:

```rust
//! Subscription frames.
//!
//! The venue's `filters` field is a JSON document embedded in a JSON string,
//! and it is unforgiving: `{"symbol": "btc/usd"}` — one space — still delivers
//! the subscribe backfill and then never sends another update, with no error
//! of any kind. This module therefore never accepts a caller-supplied filter
//! string; it builds one with [`serde_json`], which is compact by
//! construction.

use serde::Serialize;

use crate::topic::Topic;

/// The symbol filter, serialised as compact JSON into the `filters` string.
#[derive(Debug, Serialize)]
struct SymbolFilter<'a> {
    symbol: &'a str,
}

/// One topic subscription, optionally narrowed to a single symbol.
///
/// The venue accepts at most one symbol per entry — an array of symbols
/// returns zero frames — so several symbols means several entries. Build them
/// with [`Subscription::symbols`], which fans out for you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    topic: Topic,
    symbol: Option<String>,
}

impl Subscription {
    /// Subscribe to every symbol on a topic.
    pub fn for_topic(topic: Topic) -> Self {
        Self {
            topic,
            symbol: None,
        }
    }

    /// Narrow this subscription to one symbol.
    ///
    /// Symbol format is topic-specific: `btcusdt` for
    /// [`Topic::BinanceSpot`], `btc/usd` for the Chainlink topics. Matching is
    /// case-insensitive despite the upstream documentation's claim otherwise.
    pub fn symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    /// Fan out into one subscription per symbol.
    ///
    /// Returns an empty vector for an empty input, which is deliberate: an
    /// empty list must not collapse into an unfiltered subscription for every
    /// symbol on the topic.
    pub fn symbols<I, S>(self, symbols: I) -> Vec<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        symbols
            .into_iter()
            .map(|symbol| Self {
                topic: self.topic,
                symbol: Some(symbol.into()),
            })
            .collect()
    }

    /// The topic this subscription covers.
    pub fn topic(&self) -> Topic {
        self.topic
    }

    /// The symbol this subscription is narrowed to, if any.
    pub fn symbol_filter(&self) -> Option<&str> {
        self.symbol.as_deref()
    }
}

impl Serialize for Subscription {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut entry = serializer.serialize_struct("Subscription", 3)?;
        entry.serialize_field("topic", self.topic.as_wire())?;
        entry.serialize_field("type", "update")?;
        if let Some(symbol) = &self.symbol {
            // `to_string` never emits spaces, which is exactly the property
            // the venue requires and hand-written filters get wrong.
            let filters = serde_json::to_string(&SymbolFilter { symbol })
                .map_err(serde::ser::Error::custom)?;
            entry.serialize_field("filters", &filters)?;
        } else {
            entry.skip_field("filters")?;
        }
        entry.end()
    }
}

/// The frame sent immediately after connecting.
#[derive(Debug, Serialize)]
pub struct SubscriptionRequest {
    action: &'static str,
    subscriptions: Vec<Subscription>,
}

impl SubscriptionRequest {
    /// Build a subscribe frame from a set of subscriptions.
    pub fn new(subscriptions: impl IntoIterator<Item = Subscription>) -> Self {
        Self {
            action: "subscribe",
            subscriptions: subscriptions.into_iter().collect(),
        }
    }

    /// The subscriptions carried by this frame.
    pub fn subscriptions(&self) -> &[Subscription] {
        &self.subscriptions
    }
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod subscription;

pub use subscription::{Subscription, SubscriptionRequest};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds subscription`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/src/subscription.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): build subscription filters as compact JSON"
```

---

### Task 4: Error type

> **AMENDED during execution.** The `is_recoverable() -> bool` shown below was
> replaced by a three-way `recovery() -> Recovery` (`Reconnect` / `SkipFrame` /
> `Fatal`), and `Json` now retains the frame text. Two reasons, both found in
> review:
>
> 1. One boolean answered two questions. `false` did not mean "don't reconnect",
>    it meant "terminate the feed" — so a single unparseable frame killed a 24/7
>    price feed. `SkipFrame` is the missing third outcome, and it is what
>    `Json` and `Precision` actually warrant.
> 2. `Connection` was recoverable unconditionally, but
>    `tungstenite::Error::{Url, Tls, Http, AlreadyClosed, AttackAttempt}` are
>    permanent — a bad certificate retried forever at full backoff, which is
>    the exact failure this classifier exists to prevent.
>
> Tasks 8, 9 and 11 below were updated to match. The code in *this* section is
> left as originally written, for the record.

**Files:**
- Create: `polyoxide-rtds/src/error.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-rtds/src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rejected_subscription_is_never_recoverable() {
        // Reconnecting into a subscription the venue rejects replays the same
        // rejection forever, at full backoff, silently.
        let err = RtdsError::Server {
            status_code: 401,
            message: "topic: nope and type: update not found".into(),
        };
        assert!(!err.is_recoverable());
    }

    #[test]
    fn transport_failures_are_recoverable() {
        assert!(RtdsError::ConnectionClosed.is_recoverable());
        assert!(RtdsError::Stalled {
            elapsed: std::time::Duration::from_secs(30)
        }
        .is_recoverable());
    }

    #[test]
    fn decode_failures_are_not_recoverable() {
        // A value we cannot represent will not become representable on a
        // reconnect, and a malformed frame means our model is wrong.
        assert!(!RtdsError::Precision {
            raw: "1".repeat(40),
            topic: crate::Topic::ChainlinkSpot,
        }
        .is_recoverable());
    }

    #[test]
    fn server_errors_render_both_status_and_message() {
        let err = RtdsError::Server {
            status_code: 401,
            message: "not found".into(),
        };
        let rendered = err.to_string();
        assert!(rendered.contains("401"), "{rendered}");
        assert!(rendered.contains("not found"), "{rendered}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds error`
Expected: FAIL — `cannot find type RtdsError in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `polyoxide-rtds/src/error.rs`:

```rust
//! Errors produced by the RTDS client.

use std::time::Duration;

use thiserror::Error;

use crate::topic::Topic;

/// An error from the RTDS stream.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RtdsError {
    /// The underlying WebSocket transport failed.
    #[error("RTDS connection error: {0}")]
    Connection(Box<tokio_tungstenite::tungstenite::Error>),

    /// A frame could not be parsed.
    #[error("RTDS JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The connection closed.
    #[error("RTDS connection closed")]
    ConnectionClosed,

    /// The venue rejected the subscription.
    ///
    /// Arrives in an envelope of its own — `{"body":{"message":…},
    /// "statusCode":N}` — that shares no fields with a data frame. The status
    /// is not trustworthy: an unrecognised topic reports `401` for what the
    /// message body describes as a not-found.
    ///
    /// Never recoverable. One rejected topic zeroes every topic in the same
    /// batch, so retrying replays the same silence.
    #[error("RTDS rejected the subscription (status {status_code}): {message}")]
    Server {
        /// The `statusCode` field, reported verbatim.
        status_code: u16,
        /// The `body.message` field.
        message: String,
    },

    /// A price could not be represented as a [`Decimal`](rust_decimal::Decimal).
    #[error("RTDS value {raw} on topic {topic:?} does not fit a Decimal")]
    Precision {
        /// The undecoded wire value.
        raw: String,
        /// The topic it arrived on.
        topic: Topic,
    },

    /// No frame arrived on any subscription within the configured window.
    ///
    /// RTDS never answers a ping, so a half-open socket is indistinguishable
    /// from a quiet market except by timing the gap between updates.
    #[error("RTDS stream stalled: no frames for {elapsed:?}")]
    Stalled {
        /// How long the stream was silent.
        elapsed: Duration,
    },

    /// The endpoint URL could not be parsed.
    #[error("RTDS URL parse error: {0}")]
    Url(#[from] url::ParseError),
}

impl From<tokio_tungstenite::tungstenite::Error> for RtdsError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(Box::new(err))
    }
}

impl RtdsError {
    /// Whether reconnecting could plausibly succeed.
    ///
    /// This gates the supervisor's retry loop. Without it, a rejected
    /// subscription becomes an infinite reconnect at full backoff with no
    /// output — the same failure the error was meant to surface.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Connection(_) | Self::ConnectionClosed | Self::Stalled { .. } => true,
            Self::Server { .. } | Self::Precision { .. } | Self::Json(_) | Self::Url(_) => false,
        }
    }
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod error;

pub use error::RtdsError;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds error`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/src/error.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): add RtdsError with a recoverability classifier"
```

---

### Task 5: Decimal decoding

**Files:**
- Create: `polyoxide-rtds/src/decode.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

The only place in the crate where a scale is applied. Keeping it in one file makes the E18-vs-plain split auditable.

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-rtds/src/decode.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::{Topic, TwapWindow};

    #[test]
    fn e18_decodes_a_captured_twap_value_exactly() {
        // Captured 2026-09-05 from crypto_prices_twap_thirty, btc/usd.
        let value = decode_e18(
            "79697474565615044788224",
            Topic::ChainlinkTwap(TwapWindow::Thirty),
        )
        .unwrap();
        assert_eq!(value.to_string(), "79697.474565615044788224");
    }

    #[test]
    fn plain_decodes_a_captured_binance_value_exactly() {
        // Captured 2026-09-05 from crypto_prices, btcusdt. NOT E18.
        let value = decode_plain("79697.73000000", Topic::BinanceSpot).unwrap();
        assert_eq!(value.normalize().to_string(), "79697.73");
    }

    #[test]
    fn e18_refuses_an_out_of_range_value_instead_of_panicking() {
        // Decimal's 96-bit mantissa caps out around 7.9e28. The panicking
        // constructor would abort a caller's task; this must not.
        let err = decode_e18(&"9".repeat(30), Topic::ChainlinkSpot).unwrap_err();
        assert!(matches!(err, RtdsError::Precision { .. }), "{err:?}");
    }

    #[test]
    fn e18_refuses_a_non_integer_value() {
        // A plain-decimal string on an E18 topic means our topic mapping is
        // wrong. Fail loudly rather than guess.
        let err = decode_e18("79697.73000000", Topic::ChainlinkSpot).unwrap_err();
        assert!(matches!(err, RtdsError::Precision { .. }), "{err:?}");
    }

    #[test]
    fn e18_handles_negative_values() {
        // Upstream describes the field as a signed fixed-point value.
        let value = decode_e18("-1500000000000000000", Topic::ChainlinkSpot).unwrap();
        assert_eq!(value.normalize().to_string(), "-1.5");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds decode`
Expected: FAIL — `cannot find function decode_e18 in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `polyoxide-rtds/src/decode.rs`:

```rust
//! Decoding `full_accuracy_value` into an exact [`Decimal`].
//!
//! The scale of that field **varies by topic**: it is E18 fixed-point on the
//! Chainlink topics and a plain decimal on Binance. Both parse cleanly as
//! decimal strings, so applying the wrong one is silent — a $79,697 Binance
//! price decoded as E18 becomes $0.00000000000008. Every scale decision in
//! this crate happens in this file.

use std::str::FromStr;

use rust_decimal::Decimal;

use crate::{error::RtdsError, topic::Topic};

/// Number of decimal places in Chainlink's fixed-point encoding.
const E18_SCALE: u32 = 18;

/// Decode an E18 fixed-point value, as sent on the Chainlink topics.
///
/// Uses the fallible constructor deliberately: the panicking
/// `from_i128_with_scale` would abort the caller's task on a value out of
/// range, and a price feed must not be able to do that.
pub fn decode_e18(raw: &str, topic: Topic) -> Result<Decimal, RtdsError> {
    let precision_error = || RtdsError::Precision {
        raw: raw.to_string(),
        topic,
    };

    let units: i128 = raw.parse().map_err(|_| precision_error())?;
    Decimal::try_from_i128_with_scale(units, E18_SCALE).map_err(|_| precision_error())
}

/// Decode a plain decimal value, as sent on [`Topic::BinanceSpot`].
pub fn decode_plain(raw: &str, topic: Topic) -> Result<Decimal, RtdsError> {
    Decimal::from_str(raw).map_err(|_| RtdsError::Precision {
        raw: raw.to_string(),
        topic,
    })
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod decode;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds decode`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/src/decode.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): decode E18 and plain decimal values without panicking"
```

---

### Task 6: Captured fixtures

**Files:**
- Create: `polyoxide-rtds/src/fixtures.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

These frames are the golden vectors for every later task. They were captured verbatim on 2026-09-05 and must not be edited to make a test pass.

- [ ] **Step 1: Create the fixtures module**

`polyoxide-rtds/src/fixtures.rs`:

```rust
//! Frames captured verbatim from `wss://ws-live-data.polymarket.com` on
//! 2026-09-05.
//!
//! Do not edit these to make a test pass. They are what the venue actually
//! sent, and several of them contradict Polymarket's published documentation —
//! see `docs/specs/rtds/OBSERVED.md`. Snapshot fixtures have had their `data`
//! arrays truncated to three points; the original lengths are noted on each.

/// Binance spot update. `full_accuracy_value` is a **plain decimal**.
pub const BINANCE_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697.73000000","symbol":"btcusdt","timestamp":1788600389000,"value":79697.73},"timestamp":1788600389154,"topic":"crypto_prices","type":"update"}"#;

/// Chainlink spot update. `full_accuracy_value` is **E18**.
///
/// Captured roughly one second after [`BINANCE_UPDATE`], reporting the same
/// asset at the same price, with byte-identical payload keys. The pair is the
/// evidence that these two topics cannot share a type.
pub const CHAINLINK_SPOT_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79696948174287960000000","symbol":"btc/usd","timestamp":1788600388000,"value":79696.94817428796},"timestamp":1788600389451,"topic":"crypto_prices_chainlink","type":"update"}"#;

/// 30-second TWAP update. E18, plus `window_s`.
pub const TWAP_THIRTY_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697474565615044788224","symbol":"btc/usd","timestamp":1788600388000,"value":79697.47456561505,"window_s":30},"timestamp":1788600389537,"topic":"crypto_prices_twap_thirty","type":"update"}"#;

/// 60-second TWAP update.
pub const TWAP_SIXTY_UPDATE: &str = r#"{"connection_id":"gZexFa6cUWeIKEiTDA==","payload":{"full_accuracy_value":"79697575317474428059648","symbol":"btc/usd","timestamp":1788600388000,"value":79697.57531747443,"window_s":60},"timestamp":1788600389495,"topic":"crypto_prices_twap_sixty","type":"update"}"#;

/// TWAP snapshot, truncated from 55 points.
///
/// Points carry `full_accuracy_value`, and the payload carries `window_s`.
/// Note the envelope has **no** `connection_id` — snapshots never do.
pub const TWAP_THIRTY_SNAPSHOT: &str = r#"{"payload":{"data":[{"full_accuracy_value":"79696840994573453885440","timestamp":1788600329000,"value":79696.84099457346},{"full_accuracy_value":"79696885010084155883520","timestamp":1788600330000,"value":79696.88501008415},{"full_accuracy_value":"79696928978311781023744","timestamp":1788600331000,"value":79696.92897831179}],"symbol":"btc/usd","window_s":30},"timestamp":1788600388753,"topic":"crypto_prices_twap_thirty","type":"subscribe"}"#;

/// Binance snapshot, truncated from 120 points.
///
/// Points carry **no** `full_accuracy_value`, so no exact value exists in this
/// backfill, and the payload carries no `window_s`.
pub const BINANCE_SNAPSHOT: &str = r#"{"payload":{"data":[{"timestamp":1788600269000,"value":79697.73},{"timestamp":1788600270000,"value":79697.73},{"timestamp":1788600271000,"value":79697.73}],"symbol":"btcusdt"},"timestamp":1788600388752,"topic":"crypto_prices","type":"subscribe"}"#;

/// The error frame produced by including one unrecognised topic in an
/// otherwise valid five-topic batch. All five topics returned zero frames.
pub const REJECTED_SUBSCRIPTION: &str = r#"{"body":{"message":"leger GetTopics error: rpc error: code = NotFound desc = topic: definitely_not_a_topic and type: update not found, status: rpc error: code = NotFound desc = topic: definitely_not_a_topic and type: update not found, message: topic: definitely_not_a_topic and type: update not found"},"statusCode":401}"#;

/// The empty text frame RTDS sends immediately after the connection opens.
pub const EMPTY_GREETING: &str = "";
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
#[cfg(test)]
pub(crate) mod fixtures;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo test -p polyoxide-rtds --no-run`
Expected: `Finished`, no warnings.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/src/fixtures.rs polyoxide-rtds/src/lib.rs
git commit -m "test(rtds): add frames captured verbatim from the live host"
```

---

### Task 7: Payload types

**Files:**
- Create: `polyoxide-rtds/src/payload.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

- [ ] **Step 1: Write the implementation**

There is no separate failing-test step here: these are plain data structs with no behaviour, and Task 8 tests them through the dispatcher that constructs them. Writing tests that only assert field assignment would be the shape-only testing this design exists to avoid.

`polyoxide-rtds/src/payload.rs`:

```rust
//! Per-topic payload types.
//!
//! [`BinanceUpdate`] and [`ChainlinkSpotUpdate`] carry identical fields and
//! are deliberately **not** merged. Their `full_accuracy_value` differs in
//! scale — plain decimal versus E18 — and keeping two structurally identical
//! things apart is the one thing a type can do that a runtime scale tag
//! cannot.

use rust_decimal::Decimal;

use crate::topic::TwapWindow;

/// A Binance spot price update, from `crypto_prices`.
#[derive(Debug, Clone, PartialEq)]
pub struct BinanceUpdate {
    /// Venue symbol, e.g. `btcusdt`. Always lowercase on the wire.
    pub symbol: String,
    /// Venue observation time, Unix milliseconds (`payload.timestamp`).
    pub observed_at: i64,
    /// When the publisher submitted this to RTDS, Unix milliseconds.
    pub published_at: i64,
    /// Server-side connection handle. Undocumented; present on updates only.
    pub connection_id: Option<String>,
    /// Exact price, decoded from the **plain decimal** `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded `full_accuracy_value`, retained for auditing.
    pub raw: String,
    /// The venue's lossy float. For display only — never use it for
    /// arithmetic; use [`value`](Self::value).
    pub display_value: f64,
}

/// A Chainlink spot price update, from `crypto_prices_chainlink`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainlinkSpotUpdate {
    /// Chainlink symbol, e.g. `btc/usd`.
    pub symbol: String,
    /// Chainlink observation time, Unix milliseconds.
    pub observed_at: i64,
    /// When the publisher submitted this to RTDS, Unix milliseconds.
    pub published_at: i64,
    /// Server-side connection handle.
    pub connection_id: Option<String>,
    /// Exact price, decoded from the **E18** `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded E18 integer string, retained for auditing.
    pub raw: String,
    /// The venue's lossy float. For display only.
    pub display_value: f64,
}

/// A Chainlink TWAP update, from `crypto_prices_twap_thirty` or
/// `crypto_prices_twap_sixty`.
#[derive(Debug, Clone, PartialEq)]
pub struct TwapUpdate {
    /// Chainlink symbol, e.g. `btc/usd`.
    pub symbol: String,
    /// Lookback window. Not optional: this type only exists for TWAP frames,
    /// which always carry `window_s`.
    pub window: TwapWindow,
    /// Chainlink observation time, Unix milliseconds.
    pub observed_at: i64,
    /// When the publisher submitted this to RTDS, Unix milliseconds.
    pub published_at: i64,
    /// Server-side connection handle.
    pub connection_id: Option<String>,
    /// Exact price, decoded from the **E18** `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded E18 integer string, retained for auditing.
    pub raw: String,
    /// The venue's lossy float. For display only.
    pub display_value: f64,
}

/// One point in a backfill that carries no exact value.
///
/// Binance and Chainlink-spot snapshots omit `full_accuracy_value` entirely,
/// so the float is all there is.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayPoint {
    /// Observation time, Unix milliseconds.
    pub observed_at: i64,
    /// The venue's lossy float — the only value available on these points.
    pub display_value: f64,
}

/// One point in a backfill that carries an exact value.
#[derive(Debug, Clone, PartialEq)]
pub struct ExactPoint {
    /// Observation time, Unix milliseconds.
    pub observed_at: i64,
    /// Exact price, decoded from the E18 `full_accuracy_value`.
    pub value: Decimal,
    /// The undecoded E18 integer string.
    pub raw_e18: String,
    /// The venue's lossy float. For display only.
    pub display_value: f64,
}

/// The points of a backfill, whose shape depends on the topic.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SnapshotPoints {
    /// Binance and Chainlink spot. No exact values exist in these backfills.
    DisplayOnly(Vec<DisplayPoint>),
    /// Chainlink TWAP. Exact values present.
    Exact(Vec<ExactPoint>),
}

impl SnapshotPoints {
    /// How many points the backfill carried.
    pub fn len(&self) -> usize {
        match self {
            Self::DisplayOnly(points) => points.len(),
            Self::Exact(points) => points.len(),
        }
    }

    /// Whether the backfill was empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The backfill RTDS sends once, immediately after each subscribe.
///
/// Upstream documents that no snapshot exists; it does. Every resubscribe
/// replays one, so a reconnect re-initialises caller state rather than merely
/// resuming the feed — and callers therefore see this event more than once.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The topic backfilled.
    pub topic: crate::topic::Topic,
    /// The symbol backfilled.
    pub symbol: String,
    /// When RTDS sent the backfill, Unix milliseconds.
    pub published_at: i64,
    /// The points, roughly one per second — about 55-59 for the Chainlink
    /// topics and 120 for Binance.
    pub points: SnapshotPoints,
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod payload;

pub use payload::{
    BinanceUpdate, ChainlinkSpotUpdate, DisplayPoint, ExactPoint, Snapshot, SnapshotPoints,
    TwapUpdate,
};
```

- [ ] **Step 2: Verify it compiles cleanly**

Run: `cargo clippy -p polyoxide-rtds --all-targets -- -D warnings`
Expected: `Finished`, no warnings. (`clippy::len_without_is_empty` is why `SnapshotPoints::is_empty` exists.)

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/src/payload.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): add per-topic payload types"
```

---

### Task 8: Frame dispatch and the differential scale test

**Files:**
- Create: `polyoxide-rtds/src/event.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

This is the central task. The differential test in Step 1 is the one that holds the whole design up — it fails the moment anyone merges the two spot types or copies the wrong scale.

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-rtds/src/event.rs`:

```rust
#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use std::str::FromStr;

    use super::*;
    use crate::{decode::decode_e18, fixtures, payload::SnapshotPoints, topic::TwapWindow};

    fn update(frame: &str) -> PriceUpdate {
        match PriceEvent::from_json(frame) {
            Ok(Some(PriceEvent::Update(update))) => update,
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn the_two_spot_topics_do_not_share_a_scale() {
        // THE test. These frames were captured about one second apart, both
        // reporting BTC at roughly $79,697, with byte-identical payload keys.
        // Only the scale of `full_accuracy_value` differs. A shape-only
        // assertion — "value is a positive Decimal" — passes on both and
        // proves nothing.
        let binance = update(fixtures::BINANCE_UPDATE);
        let chainlink = update(fixtures::CHAINLINK_SPOT_UPDATE);

        assert!(
            (binance.value() - chainlink.value()).abs() < Decimal::from(100u32),
            "same asset, same moment, but got {} vs {}",
            binance.value(),
            chainlink.value()
        );

        // Decoding Chainlink's raw string the way Binance's must be decoded —
        // as a plain decimal — silently yields 7.9e22 instead of 7.9e4. The
        // ratio is exactly 10^18. This is the bug the split types prevent.
        let misdecoded = Decimal::from_str(chainlink.raw()).unwrap();
        assert_eq!(
            (misdecoded / chainlink.value()).normalize(),
            Decimal::from(1_000_000_000_000_000_000u64),
            "misdecoding an E18 value as plain must be off by exactly 10^18"
        );

        // The reverse misdecode fails loudly instead of silently: Binance's
        // raw value is not an integer, so the E18 path cannot parse it at all.
        assert!(decode_e18(binance.raw(), Topic::BinanceSpot).is_err());
    }

    #[test]
    fn binance_decodes_to_its_face_value() {
        let binance = update(fixtures::BINANCE_UPDATE);
        assert_eq!(
            binance.value().normalize().to_string(),
            "79697.73",
            "if this reads 0.00000000000008, the E18 scale leaked onto Binance"
        );
        assert_eq!(binance.symbol(), "btcusdt");
        assert_eq!(binance.observed_at(), 1788600389000);
        assert_eq!(binance.published_at(), 1788600389154);
        assert_eq!(binance.window(), None);
        assert_eq!(binance.topic(), Topic::BinanceSpot);
    }

    #[test]
    fn chainlink_spot_decodes_from_e18() {
        let spot = update(fixtures::CHAINLINK_SPOT_UPDATE);
        assert_eq!(spot.value().to_string(), "79696.948174287960000000");
        assert_eq!(spot.symbol(), "btc/usd");
        assert_eq!(spot.window(), None);
        assert_eq!(spot.topic(), Topic::ChainlinkSpot);
    }

    #[test]
    fn twap_carries_its_window_and_decodes_from_e18() {
        let thirty = update(fixtures::TWAP_THIRTY_UPDATE);
        assert_eq!(thirty.value().to_string(), "79697.474565615044788224");
        assert_eq!(thirty.window(), Some(TwapWindow::Thirty));
        assert_eq!(thirty.topic(), Topic::ChainlinkTwap(TwapWindow::Thirty));

        let sixty = update(fixtures::TWAP_SIXTY_UPDATE);
        assert_eq!(sixty.window(), Some(TwapWindow::Sixty));
        assert_eq!(sixty.topic(), Topic::ChainlinkTwap(TwapWindow::Sixty));
    }

    #[test]
    fn updates_retain_the_undocumented_connection_id() {
        let update = update(fixtures::TWAP_THIRTY_UPDATE);
        assert_eq!(update.connection_id(), Some("gZexFa6cUWeIKEiTDA=="));
    }

    #[test]
    fn twap_snapshots_carry_exact_values() {
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::TWAP_THIRTY_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(snapshot.topic, Topic::ChainlinkTwap(TwapWindow::Thirty));
        assert_eq!(snapshot.symbol, "btc/usd");
        let SnapshotPoints::Exact(points) = &snapshot.points else {
            panic!("TWAP backfills carry full_accuracy_value, so they are Exact");
        };
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value.to_string(), "79696.840994573453885440");
    }

    #[test]
    fn a_mislabelled_chainlink_spot_snapshot_is_attributed_correctly() {
        // The venue labels this backfill `crypto_prices` (Binance) even
        // though it was produced by a `crypto_prices_chainlink` subscription.
        // Taking the label at face value files Chainlink prices under
        // Binance, silently — the points parse either way, because both spot
        // snapshots are display-only.
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::CHAINLINK_SPOT_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(
            snapshot.topic,
            Topic::ChainlinkSpot,
            "a slash in the symbol is the only thing distinguishing this from \
             a Binance backfill"
        );
        assert_eq!(snapshot.symbol, "btc/usd");

        // A genuine Binance backfill must be left alone.
        let Ok(Some(PriceEvent::Snapshot(binance))) =
            PriceEvent::from_json(fixtures::BINANCE_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };
        assert_eq!(binance.topic, Topic::BinanceSpot);
        assert_eq!(binance.symbol, "btcusdt");
    }

    #[test]
    fn spot_snapshots_have_no_exact_values_to_offer() {
        // Binance and Chainlink-spot backfills omit full_accuracy_value
        // entirely. Modelling them as Exact-with-Option would invent a value
        // the venue never sent.
        let Ok(Some(PriceEvent::Snapshot(snapshot))) =
            PriceEvent::from_json(fixtures::BINANCE_SNAPSHOT)
        else {
            panic!("expected a snapshot");
        };

        assert_eq!(snapshot.topic, Topic::BinanceSpot);
        let SnapshotPoints::DisplayOnly(points) = &snapshot.points else {
            panic!("Binance backfills carry no exact values");
        };
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].display_value, 79697.73);
    }

    #[test]
    fn a_rejected_subscription_surfaces_as_an_error_not_silence() {
        // If this frame were dropped, a poisoned batch would be
        // indistinguishable from an idle feed.
        let err = PriceEvent::from_json(fixtures::REJECTED_SUBSCRIPTION).unwrap_err();
        match err {
            RtdsError::Server {
                status_code,
                ref message,
            } => {
                assert_eq!(status_code, 401);
                assert!(message.contains("definitely_not_a_topic"), "{message}");
            }
            other => panic!("expected Server, got {other:?}"),
        }
        assert_eq!(err.recovery(), Recovery::Fatal);
    }

    #[test]
    fn keepalive_and_greeting_frames_are_skipped() {
        for frame in [fixtures::EMPTY_GREETING, "PONG", "{}"] {
            assert!(
                matches!(PriceEvent::from_json(frame), Ok(None)),
                "frame {frame:?} should be skipped, not surfaced or errored"
            );
        }
    }

    #[test]
    fn frames_from_unmodelled_topics_are_skipped() {
        // equity_prices and comments ride the same connection. Receiving one
        // must not kill the stream.
        let frame = r#"{"topic":"equity_prices","type":"update","timestamp":1,
            "payload":{"symbol":"aapl","timestamp":1,"value":189.42}}"#;
        assert!(matches!(PriceEvent::from_json(frame), Ok(None)));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds event`
Expected: FAIL — `cannot find type PriceEvent in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `polyoxide-rtds/src/event.rs`:

```rust
//! Stream events and frame dispatch.

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    decode::{decode_e18, decode_plain},
    error::RtdsError,
    payload::{
        BinanceUpdate, ChainlinkSpotUpdate, DisplayPoint, ExactPoint, Snapshot, SnapshotPoints,
        TwapUpdate,
    },
    topic::{Topic, TwapWindow},
};

/// A data frame's envelope, before topic-specific interpretation.
#[derive(Deserialize)]
struct RawFrame {
    topic: String,
    #[serde(rename = "type")]
    kind: String,
    timestamp: i64,
    #[serde(default)]
    connection_id: Option<String>,
    payload: serde_json::Value,
}

/// An update frame's payload.
#[derive(Deserialize)]
struct RawUpdatePayload {
    symbol: String,
    timestamp: i64,
    value: f64,
    full_accuracy_value: String,
    #[serde(default)]
    window_s: Option<u32>,
}

/// A backfill frame's payload.
#[derive(Deserialize)]
struct RawSnapshotPayload {
    symbol: String,
    data: Vec<RawSnapshotPoint>,
}

#[derive(Deserialize)]
struct RawSnapshotPoint {
    timestamp: i64,
    value: f64,
    #[serde(default)]
    full_accuracy_value: Option<String>,
}

/// The venue's error envelope, which shares no fields with a data frame.
#[derive(Deserialize)]
struct RawServerError {
    #[serde(rename = "statusCode")]
    status_code: u16,
    body: RawServerErrorBody,
}

#[derive(Deserialize)]
struct RawServerErrorBody {
    message: String,
}

/// A live price update, typed by its topic.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PriceUpdate {
    /// Binance spot. Its exact value came from a plain decimal.
    Binance(BinanceUpdate),
    /// Chainlink spot. Its exact value came from an E18 integer.
    ChainlinkSpot(ChainlinkSpotUpdate),
    /// Chainlink TWAP. Its exact value came from an E18 integer.
    Twap(TwapUpdate),
}

impl PriceUpdate {
    /// The topic this update arrived on.
    pub fn topic(&self) -> Topic {
        match self {
            Self::Binance(_) => Topic::BinanceSpot,
            Self::ChainlinkSpot(_) => Topic::ChainlinkSpot,
            Self::Twap(update) => Topic::ChainlinkTwap(update.window),
        }
    }

    /// The symbol, in whatever format the topic uses.
    pub fn symbol(&self) -> &str {
        match self {
            Self::Binance(u) => &u.symbol,
            Self::ChainlinkSpot(u) => &u.symbol,
            Self::Twap(u) => &u.symbol,
        }
    }

    /// Venue observation time, Unix milliseconds.
    pub fn observed_at(&self) -> i64 {
        match self {
            Self::Binance(u) => u.observed_at,
            Self::ChainlinkSpot(u) => u.observed_at,
            Self::Twap(u) => u.observed_at,
        }
    }

    /// When RTDS received this update, Unix milliseconds.
    pub fn published_at(&self) -> i64 {
        match self {
            Self::Binance(u) => u.published_at,
            Self::ChainlinkSpot(u) => u.published_at,
            Self::Twap(u) => u.published_at,
        }
    }

    /// The exact price. Correctly scaled for the topic it arrived on.
    pub fn value(&self) -> Decimal {
        match self {
            Self::Binance(u) => u.value,
            Self::ChainlinkSpot(u) => u.value,
            Self::Twap(u) => u.value,
        }
    }

    /// The undecoded `full_accuracy_value`, retained for auditing.
    pub fn raw(&self) -> &str {
        match self {
            Self::Binance(u) => &u.raw,
            Self::ChainlinkSpot(u) => &u.raw,
            Self::Twap(u) => &u.raw,
        }
    }

    /// The server-side connection handle, if the frame carried one.
    pub fn connection_id(&self) -> Option<&str> {
        match self {
            Self::Binance(u) => u.connection_id.as_deref(),
            Self::ChainlinkSpot(u) => u.connection_id.as_deref(),
            Self::Twap(u) => u.connection_id.as_deref(),
        }
    }

    /// The TWAP lookback window, for TWAP updates only.
    pub fn window(&self) -> Option<TwapWindow> {
        match self {
            Self::Twap(update) => Some(update.window),
            _ => None,
        }
    }
}

/// An event from the RTDS stream.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PriceEvent {
    /// A live price update.
    Update(PriceUpdate),
    /// The backfill sent immediately after each subscribe. Seen again after
    /// every reconnect, because resubscribing replays it.
    Snapshot(Snapshot),
}

impl PriceEvent {
    /// Parse one text frame.
    ///
    /// Returns `Ok(None)` for frames that carry no event: the empty greeting
    /// sent at connect, keep-alive replies, and frames from topics this crate
    /// does not model. Returns `Err` for a rejected subscription, which must
    /// never be mistaken for an idle feed.
    pub fn from_json(text: &str) -> Result<Option<Self>, RtdsError> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed == "PONG" || trimmed == "{}" {
            return Ok(None);
        }

        // The error envelope shares no fields with a data frame, so try it
        // first rather than letting it fail as a malformed frame.
        if let Ok(error) = serde_json::from_str::<RawServerError>(trimmed) {
            return Err(RtdsError::Server {
                status_code: error.status_code,
                message: error.body.message,
            });
        }

        let frame: RawFrame =
            serde_json::from_str(trimmed).map_err(|e| RtdsError::json(trimmed, e))?;
        let Some(topic) = Topic::from_wire(&frame.topic) else {
            tracing::trace!(topic = %frame.topic, "skipping unmodelled RTDS topic");
            return Ok(None);
        };

        match frame.kind.as_str() {
            "update" => Ok(Some(Self::Update(parse_update(topic, &frame)?))),
            "subscribe" => Ok(Some(Self::Snapshot(parse_snapshot(topic, &frame)?))),
            other => {
                tracing::trace!(kind = %other, "skipping unmodelled RTDS frame type");
                Ok(None)
            }
        }
    }
}

fn parse_update(topic: Topic, frame: &RawFrame) -> Result<PriceUpdate, RtdsError> {
    let payload: RawUpdatePayload = serde_json::from_value(frame.payload.clone())
        .map_err(|e| RtdsError::json(frame.payload.to_string(), e))?;
    let raw = payload.full_accuracy_value;

    Ok(match topic {
        Topic::BinanceSpot => PriceUpdate::Binance(BinanceUpdate {
            value: decode_plain(&raw, topic)?,
            symbol: payload.symbol,
            observed_at: payload.timestamp,
            published_at: frame.timestamp,
            connection_id: frame.connection_id.clone(),
            display_value: payload.value,
            raw,
        }),
        Topic::ChainlinkSpot => PriceUpdate::ChainlinkSpot(ChainlinkSpotUpdate {
            value: decode_e18(&raw, topic)?,
            symbol: payload.symbol,
            observed_at: payload.timestamp,
            published_at: frame.timestamp,
            connection_id: frame.connection_id.clone(),
            display_value: payload.value,
            raw,
        }),
        Topic::ChainlinkTwap(window) => {
            // Trust the topic over `window_s` when they disagree: the topic is
            // what we subscribed to, and a mismatch means our model is wrong.
            if let Some(seconds) = payload.window_s {
                if TwapWindow::from_seconds(seconds) != Some(window) {
                    tracing::warn!(
                        expected = window.seconds(),
                        received = seconds,
                        "RTDS TWAP window_s disagrees with its topic"
                    );
                }
            }
            PriceUpdate::Twap(TwapUpdate {
                value: decode_e18(&raw, topic)?,
                symbol: payload.symbol,
                window,
                observed_at: payload.timestamp,
                published_at: frame.timestamp,
                connection_id: frame.connection_id.clone(),
                display_value: payload.value,
                raw,
            })
        }
    })
}

/// Correct a server bug: a Chainlink-spot backfill arrives labelled with the
/// **Binance** topic.
///
/// Verified 2026-09-05 on a connection subscribed to
/// `crypto_prices_chainlink` and nothing else — its updates are labelled
/// correctly, its snapshot is not. Both spot topics' backfills come back as
/// `crypto_prices`, and the symbol format is the only discriminator:
/// Chainlink uses `btc/usd`, Binance uses `btcusdt`.
///
/// Deliberately narrow. It only ever reassigns `BinanceSpot`, only for
/// snapshots, and only when the symbol contains a slash — so if the venue
/// fixes the label, this becomes a no-op rather than a new bug. The
/// `a_chainlink_spot_snapshot_is_mislabelled_as_the_binance_topic` fixture
/// test fails if that happens, which is the prompt to delete this.
fn correct_mislabelled_spot_snapshot(topic: Topic, symbol: &str) -> Topic {
    if topic == Topic::BinanceSpot && symbol.contains('/') {
        tracing::debug!(
            symbol,
            "relabelling a snapshot the venue reported as crypto_prices"
        );
        return Topic::ChainlinkSpot;
    }
    topic
}

fn parse_snapshot(topic: Topic, frame: &RawFrame) -> Result<Snapshot, RtdsError> {
    let payload: RawSnapshotPayload = serde_json::from_value(frame.payload.clone())
        .map_err(|e| RtdsError::json(frame.payload.to_string(), e))?;

    let topic = correct_mislabelled_spot_snapshot(topic, &payload.symbol);

    // Only the TWAP topics backfill exact values. Deciding on the topic rather
    // than on whether the field happens to be present keeps a shape change
    // upstream from silently downgrading TWAP points to display-only.
    let points = match topic {
        Topic::ChainlinkTwap(_) => {
            let mut exact = Vec::with_capacity(payload.data.len());
            for point in payload.data {
                let raw_e18 = point.full_accuracy_value.ok_or_else(|| RtdsError::Precision {
                    raw: String::new(),
                    topic,
                })?;
                exact.push(ExactPoint {
                    value: decode_e18(&raw_e18, topic)?,
                    observed_at: point.timestamp,
                    display_value: point.value,
                    raw_e18,
                });
            }
            SnapshotPoints::Exact(exact)
        }
        Topic::BinanceSpot | Topic::ChainlinkSpot => SnapshotPoints::DisplayOnly(
            payload
                .data
                .into_iter()
                .map(|point| DisplayPoint {
                    observed_at: point.timestamp,
                    display_value: point.value,
                })
                .collect(),
        ),
    };

    Ok(Snapshot {
        topic,
        symbol: payload.symbol,
        published_at: frame.timestamp,
        points,
    })
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod event;

pub use event::{PriceEvent, PriceUpdate};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds event`
Expected: PASS, 11 tests.

- [ ] **Step 5: Prove the differential test actually catches the bug**

Temporarily change `Topic::BinanceSpot`'s arm in `parse_update` to use `decode_e18` instead of `decode_plain`, then run:

Run: `cargo test -p polyoxide-rtds event`
Expected: FAIL. A test that cannot fail is not evidence — confirm this before reverting.

Revert the change and re-run to confirm PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
cargo clippy -p polyoxide-rtds --all-targets -- -D warnings
git add polyoxide-rtds/src/event.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): dispatch frames into per-topic events"
```

---

### Task 9: The Rtds client (tier 1)

**Files:**
- Create: `polyoxide-rtds/src/client.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide-rtds/src/client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::topic::TwapWindow;

    #[test]
    fn the_default_url_is_the_live_data_host() {
        assert_eq!(RTDS_URL, "wss://ws-live-data.polymarket.com");
    }

    #[test]
    fn connecting_with_no_subscriptions_is_refused() {
        // An empty `subscriptions` array produces a connection that receives
        // nothing, which is indistinguishable from a broken feed.
        let request = SubscriptionRequest::new(Vec::new());
        assert!(
            validate_subscriptions(request.subscriptions()).is_err(),
            "an empty subscription set must be refused up front"
        );
    }

    #[test]
    fn a_populated_subscription_set_is_accepted() {
        let subs = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
            .symbols(["btc/usd"]);
        let request = SubscriptionRequest::new(subs);
        assert!(validate_subscriptions(request.subscriptions()).is_ok());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds client`
Expected: FAIL — `cannot find value RTDS_URL in this scope`.

- [ ] **Step 3: Write the implementation**

Prepend to `polyoxide-rtds/src/client.rs`:

```rust
//! The bare RTDS client.

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    error::RtdsError,
    event::PriceEvent,
    subscription::{Subscription, SubscriptionRequest},
    topic::Topic,
};

/// The RTDS endpoint.
pub const RTDS_URL: &str = "wss://ws-live-data.polymarket.com";

/// Make sure rustls has a default `CryptoProvider` before we open a connection.
///
/// This is a deliberate twin of `ensure_crypto_provider` in
/// `polyoxide-clob/src/ws/client.rs`. `tokio-tungstenite` builds its TLS
/// config from the process-wide default provider, and rustls picks one
/// automatically only when exactly one backend feature is enabled. Faced with
/// two candidates it installs neither and panics inside `connect_async`.
///
/// Duplicating rather than sharing is safe: `install_default` returns `Err`
/// when a provider is already set, so two crates racing is a no-op rather than
/// a conflict, and the host application's own choice is left alone.
fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Reject a subscription set that would produce a silent connection.
fn validate_subscriptions(subscriptions: &[Subscription]) -> Result<(), RtdsError> {
    if subscriptions.is_empty() {
        return Err(RtdsError::Server {
            status_code: 0,
            message: "no subscriptions: the connection would receive nothing".into(),
        });
    }
    Ok(())
}

/// A connected RTDS stream.
///
/// Ends when the connection drops. For a feed that recovers on its own, use
/// the supervised tier built on top of this one. (Deliberately prose, not
/// an intra-doc link: `supervisor` does not exist until Task 11, and a
/// link to a missing item is a hard error under `RUSTDOCFLAGS=-D warnings`,
/// which silently withholds the release tag. Task 11 restores the link.)
///
/// # Example
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_rtds::{PriceEvent, Rtds, Subscription, Topic, TwapWindow};
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let mut stream = Rtds::connect(
///     Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
///         .symbols(["btc/usd"]),
/// )
/// .await?;
///
/// while let Some(event) = stream.next().await {
///     if let PriceEvent::Update(update) = event? {
///         println!("{} {}", update.symbol(), update.value());
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct Rtds {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    subscriptions: Vec<Subscription>,
}

impl Rtds {
    /// Connect to RTDS and subscribe.
    pub async fn connect(
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<Self, RtdsError> {
        Self::connect_to(RTDS_URL, subscriptions).await
    }

    /// Connect to a specific endpoint. Used by tests against a local server.
    pub async fn connect_to(
        url: &str,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<Self, RtdsError> {
        let request = SubscriptionRequest::new(subscriptions);
        validate_subscriptions(request.subscriptions())?;

        ensure_crypto_provider();
        let (mut inner, _) = connect_async(url).await?;
        let frame = serde_json::to_string(&request)?;
        inner.send(Message::Text(frame.into())).await?;

        Ok(Self {
            inner,
            subscriptions: request.subscriptions().to_vec(),
        })
    }

    /// Send the application keep-alive.
    ///
    /// RTDS documents a five-second cadence, but the connection survives far
    /// longer without it, and the server never replies. Do not use this to
    /// detect liveness — nothing comes back.
    pub async fn ping(&mut self) -> Result<(), RtdsError> {
        self.inner.send(Message::Text("PING".into())).await?;
        Ok(())
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), RtdsError> {
        self.inner.close(None).await?;
        Ok(())
    }

    /// The subscriptions this connection was opened with.
    pub fn subscriptions(&self) -> &[Subscription] {
        &self.subscriptions
    }

    /// The topics this connection is subscribed to.
    pub fn topics(&self) -> impl Iterator<Item = Topic> + '_ {
        self.subscriptions.iter().map(Subscription::topic)
    }
}

impl Stream for Rtds {
    type Item = Result<PriceEvent, RtdsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => match message {
                    Message::Text(text) => match PriceEvent::from_json(&text) {
                        Ok(Some(event)) => Poll::Ready(Some(Ok(event))),
                        // Greetings, keep-alives and unmodelled topics.
                        Ok(None) => continue,
                        Err(err) => Poll::Ready(Some(Err(err))),
                    },
                    Message::Close(_) => Poll::Ready(None),
                    Message::Ping(_) | Message::Pong(_) | Message::Binary(_) => continue,
                    Message::Frame(_) => continue,
                },
                Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            };
        }
    }
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod client;

pub use client::{Rtds, RTDS_URL};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds client`
Expected: PASS, 3 tests.

- [ ] **Step 5: Verify the doc example compiles**

Run: `cargo test -p polyoxide-rtds --doc`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
cargo clippy -p polyoxide-rtds --all-targets -- -D warnings
git add polyoxide-rtds/src/client.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): add the Rtds stream client"
```

---

### Task 10: Scripted test server

**Files:**
- Create: `polyoxide-rtds/tests/scripted_server.rs`

`mockito` is HTTP-only, so there is no WebSocket mocking anywhere in this workspace. Tier 2's behaviour — backoff, staleness, resubscribe — cannot be exercised against a live host deterministically, so it needs a local server that can fall silent and drop connections on cue.

- [ ] **Step 1: Write the harness**

`polyoxide-rtds/tests/scripted_server.rs`:

```rust
//! A local WebSocket server for testing supervision behaviour.
//!
//! Not a test file in its own right — `supervision.rs` includes it with
//! `#[path]`. It exists because the behaviours tier 2 adds (reconnect,
//! staleness detection, resubscribe) cannot be triggered on demand against the
//! real host.

#![allow(dead_code)]

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// What the server should do on a given connection.
#[derive(Debug, Clone)]
pub enum Script {
    /// Send these frames, then hold the connection open and silent.
    SendThenIdle(Vec<String>),
    /// Send these frames, then close the connection.
    SendThenClose(Vec<String>),
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL clients should connect to.
    pub url: String,
    /// How many connections have been accepted so far.
    connections: Arc<AtomicUsize>,
    /// The subscription frame received on each connection, in order.
    received: Arc<std::sync::Mutex<Vec<String>>>,
}

impl ScriptedServer {
    /// Start a server that applies `scripts[n]` to the n-th connection,
    /// repeating the last script for any further connections.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let connections = Arc::new(AtomicUsize::new(0));
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));

        let task_connections = Arc::clone(&connections);
        let task_received = Arc::clone(&received);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let index = task_connections.fetch_add(1, Ordering::SeqCst);
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or(Script::SendThenIdle(Vec::new()));
                let received = Arc::clone(&task_received);
                tokio::spawn(async move {
                    let _ = serve(stream, script, received).await;
                });
            }
        });

        Self {
            url: format!("ws://{addr}"),
            connections,
            received,
        }
    }

    /// How many connections the server has accepted.
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The subscription frames received, one per connection, in order.
    pub fn received_subscriptions(&self) -> Vec<String> {
        self.received.lock().await.clone()
    }
}

async fn serve(
    stream: TcpStream,
    script: Script,
    received: Arc<std::sync::Mutex<Vec<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = accept_async(stream).await?;

    // The client sends its subscription frame immediately on connect.
    if let Some(Ok(Message::Text(frame))) = ws.next().await {
        received.lock().await.push(frame.to_string());
    }

    let (frames, close_after) = match script {
        Script::SendThenIdle(frames) => (frames, false),
        Script::SendThenClose(frames) => (frames, true),
    };

    for frame in frames {
        ws.send(Message::Text(frame.into())).await?;
    }

    if close_after {
        ws.close(None).await?;
        return Ok(());
    }

    // Hold open and silent, draining pings so the socket stays healthy. This
    // is what a stalled-but-not-closed feed looks like.
    while let Some(Ok(message)) = ws.next().await {
        if matches!(message, Message::Close(_)) {
            break;
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo test -p polyoxide-rtds --features test-fixtures --test scripted_server`
Expected: `running 0 tests`, no warnings.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/tests/scripted_server.rs
git commit -m "test(rtds): add a scripted local WebSocket server"
```

---

### Task 11: The supervisor (tier 2)

**Files:**
- Create: `polyoxide-rtds/src/supervisor.rs`
- Create: `polyoxide-rtds/tests/supervision.rs`
- Modify: `polyoxide-rtds/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

`polyoxide-rtds/tests/supervision.rs`:

```rust
//! Supervision behaviour, exercised against a local scripted server.

#[path = "scripted_server.rs"]
mod scripted_server;

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use polyoxide_rtds::{PriceEvent, RtdsBuilder, Subscription, Topic, TwapWindow};
use scripted_server::{Script, ScriptedServer};

// The same golden vector the unit tests use, not a second copy. `fixtures`
// is behind the `test-fixtures` feature precisely so integration tests can
// reach it — a pasted duplicate would silently diverge on the next capture.
use polyoxide_rtds::fixtures::TWAP_THIRTY_UPDATE as TWAP_UPDATE;

fn subs() -> Vec<Subscription> {
    Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"])
}

#[tokio::test]
async fn reconnects_and_resubscribes_after_a_drop() {
    let server = ScriptedServer::start(vec![
        Script::SendThenClose(vec![TWAP_UPDATE.into()]),
        Script::SendThenIdle(vec![TWAP_UPDATE.into()]),
    ])
    .await;

    let seen = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&seen);

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_secs(60))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    // Stop once we have seen an update from each of the two connections.
    let _ = tokio::time::timeout(
        Duration::from_secs(10),
        supervised.run(move |event| {
            let counter = Arc::clone(&counter);
            async move {
                if matches!(event, PriceEvent::Update(_)) {
                    *counter.lock().unwrap() += 1;
                }
                Ok(())
            }
        }),
    )
    .await;

    assert!(
        server.connection_count() >= 2,
        "expected a reconnect, saw {} connection(s)",
        server.connection_count()
    );
    assert!(
        *seen.lock().unwrap() >= 2,
        "expected updates from both connections"
    );

    // The resubscribe must send the same frame, or the new connection is
    // subscribed to nothing and goes quiet without erroring.
    let frames = server.received_subscriptions();
    assert!(frames.len() >= 2, "expected 2 subscription frames");
    assert_eq!(
        frames[0], frames[1],
        "resubscribe must replay the original subscription exactly"
    );
    assert!(frames[0].contains(r#"{\"symbol\":\"btc/usd\"}"#), "{}", frames[0]);
}

#[tokio::test]
async fn a_silent_connection_is_treated_as_dead() {
    // The server holds the socket open and sends nothing. There is no PONG to
    // detect this with, so only the staleness timer can.
    let server = ScriptedServer::start(vec![Script::SendThenIdle(Vec::new())]).await;

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_millis(200))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    let _ = tokio::time::timeout(
        Duration::from_secs(5),
        supervised.run(|_event| async move { Ok(()) }),
    )
    .await;

    assert!(
        server.connection_count() >= 2,
        "a stalled connection must be reconnected, saw {}",
        server.connection_count()
    );
}

#[tokio::test]
async fn a_rejected_subscription_stops_instead_of_looping() {
    // One unrecognised topic zeroes an entire batch. Retrying replays the same
    // rejection forever, so the run loop must give up and return the error.
    const REJECTION: &str = r#"{"body":{"message":"topic not found"},"statusCode":401}"#;
    let server = ScriptedServer::start(vec![Script::SendThenIdle(vec![REJECTION.into()])]).await;

    let supervised = RtdsBuilder::new()
        .url(&server.url)
        .stale_after(Duration::from_secs(60))
        .backoff(Duration::from_millis(10), Duration::from_millis(50))
        .connect(subs())
        .await
        .expect("connect");

    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        supervised.run(|_event| async move { Ok(()) }),
    )
    .await
    .expect("run must return rather than retry forever");

    assert!(outcome.is_err(), "a rejected subscription must surface");
    assert_eq!(
        server.connection_count(),
        1,
        "must not reconnect into a rejected subscription"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p polyoxide-rtds --features test-fixtures --test supervision`
Expected: FAIL — `unresolved import polyoxide_rtds::RtdsBuilder`.

- [ ] **Step 3: Write the implementation**

`polyoxide-rtds/src/supervisor.rs`:

```rust
//! Keep-alive, staleness detection, and reconnect.
//!
//! RTDS fails silently: it never answers a ping, sends no error when a
//! connection dies, and restores nothing server-side after a reconnect. A
//! half-open socket is therefore indistinguishable from a quiet market except
//! by timing the gap between updates, which is what [`SupervisedRtds`] does.

use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use tokio::time::{timeout, Instant};

use crate::{
    client::{Rtds, RTDS_URL},
    error::RtdsError,
    event::PriceEvent,
    subscription::Subscription,
};

/// Default keep-alive cadence, matching the cadence upstream documents.
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(5);
/// Default silence before a connection is presumed dead.
///
/// Observed cadence is roughly one update per second per symbol per topic, so
/// this is about 30x headroom. Raise it for a thinly-traded filtered feed.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);

/// Builder for a supervised RTDS connection.
#[derive(Debug, Clone)]
pub struct RtdsBuilder {
    url: String,
    ping_interval: Duration,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl Default for RtdsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RtdsBuilder {
    /// A builder with the default cadences.
    pub fn new() -> Self {
        Self {
            url: RTDS_URL.to_string(),
            ping_interval: DEFAULT_PING_INTERVAL,
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
        }
    }

    /// Point at a different endpoint. Used by tests.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// How often to send the application keep-alive.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// How long the stream may be silent before it is presumed dead.
    ///
    /// This is the **only** liveness signal available: RTDS sends no reply to
    /// a ping, so a half-open socket cannot otherwise be distinguished from a
    /// quiet market.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect backoff bounds. The delay doubles from `initial` up to `max`.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// Connect and subscribe.
    pub async fn connect(
        self,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<SupervisedRtds, RtdsError> {
        let subscriptions: Vec<_> = subscriptions.into_iter().collect();
        let stream = Rtds::connect_to(&self.url, subscriptions.clone()).await?;
        Ok(SupervisedRtds {
            config: self,
            subscriptions,
            stream,
        })
    }
}

/// A supervised RTDS connection that pings, detects stalls, and reconnects.
///
/// Because every resubscribe replays the backfill, callers see
/// [`PriceEvent::Snapshot`] again after each reconnect. That is the intended
/// way to re-initialise state.
pub struct SupervisedRtds {
    config: RtdsBuilder,
    subscriptions: Vec<Subscription>,
    stream: Rtds,
}

impl SupervisedRtds {
    /// Run until the handler errors or the subscription is rejected.
    ///
    /// Recoverable failures — drops and stalls — reconnect with backoff.
    /// Unrecoverable ones, chiefly [`RtdsError::Server`], return: retrying a
    /// rejected subscription replays the same rejection forever.
    pub async fn run<F, Fut>(mut self, mut handler: F) -> Result<(), RtdsError>
    where
        F: FnMut(PriceEvent) -> Fut,
        Fut: Future<Output = Result<(), RtdsError>>,
    {
        let mut backoff = self.config.initial_backoff;

        loop {
            match self.pump(&mut handler).await {
                Ok(()) => return Ok(()),
                // `pump` only ever returns Reconnect or Fatal errors —
                // SkipFrame ones are handled inside it, without dropping the
                // connection. See `Recovery` in error.rs.
                Err(err) if err.recovery() == Recovery::Reconnect => {
                    tracing::warn!(%err, ?backoff, "RTDS connection lost, reconnecting");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(self.config.max_backoff);

                    // A *fresh* `Rtds`, never the exhausted one. `Rtds` is
                    // not `FusedStream`, so re-polling after it has yielded
                    // `None` is not contractually defined — and the socket is
                    // gone anyway. Do not "optimise" this into reuse.
                    match Rtds::connect_to(&self.config.url, self.subscriptions.clone()).await {
                        Ok(stream) => {
                            self.stream = stream;
                            backoff = self.config.initial_backoff;
                        }
                        Err(connect_err) if connect_err.recovery() == Recovery::Reconnect => {
                            continue
                        }
                        Err(fatal) => return Err(fatal),
                    }
                }
                Err(fatal) => return Err(fatal),
            }
        }
    }

    /// Drive one connection until it fails.
    ///
    /// Deliberately not a `tokio::select!` over `ping()` and `next()`:
    /// `select!` builds every branch future before polling, so those two
    /// branches would hold simultaneous mutable borrows of the stream and the
    /// function would not compile. Polling on a single tick also fixes a
    /// second problem — waiting `stale_after` for each read would delay stall
    /// detection whenever `ping_interval` is the shorter of the two.
    async fn pump<F, Fut>(&mut self, handler: &mut F) -> Result<(), RtdsError>
    where
        F: FnMut(PriceEvent) -> Fut,
        Fut: Future<Output = Result<(), RtdsError>>,
    {
        // Wake often enough to honour whichever deadline is nearer.
        let tick = self.config.ping_interval.min(self.config.stale_after);
        let mut last_frame = Instant::now();
        let mut last_ping = Instant::now();

        loop {
            match timeout(tick, self.stream.next()).await {
                // No frame this tick. Check for death first, then keep alive.
                Err(_) => {
                    let elapsed = last_frame.elapsed();
                    if elapsed >= self.config.stale_after {
                        return Err(RtdsError::Stalled { elapsed });
                    }
                    if last_ping.elapsed() >= self.config.ping_interval {
                        self.stream.ping().await?;
                        last_ping = Instant::now();
                    }
                }
                Ok(None) => return Err(RtdsError::ConnectionClosed),
                // A bad frame is not a bad connection. Surface it and keep
                // reading, or one unparseable message ends a 24/7 feed.
                Ok(Some(Err(err))) if err.recovery() == Recovery::SkipFrame => {
                    last_frame = Instant::now();
                    tracing::warn!(%err, "skipping an RTDS frame this client could not read");
                }
                Ok(Some(Err(err))) => return Err(err),
                Ok(Some(Ok(event))) => {
                    last_frame = Instant::now();
                    handler(event).await?;
                }
            }
        }
    }
}
```

Add to `polyoxide-rtds/src/lib.rs`:

```rust
pub mod supervisor;

pub use supervisor::{RtdsBuilder, SupervisedRtds};
```

- [ ] **Step 3b: Restore the forward doc link in `client.rs`**

Task 9 had to write prose where a link belonged, because `supervisor` did not
exist and `rustdoc::broken_intra_doc_links` is a hard error under
`RUSTDOCFLAGS="-D warnings"` — the gate that silently withholds the release
tag. Now that `RtdsBuilder` exists, turn it back into a link:

```rust
/// Ends when the connection drops. For a feed that recovers on its own, use
/// [`RtdsBuilder`](crate::supervisor::RtdsBuilder).
```

Re-run `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features -p polyoxide-rtds` and confirm it resolves.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p polyoxide-rtds --features test-fixtures --test supervision`
Expected: PASS, 3 tests.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test -p polyoxide-rtds --all-features`
Expected: PASS, all tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
cargo clippy -p polyoxide-rtds --all-targets -- -D warnings
git add polyoxide-rtds/src/supervisor.rs polyoxide-rtds/tests/supervision.rs polyoxide-rtds/src/lib.rs
git commit -m "feat(rtds): add reconnect, staleness detection, and keep-alive"
```

---

### Task 12: Resolve the open question — live resubscribe

**Files:**
- Create: `polyoxide-rtds/tests/live_api.rs` (first test only)

The design spec leaves exactly one question open, and it must be answered by observation rather than guessed.

- [ ] **Step 1: Write the probe test**

`polyoxide-rtds/tests/live_api.rs`:

```rust
//! Tests against the real RTDS host. Ignored by default; run with
//! `cargo test -p polyoxide-rtds --test live_api -- --ignored`.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_rtds::{PriceEvent, Rtds, Subscription, Topic, TwapWindow};

/// Answers the design's open question: does RTDS accept a second subscribe
/// frame on an open connection?
///
/// If this passes, `Rtds::subscribe_more` is worth adding. If it fails,
/// changing subscriptions means reconnecting, and no such method should exist.
#[tokio::test]
#[ignore]
async fn reports_whether_a_second_subscribe_frame_is_accepted() {
    let mut stream = Rtds::connect(
        Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty)).symbols(["btc/usd"]),
    )
    .await
    .expect("connect");

    // Drain the first topic's traffic briefly to confirm the feed is alive.
    let mut saw_first = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(_)))) => {
                saw_first = true;
                break;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => panic!("stream ended early"),
            Err(_) => continue,
        }
    }

    assert!(
        saw_first,
        "no updates on an unfiltered-cadence topic in 20s; the feed may \
         legitimately time out if upstream is down"
    );

    // The probe itself is recorded rather than asserted: this test exists to
    // observe, and the finding goes into docs/specs/rtds/OBSERVED.md.
    eprintln!("first subscription confirmed live; extend this test with a second subscribe frame");
}
```

- [ ] **Step 2: Run it against the live host**

Run: `cargo test -p polyoxide-rtds --test live_api -- --ignored --nocapture`
Expected: PASS.

- [ ] **Step 3: Extend the test to send a second subscribe frame**

Add a `subscribe_more` method to `Rtds` in `polyoxide-rtds/src/client.rs`, immediately after `ping`:

```rust
    /// Send an additional subscribe frame on an open connection.
    ///
    /// Whether RTDS honours this is recorded in
    /// `docs/specs/rtds/OBSERVED.md`; see the live test that established it.
    pub async fn subscribe_more(
        &mut self,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<(), RtdsError> {
        let request = SubscriptionRequest::new(subscriptions);
        validate_subscriptions(request.subscriptions())?;
        let frame = serde_json::to_string(&request)?;
        self.inner.send(Message::Text(frame.into())).await?;
        self.subscriptions.extend(request.subscriptions().iter().cloned());
        Ok(())
    }
```

Then extend the live test to call `subscribe_more` with `Topic::ChainlinkSpot` and assert that frames from **both** topics arrive within 30 seconds.

- [ ] **Step 4: Run it and record the answer**

Run: `cargo test -p polyoxide-rtds --test live_api -- --ignored --nocapture`

**If it passes:** keep `subscribe_more` and note in `OBSERVED.md` that RTDS accepts additional subscribe frames.

**If it fails:** delete `subscribe_more` from `client.rs`, replace the test with one asserting that a second subscribe frame yields no new topic's frames, and note the finding in `OBSERVED.md`. Do not leave a method that does not work.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-rtds/tests/live_api.rs polyoxide-rtds/src/client.rs
git commit -m "test(rtds): establish whether RTDS accepts a second subscribe frame"
```

---

### Task 13: Live tests with a control subscription

**Files:**
- Modify: `polyoxide-rtds/tests/live_api.rs`
- Modify: `.github/workflows/nightly-behavioral.yml:37-41`

A silent feed is ambiguous: it happens both when upstream is down and when our filter encoding regresses — and the whitespace trap makes the second look exactly like the first. Classifying silence as environmental without a control would hide the most likely real failure permanently.

- [ ] **Step 1: Write the control test**

Append to `polyoxide-rtds/tests/live_api.rs`:

```rust
use std::collections::HashSet;

/// Collect the symbols seen on one subscription within a time budget.
async fn symbols_seen(subscriptions: Vec<Subscription>, budget: Duration) -> HashSet<String> {
    let mut stream = Rtds::connect(subscriptions).await.expect("connect");
    let mut symbols = HashSet::new();
    let deadline = tokio::time::Instant::now() + budget;

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(update)))) => {
                symbols.insert(update.symbol().to_string());
            }
            Ok(Some(Ok(PriceEvent::Snapshot(_)))) => continue,
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    symbols
}

#[tokio::test]
#[ignore]
async fn the_symbol_filter_actually_binds() {
    // A filtered subscription that yields nothing could mean upstream is down
    // OR that our filter encoding broke. The unfiltered control tells the two
    // apart, so "no frames" is only environmental when the control is silent
    // too.
    let topic = Topic::ChainlinkTwap(TwapWindow::Thirty);
    let budget = Duration::from_secs(25);

    let control = symbols_seen(vec![Subscription::for_topic(topic)], budget).await;
    let filtered = symbols_seen(
        Subscription::for_topic(topic).symbols(["btc/usd"]),
        budget,
    )
    .await;

    if control.is_empty() {
        panic!(
            "no frames on an unfiltered subscription in {budget:?}; upstream may \
             legitimately time out"
        );
    }

    assert!(
        !filtered.is_empty(),
        "the unfiltered control received {} symbol(s) but the filtered \
         subscription received none — the filter encoding is broken, which is \
         exactly what a stray space in `filters` looks like",
        control.len()
    );
    assert_eq!(
        filtered,
        HashSet::from(["btc/usd".to_string()]),
        "the filter must bind to exactly one symbol; control saw {control:?}"
    );
    assert!(
        control.len() > 1,
        "an unfiltered subscription should see several symbols, saw {control:?}"
    );
}

#[tokio::test]
#[ignore]
async fn twap_updates_decode_to_a_plausible_price() {
    let budget = Duration::from_secs(20);
    let mut stream = Rtds::connect(
        Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Sixty)).symbols(["btc/usd"]),
    )
    .await
    .expect("connect");

    let deadline = tokio::time::Instant::now() + budget;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
            Ok(Some(Ok(PriceEvent::Update(update)))) => {
                // A scale error shows up here as a wildly wrong magnitude:
                // an E18 misdecode reads ~8e-14, the reverse reads ~8e22.
                let value = update.value();
                assert!(
                    value > rust_decimal::Decimal::from(100u32)
                        && value < rust_decimal::Decimal::from(10_000_000u32),
                    "BTC TWAP decoded to {value}, which is not a plausible price — \
                     check the E18 scale"
                );
                assert_eq!(update.window(), Some(TwapWindow::Sixty));
                return;
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(err))) => panic!("stream error: {err}"),
            Ok(None) => break,
            Err(_) => continue,
        }
    }
    panic!("no TWAP updates in {budget:?}; upstream may legitimately time out");
}
```

- [ ] **Step 2: Run the live tests**

Run: `cargo test -p polyoxide-rtds --test live_api -- --ignored`
Expected: PASS, all tests.

- [ ] **Step 3: Add the nightly matrix entry**

In `.github/workflows/nightly-behavioral.yml`, add to the `include` list after the `polyoxide-relay` line:

```yaml
          - { crate: polyoxide-rtds,  flags: "--test live_api" }
```

No change to `.github/scripts/classify_failures.py` is needed: the panic messages above use the phrase `legitimately time out`, which `ENVIRONMENTAL_RE` at line 37 already matches.

- [ ] **Step 4: Verify the classifier agrees**

Run:

```bash
python3 -c "
import re, pathlib
src = pathlib.Path('.github/scripts/classify_failures.py').read_text()
pattern = re.search(r'ENVIRONMENTAL_RE = re.compile\(\s*r\"([^\"]+)\"', src).group(1)
msg = 'no frames on an unfiltered subscription in 25s; upstream may legitimately time out'
print('environmental:', bool(re.search(pattern, msg, re.IGNORECASE)))
"
```

Expected: `environmental: True`

- [ ] **Step 5: Commit**

```bash
git add polyoxide-rtds/tests/live_api.rs .github/workflows/nightly-behavioral.yml
git commit -m "test(rtds): add live tests with an unfiltered control subscription"
```

---

### Task 14: Spec mirror and observations

**Files:**
- Create: `docs/specs/rtds/asyncapi-live-data.json`
- Create: `docs/specs/rtds/OBSERVED.md`
- Modify: `docs/specs/INDEX.md` (WebSocket specs table, lines 66-70)

- [ ] **Step 1: Write the observed mirror**

Create `docs/specs/rtds/asyncapi-live-data.json` modelled on `docs/specs/clob/asyncapi-sports.json`. Read that file first for the exact structure, then produce an AsyncAPI 3.0 document with:

- `info.title`: `Polymarket RTDS (ws-live-data)`
- `info.description` stating plainly that upstream publishes **no** AsyncAPI for this host, so this document is modelled on frames captured 2026-09-05 and must not be compared against any published document
- one channel per topic: `crypto_prices`, `crypto_prices_chainlink`, `crypto_prices_twap_thirty`, `crypto_prices_twap_sixty`
- for each, `update` and `subscribe` messages
- an `x-observed-payload` block on each message holding the verbatim captured frame from `polyoxide-rtds/src/fixtures.rs`
- an `x-observed-notes` field on `crypto_prices` recording that its `full_accuracy_value` is a plain decimal while every other topic's is E18

- [ ] **Step 2: Write the observations file**

`docs/specs/rtds/OBSERVED.md`:

```markdown
# RTDS: observed behaviour

Polymarket publishes no AsyncAPI for `wss://ws-live-data.polymarket.com`, and
its prose documentation disagrees with the server in six places. This file
records what the host actually does, observed on 2026-09-05 across seven
probes. `asyncapi-live-data.json` beside it is modelled on captured frames, not
on any upstream document, so it must never be added to `nightly-schema.yml` —
there is nothing to diff it against.

## Contradictions with the published documentation

| # | Documented | Observed |
|---|---|---|
| 1 | `full_accuracy_value` is "the exact signed E18 fixed-point value" | E18 on the three Chainlink topics; a **plain decimal** on `crypto_prices` |
| 2 | TWAP: "no snapshot, history, or replay" | Every topic sends a `type:"subscribe"` backfill first — ~55-59 points on the Chainlink topics, 120 on Binance |
| 3 | Binance filter is `"btcusdt,ethusdt"` | Yields zero frames. The working form is `{"symbol":"btcusdt"}` |
| 4 | Symbols must be lowercase | `{"symbol":"BTC/USD"}` works; matching is case-insensitive |
| 5 | Envelope is `{topic,type,timestamp,payload}` | `update` frames carry an undocumented `connection_id`; `subscribe` frames do not |
| 6 | Chainlink supports 4 symbols; Binance 4 | 8 Chainlink live, 6 Binance |

## Undocumented behaviour

**The whitespace trap.** `{"symbol": "btc/usd"}` — one space — still delivers
the subscribe backfill and then never sends another update, with no error. The
failure is indistinguishable from an idle feed. `polyoxide-rtds` never accepts a
caller-supplied filter string for this reason.

**Batch poisoning.** One unrecognised topic in a subscription array returns zero
frames for **every** topic in that array, answering only with
`{"body":{"message":"leger GetTopics error: … not found"},"statusCode":401}`.
The `401` is not meaningful — the body describes a not-found.

**`PING` is not load-bearing, and there is no `PONG`.** With both the
application `PING` and the protocol-level ping disabled, a subscription ran 240
seconds and 224 frames without interruption. The only non-JSON text frame RTDS
ever sent was a single empty string at connect. Liveness must be inferred from
update staleness; nothing comes back from a ping.

## Symbols observed

- Chainlink (`btc/usd` form): `btc`, `eth`, `sol`, `xrp`, `bnb`, `doge`, `hype`, `zec`
- Binance (`btcusdt` form): `btc`, `eth`, `sol`, `xrp`, `bnb`, `doge`

Symbol sets moved beyond the documented four within one observation window, so
symbols are modelled as `String` rather than an enum.
```

- [ ] **Step 3: Add the INDEX.md row**

In the "WebSocket specs" table of `docs/specs/INDEX.md`, add:

```markdown
| [rtds/asyncapi-live-data.json](rtds/asyncapi-live-data.json) | RTDS crypto prices (4 topics) — **observed, not published upstream**; see [rtds/OBSERVED.md](rtds/OBSERVED.md) |
```

- [ ] **Step 4: Verify the JSON parses**

Run: `python3 -m json.tool docs/specs/rtds/asyncapi-live-data.json > /dev/null && echo OK`
Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git add docs/specs/rtds/ docs/specs/INDEX.md
git commit -m "docs(specs): mirror the observed RTDS contract"
```

---

### Task 15: Unified crate and example

**Files:**
- Modify: `polyoxide/Cargo.toml` (features and dependencies)
- Modify: `polyoxide/src/lib.rs:81-86` (re-exports) and the `prelude` module at line 96
- Create: `polyoxide-rtds/examples/twap_stream.rs`

- [ ] **Step 1: Wire the feature**

In `polyoxide/Cargo.toml`, add to `[features]`:

```toml
rtds = ["dep:polyoxide-rtds"]
```

and change `full` to:

```toml
full = ["clob", "gamma", "data", "ws", "rtds"]
```

and add to `[dependencies]`:

```toml
polyoxide-rtds = { workspace = true, optional = true }
```

Leave `default` unchanged so the default build stays light.

- [ ] **Step 2: Re-export from the unified crate**

In `polyoxide/src/lib.rs`, add alongside the other re-exports near line 81:

```rust
#[cfg(feature = "rtds")]
pub use polyoxide_rtds;
```

and in the `prelude` module:

```rust
    #[cfg(feature = "rtds")]
    pub use polyoxide_rtds::{PriceEvent, PriceUpdate, Rtds, RtdsBuilder, Subscription, Topic, TwapWindow};
```

- [ ] **Step 3: Write the example**

`polyoxide-rtds/examples/twap_stream.rs`:

```rust
//! Stream Chainlink 30s TWAP prices for BTC and ETH.
//!
//! Run with: `cargo run -p polyoxide-rtds --example twap_stream`

use std::time::Duration;

use polyoxide_rtds::{PriceEvent, RtdsBuilder, Subscription, Topic, TwapWindow};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let subscriptions = Subscription::for_topic(Topic::ChainlinkTwap(TwapWindow::Thirty))
        .symbols(["btc/usd", "eth/usd"]);

    let stream = RtdsBuilder::new()
        .stale_after(Duration::from_secs(30))
        .connect(subscriptions)
        .await?;

    stream
        .run(|event| async move {
            match event {
                // Sent on connect and again after every reconnect — this is
                // how you re-initialise state, not a one-off.
                PriceEvent::Snapshot(snapshot) => {
                    println!(
                        "backfill: {} points for {}",
                        snapshot.points.len(),
                        snapshot.symbol
                    );
                }
                PriceEvent::Update(update) => {
                    // `value()` is exact. `display_value` is a lossy float and
                    // must not be used for arithmetic.
                    println!(
                        "{:>8} {:>4}s {}",
                        update.symbol(),
                        update.window().map(TwapWindow::seconds).unwrap_or(0),
                        update.value()
                    );
                }
            }
            Ok(())
        })
        .await?;

    Ok(())
}
```

- [ ] **Step 4: Verify everything builds**

Run:

```bash
cargo build -p polyoxide --features full
cargo build -p polyoxide-rtds --example twap_stream
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all three finish with no warnings.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide/ polyoxide-rtds/examples/
git commit -m "feat(rtds): re-export through the unified crate and add an example"
```

---

### Task 16: CLI command

**Files:**
- Create: `polyoxide-cli/src/commands/ws/prices.rs`
- Modify: `polyoxide-cli/src/commands/ws/mod.rs`
- Modify: `polyoxide-cli/Cargo.toml` (dependencies)

- [ ] **Step 1: Add the dependency**

In `polyoxide-cli/Cargo.toml`, add to `[dependencies]`:

```toml
polyoxide-rtds = { workspace = true }
```

- [ ] **Step 2: Write the command**

`polyoxide-cli/src/commands/ws/prices.rs`:

```rust
use std::time::Duration;

use clap::Args;
use color_eyre::eyre::Result;
use futures_util::StreamExt;
use polyoxide_rtds::{PriceEvent, Rtds, Subscription, Topic, TwapWindow};

use crate::commands::common::parsing::parse_duration;

/// Which RTDS price topic to stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PriceTopic {
    /// Binance spot prices. Symbols look like `btcusdt`.
    Binance,
    /// Chainlink spot prices. Symbols look like `btc/usd`.
    Chainlink,
    /// Chainlink time-weighted average prices. Needs `--window`.
    Twap,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable one line per update.
    #[default]
    Pretty,
    /// Compact JSON, one object per line.
    Json,
}

#[derive(Args)]
pub struct PricesArgs {
    /// Which price topic to stream
    #[arg(long, value_enum, default_value = "twap")]
    topic: PriceTopic,

    /// TWAP lookback window in seconds (only valid with `--topic twap`)
    #[arg(long, value_parser = ["30", "60"], default_value = "30")]
    window: String,

    /// Symbols to filter to. Omit to receive every symbol on the topic.
    #[arg(long)]
    symbol: Vec<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    format: OutputFormat,

    /// Exit after receiving N updates
    #[arg(short = 'n', long)]
    count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    timeout: Option<Duration>,
}

pub async fn run(args: PricesArgs) -> Result<()> {
    let topic = match args.topic {
        PriceTopic::Binance => Topic::BinanceSpot,
        PriceTopic::Chainlink => Topic::ChainlinkSpot,
        PriceTopic::Twap => {
            let seconds: u32 = args.window.parse()?;
            let window = TwapWindow::from_seconds(seconds)
                .ok_or_else(|| color_eyre::eyre::eyre!("window must be 30 or 60"))?;
            Topic::ChainlinkTwap(window)
        }
    };

    // An empty --symbol list means "every symbol", which is a subscription
    // with no filter — not an empty fan-out.
    let subscriptions = if args.symbol.is_empty() {
        vec![Subscription::for_topic(topic)]
    } else {
        Subscription::for_topic(topic).symbols(args.symbol)
    };

    let mut stream = Rtds::connect(subscriptions).await?;
    let deadline = args.timeout.map(|t| tokio::time::Instant::now() + t);
    let mut seen = 0u64;

    loop {
        if let Some(deadline) = deadline {
            if tokio::time::Instant::now() >= deadline {
                break;
            }
        }

        let Some(event) = stream.next().await else {
            break;
        };

        match event? {
            PriceEvent::Snapshot(snapshot) => {
                eprintln!(
                    "# backfill: {} points for {}",
                    snapshot.points.len(),
                    snapshot.symbol
                );
            }
            PriceEvent::Update(update) => {
                match args.format {
                    OutputFormat::Pretty => println!(
                        "{:>10} {:>20} {}",
                        update.symbol(),
                        update.value(),
                        update.observed_at()
                    ),
                    // `value` is printed as a string so the exact decimal is
                    // not degraded by a JSON float.
                    OutputFormat::Json => println!(
                        r#"{{"symbol":"{}","value":"{}","observed_at":{},"window_s":{}}}"#,
                        update.symbol(),
                        update.value(),
                        update.observed_at(),
                        update
                            .window()
                            .map(|w| w.seconds().to_string())
                            .unwrap_or_else(|| "null".into())
                    ),
                }

                seen += 1;
                if args.count.is_some_and(|max| seen >= max) {
                    break;
                }
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Register the subcommand**

Replace `polyoxide-cli/src/commands/ws/mod.rs` with:

```rust
mod market;
mod prices;
mod user;

use clap::Subcommand;
use color_eyre::eyre::Result;

#[derive(Subcommand)]
pub enum WsCommand {
    /// Subscribe to market channel (order book, price changes)
    Market {
        #[command(flatten)]
        args: market::MarketArgs,
    },
    /// Subscribe to user channel (orders, trades) - requires authentication
    User {
        #[command(flatten)]
        args: user::UserArgs,
    },
    /// Stream RTDS reference prices (Binance, Chainlink spot, Chainlink TWAP)
    Prices {
        #[command(flatten)]
        args: prices::PricesArgs,
    },
}

impl WsCommand {
    pub async fn run(self) -> Result<()> {
        match self {
            Self::Market { args } => market::run(args).await,
            Self::User { args } => user::run(args).await,
            Self::Prices { args } => prices::run(args).await,
        }
    }
}
```

- [ ] **Step 4: Verify it runs**

Run:

```bash
cargo run -p polyoxide-cli -- ws prices --help
cargo run -p polyoxide-cli -- ws prices --topic twap --window 30 --symbol btc/usd -n 3
```

Expected: the help text lists `--topic`, `--window`, `--symbol`; the second command prints three lines with a price near the current BTC price, then exits.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
git add polyoxide-cli/
git commit -m "feat(cli): add ws prices for RTDS reference price streams"
```

---

### Task 17: Release wiring and CLAUDE.md

**Files:**
- Modify: `.github/workflows/release.yml:83`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add to the publish order**

In `.github/workflows/release.yml` line 83, change:

```bash
CRATES=("polyoxide-core" "polyoxide-relay" "polyoxide-gamma" "polyoxide-data" "polyoxide-clob" "polyoxide")
```

to:

```bash
CRATES=("polyoxide-core" "polyoxide-rtds" "polyoxide-relay" "polyoxide-gamma" "polyoxide-data" "polyoxide-clob" "polyoxide")
```

`polyoxide-rtds` depends on nothing in the workspace, so its position only needs to precede `polyoxide`.

- [ ] **Step 2: Update the crate graph in CLAUDE.md**

In the "Workspace Architecture" section, change "Eight crates" to "Nine crates" and add to the graph:

```
polyoxide-rtds          (RTDS price streams; depends on nothing in-workspace)
```

- [ ] **Step 3: Add an RTDS section to CLAUDE.md**

Add after the WebSocket section in "Module Organization":

```markdown
**RTDS is a separate crate and a separate protocol.** `polyoxide-rtds` covers
`wss://ws-live-data.polymarket.com`, which multiplexes many topics over one
connection under an `action`/`subscriptions` envelope — unlike the CLOB
channels, which are one channel per connection. It depends on nothing else in
the workspace (not even core) so a credential-free price feed does not pull in
`alloy`: `polyoxide-clob --features ws` builds 352 crates, and none of that
signing stack is needed to read a price.

**`full_accuracy_value` does not mean the same thing on every topic.** It is
E18 fixed-point on the three Chainlink topics and a **plain decimal** on
`crypto_prices` (Binance). The two spot payloads are otherwise field-identical,
so they are separate types and the scale is never a runtime decision. A test
that asserts only "the value is a positive Decimal" passes on both and proves
nothing; `the_two_spot_topics_do_not_share_a_scale` in `event.rs` is the one
that holds this up.

**Three RTDS behaviours have no counterpart in the docs**, all recorded in
`docs/specs/rtds/OBSERVED.md`: a filter with one stray space delivers the
backfill and then goes silent forever with no error; one unrecognised topic
returns zero frames for every topic in the same batch; and the documented
5-second `PING` is neither required nor answered, so staleness — not the
heartbeat — is the only liveness signal. `docs/specs/rtds/` is modelled on
captured frames and is deliberately excluded from `nightly-schema.yml`, since
upstream publishes nothing to diff it against.
```

- [ ] **Step 4: Full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

Expected: all four pass with no warnings. The `cargo doc` gate is the one most likely to fail — check that no `pub` item's docs link to a `pub(crate)` item.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release.yml CLAUDE.md
git commit -m "chore(rtds): wire the crate into release and document its gotchas"
```

---

## Self-Review Notes

**Spec coverage.** Every section of the design spec maps to a task: crate and module layout (1), protocol layer (2, 3), event types (6, 7, 8), connection tiers (9, 11), errors (4), decoding (5), testing (8, 10, 11), the open question (12), live tests and nightly (13), spec mirror (14), unified crate and example (15), CLI (16), release and CLAUDE.md (17).

**Deliberate ordering.** Task 12 resolves the design's open question *before* Task 13 writes the rest of the live tests, so `subscribe_more` either exists and works or never ships. Task 8 Step 5 requires proving the differential test fails when the bug is reintroduced — a test that cannot fail is not evidence.

**Known risk.** The `stale_after` default of 30 seconds is derived from an observed ~1 update/second cadence on major symbols over short windows. A thinly-traded symbol on a filtered subscription could plausibly be slower and trigger spurious reconnects. It is configurable for that reason, and Task 13's live tests will show if the default is wrong.

**Accepted limitation: a persistently-broken symbol flatlines quietly.** `Recovery::SkipFrame` is right *because* RTDS multiplexes every topic and symbol over one connection — taking the socket down to punish one symbol's bad value would stop every other subscription riding it. But the consequence is that a symbol whose payload this crate consistently cannot decode simply stops advancing, while everything else keeps flowing. The supervisor logs a `WARN` per skipped frame, so it is visible in logs rather than truly silent, but nothing escalates.

A fix, deliberately not in scope here: count consecutive `SkipFrame` errors per topic+symbol in `SupervisedRtds` and surface a distinct diagnostic (or unsubscribe) past a threshold. That needs per-symbol state the supervisor does not otherwise carry, and it is only worth building once there is evidence a symbol actually behaves this way. Revisit if Task 13's nightly live tests ever report a symbol going quiet while its siblings continue.
