# polyoxide-perps

Rust client library for the Polymarket Perps API (perpetual futures).

Public market data: exchange and instrument reference data, tickers, order
books, klines, trades, funding, fees and leaderboards. No authentication
required for anything in this crate.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-perps/).

## Installation

```toml
[dependencies]
polyoxide-perps = "0.33"
```

## Usage

```no_run
use polyoxide_perps::Perps;

# async fn example() -> Result<(), polyoxide_perps::PerpsError> {
let perps = Perps::new()?;
let instruments = perps.exchange().instruments().send().await?;
for instrument in &instruments {
    println!("{} {}", instrument.instrument_id, instrument.symbol);
}
# Ok(())
# }
```
