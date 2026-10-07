# polyoxide-binance

Rust client library for Binance USDⓈ-M futures public market data
(`fapi.binance.com`): contracts, tickers, the premium index and funding, klines,
open interest, aggregate trades and order book snapshots. No credentials are
needed.

Every request is charged against a `WeightBudget`, because Binance limits each
IP by request *weight*, which varies with the route and its parameters, rather
than by request count. The budget follows the server's own count from the
`X-MBX-USED-WEIGHT-1M` header, and a `429` or `418` holds every request until
the server's wait is over.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-binance/).

## Installation

```toml
[dependencies]
polyoxide-binance = "0.36"
```

## Usage

```no_run
use polyoxide_binance::{
    usdm::types::{Interval, Symbol},
    Usdm,
};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let usdm = Usdm::new()?;
let btc = Symbol::new("BTCUSDT")?;

let index = usdm.market().premium_index(&btc).send().await?;
println!("mark {} index {}", index.mark_price, index.index_price);

let candles = usdm
    .market()
    .klines(&btc, Interval::H1)
    .limit(24)
    .send()
    .await?;
println!("{} hourly candles", candles.len());
# Ok(())
# }
```

Clients in one process should share one budget, since Binance counts weight
per IP:

```no_run
use polyoxide_binance::{Usdm, WeightBudget};

# fn example() -> Result<(), polyoxide_binance::BinanceError> {
let budget = WeightBudget::new();
let a = Usdm::builder().weight_budget(budget.clone()).build()?;
let b = Usdm::builder().weight_budget(budget).build()?;
# Ok(())
# }
```

Each client may have `max_concurrent` requests in flight (default 4), and the budget's
reserve absorbs in-flight weight only while it stays under 240, so two clients that send
weighted routes should each set `max_concurrent(2)` (2 × 2 × 40 = 160). A client that sends
only the funding routes carries no weight and can keep the default.

## Streaming

With the `ws` feature, the market streams arrive over one connection per routed path
(`/market` for trades, klines, mark prices and tickers; `/public` for depth and book
tickers). `UsdmWsBuilder` keeps each connection alive, replaces a dead or 24-hour-old one,
replays its streams at Binance's pace, and reports each outage: every
`Event::Disconnected { path }` is followed by `Event::Reconnected { path }`, after which
anything built from that path's streams should be rebuilt.

```text
use futures_util::StreamExt;
use polyoxide_binance::usdm::{types::Symbol, ws::{Event, StreamName, UsdmWsBuilder}};

let btc = Symbol::new("BTCUSDT")?;
let mut feed = UsdmWsBuilder::new()
    .streams([StreamName::MarkPrice(btc.clone()), StreamName::BookTicker(btc)])
    .connect()
    .await?;
while let Some(event) = feed.next().await {
    match event? {
        Event::Update(update) => println!("{}", serde_json::to_string(&update)?),
        Event::Disconnected { path, reason } => eprintln!("{path} down: {reason}"),
        Event::Reconnected { path } => eprintln!("{path} back"),
        _ => {}
    }
}
```
