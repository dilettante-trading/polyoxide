# polyoxide-polymarket

Rust client library for Polymarket's APIs, one feature per module: `clob`, `gamma`, `data`,
`relay`, `perps`, `rtds` and `sports`. Part of the [polyoxide](https://crates.io/crates/polyoxide)
workspace.

The modules moved in from their own crates during stage S2. Each has its own README but `rtds`,
whose guide is its module documentation:

| Module | Feature | API |
|---|---|---|
| [`clob`](src/clob/README.md) | `clob`; its WebSocket channels, `clob::ws`, under `clob-ws` | The CLOB: order signing and placement, order books, prices, account, rewards and API keys |
| [`gamma`](src/gamma/README.md) | `gamma` | Gamma: markets, events, series, tags, comments, sports, search and profiles; read-only |
| [`data`](src/data/README.md) | `data` | The Data API, v1 and v2 (`data.v2()`), and the undocumented `user-pnl-api` and `lb-api` hosts; read-only |
| [`relay`](src/relay/README.md) | `relay` | The Relayer: gasless transactions for Safe, Proxy and Deposit Wallets, and session signers |
| [`perps`](src/perps/README.md) | `perps`; its WebSocket channels, `perps::ws`, under `perps-ws` | Perps: perpetual futures market data on the public `/v1/info/*` routes; read-only |
| `rtds` | `rtds` | RTDS: Binance and Chainlink crypto price streams over one socket |
| [`sports`](src/sports/README.md) | `sports` | The live sports score feed, server push only |

What the HTTP modules share is in `shared`.

The defaults are `clob`, `gamma` and `data`. Only `clob` and `relay` build the signing stack,
`alloy`, so a build with `gamma`, `data` or `perps` alone carries none of it. `rtds` and `sports`
are sockets alone: they build neither `polyoxide-core` nor `shared`, so a credential-free feed
carries no HTTP stack either. `clob` does not turn on `gamma`: with both, the CLOB client looks a
proxy wallet's address up through Gamma.

`keychain` stores credentials in the OS keychain; its API is in `clob` and `relay`, but it builds
core's `keyring` whenever any HTTP module is on, `gamma` or `data` alone included, since cargo cannot
limit it to the two. `specta` derives `specta::Type` on the response types of `gamma` and `data`;
`clob` and `relay` derive none. `test-server` exposes the captured frames of `rtds` and `sports`
to integration tests; it is not for consumers.

With `clob`, `gamma` and `data` on, as by default, the crate root holds the unified client,
`Polymarket`: one CLOB, Gamma and Data API client, built from one `Account` by
`Polymarket::builder(account)`. Its `build` fails with a `BuildError` (`Clob`, `Gamma` or `Data`)
naming the client that could not be built, and classed as that client's error. The `polyoxide`
umbrella re-exports this crate as `polyoxide::polymarket`, and its prelude re-exports the client.

## What `shared` holds

| Item | Purpose | Feature |
|---|---|---|
| `shared::PolymarketRetryPolicy` | The one retry policy every HTTP module installs on core's send loop: core's `DefaultRetryPolicy` plus `425 Too Early` | any HTTP module |
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
