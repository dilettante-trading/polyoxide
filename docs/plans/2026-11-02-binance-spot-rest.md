# Binance Spot REST Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** `polyoxide-binance` gains a `spot` module that reads Binance spot's public market data on `api.binance.com` through a measured weight budget, implements the market-data traits as `polyoxide-venue`'s first `ProductClass::Spot` product, and is re-exported by the umbrella under `binance-spot`.

**Architecture:** The Binance-wide plumbing `usdm` holds today (the minute-weight engine and its hold, the retry decision, the error decode, the routed request and `Symbol`) moves into a crate-private `shared` module, with `usdm`'s public API and every test unchanged. `polyoxide-venue` gains `ProductClass::Spot` and the `SpotMarket` bundle. The `spot` module is then built on `shared` in `usdm`'s shape: client, weight table, routes, types, errors, venue impls, keys, the live suite and the umbrella feature. Sockets are plan 2's, written after the WebSocket probe.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), tokio, rust_decimal, serde, thiserror, tracing, mockito, `polyoxide-core`'s send loop and `Throttle`, `polyoxide_venue::positional`, and Python capture and probe scripts.

**Spec:** `docs/superpowers/specs/2026-11-02-binance-spot-design.md`, read with slice 1's spec (`docs/superpowers/specs/2026-10-07-polyoxide-binance-design.md`), whose decisions stand unless this one replaces them, and `.claude/rules/binance.md`.

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `nix develop --command`. A story's gates are the profile's; its **Verify** line is the developer's done-check.
- **No test changes body** (AD-12). A test is added after a file's last test, or in a file of its own. Story 2 moves no test: `usdm`'s unit tests stay in their files, byte for byte, and every name they use stays in scope in their module, imports that only tests use going under `#[cfg(test)]` at module level, never inside the tests module.
- **`usdm`'s public API does not change.** Every public path, name, variant and signature in `polyoxide_binance::usdm` stays; a moved item stays reachable at its old path through `pub use`. prader-rs pins `usdm`'s `Interval`, `StreamPath` and `Cost` exhaustive.
- **The foundation crates name no venue** (`python3 scripts/venue_gate.py`). Story 3's docs and tests in `polyoxide-venue` never say "binance", "spot" as a venue, "usdm", "kalshi" or "polymarket"; they use made-up instruments.
- **Generated regions are regenerated, never edited**: `python3 scripts/gen_registry.py --write` after any `[package.metadata.polyoxide]` change. `docs/ARCHITECTURE.md` and `ci/dep-allowlists/*.txt` are never edited by hand.
- **README text that a doctest compiles**: a module README's `rust` fences compile under its feature, so every fence in `spot/README.md` builds with `spot` alone.
- **Fixtures are captured, never hand-written.** Story 1 captures them from production with no credentials; a later story that needs another one stops and asks.
- **A weight is measured, never copied** (`OBSERVED.md` holds the measurement; `Route::cost` cites it).
- **Live suites**: every test `#[ignore]`d; failures only through `polyoxide-test-support`'s tags (`or_fail`, `fail`, `environmental`, `transient`); no `.unwrap()`, `.expect(` or `panic!` (`python3 scripts/live_unwraps.py`).
- **A rule's text changes in the commit that changes the rule** (AD-21): a story that adds a hand-written setter adds its row to `.claude/rules/clients.md` in the same commit.
- **Nothing publishes.** No `cargo publish` without `--dry-run`, no `scripts/finish_release.sh`, no `gh workflow run Release`, no version bump.
- **Commit by pathspec**, conventional, with `Cargo.lock` when cargo changes it, never `--no-verify`; every message ends with `(binance spot, Story <n>)`.

## File structure

Paths are from the repo root.

**Story 1: REST probe and captures.**

- `docs/specs/binance/probes/probe_rest.py` (modify): a `--product` choice, `usdm` the default.
- `scripts/capture_binance_spot.py` (create): the `rest` capture.
- `polyoxide-binance/tests/fixtures/spot/rest/*.json` (create, captured): the files the names contract lists.
- `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md` (create).
- `docs/specs/binance/OBSERVED.md` (modify): the `## Spot` section, ending with its anchor.

**Story 2: shared REST plumbing.**

- `polyoxide-binance/src/lib.rs` (modify): `mod shared;`.
- `polyoxide-binance/src/shared/mod.rs`, `weight.rs`, `policy.rs`, `error.rs`, `request.rs`, `symbol.rs` (create).
- `polyoxide-binance/src/usdm/weight.rs`, `policy.rs`, `error.rs`, `request.rs`, `types.rs` (modify): onto `shared`, their tests untouched.
- `docs/MUTANTS.md` (modify): rows (a) and (k) at their new lines, re-proved.

**Story 3: the spot class.**

- `polyoxide-venue/src/key.rs` (modify): `ProductClass::Spot`.
- `polyoxide-venue/src/market.rs` (modify): `SpotMarket`.
- `polyoxide-venue/src/lib.rs` (modify): the re-export and the crate docs' bundle sentence.
- `polyoxide-venue/tests/spot_class.rs` (create).

**Story 4: the spot client.**

- `polyoxide-binance/Cargo.toml` (modify): the `spot` feature, `default`, description, keywords, docs.rs features, the `[[test]]` stanzas.
- `polyoxide-binance/src/lib.rs` (modify): `shared`'s `cfg`, `pub mod spot;`, the README doctest, the `Classify` assertion.
- `polyoxide-binance/src/spot/mod.rs`, `README.md`, `error.rs`, `policy.rs`, `request.rs`, `types.rs`, `weight.rs`, `api/mod.rs`, `api/health.rs`, `api/exchange.rs` (create).
- `polyoxide-binance/tests/mock_spot.rs`, `headers_spot.rs`, `error_classes_spot.rs`, `failure_tags_spot.rs`, `ban_log_spot.rs` (create).
- `.github/scripts/tests/test_dep_allowlists.py` (modify) and `ci/dep-allowlists/polyoxide-binance@spot.txt` (by `--write`).

