# polyoxide-binance WebSocket Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stream Binance USDⓈ-M market data from `fstream.binance.com` through `polyoxide-binance`'s `ws` feature, with a bare connection, a supervised one that enforces Binance's connection rules and reports every outage, and `polyoxide ws binance`.

**Architecture:** One module, `polyoxide-binance/src/usdm/ws/`, behind the `ws` feature. `StreamName` is the only way to name a stream, and it knows its routed path (`/market` or `/public`). `UsdmWs` is one connection on one path. It enforces the 1024-stream cap, 200 names per request and one request per 200 ms before sending, and each request waits for its answer. `UsdmWsBuilder`/`SupervisedUsdmWs` runs a task per path, on the shape of `polyoxide-perps`'s supervised socket. The supervisor pings on the wall clock, treats silence as staleness, reconnects with a paced replay and rotates before the 24-hour cutoff. It also pairs every `Disconnected` with a `Reconnected`. `polyoxide ws binance` reads the supervised tier. This is plan 2 of 2. It needs `2026-10-07-polyoxide-binance-http.md` executed first, and ends with the 0.37.0 release, which waits for the owner's go-ahead.

**Tech Stack:** Rust 2021 (MSRV 1.91), tokio, tokio-tungstenite 0.26 (rustls), futures-util, serde/serde_json, rust_decimal, clap (CLI). The capture script is Python 3, stdlib only.

**Spec:** `docs/superpowers/specs/2026-10-07-polyoxide-binance-design.md`, as of 816baba. That commit adopted prader-rs's stream contract (items 6–14 of its message), which this plan implements.

---

## Before you start

