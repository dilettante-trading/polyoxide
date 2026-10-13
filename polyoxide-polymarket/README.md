# polyoxide-polymarket

Rust client library for Polymarket's APIs, one feature per module: `clob`, `gamma`, `data`,
`relay` and `perps`. Part of the [polyoxide](https://crates.io/crates/polyoxide) workspace.

The modules move in from their own crates during stage S2. Four are here, each with its own
README:

| Module | Feature | API |
|---|---|---|
| [`clob`](src/clob/README.md) | `clob`; its WebSocket channels, `clob::ws`, under `clob-ws` | The CLOB: order signing and placement, order books, prices, account, rewards and API keys |
| [`gamma`](src/gamma/README.md) | `gamma` | Gamma: markets, events, series, tags, comments, sports, search and profiles; read-only |
| [`data`](src/data/README.md) | `data` | The Data API, v1 and v2 (`data.v2()`), and the undocumented `user-pnl-api` and `lb-api` hosts; read-only |
| [`relay`](src/relay/README.md) | `relay` | The Relayer: gasless transactions for Safe, Proxy and Deposit Wallets, and session signers |

`polyoxide-perps`, `polyoxide-rtds` and `polyoxide-sports` arrive in Story 5.5. What the
modules share is in `shared`.

The defaults are `clob`, `gamma` and `data`. Only `clob` and `relay` build the signing stack,
`alloy`, so a build with `gamma` or `data` alone carries none of it. `clob` does not turn on
`gamma`: with both, the CLOB client looks a proxy wallet's address up through Gamma.

`keychain` stores the credentials of whichever of `clob` and `relay` is enabled in the OS
keychain, and `specta` derives `specta::Type` on the enabled modules' response types.

## What `shared` holds

| Item | Purpose | Feature |
|---|---|---|
| `shared::PolymarketRetryPolicy` | The one retry policy every HTTP module installs on core's send loop: core's `DefaultRetryPolicy` plus `425 Too Early` | any module |
| `shared::{clob,gamma,data,relay,perps}_limits` | The five window-quota tables, one per API surface, each pinned by its `documented_*_limits` test | the table's module |
| `shared::ClobThrottle`, `clob_throttle`, `signer_cost` | The CLOB's throttle: its IP table and the per-signer layer over one `Hold`, and the cost a trading request charges | `clob` |
| `shared::signer_limit` | The per-signer layer: `SignerLimiter`, `Tier`, `TradingBucket`, `TradingRequest`, the `Poly-RateLimit-*` telemetry (`RateLimitStatus`) and the refusal of a batch no bucket can hold (`BurstCapacityExceeded`) | `clob` |
| `shared::auth` | The HMAC-SHA256 `Signer`, `Base64Format` and `current_timestamp` | `clob` or `relay` |
| `shared::session_signer` | `DepositWalletRole` and `SessionSignerScope`, shared by clob's and relay's session-key calls | `clob` or `relay` |

## Example

A client on core's HTTP client with Gamma's table and Polymarket's retry policy, as Gamma's own
builder installs them:

```rust
use polyoxide_core::HttpClientBuilder;
use polyoxide_polymarket::shared::{self, PolymarketRetryPolicy};

let client = HttpClientBuilder::new("https://gamma-api.polymarket.com")
    .with_rate_limiter(shared::gamma_limits())
    .with_retry_policy(PolymarketRetryPolicy)
    .build()
    .unwrap();
# let _ = client;
```

## Installation

The defaults build `clob`, `gamma` and `data`:

```toml
[dependencies]
polyoxide-polymarket = "0.39"
```

Or name the modules you use:

```toml
[dependencies]
polyoxide-polymarket = { version = "0.39", default-features = false, features = ["gamma", "data"] }
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
