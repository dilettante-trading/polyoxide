## [Unreleased]

- **Data v2 token module `directional` (#78).** Upstream's `TokenModule` schema gained
  `directional`, which `data::v2::types::TokenModule` gains as `Directional`. The enum is open, so
  this is additive, but a `directional` row that decoded as `Other("directional")` now decodes as
  `Directional`. No such token has been seen on the wire yet.

## [0.43.0] - 2026-10-22

Kalshi's catalog routes (#71): the reads a catalog kept in sync needs, and the market-data traits
reading markets that settled before Kalshi's historical cutoff. Everything is new API in
`polyoxide-kalshi`, so the 0.x minor rises for the features alone. Nothing is removed or renamed,
no existing type changes, [`docs/s3-removals.md`](docs/s3-removals.md) still lists nothing, and no
build gains a crate.

- **Markets.** `markets().list().tickers(..)` reads a named set of markets in one request, and
  `markets().orderbooks(..)` up to 100 books (`MAX_ORDERBOOK_TICKERS`, `OrderbooksResponse`).
  `markets().window(MarketWindow)` lists the markets created, closing or settled in a span, one
  timestamp family per request since Kalshi refuses two, each with the states it allows
  (`CreatedStatus`); `markets().updated(min_ts)` lists those whose metadata changed since, which a
  price move alone does not, with multivariate combinations excluded.
- **The tickers budget.** A ticker set is refused before anything is sent when it is empty or its
  query would pass `TICKERS_QUERY_BUDGET` (8,000 bytes, half the request target Kalshi answers
  `414` past), on `ListMarkets`, `ListMarketWindow` and `ListEvents` and in
  `HistoricalSelection::Tickers`. `ticker_chunks` splits a larger set into requests that fit.
- **Series and events.** The namespace `series()`: `list()` reads every series a category, tags
  or update time admits, in one body (`SeriesList`), and `get(ticker)` one `Series`, with its
  volume on request. `events().list()` pages events (`EventsPage`) by status
  (`EventStatusFilter`), series, tickers, nested markets, and close and update times; milestones
  are kept as raw JSON.
- **The archive.** The namespace `historical()` reads what Kalshi has moved past its cutoff:
  `cutoff()` (`HistoricalCutoff`), `markets(HistoricalSelection)` (every archived market, all but
  the multivariate combinations, a set of tickers, or one event's; Kalshi refuses two selections
  at once), `market(ticker)`, `candlesticks(..)` in the archive's own shape
  (`HistoricalCandlestick`, whose every price key is sent, `null` when empty) and `trades()`.
- **The venue traits read the archive.** `predictions::venue`'s `instrument`, `candles`, `trades`
  and `event_group` read live first and ask the archive only when the live answer says the market
  is not there: `instrument` on the live route's `404`, `candles` when the batch answer leaves the
  market out (clamped to 5,000 periods, the archive's cap), `trades` when the live route has none,
  and `event_group` when the live event does not nest the key's market. A market settled before
  the cutoff, which was `NotListed`, now resolves; a ticker neither knows is still `NotListed`
  (the archive's `404`), one request later.

### 🚀 Features

- *(kalshi)* The tickers budget, ticker_chunks and nine catalog routes ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 2)
- *(kalshi)* Markets tickers, timestamp windows, updated and orderbooks ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 3)
- *(kalshi)* The series namespace ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 4)
- *(kalshi)* The events listing ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 5)
- *(kalshi)* The historical namespace ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 6)
- *(kalshi)* The venue traits read archived markets from the archive ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 7)

### 📚 Documentation

