# polyoxide-gamma

Rust client library for Polymarket Gamma (market data) API.

The Gamma API provides read-only access to Polymarket's market data, including markets, events, series, tags, and sports metadata.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-gamma/).

## Features

- **Type-Safe API**: Strongly-typed responses with serde deserialization
- **Fluent Builder Pattern**: Chainable methods for constructing queries
- **Comprehensive Coverage**: Support for markets, events, series, tags, sports, and comments

## Installation

```toml
[dependencies]
polyoxide-gamma = "0.1.0"
```

Or use the unified client:

```toml
[dependencies]
polyoxide = "0.1.0"
```

## Usage

### Basic Example

```rust
use polyoxide_gamma::Gamma;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gamma = Gamma::new()?;

    // List active markets
    let markets = gamma.markets()
        .list()
        .open(true)
        .limit(10)
        .send()
        .await?;

    for market in markets {
        println!("{}: {}", market.question, market.volume);
    }

    Ok(())
}
```

### Get Market by ID

```rust
let market = gamma.markets()
    .get("condition_id_here")
    .send()
    .await?;

println!("Market: {}", market.question);
println!("Volume: {}", market.volume);
println!("Liquidity: {}", market.liquidity);
```

### List Series

```rust
let series = gamma.series()
    .list()
    .open(true)
    .send()
    .await?;

for s in series {
    println!("{} - {} events", s.title, s.events.len());
}
```

### Get Tags

```rust
let tags = gamma.tags().list().send().await?;

for tag in tags {
    println!("{}: {}", tag.label, tag.slug);
}
```

## Configuration

```rust
let gamma = Gamma::builder()
    .base_url("https://gamma-api.polymarket.com")
    .timeout_ms(30_000)
    .pool_size(10)
    .build()?;
```

## API Coverage

- **Markets**: List, get by ID, with filtering (active, closed, archived)
- **Events**: List and get event data with nested markets
- **Series**: Tournament/season data with events
- **Tags**: Market categorization and related tags
- **Sports**: Sports metadata and information
- **Comments**: Market comments and discussions

## Examples

The crate includes several examples:

```bash
# List markets
cargo run --example retrieve_markets

# Get sports data
cargo run --example retrieve_sports

# Browse tags
cargo run --example retrieve_tags

# Explore series
cargo run --example retrieve_series
```

## License

Licensed under either of [MIT](../LICENSE-MIT) or [Apache-2.0](../LICENSE-APACHE) at your option.
