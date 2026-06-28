# polyoxide-clob

Rust client library for Polymarket CLOB (Central Limit Order Book) API.

Provides authenticated order creation, EIP-712 signing, and submission, plus read-only market data and order book access. Part of the [polyoxide](https://github.com/DilettanteTrading/polyoxide) workspace.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-clob/).

## Features

- **Order Management**: Create, sign, and post limit and market orders with EIP-712
- **Market Data**: Order books, prices, midpoints, spreads, last trade prices, and price history
- **Account Management**: Balances, allowances, trade history, and session heartbeats
- **API Key Management**: Create, list, and delete standard, read-only, and builder API keys
- **Liquidity Rewards**: Query earnings, percentages, and reward markets
- **Notifications**: List and dismiss user notifications
- **WebSocket**: Real-time market data and user order/trade updates (feature-gated)

## Installation

```
cargo add polyoxide-clob
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `gamma` | Yes | Enables the `polyoxide-gamma` dependency, used to auto-resolve proxy wallet addresses for proxy signature types |
| `ws` | No | Enables WebSocket support (`tokio-tungstenite`, `futures-util`) for real-time streaming |
| `keychain` | No | Enables OS keychain storage for credentials via `keyring` (macOS Keychain, Windows Credential Manager, Linux Secret Service) |

```
# With WebSocket support
cargo add polyoxide-clob --features ws

# Without the gamma dependency
cargo add polyoxide-clob --no-default-features
```

## Authentication

The CLOB API uses two authentication layers:

- **L1 (EIP-712)**: On-chain signing with a private key via `alloy`. Used for API key creation/derivation.
- **L2 (HMAC-SHA256)**: Signing with API credentials. Used for order management, account operations, and all authenticated endpoints.

Both layers are managed through the `Account` type.

### Environment Variables

```
POLYMARKET_PRIVATE_KEY        # Hex-encoded private key
POLYMARKET_API_KEY            # L2 API key
POLYMARKET_API_SECRET         # L2 API secret (base64)
POLYMARKET_API_PASSPHRASE     # L2 API passphrase
```

## Usage

### Client Construction

```rust
use polyoxide_clob::{Account, Chain, Clob, ClobBuilder, Credentials};

// Read-only client (no authentication, market data only)
let clob = Clob::public();

// Authenticated client from Account
let account = Account::from_env()?;
let clob = Clob::from_account(account)?;

// Shorthand: private key + credentials directly
let clob = Clob::new("0xprivate_key", credentials)?;

// Full builder control
let clob = ClobBuilder::new()
    .with_account(account)
    .chain(Chain::PolygonMainnet)
    .base_url("https://clob.polymarket.com")
    .timeout_ms(30_000)
    .pool_size(10)
    .max_concurrent(16)       // default: 8
    .build()?;
```

### Account Configuration

```rust
use polyoxide_clob::{Account, Credentials};

// From environment variables
let account = Account::from_env()?;

// From a JSON file
let account = Account::from_file("config/account.json")?;

// From the OS keychain (feature `keychain`)
let account = Account::from_keychain()?;

// Direct construction
let credentials = Credentials {
    key: "api_key".to_string(),
    secret: "api_secret".to_string(),
    passphrase: "passphrase".to_string(),
};
let account = Account::new("0x...", credentials)?;
```

### API Namespaces

The client organizes endpoints into namespaces. Public namespaces are always available; authenticated namespaces return `Result<_, ClobError>` and require an `Account`.

| Namespace | Access | Method |
|-----------|--------|--------|
| `markets()` | Public | `clob.markets()` |
| `health()` | Public | `clob.health()` |
| `orders()` | Authenticated | `clob.orders()?` |
| `account_api()` | Authenticated | `clob.account_api()?` |
| `auth()` | Authenticated | `clob.auth()?` |
| `rewards()` | Authenticated | `clob.rewards()?` |
| `notifications()` | Authenticated | `clob.notifications()?` |

### Market Data (public)

```rust
use polyoxide_clob::{Clob, OrderSide};

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
use polyoxide_clob::BookParams;
let params = vec![BookParams { token_id: "t1".into(), side: None }];
let books = clob.markets().order_books(&params).await?;
```

### Placing Orders (authenticated)

```rust
use polyoxide_clob::{Account, Clob, CreateOrderParams, OrderKind, OrderSide};

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
```

### Order Management (authenticated)

```rust
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
```

### Account Operations (authenticated)

```rust
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

// Builder trades
let builder_trades = clob.account_api()?.builder_trades().send().await?;
```

### Health and Latency

```rust
let clob = Clob::public();

// Measure API round-trip time
let latency = clob.health().ping().await?;
println!("Latency: {}ms", latency.as_millis());

// Server time
let time = clob.health().server_time().send().await?;
```

### WebSocket (feature `ws`)

#### Market Channel

Subscribe to real-time order book and price updates (no authentication required):

```rust
use polyoxide_clob::ws::{WebSocket, Channel, MarketMessage};
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
```

#### User Channel

Subscribe to authenticated order and trade updates:

```rust
use polyoxide_clob::ws::{ApiCredentials, WebSocket, Channel, UserMessage};
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
```

#### Auto-Ping with WebSocketBuilder

For long-running connections with automatic keep-alive:

```rust
use polyoxide_clob::ws::{WebSocketBuilder, Channel};
use std::time::Duration;

let ws = WebSocketBuilder::new()
    .ping_interval(Duration::from_secs(10))
    .connect_market(vec!["asset_id".to_string()])
    .await?;

ws.run(|msg| async move {
    println!("Received: {:?}", msg);
    Ok(())
}).await?;
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