- *(spec)* Kalshi catalog routes design ([#71](https://github.com/dilettante-trading/polyoxide/issues/71))
- *(plan)* Kalshi catalog routes implementation plan ([#71](https://github.com/dilettante-trading/polyoxide/issues/71))
- *(spec)* The archive's candle route caps at 5,000, so its read is clamped there ([#71](https://github.com/dilettante-trading/polyoxide/issues/71))

### 🧪 Testing

- *(kalshi)* Capture the catalog and historical routes ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 1)
- *(kalshi)* Live catalog and archive suites, and the records ([#71](https://github.com/dilettante-trading/polyoxide/issues/71), Story 8)

## [0.42.0] - 2026-10-21

Kalshi's first release on crates.io (Epic 9), product classes (ADR 0001), a CLOB fix (#70), and
upstream drift adopted for data v2 (#67), perps (#56, #57) and Kalshi's mirrors (#68, #69).
Three changes are major-level, so the 0.x minor rises: the CLOB's batch reads change their
return types (#70), three perps fields lose their `Option` and `LeaderboardSort` gains a variant
(#56), and `KeySpace` gains a required `CLASS` (ADR 0001), which only a crate implementing it
outside the workspace notices. Nothing is removed or renamed, and
[`docs/s3-removals.md`](docs/s3-removals.md) still lists nothing. Epic 9 and ADR 0001 are
summarized by their consumer-impact notes from [`docs/s3-migration.md`](docs/s3-migration.md).

- **CLOB batch reads decode (#70).** `Markets::prices`, `midpoints` and `spreads` declared a
  `Vec` return, but the CLOB answers a map keyed by token id, so every call failed to decode.
  They now return `HashMap<String, HashMap<OrderSide, Decimal>>` (token, side, price) and
  `HashMap<String, Decimal>`, and accept each value as a string, as the venue sends it, or a
  number, as the spec types it. A token with no order book is absent. `OrderSide` derives
  `Hash`. The return types change, so the release that carries this raises the 0.x minor.
  `last_trade_prices` is unchanged: `POST /last-trades-prices` does answer an array.
- **Data v2 activity type `MIGRATION` (#67).** Upstream's spec now lists the values
  `/v2/activity`'s `type` accepts, one of them `MIGRATION`, which `data::v2::types::ActivityType`
  gains as `Migration` (DRIFT R16). The enum is `#[non_exhaustive]`, so this is additive, but a
  `MIGRATION` row that decoded as `Other("MIGRATION")` now decodes as `Migration`. The host
  refuses any value it does not list with a `400`, so an `Other` sent as a filter fails, and
  `DEPOSIT` or `WITHDRAWAL` alone returns an empty page unless
  `exclude_deposits_withdrawals(false)` is sent; `ListActivity::types` now says both.

- **Perps spec drift (#56).** Seven public routes, documented fields and `roi` (DRIFT R17).
  The changed types make this release raise the 0.x minor for perps too.
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
  - New routes: `perps.public().registered(address)`; `perps.public().position_snapshots(address)`,
    a `POST` with `.active(instrument_ids)` (at most 20) and `.history(fills)` (at most 4,
    `PositionSnapshotFill`, which converts from a `PositionFill`), refused before sending when it
    names neither, too many or one twice, or an instrument id or fill time past the host's bounds;
    and the namespace `perps.competitions()`: `list()`, `get(id)`, `leaderboard(id)`, `account(id,
    address)` and `widget()`, with `CompetitionId`, `CompetitionStatus` and `CompetitionRowState` in
    `perps::types`. No competition has existed yet, so the competition types are modelled from the
    spec alone.

**Epic 9: Kalshi published.** Additive; `polyoxide-kalshi` is new on crates.io, and the
umbrella's default build, `polymarket`, gains no crate.

- `polyoxide-kalshi`, first published in 0.42.0 (it was `publish = false`). The module
  `predictions`, feature `predictions` (on by default): `Predictions` and `PredictionsBuilder`,
  for Kalshi's production and demo hosts, with optional `Credentials` that sign a keyed client's
  requests; the namespaces `health`, `exchange`, `markets` (`list`, `get`, `orderbook`, `trades`,
  `candlesticks` and `series_candlesticks`), `events` and `account`; `PredictionsError`;
  `predictions::key`, whose keys read `kalshi.predictions:<TICKER>:yes|no`; and
  `predictions::venue`, whose `Predictions` implements `MarketData`, `Trades`, `EventGroups`
  and `Candles` with `InstrumentExt`, `TradeExt`, `EventGroupExt` and `CandleExt`. Its candles
  come at one minute, one hour and one day (any other width is refused before a request), and a
  span can return fewer candles than it has periods, since the venue's route leaves out a period
  with no row at all.
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

No existing build gains a third-party crate; `full` gains `polyoxide-kalshi`. `kalshi-predictions`
builds core's HTTP stack, as the other venues' HTTP modules do, and `kalshi-predictions-ws` adds the
socket kit.

**ADR 0001: product classes.** One line for an outside `KeySpace`; code that only uses keys
and the market-data traits builds unchanged.

- `polyoxide-venue`, feature `keys`: `ProductClass` (`EventContract`, `PerpetualFuture`;
  `#[non_exhaustive]`; `as_str` and `Display` spell it as a registration does), the required
  `KeySpace::CLASS`, and `MarketKey::class()`. The class follows from the venue and product, so a
  key's text form, equality and hash are unchanged.
- `polyoxide-venue`, feature `market`: the class bundles `PerpetualMarket` (`MarketData + Funding`)
  and `EventContractMarket` (`MarketData + EventGroups`), each implemented for every type that
  implements its members. Polymarket's `Perps` and Binance's `Usdm` are perpetual markets;
  Polymarket's `Clob` (with `gamma`) and Kalshi's `Predictions` are event-contract markets.

### 🚀 Features

- *(kalshi)* The candlestick fixtures and their findings (Epic 9, Story 1)
- *(venue)* UnservedInterval and CandleQuery::ensure_served (Epic 9, Story 2)
- *(venue)* Candles::intervals, forwarded by DynCandles (Epic 9, Story 2)
- *(kalshi)* The route table holds the two candlestick routes (Epic 9, Story 3)
- *(kalshi)* PeriodInterval and the candlestick types (Epic 9, Story 3)
- *(kalshi)* Markets().candlesticks and series_candlesticks (Epic 9, Story 3)
- *(kalshi)* Candles on Predictions, with CandleExt and Ohlc (Epic 9, Story 3)
- *(kalshi)* Clamp the candle span to 10,000 periods (Epic 9, Story 3)
- *(ws)* Protocol::follow_up sends a frame after a decode (Epic 9, Story 4)
- *(kalshi)* The multi-channel socket fixtures and their findings (Epic 9, Story 1)
- *(kalshi)* The channel enum, the ticker and trade types and the two new socket errors (Epic 9, Story 5)
- *(kalshi)* The socket carries ticker and trade, empties to nothing and re-snapshots a gap in place (Epic 9, Story 5)
- *(kalshi)* A local order book kept from snapshots and deltas (Epic 9, Story 6)
- *(kalshi)* Resyncing drops its tickers' books and Reconnected drops all (Epic 9, Story 6)
- *(kalshi)* Read_soak's arguments, URL supply and the signed probe's verdict (Epic 9, Story 7)
- *(kalshi)* Read_soak's drivers, the ramp, the shipped-client validation and the signed probe (Epic 9, Story 7)
- *(kalshi)* Size the unsigned Read bucket from the soak, a tenth below the clean rate (Epic 9, Story 7)
- *(kalshi)* A keyed client's public reads draw on the unsigned bucket, not the key's (Epic 9, Story 7)
- *(kalshi)* Publish polyoxide-kalshi and add the umbrella's kalshi features (Epic 9, Story 8)
- *(umbrella)* Polyoxide::kalshi, parse_key's Kalshi arm and the prelude's Kalshi names (Epic 9, Story 8)
- *(umbrella)* The venue-neutral example and its mock suite read Kalshi (Epic 9, Story 8)
- *(polyoxide)* The Kalshi prelude adds PredictionsMembership and Channel as PredictionsChannel (Epic 9, final review M5)
- *(data)* ActivityType::Migration, adopting drift issue [#67](https://github.com/dilettante-trading/polyoxide/pull/67)
- *(perps)* Adopt the perps spec drift ([#56](https://github.com/dilettante-trading/polyoxide/issues/56))
- *(venue)* Every product declares its ProductClass, and every key carries it (ADR 0001)
- *(registry)* Each crate registers its products' classes, held to KeySpace::CLASS (ADR 0001)
- *(venue)* PerpetualMarket and EventContractMarket, with each product pinned to its bundle (ADR 0001)

### 🐛 Bug Fixes

- *(clob)* Markets::{prices, midpoints, spreads} decode the token-keyed map ([#70](https://github.com/dilettante-trading/polyoxide/issues/70))
- *(kalshi)* D8's one-sid membership answers, the unknown and outside ticker records, tighter checks (Epic 9, Story 1 review)
- *(kalshi)* A re-snapshot that empties re-anchors the numbering, and a snapshot request leaves no follow-up behind (Epic 9, Story 5 review)
- *(kalshi)* A change refused after a command was accepted replaces the connection (Epic 9, Story 5 follow-up)
- *(kalshi)* Removing the last market awaited keeps a numbering a snapshot set (Epic 9, Story 5 follow-up)
- *(kalshi)* Pin the public bucket's size and the key's routes, and print both buckets (Epic 9, Story 7 review fix)
- *(kalshi)* A soak worker parked in the pacer stops at the first 429 (Epic 9, Story 7 review fix)
- *(perps)* Review fixes for the [#56](https://github.com/dilettante-trading/polyoxide/pull/56) adoption
- *(registry)* The perps AsyncAPI row counts 28 channels, as its region does ([#57](https://github.com/dilettante-trading/polyoxide/issues/57))

### 🚜 Refactor

- *(kalshi)* The local book matches its remaining variants by name (Epic 9, Story 6 follow-up)
- *(kalshi)* Wire_period refuses an unserved width as ensure_served does (Epic 9, final review)

### 📚 Documentation

- *(spec)* Epic 9, Kalshi published: design
- *(plan)* Epic 9, Kalshi published: the skeleton, and the spec's probe and re-split
- *(spec)* Epic 9, the owner lets Story 3 edit the four eight-route tests
- *(spec)* Epic 9, the owner lets Story 5 rewrite two gap tests and the protocol unit tests
- *(plan)* Epic 9, the skeleton follows the owner's §13 exceptions and settles three contract names
- *(spec)* Epic 9, the owner admits ci.yml's comment for Story 8 and pre-grants Story 7's two signed-bucket tests
- *(plan)* Epic 9, Kalshi published: the nine stories drafted
- *(kalshi)* The Epic 9 section of OBSERVED.md and its anchor (Epic 9, Story 1)
- *(spec)* Epic 9, the owner clamps Kalshi's candle span to 10,000 periods; Story 1's width-5 finding
- *(kalshi)* Period_interval=5 on the batch route answers the daily candles (Epic 9, Story 1)
- *(spec)* Epic 9, the owner takes the host's outside-ticker snapshot in place of the error fixture
- *(spec)* Epic 9, an empty ticker or trade subscribe is refused, not every market; the rule stands
- *(kalshi)* The batch candlestick route omits idle periods; restore the 2026-10-19 width-5 statement (Epic 9, Story 1 review)
- *(kalshi)* The Story 1 records state only what was observed (Epic 9, Story 1 record fix)
- *(spec)* Epic 9, the owner reconnects on a change refused partway across sids
- *(kalshi)* The unsigned read soak on production (Epic 9, Story 7)
- *(kalshi)* The signed-bucket probe on the demo host (Epic 9, Story 7)
- *(spec)* Epic 9, the owner sizes Kalshi's unsigned refill at 180 tokens a second
- *(kalshi)* Correct the Epic 8 rate-limit notes and bound the signed-probe verdict (Epic 9, Story 7 review fix)
- *(kalshi)* The crate docs and READMEs describe a published crate, and the umbrella's README names Kalshi (Epic 9, Story 8)
- *(spec)* Epic 9, polyoxide-kalshi's keywords and categories
- *(claude)* Kalshi is a published venue, with its intervals, channels, re-snapshot and book (Epic 9, Story 9)
- The README, spec index, SELF-HEALING and .env.example name a published Kalshi (Epic 9, Story 9)
- *(migration)* Epic 9's consumer-impact notes (Epic 9, Story 9)
- *(spine)* Epic 9's amendments A9-1 to A9-5 (Epic 9, Story 9)
- *(deferred-work)* Close what Epic 9 settled, and file two open Kalshi rate-limit entries (Epic 9, Story 9)
- *(kalshi)* A refusal partway yields to code 9, and the daily candle's clock-change question (Epic 9, Story 9)
- Epic 9 review fixes, the CLI's venues, A9-3's five tests, the migration note, Kalshi's drift and the Books/OrderBook split (Epic 9, Story 9)
- *(kalshi)* The throttle's sizing, Snapshot answered once sent and the snapshot's cases (Epic 9, final review)
- *(kalshi)* Rewrap the predictions module's throttle paragraph at 100 columns
- *(spec)* Epic 9, D9's snapshot() is answered once the command is sent
- *(kalshi)* The demo host accepts the re-snapshot's command ids (Epic 9, final review M7)
- *(specs)* Adopt Kalshi's OpenAPI drift ([#69](https://github.com/dilettante-trading/polyoxide/issues/69))
- *(spec)* Adopt Kalshi's AsyncAPI drift ([#68](https://github.com/dilettante-trading/polyoxide/issues/68))
- *(mutants)* Re-cite and re-prove rows (e), case and (e), only a 400
- *(specs)* Adopt Kalshi's OpenAPI drift ([#69](https://github.com/dilettante-trading/polyoxide/issues/69))
- *(spec)* Adopt the perps AsyncAPI drift ([#57](https://github.com/dilettante-trading/polyoxide/issues/57))
- *(perps)* Statistic::open_price is the window's earliest candle open ([#57](https://github.com/dilettante-trading/polyoxide/issues/57))
- *(decisions)* ADR 0001, product classes over one capability core (proposed)
- *(decisions)* ADR 0001 follows Epic 9, both class bundles have two implementers
- *(plans)* ADR 0001's implementation plan, product classes and their bundles
- Product classes in the standing rules, the S3 notes and the spine amendments; ADR 0001 accepted
- *(spine)* The guide says Kalshi is published and in the umbrella (A9-1)

### 🧪 Testing

- *(kalshi)* Live_predictions_venue reads candles at each served width (Epic 9, Story 3)
- *(kalshi)* Look through the busiest hundred markets for an established one (Epic 9, Story 3)
- *(kalshi)* The live candles test fails on a missing NO candle or an unreadable open time (Epic 9, Story 3)
- *(ws)* Follow_up's tests pin the error path and stop racing staleness (Epic 9, Story 4)
- *(kalshi)* The scripted answer carries several channels and answers get_snapshot (Epic 9, Story 5)
- *(kalshi)* The supervised socket's channels, empty-set rule and re-snapshot on the scripted server (Epic 9, Story 5)
- *(kalshi)* Wire agreement for ticker, trade and the requested snapshot, and the live suite for all three channels (Epic 9, Story 5)
- *(kalshi)* The requested-snapshot live test fails untagged and ignores a replayed snapshot; reword the socket docs (Epic 9, Story 5 review)
- *(kalshi)* The local book replays the captured snapshot and deltas (Epic 9, Story 6)
- *(kalshi)* Widen the parked-worker soak test's slot to 1 s (Epic 9, Story 7 review fix)
- *(umbrella)* Live_venue reads Kalshi's Predictions, and its nightly job builds it (Epic 9, Story 8)
- *(ci)* The classify sweep knows Kalshi's error types and the candle refusal (Epic 9, Story 9)
- *(kalshi)* Pin the unknown-ticker snapshot fixture (Epic 9, final review)
- *(kalshi)* A gap's re-snapshot rebuilds the local book end to end (Epic 9, final review M6)
- *(polyoxide)* Every product the umbrella builds keys its own class (ADR 0001)

### ⚙️ Miscellaneous Tasks

- *(epic-9)* The touch-set gate every story ends with (Epic 9, Story 1)
- *(umbrella)* Allowlists for kalshi-predictions and kalshi-predictions-ws; the trees the two new umbrella features build, no existing file moves (Epic 9, Story 8)
- *(kalshi)* Keywords and categories, and the manifest comments name the three channels (Epic 9, Story 9)

## [0.41.1] - 2026-10-19

Three upstream drifts adopted, each additive: nothing is removed or renamed, no signature
changes, and [`docs/s3-removals.md`](docs/s3-removals.md) still lists nothing. One behaviour
changes, in `gamma`'s `Markets::get_many`.

- **Gamma `closed=any` (#59).** `gamma::types::ClosedFilter` (`Open`, `Closed`, `Any`) and a
  `closed_filter` setter on `ListEvents`, `ListKeysetEvents`, `ListMarkets` and
  `ListKeysetMarkets`; `closed(bool)` is unchanged. `Any` returns both states in one request; on
  the two markets routes it needs an identifier filter, or the host answers `422`.
  `Markets::get_many` (DRIFT R13) now sends `closed=any` for each 100 distinct ids, each with a
  `limit` of its ids' number, one request after the other, where it sent `closed=true` and
  `closed=false` and merged them. A call for up to 100 ids is one request where it was two, a
  longer list works where it was a `422`, and 21 to 100 open ids come back whole where they lost
  every row past the 20th. A result is in the host's order for each chunk, not closed markets
  first, and a repeated id is sent once. The CLOB's `event_group` makes one `closed=any` request
  where it made one or two. The `id` setters of `ListMarkets` and `ListEvents` documented a batch
  of 400 ids; the host takes 100.
- **CLOB `GET /v1/user/rate-limit-tier` (#60).** `AccountApi::rate_limit_tier()` returns a
  `RateLimitTierResponse` (with `RateLimitTierDetails`), whose `known_tier()` maps the venue's
  name onto the client's `Tier` when it models it. It reads the tier and does not change the one
  `Clob::tier` reports.
- **Data API `GET /v2/tokens` (#52).** `data.v2().tokens(TokenKey)` takes one selector family,
  `TokenKey::TokenIds` (at most 50) or `TokenKey::Conditions` (at most 10), and returns
  `Vec<TokenRef>`; `TokenModule` is an open enum. An unknown id is absent from the list rather
  than a `404`. The Python bindings gain `DataV2.tokens(token_ids=, conditions=)` and
  `polyoxide.v2.TokenRef`.
- `polyoxide-ws`'s doc-hidden scripted test server (feature `test-server`) records each
  handshake's request headers (`handshake_headers`).

The rest is Epic 8, `polyoxide-kalshi`, a walking skeleton for Kalshi's event-contract market data
and its order-book socket. It is `publish = false` and the umbrella does not include it, so
nothing of it reaches crates.io.

### 🚀 Features

- *(clob)* Adopt upstream's GET /v1/user/rate-limit-tier (schema drift [#60](https://github.com/dilettante-trading/polyoxide/pull/60))
- *(gamma)* Closed=any, through ClosedFilter and closed_filter
- *(data,py)* GET /v2/tokens, adopting drift issue [#52](https://github.com/dilettante-trading/polyoxide/pull/52)
- *(kalshi)* Register polyoxide-kalshi, its mirrors and its dependency rule (Story 1)
- *(kalshi)* The predictions key space (Story 1)
- *(kalshi)* PredictionsError and its one decode of Kalshi's error bodies (Story 1)
- *(kalshi)* The exchange-status types (Story 1)
- *(kalshi)* The route table and the provisional Read throttle (Story 1)
- *(kalshi)* The client, health().ping() and exchange().status() (Story 1)
- *(kalshi)* Capture the public REST fixtures (Story 8.2)
- *(kalshi)* The market, event and endpoint-cost types (Story 8.2)
- *(kalshi)* The markets, events and account namespaces (Story 8.2)
- *(kalshi)* MarketData on Predictions, one YES/NO conversion (Story 8.3)
- *(kalshi)* Trades on Predictions (Story 8.3)
- *(kalshi)* EventGroups on Predictions (Story 8.3)
- *(ws)* The scripted server records each handshake's request headers (Epic 8, Story 5)
- *(kalshi)* Throwaway signing vectors and the account capture script (Epic 8, Story 4)
- *(kalshi)* Credentials over ring and a signing authenticator (Epic 8, Story 4)
- *(kalshi)* Account().limits() and the limits types (Epic 8, Story 4)
- *(kalshi)* The first-use confirmation of costs and the Read bucket (Epic 8, Story 4)
- *(kalshi)* The demo account's limits, captured, and the signed live suite (Epic 8, Story 4)
- *(kalshi)* Capture the order-book socket's frames from the demo host (Story 6)
- *(kalshi)* The predictions-ws feature and PredictionsWsError (Story 6)
- *(kalshi)* The order-book socket's frames and events, held to the capture (Story 6)
- *(kalshi)* PredictionsProtocol and its builder, a handshake signed on every attempt (Story 6)

### 🐛 Bug Fixes

- *(gamma)* Get_many and event_group send closed=any (DRIFT R13)
- *(ci)* Report a published venue crate the umbrella forgot (Story 1 review)
- *(kalshi)* Refuse an empty, . or .. route parameter before sending (Story 1 review)
- *(ci)* List a rename's source in the touch-set gate (Story 1 review)
- *(kalshi)* Capture a full page of trades; ledger, mirror enum and route checks (Story 2 review)
- *(kalshi)* Story 3 review, one busiest_market, expiration-time decode tests, explicit taker side
- *(kalshi)* Story 2 review, rewrap an allowlist comment, TRADES_PAGE in the capture script
- *(kalshi)* Story 3 review, an instrument's name appends its outcome and the name tests can fail
- *(kalshi)* Story 4 review, the confirmation cannot wait on itself
- *(kalshi)* Story 6 review, a refused replay ends the stream and codes 17 and 18 reconnect
- *(kalshi)* Final review, a replay answered with a frame that does not decode ends the stream
- *(kalshi)* Final review, a handshake header that cannot be built is a signing error
- *(kalshi)* Final review, drop the stale dead_code allow on Routed::query
- *(kalshi)* Final review, predictions-ws turns on only what the sources use
- *(kalshi)* Final review, the uncrossed live check picks a market where it can fail

### 🚜 Refactor

- *(kalshi)* Refuse through the module's one decode; list Kalshi's row (Story 1 review)

### 📚 Documentation

- *(specs)* Epic 8, the Kalshi walking skeleton — design
- *(specs)* Epic 8's touch-set base is the run's start, after the branch merges main
- *(plans)* Epic 8's skeleton, and its spec split so stories draft apart
- *(specs)* Epic 8's market listing is filtered with mve_filter=exclude, and its status filter has paused
- *(specs)* Epic 8's signing vectors always store key bodies without armour lines
- *(specs)* Epic 8's socket reconnects on code 10, and a ticker needs a letter or digit
- *(specs)* Epic 8's AsyncAPI mirror covers 15 channels, counted as the repo counts them
- *(plans)* Epic 8's seven stories, drafted from the skeleton
- *(specs)* Kalshi's public REST findings, the filtered listing and the five status values (Story 8.2)
- *(claude)* The full-pin clause covers every published venue crate (Story 1 review)
- *(specs)* Epic 8's instrument name appends its outcome, and Story 5's recorder takes DRIFT R15
- *(specs)* Epic 8's DRIFT R15 also moves the test-body-diff config test
- *(claude)* The scripted server records handshake headers (Epic 8, Story 5)
- *(ws)* Story 5 review, the header recorder's order and two wordings
- *(specs)* Epic 8's signed public reads may not draw from the account's bucket; a probe is deferred
- *(claude)* Kalshi's order-book socket, its live targets and its allowlist file (Story 6)
- *(specs)* Epic 8's socket ends on a refused replay, and codes 17 and 18 reconnect
- *(kalshi)* Story 6 review, the order of the book's sides is the evidence for YES pricing
- *(specs)* Epic 8's socket also ends on a replay answer that does not decode
- *(specs)* The Kalshi trading mapping's keys are kalshi.predictions and its size unit is shares (Epic 8, Story 7)
- *(deferred-work)* Re-file the Kalshi mapping's move and add Epic 8's deferred work (Epic 8, Story 7)
- *(spine)* Epic 8's amendments A8-1 to A8-11 and the audit of its diff (Epic 8, Story 7)
- *(spine)* Epic 8 final review, A8-11 states the guide's step and the kit's Skip, and a replay answer that does not decode stops the stream
- *(kalshi)* Final review, the crate docs name the order-book socket and its credentials
- *(specs)* Epic 8's spec matches the run: the socket feature, A8-11, the recorder's order and the touch-set command

### 🧪 Testing

- *(ci)* Require Kalshi's allowlist and its reqwest row (Story 1 review)
- *(kalshi)* Pin that a 429 is retried (Story 1 review)
- *(kalshi)* Hold the types and query keys to the OpenAPI mirror (Story 8.2)
- *(kalshi)* The public REST live suite against production (Story 8.2)
- *(kalshi)* Name the error-class check's type for clippy (Story 8.2)
- *(kalshi)* Live_predictions_venue, the traits against production (Story 8.3)
- *(kalshi)* The supervised order-book socket on the kit's scripted server (Story 6)
- *(kalshi)* Live_predictions_ws, the order-book socket against the demo host (Story 6)

### ⚙️ Miscellaneous Tasks

- *(epic-8)* The touch-set gate every story ends with (Story 1)
- *(kalshi)* Pin the header set of Kalshi's requests in the features job (Story 1)

## [0.41.0] - 2026-10-18

The first release of stage S3 of the multi-venue restructure, the additions stage: venue-neutral
market data (Epic 6) and supervised CLOB sockets with normalized trading (Epic 7). Both are
additive. Nothing is removed or renamed, and no signature changes; code that builds on 0.40
builds unchanged, and [`docs/s3-removals.md`](docs/s3-removals.md) lists nothing. What follows
is each epic's consumer-impact note from [`docs/s3-migration.md`](docs/s3-migration.md).

**Epic 6: venue-neutral market data.**

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

**Epic 7: supervised CLOB sockets and normalized trading.**

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

Where Epic 7 departs from its acceptance criteria:

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

### 🚀 Features

- *(venue)* Canonical market keys that only a product's KeySpace makes (Story 6.2)
- *(binance)* Usdm::key, USDⓈ-M's market keys over Symbol (Story 6.2)
- *(polymarket)* Shared::keys, the clob's and perps' market keys (Story 6.2)
- *(umbrella)* Parse_key reads a key through its product's constructors (Story 6.2)
- *(venue)* The shared records and their typed Extensions (Story 6.1)
- *(venue)* The market-data traits, with the records they return (Story 6.3)
- *(binance)* USDⓈ-M through the market-data traits (Story 6.3)
- *(perps)* Perps through the market-data traits (Story 6.4)
- *(clob)* The CLOB through the market-data traits, its trades through Data (Story 6.5)
- *(umbrella)* One best_bid_ask and one classifier across the CLOB, perps and USDⓈ-M (Story 6.6)
- *(venue)* Epic 6 final review, a funding rate carries its interval
- *(ws)* Protocol::connect, the dial the Supervisor calls for every attempt (Epic 7, Story 1)
- *(polymarket)* The supervised sockets' shared items and the scripted PONG answerer (Epic 7, Story 2)
- *(polymarket)* The supervised CLOB market socket (Epic 7, Story 2)
- *(polymarket)* Supervised CLOB user socket (Epic 7, Story 3)
- *(polymarket)* The user socket stops on a refused login (Epic 7, Story 3)
- *(venue)* The trading feature and the opaque order, fill and client-order ids (Epic 7, Story 4)
- *(venue)* Extensions::ensure_declared and UndeclaredExtension (Epic 7, Story 4)
- *(venue)* The Trading trait, TradingEvents and the trading records (Epic 7, Story 4)
- *(clob)* ClobOrderOptions, the trading Ext types and order_hash (Epic 7, Story 5)
- *(clob)* Trading::place with the client-id ledger and its checks (Epic 7, Story 5)
- *(clob)* Trading cancel, open orders, positions and balances (Epic 7, Story 5)
- *(clob)* Trading::events on the supervised user socket (Epic 7, Story 5)
- *(clob)* Trading through the umbrella, its docs and the registry (Epic 7, Story 5)

### 🐛 Bug Fixes

- *(venue)* Story 6.2 review fixes, id_of private, sound test ids, token id doc, id charset test
- *(perps)* Story 6.4 review fixes, funding_history asks for a 100-hour window, tests for the funding filter, foreign keys, error carry-over, zero quantity, close-only, open-ended candles
- *(clob)* Story 6.5 review fixes, event_group finds a resolved market's token, candles stay inside the 14-day window, tests for the v1 filter, length mismatch and NotListed
- *(scripts)* Story 6.5 review fixes, the CLOB capture checks a book is a JSON object before reading it
- *(umbrella)* Story 6.6 review fixes, the live reads pick an instrument with both sides on the quote's own route, the mock suite matches a module enum, the classifier names its rejections
- *(clob)* Epic 6 final review, a token id Gamma sends that no key can carry is a Decode failure
- *(clob)* Epic 6 final review, fidelity still refuses a width that is not whole minutes
- *(perps)* Epic 6 final review, the funding window holds at most 100 settlements whatever their offset
- *(perps)* Epic 6 final review, perps sizes are Base
- *(perps,usdm)* Epic 6 final review, statistics volume is USD and a zero funding interval is refused
- *(binance)* Usdm instruments skips an exchangeInfo row that does not convert
- *(polymarket)* The unified client hands clob its Gamma client
- *(polymarket)* Epic 7, Story 2 review fixes: dial IP literals and scheme-default ports, one subscription-count rule, untested paths pinned
- *(venue)* Trading::events doc, a Debug and a size_hint test for TradingEvents (Epic 7, Story 4 review fixes)
- *(clob)* A maker fee rate that is not a number is a Decode (Epic 7, Story 5 review fixes)
- *(clob)* Trading checks before the tick lookup, decodes an unknown tick, tests the ledger and WARNs (Epic 7, Story 5 review fixes)
- *(clob)* The account is a trade's taker only when no maker entry is its own (Epic 7, Story 5 review fixes, D17)
- *(clob)* The client-id ledger keeps an order's entry after a Decode failure (Epic 7, Story 5 review fixes, D18)
- *(polymarket)* The live trading round trip cancels a bid that place or cancel may have left resting (Epic 7, Story 6 review fixes)
- *(polymarket)* Live trading and market socket tests end a missing ack or book as transient (Epic 7, Story 6 review fixes)
- *(polymarket)* The client-id ledger keeps an order's id until evicted and drops only its reuse guard (Epic 7, final review, D21)
- *(polymarket)* A matched FillOrKill read back short of its size is Cancelled with what matched (Epic 7, final review, D22)
- *(polymarket)* The user channel's timestamp is read in seconds below 10^11 and milliseconds otherwise (Epic 7, final review, D23)
- *(polymarket)* The ledger's rekey replaces an entry held under its new id, guard-aware (Epic 7, final review, D21 follow-up)

### 🚜 Refactor

- *(clob)* Epic 6 final review, fidelity is the interval's duration in minutes
- *(venue)* CandleInterval is exhaustive
- *(venue)* NotListed is non_exhaustive and built with NotListed::new
- *(clob)* Split dial out of connect_ws and raise four helpers to pub(super) (Epic 7, Story 1)
- *(polymarket)* The market and user decoders share one array-frame rule, leading whitespace allowed (Epic 7, final review minors)

### 📚 Documentation

- CLAUDE.md names the dev shell, and flake.nix as the pinned pair's third home
- *(plans)* Supervised flow plan for Epic 6, venue-neutral market data
- *(epics)* Epic 6 is built at stage S2, ahead of the S2 release
- CLAUDE.md states the market-key rules (Story 6.2)
- CLAUDE.md states the record and Extensions rules (Story 6.1)
- Amendment A6-1 and CLAUDE.md's market-data trait rules (Story 6.3)
- CLAUDE.md says what perps::venue implements (Story 6.4)
- Story 6.4 review fixes, perps funding window in OBSERVED.md and CLAUDE.md, pointer to perps' traits
- *(mutants)* Move rows (e) after ClobError::Data and prove them again (Story 6.5)
- CLAUDE.md says what clob::venue implements and where its trades come from (Story 6.5)
- Amendment A6-2, Epic 6's consumer-impact note and the nightly venue suite (Story 6.6)
- Story 6.6 review fixes, s3-migration.md lists everything Epic 6 added and CLAUDE.md points at it
- Epic 6 final review, an unlisted key is a VenueRefusal by class and a bounded span is read on
- Epic 6 final review, a bounded span reads on from where the venue stopped
- Epic 6 final review, the last wording of the trait and funding-window docs
- *(epics)* Epic 6 funding records carry their interval, and perps sizes are Base
- *(venue)* Epic 6 final review, the trait docs name no venue
- *(epics)* The owner settles Epic 6's deferred calls
- The S3 note covers clob's Gamma client, and CLAUDE.md's usdm paragraph is rewrapped
- *(plans)* Epic 7's design and plan skeleton, supervised CLOB sockets and trading
- *(specs)* Epic 7's live trading target declares the secrets it reads
- *(specs)* Epic 7's D15 keeps the reconnect after a use after close
- *(specs)* Epic 7's Trading::place supports Deposit Wallet accounts
- *(specs)* Epic 7's order_hash test pins the v2_exchange vector
- *(specs)* Epic 7's Trading signs and reads with the client's signature type
- *(plans)* Epic 7's plan, drafted story by story
- *(claude)* The supervised CLOB user socket (Epic 7, Story 3)
- *(polymarket)* A user outage may end on a stop error without Reconnected (Epic 7, Story 3 review fixes)
- *(polymarket)* Name only refused reconnects as ending an outage (Epic 7, Story 3 review fixes)
- *(venue)* The trading trait in CLAUDE.md, the venue README and the registry lists (Epic 7, Story 4)
- *(specs)* Epic 7's taker rule needs no maker leg, and the ledger keeps an entry after a Decode failure
- *(epic-7)* The Kalshi trading mapping, ported onto the Trading trait (Story 7)
- *(epic-7)* Spine amendments A7-1 to A7-4, and 7.5's no amendment (Story 7)
- *(s3)* Epic 7's consumer-impact note (Story 7)
- *(epic-7)* Deferred-work closes the close-code entry, narrows the prune entry and adds the Kalshi moves and the maker-frame capture (Story 7)
- *(epic-7)* Divergences.md's clob rows say what the supervised sockets do (Story 7)
- *(epic-7)* Wording fixes to the release note, the amendments and the records (Epic 7, Story 7 review fixes)
- *(specs)* Epic 7's final-review calls: the ledger keeps ids until evicted, a short FOK is Cancelled, timestamps in either unit
- *(deferred-work)* The maker-frame capture also settles the user channel's timestamp unit (Epic 7, final review, D23)
- *(venue,ws,polymarket)* The events stream may open with its error, and a per-address connect needs no handshake (Epic 7, final review minors)
- *(s3-migration)* The outage marker has no rotation, and the IPv6 note ends its sentence (Epic 7, final review minors)
- *(venue)* The events stream's first item is Resync unless the stream cannot open, everywhere it is stated (Epic 7, final review minors)
- *(specs)* Epic 7's live round trip does not check the computed order id
- *(s3-migration)* The events stream's first item is Resync unless the stream cannot open (Epic 7, final review minors)

### ⚡ Performance

- *(binance)* Usdm trades, candles and funding history send explicit limits

### 🧪 Testing

- *(support)* Registration!() reads the ids a crate registers (Story 6.2)
- *(venue)* Story 6.2 review fixes, Upper tests build keys through an upper helper, no shim
- *(venue)* Story 6.1 review fixes, Extensions has no serde doctests, instrument constructor checks name and assets
- *(support)* Or_fail and fail_classified tag a ClassifiedError (Story 6.3)
- *(binance)* Story 6.3 review fixes, zero next funding, foreign keys on every trait, open-ended candles, no mark price, error carry-over; docs: EventOutcome carries no Extensions
- *(clob)* Capture the CLOB, Data and Gamma bodies the venue tests serve (Story 6.5)
- *(umbrella)* Story 6.6 review fixes, the live CLOB walk skips only a token with no book (404)
- *(binance)* Usdm instruments' skipped-row WARN is asserted
- *(binance)* The live instruments test compares symbols exactly and tolerates a listing change
- *(venue)* A doctest pins NotListed's non_exhaustive
- *(polymarket)* A user change on a closed connection is answered Ok (Epic 7, Story 3 review fixes)
- *(polymarket)* Epic 7: hold a second tracing dispatcher in the clob supervised tests
- *(ci)* Pin the features job's clob trading without gamma step (Epic 7, Story 5 review fixes)
- *(clob)* D17's maker check ignores the case of the maker address (Epic 7, Story 5 review fixes)
- *(polymarket)* Live tests for the supervised market and user sockets (Epic 7, Story 6)
- *(polymarket)* Live suite for Trading on the CLOB, and its nightly registration (Epic 7, Story 6)
- *(polymarket)* The live round trip's comment says what its client-id check proves (Epic 7, final review, D21 follow-up)

### 🔧 Build

- A nix dev shell holds the toolchains and cargo tools CI installs

### ⚙️ Miscellaneous Tasks

- *(stage)* Switch to S3 after the v0.40.0 release
- *(clob)* Run trading_clob without gamma (Epic 7, Story 5 review fixes)

## [0.40.0] - 2026-10-17

Stage S2 of the multi-venue restructure, the renames stage: every crate merge, module move
and rename the restructure needs ships in this one release. `polyoxide-clob`, `-gamma`,
`-data`, `-relay`, `-perps`, `-rtds` and `-sports` are the modules of one new crate,
`polyoxide-polymarket`, each behind the feature of its name. `polyoxide-core` names no venue
(Polymarket's tables, retry policy, throttle and signers are `polyoxide_polymarket::shared`),
`polyoxide-binance` puts everything under `usdm`, the `polyoxide` umbrella is multi-venue
(`polyoxide::polymarket` and `polyoxide::binance`), and the CLI's commands are
`polyoxide <venue> <module> <verb…>`. `polyoxide-polymarket` is published for the first time,
and the seven retired crates publish one last version, 0.40.0, as a tombstone whose build
stops with a `compile_error!` naming its replacement. No old version is yanked.

**Breaking changes.** A move breaks the old path, with no shim, alias or re-export (AD-16).
[`docs/s2-migration.md`](docs/s2-migration.md) is the upgrade checklist,
[`docs/rename-manifest.md`](docs/rename-manifest.md) gives every public path, feature and CLI
command that moved or went, and [`docs/rename-map.tsv`](docs/rename-map.tsv) expands it to
every path, one per line. Errors named for their crate take their module's name
(`DataApiError` → `DataError`, `WebSocketError` → `ClobWsError`, `BinanceError` →
`UsdmError`), keeping their text and classes, and the unified client's `PolymarketError` is
`BuildError`.

**Behaviour that changes** (details in the migration guide):

- Library behaviour does not change: every moved test kept its body (AD-12). Log lines take
  their module's new path as target, so a `RUST_LOG` filter naming an old crate matches
  nothing.
- CLI: `--format pretty`, the default, prints one line per event on market and user; a stream
  error that cannot be skipped exits non-zero; a closed stdout ends a stream quietly; every
  list argument splits on commas; `credentials store` checks every value before writing any.
- `data.pnl()` and `data.rankings()` are deprecated, since Polymarket retired their hosts
  (DRIFT R12).

### 🚀 Features

- *(polymarket)* Polyoxide-polymarket takes core's Polymarket code (Story 5.2)
- *(cli)* One stream runner, OutputFormat and excerpt for every socket command (Stories 5.8 and 5.9)
- *(cli)* [**breaking**] The command tree on the venue axis, every socket a stream on one runner (Stories 5.8 and 5.9)
- *(core)* StoredCredential, with generic store, show and delete_all over the keychain (Stories 5.8 and 5.9)
- *(polymarket)* StoredCredential for the clob and relay accounts (Stories 5.8 and 5.9)
- *(cli)* [**breaking**] Polymarket credentials over StoredCredential, its kinds clob and builder (Stories 5.8 and 5.9)
- *(release)* Tombstones for the seven retired crates, and their check (Stories 5.11 and 5.12)
- *(release)* The S3 removal gate, keyed by stage (Stories 5.11 and 5.12)

### 🐛 Bug Fixes

- *(ci)* The venue gate catches plural acronyms, digit products and src/target (Story 5.2 review)
- Review findings on gamma and data's move (Story 5.3 review)
- *(docs)* Clob's timestamp link resolves in the module, not through the crate name (Story 5.4)
- Review findings on clob and relay's move (Story 5.4 review)
- *(tests)* Clob's refusal test goes last, so the cited kill tests keep their lines (Story 5.4 review)
- Review findings on perps, rtds and sports's move (Story 5.5 review)
- Review findings on Binance's conventions (Story 5.6 review)
- Review findings on the multi-venue umbrella (Story 5.7 review)
- Review findings on the venue-axis CLI and shared credentials (Stories 5.8 and 5.9 review)
- *(release)* The tombstones keep their old features and say where each went (Stories 5.11 and 5.12)
- *(release)* Check-manifests names missing metadata before the tombstones' check (Stories 5.11 and 5.12)
- *(ci)* The tombstone build test reads cargo's messages uncoloured, and a stream test's bail! ends in a semicolon (Stories 5.11 and 5.12)
- *(data)* Pnl() and rankings() are deprecated, since Polymarket retired their hosts (DRIFT R12)

### 🚜 Refactor

- Gamma, data, perps, relay and clob take Polymarket's shared code from polyoxide-polymarket (Story 5.2)
- Gamma and data become polyoxide-polymarket's modules (Story 5.3)
- Clob and relay become polyoxide-polymarket's modules (Story 5.4)
- *(rtds)* [**breaking**] Test-fixtures becomes test-server (DRIFT R6)
- Perps, rtds and sports become polyoxide-polymarket's modules (Story 5.5)
- *(binance)* [**breaking**] Usdm holds the error and weight budget, behind its own features (Story 5.6)
- The umbrella goes multi-venue, and the unified client moves into polyoxide-polymarket (Story 5.7)

### 📚 Documentation

- S1 shipped as v0.39.0; the workspace is in stage S2 (Story 4.11)
- *(planning)* Story 5.1 done and to review; Epic 5 context and the S2 bundle plan
- *(planning)* PR [#55](https://github.com/dilettante-trading/polyoxide/pull/55)'s first CI run settles review 1's G19 (Story 5.1)
- *(planning)* Story 5.2's spec, polyoxide-polymarket and a venue-neutral core
- The rules and ledger that named core's Polymarket paths follow them (Story 5.2)
- *(planning)* Story 5.3's spec, gamma and data become modules
- The rules, specs and registry follow gamma and data (Story 5.3)
- *(mutants)* Re-cite and re-prove the rows gamma and data moved (Story 5.3)
- *(planning)* Story 5.3 done and to review, with its triage log (Story 5.3)
- *(planning)* Story 5.4's spec, clob and relay become modules
- The rules, specs and registry follow clob and relay (Story 5.4)
- *(mutants)* Re-cite and re-prove the rows clob moved (Story 5.4)
- *(planning)* Story 5.4 done and to review, with its triage log (Story 5.4)
- *(planning)* Story 5.5's draft spec, perps, rtds and sports become modules
- *(planning)* Story 5.5's spec approved, in progress
- *(planning)* Story 5.5's R6 task is done; a fresh implementer resumes
- The rules, specs and registry follow perps, rtds and sports (Story 5.5)
- *(mutants)* Re-cite and re-prove the row perps, rtds and sports moved (Story 5.5)
- *(perps)* OBSERVED names the agreement suites by their new names (Story 5.5)
- *(planning)* Story 5.5 done and to review, with its triage log (Story 5.5)
- *(planning)* Story 5.6's spec approved, in progress
- The rules, specs and registry follow Binance's usdm module (Story 5.6)
- *(mutants)* Re-cite and re-prove the rows Binance's suites moved (Story 5.6)
- *(planning)* Story 5.6's tasks done, with its implementation notes (Story 5.6)
- *(planning)* Story 5.6's review 1 triage log and deferred items (Story 5.6 review)
- *(planning)* Story 5.6 done and to review (Story 5.6)
- *(manifest)* The umbrella's paths and features, and the retired PolymarketError test (Story 5.7)
- The multi-venue umbrella in the READMEs, the specs INDEX and CLAUDE.md (Story 5.7)
- *(planning)* Story 5.7 done and to review, with its triage log and deferred items (Story 5.7)
- *(planning)* Stories 5.8 and 5.9's spec approved, ready for dev
- *(manifest)* The CLI's paths and command tree, and its retired tests (Stories 5.8 and 5.9)
- The CLI on the venue axis and its credentials command in the READMEs and CLAUDE.md (Stories 5.8 and 5.9)
- *(planning)* Stories 5.8 and 5.9 done and to review, with their triage log and deferred item (Stories 5.8 and 5.9)
- *(planning)* Drop Story 5.10, Python on the venue axis (spine amendment A5-1)
- *(planning)* Epic 5 context keeps polyoxide.v2 and lists A5-1 among unmerged amendments
- *(planning)* Spec for Stories 5.11 and 5.12, approved and ready for development
- The four guides describe a multi-venue toolkit (Stories 5.11 and 5.12)
- The S2 migration guide, README pins at 0.40, and the deferred items settled (Stories 5.11 and 5.12)
- The frozen S2 rename map (Stories 5.11 and 5.12)
- The twenty mutants proved again on the S2 hub's final tree (Stories 5.11 and 5.12)
- The migration guide's checklist, prelude and map notes, and S3's removals agree (Stories 5.11 and 5.12)
- *(planning)* Stories 5.11 and 5.12 done and to review, with their triage log and deferred items (Stories 5.11 and 5.12)

### 🧪 Testing

- The S3 listing's starting state is checked only in stage S2 (Stories 5.11 and 5.12)

### ⚙️ Miscellaneous Tasks

- The S2 gate reports into a rename manifest, and moved tests keep their bodies (Story 5.1)
- The moved tests pair with 0 diffs, and `[gates]` takes a cfg predicate (Story 5.2)
- The venue gate, and the foundation crates name no venue (Story 5.2, CAP-7)
- *(registry)* `identifiers` and `gate_exceptions`, and a venue's own crate (Story 5.2)
- The body diff and the rename manifest follow gamma and data (Story 5.3)
- The scripts job follows gamma and data into polyoxide-polymarket (Story 5.3)
- The scripts job follows clob and relay into polyoxide-polymarket (Story 5.4)
- *(body-diff)* `[fixtures]` reads a moved crate's `fixtures!` calls under its module (Story 5.5)
- The scripts job follows perps, rtds and sports into polyoxide-polymarket (Story 5.5)
- The body diff and the scripts job follow Binance's usdm module (Story 5.6)
- *(deps)* Fence Binance's usdm build from the socket stack (Story 5.6)
- Pin the venue headers under the umbrella's full, and fence its socket passthroughs (Story 5.7)
- The removals job runs the S2 gate with --strict (Stories 5.11 and 5.12)
- *(nightly)* Only a run on main files, comments on or closes the behavioral issue (Stories 5.11 and 5.12)
- *(nightly)* A run off main shows its real failures in the summary and goes red (Stories 5.11 and 5.12)
- The removals job holds the frozen rename map to the one it writes (Stories 5.11 and 5.12)

## [0.39.0] - 2026-10-12

Stage S1 of the multi-venue restructure: the internals every venue crate shares are
consolidated, and the crate names stay. Every error implements `polyoxide_venue::Classify`
(its class, whether it is a fault, and the wait the server asked for), which replaces each
inherent `is_retriable` and `retry_after`. Every HTTP request goes through core's one send
loop, and every WebSocket crate runs on the new socket kit, `polyoxide-ws`.
`polyoxide-venue`, `polyoxide-ws` and `polyoxide-cli` are published for the first time. The
renames and crate merges come in S2.

**Breaking changes.** Each removed path and its replacement is in
[`docs/s1-removals.md`](docs/s1-removals.md), and
[`docs/s1-migration.md`](docs/s1-migration.md) explains the call-site changes. A caller of
`is_retriable()` or `retry_after()` on an error needs `use polyoxide_venue::Classify` and a
direct dependency on `polyoxide-venue` 0.39. Core's `ApiError::Response(Box<ErrorResponse>)`
replaces `Api`, `Authentication`, `RateLimit` and `Timeout`. Socket errors wrap
`polyoxide_ws::WsError`, which is read through `kind()`.

**Behaviour that changes** (details in the migration guide):

- DRIFT R1: a reconnect refused with 408, 425, 429 or 5xx is retried. perps adds 408 and 425,
  and rtds no longer ends `run()` on a 503 or 429.
- R2: every socket connect gives up after 10 s by default (perps, rtds).
- R3: the server's close is answered with the RFC 6455 close reply (perps, Binance, rtds,
  clob).
- R4: one `Retry-After` parser. A zero is no wait (`None`), and fractional and padded values
  parse.
- R11: a reconnect after a refusal waits out its `Retry-After`, up to the backoff ceiling.
- An upgrade refused with 451 is `Restricted` and not a fault, on every socket crate.
- Python exceptions are chosen by class; `UnavailableError`, `RestrictedError` and
  `DecodeError` are new.
- Log targets: retries, holds and decode failures log under `polyoxide_core`. The supervised
  sockets' reconnects (perps, Binance) and rtds' bare-tier close log under `polyoxide_ws`.

### 🚀 Features

- *(release)* Publish what crates.io lacks, from one resumable loop
- *(registry)* One registration source, generated docs and nightly rows
- *(venue)* One classification for every polyoxide error
- *(test-support)* Failure tags at the source; the classifier reads them first
- *(core)* One send loop with Throttle, RetryPolicy and Authenticator hooks (Story 3.1)
- *(core)* Warn on a hold that is not a retry (DRIFT R10)
- *(core)* WindowQuotaTable and Polymarket's tables in polymarket (Story 3.2)
- *(core)* Hold, the cooldown as a shared handle, and AD-23's re-check (Story 3.3)
- *(core)* CapacityBucket, and the signer layer rebuilt on it (Story 3.3)
- *(core)* Polymarket::ClobThrottle composes the CLOB's two layers (Story 3.3)
- *(core)* What clob, relay and Binance need from the loop (Story 3.4, G3)
- *(clob)* Clob on the one send loop (Story 3.4)
- *(binance)* Binance on the one send loop (Story 3.6)
- *(py)* Exceptions by class, not by message (Story 3.12, J4)
- *(ws)* The socket kit crate (Story 4.1, K1)
- *(ws)* Connect, the bare tier and request correlation, once (Story 4.2, K2)
- *(venue)* One socket classification table, impl_ws_classification! (Story 4.3, K4)
- *(ws)* One scripted test server, behind test-server (Story 4.4, K5)
- *(ws)* The generic Supervisor for a venue Protocol (Story 4.5, L1)
- *(perps)* [**breaking**] The bare tier on the kit; DRIFT R1 and R2 (Story 4.6, L3)
- *(perps)* [**breaking**] The supervised tier on the kit's Supervisor (Story 4.6, L4)
- *(perps)* [**breaking**] Delete perps' supervision loop and scripted server (Story 4.6, L5)
- *(ws)* Protocol::configure and an owned Outage (Story 4.7, L2-2)
- *(binance)* [**breaking**] The bare tier on the kit; DRIFT R3 (Story 4.7, L2-3)
- *(binance)* [**breaking**] The supervised tier on the kit's Supervisor (Story 4.7, L2-4)
- *(binance)* [**breaking**] Delete Binance's supervision loop and scripted server (Story 4.7, L2-5)
- *(rtds)* [**breaking**] Connect through the kit with a timeout; DRIFT R2 (Story 4.8, L2-7)
- *(rtds)* [**breaking**] Retry a refusal the host can recover from; DRIFT R1 (Story 4.8, L2-8)
- *(rtds)* [**breaking**] Read through the kit's Connection; DRIFT R3 (Story 4.8, L2-9)
- *(rtds)* [**breaking**] Delete rtds' Backoff copy and scripted server (Story 4.8, L2-10)
- *(sports)* [**breaking**] The kit's blocks; DRIFT R9; delete sports' copies (Story 4.8, L2-12)
- *(ws)* Connect_over, the upgrade of a caller's own TCP connection (Story 4.9, M-2)
- *(clob)* [**breaking**] Read through the kit's Connection; WebSocketError on the socket table; DRIFT R3 (Story 4.9, M-3)
- *(clob)* [**breaking**] Delete clob's TLS copy and its direct transport; DRIFT R9 (Story 4.9, M-4)
- *(sports)* [**breaking**] The kit's connect, Connection and retrying_can_fix; SportsError on the socket table (Story 4.10, M-5)
- Every socket error surfaces a refusal's Retry-After (Story 4.10, M-6)
- [**breaking**] A reconnect waits out a refusal's Retry-After; DRIFT R11 (Story 4.10, M-7)
- *(binance)* [**breaking**] UsdmWsError::Stopped keeps the error a path stopped on (Story 4.10, M-8)

### 🐛 Bug Fixes

- *(deps)* Declare rustls once, with std, for every socket crate (DRIFT R5)
- *(core)* Bundle F review patches
- DRIFT R4, one Retry-After parser (Story 3.10)
- DRIFT R8, the four unlooped sends go through the loop
- *(relay)* DRIFT R7, relay on the one loop and RelayError wraps ApiError
- Bundle G's review (Stories 3.4, 3.5, 3.6 and 3.10)
- Bundle I's review (Stories 3.7, 3.8 and 3.9)
- Bundle J's review (Stories 3.11, 3.12 and 3.13)
- *(ws)* DRIFT R3, the bare tier sends the RFC 6455 close reply
- Bundle K's review (Stories 4.1 to 4.4)
- Bundle L1's review 1, G1 to G16 (Stories 4.5 and 4.6)
- Bundle L2's review 1, G1 to G12 (Stories 4.7 and 4.8)
- Bundle M's review 1, G1 to G10 and G12 (Stories 4.9 and 4.10)

### 🚜 Refactor

- *(live-tests)* Every live failure carries a tag; the regex classifier is gone
- *(test-support)* One copy of each agreement, fixture, soak and capture helper
- One decode_json for the four decode-and-log copies (H3)
- One retriable-status rule (Story 3.10, H12)
- *(relay)* Relay's credentials in Secret (Story 3.5)
- *(core)* Remove should_retry, note_rate_limited and acquire_rate_limit
- One client builder over core's ClientConfig (Story 3.7, I1)
- One namespace pattern through core's namespaces! (Story 3.7, I2)
- One health ping, HttpClient::health (Story 3.7, I3)
- One query-setter macro, and perps on it (Story 3.8, I5)
- *(gamma)* 196 query setters onto query_setters! (Story 3.8, I6)
- *(data)* 153 query setters onto query_setters! (Story 3.8, I7)
- *(clob, binance)* Query setters onto query_setters! (Story 3.8, I8)
- One wire-enum vocabulary in polyoxide-venue (Story 3.9, I9)
- Positional decimal serde in polyoxide-venue (Story 3.9, I10)
- UnixMillis::now() in polyoxide-venue (Story 3.9, I11)
- One ApiError::Response, and each module's From<ApiError> its decode (Story 3.11, J1)
- The Polymarket modules' removals (Story 3.11, J2)
- *(binance)* Binance's decode and its inherent methods gone (Story 3.12, J3)

### 📚 Documentation

- *(planning)* Sprint status for the venue-extensibility restructure
- *(planning)* Story 1.1 spec and the Epic 1 context
- *(planning)* Story 1.1, re-baseline on v0.38.1
- *(planning)* Story 1.2 spec
- *(planning)* Bundle spec for Stories 1.4 and 1.5
- *(planning)* Bundle spec for Stories 1.6, 1.7 and 1.8
- *(planning)* Epic 2 context and the bundle spec for Stories 2.1 and 2.2
- *(planning)* Bundle spec for Stories 2.3 and 2.4
- *(planning)* Defer the socket-side 451 fault rule to Story 4.3
- *(planning)* Bundle spec for Stories 2.5, 2.6 and 2.7
- *(planning)* Bundle spec for Stories 2.8, 2.9 and 2.10
- *(planning)* Epic 3 context
- *(planning)* Bundle spec for Stories 3.1, 3.2 and 3.3
- *(planning)* Start bundle F (Stories 3.1–3.3)
- *(planning)* Bundle F (Stories 3.1–3.3) done
- *(planning)* Bundle spec for Stories 3.4, 3.5, 3.6 and 3.10
- *(planning)* Start bundle G
- *(planning)* Bundle spec for Stories 3.7, 3.8 and 3.9
- *(planning)* Start bundle I
- *(planning)* Bundle spec for Stories 3.11, 3.12 and 3.13
- *(planning)* Start bundle J
- *(planning)* Bundle J's implementation notes
- *(planning)* Bundle J done; Stories 3.11, 3.12 and 3.13 to review
- *(planning)* Bundle K's spec, Stories 4.1 to 4.4, and Epic 4's context
- *(planning)* Start bundle K
- The socket kit in CLAUDE.md, Epic 4's amendments and release notes (K6)
- *(planning)* Bundle K done; Stories 4.1 to 4.4 to review
- *(planning)* Bundle L1's spec, Stories 4.5 and 4.6; Epic 4's context recompiled
- Perps' observed keep-alive, bundle L1's notes and A4-4 (Story 4.6, L6)
- *(planning)* Bundle L1 done; Stories 4.5 and 4.6 to review
- *(planning)* Bundle L2's spec, Stories 4.7 and 4.8; Epic 4's context recompiled
- Bundle L2's notes, deferred work and the kit's basis notes (Stories 4.7 and 4.8, L2-13)
- *(planning)* Bundle L2 done; Stories 4.7 and 4.8 to review
- *(planning)* Record Lane B, S3's venue-only groundwork in a second session
- *(planning)* Bundle M's spec, Stories 4.9 and 4.10
- *(planning)* Bundle M in progress
- Bundle M's deferred work (Stories 4.9 and 4.10, M-10)
- *(planning)* Bundle M's implementation notes (Stories 4.9 and 4.10)
- *(spec)* Divergence D20 and DRIFT R11; D10 and D19 restated (Stories 4.9 and 4.10, M-10)
- *(spec)* D19 states sports' reconnect as any lost connection (bundle M review, B1)
- *(planning)* Bundle M done; Stories 4.9 and 4.10 to review
- The S1 migration guide; s1-removals made final (Story 4.11)

### 🧪 Testing

- *(core)* Pin the reserved tenth and record the signer layer's allow_burst
- *(core)* The table tests assert through public API (Story 3.2)
- *(core)* Move the table tests to tests/polymarket_limits.rs (Story 3.2)
- *(test-support)* A token-cost throttle built outside core (CAP-2)
- *(binance)* Row (k), a 418 holds every request on the budget (G0)
- *(ci)* Fence governor out of every venue crate (Story 3.6)
- *(core)* The transitional methods' tests on public API (AD-12)
- Bundle G's matrix rows asserted end to end
- Golden query-setter tests before any setter moves (Story 3.8, I4)
- Bundle I's matrix rows asserted end to end
- Wire-level class pins for every HTTP module (Story 3.11, J0)
- One header set pinned per HTTP module, minimal and workspace builds (Story 3.13, J6)
- Bundle J's matrix rows asserted end to end
- *(perps)* The ws suites run on the shared scripted server (Story 4.6, L2)
- *(binance)* The ws suites run on the shared scripted server (Story 4.7, L2-1)
- *(rtds)* The suites run on the shared scripted server (Story 4.8, L2-6)
- *(sports)* The suites run on the shared scripted server (Story 4.8, L2-11)
- *(clob)* The ws membership tests run on the shared scripted server (Story 4.9, M-1)

### 🔧 Build

- Reqwest owned by core, and the fence (Story 3.13, J5)

### ⚙️ Miscellaneous Tasks

- *(bmad)* Commit the BMad config, ignore its runtime
- Package every crate on every PR
- MSRV, per-feature and removal gates; the agent guide and mutant ledger
- The transport owner fence and the socket-table fence (Story 4.10, M-9)
- The scripts job runs cargo tree without the sccache wrapper

## [0.38.2] - 2026-10-08

`Event::game_id` reads a negative `gameId` as `None`. Gamma sends `-1` on at
least one event that is not a game (`blue-wave-in-2026`), and on 0.38.0 and
0.38.1 that one event failed the whole `GET /events` or `/events/keyset` page
it was on. The field is still `Option<u64>`. A walk of 54,528 events found no
other value that `Event` could not decode. Everything else in the workspace is
unchanged from 0.38.1.

### 🐛 Bug Fixes

- *(gamma)* Read a negative Event::game_id as None

### 📚 Documentation

- *(planning)* Venue-extensibility spec, architecture spine and epics

## [0.38.1] - 2026-10-08

Adds `ListKeysetEvents::include_markets`, which `ListEvents` has had since
0.32.1. `include_markets(false)` drops each event's `markets` key from
`GET /events/keyset`: 100 open sports events came to 366 KB instead of 5.9 MB.
Everything else in the workspace is unchanged from 0.38.0.

### 🚀 Features

- *(gamma)* Add include_markets(bool) to ListKeysetEvents

## [0.38.0] - 2026-10-08

Gamma's game events now carry what joins them to a `polyoxide-sports` score.
`Event` gains `game_id` (`u64`, the same number as `GameKey::Game`), `teams`,
`sport`, `event_metadata` and `parent_event_id`, plus `volume`, `comment_count`,
`neg_risk_augmented` and `version`. `Team` gains `provider_id`, `color` and
`ordering`, a new `HomeAway`. `SportMetadata` gains `name` and `primary_tag_id`.
`ListEvents::game_id` filters `GET /events` by game. One game id can return the
game and its child events, so keep the one with no `parent_event_id`. Cricket
events carry no `game_id`. See `docs/specs/gamma/OBSERVED.md`.

**Breaking changes.** `Event`, `Team` and `SportMetadata` have new public fields
and are not `#[non_exhaustive]`, so code that builds one with a struct literal
must set them. Deserializing is unaffected.

### 🚀 Features

- *(gamma)* [**breaking**] Model game events' sports fields and filter /events by game_id

## [0.37.1] - 2026-10-07

Every polyoxide client asks for gzip again. In 0.37.0 `HttpClientBuilder::gzip`
was off unless set, which took compression away from the Gamma, Data, CLOB and
perps clients of a consumer whose workspace enables reqwest's `gzip` feature, as
0.36 had not. Unset, it now leaves reqwest's default, which asks for gzip because
the crates enable the feature, and decodes the body transparently.
`.gzip(false)` still opts out.

### 🐛 Bug Fixes

- *(core)* Gzip follows reqwest's default unless set

## [0.37.0] - 2026-10-07

Adds `polyoxide-binance`, a new credential-free crate for Binance USDⓈ-M
futures public market data on `fapi.binance.com` and, behind its `ws` feature,
the market streams on `fstream.binance.com`. It is not a Polymarket host, so it
is not in the `polyoxide` umbrella crate or `full`. Core gains
`HttpClientBuilder::gzip`, off by default. The crates now enable reqwest 0.12's
`gzip` feature, and feature unification turns it on for a consumer's own
reqwest 0.12 clients too, so those send `Accept-Encoding: gzip` unless built
with `.gzip(false)`.

### 🚀 Features

- *(binance)* New `polyoxide-binance` crate. `Usdm::new()` reads eleven public routes through the namespaces `health()`, `exchange()` and `market()`: contracts, tickers, the premium index and funding, klines, open interest, aggregate trades and order book snapshots. `Symbol` uppercases ASCII and accepts `_` for quarterlies (`BTCUSDT_261225`)
- *(binance)* `WeightBudget` charges every request its measured weight against the UTC minute, at 2160 of the published 2400. It follows the server's `X-MBX-USED-WEIGHT-1M` header, paces the weightless funding routes separately, and holds a `429` or `418` as a client-wide cooldown that only ever extends. A `429` with no retry left holds every request to the next minute, since sending into a spent minute is how a `429` becomes a `418` ban
- *(binance)* The `ws` feature streams eight USDⓈ-M market streams. `UsdmWs` is one connection on one path. `UsdmWsBuilder`/`SupervisedUsdmWs` runs one supervised connection per routed path (`/market` or `/public`). It pings on the wall clock, treats silence as staleness, reconnects with a paced replay and rotates before Binance's 24-hour cutoff. Every `Event::Disconnected { path }` is followed by `Event::Reconnected { path }`. `StreamName` is the only way to name a stream, and the client enforces Binance's limits before sending, rather than letting the server close the connection: 1024 streams per connection, 200 names per request, one request per 200 ms
- *(binance)* The `test-server` feature exposes the scripted Binance server and the captured frames as `usdm::ws::fixtures` for downstream tests
- *(binance)* `cargo run -p polyoxide-binance --example weight_probe` re-measures the weight table against the live host
- *(core)* `HttpClientBuilder::gzip`, off by default. Only `polyoxide-binance` turns it on (`exchangeInfo` is 1.15 MB raw, 51 KB gzipped)
- *(cli)* `polyoxide ws binance` streams Binance market data through the supervised tier. `--symbol` and `--kind` are comma-separated, and `--all-tickers` and `--all-mark-prices` add the two array streams. Outage markers go to stderr, and `--format json` prints each frame's envelope

### 🐛 Bug Fixes

- *(perps)* `info_soak` pins `.gzip(false)`, like the other rate-limit measuring examples, so its measurements are not skewed by compression

### 📚 Documentation

- *(specs)* `docs/specs/binance/` records what the host does in `INDEX.md` and `OBSERVED.md`. Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M, so there is nothing for `nightly-schema.yml` to diff. The weight table is measured, not copied: Binance's page is one off at the 100 and 500 `klines` edges, and does not say that omitting `limit` costs 5 on `klines` and 1 on `depth`

### 🧪 Testing

- *(binance)* Wire agreement tests check the REST rows and stream payloads against captured fixtures, which `scripts/capture_binance_fixtures.py` refreshes. The live suites' `live_responses_carry_no_unmodelled_keys` and `live_frames_carry_no_unmodelled_keys`, run nightly, are the host's drift detectors
- *(binance)* Offline tests drive a scripted server through the supervised tier's liveness, reconnect, replay and rotation paths, and pin the `Disconnected`/`Reconnected` pairing
- *(cli)* `tests/ws_binance.rs` drives `ws binance` with the crate's captured frames

### ⚙️ Miscellaneous Tasks

- `release.yml` publishes `polyoxide-binance` after `polyoxide-perps`
- The behavioral nightly runs the Binance live suites. It classifies `BinanceError`'s retriable arms and dropped streams as transient, and a region block (HTTP 451) as environmental

## [0.36.0] - 2026-10-07

Adds `polyoxide-sports`, a new credential-free crate for the live sports
scores feed on `sports-api.polymarket.com`, and removes that feed from
`polyoxide-clob`. It has two tiers: `SportsWs`, a bare `Stream`, and
`SupervisedSportsWs`, which treats the server's protocol pings as liveness and
reconnects on its own. The unified crate exposes it as `sports`, which `full`
now includes, and the CLI streams it as `polyoxide ws sports`. Gamma's
`Market` gains Polymarket Protocol V2's `version`, `position_ids` and
`resolution_status`. Trading a V2 market is not implemented (#51).

**Breaking changes.**

The sports channel is gone from `polyoxide_clob::ws`:

| Before, in `polyoxide_clob::ws` | Now, in `polyoxide_sports` |
|---|---|
| `WebSocket::connect_sports()` | `SportsWs::connect()`, or `SportsWsBuilder::new().connect()` to reconnect on its own |
| `Channel::Sports(SportsMessage::Update(update))` | `MatchUpdate` from `SportsWs`; `Event::Update(Box<MatchUpdate>)` from the supervised stream |
| `SportsUpdateMessage` | `MatchUpdate`, with the same field names |
| `SportsMessage::from_json` | `MatchUpdate::from_json` |
| `WS_SPORTS_URL` | `SPORTS_WS_URL` |
| `WebSocketError` | `SportsError` |

`ChannelType::Sports` is removed too. `MatchUpdate` is `#[non_exhaustive]`, its
`extra` is a `serde_json::Map` rather than a `Value`, and serialising it omits
absent optional fields rather than writing `null`. In the unified crate,
enable `sports`. The prelude exports `SportsWs`, `SportsWsBuilder`,
`SupervisedSportsWs`, `SportsError` and the aliases `SportsEvent`,
`SportsMatchUpdate` and `SportsGameKey`.

`polyoxide_gamma::types::Market` has three new public fields and is not
`#[non_exhaustive]`, so code that builds one with a struct literal must set
them. Deserializing is unaffected.

### 🚀 Features

- *(sports)* New `polyoxide-sports` crate. `MatchUpdate` and `GameKey` are modelled on captured frames, not on upstream's AsyncAPI, whose `slug`-keyed payload and text ping the server never sends. It depends on nothing else in the workspace
- *(sports)* `SupervisedSportsWs`, built with `SportsWsBuilder`. Any inbound frame resets its staleness timer, pings included, because the server sends data only while a match is live. It yields `Event::Update` and `Event::Reconnected`. A reconnect refused in a way retrying cannot fix (an HTTP status other than 408, 425, 429 or 5xx, or a malformed URL) is yielded as `SportsError::Connect` and ends the stream
- *(cli)* `polyoxide ws sports` streams live match scores, with comma-separated `--league` and `--game` filters and `--changes-only`. It reprints the games after a reconnect and ends cleanly on a closed pipe
- *(cli)* A `tracing` subscriber writes library warnings to stderr at `warn`, so retried reconnects and waited-out 429s are visible. `RUST_LOG` sets the level
- *(polyoxide)* `sports` feature re-exporting `polyoxide-sports`, included in `full`. `release.yml` and `finish_release.sh` publish the crate after `polyoxide-rtds`
- *(gamma)* `Market::version` (`ProtocolVersion::{V1, V2}`, anything else kept as `Other`), `Market::position_ids` and `Market::resolution_status`. Choose a market's trading ids by `version`, never by which field is present: many `v1` markets also carry `position_ids`, and the CLOB has no book for them
- *(data,py)* `first_entry_at` on v2 positions, in Rust and on the Python class. `docs/specs/data-v2/OBSERVED.md` records that a user's CLOSED positions are served in `REALIZED_PNL` order whatever the sort key

### 🐛 Bug Fixes

- *(perps)* A `Duration::MAX` staleness or ping interval no longer panics inside the spawned task, which had ended the stream silently
- *(rtds)* Reconnect backoff has a 1 ms floor and saturates instead of overflowing
- *(clob)* A zero or `Duration::MAX` ping interval no longer panics `run`
- *(ci)* The nightly classifier treats a dropped WebSocket as transient: a reset without a closing handshake, a TLS EOF without `close_notify`, close codes 1001/1011/1012/1013, or a test's own "server ended the connection"

### 📚 Documentation

- *(specs)* The sports AsyncAPI mirror moves to `docs/specs/sports/`, with `x-observed-*` annotations and an `OBSERVED.md`. It stays out of `nightly-schema.yml`
- *(gamma)* `docs/specs/gamma/OBSERVED.md` and CLAUDE.md tell Polymarket Protocol V2 apart from the CLOB V2 migration

### 🧪 Testing

- *(sports)* Offline tests drive a scripted local server through liveness, reconnect, backoff and refusal paths. The live suite's wire agreement test, run nightly, is the host's drift detector
- *(cli)* `tests/ws_sports.rs` drives every `ws sports` flag with captured frames

### ⚙️ Miscellaneous Tasks

- The behavioral nightly also runs at 18:30 UTC on Saturday and Sunday, when North American leagues and weekend soccer are live
- The workspace builds clean under Rust 1.99's lints

## [0.35.0] - 2026-09-30

Adds the public Perps WebSocket to `polyoxide-perps`, behind a new `ws`
feature. It covers the six public channels (`bbo`, `book`, `trades`,
`klines`, `tickers`, `statistics`) in two tiers: `PerpsWs`, a bare `Stream`,
and `SupervisedPerpsWs`, which adds keep-alive, a staleness watchdog,
reconnect with resubscribe, and membership changes on a live connection. The
unified crate exposes it as `perps-ws`, which `full` now includes. Nothing in
0.34.0 changes, and the default features compile the same code.

### 🚀 Features

- *(perps)* `ws` feature with the `ws` module. `Channel` is the only way to name a subscription (`Bbo`, `Book` at a `StreamDepth` of 20 or 50 levels, `Trades`, `Klines` at an `Interval`, and `Tickers`/`Statistics` for one instrument or all of them). Payload structs carry the REST field names in place of the socket's terse keys
- *(perps)* `PerpsWs`: connect, `subscribe`/`unsubscribe`, an application `ping` that returns the pong's `sq`, `close`, and a `Stream` of `Frame`s
- *(perps)* `SupervisedPerpsWs`, built with `PerpsWsBuilder` (defaults: ping every 20 s, stale after 30 s, backoff from 500 ms to 60 s). It pings on the wall clock whether or not traffic is flowing, since the host closes a socket after 60 s without an inbound message. It yields `Event`s: `Update`, `Unknown`, `Reconnected` (book consumers must discard state) and `SequenceRegressed`. A `MembershipHandle` changes subscriptions while the stream is being driven
- *(perps)* `PerpsWsError::recovery()` classifies each failure as `Reconnect`, `Retry`, `SkipFrame` or `Fatal`. A refused subscription lists every `Refusal`. A handshake refused with 5xx or 429 reconnects, any other refusal is fatal, and a rate-limited subscription retries
- *(perps)* `test-server` feature exposes the scripted local WebSocket server the crate's own offline tests use, for downstream integration tests
- *(polyoxide)* `perps-ws` feature enabling `polyoxide-perps/ws`, included in `full`

### 📚 Documentation

- *(perps)* `docs/specs/perps/OBSERVED.md` records where the socket departs from the AsyncAPI mirror. `tickers`/`statistics` data are objects, and `::all` fans out into one frame per instrument labelled `::N`. `ets` is on every frame. `sq` is a server-wide stamp that is not contiguous per channel, so only a regression is detectable, never a gap. An unknown instrument subscribes without error

### 🧪 Testing

- *(perps)* Offline tests drive the scripted server through reconnects, refusals and faulted pongs, and check that the supervised tier keeps pinging under steady traffic
- *(perps)* `ws_wire_agreement.rs` compares the payload types value-for-value against captured frames in `tests/fixtures/ws/` (`scripts/capture_perps_ws_fixtures.py` refreshes them); `live_ws.rs` is the live suite, now in the nightly run

## [0.34.0] - 2026-09-30

Adds `polyoxide-perps`, a new crate for the public half of Polymarket's
perpetual futures API on `api.perpetuals.polymarket.com`. It needs no
credentials: `Perps::new()` covers all 21 `/v1/info/*` routes through the
`health()`, `exchange()`, `market()` and `public()` namespaces. The unified
`polyoxide` crate exposes it behind a new `perps` feature, which `full` now
includes. Authentication, account reads, trading, funds and the WebSocket are
not implemented yet.

### 🚀 Features

- *(perps)* New `polyoxide-perps` crate: `Perps` client and builder, `PerpsError` with a `#[non_exhaustive]` `VenueError`, and `is_retriable()` classifying errors the same way as core
- *(perps)* `health()` (ping, server time), `exchange()` (exchange and instrument reference data), `market()` and `public()` namespaces covering all 21 public `/v1/info/*` routes: assets, instruments, indices, tickers, statistics, exchange stats, order books, BBO, klines, mark history, trades, funding, fees, limit tiers, public portfolios, position fills, leaderboards and invite checks. Request builders take typed setters, closed sets are enums, and fields the schema marks nullable are `Option`
- *(perps)* Klines and mark points are positional arrays on the wire and deserialize into named structs. `LimitTier`'s four WebSocket fields carry a `u32::MAX` sentinel, not a real budget
- *(perps)* The default client paces itself to rate limits measured against the live host (klines 30, trades 10, portfolio 30 and BBO 50 per 10 s, a `/v1/info` catch-all of 10, and a client-wide cap of 30), and retries `429` the way core does
- *(perps)* `examples/info_soak.rs` ramps and validates a route's rate over distinct URLs, since the host sits behind CloudFront and repeated URLs never reach the origin
- *(polyoxide)* `perps` feature re-exporting `polyoxide-perps`, included in `full`. `release.yml` and `finish_release.sh` publish the crate between `polyoxide-rtds` and `polyoxide-relay`, and the nightly workflows cover it

### 📚 Documentation

- *(perps)* `docs/specs/perps/OBSERVED.md` records where the host departs from its published schema. Notably, `/v1/info/tickers` and `/v1/info/statistics` ignore `instrument_id` and return every instrument, so callers filter client-side, and every index currently has empty constituents. Its `## Rate limits` section holds the soak runs behind the pacing table
- *(polyoxide)* README, crate docs and `CLAUDE.md` name the `rtds` and `perps` features

### 🧪 Testing

- *(perps)* `spec_agreement.rs` checks types, enums and query keys against `docs/specs/perps/openapi.json`, following `$ref` targets for nullability. `wire_agreement.rs` checks the types against one live capture per route (`scripts/capture_perps_fixtures.py` refreshes them), and fails when an allow-list entry is no longer needed by any fixture
- *(perps)* `live_api.rs` hits every public route (`#[ignore]`d in CI, run nightly)

## [0.33.1] - 2026-09-30

Adds upstream's settlement estimates to `/v2/resolutions` rows.

### 🚀 Features

- *(data, py)* `Resolution` gains `expected_settlement_time` and `settlement_time_basis`, both `Option<String>`, in Rust and on the Python `polyoxide.v2.Resolution` class. Upstream's route now answers with `ResolutionWithSettlementTime`, an `allOf` that keeps the row flat. `Resolution` is `#[non_exhaustive]`, so no caller breaks

### 📚 Documentation

- *(specs)* Adopt upstream's `data-v2` spec change. `docs/specs/data-v2/OBSERVED.md` records a 2026-09-29 probe of 70 proposed or disputed conditions. Both keys appeared on 37 rows, always together and never null: `managed_proposal_expiration` on fresh proposals and `dvm_round_estimate` on disputes. The documented `liveness` basis never appeared

### 🧪 Testing

- *(data)* `v2_spec_agreement.rs` checks an `allOf` schema as the flat object the server sends, and panics on a property that two arms both declare. A new `resolutions_pending.json` fixture pins one row of each basis seen

## [0.33.0] - 2026-09-26

Adds Deposit Wallets and session keys. `polyoxide-clob` signs signature-type-3
orders, an ERC-7739 `TypedDataSign` envelope pinned to py-sdk's golden digest,
for a `SigningTarget::DepositWallet { wallet, role }`. It also accepts any alloy
signer (`Account::with_signer`) or none (`Account::l2_only`), takes an externally
produced L1 signature and checks it locally, and lists a wallet's session signers.
`polyoxide-relay` speaks the relayer's `type: WALLET` dialect: CREATE2 wallet
derivation, `resolve_wallet`, Deposit Wallet batches, session-signer
authorization and revocation, and the redemption pair, which targets the
collateral adapter as py-sdk does. `RelayClientBuilder::with_auth` builds a
client that has auth but no account. The contract and the SDK behaviours the
upstream pages omit are recorded in `docs/specs/session-keys/`.

Breaking: `Wallet::signer()` returns `Result<&DynSigner, ClobError>` instead of
`&PrivateKeySigner`, and `Wallet::ethereum_wallet()` is removed.
`BuilderAccount::signer()` returns `&DynSigner`, so `.credential()` and
`.to_bytes()` on it no longer compile. `AuthMode` gains `L1Signed`. `WalletType`
gains `DepositWallet` (wire `"WALLET"`) and is now `#[non_exhaustive]`.
`ContractConfig` gains five fields, so an external struct literal breaks.
`Account::sign_order` and `Clob::sign_order` sign type-3 orders for a Deposit
Wallet target instead of refusing them, and `ClobBuilder::signature_type`
defaults from the account's target. Every existing loader yields an EOA target,
so no existing caller changes behaviour.

### 🚀 Features

- *(clob)* SigningTarget names the account an Account signs for
- *(clob)* Wallet holds any alloy signer, or none for L2-only use
- *(clob)* Account::with_signer, Account::l2_only and a signing target
- *(clob)* ERC-7739 Deposit Wallet order signing pinned to py-sdk vectors
- *(clob)* Account::sign_order honours the signing target
- *(clob)* Create_order builds Deposit Wallet orders from the signing target
- *(clob)* L1 auth typed data out, signature in, for external wallets
- *(clob)* L1 signature-in is checked locally and pinned to py-sdk's typed data
- *(clob)* List a Deposit Wallet's session signers; SessionSignerScope in core
- *(relay)* Pure CREATE2 derivations for Deposit Wallet, Safe and Proxy, pinned to py-sdk
- *(relay)* WalletType::DepositWallet, v1 params and transaction routes, resolve_wallet
- *(relay)* Deposit Wallet Batch typed data, calldata encoders and session envelope pinned to py-sdk
- *(relay)* Execute through a Deposit Wallet; auth without a key; typed-data-out, signature-in batches
- *(relay)* Session-signer authorization and revocation under Builder HMAC
- *(relay)* Deposit Wallet redemption pair; document the Deposit Wallet and session-key surface
- *(relay)* Resolve_wallet probes the Proxy with type=PROXY, as py-sdk does; authorize_session_signer refuses a relayer key before I/O

### 🐛 Bug Fixes

- *(clob)* Actionable session-signers mismatch error, verify L2-only POLY_ADDRESS
- *(relay)* Resolver tests assert their probe counts; encode transaction ids; WalletType is non_exhaustive
- *(relay)* Keep a Deposit Wallet call value above u64 exact in the typed data
- *(relay)* Pin the session-key batch signature; refuse a Deposit Wallet on an unsupported chain before I/O; export RelayClientBuilder
- *(relay)* Let revocations take a relayer API key; wait 300s on session-signer POSTs
- *(relay)* Request py-sdk's full trading-approval set; always send Deposit Wallet metadata
- *(relay)* Deposit Wallet redemption targets the collateral adapter, as py-sdk does

### 🚜 Refactor

- *(clob)* Share the exchange domain and 0x1901 digest; type 3 needs maker == signer
- *(clob)* Deposit Wallet primitives are crate-private; guards are validation errors
- *(core)* SessionSignerScope is non_exhaustive and parses via FromStr
- *(core)* DepositWalletRole lives in core for clob and relay to share
- *(relay)* WalletKind is non_exhaustive with an infallible address; Safe init hash lives in ContractConfig

### 📚 Documentation

- *(specs)* Adopt upstream data, data-v2, perps and perps-ws specs
- *(handoff)* Correct the ERC-7739 domain orientation for session keys
- Handoff amendments for two Deposit Wallet generations, relayer auth enum, tx state
- *(specs)* Design for the offline Deposit Wallet session-key surface
- *(specs)* Align the session-key spec with prader-rs [#126](https://github.com/dilettante-trading/prader-rs/issues/126) and its §8 contract
- *(plans)* Session keys plan 1, CLOB signing core and account model
- *(plans)* Vector() needs a mutable map
- *(plans)* Refuse non-type-3 overrides on a Deposit Wallet target before I/O; target() by value
- *(clob)* SigningTarget docs state what overrides can and cannot do
- *(clob)* [**breaking**] DynSigner needs sign_hash; export it from the crate root
- *(plans)* Task 9 relinks the DynSigner doc once clob_auth_typed_data exists
- *(plans)* Fold Task 4 review notes into Tasks 7 and 11
- *(plans)* Task 8 refuses a foreign funder on a Deposit Wallet target; Task 7 pins the full vector
- *(plans)* Task 8 guards market orders' funder too
- *(plans)* MarketOrderArgs lives under types
- *(plans)* Fold Task 7 review notes into Tasks 8 and 11
- Balance queries default to the target's signature type (Task 10); spec names the right params type
- L1 signature-in lives on Clob and is checked locally; Task 11 shows the onboarding round trip
- *(specs)* Scope validator belongs to the relay; session-key credentials on session-signers is an open item
- *(clob)* Document Deposit Wallet accounts and session keys
- *(specs)* Align section 1 and 2 text with the shipped code; record plan 1's breaking changes
- *(plans)* Session keys plan 2, relay Deposit Wallet dialect
- *(plans)* Calldata lengths count the 0x prefix
- *(plans)* Relay tests read the py-sdk-built bodies and the two-call batch
- *(plans)* Redemption submit sends the empty metadata py-sdk sends
- *(plans)* Resolve_wallet returns Option<WalletKind>
- *(plans)* Assert every expect(n) mock
- *(relay)* Batch_typed_data departs from py-sdk in two places, not one
- *(plans)* Revocation accepts a Relayer API key; session-signer routes use a 300 s timeout
- *(specs)* List plan 2's breaking changes for the 0.33.0 release notes
- *(relay)* Bring the README and crate docs up to the Deposit Wallet surface
- *(plans)* Plan 3, docs and the ignored live round trip for session keys
- *(specs)* Record the Deposit Wallet and session-key contract
- *(specs)* Record what py-sdk and ts-sdk do for session keys that the pages omit
- *(plans)* Plan 3 Task 6, resolve_wallet probes the Proxy as py-sdk does
- Point the spec index, the drift workflow and CLAUDE.md at docs/specs/session-keys
- *(specs)* Session-keys README, corrections from review
- *(plans)* Plan 3 Task 7, Deposit Wallet redemption goes through the collateral adapter
- *(specs)* Session-keys OBSERVED, corrections from review
- *(plans)* Plan 3 Task 6 also moves the relayer-key refusal ahead of the nonce fetch
- *(plans)* Plan 3 Task 7 also drops the unobserved venue claim in OBSERVED row 9
- The relay mirror does document /deployed?type=WALLET; no unobserved venue claims
- *(relay)* /deployed answers every non-WALLET type alike; revoke accepts a relayer key
- *(plans)* Seven signed batches after the adapter redemption vectors
- Status of every handoff item; plans 2 and 3 marked done in the design
- *(handoff)* Item 8 status line says seven batches, not five
- Handoff status wording; the redemption pair's real names
- OBSERVED intro admits the /deployed probe; redeem_submit_body is unconsumed; lifetime claim attributed

### 🧪 Testing

- *(clob)* Golden vectors for Deposit Wallet order signing from py-sdk
- *(clob)* Session-key vectors carry provenance and assert py-sdk's golden digest
- *(clob)* Session-key provenance names the right envelope
- *(clob)* Deposit Wallet market orders carry the wallet as maker and signer
- *(clob)* Expect(0) mocks can match; pin the proxy-target maker path
- *(clob)* L1 signature check names the real signer and normalises v
- *(relay)* Golden vectors for Deposit Wallet batches and derivations from py-sdk
- *(relay)* Session-signer bodies come from py-sdk; a two-call batch; one documented command
- *(relay)* Provenance flags the synthetic two-call batch
- *(clob)* Ignored live round trip for a Deposit Wallet session key
- *(clob)* Live session-key round trip selects its market first and self-heals; own nightly row
- *(clob)* Live session-key round trip never aborts on the owner listing; self-heal waits for the revocation
- *(relay)* Adapter batches in the digest and signature loops
- *(relay)* Pin the adapter redemption batches' typed data to py-sdk; digest test covers every batch
- *(relay)* Neg-risk submit target, estimate refusal, and no RPC before the client checks

## [0.32.2] - 2026-09-23

Adds the new v2 positions filters and sort key to `polyoxide-data` and the CLI,
and deprecates `data.approvals()`, whose upstream route now returns 404.

### 🚀 Features

- *(data)* `PositionStatus::RedeemableLost` (settled losing positions; rows come back labelled `REDEEMABLE`), `PositionStatus::Mergeable` (`OPEN` narrowed to conditions holding a complementary set of live outcome tokens; rows come back labelled `OPEN`) and `PositionSortBy::Price` (by `current_price`) on `/v2/positions`. Both enums are `#[non_exhaustive]` (#38)
- *(cli)* `data positions list --status` accepts `redeemable-lost` and `mergeable` (and the upstream `REDEEMABLE_LOST` spelling), and `--sort-by` accepts `price` (#38)

### ⚠️ Deprecated

- *(data)* `DataApi::approvals()` and `ApprovalsApi`: upstream removed `GET /v1/approvals` and the host answers `404` for it. Use `data.v2().approvals(user)`. Removal is deferred to a breaking release (#41)

### 📚 Documentation

- *(data, cli)* The `event_id` filters on trades, activity, positions and live-volume document upstream's cap of 20 distinct ids, which the server enforces with a non-retryable 400 (#38)
- *(gamma)* `ListKeysetEvents::limit` is capped at 100 (was documented as 500). The server clamps larger values rather than rejecting them, so a short page does not mean the end of the data. Page on `next_cursor` (#42)
- *(specs)* Adopt upstream perps and perps-ws specs: `GET /v1/account/backstops`, a required `fee_tier` on `Portfolio`, and 503 responses on the leverage, margin and internal-transfer routes (#39, #40)

## [0.32.1] - 2026-09-23

Adds `ListEvents::include_markets`. Everything else in the workspace is
byte-identical to 0.32.0.

### 🚀 Features

- *(gamma)* Add `include_markets(bool)` to `ListEvents`. The parameter is not in upstream's `openapi.yaml`, but the server applies it (verified 2026-09-23): omitting it is the same as `true`, and `false` removes the `markets` key from each event rather than sending `[]`. `Event::markets` is `#[serde(default)]`, so such a response parses with an empty `markets`

### 📚 Documentation

- *(gamma)* Record `include_markets` in `docs/specs/gamma/OBSERVED.md` as an undocumented parameter the server applies on both `GET /events` and `GET /events/keyset`

## [0.32.0] - 2026-09-16

Adds Data API v2 (20 routes under `/v2` on `data-api.polymarket.com`) to
`polyoxide-data` as `data.v2()`, alongside the v1 routes, which upstream says
keep working. v2 has its own contract: a `data` envelope, cursor-only
pagination and snake_case fields. Paged builders return `Page<T>` from
`send()` and a `Stream` from `.pages()`. `RateLimiter::data_default` carries
measured rows for six v2 routes. The Python bindings expose v2 as
`DataApi().v2()` / `DataApiSync().v2()`, with the row classes on
`polyoxide.v2`.

Breaking: `DataApiError` is now `#[non_exhaustive]` and gains `V2` (a v2 error
body's `code`, `retryable` flag and `trace_id`) and `Pagination` (for
`.pages()` walks), so exhaustive matches must change. `is_retriable` follows
the server's `retryable` flag for v2 errors.

Breaking for scripts: `polyoxide data` commands now read Data API v2. Output
is the v2 `{data, pagination}` envelope with snake_case fields
(`proxy_wallet`, not `proxyWallet`). `--offset` is replaced by `--cursor`,
`--all` and `--max-pages`, `positions closed` by
`positions list --status closed`, and `holders` requires `--condition`
(`--market` remains an alias). `data traded` prints the `/v2/user-stats`
object (its `trades` field is the distinct-market count), or `null` for a
wallet the API does not know, instead of `{user, traded}`. `data health` is
unchanged. Comma-separated `--market` and `--event-id` values, which panicked
in earlier releases, now work.

Also fixes `data.holders().list()`, which failed with a serialization error
when the venue answered a miss with `null`; it now returns no holders.

### 🚀 Features

- *(core)* Make Request cloneable without Clone markers
- *(data)* [**breaking**] Structured Data API v2 errors on DataApiError
- *(data)* Data API v2 response and parameter types
- *(data)* Data API v2 envelopes, cursor walks, trades and user-stats
- *(data)* V2 activity and combo activity
- *(data)* V2 wallet routes
- *(data)* V2 market routes
- *(data)* V2 board routes
- *(data)* V2 status, completing the v2 surface
- *(core)* Provisional rate limits for Data API v2
- *(data)* V2_soak harness for measuring Data API v2 rate limits
- *(core)* Pin measured Data API v2 rate limits
- *(cli)* Cursor paging and JSONL output helpers for data commands
- *(cli)* [**breaking**] Data trades on Data API v2 with cursor paging
- *(cli)* [**breaking**] Data activity and positions on Data API v2
- *(cli)* [**breaking**] Data holders, open-interest and live-volume on Data API v2
- *(cli)* [**breaking**] Data traded and builders on Data API v2
- *(py)* Data API v2 row classes on a polyoxide.v2 submodule
- *(py)* Polyoxide.v2 module and typed stub for the v2 rows
- *(py)* DataApi.v2() with every Data API v2 route
- *(py)* Map Data API v2 errors by code and keep their fields

### 🐛 Bug Fixes

- *(data)* Read a null /holders body as no holders
- *(data)* V2_soak builds holder probes from market condition ids only

### 🚜 Refactor

- *(core)* Split ApiError::from_status_and_body out of from_response
- *(data)* Share the soak harness Pacer via examples/common

### 📚 Documentation

- Handoff for Deposit Wallet and Session Key support
- *(specs)* Sync the data mirror
- *(clob)* Sync the clob mirror and document order status semantics
- *(gamma)* Sync the gamma mirror and pin the market-maker surface it dropped
- *(specs)* Sync the perps and perps-ws mirrors
- *(specs)* Design Data API v2 support
- *(plans)* Data API v2 implementation plan for Phases 0-2
- *(data)* Data API v2 in the README
- Mark Data API v2 implemented and document its overlaps
- *(plans)* Data API v2 rate-limit measurement plan (Phase 3)
- *(specs)* Record Data API v2 rate-limit ramps
- Document the measured Data API v2 rate limits
- *(plans)* Data API v2 CLI (Phase 5) implementation plan
- *(plans)* Data traded prints the v2 user-stats object
- *(plans)* Keep mutation-check copies out of the loom worktree
- *(cli)* Document the Data API v2 data commands
- *(plans)* Data API v2 Python bindings (Phase 4) implementation plan
- Document the v2 data CLI commands and Python bindings

### 🧪 Testing

- *(data)* Capture Data API v2 fixtures and record observed behaviour
- *(data)* Hold the v2 types to live captures in both directions
- *(data)* Live tests for every Data API v2 route
- *(data)* V2_soak probe space of distinct URLs per route
- *(data)* V2_soak verdict rules for stages and pinned counts
- *(data)* Validate the pinned Data API v2 rows per route
- *(data)* Validate the Data API v2 rows across all routes at once
- *(data)* Pin that empty v2 multi-value setters omit their parameter
- *(data)* Pin every v2 enum's wire values to what the server accepts
- *(data)* Decode a captured response through every v2 builder offline
- *(cli)* Live tests for the Data API v2 data commands
- *(py)* Live Data API v2 tests and README usage

### ⚙️ Miscellaneous Tasks

- *(specs)* Mirror Data API v2 and watch it for drift
- *(data)* Run the rate-limit harnesses' unit tests

## [0.31.0] - 2026-09-09

The CLOB market channel can change membership on a live socket. The venue
has always accepted `{"operation":"subscribe"|"unsubscribe","assets_ids":[…]}`
on the market channel (it is `SubscriptionRequestUpdate` in
`asyncapi-market.json`), but the crate gated subscription updates to the user
channel. `WebSocket::subscribe_assets` / `unsubscribe_assets` send the frame
on a plain socket, and a `MembershipHandle` (from
`WebSocketWithPing::membership`, taken before `run`) sends it while the ping
loop drives the connection. Verified live on 2026-09-09: an added asset gets a
fresh `book` in ~155 ms, a duplicate add gets nothing (unsubscribe then
subscribe to force a snapshot), and an empty-membership socket stays open
under the 10 s `PING`.

Breaking: `WebSocketError` is not `#[non_exhaustive]` and gains
`MembershipClosed`, so exhaustive matches must add an arm. Everything else in
the workspace is byte-identical to 0.30.0.

### 🚀 Features

- *(clob)* Add the market-channel MarketSubscriptionUpdate frame
- *(clob)* [**breaking**] Market-channel subscribe_assets/unsubscribe_assets on WebSocket
- *(clob)* MembershipHandle sends market subscription updates while run pumps

### 📚 Documentation

- *(clob)* The market channel changes membership on a live socket
- *(clob)* Membership() is market-only, name the duplicate-add remedy, README section for live membership

## [0.30.0] - 2026-09-07

Adds `polyoxide-rtds`, a client for Polymarket's Real-Time Data Service
(`wss://ws-live-data.polymarket.com`) crypto price streams. It depends on
nothing else in the workspace — not even `polyoxide-core` — so a
credential-free price feed does not pull in the signing stack. Reachable from
the unified crate under the `rtds` feature and from the CLI as
`polyoxide ws prices`.

Also carries two fixes that landed after the 0.29.0 tag was cut, and so appear
here rather than in that release's section.

### 🚀 Features

- *(rtds)* Add the polyoxide-rtds crate skeleton
- *(rtds)* Add Topic and TwapWindow with wire mappings
- *(rtds)* Build subscription filters as compact JSON
- *(rtds)* Add RtdsError with a recoverability classifier
- *(rtds)* Classify errors three ways instead of two
- *(rtds)* Decode E18 and plain decimal values without panicking
- *(rtds)* Add per-topic payload types
- *(rtds)* Dispatch frames into per-topic events
- *(rtds)* Add the Rtds stream client
- *(rtds)* Add reconnect, staleness detection, and keep-alive
- *(rtds)* Re-export through the unified crate and add an example
- *(cli)* Add ws prices for RTDS reference price streams

### 🐛 Bug Fixes

- *(rtds)* Compute the subscription struct length from its fields
- *(rtds)* Treat a malformed handshake request as fatal
- *(rtds)* Say what a decode failure actually was
- *(rtds)* Make dropped frames diagnosable
- *(rtds)* Reconnect with a fresh stream and back off a flapping host
- *(release)* Publish polyoxide-rtds to crates.io
- *(ci)* Match transient failures in both of a panic's error renderings
- *(data)* Select a holders market instead of trusting the newest trade

### 🚜 Refactor

- *(rtds)* Derive topic and window parsing from their wire mappings
- *(rtds)* Keep the test seam off the public API

### 📚 Documentation

- *(specs)* Design for the RTDS crypto price streams crate
- *(plans)* Implementation plan for polyoxide-rtds
- *(rtds)* Stop TwapWindow::ALL claiming a guarantee it lacks
- *(plans)* Fix a step ordering that made red-then-green unobservable
- *(plans)* Classify errors three ways, and keep the frame that failed
- *(plans)* Record what SkipFrame costs when a symbol stays broken
- *(rtds)* Correct what decode_plain actually accepts
- *(plans)* Share the captured frames instead of pasting one
- *(plans)* Correct the mislabelled spot snapshot in Task 8
- *(plans)* Fix a stale test count in Task 8
- *(rtds)* Record which field actually keeps the envelopes apart
- *(plans)* Stop Task 9 shipping a doc link to a module that does not exist
- *(plans)* Say why Task 11 reconnects with a fresh Rtds
- *(plans)* Match the harness the crate's features can actually build
- *(plans)* Finish the std Mutex swap in the harness
- *(rtds)* Record the harness ordering the supervision tests rely on
- *(rtds)* Say what the delivered flag does not fix
- *(specs)* Close the design's open question — RTDS does accept resubscribe
- *(specs)* Mirror the observed RTDS contract
- *(rtds)* Stop claiming a fixed backfill length
- *(rtds)* A backfill only arrives for a filtered subscription
- *(claude)* Document polyoxide-rtds and its gotchas

### 🧪 Testing

- *(rtds)* Tie the topic list to the window list
- *(rtds)* Add frames captured verbatim from the live host
- *(rtds)* Let integration tests share the captured frames
- *(rtds)* Assert the fixture cross-check exactly, not within 0.001
- *(rtds)* Pin a server bug that mislabels Chainlink-spot backfills
- *(rtds)* Add a scripted local WebSocket server
- *(rtds)* Give the scripted server its own smoke test
- *(rtds)* Stop the supervision tests waiting on timeouts
- *(rtds)* Establish that RTDS accepts a second subscribe frame
- *(rtds)* Add live tests with an unfiltered control subscription
- *(rtds)* Cover the supervision layer, which no test could see

## [0.29.0] - 2026-08-26

### 🚀 Features

- *(clob)* Per-address connect timeout and IPv6/IPv4 fallback for WebSocket connects. `connect_async` resolves DNS internally and connects to only the first address with no timeout, so one blackholed AAAA record hung the connect indefinitely. Addresses are now resolved up front and interleaved by family (RFC 8305 §4), and each address's TCP connect and TLS+handshake is bounded by the new `WebSocketBuilder::connect_timeout` (default 10s). The free `connect_*` functions delegate to the shared helper

## [0.28.1] - 2026-08-21

No library code changed in this release: every crate's shipped source is
identical to 0.28.0. The entries below cover the changelog itself, the live
test suite, and CI.

### 📚 Documentation

- *(changelog)* Backfill the missing 0.27.0 entry. The 0.27.0 bump rode along inside `chore(specs): sync the data spec and release 0.27.0` rather than a dedicated release commit, so the step that regenerates `CHANGELOG.md` was never run and the file jumped 0.28.0 -> 0.26.1 with nothing to signal it
- *(changelog)* Backfill six 0.28.0 entries. All six commits landed between the release commit and the tag, after the section had already been written — including a behavioural fix to `ParentEntityType`'s `Display` impl that shipped in 0.28.0 undocumented

### 🧪 Testing

- *(clob)* Select the order-test markets by price rather than asserting the precondition about an arbitrary one. `find_active_token_id` returns whichever open market gamma lists first, and gamma's ordering is stable, so a listing whose first market asked 0.042 failed `live_fak_unmatched_is_typed_error` on four consecutive runs with no way to retry out of it. The new `find_token_id_with_min_ask` filters on the `best_ask` gamma already returns in the listing, then confirms the candidate against the live CLOB book
- *(clob)* Read credentials from the OS keychain when the environment is unset. The live suites called `Account::from_env()` only, so credentials already held in the keyring did nothing for them, and bridging the gap by materialising a `.env` would put four secrets on disk to run a test suite. `load_account()` and `l1_account()` now try the environment, then `Account::from_keychain()` under the crate's non-default `keychain` feature, then panic as before — CI has no keychain and still falls through to the same auth-gated panics
- *(ci)* Assert that `CHANGELOG.md` and the workspace version agree. `release.yml` runs git-cliff only to compose the GitHub release body and never writes the file back, so the version bump and the changelog entry are two independent manual steps that nothing checked. The new `.github/scripts/tests/test_changelog.py` pins that the version in `Cargo.toml` has a dated section, that it heads the file, that sections stay strictly descending, and that every `[workspace.dependencies]` path pin moved with the workspace version

### ⚙️ Miscellaneous Tasks

- *(ci)* Teach the nightly failure classifier that a market-state refusal is environmental, not a defect. `ENVIRONMENTAL_RE` now also matches `no (?:qualifying|suitable) market`, the wording both the CLOB helper's give-up panic and the tests' own guards use — without it, wiring up the `POLYMARKET_*` secrets would file a tracking issue every night the listing happened to open cheap

## [0.28.0] - 2026-08-19

### 🚀 Features

- *(gamma)* Add `ParentEntityType` with the server's accepted values

### 🐛 Bug Fixes

- *(gamma)* [**breaking**] Model comments against the wire, not the fork's invention. `Comment`, `CommentReaction` and `CommentPosition` described a payload Polymarket has never sent; `CommentUser` had no upstream counterpart at all and is removed. Removed fields: `Comment::user`, `market_id`, `event_id`, `series_id`, `parent_id`, `positions`, `like_count`, `dislike_count`, `reply_count`; `CommentReaction::user_id`; `CommentPosition::outcome` and `shares`. Added: `Comment::parent_entity_type`, `parent_entity_id`, `parent_comment_id`, `user_address`, `reply_address`, `profile`, `report_count`, `reaction_count`; the new `CommentProfile`; and `CommentPosition::position_size`. Any code touching these types was already failing at runtime with a deserialization error
- *(gamma)* Comment endpoints no longer fail with `missing field 'userId'` on any response containing a reaction (#28)
- *(gamma)* [**breaking**] `GET /comments/{id}` returns a thread, not one comment — `Comments::get` now returns `Vec<Comment>`
- *(gamma)* [**breaking**] `ListComments::parent_entity_type` takes `ParentEntityType`, not a string — the server rejects `market` in either casing
- *(gamma)* `ParentEntityType`'s `Display` impl emitted `UNKNOWN` for the unknown variant while `Serialize` emitted `Unknown` for the same variant — `Display` now agrees with the wire vocabulary
- *(gamma)* [**breaking**] `Profile` described a payload `/profiles/user_address/{address}` has never sent — `id` was required and the server never sends it, so `Gamma::user().get_by_address` failed for every address. Rewritten against the endpoint's own published `PublicProfile.json` schema (linked from the response's `$schema` key) rather than `openapi.yaml`, whose `Profile` schema turns out to describe an unrelated object. Removed: `id`, `user`, `referral`, `created_by`, `updated_by`, `updated_at`, `utm_source`, `utm_medium`, `utm_campaign`, `utm_content`, `utm_term`, `wallet_activated`, `display_username_public`, `profile_image_optimized`, `is_close_only`, `is_cert_req`, `cert_req_date`. Added, now required: `taker_tier`, `taker_tier_name`, `weighted_volume` — sent on every response, and not in `openapi.yaml` at all
- *(cli)* [**breaking**] Drop the rejected `market` entity type, add `perps-asset`
- *(py)* [**breaking**] Align the comment bindings with the corrected types — `CommentUser` is removed and `CommentProfile` added, the comment getters renamed to match, and `GammaComments.get` now returns `list[Comment]`
- *(gamma)* [**breaking**] `UserResponse` and `UserInfo` described a payload `/public-profile` has never sent. Rewritten against the endpoint's own published `PublicProfileResponse.json` / `PublicProfileUser.json` schemas (linked from the response's `$schema` key) rather than invented fields. Removed: `UserResponse::address`, `UserResponse::id` — the wire never sends either; the account id lives at `users[].id` instead. Added, now required: `UserResponse::taker_tier`, `taker_tier_name`, `weighted_volume` — sent on every response. Added, optional: `UserResponse::discord_username`. `UserInfo::id` is now a required `String`, not `Option<String>` — the schema's only required property on that object. Added `UserInfo::community_mod`
- *(py)* [**breaking**] Align the `UserResponse` / `UserInfo` bindings with the corrected types — `address` and `id` are removed from `UserResponse`, `taker_tier`/`taker_tier_name`/`weighted_volume`/`discord_username` are added, and `UserInfo` gains `community_mod`
- *(gamma)* `SearchResponse::profiles` no longer fails the whole call with `invalid type: null, expected struct SearchProfile` — `/public-search` sends a JSON `null` for some entries in the array (reproducible on `q=sports&search_profiles=true&limit_per_type=20`, stable across 5 attempts), and the old `Vec<SearchProfile>` couldn't hold one
- *(gamma)* [**breaking**] `SearchProfile::address` described a payload `/public-search` has never sent (0 of 228 sampled profiles) — removed, along with `SearchResponse::profiles`'s element type changing to `Option<SearchProfile>` to tolerate the server's `null` entries. Added `SearchProfile::display_username_public`, sent on every sampled profile and previously unmodelled. Completes finding #10
- *(py)* [**breaking**] Align the `SearchProfile` bindings with the corrected type — `address` is removed, `display_username_public` is added

### 📚 Documentation

- *(gamma)* Update the README comments example for the typed filter
- Add `docs/specs/gamma/OBSERVED.md`, recording where gamma's published spec disagrees with gamma's server
- Record the gamma type parity sweep in `docs/plans/2026-08-19-gamma-type-parity-worklist.md`, cataloguing 15 further parity findings across the crate, including a second runtime failure of the same class (`Profile::id` is required and never sent)
- *(specs)* Sync perps, perps-ws, bridge and combos-rfq mirrors to upstream (#23, #20, #24, #21) — mirror-only, no client crate implements them
- Refresh the perps endpoint and channel counts in `CLAUDE.md` and both `INDEX.md` files after the mirror sync — it added 6 endpoints and 2 WebSocket channels the prose still under-counted
- *(gamma)* Record the wire-agreement guard's blind spot: an invented `Option<T>` field passes it, because the no-invented-fields direction must exempt null and cannot distinguish "optional and absent this time" from "does not exist"
- *(gamma)* Correct false and stale claims found by review — `/profiles/user_address/{address}` returns 200 rather than 404 for an address with no profile, `limit` bounds top-level comments rather than returned rows, and `comment_full.json`'s nested profile still lacks `pseudonym`
- *(gamma)* Record in `docs/specs/gamma/OBSERVED.md` that some endpoints publish a live, authoritative JSON Schema via a `$schema` response key — a better oracle than `openapi.yaml` where present
- *(gamma)* Record in `docs/plans/2026-08-19-gamma-type-parity-worklist.md` that finding #10 is partially fixed — `UserResponse`/`UserInfo` done, `SearchProfile::address` still open — and that `/public-search` serves no `$schema`
- *(gamma)* Record in `docs/specs/gamma/OBSERVED.md` and `docs/plans/2026-08-19-gamma-type-parity-worklist.md` that finding #10 is fully fixed, and add the `profiles[]` null-element failure as a new confirmed finding found while fixing it

### 🧪 Testing

- *(gamma)* Capture live comment payloads as fixtures
- *(gamma)* Add `tests/wire_agreement.rs`, asserting both directions of agreement between the comment types and captured live payloads
- *(gamma)* Fix the four live comment tests
- *(gamma)* Close the wire-agreement guard's null-exemption hole — every key a type emits must now be present on the wire or declared in `EXPECTED_ABSENT` with a reason, and array lengths must agree before zipping rather than comparing only `min(len)`
- *(py)* Guard the comment getters against silent `None` with a real instance — `hasattr` on the class object is true for every `py_type!` property regardless of which JSON key it resolves, so the guard moved to a Rust test that deserializes the shared fixture and asserts every getter is non-`None`
- *(gamma)* Capture live profile payloads as fixtures and extend `tests/wire_agreement.rs` to `Profile`
- *(gamma)* Stop `live_get_profile_by_address` from swallowing a deserialization error as if it were a 404
- *(gamma)* Capture live `/public-profile` payloads as fixtures and extend `tests/wire_agreement.rs` to `UserResponse`, including a hand-written case for the schema's explicit `null` on `users`
- *(gamma)* Capture live `/public-search` payloads as fixtures, including a response with a `null` `profiles[]` entry, and extend `tests/wire_agreement.rs` to `SearchProfile`/`SearchResponse`
- *(gamma)* Add a live test asserting `public_search(...).search_profiles(true)` succeeds — the regression that would have caught the `profiles[]` null failure

### 🎨 Styling

- Rustfmt the mock_api thread assertion

## [0.27.0] - 2026-08-15

### 🚀 Features

- *(ci)* Add canonical-tree walker producing JSON-pointer changes
- *(ci)* Report changed key paths in the drift summary
- *(ci)* Compose the issue body in Python under a tested size budget
- *(ci)* Fingerprint the canonical diff
- *(ci)* Exit 3 for drift we have accepted as permanent
- *(ci)* Put adopt and acknowledge commands in the issue body
- *(data)* [**breaking**] Add Position fee-basis fields, mark non_exhaustive
- *(py)* Expose Position fee-basis fields
- *(data)* Add typed Allowance for approval amounts
- *(data)* Add approval types for /v1/approvals
- *(data)* Add approvals namespace for /v1/approvals

### 🐛 Bug Fixes

- *(ci)* Treat integral floats as ints when canonicalizing specs
- *(ci)* Distinguish int from bool when walking canonical trees
- *(ci)* Identify drift issues by spec label, not tokenized title search
- *(ci)* Drop branch and PR machinery from the nightly drift check
- *(data)* Make deposit and withdrawal activity filters reachable

### 📚 Documentation

- *(specs)* Design for trustworthy schema drift findings
- *(plans)* Implementation plan for trustworthy schema drift findings
- Describe label-based drift issue identity in CLAUDE.md
- *(specs)* Design for drift findings that converge
- *(plans)* Implementation plan for drift convergence
- *(specs)* Design for absorbing the data spec drift
- *(plans)* Implementation plan for absorbing the data spec drift

### 🧪 Testing

- *(ci)* Make the bool/int branch-order guard mutation-sensitive

### ⚙️ Miscellaneous Tasks

- *(specs)* Acknowledge clob's upstream quoting regression
- *(specs)* Sync the data spec and release 0.27.0

## [0.26.1] - 2026-08-06

### 🐛 Bug Fixes

- *(core)* Stop rate limit buckets over-spending their published quota

### 🧪 Testing

- *(data)* Soak /closed-positions against the live rate limit
- *(data)* Bracket the real burst ceiling for /closed-positions

## [0.26.0] - 2026-08-06

### 🚀 Features

- *(core)* Make an observed 429 back off the whole client

### 🐛 Bug Fixes

- *(core)* Stop a zero Retry-After from collapsing the retry backoff

## [0.25.0] - 2026-08-05

### 🚀 Features

- *(clob)* Model the per-signer trading rate limits

### 🐛 Bug Fixes

- *(core)* Cap the health routes the hosts actually serve

## [0.24.2] - 2026-08-05

### 🐛 Bug Fixes

- *(clob)* Stop size truncation from shortening compliant amounts
- *(clob)* Apply the venue's per-tick decimal limits to limit orders
- *(clob)* Reject order quantities below the venue's 2-decimal floor

## [0.24.1] - 2026-08-03

### 🐛 Bug Fixes

- *(clob)* Apply the venue's per-tick decimal limits to market orders

### 📚 Documentation

- *(clob)* Stop linking public docs at pub(crate) precision helpers
- *(claude.md)* Document the rustdoc gate and correct the CI job count

## [0.24.0] - 2026-08-03

### 🚀 Features

- *(ci)* Classifier core for nextest failure categorization
- *(ci)* Nextest NDJSON parser in classify_failures.py
- *(ci)* CLI for classify_failures.py with classify and merge subcommands
- *(ci)* Canonicalize() for OpenAPI YAML drift comparison
- *(ci)* Structural drift detection for OpenAPI specs
- *(ci)* CLI for diff_openapi.py with check subcommand
- *(ci)* Channel-aware drift detection with per-entry vendored labels

### 🐛 Bug Fixes

- *(ci)* Classify live_ws panics and emit binary-aware retry filters
- *(ci)* Harden nightly workflows against review findings
- *(ci)* Disable rustdoc for the polyoxide-cli bin to stop the doc-build race
- *(ci)* Degrade gracefully when Actions may not open drift PRs
- *(data)* Pin holders `limit` clamping after upstream dropped the 400
- *(clob)* [**breaking**] Type Trade.owner as String, not Address
- *(clob)* [**breaking**] Type Notification.id as u64, not String

### 📚 Documentation

- Add design for nightly Polymarket API smoketest
- Add implementation plan for nightly API smoketest
- *(claude.md)* Document nightly API smoketest workflows
- Record nightly smoketest coverage extension
- Add SELF-HEALING.md describing the nightly drift machinery
- *(specs)* Refresh perps counts and guard them against future rot
- Use backticks for the org settings URL placeholder

### 🧪 Testing

- *(ci)* Add nextest output fixtures for classifier
- *(ci)* Add OpenAPI drift fixtures
- *(ci)* Cover diff_openapi exit code 2 on YAML parse error

### ⚙️ Miscellaneous Tasks

- *(ci)* Bootstrap python project for nightly helper scripts
- *(ci)* Commit scripts uv.lock for reproducibility
- *(ci)* Drop unused pytest import from classifier tests
- *(ci)* Drop unused TestOutcome import from classifier tests
- *(ci)* Consolidate diff_openapi imports at top of test file
- Run helper script unit tests on every PR
- Nightly behavioral smoketest workflow against live Polymarket APIs
- Nightly schema-drift workflow with auto-PR + tracking issue
- Cover clob live_ws and polyoxide-cli in the nightly behavioral matrix
- Extend nightly schema drift to all published Polymarket specs
- Bump pinned actions to their first Node 24-native majors
- *(specs)* Sync perps spec from upstream
- *(specs)* Sync perps-ws spec from upstream

## [0.23.0] - 2026-07-25

### 🚀 Features

- *(clob)* Document the TradeStatus lifecycle and add terminal-state helpers

### 🐛 Bug Fixes

- *(core)* Align the CLOB rate limiter with the published limits
- *(gamma)* [**breaking**] Return relationship rows, not tags, from related-tags
- *(data)* Add the missing ESPORTS leaderboard category
- *(clob)* Stop every WebSocket connect panicking on rustls provider ambiguity
- *(clob)* [**breaking**] Parse the sports frames the venue sends, and make the user channel usable
- *(gamma)* Tolerate the nulls the schema permits in RelatedTag

### 📚 Documentation

- *(plans)* Add the prader-audit upstream worklist
- *(data)* Correct the holders limit range to 0-500 default 20
- *(specs)* Record where upstream's sports contract diverges from the wire
- Fix broken intra-doc links across the workspace

### 🧪 Testing

- *(clob)* Add live WebSocket tests

### ⚙️ Miscellaneous Tasks

- Fail the build on rustdoc warnings

## [0.22.0] - 2026-07-25

### 🚀 Features

- *(clob)* Add ListOrders::send_raw as an escape hatch

### 🐛 Bug Fixes

- *(clob)* [**breaking**] Pin OpenOrder to the shape the venue actually returns
- *(clob)* Make live_v2_place_and_cancel rest instead of filling
- *(clob)* [**breaking**] Surface FAK/FOK kill outcomes as typed non-retriable errors
- *(core)* Retry 425 Too Early alongside 429

### 📚 Documentation

- Correct the inverted L1/L2 description in CLAUDE.md

## [0.21.0] - 2026-07-24

### 🐛 Bug Fixes

- *(clob)* [**breaking**] Correct the ClobAuth EIP-712 struct and auth domain
- *(clob)* Send a checksummed POLY_ADDRESS on L1 auth

### 🧪 Testing

- Close coverage gaps in the new WebSocket and sibling-host code
- *(clob)* Add a live L1 auth test

## [0.20.0] - 2026-07-24

### 🚀 Features

- *(clob)* [**breaking**] Add sports channel and the gated market WebSocket events
- *(data)* Add pnl and rankings namespaces on undocumented hosts

### ⚙️ Miscellaneous Tasks

- *(specs)* Mirror AsyncAPI specs and record undocumented hosts

## [0.19.0] - 2026-07-24

### 🚀 Features

- *(data)* Add combos and misc namespaces, close query param gaps
- *(clob)* [**breaking**] Add rewards multi/rebates, public rewards, cursor pagination
- *(gamma)* Expose missing keyset, search, and series query params

### 📚 Documentation

- Record new namespaces and the unimplemented upstream APIs

### ⚙️ Miscellaneous Tasks

- *(specs)* Refresh upstream specs and mirror perps, bridge, combos-rfq

## [0.18.2] - 2026-07-24

### 🐛 Bug Fixes

- *(clob)* Remove live_activity endpoint hitting nonexistent path
- *(data)* Tolerate unrecognized ActivityType/TradeSide values

## [0.18.1] - 2026-07-15

### 🚀 Features

- *(clob)* Add PricesHistoryQuery + prices_history_with for time-bounded history
- *(cli)* Scaffold clob prices download command group
- *(cli)* Prices download domain types (format, target, manifest)
- *(cli)* Prices download selection helpers (file parse, dedupe, token ids)
- *(cli)* Gamma-based market discovery for prices download
- *(cli)* Dataset writers (csv/jsonl) with atomic temp-then-rename
- *(cli)* Rate-limited, retrying single-market price fetch
- *(cli)* Write prices download run manifest as jsonl
- *(cli)* Orchestrate concurrent resumable prices download + manifest
- *(cli)* Feature-gated Parquet dataset writer

### 🐛 Bug Fixes

- *(cli)* Validate token ids for path safety; guard closed/open + fidelity

### 📚 Documentation

- Skip README doctests that require non-default features
- *(cli)* Design spec for clob prices download (bulk ML dataset)
- *(cli)* Implementation plan for clob prices download

### 🧪 Testing

- *(cli)* Live prices download test + document clob command group

## [0.18.0] - 2026-06-30

### 💥 Breaking Changes

- *(clob)* [**breaking**] **CLOB V2 migration.** Orders are now signed with the Polymarket CLOB V2 EIP-712 scheme (domain version "2", V2 exchange contracts, 11-field signed struct). V1-shaped orders are rejected by the live exchange as of 2026-04-28. `Order`/`SignedOrder` gained `timestamp`/`metadata`/`builder` and dropped `taker`/`nonce`/`feeRateBps` from the signed struct; fees are no longer signed (collected on-chain at match).
- *(clob)* [**breaking**] `SignatureType` gained `Poly1271` (EIP-1271; signing not yet implemented — rejected at the signing layer).
- *(clob)* [**breaking**] `create_builder_key` now uses L2 auth and returns `BuilderApiKeyResponse { key, secret, passphrase }`.

### 🚀 Features

- *(clob)* Builder-program attribution: `ClobBuilder::builder_code(B256)` stamps the signed `builder` field on every order.

### 🐛 Bug Fixes

- *(clob)* Mask the order `salt` to the JS-safe-integer range (2^53-1) so the live CLOB V2 exchange accepts orders. A raw 64-bit salt overflows the server's numeric parse and, since `salt` is part of the EIP-712 signed struct, corrupts the signature — orders were rejected with "Invalid order payload". Matches the official Polymarket clients; verified against the live exchange.

## [0.17.0] - 2026-06-28

### 🚀 Features

- *(clob)* [**breaking**] Remove dead RFQ trading API
- *(clob)* [**breaking**] Require builder_code arg for builder_trades

### 📚 Documentation

- *(specs)* Re-sync OpenAPI mirrors from upstream

### 🧪 Testing

- *(keychain)* Isolate tests in per-test keychain services

### ⚙️ Miscellaneous Tasks

- Add .env.example documenting auth credentials

## [0.16.0] - 2026-06-02

### 🐛 Bug Fixes

- *(gamma)* [**breaking**] Make Market.description optional for abridged markets
- *(clob)* [**breaking**] Align reads and schemas with the live API

### 📚 Documentation

- *(specs)* Refresh OpenAPI mirrors and endpoint docs from upstream

## [0.15.1] - 2026-05-22

### 🐛 Bug Fixes

- *(gamma)* Fix double-slash URL in health ping — `format!("{}/status", base_url)` produced `//status` because `Url::Display` normalizes to include a trailing `/`; switched to `base_url.join("status")?`

### 🧪 Testing

- *(clob, data, gamma)* Add per-crate ping mock tests covering the 200 happy path, unexpected 3xx as error, and 5xx as error

## [0.15.0] - 2026-04-23

### 💥 Breaking Changes

- *(gamma)* `markets().query_by_information()` and `markets().query_abridged()` now return request builders; add `.send().await` and pass the body by value instead of by reference

### 🚀 Features

- *(gamma)* Add `limit()` / `offset()` pagination to `query_by_information` and `query_abridged` builders, sent on the URL query string because the server ignores body-level pagination; defaults to `limit=1000` to prevent silent truncation at the server's 20-row default

### 🧪 Testing

- *(gamma)* Add mock coverage for explicit `limit` / `offset` on `query_by_information`

## [0.14.0] - 2026-04-22

### 🚀 Features

- *(core)* Add `HttpClient::get_bytes` helper for endpoints that return binary responses (e.g. ZIP archives)
- *(clob)* Add path-variant market metadata endpoints: `fee_rate_path`, `tick_size_path`, `neg_risk_path`
- *(clob)* Add `markets().clob_market_details(condition_id)` returning structured CLOB market metadata
- *(clob)* Add `markets().market_by_token(token_id)` for token→market lookup
- *(clob)* Add `markets().live_activity_bulk(ids)` and `live_activity_market(condition_id)` for real-time order/trade counters
- *(clob)* Add `markets().batch_prices_history(req)` for bulk historical price queries
- *(gamma)* Add events endpoints: `list_creators`, `get_creator`, `list_paginated`, `list_results`, `list_keyset`
- *(gamma)* Add markets endpoints: `get_description`, `query_by_information`, `query_abridged`, `list_keyset`
- *(gamma)* Add `series().get_summary`, `get_summary_by_slug`, and `comment_count`
- *(gamma)* Add `sports().get_team(id)` and `user().get_by_address(addr)`
- *(data)* Add `data.market_positions()` namespace with `ListMarketPositions` builder
- *(data)* Add `data.accounting().snapshot(user)` returning raw ZIP bytes
- *(relay)* Add `list_relayer_api_keys()` and `list_transactions()` methods with per-endpoint auth dispatch

### 💥 Breaking Changes

- *(clob)* `update_balance_allowance` now calls `PUT /balance-allowance` with query params; signature changed to `(asset_type, token_id: Option<_>, signature_type: Option<_>)`
- *(gamma)* `tags().get_related_detailed` now returns `Vec<Tag>` instead of a single `Tag`
- *(gamma)* `keep_closed_markets` is typed as an integer to match the upstream contract
- *(gamma)* Removed ghost `/events/slug/{slug}/related` endpoint that never existed upstream
- *(clob)* `GET /trades` now requires `maker_address` per upstream contract
- *(relay)* Response type fields aligned with upstream OpenAPI (some renames)

### 🐛 Bug Fixes

- *(clob)* Add `POST /heartbeats` endpoint for session keep-alive
- *(gamma)* Probe `/status` for health pings instead of a non-existent path
- *(data)* Add `MakerRebate` and `ReferralReward` activity variants

### 📚 Documentation

- *(specs)* Vendor upstream Polymarket OpenAPI YAMLs as the source of truth
- *(specs)* Sync per-endpoint markdown docs with upstream OpenAPI (CLOB, Gamma, Data, Relay)

### 🧪 Testing

- *(clob)* Add mock and live coverage for all new market endpoints and the PUT balance-allowance migration
- *(clob)* Add mock coverage for `heartbeat`
- *(gamma)* Add mock, live, and serde roundtrip coverage for all new events/markets/series/sports/user endpoints
- *(gamma)* Add mock and live coverage for `get_related_detailed` and `get_related_detailed_by_slug`
- *(data)* Add mock, live, and serde coverage for market-positions and accounting snapshot
- *(relay)* Add mock coverage for new endpoints including builder-HMAC vs static-key auth dispatch
- *(py)* Remove stale xfail markers on CLOB market tests

### 🎨 Styling

- *(gamma)* Apply rustfmt to example probes

## [0.13.1] - 2026-04-22

### 🚀 Features

- *(gamma)* Add `markets().get_many(ids)` for batch market lookup regardless of open/closed state

### 🐛 Bug Fixes

- *(gamma)* Work around the upstream `closed=false` default that silently dropped closed markets from `list().id()`, `.slug()`, and `.condition_ids()` lookups; new `get_many` helper fans out `closed=true` + `closed=false` requests in parallel and the trap is called out in the doc comments of the affected list builder methods

### 🚜 Refactor

- *(gamma)* Move Cloudflare probes from tests to examples

### 📚 Documentation

- *(gamma)* Document safe batch sizes on `query_many` methods

### 🧪 Testing

- *(gamma)* Add binary-search probe for batch-ID URL ceiling
- *(gamma)* Add burst probe for Cloudflare rate-limit responses

### ⚙️ Miscellaneous Tasks

- Ignore `.loom/` local data directory

## [0.13.0] - 2026-04-16

### 🚀 Features

- *(core)* Add `keychain` module for OS credential storage (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- *(core)* Add `keychain::delete` function for credential removal
- *(clob)* Add `Account::from_keychain()` and `Account::save_to_keychain()` for keychain-based credential loading
- *(clob)* Add `Account::delete_from_keychain()` to remove all stored credentials
- *(clob)* Add `ApiCredentials::from_keychain()` for WebSocket authentication
- *(relay)* Add `BuilderAccount::from_keychain()` and `from_keychain_relayer_api_key()` for keychain-based credential loading
- *(relay)* Add `BuilderAccount::delete_from_keychain()` to remove all stored credentials
- *(cli)* Add `credentials store` and `credentials show` subcommands for keychain management
- *(cli)* Add `credentials delete` subcommand to remove stored credentials
- *(cli)* Add `--credential-source keychain` flag to `ws user` command

### 🐛 Bug Fixes

- *(relay)* Clear stale passphrase from keychain when saving config with `passphrase = None`

### 🚜 Refactor

- Consolidate keychain service name strings into shared `KEYCHAIN_SERVICE` constants

### 🔧 Build

- Add `keyring` dependency with `apple-native`, `windows-native`, `async-secret-service`, and `crypto-rust` backends
- Add `keychain` feature flag to core, clob, relay, polyoxide, and cli crates

### 🧪 Testing

- *(clob)* Add keychain roundtrip and delete integration tests
- *(relay)* Add keychain roundtrip, no-config, and stale passphrase integration tests
- *(cli)* Add parsing tests for credentials delete and keychain credential source

## [0.12.5] - 2026-04-15

### 🚀 Features

- *(relay)* Add relayer API key authentication as alternative to HMAC builder credentials
- *(relay)* Add `AuthConfig` enum and `RelayerApiKeyConfig` for dual auth support
- *(relay)* Wire `AuthConfig` into request signing and add builder convenience methods

### 🚜 Refactor

- *(relay)* Encapsulate `RelayerApiKeyConfig` and validate inputs
- *(relay)* Remove deprecated `config()` and extract `parse_signer`

### 🧪 Testing

- *(relay)* Add relayer API key integration tests

### 📚 Documentation

- Rewrite all workspace READMEs with accurate code examples and full API coverage
- *(relay)* Add README with both auth methods, builder pattern, and gasless redemption examples
- Fix CLAUDE.md data API namespace, add MSRV, correct relay env vars

## [0.12.4] - 2026-04-14

### 🐛 Bug Fixes

- *(clob)* Limit order salt to u64 range to prevent serialization panic and API rejection

## [0.12.3] - 2026-04-14

### 🐛 Bug Fixes

- *(clob)* Serialize order salt as u128 number using serde_json `arbitrary_precision` instead of string encoding

## [0.12.2] - 2026-04-14

### 🐛 Bug Fixes

- *(clob)* Serialize order salt as string to avoid serde_json rejection of u128 values exceeding u64::MAX

## [0.12.1] - 2026-04-01

### 🐛 Bug Fixes

- *(clob)* Add missing `id` and `timestamp` fields to `Notification` struct
- *(clob)* Add `next_cursor` pagination support to `ListClobTrades` request builder

### 📚 Documentation

- Fix `transactionsHashes` field name in CLOB orders spec

### 💼 Other

- Add MIT/Apache-2.0 dual license and PyPI package metadata
- Skip already-published crates during crates.io release

## [0.12.0] - 2026-03-27

### ⚠️ Breaking Changes

- *(clob)* `BalanceAllowanceResponse.allowance: String` replaced with `allowances: HashMap<String, String>` to match upstream API change (#1)
- *(clob)* Several fields on `Market` changed to `Option<T>`: `question_id`, `minimum_order_size`, `minimum_tick_size`, `description`, `question` (#1)
- *(clob)* Several fields on `SpreadResponse` changed to `Option<T>`: `token_id`, `bid`, `ask` (#1)
- *(clob)* Several fields on `LastTradePriceResponse` changed to `Option<T>`: `token_id`, `last_trade_price`, `timestamp`; new optional fields `price`, `side` (#1)

### 🚀 Features

- *(core)* Add concurrency limiter to `HttpClient` for Cloudflare connection limits (#3)
- *(core)* Set default concurrency limits in all API client builders (#3)
- *(relay)* Hold concurrency permit across full request lifetime (#3)
- *(data)* Hold concurrency permit across full request lifetime (#3)
- *(py)* Scaffold `polyoxide-py` crate with PyO3 + maturin (#2)
- *(py)* Add Gamma, Data, and CLOB domain type wrappers via `py_type!` macro (#2)
- *(py)* Add Gamma, Data, and CLOB clients with sync + async variants (#2)
- *(py)* Add `.pyi` type stubs and `py.typed` marker (#4)
- *(py)* Export all type classes from polyoxide package (#4)

### 🐛 Bug Fixes

- *(clob)* Update `BalanceAllowanceResponse` for upstream API field change from `allowance` to `allowances` (#1)
- *(clob)* Make optional fields on `Market`, `SpreadResponse`, and `LastTradePriceResponse` to match upstream API (#1)
- *(clob)* Serialize order salt as string to avoid serde_json rejection of u128 values exceeding u64::MAX

### 🧪 Testing

- *(core)* Add concurrency limiter integration and default verification tests (#3)
- *(py)* Add live API tests and expand unit tests (#2)

### 📚 Documentation

- Add upstream Polymarket API specs for CLOB, Gamma, Data, and Relay
- Add design and implementation plans for PyPI publishing (#4)

### 🔧 CI

- Add Python bindings test job (#2)
- Add PyPI wheel build and publish to release workflow (#4)
- Filter release artifacts to exclude Python wheels (#4)

### ⚙️ Miscellaneous Tasks

- Add `*.so` and `.claude-squad` directory to `.gitignore`

## [0.11.0] - 2026-03-05

### 🚀 Features

- *(clob)* Add RFQ namespace for request-for-quote trading
- *(clob)* Add rewards namespace for liquidity reward tracking
- *(clob)* Add auth namespace for API key management
- *(clob)* Add batch order operations and single order lookup
- *(clob)* Add batch pricing endpoints for books, prices, midpoints, spreads, and last trades
- *(clob)* Add single pricing, live activity, calculate price, and server time endpoints
- *(clob)* Add heartbeat, notifications, order scoring, and ban status endpoints
- *(clob)* Add simplified/sampling market lists and builder trades endpoint
- *(clob)* Add `ListClobTrades` request builder with filter methods
- *(gamma)* Add missing endpoints and query params across all namespaces
- *(gamma)* Add public search endpoint for profiles, events, and tags
- *(gamma)* Complete `UserResponse` with profile, bio, and badge fields
- *(data)* Add trader leaderboard endpoint and move `TimePeriod` to types

### 🐛 Bug Fixes

- *(clob)* Fix 5 critical deserialization crashes against live API
- *(clob)* Add missing fields to `OpenOrder`, `OrderResponse`, and `Trade` types
- *(clob)* Add missing fields to `OrderBook` type
- *(gamma)* Correct 6 serde renames and expand `SeriesInfo` to match live API
- *(gamma)* Align SDK types with real Polymarket API responses
- *(data)* Add missing `verified` field to `Holder` type
- *(core)* Replace silent epoch fallback with explicit panic in `current_timestamp`
- *(cli)* Use `floor_char_boundary` for safe UTF-8 string truncation

### 🚜 Refactor

- *(clob)* Make gamma dependency optional behind `gamma` feature flag
- *(clob)* Extract WebSocket subscription validation helper
- *(clob)* Deduplicate EIP-712 order conversion and digest computation
- *(core)* Simplify rate limiter config with `endpoint_limit` helper
- *(relay)* Extract retry helper, named constants, and module-level types

### 🧪 Testing

- *(clob)* Add 73 new tests: WebSocket message types, utils, error, mock API (retry, errors, order creation), rejection and edge cases
- *(gamma)* Add mock tests for open() inversion, volume serde renames, and events namespace
- *(data)* Add 12 mock HTTP tests bootstrapping polyoxide-data coverage
- *(core)* Fix retry mock strictness and add 401/403/408 error tests
- *(cli)* Add multibyte and emoji edge case tests for truncate
- Add mockito HTTP mock tests for core, gamma, and clob

### 📚 Documentation

- Add docstrings across workspace, complete relay crate coverage
- Fix incorrect API examples and update project documentation

### 🔧 Build

- Add mockito workspace dev-dependency for HTTP mock tests
- Move futures-util from workspace deps to per-crate
- Specify per-crate tokio features instead of workspace-wide

### 🎨 Styling

- Apply rustfmt to gamma and data

## [0.10.0] - 2026-03-01

### ⚠️ Breaking Changes

- *(clob)* `WebSocketBuilder::market_url()` and `user_url()` now return `Result<Self, WebSocketError>` to enforce `wss://` scheme validation
- *(core)* MSRV raised from 1.75 to 1.91 (required by `str::floor_char_boundary`)
- *(core)* HTTP client now disables redirect following to prevent open redirect attacks

### 🐛 Bug Fixes

- *(relay)* Strip `0x` prefix from `PROXY_INIT_CODE_HASH` to prevent `hex::decode` panic at runtime
- *(core)* Truncate response bodies in error logs to 512 chars to prevent sensitive data leakage
- *(clob)* Truncate response bodies in error logs to 512 chars
- *(relay)* Truncate response bodies in error logs to 512 chars
- *(core)* Add 10-second connect timeout to HTTP client
- *(clob)* Enforce `wss://` scheme on WebSocket builder URLs to prevent plaintext connections

### 🛡️ Security

- *(clob)* Redact `private_key` in `AccountConfig` `Debug` impl to prevent secret leakage in logs
- *(relay)* Redact signer key in `BuilderAccount` `Debug` impl, showing only address
- Harden `.gitignore` to cover `.env.*`, `*.pem`, `*.key`, and `account.json`

### 🧪 Testing

- *(core)* Add tests for prefix collisions, concurrency, and retry edge cases
- *(core)* Add unit tests for `truncate_for_log` including multibyte boundary handling
- *(clob)* Add tests for WebSocket URL scheme validation
- *(clob)* Add test for `AccountConfig` Debug redaction
- *(relay)* Add test for `BuilderAccount` Debug redaction

### 🔧 CI

- Remove sccache and add lightweight ci profile
- Consolidate publish steps into retry loop
- Use cargo-nextest for parallel test execution
- Merge lint/test jobs and remove redundant release build

### 🎨 Styling

- Apply cargo fmt across workspace

## [0.9.2] - 2026-03-01

### 🚀 Features

- *(core)* Parse `Retry-After` header for server-guided backoff delays
- *(core)* Expose `RetryConfig` through all high-level client builders (`Clob`, `Gamma`, `DataApi`, `RelayClient`)

### 🐛 Bug Fixes

- *(core)* Add segment-boundary-aware endpoint matching to prevent `/price` from matching `/prices-history`
- *(core)* Replace `SystemTime` nanos with `fastrand` for uniform backoff jitter
- *(clob)* Generate fresh L1 auth timestamp on each retry to avoid staleness
- *(relay)* Add retry loops with 429 handling to all relay endpoints

## [0.9.1] - 2026-02-28

### ⚙️ Miscellaneous Tasks

- Prune unused deps, tokio/alloy features, and fix TLS duplication
- Apply rustfmt formatting across workspace

### 📚 Documentation

- Add testing conventions and module organization to CLAUDE.md

### 🔧 CI

- Replace rust-cache with sccache for shared compilation caching

## [0.9.0] - 2026-02-28

### 🚀 Features

- *(core)* Add per-endpoint rate limiting with configurable quotas, retry-on-429 backoff with jitter, and governor-based throttling

### 🐛 Bug Fixes

- *(core)* Fix rate limit quota precision, backoff jitter range, and add missing endpoint quota
- *(core)* Carry message context in RateLimit error variant and downgrade retry log level
- *(core)* Redact secrets from Debug impls to prevent log leakage
- *(clob)* Use BUY/SELL strings for price endpoint side parameter
- *(clob)* Use typed request for `get_fee_rate` with correct field and token_id
- *(clob)* Fix tautological assertion in salt test
- *(clob)* Reject NaN and infinity in order parameter validation
- *(clob)* Classify service errors as Api instead of Validation
- *(clob)* Return None on insufficient liquidity and increase salt entropy
- *(data)* Route all HTTP calls through Request<T> for rate limiting and 429 retries
- *(data)* Align Display impls with serde SCREAMING_SNAKE_CASE for sort enums
- *(relay)* Replace unwraps with error propagation and compile-time address validation
- *(cli)* Replace `process::exit` with Result-based error handling in WS credentials and completions
- *(cli)* Reject invalid activity types with error instead of silently dropping

### 🚜 Refactor

- *(core)* Make `Signer::new` infallible

### 🧪 Testing

- *(core)* Add unit tests for Request query builder and typed request
- *(clob)* Add unit tests for EIP-712 signing, WS types, and auth credentials
- *(clob)* Add live integration tests for CLOB public endpoints
- *(data)* Add unit tests for enum serialization, builders, and type serde
- *(data)* Add live integration tests for data API public endpoints
- *(gamma)* Add unit tests for type deserialization and client builder
- *(relay)* Add unit tests for types serde, address derivation, signature packing, hex constants, contract config, and builder defaults
- *(cli)* Add unit tests for argument parsing across all subcommands
- Add live integration tests for all API endpoints

## [0.8.1] - 2026-02-26

### 🚜 Refactor

- *(core)* Remove verbose request/response body logging from HTTP clients
- *(clob)* Remove verbose request/response body logging from HTTP clients
- *(relay)* Remove verbose request/response body logging and leftover `eprintln!` debug statements

## [0.8.0] - 2026-02-25

### 🚀 Features

- Migrate price and size fields from String to Decimal with `serde(with = "rust_decimal::serde::str")` for accurate serialization

## [0.7.1] - 2026-02-24

### 🐛 Bug Fixes

- *(clob)* Add `canceled_order_id` and `message` fields to `CancelResponse` and mark `success` as default.

## [0.7.0] - 2026-02-24

### 🚀 Features

- *(relay)* Update builder to default to Polygon Mainnet (137) and relay V2 (`https://relayer-v2.polymarket.com/`)
- *(relay)* Update `RelayClientBuilder` to implement `Default`

## [0.6.1] - 2026-02-20

### 🚀 Features

- *(core)* Add unified authentication module with HMAC signing and timestamp generation
- *(core)* Add `Signer` struct supporting multiple base64 formats (URL-safe and standard)
- *(core)* Add `current_timestamp()` function for safe Unix timestamp generation
- *(core)* Add `Base64Format` enum to support both URL-safe and standard base64 encoding
- *(core)* Add `impl_api_error_conversions!` macro to reduce error conversion boilerplate

### 🚜 Refactor

- *(core)* Consolidate HMAC signing logic from CLOB and Relay into shared `Signer` implementation
- *(core)* Consolidate timestamp generation into single safe implementation
- *(clob)* Refactor `Signer` to use `polyoxide_core::Signer` as thin wrapper with CLOB-specific error handling
- *(clob)* Extract market metadata fetching into `get_market_metadata()` helper method
- *(clob)* Extract fee rate fetching into `get_fee_rate()` helper method
- *(clob)* Extract maker address resolution into `resolve_maker_address()` helper method
- *(clob)* Extract order building into `build_order()` helper method
- *(clob)* Simplify `create_order()` and `create_market_order()` by using extracted helpers (~140 lines removed)
- *(relay)* Update to use `polyoxide_core::Signer` and `current_timestamp()` for authentication
- *(gamma)* Use `impl_api_error_conversions!` macro to reduce error conversion boilerplate
- *(data)* Use `impl_api_error_conversions!` macro to reduce error conversion boilerplate

## [0.6.0] - 2026-02-19

### 🚀 Features

- *(relay)* Add gas estimation for redemption transactions with safety buffer and relayer overhead
- *(relay)* Add `estimate_redemption_gas` method to estimate gas costs using RPC provider simulation
- *(relay)* Add `submit_gasless_redemption_with_gas_estimation` method for redemptions with optional gas estimation
- *(relay)* Add default RPC URLs to contract configuration for Polygon mainnet and Amoy testnet
- *(repo)* Rename project from `polyte` to `polyoxide`

## [0.5.0] - 2026-02-19

### 🚀 Features

- *(clob)* Add health API namespace with ping method
- *(relay)* Introduce `polyte-relay` crate for interacting with relayer services
- *(relay)* Add gasless redemption functionality via relayer v2 API
- *(relay)* Introduce `BuilderAccount` for centralized signer and config management
- *(clob)* Introduce `MarketOrderArgs` and market order calculation utilities
- *(clob)* Enhance order creation logic with maker address determination and optional funder parameter
- *(clob)* Integrate polyte-gamma client into Clob and ClobBuilder
- *(clob)* Add `neg_risk` and `tick_size` methods to markets search
- *(clob)* Add `neg_risk` support for orders
- *(clob)* Implement funder and signature type support
- *(clob)* Add `get_by_token_ids` method to retrieve markets by token IDs
- *(clob)* Add `prices_history` method for historical token prices
- *(clob)* Add Display impl for OrderKind and SignatureType
- *(clob)* Introduce PartialCreateOrderOptions for enhanced order creation flexibility
- *(gamma)* Introduce Gamma User API
- *(gamma)* Add `volume_1yr` field to match Gamma API naming conventions
- *(data)* Add USDC balance endpoint to account API
- *(data)* Update `BalanceAllowanceResponse` to use HashMap for allowances
- *(polyte)* Add DataApi to unified Polymarket client
- *(types)* Add `is_proxy` method to `SignatureType` enum
- *(error)* Add service error creation method to ClobError

### 🐛 Bug Fixes

- *(clob)* Use precise decimal arithmetic and explicit TickSize parsing
- *(clob)* Update order amount calculations to support 6 decimal places
- *(clob)* Update owner field in order payload to use account address
- *(clob)* Add custom deserialization for minimum_tick_size to handle both string and number formats
- *(gamma)* Correct typos in Market and Event field names
- *(error)* Enhance API error logging by capturing raw response body
- *(tests)* Update salt generation test to check for non-empty output

### 🚜 Refactor

- *(clob)* Serialize OrderSide enum variants as 'BUY' and 'SELL' strings
- *(clob)* Update ClobBuilder to use optional account and introduce with_account method
- *(clob)* Restructure EIP-712 domain and order definitions into protocol module
- *(clob)* Implement custom serialization and deserialization for `SignatureType` enum
- *(gamma)* Rename `active` filter to `open` for market and series listing
- *(gamma)* Rename user proxy field to `proxyWallet` in API response
- *(gamma)* Rename `wallet_address` query parameter to `address` in public profile API
- *(core)* Add shared HTTP client infrastructure
- Remove Result type aliases in favor of explicit types
- Refactor amount calculations to use f64 arithmetic

### ⚙️ Miscellaneous Tasks

- Add CLAUDE.md with project guidance and architecture overview
- Update `thiserror` dependency to version 2.0.17
- Add `specta` support in multiple modules
- Add `dotenvy` dependency

## [0.4.0] - 2026-01-05

### 🐛 Bug Fixes

- *(clob)* Correct the type of the OrderBook timestamp

### ⚙️ Miscellaneous Tasks

- Add changelog and publish it on Github Releases page

## [cli-v0.3.2] - 2025-12-04

### 🐛 Bug Fixes

- *(cli)* Use limit flag instead of hardcorded value
- *(gamma)* Typo

### 🚜 Refactor

- *(cli)* Move duplicates into `common` module
- Use clap `value_parser` for comma-separated arguments

### ⚙️ Miscellaneous Tasks

- Format
- Remove unnecessary doc

## [cli-v0.3.1] - 2025-12-04

### 🚜 Refactor

- *(cli)* Improve credential error messages for `ws user` command

## [cli-v0.3.0] - 2025-12-03

### 🚀 Features

- *(clob)* Add websocket support
- *(cli)* Add support for Clob websockets

### 🚜 Refactor

- Consolidate auth into account module

### 📚 Documentation

- Update Clob documentation

### ⚙️ Miscellaneous Tasks

- Remove clob examples

## [cli-v0.2.4] - 2025-12-01

### 🐛 Bug Fixes

- Change `comment_count` type from u32 to i64 to prevent sentinel value issues

### 🚜 Refactor

- Extract common Request builder to `polyte-core`

### 📚 Documentation

- Update CLI README
- Update `polyte` README

### ⚙️ Miscellaneous Tasks

- Remove gamma examples
- Update Event type in Gamma

## [cli-v0.2.1] - 2025-12-01

### 🚀 Features

- Add support for Builders API

### 📚 Documentation

- Fix typo

## [cli-v0.2.0] - 2025-11-30

### 🚀 Features

- Add support for Data API

### 🚜 Refactor

- Remove deprecated code
- Reuse `SortOrder` enum

## [cli-v0.1.5] - 2025-11-28

### 🐛 Bug Fixes

- *(gamma)* Change `order_min_price_tick_size` and `order_min_size` to `f64`

### 🚜 Refactor

- *(cli)* Chain builder methods for request construction

## [cli-v0.1.4] - 2025-11-28

### 🚀 Features

- Bump versions
- Release cli-v0.1.4

### 🐛 Bug Fixes

- Clean-up types and make them more exhaustive
- Typo

### ⚙️ Miscellaneous Tasks

- *(cli)* Set default values to flags
- Enable retrieving a market by its slug

## [cli-v0.1.3] - 2025-11-28

### 🚀 Features

- Add cli commands presets and more flags

### 🐛 Bug Fixes

- Deserialize API responses into correct structs

### ⚙️ Miscellaneous Tasks

- Run `cargo fmt`

## [cli-v0.1.2] - 2025-11-27

### 🚀 Features

- *(cli)* Add command to display CLI version

### ⚙️ Miscellaneous Tasks

- Add more unit tests for utils

## [cli-v0.1.1] - 2025-11-27

### 🚀 Features

- Enable generating shell completions

## [cli-v0.1.0] - 2025-11-27

### 🚀 Features

- Add cli

### 📚 Documentation

- Add links to crates documentation
- Say it's wip in README

### ⚙️ Miscellaneous Tasks

- Make Polymarket client clonable
- Bump deps
- Bump `alloy` to latest and move it clob crate
- Add install script and workflow to release binaries on Github Releases
- Fix release workflow
