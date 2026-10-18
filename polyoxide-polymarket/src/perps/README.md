# polyoxide-polymarket: `perps`

Rust client for the Polymarket Perps API (perpetual futures), the `perps` module of `polyoxide-polymarket`.

Public market data: exchange and instrument reference data, tickers, order
books, klines, trades, funding, fees and leaderboards. No authentication
required for anything in this module.

More information about this module can be found in the [crate documentation](https://docs.rs/polyoxide-polymarket/latest/polyoxide_polymarket/perps/).

## Installation

```toml
[dependencies]
polyoxide-polymarket = { version = "0.41", default-features = false, features = ["perps"] }
```

Add `perps-ws` for the WebSocket channels; it turns on `perps`.

## Usage

```no_run
use polyoxide_polymarket::perps::Perps;

# async fn example() -> Result<(), polyoxide_polymarket::perps::PerpsError> {
let perps = Perps::new()?;
let instruments = perps.exchange().instruments().send().await?;
for instrument in &instruments {
    println!("{} {}", instrument.instrument_id, instrument.symbol);
}
# Ok(())
# }
```

## Through the venue-neutral traits

`Perps` implements `polyoxide-venue`'s `MarketData`, `Instruments`, `Trades`,
`Candles` and `Funding`, keyed `polymarket.perps:<instrument id>`
(`shared::keys::perps`), so code written against those traits reads perps as it
reads Binance. Fee tiers, risk tiers, price bounds and perps' other facts ride
in `perps::venue`'s `<Record>Ext` types.
A `FundingRate` carries its `interval` from the instrument's `funding_interval`, which
costs one `/v1/info/instruments` request beside the rate or the settlements.
The `perps::venue` module's documentation has an example.

## Streaming

With the `perps-ws` feature, the public channels stream over one connection.
`PerpsWsBuilder` keeps the socket alive, detects a stall, and reconnects with
the same subscriptions; `Event::Reconnected` and `Event::SequenceRegressed`
tell a book consumer to resync.

```text
use futures_util::StreamExt;
use polyoxide_polymarket::perps::{types::InstrumentId, ws::{Channel, Event, PerpsWsBuilder, StreamDepth}};

let mut ws = PerpsWsBuilder::new()
    .connect([Channel::Book(InstrumentId(1), StreamDepth::Twenty), Channel::Trades(InstrumentId(1))])
    .await?;
while let Some(event) = ws.next().await {
    if let Event::Update(update) = event? {
        println!("{} @ {}", update.channel, update.ts);
    }
}
```