**Story 5: spot market routes.**

- `polyoxide-binance/src/spot/api/market.rs` (create); `spot/api/mod.rs`, `spot/types.rs` (modify).
- `polyoxide-binance/Cargo.toml` (modify): two `[[test]]` stanzas.
- `polyoxide-binance/tests/mock_spot.rs` (modify, append only); `wire_agreement_spot.rs`, `query_setters_spot.rs` (create).
- `polyoxide-binance/examples/weight_probe.rs` (modify): the spot routes.
- `.claude/rules/clients.md` (modify): `spot::api::market::GetDepth::limit`'s row.
- `docs/MUTANTS.md` (modify): spot's depth re-pricing row.

**Story 6: spot venue, keys and the umbrella.**

- `polyoxide-binance/src/spot/key.rs`, `venue.rs` (create); `spot/mod.rs` (modify: the module block replaced whole).
- `polyoxide-binance/src/lib.rs` (modify): the bundle pin.
- `polyoxide-binance/Cargo.toml` (modify): registration and two `[[test]]` stanzas.
- `polyoxide-binance/tests/venue_spot.rs`, `live_spot.rs` (create).
- `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs` (modify); `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs` (modify, append only).
- `.github/scripts/tests/test_dep_allowlists.py` (modify) and `ci/dep-allowlists/polyoxide@binance-spot.txt` (by `--write`).
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 7: docs.**

- `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide/README.md`, `README.md` (modify, outside generated regions).
- `.claude/rules/binance.md`, `.claude/rules/umbrella.md` (modify).
- `Cargo.toml` (modify): `[workspace.metadata.polyoxide.mirrors.binance]`'s `name`, `base_urls`, `description` and `exclude`, which feed the generated spec index and the nightly-schema exclusion.
- `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md` (create).
- The files `gen_registry.py --write` rewrites.

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story quotes. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths under `src/` are `polyoxide-binance/src/` unless they name a crate.

**Story 1 → Stories 4 to 7.**

- **Fixtures**, under `polyoxide-binance/tests/fixtures/spot/rest/`, each from `https://api.binance.com` with no credentials, trimmed only in list length, every key kept:
  - `time.json`: `GET /api/v3/time`;
  - `exchange_info.json`: `GET /api/v3/exchangeInfo`, `symbols` trimmed to `BTCUSDT`, the first symbol whose `status` is not `TRADING`, the first symbol with a non-ASCII character if one is listed, and the first symbol whose `filters` hold a filter type none of those has, until every filter type listed anywhere is held;
  - `depth.json`: `GET /api/v3/depth?symbol=BTCUSDT&limit=5`;
  - `trades.json`: `GET /api/v3/trades?symbol=BTCUSDT&limit=3`;
  - `agg_trades.json`: `GET /api/v3/aggTrades?symbol=BTCUSDT&limit=3`;
  - `klines.json`: `GET /api/v3/klines?symbol=BTCUSDT&interval=1m&limit=2`;
  - `klines_1s.json`: `GET /api/v3/klines?symbol=BTCUSDT&interval=1s&limit=2`;
  - `avg_price.json`: `GET /api/v3/avgPrice?symbol=BTCUSDT`;
  - `ticker_24hr.json`: `GET /api/v3/ticker/24hr?symbol=BTCUSDT` (an object);
  - `tickers_24hr.json`: `GET /api/v3/ticker/24hr` (an array), trimmed to `BTCUSDT`, `ETHUSDT` and the non-ASCII symbol if listed;
  - `price.json` and `prices.json`: `GET /api/v3/ticker/price` with `symbol=BTCUSDT` (an object) and without (an array, trimmed to the same rows);
  - `book_ticker.json` and `book_tickers.json`: `GET /api/v3/ticker/bookTicker` likewise;
  - `error_invalid_symbol.json`: the body of `GET /api/v3/exchangeInfo?symbol=NOPEUSDT`, with `PROVENANCE.md` recording its status, `400`.
- **`polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`** has a `# Provenance` title and a `## REST (rest/)` section with one table row per file (file, request, trimmed to), as `fixtures/usdm/PROVENANCE.md` does; plan 2 appends `## Streams (ws/)`.
- **`scripts/capture_binance_spot.py rest`** writes them, and nothing else; plan 2 adds a `ws` subcommand.
- **`docs/specs/binance/probes/probe_rest.py --product {usdm,spot}`**, `usdm` the default, so its existing run is unchanged.
- **The `OBSERVED.md` anchor.** Story 1 appends a section headed `## Spot`, with `### REST weights`, `### Limits past a route's maximum`, `### The weight window` and `### One counter across hosts`, whose last line is, verbatim, `<!-- spot findings go above this line -->`. A later story, and plan 2, inserts its findings directly above that line under a `###` heading of its own.

**Story 2 → Stories 4 and 5.** In `polyoxide-binance`:

- `src/lib.rs` gains, after the module README doctests and before `#[cfg(feature = "usdm")] pub mod usdm;`:

      #[cfg(feature = "usdm")]
      mod shared;

  Story 4 replaces that `cfg` with `#[cfg(any(feature = "usdm", feature = "spot"))]`.