- Plan 1 (`2026-10-07-polyoxide-binance-http.md`) must be done: this plan edits files it creates.
- Work on branch `aidanb/polyoxide-binance` in its loom worktree. Do not switch branches or create worktrees.
- Every code block was compiled, rustfmt-formatted, clippy-checked and tested in a scratch copy on 2026-10-07 with Rust 1.95, and the tasks were replayed in order on a fresh tree of plan 1's result. CI floats on stable (1.99), which can add lints.
- Build with `-j 4`. `signal: 15` or `exit status: 254` is the OOM reaper, not a failure. Never put `CARGO_TARGET_DIR` under `/tmp`.
- Run `cargo fmt --all` before every commit.
- **Cargo checks a `[[test]]` target's file when it parses the manifest**, whatever `required-features` says. Each `[[test]]` stanza is added in the task that creates its file, never earlier, or the whole workspace stops building.
- **Expected warnings until Task 4.** `StreamPath::url`, `ensure_crypto_provider` and the constants are first used by the bare tier (Task 3), and some of the bare tier's crate-private methods by the supervised tier (Task 4). Run clippy with `-D warnings` from Task 4 on.
- The supervision tests run real sockets against a local server with limits of tens of milliseconds. They passed five runs in a row in the scratch copy. A failure that does not reproduce on a rerun is a timing flake to report, not to retry away.
- Live steps (Tasks 5, 6 and 7) connect to `fstream.binance.com`.
- End every commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN
  ```
- The first edit of a file under `.github/workflows/` can be refused by a security hook. Retry the same edit once, then grep to confirm it landed.

## Facts the code relies on (measured 2026-10-07)

| Fact | Where it matters |
|---|---|
| The 1025th stream is answered with code 4 and the server then closes with 1008, losing all 1024 | `MAX_STREAMS_PER_CONNECTION`, checked before sending |
| Of 40 requests sent back to back, 15 were answered, then a close with 1008 "Too many requests" | `MIN_REQUEST_INTERVAL` = 200 ms |
| Every `SUBSCRIBE` is acknowledged, including names that will never deliver; uppercase symbols deliver nothing | `StreamName` builds every name; `FromStr` refuses an uppercase symbol |
| Depth and book tickers ride `/public`, the rest `/market` | `StreamName::path`, one connection per path |
| Server pings about every 180 s | wall-clock client pings every 20 s; liveness counts pongs and pings |
| COIN-M rows (`st: 2`) arrive on this host; a mark-price row with no funding scheduled sends `T: 0` | `SymbolType`, `next_funding_time` |
| The kline's `B` is documented as "Ignore" | not modelled; the agreement tests allow it |

## File structure

| File | Responsibility |
|---|---|
| `polyoxide-binance/Cargo.toml` | `ws` and `test-server` features, socket dependencies, three `[[test]]` stanzas |
| `polyoxide-binance/src/usdm/mod.rs` | `pub mod ws` behind the feature |
| `polyoxide-binance/src/usdm/ws/mod.rs` | `StreamPath`, the connection-rule constants, `ensure_crypto_provider`, re-exports |
| `polyoxide-binance/src/usdm/ws/stream.rs` | `StreamName`, `DepthLevels`, `DepthSpeed` |
| `polyoxide-binance/src/usdm/ws/error.rs` | `UsdmWsError`, `Recovery` |
| `polyoxide-binance/src/usdm/ws/event.rs` | The six payload types, `SymbolType`, `Payload`, `Update::from_json` |
| `polyoxide-binance/src/usdm/ws/fixtures.rs` | The captured frames as constants, for tests here and downstream |
| `polyoxide-binance/src/usdm/ws/client.rs` | `UsdmWs`, the bare tier |
| `polyoxide-binance/src/usdm/ws/test_server.rs` | `ScriptedServer`, a scripted Binance-shaped server |
| `polyoxide-binance/src/usdm/ws/supervised.rs` | `UsdmWsBuilder`, `SupervisedUsdmWs`, `MembershipHandle`, `Event`, `DisconnectReason` |
| `polyoxide-binance/tests/common/mod.rs` | Gains `compare_values` for the stream agreement test |
| `polyoxide-binance/tests/{ws_wire_agreement,supervision,live_ws}.rs` | Stream agreement, supervision, live |
| `polyoxide-binance/tests/fixtures/ws/*.json`, `PROVENANCE.md`, `scripts/capture_binance_fixtures.py` | Stream fixtures and their capture |
| `polyoxide-binance/README.md` | A streaming section |
| `polyoxide-cli/src/commands/ws/{binance,mod}.rs`, `polyoxide-cli/Cargo.toml`, `polyoxide-cli/tests/{ws_binance,live_api}.rs` | `polyoxide ws binance` |
| `docs/specs/binance/{OBSERVED,INDEX}.md`, `CLAUDE.md`, `README.md`, `polyoxide-cli/README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml` | Docs and the nightly row |

---

### Task 1: The feature, stream names and errors

**Files:**
- Modify: `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/mod.rs`
- Create: `polyoxide-binance/src/usdm/ws/mod.rs`, `stream.rs`, `error.rs`

- [ ] **Step 1: Add the feature and its dependencies**

In `polyoxide-binance/Cargo.toml`, replace

```toml
[features]
default = []
```

with

```toml
[features]
default = []
# The USDⓈ-M market streams.
ws = ["dep:tokio-tungstenite", "dep:futures-util", "dep:rustls", "tokio/net", "tokio/rt", "tokio/macros"]
# Exposes the scripted local server and the captured frames for downstream
# tests. Not for consumers.
test-server = ["ws"]
```

Replace

```toml
tokio = { workspace = true, features = ["time", "sync"] }
tracing = { workspace = true }
```

with

```toml
tokio = { workspace = true, features = ["time", "sync"] }
tokio-tungstenite = { workspace = true, optional = true }
futures-util = { version = "0.3", optional = true }
# Needed only to install a process-default rustls CryptoProvider; see
# `ensure_crypto_provider` in src/usdm/ws/mod.rs.
rustls = { version = "0.23", default-features = false, features = ["ring", "std"], optional = true }
tracing = { workspace = true }
```

and in `[dev-dependencies]`, before `mockito`, add:

```toml
futures-util = "0.3"
```

In `polyoxide-binance/src/usdm/mod.rs`, replace `pub mod types;` with

```rust
pub mod types;
#[cfg(feature = "ws")]
pub mod ws;
```

- [ ] **Step 2: Write the module root**

Create `polyoxide-binance/src/usdm/ws/mod.rs`. Tasks 2 to 4 add the remaining modules and re-exports.

```rust
//! USDⓈ-M market streams on `fstream.binance.com`.
//!
//! Binance routes streams by path: `/public` carries partial depth and book
//! tickers, `/market` the rest of the streams here. Every [`StreamName`] knows
//! its path, the bare [`UsdmWs`] holds one connection on one path, and the
//! supervised tier ([`UsdmWsBuilder`]) keeps one connection per path, opened
//! when its first stream is wanted and closed when its last leaves.
//!
//! Binance acknowledges every `SUBSCRIBE` and enforces its connection rules by
//! closing the connection, so both tiers enforce them before sending: at most
//! [`MAX_STREAMS_PER_CONNECTION`] streams, [`MAX_NAMES_PER_REQUEST`] names per
//! request, and one request per [`MIN_REQUEST_INTERVAL`]. The measurements are
//! in `docs/specs/binance/OBSERVED.md`.
pub mod error;
pub mod stream;

use std::{fmt, time::Duration};

pub use error::{Recovery, UsdmWsError};
pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};

/// The production stream host.
pub const USDM_WS_BASE: &str = "wss://fstream.binance.com";

/// Streams one connection may carry. The 1025th is answered with an error,
/// and then the server closes the connection.
pub const MAX_STREAMS_PER_CONNECTION: usize = 1024;

/// Stream names per `SUBSCRIBE` or `UNSUBSCRIBE`. 200 were measured accepted.
pub const MAX_NAMES_PER_REQUEST: usize = 200;

/// The shortest gap between two requests on one connection: 5 per second,
/// half the documented 10. Of 40 requests sent back to back, 15 were answered
/// before the server closed the connection.
pub const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(200);

/// Which routed path a connection is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamPath {
    /// `/market`: tickers, mark prices, aggregate trades, klines.
    Market,
    /// `/public`: partial depth and book tickers.
    Public,
}

impl StreamPath {
    /// Both paths.
    pub const ALL: &'static [Self] = &[Self::Market, Self::Public];

    /// The path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Market => "market",
            Self::Public => "public",
        }
    }

    /// The combined-stream URL on `base`.
    pub(crate) fn url(self, base: &str) -> String {
        format!("{}/{}/stream", base.trim_end_matches('/'), self.as_str())
    }
}

impl fmt::Display for StreamPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Make sure rustls has a default `CryptoProvider` before opening a connection.
///
/// The fifth twin of the function in `polyoxide-clob`, `-rtds`, `-perps` and
/// `-sports`: `tokio-tungstenite` builds its TLS config from the process-wide
/// default provider, and rustls installs one automatically only when exactly
/// one backend is enabled. With `ring` and `aws-lc-rs` both in the graph it
/// installs neither and panics inside `connect_async`.
pub(crate) fn ensure_crypto_provider() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
```

- [ ] **Step 3: Write the tests for stream names and errors**

Create `polyoxide-binance/src/usdm/ws/stream.rs` with its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn every_kind(symbol: &str) -> Vec<StreamName> {
        let s = Symbol::new(symbol).unwrap();
        let mut names = vec![
            StreamName::AllTickers,
            StreamName::AllMarkPrices,
            StreamName::AggTrade(s.clone()),
            StreamName::MarkPrice(s.clone()),
            StreamName::Ticker(s.clone()),
            StreamName::BookTicker(s.clone()),
        ];
        names.extend(
            Interval::ALL
                .iter()
                .map(|i| StreamName::Kline(s.clone(), *i)),
        );
        for levels in DepthLevels::ALL {
            for speed in DepthSpeed::ALL {
                names.push(StreamName::PartialDepth(s.clone(), *levels, *speed));
            }
        }
        names
    }

    #[test]
    fn every_name_round_trips_through_its_wire_spelling() {
        for symbol in ["BTCUSDT", "BTCUSDT_261225", "币安人生USDT", "1000PEPEUSDT"] {
            for name in every_kind(symbol) {
                let wire = name.to_string();
                assert_eq!(wire.parse::<StreamName>(), Ok(name.clone()), "{wire}");
            }
        }
    }

    #[test]
    fn names_are_spelled_as_the_wire_spells_them() {
        let btc = Symbol::new("BTCUSDT").unwrap();
        let chinese = Symbol::new("币安人生USDT").unwrap();
        let cases = [
            (StreamName::AllTickers, "!ticker@arr"),
            (StreamName::AllMarkPrices, "!markPrice@arr@1s"),
            (StreamName::AggTrade(btc.clone()), "btcusdt@aggTrade"),
            (
                StreamName::Kline(btc.clone(), Interval::Mo1),
                "btcusdt@kline_1M",
            ),
            (StreamName::MarkPrice(chinese), "币安人生usdt@markPrice@1s"),
            (StreamName::Ticker(btc.clone()), "btcusdt@ticker"),
            (
                StreamName::PartialDepth(btc.clone(), DepthLevels::Twenty, DepthSpeed::Ms100),
                "btcusdt@depth20@100ms",
            ),
            (StreamName::BookTicker(btc), "btcusdt@bookTicker"),
        ];
        for (name, wire) in cases {
            assert_eq!(name.to_string(), wire);
        }
    }

    #[test]
    fn depth_and_book_ticker_ride_the_public_path_and_the_rest_the_market_path() {
        for name in every_kind("BTCUSDT") {
            let expected = match name {
                StreamName::PartialDepth(..) | StreamName::BookTicker(_) => StreamPath::Public,
                _ => StreamPath::Market,
            };
            assert_eq!(name.path(), expected, "{name}");
        }
    }

    #[test]
    fn a_name_this_crate_does_not_build_does_not_parse() {
        for bad in [
            "BTCUSDT@aggTrade",
            "btcusdt@nonsense",
            "btcusdt@depth7@100ms",
            "btcusdt@depth5",
            "btcusdt@kline_1s",
            "btcusdt",
            "!bookTicker",
            "btc usdt@ticker",
        ] {
            assert_eq!(
                bad.parse::<StreamName>(),
                Err(InvalidStreamName(bad.to_owned())),
                "{bad}"
            );
        }
    }
}
```

Create `polyoxide-binance/src/usdm/ws/error.rs` with its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_tungstenite::tungstenite::{self, http};

    fn http_error(status: u16) -> UsdmWsError {
        UsdmWsError::from(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_and_one_refused_for_us_is_fatal() {
        assert_eq!(http_error(503).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(429).recovery(), Recovery::Reconnect);
        assert_eq!(http_error(451).recovery(), Recovery::Fatal);
        assert_eq!(http_error(404).recovery(), Recovery::Fatal);
    }

    #[test]
    fn losing_the_connection_reconnects_and_a_bad_frame_is_skipped() {
        for err in [
            UsdmWsError::from(tungstenite::Error::ConnectionClosed),
            UsdmWsError::ConnectTimeout(Duration::from_secs(10)),
            UsdmWsError::Closed {
                code: Some(1008),
                reason: "Too many requests".into(),
            },
            UsdmWsError::NoAnswer {
                id: 3,
                timeout: Duration::from_secs(10),
            },
        ] {
            assert_eq!(err.recovery(), Recovery::Reconnect, "{err}");
        }
        let frame = UsdmWsError::Frame {
            stream: "btcusdt@aggTrade".into(),
            raw: "{}".into(),
            reason: "missing field".into(),
        };
        assert_eq!(frame.recovery(), Recovery::SkipFrame);
    }

    #[test]
    fn a_refusal_and_a_request_the_client_refused_to_send_are_fatal() {
        let refused = UsdmWsError::Refused {
            code: 2,
            msg: "Invalid request".into(),
        };
        assert_eq!(refused.recovery(), Recovery::Fatal);
        let full = UsdmWsError::TooManyStreams {
            path: StreamPath::Market,
            limit: 1024,
        };
        assert_eq!(full.recovery(), Recovery::Fatal);
        assert_eq!(
            full.to_string(),
            "a market connection carries at most 1024 streams"
        );
        assert_eq!(UsdmWsError::Stopped.recovery(), Recovery::Fatal);
    }

    #[test]
    fn usdm_ws_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<UsdmWsError>();
    }
}
```

Run: `cargo test -j 4 -p polyoxide-binance --features ws --lib usdm::ws`
Expected: FAIL to compile: ``cannot find type `StreamName` in this scope`` and the like.

- [ ] **Step 4: Write the stream names**

Put this above the test module in `stream.rs`:

```rust
//! Stream names: the only way to name a USDⓈ-M market stream.
//!
//! Binance acknowledges every `SUBSCRIBE`, including an unknown symbol, a
//! stream type that does not exist and an uppercase symbol, and the last two
//! deliver nothing. A name that will never deliver is indistinguishable from a
//! quiet one, so names are built here, by construction, and never accepted as
//! caller strings.

use std::{fmt, str::FromStr};

use crate::usdm::{
    types::{Interval, Symbol},
    ws::StreamPath,
};

/// Levels per side of a partial depth stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthLevels {
    /// `depth5`.
    Five,
    /// `depth10`.
    Ten,
    /// `depth20`.
    Twenty,
}

impl DepthLevels {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Five, Self::Ten, Self::Twenty];

    fn as_str(self) -> &'static str {
        match self {
            Self::Five => "5",
            Self::Ten => "10",
            Self::Twenty => "20",
        }
    }
}

/// How often a partial depth stream pushes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthSpeed {
    /// `@100ms`.
    Ms100,
    /// `@250ms`.
    Ms250,
    /// `@500ms`.
    Ms500,
}

impl DepthSpeed {
    /// Every value, in order.
    pub const ALL: &'static [Self] = &[Self::Ms100, Self::Ms250, Self::Ms500];

    fn as_str(self) -> &'static str {
        match self {
            Self::Ms100 => "100ms",
            Self::Ms250 => "250ms",
            Self::Ms500 => "500ms",
        }
    }
}

/// A USDⓈ-M market stream.
///
/// `Display` renders the wire name, with the symbol's ASCII letters
/// lowercased (`btcusdt@aggTrade`, `币安人生usdt@markPrice@1s`), and `FromStr`
/// parses an echoed name back; [`Symbol::new`] uppercases ASCII, so the two
/// round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StreamName {
    /// `!ticker@arr`: 24-hour tickers of every symbol that changed.
    AllTickers,
    /// `!markPrice@arr@1s`: mark price and funding of every symbol, each second.
    AllMarkPrices,
    /// `<s>@aggTrade`.
    AggTrade(Symbol),
    /// `<s>@kline_<interval>`.
    Kline(Symbol, Interval),
    /// `<s>@markPrice@1s`.
    MarkPrice(Symbol),
    /// `<s>@ticker`.
    Ticker(Symbol),
    /// `<s>@depth<levels>@<speed>`, on the `/public` path.
    PartialDepth(Symbol, DepthLevels, DepthSpeed),
    /// `<s>@bookTicker`, on the `/public` path.
    BookTicker(Symbol),
}

impl StreamName {
    /// The path whose connection carries this stream.
    pub fn path(&self) -> StreamPath {
        match self {
            Self::PartialDepth(..) | Self::BookTicker(_) => StreamPath::Public,
            Self::AllTickers
            | Self::AllMarkPrices
            | Self::AggTrade(_)
            | Self::Kline(..)
            | Self::MarkPrice(_)
            | Self::Ticker(_) => StreamPath::Market,
        }
    }

    /// The symbol, for a single-symbol stream.
    pub fn symbol(&self) -> Option<&Symbol> {
        match self {
            Self::AllTickers | Self::AllMarkPrices => None,
            Self::AggTrade(s)
            | Self::Kline(s, _)
            | Self::MarkPrice(s)
            | Self::Ticker(s)
            | Self::PartialDepth(s, ..)
            | Self::BookTicker(s) => Some(s),
        }
    }
}

impl fmt::Display for StreamName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lower = |s: &Symbol| s.as_str().to_ascii_lowercase();
        match self {
            Self::AllTickers => f.write_str("!ticker@arr"),
            Self::AllMarkPrices => f.write_str("!markPrice@arr@1s"),
            Self::AggTrade(s) => write!(f, "{}@aggTrade", lower(s)),
            Self::Kline(s, interval) => write!(f, "{}@kline_{interval}", lower(s)),
            Self::MarkPrice(s) => write!(f, "{}@markPrice@1s", lower(s)),
            Self::Ticker(s) => write!(f, "{}@ticker", lower(s)),
            Self::PartialDepth(s, levels, speed) => write!(
                f,
                "{}@depth{}@{}",
                lower(s),
                levels.as_str(),
                speed.as_str()
            ),
            Self::BookTicker(s) => write!(f, "{}@bookTicker", lower(s)),
        }
    }
}

/// A string that is not a stream name this crate builds.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a USDⓈ-M stream name this crate supports")]
pub struct InvalidStreamName(pub String);

impl FromStr for StreamName {
    type Err = InvalidStreamName;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidStreamName(name.to_owned());
        match name {
            "!ticker@arr" => return Ok(Self::AllTickers),
            "!markPrice@arr@1s" => return Ok(Self::AllMarkPrices),
            _ => {}
        }
        let (symbol, kind) = name.split_once('@').ok_or_else(invalid)?;
        // A stream name spells its symbol in lowercase; one that does not was
        // not built here and, on the wire, delivers nothing.
        if symbol.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(invalid());
        }
        let symbol = Symbol::new(symbol).map_err(|_| invalid())?;
        match kind {
            "aggTrade" => Ok(Self::AggTrade(symbol)),
            "markPrice@1s" => Ok(Self::MarkPrice(symbol)),
            "ticker" => Ok(Self::Ticker(symbol)),
            "bookTicker" => Ok(Self::BookTicker(symbol)),
            _ => {
                if let Some(interval) = kind.strip_prefix("kline_") {
                    let interval = interval.parse().map_err(|_| invalid())?;
                    return Ok(Self::Kline(symbol, interval));
                }
                let depth = kind.strip_prefix("depth").ok_or_else(invalid)?;
                let (levels, speed) = depth.split_once('@').ok_or_else(invalid)?;
                let levels = *DepthLevels::ALL
                    .iter()
                    .find(|l| l.as_str() == levels)
                    .ok_or_else(invalid)?;
                let speed = *DepthSpeed::ALL
                    .iter()
                    .find(|s| s.as_str() == speed)
                    .ok_or_else(invalid)?;
                Ok(Self::PartialDepth(symbol, levels, speed))
            }
        }
    }
}
```

Notes for the reviewer:
- The symbol is lowercased with `to_ascii_lowercase`, mirroring `Symbol::new`'s `to_ascii_uppercase`. That is what makes `parse(display(x)) == x` hold, and prader-rs relies on it.
- `FromStr` refuses an uppercase symbol: such a name was not built here, and on the wire it delivers nothing.

- [ ] **Step 5: Write the error type**

Put this above the test module in `error.rs`:

```rust
//! Errors of the socket tiers and how each one recovers.

use std::time::Duration;

use thiserror::Error;

use crate::usdm::ws::{stream::StreamName, StreamPath};

/// What to do about an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Recovery {
    /// Replace the connection and replay its streams.
    Reconnect,
    /// Skip this frame and keep reading.
    SkipFrame,
    /// Give up: retrying replays the same failure.
    Fatal,
}

/// Error type for the socket tiers.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum UsdmWsError {
    /// The connection could not be opened, or failed mid-stream.
    #[error("WebSocket transport error: {0}")]
    Connect(#[from] Box<tokio_tungstenite::tungstenite::Error>),

    /// The connection did not open within the connect timeout.
    #[error("no connection within {0:?}")]
    ConnectTimeout(Duration),

    /// The server closed the connection, with its close code and reason when
    /// it sent them: `1008 "Too many requests"` after a burst of requests,
    /// `1008 "Invalid request"` after a refused 1025th stream.
    #[error("the server closed the connection ({code:?}: {reason})")]
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },

    /// The server answered a request with `{"error": {"code", "msg"}}`.
    #[error("the server refused the request: {code} {msg}")]
    Refused {
        /// Binance's code.
        code: i64,
        /// Binance's message.
        msg: String,
    },

    /// No answer to a request, or no pong to a ping (id 0), within the answer
    /// timeout.
    #[error("no answer to request {id} within {timeout:?}")]
    NoAnswer {
        /// The request id.
        id: u64,
        /// How long the client waited.
        timeout: Duration,
    },

    /// An answer that was not the documented shape.
    #[error("malformed answer to request {id}: {raw}")]
    Response {
        /// The request id.
        id: u64,
        /// The answer as sent.
        raw: String,
    },

    /// A subscribe that would take a connection past Binance's cap. Nothing
    /// was sent: the server answers the 1025th stream with an error and then
    /// closes the connection, losing all 1024.
    #[error("a {path} connection carries at most {limit} streams")]
    TooManyStreams {
        /// The path whose connection is full.
        path: StreamPath,
        /// The cap.
        limit: usize,
    },

    /// A stream for the other path, on the bare tier. Nothing was sent.
    #[error("{stream} rides the {} path, not {path}", stream.path())]
    WrongPath {
        /// The stream.
        stream: StreamName,
        /// The connection's path.
        path: StreamPath,
    },

    /// A text frame that did not decode as an update.
    #[error("a frame on {stream:?} did not decode: {reason}")]
    Frame {
        /// The envelope's stream name, or empty when there was none.
        stream: String,
        /// The frame as sent.
        raw: String,
        /// Why it did not decode.
        reason: String,
    },

    /// The supervised tier has stopped; its handles can no longer be used.
    #[error("the supervised connection has stopped")]
    Stopped,
}

impl From<tokio_tungstenite::tungstenite::Error> for UsdmWsError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connect(Box::new(err))
    }
}

impl UsdmWsError {
    /// How the supervised tier treats this error.
    pub fn recovery(&self) -> Recovery {
        use tokio_tungstenite::tungstenite::Error as Ws;
        match self {
            Self::Connect(err) => match &**err {
                // A handshake refused with 5xx or 429 is the host being unwell
                // or throttling; any other status is our request.
                Ws::Http(response) => {
                    let status = response.status();
                    if status.is_server_error() || status.as_u16() == 429 {
                        Recovery::Reconnect
                    } else {
                        Recovery::Fatal
                    }
                }
                Ws::Url(_) | Ws::Tls(_) | Ws::HttpFormat(_) | Ws::AttackAttempt => Recovery::Fatal,
                // `tungstenite::Error` is non_exhaustive; an unknown transport
                // error costs one retry cycle if it turns out permanent.
                _ => Recovery::Reconnect,
            },
            Self::ConnectTimeout(_)
            | Self::Closed { .. }
            | Self::NoAnswer { .. }
            | Self::Response { .. } => Recovery::Reconnect,
            Self::Frame { .. } => Recovery::SkipFrame,
            Self::Refused { .. }
            | Self::TooManyStreams { .. }
            | Self::WrongPath { .. }
            | Self::Stopped => Recovery::Fatal,
        }
    }
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --features ws --lib usdm::ws`
Expected: PASS, 8 tests. Dead-code warnings for `StreamPath::url` and `ensure_crypto_provider` are expected until Task 3.

Run: `cargo test -j 4 -p polyoxide-binance --lib`
Expected: PASS, 38 tests: without the feature nothing new compiles.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/Cargo.toml Cargo.lock polyoxide-binance/src/usdm/mod.rs polyoxide-binance/src/usdm/ws
git commit -m "feat(binance): ws feature, StreamName and UsdmWsError

StreamName is the only way to name one of the eight market streams: it
renders the wire name with the symbol lowercased, parses an echoed one
back, and knows its routed path. UsdmWsError says how each failure
recovers.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 2: Payloads and stream wire agreement

**Files:**
- Create: `polyoxide-binance/src/usdm/ws/event.rs`, `polyoxide-binance/src/usdm/ws/fixtures.rs`, `polyoxide-binance/tests/ws_wire_agreement.rs`
- Modify: `polyoxide-binance/src/usdm/ws/mod.rs`, `polyoxide-binance/tests/common/mod.rs`, `polyoxide-binance/Cargo.toml`

- [ ] **Step 1: Expose the captured frames**

Create `polyoxide-binance/src/usdm/ws/fixtures.rs`:

```rust
//! The captured stream frames, one combined-stream envelope per stream kind,
//! for tests here and downstream. Provenance is in `tests/fixtures/PROVENANCE.md`.

/// `!ticker@arr`, trimmed to two rows.
pub const ALL_TICKERS: &str = include_str!("../../../tests/fixtures/ws/stream_all_ticker_arr.json");
/// `!markPrice@arr@1s`, trimmed to two rows.
pub const ALL_MARK_PRICES: &str =
    include_str!("../../../tests/fixtures/ws/stream_all_markPrice_arr_1s.json");
/// `btcusdt@aggTrade`.
pub const AGG_TRADE: &str = include_str!("../../../tests/fixtures/ws/stream_btcusdt_aggTrade.json");
/// `btcusdt@kline_1m`.
pub const KLINE: &str = include_str!("../../../tests/fixtures/ws/stream_btcusdt_kline_1m.json");
/// `btcusdt@markPrice@1s`.
pub const MARK_PRICE: &str =
    include_str!("../../../tests/fixtures/ws/stream_btcusdt_markPrice_1s.json");
/// `btcusdt@ticker`.
pub const TICKER: &str = include_str!("../../../tests/fixtures/ws/stream_btcusdt_ticker.json");
/// `btcusdt@depth20@100ms`, sides trimmed to three levels.
pub const PARTIAL_DEPTH: &str =
    include_str!("../../../tests/fixtures/ws/stream_btcusdt_depth20_100ms.json");
/// `btcusdt@bookTicker`.
pub const BOOK_TICKER: &str =
    include_str!("../../../tests/fixtures/ws/stream_btcusdt_bookTicker.json");

/// Every frame, with its fixture's file stem.
pub const ALL: &[(&str, &str)] = &[
    ("stream_all_ticker_arr", ALL_TICKERS),
    ("stream_all_markPrice_arr_1s", ALL_MARK_PRICES),
    ("stream_btcusdt_aggTrade", AGG_TRADE),
    ("stream_btcusdt_kline_1m", KLINE),
    ("stream_btcusdt_markPrice_1s", MARK_PRICE),
    ("stream_btcusdt_ticker", TICKER),
    ("stream_btcusdt_depth20_100ms", PARTIAL_DEPTH),
    ("stream_btcusdt_bookTicker", BOOK_TICKER),
];
```

In `polyoxide-binance/src/usdm/ws/mod.rs`, replace

```rust
pub mod error;
pub mod stream;
```

with

```rust
pub mod error;
pub mod event;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod fixtures;
pub mod stream;
```

and replace

```rust
pub use error::{Recovery, UsdmWsError};
```

with

```rust
pub use error::{Recovery, UsdmWsError};
pub use event::{
    AggTradeEvent, BookTickerEvent, KlineBar, KlineEvent, MarkPriceEvent, PartialDepthEvent,
    Payload, SymbolType, TickerEvent, Update,
};
```

- [ ] **Step 2: Write the payload tests**

Create `polyoxide-binance/src/usdm/ws/event.rs` with its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::usdm::ws::fixtures;

    #[test]
    fn every_captured_frame_decodes_as_its_stream() {
        for (name, frame) in fixtures::ALL {
            let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                !matches!(update.payload, Payload::Unknown { .. }),
                "{name} decoded as Unknown"
            );
        }
    }

    #[test]
    fn the_mark_price_array_keeps_every_field_prader_reads() {
        let update = Update::from_json(fixtures::ALL_MARK_PRICES).unwrap();
        assert_eq!(update.stream, StreamName::AllMarkPrices);
        let Payload::MarkPrices(rows) = update.payload else {
            panic!("not MarkPrices");
        };
        assert_eq!(rows.len(), 2);
        assert!(rows[0].mark_price > Decimal::ZERO);
        assert!(rows[0].next_funding_time > 0);
        assert_eq!(rows[0].symbol_type, SymbolType::Um);
    }

    #[test]
    fn a_coin_m_row_and_an_unknown_symbol_type_both_decode() {
        assert_eq!(
            serde_json::from_str::<SymbolType>("2").unwrap(),
            SymbolType::Cm
        );
        assert_eq!(
            serde_json::from_str::<SymbolType>("7").unwrap(),
            SymbolType::Other(7)
        );
        assert_eq!(serde_json::to_string(&SymbolType::Other(7)).unwrap(), "7");
    }

    #[test]
    fn a_renamed_event_is_kept_whole_and_a_broken_one_is_a_frame_error() {
        let renamed = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTradeV2","x":1}}"#;
        let update = Update::from_json(renamed).unwrap();
        assert!(matches!(
            &update.payload,
            Payload::Unknown { event_type, .. } if event_type == "aggTradeV2"
        ));

        let broken = r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade","p":"x"}}"#;
        let err = Update::from_json(broken).unwrap_err();
        assert!(
            matches!(&err, UsdmWsError::Frame { stream, .. } if stream == "btcusdt@aggTrade"),
            "{err:?}"
        );

        let unbuilt = r#"{"stream":"btcusdt@forceOrder","data":{}}"#;
        assert!(matches!(
            Update::from_json(unbuilt),
            Err(UsdmWsError::Frame { .. })
        ));
        assert!(matches!(
            Update::from_json("not json"),
            Err(UsdmWsError::Frame { .. })
        ));
    }

    #[test]
    fn an_update_serialises_back_to_its_envelope() {
        let update = Update::from_json(fixtures::BOOK_TICKER).unwrap();
        let again: Value = serde_json::to_value(&update).unwrap();
        let wire: Value = serde_json::from_str(fixtures::BOOK_TICKER).unwrap();
        assert_eq!(again, wire);
    }
}
```

Replace `polyoxide-binance/tests/common/mod.rs` with this version. It allows dead code, since each test file uses a different part, and adds `compare_values`:

```rust
//! Wire-agreement machinery shared by the REST and stream agreement tests and
//! the live suites.

// Each test file that includes this module uses a different part of it.
#![allow(dead_code)]

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
    compare_values(what, &wire, &serde_json::to_value(&parsed).unwrap())
}

/// Compares what the wire sent with what a type emitted after decoding it.
pub fn compare_values(what: &str, wire: &Value, emitted: &Value) -> Disagreement {
    assert_values_agree(what, "", wire, emitted);
    let mut sent = BTreeSet::new();
    key_paths(wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(emitted, "", &mut modelled);
    Disagreement {
        unmodelled: sent.difference(&modelled).cloned().collect(),
        invented: modelled.difference(&sent).cloned().collect(),
    }
}
```

Create `polyoxide-binance/tests/ws_wire_agreement.rs`:

```rust
//! Agreement between the stream payload types and the captured frames in
//! `tests/fixtures/ws/`, by the rules of `wire_agreement.rs`: nothing
//! unmodelled, nothing invented, nothing altered.

mod common;

use polyoxide_binance::usdm::ws::{fixtures, Payload, Update};
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately drop.
pub const IGNORED: &[(&str, &str, &str)] = &[(
    "stream_btcusdt_kline_1m",
    "/data/k/B",
    "Binance documents the kline's `B` as \"Ignore\"",
)];

#[test]
fn every_stream_fixture_agrees_with_its_type() {
    let mut used = Vec::new();
    for (fixture, frame) in fixtures::ALL {
        let update = Update::from_json(frame).unwrap_or_else(|e| panic!("{fixture}: {e}"));
        assert!(
            !matches!(update.payload, Payload::Unknown { .. }),
            "{fixture}: decoded as Unknown"
        );
        let wire: Value = serde_json::from_str(frame).unwrap();
        let emitted = serde_json::to_value(&update).unwrap();
        let diff = common::compare_values(fixture, &wire, &emitted);
        for path in diff.unmodelled {
            match IGNORED.iter().find(|(f, p, _)| f == fixture && *p == path) {
                Some(entry) => used.push(entry),
                None => panic!("{fixture}: the server sent {path}, which the type does not model"),
            }
        }
        assert!(
            diff.invented.is_empty(),
            "{fixture}: the type emits {:?}, which the server did not send",
            diff.invented
        );
    }
    assert_eq!(
        used.len(),
        IGNORED.len(),
        "an IGNORED entry no fixture needs"
    );
}
```

Append to `polyoxide-binance/Cargo.toml`, after a blank line:

```toml
# The socket tests import items that exist only behind their feature;
# `required-features` keeps a plain `cargo test --all-targets` from building
# them without it.
[[test]]
name = "ws_wire_agreement"
required-features = ["test-server"]
```

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --lib usdm::ws::event`
Expected: FAIL to compile: ``cannot find type `Update` in this scope`` and the like.

- [ ] **Step 3: Write the payloads**

Put this above the test module in `event.rs`:

```rust
//! Stream payloads, and the update that carries one.
//!
//! The socket uses one-letter keys and different fields from REST, so each
//! stream has its own payload type, with long field names over the wire's keys.
//! Symbols are `String`, as sent, in the listing's case.

use std::fmt;

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::usdm::{
    types::{Interval, Level},
    ws::{error::UsdmWsError, stream::StreamName},
};

/// Which futures market a symbol belongs to, from a payload's `st`.
///
/// Binance documents it as "(After CM migration) Symbol type: 1 = UM, 2 = CM".
/// COIN-M symbols do arrive on this host: 30 of 745 rows of one
/// `!markPrice@arr@1s` frame on 2026-10-07 carried `2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SymbolType {
    /// `1`: USDⓈ-M.
    Um,
    /// `2`: COIN-M.
    Cm,
    /// A value this version does not know, kept as sent.
    Other(u64),
}

impl Serialize for SymbolType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(match self {
            Self::Um => 1,
            Self::Cm => 2,
            Self::Other(raw) => *raw,
        })
    }
}

impl<'de> Deserialize<'de> for SymbolType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match u64::deserialize(deserializer)? {
            1 => Self::Um,
            2 => Self::Cm,
            raw => Self::Other(raw),
        })
    }
}

/// `<s>@ticker` and each row of `!ticker@arr`: rolling 24-hour statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TickerEvent {
    /// `24hrTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The pair, as listed.
    #[serde(rename = "ps")]
    pub pair: String,
    /// Last price less the open price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price_change: Decimal,
    /// The change as a percentage.
    #[serde(rename = "P", with = "rust_decimal::serde::str")]
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    #[serde(rename = "w", with = "rust_decimal::serde::str")]
    pub weighted_avg_price: Decimal,
    /// Last price.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub last_price: Decimal,
    /// Last quantity.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub last_quantity: Decimal,
    /// Price 24 hours ago.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open_price: Decimal,
    /// Highest price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high_price: Decimal,
    /// Lowest price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low_price: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Window start.
    #[serde(rename = "O")]
    pub open_time: u64,
    /// Window end.
    #[serde(rename = "C")]
    pub close_time: u64,
    /// First trade id in the window.
    #[serde(rename = "F")]
    pub first_trade_id: i64,
    /// Last trade id in the window.
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Trades in the window.
    #[serde(rename = "n")]
    pub trade_count: u64,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// `<s>@markPrice@1s` and each row of `!markPrice@arr@1s`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarkPriceEvent {
    /// `markPriceUpdate`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Mark price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub mark_price: Decimal,
    /// Mark price moving average.
    #[serde(rename = "ap", with = "rust_decimal::serde::str")]
    pub mark_price_moving_average: Decimal,
    /// Estimated settle price, meaningful only in the hour before a settlement.
    #[serde(rename = "P", with = "rust_decimal::serde::str")]
    pub estimated_settle_price: Decimal,
    /// Index price.
    #[serde(rename = "i", with = "rust_decimal::serde::str")]
    pub index_price: Decimal,
    /// Funding rate. `0` where no funding is scheduled.
    #[serde(rename = "r", with = "rust_decimal::serde::str")]
    pub funding_rate: Decimal,
    /// Next funding time; `0` means none is scheduled, as on 51 of 745 rows on
    /// 2026-10-07 (delisted contracts).
    #[serde(rename = "T")]
    pub next_funding_time: u64,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// `<s>@aggTrade`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AggTradeEvent {
    /// `aggTrade`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// Aggregate trade id.
    #[serde(rename = "a")]
    pub id: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// Price.
    #[serde(rename = "p", with = "rust_decimal::serde::str")]
    pub price: Decimal,
    /// Quantity.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    /// Quantity without the trades involving RPI orders.
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
    pub trade_time: u64,
    /// Whether the buyer was the maker, so the taker sold.
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// `<s>@kline_<interval>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KlineEvent {
    /// `kline`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The candle so far.
    #[serde(rename = "k")]
    pub kline: KlineBar,
}

/// The candle in a [`KlineEvent`]. The wire's `B`, documented as "Ignore", is
/// dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KlineBar {
    /// Candle start.
    #[serde(rename = "t")]
    pub open_time: u64,
    /// Candle end, inclusive.
    #[serde(rename = "T")]
    pub close_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The candle's width.
    #[serde(rename = "i")]
    pub interval: Interval,
    /// First trade id.
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    /// Last trade id.
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Open price.
    #[serde(rename = "o", with = "rust_decimal::serde::str")]
    pub open: Decimal,
    /// Close price so far.
    #[serde(rename = "c", with = "rust_decimal::serde::str")]
    pub close: Decimal,
    /// High price.
    #[serde(rename = "h", with = "rust_decimal::serde::str")]
    pub high: Decimal,
    /// Low price.
    #[serde(rename = "l", with = "rust_decimal::serde::str")]
    pub low: Decimal,
    /// Base-asset volume.
    #[serde(rename = "v", with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Trades.
    #[serde(rename = "n")]
    pub trade_count: u64,
    /// Whether the candle is closed.
    #[serde(rename = "x")]
    pub is_closed: bool,
    /// Quote-asset volume.
    #[serde(rename = "q", with = "rust_decimal::serde::str")]
    pub quote_volume: Decimal,
    /// Base-asset volume bought by takers.
    #[serde(rename = "V", with = "rust_decimal::serde::str")]
    pub taker_buy_base_volume: Decimal,
    /// Quote-asset volume bought by takers.
    #[serde(rename = "Q", with = "rust_decimal::serde::str")]
    pub taker_buy_quote_volume: Decimal,
}

/// `<s>@depth<levels>@<speed>`: the top of the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PartialDepthEvent {
    /// `depthUpdate`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// When the book last changed.
    #[serde(rename = "T")]
    pub transaction_time: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The pair, as listed.
    #[serde(rename = "ps")]
    pub pair: String,
    /// First book update id in the event.
    #[serde(rename = "U")]
    pub first_update_id: u64,
    /// Last book update id in the event.
    #[serde(rename = "u")]
    pub final_update_id: u64,
    /// Last book update id of the previous event.
    #[serde(rename = "pu")]
    pub previous_final_update_id: u64,
    /// Bids, best first.
    #[serde(rename = "b")]
    pub bids: Vec<Level>,
    /// Asks, best first.
    #[serde(rename = "a")]
    pub asks: Vec<Level>,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// `<s>@bookTicker`: the best bid and ask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BookTickerEvent {
    /// `bookTicker`.
    #[serde(rename = "e")]
    pub event_type: String,
    /// Book update id.
    #[serde(rename = "u")]
    pub update_id: u64,
    /// The symbol, as listed.
    #[serde(rename = "s")]
    pub symbol: String,
    /// The pair, as listed.
    #[serde(rename = "ps")]
    pub pair: String,
    /// Best bid price.
    #[serde(rename = "b", with = "rust_decimal::serde::str")]
    pub bid_price: Decimal,
    /// Best bid quantity.
    #[serde(rename = "B", with = "rust_decimal::serde::str")]
    pub bid_quantity: Decimal,
    /// Best ask price.
    #[serde(rename = "a", with = "rust_decimal::serde::str")]
    pub ask_price: Decimal,
    /// Best ask quantity.
    #[serde(rename = "A", with = "rust_decimal::serde::str")]
    pub ask_quantity: Decimal,
    /// When the book last changed.
    #[serde(rename = "T")]
    pub transaction_time: u64,
    /// When the event was produced.
    #[serde(rename = "E")]
    pub event_time: u64,
    /// USDⓈ-M or COIN-M.
    #[serde(rename = "st")]
    pub symbol_type: SymbolType,
}

/// What a stream carries, by stream kind.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Payload {
    /// `!ticker@arr`.
    Tickers(Vec<TickerEvent>),
    /// `!markPrice@arr@1s`.
    MarkPrices(Vec<MarkPriceEvent>),
    /// `<s>@aggTrade`.
    AggTrade(AggTradeEvent),
    /// `<s>@kline_<interval>`.
    Kline(KlineEvent),
    /// `<s>@markPrice@1s`.
    MarkPrice(MarkPriceEvent),
    /// `<s>@ticker`.
    Ticker(TickerEvent),
    /// `<s>@depth<levels>@<speed>`.
    PartialDepth(PartialDepthEvent),
    /// `<s>@bookTicker`.
    BookTicker(BookTickerEvent),
    /// An object whose event type (`e`) is not the one its stream carries,
    /// kept whole rather than failing the frame. The live suite fails on one,
    /// since it means Binance renamed an event.
    Unknown {
        /// The payload's `e`, or empty when it has none.
        event_type: String,
        /// The payload as sent.
        raw: String,
    },
}

impl Serialize for Payload {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Tickers(rows) => rows.serialize(serializer),
            Self::MarkPrices(rows) => rows.serialize(serializer),
            Self::AggTrade(event) => event.serialize(serializer),
            Self::Kline(event) => event.serialize(serializer),
            Self::MarkPrice(event) => event.serialize(serializer),
            Self::Ticker(event) => event.serialize(serializer),
            Self::PartialDepth(event) => event.serialize(serializer),
            Self::BookTicker(event) => event.serialize(serializer),
            Self::Unknown { raw, .. } => serde_json::from_str::<Value>(raw)
                .map_err(serde::ser::Error::custom)?
                .serialize(serializer),
        }
    }
}

/// One frame of a combined stream: `{"stream": <name>, "data": <payload>}`.
///
/// It serialises back to that envelope, so `serde_json::to_string(&update)`
/// is the frame as the wire sent it, less the fields this crate drops.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Update {
    /// The stream the frame arrived on.
    pub stream: StreamName,
    /// What it carries.
    pub payload: Payload,
}

impl Serialize for Update {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("stream", &self.stream.to_string())?;
        map.serialize_entry("data", &self.payload)?;
        map.end()
    }
}

#[derive(Deserialize)]
struct Envelope {
    stream: String,
    data: Value,
}

impl Update {
    /// Decodes one combined-stream frame.
    ///
    /// Fails with [`UsdmWsError::Frame`] when the text is not an envelope, its
    /// stream is not one this crate builds, or its payload does not decode as
    /// the stream's type.
    pub fn from_json(text: &str) -> Result<Self, UsdmWsError> {
        let envelope: Envelope =
            serde_json::from_str(text).map_err(|err| frame_error("", text, err))?;
        let stream: StreamName = envelope
            .stream
            .parse()
            .map_err(|err| frame_error(&envelope.stream, text, err))?;
        let payload = decode(&stream, envelope.data)
            .map_err(|err| frame_error(&envelope.stream, text, err))?;
        Ok(Self { stream, payload })
    }
}

fn frame_error(stream: &str, raw: &str, reason: impl fmt::Display) -> UsdmWsError {
    UsdmWsError::Frame {
        stream: stream.to_owned(),
        raw: raw.to_owned(),
        reason: reason.to_string(),
    }
}

fn decode(stream: &StreamName, data: Value) -> Result<Payload, serde_json::Error> {
    let expected = match stream {
        StreamName::AllTickers => return serde_json::from_value(data).map(Payload::Tickers),
        StreamName::AllMarkPrices => return serde_json::from_value(data).map(Payload::MarkPrices),
        StreamName::AggTrade(_) => "aggTrade",
        StreamName::Kline(..) => "kline",
        StreamName::MarkPrice(_) => "markPriceUpdate",
        StreamName::Ticker(_) => "24hrTicker",
        StreamName::PartialDepth(..) => "depthUpdate",
        StreamName::BookTicker(_) => "bookTicker",
    };
    let event_type = data.get("e").and_then(Value::as_str).unwrap_or_default();
    if event_type != expected {
        return Ok(Payload::Unknown {
            event_type: event_type.to_owned(),
            raw: data.to_string(),
        });
    }
    Ok(match stream {
        StreamName::AggTrade(_) => Payload::AggTrade(serde_json::from_value(data)?),
        StreamName::Kline(..) => Payload::Kline(serde_json::from_value(data)?),
        StreamName::MarkPrice(_) => Payload::MarkPrice(serde_json::from_value(data)?),
        StreamName::Ticker(_) => Payload::Ticker(serde_json::from_value(data)?),
        StreamName::PartialDepth(..) => Payload::PartialDepth(serde_json::from_value(data)?),
        StreamName::BookTicker(_) => Payload::BookTicker(serde_json::from_value(data)?),
        StreamName::AllTickers | StreamName::AllMarkPrices => unreachable!("returned above"),
    })
}
```

Notes for the reviewer:
- Field names follow prader-rs's contract (`event_time`, `last_price`, `funding_rate`, `next_funding_time`, `trade_time`, `is_buyer_maker`, `bid_quantity`, ...); payload symbols are `String`, in the listing's case.
- `Payload::Unknown` is for an object whose `e` is not its stream's: kept whole, not an error, and the live suite fails on one. A payload that has the right `e` but does not decode is `UsdmWsError::Frame`.
- `Update` serialises back to the `{"stream", "data"}` envelope, which is what the CLI prints in JSON mode.

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --lib usdm::ws`
Expected: PASS, 13 tests.

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --test ws_wire_agreement --test wire_agreement`
Expected: PASS, 1 and 2 tests.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-binance
git commit -m "feat(binance): stream payloads, Update::from_json, stream wire agreement

One type per stream kind with long names over the socket's one-letter
keys, SymbolType for st (COIN-M rows arrive on this host), and Update,
which decodes a combined-stream envelope and serialises back to it. The
captured frames are compiled in under test-server for downstream tests.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 3: The bare tier and the scripted server

**Files:**
- Create: `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/test_server.rs`
- Modify: `polyoxide-binance/src/usdm/ws/mod.rs`

- [ ] **Step 1: Write the scripted server**

`polyoxide-binance/src/usdm/ws/test_server.rs`, in the shape prader-rs's contract expects (`ScriptedServer::start(Vec<Script>)`, `Script { pushes, close_after, ..Default::default() }`, `server.url`, `server.connection_count()`):

```rust
//! A local server speaking Binance's combined-stream protocol from a script,
//! for the offline tests here and downstream. Behind `test-server`.
//!
//! It accepts any path, answers `SUBSCRIBE`, `UNSUBSCRIBE` and
//! `LIST_SUBSCRIPTIONS` as Binance does, and answers protocol pings while it
//! reads.

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    net::{TcpListener, TcpStream},
    time::Instant,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{Request, Response},
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
};

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Frames to send, in order, after answering the first request.
    pub pushes: Vec<String>,
    /// While the connection is held open, resend the first push on this
    /// cadence: steady traffic.
    pub push_every: Option<Duration>,
    /// Close the connection after the pushes.
    pub close_after: bool,
    /// The close code and reason to send when closing.
    pub close_code: Option<(u16, String)>,
    /// Close the connection this long after the handshake, whatever else is
    /// happening.
    pub drop_after: Option<Duration>,
    /// After the pushes, stop reading and writing for good: a dead socket that
    /// answers neither requests nor pings.
    pub go_silent: bool,
    /// Send a protocol ping on this cadence while the connection is held open.
    pub ping_every: Option<Duration>,
    /// A request naming one of these streams is answered with
    /// `{"error": {"code", "msg"}}`.
    pub refuse: Vec<(String, i64, String)>,
    /// Leave every request after the first unanswered.
    pub ignore_later_requests: bool,
    /// Accept the TCP connection and drop it without a handshake.
    pub reject_handshake: bool,
}

/// One text frame the client sent.
#[derive(Debug, Clone)]
pub struct Received {
    /// Which connection, counted from zero.
    pub connection: usize,
    /// When it arrived.
    pub at: Instant,
    /// The frame, parsed.
    pub request: Value,
}

#[derive(Default)]
struct Recorder {
    paths: Mutex<Vec<String>>,
    received: Mutex<Vec<Received>>,
    pings: AtomicUsize,
    closes: AtomicUsize,
    ended: AtomicUsize,
}

/// A running local server.
pub struct ScriptedServer {
    /// The base URL to connect to, `ws://127.0.0.1:<port>`.
    pub url: String,
    connections: Arc<AtomicUsize>,
    recorder: Arc<Recorder>,
}

impl ScriptedServer {
    /// Apply `scripts[n]` to the n-th connection, repeating the last.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let connections = Arc::new(AtomicUsize::new(0));
        let recorder = Arc::new(Recorder::default());
        let (count, rec) = (Arc::clone(&connections), Arc::clone(&recorder));
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let index = count.fetch_add(1, Ordering::SeqCst);
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or_default();
                let recorder = Arc::clone(&rec);
                tokio::spawn(async move {
                    let _ = serve(stream, index, script, &recorder).await;
                    recorder.ended.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        Self {
            url: format!("ws://{addr}"),
            connections,
            recorder,
        }
    }

    /// Connections accepted so far, counted before the handshake.
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.recorder.paths.lock().unwrap().clone()
    }

    /// Every request received, in arrival order, across connections.
    pub fn received(&self) -> Vec<Received> {
        self.recorder.received.lock().unwrap().clone()
    }

    /// Every request received, parsed.
    pub fn requests(&self) -> Vec<Value> {
        self.received().into_iter().map(|r| r.request).collect()
    }

    /// Protocol pings received from the client.
    pub fn ping_count(&self) -> usize {
        self.recorder.pings.load(Ordering::SeqCst)
    }

    /// Connections the client closed with a Close frame.
    pub fn close_count(&self) -> usize {
        self.recorder.closes.load(Ordering::SeqCst)
    }

    /// Connections that have ended, however they ended.
    pub fn ended_count(&self) -> usize {
        self.recorder.ended.load(Ordering::SeqCst)
    }

    /// Poll until `predicate` holds, or panic with `label` after two seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for {label}");
    }
}

/// The answer Binance gives to one request, given the connection's streams.
fn answer(script: &Script, request: &Value, streams: &mut Vec<String>) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params: Vec<String> = request["params"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|n| n.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some((_, code, msg)) = script
        .refuse
        .iter()
        .find(|(name, _, _)| params.contains(name))
    {
        return json!({ "error": { "code": code, "msg": msg }, "id": id });
    }
    match request["method"].as_str() {
        Some("SUBSCRIBE") => {
            for name in params {
                if !streams.contains(&name) {
                    streams.push(name);
                }
            }
            json!({ "result": null, "id": id })
        }
        Some("UNSUBSCRIBE") => {
            streams.retain(|name| !params.contains(name));
            json!({ "result": null, "id": id })
        }
        Some("LIST_SUBSCRIPTIONS") => json!({ "result": streams, "id": id }),
        _ => json!({ "error": { "code": 2, "msg": "Invalid request" }, "id": id }),
    }
}

// The handshake callback's error type is tungstenite's, fixed by its signature.
#[allow(clippy::result_large_err)]
async fn serve(
    stream: TcpStream,
    index: usize,
    script: Script,
    recorder: &Recorder,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if script.reject_handshake {
        drop(stream);
        return Ok(());
    }
    let path = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&path);
    let mut ws = accept_hdr_async(stream, move |request: &Request, response: Response| {
        *seen.lock().unwrap() = request.uri().path().to_owned();
        Ok(response)
    })
    .await?;
    recorder
        .paths
        .lock()
        .unwrap()
        .push(path.lock().unwrap().clone());

    let close_frame = script.close_code.clone().map(|(code, reason)| CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    });
    let drop_at = script.drop_after.map(|after| Instant::now() + after);
    let mut streams: Vec<String> = Vec::new();
    let mut answered = 0usize;
    let mut pushed = false;
    let mut push_every = script.push_every.map(tokio::time::interval);
    let mut ping_every = script.ping_every.map(tokio::time::interval);

    loop {
        tokio::select! {
            () = async {
                match drop_at {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            } => {
                let _ = ws.close(close_frame.clone()).await;
                return Ok(());
            }
            _ = async {
                match ping_every.as_mut() {
                    Some(interval) => { interval.tick().await; }
                    None => std::future::pending::<()>().await,
                }
            } => {
                ws.send(Message::Ping(Vec::new().into())).await?;
            }
            _ = async {
                match push_every.as_mut() {
                    Some(interval) if pushed => { interval.tick().await; }
                    _ => std::future::pending::<()>().await,
                }
            } => {
                if let Some(first) = script.pushes.first() {
                    ws.send(Message::Text(first.clone().into())).await?;
                }
            }
            message = ws.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    let Ok(request) = serde_json::from_str::<Value>(&text) else { continue };
                    recorder.received.lock().unwrap().push(Received {
                        connection: index,
                        at: Instant::now(),
                        request: request.clone(),
                    });
                    if answered > 0 && script.ignore_later_requests {
                        continue;
                    }
                    let reply = answer(&script, &request, &mut streams);
                    ws.send(Message::Text(reply.to_string().into())).await?;
                    answered += 1;
                    if answered == 1 {
                        for push in &script.pushes {
                            ws.send(Message::Text(push.clone().into())).await?;
                        }
                        pushed = true;
                        if script.close_after {
                            let _ = ws.close(close_frame.clone()).await;
                            return Ok(());
                        }
                        if script.go_silent {
                            // Hold the socket without reading it: nothing is
                            // answered, pings included.
                            std::future::pending::<()>().await;
                        }
                    }
                }
                Some(Ok(Message::Ping(_))) => {
                    recorder.pings.fetch_add(1, Ordering::SeqCst);
                }
                Some(Ok(Message::Close(_))) => {
                    recorder.closes.fetch_add(1, Ordering::SeqCst);
                    return Ok(());
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return Ok(()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_list_and_unsubscribe_answer_as_binance_does() {
        let script = Script::default();
        let mut streams = Vec::new();
        let sub = json!({"method": "SUBSCRIBE", "params": ["btcusdt@aggTrade", "ethusdt@aggTrade"], "id": 1});
        assert_eq!(
            answer(&script, &sub, &mut streams),
            json!({"result": null, "id": 1})
        );
        let unsub = json!({"method": "UNSUBSCRIBE", "params": ["btcusdt@aggTrade"], "id": 2});
        answer(&script, &unsub, &mut streams);
        let list = json!({"method": "LIST_SUBSCRIPTIONS", "id": 3});
        assert_eq!(
            answer(&script, &list, &mut streams),
            json!({"result": ["ethusdt@aggTrade"], "id": 3})
        );
    }

    #[test]
    fn a_scripted_refusal_is_an_error_answer() {
        let script = Script {
            refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
            ..Script::default()
        };
        let sub = json!({"method": "SUBSCRIBE", "params": ["ethusdt@aggTrade"], "id": 9});
        assert_eq!(
            answer(&script, &sub, &mut Vec::new()),
            json!({"error": {"code": 2, "msg": "Invalid request"}, "id": 9})
        );
    }
}
```

- [ ] **Step 2: Register the modules**

In `polyoxide-binance/src/usdm/ws/mod.rs`, replace

```rust
pub mod error;
pub mod event;
```

with

```rust
pub mod client;
pub mod error;
pub mod event;
```

replace

```rust
pub mod stream;

use std::{fmt, time::Duration};
```

with

```rust
pub mod stream;
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;

use std::{fmt, time::Duration};
```

and replace

```rust
pub use error::{Recovery, UsdmWsError};
```

with

```rust
pub use client::UsdmWs;
pub use error::{Recovery, UsdmWsError};
```

- [ ] **Step 3: Write the bare tier's tests**

Create `polyoxide-binance/src/usdm/ws/client.rs` with its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::usdm::{
        types::Symbol,
        ws::{
            event::Payload,
            fixtures,
            stream::{DepthLevels, DepthSpeed},
            test_server::{Script, ScriptedServer},
        },
    };

    fn agg(symbol: &str) -> StreamName {
        StreamName::AggTrade(Symbol::new(symbol).unwrap())
    }

    #[tokio::test]
    async fn connect_subscribes_on_its_path_and_yields_updates() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![fixtures::AGG_TRADE.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
        assert_eq!(server.paths(), ["/market/stream"]);
        assert_eq!(
            server.requests()[0]["params"],
            serde_json::json!(["btcusdt@aggTrade"])
        );
    }

    #[tokio::test]
    async fn a_stream_for_the_other_path_is_refused_before_sending() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let depth = StreamName::PartialDepth(
            Symbol::new("BTCUSDT").unwrap(),
            DepthLevels::Five,
            DepthSpeed::Ms100,
        );
        let err = ws.subscribe(&[depth]).await.unwrap_err();
        assert!(matches!(
            err,
            UsdmWsError::WrongPath {
                path: StreamPath::Market,
                ..
            }
        ));
        assert_eq!(server.requests().len(), 1);
    }

    #[tokio::test]
    async fn requests_carry_at_most_200_names_and_are_paced() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let streams: Vec<StreamName> = (0..450).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
        let start = Instant::now();
        let ws = UsdmWs::connect_to(&server.url, StreamPath::Market, streams.clone())
            .await
            .unwrap();
        assert_eq!(ws.streams().len(), 450);
        let sizes: Vec<usize> = server
            .requests()
            .iter()
            .map(|r| r["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [200, 200, 50]);
        assert!(
            start.elapsed() >= MIN_REQUEST_INTERVAL * 2,
            "three requests in {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn the_1025th_stream_is_refused_and_nothing_is_sent() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let streams: Vec<StreamName> = (0..1024).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, streams)
            .await
            .unwrap();
        let sent = server.requests().len();
        let err = ws.subscribe(&[agg("BTCUSDT")]).await.unwrap_err();
        assert!(matches!(
            err,
            UsdmWsError::TooManyStreams { limit: 1024, .. }
        ));
        assert_eq!(server.requests().len(), sent);
        // Already-subscribed names do not count twice.
        ws.subscribe(&[agg("ZZ0000USDT")]).await.unwrap();
    }

    #[tokio::test]
    async fn an_error_answer_is_refused_and_the_connection_kept() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, UsdmWsError::Refused { code: 2, .. }),
            "{err:?}"
        );
        assert_eq!(ws.streams(), &[agg("BTCUSDT")]);
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["btcusdt@aggTrade"]);
    }

    #[tokio::test]
    async fn a_ping_is_answered_and_unsubscribe_leaves_the_rest() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = UsdmWs::connect_to(
            &server.url,
            StreamPath::Market,
            [agg("BTCUSDT"), agg("ETHUSDT")],
        )
        .await
        .unwrap();
        assert!(ws.ping().await.unwrap() < Duration::from_secs(1));
        ws.unsubscribe(&[agg("BTCUSDT")]).await.unwrap();
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["ethusdt@aggTrade"]);
    }

    #[tokio::test]
    async fn the_server_s_close_code_is_reported() {
        let server = ScriptedServer::start(vec![Script {
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        assert!(ws.next().await.is_none());
        assert_eq!(
            ws.close_frame(),
            Some((Some(1008), "Too many requests".to_owned()))
        );
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, UsdmWsError::Connect(_) | UsdmWsError::Closed { .. }),
            "{err:?}"
        );
    }
}
```

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --lib usdm::ws`
Expected: FAIL to compile: ``cannot find type `UsdmWs` in this scope``.

- [ ] **Step 4: Write the bare tier**

Put this above the test module in `client.rs`:

```rust
//! The bare tier: one connection on one path, control methods, and a `Stream`
//! of updates.

use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use tokio::{net::TcpStream, time::Instant};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::CloseFrame, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::usdm::ws::{
    ensure_crypto_provider, error::UsdmWsError, event::Update, stream::StreamName, StreamPath,
    MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL, USDM_WS_BASE,
};

/// How long the bare tier waits for a connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the bare tier waits for a request's answer.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// A bare connection to one path's combined stream.
///
/// Requests are batched at most 200 names each and sent no faster than one
/// per 200 ms; each waits for the answer carrying its id, and updates read
/// meanwhile are kept and yielded by the stream afterwards. A stream for the
/// other path, or one that would take the connection past 1024 streams, is
/// refused before anything is sent. The server's protocol pings are answered
/// while the caller polls. Binance closes a connection after 24 hours; the
/// supervised tier ([`UsdmWsBuilder`](crate::usdm::ws::UsdmWsBuilder))
/// replaces it before then, and this tier does not.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{StreamName, StreamPath, UsdmWs}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::AggTrade(btc)]).await?;
/// while let Some(update) = ws.next().await {
///     println!("{}", serde_json::to_string(&update?)?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct UsdmWs {
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    path: StreamPath,
    streams: Vec<StreamName>,
    next_id: u64,
    buffered: VecDeque<Result<Update, UsdmWsError>>,
    last_request: Option<Instant>,
    last_inbound: Instant,
    answer_timeout: Duration,
    close_frame: Option<(Option<u16>, String)>,
}

impl std::fmt::Debug for UsdmWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsdmWs")
            .field("path", &self.path)
            .field("streams", &self.streams.len())
            .field("buffered", &self.buffered.len())
            .finish_non_exhaustive()
    }
}

/// What a text frame that is not an update is.
enum Answer {
    /// `{"result": .., "id": n}`.
    Result { id: Option<u64>, result: Value },
    /// `{"error": {"code", "msg"}, "id": n}`.
    Error {
        id: Option<u64>,
        code: i64,
        msg: String,
    },
}

impl Answer {
    fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        if object.contains_key("stream") {
            return None;
        }
        let id = object.get("id").and_then(Value::as_u64);
        if let Some(error) = object.get("error") {
            return Some(Self::Error {
                id,
                code: error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                msg: error
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
        if object.contains_key("result") || object.contains_key("id") {
            return Some(Self::Result {
                id,
                result: object.get("result").cloned().unwrap_or(Value::Null),
            });
        }
        None
    }

    fn id(&self) -> Option<u64> {
        match self {
            Self::Result { id, .. } | Self::Error { id, .. } => *id,
        }
    }
}

impl UsdmWs {
    /// Connect to `path` on the production host and subscribe.
    pub async fn connect(
        path: StreamPath,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, UsdmWsError> {
        Self::connect_to(USDM_WS_BASE, path, streams).await
    }

    /// Connect to `path` on another host, such as a local test server, and
    /// subscribe.
    pub async fn connect_to(
        base_url: &str,
        path: StreamPath,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, UsdmWsError> {
        let streams: Vec<StreamName> = streams.into_iter().collect();
        let mut ws = Self::open(base_url, path, CONNECT_TIMEOUT, ANSWER_TIMEOUT).await?;
        ws.subscribe(&streams).await?;
        Ok(ws)
    }

    /// Open a connection with no streams.
    pub(crate) async fn open(
        base_url: &str,
        path: StreamPath,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, UsdmWsError> {
        ensure_crypto_provider();
        let (inner, _) = tokio::time::timeout(connect_timeout, connect_async(path.url(base_url)))
            .await
            .map_err(|_| UsdmWsError::ConnectTimeout(connect_timeout))??;
        Ok(Self {
            inner,
            path,
            streams: Vec::new(),
            next_id: 1,
            buffered: VecDeque::new(),
            last_request: None,
            last_inbound: Instant::now(),
            answer_timeout,
            close_frame: None,
        })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with [`UsdmWsError::WrongPath`] or [`UsdmWsError::TooManyStreams`]
    /// before sending anything, and with [`UsdmWsError::Refused`] when the
    /// server answers with an error; batches answered before it stay
    /// subscribed, and [`streams`](Self::streams) says which.
    pub async fn subscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        if let Some(stream) = streams.iter().find(|s| s.path() != self.path) {
            return Err(UsdmWsError::WrongPath {
                stream: stream.clone(),
                path: self.path,
            });
        }
        let mut new: Vec<StreamName> = Vec::new();
        for stream in streams {
            if !self.streams.contains(stream) && !new.contains(stream) {
                new.push(stream.clone());
            }
        }
        if self.streams.len() + new.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(UsdmWsError::TooManyStreams {
                path: self.path,
                limit: MAX_STREAMS_PER_CONNECTION,
            });
        }
        for batch in new.chunks(MAX_NAMES_PER_REQUEST) {
            self.request("SUBSCRIBE", Some(batch)).await?;
            self.streams.extend(batch.iter().cloned());
        }
        Ok(())
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub async fn unsubscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        let mut gone: Vec<StreamName> = Vec::new();
        for stream in streams {
            if self.streams.contains(stream) && !gone.contains(stream) {
                gone.push(stream.clone());
            }
        }
        for batch in gone.chunks(MAX_NAMES_PER_REQUEST) {
            self.request("UNSUBSCRIBE", Some(batch)).await?;
            self.streams.retain(|s| !batch.contains(s));
        }
        Ok(())
    }

    /// The stream names the server says this connection carries.
    pub async fn list_subscriptions(&mut self) -> Result<Vec<String>, UsdmWsError> {
        let id = self.next_id;
        let result = self.request("LIST_SUBSCRIPTIONS", None).await?;
        serde_json::from_value(result.clone()).map_err(|_| UsdmWsError::Response {
            id,
            raw: result.to_string(),
        })
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub async fn ping(&mut self) -> Result<Duration, UsdmWsError> {
        let start = Instant::now();
        self.send_ping().await?;
        tokio::time::timeout(self.answer_timeout, self.await_pong())
            .await
            .map_err(|_| UsdmWsError::NoAnswer {
                id: 0,
                timeout: self.answer_timeout,
            })??;
        Ok(start.elapsed())
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), UsdmWsError> {
        self.inner.close(None).await?;
        Ok(())
    }

    /// The streams this connection carries.
    pub fn streams(&self) -> &[StreamName] {
        &self.streams
    }

    /// The path this connection is on.
    pub fn path(&self) -> StreamPath {
        self.path
    }

    /// Send a protocol ping without waiting; the pong arrives as any inbound
    /// frame does and refreshes [`last_inbound`](Self::last_inbound).
    pub(crate) async fn send_ping(&mut self) -> Result<(), UsdmWsError> {
        self.inner.send(Message::Ping(Vec::new().into())).await?;
        Ok(())
    }

    /// When the last frame of any kind arrived, pings and pongs included.
    pub(crate) fn last_inbound(&self) -> Instant {
        self.last_inbound
    }

    /// The server's close code and reason, once it has closed the connection.
    pub(crate) fn close_frame(&self) -> Option<(Option<u16>, String)> {
        self.close_frame.clone()
    }

    fn record_close(&mut self, frame: Option<CloseFrame>) {
        self.close_frame = Some(match frame {
            Some(frame) => (Some(u16::from(frame.code)), frame.reason.to_string()),
            None => (None, String::new()),
        });
    }

    fn closed_error(&self) -> UsdmWsError {
        let (code, reason) = self.close_frame.clone().unwrap_or((None, String::new()));
        UsdmWsError::Closed { code, reason }
    }

    /// Send one request, paced, and wait for its answer's `result`.
    async fn request(
        &mut self,
        method: &str,
        params: Option<&[StreamName]>,
    ) -> Result<Value, UsdmWsError> {
        if let Some(last) = self.last_request {
            tokio::time::sleep_until(last + MIN_REQUEST_INTERVAL).await;
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut body = json!({ "method": method, "id": id });
        if let Some(params) = params {
            let names: Vec<String> = params.iter().map(ToString::to_string).collect();
            body["params"] = json!(names);
        }
        self.inner
            .send(Message::Text(body.to_string().into()))
            .await?;
        self.last_request = Some(Instant::now());
        tokio::time::timeout(self.answer_timeout, self.await_answer(id))
            .await
            .map_err(|_| UsdmWsError::NoAnswer {
                id,
                timeout: self.answer_timeout,
            })?
    }

    /// Read until the answer to `id`, keeping updates read meanwhile.
    async fn await_answer(&mut self, id: u64) -> Result<Value, UsdmWsError> {
        loop {
            let message = match self.inner.next().await {
                None => return Err(self.closed_error()),
                Some(Err(err)) => return Err(err.into()),
                Some(Ok(message)) => message,
            };
            self.last_inbound = Instant::now();
            match message {
                Message::Text(text) => match Answer::parse(&text) {
                    Some(answer) if answer.id() == Some(id) => {
                        return match answer {
                            Answer::Result { result, .. } => Ok(result),
                            Answer::Error { code, msg, .. } => {
                                Err(UsdmWsError::Refused { code, msg })
                            }
                        };
                    }
                    Some(other) => {
                        tracing::debug!(id = ?other.id(), "uncorrelated answer while awaiting {id}");
                    }
                    None => self.buffered.push_back(Update::from_json(&text)),
                },
                Message::Close(frame) => {
                    self.record_close(frame);
                    return Err(self.closed_error());
                }
                _ => {}
            }
        }
    }

    /// Read until a pong, keeping updates read meanwhile.
    async fn await_pong(&mut self) -> Result<(), UsdmWsError> {
        loop {
            let message = match self.inner.next().await {
                None => return Err(self.closed_error()),
                Some(Err(err)) => return Err(err.into()),
                Some(Ok(message)) => message,
            };
            self.last_inbound = Instant::now();
            match message {
                Message::Pong(_) => return Ok(()),
                Message::Text(text) if Answer::parse(&text).is_none() => {
                    self.buffered.push_back(Update::from_json(&text));
                }
                Message::Close(frame) => {
                    self.record_close(frame);
                    return Err(self.closed_error());
                }
                _ => {}
            }
        }
    }
}

impl Stream for UsdmWs {
    type Item = Result<Update, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(kept) = self.buffered.pop_front() {
            return Poll::Ready(Some(kept));
        }
        loop {
            let message = match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            };
            self.last_inbound = Instant::now();
            match message {
                Message::Text(text) => match Update::from_json(&text) {
                    Ok(update) => return Poll::Ready(Some(Ok(update))),
                    // An answer nobody is waiting for, such as one to a
                    // request whose caller gave up.
                    Err(_) if Answer::parse(&text).is_some() => continue,
                    Err(err) => return Poll::Ready(Some(Err(err))),
                },
                Message::Close(frame) => {
                    tracing::debug!(?frame, "binance stream closed by the server");
                    self.record_close(frame);
                    return Poll::Ready(None);
                }
                // Pings are answered by tungstenite while it reads; pongs only
                // refresh `last_inbound`.
                _ => continue,
            }
        }
    }
}
```

Notes for the reviewer:
- `last_inbound` is refreshed by every frame, pings and pongs included, because the supervised tier's staleness rule counts them all. Counting only data would drop a healthy quiet connection.
- The pacer sleeps until 200 ms after the previous request before sending the next one, so a replay of 1024 streams in six requests takes about a second.
- An answer that arrives for a request whose caller gave up is skipped by the stream. An answer to an awaited request is matched by `id`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --lib usdm::ws`
Expected: PASS, 22 tests, in about a second.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/src/usdm/ws
git commit -m "feat(binance): UsdmWs, the bare tier, and a scripted Binance server

One connection on one path. A stream for the other path or past 1024 is
refused before anything is sent; requests carry at most 200 names, go
out no faster than one per 200 ms, and each waits for the answer with
its id. The scripted server answers SUBSCRIBE, UNSUBSCRIBE and
LIST_SUBSCRIPTIONS as Binance does, and can close with a code, go
silent, ping, refuse or drop a handshake.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 4: The supervised tier

**Files:**
- Create: `polyoxide-binance/src/usdm/ws/supervised.rs`, `polyoxide-binance/tests/supervision.rs`
- Modify: `polyoxide-binance/src/usdm/ws/mod.rs`, `polyoxide-binance/Cargo.toml`

- [ ] **Step 1: Write the supervision tests**

Each test names the bug it exists to catch. `polyoxide-binance/tests/supervision.rs`:

```rust
//! The supervised tier against the scripted server, with limits in hundreds of
//! milliseconds. Each test names the bug it exists to catch.

use std::time::Duration;

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::Symbol,
    ws::{
        fixtures,
        test_server::{Script, ScriptedServer},
        DepthLevels, DepthSpeed, DisconnectReason, Event, StreamName, StreamPath, SupervisedUsdmWs,
        UsdmWsBuilder, UsdmWsError, MIN_REQUEST_INTERVAL,
    },
};

fn symbol(s: &str) -> Symbol {
    Symbol::new(s).unwrap()
}

fn agg(s: &str) -> StreamName {
    StreamName::AggTrade(symbol(s))
}

fn btc_mark() -> StreamName {
    StreamName::MarkPrice(symbol("BTCUSDT"))
}

fn fast(server: &ScriptedServer) -> UsdmWsBuilder {
    UsdmWsBuilder::new()
        .base_url(&server.url)
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}

async fn next(ws: &mut SupervisedUsdmWs) -> Result<Event, UsdmWsError> {
    tokio::time::timeout(Duration::from_secs(3), ws.next())
        .await
        .expect("an event within 3 s")
        .expect("the stream is open")
}

async fn next_event(ws: &mut SupervisedUsdmWs) -> Event {
    next(ws).await.expect("an event, not an error")
}

/// Asserts nothing arrives for `quiet`.
async fn assert_quiet(ws: &mut SupervisedUsdmWs, quiet: Duration) {
    if let Ok(item) = tokio::time::timeout(quiet, ws.next()).await {
        panic!("expected silence, got {item:?}");
    }
}

#[tokio::test]
async fn each_path_gets_its_own_connection() {
    // Bug: one connection for both paths, which drops one path's data silently.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let depth = StreamName::PartialDepth(symbol("BTCUSDT"), DepthLevels::Twenty, DepthSpeed::Ms100);
    let ws = fast(&server)
        .streams([agg("BTCUSDT"), depth.clone()])
        .connect()
        .await
        .unwrap();
    let mut paths = server.paths();
    paths.sort();
    assert_eq!(paths, ["/market/stream", "/public/stream"]);
    for received in server.received() {
        let names = received.request["params"].as_array().unwrap().clone();
        let path = &server.paths()[received.connection];
        let expected = if path == "/market/stream" {
            "btcusdt@aggTrade".to_owned()
        } else {
            depth.to_string()
        };
        assert_eq!(names, [serde_json::Value::from(expected)], "{path}");
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_1000_stream_replay_is_paced_in_batches_of_200() {
    // Bugs: bursting a resubscribe past Binance's message limit, which closes
    // the connection; an oversized request.
    let server = ScriptedServer::start(vec![
        Script {
            drop_after: Some(Duration::from_millis(1500)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let streams: Vec<StreamName> = (0..1000).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
    let mut ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(streams)
        .connect()
        .await
        .unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let received = server.received();
    for connection in 0..2 {
        let requests: Vec<_> = received
            .iter()
            .filter(|r| r.connection == connection)
            .collect();
        let sizes: Vec<usize> = requests
            .iter()
            .map(|r| r.request["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [200; 5], "connection {connection}");
        for pair in requests.windows(2) {
            let gap = pair[1].at - pair[0].at;
            assert!(
                gap >= MIN_REQUEST_INTERVAL - Duration::from_millis(10),
                "connection {connection}: requests {gap:?} apart"
            );
        }
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn the_1025th_stream_is_refused_and_nothing_reaches_the_server() {
    // Bug: sending it, which makes the server close the connection and lose
    // all 1024.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let streams: Vec<StreamName> = (0..1024).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
    let ws = fast(&server)
        .stale_after(Duration::from_secs(2))
        .streams(streams)
        .connect()
        .await
        .unwrap();
    let sent = server.received().len();
    let err = ws
        .membership()
        .subscribe([agg("BTCUSDT")])
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            UsdmWsError::TooManyStreams {
                path: StreamPath::Market,
                limit: 1024
            }
        ),
        "{err:?}"
    );
    assert_eq!(server.received().len(), sent);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_silent_server_is_stale_then_replaced() {
    // Bug: staleness counting only data, or not enforced at all.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            go_silent: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Stale
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn server_pings_alone_keep_a_quiet_connection() {
    // Bug: dropping a healthy connection whose streams are quiet. The client
    // never pings here, so only the server's pings show it is alive.
    let server = ScriptedServer::start(vec![Script {
        ping_every: Some(Duration::from_millis(50)),
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server)
        .ping_interval(Duration::MAX)
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert_quiet(&mut ws, Duration::from_millis(900)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn answered_client_pings_keep_a_quiet_connection() {
    // The same, kept alive by pongs to the client's own pings.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert_quiet(&mut ws, Duration::from_millis(900)).await;
    assert_eq!(server.connection_count(), 1);
    assert!(server.ping_count() >= 10, "pings: {}", server.ping_count());
    ws.close().await.unwrap();
}

#[tokio::test]
async fn pings_keep_going_under_steady_traffic() {
    // Bug: a ping sent only when the connection is quiet, which a busy
    // connection never is.
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![fixtures::MARK_PRICE.into()],
        push_every: Some(Duration::from_millis(10)),
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    let mut updates = 0;
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, ws.next()).await {
        assert!(matches!(event.unwrap(), Event::Update(_)));
        updates += 1;
    }
    assert!(updates >= 20, "traffic stopped: {updates} updates");
    assert!(
        server.ping_count() >= 5,
        "pings under traffic: {}",
        server.ping_count()
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_closed_connection_is_reported_replaced_and_resubscribed() {
    // Bug: missing resubscribe after a reconnect.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let Event::Disconnected { path, reason } = next_event(&mut ws).await else {
        panic!("expected Disconnected");
    };
    assert_eq!(path, StreamPath::Market);
    assert!(
        matches!(&reason, DisconnectReason::Closed { code: Some(1008), reason } if reason == "Too many requests"),
        "{reason:?}"
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let replayed = server
        .received()
        .into_iter()
        .find(|r| r.connection == 1)
        .unwrap();
    assert_eq!(replayed.request["params"][0], "btcusdt@markPrice@1s");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn prader_s_scenario_update_disconnected_reconnected_update() {
    // The consumer's own test, as it will run against this server.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = UsdmWsBuilder::new()
        .base_url(&server.url)
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn failed_reconnects_make_one_outage_not_one_per_attempt() {
    // Bug: a Disconnected per attempt, which a consumer would count as many
    // outages.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            reject_handshake: true,
            ..Default::default()
        },
        Script {
            reject_handshake: true,
            ..Default::default()
        },
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert_eq!(server.connection_count(), 4);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_old_connection_is_rotated_without_backoff() {
    // Bug: running into Binance's 24-hour cutoff unannounced.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .backoff(Duration::from_secs(10), Duration::from_secs(10))
        .max_connection_age(Duration::from_millis(200))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let started = tokio::time::Instant::now();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected {
            reason: DisconnectReason::Rotation,
            ..
        }
    ));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "rotation waited out the backoff: {:?}",
        started.elapsed()
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_error_answer_fails_only_the_call_that_sent_it() {
    // Bug: one refused request tearing down the path.
    let server = ScriptedServer::start(vec![Script {
        refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server)
        .streams([agg("BTCUSDT")])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    let err = membership.subscribe([agg("ETHUSDT")]).await.unwrap_err();
    assert!(
        matches!(err, UsdmWsError::Refused { code: 2, .. }),
        "{err:?}"
    );
    membership.subscribe([agg("SOLUSDT")]).await.unwrap();
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_bad_frame_is_an_error_and_the_next_update_still_arrives() {
    // Bug: one undecodable frame ending the stream.
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![
            r#"{"stream":"btcusdt@markPrice@1s","data":{"e":"markPriceUpdate"}}"#.into(),
            fixtures::MARK_PRICE.into(),
        ],
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(
        next(&mut ws).await,
        Err(UsdmWsError::Frame { .. })
    ));
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_closes_when_its_last_stream_leaves_and_says_nothing() {
    // Bug: a leaked connection; and a path that closes while up must yield
    // neither marker.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let book = StreamName::BookTicker(symbol("BTCUSDT"));
    let mut ws = fast(&server)
        .streams([btc_mark(), book.clone()])
        .connect()
        .await
        .unwrap();
    ws.membership().unsubscribe([book]).await.unwrap();
    server
        .wait_for("the public connection to end", |s| s.ended_count() == 1)
        .await;
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 2);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_path_wanted_again_reopens() {
    let server = ScriptedServer::start(vec![Script {
        pushes: vec![fixtures::BOOK_TICKER.into()],
        ..Default::default()
    }])
    .await;
    let book = StreamName::BookTicker(symbol("BTCUSDT"));
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    ws.membership().subscribe([book]).await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    let mut paths = server.paths();
    paths.sort();
    assert_eq!(paths, ["/market/stream", "/public/stream"]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_outage_ends_with_reconnected_even_when_its_last_stream_leaves() {
    // Bug: a Disconnected with no Reconnected, which leaves a consumer that
    // folds outages into one state stale for good.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            reject_handshake: true,
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(100), Duration::from_millis(100))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    ws.membership().unsubscribe([btc_mark()]).await.unwrap();
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected {
            path: StreamPath::Market
        }
    ));
    let attempts = server.connection_count();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        server.connection_count(),
        attempts,
        "kept reconnecting a path nobody wants"
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_change_during_an_outage_is_answered_at_once_and_replayed() {
    // Bug: a membership call failing, or hanging, while its path is down.
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            reject_handshake: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_millis(300), Duration::from_millis(300))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    let started = tokio::time::Instant::now();
    ws.membership().subscribe([agg("ETHUSDT")]).await.unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Reconnected { .. }
    ));
    let replayed: Vec<String> = server
        .received()
        .into_iter()
        .filter(|r| r.connection == 2)
        .flat_map(|r| r.request["params"].as_array().unwrap().clone())
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(replayed, ["btcusdt@markPrice@1s", "ethusdt@aggTrade"]);
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_refused_replay_is_an_error_and_ends_the_stream() {
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            refuse: vec![("btcusdt@markPrice@1s".into(), 2, "Invalid request".into())],
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server).streams([btc_mark()]).connect().await.unwrap();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    assert!(matches!(
        next(&mut ws).await,
        Err(UsdmWsError::Refused { code: 2, .. })
    ));
    let end = tokio::time::timeout(Duration::from_secs(2), ws.next())
        .await
        .expect("the end");
    assert!(end.is_none(), "{end:?}");
}

#[tokio::test]
async fn dropping_the_stream_closes_every_socket() {
    // Bug: a leaked task holding connections open.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .streams([btc_mark(), StreamName::BookTicker(symbol("BTCUSDT"))])
        .connect()
        .await
        .unwrap();
    drop(ws);
    server
        .wait_for("both connections to end", |s| s.ended_count() == 2)
        .await;
}

#[tokio::test]
async fn a_refused_first_connect_is_an_error_from_connect() {
    // Bug: retrying silently when the setup is wrong.
    let server = ScriptedServer::start(vec![Script {
        reject_handshake: true,
        ..Default::default()
    }])
    .await;
    let err = fast(&server)
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap_err();
    assert!(matches!(err, UsdmWsError::Connect(_)), "{err:?}");
}

#[tokio::test]
async fn close_ends_the_stream_and_the_handle_promptly_even_mid_backoff() {
    let server = ScriptedServer::start(vec![
        Script {
            pushes: vec![fixtures::MARK_PRICE.into()],
            close_after: true,
            ..Default::default()
        },
        Script {
            reject_handshake: true,
            ..Default::default()
        },
    ])
    .await;
    let mut ws = fast(&server)
        .backoff(Duration::from_secs(5), Duration::from_secs(5))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Update(_)));
    assert!(matches!(
        next_event(&mut ws).await,
        Event::Disconnected { .. }
    ));
    let started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(2), ws.close())
        .await
        .expect("close returns")
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(
        membership.subscribe([agg("ETHUSDT")]).await,
        Err(UsdmWsError::Stopped)
    ));
}
```

Append to `polyoxide-binance/Cargo.toml`, after a blank line:

```toml
[[test]]
name = "supervision"
required-features = ["test-server"]
```

- [ ] **Step 2: Register the module**

In `polyoxide-binance/src/usdm/ws/mod.rs`, replace

```rust
pub mod stream;
#[cfg(any(test, feature = "test-server"))]
```

with

```rust
pub mod stream;
pub mod supervised;
#[cfg(any(test, feature = "test-server"))]
```

and replace

```rust
pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};
```

with

```rust
pub use stream::{DepthLevels, DepthSpeed, InvalidStreamName, StreamName};
pub use supervised::{DisconnectReason, Event, MembershipHandle, SupervisedUsdmWs, UsdmWsBuilder};
```

Create `polyoxide-binance/src/usdm/ws/supervised.rs` with its test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_cadences() {
        let b = UsdmWsBuilder::new();
        assert_eq!(b.base_url, USDM_WS_BASE);
        assert_eq!(b.ping_interval, Duration::from_secs(20));
        assert_eq!(b.stale_after, Duration::from_secs(30));
        assert_eq!(b.initial_backoff, Duration::from_millis(500));
        assert_eq!(b.max_backoff, Duration::from_secs(60));
        assert_eq!(b.connect_timeout, Duration::from_secs(10));
        assert_eq!(b.max_connection_age, Duration::from_secs(85_800));
        assert!(b.max_connection_age < Duration::from_secs(24 * 3600));
    }

    #[test]
    fn backoff_doubles_to_the_ceiling_and_resets_after_a_working_connection() {
        let mut b = Backoff::new(Duration::from_millis(100), Duration::from_millis(350));
        assert_eq!(b.take(), Duration::from_millis(100));
        assert_eq!(b.take(), Duration::from_millis(200));
        assert_eq!(b.take(), Duration::from_millis(350));
        b.after_connection_ended(true);
        assert_eq!(b.take(), Duration::from_millis(100));
        b.after_connection_ended(false);
        assert_eq!(b.take(), Duration::from_millis(200));
    }

    #[test]
    fn a_zero_initial_delay_still_grows_and_a_huge_one_saturates() {
        let mut b = Backoff::new(Duration::ZERO, Duration::from_secs(1));
        assert_eq!(b.take(), Duration::from_millis(1));
        assert_eq!(b.take(), Duration::from_millis(2));
        let mut b = Backoff::new(Duration::MAX, Duration::MAX);
        assert_eq!(b.take(), Duration::MAX);
        assert_eq!(b.take(), Duration::MAX);
    }
}
```

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --test supervision`
Expected: FAIL to compile: ``unresolved imports `polyoxide_binance::usdm::ws::DisconnectReason` `` and the like.

- [ ] **Step 3: Write the supervised tier**

Put this above the test module in `supervised.rs`:

```rust
//! The supervised tier: one connection per path, each on its own task, with
//! keep-alive on the wall clock, staleness detection, reconnect with a paced
//! replay of the path's streams, rotation before Binance's 24-hour cutoff,
//! outage markers, and live membership changes.

use std::{
    collections::HashMap,
    fmt,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

use crate::usdm::ws::{
    client::UsdmWs,
    error::{Recovery, UsdmWsError},
    event::Update,
    stream::StreamName,
    StreamPath, MAX_STREAMS_PER_CONNECTION, USDM_WS_BASE,
};

/// Default keep-alive cadence. The server pings only about every 180 s, so a
/// connection carrying quiet streams would otherwise go minutes without an
/// inbound frame.
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(20);
/// Default silence before a connection is presumed dead. Pongs and the
/// server's pings count as inbound frames.
const DEFAULT_STALE_AFTER: Duration = Duration::from_secs(30);
const DEFAULT_INITIAL_BACKOFF: Duration = Duration::from_millis(500);
const DEFAULT_MAX_BACKOFF: Duration = Duration::from_secs(60);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Ten minutes under Binance's documented 24-hour connection lifetime.
const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(23 * 3600 + 50 * 60);
/// Events the consumer may leave unread before the path tasks block.
const EVENT_BUFFER: usize = 1024;
/// The shortest backoff. Zero would never grow, so a dead server would be
/// retried on every timer tick.
const MIN_BACKOFF: Duration = Duration::from_millis(1);

/// The reconnect delay schedule: doubling to a ceiling, reset after a
/// connection that delivered at least one update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Backoff {
    initial: Duration,
    max: Duration,
    next: Duration,
}

impl Backoff {
    fn new(initial: Duration, max: Duration) -> Self {
        let max = max.max(MIN_BACKOFF);
        let initial = initial.clamp(MIN_BACKOFF, max);
        Self {
            initial,
            max,
            next: initial,
        }
    }

    fn take(&mut self) -> Duration {
        let delay = self.next;
        self.next = self.next.saturating_mul(2).min(self.max);
        delay
    }

    fn after_connection_ended(&mut self, delivered: bool) {
        if delivered {
            self.next = self.initial;
        }
    }
}

/// What the supervised stream yields.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// One frame of one stream, boxed, as the other variants are small.
    Update(Box<Update>),
    /// A path's connection was lost. Every update on that path is stale until
    /// the matching [`Event::Reconnected`], which always follows while the
    /// client runs.
    Disconnected {
        /// The path that went down.
        path: StreamPath,
        /// Why.
        reason: DisconnectReason,
    },
    /// The path is current again: a fresh connection has replayed its
    /// streams, or the path's last stream left during the outage, so nothing
    /// on it is stale. Sent once per outage, however many attempts it took.
    /// Drop book state for the path and resync.
    Reconnected {
        /// The path that came back.
        path: StreamPath,
    },
}

/// Why a path's connection went down.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed it, with the code and reason it sent.
    Closed {
        /// The close code.
        code: Option<u16>,
        /// The close reason.
        reason: String,
    },
    /// A transport error, a request left unanswered, or a failed first
    /// connect of a path opened by a membership change.
    Error(UsdmWsError),
    /// Nothing arrived, pongs included, for the staleness window.
    Stale,
    /// Replaced on purpose before Binance's 24-hour cutoff.
    Rotation,
}

impl fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed { code, reason } => match code {
                Some(code) => write!(f, "closed by the server ({code} {reason})"),
                None => f.write_str("closed by the server"),
            },
            Self::Error(err) => write!(f, "{err}"),
            Self::Stale => f.write_str("no frame within the staleness window"),
            Self::Rotation => f.write_str("rotated before the 24-hour cutoff"),
        }
    }
}

/// Builder for a supervised connection.
#[derive(Debug, Clone)]
pub struct UsdmWsBuilder {
    base_url: String,
    ping_interval: Duration,
    stale_after: Duration,
    initial_backoff: Duration,
    max_backoff: Duration,
    connect_timeout: Duration,
    max_connection_age: Duration,
    streams: Vec<StreamName>,
}

impl Default for UsdmWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdmWsBuilder {
    /// Defaults: the production host, a ping every 20 s, stale after 30 s of
    /// silence, reconnect backoff from 500 ms doubling to 60 s, a 10 s connect
    /// timeout, rotation after 23 h 50 min, and no streams.
    pub fn new() -> Self {
        Self {
            base_url: USDM_WS_BASE.to_owned(),
            ping_interval: DEFAULT_PING_INTERVAL,
            stale_after: DEFAULT_STALE_AFTER,
            initial_backoff: DEFAULT_INITIAL_BACKOFF,
            max_backoff: DEFAULT_MAX_BACKOFF,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            max_connection_age: DEFAULT_MAX_CONNECTION_AGE,
            streams: Vec::new(),
        }
    }

    /// Connect to another host, such as a local test server.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// How often each connection sends a protocol ping, whatever its traffic.
    pub fn ping_interval(mut self, interval: Duration) -> Self {
        self.ping_interval = interval;
        self
    }

    /// Silence after which a connection is presumed dead and replaced. Pongs
    /// and the server's pings count, so a quiet connection that answers stays
    /// up. Also bounds each request's wait for its answer.
    pub fn stale_after(mut self, stale_after: Duration) -> Self {
        self.stale_after = stale_after;
        self
    }

    /// Reconnect delay schedule: `initial`, doubling to `max`, with no cap on
    /// attempts. Both are raised to at least 1 ms, and an `initial` above
    /// `max` is lowered to `max`.
    pub fn backoff(mut self, initial: Duration, max: Duration) -> Self {
        self.initial_backoff = initial;
        self.max_backoff = max;
        self
    }

    /// How long one connection attempt may take to open.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// How long a connection may live before it is replaced, without backoff,
    /// ahead of Binance's 24-hour cutoff.
    pub fn max_connection_age(mut self, age: Duration) -> Self {
        self.max_connection_age = age;
        self
    }

    /// The streams to open with.
    pub fn streams(mut self, streams: impl IntoIterator<Item = StreamName>) -> Self {
        self.streams = streams.into_iter().collect();
        self
    }

    /// Open a connection for each path the streams need, subscribe, and start
    /// supervising.
    ///
    /// Fails if any first connect fails, closing the others: a setup that is
    /// wrong is reported, not retried. With no streams it opens nothing until
    /// the first [`MembershipHandle::subscribe`].
    pub async fn connect(self) -> Result<SupervisedUsdmWs, UsdmWsError> {
        let mut membership: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
        for stream in self.streams.iter().cloned() {
            let wanted = membership.entry(stream.path()).or_default();
            if !wanted.contains(&stream) {
                wanted.push(stream);
            }
        }
        for (path, wanted) in &membership {
            if wanted.len() > MAX_STREAMS_PER_CONNECTION {
                return Err(UsdmWsError::TooManyStreams {
                    path: *path,
                    limit: MAX_STREAMS_PER_CONNECTION,
                });
            }
        }
        let mut opened: Vec<(StreamPath, UsdmWs)> = Vec::new();
        for path in StreamPath::ALL {
            let Some(wanted) = membership.get(path) else {
                continue;
            };
            match self.open(*path, wanted).await {
                Ok(ws) => opened.push((*path, ws)),
                Err(err) => {
                    for (_, mut ws) in opened {
                        let _ = ws.close().await;
                    }
                    return Err(err);
                }
            }
        }
        let (events_tx, events_rx) = mpsc::channel(EVENT_BUFFER);
        let (commands_tx, commands_rx) = mpsc::channel(16);
        let task = tokio::spawn(supervise(self, opened, membership, commands_rx, events_tx));
        Ok(SupervisedUsdmWs {
            events: events_rx,
            commands: commands_tx,
            task,
        })
    }

    async fn open(&self, path: StreamPath, streams: &[StreamName]) -> Result<UsdmWs, UsdmWsError> {
        let mut ws =
            UsdmWs::open(&self.base_url, path, self.connect_timeout, self.stale_after).await?;
        ws.subscribe(streams).await?;
        Ok(ws)
    }
}

type Reply = oneshot::Sender<Result<(), UsdmWsError>>;

enum Command {
    Subscribe(Vec<StreamName>, Reply),
    Unsubscribe(Vec<StreamName>, Reply),
    /// Close every socket and end. Sent by [`SupervisedUsdmWs::close`]; a
    /// dropped sender is not enough, because every [`MembershipHandle`] holds
    /// a clone.
    Close,
}

enum PathCommand {
    Subscribe(Vec<StreamName>, Reply),
    Unsubscribe(Vec<StreamName>, Reply),
    Close,
}

/// Changes the streams of a running [`SupervisedUsdmWs`].
///
/// Each stream is routed to its path's connection, opening it if this is the
/// path's first stream and closing it when its last leaves. The membership is
/// a set: subscribing a stream twice holds it once, and counting references is
/// the caller's job.
///
/// During a path's outage a change is recorded and answered `Ok` at once, and
/// the replay applies it. A call fails only when the server refuses it
/// ([`UsdmWsError::Refused`]), when it would pass 1024 streams on a path
/// ([`UsdmWsError::TooManyStreams`], nothing sent), or when the client has
/// stopped ([`UsdmWsError::Stopped`]).
///
/// A call waits for the path task, which cannot take it while parked behind a
/// full event buffer (1024 unread events). Make membership changes from a task
/// other than the one draining the stream, or keep draining while they run.
#[derive(Debug, Clone)]
pub struct MembershipHandle {
    commands: mpsc::Sender<Command>,
}

impl MembershipHandle {
    /// Subscribe to more streams.
    pub async fn subscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.send(|reply| Command::Subscribe(streams.into_iter().collect(), reply))
            .await
    }

    /// Unsubscribe from streams. A stream not subscribed is ignored.
    pub async fn unsubscribe(
        &self,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<(), UsdmWsError> {
        self.send(|reply| Command::Unsubscribe(streams.into_iter().collect(), reply))
            .await
    }

    async fn send(&self, make: impl FnOnce(Reply) -> Command) -> Result<(), UsdmWsError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.commands
            .send(make(reply_tx))
            .await
            .map_err(|_| UsdmWsError::Stopped)?;
        reply_rx.await.map_err(|_| UsdmWsError::Stopped)?
    }
}

/// A supervised set of connections: a `Stream` of [`Event`]s that survives
/// drops, stalls and Binance's 24-hour cutoff.
///
/// Every [`Event::Disconnected`] is followed by an [`Event::Reconnected`] for
/// the same path while the client runs. A path that closes because its last
/// stream left, while it is up, yields neither.
///
/// A fatal error, such as the server refusing a replay after a reconnect, is
/// yielded as `Err` and then the stream ends.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{Event, StreamName, UsdmWsBuilder}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut feed = UsdmWsBuilder::new()
///     .streams([StreamName::MarkPrice(btc.clone()), StreamName::BookTicker(btc)])
///     .connect()
///     .await?;
/// let membership = feed.membership();
/// while let Some(event) = feed.next().await {
///     match event? {
///         Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
///         Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
///         Event::Reconnected { path } => eprintln!("{path} back"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([StreamName::AllMarkPrices]).await?;
/// # Ok(())
/// # }
/// ```
pub struct SupervisedUsdmWs {
    events: mpsc::Receiver<Result<Event, UsdmWsError>>,
    commands: mpsc::Sender<Command>,
    task: tokio::task::JoinHandle<Result<(), UsdmWsError>>,
}

impl fmt::Debug for SupervisedUsdmWs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupervisedUsdmWs")
            .field("running", &!self.task.is_finished())
            .finish_non_exhaustive()
    }
}

impl SupervisedUsdmWs {
    /// A handle for changing streams while the stream runs.
    pub fn membership(&self) -> MembershipHandle {
        MembershipHandle {
            commands: self.commands.clone(),
        }
    }

    /// Close every connection and stop. Every [`MembershipHandle`] answers
    /// [`UsdmWsError::Stopped`] afterwards.
    pub async fn close(self) -> Result<(), UsdmWsError> {
        let Self {
            events,
            commands,
            task,
        } = self;
        // Dropping the receiver first unblocks a path task parked behind a
        // full buffer, which would otherwise never see the command.
        drop(events);
        let _ = commands.send(Command::Close).await;
        drop(commands);
        match task.await {
            Ok(result) => result,
            Err(_) => Err(UsdmWsError::Stopped),
        }
    }
}

impl Stream for SupervisedUsdmWs {
    type Item = Result<Event, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.events.poll_recv(cx)
    }
}

type Events = mpsc::Sender<Result<Event, UsdmWsError>>;

struct PathSlot {
    commands: mpsc::Sender<PathCommand>,
    task: tokio::task::JoinHandle<()>,
}

/// How a path task starts.
enum Start {
    /// `connect` opened the connection already.
    Open(Box<UsdmWs>),
    /// A membership change wants the path: connect, and say how the first
    /// attempt went.
    Connect(Reply),
}

/// The supervisor: routes membership changes to path tasks, opens a path for
/// its first stream, and lets it close when its last leaves.
async fn supervise(
    config: UsdmWsBuilder,
    opened: Vec<(StreamPath, UsdmWs)>,
    mut membership: HashMap<StreamPath, Vec<StreamName>>,
    mut commands: mpsc::Receiver<Command>,
    events: Events,
) -> Result<(), UsdmWsError> {
    let (fatal_tx, mut fatal_rx) = mpsc::channel::<()>(StreamPath::ALL.len());
    let mut paths: HashMap<StreamPath, PathSlot> = HashMap::new();
    for (path, ws) in opened {
        let streams = membership.get(&path).cloned().unwrap_or_default();
        let slot = spawn_path(
            &config,
            path,
            Start::Open(Box::new(ws)),
            streams,
            &events,
            &fatal_tx,
        );
        paths.insert(path, slot);
    }

    let result = loop {
        tokio::select! {
            biased;
            _ = fatal_rx.recv() => break Err(UsdmWsError::Stopped),
            () = events.closed() => break Ok(()),
            command = commands.recv() => match command {
                None | Some(Command::Close) => break Ok(()),
                Some(Command::Subscribe(streams, reply)) => {
                    let result = subscribe(&config, &mut paths, &mut membership, streams, &events, &fatal_tx).await;
                    let _ = reply.send(result);
                }
                Some(Command::Unsubscribe(streams, reply)) => {
                    let result = unsubscribe(&mut paths, &mut membership, streams).await;
                    let _ = reply.send(result);
                }
            },
        }
    };
    for (_, slot) in paths.drain() {
        let _ = slot.commands.send(PathCommand::Close).await;
        let _ = slot.task.await;
    }
    result
}

fn spawn_path(
    config: &UsdmWsBuilder,
    path: StreamPath,
    start: Start,
    streams: Vec<StreamName>,
    events: &Events,
    fatal: &mpsc::Sender<()>,
) -> PathSlot {
    let (commands_tx, commands_rx) = mpsc::channel(16);
    let task = tokio::spawn(run_path(
        config.clone(),
        path,
        start,
        streams,
        commands_rx,
        events.clone(),
        fatal.clone(),
    ));
    PathSlot {
        commands: commands_tx,
        task,
    }
}

/// Group `streams` by path, dropping any already wanted and duplicates.
fn new_by_path(
    membership: &HashMap<StreamPath, Vec<StreamName>>,
    streams: Vec<StreamName>,
) -> HashMap<StreamPath, Vec<StreamName>> {
    let mut by_path: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
    for stream in streams {
        let path = stream.path();
        let known = membership.get(&path).is_some_and(|m| m.contains(&stream));
        let batch = by_path.entry(path).or_default();
        if !known && !batch.contains(&stream) {
            batch.push(stream);
        }
    }
    by_path.retain(|_, batch| !batch.is_empty());
    by_path
}

async fn subscribe(
    config: &UsdmWsBuilder,
    paths: &mut HashMap<StreamPath, PathSlot>,
    membership: &mut HashMap<StreamPath, Vec<StreamName>>,
    streams: Vec<StreamName>,
    events: &Events,
    fatal: &mpsc::Sender<()>,
) -> Result<(), UsdmWsError> {
    let by_path = new_by_path(membership, streams);
    // Refuse before sending anything: the server answers the 1025th stream
    // with an error and then closes the connection, losing all 1024.
    for (path, add) in &by_path {
        let held = membership.get(path).map_or(0, Vec::len);
        if held + add.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(UsdmWsError::TooManyStreams {
                path: *path,
                limit: MAX_STREAMS_PER_CONNECTION,
            });
        }
    }
    for path in StreamPath::ALL {
        let Some(add) = by_path.get(path) else {
            continue;
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        match paths.get(path) {
            Some(slot) => {
                if slot
                    .commands
                    .send(PathCommand::Subscribe(add.clone(), reply_tx))
                    .await
                    .is_err()
                {
                    return Err(UsdmWsError::Stopped);
                }
            }
            None => {
                let slot = spawn_path(
                    config,
                    *path,
                    Start::Connect(reply_tx),
                    add.clone(),
                    events,
                    fatal,
                );
                paths.insert(*path, slot);
            }
        }
        match reply_rx.await {
            Ok(Ok(())) => membership
                .entry(*path)
                .or_default()
                .extend(add.iter().cloned()),
            Ok(Err(err)) => {
                if !membership.contains_key(path) {
                    // The path's first connect was refused; its task has ended.
                    paths.remove(path);
                }
                return Err(err);
            }
            Err(_) => return Err(UsdmWsError::Stopped),
        }
    }
    Ok(())
}

async fn unsubscribe(
    paths: &mut HashMap<StreamPath, PathSlot>,
    membership: &mut HashMap<StreamPath, Vec<StreamName>>,
    streams: Vec<StreamName>,
) -> Result<(), UsdmWsError> {
    let mut by_path: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
    for stream in streams {
        let path = stream.path();
        if membership.get(&path).is_some_and(|m| m.contains(&stream)) {
            let batch = by_path.entry(path).or_default();
            if !batch.contains(&stream) {
                batch.push(stream);
            }
        }
    }
    for path in StreamPath::ALL {
        let (Some(remove), Some(slot)) = (by_path.get(path), paths.get(path)) else {
            continue;
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if slot
            .commands
            .send(PathCommand::Unsubscribe(remove.clone(), reply_tx))
            .await
            .is_err()
        {
            return Err(UsdmWsError::Stopped);
        }
        match reply_rx.await {
            Ok(Ok(())) => {
                let left = membership.entry(*path).or_default();
                left.retain(|s| !remove.contains(s));
                if left.is_empty() {
                    // The path task ends by itself once its last stream has
                    // left; it is not awaited, so a full event buffer cannot
                    // hold this call.
                    membership.remove(path);
                    paths.remove(path);
                }
            }
            Ok(Err(err)) => return Err(err),
            Err(_) => return Err(UsdmWsError::Stopped),
        }
    }
    Ok(())
}

/// How a path's outage ended.
enum Recovered {
    /// A fresh connection carries the path's streams.
    Up(Box<UsdmWs>),
    /// The path's last stream left; nothing on it can be stale.
    NotWanted,
    /// The consumer is gone or the client is closing.
    Closed,
    /// A failure retrying cannot fix.
    Fatal(UsdmWsError),
}

/// How one connection's life ended.
enum End {
    /// The consumer is gone, the client is closing, or the last stream left.
    Closed,
    /// It reached `max_connection_age`.
    Rotation,
    /// It was lost.
    Lost(DisconnectReason),
}

/// Send a Close frame, but not wait on a peer that has stopped reading.
async fn close_politely(ws: &mut UsdmWs) {
    let _ = tokio::time::timeout(Duration::from_secs(1), ws.close()).await;
}

async fn emit(events: &Events, event: Event) -> bool {
    events.send(Ok(event)).await.is_ok()
}

/// One path: pump its connection, and when it is lost, report the outage,
/// reconnect with backoff, replay, and report the recovery.
async fn run_path(
    config: UsdmWsBuilder,
    path: StreamPath,
    start: Start,
    mut streams: Vec<StreamName>,
    mut commands: mpsc::Receiver<PathCommand>,
    events: Events,
    fatal: mpsc::Sender<()>,
) {
    let mut backoff = Backoff::new(config.initial_backoff, config.max_backoff);
    // `None` while an outage has been reported and not yet ended.
    let mut socket = match start {
        Start::Open(ws) => Some(ws),
        Start::Connect(reply) => match config.open(path, &streams).await {
            Ok(ws) => {
                let _ = reply.send(Ok(()));
                Some(Box::new(ws))
            }
            Err(err) if err.recovery() == Recovery::Reconnect => {
                // Recorded: the caller's change stands, and the path keeps
                // trying, reporting the outage like any other.
                let _ = reply.send(Ok(()));
                tracing::warn!(%err, %path, "binance stream connect failed, retrying");
                if !emit(
                    &events,
                    Event::Disconnected {
                        path,
                        reason: DisconnectReason::Error(err),
                    },
                )
                .await
                {
                    return;
                }
                None
            }
            Err(err) => {
                let _ = reply.send(Err(err));
                return;
            }
        },
    };
    let mut first_delay = None;

    loop {
        let mut ws = match socket.take() {
            Some(ws) => ws,
            None => {
                let delay = first_delay.take().unwrap_or_else(|| backoff.take());
                match recover(
                    &config,
                    path,
                    &mut streams,
                    &mut commands,
                    &events,
                    &mut backoff,
                    delay,
                )
                .await
                {
                    Recovered::Up(ws) => {
                        if !emit(&events, Event::Reconnected { path }).await {
                            return;
                        }
                        ws
                    }
                    Recovered::NotWanted => {
                        let _ = emit(&events, Event::Reconnected { path }).await;
                        return;
                    }
                    Recovered::Closed => return,
                    Recovered::Fatal(err) => {
                        let _ = events.send(Err(err)).await;
                        let _ = fatal.send(()).await;
                        return;
                    }
                }
            }
        };
        let mut delivered = false;
        let end = pump(
            &config,
            &mut ws,
            &mut streams,
            &mut commands,
            &events,
            &mut delivered,
        )
        .await;
        let reason = match end {
            End::Closed => {
                close_politely(&mut ws).await;
                return;
            }
            End::Rotation => {
                close_politely(&mut ws).await;
                first_delay = Some(Duration::ZERO);
                DisconnectReason::Rotation
            }
            // A lost connection is dropped, not closed: the peer may not be
            // reading, and a Close frame would wait on it.
            End::Lost(reason) => {
                backoff.after_connection_ended(delivered);
                tracing::warn!(%path, %reason, "binance stream lost, reconnecting");
                reason
            }
        };
        if !emit(&events, Event::Disconnected { path, reason }).await {
            return;
        }
    }
}

/// Wait out `delay`, then connect and replay, repeating with backoff while
/// retrying can fix the failure. Membership changes during the wait are
/// recorded and answered at once.
async fn recover(
    config: &UsdmWsBuilder,
    path: StreamPath,
    streams: &mut Vec<StreamName>,
    commands: &mut mpsc::Receiver<PathCommand>,
    events: &Events,
    backoff: &mut Backoff,
    mut delay: Duration,
) -> Recovered {
    loop {
        if streams.is_empty() {
            return Recovered::NotWanted;
        }
        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                biased;
                () = events.closed() => return Recovered::Closed,
                command = commands.recv() => match command {
                    None | Some(PathCommand::Close) => return Recovered::Closed,
                    Some(PathCommand::Subscribe(add, reply)) => {
                        for stream in add {
                            if !streams.contains(&stream) {
                                streams.push(stream);
                            }
                        }
                        let _ = reply.send(Ok(()));
                    }
                    Some(PathCommand::Unsubscribe(remove, reply)) => {
                        streams.retain(|s| !remove.contains(s));
                        let _ = reply.send(Ok(()));
                        if streams.is_empty() {
                            return Recovered::NotWanted;
                        }
                    }
                },
                () = &mut sleep => break,
            }
        }
        tokio::select! {
            biased;
            () = events.closed() => return Recovered::Closed,
            result = config.open(path, streams) => match result {
                Ok(ws) => return Recovered::Up(Box::new(ws)),
                Err(err) if err.recovery() == Recovery::Reconnect => {
                    tracing::warn!(%err, %path, "binance stream reconnect failed, retrying");
                    delay = backoff.take();
                }
                Err(err) => return Recovered::Fatal(err),
            },
        }
    }
}

/// Drive one connection until it is lost, rotated, or no longer wanted.
///
/// The ping is due on the wall clock, traffic or not, and every wait is bounded
/// by whichever of the next ping, the staleness deadline and the rotation
/// comes first.
async fn pump(
    config: &UsdmWsBuilder,
    ws: &mut UsdmWs,
    streams: &mut Vec<StreamName>,
    commands: &mut mpsc::Receiver<PathCommand>,
    events: &Events,
    delivered: &mut bool,
) -> End {
    let opened = Instant::now();
    let mut last_ping = Instant::now();
    loop {
        if opened.elapsed() >= config.max_connection_age {
            return End::Rotation;
        }
        if last_ping.elapsed() >= config.ping_interval {
            if let Err(err) = ws.send_ping().await {
                return End::Lost(DisconnectReason::Error(err));
            }
            last_ping = Instant::now();
        }
        let silent = ws.last_inbound().elapsed();
        if silent >= config.stale_after {
            return End::Lost(DisconnectReason::Stale);
        }
        // A wait, not an `Instant`: a limit such as `Duration::MAX` overflows
        // when added to one.
        let wait = config
            .ping_interval
            .saturating_sub(last_ping.elapsed())
            .min(config.stale_after.saturating_sub(silent))
            .min(config.max_connection_age.saturating_sub(opened.elapsed()));

        tokio::select! {
            biased;
            () = events.closed() => return End::Closed,
            command = commands.recv() => match command {
                None | Some(PathCommand::Close) => return End::Closed,
                Some(PathCommand::Subscribe(add, reply)) => {
                    let result = ws.subscribe(&add).await;
                    *streams = ws.streams().to_vec();
                    match result {
                        Ok(()) => { let _ = reply.send(Ok(())); }
                        Err(err @ (UsdmWsError::Refused { .. } | UsdmWsError::TooManyStreams { .. })) => {
                            let _ = reply.send(Err(err));
                        }
                        Err(err) => {
                            // The connection failed under the request: record
                            // the change, and let the replay apply it.
                            for stream in add {
                                if !streams.contains(&stream) {
                                    streams.push(stream);
                                }
                            }
                            let _ = reply.send(Ok(()));
                            return End::Lost(DisconnectReason::Error(err));
                        }
                    }
                }
                Some(PathCommand::Unsubscribe(remove, reply)) => {
                    if streams.iter().all(|s| remove.contains(s)) {
                        streams.clear();
                        let _ = reply.send(Ok(()));
                        return End::Closed;
                    }
                    let result = ws.unsubscribe(&remove).await;
                    *streams = ws.streams().to_vec();
                    match result {
                        Ok(()) => { let _ = reply.send(Ok(())); }
                        Err(err @ UsdmWsError::Refused { .. }) => { let _ = reply.send(Err(err)); }
                        Err(err) => {
                            streams.retain(|s| !remove.contains(s));
                            let _ = reply.send(Ok(()));
                            return End::Lost(DisconnectReason::Error(err));
                        }
                    }
                }
            },
            next = tokio::time::timeout(wait, ws.next()) => match next {
                // The ping, staleness or rotation is due; the top of the loop
                // sees to it.
                Err(_) => {}
                Ok(None) => {
                    let (code, reason) = ws.close_frame().unwrap_or((None, String::new()));
                    return End::Lost(DisconnectReason::Closed { code, reason });
                }
                Ok(Some(Err(err))) if err.recovery() == Recovery::SkipFrame => {
                    if events.send(Err(err)).await.is_err() {
                        return End::Closed;
                    }
                }
                Ok(Some(Err(err))) => return End::Lost(DisconnectReason::Error(err)),
                Ok(Some(Ok(update))) => {
                    *delivered = true;
                    if !emit(events, Event::Update(Box::new(update))).await {
                        return End::Closed;
                    }
                }
            },
        }
    }
}
```

Notes for the reviewer:
- The supervisor routes each stream to its path's task. It spawns a task for a path's first stream, waiting for that first connect so a refusal fails the call, and drops the task when the path's last stream leaves. It never awaits a path task that is ending, so a full event buffer cannot hold a membership call.
- `pump` pings on the wall clock whatever the traffic, as `polyoxide-perps`'s fixed supervised socket does (memory: copying the rtds shape stops pings under traffic). Every wait is bounded by the next ping, the staleness deadline and the rotation.
- `recover` serves membership changes while it waits, recording them and answering `Ok`, and ends the outage with `Reconnected` when the path's last stream leaves. That keeps every `Disconnected` paired, which prader-rs folds outages on.
- A lost connection is dropped, not closed: the peer may not be reading, and a Close frame would wait on it. A polite close is bounded at one second.

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --test supervision`
Expected: PASS, 21 tests, in under four seconds.

Run: `cargo test -j 4 -p polyoxide-binance --features test-server --lib`
Expected: PASS, 63 tests.

- [ ] **Step 5: Show the tests can fail**

Break each rule, run `cargo test -j 4 -p polyoxide-binance --features test-server --no-fail-fast --lib --test supervision`, see the named test fail, then restore the line:

| Change | Test that must fail |
|---|---|
| `supervised.rs`: `if last_ping.elapsed() >= config.ping_interval {` → `if last_ping.elapsed() >= config.ping_interval && ws.last_inbound().elapsed() >= config.ping_interval {` | `pings_keep_going_under_steady_traffic` |
| `client.rs`, in `poll_next`: `self.last_inbound = Instant::now();` → `if matches!(message, Message::Text(_)) { self.last_inbound = Instant::now(); }` | `server_pings_alone_keep_a_quiet_connection`, `answered_client_pings_keep_a_quiet_connection` |
| `supervised.rs`, in `run_path`'s `Recovered::NotWanted` arm: delete `let _ = emit(&events, Event::Reconnected { path }).await;` | `an_outage_ends_with_reconnected_even_when_its_last_stream_leaves` |
| `client.rs`: `tokio::time::sleep_until(last + MIN_REQUEST_INTERVAL).await;` → `let _ = last;` | `a_1000_stream_replay_is_paced_in_batches_of_200`, `requests_carry_at_most_200_names_and_are_paced` |
| `> MAX_STREAMS_PER_CONNECTION` → `> usize::MAX - 1`, in both `supervised.rs` (`subscribe`) and `client.rs` (`subscribe`) | `the_1025th_stream_is_refused_and_nothing_reaches_the_server`, `the_1025th_stream_is_refused_and_nothing_is_sent` |
| `supervised.rs`: `first_delay = Some(Duration::ZERO);` → `first_delay = None;` | `an_old_connection_is_rotated_without_backoff` |
| `supervised.rs`, in `recover`'s `PathCommand::Subscribe` arm: `let _ = reply.send(Ok(()));` → `drop(reply);` | `a_change_during_an_outage_is_answered_at_once_and_replayed` |

Afterwards `git diff --stat polyoxide-binance/src` must show only new files.

- [ ] **Step 6: Lint and the doc gate**

Run: `cargo clippy -j 4 -p polyoxide-binance --all-targets --all-features -- -D warnings`
Expected: no warnings.

Run: `RUSTDOCFLAGS="-D warnings" cargo doc -j 4 --no-deps --all-features -p polyoxide-binance`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add polyoxide-binance
git commit -m "feat(binance): SupervisedUsdmWs, one supervised connection per path

UsdmWsBuilder opens a connection for each routed path its streams need
and supervises each on its own task: wall-clock pings, staleness that
counts pongs and server pings, reconnect with a paced replay, rotation
at 23 h 50 min, and a Disconnected that is always followed by a
Reconnected, even when the path's last stream leaves mid-outage. A
membership change during an outage is recorded and answered at once.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 5: The live socket suite

**Files:**
- Create: `polyoxide-binance/tests/live_ws.rs`
- Modify: `polyoxide-binance/Cargo.toml`

- [ ] **Step 1: Write the suite**

`live_frames_carry_no_unmodelled_keys` reads raw frames over its own connection, since the client hides them, and is the streams' drift detector. `live_every_stream_kind_delivers_on_its_path` panics with "a quiet market can legitimately time out", which the nightly classifier files as environmental.

```rust
//! Live tests against `fstream.binance.com`, `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --features ws --test live_ws -- --ignored
//! ```
//!
//! `live_frames_carry_no_unmodelled_keys` is the streams' drift detector.

mod common;

use std::{collections::HashSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_binance::usdm::{
    types::{Interval, Symbol},
    ws::{
        DepthLevels, DepthSpeed, Event, Payload, StreamName, StreamPath, Update, UsdmWs,
        UsdmWsBuilder,
    },
};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

fn every_kind() -> Vec<StreamName> {
    vec![
        StreamName::AllTickers,
        StreamName::AllMarkPrices,
        StreamName::AggTrade(btc()),
        StreamName::Kline(btc(), Interval::M1),
        StreamName::MarkPrice(btc()),
        StreamName::Ticker(btc()),
        StreamName::PartialDepth(btc(), DepthLevels::Twenty, DepthSpeed::Ms100),
        StreamName::BookTicker(btc()),
    ]
}

fn kind(payload: &Payload) -> &'static str {
    match payload {
        Payload::Tickers(_) => "tickers",
        Payload::MarkPrices(_) => "mark prices",
        Payload::AggTrade(_) => "aggTrade",
        Payload::Kline(_) => "kline",
        Payload::MarkPrice(_) => "mark price",
        Payload::Ticker(_) => "ticker",
        Payload::PartialDepth(_) => "partial depth",
        Payload::BookTicker(_) => "book ticker",
        _ => "unknown",
    }
}

#[tokio::test]
#[ignore]
async fn live_every_stream_kind_delivers_on_its_path() {
    let mut feed = UsdmWsBuilder::new()
        .streams(every_kind())
        .connect()
        .await
        .expect("connect");
    let mut seen = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while seen.len() < 8 {
        let event = tokio::time::timeout_at(deadline, feed.next())
            .await
            .unwrap_or_else(|_| {
                panic!("saw only {seen:?} in 60 s; a quiet market can legitimately time out")
            })
            .expect("the stream is open")
            .expect("not an error");
        if let Event::Update(update) = event {
            assert!(
                !matches!(update.payload, Payload::Unknown { .. }),
                "{update:?}"
            );
            seen.insert(kind(&update.payload));
        }
    }
    feed.close().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn live_frames_carry_no_unmodelled_keys() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut unmodelled = Vec::new();
    for path in StreamPath::ALL {
        let names: Vec<String> = every_kind()
            .into_iter()
            .filter(|s| s.path() == *path)
            .map(|s| s.to_string())
            .collect();
        let url = format!(
            "wss://fstream.binance.com/{}/stream?streams={}",
            path.as_str(),
            names.join("/")
        );
        let (mut socket, _) = connect_async(url).await.expect("connect");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while let Ok(Some(message)) = tokio::time::timeout_at(deadline, socket.next()).await {
            let Message::Text(text) = message.expect("frame") else {
                continue;
            };
            let update = Update::from_json(&text).unwrap_or_else(|e| panic!("{e}"));
            assert!(!matches!(update.payload, Payload::Unknown { .. }), "{text}");
            let wire: Value = serde_json::from_str(&text).unwrap();
            let diff = common::compare_values(
                &update.stream.to_string(),
                &wire,
                &serde_json::to_value(&update).unwrap(),
            );
            for key in diff.unmodelled {
                // Binance documents the kline's `B` as "Ignore".
                if key != "/data/k/B" {
                    unmodelled.push(format!("{}: {key}", update.stream));
                }
            }
        }
    }
    unmodelled.sort();
    unmodelled.dedup();
    assert!(
        unmodelled.is_empty(),
        "the host sent keys the stream types do not model; add them and record them in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}

#[tokio::test]
#[ignore]
async fn live_a_quiet_supervised_connection_stays_up() {
    // A listed symbol with no trades: nothing arrives but pings and pongs.
    let quiet = StreamName::AggTrade(Symbol::new("ZZ0000USDT").unwrap());
    let mut feed = UsdmWsBuilder::new()
        .ping_interval(Duration::from_secs(2))
        .stale_after(Duration::from_secs(5))
        .streams([quiet])
        .connect()
        .await
        .expect("connect");
    let held = tokio::time::timeout(Duration::from_secs(12), feed.next()).await;
    assert!(held.is_err(), "expected silence, got {held:?}");
    feed.close().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn live_a_client_ping_is_answered() {
    let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::MarkPrice(btc())])
        .await
        .expect("connect");
    let rtt = ws.ping().await.expect("pong");
    assert!(rtt < Duration::from_secs(5), "{rtt:?}");
    assert_eq!(
        ws.list_subscriptions().await.expect("list"),
        ["btcusdt@markPrice@1s"]
    );
    ws.close().await.unwrap();
}
```

Append to `polyoxide-binance/Cargo.toml`, after a blank line:

```toml
[[test]]
name = "live_ws"
required-features = ["ws"]
```

- [ ] **Step 2: Run it**

Run: `cargo test -j 4 -p polyoxide-binance --features ws --test live_ws -- --ignored`
Expected: PASS, 4 tests, in about 25 seconds.

- [ ] **Step 3: Commit**

```bash
cargo fmt --all
git add polyoxide-binance/tests/live_ws.rs polyoxide-binance/Cargo.toml
git commit -m "test(binance): live socket suite and the streams' drift detector

Every stream kind delivers on its path; ten seconds of raw frames from
both paths carry no key the payload types do not model; a quiet
supervised connection stays up on pings alone; a client ping is
answered.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 6: Capture the stream fixtures

The stream fixtures so far are the handover's. The script gains a stream capture that picks array rows on purpose: a USDⓈ-M row with a scheduled funding time, and a COIN-M row when the frame carries one.

**Files:**
- Replace: `scripts/capture_binance_fixtures.py`
- Replace: `polyoxide-binance/tests/fixtures/{rest,ws}/*.json`, `polyoxide-binance/tests/fixtures/PROVENANCE.md`

- [ ] **Step 1: Replace the script**

```python
#!/usr/bin/env python3
"""Capture Binance USDⓈ-M REST responses and stream frames as test fixtures for
polyoxide-binance.

Usage: python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures

Writes one JSON file per route under OUT_DIR/rest/, one combined-stream envelope per
stream kind under OUT_DIR/ws/, and rewrites OUT_DIR/PROVENANCE.md. Every key is kept;
only list lengths are trimmed, so a wire-agreement test sees each field the server
sends. Stdlib only, no credentials. Costs about 60 request weight, well inside the
2400 per minute, and two short WebSocket connections.
"""
import base64
import datetime
import json
import os
import socket
import ssl
import struct
import sys
import time
import urllib.parse
import urllib.request

BASE = "https://fapi.binance.com"
WS_HOST = "fstream.binance.com"
STREAMS = {
    "market": ["!ticker@arr", "!markPrice@arr@1s", "btcusdt@aggTrade", "btcusdt@kline_1m",
               "btcusdt@markPrice@1s", "btcusdt@ticker"],
    "public": ["btcusdt@depth20@100ms", "btcusdt@bookTicker"],
}


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


def ws_open(path):
    raw = socket.create_connection((WS_HOST, 443), timeout=15)
    sock = ssl.create_default_context().wrap_socket(raw, server_hostname=WS_HOST)
    key = base64.b64encode(os.urandom(16)).decode()
    sock.sendall((f"GET {path} HTTP/1.1\r\nHost: {WS_HOST}\r\nUpgrade: websocket\r\n"
                  f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
                  f"Sec-WebSocket-Version: 13\r\n\r\n").encode())
    head = b""
    while b"\r\n\r\n" not in head:
        head += sock.recv(1)
    status = head.split(b"\r\n")[0]
    if b" 101 " not in status:
        sys.exit(f"{path}: handshake refused: {status.decode()}")
    return sock


def read_exact(sock, n):
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise EOFError("the server closed the connection")
        buf += chunk
    return buf


def read_message(sock):
    """One message as (opcode, payload), joining continuation frames."""
    opcode, payload = None, b""
    while True:
        b1, b2 = read_exact(sock, 2)
        length = b2 & 0x7F
        if length == 126:
            length = struct.unpack(">H", read_exact(sock, 2))[0]
        elif length == 127:
            length = struct.unpack(">Q", read_exact(sock, 8))[0]
        if b2 & 0x80:
            read_exact(sock, 4)
        data = read_exact(sock, length)
        if b1 & 0x0F:
            opcode = b1 & 0x0F
        payload += data
        if b1 & 0x80:
            return opcode, payload


def pick_two(rows):
    """A USDⓈ-M row (`st: 1`) with a funding time when the payload has one, and a
    COIN-M row (`st: 2`) when the frame carries one, else another row."""
    first_row = next((r for r in rows if r.get("st") == 1 and r.get("T", 1) > 0), rows[0])
    second = next((r for r in rows if r.get("st") == 2), None)
    if second is None:
        second = next(r for r in rows if r is not first_row)
    return [first_row, second]


def capture_streams(out):
    ws_dir = os.path.join(out, "ws")
    os.makedirs(ws_dir, exist_ok=True)
    for path, streams in STREAMS.items():
        sock = ws_open(f"/{path}/stream?streams=" + "/".join(streams))
        sock.settimeout(15)
        wanted = set(streams)
        deadline = time.time() + 30
        while wanted and time.time() < deadline:
            opcode, data = read_message(sock)
            if opcode != 1:
                continue
            envelope = json.loads(data.decode("utf-8"))
            name = envelope.get("stream")
            if name not in wanted:
                continue
            wanted.discard(name)
            if isinstance(envelope["data"], list):
                envelope["data"] = pick_two(envelope["data"])
            elif "@depth" in name:
                envelope["data"]["b"] = envelope["data"]["b"][:3]
                envelope["data"]["a"] = envelope["data"]["a"][:3]
            file = "stream_" + name.replace("!", "all_").replace("@", "_") + ".json"
            save(ws_dir, file, envelope)
        sock.close()
        if wanted:
            sys.exit(f"no frame within 30 s on {sorted(wanted)}; run again")
    return sorted(os.listdir(ws_dir))


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
    stream_files = capture_streams(out)

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

Captured {today} from `wss://fstream.binance.com` by the same script: one combined-stream
envelope (`{{"stream", "data"}}`) per stream, from `/market/stream` for `!ticker@arr`,
`!markPrice@arr@1s`, `btcusdt@aggTrade`, `btcusdt@kline_1m`, `btcusdt@markPrice@1s` and
`btcusdt@ticker`, and from `/public/stream` for `btcusdt@depth20@100ms` and
`btcusdt@bookTicker`. An array keeps two rows: a USDⓈ-M row (`st: 1`) with a scheduled
funding time, and a COIN-M row (`st: 2`) when the frame carried one. Depth sides keep
three levels. Files: {", ".join(f"`{f}`" for f in stream_files)}.

## Probes

`docs/specs/binance/probes/` holds the scripts behind the design spec's measured facts:
`probe_rest.py` (weights from `X-MBX-USED-WEIGHT-1M` deltas), `probe_ws.py` and
`probe_ws2.py` (acknowledgements, case, the 1024 cap, the message rate),
`probe_ws_ping.py` (server ping cadence), and `wsprobe.py` (the frame reader they share).
""")
    print("captured:", ", ".join(sorted(os.listdir(rest))), "and", ", ".join(stream_files))


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Capture**

Run: `python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures`
Expected: `captured: agg_trades.json, ... time.json and stream_all_markPrice_arr_1s.json, ..., stream_btcusdt_ticker.json`. A `no frame within 30 s` exit means a stream was quiet; run it again.

- [ ] **Step 3: Run every offline test against the new captures**

Run: `cargo test -j 4 -p polyoxide-binance --all-features --lib --test wire_agreement --test ws_wire_agreement --test mock_api`
Expected: PASS. A failure naming a key the server sent is drift since 2026-10-07: model it, and record it in Task 8's `OBSERVED.md`.

- [ ] **Step 4: Commit**

```bash
git add scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures
git commit -m "test(binance): capture the stream fixtures with the REST ones

The capture script records one combined-stream envelope per stream kind,
keeping a USDⓈ-M row with a scheduled funding time and, when the frame
carries one, a COIN-M row of each array stream.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 7: `polyoxide ws binance`

**Files:**
- Create: `polyoxide-cli/src/commands/ws/binance.rs`, `polyoxide-cli/tests/ws_binance.rs`
- Modify: `polyoxide-cli/src/commands/ws/mod.rs`, `polyoxide-cli/Cargo.toml`, `polyoxide-cli/tests/live_api.rs`

- [ ] **Step 1: Add the dependency**

In `polyoxide-cli/Cargo.toml`, before the `polyoxide-clob` line of `[dependencies]`, add:

```toml
polyoxide-binance = { workspace = true, features = ["ws"] }
```

and at the end of `[dev-dependencies]`, add:

```toml
# The captured frames, for the `ws binance` tests.
polyoxide-binance = { workspace = true, features = ["test-server"] }
```

- [ ] **Step 2: Write the tests**

`polyoxide-cli/tests/ws_binance.rs`:

```rust
//! `polyoxide ws binance` over scripted event streams built from the captured
//! frames, so a flag that parses but never reaches the output fails here.

use std::io;

use clap::Parser;
use futures_util::stream;
use polyoxide_binance::usdm::ws::{
    fixtures, DisconnectReason, Event, StreamPath, Update, UsdmWsError,
};
use polyoxide_cli::commands::ws::binance::{run_with, BinanceArgs};
use serde_json::Value;

#[derive(Parser)]
struct Cli {
    #[command(flatten)]
    args: BinanceArgs,
}

type Item = Result<Event, UsdmWsError>;

fn update(frame: &str) -> Item {
    Ok(Event::Update(Box::new(Update::from_json(frame).unwrap())))
}

struct Run {
    out: String,
    err: String,
}

async fn run(argv: &[&str], events: Vec<Item>) -> Run {
    let mut full = vec!["binance", "--all-mark-prices"];
    full.extend_from_slice(argv);
    let cli = Cli::try_parse_from(full).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    run_with(cli.args, stream::iter(events), &mut out, &mut err)
        .await
        .unwrap();
    Run {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

#[tokio::test]
async fn json_lines_are_the_frames_envelopes() {
    let events: Vec<Item> = fixtures::ALL
        .iter()
        .map(|(_, frame)| update(frame))
        .collect();
    let run = run(&["--format", "json"], events).await;
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), fixtures::ALL.len());
    for (line, (name, frame)) in lines.iter().zip(fixtures::ALL) {
        let printed: Value = serde_json::from_str(line).unwrap();
        let mut wire: Value = serde_json::from_str(frame).unwrap();
        // The kline's `B`, documented as "Ignore", is not modelled.
        if let Some(k) = wire.pointer_mut("/data/k").and_then(Value::as_object_mut) {
            k.remove("B");
        }
        assert_eq!(printed, wire, "{name}");
    }
    assert!(run.err.contains("The feed ended"), "{}", run.err);
}

#[tokio::test]
async fn count_stops_after_n_updates_and_markers_go_to_stderr() {
    let events = vec![
        update(fixtures::MARK_PRICE),
        Ok(Event::Disconnected {
            path: StreamPath::Market,
            reason: DisconnectReason::Stale,
        }),
        Ok(Event::Reconnected {
            path: StreamPath::Market,
        }),
        update(fixtures::MARK_PRICE),
        update(fixtures::MARK_PRICE),
    ];
    let run = run(&["--format", "json", "-n", "2"], events).await;
    assert_eq!(run.out.lines().count(), 2);
    assert!(
        !run.out.contains('#'),
        "markers leaked into stdout: {}",
        run.out
    );
    assert!(run.err.contains("# market disconnected"), "{}", run.err);
    assert!(run.err.contains("# market reconnected"), "{}", run.err);
    assert!(run.err.contains("Reached 2 update(s)"), "{}", run.err);
}

#[tokio::test]
async fn pretty_prints_a_line_per_update_and_per_array_row() {
    let run = run(
        &[],
        vec![
            update(fixtures::ALL_MARK_PRICES),
            update(fixtures::AGG_TRADE),
            update(fixtures::BOOK_TICKER),
        ],
    )
    .await;
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 4, "{}", run.out);
    assert!(
        lines[0].contains("mark") && lines[1].contains("mark"),
        "{}",
        run.out
    );
    assert!(
        lines[2].starts_with("BTCUSDT") && lines[2].contains("trade"),
        "{}",
        run.out
    );
    assert!(lines[3].contains("book"), "{}", run.out);
}

#[tokio::test]
async fn a_bad_frame_is_skipped_on_stderr_and_a_fatal_error_ends_the_run() {
    let bad =
        Update::from_json(r#"{"stream":"btcusdt@aggTrade","data":{"e":"aggTrade"}}"#).unwrap_err();
    let events = vec![Err(bad), update(fixtures::MARK_PRICE)];
    let run = run(&["--format", "json"], events).await;
    assert_eq!(run.out.lines().count(), 1);
    assert!(
        run.err
            .contains("# skipped a frame on \"btcusdt@aggTrade\""),
        "{}",
        run.err
    );

    let cli = Cli::try_parse_from(["binance", "--all-mark-prices"]).unwrap();
    let events: Vec<Item> = vec![Err(UsdmWsError::Refused {
        code: 2,
        msg: "Invalid request".into(),
    })];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let result = run_with(cli.args, stream::iter(events), &mut out, &mut err).await;
    assert!(result.unwrap_err().to_string().contains("Invalid request"));
}

#[tokio::test]
async fn a_closed_reader_ends_the_run_quietly() {
    struct Closed;
    impl io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let cli = Cli::try_parse_from(["binance", "--all-mark-prices"]).unwrap();
    let mut err = Vec::new();
    run_with(
        cli.args,
        stream::iter(vec![update(fixtures::MARK_PRICE)]),
        &mut Closed,
        &mut err,
    )
    .await
    .unwrap();
}
```

Append to `polyoxide-cli/tests/live_api.rs`, after a blank line:

```rust
// ── ws binance ───────────────────────────────────────────────────────

mod ws_binance {
    use clap::Parser;
    use polyoxide_binance::usdm::ws::UsdmWsBuilder;
    use polyoxide_cli::commands::ws::binance::{run_with, BinanceArgs};
    use serde_json::Value;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        args: BinanceArgs,
    }

    #[tokio::test]
    #[ignore = "hits the real Binance API"]
    async fn live_ws_binance_prints_one_json_line() {
        let cli = Cli::try_parse_from([
            "binance",
            "--all-mark-prices",
            "-n",
            "1",
            "--format",
            "json",
            "--timeout",
            "60s",
        ])
        .unwrap();
        let feed = UsdmWsBuilder::new()
            .streams(cli.args.streams().unwrap())
            .connect()
            .await
            .unwrap_or_else(|e| panic!("could not connect to Binance: {e:?}"));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        run_with(cli.args, feed, &mut out, &mut err).await.unwrap();
        let out = String::from_utf8(out).unwrap();
        let line = out.lines().next().unwrap_or_else(|| {
            panic!(
                "no update within 60 s; stderr: {}",
                String::from_utf8_lossy(&err)
            )
        });
        let frame: Value = serde_json::from_str(line).unwrap();
        assert_eq!(frame["stream"], "!markPrice@arr@1s");
        assert!(
            frame["data"]
                .as_array()
                .is_some_and(|rows| !rows.is_empty()),
            "{frame}"
        );
    }
}
```

Run: `cargo test -j 4 -p polyoxide-cli --test ws_binance`
Expected: FAIL to compile: ``could not find `binance` in `ws` ``.

- [ ] **Step 3: Write the command**

`polyoxide-cli/src/commands/ws/binance.rs`:

```rust
//! `polyoxide ws binance`: stream Binance USDⓈ-M futures market data.

use std::{
    io::{self, Write},
    time::Duration,
};

use clap::Args;
use color_eyre::eyre::{bail, Result};
use futures_util::{Stream, StreamExt};
use polyoxide_binance::usdm::{
    types::{Interval, Symbol},
    ws::{
        AggTradeEvent, BookTickerEvent, DepthLevels, DepthSpeed, Event, KlineEvent, MarkPriceEvent,
        PartialDepthEvent, Payload, StreamName, TickerEvent, Update, UsdmWsBuilder, UsdmWsError,
    },
};

use crate::commands::common::parsing::{parse_duration, parse_list_entry};

/// How each update is printed.
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable: one line per update, and one per row of an array stream.
    #[default]
    Pretty,
    /// The frame's `{"stream", "data"}` envelope as compact JSON, one per line.
    Json,
}

/// The kinds a symbol can be streamed as.
const KINDS: &str = "agg-trade, book-ticker, depth5, depth10, depth20, kline-<interval>, \
                     mark-price, ticker";

#[derive(Args, Debug)]
pub struct BinanceArgs {
    /// Symbols, comma-separated, e.g. BTCUSDT,ETHUSDT. Each `--kind` is
    /// streamed for each symbol. Letters are matched case-blind.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub symbol: Vec<String>,

    /// Kinds to stream for each symbol, comma-separated: agg-trade,
    /// book-ticker, depth5, depth10, depth20 (100 ms), kline-<interval>
    /// (kline-1m, kline-1h, kline-1d, ...), mark-price, ticker.
    #[arg(long, value_delimiter = ',', value_parser = parse_list_entry)]
    pub kind: Vec<String>,

    /// Stream the 24-hour ticker of every symbol that changed (`!ticker@arr`).
    #[arg(long)]
    pub all_tickers: bool,

    /// Stream every symbol's mark price and funding each second
    /// (`!markPrice@arr@1s`).
    #[arg(long)]
    pub all_mark_prices: bool,

    /// Output format
    #[arg(short, long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,

    /// Exit after printing N updates
    #[arg(short = 'n', long)]
    pub count: Option<u64>,

    /// Exit after the given duration (e.g. "30s", "5m")
    #[arg(short, long, value_parser = parse_duration)]
    pub timeout: Option<Duration>,
}

impl BinanceArgs {
    /// The streams the arguments name.
    pub fn streams(&self) -> Result<Vec<StreamName>> {
        let symbols = self
            .symbol
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| Symbol::new(s.as_str()).map_err(|e| color_eyre::eyre::eyre!("{e}")))
            .collect::<Result<Vec<_>>>()?;
        let kinds: Vec<&str> = self
            .kind
            .iter()
            .map(String::as_str)
            .filter(|k| !k.is_empty())
            .collect();
        if symbols.is_empty() != kinds.is_empty() {
            bail!("--symbol and --kind go together: each kind is streamed for each symbol");
        }
        let mut streams = Vec::new();
        if self.all_tickers {
            streams.push(StreamName::AllTickers);
        }
        if self.all_mark_prices {
            streams.push(StreamName::AllMarkPrices);
        }
        for symbol in &symbols {
            for kind in &kinds {
                streams.push(stream(symbol.clone(), kind)?);
            }
        }
        if streams.is_empty() {
            bail!(
                "nothing to stream: pass --symbol with --kind, --all-tickers or --all-mark-prices"
            );
        }
        Ok(streams)
    }
}

/// One `--kind` for one symbol.
fn stream(symbol: Symbol, kind: &str) -> Result<StreamName> {
    Ok(match kind {
        "agg-trade" => StreamName::AggTrade(symbol),
        "book-ticker" => StreamName::BookTicker(symbol),
        "depth5" => StreamName::PartialDepth(symbol, DepthLevels::Five, DepthSpeed::Ms100),
        "depth10" => StreamName::PartialDepth(symbol, DepthLevels::Ten, DepthSpeed::Ms100),
        "depth20" => StreamName::PartialDepth(symbol, DepthLevels::Twenty, DepthSpeed::Ms100),
        "mark-price" => StreamName::MarkPrice(symbol),
        "ticker" => StreamName::Ticker(symbol),
        _ => match kind
            .strip_prefix("kline-")
            .and_then(|i| i.parse::<Interval>().ok())
        {
            Some(interval) => StreamName::Kline(symbol, interval),
            None => bail!(
                "unknown kind {kind:?}; expected one of {KINDS}, where <interval> is one of {}",
                Interval::ALL
                    .iter()
                    .map(|i| i.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        },
    })
}

/// Connect to the production host and stream until `-n`, `-t` or Ctrl+C.
pub async fn run(args: BinanceArgs) -> Result<()> {
    let streams = args.streams()?;
    eprintln!("Connecting to Binance USDⓈ-M streams...");
    let feed = UsdmWsBuilder::new().streams(streams).connect().await?;
    eprintln!("Connected. Press Ctrl+C to exit.");
    run_with(args, feed, &mut std::io::stdout(), &mut std::io::stderr()).await
}

/// Print events from any stream until `-n` or `-t` is reached or the stream
/// ends.
///
/// Takes the stream rather than connecting, so tests can drive every flag with
/// a scripted list of events. Updates go to `out`; outage markers and skipped
/// frames go to `err`, so JSON output stays clean JSONL.
pub async fn run_with<S>(
    args: BinanceArgs,
    mut events: S,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<()>
where
    S: Stream<Item = Result<Event, UsdmWsError>> + Unpin,
{
    let deadline = args
        .timeout
        .and_then(|t| tokio::time::Instant::now().checked_add(t));
    let mut printed: u64 = 0;
    loop {
        if let Some(n) = args.count {
            if printed >= n {
                writeln!(err, "Reached {n} update(s)")?;
                break;
            }
        }
        let next = match deadline {
            Some(deadline) => match tokio::time::timeout_at(deadline, events.next()).await {
                Ok(next) => next,
                Err(_) => {
                    writeln!(err, "Timeout reached")?;
                    break;
                }
            },
            None => events.next().await,
        };
        match next {
            Some(Ok(Event::Update(update))) => match print_update(&update, args.format, out) {
                Ok(()) => printed += 1,
                // The reader went away, as with `| head -1`; not an error.
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => break,
                Err(error) => return Err(error.into()),
            },
            Some(Ok(Event::Disconnected { path, reason })) => writeln!(
                err,
                "# {path} disconnected: {reason}. Its streams are stale until it reconnects."
            )?,
            Some(Ok(Event::Reconnected { path })) => writeln!(
                err,
                "# {path} reconnected. Rebuild anything built from its streams."
            )?,
            // `Event` is #[non_exhaustive]; a future variant is not a fault.
            Some(Ok(_)) => {}
            Some(Err(UsdmWsError::Frame {
                stream,
                raw,
                reason,
            })) => writeln!(
                err,
                "# skipped a frame on {stream:?} that did not decode ({reason}): {}",
                excerpt(&raw)
            )?,
            Some(Err(error)) => return Err(error.into()),
            None => {
                writeln!(err, "The feed ended")?;
                break;
            }
        }
    }
    Ok(())
}

/// Print one update and flush it.
fn print_update(update: &Update, format: OutputFormat, out: &mut dyn Write) -> io::Result<()> {
    match format {
        OutputFormat::Json => writeln!(out, "{}", serde_json::to_string(update)?)?,
        OutputFormat::Pretty => match &update.payload {
            Payload::Tickers(rows) => {
                for row in rows {
                    writeln!(out, "{}", ticker(row))?;
                }
            }
            Payload::MarkPrices(rows) => {
                for row in rows {
                    writeln!(out, "{}", mark_price(row))?;
                }
            }
            Payload::AggTrade(event) => writeln!(out, "{}", agg_trade(event))?,
            Payload::Kline(event) => writeln!(out, "{}", kline(event))?,
            Payload::MarkPrice(event) => writeln!(out, "{}", mark_price(event))?,
            Payload::Ticker(event) => writeln!(out, "{}", ticker(event))?,
            Payload::PartialDepth(event) => writeln!(out, "{}", depth(event))?,
            Payload::BookTicker(event) => writeln!(out, "{}", book_ticker(event))?,
            Payload::Unknown { event_type, .. } => writeln!(
                out,
                "{:<16} unknown event {event_type:?}",
                update.stream.to_string()
            )?,
            _ => writeln!(out, "{}", update.stream)?,
        },
    }
    out.flush()
}

fn agg_trade(t: &AggTradeEvent) -> String {
    let side = if t.is_buyer_maker { "sell" } else { "buy" };
    format!(
        "{:<16} trade   {} @ {} {side}",
        t.symbol, t.quantity, t.price
    )
}

fn kline(k: &KlineEvent) -> String {
    let bar = &k.kline;
    let closed = if bar.is_closed { " closed" } else { "" };
    format!(
        "{:<16} kline   {} o {} h {} l {} c {} v {}{closed}",
        k.symbol, bar.interval, bar.open, bar.high, bar.low, bar.close, bar.volume
    )
}

fn mark_price(m: &MarkPriceEvent) -> String {
    format!(
        "{:<16} mark    {} index {} funding {}",
        m.symbol, m.mark_price, m.index_price, m.funding_rate
    )
}

fn ticker(t: &TickerEvent) -> String {
    format!(
        "{:<16} ticker  last {} open {} quote volume {}",
        t.symbol, t.last_price, t.open_price, t.quote_volume
    )
}

fn depth(d: &PartialDepthEvent) -> String {
    let level = |side: &[polyoxide_binance::usdm::types::Level]| match side.first() {
        Some(l) => format!("{} x {}", l.price, l.quantity),
        None => "-".to_owned(),
    };
    format!(
        "{:<16} depth   bid {} ask {} ({} levels)",
        d.symbol,
        level(&d.bids),
        level(&d.asks),
        d.bids.len().max(d.asks.len())
    )
}

fn book_ticker(b: &BookTickerEvent) -> String {
    format!(
        "{:<16} book    {} x {} / {} x {}",
        b.symbol, b.bid_price, b.bid_quantity, b.ask_price, b.ask_quantity
    )
}

/// The first 200 characters of a frame, escaped, so it stays one stderr line.
fn excerpt(raw: &str) -> String {
    const LIMIT: usize = 200;
    let escaped: String = raw
        .chars()
        .take(LIMIT)
        .flat_map(char::escape_debug)
        .collect();
    if raw.chars().count() > LIMIT {
        escaped + "…"
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct Wrapper {
        #[command(flatten)]
        args: BinanceArgs,
    }

    fn parse(argv: &[&str]) -> BinanceArgs {
        Wrapper::try_parse_from(argv).unwrap().args
    }

    #[test]
    fn symbols_and_kinds_split_on_commas_and_multiply() {
        let args = parse(&[
            "test",
            "--symbol",
            "btcusdt, 币安人生USDT",
            "--kind",
            "agg-trade,kline-1h",
        ]);
        let names: Vec<String> = args
            .streams()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            names,
            [
                "btcusdt@aggTrade",
                "btcusdt@kline_1h",
                "币安人生usdt@aggTrade",
                "币安人生usdt@kline_1h"
            ]
        );
    }

    #[test]
    fn every_kind_parses_and_depth_streams_at_100ms() {
        let args = parse(&[
            "test",
            "--symbol",
            "BTCUSDT",
            "--kind",
            "agg-trade,book-ticker,depth5,depth10,depth20,kline-1M,mark-price,ticker",
            "--all-tickers",
            "--all-mark-prices",
        ]);
        let names: Vec<String> = args
            .streams()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            names,
            [
                "!ticker@arr",
                "!markPrice@arr@1s",
                "btcusdt@aggTrade",
                "btcusdt@bookTicker",
                "btcusdt@depth5@100ms",
                "btcusdt@depth10@100ms",
                "btcusdt@depth20@100ms",
                "btcusdt@kline_1M",
                "btcusdt@markPrice@1s",
                "btcusdt@ticker",
            ]
        );
    }

    #[test]
    fn a_bad_kind_names_the_valid_ones() {
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "trades"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("\"trades\"") && err.contains("agg-trade") && err.contains("1M"),
            "{err}"
        );
        let err = parse(&["test", "--symbol", "BTCUSDT", "--kind", "kline-1s"])
            .streams()
            .unwrap_err()
            .to_string();
        assert!(err.contains("kline-1s"), "{err}");
    }

    #[test]
    fn a_symbol_needs_a_kind_and_something_must_be_streamed() {
        assert!(parse(&["test", "--symbol", "BTCUSDT"]).streams().is_err());
        assert!(parse(&["test", "--kind", "ticker"]).streams().is_err());
        assert!(parse(&["test"])
            .streams()
            .unwrap_err()
            .to_string()
            .contains("nothing to stream"));
        assert!(parse(&["test", "--symbol", "BTC USDT", "--kind", "ticker"])
            .streams()
            .is_err());
    }

    #[test]
    fn count_timeout_and_format_parse() {
        let args = parse(&[
            "test",
            "--all-mark-prices",
            "-n",
            "3",
            "-t",
            "5m",
            "--format",
            "json",
        ]);
        assert_eq!(args.count, Some(3));
        assert_eq!(args.timeout, Some(Duration::from_secs(300)));
        assert_eq!(args.format, OutputFormat::Json);
    }
}
```

In `polyoxide-cli/src/commands/ws/mod.rs`, replace `mod market;` with

```rust
pub mod binance;
mod market;
```

add a variant after `Sports`:

```rust
    /// Stream Binance USDⓈ-M futures market data: trades, klines, mark prices,
    /// tickers, depth and book tickers
    Binance {
        #[command(flatten)]
        args: binance::BinanceArgs,
    },
```

and in `run`, after the `Sports` arm:

```rust
            Self::Binance { args } => binance::run(args).await,
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -j 4 -p polyoxide-cli --lib ws::binance && cargo test -j 4 -p polyoxide-cli --test ws_binance`
Expected: PASS, 5 and 5 tests.

Run: `cargo test -j 4 -p polyoxide-cli --test live_api live_ws_binance -- --ignored`
Expected: PASS in about 5 seconds.

Run: `cargo clippy -j 4 -p polyoxide-cli --all-targets --all-features -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add polyoxide-cli Cargo.lock
git commit -m "feat(cli): ws binance

Streams Binance USDⓈ-M market data through the supervised tier: each
--kind for each --symbol, plus --all-tickers and --all-mark-prices.
Outage markers go to stderr; --format json prints each frame's
envelope. run_with takes any event stream, so tests drive it with the
crate's captured frames.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 8: Docs and the nightly row

**Files:**
- Modify: `docs/specs/binance/OBSERVED.md`, `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `CLAUDE.md`, `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `polyoxide-cli/README.md`, `.github/workflows/nightly-behavioral.yml`

- [ ] **Step 1: What the streams do**

In `docs/specs/binance/OBSERVED.md`, replace

````markdown
## Streams

Recorded in the design spec's venue contract (1024 streams per connection, about 15
requests back to back before a close, every `SUBSCRIBE` acknowledged, server pings about
every 180 s) and moved here by the WebSocket plan.
````

with

````markdown
## Streams

Measured on `fstream.binance.com` on 2026-10-07 with `probes/probe_ws.py`,
`probe_ws2.py` and `probe_ws_ping.py`, and from full frames read while planning.

| Rule | Documented | Measured on `/market` |
|---|---|---|
| Streams per connection | 1024 | 1024 accepted. The 1025th is answered `{"error":{"code":4,"msg":"Too many subscriptions"}}`, then the server closes with 1008 "Invalid request", losing all 1024 |
| Incoming messages | 10 per second | 15 back-to-back requests all answered; of 40 sent back to back, 15 were answered, then a close with 1008 "Too many requests" |
| Acknowledgement | | Every `SUBSCRIBE` is acknowledged: an unknown symbol, a stream type that does not exist, an uppercase symbol. `LIST_SUBSCRIPTIONS` echoes them. An `UNSUBSCRIBE` of a name never subscribed is acknowledged |
| Case | Symbols in stream names are lowercase | `BTCUSDT@aggTrade` is acknowledged and delivers nothing; `btcusdt@aggTrade` delivers |
| Server ping | Every 3 minutes; no pong for 10 minutes disconnects | Pings at 140 s and 320 s on a quiet connection, payload a millisecond timestamp |
| Client ping | | Answered in about 0.3 s |
| Request size | | 200 names in one `SUBSCRIBE` acknowledged |
| Non-ASCII symbols | | Raw UTF-8 in `SUBSCRIBE` accepted; the stream name is echoed exactly |
| Paths | `/public` for depth and book tickers, `/market` for the rest | The legacy `/stream` served depth but sent nothing for `!markPrice@arr@1s` |
| Connection lifetime | 24 hours | Not measured; the supervised tier rotates at 23 h 50 min |

So an acknowledgement proves nothing about a stream's validity, and a name that will never
deliver looks like a quiet one. `StreamName` builds every name, and both tiers enforce the
cap, 200 names per request and one request per 200 ms before sending.

Payloads:

- `!markPrice@arr@1s` sends two frames a second (745 and 217 rows). A row with no funding
  scheduled (51 of 745, delisted contracts) sends `T: 0` and `r: "0.00000000"`. `ap` is
  the "mark price moving average", equal to `p` on every row seen.
- `!ticker@arr` sends only the symbols that changed: 226 rows in one frame.
- `st`, "(After CM migration) Symbol type: 1 = UM, 2 = CM", is on every payload but the
  kline event. COIN-M rows arrive on this USDⓈ-M host: 30 of 745 mark-price rows, and
  `AAVEUSD_PERP` in `!ticker@arr`.
- The kline's `B` is documented as "Ignore" and is not modelled.
````

In `docs/specs/binance/INDEX.md`, replace

````markdown
| Market streams | `wss://fstream.binance.com/{market,public}/stream` | the WebSocket plan, not yet implemented |
````

with

````markdown
| Market streams | `wss://fstream.binance.com/{market,public}/stream` | `polyoxide-binance` with the `ws` feature (`UsdmWs`, `UsdmWsBuilder`) |
````

In `docs/specs/binance/INDEX.md`, replace

````markdown
records it. The drift detector is the live suite:
`polyoxide-binance/tests/live_api.rs::live_responses_carry_no_unmodelled_keys` fails on any
key the types do not model.
````

with

````markdown
records it. The drift detectors are the live suites:
`polyoxide-binance/tests/live_api.rs::live_responses_carry_no_unmodelled_keys` and
`live_ws.rs::live_frames_carry_no_unmodelled_keys` fail on any key the types do not model.
````

In `docs/specs/binance/INDEX.md`, replace

````markdown
## Fixtures and probes
````

with

````markdown
## Streams covered

| Stream | `StreamName` | Path | `Payload` |
|---|---|---|---|
| `!ticker@arr` | `AllTickers` | market | `Tickers(Vec<TickerEvent>)` |
| `!markPrice@arr@1s` | `AllMarkPrices` | market | `MarkPrices(Vec<MarkPriceEvent>)` |
| `<s>@aggTrade` | `AggTrade(symbol)` | market | `AggTrade(AggTradeEvent)` |
| `<s>@kline_<interval>` | `Kline(symbol, interval)` | market | `Kline(KlineEvent)` |
| `<s>@markPrice@1s` | `MarkPrice(symbol)` | market | `MarkPrice(MarkPriceEvent)` |
| `<s>@ticker` | `Ticker(symbol)` | market | `Ticker(TickerEvent)` |
| `<s>@depth<5\|10\|20>@<100ms\|250ms\|500ms>` | `PartialDepth(symbol, levels, speed)` | public | `PartialDepth(PartialDepthEvent)` |
| `<s>@bookTicker` | `BookTicker(symbol)` | public | `BookTicker(BookTickerEvent)` |

## Fixtures and probes
````

In `docs/specs/binance/INDEX.md`, replace

````markdown
- `polyoxide-binance/tests/fixtures/ws/`: stream envelopes captured 2026-10-07.
````

with

````markdown
- `polyoxide-binance/tests/fixtures/ws/`: stream envelopes, refreshed by the same script and
  compiled into `polyoxide_binance::usdm::ws::fixtures` under `test-server`.
````

- [ ] **Step 2: The crate README**

Append to `polyoxide-binance/README.md`, after a blank line:

````markdown
## Streaming

With the `ws` feature, the market streams arrive over one connection per routed path
(`/market` for trades, klines, mark prices and tickers; `/public` for depth and book
tickers). `UsdmWsBuilder` keeps each connection alive, replaces a dead or 24-hour-old one,
replays its streams at Binance's pace, and reports each outage: every
`Event::Disconnected { path }` is followed by `Event::Reconnected { path }`, after which
anything built from that path's streams should be rebuilt.

```text
use futures_util::StreamExt;
use polyoxide_binance::usdm::{types::Symbol, ws::{Event, StreamName, UsdmWsBuilder}};

let btc = Symbol::new("BTCUSDT")?;
let mut feed = UsdmWsBuilder::new()
    .streams([StreamName::MarkPrice(btc.clone()), StreamName::BookTicker(btc)])
    .connect()
    .await?;
while let Some(event) = feed.next().await {
    match event? {
        Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
        Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
        Event::Reconnected { path } => eprintln!("{path} back"),
        _ => {}
    }
}
```
````

- [ ] **Step 3: The repository's docs**

In `CLAUDE.md`, replace

````text
├── polyoxide-binance   (Binance USDⓈ-M futures public market data; not in the umbrella crate)
````

with

````text
├── polyoxide-binance   (Binance USDⓈ-M futures market data and streams; not in the umbrella crate)
````

In `CLAUDE.md`, replace

````markdown
`polyoxide-rtds` and `polyoxide-sports` — plus
````

with

````markdown
`polyoxide-rtds`, `polyoxide-sports` and `polyoxide-binance` (with `ws`) — plus
````

In `CLAUDE.md`, replace

````markdown
drives every flag with captured frames.
````

with

````markdown
drives every flag with captured frames.

`ws binance` streams Binance USDⓈ-M market data through `polyoxide-binance`'s supervised
tier. `--symbol` and `--kind` are comma-separated, and each kind is streamed for each
symbol; `--all-tickers` and `--all-mark-prices` add the two array streams. Outage markers
go to stderr, and `--format json` prints each frame's envelope. Its `run_with` takes any
event stream, and `polyoxide-cli/tests/ws_binance.rs` drives it with the crate's captured
frames (`polyoxide_binance::usdm::ws::fixtures`, feature `test-server`).
````

In `CLAUDE.md`, replace

````markdown
which is why four rate-limit examples pin `.gzip(false)`.
````

with

````markdown
which is why four rate-limit examples pin `.gzip(false)`.

With the `ws` feature `polyoxide-binance` also streams eight USDⓈ-M market streams on
`fstream.binance.com`: `UsdmWs` (one connection on one path) and
`UsdmWsBuilder`/`SupervisedUsdmWs` (one connection per routed path, `/market` or
`/public`, since a stream subscribed on the wrong path delivers nothing; pings on the wall
clock; staleness counting pongs and the server's pings; reconnect with a paced replay;
rotation at 23 h 50 min, under Binance's 24-hour cutoff). Binance acknowledges every
`SUBSCRIBE`, unknown and uppercase names included, and enforces its rules by closing the
connection (on the 1025th stream, or after about 15 requests in a burst), so the client
enforces them first: `StreamName` is the only way to name a stream, and a connection
carries at most 1024 streams, 200 names per request, one request per 200 ms. Every
`Event::Disconnected { path }` is followed by `Event::Reconnected { path }` while the
client runs, even when the path's last stream leaves mid-outage; prader-rs folds outages
on that invariant, and `tests/supervision.rs` pins it. COIN-M rows (`st: 2`) arrive on this
host. The offline tests drive the scripted server in `src/usdm/ws/test_server.rs` (feature
`test-server`), which also exposes the captured frames as `usdm::ws::fixtures`.
````

In `CLAUDE.md`, replace

````markdown
sports, binance, cli)
````

with

````markdown
sports, binance incl. `live_ws`, cli)
````

In `CLAUDE.md`, replace

````markdown
The Perps socket (`polyoxide-perps/src/ws/`, feature `ws`), RTDS (`polyoxide-rtds`) and the sports feed (`polyoxide-sports`) are separate protocols in their own crates, each with its own `ensure_crypto_provider` copy.
````

with

````markdown
The Perps socket (`polyoxide-perps/src/ws/`, feature `ws`), RTDS (`polyoxide-rtds`), the sports feed (`polyoxide-sports`) and Binance's market streams (`polyoxide-binance/src/usdm/ws/`, feature `ws`) are separate protocols in their own crates, each with its own `ensure_crypto_provider` copy.
````

In `README.md`, replace

````markdown
| [polyoxide-binance](./polyoxide-binance) | Client library for Binance USDⓈ-M futures public market data (not part of the unified crate) |
````

with

````markdown
| [polyoxide-binance](./polyoxide-binance) | Client library for Binance USDⓈ-M futures market data and streams (not part of the unified crate) |
````

In `docs/specs/INDEX.md`, replace

````markdown
| [Binance USDⓈ-M](binance/INDEX.md) | `https://fapi.binance.com` | Futures public market data. No published spec, so not a mirror | `polyoxide-binance` |
````

with

````markdown
| [Binance USDⓈ-M](binance/INDEX.md) | `https://fapi.binance.com`, `wss://fstream.binance.com` | Futures public market data and market streams. No published spec, so not a mirror | `polyoxide-binance` (streams behind `ws`) |
````

In `SELF-HEALING.md`, replace

````markdown
| polyoxide-binance | `live_api` |
````

with

````markdown
| polyoxide-binance | `live_api`, `live_ws` (built with `--features ws`) |
````

In `SELF-HEALING.md`, replace

````markdown
  `polyoxide-binance/tests/live_api.rs` is the drift check.
````

with

````markdown
  `polyoxide-binance/tests/live_api.rs` and `live_ws.rs` are the drift check.
````

In `polyoxide-cli/README.md`, replace

````markdown
Stream real-time market, user and sports updates.
````

with

````markdown
Stream real-time market, user and sports updates, and Binance futures market data.
````

In `polyoxide-cli/README.md`, replace

````markdown
---

### Credentials (feature `keychain`)
````

with

````markdown
#### `ws binance`

Binance USDⓈ-M futures market data. No credentials needed. Each `--kind` is streamed for
each `--symbol`; connection notices go to stderr.

```bash
# Every symbol's mark price and funding, each second
polyoxide ws binance --all-mark-prices

# Trades and hourly klines for two symbols
polyoxide ws binance --symbol BTCUSDT,ETHUSDT --kind agg-trade,kline-1h

# Top 20 book levels every 100 ms, as JSON envelopes, stop after 10 updates
polyoxide ws binance --symbol BTCUSDT --kind depth20 --format json -n 10
```

Kinds: `agg-trade`, `book-ticker`, `depth5`, `depth10`, `depth20`, `kline-<interval>`
(`1m` to `1M`), `mark-price`, `ticker`.

---

### Credentials (feature `keychain`)
````

- [ ] **Step 4: The nightly row**

In `.github/workflows/nightly-behavioral.yml`, replace

````yaml
          - { crate: polyoxide-binance, suite: live,       timeout: 15, flags: "--test live_api" }
````

with

````yaml
          - { crate: polyoxide-binance, suite: live,       timeout: 15, flags: "--features ws --test live_api --test live_ws" }
````

Run: `grep -n 'polyoxide-binance' .github/workflows/nightly-behavioral.yml`
Expected: the row with `--features ws --test live_api --test live_ws`.

- [ ] **Step 5: The full gate**

```bash
cargo fmt --all -- --check
cargo clippy -j 4 --all-targets --all-features -- -D warnings
cargo test -j 4 --all-features --workspace
cargo test -j 4 --doc --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc -j 4 --no-deps --all-features --workspace
```

All must be clean. If the workspace builds are reaped (`signal: 15`), rerun with `-j 2`, or run `-p polyoxide-binance -p polyoxide-cli` and leave the rest to CI.

- [ ] **Step 6: Commit**

```bash
git add docs CLAUDE.md README.md SELF-HEALING.md polyoxide-binance/README.md polyoxide-cli/README.md .github/workflows/nightly-behavioral.yml
git commit -m "docs: Binance market streams in OBSERVED, INDEX, the READMEs and CLAUDE.md; nightly live_ws

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

---

### Task 9: Release 0.37.0, when the owner says so

prader-rs waits for 0.37.0 on crates.io and never takes a path or git dependency. This task is a procedure for the release, gated on the owner. Do not run it until the owner asks for the release.

- [ ] **Step 1: Land the branch on `main`**

Both plans' commits reach `main` through loom's merge, not from this worktree. A release commit cut in a worktree has sat unmerged before, and was then cut a second time on `main`. Run `git log --all --oneline --grep='chore(release)' -3` and `git branch --list '*vbump*' -v`: an unmerged release commit is landed, not duplicated.

- [ ] **Step 2: Check the version is free, from origin, not from local state**

```bash
git fetch origin --tags --prune-tags
git rev-list --left-right --count origin/main...HEAD
git ls-remote --tags origin | grep 'v0.37.0' || echo "tag free"
gh release list --limit 3
curl -s https://crates.io/api/v1/crates/polyoxide/versions | python3 -c 'import json,sys; print([v["num"] for v in json.load(sys.stdin)["versions"][:3]])'
```

A non-zero left count means `main` has moved: integrate first and derive the number again. `release.yml` skips every publish job, silently, when the release for the version in `Cargo.toml` already exists. A new crate and a new CLI command are features, so the bump is minor: 0.37.0.

- [ ] **Step 3: Bump**

- `Cargo.toml`: `version = "0.37.0"` in `[workspace.package]`, and every `[workspace.dependencies]` path pin at `"0.37.0"`. There are nine pins now that `polyoxide-binance` has one, and `.github/scripts/tests/test_changelog.py` checks all of them.
- Every crate README's install line from `"0.36"` to `"0.37"`, `polyoxide-binance/README.md` included (`grep -rn '"0\.36"' */README.md`).
- `cargo update --workspace` (never `cargo generate-lockfile`, which re-resolves every transitive dependency).
- `git-cliff --unreleased --tag v0.37.0 --prepend CHANGELOG.md`, then restore the blank line `--prepend` omits before the next `##`. Never `-o`, which re-renders shipped sections.
- At the top of the new section, write three sentences by hand: `polyoxide-binance` is new and is not in the `polyoxide` umbrella crate or `full`; `HttpClientBuilder::gzip` is new and off by default; and the crates now enable reqwest 0.12's `gzip` feature, which feature unification turns on for a consumer's own reqwest 0.12 clients too, so those send `Accept-Encoding: gzip` unless built with `.gzip(false)`.

