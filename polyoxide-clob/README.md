# polyoxide-clob

Rust client library for Polymarket CLOB (Central Limit Order Book) API.

The CLOB API enables trading operations on Polymarket, including order creation, signing, posting, and account management.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-clob/).

## Features

- **Account Management**: Check balances, allowances, and trade history
- **Order Management**: List and cancel orders
- **Market Data**: Get order books, prices, and market information
- **WebSocket**: Real-time market data and user order/trade updates

## Installation

```
cargo add polyoxide-clob
```

## Usage

### Setup

```rust
use polyoxide_clob::{Account, Chain, ClobBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load account from environment variables
    let account = Account::from_env()?;

    // Create CLOB client
    let clob = ClobBuilder::new()
        .with_account(account)
        .chain(Chain::PolygonMainnet)
        .build()?;

    Ok(())
}
```

#### Account Configuration

The `Account` abstraction provides multiple ways to load credentials:

```rust
use polyoxide_clob::{Account, Credentials};

// Option 1: From environment variables
// Reads: POLYMARKET_PRIVATE_KEY, POLYMARKET_API_KEY,
//        POLYMARKET_API_SECRET, POLYMARKET_API_PASSPHRASE
let account = Account::from_env()?;

// Option 2: From a JSON file
let account = Account::from_file("config/account.json")?;

// Option 3: Direct construction
let credentials = Credentials {
    key: "api_key".to_string(),
    secret: "api_secret".to_string(),
    passphrase: "passphrase".to_string(),
};
let account = Account::new("0x...", credentials)?;
```

### Place an Order

```rust
use polyoxide_clob::{CreateOrderParams, OrderSide};

let params = CreateOrderParams {
    token_id: "token_id_here".to_string(),
    price: 0.52,
    size: 100.0,
    side: OrderSide::Buy,
    expiration: None,
};

// Single method to create, sign, and post order
let response = clob.place_order(&params).await?;

if response.success {
    println!("Order placed: {:?}", response.order_id);
} else {
    eprintln!("Order failed: {:?}", response.error_msg);
}
```

### WebSocket

#### Market Channel

Subscribe to real-time order book and price updates (no authentication required):

```rust
use polyoxide_clob::ws::{WebSocket, Channel, MarketMessage};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = WebSocket::connect_market(vec![
        "asset_id".to_string(),
    ]).await?;

    while let Some(msg) = ws.next().await {
        match msg? {
            Channel::Market(MarketMessage::Book(book)) => {
                println!("Order book: {} bids, {} asks", book.bids.len(), book.asks.len());
            }
            Channel::Market(MarketMessage::PriceChange(pc)) => {
                println!("Price change: {:?}", pc.price_changes);
            }
            _ => {}
        }
    }

    Ok(())
}
```

#### User Channel

Subscribe to authenticated order and trade updates:

```rust
use polyoxide_clob::ws::{ApiCredentials, WebSocket, Channel, UserMessage};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credentials = ApiCredentials::from_env()?;

    let mut ws = WebSocket::connect_user(
        vec!["condition_id".to_string()],
        credentials,
    ).await?;

    while let Some(msg) = ws.next().await {
        match msg? {
            Channel::User(UserMessage::Order(order)) => {
                println!("Order update: {} {:?}", order.id, order.order_type);
            }
            Channel::User(UserMessage::Trade(trade)) => {
                println!("Trade: {} @ {}", trade.size, trade.price);
            }
            _ => {}
        }
    }

    Ok(())
}
```

#### Auto-Ping with WebSocketBuilder

For long-running connections with automatic keep-alive:

```rust
use polyoxide_clob::ws::{WebSocketBuilder, Channel};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ws = WebSocketBuilder::new()
        .ping_interval(Duration::from_secs(10))
        .connect_market(vec!["asset_id".to_string()])
        .await?;

    ws.run(|msg| async move {
        println!("Received: {:?}", msg);
        Ok(())
    }).await?;

    Ok(())
}
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
