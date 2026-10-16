# polyoxide-polymarket: `clob`

Rust client for the Polymarket CLOB (Central Limit Order Book) API, the `clob` module of `polyoxide-polymarket`.

Provides authenticated order creation, EIP-712 signing, and submission, plus read-only market data and order book access. Part of the [polyoxide](https://github.com/dilettante-trading/polyoxide) workspace.

More information about this module can be found in the [crate documentation](https://docs.rs/polyoxide-polymarket/latest/polyoxide_polymarket/clob/).

## Features

- **Order Management**: Create, sign, and post limit and market orders with EIP-712
- **Market Data**: Order books, prices, midpoints, spreads, last trade prices, and price history
- **Account Management**: Balances, allowances, trade history, and session heartbeats
- **API Key Management**: Create, list, and delete standard, read-only, and builder API keys
- **Liquidity Rewards**: Query earnings, percentages, and reward markets
- **Notifications**: List and dismiss user notifications
- **WebSocket**: Real-time market and user channels (feature `clob-ws`). Live sports scores are in the [`sports`](../sports/README.md) module, feature `sports`.

## Installation

```toml
[dependencies]
polyoxide-polymarket = { version = "0.40", default-features = false, features = ["clob", "gamma"] }
```

`gamma` lets a proxy signature type find its proxy wallet through Gamma when an order names no
`funder`; with `clob` alone, such an order is refused before anything is sent.

### Feature Flags

These are `polyoxide-polymarket`'s features. Its defaults are `clob`, `gamma` and `data`.

| Feature | Default | Description |
|---------|---------|-------------|
| `clob` | Yes | This module: the client, order signing (`alloy`) and L2 authentication |
| `gamma` | Yes | With `clob`, auto-resolves proxy wallet addresses for proxy signature types through the `gamma` module |
| `clob-ws` | No | WebSocket support, `clob::ws` (the `polyoxide-ws` socket kit, `futures-util`), for real-time streaming; turns on `clob` |
| `keychain` | No | OS keychain storage for credentials via `keyring` (macOS Keychain, Windows Credential Manager, Linux Secret Service); it builds `keyring` with any HTTP module, since cargo cannot limit it to `clob` and `relay` |

```bash
# With WebSocket support
cargo add polyoxide-polymarket --no-default-features --features clob-ws

# With the proxy-wallet lookup through Gamma
cargo add polyoxide-polymarket --no-default-features --features clob,gamma
```

## Authentication

The CLOB API uses two authentication layers:

- **L1 (EIP-712)**: On-chain signing with a private key via `alloy`. Used for API key creation/derivation.
- **L2 (HMAC-SHA256)**: Signing with API credentials. Used for order management, account operations, and all authenticated endpoints.

Both layers are managed through the `Account` type.

### Environment Variables

```text
POLYMARKET_PRIVATE_KEY        # Hex-encoded private key
POLYMARKET_API_KEY            # L2 API key
POLYMARKET_API_SECRET         # L2 API secret (base64)
POLYMARKET_API_PASSPHRASE     # L2 API passphrase
```

## Usage

### Client Construction

```rust
# async fn doctest(credentials: polyoxide_polymarket::clob::Credentials) -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::{Account, Chain, Clob, ClobBuilder, Credentials};

// Read-only client (no authentication, market data only)
let clob = Clob::public();

// Authenticated client from Account
let account = Account::from_env()?;
let clob = Clob::from_account(account)?;

// Shorthand: private key + credentials directly
let clob = Clob::new("0xprivate_key", credentials)?;

// Full builder control
# let account = Account::from_env()?;
let clob = ClobBuilder::new()
    .with_account(account)
    .chain(Chain::PolygonMainnet)
    .base_url("https://clob.polymarket.com")
    .timeout_ms(30_000)
    .pool_size(10)
    .max_concurrent(16)       // default: 8
    .build()?;
# Ok(())
# }
```

### Account Configuration

```rust
# fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::{Account, Credentials};

// From environment variables
let account = Account::from_env()?;

// From a JSON file
let account = Account::from_file("config/account.json")?;

// From the OS keychain (feature `keychain`)
// let account = Account::from_keychain()?;

// Direct construction
let credentials = Credentials {
    key: "api_key".to_string(),
    secret: "api_secret".to_string(),
    passphrase: "passphrase".to_string(),
};
let account = Account::new("0x...", credentials)?;
# Ok(())
# }
```

### API Namespaces

The client organizes endpoints into namespaces. Public namespaces are always available; authenticated namespaces return `Result<_, ClobError>` and require an `Account`.

| Namespace | Access | Method |
|-----------|--------|--------|
| `markets()` | Public | `clob.markets()` |
| `health()` | Public | `clob.health()` |
| `public_rewards()` | Public | `clob.public_rewards()` |
| `orders()` | Authenticated | `clob.orders()?` |
| `account_api()` | Authenticated | `clob.account_api()?` |
| `auth()` | Authenticated | `clob.auth()?` |
| `rewards()` | Authenticated | `clob.rewards()?` |
| `notifications()` | Authenticated | `clob.notifications()?` |

`public_rewards()` covers the reward endpoints that are public upstream —
`/rewards/markets/current`, `/rewards/markets/{condition_id}`,
`/rewards/markets/multi`, and `/rebates/current`. The same four methods are
mirrored on `rewards()` for callers that already hold the authenticated
namespace.

### Market Data (public)

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::{Clob, OrderSide};

let clob = Clob::public();

// List markets (paginated)
let markets = clob.markets().list().send().await?;

// Get a single market by condition ID
let market = clob.markets().get("0xcondition_id").send().await?;

// Order book
let book = clob.markets().order_book("token_id").send().await?;
println!("{} bids, {} asks", book.bids.len(), book.asks.len());

// Price, midpoint, spread
let price = clob.markets().price("token_id", OrderSide::Buy).send().await?;
let mid = clob.markets().midpoint("token_id").send().await?;
let spread = clob.markets().spread("token_id").send().await?;

// Last trade price and price history
let last = clob.markets().last_trade_price("token_id").send().await?;
let history = clob.markets().prices_history("token_id").send().await?;

// Batch operations (multiple tokens at once)
use polyoxide_polymarket::clob::BookParams;
let params = vec![BookParams { token_id: "t1".into(), side: None }];
let books = clob.markets().order_books(&params).await?;
# Ok(())
# }
```

### Placing Orders (authenticated)

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::{Account, Clob, CreateOrderParams, OrderKind, OrderSide};

let account = Account::from_env()?;
let clob = Clob::from_account(account)?;

// place_order: create + sign + post in one call
let params = CreateOrderParams {
    token_id: "token_id".to_string(),
    price: 0.52,
    size: 100.0,
    side: OrderSide::Buy,
    order_type: OrderKind::Gtc,
    post_only: false,
    expiration: None,
    funder: None,
    signature_type: None,
};

let response = clob.place_order(&params, None).await?;
if response.success {
    println!("Order placed: {:?}", response.order_id);
}

// Or step-by-step: create, sign, then post
let order = clob.create_order(&params, None).await?;
let signed = clob.sign_order(&order).await?;
let response = clob.post_order(&signed, OrderKind::Gtc, false).await?;
# Ok(())
# }
```

### Order Management (authenticated)

```rust
# use polyoxide_polymarket::clob::Clob;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let clob = Clob::public();
// List your orders
let orders = clob.orders()?.list().send().await?;

// Get a specific order
let order = clob.orders()?.get("order_id").send().await?;

// Cancel a single order
let result = clob.orders()?.cancel("order_id").send().await?;

// Cancel multiple orders (up to 3000)
let result = clob.orders()?.cancel_many(vec!["id1".into(), "id2".into()]).await?;

// Cancel all open orders
let result = clob.orders()?.cancel_all().await?;

// Check reward scoring status
let scoring = clob.orders()?.is_scoring("order_id").send().await?;
# Ok(())
# }
```

### Account Operations (authenticated)

```rust
# use polyoxide_polymarket::clob::Clob;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let clob = Clob::public();
# let builder_code = "my_builder_code";
// Token balance and allowance
let bal = clob.account_api()?.balance_allowance("token_id").send().await?;

// USDC balance
let usdc = clob.account_api()?.usdc_balance().send().await?;

// Trade history with filters (maker_address is required)
let trades = clob.account_api()?.trades("0xmaker_address")
    .market("0xcondition_id")
    .after("1700000000")
    .send()
    .await?;

// Builder trades (builder_code is required)
let builder_trades = clob.account_api()?.builder_trades(builder_code).send().await?;
# Ok(())
# }
```

### Health and Latency

```rust
# use polyoxide_polymarket::clob::Clob;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
let clob = Clob::public();

// Measure API round-trip time
let latency = clob.health().ping().await?;
println!("Latency: {}ms", latency.as_millis());

// Server time
let time = clob.health().server_time().send().await?;
# Ok(())
# }
```

### WebSocket (feature `clob-ws`)

#### Market Channel

Subscribe to real-time order book and price updates (no authentication required):

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::ws::{WebSocket, Channel, MarketMessage};
use futures_util::StreamExt;

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
# Ok(())
# }
```

#### Gated market events

`best_bid_ask`, `new_market`, and `market_resolved` are withheld by the server
unless the subscription opts in. Without `with_custom_features()` you will
never observe them:

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::ws::{Channel, MarketMessage, MarketSubscriptionOptions, WebSocket};
use futures_util::StreamExt;

let mut ws = WebSocket::connect_market_with(
    vec!["asset_id".to_string()],
    MarketSubscriptionOptions::default().with_custom_features(),
).await?;

while let Some(msg) = ws.next().await {
    if let Channel::Market(MarketMessage::BestBidAsk(bba)) = msg? {
        println!("{} / {} (spread {})", bba.best_bid, bba.best_ask, bba.spread);
    }
}
# Ok(())
# }
```

`MarketMessage` and `Channel` are `#[non_exhaustive]`, so matches need a `_`
arm — upstream adds event types without a major release.

#### User Channel

Subscribe to authenticated order and trade updates:

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::ws::{ApiCredentials, WebSocket, Channel, UserMessage};
use futures_util::StreamExt;

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
# Ok(())
# }
```

#### Auto-Ping with WebSocketBuilder

For long-running connections with automatic keep-alive:

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::ws::{WebSocketBuilder, Channel};
use std::time::Duration;

let ws = WebSocketBuilder::new()
    .ping_interval(Duration::from_secs(10))
    .connect_market(vec!["asset_id".to_string()])
    .await?;

ws.run(|msg| async move {
    println!("Received: {:?}", msg);
    Ok(())
}).await?;
# Ok(())
# }
```

#### Changing membership on a live socket

Both channels accept subscription updates without reconnecting. On a plain
`WebSocket`, `subscribe_assets` / `unsubscribe_assets` (market channel) and
`subscribe_markets` / `unsubscribe_markets` (user channel) send the frame
directly. While `run` drives a `WebSocketWithPing`, take a `MembershipHandle`
**before** calling `run`:

```rust
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
use polyoxide_polymarket::clob::ws::WebSocketBuilder;
use std::time::Duration;

let ws = WebSocketBuilder::new()
    .ping_interval(Duration::from_secs(10))
    .connect_market(vec!["asset_id".to_string()])
    .await?;
let membership = ws.membership();
tokio::spawn(ws.run(|_msg| async { Ok(()) }));

// Later — the venue answers with a `book` snapshot for the new asset.
membership.subscribe_assets(vec!["another_asset_id".to_string()]).await?;
# Ok(())
# }
```

An asset already on the socket gets no snapshot on a repeated subscribe;
unsubscribe it first to force one. See `MarketSubscriptionUpdate` for the
verified venue behaviour.

## Through the venue-neutral traits

`Clob` implements `polyoxide-venue`'s `MarketData` and `Candles` (from
`/prices-history`, one price per interval), and with the `data` feature
`Trades`, since the CLOB serves no public trades and the Data API does, and with
`gamma` `EventGroups`. A key names one outcome token,
`polymarket.clob:<token id>` (`shared::keys::clob`), and its book is bids and
asks in that outcome's own price terms, best first. Neg risk, the book's tick
size and the CLOB's other facts ride in `clob::venue`'s `<Record>Ext` types.
The `clob::venue` module's documentation has an example.

## License

Licensed under either of [MIT](../../../LICENSE-MIT) or [Apache-2.0](../../../LICENSE-APACHE) at your option.
