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

```text
use polyoxide_perps::Perps;

let perps = Perps::new()?;
let instruments = perps.exchange().instruments().send().await?;
```

The usage block becomes a `no_run` doctest in Task 4, once `exchange()` exists.
