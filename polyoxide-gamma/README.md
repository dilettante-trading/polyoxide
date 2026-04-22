# polyoxide-gamma

Read-only Rust client for the [Polymarket Gamma](https://gamma-api.polymarket.com) market data API. No authentication required.

[![crates.io](https://img.shields.io/crates/v/polyoxide-gamma.svg)](https://crates.io/crates/polyoxide-gamma)
[![docs.rs](https://docs.rs/polyoxide-gamma/badge.svg)](https://docs.rs/polyoxide-gamma)

## Installation

```toml
[dependencies]
polyoxide-gamma = "0.13"
```

Or use the unified `polyoxide` crate (includes Gamma by default):

```toml
[dependencies]
polyoxide = "0.13"
```

## Quick start

```rust
use polyoxide_gamma::Gamma;

#[tokio::main]
async fn main() -> Result<(), polyoxide_gamma::GammaError> {
    let gamma = Gamma::builder().build()?;

    // List open markets, sorted by volume
    let markets = gamma.markets()
        .list()
        .open(true)
        .order("volume")
        .ascending(false)
        .limit(10)
        .send()
        .await?;

    for m in &markets {
        println!("{}: vol={}, liq={}", m.question, m.volume, m.liquidity);
    }

    Ok(())
}
```

## Builder configuration

`Gamma::builder()` returns a `GammaBuilder` with sensible defaults. Every setting is optional:

```rust
use polyoxide_gamma::Gamma;
use polyoxide_core::RetryConfig;

let gamma = Gamma::builder()
    .base_url("https://gamma-api.polymarket.com")  // default
    .timeout_ms(30_000)           // request timeout
    .pool_size(10)                // HTTP connection pool
    .max_concurrent(4)            // in-flight request cap (prevents Cloudflare 1015)
    .with_retry_config(RetryConfig {
        max_retries: 3,
        initial_backoff_ms: 500,
        max_backoff_ms: 10_000,
    })
    .build()?;
```

`Gamma::new()` is shorthand for `Gamma::builder().build()`.

## API namespaces

All endpoints are read-only. Queries use a fluent builder pattern: chain filters, then call `.send().await?`.

### Markets

```rust
// List with filters
let markets = gamma.markets().list()
    .open(true)
    .limit(25)
    .volume_num_min(1000.0)
    .tag_id(42)
    .send().await?;

// Get by condition ID or slug
let market = gamma.markets().get("condition_id").send().await?;
let market = gamma.markets().get_by_slug("will-x-happen").include_tag(true).send().await?;

// Get tags for a market
let tags = gamma.markets().tags("condition_id").send().await?;
```

`ListMarkets` supports: `limit`, `offset`, `order`, `ascending`, `id`, `slug`, `clob_token_ids`, `condition_ids`, `market_maker_address`, `liquidity_num_min/max`, `volume_num_min/max`, `start_date_min/max`, `end_date_min/max`, `tag_id`, `related_tags`, `cyom`, `uma_resolution_status`, `game_id`, `sports_market_types`, `rewards_min_size`, `question_ids`, `include_tag`, `closed`, `open`, `archived`.

### Events

```rust
// List active events
let events = gamma.events().list()
    .active(true)
    .limit(10)
    .send().await?;

// Get by ID or slug
let event = gamma.events().get("event_id").include_chat(true).send().await?;
let event = gamma.events().get_by_slug("slug").send().await?;

// Tags, tweet count, comment count
let tags    = gamma.events().tags("event_id").send().await?;
let tweets  = gamma.events().tweet_count("event_id").send().await?;
let comments = gamma.events().comment_count("event_id").send().await?;
```

### Series

```rust
let series_list = gamma.series().list()
    .closed(false)
    .categories_labels(vec!["Sports"])
    .send().await?;

let series = gamma.series().get("series_id").include_chat(true).send().await?;
```

### Tags

```rust
let tags = gamma.tags().list()
    .limit(50)
    .is_carousel(true)
    .send().await?;

let tag = gamma.tags().get("42").send().await?;
let tag = gamma.tags().get_by_slug("politics").send().await?;

// Related tags with filters
let related = gamma.tags().get_related("42")
    .omit_empty(true)
    .status("active")
    .send().await?;

// Related tags with full event data
let detailed = gamma.tags().get_related_detailed("42").send().await?;
```

### Comments

```rust
let comments = gamma.comments().list()
    .parent_entity_type("Event")
    .parent_entity_id(42)
    .holders_only(true)
    .limit(20)
    .send().await?;

let comment = gamma.comments().get("comment_id").send().await?;
let user_comments = gamma.comments().by_user("0xaddress").send().await?;
```

### Sports

```rust
let sports = gamma.sports().list().send().await?;
let market_types = gamma.sports().market_types().send().await?;

let teams = gamma.sports().list_teams()
    .league(vec!["NFL"])
    .limit(50)
    .send().await?;
```

### Search

```rust
use polyoxide_gamma::api::search::SearchResponse;

let results: SearchResponse = gamma.search()
    .public_search("bitcoin")
    .search_profiles(true)
    .search_tags(true)
    .limit_per_type(10)
    .events_status("active")
    .send().await?;

// results.events, results.profiles, results.tags
```

### User

```rust
use polyoxide_gamma::api::user::UserResponse;

let profile: UserResponse = gamma.user()
    .get("0xsigner_address")
    .send().await?;
```

### Health

```rust
let latency = gamma.health().ping().await?;
println!("API latency: {}ms", latency.as_millis());
```

## Feature flags

| Flag | Effect |
|------|--------|
| `specta` | Derives `specta::Type` on response types for TypeScript bindings |

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
