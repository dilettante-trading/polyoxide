# polyoxide-polymarket: `perps`

Rust client for the Polymarket Perps API (perpetual futures), the `perps` module of `polyoxide-polymarket`.

Public market data: exchange and instrument reference data, tickers, order
books, klines, trades, funding, fees, leaderboards, trading competitions and
position share cards. No authentication required for anything in this module.

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

## Competitions and position snapshots

`perps.competitions()` reads the trading competitions: the list, one competition's
settings and prize table, its leaderboard, an account's standing and the live widget.
`perps.public().position_snapshots(address)` posts a selection of open positions and
past fills and returns share-card numbers and a small chart for each; without the
account's credentials `leverage` and `pnl_percent` come back `None`.

```no_run
use polyoxide_polymarket::perps::{api::public::PositionSnapshotFill, Perps};

# async fn example() -> Result<(), polyoxide_polymarket::perps::PerpsError> {
let perps = Perps::new()?;
let address = "0xE0Ebcff994a0f3A644634927aE1B68913ad58A39";
if !perps.public().registered(address).send().await?.registered {
    return Ok(());
}
let fills = perps
    .public()
    .position_fills(address, 2.into())
    .send()
    .await?;
let cards = perps
    .public()
    .position_snapshots(address)
    .history(fills.data.iter().take(4).map(PositionSnapshotFill::from))
    .send()
    .await?;
for result in &cards.history {
    println!("{:?} {:?}", result.trade_id, result.status);
}
let competitions = perps.competitions().list().send().await?;
println!("{} competitions", competitions.competitions.len());
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
