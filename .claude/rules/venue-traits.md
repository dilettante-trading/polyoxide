---
paths:
  - "polyoxide-venue/**"
  - "{polyoxide,polyoxide-binance,polyoxide-kalshi,polyoxide-polymarket}/src/lib.rs"
  - "**/src/**/venue.rs"
  - "**/src/**/venue/**"
  - "**/src/**/key.rs"
  - "polyoxide-polymarket/src/shared/keys.rs"
  - "**/tests/venue_*.rs"
  - "**/tests/trading*.rs"
  - "polyoxide/tests/{parse_key,venue_neutral,live_venue}.rs"
  - "polyoxide/examples/venue_neutral.rs"
---

# Market keys, records and the venue traits

`polyoxide-venue` depends on nothing by default and has no HTTP, socket or signing dependency;
its `decimal` feature, which perps, Binance and, through `market`, clob enable, adds rust_decimal
and serde for `positional`. Each venue's implementation of these traits is described in its own
rules file.

## Market keys (feature `keys`)

- `polyoxide_venue::MarketKey` names one instrument as `venue.product:id` and has no public
  constructor from text or from parts: only a product's `KeySpace` makes one, after its `parse_id`
  has checked and canonicalized the id, so `Eq` is canonical equality.
- Each venue crate implements `KeySpace` once per product, in one place:
  - Binance's `usdm::key`: `UsdmKeys`, over `Symbol` (uppercases ASCII); `usdm::key(&symbol)`,
    `key::parse`, `key::symbol`.
  - Polymarket's `shared::keys`: `ClobKeys`, over `TokenId` (1 to 78 decimal digits, no leading
    zero), built with any of `clob`, `gamma` and `data`, since gamma and data name the clob's
    outcomes; `PerpsKeys`, over `InstrumentId` (written without leading zeros), built with `perps`.
    A clob key names one outcome token, never a market.
  - Kalshi's `predictions::key`: `PredictionsKeys`, over `OutcomeId`, a `Ticker` (1 to 200
    characters of `A-Z`, `0-9`, `.` and `-`, ASCII letters uppercased) and an `Outcome` (`yes` or
    `no`, lowercased); `predictions::key(&ticker, outcome)`, `key::parse`, `key::outcome`. It names
    one outcome of one market, never the market.
- `RawKey::parse` splits text at its first `:`, then the part before it at its first `.`, so an id
  may hold either. `RawKey` round-trips through serde as text; `MarketKey` serializes as text only;
  `polyoxide::parse_key` hands a `RawKey` to the constructors of whichever enabled product it
  names. Every `KeyError` is an `InvalidRequest`.