- `src/shared/mod.rs` declares `pub(crate) mod error; pub(crate) mod policy; pub(crate) mod request; pub(crate) mod symbol; pub(crate) mod weight;`.
- **`shared::weight`**:
  - `pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";`, `pub const DEFAULT_BAN: Duration = Duration::from_secs(120);`, `pub const MAX_COOLDOWN: Duration = Duration::from_secs(3 * 24 * 60 * 60);`, each with today's `usdm` doc text, naming no product.
  - `#[derive(Debug, Clone)] pub(crate) enum Clock { System, #[cfg(test)] Tokio { start: tokio::time::Instant, start_ms: u64 }, #[cfg(test)] Manual(std::sync::Arc<std::sync::atomic::AtomicU64>) }` with `pub(crate) fn now_ms(&self) -> u64`, moved from `usdm::weight`.
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub(crate) struct MinuteCharge { pub(crate) minute: Option<u64> }`.
  - `#[derive(Debug)] pub(crate) struct Engine`, with `pub(crate) fn new(published_per_minute: u32, paced: Option<(u32, Duration)>, clock: Clock) -> Self` (the minute budget is the published count less a tenth, today's `after_reserve` with `RESERVED_FRACTION = 10`; `paced` is a published count and its period, paced through `WindowQuotaTable::paced_interval` with a depth of one, `None` for a product with no paced layer), and `pub(crate) fn per_minute(&self) -> u32`, `pub(crate) fn used(&self) -> u32`, `pub(crate) async fn acquire_weight(&self, weight: u32) -> MinuteCharge`, `pub(crate) async fn acquire_paced(&self) -> MinuteCharge` (`minute: None`; on an engine with no paced layer it waits out the cooldown and returns), `pub(crate) fn record_used(&self, charge: MinuteCharge, used: u32)`, `pub(crate) fn begin_cooldown(&self, delay: Duration)`, `pub(crate) fn until_next_minute(&self) -> Duration`, `pub(crate) async fn await_cooldown(&self)`, `pub(crate) async fn charge(&self, meta: &RequestMeta<'_>, weight: LayerId, paced: Option<LayerId>) -> Charge` (today's `Throttle::acquire` body for those two layers), and `pub(crate) fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, weight: LayerId)` (today's `Throttle::observe` body). Behaviour is today's, line for line, with `usdm`'s two layers as parameters.
