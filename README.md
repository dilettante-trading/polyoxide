# Polyoxide

Rust SDK toolkit for Polymarket APIs. It includes library crates for use in your projects and a standalone CLI.

> [!WARNING]
> This is currently work-in-progress so the API may change and some features may be missing

## Crates

| Crate | Description |
|-------|-------------|
| [polyoxide](./polyoxide) | Unified client for Polymarket APIs (CLOB, Gamma, Data, WebSocket) |
| [polyoxide-cli](./polyoxide-cli) | CLI tool for querying Polymarket APIs |
| [polyoxide-clob](./polyoxide-clob) | Client library for Polymarket CLOB (order book) API |
| [polyoxide-core](./polyoxide-core) | Core utilities and shared types |
| [polyoxide-data](./polyoxide-data) | Client library for Polymarket Data API |
| [polyoxide-gamma](./polyoxide-gamma) | Client library for Polymarket Gamma (market data) API |
| [polyoxide-relay](./polyoxide-relay) | Client library for Polymarket Relayer API (gasless transactions) |

## Installation

### Libraries

```
cargo add polyoxide
```

Or install individual APIs:

```
# Market data only
cargo add polyoxide --no-default-features --features gamma

# Trading only
cargo add polyoxide --no-default-features --features clob

# Data API only
cargo add polyoxide --no-default-features --features data

# WebSocket only
cargo add polyoxide --no-default-features --features ws
```

### CLI

Install using cargo

```
cargo install polyoxide-cli
```

Or download binaries directly from Github releases

```
curl -fsSL https://raw.githubusercontent.com/dilettante-trading/polyoxide/main/scripts/install.sh | sh
```

See more information [here](./polyoxide-cli/README.md).

## Usage

### REST API

```rust
use polyoxide::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load account from environment variables
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

    Ok(())
}
```

### WebSocket

```rust
use polyoxide::prelude::*;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    Ok(())
}
```

### Gasless Redemptions (Relay)

```rust
use polyoxide_relay::{RelayClient, BuilderAccount, BuilderConfig, WalletType};
use alloy::primitives::U256;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup account with builder credentials
    let builder_config = BuilderConfig::new(
        std::env::var("BUILDER_API_KEY")?,
        std::env::var("BUILDER_SECRET")?,
        std::env::var("BUILDER_PASS_PHRASE").ok(),
    );
    let account = BuilderAccount::new(
        std::env::var("POLYMARKET_PRIVATE_KEY")?,
        Some(builder_config),
    )?;

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
