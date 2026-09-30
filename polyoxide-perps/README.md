# polyoxide-perps

Rust client library for the Polymarket Perps API (perpetual futures).

Public market data: exchange and instrument reference data, tickers, order
books, klines, trades, funding, fees and leaderboards. No authentication
required for anything in this crate.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-perps/).

## Installation

```toml
[dependencies]
polyoxide-perps = "0.35"
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

## Streaming

With the `ws` feature, the public channels stream over one connection.
`PerpsWsBuilder` keeps the socket alive, detects a stall, and reconnects with
the same subscriptions; `Event::Reconnected` and `Event::SequenceRegressed`
tell a book consumer to resync.

```text
use futures_util::StreamExt;
use polyoxide_perps::{types::InstrumentId, ws::{Channel, Event, PerpsWsBuilder, StreamDepth}};

let mut ws = PerpsWsBuilder::new()
    .connect([Channel::Book(InstrumentId(1), StreamDepth::Twenty), Channel::Trades(InstrumentId(1))])
    .await?;
while let Some(event) = ws.next().await {
    if let Event::Update(update) = event? {
        println!("{} @ {}", update.channel, update.ts);
    }
}
```