- **`usdm::weight` after Story 2**: `pub struct WeightBudget { inner: Arc<Engine> }`, derives and docs as today; `pub use crate::shared::weight::{DEFAULT_BAN, MAX_COOLDOWN};`; `PUBLISHED_WEIGHT_PER_MINUTE`, `PUBLISHED_FUNDING_PER_FIVE_MINUTES`, `WEIGHT_LAYER`, `FUNDING_LAYER`, `Cost`, `Route` and `const FUNDING_PERIOD` unchanged; `WeightBudget::new`, `with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self`, `per_minute`, `used`, `acquire(&self, cost: Cost) -> MinuteCharge`, `record_used`, `begin_cooldown`, `until_next_minute` with today's names and signatures, delegating to `Engine`; the `#[cfg(test)]` hooks `at_ms`, `is_shared_with` and `hold_until_next_minute` byte for byte; `impl Throttle for WeightBudget` through `Engine::charge(meta, WEIGHT_LAYER, Some(FUNDING_LAYER))` and `Engine::observe(.., WEIGHT_LAYER)`. In scope for its tests: `Arc`, `tokio::time::Instant`, `Clock`, `FUNDING_PERIOD`, `MAX_COOLDOWN`.
- **`shared::policy`**: `pub(crate) fn decide(response: &ResponseMeta<'_>, attempt: &AttemptInfo, schedule: &RetryConfig, until_next_minute: Duration) -> Decision`, today's `UsdmRetryPolicy::decide` with `self.budget.until_next_minute()` replaced by the argument. `usdm::policy::UsdmRetryPolicy { pub(crate) budget: WeightBudget }` stays, its `decide` calling `shared::policy::decide(response, attempt, schedule, self.budget.until_next_minute())`; its tests keep `MAX_COOLDOWN` and `ResponseMeta` in scope.
- **`shared::error`**:

      pub(crate) enum Decoded {
          Api(ApiError),
          Venue { status: u16, code: i64, msg: String },
          RateLimited { retry_after: Option<Duration> },
          IpBanned { retry_after: Option<Duration>, body: String },
          RegionBlocked { msg: String },
          Forbidden { msg: String },
      }

  with `pub(crate) fn decode(err: ApiError) -> Decoded` (today's `From<ApiError>` and `from_response`, by status first, `body` the unclipped `418` body), `pub(crate) fn retry_after_secs(value: Option<&str>) -> Option<Duration>`, `pub(crate) fn clip(text: &str) -> String` and `pub(crate) fn venue_class(status: u16, code: i64) -> Class` (`class_for_status(status).unwrap_or(Class::Decode).with_code(code.to_string())`). A product's `From<ApiError>` maps `Decoded` one to one onto its enum, logging `tracing::warn!("418: IP banned: {}", truncate_for_log(&body))` itself, so the WARN's target stays `polyoxide_binance::<module>::error`. `UsdmError`'s variants, docs, `code()` and `Classify` stay; its tests keep `Arc`, `ErrorResponse`, `MAX_COOLDOWN` and `retry_after_secs` in scope.
- **`shared::request`**:

      pub(crate) trait Priced: Copy {
          type Cost: Copy + Into<polyoxide_core::Cost>;
          fn path(self) -> &'static str;
          fn cost(self) -> Self::Cost;
      }

  `pub(crate) struct Routed<T, R, E>` with `pub(crate) fn new(http: &HttpClient, route: R) -> Self`, `pub(crate) fn query(self, key: &'static str, value: impl ToString) -> Self`, `pub(crate) fn route(self, route: R) -> Self`, `pub(crate) fn cost(&self) -> R::Cost`, and, for `T: DeserializeOwned` and `E: polyoxide_core::RequestError`, `pub(crate) async fn send(self) -> Result<T, E>`; and the macro `route_builder!($(#[$doc:meta])* $name:ident => $output:ty; module = $module:ident, error = $error:ident)`, exported with `pub(crate) use route_builder;`, generating today's builder with `request: $crate::$module::request::Routed<$output>`, `cost(&self) -> $crate::$module::weight::Cost` and `send(self) -> Result<$output, $crate::$module::$error>`, and today's doc text. `usdm::request` keeps `pub(crate) type Routed<T> = crate::shared::request::Routed<T, Route, UsdmError>;`, a `route_builder!` of today's form forwarding with `module = usdm, error = UsdmError`, and `pub use crate::shared::weight::USED_WEIGHT_HEADER;`; `usdm::weight::Route` implements `Priced` through its inherent `path` and `cost`. `usdm/api/*.rs` do not change.
- **`shared::symbol`**: `pub struct Symbol` and `pub struct InvalidSymbol(pub String)` with every impl they have today (`new`, `MAX_LEN`, `Display`, `FromStr`, `AsRef<str>`, `Classify` and the rest), docs naming no single host. `usdm::types` has `pub use crate::shared::symbol::{InvalidSymbol, Symbol};`.
- **`docs/MUTANTS.md`**: rows (a), "Binance's policy", and (k) name `polyoxide-binance/src/shared/policy.rs:<line>` (the arms in `shared::policy::decide`) and the new line of `a_418_without_retry_after_holds_two_minutes` in `src/usdm/policy.rs`; each re-proved.

**Story 3 → Story 6.** In `polyoxide-venue`:

- `ProductClass::Spot`, after `PerpetualFuture`, `as_str()` `"spot"`, its doc: buying and selling the asset itself; a price is unbounded, a position is a balance of the base asset and so never negative, and there is no funding.
- `pub trait SpotMarket: MarketData {}` and `impl<T: MarketData> SpotMarket for T {}`, feature `market`, after `EventContractMarket` in `market.rs`, re-exported at the crate root beside `PerpetualMarket` and `EventContractMarket`.

**Story 4 → Stories 5, 6 and 7.** In `polyoxide-binance`:

- **Features**: `default = ["usdm", "spot"]`; `spot` lists exactly what `usdm` lists today (`dep:polyoxide-core`, `polyoxide-venue/decimal`, `polyoxide-venue/market`, `dep:rust_decimal`, `dep:serde`, `dep:serde_json`, `dep:thiserror`, `dep:tokio`, `dep:tracing`, `dep:url`); `[package.metadata.docs.rs] features = ["usdm", "usdm-ws", "spot"]`.
- **The module block of `src/spot/mod.rs`** that Story 4 writes, after the module docs:

      pub mod api;
      pub mod error;
      mod policy;
      pub mod request;
      pub mod types;
      pub mod weight;

      pub use error::SpotError;
      pub use weight::WeightBudget;

  Story 6 replaces it whole with the same block plus `pub mod key;` directly after `pub mod error;`, `pub mod venue;` directly after `pub mod types;`, and `pub use key::key;` directly after `pub use error::SpotError;`.
- `pub const DEFAULT_BASE_URL: &str = "https://api.binance.com";`, `pub const DEFAULT_MAX_CONCURRENT: usize = 4;`.
- `#[derive(Debug, Clone)] pub struct Spot`: `pub fn new() -> Result<Self, SpotError>`, `pub fn builder() -> SpotBuilder`, namespaces `health() -> Health`, `exchange() -> ExchangeApi`, `market() -> MarketApi` (Story 5's), `pub fn weight_budget(&self) -> &WeightBudget`. `#[derive(Debug)] pub struct SpotBuilder` with `client_config_setters!`, `pub fn weight_budget(self, budget: WeightBudget) -> Self`, `pub fn build(self) -> Result<Spot, SpotError>`. Story 4 declares `market` in `namespaces!` only once Story 5 adds it; Story 4's `api/mod.rs` declares `pub mod exchange; pub mod health;`, and Story 5 adds `pub mod market;`.
- **`spot::weight`**: `pub const PUBLISHED_WEIGHT_PER_MINUTE: u32 = 6000;`, `pub const WEIGHT_LAYER: LayerId = LayerId("binance-spot-weight");`, `pub use crate::shared::weight::{DEFAULT_BAN, MAX_COOLDOWN};`, `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Cost { Weight(u32) }` with `From<Cost> for polyoxide_core::Cost` (exact), and

      #[derive(Debug, Clone, Copy, PartialEq, Eq)]
      #[non_exhaustive]
      pub enum Route {
          Ping,
          Time,
          ExchangeInfo,
          Depth { limit: Option<u32> },
          Trades,
          AggTrades,
          Klines,
          AvgPrice,
          Ticker24h { all: bool },
          Price { all: bool },
          BookTicker { all: bool },
      }

  with `pub fn path(self) -> &'static str` (`/api/v3/…`), `pub fn cost(self) -> Cost` (spec §3.1: `Depth` 5 for `None` and 1 to 100, 25 to 500, 50 to 1000, 250 above; `ExchangeInfo` 20; `Trades` 25; `AggTrades` 4; `Klines`, `AvgPrice` and the one-symbol tickers 2; `Ticker24h { all: true }` 80; `Price` and `BookTicker` with `all` 4; `Ping` and `Time` 1), and `Priced`. `#[derive(Debug, Clone)] pub struct WeightBudget` (and `Default`) with `pub fn new() -> Self`, `pub fn per_minute(&self) -> u32` (5400), `pub fn used(&self) -> u32`, and `Throttle` through `Engine::charge(meta, WEIGHT_LAYER, None)`.
- **`spot::request`**: `pub use crate::shared::weight::USED_WEIGHT_HEADER;`, `pub(crate) type Routed<T> = crate::shared::request::Routed<T, Route, SpotError>;`, and `route_builder!` forwarding with `module = spot, error = SpotError`.
- **`spot::error::SpotError`**, `#[derive(Error, Debug)] #[non_exhaustive]`: `Api(ApiError)`, `Venue { status: u16, code: i64, msg: String }`, `RateLimited { retry_after: Option<Duration> }`, `IpBanned { retry_after: Option<Duration> }`, `RegionBlocked { msg: String }`, `Forbidden { msg: String }`, with `pub fn code(&self) -> Option<i64>` and `Classify` as `UsdmError`'s; listed in `lib.rs`'s assertion with `spot::types::InvalidSymbol`.
- **`spot::policy::SpotRetryPolicy { pub(crate) budget: WeightBudget }`** (crate-private), deciding through `shared::policy::decide`.
- **`spot::api::health::Health`**: `pub async fn ping(&self) -> Result<Duration, SpotError>`, `pub fn time(&self) -> GetTime` (`send() -> Result<ServerTime, SpotError>`).
- **`spot::api::exchange::ExchangeApi`**: `pub fn exchange_info(&self) -> GetExchangeInfo`, with `symbol: &Symbol => "symbol"` from `query_setters!`; `send() -> Result<ExchangeInfo, SpotError>`.
- **`spot::types`** (Story 4's part): `pub use crate::shared::symbol::{InvalidSymbol, Symbol};`; `ServerTime { pub server_time: u64 }`; `ExchangeInfo` with `pub timezone: String`, `pub server_time: u64`, `pub rate_limits: Vec<RateLimit>`, `pub exchange_filters: Vec<Filter>`, `pub symbols: Vec<SymbolInfo>` and every other key the fixture carries; `RateLimit`; `SymbolInfo` with at least `pub symbol: String`, `pub status: SymbolStatus`, `pub base_asset: String`, `pub base_asset_precision: u32`, `pub quote_asset: String`, `pub quote_asset_precision: u32`, `pub order_types: Vec<String>`, `pub is_spot_trading_allowed: bool`, `pub is_margin_trading_allowed: bool`, `pub filters: Vec<Filter>`, `pub permission_sets: Vec<Vec<String>>`, and every other key the fixture carries; `SymbolStatus`, an `open_enum!` with `Trading => "TRADING"`, `Break => "BREAK"`, `Halt => "HALT"`, `PreTrading => "PRE_TRADING"`, `PostTrading => "POST_TRADING"`, `EndOfDay => "END_OF_DAY"`, `AuctionMatch => "AUCTION_MATCH"`; `Filter`, open, keeping an unknown type with its object, with at least `PriceFilter { min_price, max_price, tick_size }`, `LotSize { min_qty, max_qty, step_size }`, `MarketLotSize { min_qty, max_qty, step_size }` and `Notional { min_notional, apply_min_to_market, max_notional, apply_max_to_market, avg_price_mins }`, every `Decimal` field from a decimal string, and a variant for each other type in the fixture. Every struct is `#[non_exhaustive]` and `Serialize` + `Deserialize`.
- **Test files**: `tests/mock_spot.rs` with a helper `fn fixture(name: &str) -> String` reading `tests/fixtures/spot/rest/<name>`; Story 5 appends after its last test. Each `[[test]]` stanza has `required-features = ["spot"]`.

**Story 5 → Stories 6 and 7.**

- **`spot::api::market::MarketApi`**: `depth(&self, symbol: &Symbol) -> GetDepth`, `trades(&self, symbol: &Symbol) -> GetTrades`, `agg_trades(&self, symbol: &Symbol) -> GetAggTrades`, `klines(&self, symbol: &Symbol, interval: Interval) -> GetKlines`, `avg_price(&self, symbol: &Symbol) -> GetAvgPrice`, `ticker_24h(&self, symbol: &Symbol) -> GetTicker24h`, `tickers_24h(&self) -> GetTickers24h`, `price(&self, symbol: &Symbol) -> GetPrice`, `prices(&self) -> GetPrices`, `book_ticker(&self, symbol: &Symbol) -> GetBookTicker`, `book_tickers(&self) -> GetBookTickers`; each builder has `pub fn cost(&self) -> Cost` and `pub async fn send(self) -> Result<_, SpotError>`, returning `Depth`, `Vec<Trade>`, `Vec<AggTrade>`, `Vec<Kline>`, `AvgPrice`, `Ticker24h`, `Vec<Ticker24h>`, `PriceTicker`, `Vec<PriceTicker>`, `BookTicker`, `Vec<BookTicker>`.
- **Setters**: `GetDepth::limit(self, limit: u32) -> Self`, hand-written, replacing the route with `Route::Depth { limit: Some(limit) }`; from `query_setters!`: `GetTrades::limit: u32`; `GetAggTrades::{from_id: u64 => "fromId", start_time: u64 => "startTime", end_time: u64 => "endTime", limit: u32}`; `GetKlines::{start_time: u64, end_time: u64, limit: u32}` (no time-zone setter).
- **`spot::types`** (Story 5's part): `Interval`, exhaustive, from `wire_enum!`: `Second1 => "1s"`, `Minute1 => "1m"`, `Minute3 => "3m"`, `Minute5 => "5m"`, `Minute15 => "15m"`, `Minute30 => "30m"`, `Hour1 => "1h"`, `Hour2 => "2h"`, `Hour4 => "4h"`, `Hour6 => "6h"`, `Hour8 => "8h"`, `Hour12 => "12h"`, `Day1 => "1d"`, `Day3 => "3d"`, `Week1 => "1w"`, `Month1 => "1M"`; `Depth { pub last_update_id: u64, pub bids: Vec<Level>, pub asks: Vec<Level> }`; `Level { pub price: Decimal, pub quantity: Decimal }` with `Level::new`, positional; `Trade { pub id: u64, pub price: Decimal, pub qty: Decimal, pub quote_qty: Decimal, pub time: u64, pub is_buyer_maker: bool, pub is_best_match: bool }`; `AggTrade { pub id: u64, pub price: Decimal, pub quantity: Decimal, pub first_trade_id: u64, pub last_trade_id: u64, pub time: u64, pub is_buyer_maker: bool, pub is_best_match: bool }` (wire `a`, `p`, `q`, `f`, `l`, `T`, `m`, `M`); `Kline` with `usdm::types::Kline`'s fields, positional, extra elements tolerated; `AvgPrice { pub mins: u32, pub price: Decimal, pub close_time: u64 }`; `Ticker24h`, `PriceTicker { pub symbol: String, pub price: Decimal }`, `BookTicker { pub symbol: String, pub bid_price: Decimal, pub bid_qty: Decimal, pub ask_price: Decimal, pub ask_qty: Decimal }`; each with every key its fixture carries, `#[non_exhaustive]`.
- **`.claude/rules/clients.md`** gains `spot::api::market::GetDepth::limit` beside `usdm`'s row, for the same reason.

**Story 6 → Story 7.**

- **`spot::key`**: `pub const VENUE: VenueId = VenueId::from_static("binance");`, `pub const PRODUCT: ProductId = ProductId::from_static("spot");`, `pub struct SpotKeys;` implementing `KeySpace` with `const CLASS: ProductClass = ProductClass::Spot`, `pub fn key(symbol: &Symbol) -> MarketKey`, `pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError>`, `pub fn symbol(key: &MarketKey) -> Result<Symbol, KeyError>`, as `usdm::key`'s.
- **`spot::venue`**: `impl MarketData`, `Instruments`, `Trades`, `Candles` for `Spot` (spec §5.8); `InstrumentExt { pub status: Option<SymbolStatus>, pub permission_sets: Option<Vec<Vec<String>>>, pub order_types: Option<Vec<String>>, pub is_spot_trading_allowed: Option<bool>, pub is_margin_trading_allowed: Option<bool>, pub filters: Option<Vec<Filter>> }`, `BookExt { pub last_update_id: Option<u64> }`, `TradeExt { pub first_trade_id: Option<u64>, pub last_trade_id: Option<u64>, pub is_best_match: Option<bool> }`, `CandleExt { pub close_time: Option<UnixMillis>, pub taker_buy_volume: Option<Decimal>, pub taker_buy_notional: Option<Decimal> }`, each `#[derive(Debug, Clone, PartialEq, Eq, Default)] #[non_exhaustive]` (`Copy` where every field is).
- **Registration** in `polyoxide-binance/Cargo.toml`: `products = ["usdm", "spot"]`, `classes = { usdm = "perpetual-future", spot = "spot" }`, `identifiers = ["fapi", "fstream", "X-MBX", "api/v3"]`, the `readme` line `Client library for Binance's APIs, one feature per module: \`usdm\` (USDⓈ-M futures market data, with its streams under \`usdm-ws\`) and \`spot\` (spot market data); re-exported by the umbrella as \`polyoxide::binance\``, and `[package.metadata.polyoxide.live.live_spot]` with `suite = "spot"`, `timeout = 15`, `features = ["spot"]`, `secrets = []`.
- **Umbrella**: features `binance-spot = ["dep:polyoxide-binance", "polyoxide-binance/spot", "polyoxide-venue/market"]` and `binance = ["binance-usdm", "binance-spot"]`; `full` gains `binance-spot`; `polyoxide::binance`'s `cfg` gains `feature = "binance-spot"`; `parse_key` reads `binance.spot` through `binance::spot::key::parse`; the prelude gains, under `binance-spot`, `pub use polyoxide_binance::spot::{Spot, SpotBuilder, SpotError, WeightBudget as SpotWeightBudget};`; the passthrough check gains `#[cfg(feature = "binance-spot")] const _: fn(crate::binance::spot::Spot) = drop;`. `live_venue`'s registered features gain `binance-spot`.

**Who edits the shared docs.** Story 6 changes `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md` and the two nightly workflows only through `python3 scripts/gen_registry.py --write`, inside generated regions; every hand-written line of those files is Story 7's. `polyoxide-binance/README.md` and `src/lib.rs`'s crate docs are Story 7's, but for `lib.rs`'s code lines, which Stories 2, 4 and 6 write. `docs/specs/binance/OBSERVED.md` is Story 1's, and a later story adds only above its anchor.

## Stories

- **Story 1** — REST probe and captures (spec §3.1, §5.13, §10).

  Gives `probe_rest.py` a `--product` choice and runs it on spot: every row of §3.1's table from `X-MBX-USED-WEIGHT-1M` deltas over distinct URLs, the limits past each route's maximum, the minute window across a boundary, and whether `api.binance.com` and `data-api.binance.vision` share one counter (alternating calls, each read against the other's last value). Writes `scripts/capture_binance_spot.py rest`, runs it, and writes `PROVENANCE.md` and `OBSERVED.md`'s `## Spot` section ending with its anchor. No Rust. A measured weight that differs from §3.1, or a counter that is not shared, goes into `OBSERVED.md` and to the owner before Story 4.

  - **Developer:** intermediate-dev
  - **Files:** `docs/specs/binance/probes/probe_rest.py`, `scripts/capture_binance_spot.py`, `polyoxide-binance/tests/fixtures/spot/rest/time.json`, `polyoxide-binance/tests/fixtures/spot/rest/exchange_info.json`, `polyoxide-binance/tests/fixtures/spot/rest/depth.json`, `polyoxide-binance/tests/fixtures/spot/rest/trades.json`, `polyoxide-binance/tests/fixtures/spot/rest/agg_trades.json`, `polyoxide-binance/tests/fixtures/spot/rest/klines.json`, `polyoxide-binance/tests/fixtures/spot/rest/klines_1s.json`, `polyoxide-binance/tests/fixtures/spot/rest/avg_price.json`, `polyoxide-binance/tests/fixtures/spot/rest/ticker_24hr.json`, `polyoxide-binance/tests/fixtures/spot/rest/tickers_24hr.json`, `polyoxide-binance/tests/fixtures/spot/rest/price.json`, `polyoxide-binance/tests/fixtures/spot/rest/prices.json`, `polyoxide-binance/tests/fixtures/spot/rest/book_ticker.json`, `polyoxide-binance/tests/fixtures/spot/rest/book_tickers.json`, `polyoxide-binance/tests/fixtures/spot/rest/error_invalid_symbol.json`, `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`, `docs/specs/binance/OBSERVED.md`
  - **Interfaces:** exposes the fixtures, `PROVENANCE.md`, the capture and probe commands and the `OBSERVED.md` anchor under "Story 1 → Stories 4 to 7"; uses nothing from earlier stories
  - **Verify:** `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-binance/tests/fixtures/spot/rest/*.json')]" && grep -qx '<!-- spot findings go above this line -->' docs/specs/binance/OBSERVED.md && python3 docs/specs/binance/probes/probe_rest.py --help && python3 scripts/capture_binance_spot.py --help`
  - **Review:** light (scripts, captures and records; no library code)
  - **Reading:** spec §3.1, §5.13, §10; `docs/specs/binance/probes/probe_rest.py`, `scripts/capture_binance_usdm.py`, `polyoxide-binance/tests/fixtures/usdm/PROVENANCE.md`, `docs/specs/binance/OBSERVED.md` (its headings and first 40 lines)

- **Story 2** — Shared REST plumbing under `usdm` (spec §5.1, §5.2).

  Moves the minute-weight engine, the retry decision, the error decode, the routed request and its macro, and `Symbol` into `src/shared/`, and rebuilds `usdm`'s weight budget, policy, error, request and types on them. Behaviour is unchanged line for line; `usdm`'s public paths stay through `pub use`; no test file and no test module changes, and every `#[cfg(test)]` hook keeps its name, signature and body. The moved code holds the landmine marker `RESERVED_FRACTION` (the reserved tenth, which must stay 10) and the `Retry-After` floor and `Hold` extension the profile's notes describe: moved, never changed. Rows (a) and (k) of `docs/MUTANTS.md` move to `shared/policy.rs` and are re-proved by applying each mutation and seeing its named tests fail.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/weight.rs`, `polyoxide-binance/src/shared/policy.rs`, `polyoxide-binance/src/shared/error.rs`, `polyoxide-binance/src/shared/request.rs`, `polyoxide-binance/src/shared/symbol.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/src/usdm/policy.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/types.rs`, `docs/MUTANTS.md`
  - **Interfaces:** exposes "Story 2 → Stories 4 and 5"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'`
  - **Review:** deep (a refactor under a public API and two mutation-tested rows; `RESERVED_FRACTION`)
  - **Reading:** spec §2 (the moved-tests, shared-types and `Symbol` rows), §5.1, §5.2; `polyoxide-binance/src/usdm/weight.rs:1-505`, `polyoxide-binance/src/usdm/policy.rs:1-90`, `polyoxide-binance/src/usdm/error.rs:1-230`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/types.rs:1-190`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/lib.rs`; `docs/MUTANTS.md` (rows (a) and (k), and the paragraphs on re-proving); `scripts/test_body_diff.py:1-90` (what counts as a test-context item)

- **Story 3** — `ProductClass::Spot` and the `SpotMarket` bundle (spec §5.3).

  Adds the class with its `as_str` and doc, the bundle with its blanket impl and docs in the shape of `PerpetualMarket`'s, the root re-export, and the crate docs' sentence naming the bundles. The test file proves `as_str`, that a key a made-up spot `KeySpace` makes carries the class, and that a type with `MarketData` alone satisfies a `SpotMarket` bound. The test file opens with `#![cfg(all(feature = "keys", feature = "market"))]`, as the crate's other test files gate on their features. The docs and tests name no venue.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-venue/src/key.rs`, `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/tests/spot_class.rs`
  - **Interfaces:** exposes "Story 3 → Story 6"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-venue --all-features && cargo test -p polyoxide-venue --features keys,market --test spot_class && python3 scripts/venue_gate.py`
  - **Review:** deep (a foundation crate's public enum and trait)
  - **Reading:** spec §2 (the product-class row), §5.3; `polyoxide-venue/src/key.rs:85-130`, `polyoxide-venue/src/market.rs:170-215`, `polyoxide-venue/src/lib.rs:1-110`; `docs/plans/2026-10-21-adr-0001-product-classes.md` (Task 3)

- **Story 4** — The spot client: feature, builder, weight budget, policy, errors, health and exchange routes (spec §4, §5.4 to §5.7, §5.10).

  Adds the `spot` feature (and `default = ["usdm", "spot"]`), widens `shared`'s `cfg`, and writes `spot/mod.rs`, `weight.rs` with the whole route table and `documented_weights` unit test, `policy.rs`, `error.rs`, `request.rs`, `types.rs`'s exchange part, `api/health.rs`, `api/exchange.rs` and `README.md` (a doctest under `spot`), with the builder's in-flight note (spec §4). The suites cover both routes on a mock server, the exact `Cost` each request carries, a `429` retried and holding the budget, a `418` failing and holding it, the `418`'s WARN under `polyoxide_binance::spot::error`, every error's class, the request headers, and the failure tags. `cargo hack --each-feature` builds `spot` without `usdm`. The dep allowlists gain `polyoxide-binance@spot.txt`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/spot/error.rs`, `polyoxide-binance/src/spot/policy.rs`, `polyoxide-binance/src/spot/request.rs`, `polyoxide-binance/src/spot/types.rs`, `polyoxide-binance/src/spot/weight.rs`, `polyoxide-binance/src/spot/api/mod.rs`, `polyoxide-binance/src/spot/api/health.rs`, `polyoxide-binance/src/spot/api/exchange.rs`, `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/headers_spot.rs`, `polyoxide-binance/tests/error_classes_spot.rs`, `polyoxide-binance/tests/failure_tags_spot.rs`, `polyoxide-binance/tests/ban_log_spot.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide-binance@spot.txt`, `Cargo.lock`
  - **Interfaces:** exposes "Story 4 → Stories 5, 6 and 7"; uses Story 1's `time.json`, `exchange_info.json` and `error_invalid_symbol.json`, and Story 2's `shared` items
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_module_readmes.py -q'`
  - **Review:** deep (a new public client and its rate budget)
  - **Reading:** spec §3.1, §4, §5.4 to §5.7, §5.10; `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/usdm/api/exchange.rs`, `polyoxide-binance/src/usdm/types.rs:190-600`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/mock_usdm.rs:1-120`, `polyoxide-binance/tests/headers_usdm.rs`, `polyoxide-binance/tests/error_classes_usdm.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/ban_log_usdm.rs`, `.github/scripts/tests/test_dep_allowlists.py:100-160`; `docs/specs/binance/OBSERVED.md` (the `## Spot` section)

- **Story 5** — Spot market routes (spec §3.1, §4, §5.6).

  Adds `api/market.rs` with the eleven builders, their types and `Interval`, the hand-written `GetDepth::limit` that re-prices the route by its band, and `market` in `Spot`'s namespaces. The suites cover each route's path, query and `Cost` on a mock server (appended to `mock_spot.rs`), every setter's exact query pair, and every fixture decoding with no unmodelled key and serializing back. `weight_probe` gains the spot routes behind a `--product` argument; `clients.md` gains the setter's row; `MUTANTS.md` gains spot's depth re-pricing row (the mutation: `GetDepth::limit` leaving the route at `Depth { limit: None }`), proved.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/spot/api/market.rs`, `polyoxide-binance/src/spot/api/mod.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/types.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/wire_agreement_spot.rs`, `polyoxide-binance/tests/query_setters_spot.rs`, `polyoxide-binance/examples/weight_probe.rs`, `.claude/rules/clients.md`, `docs/MUTANTS.md`
  - **Interfaces:** exposes "Story 5 → Stories 6 and 7"; uses Story 1's fixtures and Story 4's client, `Route`, `Cost`, `SpotError` and the `mock_spot.rs` helper
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot && cargo test -p polyoxide-binance --all-features --example weight_probe && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps`
  - **Review:** deep (the weight table every request is charged by)
  - **Reading:** spec §3.1, §4, §5.6; `polyoxide-binance/src/usdm/api/market.rs`, `polyoxide-binance/src/usdm/types.rs:600-920`, `polyoxide-binance/tests/wire_agreement_usdm.rs`, `polyoxide-binance/tests/query_setters_usdm.rs`, `polyoxide-binance/examples/weight_probe.rs`, `.claude/rules/clients.md`; `docs/MUTANTS.md` (one row's format)

- **Story 6** — `spot::venue`, keys, registration, `live_spot` and the umbrella's `binance-spot` (spec §5.8, §5.10, §5.11).

  Adds `spot::key` with its registration unit test (as `usdm::key`'s), `spot::venue` with the four trait impls, their extensions and the skipped-row WARN, and pins `Spot` to `SpotMarket` in `lib.rs`. Registers `spot` in the crate's metadata with its class, identifier and live entry, then runs `gen_registry.py --write`. `venue_spot` holds each method to the native client on the fixtures and the skipped row's WARN; `live_spot` reads every route and every venue method against production, with `live_responses_carry_no_unmodelled_keys` and a test that fails when `instruments` skips a row. The umbrella gains `binance-spot`, `parse_key`'s branch, the prelude names and the passthrough check; new tests in `parse_key.rs` read `binance.spot:btcusdt` and check its class, and `live_venue` gains a spot case. The dep allowlists gain `polyoxide@binance-spot.txt`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/spot/key.rs`, `polyoxide-binance/src/spot/venue.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/venue_spot.rs`, `polyoxide-binance/tests/live_spot.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide@binance-spot.txt`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`, `Cargo.lock`
  - **Interfaces:** exposes "Story 6 → Story 7"; uses Story 3's `ProductClass::Spot` and `SpotMarket`, and Stories 4 and 5's client, routes and types
  - **Verify:** `cargo test -p polyoxide-binance --all-features && cargo test -p polyoxide --all-features && cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests -q'`
  - **Review:** deep (the venue mapping every trait consumer reads, and the umbrella's public surface)
  - **Reading:** spec §5.8, §5.10, §5.11; `polyoxide-binance/src/usdm/key.rs`, `polyoxide-binance/src/usdm/venue.rs`, `polyoxide-binance/tests/venue_usdm.rs:1-140`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-binance/Cargo.toml` (the `[package.metadata.polyoxide]` tables), `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs:100-300`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`

- **Story 7** — Docs (spec §5.13).

  Adds spot to `docs/specs/binance/INDEX.md` (a section with the routes table and the drift detectors), the crate README and `src/lib.rs`'s crate docs (both modules), the umbrella README and the root README's hand-written lines, `.claude/rules/binance.md` (a `shared` section, a `spot` section, and `scripts/capture_binance_spot.py` and `polyoxide-binance/src/spot/**` covered by its `paths:`), `.claude/rules/umbrella.md` (the `binance-spot` feature and prelude names); updates `[workspace.metadata.polyoxide.mirrors.binance]` in the root manifest (`name` `Binance USDⓈ-M and spot`, `base_urls` gaining `https://api.binance.com`, `description` naming spot, and `exclude` saying Binance publishes no spec for USDⓈ-M and that spot's `spot_api.yaml` is stale since 2024-10-08 and lacks fields the wire sends, so the live suites are the drift check), then runs `gen_registry.py --write`; and writes the spine amendment recording `ProductClass::Spot` and `SpotMarket`.

  - **Developer:** junior-dev
  - **Files:** `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `README.md`, `.claude/rules/binance.md`, `.claude/rules/umbrella.md`, `Cargo.toml`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md`, `CLAUDE.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-schema.yml`
  - **Interfaces:** uses every earlier story's names; exposes none
  - **Verify:** `cargo test --doc -p polyoxide-binance --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q' && bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide --all-features'`
  - **Review:** light (docs and records; no library code)
  - **Reading:** spec §1, §5.13, §9; `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `.claude/rules/binance.md`, `.claude/rules/umbrella.md`, `Cargo.toml:265-271`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/adr-0001.md`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->
