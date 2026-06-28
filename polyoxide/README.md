# polyoxide

Unified Rust client for [Polymarket](https://polymarket.com) APIs, re-exporting the CLOB (trading), Gamma (market data), and Data (user positions/trades) crates behind feature flags.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide/).

## Installation

By default, all three REST API modules are enabled:

```
cargo add polyoxide
```

Select only what you need:

```
# Market data only
cargo add polyoxide --no-default-features --features gamma

# Trading only
cargo add polyoxide --no-default-features --features clob

# User data only
cargo add polyoxide --no-default-features --features data

# Everything including WebSocket
cargo add polyoxide --no-default-features --features full
```

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `clob`  | yes | CLOB order-book trading via `polyoxide-clob` |
| `gamma` | yes | Read-only market data via `polyoxide-gamma` |
| `data`  | yes | Read-only user positions/trades via `polyoxide-data` |
| `ws`    | no  | WebSocket streaming (implies `clob`) |
| `full`  | no  | Enables `clob` + `gamma` + `data` + `ws` |

## Usage

### Unified client (all three features enabled)

When `clob`, `gamma`, and `data` are all enabled, the `Polymarket` struct provides a single entry point. It requires an `Account` for authenticated CLOB operations.

```rust
use polyoxide::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let account = Account::from_env()?;

    let pm = Polymarket::builder(account)
        .chain(Chain::PolygonMainnet)
        .build()?;

    // Gamma: list open markets
    let markets = pm.gamma.markets()
        .list()
        .open(true)
        .limit(10)
        .send()
        .await?;

    // Data: list a user's positions
    let positions = pm.data
        .user("0xabc...")
        .list_positions()
        .limit(5)
        .send()
        .await?;

    for pos in &positions {
        println!("{}: {} shares @ {}", pos.title, pos.size, pos.cur_price);
    }

    // CLOB: place a limit order
    let params = CreateOrderParams {
        token_id: "token_id".into(),
        price: 0.52,
        size: 100.0,
        side: OrderSide::Buy,
        order_type: OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: Some(SignatureType::PolyProxy),
    };

    let response = pm.clob.place_order(&params, None).await?;
    println!("Order placed: {:?}", response);

    Ok(())
}
```

### Using individual crates directly

Each sub-client can be used standalone without the unified `Polymarket` wrapper.

```rust
// Read-only Gamma client (no auth required)
use polyoxide::polyoxide_gamma::Gamma;

let gamma = Gamma::builder().build()?;
let events = gamma.events().list().limit(5).send().await?;
```

```rust
// Read-only Data API client (no auth required)
use polyoxide::polyoxide_data::DataApi;

let data = DataApi::builder().build()?;
let leaders = data.leaderboard().get().send().await?;
```

```rust
// Public (unauthenticated) CLOB client for read-only market data
use polyoxide::polyoxide_clob::Clob;

let clob = Clob::public();
let book = clob.markets().order_book("token_id").send().await?;
```

### WebSocket (requires `ws` feature)

```rust
use polyoxide::prelude::*;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = ws::WebSocket::connect_market(vec![
        "token_id".to_string(),
    ]).await?;

    while let Some(msg) = stream.next().await {
        match msg? {
            ws::Channel::Market(ws::MarketMessage::Book(book)) => {
                println!("Book: {} bids, {} asks", book.bids.len(), book.asks.len());
            }
            ws::Channel::Market(ws::MarketMessage::PriceChange(pc)) => {
                println!("Price changes: {:?}", pc.price_changes);
            }
            _ => {}
        }
    }

    Ok(())
}
```

For long-running connections, `WebSocketBuilder` provides automatic keep-alive pings:

```rust
use polyoxide::prelude::*;
use std::time::Duration;

let stream = ws::WebSocketBuilder::new()
    .ping_interval(Duration::from_secs(10))
    .connect_market(vec!["token_id".to_string()])
    .await?;

stream.run(|msg| async move {
    println!("{:?}", msg);
    Ok(())
}).await?;
```

## Environment variables

Authenticated operations (CLOB trading) require:

| Variable | Description |
|----------|-------------|
| `POLYMARKET_PRIVATE_KEY` | Hex-encoded private key |
| `POLYMARKET_API_KEY` | L2 API key |
| `POLYMARKET_API_SECRET` | L2 API secret (base64) |
| `POLYMARKET_API_PASSPHRASE` | L2 API passphrase |

`Account::from_env()` reads all four. Alternatively, use `Account::new(private_key, credentials)` or `Account::from_file(path)` for file-based config.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
