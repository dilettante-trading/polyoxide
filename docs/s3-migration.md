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
  process). `positions` reads the Data API (v1 with no dust filter then; v2 since the Data API v1 section below), `balances` holds the pUSD collateral,
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

## Perps: the spec's builder-fee drift (#76)

**Consumer impact: breaking for perps.** Two field types change (DRIFT R18); nothing is
removed, and the rest is additive.

- `PositionFill::builder_fee` and `PositionFill::total_fee` (`perps::api::public`) are `Decimal`
  where they were `Option<Decimal>`: the spec now documents and requires both, and the host sent
  them on every row. Replace `Some(x)` with `x`. `builder_fee` is the builder's additional fee,
  never negative; `total_fee` is `fee` plus `builder_fee`, and `pnl` is before both.
- `PositionFill` gains `builder: Option<BuilderTerms>`, the order's saved builder address and
  fee rate. The host sends it only on an authenticated account's own fills, so the public route
  always gives `None`.
- `CompetitionEntry` and `CompetitionAccount` (`perps::api::competition`) gain
  `first_fill_at: Option<u64>`, the account's first counted fill, a tie-break after boosted
  volume.
- New route: `perps.public().builder(address)`, an address's public builder profile
  (`BuilderProfile`: registration, enablement, the platform's admission switch, the fee cap and
  the display name).
- The builder routes under `/v1/account/*` and the `builder` field on orders belong to the
  authenticated slice, which is not implemented; they are mirrored only.

## Perps: the undocumented `pre_ipo` category (#82)

**Consumer impact: breaking for perps.** One enum changes shape (DRIFT R21); nothing is removed.

- `InstrumentCategory` (`perps::types`) is an open enum: it gains `PreIpo` (`"pre_ipo"`, which the
  host lists and its spec does not) and `Other(String)` for a spelling this version does not know,
  is `#[non_exhaustive]`, and is no longer `Copy`. A `match` on it needs a wildcard arm, and a
  copy of it a `.clone()`. Until this change an instrument of an unknown category failed the whole
  `/v1/info/instruments` page, and with it `perps::venue`'s `instrument`, `instruments` and
  `funding_rate`.
- `ListTickers::instrument_id` is now honoured by the host (DRIFT R20): `tickers()` with it
  returns that instrument's row only. Nothing changes in the client.

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

## ADR 0001: product classes

**Consumer impact: one line for an outside `KeySpace`.** `KeySpace` gains a required
`const CLASS: ProductClass`, so a crate outside the workspace that implements it adds that line.
Code that only uses keys and the market-data traits builds unchanged. This is a major-level change
for such an implementer, and it ships in a 0.x minor release.

- `polyoxide-venue`, feature `keys`: `ProductClass` (`EventContract`, `PerpetualFuture`;
  `#[non_exhaustive]`; `as_str` and `Display` spell it as a registration does), the required
  `KeySpace::CLASS`, and `MarketKey::class()`. The class follows from the venue and product, so a
  key's text form, equality and hash are unchanged.
- `polyoxide-venue`, feature `market`: the class bundles `PerpetualMarket` (`MarketData + Funding`)
  and `EventContractMarket` (`MarketData + EventGroups`), each implemented for every type that
  implements its members. Polymarket's `Perps` and Binance's `Usdm` are perpetual markets;
  Polymarket's `Clob` (with `gamma`) and Kalshi's `Predictions` are event-contract markets.

## wasm32: an injected runtime and transports

**Consumer impact: breaking.** Timers, spawns and clock reads go through an injected
`polyoxide_rt::Runtime`, and every HTTP and socket exchange through an injected transport, so
the same crates build for `wasm32-unknown-unknown`. A native build behaves as before, but the
types below change, and a consumer that matched a reqwest error or held a socket by its old type
migrates. Each break has its entry in [`s3-removals.md`](s3-removals.md), under "Removed" (the
ones cargo-semver-checks keys) and "Changes the gate does not key" (the signature and auto-trait
changes it does not).

- The HTTP failure moves. `ApiError::Network` is removed: a failure to get a response, or to read
  its body, is `ApiError::Transport(TransportError)`, so where you matched `ApiError::Network(e)
  if e.is_timeout()` match `ApiError::Transport(e) if e.kind() == TransportKind::Timeout`
  (`is_connect` is `TransportKind::Connect`). `Classify` is unchanged in effect.
