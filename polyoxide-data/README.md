# polyoxide-data

Rust client library for the Polymarket Data API.

Read-only access to Polymarket user positions, trades, activity, holders, open interest, live volume, leaderboards, builder analytics, market-positions, combinatorial positions, and accounting snapshots. No authentication required.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-data/).

## Features

- **Read-Only, No Auth**: All endpoints are public -- no API keys or signing needed
- **Fluent Builder Pattern**: Chainable query parameters with `.send().await?`
- **Type-Safe Responses**: Strongly-typed structs with serde deserialization
- **Built-In Rate Limiting**: Automatic rate limiting and concurrency control (default: 4 concurrent requests)

## Installation

```toml
[dependencies]
polyoxide-data = "0.26"
```

Or use the unified client:

```toml
[dependencies]
polyoxide = "0.26"
```

## Usage

### Create a Client

```rust
use polyoxide_data::DataApi;

# fn doctest() -> Result<(), Box<dyn std::error::Error>> {
// Default configuration
let data = DataApi::new()?;

// Or customize via the builder
let data = DataApi::builder()
    .timeout_ms(30_000)
    .pool_size(10)
    .max_concurrent(8)
    .build()?;
# let _ = data;
# Ok(())
# }
```

### User Positions

```rust
# use polyoxide_data::DataApi;
use polyoxide_data::types::PositionSortBy;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let positions = data.user("0x1234...abcd")
    .list_positions()
    .limit(10)
    .sort_by(PositionSortBy::CashPnl)
    .send()
    .await?;

for pos in positions {
    println!("{}: size={} pnl={}", pos.title, pos.size, pos.cash_pnl);
}
# Ok(())
# }
```

### User Position Value

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let values = data.user("0x1234...abcd")
    .positions_value()
    .send()
    .await?;

for v in values {
    println!("Total value: {}", v.value);
}
# Ok(())
# }
```

### Closed Positions

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let closed = data.user("0x1234...abcd")
    .closed_positions()
    .limit(5)
    .send()
    .await?;

for pos in closed {
    println!("{}: realized P&L = {}", pos.title, pos.realized_pnl);
}
# Ok(())
# }
```

### User Trades

```rust
# use polyoxide_data::DataApi;
use polyoxide_data::types::TradeSide;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let trades = data.user("0x1234...abcd")
    .trades()
    .side(TradeSide::Buy)
    .limit(20)
    .send()
    .await?;

for trade in trades {
    println!("{} {} @ {}", trade.title, trade.size, trade.price);
}
# Ok(())
# }
```

### User Activity

```rust
# use polyoxide_data::DataApi;
use polyoxide_data::types::ActivityType;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let activity = data.user("0x1234...abcd")
    .activity()
    .activity_type([ActivityType::Trade, ActivityType::Redeem])
    .limit(50)
    .send()
    .await?;

for a in activity {
    println!("{:?}: {} USDC", a.activity_type, a.usdc_size);
}
# Ok(())
# }
```

### Markets Traded Count

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let traded = data.user("0x1234...abcd").traded().await?;
println!("Total markets traded: {}", traded.traded);
# Ok(())
# }
```

### Global Trades

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let trades = data.trades()
    .list()
    .limit(10)
    .send()
    .await?;

for trade in trades {
    println!("{}: {} @ {}", trade.title, trade.size, trade.price);
}
# Ok(())
# }
```

### Top Holders

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let holders = data.holders()
    .list(["condition_id_1", "condition_id_2"])
    .limit(50)
    .min_balance(100)
    .send()
    .await?;

for market in holders {
    println!("Token {}: {} holders", market.token, market.holders.len());
}
# Ok(())
# }
```

### Open Interest

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let oi = data.open_interest()
    .get()
    .market(["condition_id_here"])
    .send()
    .await?;

for entry in oi {
    println!("{}: {}", entry.market, entry.value);
}
# Ok(())
# }
```

### Live Volume

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let volumes = data.live_volume().get(12345).await?;

