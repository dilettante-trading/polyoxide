# polyoxide-data

Rust client library for the Polymarket Data API.

Read-only access to Polymarket user positions, trades, activity, holders, open interest, live volume, leaderboards, and builder analytics. No authentication required.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-data/).

## Features

- **Read-Only, No Auth**: All endpoints are public -- no API keys or signing needed
- **Fluent Builder Pattern**: Chainable query parameters with `.send().await?`
- **Type-Safe Responses**: Strongly-typed structs with serde deserialization
- **Built-In Rate Limiting**: Automatic rate limiting and concurrency control (default: 4 concurrent requests)

## Installation

```toml
[dependencies]
polyoxide-data = "0.14"
```

Or use the unified client:

```toml
[dependencies]
polyoxide = "0.14"
```

## Usage

### Create a Client

```rust
use polyoxide_data::DataApi;

// Default configuration
let data = DataApi::new()?;

// Or customize via the builder
let data = DataApi::builder()
    .timeout_ms(30_000)
    .pool_size(10)
    .max_concurrent(8)
    .build()?;
```

### User Positions

```rust
use polyoxide_data::types::PositionSortBy;

let positions = data.user("0x1234...abcd")
    .list_positions()
    .limit(10)
    .sort_by(PositionSortBy::CashPnl)
    .send()
    .await?;

for pos in positions {
    println!("{}: size={} pnl={}", pos.title, pos.size, pos.cash_pnl);
}
```

### User Position Value

```rust
let values = data.user("0x1234...abcd")
    .positions_value()
    .send()
    .await?;

for v in values {
    println!("Total value: {}", v.value);
}
```

### Closed Positions

```rust
let closed = data.user("0x1234...abcd")
    .closed_positions()
    .limit(5)
    .send()
    .await?;

for pos in closed {
    println!("{}: realized P&L = {}", pos.title, pos.realized_pnl);
}
```

### User Trades

```rust
use polyoxide_data::types::TradeSide;

let trades = data.user("0x1234...abcd")
    .trades()
    .side(TradeSide::Buy)
    .limit(20)
    .send()
    .await?;

for trade in trades {
    println!("{} {} @ {}", trade.title, trade.size, trade.price);
}
```

### User Activity

```rust
use polyoxide_data::types::ActivityType;

let activity = data.user("0x1234...abcd")
    .activity()
    .activity_type([ActivityType::Trade, ActivityType::Redeem])
    .limit(50)
    .send()
    .await?;

for a in activity {
    println!("{:?}: {} USDC", a.activity_type, a.usdc_size);
}
```

### Markets Traded Count

```rust
let traded = data.user("0x1234...abcd").traded().await?;
println!("Total markets traded: {}", traded.traded);
```

### Global Trades

```rust
let trades = data.trades()
    .list()
    .limit(10)
    .send()
    .await?;

for trade in trades {
    println!("{}: {} @ {}", trade.title, trade.size, trade.price);
}
```

### Top Holders

```rust
let holders = data.holders()
    .list(["condition_id_1", "condition_id_2"])
    .limit(50)
    .min_balance(100)
    .send()
    .await?;

for market in holders {
    println!("Token {}: {} holders", market.token, market.holders.len());
}
```

### Open Interest

```rust
let oi = data.open_interest()
    .get()
    .market(["condition_id_here"])
    .send()
    .await?;

for entry in oi {
    println!("{}: {}", entry.market, entry.value);
}
```

### Live Volume

```rust
let volumes = data.live_volume().get(12345).await?;

for vol in volumes {
    println!("Total volume: {}", vol.total);
}
```

### Trader Leaderboard

```rust
use polyoxide_data::api::leaderboard::{LeaderboardCategory, LeaderboardOrderBy};
use polyoxide_data::types::TimePeriod;

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
```

### Builder Leaderboard

```rust
use polyoxide_data::types::TimePeriod;

let rankings = data.builders()
    .leaderboard()
    .time_period(TimePeriod::Week)
    .limit(10)
    .send()
    .await?;

for b in rankings {
    println!("#{} {} - volume: {}", b.rank, b.builder, b.volume);
}
```

### Builder Volume Time Series

```rust
use polyoxide_data::types::TimePeriod;

let volumes = data.builders()
    .volume()
    .time_period(TimePeriod::Month)
    .send()
    .await?;

for entry in volumes {
    println!("{}: {} - volume {}", entry.dt, entry.builder, entry.volume);
}
```

### Health Check

```rust
let health = data.health().check().await?;
println!("Status: {}", health.data);

let latency = data.health().ping().await?;
println!("API latency: {}ms", latency.as_millis());
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
| `data.health()` | `.check()`, `.ping()` | API health and latency |

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
