# polyoxide-binance: `spot`

Rust client for Binance spot public market data (`api.binance.com`), the `spot` module of
`polyoxide-binance`: liveness, the exchange listing with its symbols and filters, and the market
routes in the `market()` namespace: order book depth, trades, aggregate trades, klines, the
average price and the tickers. No credentials are needed.

Every request is charged against a `WeightBudget`, because Binance limits each IP by request
*weight*, which varies with the route and its parameters, rather than by request count. The
budget is 5400 of the published 6000 per minute, follows the server's own count from the
`X-MBX-USED-WEIGHT-1M` header, and a `429` or `418` holds every request until the server's wait
is over. The venue's `RAW_REQUESTS` limit is not modelled: every request weighs at least 1, so
the budget keeps a client under it.

More information about this module can be found in the [crate documentation](https://docs.rs/polyoxide-binance/latest/polyoxide_binance/spot/).

## Installation

```toml
[dependencies]
polyoxide-binance = "0.44"
```

`spot` is on by default, and builds without `usdm`.

## Usage

```no_run
use polyoxide_binance::spot::{
    types::{Interval, Symbol},
    Spot,
};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let spot = Spot::new()?;

let latency = spot.health().ping().await?;
println!("ping {latency:?}");

let btc = Symbol::new("BTCUSDT")?;
let info = spot.exchange().exchange_info().symbol(&btc).send().await?;
for symbol in &info.symbols {
    println!("{} is {}", symbol.symbol, symbol.status);
}

let book = spot.market().depth(&btc).limit(10).send().await?;
println!("{} bids, {} asks", book.bids.len(), book.asks.len());

let candles = spot
    .market()
    .klines(&btc, Interval::Hour1)
    .limit(24)
    .send()
    .await?;
println!("{} hourly candles", candles.len());
# Ok(())
# }
```

`exchangeInfo` weighs 20 with a symbol or without. Clients in one process should share one
budget, since Binance counts weight per IP:

```no_run
use polyoxide_binance::spot::{Spot, WeightBudget};

# fn example() -> Result<(), polyoxide_binance::spot::SpotError> {
let budget = WeightBudget::new();
let a = Spot::builder().weight_budget(budget.clone()).build()?;
let b = Spot::builder().weight_budget(budget).build()?;
# Ok(())
# }
```

Each client may have `max_concurrent` requests in flight (default 4), and the budget's reserve
absorbs in-flight weight only while it stays under 600: at the default, one client has at most
320 in flight on a route at its default (4 × the heaviest, `tickers_24h`'s 80). Four `depth`
calls with a `limit` above 1000 (250 each) pass it.

## Through the venue-neutral traits

`Spot` implements `polyoxide-venue`'s `MarketData`, `Instruments`, `Trades` and `Candles`, keyed
`binance.spot:<SYMBOL>` (`spot::key`), so code written against those traits reads spot as it
reads any other venue. Spot has no funding, so it implements no `Funding`. What only Binance
publishes rides in `spot::venue`'s `<Record>Ext` types (`InstrumentExt`, `BookExt`, `TradeExt`
and `CandleExt`); a quote has none.

```no_run
use std::sync::Arc;

use polyoxide_binance::spot::{self, types::Symbol, Spot};
use polyoxide_venue::{DynMarketData, MarketData};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Spot::new()?);
let quote = venue.quote(&spot::key(&Symbol::new("BTCUSDT")?)).await?;
println!("bid {:?} ask {:?}", quote.bid, quote.ask);
# Ok(())
# }
```

The `spot::venue` module's documentation lists each method's route and weight.
