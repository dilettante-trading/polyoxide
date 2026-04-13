# Polyoxide

Rust client for Polymarket APIs.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide/).

## Crates

| Crate | Description |
|-------|-------------|
| [polyoxide](./) | Unified client for Polymarket APIs (CLOB, Gamma, Data, WebSocket) |
| [polyoxide-cli](../polyoxide-cli) | CLI tool for querying Polymarket APIs |
| [polyoxide-clob](../polyoxide-clob) | Client library for Polymarket CLOB (order book) API |
| [polyoxide-core](../polyoxide-core) | Core utilities and shared types |
| [polyoxide-data](../polyoxide-data) | Client library for Polymarket Data API |
| [polyoxide-gamma](../polyoxide-gamma) | Client library for Polymarket Gamma (market data) API |

## Installation

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

    // Get markets
    let markets = client.gamma.markets().list().send().await?;

    // Get balance
    let balance = client.clob.balance_allowance().await?;

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

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
