# polyoxide-polymarket

Rust client library for Polymarket's APIs, one feature per module: `clob`, `gamma`, `data`,
`relay` and `perps`. Part of the [polyoxide](https://crates.io/crates/polyoxide) workspace.

The modules move in from their own crates during stage S2 (`polyoxide-gamma` and
`polyoxide-data` in Story 5.3, `polyoxide-clob` and `polyoxide-relay` in 5.4, and
`polyoxide-perps`, `polyoxide-rtds` and `polyoxide-sports` in 5.5). Until then the crate holds
what those modules share, in `shared`, and every feature is off by default.

## What's inside

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

```toml
[dependencies]
polyoxide-polymarket = { version = "0.39", features = ["gamma"] }
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
