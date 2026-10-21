# polyoxide in stage S3: what each release adds

Stage S3 of the multi-venue restructure is for additions (AD-16): the venue-neutral traits, a
supervised clob socket, and a third venue, Kalshi. Its removals, which are few, are listed in
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
- Every events stream begins with `Resync`, unless the stream cannot open, when its only item is
  that error, and a fill the venue voids yields `Resync`, where the architecture asked for one
  only after a reconnect (amendment A7-2).

## Gamma: `closed=any` (#59)

**Consumer impact: additive, with one behaviour change to `get_many`.** Nothing is removed or
renamed, and no signature changes.

- `polyoxide-polymarket`: `gamma::types::ClosedFilter` (`Open`, `Closed`, `Any`) and a
  `closed_filter` setter on `ListEvents`, `ListKeysetEvents`, `ListMarkets` and
  `ListKeysetMarkets`. `closed(bool)` is unchanged. `Any` returns both states in one request; on
  the two markets routes it needs an identifier filter, or the host answers `422`.
- `Markets::get_many` (DRIFT R13) sends `closed=any` for each 100 distinct ids, each with a
  `limit` of its ids' number, one request after the other, where it sent `closed=true` and
  `closed=false` and merged them. A call for up to 100 ids is one request where it was two. A
  longer list works where it was a `422`, and 21 to 100 open ids come back whole where they lost
  every row past the 20th. A result is in the host's order for each chunk, not closed markets
  first, and a repeated id is sent once.
- The CLOB's `event_group` makes one `closed=any` request where it made one or two.
- The `id` setters of `ListMarkets` and `ListEvents` documented a batch of 400 ids, and the
  host takes 100.

## Perps: the spec's competition, snapshot and settlement drift (#56)

**Consumer impact: breaking for perps.** Three field types change and an enum gains a variant
(DRIFT R17); nothing is removed, and the rest is additive.

- `Instrument::close_only` (`perps::api::exchange`), `Trade::settlement` (`perps::api::market`)
  and `PositionFill::settlement` (`perps::api::public`) are `bool` where they were
  `Option<bool>`: the spec now requires all three, and the host sent them on every row. Replace
  `Some(x)` with `x`. `perps::venue`'s `InstrumentExt::close_only` and `TradeExt::settlement`
  stay `Option<bool>` and are now always `Some`.
- `LeaderboardEntry` and `LeaderboardAccount` gain a required `roi`, return over the window as a
  fraction, and the undocumented `instruments_traded` and `top_instrument_ids`, both `Option`.
  `LeaderboardSort` gains `Roi`, which breaks an exhaustive `match` on it.
- `Instrument` gains `settlement: Option<InstrumentSettlement>`. `GET /v1/info/instruments` lists
  enabled instruments only, but filtered by `instrument_id` it also returns a settled one, with
  `close_only` set and its settlement record; `perps::venue`'s `instrument` now reports such an
  instrument with `tradable: Some(false)` and the record in `InstrumentExt::settlement`.
- New routes: `perps.public().registered(address)`; `perps.public().position_snapshots(address)`, a
  `POST` with `.active(instrument_ids)` (at most 20) and `.history(fills)` (at most 4,
  `PositionSnapshotFill`, which converts from a `PositionFill`), refused before sending when it
  names neither, too many or one twice, or an instrument id or fill time past the host's bounds; and
  the namespace `perps.competitions()`: `list()`, `get(id)`, `leaderboard(id)`, `account(id,
  address)` and `widget()`, with `CompetitionId`, `CompetitionStatus` and `CompetitionRowState` in
  `perps::types`. No competition has existed yet, so the competition types are modelled from the
  spec alone.

## Epic 9: Kalshi published

**Consumer impact: additive.** Nothing is removed or renamed, and no signature changes; code
that builds on 0.41 builds unchanged. `polyoxide-kalshi` is new on crates.io, and the umbrella's
default build, `polymarket`, gains no crate.

- `polyoxide-kalshi`, first published in 0.42.0 (it was `publish = false`). The module
  `predictions`, feature `predictions` (on by default): `Predictions` and `PredictionsBuilder`,
  for Kalshi's production and demo hosts, with optional `Credentials` that sign a keyed client's
  requests; the namespaces `health`, `exchange`, `markets` (`list`, `get`, `orderbook`, `trades`,
  `candlesticks` and `series_candlesticks`), `events` and `account`; `PredictionsError`;
  `predictions::key`, whose keys read `kalshi.predictions:<TICKER>:yes|no`; and
  `predictions::venue`, whose `Predictions` implements `MarketData`, `Trades`, `EventGroups`
  and `Candles` with `InstrumentExt`, `TradeExt`, `EventGroupExt` and `CandleExt`. Its candles
  come at one minute, one hour and one day (any other width is refused before a request), and a span can return fewer candles than it has periods, since the venue's route leaves out a period with no row at all.
- `polyoxide-kalshi`, with `predictions-ws`: `PredictionsWsBuilder` connects a
  `SupervisedPredictionsWs` on the three channels `orderbook_delta`, `ticker` and `trade`
  (`Channel`, chosen with `channels`, the order book by default). It yields `Event::Snapshot`,
  `Delta`, `Ticker`, `Trade`, `Resyncing`, `Disconnected` and `Reconnected`. A gap in the
  order-book sequence asks Kalshi for fresh snapshots on the same connection instead of
  reconnecting (`PredictionsMembership::snapshot` asks on demand), and `ws::book::Books` keeps
  each ticker's ladder from the events. The handshake needs `Credentials`, public data
  included.
- The Read throttle: a client with no credentials has one bucket, depth 600 refilling 180 tokens
  a second (20 reads a second measured clean, less a tenth); a keyed client's public reads draw on
  a second bucket sized the same, and only `GET /account/limits` draws on the key's bucket.
- The umbrella: the features `kalshi`, `kalshi-predictions` and `kalshi-predictions-ws`, all in
  `full` and none in the default; `polyoxide::kalshi`; `polyoxide::parse_key` names
  `kalshi.predictions` under `kalshi-predictions`; and the prelude names `Predictions`,
  `PredictionsBuilder`, `PredictionsError`, `KalshiCredentials`, `PredictionsEvent`,
  `PredictionsWsBuilder`, `SupervisedPredictionsWs`, `PredictionsWsError`,
  `PredictionsMembership`, `PredictionsChannel` and `PredictionsBooks`.
- `polyoxide-venue`, feature `market`: `Candles::intervals()`, a provided method that returns
  the widths a venue serves, `CandleInterval::ALL` by default, so every existing implementation
  keeps its behaviour and `DynCandles` forwards it. `CandleQuery::ensure_served` and the new
  `UnservedInterval` (`InvalidRequest`, a fault) let a venue that serves fewer widths refuse
  another before it sends anything.
- `polyoxide-ws`: `Protocol::follow_up`, a provided method that returns no frame by default. The
  `Supervisor` calls it once after each `decode` that leaves the connection up and sends the
  frame it returns, bounded by `stale_after`. A `Protocol` written before it is unchanged.

No existing build gains a third-party crate; `full` gains `polyoxide-kalshi`. `kalshi-predictions` builds core's HTTP stack,
as the other venues' HTTP modules do, and `kalshi-predictions-ws` adds the socket kit.