- Each `KeySpace` declares its product's `ProductClass` as `CLASS` (ADR 0001): `EventContract`
  (the clob, Kalshi's `predictions`) or `PerpetualFuture` (perps, USDⓈ-M), `#[non_exhaustive]`, a
  variant added only when a product of that class lands. Every key carries it as
  `MarketKey::class()`; it follows from the venue and product, so the text form, `Eq` and `Hash`
  are unchanged.
- Each venue crate's `the_ids_are_the_ones_the_crate_registers` test holds its `VENUE`, product and
  class constants to its `[package.metadata.polyoxide]`, read with
  `polyoxide_test_support::registration!()`: `products`, and `classes`, a table of
  product = class for each product with a `KeySpace`, which `gen_registry.py` checks names only the
  crate's products.

## Records (feature `market`)

- `Instrument` is the one record that carries tick size, minimum size and `SizeUnit` (`Base`,
  `Contracts` or `Shares`). `Level` (a price and a size) and `EventOutcome` (an `EventGroup`'s
  outcome) are values inside records and carry no `Extensions`. `Side { Buy, Sell }`.
- A size is the instrument's native quantity; a quote-currency amount is a separate `notional`;
  a field a venue does not publish is an `Option`.
- Records derive `Clone`, `Debug` and `PartialEq`, are `#[non_exhaustive]` with a `new` that takes
  their required fields, and carry `Extensions`: at most one value per type, each
  `Clone + Debug + PartialEq + Send + Sync + 'static`, compared by value, with no serde, and
  `type_ids()` to list them. Inserting a type already held is a `DuplicateExtension`
  (`InvalidRequest`); `Extensions::of(value)` starts one with a single value.
- A product's own facts go in its one `<Record>Ext` per record type, never in a new field on the
  shared record. A class fact joins a shared record, a capability trait or a bundle only when two
  products of the class supply it.

## Market-data traits (feature `market`; AD-5, AD-6, amendment A6-1)

- `MarketData` (`instrument(key)`, `quote(key)`, `book(key)`) is implemented by every product.
  The capability traits `Instruments` (the whole listing, only where one request or a few return
  it), `Trades`, `Candles`, `Funding` and `EventGroups` are implemented only where offered; no
  method answers "unsupported".
- Each has `Send + Sync` supertraits, methods returning
  `impl Future<Output = Result<_, ClassifiedError>> + Send`, and a dynosaur wrapper
  (`DynMarketData` and so on), held as `Arc<Dyn…<'static>>`. An implementation writes `async fn`
  and converts its module's error with `?`, so the native clients keep their typed errors.
- A key of another product is a `KeyError` (`InvalidRequest`), sent nowhere. A key the venue does
  not list is a `VenueRefusal` by its class (`NotListed` where the venue's answer identifies the
  instrument as unlisted, otherwise the venue's own refusal), except where the venue cannot tell
  (perps), whose quote and book are then empty. Callers match on `class()` and confirm a listing
  with `instrument`.
- The records returned: `Quote`; `Book` (bids highest first, asks lowest first); `Trade` (its
  `TradeId` an opaque `Arc<str>`); `Candle` (a venue that samples one price per interval gives
  only `close`); `FundingRate` (the rate accruing has a `next_funding` unless none is scheduled, a
  settled one none; its `interval`, an `Option<Duration>`, is the length of the funding period its
  `rate` is for, and settled rows carry the instrument's current interval, since no venue keeps a
  history of it); `EventGroup` with its `EventOutcome`s. Every list a trait returns is oldest
  first, one request's worth.
- `CandleQuery` names a `CandleInterval` (one minute to one week) and a span.
  `Candles::intervals()` is a provided method returning the widths the venue serves, every one
  (`CandleInterval::ALL`) by default, which the CLOB, perps and USDⓈ-M keep and Kalshi overrides.
  A query whose width is outside it is refused as `InvalidRequest`, sent nowhere, through
  `CandleQuery::ensure_served`, whose `UnservedInterval` (`InvalidRequest`, a fault, no wait) an
  implementation converts with `?` as it does a `KeyError` (amendment A9-2).
- A live test unwraps a trait's `Result<_, ClassifiedError>` with test-support's
  `ClassifiedResultExt::or_fail` (or `fail_classified`), since `ClassifiedError` does not
  implement `Classify`; `ResultExt::or_fail` and it may be in scope together.

## Class bundles (ADR 0001)

`PerpetualMarket` is `MarketData + Funding`; `EventContractMarket` is `MarketData + EventGroups`.
Each has a blanket impl and no `Dyn` wrapper, so a class-generic function needs one bound, and
code that holds trait objects holds one per member. A bundle holds only what defines its class; a
capability other classes share, such as `Candles`, stays out, so a bundle never has to lose a
member. Each venue crate's `lib.rs` pins its products to their bundles in a `const _` beside its
`Classify` assertions: `Perps` (`perps`) and `Usdm` (`usdm`) to `PerpetualMarket`, `Clob`
(`all(clob, gamma)`) and `Predictions` (`predictions`) to `EventContractMarket`.
`polyoxide/tests/parse_key.rs` checks every product's class.

## The trading trait (feature `trading`, which turns on `market`; AD-3, AD-5, amendment A7-1)

- `Trading` is the one interface a venue with signed order routes implements, so a consumer
  trades the same way on each; `DynTrading` is held as `Arc<DynTrading<'static>>`. Its methods
  return `Result<_, ClassifiedError>`: `place(OrderRequest)` gives the `Order` as the venue
  accepted it; `cancel(&OrderId)`; `cancel_all(Option<&MarketKey>)` gives the ids cancelled;
  `open_orders(Option<&MarketKey>)`; `positions()`; `balances()`; `events()` is not async and
  returns a `TradingEvents`.
- The records are `#[non_exhaustive]` with a `new` that takes their required fields and a
  `pub extensions: Extensions`: `OrderRequest` (a `TimeInForce` defaulting to `GoodTilCancelled`,
  an optional `ClientOrderId`, `post_only` and `reduce_only`), `Order` (an `OrderStatus` of
  `Open`, `Filled`, `Cancelled` or `Killed { reason: KillReason }`), `Fill` (its `trade_id` is
  `market`'s `TradeId`, its `liquidity` a `Liquidity`), `Position` and `Balance`. `OrderId`,
  `FillId` and `ClientOrderId` are opaque `Arc<str>` ids.
- A request is refused, never ignored, as `InvalidRequest` with nothing sent when it carries an
  extension its venue does not declare, a key of another product, or a `post_only`, `reduce_only`
  or time in force the venue cannot honour. `Extensions::ensure_declared(&[TypeId])` (feature
  `market`) refuses the first undeclared value in `TypeId` order with an `UndeclaredExtension`
  (`InvalidRequest`, in the `Classify` assertion).
- An immediate-or-cancel order that matched nothing and a fill-or-kill order that could not fill
  are `Ok` with `Killed`, not errors.
- `TradingEvents` is a newtype over a pinned boxed stream, `Send + Unpin + 'static`, because
  dynosaur 0.3.1 drops `'static` from a returned `impl Stream` and the stream would borrow the
  `Arc` (E0597, A7-1). The queue behind it is unbounded, so a slow consumer loses no ack or fill,
  and the venue replays nothing.
- The stream's first item is `TradingEvent::Resync` (A7-2, once the fills connection's first
  attempt has ended, whether it went live or began in an outage), unless the stream cannot open,
  when its only item is that error. It yields `Resync` after every reconnect of that connection,
  and when the venue voids a fill it reported (a trade that failed after it matched), meaning
  "reconcile through `open_orders()` and the trade history: the stream cannot vouch for what it
  reported or missed". An order may be reported before the `place` that created it returns.
- `polyoxide-venue/tests/trading.rs` proves the trait on a made-up venue, statically and through
  `Arc<DynTrading<'static>>`, including a stream that outlives every `Arc` on another thread.