- The response type moves. `HttpClient::send` and the `send_raw` methods (`Request::send_raw` and
  clob's `ListOrders::send_raw`) return `polyoxide_core::Response` where they returned a
  `reqwest::Response`; its `status`, `headers`, `url`, `bytes`, `text` and `json` read as before
  and fail with `ApiError`. `Pong::response`, `ErrorResponse::read`, `ApiError::from_response` and
  `retry_after_header` take it or its head, and `HttpClient`'s `client` field is gone: inject a
  reqwest client of your own as `ReqwestTransport::new(client)`.
- The socket type moves. `polyoxide_ws::Socket` is removed for `polyoxide_ws::BoxSocket`, which
  `connect`, `connect_over` and `Connection::new` use; `Protocol::connect` takes
  `(transport, runtime, timeout)` and returns a `BoxSocket` (it first shipped in 0.41, so
  semver-checks has no baseline for it). `Connection::last_inbound` returns
  `polyoxide_rt::Instant`, which is `std::time::Instant` on native.
- Auto traits. `Connection` and every type holding one (`WebSocket`, `WebSocketWithPing`,
  `UsdmWs`, `Rtds`, `SupervisedRtds`, `SupervisedSportsWs`, `PerpsWs`, `SportsWs`) are no longer
  `Sync`, and `Supervisor`, `Supervised` and the builders and clients that hold a runtime or
  transport are no longer `UnwindSafe`. Share a connection through a `Mutex`, and wrap a client
  in `AssertUnwindSafe` where `catch_unwind` needs it.
- `Kind::Unsupported` joins `polyoxide_ws::Kind` (an operation the transport cannot do, such as a
  ping frame in a browser); reconnecting never fixes it. `Kind` is `#[non_exhaustive]`, so a
  `match` on it already has a wildcard arm.

What is added, so a native build that sets none of it is unchanged:

- `polyoxide-rt`, new and first in the publish order: the `Runtime` trait, `Task`, `timeout`,
  `Interval`, `default_runtime()` and `TokioRuntime` on a host target, and the `Instant` and
  `SystemTime` of `web-time` (std's own types on native).
- Setters on every client and socket builder: `runtime(..)` and, on an HTTP builder,
  `http_transport(..)` (from `client_config_setters!`), and `runtime(..)` with
  `socket_transport(..)` on a socket builder (from `polyoxide_ws::transport_setters!`).
  `HttpClientBuilder::http_transport`, `ClientConfig::http_transport` and `Supervisor`'s own
  `runtime` and `socket_transport` take the same. With none set, a host target uses
  `ReqwestTransport`, `TungsteniteTransport` and `TokioRuntime`, and `wasm32` uses `FetchTransport`
  (`polyoxide-core`), `WebSocketTransport` (`polyoxide-ws`) and `BrowserRuntime`.
- `polyoxide_ws::Capabilities`, which a `SocketTransport` reports (`client_pings`,
  `sees_server_pings`, `handshake_headers`), so a protocol learns what its transport can do.

**For a crate outside the workspace that invokes `client_config_setters!`:** the macro's
`runtime` setter names `polyoxide_rt::Runtime`, which `polyoxide-core` does not re-export, so the
invoking crate declares `polyoxide-rt` itself. `transport_setters!` needs it the same way.

## Data API v1 retired (retire-data-v1)

Consumer impact: breaking. Polymarket retired Data API v1 on 2026-11-02: every v1 route on
`data-api.polymarket.com`, the bare `/`, and the `user-pnl-api` and `lb-api` hosts answer `410`.
polyoxide removes v1 and makes v2 the `data` module itself. Each removed path is keyed in
[`s3-removals.md`](s3-removals.md).

**Rust: each v1 call and its replacement.** Every route is a `DataApi` method now; the v2 rows
are snake_case, page by cursor (`Page<T>`, `.cursor(..)`, `.pages()`), and use `Option` for a
value the API may not have.

| v1 call | Now |
|---|---|
| `data.user(addr).list_positions()`, `.closed_positions()` | `data.positions(addr)`, with `.status(PositionStatus::Open \| Closed \| Redeemable \| ..)` |
| `data.user(addr).positions_value()` | `data.value(addr)` |
| `data.user(addr).trades()`, `data.trades().list()` | `data.trades()`, with `.user(addr)` or `.conditions(..)` |
| `data.user(addr).activity()` | `data.activity(addr)` |
| `data.user(addr).traded()`, `data.traded(addr)` | `data.user_stats(addr)`; `None` for a wallet the API does not know |
| `data.holders().list(markets)` | `data.holders(conditions)` |
| `data.open_interest()`, `data.live_volume()` | `data.open_interest()`, `data.live_volume(event_ids)` |
| `data.leaderboard()` | `data.leaderboard()`, `data.leaderboard_user(addr)` |
| `data.builders()` | `data.builders_leaderboard()`, `data.builder_volume()` |
| `data.market_positions()` | `data.positions(PositionAnchor::Condition(condition))` |
| `data.combos()` | `data.combo_positions(addr)`, `data.combo_activity(addr)` |
| `data.approvals()` | `data.approvals(addr)` |
| `data.pnl()` (`user-pnl-api`) | `data.user_pnl(addr)`, which is not the same series |
| `data.rankings()` (`lb-api`) | `data.leaderboard()`, which ranks volume in shares, not USDC |
| `data.health()` | `data.status()`; its 200 is the host's liveness signal now that `/` is gone |
| `data.accounting()`, `data.misc()` | nothing: v2 serves no accounting snapshot, `/other` or `/revisions` |
| `DataApiBuilder::pnl_base_url`, `rankings_base_url` | nothing: the client keeps one host |

**Rust: the flatten.** `data::v2::X` is `data::X` (`data::types::Position`, `data::Page`,
`data::api::feeds::ListTrades`), and `data.v2().x()` is `data.x()`; `DataV2` is gone.
`V2Error` is `data::VenueError` and `DataError::V2` is `DataError::Venue`, fields unchanged.
`SortDirection` and `Allowance` keep their `data::types` paths.

**Rust: building records.** Every struct under `data::types`, and `Page` and `Pagination`, drops
`#[non_exhaustive]`, so a consumer builds its own rows and pages by struct literal; a field added
to one is therefore a breaking change from here on. The enums stay `#[non_exhaustive]` beside
their `Other(raw)`. `OutcomeIndex::new(raw)` builds an index, `999` (`UNLABELED`) included.

**Rust: the CLOB's `Trading` and `Trades`.** `Trades` reads one taker-only `/v2/trades` page of
100. `positions` walks `/v2/positions` (`status=OPEN`, which includes redeemable rows); v2 serves a
holding of 0.1 share or less only as closed, so such dust is no longer listed. `PositionExt`'s
`initial_value` is `entry_cost` (v2's `entry_cost_usdc`) and `cash_pnl` is `unrealized_pnl`; an end
date of `1970-01-01` or empty, and an outcome index of 999, are `None`.

**Rate limits.** The data client paces by Polymarket's published v2 table: 800/10s general, 200
per printed pair (`/v2/positions` with its `/combos`, `/v2/activity` with its `/combos`, one shared
allowance each), `/v2/prices-history` 200, `/v2/status` 100. `/v2/trades` keeps the measured 200,
since the published 300 drew 429s at its pace; `/v2/user-pnl` and `/v2/holders` keep 400.

**TypeScript (specta).** Data's `TradeSide`, `ActivitySide`, `ActivityType`, `PositionStatus` and
`TokenModule`, and gamma's `ProtocolVersion`, `ResolutionStatus` and `HomeAway`, export as named
unions of their known spellings and `OpenString` (`string & {}`) instead of `string`, for example
`export type TradeSide = "BUY" | "SELL" | OpenString`. `polyoxide_venue::specta_as_string!` is
removed.

**Python.** `DataApi()` and `DataApiSync()` keep their constructor; every route is a method
(`DataApi().trades()`), and the row classes, `Page` and its iterators are importable from
`polyoxide` (`from polyoxide import Trade`). `DataApi().v2()`, `polyoxide.v2`, the v1 methods
(`user()`, the v1 `trades()`, `holders()`, `leaderboard()`, `health()`, ...) and the v1 row classes
are gone.

**CLI.** `polyoxide polymarket data …` keeps every command and flag; `data health` prints
`/v2/status`.
