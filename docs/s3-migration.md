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

## Epic 7: supervised CLOB sockets and normalized trading

**Consumer impact: additive.** Nothing is removed or renamed, and no signature changes; code
that builds on 0.40 builds unchanged.

- `polyoxide-ws`: `Protocol` gains a provided method, `Protocol::connect`, which the
  `Supervisor` calls for every attempt. Its default builds `handshake()`'s request and dials it
  with the kit's `connect`, so a `Protocol` written before it keeps its behaviour, and perps and
  Binance are untouched. The CLOB's two protocols override it with the CLOB's per-address dial.
- `polyoxide-polymarket`, with `clob-ws`: two supervised sockets beside the bare tier.
  `MarketWsBuilder` connects a `SupervisedMarketWs` (`ClobMarketProtocol`, `MarketEvent`,
  `MarketChange` and the `MarketMembership` trait), and `UserWsBuilder` a `SupervisedUserWs`
  (`ClobUserProtocol`, `UserEvent`, `UserChange` and the `UserMembership` trait). Both yield a
  `DisconnectReason` (the server's close code and reason, an error or a stall) with
  each `Disconnected`. They reconnect after a close or a stall, replay their membership on every
  connection (the user socket its credentials too), and yield one `Disconnected` and one
  `Reconnected` per outage. They send a text `PING` every 10 s, count any frame as liveness and
  are stale after 30 s, back off from 500 ms to 60 s, and bound each address and step of a
  connect by 10 s. They take `wss://` URLs, or `ws://` to a loopback host, and at most 500 ids
  or markets per connection. The market queue holds 1024 events and the user queue is
  unbounded. The market socket yields every book of an array frame. `WebSocket`,
  `WebSocketBuilder` and `WebSocketWithPing` are unchanged, except that their connect now dials a
  URL whose host is an IPv6 literal, which failed its lookup before. `MarketMessage::from_json`
  still keeps only the first book of an array.
- `ClobWsError::Authentication` has its first use: the user socket decodes the server's 1008
  close as `ClobWsError::Authentication(<the close reason>)`, classed `Unauthorized`, and stops
  on it instead of reconnecting. The enum's variants are unchanged.
- `polyoxide-venue` gains a feature, `trading`, which turns on `market` and builds futures-core.
  It holds the trait `Trading` (`place`, `cancel`, `cancel_all`, `open_orders`, `positions`,
  `balances` and `events`) with its `DynTrading` wrapper; `TradingEvents`, the named stream
  `events()` returns, since dynosaur drops `'static` from a returned `impl Stream`;
  `TradingEvent` (`Ack`, `Fill` and `Resync`); the records `OrderRequest`, `Order`, `Fill`,
  `Position` and `Balance`; `OrderStatus`, `KillReason`, `TimeInForce` and `Liquidity`; and the
  opaque `OrderId`, `FillId` and `ClientOrderId`. A kill is `Ok` with `Killed { reason }`, never
  an error. Under `market`, `Extensions::ensure_declared` and `UndeclaredExtension` (an
  `InvalidRequest`) give a venue the refusal it runs on an option it does not declare.
- `polyoxide-polymarket`: with `clob-ws`, `ClobBuilder::user_socket`, and, with `clob-ws` and
  `data`, `impl Trading for Clob` with `ClobOrderOptions` (`neg_risk`, `tick_size`, `expiration`)
  and the venue's own facts in `OrderExt`, `FillExt`, `PositionExt` and `BalanceExt`, all in
  `polyoxide_polymarket::clob::venue`.
  `clob-ws` now turns on `polyoxide-venue/trading`, and so does the umbrella's
  `polymarket-clob-ws`, whose crate root pins `DynTrading` and, with `polymarket-data`,
  `Clob: Trading`. The CLOB stores no client
  order id, so a `Clob` remembers the ones it placed in-process (at most 10,000, lost with the
  process). `positions` reads Data v1 with no dust filter, `balances` holds the pUSD collateral,
  and `events()` opens its own user socket for each call.
- No dependency tree gains a crate: `trading` builds futures-core, which every build that turns
  it on already has through futures-util.

Where this epic departs from its acceptance criteria:

- The CLOB's `Trading` is gated `all(clob-ws, data)`, not `all(clob, data)`, because `events()`
  needs the user socket, which is `clob-ws`'s.
- `ClobOrderOptions` carries no fee terms: a V2 order signs no fee rate, and a fill's `fee` is
  `None`, with the maker's `fee_rate_bps` in `FillExt`.
- The Kalshi trading mapping stays a planning document,
  `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, until Story 8.1
  registers Kalshi's mirror.
- Every events stream begins with `Resync`, and a fill the venue voids yields `Resync`, where the
  architecture asked for one only after a reconnect (amendment A7-2).
