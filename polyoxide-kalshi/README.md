# polyoxide-kalshi

A Rust client library for Kalshi's event-contract exchange, the third venue of the polyoxide workspace after Polymarket and Binance.

The umbrella crate `polyoxide` re-exports it as `polyoxide::kalshi`, under its `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws` features.

It has one module, `predictions`, behind the `predictions` feature, which is on by default. The module covers Kalshi's Predictions exchange on `external-api.kalshi.com`, and on the demo host `external-api.demo.kalshi.co`. It reads market data through `polyoxide-venue`'s traits, `MarketData`, `Trades`, `Candles` and `EventGroups`; its candles come at three widths, one minute, one hour and one day, which `Candles::intervals()` lists, and a request for any other width is refused before anything is sent. Its documentation says what else it offers.

The feature `predictions-ws`, which is off by default and turns on `predictions`, adds the module's sockets, `predictions::ws`, on the polyoxide socket kit. It streams the order-book channel, a snapshot of each subscribed market's book and then its changes in YES pricing, and the `ticker` and `trade` channels, chosen with `PredictionsWsBuilder::channels`. A local book, `Books`, folds the order-book events into each market's ladders. The stream reconnects after a drop or a stall, and a gap in the order-book subscription's sequence is repaired in place with a fresh snapshot, falling back to a reconnect. Kalshi refuses a socket handshake that carries no API-key headers, public data included, so the socket needs `Credentials`, where the REST reads of public data need none.
