# polyoxide

The multi-venue umbrella for the polyoxide toolkit: each venue crate under its venue's name,
behind features named for the venue and its modules.

| Path | Crate | Features |
|------|-------|----------|
| `polyoxide::venue` | `polyoxide-venue`: the vocabulary every venue shares, such as the error classes and `Classify` | always on |
| `polyoxide::polymarket` | `polyoxide-polymarket`: [Polymarket](https://polymarket.com)'s CLOB, Gamma, Data, Relay, Perps, RTDS and Sports modules, and the unified `Polymarket` client | `polymarket` and `polymarket-<module>` |
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures market data and streams | `binance` and `binance-<module>` |
| `polyoxide::prelude` | A selection of the enabled modules' clients, errors and stream types | |

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide/).

## Installation

By default, Polymarket's three REST modules (CLOB, Gamma, Data) are enabled; every other module
is opt-in:

```bash
cargo add polyoxide
```

Select only what you need:

```bash
# Polymarket market data only
cargo add polyoxide --no-default-features --features polymarket-gamma

# Polymarket trading only
cargo add polyoxide --no-default-features --features polymarket-clob

# Polymarket user data only
cargo add polyoxide --no-default-features --features polymarket-data

# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws

# Every module of every venue, and the keychain
cargo add polyoxide --no-default-features --features full
```

## Feature flags

A `-ws` feature turns its module's own feature on.

| Feature | Default | Description |
|---------|---------|-------------|
| `polymarket` | yes | `polymarket-clob` + `polymarket-gamma` + `polymarket-data`, `polyoxide-polymarket`'s defaults |
| `polymarket-clob` | yes | CLOB order-book trading via `polyoxide-polymarket`'s `clob` module |
| `polymarket-clob-ws` | no | CLOB WebSocket streaming via `polyoxide-polymarket`'s `clob-ws` feature |
| `polymarket-gamma` | yes | Read-only market data via `polyoxide-polymarket`'s `gamma` module |
| `polymarket-data` | yes | Read-only user positions/trades via `polyoxide-polymarket`'s `data` module |
| `polymarket-relay` | no | Gasless transactions via `polyoxide-polymarket`'s `relay` module |
| `polymarket-perps` | no | Perps public market data via `polyoxide-polymarket`'s `perps` module |
| `polymarket-perps-ws` | no | Perps WebSocket streaming via `polyoxide-polymarket`'s `perps-ws` feature |
| `polymarket-rtds` | no | RTDS crypto price streams via `polyoxide-polymarket`'s `rtds` module |
| `polymarket-sports` | no | Live sports scores via `polyoxide-polymarket`'s `sports` module |
| `binance` | no | `binance-usdm`, `polyoxide-binance`'s defaults |
| `binance-usdm` | no | Binance USDⓈ-M futures market data via `polyoxide-binance`'s `usdm` module |
| `binance-usdm-ws` | no | Binance USDⓈ-M market streams via `polyoxide-binance`'s `usdm-ws` feature |
| `full` | no | Every module of every venue above, and `keychain` |
| `keychain` | no | OS keychain credential storage, for the venues that store credentials |

## Usage

### Unified client (all three REST modules enabled)

When `polymarket-clob`, `polymarket-gamma` and `polymarket-data` are all enabled, as by default, `polyoxide::polymarket::Polymarket` provides a single entry point, which the prelude re-exports. It requires an `Account` for authenticated CLOB operations, and its `build` fails with a `BuildError` naming the client that could not be built.

```rust
use polyoxide::prelude::*;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
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
# Ok(())
# }
```

### Using each module directly

Each module's client can be used standalone without the unified `Polymarket` wrapper.

```rust
// Read-only Gamma client (no auth required)
use polyoxide::polymarket::gamma::Gamma;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
let gamma = Gamma::builder().build()?;
let events = gamma.events().list().limit(5).send().await?;
# Ok(())
# }
```

```rust
// Read-only Data API client (no auth required)
use polyoxide::polymarket::data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
let data = DataApi::builder().build()?;
let leaders = data.leaderboard().get().send().await?;
# Ok(())
# }
```

```rust
// Public (unauthenticated) CLOB client for read-only market data
use polyoxide::polymarket::clob::Clob;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
let clob = Clob::public();
let book = clob.markets().order_book("token_id").send().await?;
# Ok(())
# }
```

### WebSocket (requires `polymarket-clob-ws`)

```rust,ignore
use polyoxide::prelude::*;
use futures_util::StreamExt;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
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
# Ok(())
# }
```

For long-running connections, `WebSocketBuilder` provides automatic keep-alive pings:

```rust,ignore
use polyoxide::prelude::*;
use std::time::Duration;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
let stream = ws::WebSocketBuilder::new()
    .ping_interval(Duration::from_secs(10))
    .connect_market(vec!["token_id".to_string()])
    .await?;

stream.run(|msg| async move {
    println!("{:?}", msg);
    Ok(())
}).await?;
# Ok(())
# }
```

## Environment variables

Authenticated Polymarket operations (CLOB trading) require:

| Variable | Description |
|----------|-------------|
| `POLYMARKET_PRIVATE_KEY` | Hex-encoded private key |
| `POLYMARKET_API_KEY` | L2 API key |
| `POLYMARKET_API_SECRET` | L2 API secret (base64) |
| `POLYMARKET_API_PASSPHRASE` | L2 API passphrase |

`Account::from_env()` reads all four. Alternatively, use `Account::new(private_key, credentials)` or `Account::from_file(path)` for file-based config. With the `keychain` feature enabled, `Account::from_keychain()` loads the same credentials from the OS keychain instead of environment variables.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
