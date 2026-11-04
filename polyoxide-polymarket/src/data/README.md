# polyoxide-polymarket: `data`

Rust client library for the Polymarket Data API, the `data` module of `polyoxide-polymarket`.

Read-only access to Polymarket wallet positions, PnL, stats and value, the trade and activity feeds, holders, open interest, live volume, price history, resolutions, tokens, leaderboards and builder analytics, through Data API v2. No authentication required. Polymarket retired Data API v1, and its `user-pnl-api` and `lb-api` hosts, on 2026-11-02.

More information about this module can be found in the [crate documentation](https://docs.rs/polyoxide-polymarket/latest/polyoxide_polymarket/data/).

## Features

- **Read-Only, No Auth**: All endpoints are public -- no API keys or signing needed
- **Fluent Builder Pattern**: Chainable query parameters with `.send().await?`
- **Type-Safe Responses**: Strongly-typed structs with serde deserialization
- **Built-In Rate Limiting**: Automatic rate limiting and concurrency control (default: 4 concurrent requests)

## Installation

```toml
[dependencies]
polyoxide-polymarket = { version = "0.47", default-features = false, features = ["data"] }
```

Or use the `polyoxide` umbrella, whose default `polymarket` feature includes the Data API, at
`polyoxide::polymarket::data`:

```toml
[dependencies]
polyoxide = "0.47"
```

## Usage

Every route is a method of `DataApi`, and its clones share one connection pool,
rate limiter and concurrency budget. The API wraps every response in `data`, pages by
cursor only, uses snake_case field names, and returns structured errors. A
missing or `null` number means the value was unavailable, never zero, so those
fields are `Option`s.

### Create a Client

```rust
use polyoxide_polymarket::data::DataApi;

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

### One page, and every page

```rust
# use polyoxide_polymarket::data::DataApi;
use futures_util::TryStreamExt;
use polyoxide_polymarket::data::types::PositionStatus;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
// One page. `next_cursor` is `None` on the last one.
let page = data.trades().user("0x1234...abcd").limit(100).send().await?;
for trade in &page.data {
    // `outcome_index` is `None` when upstream could not label the outcome.
    println!("{} {} @ {} (outcome {:?})", trade.side, trade.size, trade.price, trade.outcome_index.get());
}

// Every page. The filters are re-sent unchanged on each request.
let mut pages = data
    
    .positions("0x1234...abcd")
    .status(PositionStatus::Closed)
    .pages();
while let Some(page) = pages.try_next().await? {
    for position in page.data {
        println!("{}: realized {}", position.title, position.realized_pnl);
    }
}
# Ok(())
# }
```

### A wallet

```rust
# use polyoxide_polymarket::data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let user = "0x1234...abcd";
let value = data.value(user).send().await?;
println!("portfolio value: {}", value.value);

let activity = data.activity(user).limit(20).send().await?;
for row in &activity.data {
    println!("{} @ {}", row.outcome, row.price);
}
# Ok(())
# }
```

### A market

```rust
# use polyoxide_polymarket::data::DataApi;
# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
let holders = data.holders(["0xcondition..."]).limit(10).send().await?;
for token in &holders.data {
    println!("{}: {} holders", token.token_id, token.holders.len());
}
# Ok(())
# }
```

### Structured errors

```rust
# use polyoxide_polymarket::data::DataApi;
use polyoxide_polymarket::data::{DataError, ErrorCode};
use polyoxide_venue::Classify;

# async fn doctest() -> Result<(), Box<dyn std::error::Error>> {
# let data = DataApi::builder().build()?;
match data.user_stats("0x1234...abcd").send().await {
    Ok(Some(stats)) => println!("{} markets traded", stats.trades),
    Ok(None) => println!("not a known user"),
    Err(DataError::Venue(err)) if err.code == ErrorCode::InvalidRequest => {
        println!("bad {:?}: {} (trace {})", err.parameter, err.message, err.trace_id);
    }
    Err(err) if err.is_retriable() => println!("try again after {:?}", err.retry_after()),
    Err(err) => return Err(err.into()),
}
# Ok(())
# }
```

## License

Licensed under either of [MIT](../../../LICENSE-MIT) or [Apache-2.0](../../../LICENSE-APACHE) at your option.