for vol in volumes {
    println!("Total volume: {}", vol.total);
}
# Ok(())
# }
```

### Trader Leaderboard

```rust
# use polyoxide_data::DataApi;
use polyoxide_data::api::leaderboard::{LeaderboardCategory, LeaderboardOrderBy};
use polyoxide_data::types::TimePeriod;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let rankings = data.leaderboard()
    .get()
    .category(LeaderboardCategory::Politics)
    .time_period(TimePeriod::Week)
    .order_by(LeaderboardOrderBy::Pnl)
    .limit(10)
    .send()
    .await?;

for trader in rankings {
    println!("#{} {} - PnL: {}", trader.rank, trader.proxy_wallet, trader.pnl);
}
# Ok(())
# }
```

### Builder Leaderboard

```rust
# use polyoxide_data::DataApi;
use polyoxide_data::types::TimePeriod;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let rankings = data.builders()
    .leaderboard()
    .time_period(TimePeriod::Week)
    .limit(10)
    .send()
    .await?;

for b in rankings {
    println!("#{} {} - volume: {}", b.rank, b.builder, b.volume);
}
# Ok(())
# }
```

### Builder Volume Time Series

```rust
# use polyoxide_data::DataApi;
use polyoxide_data::types::TimePeriod;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let volumes = data.builders()
    .volume()
    .time_period(TimePeriod::Month)
    .send()
    .await?;

for entry in volumes {
    println!("{}: {} - volume {}", entry.dt, entry.builder, entry.volume);
}
# Ok(())
# }
```

### Health Check

```rust
# use polyoxide_data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let health = data.health().check().await?;
println!("Status: {}", health.data);

let latency = data.health().ping().await?;
println!("API latency: {}ms", latency.as_millis());
# Ok(())
# }
```

## API Namespaces

| Namespace | Access | Description |
|-----------|--------|-------------|
| `data.user(addr)` | `.list_positions()`, `.positions_value()`, `.closed_positions()`, `.trades()`, `.activity()`, `.traded()` | Per-user positions, trades, and activity |
| `data.trades()` | `.list()` | Global trade history with filtering |
| `data.holders()` | `.list(markets)` | Top token holders per market |
| `data.open_interest()` | `.get()` | Market open interest |
| `data.live_volume()` | `.get(event_id)` | Real-time event volume |
| `data.leaderboard()` | `.get()` | Trader rankings by PnL or volume |
| `data.builders()` | `.leaderboard()`, `.volume()` | Builder analytics and rankings |
| `data.market_positions()` | `.list(market)` | Aggregated positions for a market |
| `data.accounting()` | `.snapshot(addr)` | Accounting snapshot (ZIP bytes) |
| `data.combos()` | `.positions(addr)`, `.activity(addr)` | Combinatorial (multi-market) positions and lifecycle events |
| `data.misc()` | `.other_size(event_id, addr)`, `.revisions(question_id)` | Neg-risk "Other" size and moderated question revisions |
| `data.pnl()` | `.history(addr)` | PnL time series (`user-pnl-api` host) |
| `data.rankings()` | `.volume()`, `.profit()` | Trader rankings by volume or profit (`lb-api` host) |
| `data.health()` | `.check()`, `.ping()` | API health and latency |

### Undocumented hosts

`data.pnl()` and `data.rankings()` call sibling hosts that Polymarket publishes
no OpenAPI spec for. They are verified live and covered by `#[ignore]`d
integration tests, but carry no upstream compatibility guarantee — expect them
to change more freely than the rest of this crate. Both base URLs are
configurable:

```rust,no_run
use polyoxide_data::DataApi;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let data = DataApi::builder()
    .pnl_base_url("https://user-pnl-api.polymarket.com")
    .rankings_base_url("https://lb-api.polymarket.com")
    .build()?;
# Ok(())
# }
```

`data.rankings()` is a different service from `data.leaderboard()`, which calls
`/v1/leaderboard` on the main Data API host and returns a different shape.

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
