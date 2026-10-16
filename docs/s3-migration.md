# polyoxide in stage S3: what each release adds

Stage S3 of the multi-venue restructure is for additions (AD-16): the venue-neutral traits, a
supervised clob socket, and the Kalshi skeleton. Its removals, which are few, are listed in
[`s3-removals.md`](s3-removals.md). This page holds the consumer-impact note of each S3 piece of
work, for the CHANGELOG section of the first release that ships it.

## Epic 6: venue-neutral market data

**Consumer impact: additive.** Nothing is removed or renamed, and no signature changes; code
that builds on 0.40 builds unchanged.

- `polyoxide-venue` gains two features. `keys`: `MarketKey`, written `venue.product:id`, which only
  a product's `KeySpace` makes, so two keys are equal exactly when they name the same instrument;
  `RawKey`, text split into its parts; `VenueId`, `ProductId` and `KeyError`. `market`: the
  records (`Instrument`, `Quote`, `Book`, `Level`, `Trade`, `Candle`, `FundingRate`,
  `EventGroup`) and the types they and the traits use (`Side`, `SizeUnit`, `TradeId`,
  `CandleInterval`, `CandleQuery`, `EventOutcome`), `Extensions` for each venue's own facts and its
  `DuplicateExtension` error, `NotListed`, and the traits `MarketData` (`instrument`, `quote`,
  `book`) and `Instruments`, `Trades`, `Candles`, `Funding` and `EventGroups`, each with a `Dyn`
  wrapper. Every trait method returns `ClassifiedError`. A `FundingRate` carries its `interval`,
  the length of the funding period its `rate` is for.
- `polyoxide-binance`: the module `usdm::key` (`UsdmKeys`, `parse`, `symbol`, `VENUE`, `PRODUCT`)
  and the function `usdm::key(&symbol)` it re-exports, and `usdm::venue`, whose `Usdm` implements
  `MarketData`, `Instruments`, `Trades`, `Candles` and `Funding`, with its `InstrumentExt`,
  `QuoteExt`, `BookExt`, `TradeExt`, `CandleExt` and `FundingExt`. Its funding methods send
  `fundingInfo` as well, one extra request on the funding bucket, for the `interval`.
- `polyoxide-polymarket`: `shared::keys` (the functions `clob`, `perps`, `token_id`,
  `instrument_id` and `parse`; the consts `VENUE`, `CLOB` and `PERPS`; and `TokenId`, `ClobKeys`
  and `PerpsKeys`); `perps::venue`, whose `Perps` implements `MarketData`, `Instruments`,
  `Trades`, `Candles` and `Funding`, with its `InstrumentExt`, `BookExt`, `TradeExt` and
  `FundingExt` (its funding methods send one extra `/v1/info/instruments` request for the
  `interval`); `clob::venue`, whose `Clob` implements `MarketData` and `Candles`, `Trades` with
  `data`, and `EventGroups` with `gamma`, with its `InstrumentExt`, `QuoteExt`, `BookExt`,
  `TradeExt` and `EventGroupExt`.
- `ClobBuilder::data` and `ClobError::Data` are new (`ClobError` is `#[non_exhaustive]`). With
  the `data` feature, which `polyoxide-polymarket`'s defaults include, `ClobBuilder::build` also
  builds a Data client for `Trades`, with the builder's timeout and pool size; it sends nothing
  until a trait method asks. The unified `Polymarket` client hands clob its own Gamma and Data
  clients. With `gamma_base_url` set on `PolymarketBuilder`, clob's proxy-wallet lookup and
  `event_group` now go to that URL, with the builder's timeout and through Gamma's pool and
  throttle; clob used to build its own Gamma client for them, on the default Gamma host.
- The umbrella: `polyoxide::parse_key`, the traits on `polyoxide::venue` with any module that
  implements them, and `examples/venue_neutral.rs`, one `best_bid_ask` and one error classifier
  that run unchanged on the CLOB, perps and USDⓈ-M.

No dependency tree changes: `market` builds only rust_decimal, serde and dynosaur, which every
module that turns it on already builds, and the socket-only modules turn on neither feature.
