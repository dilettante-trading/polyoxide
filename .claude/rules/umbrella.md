---
paths:
  - "polyoxide/**"
---

# The umbrella (`polyoxide`)

- The crate root holds `venue` (`polyoxide-venue`, always on), `polymarket`
  (`polyoxide-polymarket`, under any `polymarket*` feature), `binance` (`polyoxide-binance`, under
  any `binance*` feature), `kalshi` (`polyoxide-kalshi`, under any `kalshi*` feature), `parse_key`
  (under the features that build a product's keys) and `prelude`, and nothing else.
- The features `polymarket` and `binance` turn on their venue crate's defaults; each
  `<venue>-<module>` passes its module's feature through, a `-ws` one turning its module's on,
  relay's included; `kalshi` turns on `kalshi-predictions`; `keychain` stays a cross-venue extra,
  each venue adding its own `?/keychain`; there is no umbrella `specta`. The generated feature list
  is in CLAUDE.md.
- A module that implements the market-data traits (`polymarket-clob`, `polymarket-perps`,
  `binance-usdm`, `kalshi-predictions`) also turns on `polyoxide-venue/market`, and one that only
  names keys (`polymarket-gamma`, `polymarket-data`) `polyoxide-venue/keys`, so `polyoxide::venue`
  carries the traits with every module that implements them, which a `const _` pins.
  `polymarket-clob-ws` turns on `polyoxide-venue/trading`, and its `const _` pins `DynTrading` and
  `Clob: Trading`.
- Each passthrough names one type of its module through the path it opens, in a `const _` outside
  `cfg(test)`, so `cargo hack --each-feature` fails a re-export whose `cfg` leaves a feature out.
- The prelude re-exports a selection of the enabled modules' clients, errors and stream types
  under the umbrella's feature names, aliasing a stream's `Event` by its venue (`PerpsEvent`,
  `SportsEvent`, `UsdmEvent`, `PredictionsEvent`). One `const _` per prelude block pins it to its
  feature, and one more pins the unified client (`polyoxide_polymarket::Polymarket`, also at
  `polyoxide::polymarket::Polymarket`) to `polymarket`.
- Kalshi's prelude names are `Predictions`, `PredictionsBuilder` and `PredictionsError`,
  `Credentials` as `KalshiCredentials` (a bare `Credentials` names the CLOB's), and, with
  `kalshi-predictions-ws`, `Event` as `PredictionsEvent`, `PredictionsWsBuilder`,
  `SupervisedPredictionsWs`, `PredictionsWsError`, `PredictionsMembership`, `Channel` as
  `PredictionsChannel` and `Books` as `PredictionsBooks`. `parse_key` also names
  `kalshi.predictions` under `kalshi-predictions`.
- `examples/venue_neutral.rs` reads the best bid and ask of the CLOB, perps, USDⓈ-M and Kalshi with
  one `best_bid_ask(&DynMarketData, &MarketKey)`, and their errors with one classifier generic
  over `Classify` (CAP-5 and CAP-4). `tests/venue_neutral.rs` includes it by `#[path]` and runs
  both against mock servers for each module; `tests/live_venue.rs` runs `best_bid_ask` against the
  live hosts; `tests/parse_key.rs` checks every product's class.