- [ ] **Step 4: Check and commit, last**

```bash
cd .github/scripts && uv run pytest tests/test_changelog.py -q && cd ../..
cargo build -j 4 --workspace
git add Cargo.toml Cargo.lock CHANGELOG.md */README.md
git commit -m "chore(release): v0.37.0

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01KDbtfY1gMjGiQXvcFGcPeN"
```

The release commit must be the last commit. Anything committed after it ships without a changelog entry, and no check notices. Never reset a pushed release commit: if more must go in, cut 0.37.1. This is `polyoxide-binance`'s first publish; the CI token's new-crate scope published `polyoxide-perps` for the first time in 0.34.0.

---

## Done when

- `cargo test -p polyoxide-binance --all-features` passes offline, the supervision tests among them; the live socket suite and `live_ws_binance_prints_one_json_line` pass against the host.
- The full gate in Task 8 is clean.
- prader-rs's stream contract compiles against the crate. That means `StreamName`'s eight variants, `DepthLevels`/`DepthSpeed`/`StreamPath`, and `Update::from_json` over the eight fixtures. It also means the `Payload` and event field names, `Event::Update(Box<Update>)`, the constructible `Disconnected { path, reason: DisconnectReason::Stale }` and `Reconnected { path }`, `UsdmWsBuilder`'s builder methods, `MembershipHandle::subscribe(Vec<StreamName>)`, and `ScriptedServer::start(vec![Script { pushes, close_after, ..Default::default() }])`.
- 0.37.0 is on crates.io, once the owner asks for it.
