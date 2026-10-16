# Polyoxide

Multi-venue Rust SDK toolkit: Polymarket's APIs and Binance's USDⓈ-M futures market data, one
crate per venue. It includes library crates for use in your projects, the `polyoxide` umbrella
over them, and a standalone CLI.

> [!WARNING]
> This is currently work-in-progress so the API may change and some features may be missing

## Crates

<!-- generated:begin readme-crates -->
| Crate | Description |
|-------|-------------|
| [polyoxide](./polyoxide) | The multi-venue umbrella: `polyoxide::polymarket` and `polyoxide::binance` behind features named for each venue and module, `polyoxide::venue` and a `prelude` |
| [polyoxide-binance](./polyoxide-binance) | Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`); re-exported by the umbrella as `polyoxide::binance` |
| [polyoxide-cli](./polyoxide-cli) | CLI tool for querying Polymarket APIs and Binance USDⓈ-M market data |
| [polyoxide-core](./polyoxide-core) | Core utilities and shared types |
| [polyoxide-polymarket](./polyoxide-polymarket) | Client library for Polymarket's APIs, one feature per module: `clob` (order book, with its sockets under `clob-ws`), `gamma` (market data), `data` (user data, v1 and v2), `relay` (gasless transactions), `perps` (perpetual futures market data, with its sockets under `perps-ws`), `rtds` (crypto price streams) and `sports` (live sports scores), with the code the HTTP modules share in `shared` and, with `clob`, `gamma` and `data`, the unified `Polymarket` client; re-exported by the umbrella as `polyoxide::polymarket` |
| [polyoxide-py](./polyoxide-py) | Python bindings via PyO3 (`publish = false`, wheels on PyPI) |
| [polyoxide-test-support](./polyoxide-test-support) | Test toolkit: the failure tags the nightly classifier reads, the credential loaders, and the agreement, fixture and soak helpers (`publish = false`) |
| [polyoxide-venue](./polyoxide-venue) | Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), and the market-data traits with the records they return and their typed `Extensions` (feature `market`) |
| [polyoxide-ws](./polyoxide-ws) | The socket kit: the TLS install, `Backoff`, `Timing` and its setters, the deadline arithmetic, `WsError`, connect with a timeout, the bare tier, request/answer correlation, the generic `Supervisor` for a venue `Protocol`, and the scripted test server (feature `test-server`) |
<!-- generated:end readme-crates -->

## Installation

### Libraries

```bash
cargo add polyoxide
```

Its features are named for each venue and module, `polymarket-<module>` and `binance-<module>`,
and its paths too: `polyoxide::polymarket` and `polyoxide::binance`. Or install individual APIs:

```bash
# Market data only
cargo add polyoxide --no-default-features --features polymarket-gamma

# Trading only
cargo add polyoxide --no-default-features --features polymarket-clob

# Data API only
cargo add polyoxide --no-default-features --features polymarket-data

# CLOB WebSocket only
cargo add polyoxide --no-default-features --features polymarket-clob-ws

# Gasless relay
cargo add polyoxide --no-default-features --features polymarket-relay

# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws
```

### CLI

Install using cargo

```bash
cargo install polyoxide-cli
```

Or download binaries directly from Github releases

```bash
curl -fsSL https://raw.githubusercontent.com/dilettante-trading/polyoxide/main/scripts/install.sh | sh
```

Commands are `polyoxide <venue> <module> <command>`:

```bash
polyoxide polymarket gamma markets list --preset trending
polyoxide polymarket clob stream market <ASSET_ID> --format json
polyoxide binance usdm stream --all-mark-prices -n 10
```

See more information [here](./polyoxide-cli/README.md).

## Usage

### REST API

```rust
use polyoxide::prelude::*;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
// Load account from environment variables
// (or use Account::from_keychain() with the `keychain` feature)
let account = Account::from_env()?;

let client = Polymarket::builder(account)
    .chain(Chain::PolygonMainnet)
    .build()?;

// List open markets via Gamma API
let markets = client.gamma.markets()
    .list()
    .open(true)
    .limit(10)
    .send()
    .await?;

// Get user positions via Data API
let positions = client.data
    .user("0x1234...")
    .list_positions()
    .send()
    .await?;

// Check balance via CLOB API (requires auth)
let balance = client.clob
    .account_api()?
    .balance_allowance("token_id")
    .send()
    .await?;
# Ok(())
# }
```

### WebSocket

The CLOB's sockets are behind the `polymarket-clob-ws` feature, which `prelude::ws` needs:

```bash
cargo add polyoxide --features polymarket-clob-ws
```

```rust,ignore
use polyoxide::prelude::*;
use futures_util::StreamExt;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
// Connect to market channel (no auth required)
let mut ws = ws::WebSocket::connect_market(vec![
    "token_id".to_string(),
]).await?;

while let Some(msg) = ws.next().await {
    match msg? {
        ws::Channel::Market(ws::MarketMessage::Book(book)) => {
            println!("Order book: {} bids, {} asks", book.bids.len(), book.asks.len());
        }
        ws::Channel::Market(ws::MarketMessage::PriceChange(pc)) => {
            println!("Price change: {:?}", pc.price_changes);
        }
        _ => {}
    }
}
# Ok(())
# }
```

### Gasless Redemptions (Relay)

The relay is behind the `polymarket-relay` feature, which the default does not turn on:

```bash
cargo add polyoxide --features polymarket-relay
```

```rust,ignore
use polyoxide::polymarket::relay::{RelayClient, BuilderAccount, BuilderConfig, WalletType};
use alloy::primitives::U256;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Option 1: Builder API credentials (HMAC-signed) ---
    let builder_config = BuilderConfig::new(
        std::env::var("BUILDER_API_KEY")?,
        std::env::var("BUILDER_SECRET")?,
        std::env::var("BUILDER_PASS_PHRASE").ok(),
    );
    let account = BuilderAccount::new(
        std::env::var("POLYMARKET_PRIVATE_KEY")?,
        Some(builder_config),
    )?;

    // --- Option 2: Relayer API key (simpler alternative) ---
    // let account = BuilderAccount::with_relayer_api_key(
    //     std::env::var("POLYMARKET_PRIVATE_KEY")?,
    //     std::env::var("RELAYER_API_KEY")?,
    //     std::env::var("RELAYER_API_KEY_ADDRESS")?,
    // )?;

    // Create relay client for Polygon mainnet
    let client = RelayClient::default_builder()?
        .with_account(account)
        .wallet_type(WalletType::Proxy)
        .build()?;

    // Submit gasless redemption with gas estimation
    let condition_id: [u8; 32] = /* your condition ID */;
    let index_sets = vec![U256::from(1), U256::from(2)];

    let response = client
        .submit_gasless_redemption_with_gas_estimation(condition_id, index_sets, true)
        .await?;

    println!("Transaction ID: {}", response.transaction_id);

    Ok(())
}
```

## License

Licensed under either of [MIT](./LICENSE-MIT) or [Apache-2.0](./LICENSE-APACHE) at your option.

## Acknowledgements

`polyoxide` is a hard fork of the [polyte](https://github.com/roushou/polyte) project helmed by Roushou.
