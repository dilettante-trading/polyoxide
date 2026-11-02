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
- `.github/scripts/tests/test_mutants_ledger.py` (modify): `SNIPPETS`' entries for the moved citations.
- `.claude/rules/rate-limits.md`, `send-loop.md`, `clients.md` (modify): their `paths:` globs gain the `shared` files the moved code lands in.

**Story 3: the spot class.**

- `polyoxide-venue/src/key.rs` (modify): `ProductClass::Spot`.
- `polyoxide-venue/src/market.rs` (modify): `SpotMarket`.
- `polyoxide-venue/src/lib.rs` (modify): the re-export and the crate docs' bundle sentence.
- `polyoxide-venue/tests/spot_class.rs` (create).
- `.claude/rules/venue-traits.md` (modify): `Spot` in the class list and `SpotMarket` in the bundles paragraph.
- `polyoxide-venue/README.md` (modify): the classes sentence names spot, and the bundles sentence names `SpotMarket`.

**Story 4: the spot client.**

- `polyoxide-binance/Cargo.toml` (modify): the `spot` feature, `default`, description, keywords, docs.rs features, the `[[test]]` stanzas.
- `polyoxide-binance/src/lib.rs` (modify): `shared`'s `cfg`, `pub mod spot;`, the README doctest, the `Classify` assertion.
- `polyoxide-binance/src/spot/mod.rs`, `README.md`, `error.rs`, `policy.rs`, `request.rs`, `types.rs`, `weight.rs`, `api/mod.rs`, `api/health.rs`, `api/exchange.rs` (create).
- `polyoxide-binance/tests/mock_spot.rs`, `headers_spot.rs`, `error_classes_spot.rs`, `failure_tags_spot.rs`, `ban_log_spot.rs` (create).
- `polyoxide-binance/src/shared/weight.rs` (modify): `allow(dead_code)` under `not(feature = "usdm")` on the items only `usdm` uses.
- `.claude/rules/ci-gates.md` (modify): the `polyoxide-binance` allowlist files.
- `.claude/rules/rate-limits.md`, `send-loop.md`, `clients.md` (modify): their `paths:` globs gain spot's counterparts of the `usdm` files they name.
- `.github/scripts/tests/test_dep_allowlists.py` (modify) and `ci/dep-allowlists/polyoxide-binance@spot.txt` (by `--write`).

**Story 5: spot market routes.**

- `polyoxide-binance/src/spot/api/market.rs` (create); `spot/api/mod.rs`, `spot/types.rs` (modify).
- `polyoxide-binance/Cargo.toml` (modify): two `[[test]]` stanzas and the `weight_probe_spot` `[[example]]` stanza.
- `polyoxide-binance/tests/mock_spot.rs` (modify, append only); `wire_agreement_spot.rs`, `query_setters_spot.rs` (create).
- `polyoxide-binance/examples/weight_probe_spot.rs` (create): the spot routes; `examples/weight_probe.rs` is not touched.
- `.claude/rules/clients.md` (modify): `spot::api::market::GetDepth::limit`'s row.
- `docs/MUTANTS.md` (modify): spot's depth re-pricing row.
- `.github/scripts/tests/test_mutants_ledger.py` (modify): a `SNIPPETS` entry for each of that row's citations.

**Story 6: spot venue, keys and the umbrella.**

- `polyoxide-binance/src/spot/key.rs`, `venue.rs` (create); `spot/mod.rs` (modify: the module block replaced whole).
- `polyoxide-binance/src/lib.rs` (modify): the bundle pin.
- `polyoxide-binance/Cargo.toml` (modify): registration and two `[[test]]` stanzas.
- `polyoxide-binance/tests/venue_spot.rs`, `live_spot.rs` (create).
- `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs` (modify); `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs` (modify, append only).
- `.github/scripts/tests/test_dep_allowlists.py` (modify) and `ci/dep-allowlists/polyoxide@binance-spot.txt` (by `--write`).
- `ci/venue-gate-skip.txt` (modify): `spot` among the generic words.
- `polyoxide/Cargo.toml`'s `description` names Binance spot (in the umbrella task).
- `.github/workflows/ci.yml` (modify): `headers_spot` in both header-pin steps, which `test_ci_workflow.py` derives once `spot` is registered and in the umbrella's `full`.
- `.claude/rules/venue-traits.md` (modify): spot's key space, its class's product and its bundle pin; `.claude/rules/ci-gates.md` (modify): the generic words and Binance's identifiers.
- The files `gen_registry.py --write` rewrites; `Cargo.lock`.

**Story 7: docs.**

- `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide/README.md`, `README.md` (modify, outside generated regions).
- `.claude/rules/binance.md`, `.claude/rules/umbrella.md` (modify).
- `.claude/rules/rate-limits.md`, `send-loop.md`, `live-tests.md`, `testing.md`, `errors.md`, `clients.md`, `wire-enums.md`, `specs.md` (modify, text only): each sentence that lists Binance's `usdm` counterparts gains spot's.
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
- **`usdm::weight` after Story 2**: `pub struct WeightBudget { inner: Arc<Engine> }`, derives and docs as today; `pub use crate::shared::weight::{DEFAULT_BAN, MAX_COOLDOWN};`; `PUBLISHED_WEIGHT_PER_MINUTE`, `PUBLISHED_FUNDING_PER_FIVE_MINUTES`, `WEIGHT_LAYER`, `FUNDING_LAYER`, `Cost`, `Route` and `const FUNDING_PERIOD` unchanged; `WeightBudget::new`, `with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self`, `per_minute`, `used`, `acquire(&self, cost: Cost) -> MinuteCharge`, `record_used`, `begin_cooldown`, `until_next_minute` with today's names and signatures, delegating to `Engine`, `acquire` and `record_used` gaining `#[cfg(test)]` since only tests call them once `Throttle` goes through `Engine` (a new test-context item, not a changed one); the `#[cfg(test)]` hooks `at_ms`, `is_shared_with` and `hold_until_next_minute` byte for byte; `impl Throttle for WeightBudget` through `Engine::charge(meta, WEIGHT_LAYER, Some(FUNDING_LAYER))` and `Engine::observe(.., WEIGHT_LAYER)`. In scope for its tests: `Arc`, `tokio::time::Instant`, `Clock`, `FUNDING_PERIOD`, `MAX_COOLDOWN`.
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
- **`docs/MUTANTS.md`**: rows (a), "Binance's policy", and (k) name `polyoxide-binance/src/shared/policy.rs:<line>` (the arms in `shared::policy::decide`) and the new line of `a_418_without_retry_after_holds_two_minutes` in `src/usdm/policy.rs`; each re-proved. `.github/scripts/tests/test_mutants_ledger.py`'s `SNIPPETS` replaces its three `polyoxide-binance/src/usdm/policy.rs` entries (lines 53, 57 and 185) with one entry per new citation, keyed by the new file and line, each holding a piece of that line; its other entries do not change.

**Story 3 → Story 6.** In `polyoxide-venue`:

- `ProductClass::Spot`, after `PerpetualFuture`, `as_str()` `"spot"`, its doc: buying and selling the asset itself; a price is unbounded, a position is a balance of the base asset and so never negative, and there is no funding.
- `pub trait SpotMarket: MarketData {}` and `impl<T: MarketData> SpotMarket for T {}`, feature `market`, after `EventContractMarket` in `market.rs`, re-exported at the crate root beside `PerpetualMarket` and `EventContractMarket`.
- `.claude/rules/venue-traits.md`, in Story 3's commit (AD-21): the `ProductClass` bullet names `Spot` (with no product yet), and the bundles paragraph says `SpotMarket` is `MarketData` alone. Story 6 adds Binance's spot to both. `polyoxide-venue/README.md`'s bundles sentence names `SpotMarket` too, naming no venue.

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
- **Test files**: `tests/mock_spot.rs` with a helper `fn fixture(name: &str) -> String` reading `tests/fixtures/spot/rest/<name>.json` through `polyoxide_test_support`'s `fixtures!("spot/rest").text(name)`, so `fixture("time")` reads `time.json`; Story 5 appends after its last test, with no test name Story 4's tests use.
- **A spot-only build is warning-free**: Story 4 marks with `#[cfg_attr(not(feature = "usdm"), allow(dead_code))]` exactly the `shared` items that `cargo build -p polyoxide-binance --no-default-features --features spot` reports as unused (the build decides; it may report none, since `Engine::charge` reaches both acquire paths), and changes nothing else in `shared`; no step expects the build to fail before the marks.
- **`.claude/rules/ci-gates.md`**, in Story 4's commit (AD-21): the `polyoxide-binance` deny-list sentence names the files `@usdm`, `@usdm-ws` and `@spot`, with whatever rule `test_dep_allowlists.py` gains for `@spot`.
- **`GetExchangeInfo::symbol`** is a `query_setters!` setter; Story 4 pins it with an exact-query mock test, and Story 5's `tests/query_setters_spot.rs` gives it a row beside the market setters. Each `[[test]]` stanza has `required-features = ["spot"]`.

**Story 5 → Stories 6 and 7.**

- **`spot::api::market::MarketApi`**: `depth(&self, symbol: &Symbol) -> GetDepth`, `trades(&self, symbol: &Symbol) -> GetTrades`, `agg_trades(&self, symbol: &Symbol) -> GetAggTrades`, `klines(&self, symbol: &Symbol, interval: Interval) -> GetKlines`, `avg_price(&self, symbol: &Symbol) -> GetAvgPrice`, `ticker_24h(&self, symbol: &Symbol) -> GetTicker24h`, `tickers_24h(&self) -> GetTickers24h`, `price(&self, symbol: &Symbol) -> GetPrice`, `prices(&self) -> GetPrices`, `book_ticker(&self, symbol: &Symbol) -> GetBookTicker`, `book_tickers(&self) -> GetBookTickers`; each builder has `pub fn cost(&self) -> Cost` and `pub async fn send(self) -> Result<_, SpotError>`, returning `Depth`, `Vec<Trade>`, `Vec<AggTrade>`, `Vec<Kline>`, `AvgPrice`, `Ticker24h`, `Vec<Ticker24h>`, `PriceTicker`, `Vec<PriceTicker>`, `BookTicker`, `Vec<BookTicker>`.
- **Setters**: `GetDepth::limit(self, limit: u32) -> Self`, hand-written, replacing the route with `Route::Depth { limit: Some(limit) }`; from `query_setters!`: `GetTrades::limit: u32`; `GetAggTrades::{from_id: u64 => "fromId", start_time: u64 => "startTime", end_time: u64 => "endTime", limit: u32}`; `GetKlines::{start_time: u64, end_time: u64, limit: u32}` (no time-zone setter).
- **`spot::types`** (Story 5's part): `Interval`, exhaustive, from `wire_enum!`: `Second1 => "1s"`, `Minute1 => "1m"`, `Minute3 => "3m"`, `Minute5 => "5m"`, `Minute15 => "15m"`, `Minute30 => "30m"`, `Hour1 => "1h"`, `Hour2 => "2h"`, `Hour4 => "4h"`, `Hour6 => "6h"`, `Hour8 => "8h"`, `Hour12 => "12h"`, `Day1 => "1d"`, `Day3 => "3d"`, `Week1 => "1w"`, `Month1 => "1M"`; `Depth { pub last_update_id: u64, pub bids: Vec<Level>, pub asks: Vec<Level> }`; `Level { pub price: Decimal, pub quantity: Decimal }` with `Level::new`, positional; `Trade { pub id: u64, pub price: Decimal, pub qty: Decimal, pub quote_qty: Decimal, pub time: u64, pub is_buyer_maker: bool, pub is_best_match: bool }`; `AggTrade { pub id: u64, pub price: Decimal, pub quantity: Decimal, pub first_trade_id: u64, pub last_trade_id: u64, pub time: u64, pub is_buyer_maker: bool, pub is_best_match: bool }` (wire `a`, `p`, `q`, `f`, `l`, `T`, `m`, `M`); `Kline` with `usdm::types::Kline`'s fields, positional, extra elements tolerated; `AvgPrice { pub mins: u32, pub price: Decimal, pub close_time: u64 }`; `Ticker24h`, `PriceTicker { pub symbol: String, pub price: Decimal }`, `BookTicker { pub symbol: String, pub bid_price: Decimal, pub bid_qty: Decimal, pub ask_price: Decimal, pub ask_qty: Decimal }`; each with every key its fixture carries, `#[non_exhaustive]`.
- **`.claude/rules/clients.md`** gains `spot::api::market::GetDepth::limit` beside `usdm`'s row, for the same reason.
- **`docs/MUTANTS.md`**'s spot depth re-pricing row cites `file:line` as every other row does (the setter's line in `src/spot/api/market.rs` and each failing test's line), and `.github/scripts/tests/test_mutants_ledger.py`'s `SNIPPETS` gains one entry per citation, appended after its last entry.
- **`examples/weight_probe_spot.rs`**, its `[[example]]` stanza `name = "weight_probe_spot"`, `test = true`, `required-features = ["spot"]`, after `weight_probe`'s; it takes no `--product` argument, and `weight_probe.rs` and its stanza do not change (spec §5.5).

**Story 6 → Story 7.**

- **`spot::key`**: `pub const VENUE: VenueId = VenueId::from_static("binance");`, `pub const PRODUCT: ProductId = ProductId::from_static("spot");`, `pub struct SpotKeys;` implementing `KeySpace` with `const CLASS: ProductClass = ProductClass::Spot`, `pub fn key(symbol: &Symbol) -> MarketKey`, `pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError>`, `pub fn symbol(key: &MarketKey) -> Result<Symbol, KeyError>`, as `usdm::key`'s.
- **`spot::venue`**: `impl MarketData`, `Instruments`, `Trades`, `Candles` for `Spot` (spec §5.8); `InstrumentExt { pub status: Option<SymbolStatus>, pub permission_sets: Option<Vec<Vec<String>>>, pub order_types: Option<Vec<String>>, pub is_spot_trading_allowed: Option<bool>, pub is_margin_trading_allowed: Option<bool>, pub filters: Option<Vec<Filter>> }`, `BookExt { pub last_update_id: Option<u64> }`, `TradeExt { pub first_trade_id: Option<u64>, pub last_trade_id: Option<u64>, pub is_best_match: Option<bool> }`, `CandleExt { pub close_time: Option<UnixMillis>, pub taker_buy_volume: Option<Decimal>, pub taker_buy_notional: Option<Decimal> }`, each `#[derive(Debug, Clone, PartialEq, Eq, Default)] #[non_exhaustive]` (`Copy` where every field is), except `InstrumentExt`, which derives `Debug, Clone, PartialEq, Default` as `usdm`'s does, since `Filter` derives `PartialEq` without `Eq` as `usdm`'s `Filter` does.
- **Registration** in `polyoxide-binance/Cargo.toml`: `products = ["usdm", "spot"]`, `classes = { usdm = "perpetual-future", spot = "spot" }`, `identifiers = ["fapi", "fstream", "X-MBX", "api/v3"]`, the `readme` line `Client library for Binance's APIs, one feature per module: \`usdm\` (USDⓈ-M futures market data, with its streams under \`usdm-ws\`) and \`spot\` (spot market data); re-exported by the umbrella as \`polyoxide::binance\``, and `[package.metadata.polyoxide.live.live_spot]` with `suite = "spot"`, `timeout = 15`, `features = ["spot"]`, `secrets = []`.
- **Umbrella**: features `binance-spot = ["dep:polyoxide-binance", "polyoxide-binance/spot", "polyoxide-venue/market"]` and `binance = ["binance-usdm", "binance-spot"]`; `full` gains `binance-spot`; `polyoxide::binance`'s `cfg` gains `feature = "binance-spot"`; `parse_key` reads `binance.spot` through `binance::spot::key::parse`; the prelude gains, under `binance-spot`, `pub use polyoxide_binance::spot::{Spot, SpotBuilder, SpotError, WeightBudget as SpotWeightBudget};`; the passthrough check gains `#[cfg(feature = "binance-spot")] const _: fn(crate::binance::spot::Spot) = drop;`. `live_venue`'s registered features gain `binance-spot`.

- **The venue gate**: registering product `spot` makes `scripts/venue_gate.py` look for it in the foundation crates, where `ProductClass::Spot` and made-up `ProductId::from_static("spot")` keys hold it as an ordinary word; Story 6 adds `spot` to `ci/venue-gate-skip.txt`'s generic words, after `events`, with no `gate_exceptions` entry.
- **Rules files Story 6 edits (AD-21)**: `.claude/rules/ci-gates.md`'s generic-words sentence gains `spot`, Binance's identifiers gain `api/v3`, the features job's header pins name `--features spot --test headers_spot` beside `headers_usdm` (the pin test derives it once `spot` is a registered product), and the `polyoxide` deny-list sentence's HTTP modules gain `binance-spot`; and `.github/workflows/ci.yml`'s two header-pin steps gain the `headers_spot` lines `.github/scripts/tests/test_ci_workflow.py` derives, in the umbrella commit; `.claude/rules/venue-traits.md` gains `spot::key`'s key-space bullet beside `usdm::key`'s, Binance's spot under the `Spot` class, and `Spot` (`spot`) pinned to `SpotMarket` in the pins sentence.

**Rules files that follow the moved and new code (seams).**

- **`paths:` globs.** `.claude/rules/rate-limits.md` (today `polyoxide-binance/src/usdm/{weight,policy}.rs`), `send-loop.md` (`usdm/{policy,request}.rs`) and `clients.md` (`usdm/{mod,request}.rs`) load by globs that name the files Story 2 empties. Story 2 adds the `shared` counterparts of each (`shared/{weight,policy}.rs`, `shared/{policy,request}.rs`, `shared/request.rs`), in the commit that creates them. Story 4 adds the `spot` counterparts (`spot/{weight,policy}.rs`, `spot/{policy,request}.rs`, `spot/{mod,request}.rs`), in the commit that creates them, because a glob that matches no file fails `test_claude_rules.py`. Neither story changes a rule's body.
- **Body text** that lists Binance's `usdm` counterparts gains spot's, in Story 7, anchored on text and not on line numbers, quoting each file as Stories 2, 4 and 5 leave it. The sentences are: `rate-limits.md`'s Binance line; `send-loop.md`'s route-builder, ping, retry and headers-suite sentences; `live-tests.md`'s `failure_tags_usdm.rs` sentence; `testing.md`'s mock-suite list; `errors.md`'s module-error list; `clients.md`'s builder list; `wire-enums.md`'s Binance types file and its `Interval` sentence; `specs.md`'s "Binance publishes none for USDⓈ-M" sentence (spot's swagger is stale and not watched).
- **Not edited:** `docs/s3-migration.md`, since `ProductClass::Spot` is additive on a `#[non_exhaustive]` enum and the CHANGELOG comes from the version bump. `.github/scripts/tests/test_classify_coverage.py`'s Binance floor and `test_classify_failures.py`'s twin table stay at `usdm`, as the floor is a lower bound and spot's tags come from the same test-support code.
- **Quoted texts that fixers leave unchanged.** A later story quotes these from an earlier one: Story 2's `#[cfg(feature = "usdm")] mod shared;` lines in `src/lib.rs`; Story 4's `spot/mod.rs` module block, `Spot`'s `use` list and `namespaces!` entries, `spot/api/mod.rs`'s two `pub mod` lines, and `src/lib.rs`'s lines before the bundle pin; Story 3's `ProductClass` bullet and bundles paragraph in `.claude/rules/venue-traits.md`; `docs/MUTANTS.md`'s row (k) as Story 2 leaves it; and `test_dep_allowlists.py`'s anchors.

**Who edits the shared docs.** Story 6 changes `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md` and the two nightly workflows only through `python3 scripts/gen_registry.py --write`, inside generated regions; every hand-written line of those files is Story 7's. `polyoxide-binance/README.md` and `src/lib.rs`'s crate docs are Story 7's, but for `lib.rs`'s code lines, which Stories 2, 4 and 6 write. `docs/specs/binance/OBSERVED.md` is Story 1's, and a later story adds only above its anchor.

**Design stage:** /flow-draft session 7ecd7f2e-68e5-45e2-8ee9-ea123ccb13d5, 2026-11-02T06:53:07Z to 2026-11-02T07:51:15Z UTC: $33.79 logged, $34.08 with estimated thinking; 13264 plan lines, $2.55 per 1,000 lines.

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
  - **Files:** `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/weight.rs`, `polyoxide-binance/src/shared/policy.rs`, `polyoxide-binance/src/shared/error.rs`, `polyoxide-binance/src/shared/request.rs`, `polyoxide-binance/src/shared/symbol.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/src/usdm/policy.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/types.rs`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/clients.md`
  - **Interfaces:** exposes "Story 2 → Stories 4 and 5"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_mutants_ledger.py -q'`
  - **Review:** deep (a refactor under a public API and two mutation-tested rows; `RESERVED_FRACTION`)
  - **Reading:** spec §2 (the moved-tests, shared-types and `Symbol` rows), §5.1, §5.2; `polyoxide-binance/src/usdm/weight.rs:1-505`, `polyoxide-binance/src/usdm/policy.rs:1-90`, `polyoxide-binance/src/usdm/error.rs:1-230`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/types.rs:1-190`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/lib.rs`; `docs/MUTANTS.md` (rows (a) and (k), and the paragraphs on re-proving); `scripts/test_body_diff.py:1-90` (what counts as a test-context item)

- **Story 3** — `ProductClass::Spot` and the `SpotMarket` bundle (spec §5.3).

  Adds the class with its `as_str` and doc, the bundle with its blanket impl and docs in the shape of `PerpetualMarket`'s, the root re-export, and the crate docs' sentence naming the bundles. The test file proves `as_str`, that a key a made-up spot `KeySpace` makes carries the class, and that a type with `MarketData` alone satisfies a `SpotMarket` bound. The test file opens with `#![cfg(all(feature = "keys", feature = "market"))]`, as the crate's other test files gate on their features. The docs and tests name no venue.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-venue/src/key.rs`, `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/tests/spot_class.rs`, `.claude/rules/venue-traits.md`, `polyoxide-venue/README.md`
  - **Interfaces:** exposes "Story 3 → Story 6"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-venue --all-features && cargo test -p polyoxide-venue --features keys,market --test spot_class && python3 scripts/venue_gate.py`
  - **Review:** deep (a foundation crate's public enum and trait)
  - **Reading:** spec §2 (the product-class row), §5.3; `polyoxide-venue/src/key.rs:85-130`, `polyoxide-venue/src/market.rs:170-215`, `polyoxide-venue/src/lib.rs:1-110`; `docs/plans/2026-10-21-adr-0001-product-classes.md` (Task 3)

- **Story 4** — The spot client: feature, builder, weight budget, policy, errors, health and exchange routes (spec §4, §5.4 to §5.7, §5.10).

  Adds the `spot` feature (and `default = ["usdm", "spot"]`), widens `shared`'s `cfg`, and writes `spot/mod.rs`, `weight.rs` with the whole route table and `documented_weights` unit test, `policy.rs`, `error.rs`, `request.rs`, `types.rs`'s exchange part, `api/health.rs`, `api/exchange.rs` and `README.md` (a doctest under `spot`), with the builder's in-flight note (spec §4). The suites cover both routes on a mock server, the exact `Cost` each request carries, a `429` retried and holding the budget, a `418` failing and holding it, the `418`'s WARN under `polyoxide_binance::spot::error`, every error's class, the request headers, and the failure tags. `cargo hack --each-feature` builds `spot` without `usdm`. The dep allowlists gain `polyoxide-binance@spot.txt`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/spot/error.rs`, `polyoxide-binance/src/spot/policy.rs`, `polyoxide-binance/src/spot/request.rs`, `polyoxide-binance/src/spot/types.rs`, `polyoxide-binance/src/spot/weight.rs`, `polyoxide-binance/src/spot/api/mod.rs`, `polyoxide-binance/src/spot/api/health.rs`, `polyoxide-binance/src/spot/api/exchange.rs`, `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/headers_spot.rs`, `polyoxide-binance/tests/error_classes_spot.rs`, `polyoxide-binance/tests/failure_tags_spot.rs`, `polyoxide-binance/tests/ban_log_spot.rs`, `polyoxide-binance/src/shared/weight.rs`, `.claude/rules/ci-gates.md`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/clients.md`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide-binance@spot.txt`, `Cargo.lock`
  - **Interfaces:** exposes "Story 4 → Stories 5, 6 and 7"; uses Story 1's `time.json`, `exchange_info.json` and `error_invalid_symbol.json`, and Story 2's `shared` items
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_module_readmes.py -q'`
  - **Review:** deep (a new public client and its rate budget)
  - **Reading:** spec §3.1, §4, §5.4 to §5.7, §5.10; `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/usdm/api/exchange.rs`, `polyoxide-binance/src/usdm/types.rs:190-600`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/mock_usdm.rs:1-120`, `polyoxide-binance/tests/headers_usdm.rs`, `polyoxide-binance/tests/error_classes_usdm.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/ban_log_usdm.rs`, `.github/scripts/tests/test_dep_allowlists.py:100-160`; `docs/specs/binance/OBSERVED.md` (the `## Spot` section)

- **Story 5** — Spot market routes (spec §3.1, §4, §5.6).

  Adds `api/market.rs` with the eleven builders, their types and `Interval`, the hand-written `GetDepth::limit` that re-prices the route by its band, and `market` in `Spot`'s namespaces. The suites cover each route's path, query and `Cost` on a mock server (appended to `mock_spot.rs`), every setter's exact query pair, and every fixture decoding with no unmodelled key and serializing back. A new example, `weight_probe_spot`, probes the spot routes (`weight_probe.rs` and its stanza stay byte for byte: it is `test = true`, so the test-bodies gate counts its items); its bare client copies `weight_probe.rs`'s, which holds the landmine marker `.gzip(` (`.gzip(true)`): copied, never changed, and no other client gains one; `clients.md` gains the setter's row; `MUTANTS.md` gains spot's depth re-pricing row (the mutation: `GetDepth::limit` leaving the route at `Depth { limit: None }`), proved.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/spot/api/market.rs`, `polyoxide-binance/src/spot/api/mod.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/types.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/wire_agreement_spot.rs`, `polyoxide-binance/tests/query_setters_spot.rs`, `polyoxide-binance/examples/weight_probe_spot.rs`, `.claude/rules/clients.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`
  - **Interfaces:** exposes "Story 5 → Stories 6 and 7"; uses Story 1's fixtures and Story 4's client, `Route`, `Cost`, `SpotError` and the `mock_spot.rs` helper
  - **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot && cargo test -p polyoxide-binance --all-features --example weight_probe_spot && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`
  - **Review:** deep (the weight table every request is charged by)
  - **Reading:** spec §3.1, §4, §5.6; `polyoxide-binance/src/usdm/api/market.rs`, `polyoxide-binance/src/usdm/types.rs:600-920`, `polyoxide-binance/tests/wire_agreement_usdm.rs`, `polyoxide-binance/tests/query_setters_usdm.rs`, `polyoxide-binance/examples/weight_probe.rs`, `.claude/rules/clients.md`; `docs/MUTANTS.md` (one row's format)

- **Story 6** — `spot::venue`, keys, registration, `live_spot` and the umbrella's `binance-spot` (spec §5.8, §5.10, §5.11).

  Adds `spot::key` with its registration unit test (as `usdm::key`'s), `spot::venue` with the four trait impls, their extensions and the skipped-row WARN, and pins `Spot` to `SpotMarket` in `lib.rs`. Registers `spot` in the crate's metadata with its class, identifier and live entry, then runs `gen_registry.py --write`. `venue_spot` holds each method to the native client on the fixtures and the skipped row's WARN; `live_spot` reads every route and every venue method against production, with `live_responses_carry_no_unmodelled_keys` and a test that fails when `instruments` skips a row. The umbrella gains `binance-spot`, `parse_key`'s branch, the prelude names and the passthrough check; new tests in `parse_key.rs` read `binance.spot:btcusdt` and check its class, and `live_venue` gains a spot case. The dep allowlists gain `polyoxide@binance-spot.txt`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/spot/key.rs`, `polyoxide-binance/src/spot/venue.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/venue_spot.rs`, `polyoxide-binance/tests/live_spot.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide@binance-spot.txt`, `ci/venue-gate-skip.txt`, `.claude/rules/venue-traits.md`, `.claude/rules/ci-gates.md`, `.github/workflows/ci.yml`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`, `Cargo.lock`
  - **Interfaces:** exposes "Story 6 → Story 7"; uses Story 3's `ProductClass::Spot` and `SpotMarket`, and Stories 4 and 5's client, routes and types
  - **Verify:** `cargo test -p polyoxide-binance --all-features && cargo test -p polyoxide --all-features && cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests -q'`
  - **Review:** deep (the venue mapping every trait consumer reads, and the umbrella's public surface)
  - **Reading:** spec §5.8, §5.10, §5.11; `polyoxide-binance/src/usdm/key.rs`, `polyoxide-binance/src/usdm/venue.rs`, `polyoxide-binance/tests/venue_usdm.rs:1-140`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-binance/Cargo.toml` (the `[package.metadata.polyoxide]` tables), `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs:100-300`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`

- **Story 7** — Docs (spec §5.13).

  Adds spot to `docs/specs/binance/INDEX.md` (a section with the routes table and the drift detectors), the crate README and `src/lib.rs`'s crate docs (both modules), the umbrella README and the root README's hand-written lines, `.claude/rules/binance.md` (a `shared` section, a `spot` section, and `scripts/capture_binance_spot.py` and `polyoxide-binance/src/spot/**` covered by its `paths:`), `.claude/rules/umbrella.md` (the `binance-spot` feature and prelude names); updates `[workspace.metadata.polyoxide.mirrors.binance]` in the root manifest (`name` `Binance USDⓈ-M and spot`, `base_urls` gaining `https://api.binance.com`, `description` naming spot, and `exclude` saying Binance publishes no spec for USDⓈ-M and that spot's `spot_api.yaml` is stale since 2024-10-08 and lacks fields the wire sends, so the live suites are the drift check), then runs `gen_registry.py --write`; and writes the spine amendment recording `ProductClass::Spot` and `SpotMarket`.

  - **Developer:** junior-dev
  - **Files:** `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `README.md`, `.claude/rules/binance.md`, `.claude/rules/umbrella.md`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/live-tests.md`, `.claude/rules/testing.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`, `.claude/rules/wire-enums.md`, `.claude/rules/specs.md`, `Cargo.toml`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md`, `CLAUDE.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-schema.yml`
  - **Interfaces:** uses every earlier story's names; exposes none
  - **Verify:** `cargo test --doc -p polyoxide-binance --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q' && bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide --all-features'`
  - **Review:** light (docs and records; no library code)
  - **Reading:** spec §1, §5.13, §9; `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `.claude/rules/binance.md`, `.claude/rules/umbrella.md`, `Cargo.toml:265-271`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/adr-0001.md`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->

## Story 1: REST probe and captures

- **Developer:** intermediate-dev
- **Files:** `docs/specs/binance/probes/probe_rest.py`, `scripts/capture_binance_spot.py`, `polyoxide-binance/tests/fixtures/spot/rest/time.json`, `polyoxide-binance/tests/fixtures/spot/rest/exchange_info.json`, `polyoxide-binance/tests/fixtures/spot/rest/depth.json`, `polyoxide-binance/tests/fixtures/spot/rest/trades.json`, `polyoxide-binance/tests/fixtures/spot/rest/agg_trades.json`, `polyoxide-binance/tests/fixtures/spot/rest/klines.json`, `polyoxide-binance/tests/fixtures/spot/rest/klines_1s.json`, `polyoxide-binance/tests/fixtures/spot/rest/avg_price.json`, `polyoxide-binance/tests/fixtures/spot/rest/ticker_24hr.json`, `polyoxide-binance/tests/fixtures/spot/rest/tickers_24hr.json`, `polyoxide-binance/tests/fixtures/spot/rest/price.json`, `polyoxide-binance/tests/fixtures/spot/rest/prices.json`, `polyoxide-binance/tests/fixtures/spot/rest/book_ticker.json`, `polyoxide-binance/tests/fixtures/spot/rest/book_tickers.json`, `polyoxide-binance/tests/fixtures/spot/rest/error_invalid_symbol.json`, `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`, `docs/specs/binance/OBSERVED.md`
- **Interfaces:** exposes the fixtures, `PROVENANCE.md`, the capture and probe commands and the `OBSERVED.md` anchor under "Story 1 → Stories 4 to 7"; uses nothing from earlier stories
- **Verify:** `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-binance/tests/fixtures/spot/rest/*.json')]" && grep -qx '<!-- spot findings go above this line -->' docs/specs/binance/OBSERVED.md && python3 docs/specs/binance/probes/probe_rest.py --help && python3 scripts/capture_binance_spot.py --help`
- **Review:** light (scripts, captures and records; no library code)

Gives `probe_rest.py` a `--product {usdm,spot}` choice (`usdm` the default, its run unchanged) and a spot mode that prints four Markdown sections for `OBSERVED.md`: every row of the design spec's section 3.1 table from `X-MBX-USED-WEIGHT-1M` deltas over distinct URLs, the limits past each route's maximum, the minute window across a boundary, and whether `api.binance.com` and `data-api.binance.vision` share one counter. Writes `scripts/capture_binance_spot.py rest`, which fetches the fixtures the names contract lists from production with no credentials, trimming only list lengths, and prints `PROVENANCE.md`'s REST section; runs both against the live host; and assembles `PROVENANCE.md` and `OBSERVED.md`'s `## Spot` section, ending with its anchor line. No Rust, no landmine marker. A scripts story has no failing unit test of its own to write against the network, so each task's first step checks the pure functions the script is built from on made-up data (never kept as a fixture), or, for the two runs, checks the files the run must produce. A measured weight that differs from section 3.1, or a counter that is not shared, is recorded in `OBSERVED.md` by the run and reported to the supervisor before Story 4 starts. The runs need network access to `api.binance.com` (a `451` from a US runner is environmental: stop and report it).

### Task 1.1: `probe_rest.py --product {usdm,spot}`

**Files:**
- Modify: `docs/specs/binance/probes/probe_rest.py` (the whole file; the `usdm` run is its current code under a new function name)

- [ ] **Step 1: Write the failing check**

The pure parts of the probe are checked offline, on made-up readings. Run this from the repo root; it is not kept in the tree.

```bash
nix develop --command python3 - <<'PY'
import sys

sys.path.insert(0, "docs/specs/binance/probes")
import probe_rest as p

base = 1_800_000_000  # a multiple of 60: a UTC minute boundary

# The default product is USD-M, so the existing run is unchanged.
assert p.parse_args([]).product == "usdm" and p.parse_args([]).section is None
assert p.parse_args(["--product", "spot", "--section", "counter"]).section == "counter"
try:
    p.parse_args(["--section", "window"])
    raise AssertionError("--section without --product spot must be refused")
except SystemExit as err:
    assert err.code == 2

# The design spec's section 3.1, row by row.
want = {
    "ping": 1,
    "time": 1,
    "exchangeInfo, with symbol": 20,
    "exchangeInfo, without symbol": 20,
    "depth, no limit": 5,
    "depth, limit 1 to 100": 5,
    "depth, limit 101 to 500": 25,
    "depth, limit 501 to 1000": 50,
    "depth, limit above 1000": 250,
    "trades, no limit": 25,
    "trades, with limit": 25,
    "aggTrades, no limit": 4,
    "aggTrades, with limit": 4,
    "klines, no limit": 2,
    "klines, with limit": 2,
    "avgPrice": 2,
    "ticker/24hr, one symbol": 2,
    "ticker/24hr, all": 80,
    "ticker/price, one symbol": 2,
    "ticker/price, all": 4,
    "ticker/bookTicker, one symbol": 2,
    "ticker/bookTicker, all": 4,
}
assert {label: expected for label, expected, _ in p.SPOT_ROWS} == want
for label, _, paths in p.SPOT_ROWS:
    assert all(path.startswith("/api/v3/") for path in paths), label
    if label not in ("ping", "time"):
        assert len(set(paths)) == len(paths), f"{label}: the URLs must be distinct"

# A row agrees only when every URL cost the spec's weight.
assert p.weight_verdict([5, 5, 5], 5) == "agrees"
assert p.weight_verdict([5, 25, 5], 5) == "DIFFERS"
assert p.weight_verdict([5, None, 5], 5) == "DIFFERS"
assert p.weight_verdict([], 5) == "DIFFERS"

# The window is the UTC clock minute when the first fall in the reading crosses a boundary.
assert p.window_verdict([(base + 51.0, 8), (base + 54.0, 9), (base + 57.0, 10),
                         (base + 60.2, 1), (base + 63.2, 2)])[0] == "UTC_MINUTE"
assert p.window_verdict([(base + 10.0, 5), (base + 13.0, 6), (base + 16.0, 2)])[0] == "NOT_UTC_MINUTE"
assert p.window_verdict([(base + 50.0, 1), (base + 53.0, 2), (base + 56.0, 3)])[0] == "NO_DROP"
assert p.window_verdict([(base + 57.0, None), (base + 60.2, 1)])[0] == "NO_DROP"
assert p.clock(base + 59.5) == "08:00:59.5"
assert "08:00:57.0" in p.window_verdict([(base + 57.0, 10), (base + 60.2, 1)])[1]

# One counter: each reading is the other host's last reading plus its own weight.
shared = [("a", 20, 100), ("b", 2, 102), ("a", 20, 122), ("b", 2, 124)]
separate = [("a", 20, 20), ("b", 2, 2), ("a", 20, 40), ("b", 2, 4)]
neither = [("a", 20, 20), ("b", 2, 9), ("a", 20, 77), ("b", 2, 3)]
assert p.counter_verdict(shared)[0] == "SHARED"
assert p.counter_verdict(separate)[0] == "NOT_SHARED"
assert p.counter_verdict(neither)[0] == "INCONCLUSIVE"
assert p.counter_verdict([("a", 20, 20), ("b", 2, None), ("a", 20, 40), ("b", 2, 4)])[0] == "INCONCLUSIVE"

# exchangeInfo's rateLimits, as a sentence.
body = (b'{"rateLimits":[{"rateLimitType":"REQUEST_WEIGHT","interval":"MINUTE","intervalNum":1,'
        b'"limit":6000},{"rateLimitType":"RAW_REQUESTS","interval":"MINUTE","intervalNum":5,'
        b'"limit":300000}]}')
assert p.rate_limits_sentence(body) == (
    "`exchangeInfo`'s `rateLimits`: REQUEST_WEIGHT 6000 per 1 minute; "
    "RAW_REQUESTS 300000 per 5 minute.")
assert "could not be read" in p.rate_limits_sentence(b"<html>")
print("ok")
PY
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: it fails at the first line that uses the new code, with `AttributeError: module 'probe_rest' has no attribute 'parse_args'`.

- [ ] **Step 3: Implement**

Replace the whole of `docs/specs/binance/probes/probe_rest.py` with the file below. Its `USD-M` part is the current file's: `CALLS` and the body of the old `main()` are byte for byte the same, `main()` is renamed `usdm_main()`, and `call` gains a `base` argument that defaults to the old `BASE`, so `python3 probe_rest.py` with no argument runs what it ran before.

```python
"""Measure Binance REST request weights from X-MBX-USED-WEIGHT-1M deltas.

Usage:
  python3 -I probe_rest.py                    USD-M futures, as before
  python3 -I probe_rest.py --product spot     spot: the four sections below, as Markdown on stdout
  python3 -I probe_rest.py --product spot --section counter

Each call's weight is the header after it minus the header after the previous call,
so the calls run back to back inside one minute window.

The spot sections, each headed as OBSERVED.md's `## Spot` section heads it:
  weights   every route of the design spec's section 3.1, over distinct URLs
  limits    a limit past a route's maximum, depth at any limit, a two-hour aggTrades
            window, an unknown symbol
  window    a ping every 3 s across a minute boundary: is the window the UTC clock minute?
  counter   do api.binance.com and data-api.binance.vision share one counter?
Progress goes to stderr, the Markdown to stdout. The host's clock must be NTP-synced.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from typing import Callable, NamedTuple

BASE = "https://fapi.binance.com"
CALLS = [
    ("ping", "/fapi/v1/ping"),
    ("exchangeInfo", "/fapi/v1/exchangeInfo"),
    ("ticker24hr all", "/fapi/v1/ticker/24hr"),
    ("ticker24hr one", "/fapi/v1/ticker/24hr?symbol=BTCUSDT"),
    ("premiumIndex all", "/fapi/v1/premiumIndex"),
    ("premiumIndex one", "/fapi/v1/premiumIndex?symbol=BTCUSDT"),
    ("fundingInfo", "/fapi/v1/fundingInfo"),
    ("klines 1500", "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=1500"),
    ("klines 499", "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=499"),
    ("klines 99", "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=99"),
    ("fundingRate 1000", "/fapi/v1/fundingRate?symbol=BTCUSDT&limit=1000"),
    ("openInterest", "/fapi/v1/openInterest?symbol=BTCUSDT"),
    ("aggTrades 100", "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=100"),
    ("depth 20", "/fapi/v1/depth?symbol=BTCUSDT&limit=20"),
    ("depth 100", "/fapi/v1/depth?symbol=BTCUSDT&limit=100"),
    ("unknown symbol", "/fapi/v1/premiumIndex?symbol=NOTASYMBOLUSDT"),
]

SPOT_BASE = "https://api.binance.com"
VISION_BASE = "https://data-api.binance.vision"
USED_HEADER = "x-mbx-used-weight-1m"
SYMBOLS = ("BTCUSDT", "ETHUSDT", "BNBUSDT")
COUNTER_SYMBOLS = ("BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT")
FENCE = "`" * 3  # a Markdown code fence, built so this file holds none


def by_symbol(template):
    return [template.format(s=s) for s in SYMBOLS]


def with_limits(template, limits):
    """One URL per limit, each on a different symbol, so no two URLs are alike."""
    return [template.format(s=s, n=n) for s, n in zip(SYMBOLS, limits)]


# (row, the weight the design spec's section 3.1 gives it, the distinct URLs measured).
SPOT_ROWS = [
    ("ping", 1, ["/api/v3/ping", "/api/v3/ping"]),
    ("time", 1, ["/api/v3/time", "/api/v3/time"]),
    ("exchangeInfo, with symbol", 20, [f"/api/v3/exchangeInfo?symbol={s}" for s in SYMBOLS[:2]]),
    ("exchangeInfo, without symbol", 20,
     ["/api/v3/exchangeInfo", "/api/v3/exchangeInfo?showPermissionSets=false"]),
    ("depth, no limit", 5, by_symbol("/api/v3/depth?symbol={s}")),
    ("depth, limit 1 to 100", 5, with_limits("/api/v3/depth?symbol={s}&limit={n}", (1, 7, 100))),
    ("depth, limit 101 to 500", 25, with_limits("/api/v3/depth?symbol={s}&limit={n}", (101, 300, 500))),
    ("depth, limit 501 to 1000", 50, with_limits("/api/v3/depth?symbol={s}&limit={n}", (501, 750, 1000))),
    ("depth, limit above 1000", 250, with_limits("/api/v3/depth?symbol={s}&limit={n}", (1001, 5000, 5001))),
    ("trades, no limit", 25, by_symbol("/api/v3/trades?symbol={s}")),
    ("trades, with limit", 25, with_limits("/api/v3/trades?symbol={s}&limit={n}", (1, 3, 500))),
    ("aggTrades, no limit", 4, by_symbol("/api/v3/aggTrades?symbol={s}")),
    ("aggTrades, with limit", 4, with_limits("/api/v3/aggTrades?symbol={s}&limit={n}", (1, 3, 500))),
    ("klines, no limit", 2, by_symbol("/api/v3/klines?symbol={s}&interval=1m")),
    ("klines, with limit", 2,
     with_limits("/api/v3/klines?symbol={s}&interval=1m&limit={n}", (1, 2, 1000))),
    ("avgPrice", 2, by_symbol("/api/v3/avgPrice?symbol={s}")),
    ("ticker/24hr, one symbol", 2, by_symbol("/api/v3/ticker/24hr?symbol={s}")),
    ("ticker/24hr, all", 80, ["/api/v3/ticker/24hr", "/api/v3/ticker/24hr?type=MINI"]),
    ("ticker/price, one symbol", 2, by_symbol("/api/v3/ticker/price?symbol={s}")),
    ("ticker/price, all", 4, ["/api/v3/ticker/price", "/api/v3/ticker/price?symbolStatus=TRADING"]),
    ("ticker/bookTicker, one symbol", 2, by_symbol("/api/v3/ticker/bookTicker?symbol={s}")),
    ("ticker/bookTicker, all", 4,
     ["/api/v3/ticker/bookTicker", "/api/v3/ticker/bookTicker?symbolStatus=TRADING"]),
]


def call(path, base=BASE):
    req = urllib.request.Request(base + path, headers={"Accept-Encoding": "identity"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            body = r.read()
            return r.status, dict(r.headers), body
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def usdm_main():
    prev = None
    for name, path in CALLS:
        status, headers, body = call(path)
        used = headers.get("X-MBX-USED-WEIGHT-1M") or headers.get("x-mbx-used-weight-1m")
        used = int(used) if used is not None else None
        delta = (used - prev) if (used is not None and prev is not None) else None
        prev = used if used is not None else prev
        extra = {k: v for k, v in headers.items() if k.lower().startswith("x-mbx") and k.lower() != "x-mbx-used-weight-1m"}
        note = ""
        if status != 200:
            note = body[:200].decode("utf-8", "replace")
        print(f"{name:18} status={status} used={used} weight={delta} bytes={len(body)} {extra or ''} {note}")
    # Headers worth knowing once.
    status, headers, _ = call("/fapi/v1/ping")
    keep = {k: v for k, v in headers.items() if k.lower() in ("server", "x-cache", "via", "cache-control", "content-encoding", "retry-after")}
    print("ping headers:", keep)


# ---- spot -------------------------------------------------------------------------


class Probe(NamedTuple):
    path: str
    status: int
    body: bytes
    used: int | None
    weight: int | None


def log(text):
    print(text, file=sys.stderr, flush=True)


def used_of(headers):
    lowered = {k.lower(): v for k, v in headers.items()}
    value = lowered.get(USED_HEADER)
    return int(value) if value is not None else None


def utc_date():
    return datetime.now(timezone.utc).strftime("%Y-%m-%d")


def clock(epoch):
    moment = datetime.fromtimestamp(epoch, timezone.utc)
    return f"{moment:%H:%M:%S}.{moment.microsecond // 100000}"


def wait_for_room(seconds):
    """Sleeps into the next UTC minute when fewer than `seconds` remain in this one, so a
    measurement does not straddle the window's reset."""
    left = 60 - time.time() % 60
    if left < seconds:
        time.sleep(left + 1.0)


def guard(status, path):
    if status in (418, 429):
        sys.exit(f"{path}: HTTP {status}; the host is limiting this address. "
                 "Stop and wait out its Retry-After.")


def measure(paths, base=SPOT_BASE):
    """One Probe per path, in order. The weight is the rise in the used-weight header over
    the call before it (a `time` call gives the first one its baseline); it is None when the
    header is missing or fell, which is a minute rollover."""
    wait_for_room(8 + 2 * len(paths))
    status, headers, _ = call("/api/v3/time", base)
    guard(status, "/api/v3/time")
    prev = used_of(headers)
    probes = []
    for path in paths:
        status, headers, body = call(path, base)
        guard(status, path)
        used = used_of(headers)
        weight = used - prev if used is not None and prev is not None and used >= prev else None
        probes.append(Probe(path, status, body, used, weight))
        if used is not None:
            prev = used
    return probes


def weight_verdict(weights, expected):
    """`agrees` when every distinct URL cost the spec's weight, else `DIFFERS`."""
    if weights and all(w == expected for w in weights):
        return "agrees"
    return "DIFFERS"


def window_verdict(samples):
    """`samples` is [(epoch seconds, used weight)] in time order. Returns a code and a
    sentence: the window is the UTC clock minute when the first fall in the reading happens
    between two samples taken in different UTC minutes."""
    for (t0, u0), (t1, u1) in zip(samples, samples[1:]):
        if u0 is None or u1 is None or u1 >= u0:
            continue
        fell = f"the reading fell from {u0} at {clock(t0)} to {u1} at {clock(t1)}"
        if int(t0 // 60) != int(t1 // 60):
            return "UTC_MINUTE", f"The window is the UTC clock minute: {fell}, across a minute boundary."
        return "NOT_UTC_MINUTE", f"The window is NOT the UTC clock minute: {fell}, inside one minute."
    return "NO_DROP", (f"The reading never fell in {len(samples)} samples: the run did not cross "
                       "a boundary, or the window slides. Run the section again.")


def counter_verdict(calls):
    """`calls` is [(host, weight, reading)] in order, hosts alternating. Shared when every
    reading after the first is the other host's last reading plus its own weight; separate
    when every reading after the second is the same host's last reading plus its own weight
    and none fits the shared rule."""
    if len(calls) < 4 or any(used is None for _, _, used in calls):
        return "INCONCLUSIVE", "Fewer than four readings, or a reading without the header."
    shared = [calls[i][2] == calls[i - 1][2] + calls[i][1] for i in range(1, len(calls))]
    own = [calls[i][2] == calls[i - 2][2] + calls[i][1] for i in range(2, len(calls))]
    if all(shared):
        return "SHARED", ("One counter: each host's reading is the other host's last reading "
                          "plus its own call's weight.")
    if all(own) and not any(shared):
        return "NOT_SHARED", ("Two counters: each host's reading is its own last reading "
                              "plus its own call's weight, whatever the other host was last asked.")
    return "INCONCLUSIVE", ("The readings fit neither rule (other traffic from this address, or "
                            "a minute rollover). Run the section again.")


def json_of(body):
    try:
        return json.loads(body)
    except ValueError:
        return None


def rate_limits_sentence(body):
    info = json_of(body)
    try:
        parts = [f"{r['rateLimitType']} {r['limit']} per {r['intervalNum']} {r['interval'].lower()}"
                 for r in info["rateLimits"]]
    except (KeyError, TypeError):
        return "`exchangeInfo`'s `rateLimits` could not be read."
    return "`exchangeInfo`'s `rateLimits`: " + "; ".join(parts) + "."


def weights_section():
    log("weights: every spot route, over distinct URLs")
    lines = [
        "### REST weights",
        "",
        f"Measured {utc_date()} on `api.binance.com`: the rise in `X-MBX-USED-WEIGHT-1M` across "
        "back-to-back requests inside one UTC minute, after a `time` call for the baseline. Each "
        "row is measured over distinct URLs (symbols and limits vary within a row), so a cached "
        "answer cannot stand in for a repeat. `Result` compares every URL with the design "
        "spec's section 3.1. `docs/specs/binance/probes/probe_rest.py --product spot --section "
        "weights` re-runs it.",
        "",
        "| Row | Requests | Status | Measured | Spec §3.1 | Result |",
        "|---|---|---|---|---|---|",
    ]
    limits = None
    for label, expected, paths in SPOT_ROWS:
        probes = measure(paths)
        weights = [p.weight for p in probes]
        verdict = weight_verdict(weights, expected)
        if any(p.status != 200 for p in probes):
            verdict = "REFUSED"
        if label == "exchangeInfo, without symbol":
            limits = rate_limits_sentence(probes[0].body)
        log(f"  {label}: {weights} (spec {expected}) {verdict}")
        requests = "; ".join(f"`{p.path}`" for p in probes)
        statuses = ", ".join(str(p.status) for p in probes)
        measured = ", ".join("?" if w is None else str(w) for w in weights)
        lines.append(f"| {label} | {requests} | {statuses} | {measured} | {expected} | {verdict} |")
    if limits:
        lines += ["", limits]
    return lines


def rows(body):
    return len(body)


def levels(body):
    return len(body["bids"]), len(body["asks"])


def code(body):
    return body["code"]


def limits_section():
    log("limits: past a route's maximum, depth at any limit, a two-hour window, an unknown symbol")
    start = int(time.time() * 1000) - 3 * 3600 * 1000
    end = start + 2 * 3600 * 1000
    # (row, path, the status the spec expects, how to read the answer, what it reads, how to show it)
    checks: list[tuple[str, str, int, Callable, object, Callable]] = [
        ("klines, limit 1001", "/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=1001", 200,
         rows, 1000, lambda g: f"{g} rows"),
        ("trades, limit 1001", "/api/v3/trades?symbol=BTCUSDT&limit=1001", 200,
         rows, 1000, lambda g: f"{g} rows"),
        ("aggTrades, limit 1001", "/api/v3/aggTrades?symbol=BTCUSDT&limit=1001", 200,
         rows, 1000, lambda g: f"{g} rows"),
        ("depth, limit 5001", "/api/v3/depth?symbol=BTCUSDT&limit=5001", 200,
         levels, (5000, 5000), lambda g: f"{g[0]} bids, {g[1]} asks"),
        ("depth, limit 7", "/api/v3/depth?symbol=ETHUSDT&limit=7", 200,
         levels, (7, 7), lambda g: f"{g[0]} bids, {g[1]} asks"),
        ("aggTrades, a two-hour window",
         f"/api/v3/aggTrades?symbol=BTCUSDT&startTime={start}&endTime={end}", 200,
         rows, 500, lambda g: f"{g} rows"),
        ("depth, unknown symbol", "/api/v3/depth?symbol=NOPEUSDT", 400,
         code, -1121, lambda g: f"code {g}"),
    ]
    lines = [
        "### Limits past a route's maximum",
        "",
        f"Measured {utc_date()} on `api.binance.com`. `Weight` is the rise in `X-MBX-USED-WEIGHT-1M` "
        "over a `time` call before it; a refused request still shows what it cost. "
        "`docs/specs/binance/probes/probe_rest.py --product spot --section limits` re-runs it.",
        "",
        "| Request | Status | Returned | Weight | Spec §3.1 says | Result |",
        "|---|---|---|---|---|---|",
    ]
    for label, path, want_status, read, want, show in checks:
        (probe,) = measure([path])
        try:
            got = read(json_of(probe.body))
        except (KeyError, TypeError):
            got = None
        agrees = probe.status == want_status and got == want
        returned = "unreadable" if got is None else show(got)
        weight = "?" if probe.weight is None else str(probe.weight)
        log(f"  {label}: status {probe.status}, {returned}, weight {weight}")
        lines.append(f"| `{path}` | {probe.status} | {returned} | {weight} | "
                     f"{want_status}, {show(want)} | {'agrees' if agrees else 'DIFFERS'} |")
    return lines


def window_section():
    log("window: a ping every 3 s across a minute boundary (about 70 s)")
    time.sleep((48 - time.time() % 60) % 60)
    samples = []
    for _ in range(22):
        began = time.time()
        status, headers, _ = call("/api/v3/ping", SPOT_BASE)
        guard(status, "/api/v3/ping")
        samples.append((began, used_of(headers)))
        time.sleep(max(0.0, 3 - (time.time() - began)))
    _, sentence = window_verdict(samples)
    log(f"  {sentence}")
    lines = [
        "### The weight window",
        "",
        f"Measured {utc_date()} on `api.binance.com`: a `ping` every 3 s from 12 s before a UTC "
        f"minute boundary, {len(samples)} readings of `X-MBX-USED-WEIGHT-1M`. "
        "A sliding 60-second window would not fall to the weight of the last few calls at the "
        "boundary. `docs/specs/binance/probes/probe_rest.py --product spot --section window` "
        "re-runs it.",
        "",
        FENCE + "text",
    ]
    lines += [f"{clock(t)} used={u}" for t, u in samples]
    lines += [FENCE, "", sentence]
    return lines


def counter_section():
    log("counter: alternating api.binance.com and data-api.binance.vision")
    wait_for_room(30)
    sequence = []
    for symbol in COUNTER_SYMBOLS:
        sequence.append(("api.binance.com", SPOT_BASE, 20, f"/api/v3/exchangeInfo?symbol={symbol}"))
        sequence.append(("data-api.binance.vision", VISION_BASE, 2, f"/api/v3/ticker/price?symbol={symbol}"))
    calls, shown = [], []
    for index, (host, base, weight, path) in enumerate(sequence):
        status, headers, _ = call(path, base)
        guard(status, path)
        used = used_of(headers)
        calls.append((host, weight, used))
        shared = "-" if index < 1 or calls[index - 1][2] is None else str(calls[index - 1][2] + weight)
        own = "-" if index < 2 or calls[index - 2][2] is None else str(calls[index - 2][2] + weight)
        shown.append(f"| {index + 1} | `{host}` | `{path}` | {weight} | {used} | {shared} | {own} |")
        log(f"  {host} {path}: status {status}, reading {used}")
    _, sentence = counter_verdict(calls)
    lines = [
        "### One counter across hosts",
        "",
        f"Measured {utc_date()}: alternating calls on `api.binance.com` (`exchangeInfo?symbol=`, "
        "weight 20) and `data-api.binance.vision` (`ticker/price?symbol=`, weight 2), each "
        "read against the other host's last value, one symbol per pair so no URL repeats. If "
        "the counter is shared, a reading is the other host's last reading plus its own call's "
        "weight (`If shared`); if not, it is the same host's last reading plus its own "
        "(`If separate`). `docs/specs/binance/probes/probe_rest.py --product spot --section counter` "
        "re-runs it.",
        "",
        "| # | Host | Request | Weight | Reading | If shared | If separate |",
        "|---|---|---|---|---|---|---|",
    ]
    lines += shown
    lines += ["", sentence]
    return lines


SPOT_SECTIONS = {
    "weights": weights_section,
    "limits": limits_section,
    "window": window_section,
    "counter": counter_section,
}


def spot_main(section):
    sys.stdout.reconfigure(encoding="utf-8")
    names = [section] if section else list(SPOT_SECTIONS)
    out = []
    for name in names:
        out += SPOT_SECTIONS[name]() + [""]
    print("\n".join(out))
    flagged = sum(1 for line in out if line.endswith(("| DIFFERS |", "| REFUSED |")))
    if flagged:
        log(f"{flagged} row(s) DIFFER from the design spec or were refused: say so in OBSERVED.md "
            "and tell the owner before the spot client is written.")


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--product", choices=("usdm", "spot"), default="usdm",
                        help="the host to measure (default: usdm)")
    parser.add_argument("--section", choices=tuple(SPOT_SECTIONS),
                        help="with --product spot, run one section (default: all four)")
    args = parser.parse_args(argv)
    if args.section and args.product != "spot":
        parser.error("--section needs --product spot")
    return args


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.product == "spot":
        spot_main(args.section)
    else:
        usdm_main()


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: it prints `ok`.

Then run `python3 docs/specs/binance/probes/probe_rest.py --help`.

Expected: the usage text with `--product {usdm,spot}` and `--section {weights,limits,window,counter}`, and no request sent.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(specs): probe_rest.py takes --product usdm|spot and measures spot (binance spot, Story 1)" -- docs/specs/binance/probes/probe_rest.py
```

### Task 1.2: `scripts/capture_binance_spot.py rest`

**Files:**
- Create: `scripts/capture_binance_spot.py`

- [ ] **Step 1: Write the failing check**

The picking and trimming functions are checked offline, on made-up exchange data. Run this from the repo root; it is not kept in the tree, and its rows are never written as a fixture.

```bash
nix develop --command python3 - <<'PY'
import sys

sys.path.insert(0, "scripts")
import capture_binance_spot as c


def sym(name, status, *types):
    return {"symbol": name, "status": status, "filters": [{"filterType": t} for t in types]}


info = {"symbols": [
    sym("AAAUSDT", "TRADING", "PRICE_FILTER"),
    sym("BTCUSDT", "TRADING", "PRICE_FILTER", "LOT_SIZE"),
    sym("OLDUSDT", "BREAK", "PRICE_FILTER"),
    sym("币安人生USDT", "TRADING", "PRICE_FILTER", "NOTIONAL"),
    sym("ODDUSDT", "TRADING", "PRICE_FILTER", "ICEBERG_PARTS"),
    sym("ODD2USDT", "TRADING", "ICEBERG_PARTS", "MAX_POSITION"),
    sym("PLAINUSDT", "TRADING", "PRICE_FILTER", "LOT_SIZE"),
]}

# BTCUSDT, the first symbol not TRADING, the first non-ASCII one, then one symbol per
# filter type none of those holds, in listing order, until every type is held.
picked = c.pick_symbols(info)
assert [row["symbol"] for row, _ in picked] == [
    "BTCUSDT", "OLDUSDT", "币安人生USDT", "ODDUSDT", "ODD2USDT"], picked
held = {f["filterType"] for row, _ in picked for f in row["filters"]}
assert held == {f["filterType"] for row in info["symbols"] for f in row["filters"]}
assert all(isinstance(why, str) and why for _, why in picked)

# No non-ASCII symbol listed: it is skipped, and the rest are unchanged.
ascii_only = {"symbols": [s for s in info["symbols"] if s["symbol"].isascii()]}
assert [row["symbol"] for row, _ in c.pick_symbols(ascii_only)] == [
    "BTCUSDT", "OLDUSDT", "ODDUSDT", "ODD2USDT"]

# Every symbol TRADING: the run ends and says so.
trading = {"symbols": [s for s in info["symbols"] if s["status"] == "TRADING"]}
try:
    c.pick_symbols(trading)
    raise AssertionError("a listing with no non-TRADING symbol must end the run")
except SystemExit as err:
    assert "not TRADING" in str(err.code), err.code

# A list response is trimmed to the wanted symbols, in the response's order.
rows = [{"symbol": "BTCUSDT"}, {"symbol": "ZZZUSDT"}, {"symbol": "币安人生USDT"}, {"symbol": "ETHUSDT"}]
kept = c.keep_symbols(rows, c.wanted_symbols("币安人生USDT"), "prices.json")
assert [r["symbol"] for r in kept] == ["BTCUSDT", "币安人生USDT", "ETHUSDT"]
kept = c.keep_symbols(rows, c.wanted_symbols(None), "prices.json")
assert [r["symbol"] for r in kept] == ["BTCUSDT", "ETHUSDT"]
try:
    c.keep_symbols([{"symbol": "BTCUSDT"}], c.wanted_symbols(None), "prices.json")
    raise AssertionError("a response without ETHUSDT must end the run")
except SystemExit as err:
    assert "ETHUSDT" in str(err.code), err.code

# PROVENANCE.md's REST section.
text = c.provenance_section("2026-11-02", [["`time.json`", "`GET /api/v3/time`", "as fetched"]])
assert text.startswith("## REST (rest/)\n\nCaptured 2026-11-02 from `https://api.binance.com`"), text
assert "| File | Request | Trimmed to |\n|---|---|---|\n| `time.json` | `GET /api/v3/time` | as fetched |\n" in text
print("ok")
PY
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: ``ModuleNotFoundError: No module named 'capture_binance_spot'``.

- [ ] **Step 3: Implement**

Create `scripts/capture_binance_spot.py`:

```python
#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["certifi"]
# ///
"""Capture Binance spot REST responses as test fixtures for polyoxide-binance's `spot` module.

Usage: python3 scripts/capture_binance_spot.py rest [OUT_DIR]
       (or `uv run scripts/capture_binance_spot.py rest [OUT_DIR]`)

OUT_DIR defaults to polyoxide-binance/tests/fixtures/spot. Writes one JSON file per route
under OUT_DIR/rest/ and nothing else; it does not write PROVENANCE.md. It prints
PROVENANCE.md's `## REST (rest/)` section to stdout, and progress to stderr. Nothing is
written until every capture has succeeded, so a failed run leaves the fixtures as they
were. Every key is kept; only list lengths are trimmed, so a wire-agreement test sees each
field the server sends. No credentials. Costs about 180 request weight, well inside the
6000 per minute. Review the diff before committing.

Run it as a file, never `python3 -I`, which keeps scripts/ off sys.path.
"""
import argparse
import sys
import urllib.parse
from pathlib import Path

import capture_common

BASE = "https://api.binance.com"
DEFAULT_OUT = Path(__file__).resolve().parent.parent / "polyoxide-binance" / "tests" / "fixtures" / "spot"
SYMBOL = "BTCUSDT"

# (file, path, query, how the answer is trimmed), in the order PROVENANCE.md lists them.
REQUESTS = [
    ("time.json", "/api/v3/time", {}, None),
    ("exchange_info.json", "/api/v3/exchangeInfo", {}, "exchange"),
    ("depth.json", "/api/v3/depth", {"symbol": SYMBOL, "limit": 5}, None),
    ("trades.json", "/api/v3/trades", {"symbol": SYMBOL, "limit": 3}, None),
    ("agg_trades.json", "/api/v3/aggTrades", {"symbol": SYMBOL, "limit": 3}, None),
    ("klines.json", "/api/v3/klines", {"symbol": SYMBOL, "interval": "1m", "limit": 2}, None),
    ("klines_1s.json", "/api/v3/klines", {"symbol": SYMBOL, "interval": "1s", "limit": 2}, None),
    ("avg_price.json", "/api/v3/avgPrice", {"symbol": SYMBOL}, None),
    ("ticker_24hr.json", "/api/v3/ticker/24hr", {"symbol": SYMBOL}, None),
    ("tickers_24hr.json", "/api/v3/ticker/24hr", {}, "symbols"),
    ("price.json", "/api/v3/ticker/price", {"symbol": SYMBOL}, None),
    ("prices.json", "/api/v3/ticker/price", {}, "symbols"),
    ("book_ticker.json", "/api/v3/ticker/bookTicker", {"symbol": SYMBOL}, None),
    ("book_tickers.json", "/api/v3/ticker/bookTicker", {}, "symbols"),
]
# The one refused request: an unknown symbol, answered 400 with the venue's error body.
INVALID = ("error_invalid_symbol.json", "/api/v3/exchangeInfo", {"symbol": "NOPEUSDT"})


def log(text):
    print(text, file=sys.stderr, flush=True)


def identity(_method, _url):
    """Asks for an uncompressed body."""
    return {"Accept-Encoding": "identity"}


def reply(path, **params):
    return capture_common.get(f"{BASE}{path}", params=params, headers=identity, pause=0.2, timeout=60)


def get(path, **params):
    return capture_common.require_ok(reply(path, **params), f"GET {path}")


def first(rows, predicate, what):
    for row in rows:
        if predicate(row):
            return row
    sys.exit(f"no {what} listed; pick the fixture rows by hand")


def pick_symbols(info):
    """The `exchangeInfo` rows to keep, each with why: BTCUSDT, the first symbol whose
    status is not TRADING, the first with a non-ASCII character if one is listed, then, in
    listing order, each symbol holding a filter type none of the rows kept so far holds,
    until every filter type listed anywhere is held."""
    symbols = info["symbols"]
    picked = []

    def add(row, why):
        if not any(row is kept for kept, _ in picked):
            picked.append((row, why))

    add(first(symbols, lambda s: s["symbol"] == SYMBOL, SYMBOL), SYMBOL)
    halted = first(symbols, lambda s: s["status"] != "TRADING", "symbol whose status is not TRADING")
    add(halted, f"the first not `TRADING`, status `{halted['status']}`")
    odd = next((s for s in symbols if not s["symbol"].isascii()), None)
    if odd is not None:
        add(odd, "the first with a non-ASCII character")
    held = {f["filterType"] for row, _ in picked for f in row["filters"]}
    for row in symbols:
        types = {f["filterType"] for f in row["filters"]}
        new = types - held
        if new:
            add(row, "the first with " + ", ".join(f"`{t}`" for t in sorted(new)))
            held |= types
    return picked


def wanted_symbols(non_ascii):
    """The symbols a list response keeps: BTCUSDT, ETHUSDT and the non-ASCII symbol of
    `exchangeInfo`, if one is listed."""
    return {SYMBOL, "ETHUSDT"} | ({non_ascii} if non_ascii else set())


def keep_symbols(rows, wanted, what):
    """The rows of a list response whose symbol is wanted, in the response's order.
    BTCUSDT and ETHUSDT must be there."""
    if not isinstance(rows, list):
        sys.exit(f"{what}: expected a list, got {type(rows).__name__}")
    kept = [row for row in rows if row["symbol"] in wanted]
    held = {row["symbol"] for row in kept}
    for needed in (SYMBOL, "ETHUSDT"):
        if needed not in held:
            sys.exit(f"{what}: {needed} is missing from the response")
    return kept


def provenance_section(today, rows):
    """PROVENANCE.md's `## REST (rest/)` section, from table rows of cells."""
    lines = [
        "## REST (rest/)",
        "",
        f"Captured {today} from `{BASE}` by `scripts/capture_binance_spot.py rest`. No credentials. "
        "Every key is kept; only list lengths are trimmed.",
        "",
    ]
    lines += capture_common.provenance_table(["File", "Request", "Trimmed to"], rows, short_rule=True)
    return "\n".join(lines) + "\n"


def capture_rest():
    """Every fixture by file name, and PROVENANCE.md's table rows."""
    files, rows, non_ascii = {}, [], None
    for name, path, query, trim in REQUESTS:
        value = get(path, **query)
        request = "GET " + path + ("?" + urllib.parse.urlencode(query) if query else "")
        if trim == "exchange":
            picked = pick_symbols(value)
            non_ascii = next((s["symbol"] for s in value["symbols"] if not s["symbol"].isascii()), None)
            value = {**value, "symbols": [row for row, _ in picked]}
            trimmed = "`symbols`: " + ", ".join(f"`{row['symbol']}` ({why})" for row, why in picked)
        elif trim == "symbols":
            value = keep_symbols(value, wanted_symbols(non_ascii), name)
            trimmed = ", ".join(f"`{row['symbol']}`" for row in value)
        else:
            trimmed = "as fetched"
        files[name] = value
        rows.append([f"`{name}`", f"`{request}`", trimmed])
        log(f"  {request}")
    name, path, query = INVALID
    answer = reply(path, **query)
    if answer.status != 400 or not answer.is_json or not isinstance(answer.body, dict):
        sys.exit(f"GET {path}?symbol={query['symbol']}: expected a 400 with a JSON object, "
                 f"got HTTP {answer.status} from {answer.url}")
    files[name] = answer.body
    rows.append([
        f"`{name}`",
        f"`GET {path}?symbol={query['symbol']}`",
        f"the body of the `{answer.status}` reply (`code` {answer.body.get('code')}), as fetched",
    ])
    log(f"  GET {path}?symbol={query['symbol']} -> {answer.status}")
    return files, rows


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    rest = commands.add_parser("rest", help="capture the REST fixtures under OUT_DIR/rest/")
    rest.add_argument("out", nargs="?", type=Path, default=DEFAULT_OUT,
                      help="the fixtures directory (default: polyoxide-binance/tests/fixtures/spot)")
    args = parser.parse_args(argv)

    log(f"capturing from {BASE}")
    files, rows = capture_rest()
    for name, value in files.items():
        capture_common.write_json(args.out / "rest" / name, value, indent=1)
    log(f"wrote {len(files)} files under {args.out / 'rest'}")
    sys.stdout.reconfigure(encoding="utf-8")
    print(provenance_section(capture_common.stamp("%Y-%m-%d"), rows), end="")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: it prints `ok`.

Then run `python3 scripts/capture_binance_spot.py --help` and `python3 scripts/capture_binance_spot.py rest --help`.

Expected: both print usage; neither sends a request. Running it with no subcommand exits `2` with `the following arguments are required: command`.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/capture_binance_spot.py
git commit -m "test(binance): scripts/capture_binance_spot.py captures spot's REST fixtures (binance spot, Story 1)" -- scripts/capture_binance_spot.py
```

### Task 1.3: Run the capture: the REST fixtures and `PROVENANCE.md`

**Files:**
- Create: `polyoxide-binance/tests/fixtures/spot/rest/time.json`, `polyoxide-binance/tests/fixtures/spot/rest/exchange_info.json`, `polyoxide-binance/tests/fixtures/spot/rest/depth.json`, `polyoxide-binance/tests/fixtures/spot/rest/trades.json`, `polyoxide-binance/tests/fixtures/spot/rest/agg_trades.json`, `polyoxide-binance/tests/fixtures/spot/rest/klines.json`, `polyoxide-binance/tests/fixtures/spot/rest/klines_1s.json`, `polyoxide-binance/tests/fixtures/spot/rest/avg_price.json`, `polyoxide-binance/tests/fixtures/spot/rest/ticker_24hr.json`, `polyoxide-binance/tests/fixtures/spot/rest/tickers_24hr.json`, `polyoxide-binance/tests/fixtures/spot/rest/price.json`, `polyoxide-binance/tests/fixtures/spot/rest/prices.json`, `polyoxide-binance/tests/fixtures/spot/rest/book_ticker.json`, `polyoxide-binance/tests/fixtures/spot/rest/book_tickers.json`, `polyoxide-binance/tests/fixtures/spot/rest/error_invalid_symbol.json`
- Create: `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`

- [ ] **Step 1: Write the failing check**

The check holds the captured files to the shapes the later stories decode. Run it from the repo root; it is not kept in the tree.

```bash
nix develop --command python3 - <<'PY'
import json
from pathlib import Path

root = Path("polyoxide-binance/tests/fixtures/spot")
rest = root / "rest"
names = ("time exchange_info depth trades agg_trades klines klines_1s avg_price ticker_24hr "
         "tickers_24hr price prices book_ticker book_tickers error_invalid_symbol").split()
assert sorted(p.name for p in rest.glob("*.json")) == sorted(n + ".json" for n in names)
data = {n: json.loads((rest / f"{n}.json").read_text(encoding="utf-8")) for n in names}

info = data["exchange_info"]
assert {"timezone", "serverTime", "rateLimits", "exchangeFilters", "symbols"} <= set(info), sorted(info)
symbols = info["symbols"]
listed = [s["symbol"] for s in symbols]
assert listed[0] == "BTCUSDT" and len(set(listed)) == len(listed), listed
assert len(symbols) <= 40, len(symbols)
assert any(s["status"] != "TRADING" for s in symbols), "no non-TRADING symbol kept"
limits = {(r["rateLimitType"], r["interval"], r["intervalNum"]): r["limit"] for r in info["rateLimits"]}
assert limits[("REQUEST_WEIGHT", "MINUTE", 1)] == 6000, limits
print("symbols kept:", listed)
print("non-ASCII symbol kept:", [s for s in listed if not s.isascii()])
print("rate limits:", limits)

assert data["time"].keys() == {"serverTime"}
depth = data["depth"]
assert {"lastUpdateId", "bids", "asks"} <= set(depth) and len(depth["bids"]) == 5 and len(depth["asks"]) == 5
assert len(data["trades"]) == 3
assert {"id", "price", "qty", "quoteQty", "time", "isBuyerMaker", "isBestMatch"} <= set(data["trades"][0])
assert len(data["agg_trades"]) == 3
assert {"a", "p", "q", "f", "l", "T", "m", "M"} <= set(data["agg_trades"][0])
for name, step in (("klines", 60_000), ("klines_1s", 1_000)):
    rows = data[name]
    assert len(rows) == 2 and all(isinstance(r, list) and len(r) >= 11 for r in rows), name
    assert rows[1][0] - rows[0][0] == step, (name, rows[1][0] - rows[0][0])
assert {"mins", "price", "closeTime"} <= set(data["avg_price"])
assert data["ticker_24hr"]["symbol"] == "BTCUSDT"
assert data["price"]["symbol"] == "BTCUSDT" and "price" in data["price"]
assert data["book_ticker"]["symbol"] == "BTCUSDT" and "bidPrice" in data["book_ticker"]
for name in ("tickers_24hr", "prices", "book_tickers"):
    held = [r["symbol"] for r in data[name]]
    assert isinstance(data[name], list) and {"BTCUSDT", "ETHUSDT"} <= set(held), (name, held)
assert data["error_invalid_symbol"]["code"] == -1121, data["error_invalid_symbol"]

provenance = (root / "PROVENANCE.md").read_text(encoding="utf-8")
assert provenance.startswith("# Provenance\n\n## REST (rest/)\n"), provenance[:80]
for name in names:
    assert f"| `{name}.json` |" in provenance, name
assert "`400`" in provenance
print("ok")
PY
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: an `AssertionError` on the first assertion (the `rest/` directory does not exist, so `sorted(...)` is `[]`).

- [ ] **Step 3: Run the capture**

Run from the repo root. The first command makes the directory the shell redirects into; the second runs the capture, writes the fixtures under `rest/` and `PROVENANCE.md` from the section the script prints:

```bash
mkdir -p polyoxide-binance/tests/fixtures/spot
nix develop --command bash -c '{ printf "# Provenance\n\n"; python3 scripts/capture_binance_spot.py rest; } > polyoxide-binance/tests/fixtures/spot/PROVENANCE.md'
```

Expected: stderr lists each request and ends `wrote 15 files under .../polyoxide-binance/tests/fixtures/spot/rest`; `git status --short` shows 15 new files under `rest/` and `PROVENANCE.md`.

A script that ends with an error leaves the fixtures as they were (it writes nothing until every capture succeeded) and `PROVENANCE.md` as just its title: run it again, or stop and report if the error is a `451` or a `418`/`429` (the host is refusing this address; this is environmental, not a fault of the script). Never hand-write or edit a fixture to make the check pass; if a shape the check expects is wrong, stop and tell the supervisor which one.

Then read what was kept: `git diff --stat` is empty for tracked files, and `PROVENANCE.md` names each file with the symbols the run picked.

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: it prints the symbols kept (`BTCUSDT` first), whether a non-ASCII symbol was kept, the rate limits (`REQUEST_WEIGHT` 6000 per 1 minute among them), and `ok`.

Then run `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-binance/tests/fixtures/spot/rest/*.json')]"`.

Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/fixtures/spot
git commit -m "test(binance): spot's REST fixtures, captured from production (binance spot, Story 1)" -- polyoxide-binance/tests/fixtures/spot
```

### Task 1.4: Run the probe: `OBSERVED.md`'s `## Spot` section

**Files:**
- Modify: `docs/specs/binance/OBSERVED.md` (append a `## Spot` section after the last line, `## Streams`' last bullet)

- [ ] **Step 1: Write the failing check**

Run from the repo root; it is not kept in the tree.

```bash
nix develop --command python3 - <<'PY'
import re

text = open("docs/specs/binance/OBSERVED.md", encoding="utf-8").read()
anchor = "<!-- spot findings go above this line -->"
heads = ["## Spot", "### REST weights", "### Limits past a route's maximum",
         "### The weight window", "### One counter across hosts"]
positions = [text.index("\n" + head + "\n") for head in heads]
assert positions == sorted(positions), "the headings are out of order"
assert text.endswith("\n" + anchor + "\n"), "the anchor is not the last line"
assert text.count(anchor) == 1
spot = text[positions[0]:]
flags = re.findall(r"\| (DIFFERS|REFUSED) \|", spot)
print("rows that differ from the design spec, or were refused:", len(flags))
for phrase in ("Two counters", "NOT the UTC clock minute", "never fell", "fit neither", "Fewer than four"):
    if phrase in spot:
        print("RECORDED:", phrase)
print("ok")
PY
```

- [ ] **Step 2: Run it to see it fail**

Run the command above.

Expected: a `ValueError: substring not found` on `"\n## Spot\n"`.

- [ ] **Step 3: Run the probe and write the section**

Run the four sections one at a time (they share the host's counter, so never in parallel; the whole takes several minutes, and `weights` and `window` wait for a UTC minute boundary), each into a file outside the tree:

```bash
mkdir -p /tmp/polyoxide-spot-probe
nix develop --command python3 docs/specs/binance/probes/probe_rest.py --product spot --section weights > /tmp/polyoxide-spot-probe/weights.md
nix develop --command python3 docs/specs/binance/probes/probe_rest.py --product spot --section limits > /tmp/polyoxide-spot-probe/limits.md
nix develop --command python3 docs/specs/binance/probes/probe_rest.py --product spot --section window > /tmp/polyoxide-spot-probe/window.md
nix develop --command python3 docs/specs/binance/probes/probe_rest.py --product spot --section counter > /tmp/polyoxide-spot-probe/counter.md
```

Read each file. Run `grep -nE 'DIFFERS|REFUSED|never fell|NOT the UTC|Two counters|fit neither|Fewer than four' /tmp/polyoxide-spot-probe/*.md`.

- A line saying `never fell`, `fit neither` or `Fewer than four` is an inconclusive run (other traffic from this address, or no boundary crossed): run that section again, to the same file, until it is conclusive.
- A `?` in a `Measured` or `Weight` cell is a minute rollover inside the row: run the section again.
- A `REFUSED` row is a status other than 200 on a route that should answer: if it is `symbolStatus` or `type` on an "all" row, the host does not accept that parameter; keep the row as measured, since its status column says so, and note it in the next bullet. Anything else: stop and report.
- A `DIFFERS` row, `Two counters` or `NOT the UTC clock minute` is a finding that changes the design. Keep it in the file exactly as printed; it is recorded below, and Step 5's report to the supervisor opens with it: Story 4 waits for the owner.

Then append the section. This writes `## Spot` with a one-paragraph introduction, the four probe sections as printed, and the anchor as the file's last line:

```bash
nix develop --command bash -c '{
printf "\n## Spot\n\n"
printf "%s\n\n" "Measured against \`api.binance.com\`, and \`data-api.binance.vision\` where a section says so, with \`docs/specs/binance/probes/probe_rest.py --product spot\`; the design is \`docs/superpowers/specs/2026-11-02-binance-spot-design.md\`. Each section names the day it was measured. A later finding goes directly above the last line of this file, under a heading of its own."
cat /tmp/polyoxide-spot-probe/weights.md /tmp/polyoxide-spot-probe/limits.md /tmp/polyoxide-spot-probe/window.md /tmp/polyoxide-spot-probe/counter.md
printf "<!-- spot findings go above this line -->\n"
} >> docs/specs/binance/OBSERVED.md'
```

If Step 3's reading found a `DIFFERS` row, a `REFUSED` row, `Two counters` or `NOT the UTC clock minute`, add one sentence by hand under that section's table (or sentence), directly above the next `###` heading, that starts `Differs from the design spec's section 3.1:` and names the row, the weight or fact the design gave, and the one measured. Do not edit a number the probe printed.

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again.

Expected: it prints the count of rows that differ (0 when the design held), any `RECORDED:` phrase, and `ok`.

Then run the story's Verify command: `python3 -c "import glob, json; [json.load(open(f)) for f in glob.glob('polyoxide-binance/tests/fixtures/spot/rest/*.json')]" && grep -qx '<!-- spot findings go above this line -->' docs/specs/binance/OBSERVED.md && python3 docs/specs/binance/probes/probe_rest.py --help && python3 scripts/capture_binance_spot.py --help`.

Expected: it exits `0` and prints the two usage texts.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(specs): OBSERVED.md's Spot section, measured with probe_rest.py --product spot (binance spot, Story 1)" -- docs/specs/binance/OBSERVED.md
```

Report to the supervisor: the count and names of any `DIFFERS` or `REFUSED` rows, the window verdict, and the counter verdict, whichever way they came out.

## Story 2: Shared REST plumbing under `usdm`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/shared/mod.rs`, `polyoxide-binance/src/shared/weight.rs`, `polyoxide-binance/src/shared/policy.rs`, `polyoxide-binance/src/shared/error.rs`, `polyoxide-binance/src/shared/request.rs`, `polyoxide-binance/src/shared/symbol.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/src/usdm/policy.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/types.rs`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/clients.md`
- **Interfaces:** exposes "Story 2 → Stories 4 and 5"; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_mutants_ledger.py -q'`
- **Review:** deep (a refactor under a public API and two mutation-tested rows; `RESERVED_FRACTION`)

Moves the minute-weight engine, the retry decision, the error decode, the routed request with its macro, and `Symbol` out of `usdm` into a crate-private `src/shared/`, and rebuilds `usdm`'s weight budget, policy, error, request and types on them, one concern per task so each commit builds and passes `usdm`'s own tests. Behaviour is unchanged line for line: `usdm`'s public paths stay through `pub use`, and no test file and no test module changes, so `python3 scripts/test_body_diff.py --baseline v0.40.0` stays clean. The moved code holds the landmine marker `RESERVED_FRACTION` (the reserved tenth, which must stay 10) and the `Retry-After` floor and `Hold` extension the profile's notes describe: they move, never change, so the supervisor should dispatch this story's developer with `effort: xhigh`. Task 2.4 re-cites rows (a), "Binance's policy", and (k) of `docs/MUTANTS.md` at `shared/policy.rs`, replaces the three `usdm/policy.rs` entries of `.github/scripts/tests/test_mutants_ledger.py` that pin their citations, and proves each mutant again. The three rules files whose `paths:` globs name the emptied `usdm` files (`rate-limits.md`, `send-loop.md`, `clients.md`) gain a list entry for each `shared` file in the commit that creates it, so a session editing the moved code still loads them; no rule's body changes. The new tests are only for what `usdm`'s tests do not reach: an engine with no paced layer, a charge and an observe on a layer the caller names, and a `418`'s whole body. One consequence of the move the contract does not spell out: once `Throttle` goes through `Engine::charge` and `Engine::observe`, `WeightBudget::acquire` and `WeightBudget::record_used` are called by the unit tests alone, so they take `#[cfg(test)]` (and `MinuteCharge` is imported under it) rather than leaving dead code for `-D warnings`; their names and signatures are today's, and the test-body gate lists them as new test-context items, not changed ones.

### Task 2.1: `Symbol` moves to `shared::symbol`

No new test: the move changes no behaviour, and `usdm::types`'s own `Symbol` tests, which stay where they are, are the guard.

**Files:**
- Create: `polyoxide-binance/src/shared/mod.rs`
- Create: `polyoxide-binance/src/shared/symbol.rs`
- Modify: `polyoxide-binance/src/lib.rs:23-24`
- Modify: `polyoxide-binance/src/usdm/types.rs:9-89`
- Test: `polyoxide-binance/src/usdm/types.rs` (its `mod tests`, untouched)

- [ ] **Step 1: Run the guard tests before the move**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib usdm::types
```

Expected: `test result: ok.`, including `a_refused_symbol_and_an_unknown_spelling_are_invalid_requests`, `symbols_take_letters_digits_and_underscores_in_any_script`, `ascii_letters_are_uppercased_and_nothing_else_changes` and `symbols_are_measured_in_characters_and_only_ascii_changes_case`.

- [ ] **Step 2: Create `shared`**

Create `polyoxide-binance/src/shared/mod.rs`:

```rust
//! Binance-wide plumbing the REST products share.
//!
//! The minute-weight engine and its cooldown, the retry decision, the error
//! decode by status, the routed request and `Symbol`: everything about
//! Binance's REST hosts that does not depend on which product's routes a
//! request names. Each product keeps its own weight table, error enum, retry
//! policy handle and response types, and builds them on these.

pub(crate) mod symbol;
```

Create `polyoxide-binance/src/shared/symbol.rs`. The code is `usdm::types`'s, moved; only the type's docs change, to name no single product's contracts:

```rust
//! The symbol a Binance request names, whichever product it is sent to.

use std::{fmt, str::FromStr};

/// A Binance symbol such as `BTCUSDT`, `BTCUSDT_261225` or `币安人生USDT`.
///
/// [`Symbol::new`] accepts 1 to 32 characters, each a Unicode letter, a digit
/// or `_`. Binance lists Chinese-character symbols, and its quarterly futures
/// carry their delivery date after an underscore; on 2026-10-07 the 924
/// USDⓈ-M symbols used no other character and the longest had 17. ASCII
/// letters are uppercased: no listed symbol has a lowercase one, stream names
/// spell every symbol in lowercase, and uppercasing is what lets a stream name
/// be parsed back to the symbol that built it.
///
/// Response rows carry symbols as `String`, taken as sent, so a symbol this
/// type would refuse never fails a whole response.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(String);

/// A string [`Symbol::new`] refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a Binance symbol: 1 to 32 letters, digits or underscores")]
pub struct InvalidSymbol(pub String);

/// An `InvalidRequest`: the caller named a symbol no request can carry.
impl polyoxide_venue::Classify for InvalidSymbol {
    fn class(&self) -> polyoxide_venue::Class {
        polyoxide_venue::Class::InvalidRequest
    }
}

impl Symbol {
    /// The longest symbol [`Symbol::new`] accepts, in characters.
    pub const MAX_LEN: usize = 32;

    /// Checks a symbol and uppercases its ASCII letters.
    pub fn new(symbol: impl Into<String>) -> Result<Self, InvalidSymbol> {
        let symbol = symbol.into();
        let chars = symbol.chars().count();
        let valid = (1..=Self::MAX_LEN).contains(&chars)
            && symbol.chars().all(|c| c.is_alphanumeric() || c == '_');
        if valid {
            Ok(Self(symbol.to_ascii_uppercase()))
        } else {
            Err(InvalidSymbol(symbol))
        }
    }

    /// The symbol, as REST spells it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Symbol {
    type Err = InvalidSymbol;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
```

In `polyoxide-binance/src/lib.rs`, current (lines 23-24):

```rust
#[cfg(feature = "usdm")]
pub mod usdm;
```

becomes (Story 4 later widens the new `cfg`):

```rust
#[cfg(feature = "usdm")]
mod shared;

#[cfg(feature = "usdm")]
pub mod usdm;
```

In `polyoxide-binance/src/usdm/types.rs`, current (line 9):

```rust
use std::{fmt, str::FromStr};
```

becomes (`FromStr` was only `Symbol`'s; `fmt` is still used by the visitors below):

```rust
use std::fmt;
```

Then delete lines 20-89: from the doc comment `/// A Binance symbol such as `BTCUSDT`, `BTCUSDT_261225` or `币安人生USDT`.` through `impl AsRef<str> for Symbol`'s closing brace and the blank line after it, which is the line above `polyoxide_venue::wire_enum! {` opening `Interval`. In their place, directly after `use serde_json::Value;` and one blank line, put:

```rust
pub use crate::shared::symbol::{InvalidSymbol, Symbol};
```

so the file reads `use serde_json::Value;`, a blank line, that `pub use`, a blank line, then `polyoxide_venue::wire_enum! {`. `mod tests` in this file still reaches `Symbol` and `InvalidSymbol` through `use super::*`.

- [ ] **Step 3: Run the guard tests after the move, then the gates for the crate**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib usdm::types
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo fmt --all -- --check
```

Expected: the same tests pass, clippy prints no warning, fmt prints nothing. If fmt prints a diff, run `nix develop --command cargo fmt --all` and keep its output.

- [ ] **Step 4: Run the crate's whole suite**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features
```

Expected: every target `test result: ok.`

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/symbol.rs
git commit -m "refactor(binance): move Symbol into a crate-private shared module (binance spot, Story 2)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/symbol.rs polyoxide-binance/src/lib.rs polyoxide-binance/src/usdm/types.rs
```

### Task 2.2: the minute engine moves to `shared::weight`

`shared::weight` holds the engine, its clock, the header and the ban constants; `usdm::weight` keeps `WeightBudget`, its two layers, `Cost` and `Route`, and delegates. The engine's body is today's, line for line, with `usdm`'s two layers as parameters: the reserved tenth (`RESERVED_FRACTION = 10`), the `Retry-After`-fed `Hold` with its 3-day ceiling, which only extends, and the header applied only to the minute a request was charged in all move as they are. The unit tests that stay in `usdm/weight.rs` are the guard; the new `shared` tests cover what they do not: an engine with no paced layer, and `charge` and `observe` on layer names the caller picks (spot's will differ from `usdm`'s).

**Files:**
- Create: `polyoxide-binance/src/shared/weight.rs`
- Modify: `polyoxide-binance/src/shared/mod.rs`
- Modify: `polyoxide-binance/src/usdm/weight.rs:1-503`
- Modify: `polyoxide-binance/src/usdm/request.rs:14-15`
- Modify: `.claude/rules/rate-limits.md:5`
- Test: `polyoxide-binance/src/shared/weight.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/src/shared/mod.rs`, add after `pub(crate) mod symbol;`:

```rust
pub(crate) mod weight;
```

Create `polyoxide-binance/src/shared/weight.rs` holding only this test module (Step 3 puts the implementation above it):

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::header::{HeaderMap, HeaderValue};
    use polyoxide_core::reqwest::{Method, StatusCode};

    use super::*;

    /// 2026-10-07 08:35:00 UTC, a minute boundary: minute 29 856 035.
    const ON_A_MINUTE: u64 = 1_791_362_100_000;
    const WEIGHT: LayerId = LayerId("test-weight");
    const PACED: LayerId = LayerId("test-paced");
    const FIVE_MINUTES: Duration = Duration::from_secs(300);

    fn new_engine(paced: Option<(u32, Duration)>) -> Engine {
        Engine::new(
            2400,
            paced,
            Clock::Tokio {
                start: Instant::now(),
                start_ms: ON_A_MINUTE,
            },
        )
    }

    fn cost(layer: LayerId, units: u32) -> polyoxide_core::Cost {
        polyoxide_core::Cost {
            layer,
            units,
            exact: true,
        }
    }

    /// `engine` charging a request that carries `costs`, with `WEIGHT` as its
    /// weight layer and `paced` as its paced one.
    async fn charge(
        engine: &Engine,
        costs: &[polyoxide_core::Cost],
        paced: Option<LayerId>,
    ) -> Charge {
        let method = Method::GET;
        let meta = RequestMeta {
            method: &method,
            path: "/probe",
            query: &[],
            costs,
        };
        engine.charge(&meta, WEIGHT, paced).await
    }

    /// `engine` seeing a response that reports `used` in the weight header,
    /// for the weight layer `layer`.
    fn observe(engine: &Engine, charge: &Charge, used: &str, layer: LayerId) {
        let mut headers = HeaderMap::new();
        headers.insert(USED_WEIGHT_HEADER, HeaderValue::from_str(used).unwrap());
        engine.observe(
            charge,
            &ResponseMeta {
                status: StatusCode::OK,
                headers: &headers,
            },
            layer,
        );
    }

    #[test]
    fn the_budget_is_the_published_limit_less_a_tenth() {
        // 2400 is the futures host's published limit and 6000 the spot
        // host's; the reserve is the same tenth of each.
        for (published, budget) in [(2400, 2160), (6000, 5400)] {
            let engine = Engine::new(published, None, Clock::System);
            assert_eq!(engine.per_minute(), budget, "{published}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_charge_carries_the_layer_it_was_given_its_weight_and_its_minute() {
        let engine = new_engine(None);
        let charged = charge(&engine, &[cost(WEIGHT, 7)], None).await;
        assert_eq!(
            charged,
            Charge::none().with(LayerCharge {
                layer: WEIGHT,
                units: 7,
                window: Some(29_856_035),
            })
        );
        assert_eq!(engine.used(), 7);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cost_on_another_layer_charges_nothing_and_still_waits_out_a_cooldown() {
        let engine = new_engine(None);
        engine.begin_cooldown(Duration::from_secs(3));
        let start = Instant::now();
        let charged = charge(&engine, &[cost(LayerId("other-weight"), 5)], None).await;
        assert_eq!(charged, Charge::none());
        assert_eq!(start.elapsed(), Duration::from_secs(3));
        assert_eq!(engine.used(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_paced_cost_takes_one_unit_and_no_minute() {
        let paced = new_engine(Some((500, FIVE_MINUTES)));
        let charged = charge(&paced, &[cost(PACED, 99)], Some(PACED)).await;
        assert_eq!(
            charged,
            Charge::none().with(LayerCharge {
                layer: PACED,
                units: 1,
                window: None,
            })
        );
        assert_eq!(paced.used(), 0, "a paced request carries no weight");

        // The same cost is nothing to a request that names no paced layer.
        let unpaced = new_engine(None);
        let ignored = charge(&unpaced, &[cost(PACED, 99)], None).await;
        assert_eq!(ignored, Charge::none());
    }

    #[tokio::test(start_paused = true)]
    async fn an_engine_with_no_paced_layer_waits_out_a_cooldown_and_admits() {
        let engine = new_engine(None);
        engine.begin_cooldown(Duration::from_secs(10));
        let start = Instant::now();
        let charged = engine.acquire_paced().await;
        assert_eq!(start.elapsed(), Duration::from_secs(10));
        assert_eq!(charged, MinuteCharge { minute: None });
    }

    #[tokio::test(start_paused = true)]
    async fn a_header_raises_the_layer_the_caller_names_and_no_other() {
        let engine = new_engine(None);
        let charged = charge(&engine, &[cost(WEIGHT, 1)], None).await;

        observe(&engine, &charged, "100", LayerId("other-weight"));
        assert_eq!(engine.used(), 1, "the charge holds no such layer");

        observe(&engine, &charged, "100", WEIGHT);
        assert_eq!(engine.used(), 100);

        observe(&engine, &charged, "soon", WEIGHT);
        assert_eq!(engine.used(), 100, "not a count");

        observe(&engine, &charged, " 150 ", WEIGHT);
        assert_eq!(engine.used(), 150, "whitespace around the count is ignored");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib shared::weight
```

Expected: the crate's tests do not compile: `error[E0412]: cannot find type `Engine` in this scope`, and likewise `Clock`, `LayerId`, `Instant`, `Charge`, `MinuteCharge` and `USED_WEIGHT_HEADER`, ending `error: could not compile `polyoxide-binance` (lib test)`.

- [ ] **Step 3: Implement**

(a) Put this at the top of `polyoxide-binance/src/shared/weight.rs`, above the `#[cfg(test)]` line and one blank line before it. It is `usdm/weight.rs`'s engine moved: `RESERVED_FRACTION`, `MINUTE_MS`, `after_reserve`, `MinuteCharge`, `Clock`, `Inner` and `State` (as `Engine` and `State`), `try_charge`, `record_used`, `begin_cooldown`, `until_next_minute`, `await_cooldown`, `in_cooldown`, the slot reservation, `lock`, and `Throttle::acquire`'s and `observe`'s bodies, with the funding layer generalised to "a paced layer" and the two layer ids passed in. Do not change any of them.

```rust
//! The minute engine both Binance REST products charge through.
//!
//! Binance refuses an IP whose request weight in the current UTC minute passes
//! the host's published limit. The engine counts that minute at the published
//! limit less a reserved tenth, raised by `X-MBX-USED-WEIGHT-1M` for the minute
//! a request was charged in; holds every request on a client-wide cooldown for
//! a `429` or `418`, which is only ever extended; and, where a product has
//! routes that carry no weight but share a published request limit, paces
//! those. A product's `WeightBudget` is a handle to one engine with its own
//! layer names.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use polyoxide_core::{
    Charge, Hold, LayerCharge, LayerId, RequestMeta, ResponseMeta, WindowQuotaTable,
};
use tokio::time::Instant;

/// The header carrying the IP's weight used in the current minute.
pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";

/// How long a `418` holds every request when it carries no `Retry-After`: the
/// shortest ban Binance documents.
pub const DEFAULT_BAN: Duration = Duration::from_secs(120);

/// The longest a cooldown can last: the longest ban Binance documents. A
/// `Retry-After` beyond it is clamped.
pub const MAX_COOLDOWN: Duration = Duration::from_secs(3 * 24 * 60 * 60);

/// Reciprocal of the share of a published limit the client leaves unused, as
/// in core's `RESERVED_FRACTION`: aiming at a published quota is a bug, because
/// the server's count and the client's never agree exactly.
const RESERVED_FRACTION: u32 = 10;

const MINUTE_MS: u64 = 60_000;

fn after_reserve(published: u32) -> u32 {
    published - published.div_ceil(RESERVED_FRACTION)
}

/// The minute a request was charged in, so its response's header is applied to
/// that minute and no other. `None` for a paced layer, which reports no
/// weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MinuteCharge {
    pub(crate) minute: Option<u64>,
}

/// Where the current UTC minute comes from.
#[derive(Debug, Clone)]
pub(crate) enum Clock {
    /// The system clock, which is what Binance's minute follows.
    System,
    /// Wall time derived from tokio's clock, so a paused-time test controls it.
    #[cfg(test)]
    Tokio { start: Instant, start_ms: u64 },
    /// A clock a test sets by hand, to step it backwards.
    #[cfg(test)]
    Manual(std::sync::Arc<std::sync::atomic::AtomicU64>),
}

impl Clock {
    pub(crate) fn now_ms(&self) -> u64 {
        match self {
            Self::System => polyoxide_venue::UnixMillis::now().0,
            #[cfg(test)]
            Self::Tokio { start, start_ms } => *start_ms + start.elapsed().as_millis() as u64,
            #[cfg(test)]
            Self::Manual(ms) => ms.load(std::sync::atomic::Ordering::SeqCst),
        }
    }
}

/// One IP's request-weight minute, its optional paced layer and its cooldown.
#[derive(Debug)]
pub(crate) struct Engine {
    per_minute: u32,
    paced_interval: Option<Duration>,
    clock: Clock,
    hold: Hold,
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    /// The UTC minute (milliseconds / 60 000) that `used` counts.
    minute: u64,
    used: u32,
    next_paced: Option<Instant>,
}

impl Engine {
    /// An engine for a published limit of `published_per_minute` weight, less
    /// the reserved tenth. `paced` is a published request count and its
    /// period for the routes that carry no weight, paced with a depth of
    /// one; `None` for a product with no such routes.
    pub(crate) fn new(
        published_per_minute: u32,
        paced: Option<(u32, Duration)>,
        clock: Clock,
    ) -> Self {
        // One slot of the paced target is the bucket's depth; the rest is
        // paced, so no window admits more than the target. Core's one pacing
        // formula, on this engine's own clock.
        let paced_interval =
            paced.map(|(count, period)| WindowQuotaTable::paced_interval(count, period));
        Self {
            per_minute: after_reserve(published_per_minute),
            paced_interval,
            clock,
            hold: Hold::with_ceiling(MAX_COOLDOWN),
            state: Mutex::new(State {
                minute: 0,
                used: 0,
                next_paced: None,
            }),
        }
    }

    /// The most weight this engine spends in one minute.
    pub(crate) fn per_minute(&self) -> u32 {
        self.per_minute
    }

    /// Weight charged or reported in the current UTC minute.
    pub(crate) fn used(&self) -> u32 {
        let minute = self.clock.now_ms() / MINUTE_MS;
        let state = self.lock();
        if minute > state.minute {
            0
        } else {
            state.used
        }
    }

    /// Waits until a request of `weight` can go, then charges it.
    pub(crate) async fn acquire_weight(&self, weight: u32) -> MinuteCharge {
        loop {
            self.await_cooldown().await;
            match self.try_charge(weight) {
                Ok(charge) => return charge,
                Err(until_next_minute) => tokio::time::sleep(until_next_minute).await,
            }
        }
    }

    /// Waits for the next slot of the paced layer, then takes it. On an
    /// engine with no paced layer it waits out the cooldown and returns.
    pub(crate) async fn acquire_paced(&self) -> MinuteCharge {
        let Some(interval) = self.paced_interval else {
            self.await_cooldown().await;
            return MinuteCharge { minute: None };
        };
        loop {
            // Wait out a cooldown before taking a slot: a slot taken first
            // and slept on through a cooldown is in the past when it ends,
            // and every request parked that way would go at once.
            self.await_cooldown().await;
            let slot = self.reserve_paced_slot(interval);
            tokio::time::sleep_until(slot).await;
            if !self.in_cooldown() {
                return MinuteCharge { minute: None };
            }
            // A cooldown began while this request waited for its slot: the
            // slot is spent, and the request queues again behind it.
        }
    }

    /// Charges `weight` to the current minute, or says how long until the next.
    ///
    /// An empty minute admits any weight, so a request heavier than the whole
    /// budget is sent alone rather than held forever.
    fn try_charge(&self, weight: u32) -> Result<MinuteCharge, Duration> {
        let now_ms = self.clock.now_ms();
        let mut state = self.lock();
        // The minute only moves forward. A read that lands in an earlier minute,
        // from a stale read racing a boundary or a clock stepped back, charges
        // the newest minute seen instead of resetting its count.
        let minute = now_ms / MINUTE_MS;
        if minute > state.minute {
            state.minute = minute;
            state.used = 0;
        }
        if state.used == 0 || state.used.saturating_add(weight) <= self.per_minute {
            state.used = state.used.saturating_add(weight);
            Ok(MinuteCharge {
                minute: Some(state.minute),
            })
        } else {
            Err(Duration::from_millis(MINUTE_MS - now_ms % MINUTE_MS))
        }
    }

    /// Raises the minute's count to what the server reported for it.
    pub(crate) fn record_used(&self, charge: MinuteCharge, used: u32) {
        let Some(minute) = charge.minute else { return };
        let mut state = self.lock();
        if state.minute == minute && used > state.used {
            state.used = used;
        }
    }

    /// Holds every request for `delay`. Extends a cooldown in force, never
    /// shortens one: concurrent requests see the same `429` milliseconds
    /// apart, and taking the latest delay would release them all early.
    pub(crate) fn begin_cooldown(&self, delay: Duration) {
        self.hold.extend(delay);
    }

    /// Time to the next UTC minute on this engine's clock.
    pub(crate) fn until_next_minute(&self) -> Duration {
        let now_ms = self.clock.now_ms();
        Duration::from_millis(MINUTE_MS - now_ms % MINUTE_MS)
    }

    /// Waits out the cooldown, and any extension of it made meanwhile.
    pub(crate) async fn await_cooldown(&self) {
        self.hold.wait().await;
    }

    fn in_cooldown(&self) -> bool {
        self.hold.is_held()
    }

    fn reserve_paced_slot(&self, interval: Duration) -> Instant {
        let now = Instant::now();
        let mut state = self.lock();
        let slot = state.next_paced.map_or(now, |next| next.max(now));
        state.next_paced = Some(slot + interval);
        slot
    }

    /// A poison-tolerant lock: a panic elsewhere must not turn the budget into
    /// a permanent outage.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Charges one request on its product's layers: the cost it carries for
    /// `weight`, or else the one it carries for `paced`. A request that
    /// carries neither is charged nothing, but still waits out a cooldown.
    ///
    /// The weight layer's charge records the UTC minute it was taken in as
    /// its `window`; the paced layer's records none.
    pub(crate) async fn charge(
        &self,
        meta: &RequestMeta<'_>,
        weight: LayerId,
        paced: Option<LayerId>,
    ) -> Charge {
        let found = meta.costs.iter().find_map(|cost| {
            if cost.layer == weight {
                Some((cost.layer, Some(cost.units)))
            } else if Some(cost.layer) == paced {
                Some((cost.layer, None))
            } else {
                None
            }
        });
        let Some((layer, weighed)) = found else {
            // Nothing to charge, but a request still waits out a ban.
            self.await_cooldown().await;
            return Charge::none();
        };
        let (units, window) = match weighed {
            Some(units) => (units, self.acquire_weight(units).await.minute),
            None => (1, self.acquire_paced().await.minute),
        };
        Charge::none().with(LayerCharge {
            layer,
            units,
            window,
        })
    }

    /// Applies a response's `X-MBX-USED-WEIGHT-1M` to the minute its request
    /// was charged in on the `weight` layer, whatever the status.
    pub(crate) fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, weight: LayerId) {
        let Some(used) = used_weight(response.headers) else {
            return;
        };
        for layer in charge.layers() {
            if layer.layer == weight {
                self.record_used(
                    MinuteCharge {
                        minute: layer.window,
                    },
                    used,
                );
            }
        }
    }
}

/// The IP's weight used this minute, as a response reports it.
fn used_weight(headers: &polyoxide_core::reqwest::header::HeaderMap) -> Option<u32> {
    headers
        .get(USED_WEIGHT_HEADER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}
```

(b) Replace `polyoxide-binance/src/usdm/weight.rs` lines 1-503, everything above `#[cfg(test)]` (whose `mod tests` starts at line 505), with the text below. It keeps the constants, `Cost`, `Route` and their docs exactly as they are, and the three `#[cfg(test)]` hooks byte for byte; `WeightBudget` becomes a handle to the engine. Leave `mod tests` and its imports untouched:

```rust
//! Binance's request-weight budget, and the separate limit on the funding routes.
//!
//! Binance charges each REST request a *weight* that depends on its route and
//! its parameters, and refuses an IP whose weight in the current minute passes
//! `exchangeInfo`'s `REQUEST_WEIGHT` limit of 2400. Core's `RateLimiter` counts
//! requests per path, so it cannot model a cost that varies with `limit`;
//! [`WeightBudget`] does. The table is [`Route::cost`]; the measurements behind
//! it are in `docs/specs/binance/OBSERVED.md`.

use std::sync::Arc;
use std::time::Duration;

use polyoxide_core::{AttemptInfo, Charge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle};

use crate::shared::weight::{Clock, Engine};
use crate::usdm::types::DepthLimit;

#[cfg(test)]
use crate::shared::weight::MinuteCharge;
#[cfg(test)]
use tokio::time::Instant;

/// `exchangeInfo`'s `REQUEST_WEIGHT` limit per minute, per IP.
pub const PUBLISHED_WEIGHT_PER_MINUTE: u32 = 2400;

/// Requests per IP every five minutes that `fundingRate` and `fundingInfo`
/// share, as documented. Neither route reports a weight.
pub const PUBLISHED_FUNDING_PER_FIVE_MINUTES: u32 = 500;

pub use crate::shared::weight::{DEFAULT_BAN, MAX_COOLDOWN};

const FUNDING_PERIOD: Duration = Duration::from_secs(300);

/// The per-minute request-weight layer of a [`WeightBudget`], as a request's
/// [`polyoxide_core::Cost`] names it.
pub const WEIGHT_LAYER: LayerId = LayerId("binance-weight");

/// The funding routes' own layer of a [`WeightBudget`], as a request's
/// [`polyoxide_core::Cost`] names it.
pub const FUNDING_LAYER: LayerId = LayerId("binance-funding");

/// What one request costs, and which limit it draws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Request weight, against the per-minute budget.
    Weight(u32),
    /// One request against the funding routes' own limit.
    Funding,
}

/// The cost a request builder hands core's send loop: its weight against
/// [`WEIGHT_LAYER`], or one request against [`FUNDING_LAYER`]. Both are exact.
impl From<Cost> for polyoxide_core::Cost {
    fn from(cost: Cost) -> Self {
        let (layer, units) = match cost {
            Cost::Weight(weight) => (WEIGHT_LAYER, weight),
            Cost::Funding => (FUNDING_LAYER, 1),
        };
        Self {
            layer,
            units,
            exact: true,
        }
    }
}

/// A REST route, with the parameters its weight depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Route {
    /// `GET /fapi/v1/ping`.
    Ping,
    /// `GET /fapi/v1/time`.
    Time,
    /// `GET /fapi/v1/exchangeInfo`.
    ExchangeInfo,
    /// `GET /fapi/v1/fundingInfo`.
    FundingInfo,
    /// `GET /fapi/v1/ticker/24hr`: one symbol, or every symbol when `all`.
    Ticker24h {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /fapi/v1/premiumIndex`: one symbol, or every symbol when `all`.
    PremiumIndex {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /fapi/v1/klines`.
    Klines {
        /// The `limit` parameter, if sent.
        limit: Option<u32>,
    },
    /// `GET /fapi/v1/fundingRate`.
    FundingRate,
    /// `GET /fapi/v1/openInterest`.
    OpenInterest,
    /// `GET /fapi/v1/aggTrades`.
    AggTrades,
    /// `GET /fapi/v1/depth`.
    Depth {
        /// The `limit` parameter, if sent.
        limit: Option<DepthLimit>,
    },
}

impl Route {
    /// The route's path on `fapi.binance.com`.
    pub fn path(self) -> &'static str {
        match self {
            Self::Ping => "/fapi/v1/ping",
            Self::Time => "/fapi/v1/time",
            Self::ExchangeInfo => "/fapi/v1/exchangeInfo",
            Self::FundingInfo => "/fapi/v1/fundingInfo",
            Self::Ticker24h { .. } => "/fapi/v1/ticker/24hr",
            Self::PremiumIndex { .. } => "/fapi/v1/premiumIndex",
            Self::Klines { .. } => "/fapi/v1/klines",
            Self::FundingRate => "/fapi/v1/fundingRate",
            Self::OpenInterest => "/fapi/v1/openInterest",
            Self::AggTrades => "/fapi/v1/aggTrades",
            Self::Depth { .. } => "/fapi/v1/depth",
        }
    }

    /// What the request costs, as measured from `X-MBX-USED-WEIGHT-1M` deltas
    /// on 2026-10-07.
    ///
    /// Two rows differ from Binance's page. `klines` bands are inclusive at the
    /// top (a limit of 100 costs 1, 500 costs 2, 1000 costs 5), and omitting
    /// `limit` costs 5 although it returns 500 rows. `depth` without `limit`
    /// costs 1 and returns 500 levels, where an explicit 500 costs 10.
    pub fn cost(self) -> Cost {
        match self {
            Self::Ping | Self::Time | Self::ExchangeInfo | Self::OpenInterest => Cost::Weight(1),
            Self::FundingInfo | Self::FundingRate => Cost::Funding,
            Self::Ticker24h { all } => Cost::Weight(if all { 40 } else { 1 }),
            Self::PremiumIndex { all } => Cost::Weight(if all { 10 } else { 1 }),
            Self::Klines { limit: None } => Cost::Weight(5),
            Self::Klines { limit: Some(limit) } => Cost::Weight(match limit {
                0..=100 => 1,
                101..=500 => 2,
                501..=1000 => 5,
                _ => 10,
            }),
            Self::AggTrades => Cost::Weight(20),
            Self::Depth { limit: None } => Cost::Weight(1),
            Self::Depth { limit: Some(limit) } => Cost::Weight(match limit {
                DepthLimit::Five | DepthLimit::Ten | DepthLimit::Twenty | DepthLimit::Fifty => 2,
                DepthLimit::Hundred => 5,
                DepthLimit::FiveHundred => 10,
                DepthLimit::Thousand => 20,
            }),
        }
    }
}

/// The request-weight budget of one IP, shared by every client that clones it.
///
/// Binance limits weight per IP, not per client, so every
/// [`Usdm`](crate::usdm::Usdm) in a process should be built with one budget
/// ([`UsdmBuilder::weight_budget`](crate::usdm::UsdmBuilder::weight_budget)).
///
/// - **Window.** The UTC clock minute: the server's count fell to 1 just after
///   each minute boundary on 2026-10-07, where a sliding window would have
///   kept counting.
/// - **Budget.** The published 2400 less a tenth, 2160. A request waits while
///   its weight would take the minute past that.
/// - **Server count.** Every response's `X-MBX-USED-WEIGHT-1M` raises the
///   minute's count to at least what the server reports, because other
///   processes on the same IP spend the same budget. The count never falls
///   within a minute, and a response is applied only to the minute its request
///   was charged in.
/// - **Funding.** `fundingRate` and `fundingInfo` carry no weight and share a
///   documented 500 requests per 5 minutes, paced here at 450 with a depth of
///   one, so they never burst, not even when a cooldown ends.
/// - **Cooldown.** A `429` or `418` holds every request, on both limits, for
///   the delay the server asked for. A cooldown is core's
///   [`Hold`](polyoxide_core::Hold), with a ceiling of [`MAX_COOLDOWN`], so it
///   is only ever extended.
/// - **Throttle.** The budget is the [`Throttle`] every [`Usdm`](crate::usdm::Usdm)
///   built with it sends through: a request's weight or funding cost is
///   charged before it is sent, and each response's `X-MBX-USED-WEIGHT-1M` is
///   applied to the minute it was charged in.
/// - **In flight.** A response's header is applied only to the minute its
///   request was charged in, so a request in flight across a minute boundary is
///   counted by the server in a minute this budget no longer sees. The reserve
///   of 240 absorbs that while the weight in flight stays under it: one `Usdm`
///   at its default of 4 concurrent requests has at most 160 in flight (4 × the
///   heaviest route, 40). A higher `max_concurrent`, or several clients sharing
///   one budget, can pass it.
#[derive(Debug, Clone)]
pub struct WeightBudget {
    inner: Arc<Engine>,
}

impl Default for WeightBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightBudget {
    /// A budget for Binance's published limits, less the reserve.
    pub fn new() -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::System,
        )
    }

    fn with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self {
        Self {
            inner: Arc::new(Engine::new(
                weight_per_minute,
                Some((funding_per_five_minutes, FUNDING_PERIOD)),
                clock,
            )),
        }
    }

    /// A budget on a clock a test sets by hand, starting at `start_ms`.
    #[cfg(test)]
    pub(crate) fn at_ms(start_ms: u64) -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::Manual(Arc::new(std::sync::atomic::AtomicU64::new(start_ms))),
        )
    }

    /// Whether two handles charge one budget.
    #[cfg(test)]
    pub(crate) fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute()
    }

    /// Weight charged or reported in the current UTC minute.
    pub fn used(&self) -> u32 {
        self.inner.used()
    }

    /// Waits until the request can go, then charges it.
    ///
    /// The send loop charges through `Throttle::acquire` and the engine's
    /// `charge`; this is the unit tests' way to charge a bare cost.
    #[cfg(test)]
    pub(crate) async fn acquire(&self, cost: Cost) -> MinuteCharge {
        match cost {
            Cost::Funding => self.inner.acquire_paced().await,
            Cost::Weight(weight) => self.inner.acquire_weight(weight).await,
        }
    }

    /// Raises the minute's count to what the server reported for it.
    ///
    /// The send loop applies a response through `Throttle::observe` and the
    /// engine's `observe`; this is the unit tests' way to apply a count.
    #[cfg(test)]
    pub(crate) fn record_used(&self, charge: MinuteCharge, used: u32) {
        self.inner.record_used(charge, used);
    }

    /// Holds every request for `delay`. Extends a cooldown in force, never
    /// shortens one: concurrent requests see the same `429` milliseconds
    /// apart, and taking the latest delay would release them all early.
    pub(crate) fn begin_cooldown(&self, delay: Duration) {
        self.inner.begin_cooldown(delay);
    }

    /// Holds every request until the next UTC minute, when the weight window
    /// resets: what a `429` with no `Retry-After` and no retry left calls for.
    /// Returns the hold. The client's retry policy asks for the same hold
    /// through the send loop, with [`until_next_minute`](Self::until_next_minute).
    #[cfg(test)]
    pub(crate) fn hold_until_next_minute(&self) -> Duration {
        let hold = self.until_next_minute();
        self.begin_cooldown(hold);
        hold
    }

    /// Time to the next UTC minute on this budget's clock.
    pub(crate) fn until_next_minute(&self) -> Duration {
        self.inner.until_next_minute()
    }
}

/// The budget as core's send loop sees it.
///
/// [`acquire`](Throttle::acquire) charges the request's [`WEIGHT_LAYER`] or
/// [`FUNDING_LAYER`] cost against the minute or the funding pace, after any
/// cooldown, and records the UTC minute a weight was charged in as its
/// charge's `window`.
/// [`observe`](Throttle::observe) applies `X-MBX-USED-WEIGHT-1M` to that minute
/// alone, whatever the status. [`hold`](Throttle::hold) extends the cooldown.
impl Throttle for WeightBudget {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Ok(Engine::charge(&self.inner, meta, WEIGHT_LAYER, Some(FUNDING_LAYER)).await)
    }

    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        self.inner.observe(charge, response, WEIGHT_LAYER);
    }

    fn hold(&self, delay: Duration) {
        self.begin_cooldown(delay);
    }
}
```

(c) In `polyoxide-binance/src/usdm/request.rs`, current (lines 14-15):

```rust
/// The header carrying the IP's weight used in the current minute.
pub const USED_WEIGHT_HEADER: &str = "x-mbx-used-weight-1m";
```

becomes (the constant is `shared::weight`'s now; `usdm::request::USED_WEIGHT_HEADER` stays a public path):

```rust
pub use crate::shared::weight::USED_WEIGHT_HEADER;
```

- [ ] **Step 4: Run them to see them pass, and check the moved code kept its tests**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib shared::weight
nix develop --command cargo test -p polyoxide-binance --all-features
diff <(git show HEAD:polyoxide-binance/src/usdm/weight.rs | sed -n '/^mod tests {$/,$p') <(sed -n '/^mod tests {$/,$p' polyoxide-binance/src/usdm/weight.rs) && echo "usdm weight tests untouched"
grep -c 'const RESERVED_FRACTION: u32 = 10;' polyoxide-binance/src/shared/weight.rs
```

Expected: `running 6 tests ... test result: ok. 6 passed; 0 failed` for the first, `test result: ok.` for every target of the second (`usdm::weight`'s tests included: `the_budget_aims_a_tenth_below_the_published_limit` still sees 2160), `usdm weight tests untouched` from the diff, and `1` from the grep.

Then the gates for the crate:

```bash
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo fmt --all -- --check
```

Expected: no warning, and no diff. If fmt prints one, run `nix develop --command cargo fmt --all` and keep its output (it may reflow a chain or an import order; it must not touch `mod tests` in `usdm/weight.rs`, which the diff above rechecks).

- [ ] **Step 5: Extend `rate-limits.md`'s globs to the new file**

Each glob of a rules file's `paths:` must match a file at this commit (`test_claude_rules.py` fails on one that matches none), so the new file gets a list entry of its own, beside the `usdm` one. No rule's body changes.

```bash
python3 - <<'EOF'
from pathlib import Path

path = Path(".claude/rules/rate-limits.md")
anchor = '  - "polyoxide-binance/src/usdm/{weight,policy}.rs"\n'
new = '  - "polyoxide-binance/src/shared/weight.rs"\n'
text = path.read_text()
assert text.count(anchor) == 1, "the `polyoxide-binance/src/usdm/{weight,policy}.rs` glob of rate-limits.md moved"
assert new not in text, "rate-limits.md already lists polyoxide-binance/src/shared/weight.rs"
path.write_text(text.replace(anchor, anchor + new))
EOF
git diff --stat -- .claude/rules
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
```

Expected: `git diff --stat` shows `.claude/rules/rate-limits.md | 1 +`, and the pytest run ends `passed` (the new glob matches `shared/weight.rs`, which Step 3 created; Task 2.4 adds the `shared/policy.rs` entry when it creates that file).

- [ ] **Step 6: Commit**

```bash
git add -N polyoxide-binance/src/shared/weight.rs
git commit -m "refactor(binance): move the minute-weight engine into the shared module (binance spot, Story 2)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/weight.rs polyoxide-binance/src/usdm/weight.rs polyoxide-binance/src/usdm/request.rs .claude/rules/rate-limits.md
```

### Task 2.3: the error decode moves to `shared::error`

`shared::error` holds the decode by status, the `{code, msg}` body, `retry_after_secs`, `clip` and the class of a venue error; `UsdmError` keeps its variants, `code()`, `Classify` and the 418's WARN, and maps `Decoded` onto itself. Its WARN stays in `usdm/error.rs`, so its target stays `polyoxide_binance::usdm::error`, which `tests/ban_log_usdm.rs` reads.

**Files:**
- Create: `polyoxide-binance/src/shared/error.rs`
- Modify: `polyoxide-binance/src/shared/mod.rs`
- Modify: `polyoxide-binance/src/usdm/error.rs:1-201`
- Modify: `polyoxide-binance/src/usdm/policy.rs:8-11`
- Test: `polyoxide-binance/src/shared/error.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide-binance/src/shared/mod.rs`, add `pub(crate) mod error;` above `pub(crate) mod symbol;`, so the file ends:

```rust
pub(crate) mod error;
pub(crate) mod symbol;
pub(crate) mod weight;
```

Create `polyoxide-binance/src/shared/error.rs` holding only this test module (Step 3 puts the implementation above it). It covers what `usdm`'s tests cannot see, since `UsdmError::IpBanned` has no field for the body: the decode hands the product the whole `418` body, unclipped, for the product to log.

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::header::HeaderMap;
    use polyoxide_core::reqwest::StatusCode;

    use super::*;

    #[test]
    fn a_418_hands_its_product_the_whole_body() {
        let long = "x".repeat(10_000);
        let response = ErrorResponse::new(StatusCode::IM_A_TEAPOT, HeaderMap::new(), &long);
        let Decoded::IpBanned { retry_after, body } = decode(response.into()) else {
            panic!("a 418 is a ban");
        };
        assert_eq!(retry_after, None);
        assert_eq!(body, long);
    }
}
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib shared::error
```

Expected: the crate's tests do not compile: ``error[E0433]: failed to resolve: use of undeclared type `Decoded` ``, and likewise `ApiError`, `ErrorResponse` and `decode`, ending `error: could not compile `polyoxide-binance` (lib test)`.

- [ ] **Step 3: Implement**

(a) Put this at the top of `polyoxide-binance/src/shared/error.rs`, above the `#[cfg(test)]` line and one blank line before it. It is `usdm/error.rs`'s `from_response`, `VenueBody`, `retry_after_secs` and `clip` moved, and the class expression of `Classify::class`'s `Venue` arm as `venue_class`. The order of the checks (status first, `418`, `429`, `451`, `403`, then the `{code, msg}` body, then core's response clipped with its wait clamped) is today's:

```rust
//! Binance's one error decode, by status first, shared by every Binance
//! product's error enum.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError, ErrorResponse};
use polyoxide_venue::{class_for_status, Class};
use serde::Deserialize;

use crate::shared::weight::MAX_COOLDOWN;

/// What an unsuccessful response decodes to. A product's `From<ApiError>`
/// maps each variant one to one onto its own error enum.
#[derive(Debug)]
pub(crate) enum Decoded {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    Api(ApiError),
    /// Binance's `{"code", "msg"}` body: an unknown symbol is `400` with code
    /// `-1121`.
    Venue {
        /// HTTP status.
        status: u16,
        /// Binance's error code, always negative.
        code: i64,
        /// Binance's message, clipped to 512 bytes.
        msg: String,
    },
    /// `429`.
    RateLimited {
        /// The response's `Retry-After`.
        retry_after: Option<Duration>,
    },
    /// `418`: the IP is banned.
    IpBanned {
        /// The response's `Retry-After`. The docs give bans of 2 minutes to 3 days.
        retry_after: Option<Duration>,
        /// The response body, whole: Binance's `-1003` text names when the
        /// ban ends, and the product logs it, clipped.
        body: String,
    },
    /// `451`: Binance does not serve the caller's location.
    RegionBlocked {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
    /// `403`: Binance's web application firewall refused the request.
    Forbidden {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
}

#[derive(Deserialize)]
struct VenueBody {
    code: i64,
    msg: String,
}

/// Binance's one decode: an unsuccessful response, by status first.
///
/// `418`, `429`, `451` and `403` go by status alone: their bodies were not
/// observed from a test IP, and the status is what decides what a caller
/// should do. Anything else with Binance's `{code, msg}` body is
/// [`Decoded::Venue`]; any other body stays core's [`ApiError::Response`],
/// its message and body clipped and its `Retry-After` clamped to 3 days, as
/// every arm reads it. Every other [`ApiError`] stays core's.
///
/// A `418`'s body is Binance's `-1003` text, which names when the ban ends.
/// The product's error has no field for it, so the decode hands it over whole
/// and the product logs it at WARN under its own module's target; the send
/// loop warns of the hold itself, naming the path.
pub(crate) fn decode(err: ApiError) -> Decoded {
    match err {
        ApiError::Response(response) => from_response(*response),
        other => Decoded::Api(other),
    }
}

fn from_response(mut response: ErrorResponse) -> Decoded {
    let status = response.status.as_u16();
    let retry_after = retry_after_secs(
        response
            .headers
            .get(polyoxide_core::reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
    );
    match status {
        418 => Decoded::IpBanned {
            retry_after,
            body: response.body,
        },
        429 => Decoded::RateLimited { retry_after },
        451 => Decoded::RegionBlocked {
            msg: clip(&response.body),
        },
        403 => Decoded::Forbidden {
            msg: clip(&response.body),
        },
        _ => match serde_json::from_str::<VenueBody>(&response.body) {
            Ok(venue) => Decoded::Venue {
                status,
                code: venue.code,
                msg: clip(&venue.msg),
            },
            // Core read the whole body for its `error` or `message`
            // field; only what is kept is clipped. The wait is Binance's
            // reading, clamped as the other arms clamp it.
            Err(_) => {
                response.message = clip(&response.message);
                response.body = clip(&response.body);
                response.retry_after = retry_after;
                Decoded::Api(response.into())
            }
        },
    }
}

/// Parses `Retry-After` with [`polyoxide_venue::parse_retry_after`], clamped
/// to [`MAX_COOLDOWN`]: seconds, whole or fractional, and `None` for zero,
/// negative, non-finite and unparsable values.
pub(crate) fn retry_after_secs(value: Option<&str>) -> Option<Duration> {
    value.and_then(|value| polyoxide_venue::parse_retry_after(value, MAX_COOLDOWN))
}

pub(crate) fn clip(text: &str) -> String {
    truncate_for_log(text).into_owned()
}

/// The class of a `{code, msg}` error: the status rule, with Binance's code in
/// decimal. A status that decides nothing is a `Decode`.
pub(crate) fn venue_class(status: u16, code: i64) -> Class {
    class_for_status(status)
        .unwrap_or(Class::Decode)
        .with_code(code.to_string())
}
```

(b) Replace `polyoxide-binance/src/usdm/error.rs` lines 1-201, everything above `#[cfg(test)]` (whose `mod tests` starts at line 203), with the text below. `UsdmError`, its variants, docs, `code()` and `Classify` are today's, apart from the `Venue` arm's class expression and the decode; the `cfg(test)` imports keep `mod tests`' names in scope. Leave `mod tests` untouched:

```rust
//! Error types for the USDⓈ-M futures API.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError};
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

use crate::shared::error::{decode, venue_class, Decoded};

#[cfg(test)]
use crate::shared::error::retry_after_secs;
#[cfg(test)]
use crate::usdm::weight::MAX_COOLDOWN;
#[cfg(test)]
use polyoxide_core::ErrorResponse;

/// Error type for USDⓈ-M futures API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum UsdmError {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    #[error(transparent)]
    Api(ApiError),

    /// Binance refused the request with its `{"code", "msg"}` body: an unknown
    /// symbol is `400` with code `-1121`.
    #[error("binance answered {status}: {code} {msg}")]
    Venue {
        /// HTTP status.
        status: u16,
        /// Binance's error code, always negative.
        code: i64,
        /// Binance's message, clipped to 512 bytes.
        msg: String,
    },

    /// Still answered `429` after the retry schedule ran out.
    #[error("binance rate limit (429), retry after {retry_after:?}")]
    RateLimited {
        /// The response's `Retry-After`.
        retry_after: Option<Duration>,
    },

    /// `418`: Binance has banned this IP for continuing after a `429`. Every
    /// request on the same [`WeightBudget`](crate::usdm::WeightBudget) waits
    /// until the ban lifts.
    #[error("binance has banned this IP (418), retry after {retry_after:?}")]
    IpBanned {
        /// The response's `Retry-After`. The docs give bans of 2 minutes to 3 days.
        retry_after: Option<Duration>,
    },

    /// `451`: Binance does not serve the caller's location.
    #[error("binance does not serve this location (451): {msg}")]
    RegionBlocked {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },

    /// `403`: Binance's web application firewall refused the request.
    #[error("binance's firewall refused the request (403): {msg}")]
    Forbidden {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
}

impl UsdmError {
    /// Binance's error code, for a [`Venue`](Self::Venue) error.
    pub fn code(&self) -> Option<i64> {
        match self {
            Self::Venue { code, .. } => Some(*code),
            _ => None,
        }
    }
}

/// Binance's one decode: an unsuccessful response, by status first.
///
/// `418`, `429`, `451` and `403` go by status alone: their bodies were not
/// observed from a test IP, and the status is what decides what a caller
/// should do. Anything else with Binance's `{code, msg}` body is
/// [`Venue`](UsdmError::Venue); any other body stays core's
/// [`ApiError::Response`], its message and body clipped and its `Retry-After`
/// clamped to 3 days, as every arm reads it. Every other [`ApiError`] stays
/// core's.
///
/// A `418`'s body is Binance's `-1003` text, which names when the ban ends.
/// The error's fields have no room for it, so it is logged at WARN under
/// `polyoxide_binance::usdm::error`; the send loop warns of the hold itself,
/// naming the path.
impl From<ApiError> for UsdmError {
    fn from(err: ApiError) -> Self {
        match decode(err) {
            Decoded::Api(err) => Self::Api(err),
            Decoded::Venue { status, code, msg } => Self::Venue { status, code, msg },
            Decoded::RateLimited { retry_after } => Self::RateLimited { retry_after },
            Decoded::IpBanned { retry_after, body } => {
                tracing::warn!("418: IP banned: {}", truncate_for_log(&body));
                Self::IpBanned { retry_after }
            }
            Decoded::RegionBlocked { msg } => Self::RegionBlocked { msg },
            Decoded::Forbidden { msg } => Self::Forbidden { msg },
        }
    }
}

/// `Api` delegates, and `Venue` follows the status rule with Binance's code
/// in decimal. A `429` is `RateLimited`, and a ban, a region block and a
/// firewall refusal are all `Restricted`: Binance documents its `403` as the
/// firewall's, not as a credential failure (D14). So a `429`, and a `408`,
/// `425` or 5xx however its body is shaped, are retriable, and a ban, a region
/// block and a firewall refusal are not: sending again does not change them,
/// and sending after a `418` lengthens the ban.
///
/// A region block is the venue answering as designed, so it is not a fault. A
/// `429`'s or a ban's [`Classify::retry_after`] is its `Retry-After`, clamped
/// to 3 days: for a ban, the time it lifts.
impl Classify for UsdmError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue { status, code, .. } => venue_class(*status, *code),
            Self::RateLimited { retry_after } => {
                Class::RateLimited { retry_after: None }.with_retry_after(*retry_after)
            }
            Self::IpBanned { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => {
                Class::Restricted
            }
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::RegionBlocked { .. } => false,
            // Unreachable today, since a 451 is `RegionBlocked`, but a region
            // block whichever variant carries it.
            Self::Venue { status, .. } => *status != 451,
            Self::RateLimited { .. } | Self::IpBanned { .. } | Self::Forbidden { .. } => true,
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::RateLimited { retry_after } | Self::IpBanned { retry_after } => {
                retry_after.filter(|wait| !wait.is_zero())
            }
            Self::Venue { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => None,
        }
    }
}
```

(c) In `polyoxide-binance/src/usdm/policy.rs`, `retry_after_secs` now lives in `shared::error`. Current (lines 8-11):

```rust
use crate::usdm::{
    error::retry_after_secs,
    weight::{WeightBudget, DEFAULT_BAN},
};
```

becomes (Task 2.4 rewrites this file's top again):

```rust
use crate::shared::error::retry_after_secs;
use crate::usdm::weight::{WeightBudget, DEFAULT_BAN};
```

- [ ] **Step 4: Run it to see it pass, and check the moved code kept its tests**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib shared::error
nix develop --command cargo test -p polyoxide-binance --all-features
diff <(git show HEAD:polyoxide-binance/src/usdm/error.rs | sed -n '/^mod tests {$/,$p') <(sed -n '/^mod tests {$/,$p' polyoxide-binance/src/usdm/error.rs) && echo "usdm error tests untouched"
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo fmt --all -- --check
```

Expected: `running 1 test ... test result: ok.`, then `test result: ok.` for every target (`ban_log_usdm`, `error_classes_usdm` and `mock_usdm` among them, which read the decode and the WARN's `polyoxide_binance::usdm::error` target), `usdm error tests untouched`, no clippy warning, no fmt diff (if fmt prints one, run `nix develop --command cargo fmt --all` and keep its output).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/shared/error.rs
git commit -m "refactor(binance): move the error decode into the shared module (binance spot, Story 2)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/error.rs polyoxide-binance/src/usdm/error.rs polyoxide-binance/src/usdm/policy.rs
```

### Task 2.4: the retry decision moves to `shared::policy`, with rows (a) and (k)

`shared::policy::decide` is `UsdmRetryPolicy::decide`'s body with `self.budget.until_next_minute()` replaced by an argument; the `429` hold keeps the longer of the attempt's backoff (core's `retry_delay`, the one place the `Retry-After` floor lives) and the asked wait. `UsdmRetryPolicy { pub(crate) budget }` stays and calls it. The two `docs/MUTANTS.md` rows that cite the arms move with them, so their citations, the ledger test that pins them and their proofs change in this commit (AD-21).

**Files:**
- Create: `polyoxide-binance/src/shared/policy.rs`
- Modify: `polyoxide-binance/src/shared/mod.rs`
- Modify: `polyoxide-binance/src/usdm/policy.rs:1-69`
- Modify: `docs/MUTANTS.md:145-146,153,168`
- Modify: `.github/scripts/tests/test_mutants_ledger.py:33-35`
- Modify: `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md` (the `paths:` frontmatter only)
- Test: `polyoxide-binance/src/usdm/policy.rs` (its `mod tests`, untouched), `polyoxide-binance/tests/mock_usdm.rs` (untouched)

- [ ] **Step 1: Run the guard tests before the move**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --lib usdm::policy
nix develop --command cargo test -p polyoxide-binance --all-features --test mock_usdm
```

Expected: `test result: ok.` for both; the first holds `a_418_without_retry_after_holds_two_minutes` and its neighbours, the second the four tests rows (a) and (k) name.

- [ ] **Step 2: Implement the move**

In `polyoxide-binance/src/shared/mod.rs`, add `pub(crate) mod policy;` between `error` and `symbol`, so the file ends:

```rust
pub(crate) mod error;
pub(crate) mod policy;
pub(crate) mod symbol;
pub(crate) mod weight;
```

Create `polyoxide-binance/src/shared/policy.rs`. The match is today's, arm for arm; do not change a hold, an outcome or the order of the arms:

```rust
//! What follows each Binance REST response: the retry decision every Binance
//! product's policy makes.

use std::time::Duration;

use polyoxide_core::reqwest::StatusCode;
use polyoxide_core::{AttemptInfo, Decision, Outcome, ResponseMeta, RetryConfig};

use crate::shared::error::retry_after_secs;
use crate::shared::weight::DEFAULT_BAN;

/// What follows `response`, by Binance's rules.
///
/// - A `429` with a retry left is retried, and holds every request on the
///   budget for the longer of the attempt's backoff and its `Retry-After`.
/// - A `429` with none left fails, and holds for its `Retry-After`, or for
///   `until_next_minute` when it has none: sending again into a spent minute
///   is how a `429` becomes a `418` ban.
/// - A `418` (the IP is banned) fails, and holds for its `Retry-After`, or
///   [`DEFAULT_BAN`] when it has none.
/// - A 2xx is done. Everything else fails with no hold, a `425` included:
///   Binance does not document it as retriable.
///
/// A `Retry-After` is clamped to `MAX_COOLDOWN`, the longest ban Binance
/// documents. `until_next_minute` is the time to the next UTC minute on the
/// product's own weight budget.
pub(crate) fn decide(
    response: &ResponseMeta<'_>,
    attempt: &AttemptInfo,
    schedule: &RetryConfig,
    until_next_minute: Duration,
) -> Decision {
    let asked = retry_after_secs(response.retry_after());
    match response.status {
        StatusCode::TOO_MANY_REQUESTS if attempt.retries_left > 0 => Decision {
            // The loop's floor is the attempt's backoff; the hold makes
            // every other request wait at least as long.
            outcome: Outcome::Retry(Duration::ZERO),
            hold: Some(
                schedule
                    .retry_delay(attempt.attempt, response.retry_after())
                    .max(asked.unwrap_or_default()),
            ),
        },
        StatusCode::TOO_MANY_REQUESTS => Decision {
            outcome: Outcome::Fail,
            hold: Some(asked.unwrap_or(until_next_minute)),
        },
        StatusCode::IM_A_TEAPOT => Decision {
            outcome: Outcome::Fail,
            hold: Some(asked.unwrap_or(DEFAULT_BAN)),
        },
        status if status.is_success() => Decision {
            outcome: Outcome::Done,
            hold: None,
        },
        _ => Decision {
            outcome: Outcome::Fail,
            hold: None,
        },
    }
}
```

Replace `polyoxide-binance/src/usdm/policy.rs` lines 1-69, everything above `#[cfg(test)]` (whose `mod tests` starts at line 71), with the text below. The `cfg(test)` imports keep the names `mod tests` uses in scope (`Duration`, `StatusCode`, `Outcome`); `MAX_COOLDOWN` is named by its full path there. Leave `mod tests` untouched:

```rust
//! [`UsdmRetryPolicy`]: what follows each USDⓈ-M response.

use polyoxide_core::{AttemptInfo, Decision, ResponseMeta, RetryConfig, RetryPolicy};

use crate::usdm::weight::WeightBudget;

#[cfg(test)]
use polyoxide_core::reqwest::StatusCode;
#[cfg(test)]
use polyoxide_core::Outcome;
#[cfg(test)]
use std::time::Duration;

/// Binance's retry policy, over the [`WeightBudget`] whose minute it reads.
///
/// - A `429` with a retry left is retried, and holds every request on the
///   budget for the longer of the attempt's backoff and its `Retry-After`.
/// - A `429` with none left fails, and holds for its `Retry-After`, or until
///   the next UTC minute when it has none: sending again into a spent minute
///   is how a `429` becomes a `418` ban.
/// - A `418` (the IP is banned) fails, and holds for its `Retry-After`, or
///   [`DEFAULT_BAN`](crate::usdm::weight::DEFAULT_BAN) when it has none.
/// - A 2xx is done. Everything else fails with no hold, a `425` included:
///   Binance does not document it as retriable.
///
/// A `Retry-After` is clamped to [`MAX_COOLDOWN`](crate::usdm::weight::MAX_COOLDOWN),
/// the longest ban Binance documents. The decision is `shared::policy::decide`,
/// which every Binance product's policy calls.
#[derive(Debug, Clone)]
pub(crate) struct UsdmRetryPolicy {
    pub(crate) budget: WeightBudget,
}

impl RetryPolicy for UsdmRetryPolicy {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision {
        let until_next_minute = self.budget.until_next_minute();
        crate::shared::policy::decide(response, attempt, schedule, until_next_minute)
    }
}
```

- [ ] **Step 3: Run the guard tests after the move**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features
diff <(git show HEAD:polyoxide-binance/src/usdm/policy.rs | sed -n '/^mod tests {$/,$p') <(sed -n '/^mod tests {$/,$p' polyoxide-binance/src/usdm/policy.rs) && echo "usdm policy tests untouched"
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo fmt --all -- --check
```

Expected: `test result: ok.` for every target, `usdm policy tests untouched`, no warning, no diff (if fmt prints one, run `nix develop --command cargo fmt --all` and keep its output; it can only reorder the `use` lines, so the line numbers below hold).

- [ ] **Step 4: Re-cite rows (a) and (k) and the ledger that pins them**

The lines the rows cite are now at these places (confirm them first):

```bash
grep -n 'hold: Some(asked.unwrap_or(until_next_minute)),' polyoxide-binance/src/shared/policy.rs
grep -n 'hold: Some(asked.unwrap_or(DEFAULT_BAN)),' polyoxide-binance/src/shared/policy.rs
grep -n 'fn a_418_without_retry_after_holds_two_minutes(' polyoxide-binance/src/usdm/policy.rs
```

Expected: the line numbers `47`, `51` and `160`, in that order. If one differs, use the printed number wherever `47`, `51` or `160` appears below (rustfmt can move a line in `usdm/policy.rs`, never one in `shared/policy.rs`).

In `docs/MUTANTS.md` row (a), "Binance's policy" (line 153), the current text

```
`polyoxide-binance/src/usdm/policy.rs:53`, `UsdmRetryPolicy`'s arm for a 429 with no retry left holds the weight budget for its `Retry-After`, or to the next UTC minute
```

becomes

```
`polyoxide-binance/src/shared/policy.rs:47`, `shared::policy::decide`'s arm for a 429 with no retry left (which `UsdmRetryPolicy` calls) holds the weight budget for its `Retry-After`, or to the next UTC minute
```

In row (k) (line 168), the current text

```
`polyoxide-binance/src/usdm/policy.rs:57`, `UsdmRetryPolicy`'s 418 arm
```

becomes

```
`polyoxide-binance/src/shared/policy.rs:51`, `shared::policy::decide`'s 418 arm
```

and, in the same row, the current

```
`polyoxide-binance/src/usdm/policy.rs:185` `a_418_without_retry_after_holds_two_minutes`
```

becomes

```
`polyoxide-binance/src/usdm/policy.rs:160` `a_418_without_retry_after_holds_two_minutes`
```

In `.github/scripts/tests/test_mutants_ledger.py`, current (lines 33-35):

```python
    ("polyoxide-binance/src/usdm/policy.rs", 53): "hold: Some(asked.unwrap_or_else(|| self.budget.until_next_minute())),",
    ("polyoxide-binance/src/usdm/policy.rs", 57): "hold: Some(asked.unwrap_or(DEFAULT_BAN)),",
    ("polyoxide-binance/src/usdm/policy.rs", 185): "fn a_418_without_retry_after_holds_two_minutes(",
```

becomes

```python
    ("polyoxide-binance/src/shared/policy.rs", 47): "hold: Some(asked.unwrap_or(until_next_minute)),",
    ("polyoxide-binance/src/shared/policy.rs", 51): "hold: Some(asked.unwrap_or(DEFAULT_BAN)),",
    ("polyoxide-binance/src/usdm/policy.rs", 160): "fn a_418_without_retry_after_holds_two_minutes(",
```

Then:

```bash
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'
```

Expected: every test passes (the citations and the snippets agree, and each cited line holds its snippet).

- [ ] **Step 5: Prove each mutant again**

Each command applies one mutation to `shared/policy.rs`, runs the tests the row names, which must fail, and restores the file.

Row (a), "Binance's policy": `hold: None` in the 429-with-no-retry-left arm.

```bash
F=polyoxide-binance/src/shared/policy.rs
BACKUP=$(mktemp)
cp "$F" "$BACKUP"
sed -i '47s/.*/            hold: None,/' "$F"
nix develop --command cargo test -p polyoxide-binance --all-features --test mock_usdm -- \
  a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request \
  a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute
cp "$BACKUP" "$F"
```

Expected: `test result: FAILED. 0 passed; 2 failed`.

Row (k), first mutant: `hold: None` in the 418 arm, which must fail the first two tests.

```bash
F=polyoxide-binance/src/shared/policy.rs
BACKUP=$(mktemp)
cp "$F" "$BACKUP"
sed -i '51s/.*/            hold: None,/' "$F"
nix develop --command cargo test -p polyoxide-binance --all-features --test mock_usdm -- \
  a_418_is_not_retried_and_holds_the_next_request \
  every_usdm_on_one_budget_is_held_by_one_ban
cp "$BACKUP" "$F"
```

Expected: `test result: FAILED. 0 passed; 2 failed`.

Row (k), second mutant: `asked.unwrap_or(Duration::ZERO)`, which must fail the third.

```bash
F=polyoxide-binance/src/shared/policy.rs
BACKUP=$(mktemp)
cp "$F" "$BACKUP"
sed -i '51s/.*/            hold: Some(asked.unwrap_or(Duration::ZERO)),/' "$F"
nix develop --command cargo test -p polyoxide-binance --all-features --lib -- \
  a_418_without_retry_after_holds_two_minutes
cp "$BACKUP" "$F"
```

Expected: `test result: FAILED. 0 passed; 1 failed`.

Then, with the file restored, the same tests pass:

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --test mock_usdm -- \
  a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request \
  a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute \
  a_418_is_not_retried_and_holds_the_next_request \
  every_usdm_on_one_budget_is_held_by_one_ban
nix develop --command cargo test -p polyoxide-binance --all-features --lib -- \
  a_418_without_retry_after_holds_two_minutes
```

Expected: `test result: ok. 4 passed` and `test result: ok. 1 passed`.

- [ ] **Step 6: Record the proof in `docs/MUTANTS.md`**

Insert one paragraph at the end of the proof history, directly above `## The rules`, with today's date and the commit the proof ran on top of (`git rev-parse --short HEAD`), the way the paragraphs above it do:

```bash
python3 - <<'EOF'
import datetime
import pathlib
import subprocess

path = pathlib.Path("docs/MUTANTS.md")
head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()
today = datetime.date.today().isoformat()
paragraph = (
    "The Binance spot plan's Story 2 moved `UsdmRetryPolicy`'s decision into "
    "`polyoxide-binance/src/shared/policy.rs` (`shared::policy::decide`), which every "
    "Binance product's policy calls, and the 418 test's line in `src/usdm/policy.rs` moved "
    "with the file's top. Rows \"(a), Binance's policy\" and \"(k)\" were re-cited there and "
    f"proved again on {today} on top of `{head}`, on Rust 1.99.0: each row's tests passed, "
    "then failed with its mutant in place (two for row (k)), then passed again once the "
    "file was restored, through `cargo test -p polyoxide-binance --all-features`.\n"
)
anchor = "`git diff --exit-code` clean, through the same command.\n\n## The rules"
text = path.read_text()
assert text.count(anchor) == 1, "the anchor before `## The rules` moved"
head_of_anchor = "`git diff --exit-code` clean, through the same command.\n"
path.write_text(text.replace(anchor, head_of_anchor + paragraph + "\n## The rules"))
EOF
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'
```

Expected: the ledger tests still pass (the paragraph cites no `file:line`).

- [ ] **Step 7: Extend `rate-limits.md`'s and `send-loop.md`'s globs to the new file**

Each glob of a rules file's `paths:` must match a file at this commit (`test_claude_rules.py` fails on one that matches none), so the new file gets a list entry of its own, beside the `usdm` one. No rule's body changes.

```bash
python3 - <<'EOF'
from pathlib import Path

path = Path(".claude/rules/rate-limits.md")
anchor = '  - "polyoxide-binance/src/shared/weight.rs"\n'
new = '  - "polyoxide-binance/src/shared/policy.rs"\n'
text = path.read_text()
assert text.count(anchor) == 1, "the `polyoxide-binance/src/shared/weight.rs` glob of rate-limits.md moved"
assert new not in text, "rate-limits.md already lists polyoxide-binance/src/shared/policy.rs"
path.write_text(text.replace(anchor, anchor + new))

path = Path(".claude/rules/send-loop.md")
anchor = '  - "polyoxide-binance/src/usdm/{policy,request}.rs"\n'
new = '  - "polyoxide-binance/src/shared/policy.rs"\n'
text = path.read_text()
assert text.count(anchor) == 1, "the `polyoxide-binance/src/usdm/{policy,request}.rs` glob of send-loop.md moved"
assert new not in text, "send-loop.md already lists polyoxide-binance/src/shared/policy.rs"
path.write_text(text.replace(anchor, anchor + new))
EOF
git diff --stat -- .claude/rules
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
```

Expected: `git diff --stat` shows `.claude/rules/rate-limits.md | 1 +` and `.claude/rules/send-loop.md | 1 +`, and the pytest run ends `passed` (both new globs match `shared/policy.rs`, created in Step 2; Task 2.5 adds `shared/request.rs` to `send-loop.md` when it creates that file).

- [ ] **Step 8: Commit**

```bash
git add -N polyoxide-binance/src/shared/policy.rs
git commit -m "refactor(binance): move the retry decision into the shared module, re-cite and re-prove MUTANTS rows (a) and (k) (binance spot, Story 2)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/policy.rs polyoxide-binance/src/usdm/policy.rs docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py .claude/rules/rate-limits.md .claude/rules/send-loop.md
```

### Task 2.5: the routed request moves to `shared::request`

`shared::request` holds the `Priced` trait, `Routed<T, R, E>` and the `route_builder!` macro; `usdm::request` keeps `Routed<T>` as an alias over `usdm`'s route and error, a `route_builder!` forwarding to the shared macro with `module = usdm, error = UsdmError`, and the `USED_WEIGHT_HEADER` re-export from Task 2.2. `usdm::weight::Route` implements `Priced` through its inherent `path` and `cost`, which stay public. `usdm/api/*.rs` do not change.

**Files:**
- Create: `polyoxide-binance/src/shared/request.rs`
- Modify: `polyoxide-binance/src/shared/mod.rs`
- Modify: `polyoxide-binance/src/usdm/request.rs:1-108`
- Modify: `polyoxide-binance/src/usdm/weight.rs:15,155-157`
- Modify: `.claude/rules/send-loop.md`, `.claude/rules/clients.md` (the `paths:` frontmatter only)
- Test: `polyoxide-binance/tests/mock_usdm.rs`, `polyoxide-binance/tests/query_setters_usdm.rs` (untouched)

- [ ] **Step 1: Run the guard tests before the move**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features --test mock_usdm --test query_setters_usdm --test headers_usdm
```

Expected: `test result: ok.` for each; they send every `usdm` route through `Routed`, with its query, its re-pricing setters and its cost.

- [ ] **Step 2: Implement**

In `polyoxide-binance/src/shared/mod.rs`, add `pub(crate) mod request;` between `policy` and `symbol`, so the file ends:

```rust
pub(crate) mod error;
pub(crate) mod policy;
pub(crate) mod request;
pub(crate) mod symbol;
pub(crate) mod weight;
```

Create `polyoxide-binance/src/shared/request.rs`. `Routed` and `route_builder!` are `usdm/request.rs`'s, with the route and error types as parameters and the module named in the macro's arguments:

```rust
//! What every route builder sends through: core's request builder, with the
//! route's weight as its cost.

use std::marker::PhantomData;

use polyoxide_core::{HttpClient, QueryBuilder, Request, RequestError};
use serde::de::DeserializeOwned;

/// A product's route, as [`Routed`] reads it: where the request goes and what
/// it costs.
pub(crate) trait Priced: Copy {
    /// What a request costs, in the product's own terms.
    type Cost: Copy + Into<polyoxide_core::Cost>;

    /// The route's path.
    fn path(self) -> &'static str;

    /// What sending the route costs.
    fn cost(self) -> Self::Cost;
}

/// A route and its query, sent as core's [`Request`] with the route's cost.
///
/// The route builders wrap one. Setting a query parameter twice replaces it,
/// and a parameter that changes the weight replaces the route with it.
pub(crate) struct Routed<T, R, E> {
    http: HttpClient,
    route: R,
    query: Vec<(&'static str, String)>,
    _marker: PhantomData<fn() -> (T, E)>,
}

impl<T, R, E> Routed<T, R, E> {
    pub(crate) fn new(http: &HttpClient, route: R) -> Self {
        Self {
            http: http.clone(),
            route,
            query: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a query parameter, replacing an earlier value for the same key.
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Replaces the route, for a parameter that changes the weight.
    pub(crate) fn route(mut self, route: R) -> Self {
        self.route = route;
        self
    }
}

impl<T, R: Priced, E> Routed<T, R, E> {
    /// What sending this request will cost.
    pub(crate) fn cost(&self) -> R::Cost {
        self.route.cost()
    }
}

impl<T: DeserializeOwned, R: Priced, E: RequestError> Routed<T, R, E> {
    /// Sends on the client's send loop, which waits for the budget, and
    /// decodes the response.
    pub(crate) async fn send(self) -> Result<T, E> {
        let mut request = Request::<T, E>::new(self.http, self.route.path())
            .with_cost(self.route.cost().into());
        for (key, value) in self.query {
            request = request.query(key, value);
        }
        request.send().await
    }
}

/// A route builder with no parameters of its own beyond those its namespace
/// method sets: `cost` and `send`, like the builders with setters. A product
/// invokes it through its own `route_builder!`, which names its module and
/// error type.
macro_rules! route_builder {
    (
        $(#[$doc:meta])* $name:ident => $output:ty;
        module = $module:ident, error = $error:ident
    ) => {
        $(#[$doc])*
        #[must_use = "a request does nothing until it is sent"]
        pub struct $name {
            pub(crate) request: $crate::$module::request::Routed<$output>,
        }

        impl $name {
            /// What sending this request will cost.
            pub fn cost(&self) -> $crate::$module::weight::Cost {
                self.request.cost()
            }

            /// Waits for the budget, sends, and decodes the response.
            ///
            /// A `429` holds every request on the budget, then this request
            /// is retried on the client's schedule; with no retry left and no
            /// `Retry-After`, the hold runs to the next UTC minute. A `418`
            /// holds every request for its `Retry-After`, or 2 minutes, and
            /// is not retried.
            ///
            /// The wait has no deadline of its own: a request can wait out a
            /// minute's spent budget or a ban, which Binance documents at up
            /// to 3 days. Wrap the call in `tokio::time::timeout` where a
            /// caller needs a deadline.
            pub async fn send(self) -> Result<$output, $crate::$module::$error> {
                self.request.send().await
            }
        }
    };
}

pub(crate) use route_builder;
```

Replace `polyoxide-binance/src/usdm/request.rs` whole with:

```rust
//! What every route builder sends through: core's request builder, with the
//! route's weight as its cost.

use crate::usdm::{error::UsdmError, weight::Route};

pub use crate::shared::weight::USED_WEIGHT_HEADER;

/// A route and its query, sent as core's [`Request`](polyoxide_core::Request)
/// with the route's cost.
///
/// The route builders wrap one. Setting a query parameter twice replaces it,
/// and a parameter that changes the weight replaces the route with it.
pub(crate) type Routed<T> = crate::shared::request::Routed<T, Route, UsdmError>;

/// A route builder with no parameters of its own beyond those its namespace
/// method sets: `cost` and `send`, like the builders with setters.
macro_rules! route_builder {
    ($(#[$doc:meta])* $name:ident => $output:ty) => {
        $crate::shared::request::route_builder! {
            $(#[$doc])* $name => $output;
            module = usdm, error = UsdmError
        }
    };
}

pub(crate) use route_builder;
```

In `polyoxide-binance/src/usdm/weight.rs`, add `Priced` to the imports and the impl after `impl Route`. In the regular `use crate::shared::weight::{Clock, Engine};` group, current:

```rust
use crate::shared::weight::{Clock, Engine};
use crate::usdm::types::DepthLimit;
```

becomes:

```rust
use crate::shared::request::Priced;
use crate::shared::weight::{Clock, Engine};
use crate::usdm::types::DepthLimit;
```

and directly after the closing brace of `impl Route` (the line before the doc comment `/// The request-weight budget of one IP, shared by every client that clones it.`), add:

```rust
/// How `Routed` reads a route: through the inherent `path` and `cost`, which
/// stay public.
impl Priced for Route {
    type Cost = Cost;

    fn path(self) -> &'static str {
        Route::path(self)
    }

    fn cost(self) -> Cost {
        Route::cost(self)
    }
}
```

- [ ] **Step 3: Run the guard tests after the move**

```bash
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo fmt --all -- --check
git diff --stat HEAD -- polyoxide-binance/src/usdm/api polyoxide-binance/tests
```

Expected: `test result: ok.` for every target, no warning, no fmt diff (if fmt prints one, run `nix develop --command cargo fmt --all` and keep its output), and an empty `git diff --stat` (the `usdm/api` files and every test file are untouched).

- [ ] **Step 4: Run the story's gates**

`{packages}` for this story is `polyoxide-binance`, which `polyoxide` and `polyoxide-cli` depend on:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-targets --all-features -- -D warnings
nix develop --command cargo test -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide -p polyoxide-cli --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
git add -N polyoxide-binance/src/shared/request.rs
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
```

Expected: each exits 0. The doc build matters here: `usdm::weight::WeightBudget`'s docs link `Hold` by path now, and `DEFAULT_BAN`, `MAX_COOLDOWN`, `USED_WEIGHT_HEADER` and `Symbol` are public re-exports of items in a private module, whose docs rustdoc inlines. The test-body gate prints the new test-context items (`shared::weight`'s and `shared::error`'s tests, and `WeightBudget::acquire` and `record_used` as `#[cfg(test)]` methods) as new, and no changed or lost one.

Then the story's Verify line:

```bash
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps
nix develop --command python3 scripts/api_removals.py check --baseline v0.40.0
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_mutants_ledger.py -q'
```

Expected: each exits 0; `api_removals.py` reports nothing new, since every moved path (`usdm::types::Symbol` and `InvalidSymbol`, `usdm::weight::DEFAULT_BAN` and `MAX_COOLDOWN`, `usdm::request::USED_WEIGHT_HEADER`) is still there through `pub use`, and `InvalidSymbol` is still listed in `lib.rs`'s `Classify` assertion.

- [ ] **Step 5: Extend `send-loop.md`'s and `clients.md`'s globs to the new file**

Each glob of a rules file's `paths:` must match a file at this commit (`test_claude_rules.py` fails on one that matches none), so the new file gets a list entry of its own, beside the `usdm` one. No rule's body changes.

```bash
python3 - <<'EOF'
from pathlib import Path

path = Path(".claude/rules/send-loop.md")
anchor = '  - "polyoxide-binance/src/shared/policy.rs"\n'
new = '  - "polyoxide-binance/src/shared/request.rs"\n'
text = path.read_text()
assert text.count(anchor) == 1, "the `polyoxide-binance/src/shared/policy.rs` glob of send-loop.md moved"
assert new not in text, "send-loop.md already lists polyoxide-binance/src/shared/request.rs"
path.write_text(text.replace(anchor, anchor + new))

path = Path(".claude/rules/clients.md")
anchor = '  - "polyoxide-binance/src/usdm/{mod,request}.rs"\n'
new = '  - "polyoxide-binance/src/shared/request.rs"\n'
text = path.read_text()
assert text.count(anchor) == 1, "the `polyoxide-binance/src/usdm/{mod,request}.rs` glob of clients.md moved"
assert new not in text, "clients.md already lists polyoxide-binance/src/shared/request.rs"
path.write_text(text.replace(anchor, anchor + new))
EOF
git diff --stat -- .claude/rules
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
```

Expected: `git diff --stat` shows `.claude/rules/send-loop.md | 1 +` and `.claude/rules/clients.md | 1 +`, and the pytest run ends `passed` (both new globs match `shared/request.rs`, created in Step 2).

- [ ] **Step 6: Commit**

```bash
git add -N polyoxide-binance/src/shared/request.rs
git commit -m "refactor(binance): move the routed request and route_builder into the shared module (binance spot, Story 2)" -- polyoxide-binance/src/shared/mod.rs polyoxide-binance/src/shared/request.rs polyoxide-binance/src/usdm/request.rs polyoxide-binance/src/usdm/weight.rs .claude/rules/send-loop.md .claude/rules/clients.md
```

## Story 3: `ProductClass::Spot` and the `SpotMarket` bundle

- **Developer:** intermediate-dev
- **Files:** `polyoxide-venue/src/key.rs`, `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/tests/spot_class.rs`, `.claude/rules/venue-traits.md`, `polyoxide-venue/README.md`
- **Interfaces:** exposes "Story 3 → Story 6"; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-venue --all-features && cargo test -p polyoxide-venue --features keys,market --test spot_class && python3 scripts/venue_gate.py`
- **Review:** deep (a foundation crate's public enum and trait)

Adds `ProductClass::Spot` (after `PerpetualFuture`, `as_str()` `"spot"`) and the bundle `SpotMarket: MarketData` with its blanket impl, documented in the shape of `PerpetualMarket`'s, re-exports the bundle at the crate root, and names it in the crate docs' sentence about the bundles, in the README's sentence about them, and in `.claude/rules/venue-traits.md` (the `ProductClass` bullet and the bundles paragraph, in the same commits as the code, AD-21; the rules file says `Spot` has no product yet and `SpotMarket` is `MarketData` alone, and Story 6 adds Binance's spot to both). `ProductClass` is `#[non_exhaustive]`, so no match in another crate breaks. The new test file proves the class's id and order, that a key a made-up `KeySpace` makes carries the class, that a type with `MarketData` alone satisfies a `SpotMarket` bound, and that the bound passes a refusal through. The docs and tests name no venue and use a made-up instrument and product (`acme.cash`); the words "spot" and `SpotMarket` appear only as the class and the bundle; Story 6 adds `spot` to `ci/venue-gate-skip.txt`'s generic words, so this story needs no `gate_exceptions` entry. None of the profile's landmine markers is in this story's code.

### Task 3.1: The class

**Files:**
- Create: `polyoxide-venue/tests/spot_class.rs`
- Modify: `polyoxide-venue/src/key.rs:94-114`
- Modify: `.claude/rules/venue-traits.md:42-43`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-venue/tests/spot_class.rs`:

```rust
#![cfg(all(feature = "keys", feature = "market"))]
//! `ProductClass::Spot` and the `SpotMarket` bundle, proved on a made-up venue.

use std::borrow::Cow;

use polyoxide_venue::{KeySpace, ProductClass, ProductId, VenueId};

/// A made-up product that trades an asset outright.
struct Cash;

impl KeySpace for Cash {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("cash");
    const CLASS: ProductClass = ProductClass::Spot;
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_ascii_uppercase())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

#[test]
fn the_class_is_spelled_as_a_crate_registers_it() {
    assert_eq!(ProductClass::Spot.as_str(), "spot");
    assert_eq!(ProductClass::Spot.to_string(), "spot");
}

#[test]
fn the_class_is_distinct_from_the_others_and_sorts_after_them() {
    let ids = [
        ProductClass::EventContract,
        ProductClass::PerpetualFuture,
        ProductClass::Spot,
    ]
    .map(ProductClass::as_str);
    assert_eq!(ids, ["event-contract", "perpetual-future", "spot"]);
    assert!(ProductClass::EventContract < ProductClass::PerpetualFuture);
    assert!(ProductClass::PerpetualFuture < ProductClass::Spot);
}

#[test]
fn a_key_carries_the_class_its_key_space_declares() {
    let key = Cash::key(&Cash::parse_id("abc").unwrap());
    assert_eq!(key.class(), ProductClass::Spot);
    assert_eq!(key.to_string(), "acme.cash:ABC");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features keys,market --test spot_class`

Expected: a compile error, ``error[E0599]: no variant or associated item named `Spot` found for enum `ProductClass` in the current scope``.

- [ ] **Step 3: Implement**

In `polyoxide-venue/src/key.rs`, replace

```rust
    /// Perpetual futures. A price is unbounded, a position's size is signed,
    /// and holders pay or receive funding.
    PerpetualFuture,
}
```

with

```rust
    /// Perpetual futures. A price is unbounded, a position's size is signed,
    /// and holders pay or receive funding.
    PerpetualFuture,
    /// Buying and selling the asset itself. A price is unbounded, a position
    /// is a balance of the base asset and so never negative, and there is no
    /// funding.
    Spot,
}
```

and, in `ProductClass::as_str`, replace

```rust
            Self::PerpetualFuture => "perpetual-future",
        }
```

with

```rust
            Self::PerpetualFuture => "perpetual-future",
            Self::Spot => "spot",
        }
```

In `.claude/rules/venue-traits.md`, replace

```markdown
- Each `KeySpace` declares its product's `ProductClass` as `CLASS` (ADR 0001): `EventContract`
  (the clob, Kalshi's `predictions`) or `PerpetualFuture` (perps, USDⓈ-M), `#[non_exhaustive]`, a
  variant added only when a product of that class lands. Every key carries it as
```

with

```markdown
- Each `KeySpace` declares its product's `ProductClass` as `CLASS` (ADR 0001): `EventContract`
  (the clob, Kalshi's `predictions`), `PerpetualFuture` (perps, USDⓈ-M) or `Spot` (no product yet),
  `#[non_exhaustive]`, a variant added only when a product of that class lands. Every key carries it as
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --features keys,market --test spot_class`

Expected: `test result: ok. 3 passed; 0 failed`.

Then run the gates this task can break: `nix develop --command cargo fmt --all -- --check` (if it reports a diff in the new test file, run `nix develop --command cargo fmt -p polyoxide-venue` and keep its output), `nix develop --command cargo clippy -p polyoxide-venue --all-targets --all-features -- -D warnings`, `nix develop --command cargo test -p polyoxide-venue --all-features`, and `python3 scripts/venue_gate.py`. Each passes: nothing else in the workspace matches `ProductClass` exhaustively (it is `#[non_exhaustive]`).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-venue/tests/spot_class.rs
git commit -m "feat(venue): ProductClass::Spot (binance spot, Story 3)" -- polyoxide-venue/src/key.rs polyoxide-venue/tests/spot_class.rs .claude/rules/venue-traits.md
```

### Task 3.2: The `SpotMarket` bundle

**Files:**
- Modify: `polyoxide-venue/src/market.rs:10-13`, `polyoxide-venue/src/market.rs:203-212`
- Modify: `polyoxide-venue/src/lib.rs:28-36`, `polyoxide-venue/src/lib.rs:97-101`
- Modify: `polyoxide-venue/README.md:10-11,15-19`
- Modify: `.claude/rules/venue-traits.md:101-109`
- Test: `polyoxide-venue/tests/spot_class.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide-venue/tests/spot_class.rs`, replace the import

```rust
use polyoxide_venue::{KeySpace, ProductClass, ProductId, VenueId};
```

with

```rust
use polyoxide_venue::{
    Book, Class, ClassifiedError, Instrument, KeySpace, Level, MarketData, MarketKey, ProductClass,
    ProductId, Quote, SizeUnit, SpotMarket, VenueId,
};
use rust_decimal::Decimal;
```

and append after the file's last test:

```rust
/// A made-up product of another class, whose keys `Ledger` must refuse.
struct Swap;

impl KeySpace for Swap {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("swap");
    const CLASS: ProductClass = ProductClass::PerpetualFuture;
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_ascii_uppercase())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

/// Implements `MarketData` and nothing else: one instrument, `acme.cash:ABC`,
/// with a one-level book.
struct Ledger;

impl Ledger {
    fn check(key: &MarketKey) -> Result<String, ClassifiedError> {
        Cash::id(key).map_err(ClassifiedError::from)
    }
}

impl MarketData for Ledger {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let id = Self::check(key)?;
        Ok(Instrument::new(key.clone(), id, SizeUnit::Base))
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        Self::check(key)?;
        let mut quote = Quote::new(key.clone());
        quote.bid = Some(Level::new(Decimal::ONE, Decimal::TWO));
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        Self::check(key)?;
        let mut book = Book::new(key.clone());
        book.bids.push(Level::new(Decimal::ONE, Decimal::TWO));
        Ok(book)
    }
}

/// What a caller generic over spot products writes: one bound.
async fn read<M: SpotMarket>(
    market: &M,
    key: &MarketKey,
) -> Result<(Quote, Book), ClassifiedError> {
    Ok((market.quote(key).await?, market.book(key).await?))
}

#[tokio::test]
async fn a_type_with_market_data_alone_satisfies_a_spot_market_bound() {
    let key = Cash::key(&"ABC".to_owned());
    let (quote, book) = read(&Ledger, &key).await.unwrap();
    assert_eq!(quote.key, key);
    assert_eq!(quote.bid, Some(Level::new(Decimal::ONE, Decimal::TWO)));
    assert_eq!(book.bids, vec![Level::new(Decimal::ONE, Decimal::TWO)]);
}

#[tokio::test]
async fn a_key_of_another_product_is_refused_through_the_bound() {
    let swap = Swap::key(&"ABC".to_owned());
    let refusal = read(&Ledger, &swap).await.unwrap_err();
    assert_eq!(refusal.class, Class::InvalidRequest);
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-venue --features keys,market --test spot_class`

Expected: a compile error, ``error[E0432]: unresolved import `polyoxide_venue::SpotMarket` ``.

- [ ] **Step 3: Implement**

In `polyoxide-venue/src/market.rs`, replace the module docs' sentence

```rust
//! A class bundle names what every product of a [`ProductClass`](crate::ProductClass)
//! offers: [`PerpetualMarket`] is [`MarketData`] and [`Funding`], and
//! [`EventContractMarket`] is [`MarketData`] and [`EventGroups`]. Each is
//! implemented for every type that implements its members.
```

with

```rust
//! A class bundle names what every product of a [`ProductClass`](crate::ProductClass)
//! offers: [`PerpetualMarket`] is [`MarketData`] and [`Funding`],
//! [`EventContractMarket`] is [`MarketData`] and [`EventGroups`], and
//! [`SpotMarket`] is [`MarketData`] alone. Each is implemented for every type
//! that implements its members.
```

Then replace

```rust
impl<T: MarketData + EventGroups> EventContractMarket for T {}
```

with

```rust
impl<T: MarketData + EventGroups> EventContractMarket for T {}

/// What every [`ProductClass::Spot`](crate::ProductClass::Spot) product
/// offers: [`MarketData`], and nothing beyond it, so a function generic over
/// products that trade the asset itself needs one bound.
///
/// Like [`PerpetualMarket`], it names only what defines its class, is
/// implemented for every type that implements its members, and has no `Dyn`
/// wrapper. Nothing beyond [`MarketData`] defines the class: a capability its
/// products share with other classes, such as [`Candles`] or [`Trades`], stays
/// out, so a product of the class that lacks one still has the bundle.
///
/// ```
/// use polyoxide_venue::{ClassifiedError, MarketKey, Quote, SpotMarket};
///
/// /// The quote of one asset, on any venue that trades it outright.
/// async fn quote_of<M: SpotMarket>(
///     market: &M,
///     key: &MarketKey,
/// ) -> Result<Quote, ClassifiedError> {
///     market.quote(key).await
/// }
/// ```
///
/// A type that is not [`MarketData`] is not one:
///
/// ```compile_fail
/// use polyoxide_venue::SpotMarket;
///
/// struct Nothing;
///
/// fn needs_spot_market<M: SpotMarket>() {}
///
/// needs_spot_market::<Nothing>();
/// ```
pub trait SpotMarket: MarketData {}

impl<T: MarketData> SpotMarket for T {}
```

In `polyoxide-venue/src/lib.rs`, replace the crate docs' lines

```rust
//!   `Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper. The
//!   class bundles `PerpetualMarket` and `EventContractMarket` name what every
//!   product of a class offers. The traits return the records market data is
//!   read into (`Instrument`, `Quote`, `Book`, `Trade`, `Candle`,
```

with

```rust
//!   `Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper. The
//!   class bundles `PerpetualMarket`, `EventContractMarket` and `SpotMarket`
//!   name what every product of a class offers. The traits return the records
//!   market data is read into (`Instrument`, `Quote`, `Book`, `Trade`, `Candle`,
```

and the re-export

```rust
    EventContractMarket, EventGroups, Funding, Instruments, MarketData, PerpetualMarket, Trades,
};
```

with

```rust
    EventContractMarket, EventGroups, Funding, Instruments, MarketData, PerpetualMarket, SpotMarket,
    Trades,
};
```

In `polyoxide-venue/README.md`, replace

```markdown
Each product declares its `ProductClass`, event contract or perpetual future,
and every key it makes carries it.
```

with

```markdown
Each product declares its `ProductClass`, event contract, perpetual future or
spot, and every key it makes carries it.
```

In `polyoxide-venue/README.md`, replace

```markdown
class bundles `PerpetualMarket` and `EventContractMarket` name what every
product of a class offers, so a function generic over one class needs one
bound.
```

with

```markdown
class bundles `PerpetualMarket`, `EventContractMarket` and `SpotMarket` name
what every product of a class offers, so a function generic over one class
needs one bound.
```

In `.claude/rules/venue-traits.md`, replace

```markdown
`PerpetualMarket` is `MarketData + Funding`; `EventContractMarket` is `MarketData + EventGroups`.
Each has a blanket impl and no `Dyn` wrapper, so a class-generic function needs one bound, and
```

with

```markdown
`PerpetualMarket` is `MarketData + Funding`; `EventContractMarket` is `MarketData + EventGroups`;
`SpotMarket` is `MarketData` alone. Each has a blanket impl and no `Dyn` wrapper, so a
class-generic function needs one bound, and
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --features keys,market --test spot_class`

Expected: `test result: ok. 5 passed; 0 failed`.

Then run: `nix develop --command cargo test -p polyoxide-venue --all-features` (the `compile_fail` doctest on `SpotMarket` passes by failing to compile, on the unsatisfied `Nothing: MarketData` bound), `nix develop --command cargo fmt --all -- --check` (if it reports a diff, run `nix develop --command cargo fmt -p polyoxide-venue` and keep its output), `nix develop --command cargo clippy -p polyoxide-venue --all-targets --all-features -- -D warnings`, `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue --all-features'`, `python3 scripts/venue_gate.py` and `python3 scripts/test_body_diff.py --baseline v0.40.0`. Each passes: no existing test changed.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(venue): the SpotMarket bundle (binance spot, Story 3)" -- polyoxide-venue/src/market.rs polyoxide-venue/src/lib.rs polyoxide-venue/tests/spot_class.rs polyoxide-venue/README.md .claude/rules/venue-traits.md
```

## Story 4: The spot client: feature, builder, weight budget, policy, errors, health and exchange routes

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/README.md`, `polyoxide-binance/src/spot/error.rs`, `polyoxide-binance/src/spot/policy.rs`, `polyoxide-binance/src/spot/request.rs`, `polyoxide-binance/src/spot/types.rs`, `polyoxide-binance/src/spot/weight.rs`, `polyoxide-binance/src/spot/api/mod.rs`, `polyoxide-binance/src/spot/api/health.rs`, `polyoxide-binance/src/spot/api/exchange.rs`, `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/headers_spot.rs`, `polyoxide-binance/tests/error_classes_spot.rs`, `polyoxide-binance/tests/failure_tags_spot.rs`, `polyoxide-binance/tests/ban_log_spot.rs`, `polyoxide-binance/src/shared/weight.rs`, `.claude/rules/ci-gates.md`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/clients.md`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide-binance@spot.txt`, `Cargo.lock`
- **Interfaces:** exposes "Story 4 → Stories 5, 6 and 7"; uses Story 1's `time.json`, `exchange_info.json` and `error_invalid_symbol.json`, and Story 2's `shared` items
- **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot && cargo test -p polyoxide-binance --all-features && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_module_readmes.py -q'`
- **Review:** deep (a new public client and its rate budget)

This story adds the `spot` module to `polyoxide-binance` in `usdm`'s shape, on Story 2's crate-private `shared` plumbing: the `spot` feature (and `default = ["usdm", "spot"]`), `spot::weight` with the whole route table and its `documented_weights` test, `spot::error::SpotError`, `spot::policy::SpotRetryPolicy`, `spot::request`, the exchange part of `spot::types`, the `Spot` client and `SpotBuilder` with the in-flight note of spec §4, the `health` and `exchange` namespaces, and the module README (a doctest under `spot`). Five suites run it on a mock server (`mock_spot`, `headers_spot`, `error_classes_spot`, `failure_tags_spot`, `ban_log_spot`), `cargo hack --each-feature` builds `spot` without `usdm`, and the dependency allowlists gain `polyoxide-binance@spot.txt`, with `.claude/rules/ci-gates.md`'s `polyoxide-binance` deny-list sentence naming the file (AD-21). A spot-only build is warning-free: Task 4.10 marks with `#[cfg_attr(not(feature = "usdm"), allow(dead_code))]` exactly the `shared` items that the spot-only build reports as unused (the build decides, and it may report none), and changes nothing else in `shared`. The `paths:` globs of `.claude/rules/rate-limits.md`, `send-loop.md` and `clients.md` gain `spot`'s counterparts of their `usdm` entries in the commits that create the files (Tasks 4.1 and 4.4), and no rule's body changes. Nothing here changes a landmine marker: the budget is built through Story 2's `Engine` (no burst, the reserved tenth lives there) and the retry decision is Story 2's `shared::policy::decide`; Task 4.10 touches `shared/weight.rs`, which holds `RESERVED_FRACTION`, only if the build reports an item there, and then only to add that attribute above the item; it must leave the constant and every line that uses it as they are. Story 1's fixtures and Story 2's `shared` items are not in the tree when this draft is written; the code uses them as the names contract states them. Run `nix develop --command cargo fmt --all` before each task's commit; the profile's `cargo fmt --all -- --check` gate follows every task. The weights in `Route::cost` are the spec's §3.1 table; if Story 1's `OBSERVED.md` (`## Spot`, `### REST weights`) records a different weight, stop and ask the owner before Task 4.1.

### Task 4.1: The `spot` feature and `spot::weight`

**Files:**
- Modify: `polyoxide-binance/Cargo.toml` (the `[features]` table, the package description and keywords, the docs.rs features)
- Modify: `polyoxide-binance/src/lib.rs` (Story 2's `mod shared;` gate, and `pub mod spot;`)
- Create: `polyoxide-binance/src/spot/mod.rs`
- Create: `polyoxide-binance/src/spot/weight.rs`
- Modify: `.claude/rules/rate-limits.md` (one `paths:` glob)
- Modify: `.claude/rules/clients.md` (one `paths:` glob)
- Test: `polyoxide-binance/src/spot/weight.rs` (its `tests` module)

- [ ] **Step 1: Write the failing test**

Edit `polyoxide-binance/Cargo.toml`. Replace

```toml
# One feature per module (Story 5.6), as polyoxide-polymarket has: `usdm`, the
# USDⓈ-M futures REST client and its weight budget, and `usdm-ws`, its market
# streams, which turns `usdm` on.
[features]
default = ["usdm"]
```

with

```toml
# One feature per module (Story 5.6), as polyoxide-polymarket has: `usdm`, the
# USDⓈ-M futures REST client and its weight budget, and `usdm-ws`, its market
# streams, which turns `usdm` on; `spot`, the spot REST client, which turns
# neither on and is built without either.
[features]
default = ["usdm", "spot"]
```

Insert directly before the line `# The USDⓈ-M market streams, \`usdm::ws\`, on the socket kit.`:

```toml
# Spot public market data on `api.binance.com`: the same optional dependencies
# and venue features as `usdm`, on the crate's shared REST plumbing.
spot = [
    "dep:polyoxide-core",
    "polyoxide-venue/decimal",
    "polyoxide-venue/market",
    "dep:rust_decimal",
    "dep:serde",
    "dep:serde_json",
    "dep:thiserror",
    "dep:tokio",
    "dep:tracing",
    "dep:url",
]
```

Replace the two package lines

```toml
description = "Rust client library for Binance USDⓈ-M futures market data and streams"
keywords = ["binance", "futures", "perpetuals", "market-data"]
```

with

```toml
description = "Rust client library for Binance USDⓈ-M futures and spot market data, and USDⓈ-M streams"
keywords = ["binance", "futures", "spot", "perpetuals", "market-data"]
```

Replace

```toml
# docs.rs documents the module and its streams; `test-server` stays out of the
# docs.
[package.metadata.docs.rs]
features = ["usdm", "usdm-ws"]
```

with

```toml
# docs.rs documents both modules and the USDⓈ-M streams; `test-server` stays out
# of the docs.
[package.metadata.docs.rs]
features = ["usdm", "usdm-ws", "spot"]
```

Edit `polyoxide-binance/src/lib.rs`. Story 2 added, between the module README doctests and the `usdm` module:

```rust
#[cfg(feature = "usdm")]
mod shared;
```

Replace those two lines with

```rust
#[cfg(any(feature = "usdm", feature = "spot"))]
mod shared;
```

Replace the line `pub mod usdm;` (the only one in the file) with

```rust
pub mod usdm;

#[cfg(feature = "spot")]
pub mod spot;
```

Create `polyoxide-binance/src/spot/mod.rs`. Later tasks of this story replace it; it starts as:

```rust
//! # Spot
//!
//! Client for Binance spot public market data (`api.binance.com`), behind the
//! `spot` feature, which is on by default. No credentials are needed.

pub mod weight;

pub use weight::WeightBudget;
```

Create `polyoxide-binance/src/spot/weight.rs` holding only its test module (Step 3 puts the implementation above it):

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::{
        header::{HeaderMap, HeaderValue},
        Method, StatusCode,
    };

    use super::*;

    /// 2026-11-02 12:00:00 UTC, a minute boundary.
    const ON_A_MINUTE: u64 = 1_793_620_800_000;

    #[test]
    fn documented_weights() {
        let w = Cost::Weight;
        let table = [
            (Route::Ping, w(1)),
            (Route::Time, w(1)),
            (Route::ExchangeInfo, w(20)),
            (Route::Depth { limit: None }, w(5)),
            (Route::Depth { limit: Some(1) }, w(5)),
            (Route::Depth { limit: Some(7) }, w(5)),
            (Route::Depth { limit: Some(100) }, w(5)),
            (Route::Depth { limit: Some(101) }, w(25)),
            (Route::Depth { limit: Some(500) }, w(25)),
            (Route::Depth { limit: Some(501) }, w(50)),
            (Route::Depth { limit: Some(1000) }, w(50)),
            (Route::Depth { limit: Some(1001) }, w(250)),
            (Route::Depth { limit: Some(5000) }, w(250)),
            (Route::Depth { limit: Some(5001) }, w(250)),
            (Route::Trades, w(25)),
            (Route::AggTrades, w(4)),
            (Route::Klines, w(2)),
            (Route::AvgPrice, w(2)),
            (Route::Ticker24h { all: false }, w(2)),
            (Route::Ticker24h { all: true }, w(80)),
            (Route::Price { all: false }, w(2)),
            (Route::Price { all: true }, w(4)),
            (Route::BookTicker { all: false }, w(2)),
            (Route::BookTicker { all: true }, w(4)),
        ];
        for (route, cost) in table {
            assert_eq!(route.cost(), cost, "{route:?}");
        }
    }

    #[test]
    fn every_route_has_its_documented_path() {
        let table = [
            (Route::Ping, "/api/v3/ping"),
            (Route::Time, "/api/v3/time"),
            (Route::ExchangeInfo, "/api/v3/exchangeInfo"),
            (Route::Depth { limit: None }, "/api/v3/depth"),
            (Route::Trades, "/api/v3/trades"),
            (Route::AggTrades, "/api/v3/aggTrades"),
            (Route::Klines, "/api/v3/klines"),
            (Route::AvgPrice, "/api/v3/avgPrice"),
            (Route::Ticker24h { all: true }, "/api/v3/ticker/24hr"),
            (Route::Ticker24h { all: false }, "/api/v3/ticker/24hr"),
            (Route::Price { all: true }, "/api/v3/ticker/price"),
            (Route::Price { all: false }, "/api/v3/ticker/price"),
            (Route::BookTicker { all: true }, "/api/v3/ticker/bookTicker"),
            (Route::BookTicker { all: false }, "/api/v3/ticker/bookTicker"),
        ];
        for (route, path) in table {
            assert_eq!(route.path(), path, "{route:?}");
        }
    }

    #[test]
    fn the_budget_aims_a_tenth_below_the_published_limit() {
        assert_eq!(PUBLISHED_WEIGHT_PER_MINUTE, 6000);
        assert_eq!(WeightBudget::new().per_minute(), 5400);
    }

    #[test]
    fn a_cost_reaches_core_as_an_exact_charge_on_the_spot_layer() {
        let cost = polyoxide_core::Cost::from(Cost::Weight(20));
        assert_eq!(cost.layer, WEIGHT_LAYER);
        assert_eq!(cost.units, 20);
        assert!(cost.exact);
        assert_eq!(WEIGHT_LAYER, LayerId("binance-spot-weight"));
    }

    fn meta<'a>(method: &'a Method, costs: &'a [polyoxide_core::Cost]) -> RequestMeta<'a> {
        RequestMeta {
            method,
            path: "/api/v3/exchangeInfo",
            query: &[],
            costs,
        }
    }

    #[tokio::test]
    async fn the_throttle_charges_the_weight_layer_and_reads_the_header_back() {
        let budget = WeightBudget::at_ms(ON_A_MINUTE);
        let get = Method::GET;
        let costs = [polyoxide_core::Cost::from(Cost::Weight(20))];
        let charge = Throttle::acquire(&budget, &meta(&get, &costs))
            .await
            .unwrap();
        assert_eq!(budget.used(), 20);

        let mut headers = HeaderMap::new();
        headers.insert("x-mbx-used-weight-1m", HeaderValue::from_static("100"));
        let response = ResponseMeta {
            status: StatusCode::OK,
            headers: &headers,
        };
        let attempt = AttemptInfo {
            attempt: 0,
            retries_left: 0,
        };
        budget.observe(&charge, &response, &attempt);
        assert_eq!(budget.used(), 100, "the server's count raises the minute's");
    }

    #[tokio::test]
    async fn a_cost_on_another_layer_is_not_charged() {
        let budget = WeightBudget::at_ms(ON_A_MINUTE);
        let get = Method::GET;
        let costs = [polyoxide_core::Cost {
            layer: LayerId("some-other-layer"),
            units: 50,
            exact: true,
        }];
        Throttle::acquire(&budget, &meta(&get, &costs))
            .await
            .unwrap();
        assert_eq!(budget.used(), 0);
    }

    #[test]
    fn a_clone_shares_the_budget() {
        let budget = WeightBudget::at_ms(ON_A_MINUTE);
        let clone = budget.clone();
        assert!(budget.is_shared_with(&clone));
        assert!(!budget.is_shared_with(&WeightBudget::at_ms(ON_A_MINUTE)));
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::weight`

Expected: the build fails in `src/spot/weight.rs` with `error[E0412]: cannot find type \`Route\` in this scope`, `error[E0433]: failed to resolve: use of undeclared type \`Cost\`` and the same for `WeightBudget`, `WEIGHT_LAYER` and `PUBLISHED_WEIGHT_PER_MINUTE`: the test module compiles against nothing yet.

- [ ] **Step 3: Implement**

Put this above the `#[cfg(test)] mod tests` block in `polyoxide-binance/src/spot/weight.rs`:

```rust
//! Binance spot's request-weight budget and the table of what each route costs.
//!
//! Binance charges each REST request a *weight* that depends on its route and
//! its parameters, and refuses an IP whose weight in the current minute passes
//! `exchangeInfo`'s `REQUEST_WEIGHT` limit of 6000. Core's `RateLimiter` counts
//! requests per path, so it cannot model a cost that varies with `limit`;
//! [`WeightBudget`] does, on the engine every Binance module shares. The table is
//! [`Route::cost`]; the measurements behind it are in
//! `docs/specs/binance/OBSERVED.md` (`## Spot`).
//!
//! `exchangeInfo` also lists a `RAW_REQUESTS` limit of 300,000 per five
//! minutes. It is not modelled: every request weighs at least 1, so a budget of
//! 5400 per minute caps a client at 27,000 requests per five minutes, under it.

use std::sync::Arc;
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, Charge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle,
};

use crate::shared::request::Priced;
use crate::shared::weight::{Clock, Engine};

pub use crate::shared::weight::{DEFAULT_BAN, MAX_COOLDOWN};

/// `exchangeInfo`'s `REQUEST_WEIGHT` limit per minute, per IP.
pub const PUBLISHED_WEIGHT_PER_MINUTE: u32 = 6000;

/// The per-minute request-weight layer of a [`WeightBudget`], as a request's
/// [`polyoxide_core::Cost`] names it.
pub const WEIGHT_LAYER: LayerId = LayerId("binance-spot-weight");

/// What one request costs, and which limit it draws on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Request weight, against the per-minute budget.
    Weight(u32),
}

/// The cost a request builder hands core's send loop: its weight against
/// [`WEIGHT_LAYER`]. It is exact.
impl From<Cost> for polyoxide_core::Cost {
    fn from(cost: Cost) -> Self {
        match cost {
            Cost::Weight(weight) => Self {
                layer: WEIGHT_LAYER,
                units: weight,
                exact: true,
            },
        }
    }
}

/// A REST route, with the parameters its weight depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Route {
    /// `GET /api/v3/ping`.
    Ping,
    /// `GET /api/v3/time`.
    Time,
    /// `GET /api/v3/exchangeInfo`.
    ExchangeInfo,
    /// `GET /api/v3/depth`.
    Depth {
        /// The `limit` parameter, if sent.
        limit: Option<u32>,
    },
    /// `GET /api/v3/trades`.
    Trades,
    /// `GET /api/v3/aggTrades`.
    AggTrades,
    /// `GET /api/v3/klines`.
    Klines,
    /// `GET /api/v3/avgPrice`.
    AvgPrice,
    /// `GET /api/v3/ticker/24hr`: one symbol, or every symbol when `all`.
    Ticker24h {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /api/v3/ticker/price`: one symbol, or every symbol when `all`.
    Price {
        /// No `symbol` parameter.
        all: bool,
    },
    /// `GET /api/v3/ticker/bookTicker`: one symbol, or every symbol when `all`.
    BookTicker {
        /// No `symbol` parameter.
        all: bool,
    },
}

impl Route {
    /// The route's path on `api.binance.com`.
    pub fn path(self) -> &'static str {
        match self {
            Self::Ping => "/api/v3/ping",
            Self::Time => "/api/v3/time",
            Self::ExchangeInfo => "/api/v3/exchangeInfo",
            Self::Depth { .. } => "/api/v3/depth",
            Self::Trades => "/api/v3/trades",
            Self::AggTrades => "/api/v3/aggTrades",
            Self::Klines => "/api/v3/klines",
            Self::AvgPrice => "/api/v3/avgPrice",
            Self::Ticker24h { .. } => "/api/v3/ticker/24hr",
            Self::Price { .. } => "/api/v3/ticker/price",
            Self::BookTicker { .. } => "/api/v3/ticker/bookTicker",
        }
    }

    /// What the request costs, as measured from `X-MBX-USED-WEIGHT-1M` deltas
    /// on 2026-11-02 (`docs/specs/binance/OBSERVED.md`, `## Spot`, `### REST
    /// weights`).
    ///
    /// `depth` costs 5 without a `limit` and for 1 to 100 levels, 25 for 101 to
    /// 500, 50 for 501 to 1000 and 250 above, the last band as Binance
    /// documents it. `exchangeInfo` costs 20 with a `symbol` or without. The
    /// one-symbol tickers and `klines` and `avgPrice` cost 2, `trades` 25 and
    /// `aggTrades` 4, whatever their `limit`.
    pub fn cost(self) -> Cost {
        match self {
            Self::Ping | Self::Time => Cost::Weight(1),
            Self::ExchangeInfo => Cost::Weight(20),
            Self::Depth { limit: None } => Cost::Weight(5),
            Self::Depth { limit: Some(limit) } => Cost::Weight(match limit {
                0..=100 => 5,
                101..=500 => 25,
                501..=1000 => 50,
                _ => 250,
            }),
            Self::Trades => Cost::Weight(25),
            Self::AggTrades => Cost::Weight(4),
            Self::Klines | Self::AvgPrice => Cost::Weight(2),
            Self::Ticker24h { all } => Cost::Weight(if all { 80 } else { 2 }),
            Self::Price { all } | Self::BookTicker { all } => {
                Cost::Weight(if all { 4 } else { 2 })
            }
        }
    }
}

impl Priced for Route {
    type Cost = Cost;

    fn path(self) -> &'static str {
        Route::path(self)
    }

    fn cost(self) -> Cost {
        Route::cost(self)
    }
}

/// The request-weight budget of one IP, shared by every client that clones it.
///
/// Binance limits weight per IP, not per client, so every
/// [`Spot`](crate::spot::Spot) in a process should be built with one budget
/// ([`SpotBuilder::weight_budget`](crate::spot::SpotBuilder::weight_budget)).
///
/// - **Window.** The UTC clock minute, as on the USDⓈ-M module: the server's
///   count falls just after each minute boundary, where a sliding window would
///   keep counting.
/// - **Budget.** The published 6000 less a tenth, 5400. A request waits while
///   its weight would take the minute past that.
/// - **Server count.** Every response's `X-MBX-USED-WEIGHT-1M` raises the
///   minute's count to at least what the server reports, because other
///   processes on the same IP spend the same budget. The count never falls
///   within a minute, and a response is applied only to the minute its request
///   was charged in.
/// - **Cooldown.** A `429` or `418` holds every request for the delay the
///   server asked for, with a ceiling of [`MAX_COOLDOWN`], so a hold is only
///   ever extended.
/// - **Throttle.** The budget is the [`Throttle`] every [`Spot`](crate::spot::Spot)
///   built with it sends through: a request's weight is charged before it is
///   sent, and each response's header is applied to the minute it was charged
///   in.
/// - **In flight.** A response's header is applied only to the minute its
///   request was charged in, so a request in flight across a minute boundary is
///   counted by the server in a minute this budget no longer sees. The reserve
///   of 600 absorbs that while the weight in flight stays under it: one `Spot`
///   at its default of 4 concurrent requests has at most 320 in flight on a
///   route at its default (4 × `tickers_24h`'s 80), and four `depth` calls
///   with a `limit` above 1000 (250 each) pass it. A higher `max_concurrent`,
///   or several clients sharing one budget, can pass it too.
/// - **Hosts.** `data-api.binance.vision` shares this counter with
///   `api.binance.com` (`OBSERVED.md`, `### One counter across hosts`), so a
///   client pointed at either is charged against the one budget.
#[derive(Debug, Clone)]
pub struct WeightBudget {
    inner: Arc<Engine>,
}

impl Default for WeightBudget {
    fn default() -> Self {
        Self::new()
    }
}

impl WeightBudget {
    /// A budget for Binance spot's published limit, less the reserve.
    pub fn new() -> Self {
        Self::with_clock(Clock::System)
    }

    fn with_clock(clock: Clock) -> Self {
        Self {
            inner: Arc::new(Engine::new(PUBLISHED_WEIGHT_PER_MINUTE, None, clock)),
        }
    }

    /// A budget on a clock a test sets by hand, starting at `start_ms`.
    #[cfg(test)]
    pub(crate) fn at_ms(start_ms: u64) -> Self {
        Self::with_clock(Clock::Manual(Arc::new(std::sync::atomic::AtomicU64::new(
            start_ms,
        ))))
    }

    /// Whether two handles charge one budget.
    #[cfg(test)]
    pub(crate) fn is_shared_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute()
    }

    /// Weight charged or reported in the current UTC minute.
    pub fn used(&self) -> u32 {
        self.inner.used()
    }
}

/// The budget as core's send loop sees it.
///
/// [`acquire`](Throttle::acquire) charges the request's [`WEIGHT_LAYER`] cost
/// against the minute, after any cooldown, and records the UTC minute it was
/// charged in. [`observe`](Throttle::observe) applies `X-MBX-USED-WEIGHT-1M`
/// to that minute alone, whatever the status. [`hold`](Throttle::hold) extends
/// the cooldown.
impl Throttle for WeightBudget {
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Ok(self.inner.charge(meta, WEIGHT_LAYER, None).await)
    }

    fn observe(&self, charge: &Charge, response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        self.inner.observe(charge, response, WEIGHT_LAYER);
    }

    fn hold(&self, delay: Duration) {
        self.inner.begin_cooldown(delay);
    }
}
```

Then add the two `paths:` globs the new files need, in the same commit (a glob that matches no file fails `test_claude_rules.py`, so each names only a file this task creates). Change no rule's body. Story 2 adds the `shared` counterparts to the same lists, so anchor on the `usdm` glob's text and not on the whole line.

- In `.claude/rules/rate-limits.md`'s frontmatter, directly after the `paths:` list item that names `polyoxide-binance/src/usdm/{weight,policy}.rs` (and after any `shared` item Story 2 put beside it), add this list item, indented as its neighbours are:

```yaml
  - "polyoxide-binance/src/spot/weight.rs"
```

- In `.claude/rules/clients.md`'s frontmatter, directly after the `paths:` list item that names `polyoxide-binance/src/usdm/{mod,request}.rs` (and after any `shared` item Story 2 put beside it), add this list item:

```yaml
  - "polyoxide-binance/src/spot/mod.rs"
```

Task 4.4 widens both to the brace forms once `spot/policy.rs` and `spot/request.rs` exist.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::weight`

Expected: `test result: ok. 7 passed; 0 failed` (`documented_weights`, `every_route_has_its_documented_path`, `the_budget_aims_a_tenth_below_the_published_limit`, `a_cost_reaches_core_as_an_exact_charge_on_the_spot_layer`, `the_throttle_charges_the_weight_layer_and_reads_the_header_back`, `a_cost_on_another_layer_is_not_charged`, `a_clone_shares_the_budget`). Then run `bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'` (it passes: both new globs match a file), `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` and `nix develop --command cargo test -p polyoxide-binance --all-features --lib`: all pass, `usdm`'s tests unchanged.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/spot/weight.rs
git commit -m "feat(binance): the spot feature and its weight budget (binance spot, Story 4)" -- polyoxide-binance/Cargo.toml polyoxide-binance/src/lib.rs polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/spot/weight.rs .claude/rules/rate-limits.md .claude/rules/clients.md
```

### Task 4.2: `SpotError`

**Files:**
- Create: `polyoxide-binance/src/spot/error.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs`
- Test: `polyoxide-binance/src/spot/error.rs` (its `tests` module)

- [ ] **Step 1: Write the failing test**

Edit `polyoxide-binance/src/spot/mod.rs`. Replace

```rust
pub mod weight;

pub use weight::WeightBudget;
```

with

```rust
pub mod error;
pub mod weight;

pub use error::SpotError;
pub use weight::WeightBudget;
```

Create `polyoxide-binance/src/spot/error.rs` holding only its test module:

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::ErrorResponse;
    use polyoxide_test_support::fixtures;

    use super::*;
    use crate::spot::weight::MAX_COOLDOWN;

    /// A response with `status`, `body` and, when given, a `Retry-After`.
    fn response(status: u16, retry_after: Option<&str>, body: &str) -> ApiError {
        let mut headers = polyoxide_core::reqwest::header::HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(
                polyoxide_core::reqwest::header::RETRY_AFTER,
                value.parse().unwrap(),
            );
        }
        ErrorResponse::new(
            polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),
            headers,
            body,
        )
        .into()
    }

    /// That response, through the spot module's decode.
    fn parts(status: u16, retry_after: Option<&str>, body: &str) -> SpotError {
        SpotError::from(response(status, retry_after, body))
    }

    #[test]
    fn the_captured_invalid_symbol_body_is_a_venue_error_with_its_code() {
        // Captured from production: `GET /api/v3/exchangeInfo?symbol=NOPEUSDT`
        // answers 400 (`tests/fixtures/spot/PROVENANCE.md`).
        let body = fixtures!("spot/rest").text("error_invalid_symbol");
        let err = parts(400, None, &body);
        assert!(
            matches!(&err, SpotError::Venue { status: 400, code: -1121, msg } if msg == "Invalid symbol."),
            "{err:?}"
        );
        assert_eq!(err.code(), Some(-1121));
        assert!(!err.is_retriable());
    }

    #[test]
    fn statuses_that_decide_the_caller_s_next_step_win_over_the_body() {
        // A 429 carries a {code, msg} body too; it must still be a rate limit,
        // not a venue error a retry policy would treat as permanent.
        let body = r#"{"code":-1003,"msg":"Too many requests."}"#;
        let limited = parts(429, Some("7"), body);
        assert!(matches!(limited, SpotError::RateLimited { .. }));
        assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
        assert!(limited.is_retriable());

        let banned = parts(418, Some("120"), body);
        assert!(matches!(banned, SpotError::IpBanned { .. }));
        assert_eq!(banned.retry_after(), Some(Duration::from_secs(120)));
        assert!(!banned.is_retriable());

        let region = parts(451, None, body);
        assert!(matches!(&region, SpotError::RegionBlocked { msg } if msg.contains("-1003")));
        assert!(!region.is_retriable());

        let firewall = parts(403, None, "<html>denied</html>");
        assert!(matches!(&firewall, SpotError::Forbidden { msg } if msg == "<html>denied</html>"));
        assert!(!firewall.is_retriable());

        let firewall_json = parts(403, None, body);
        assert!(matches!(firewall_json, SpotError::Forbidden { .. }));
    }

    #[test]
    fn a_body_in_another_shape_is_left_to_core() {
        let err = parts(502, None, "<html>bad gateway</html>");
        assert!(matches!(
            &err,
            SpotError::Api(ApiError::Response(r)) if r.status.as_u16() == 502
        ));
        assert!(err.is_retriable());
        assert_eq!(err.code(), None);
    }

    #[test]
    fn a_core_error_that_is_not_a_response_stays_core_s() {
        let err = SpotError::from(ApiError::Validation("no symbol".into()));
        assert!(matches!(err, SpotError::Api(ApiError::Validation(_))), "{err:?}");
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn every_kept_body_is_clipped() {
        let long = "x".repeat(10_000);
        let venue = format!(r#"{{"code":-1121,"msg":"{long}"}}"#);
        let kept = |err: SpotError| match err {
            SpotError::Venue { msg, .. }
            | SpotError::RegionBlocked { msg }
            | SpotError::Forbidden { msg } => vec![msg],
            SpotError::Api(ApiError::Response(r)) => vec![r.message, r.body],
            other => panic!("unexpected {other:?}"),
        };
        for err in [
            parts(400, None, &venue),
            parts(451, None, &long),
            parts(403, None, &long),
            parts(502, None, &long),
        ] {
            for msg in kept(err) {
                assert!(
                    msg.len() <= 512 + "... [truncated]".len(),
                    "kept {} bytes",
                    msg.len()
                );
            }
        }
    }

    #[test]
    fn a_core_response_keeps_binance_s_clamped_wait() {
        let err = parts(503, Some("604800"), "<html>down</html>");
        match &err {
            SpotError::Api(ApiError::Response(r)) => {
                assert_eq!(r.retry_after, Some(MAX_COOLDOWN));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(Classify::retry_after(&err), Some(MAX_COOLDOWN));
    }

    #[test]
    fn every_variant_classifies() {
        let body = r#"{"code":-1121,"msg":"Invalid symbol."}"#;
        let coded = |status, retry_after| parts(status, retry_after, body);
        let code = |c: &str| Some(std::sync::Arc::from(c));
        let week = Some(MAX_COOLDOWN);
        // (error, class, is_fault, retry_after())
        let rows = [
            (
                coded(502, None),
                Class::Unavailable {
                    code: code("-1121"),
                },
                true,
                None,
            ),
            (
                coded(400, None),
                Class::VenueRefusal {
                    code: code("-1121"),
                },
                true,
                None,
            ),
            (
                coded(408, None),
                Class::Unavailable {
                    code: code("-1121"),
                },
                true,
                None,
            ),
            (
                coded(429, Some("7")),
                Class::RateLimited {
                    retry_after: Some(Duration::from_secs(7)),
                },
                true,
                Some(Duration::from_secs(7)),
            ),
            (
                coded(429, None),
                Class::RateLimited { retry_after: None },
                true,
                None,
            ),
            // A ban says when it lifts, though its class has no wait of its own.
            (coded(418, Some("604800")), Class::Restricted, true, week),
            (coded(451, None), Class::Restricted, false, None),
            // Built by hand: a 451 always becomes `RegionBlocked`, but a region
            // block is not a fault whichever variant carries it.
            (
                SpotError::Venue {
                    status: 451,
                    code: -1,
                    msg: "restricted".into(),
                },
                Class::Restricted,
                false,
                None,
            ),
            (coded(403, None), Class::Restricted, true, None),
        ];
        for (err, class, fault, wait) in rows {
            assert_eq!(err.class(), class, "{err:?}");
            assert_eq!(err.is_fault(), fault, "{err:?}");
            assert_eq!(Classify::retry_after(&err), wait, "{err:?}");
            assert_eq!(
                Classify::is_retriable(&err),
                class.is_retriable(),
                "{err:?}"
            );
        }
    }

    #[test]
    fn spot_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SpotError>();
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::error`

Expected: the build fails in `src/spot/error.rs` with `error[E0412]: cannot find type \`SpotError\` in this scope`, `cannot find type \`ApiError\`` and `cannot find value \`MAX_COOLDOWN\``.

- [ ] **Step 3: Implement**

Put this above the `#[cfg(test)] mod tests` block in `polyoxide-binance/src/spot/error.rs`:

```rust
//! Error types for Binance spot.

use std::time::Duration;

use polyoxide_core::{truncate_for_log, ApiError};
use polyoxide_venue::{Class, Classify};
use thiserror::Error;

use crate::shared::error::{decode, venue_class, Decoded};

/// Error type for Binance spot API operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SpotError {
    /// Transport, decoding, or an error body in a shape Binance does not use.
    #[error(transparent)]
    Api(ApiError),

    /// Binance refused the request with its `{"code", "msg"}` body: an unknown
    /// symbol is `400` with code `-1121`.
    #[error("binance answered {status}: {code} {msg}")]
    Venue {
        /// HTTP status.
        status: u16,
        /// Binance's error code, always negative.
        code: i64,
        /// Binance's message, clipped to 512 bytes.
        msg: String,
    },

    /// Still answered `429` after the retry schedule ran out.
    #[error("binance rate limit (429), retry after {retry_after:?}")]
    RateLimited {
        /// The response's `Retry-After`.
        retry_after: Option<Duration>,
    },

    /// `418`: Binance has banned this IP for continuing after a `429`. Every
    /// request on the same [`WeightBudget`](crate::spot::WeightBudget) waits
    /// until the ban lifts.
    #[error("binance has banned this IP (418), retry after {retry_after:?}")]
    IpBanned {
        /// The response's `Retry-After`. The docs give bans of 2 minutes to 3 days.
        retry_after: Option<Duration>,
    },

    /// `451`: Binance does not serve the caller's location.
    #[error("binance does not serve this location (451): {msg}")]
    RegionBlocked {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },

    /// `403`: Binance's web application firewall refused the request.
    #[error("binance's firewall refused the request (403): {msg}")]
    Forbidden {
        /// The response body, clipped to 512 bytes.
        msg: String,
    },
}

impl SpotError {
    /// Binance's error code, for a [`Venue`](Self::Venue) error.
    pub fn code(&self) -> Option<i64> {
        match self {
            Self::Venue { code, .. } => Some(*code),
            _ => None,
        }
    }
}

/// Binance's one decode for spot: an unsuccessful response, by status first,
/// through the crate's shared decode.
///
/// `418`, `429`, `451` and `403` go by status alone; anything else with
/// Binance's `{code, msg}` body is [`Venue`](SpotError::Venue); any other body
/// stays core's [`ApiError::Response`], its message and body clipped and its
/// `Retry-After` clamped to 3 days, as every arm reads it. Every other
/// [`ApiError`] stays core's.
///
/// A `418`'s body is Binance's `-1003` text, which names when the ban ends.
/// The error's fields have no room for it, so it is logged at WARN under
/// `polyoxide_binance::spot::error`; the send loop warns of the hold itself,
/// naming the path.
impl From<ApiError> for SpotError {
    fn from(err: ApiError) -> Self {
        match decode(err) {
            Decoded::Api(err) => Self::Api(err),
            Decoded::Venue { status, code, msg } => Self::Venue { status, code, msg },
            Decoded::RateLimited { retry_after } => Self::RateLimited { retry_after },
            Decoded::IpBanned { retry_after, body } => {
                tracing::warn!("418: IP banned: {}", truncate_for_log(&body));
                Self::IpBanned { retry_after }
            }
            Decoded::RegionBlocked { msg } => Self::RegionBlocked { msg },
            Decoded::Forbidden { msg } => Self::Forbidden { msg },
        }
    }
}

/// `Api` delegates, and `Venue` follows the status rule with Binance's code
/// in decimal. A `429` is `RateLimited`, and a ban, a region block and a
/// firewall refusal are all `Restricted`: Binance documents its `403` as the
/// firewall's, not as a credential failure (D14). So a `429`, and a `408`,
/// `425` or 5xx however its body is shaped, are retriable, and a ban, a region
/// block and a firewall refusal are not: sending again does not change them,
/// and sending after a `418` lengthens the ban.
///
/// A region block is the venue answering as designed, so it is not a fault. A
/// `429`'s or a ban's [`Classify::retry_after`] is its `Retry-After`, clamped
/// to 3 days: for a ban, the time it lifts.
impl Classify for SpotError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue { status, code, .. } => venue_class(*status, *code),
            Self::RateLimited { retry_after } => {
                Class::RateLimited { retry_after: None }.with_retry_after(*retry_after)
            }
            Self::IpBanned { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => {
                Class::Restricted
            }
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::RegionBlocked { .. } => false,
            // Unreachable today, since a 451 is `RegionBlocked`, but a region
            // block whichever variant carries it.
            Self::Venue { status, .. } => *status != 451,
            Self::RateLimited { .. } | Self::IpBanned { .. } | Self::Forbidden { .. } => true,
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::RateLimited { retry_after } | Self::IpBanned { retry_after } => {
                retry_after.filter(|wait| !wait.is_zero())
            }
            Self::Venue { .. } | Self::RegionBlocked { .. } | Self::Forbidden { .. } => None,
        }
    }
}
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::error`

Expected: `test result: ok. 8 passed; 0 failed`. Then `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` passes.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/error.rs
git commit -m "feat(binance): SpotError, decoded through the shared decode (binance spot, Story 4)" -- polyoxide-binance/src/spot/error.rs polyoxide-binance/src/spot/mod.rs
```

### Task 4.3: The exchange types and the `Classify` assertion

**Files:**
- Create: `polyoxide-binance/src/spot/types.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs`
- Modify: `polyoxide-binance/src/lib.rs`
- Test: `polyoxide-binance/src/spot/types.rs` (its `tests` module)

This task needs Story 1's `exchange_info.json` and `time.json`. Before writing the types, list what the captured `exchangeInfo` carries, so the structs below can be checked against it:

```bash
python3 - <<'EOF'
import json
d = json.load(open("polyoxide-binance/tests/fixtures/spot/rest/exchange_info.json"))
print("top:", sorted(d))
print("symbol keys:", sorted({k for s in d["symbols"] for k in s}))
print("statuses:", sorted({s["status"] for s in d["symbols"]}))
print("filter types:", sorted({f["filterType"] for s in d["symbols"] for f in s["filters"]}))
print("exchange filter types:", sorted({f["filterType"] for f in d["exchangeFilters"]}))
print("rate limit keys:", sorted({k for r in d["rateLimits"] for k in r}))
EOF
```

The types below follow Binance's documented spot `exchangeInfo`. The agreement tests in Step 1 are what hold them to the capture: a key the capture carries and a struct lacks, a status or filter type that decodes as `Other`, or a value that does not serialize back to the bytes the server sent, fails them by name. When one fails, add the field (an `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]` when some rows lack it) or the `filters!` entry it names, and run the task again.

- [ ] **Step 1: Write the failing test**

Edit `polyoxide-binance/src/spot/mod.rs`. Replace

```rust
pub mod error;
pub mod weight;
```

with

```rust
pub mod error;
pub mod types;
pub mod weight;
```

Create `polyoxide-binance/src/spot/types.rs` holding only its test module:

```rust
#[cfg(test)]
mod tests {
    use polyoxide_test_support::{agreement, fixtures};

    use super::*;

    fn text(name: &str) -> String {
        fixtures!("spot/rest").text(name)
    }

    #[test]
    fn the_exchange_info_capture_agrees_with_its_types() {
        let text = text("exchange_info");
        let diff = agreement::compare::<ExchangeInfo>("exchange_info", &text);
        assert!(
            diff.unmodelled.is_empty(),
            "the server sent {:?}, which the types do not model",
            diff.unmodelled
        );
        assert!(
            diff.invented.is_empty(),
            "the types emit {:?}, which the server did not send",
            diff.invented
        );
    }

    #[test]
    fn every_symbol_and_filter_serializes_back_to_what_the_server_sent() {
        let text = text("exchange_info");
        let raw: Value = serde_json::from_str(&text).unwrap();
        let info: ExchangeInfo = serde_json::from_str(&text).unwrap();
        assert_eq!(
            serde_json::to_value(&info.rate_limits).unwrap(),
            raw["rateLimits"]
        );
        assert_eq!(
            serde_json::to_value(&info.exchange_filters).unwrap(),
            raw["exchangeFilters"]
        );
        let wire = raw["symbols"].as_array().unwrap();
        assert_eq!(info.symbols.len(), wire.len());
        for (symbol, sent) in info.symbols.iter().zip(wire) {
            assert_eq!(
                serde_json::to_value(symbol).unwrap(),
                *sent,
                "{}",
                symbol.symbol
            );
        }
    }

    #[test]
    fn the_capture_holds_the_cases_the_types_exist_for() {
        let info: ExchangeInfo = serde_json::from_str(&text("exchange_info")).unwrap();
        assert!(info.symbols.len() >= 2, "BTCUSDT and a non-trading symbol");
        assert!(
            info.symbols
                .iter()
                .any(|s| s.symbol == "BTCUSDT" && s.status == SymbolStatus::Trading),
            "BTCUSDT, trading"
        );
        assert!(
            info.symbols
                .iter()
                .any(|s| s.status != SymbolStatus::Trading),
            "a symbol that is not trading"
        );
        // An unknown value would land in `Other` and decode fine; failing here
        // is how a value Binance adds gets modelled instead of passing silently.
        let unnamed: Vec<_> = info
            .symbols
            .iter()
            .filter(|s| matches!(s.status, SymbolStatus::Other(_)))
            .map(|s| (&s.symbol, &s.status))
            .collect();
        assert!(unnamed.is_empty(), "statuses the enum does not name: {unnamed:?}");
        // The same for filters: a misspelled tag would turn a PRICE_FILTER into
        // `Other`, and every other test would still pass.
        let unnamed: Vec<&Filter> = info
            .symbols
            .iter()
            .flat_map(|s| &s.filters)
            .chain(&info.exchange_filters)
            .filter(|f| matches!(f, Filter::Other { .. }))
            .collect();
        assert!(
            unnamed.is_empty(),
            "filter types Filter does not name, add them to `filters!`: {unnamed:?}"
        );
    }

    #[test]
    fn the_captured_btcusdt_filters_decode_to_their_values() {
        let text = text("exchange_info");
        let raw: Value = serde_json::from_str(&text).unwrap();
        let info: ExchangeInfo = serde_json::from_str(&text).unwrap();
        let btc = info.symbols.iter().find(|s| s.symbol == "BTCUSDT").unwrap();
        let wire = raw["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["symbol"] == "BTCUSDT")
            .unwrap();
        let sent = |filter_type: &str, key: &str| -> String {
            wire["filters"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["filterType"] == filter_type)
                .unwrap_or_else(|| panic!("no {filter_type}"))[key]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let price = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::PriceFilter { tick_size, .. } => Some(*tick_size),
                _ => None,
            })
            .expect("a PRICE_FILTER");
        assert_eq!(price.to_string(), sent("PRICE_FILTER", "tickSize"));
        let lot = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::LotSize { step_size, .. } => Some(*step_size),
                _ => None,
            })
            .expect("a LOT_SIZE");
        assert_eq!(lot.to_string(), sent("LOT_SIZE", "stepSize"));
        let market = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::MarketLotSize { step_size, .. } => Some(*step_size),
                _ => None,
            })
            .expect("a MARKET_LOT_SIZE");
        assert_eq!(market.to_string(), sent("MARKET_LOT_SIZE", "stepSize"));
        let notional = btc
            .filters
            .iter()
            .find_map(|f| match f {
                Filter::Notional { min_notional, .. } => Some(*min_notional),
                _ => None,
            })
            .expect("a NOTIONAL");
        assert_eq!(notional.to_string(), sent("NOTIONAL", "minNotional"));
    }

    #[test]
    fn the_time_capture_agrees_with_its_type() {
        let text = text("time");
        let diff = agreement::compare::<ServerTime>("time", &text);
        assert!(diff.unmodelled.is_empty(), "{:?}", diff.unmodelled);
        assert!(diff.invented.is_empty(), "{:?}", diff.invented);
        let time: ServerTime = serde_json::from_str(&text).unwrap();
        assert!(time.server_time > 1_790_000_000_000);
    }

    #[test]
    fn an_unknown_filter_type_is_kept_whole() {
        let raw = serde_json::json!({"filterType": "A_FILTER_FROM_THE_FUTURE", "limit": 7});
        let filter: Filter = serde_json::from_value(raw.clone()).unwrap();
        assert!(matches!(
            &filter,
            Filter::Other { filter_type, raw: kept }
                if filter_type == "A_FILTER_FROM_THE_FUTURE" && *kept == raw
        ));
        assert_eq!(serde_json::to_value(&filter).unwrap(), raw);
    }

    #[test]
    fn a_known_filter_that_loses_a_field_fails() {
        let raw = serde_json::json!({"filterType": "PRICE_FILTER", "minPrice": "0.01000000"});
        assert!(serde_json::from_value::<Filter>(raw).is_err());
    }

    #[test]
    fn a_filter_without_a_type_fails() {
        let raw = serde_json::json!({"minPrice": "0.01000000"});
        let err = serde_json::from_value::<Filter>(raw).unwrap_err();
        assert!(err.to_string().contains("filterType"), "{err}");
    }

    #[test]
    fn a_status_keeps_an_unknown_spelling() {
        assert_eq!(SymbolStatus::from_wire("TRADING"), SymbolStatus::Trading);
        assert_eq!(SymbolStatus::from_wire("PRE_TRADING"), SymbolStatus::PreTrading);
        let unknown = SymbolStatus::from_wire("A_NEW_STATE");
        assert_eq!(unknown, SymbolStatus::Other("A_NEW_STATE".into()));
        assert_eq!(unknown.as_str(), "A_NEW_STATE");
    }

    #[test]
    fn the_status_spellings_are_binance_s() {
        let table = [
            (SymbolStatus::Trading, "TRADING"),
            (SymbolStatus::Break, "BREAK"),
            (SymbolStatus::Halt, "HALT"),
            (SymbolStatus::PreTrading, "PRE_TRADING"),
            (SymbolStatus::PostTrading, "POST_TRADING"),
            (SymbolStatus::EndOfDay, "END_OF_DAY"),
            (SymbolStatus::AuctionMatch, "AUCTION_MATCH"),
        ];
        for (status, wire) in table {
            assert_eq!(status.as_str(), wire);
        }
    }
}
```

Edit `polyoxide-binance/src/lib.rs`. Insert, directly before the two comment lines

```rust
// Every product implements its class's bundle; one that drops a defining
// capability fails the build here.
```

this block:

```rust
// The spot module's error types, in the same assertion `usdm`'s are in.
#[cfg(feature = "spot")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<spot::SpotError>();
    is::<spot::types::InvalidSymbol>();
};

```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::types`

Expected: the build fails in `src/spot/types.rs` with `error[E0412]: cannot find type \`ExchangeInfo\` in this scope` and the same for `ServerTime`, `SymbolStatus`, `Filter` and `Value`, and in `src/lib.rs` with `error[E0433]: failed to resolve: could not find \`InvalidSymbol\` in \`types\``.

- [ ] **Step 3: Implement**

Put this above the `#[cfg(test)] mod tests` block in `polyoxide-binance/src/spot/types.rs`:

```rust
//! Vocabulary and response rows for Binance spot.
//!
//! Prices and quantities are [`Decimal`], decoded from the decimal strings
//! Binance sends, so no value passes through an `f64`. Timestamps are Unix
//! milliseconds. Response rows carry symbols as `String`, taken as sent, so a
//! symbol [`Symbol::new`] would refuse never fails a whole response.

use rust_decimal::Decimal;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub use crate::shared::symbol::{InvalidSymbol, Symbol};

/// `GET /api/v3/time`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ServerTime {
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
}

/// `GET /api/v3/exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExchangeInfo {
    /// Always `UTC`.
    pub timezone: String,
    /// Server clock, Unix milliseconds.
    pub server_time: u64,
    /// The IP's request-weight, raw-request and order limits.
    pub rate_limits: Vec<RateLimit>,
    /// Exchange-wide filters.
    pub exchange_filters: Vec<Filter>,
    /// Every listed symbol, including ones that are not trading.
    pub symbols: Vec<SymbolInfo>,
    /// Smart order routing: the symbols an order for a base asset may be
    /// routed across. Absent when the host sends none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sors: Option<Vec<Sor>>,
}

/// One row of `exchangeInfo`'s `rateLimits`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RateLimit {
    /// `REQUEST_WEIGHT`, `RAW_REQUESTS` or `ORDERS`.
    pub rate_limit_type: String,
    /// `SECOND`, `MINUTE` or `DAY`.
    pub interval: String,
    /// Intervals per window.
    pub interval_num: u32,
    /// The limit per window.
    pub limit: u32,
}

/// One smart-order-routing entry in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Sor {
    /// The base asset the routing applies to.
    pub base_asset: String,
    /// The symbols an order may be routed across.
    pub symbols: Vec<String>,
}

/// One symbol in `exchangeInfo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SymbolInfo {
    /// The symbol, as Binance spells it.
    pub symbol: String,
    /// Trading, halted and so on.
    pub status: SymbolStatus,
    /// Base asset.
    pub base_asset: String,
    /// Decimal places of the base asset.
    pub base_asset_precision: u32,
    /// Quote asset.
    pub quote_asset: String,
    /// Decimal places of the quote asset, as Binance's older key spells it.
    pub quote_precision: u32,
    /// Decimal places of the quote asset.
    pub quote_asset_precision: u32,
    /// Decimal places of the base asset's commission.
    pub base_commission_precision: u32,
    /// Decimal places of the quote asset's commission.
    pub quote_commission_precision: u32,
    /// Order types the symbol accepts.
    pub order_types: Vec<String>,
    /// Whether iceberg orders are allowed.
    pub iceberg_allowed: bool,
    /// Whether OCO order lists are allowed.
    pub oco_allowed: bool,
    /// Whether OTO order lists are allowed.
    pub oto_allowed: bool,
    /// Whether OPO order lists are allowed. Absent on a host that predates them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opo_allowed: Option<bool>,
    /// Whether a market order may name a quote quantity.
    pub quote_order_qty_market_allowed: bool,
    /// Whether trailing stops are allowed.
    pub allow_trailing_stop: bool,
    /// Whether cancel-replace is allowed.
    pub cancel_replace_allowed: bool,
    /// Whether order amendment is allowed. Absent on a host that predates it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amend_allowed: Option<bool>,
    /// Whether pegged orders are allowed. Absent on a host that predates them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peg_instructions_allowed: Option<bool>,
    /// Whether the symbol can be traded as spot.
    pub is_spot_trading_allowed: bool,
    /// Whether the symbol can be traded on margin.
    pub is_margin_trading_allowed: bool,
    /// Order filters.
    pub filters: Vec<Filter>,
    /// Permissions the symbol is open to (older key).
    pub permissions: Vec<String>,
    /// Permission sets: an account needs every permission of one set.
    pub permission_sets: Vec<Vec<String>>,
    /// The self-trade-prevention mode applied when an order names none.
    pub default_self_trade_prevention_mode: String,
    /// The self-trade-prevention modes an order may name.
    pub allowed_self_trade_prevention_modes: Vec<String>,
}

polyoxide_venue::open_enum! {
    /// A symbol's trading status, from `exchangeInfo`. The variants are the
    /// list in Binance's common definitions.
    pub enum SymbolStatus {
        Trading => "TRADING",
        Break => "BREAK",
        Halt => "HALT",
        PreTrading => "PRE_TRADING",
        PostTrading => "POST_TRADING",
        EndOfDay => "END_OF_DAY",
        AuctionMatch => "AUCTION_MATCH",
    }
}

/// Declares [`Filter`] and the private wire enum its serde goes through from
/// one table: each entry is a variant, its wire `filterType`, and its fields
/// (camelCase on the wire, a decimal string where `=> "..."` names the
/// `serde(with)`). Every field type is `Copy`, which the serializer relies on.
macro_rules! filters {
    ($(
        $(#[$vmeta:meta])*
        $variant:ident = $tag:literal {
            $( $field:ident : $ty:ty $(=> $with:literal)? ),* $(,)?
        }
    )+) => {
        /// An `exchangeInfo` filter, named after its wire `filterType`.
        ///
        /// A filter type this version does not know is kept whole in
        /// [`Filter::Other`], so a new one never fails the response. A known
        /// type that loses or retypes a field still fails; one that gains a
        /// field decodes without it, and the wire-agreement tests are what
        /// notice.
        #[derive(Debug, Clone, PartialEq)]
        #[non_exhaustive]
        pub enum Filter {
            $(
                $(#[$vmeta])*
                #[non_exhaustive]
                $variant { $( $field: $ty ),* },
            )+
            /// A filter type this version does not model.
            Other {
                /// The wire `filterType`.
                filter_type: String,
                /// The whole filter object, `filterType` included.
                raw: Value,
            },
        }

        /// The wire bodies of the known filters, which `Filter`'s serde goes
        /// through.
        #[derive(Serialize, Deserialize)]
        #[serde(tag = "filterType")]
        enum FilterWire {
            $(
                #[serde(rename = $tag, rename_all = "camelCase")]
                $variant { $( $(#[serde(with = $with)])? $field: $ty ),* },
            )+
        }

        impl FilterWire {
            /// Every `filterType` this version models.
            const KNOWN: &'static [&'static str] = &[$($tag),+];
        }

        impl From<FilterWire> for Filter {
            fn from(wire: FilterWire) -> Self {
                match wire {
                    $( FilterWire::$variant { $($field),* } => Self::$variant { $($field),* }, )+
                }
            }
        }

        impl<'de> Deserialize<'de> for Filter {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = Value::deserialize(deserializer)?;
                let filter_type = raw
                    .get("filterType")
                    .and_then(Value::as_str)
                    .ok_or_else(|| <D::Error as de::Error>::custom("a filter without a string filterType"))?
                    .to_owned();
                if !FilterWire::KNOWN.contains(&filter_type.as_str()) {
                    return Ok(Self::Other { filter_type, raw });
                }
                let wire: FilterWire =
                    serde_json::from_value(raw).map_err(<D::Error as de::Error>::custom)?;
                Ok(wire.into())
            }
        }

        impl Serialize for Filter {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let wire = match self {
                    $( Self::$variant { $($field),* } => FilterWire::$variant { $($field: *$field),* }, )+
                    Self::Other { raw, .. } => return raw.serialize(serializer),
                };
                wire.serialize(serializer)
            }
        }
    };
}

filters! {
    /// `PRICE_FILTER`.
    PriceFilter = "PRICE_FILTER" {
        min_price: Decimal => "rust_decimal::serde::str",
        max_price: Decimal => "rust_decimal::serde::str",
        tick_size: Decimal => "rust_decimal::serde::str",
    }
    /// `PERCENT_PRICE`.
    PercentPrice = "PERCENT_PRICE" {
        multiplier_up: Decimal => "rust_decimal::serde::str",
        multiplier_down: Decimal => "rust_decimal::serde::str",
        avg_price_mins: u32,
    }
    /// `PERCENT_PRICE_BY_SIDE`.
    PercentPriceBySide = "PERCENT_PRICE_BY_SIDE" {
        bid_multiplier_up: Decimal => "rust_decimal::serde::str",
        bid_multiplier_down: Decimal => "rust_decimal::serde::str",
        ask_multiplier_up: Decimal => "rust_decimal::serde::str",
        ask_multiplier_down: Decimal => "rust_decimal::serde::str",
        avg_price_mins: u32,
    }
    /// `LOT_SIZE`: limit orders.
    LotSize = "LOT_SIZE" {
        min_qty: Decimal => "rust_decimal::serde::str",
        max_qty: Decimal => "rust_decimal::serde::str",
        step_size: Decimal => "rust_decimal::serde::str",
    }
    /// `MIN_NOTIONAL`: the older minimum-value filter.
    MinNotional = "MIN_NOTIONAL" {
        min_notional: Decimal => "rust_decimal::serde::str",
        apply_to_market: bool,
        avg_price_mins: u32,
    }
    /// `NOTIONAL`: the minimum and maximum order value.
    Notional = "NOTIONAL" {
        min_notional: Decimal => "rust_decimal::serde::str",
        apply_min_to_market: bool,
        max_notional: Decimal => "rust_decimal::serde::str",
        apply_max_to_market: bool,
        avg_price_mins: u32,
    }
    /// `ICEBERG_PARTS`.
    IcebergParts = "ICEBERG_PARTS" {
        limit: u32,
    }
    /// `MARKET_LOT_SIZE`: market orders.
    MarketLotSize = "MARKET_LOT_SIZE" {
        min_qty: Decimal => "rust_decimal::serde::str",
        max_qty: Decimal => "rust_decimal::serde::str",
        step_size: Decimal => "rust_decimal::serde::str",
    }
    /// `MAX_NUM_ORDERS`.
    MaxNumOrders = "MAX_NUM_ORDERS" {
        max_num_orders: u32,
    }
    /// `MAX_NUM_ALGO_ORDERS`.
    MaxNumAlgoOrders = "MAX_NUM_ALGO_ORDERS" {
        max_num_algo_orders: u32,
    }
    /// `MAX_NUM_ICEBERG_ORDERS`.
    MaxNumIcebergOrders = "MAX_NUM_ICEBERG_ORDERS" {
        max_num_iceberg_orders: u32,
    }
    /// `MAX_POSITION`.
    MaxPosition = "MAX_POSITION" {
        max_position: Decimal => "rust_decimal::serde::str",
    }
    /// `TRAILING_DELTA`.
    TrailingDelta = "TRAILING_DELTA" {
        min_trailing_above_delta: u64,
        max_trailing_above_delta: u64,
        min_trailing_below_delta: u64,
        max_trailing_below_delta: u64,
    }
    /// `MAX_NUM_ORDER_AMENDS`.
    MaxNumOrderAmends = "MAX_NUM_ORDER_AMENDS" {
        max_num_order_amends: u32,
    }
    /// `MAX_NUM_ORDER_LISTS`.
    MaxNumOrderLists = "MAX_NUM_ORDER_LISTS" {
        max_num_order_lists: u32,
    }
    /// `EXCHANGE_MAX_NUM_ORDERS`: an exchange-wide filter.
    ExchangeMaxNumOrders = "EXCHANGE_MAX_NUM_ORDERS" {
        max_num_orders: u32,
    }
    /// `EXCHANGE_MAX_NUM_ALGO_ORDERS`: an exchange-wide filter.
    ExchangeMaxNumAlgoOrders = "EXCHANGE_MAX_NUM_ALGO_ORDERS" {
        max_num_algo_orders: u32,
    }
    /// `EXCHANGE_MAX_NUM_ICEBERG_ORDERS`: an exchange-wide filter.
    ExchangeMaxNumIcebergOrders = "EXCHANGE_MAX_NUM_ICEBERG_ORDERS" {
        max_num_iceberg_orders: u32,
    }
    /// `EXCHANGE_MAX_NUM_ORDER_LISTS`: an exchange-wide filter.
    ExchangeMaxNumOrderLists = "EXCHANGE_MAX_NUM_ORDER_LISTS" {
        max_num_order_lists: u32,
    }
}
```

If the capture holds a filter type this table lacks, `the_capture_holds_the_cases_the_types_exist_for` names it: add one entry to `filters!` with the keys the capture shows for it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::types`

Expected: `test result: ok. 10 passed; 0 failed`. Then:

- `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` passes.
- `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'` passes: `SpotError` and `InvalidSymbol` are in `lib.rs`'s assertion.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/types.rs
git commit -m "feat(binance): spot's exchange types and the Classify assertion (binance spot, Story 4)" -- polyoxide-binance/src/spot/types.rs polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/lib.rs
```

### Task 4.4: `Spot`, its builder, the retry policy, the routed request and `health`

**Files:**
- Create: `polyoxide-binance/src/spot/policy.rs`
- Create: `polyoxide-binance/src/spot/request.rs`
- Create: `polyoxide-binance/src/spot/api/mod.rs`
- Create: `polyoxide-binance/src/spot/api/health.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs` (replaced whole)
- Modify: `.claude/rules/rate-limits.md` (one `paths:` glob)
- Modify: `.claude/rules/send-loop.md` (one `paths:` glob)
- Modify: `.claude/rules/clients.md` (one `paths:` glob)
- Test: `polyoxide-binance/src/spot/policy.rs`, `polyoxide-binance/src/spot/mod.rs` (their `tests` modules)

- [ ] **Step 1: Write the failing test**

Create `polyoxide-binance/src/spot/policy.rs` holding only its test module:

```rust
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_core::reqwest::{
        header::{HeaderMap, HeaderValue, RETRY_AFTER},
        StatusCode,
    };
    use polyoxide_core::Outcome;

    use super::*;
    use crate::spot::weight::{DEFAULT_BAN, MAX_COOLDOWN};

    /// 2026-11-02 12:00:15 UTC: 45 s before a minute boundary.
    const FIFTEEN_INTO_A_MINUTE: u64 = 1_793_620_815_000;

    fn policy() -> SpotRetryPolicy {
        SpotRetryPolicy {
            budget: WeightBudget::at_ms(FIFTEEN_INTO_A_MINUTE),
        }
    }

    /// One retry at a 100ms base, so a retry's own backoff is 75-125ms.
    fn schedule() -> RetryConfig {
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 100,
            max_backoff_ms: 10_000,
        }
    }

    fn decide(status: u16, retry_after: Option<&str>, attempt: u32) -> Decision {
        let mut headers = HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(RETRY_AFTER, HeaderValue::from_str(value).unwrap());
        }
        let schedule = schedule();
        policy().decide(
            &ResponseMeta {
                status: StatusCode::from_u16(status).unwrap(),
                headers: &headers,
            },
            &schedule.attempt_info(attempt),
            &schedule,
        )
    }

    #[test]
    fn a_2xx_is_done() {
        assert_eq!(
            decide(200, Some("5"), 0),
            Decision {
                outcome: Outcome::Done,
                hold: None
            }
        );
    }

    #[test]
    fn a_429_with_a_retry_left_retries_and_holds_the_longer_of_its_wait_and_retry_after() {
        let asked = decide(429, Some("1"), 0);
        assert_eq!(asked.outcome, Outcome::Retry(Duration::ZERO));
        assert_eq!(asked.hold, Some(Duration::from_secs(1)));
        let unasked = decide(429, None, 0);
        assert_eq!(unasked.outcome, Outcome::Retry(Duration::ZERO));
        let hold = unasked.hold.unwrap().as_millis();
        assert!((75..=125).contains(&hold), "{hold}ms");
    }

    #[test]
    fn a_429_with_none_left_holds_to_the_next_minute_of_this_budget() {
        // Spot's budget reads its own clock: 45 s are left of the minute.
        assert_eq!(
            decide(429, None, 1),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(45))
            }
        );
        assert_eq!(
            decide(429, Some("3"), 1),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(3))
            }
        );
    }

    #[test]
    fn a_418_fails_and_holds_its_retry_after_or_two_minutes() {
        assert_eq!(
            decide(418, Some("1"), 0),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(Duration::from_secs(1))
            }
        );
        assert_eq!(
            decide(418, None, 0),
            Decision {
                outcome: Outcome::Fail,
                hold: Some(DEFAULT_BAN)
            }
        );
        assert_eq!(DEFAULT_BAN, Duration::from_secs(120));
        assert_eq!(
            decide(418, Some("864000"), 0).hold,
            Some(MAX_COOLDOWN),
            "ten days is held for the longest documented ban"
        );
    }

    #[test]
    fn a_425_and_a_5xx_fail_with_no_hold() {
        for status in [425, 500, 503, 400, 403, 451] {
            assert_eq!(
                decide(status, Some("1"), 0),
                Decision {
                    outcome: Outcome::Fail,
                    hold: None
                },
                "{status}"
            );
        }
    }
}
```

Edit `polyoxide-binance/src/spot/mod.rs`. Replace

```rust
pub mod error;
pub mod types;
pub mod weight;
```

with

```rust
pub mod api;
pub mod error;
mod policy;
pub mod request;
pub mod types;
pub mod weight;
```

and append this test module at the end of the file:

```rust

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_builder_targets_the_production_host() {
        let spot = Spot::new().expect("client builds");
        assert_eq!(spot.http.base_url.as_str(), "https://api.binance.com/");
    }

    #[test]
    fn a_bad_base_url_is_a_url_error() {
        let err = Spot::builder().base_url("not a url").build().unwrap_err();
        assert!(matches!(
            err,
            SpotError::Api(polyoxide_core::ApiError::Url(_))
        ));
    }

    #[test]
    fn clients_given_one_budget_share_it() {
        let budget = WeightBudget::new();
        let a = Spot::builder()
            .weight_budget(budget.clone())
            .build()
            .unwrap();
        let b = Spot::builder().weight_budget(budget).build().unwrap();
        assert!(a.weight_budget().is_shared_with(b.weight_budget()));
        let c = Spot::new().unwrap();
        assert!(!a.weight_budget().is_shared_with(c.weight_budget()));
    }

    #[tokio::test]
    async fn test_default_concurrency_limit_is_4() {
        let spot = Spot::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(spot.http.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|p| p.is_some()));

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            spot.http.acquire_concurrency(),
        )
        .await;
        assert!(
            result.is_err(),
            "5th permit should block with default limit of 4"
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib`

Expected: the build fails with `error[E0583]: file not found for module \`api\``, the same for `request`, and `error[E0433]` / `error[E0412]` for `Spot` and `SpotRetryPolicy` in the tests. (`policy.rs` holds only its test module, so `mod policy;` also fails to find `SpotRetryPolicy`.)

- [ ] **Step 3: Implement**

Put this above the `#[cfg(test)] mod tests` block in `polyoxide-binance/src/spot/policy.rs`:

```rust
//! [`SpotRetryPolicy`]: what follows each spot response.

use polyoxide_core::{AttemptInfo, Decision, ResponseMeta, RetryConfig, RetryPolicy};

use crate::spot::weight::WeightBudget;

/// Binance spot's retry policy, over the [`WeightBudget`] whose minute it reads.
///
/// It is the USDⓈ-M module's policy, decided by the crate's shared
/// `decide`:
///
/// - A `429` with a retry left is retried, and holds every request on the
///   budget for the longer of the attempt's backoff and its `Retry-After`.
/// - A `429` with none left fails, and holds for its `Retry-After`, or until
///   the next UTC minute when it has none: sending again into a spent minute
///   is how a `429` becomes a `418` ban.
/// - A `418` (the IP is banned) fails, and holds for its `Retry-After`, or
///   [`DEFAULT_BAN`](crate::spot::weight::DEFAULT_BAN) when it has none.
/// - A 2xx is done. Everything else fails with no hold, a `425` included:
///   Binance does not document it as retriable.
///
/// A `Retry-After` is clamped to
/// [`MAX_COOLDOWN`](crate::spot::weight::MAX_COOLDOWN), the longest ban
/// Binance documents.
#[derive(Debug, Clone)]
pub(crate) struct SpotRetryPolicy {
    pub(crate) budget: WeightBudget,
}

impl RetryPolicy for SpotRetryPolicy {
    fn decide(
        &self,
        response: &ResponseMeta<'_>,
        attempt: &AttemptInfo,
        schedule: &RetryConfig,
    ) -> Decision {
        crate::shared::policy::decide(
            response,
            attempt,
            schedule,
            self.budget.until_next_minute(),
        )
    }
}
```

Add `until_next_minute` to `WeightBudget` in `polyoxide-binance/src/spot/weight.rs`, directly after `used`:

```rust

    /// Time left in the current UTC minute: what a `429` with no `Retry-After`
    /// and no retry left holds for.
    pub(crate) fn until_next_minute(&self) -> Duration {
        self.inner.until_next_minute()
    }
```

Create `polyoxide-binance/src/spot/request.rs`:

```rust
//! What every route builder sends through: core's request builder, with the
//! route's weight as its cost. The routed request itself is the crate's
//! shared one; this module fixes its route and error type to spot's.

use crate::spot::{error::SpotError, weight::Route};

pub use crate::shared::weight::USED_WEIGHT_HEADER;

/// A route and its query, sent as core's `Request` with the route's cost.
///
/// The route builders wrap one. Setting a query parameter twice replaces it,
/// and a parameter that changes the weight replaces the route with it.
pub(crate) type Routed<T> = crate::shared::request::Routed<T, Route, SpotError>;

/// A route builder with no parameters of its own beyond those its namespace
/// method sets: `cost` and `send`, like the builders with setters.
macro_rules! route_builder {
    ($(#[$doc:meta])* $name:ident => $output:ty) => {
        $crate::shared::request::route_builder! {
            $(#[$doc])* $name => $output; module = spot, error = SpotError
        }
    };
}

pub(crate) use route_builder;
```

Create `polyoxide-binance/src/spot/api/mod.rs`:

```rust
//! API namespaces, one module per group of routes.

pub mod health;
```

Create `polyoxide-binance/src/spot/api/health.rs`:

```rust
//! Liveness routes: `/api/v3/ping` and `/api/v3/time`.

use std::time::Duration;

use polyoxide_core::{decode_json, ApiError, HttpClient};
use serde::Deserialize;

use crate::spot::{
    error::SpotError,
    request::{route_builder, Routed},
    types::ServerTime,
    weight::Route,
};

/// Health namespace.
#[derive(Debug, Clone)]
pub struct Health {
    pub(crate) http: HttpClient,
}

/// `ping`'s body, `{}`.
#[derive(Deserialize)]
struct Empty {}

impl Health {
    /// Round-trip time to the host, via `GET /api/v3/ping` (weight 1).
    ///
    /// The latency is that of the attempt that answered, as
    /// [`HttpClient::health`] times it.
    pub async fn ping(&self) -> Result<Duration, SpotError> {
        let route = Route::Ping;
        let pong = self
            .http
            .health::<SpotError>(route.path(), &[route.cost().into()])
            .await?;
        let text = pong.response.text().await.map_err(ApiError::from)?;
        let _: Empty = decode_json(route.path(), &text).map_err(ApiError::from)?;
        Ok(pong.round_trip)
    }

    /// Server time, via `GET /api/v3/time` (weight 1).
    pub fn time(&self) -> GetTime {
        GetTime {
            request: Routed::new(&self.http, Route::Time),
        }
    }
}

route_builder! {
    /// Request builder for `GET /api/v3/time`.
    GetTime => ServerTime
}
```

Edit `polyoxide-binance/src/spot/mod.rs`. Replace the module docs' second sentence block, from the line `//! Client for Binance spot public market data` to the end of the `//!` docs (the lines above `pub mod api;`), with:

```rust
//! Client for Binance spot public market data (`api.binance.com`), behind the
//! `spot` feature, which is on by default. No credentials are needed.
//!
//! Every request is charged against a [`WeightBudget`], because Binance limits
//! each IP by request *weight*, which varies by route and parameters, rather
//! than by request count. The budget is 5400 of the published 6000 per minute;
//! the venue's `RAW_REQUESTS` limit is not modelled, because every request
//! weighs at least 1 and the budget therefore caps a client at 27,000 requests
//! per five minutes, under that limit's 300,000.
//!
//! ```no_run
//! use polyoxide_binance::spot::Spot;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let spot = Spot::new()?;
//! let time = spot.health().time().send().await?;
//! println!("server time {}", time.server_time);
//! # Ok(())
//! # }
//! ```
```

Then insert, directly after the line `pub use weight::WeightBudget;`:

```rust

use polyoxide_core::{ClientConfig, HttpClient};

use crate::spot::{api::health::Health, policy::SpotRetryPolicy};

/// Production spot REST host.
pub const DEFAULT_BASE_URL: &str = "https://api.binance.com";

/// In-flight requests the client allows by default, as in the sibling crates.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for Binance spot public market data. No credentials are needed.
#[derive(Debug, Clone)]
pub struct Spot {
    http: HttpClient,
    budget: WeightBudget,
}

impl Spot {
    /// A client with default settings and its own [`WeightBudget`].
    pub fn new() -> Result<Self, SpotError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> SpotBuilder {
        SpotBuilder::new()
    }

    polyoxide_core::namespaces! { http;
        /// Liveness: `ping`, `time`.
        health: Health,
    }

    /// The budget this client charges.
    pub fn weight_budget(&self) -> &WeightBudget {
        &self.budget
    }
}

/// Builder for [`Spot`].
///
/// It allows [`DEFAULT_MAX_CONCURRENT`] in-flight requests unless told
/// otherwise. Keep the weight in flight under the budget's reserve of 600: at
/// the default, one client has at most 320 in flight on a route at its default
/// (4 × `tickers_24h`'s 80), and four `depth` calls with a `limit` above 1000
/// (250 each, 1000 in all) pass it. `WeightBudget` explains why a request in
/// flight across a minute boundary is not counted by it.
#[derive(Debug)]
pub struct SpotBuilder {
    config: ClientConfig,
    budget: Option<WeightBudget>,
}

impl SpotBuilder {
    fn new() -> Self {
        Self {
            config: ClientConfig::new(DEFAULT_BASE_URL, DEFAULT_MAX_CONCURRENT),
            budget: None,
        }
    }

    polyoxide_core::client_config_setters!(config);

    /// Charge this budget instead of a new one. Binance limits weight per IP,
    /// so every client in a process should share one budget.
    pub fn weight_budget(mut self, budget: WeightBudget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Build the client.
    ///
    /// It has no core `RateLimiter`: the [`WeightBudget`] is its throttle, and
    /// paces every request instead. Its retry policy holds that budget on a
    /// `429` or a `418`, so every client sharing it waits.
    pub fn build(self) -> Result<Spot, SpotError> {
        let budget = self.budget.unwrap_or_default();
        let http = self
            .config
            .http_builder()
            .with_throttle(budget.clone())
            .with_retry_policy(SpotRetryPolicy {
                budget: budget.clone(),
            })
            .build()?;
        Ok(Spot { http, budget })
    }
}
```

Then widen the `paths:` globs for the files this task creates, in the same commit. Change no rule's body; anchor on the `spot` items Task 4.1 added, whose text is below, and leave Story 2's `shared` items alone.

- In `.claude/rules/rate-limits.md`, replace the list item

```yaml
  - "polyoxide-binance/src/spot/weight.rs"
```

  with

```yaml
  - "polyoxide-binance/src/spot/{weight,policy}.rs"
```

- In `.claude/rules/clients.md`, replace the list item

```yaml
  - "polyoxide-binance/src/spot/mod.rs"
```

  with

```yaml
  - "polyoxide-binance/src/spot/{mod,request}.rs"
```

- In `.claude/rules/send-loop.md`'s frontmatter, directly after the `paths:` list item that names `polyoxide-binance/src/usdm/{policy,request}.rs` (and after any `shared` item Story 2 put beside it), add this list item, indented as its neighbours are:

```yaml
  - "polyoxide-binance/src/spot/{policy,request}.rs"
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib`

Expected: `test result: ok.` for every unit test (`spot::weight` 7, `spot::error` 8, `spot::types` 10, `spot::policy` 5, `spot::tests` 4). Then:

- `bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'` passes (every alternative of the three new globs matches a file).
- `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --doc` passes (the module docs' example compiles under `spot` alone).
- `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` passes.
- `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'` passes (the intra-doc links in `spot::weight`, `spot::error` and `spot::policy` resolve).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/policy.rs polyoxide-binance/src/spot/request.rs polyoxide-binance/src/spot/api/mod.rs polyoxide-binance/src/spot/api/health.rs
git commit -m "feat(binance): the Spot client, its retry policy and the health routes (binance spot, Story 4)" -- polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/spot/policy.rs polyoxide-binance/src/spot/request.rs polyoxide-binance/src/spot/weight.rs polyoxide-binance/src/spot/api/mod.rs polyoxide-binance/src/spot/api/health.rs .claude/rules/rate-limits.md .claude/rules/send-loop.md .claude/rules/clients.md
```

### Task 4.5: The `exchange` namespace

**Files:**
- Create: `polyoxide-binance/src/spot/api/exchange.rs`
- Modify: `polyoxide-binance/src/spot/api/mod.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs`
- Test: `polyoxide-binance/src/spot/mod.rs` (its `tests` module)

- [ ] **Step 1: Write the failing test**

Append to the `tests` module of `polyoxide-binance/src/spot/mod.rs`, after `test_default_concurrency_limit_is_4`:

```rust

    #[test]
    fn the_namespaces_share_the_client_s_http_client() {
        // Both namespaces are cut from one client, so a hold set through one
        // is the other's.
        let spot = Spot::new().unwrap();
        let _health = spot.health();
        let exchange = spot.exchange();
        assert_eq!(exchange.exchange_info().cost(), weight::Cost::Weight(20));
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::tests`

Expected: the build fails with `error[E0599]: no method named \`exchange\` found for struct \`Spot\` in the current scope`.

- [ ] **Step 3: Implement**

Replace `polyoxide-binance/src/spot/api/mod.rs` with

```rust
//! API namespaces, one module per group of routes.

pub mod exchange;
pub mod health;
```

Create `polyoxide-binance/src/spot/api/exchange.rs`:

```rust
//! Reference data: `/api/v3/exchangeInfo`.

use polyoxide_core::HttpClient;

use crate::spot::{
    error::SpotError,
    request::Routed,
    types::{ExchangeInfo, Symbol},
    weight::{Cost, Route},
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) http: HttpClient,
}

impl ExchangeApi {
    /// `GET /api/v3/exchangeInfo` (weight 20 with a symbol or without): every
    /// symbol, its filters, and the IP's limits.
    pub fn exchange_info(&self) -> GetExchangeInfo {
        GetExchangeInfo {
            request: Routed::new(&self.http, Route::ExchangeInfo),
        }
    }
}

/// Request builder for `GET /api/v3/exchangeInfo`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetExchangeInfo {
    request: Routed<ExchangeInfo>,
}

impl GetExchangeInfo {
    polyoxide_core::query_setters! {
        /// Restrict the listing to one symbol.
        symbol: &Symbol => "symbol",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Waits for the budget, sends, and decodes the response.
    ///
    /// A `429` holds every request on the budget, then this request is
    /// retried on the client's schedule; with no retry left and no
    /// `Retry-After`, the hold runs to the next UTC minute. A `418` holds
    /// every request for its `Retry-After`, or 2 minutes, and is not retried.
    ///
    /// The wait has no deadline of its own: a request can wait out a minute's
    /// spent budget or a ban, which Binance documents at up to 3 days. Wrap
    /// the call in `tokio::time::timeout` where a caller needs a deadline.
    pub async fn send(self) -> Result<ExchangeInfo, SpotError> {
        self.request.send().await
    }
}
```

In `polyoxide-binance/src/spot/mod.rs`, replace the module docs' example

```rust
//! use polyoxide_binance::spot::Spot;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let spot = Spot::new()?;
//! let time = spot.health().time().send().await?;
//! println!("server time {}", time.server_time);
//! # Ok(())
//! # }
//! ```
```

with

```rust
//! use polyoxide_binance::spot::{types::Symbol, Spot};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let spot = Spot::new()?;
//! let btc = Symbol::new("BTCUSDT")?;
//! let info = spot.exchange().exchange_info().symbol(&btc).send().await?;
//! println!("{} symbols listed", info.symbols.len());
//! # Ok(())
//! # }
//! ```
```

Replace

```rust
use crate::spot::{api::health::Health, policy::SpotRetryPolicy};
```

with

```rust
use crate::spot::{
    api::{exchange::ExchangeApi, health::Health},
    policy::SpotRetryPolicy,
};
```

and replace

```rust
        /// Liveness: `ping`, `time`.
        health: Health,
    }
```

with

```rust
        /// Liveness: `ping`, `time`.
        health: Health,
        /// Reference data: `exchangeInfo`.
        exchange: ExchangeApi,
    }
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::tests && nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --doc`

Expected: `test result: ok. 5 passed; 0 failed` for the unit tests, and the doctests pass (the module docs' example now reads the exchange listing). Then `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` passes.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/api/exchange.rs
git commit -m "feat(binance): the spot exchange namespace (binance spot, Story 4)" -- polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/spot/api/mod.rs polyoxide-binance/src/spot/api/exchange.rs
```

### Task 4.6: The mock suite

**Files:**
- Create: `polyoxide-binance/tests/mock_spot.rs`
- Modify: `polyoxide-binance/Cargo.toml` (one `[[test]]` stanza)
- Test: `polyoxide-binance/tests/mock_spot.rs`

- [ ] **Step 1: Write the failing test**

Edit `polyoxide-binance/Cargo.toml`. Insert directly before

```toml
[[test]]
name = "mock_usdm"
required-features = ["usdm"]
```

this stanza:

```toml
[[test]]
name = "mock_spot"
required-features = ["spot"]

```

Create `polyoxide-binance/tests/mock_spot.rs`:

```rust
//! Mock-server tests for the spot module: its routes' paths and exact
//! queries, the decoding of a captured body, error mapping, and the budget's
//! response to the server. Story 5 appends the market routes after the last
//! test.

use std::time::{Duration, Instant};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::spot::{types::Symbol, weight::Cost, Spot, SpotError, WeightBudget};
use polyoxide_core::RetryConfig;
use polyoxide_test_support::{fixtures, minute};

fn spot(server: &ServerGuard) -> Spot {
    Spot::builder().base_url(server.url()).build().unwrap()
}

fn spot_with_retries(server: &ServerGuard, config: RetryConfig) -> Spot {
    Spot::builder()
        .base_url(server.url())
        .with_retry_config(config)
        .build()
        .unwrap()
}

/// A captured body, by file name without its `.json`, from
/// `tests/fixtures/spot/rest/`.
fn fixture(name: &str) -> String {
    fixtures!("spot/rest").text(name)
}

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

/// A route answering `body`, matched on its whole query string so a missing
/// or extra parameter fails the request.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .create_async()
        .await
}

/// `GET /api/v3/time` failing with `status`, `headers` and `body`, whatever
/// its query.
async fn failing(
    server: &mut ServerGuard,
    status: usize,
    headers: &[(&str, &str)],
    body: &str,
) -> Mock {
    let mut mock = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    for (name, value) in headers {
        mock = mock.with_header(*name, value);
    }
    mock.create_async().await
}

/// Waits out the end of a minute, so a test that holds the budget for a moment
/// cannot see the window roll over underneath it.
async fn clear_of_a_minute_boundary() {
    minute::wait_unless_within(0..=57_000, 100).await;
}

// ── health and exchange ─────────────────────────────────────────

#[tokio::test]
async fn ping_reports_latency() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/api/v3/ping", "", "{}").await;
    let latency = spot(&server).health().ping().await.unwrap();
    mock.assert_async().await;
    assert!(latency < Duration::from_secs(5));
}

#[tokio::test]
async fn a_ping_whose_body_does_not_decode_is_an_error() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/api/v3/ping", "", "<html>").await;
    let result = spot(&server).health().ping().await;
    mock.assert_async().await;
    assert!(result.is_err(), "{result:?}");
}

#[tokio::test]
async fn a_ping_is_charged_its_weight() {
    // No weight header, so only the client's own charge can count the ping.
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v3/ping")
        .match_query(Matcher::Missing)
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;
    let client = spot(&server);
    client.health().ping().await.unwrap();
    mock.assert_async().await;
    assert_eq!(client.weight_budget().used(), 1);
}

#[tokio::test]
async fn a_retried_ping_reports_the_answering_attempt() {
    // The ping runs on `HttpClient::health`, so its latency is the round trip
    // of the attempt that answered, as every venue's is, not the whole call
    // with the wait for the budget and the retry.
    let mut server = Server::new_async().await;
    let served = std::sync::atomic::AtomicUsize::new(0);
    let mock = server
        .mock("GET", "/api/v3/ping")
        .match_query(Matcher::Missing)
        .with_status_code_from_request(move |_| {
            match served.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => 429,
                _ => 200,
            }
        })
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body("{}")
        .expect(2)
        .create_async()
        .await;
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        },
    );

    let start = Instant::now();
    let latency = client.health().ping().await.expect("retried to the 200");
    let elapsed = start.elapsed();
    mock.assert_async().await;
    assert!(
        elapsed >= Duration::from_millis(300),
        "the call took {elapsed:?}, inside the retry's 300ms floor"
    );
    assert!(
        elapsed.saturating_sub(latency) >= Duration::from_millis(300),
        "the latency is the answering attempt's, without the backoff: {latency:?} of {elapsed:?}"
    );
}

#[tokio::test]
async fn time_returns_the_server_clock() {
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/api/v3/time", "", &fixture("time")).await;
    let time = spot(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert!(time.server_time > 1_790_000_000_000);
}

#[tokio::test]
async fn exchange_info_decodes_the_captured_body() {
    let mut server = Server::new_async().await;
    let body = fixture("exchange_info");
    let mock = route(&mut server, "/api/v3/exchangeInfo", "", &body).await;
    let info = spot(&server).exchange().exchange_info().send().await.unwrap();
    mock.assert_async().await;

    let raw: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(info.timezone, "UTC");
    assert_eq!(info.symbols.len(), raw["symbols"].as_array().unwrap().len());
    assert!(info.symbols.iter().any(|s| s.symbol == "BTCUSDT"));
    // Binance's published weight, which the budget's 5400 is a tenth under.
    assert!(
        info.rate_limits.iter().any(|l| l.rate_limit_type == "REQUEST_WEIGHT"
            && l.interval == "MINUTE"
            && l.interval_num == 1
            && l.limit == polyoxide_binance::spot::weight::PUBLISHED_WEIGHT_PER_MINUTE),
        "{:?}",
        info.rate_limits
    );
}

#[tokio::test]
async fn exchange_info_sends_a_symbol_only_when_asked() {
    let mut server = Server::new_async().await;
    let body = fixture("exchange_info");
    let one = route(&mut server, "/api/v3/exchangeInfo", "symbol=BTCUSDT", &body).await;
    let all = route(&mut server, "/api/v3/exchangeInfo", "", &body).await;
    let client = spot(&server);
    client
        .exchange()
        .exchange_info()
        .symbol(&btc())
        .send()
        .await
        .unwrap();
    client.exchange().exchange_info().send().await.unwrap();
    one.assert_async().await;
    all.assert_async().await;
}

#[test]
fn health_and_exchange_charge_their_routes() {
    let spot = Spot::new().unwrap();
    assert_eq!(spot.health().time().cost(), Cost::Weight(1));
    assert_eq!(spot.exchange().exchange_info().cost(), Cost::Weight(20));
    assert_eq!(
        spot.exchange().exchange_info().symbol(&btc()).cost(),
        Cost::Weight(20)
    );
}

#[tokio::test]
async fn an_exchange_info_request_is_charged_twenty() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "",
        &fixture("exchange_info"),
    )
    .await;
    let client = spot(&server);
    client.exchange().exchange_info().send().await.unwrap();
    assert_eq!(client.weight_budget().used(), 20);
}

#[tokio::test]
async fn an_unknown_symbol_is_a_venue_error_with_its_code() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v3/exchangeInfo")
        .match_query(Matcher::Exact("symbol=NOPEUSDT".into()))
        .with_status(400)
        .with_body(fixture("error_invalid_symbol"))
        .create_async()
        .await;
    let err = spot(&server)
        .exchange()
        .exchange_info()
        .symbol(&Symbol::new("NOPEUSDT").unwrap())
        .send()
        .await
        .unwrap_err();
    mock.assert_async().await;
    assert!(
        matches!(&err, SpotError::Venue { status: 400, code: -1121, msg } if msg == "Invalid symbol."),
        "{err:?}"
    );
}

// ── errors ──────────────────────────────────────────────────────

#[tokio::test]
async fn a_code_and_msg_body_is_a_venue_error() {
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        400,
        &[],
        r#"{"code":-1121,"msg":"Invalid symbol."}"#,
    )
    .await;
    let err = spot(&server).health().time().send().await.unwrap_err();
    assert!(
        matches!(
            err,
            SpotError::Venue {
                status: 400,
                code: -1121,
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_451_is_region_blocked_and_a_403_forbidden() {
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        451,
        &[],
        "Service unavailable from a restricted location",
    )
    .await;
    let err = spot(&server).health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::RegionBlocked { .. }), "{err:?}");

    let mut server = Server::new_async().await;
    let _mock = failing(&mut server, 403, &[], "<html>Request blocked</html>").await;
    let err = spot(&server).health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::Forbidden { .. }), "{err:?}");
}

#[tokio::test]
async fn a_418_is_not_retried_and_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let banned = failing(
        &mut server,
        418,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"banned"}"#,
    )
    .await
    .expect(1);
    let client = spot(&server);
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::IpBanned { retry_after: Some(d) } if d == Duration::from_secs(1)),
        "{err:?}"
    );
    banned.assert_async().await;

    let start = Instant::now();
    let _ = client.health().time().send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_418_with_no_retry_after_is_still_a_failure_and_holds() {
    let mut server = Server::new_async().await;
    let banned = failing(&mut server, 418, &[], r#"{"code":-1003,"msg":"banned"}"#)
        .await
        .expect(1);
    let client = spot(&server);
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::IpBanned { retry_after: None }),
        "{err:?}"
    );
    banned.assert_async().await;
    // The two-minute default ban holds the next request.
    let held = tokio::time::timeout(
        Duration::from_millis(500),
        client.health().time().send(),
    )
    .await;
    assert!(held.is_err(), "the next request must wait out the ban");
}

#[tokio::test]
async fn a_429_cools_down_then_succeeds() {
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = route(&mut server, "/api/v3/time", "", &fixture("time")).await;

    let start = Instant::now();
    let time = spot(&server).health().time().send().await.unwrap();
    assert!(time.server_time > 1_790_000_000_000);
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn a_retry_is_charged_again() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_body(fixture("time"))
        .create_async()
        .await;
    let client = spot(&server);
    client.health().time().send().await.unwrap();
    assert_eq!(
        client.weight_budget().used(),
        2,
        "the server charges every attempt"
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn retry_after_outlasts_a_shorter_backoff() {
    let mut server = Server::new_async().await;
    let limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await
    .expect(1);
    let ok = route(&mut server, "/api/v3/time", "", &fixture("time")).await;
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_backoff_ms: 100,
            ..RetryConfig::default()
        },
    );
    let start = Instant::now();
    client.health().time().send().await.unwrap();
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
    limited.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn a_429_out_of_retries_is_rate_limited_and_still_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let _limited = failing(
        &mut server,
        429,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await;
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        },
    );
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(1)),
        "{err:?}"
    );
    let start = Instant::now();
    let _ = client.health().time().send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

#[tokio::test]
async fn a_429_with_no_retry_after_and_no_retry_left_holds_until_the_next_minute() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _limited = failing(
        &mut server,
        429,
        &[],
        r#"{"code":-1003,"msg":"Too many requests."}"#,
    )
    .await;
    let client = spot_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        },
    );
    let err = client.health().time().send().await.unwrap_err();
    assert!(
        matches!(err, SpotError::RateLimited { retry_after: None }),
        "{err:?}"
    );
    let held = tokio::time::timeout(
        Duration::from_millis(500),
        client.health().time().send(),
    )
    .await;
    assert!(
        held.is_err(),
        "the next request must wait for the weight window to reset"
    );
}

#[tokio::test]
async fn every_spot_on_one_budget_is_held_by_one_ban() {
    // Binance bans an IP, not a client: a 418 on one Spot holds every Spot
    // built with the same budget, which is the throttle they all send through.
    let mut server = Server::new_async().await;
    let banned = failing(
        &mut server,
        418,
        &[("retry-after", "1")],
        r#"{"code":-1003,"msg":"banned"}"#,
    )
    .await
    .expect(1);
    let budget = WeightBudget::new();
    let client = |budget: &WeightBudget| {
        Spot::builder()
            .base_url(server.url())
            .weight_budget(budget.clone())
            .build()
            .unwrap()
    };
    let (first, second) = (client(&budget), client(&budget));

    let err = first.health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::IpBanned { .. }), "{err:?}");
    banned.assert_async().await;

    let start = Instant::now();
    let _ = second.health().time().send().await;
    let held = start.elapsed();
    assert!(
        held >= Duration::from_millis(900) && held < Duration::from_secs(5),
        "the other client went after {held:?}, inside the ban"
    );
}

#[tokio::test]
async fn a_425_and_a_5xx_are_not_retried() {
    // The policy retries only a 429: a 425 or a 5xx reaches the caller after
    // one request, with retries left.
    for status in [425, 500, 503] {
        let mut server = Server::new_async().await;
        let mock = failing(
            &mut server,
            status,
            &[],
            r#"{"code":-1001,"msg":"Internal error; unable to process your request."}"#,
        )
        .await
        .expect(1);
        let err = spot(&server).health().time().send().await.unwrap_err();
        assert!(
            matches!(err, SpotError::Venue { status: s, .. } if usize::from(s) == status),
            "{status}: {err:?}"
        );
        mock.assert_async().await;
    }
}

#[tokio::test]
async fn a_high_used_weight_header_holds_the_next_request() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "5400")
        .with_body(fixture("time"))
        .create_async()
        .await;
    let client = spot(&server);
    client.health().time().send().await.unwrap();
    assert_eq!(
        client.weight_budget().used(),
        5400,
        "another process spent the minute"
    );

    let held = tokio::time::timeout(
        Duration::from_millis(500),
        client.health().time().send(),
    )
    .await;
    assert!(
        held.is_err(),
        "the next request must wait for the next minute"
    );
}

#[tokio::test]
async fn a_refused_request_s_weight_header_is_recorded() {
    clear_of_a_minute_boundary().await;
    let mut server = Server::new_async().await;
    let _mock = failing(
        &mut server,
        400,
        &[("x-mbx-used-weight-1m", "5400")],
        r#"{"code":-1121,"msg":"Invalid symbol."}"#,
    )
    .await;
    let client = spot(&server);
    client.health().time().send().await.unwrap_err();
    assert_eq!(client.weight_budget().used(), 5400);
}

#[tokio::test]
async fn a_gzip_body_is_requested_and_decoded() {
    // `{"serverTime":1}` compressed by python3's `gzip.compress(.., mtime=0)`.
    const GZIPPED: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0xab, 0x56, 0x2a, 0x4e, 0x2d,
        0x2a, 0x4b, 0x2d, 0x0a, 0xc9, 0xcc, 0x4d, 0x55, 0xb2, 0x32, 0xac, 0x05, 0x00, 0xe2, 0x1d,
        0x3e, 0x1a, 0x10, 0x00, 0x00, 0x00,
    ];
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/api/v3/time")
        .match_header("accept-encoding", Matcher::Regex("gzip".into()))
        .with_header("content-encoding", "gzip")
        .with_body(GZIPPED)
        .create_async()
        .await;
    let time = spot(&server).health().time().send().await.unwrap();
    mock.assert_async().await;
    assert_eq!(time.server_time, 1);
}
```

- [ ] **Step 2: Run it to see it fail**

Run, with the stanza saved and `mock_spot.rs` not yet created: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot`

Expected: `error: couldn't read \`polyoxide-binance/tests/mock_spot.rs\`: No such file or directory`. With the file saved, the suite exercises code the earlier tasks wrote, so it passes at once; Step 3 shows its assertions can fail.

- [ ] **Step 3: Implement**

There is no new library code. Prove the budget and policy assertions can fail:

- In `polyoxide-binance/src/spot/weight.rs` change `Self::ExchangeInfo => Cost::Weight(20)` to `Self::ExchangeInfo => Cost::Weight(1)` and run `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot -- an_exchange_info_request_is_charged_twenty health_and_exchange_charge_their_routes`. Expected: both fail with `assertion \`left == right\` failed`, `left: 1`, `right: 20` (the `Weight(1)` against `Weight(20)` for the second). Revert it.
- In `polyoxide-binance/src/spot/policy.rs` change `self.budget.until_next_minute()` to `std::time::Duration::ZERO` and run `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::policy`. Expected: `a_429_with_none_left_holds_to_the_next_minute_of_this_budget` fails with `left: Decision { outcome: Fail, hold: Some(0ns) }`. Revert it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot`

Expected: `test result: ok. 24 passed; 0 failed` (the suite takes about 10 seconds: several tests wait out a one-second `Retry-After`). Then `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` passes.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/mock_spot.rs
git commit -m "test(binance): spot's mock suite, its budget and its failure paths (binance spot, Story 4)" -- polyoxide-binance/Cargo.toml polyoxide-binance/tests/mock_spot.rs
```

### Task 4.7: The headers, error-class, failure-tag and ban-log suites

**Files:**
- Create: `polyoxide-binance/tests/headers_spot.rs`
- Create: `polyoxide-binance/tests/error_classes_spot.rs`
- Create: `polyoxide-binance/tests/failure_tags_spot.rs`
- Create: `polyoxide-binance/tests/ban_log_spot.rs`
- Modify: `polyoxide-binance/Cargo.toml` (four `[[test]]` stanzas)
- Test: the four new files

- [ ] **Step 1: Write the failing test**

Edit `polyoxide-binance/Cargo.toml`. Insert, each directly before its `usdm` sibling's stanza:

before `[[test]]` / `name = "ban_log_usdm"`:

```toml
[[test]]
name = "ban_log_spot"
required-features = ["spot"]

```

before `[[test]]` / `name = "error_classes_usdm"`:

```toml
[[test]]
name = "error_classes_spot"
required-features = ["spot"]

```

before `[[test]]` / `name = "failure_tags_usdm"`:

```toml
[[test]]
name = "failure_tags_spot"
required-features = ["spot"]

```

before `[[test]]` / `name = "headers_usdm"`:

```toml
[[test]]
name = "headers_spot"
required-features = ["spot"]

```

Create `polyoxide-binance/tests/headers_spot.rs`:

```rust
//! The full header set one request goes out with (AD-18).
//!
//! The expected set is a literal, never read from the client under test. CI
//! runs this file in this crate alone with `--no-default-features --features
//! spot`, again in the workspace build, and again beside the umbrella with
//! `--features polyoxide/full`, so a change to what a client sends fails here
//! in one of the three builds, whether a code default (0.37.0's gzip
//! regression was core's builder turning gzip off) or a reqwest feature
//! another member, or another venue's module, switches on.

use polyoxide_binance::spot::Spot;
use polyoxide_test_support::query::headers_sent;

/// One unauthenticated `GET /api/v3/time`, and the headers it carried.
#[tokio::test]
async fn a_time_request_sends_exactly_the_default_headers() {
    let mut base = String::new();
    let headers = headers_sent("/api/v3/time", |url| {
        base = url.clone();
        async move {
            let spot = Spot::builder().base_url(url).build().unwrap();
            let _ = spot.health().time().send().await;
        }
    })
    .await;

    let host = base.trim_start_matches("http://");
    assert_eq!(
        headers,
        [
            ("accept", "*/*"),
            ("accept-encoding", "gzip"),
            ("host", host)
        ]
        .map(|(name, value)| (name.to_owned(), value.to_owned())),
        "`health().time` sent a header set other than reqwest's defaults with gzip"
    );
}
```

Create `polyoxide-binance/tests/error_classes_spot.rs`:

```rust
//! The class each status reaches a caller with, read through `Classify` alone.
//!
//! One request per status, each on a fresh client with no retries, so a `429`
//! or `418` hold cannot reach the next case.

use mockito::{Matcher, Server};
use polyoxide_binance::spot::{Spot, SpotError};
use polyoxide_core::RetryConfig;
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};

/// `GET /api/v3/time`, answered with `status` and `body`, on a client that
/// does not retry.
async fn fail(status: usize, body: &str) -> SpotError {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body)
        .create_async()
        .await;
    let spot = Spot::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap();
    spot.health().time().send().await.unwrap_err()
}

#[tokio::test]
async fn every_status_reaches_the_caller_with_its_class() {
    let refusal = Class::VenueRefusal { code: None };
    let unavailable = Class::Unavailable { code: None };
    // (status, class, is_fault), each with a body not in Binance's shape. The
    // firewall's 403 is restricted (D14), and only a 451 is not a fault.
    let rows = [
        (400, refusal.clone(), true),
        (401, Class::Unauthorized, true),
        (403, Class::Restricted, true),
        (404, refusal, true),
        (408, unavailable.clone(), true),
        (418, Class::Restricted, true),
        (425, unavailable.clone(), true),
        (429, Class::RateLimited { retry_after: None }, true),
        (451, Class::Restricted, false),
        (500, unavailable.clone(), true),
        (503, unavailable, true),
    ];
    for (status, class, fault) in rows {
        let err = fail(status, "<html>x</html>").await;
        assert_eq!(err.class(), class, "{status}: {err:?}");
        assert_eq!(err.is_fault(), fault, "{status}: {err:?}");
    }
}

#[tokio::test]
async fn binance_s_body_carries_its_code_into_the_class() {
    let err = fail(400, r#"{"code":-1121,"msg":"Invalid symbol."}"#).await;
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(std::sync::Arc::from("-1121"))
        },
        "{err:?}"
    );
    assert!(err.is_fault(), "{err:?}");
}

#[tokio::test]
async fn the_captured_invalid_symbol_body_carries_its_code_into_the_class() {
    // Captured from production, answered 400 (`tests/fixtures/spot/PROVENANCE.md`).
    let err = fail(400, &fixtures!("spot/rest").text("error_invalid_symbol")).await;
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(std::sync::Arc::from("-1121"))
        },
        "{err:?}"
    );
    assert_eq!(err.code(), Some(-1121));
}

#[tokio::test]
async fn a_2xx_body_that_does_not_decode_is_decode() {
    let err = fail(200, "not json").await;
    assert_eq!(err.class(), Class::Decode, "{err:?}");
    assert!(err.is_fault(), "{err:?}");
}
```

Create `polyoxide-binance/tests/failure_tags_spot.rs`:

```rust
//! The Rust twins of the nightly classifier's tag table, for the errors the
//! spot live suite fails with.
//!
//! Each builds an error, checks it renders the text the classifier reads, and
//! asserts the tag it fails a test with, so the tag a live test prints is the
//! one the table expects.

use std::time::Duration;

use polyoxide_binance::spot::SpotError;
use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_test_support::{tag_for, Tag};
use polyoxide_venue::Classify;

/// The tag `err` fails a test with, through `or_fail` or `fail`.
fn tag(err: &impl Classify) -> Tag {
    tag_for(&err.class(), err.is_fault())
}

fn venue(status: u16, code: i64, msg: &str) -> SpotError {
    SpotError::Venue {
        status,
        code,
        msg: msg.to_owned(),
    }
}

#[test]
fn rate_limited() {
    let err = SpotError::RateLimited {
        retry_after: Some(Duration::from_secs(1)),
    };
    assert_eq!(format!("{err:?}"), "RateLimited { retry_after: Some(1s) }");
    assert_eq!(
        err.to_string(),
        "binance rate limit (429), retry after Some(1s)"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn venue_5xx() {
    let err = venue(503, -1001, "Internal error");
    assert_eq!(
        format!("{err:?}"),
        r#"Venue { status: 503, code: -1001, msg: "Internal error" }"#
    );
    assert_eq!(
        err.to_string(),
        "binance answered 503: -1001 Internal error"
    );
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn venue_408() {
    let err = venue(408, -1007, "Timeout");
    assert_eq!(err.to_string(), "binance answered 408: -1007 Timeout");
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn venue_4xx() {
    let err = venue(400, -1121, "Invalid symbol.");
    assert_eq!(
        format!("{err:?}"),
        r#"Venue { status: 400, code: -1121, msg: "Invalid symbol." }"#
    );
    assert_eq!(
        err.to_string(),
        "binance answered 400: -1121 Invalid symbol."
    );
    assert_eq!(tag(&err), Tag::Real);
}

/// A ban is the client's own doing, so it files (amendment A2-1).
#[test]
fn ip_banned() {
    let err = SpotError::IpBanned { retry_after: None };
    assert_eq!(format!("{err:?}"), "IpBanned { retry_after: None }");
    assert_eq!(
        err.to_string(),
        "binance has banned this IP (418), retry after None"
    );
    assert_eq!(tag(&err), Tag::Real);
}

/// The firewall's refusal is about how the client behaved, so it files
/// (amendment A2-1).
#[test]
fn forbidden() {
    let err = SpotError::Forbidden {
        msg: "<html>".into(),
    };
    assert_eq!(
        err.to_string(),
        "binance's firewall refused the request (403): <html>"
    );
    assert_eq!(tag(&err), Tag::Real);
}

#[test]
fn region_blocked() {
    let msg = "Service unavailable from a restricted location";
    let err = SpotError::RegionBlocked { msg: msg.into() };
    assert_eq!(
        format!("{err:?}"),
        format!("RegionBlocked {{ msg: {msg:?} }}")
    );
    assert_eq!(
        err.to_string(),
        format!("binance does not serve this location (451): {msg}")
    );
    assert_eq!(tag(&err), Tag::Environmental);
}

/// A live suite's raw fetch fails a refused request with core's reading of
/// its status.
fn raw(status: u16, body: &str) -> ApiError {
    ErrorResponse::new(
        polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),
        Default::default(),
        body,
    )
    .into()
}

#[test]
fn raw_status_503() {
    let err = raw(503, "Service Unavailable");
    assert_eq!(err.to_string(), "API error: 503 - Service Unavailable");
    assert_eq!(tag(&err), Tag::Transient);
}

#[test]
fn raw_status_451() {
    assert_eq!(
        tag(&raw(451, "Unavailable For Legal Reasons")),
        Tag::Environmental
    );
}

#[test]
fn raw_status_403() {
    assert_eq!(tag(&raw(403, "Forbidden")), Tag::Real);
}
```

Create `polyoxide-binance/tests/ban_log_spot.rs`:

```rust
//! A `418`'s body names when the ban ends, and the error has no field for it,
//! so the client logs it at WARN under `polyoxide_binance::spot::error`. A
//! binary of its own, because it installs the process's global subscriber,
//! which `mock_spot`'s other `418` tests would otherwise race to reach first.

use std::fmt::Debug;
use std::sync::Mutex;

use mockito::{Matcher, Server};
use polyoxide_binance::spot::{Spot, SpotError};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, SubscriberExt};

/// Every `WARN` logged while this binary runs, as `(target, message)`.
static WARNINGS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Installs, once and before any request is sent, the global subscriber that
/// fills [`WARNINGS`]. A subscriber scoped to one test would race: a callsite
/// first reached from a test with none caches `Interest::never` for every
/// thread, and the scoped test then sees nothing.
fn capture_warnings() {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Warnings))
            .expect("this binary installs no other subscriber");
    });
}

struct Warnings;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Warnings {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::WARN {
            return;
        }
        let mut message = Message(String::new());
        event.record(&mut message);
        WARNINGS
            .lock()
            .unwrap()
            .push((event.metadata().target().to_owned(), message.0));
    }
}

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

#[tokio::test]
async fn a_418_logs_its_body_at_warn_under_the_spot_error_target() {
    capture_warnings();
    let mut server = Server::new_async().await;
    let banned = server
        .mock("GET", "/api/v3/time")
        .match_query(Matcher::Any)
        .with_status(418)
        .with_body(
            r#"{"code":-1003,"msg":"Way too many requests; IP banned until 1700000000000."}"#,
        )
        .expect(1)
        .create_async()
        .await;
    let client = Spot::builder().base_url(server.url()).build().unwrap();

    let err = client.health().time().send().await.unwrap_err();
    assert!(matches!(err, SpotError::IpBanned { .. }), "{err:?}");
    banned.assert_async().await;

    // The send loop warns of the hold under `polyoxide_core`, naming the
    // path; the body is Binance's own warning, from the module's decode,
    // which sees the response but not the request.
    let warnings = WARNINGS.lock().unwrap().clone();
    let seen: Vec<_> = warnings
        .iter()
        .filter(|(target, _)| target.starts_with("polyoxide_binance"))
        .cloned()
        .collect();
    assert_eq!(seen.len(), 1, "one 418, one warning: {seen:?}");
    let (target, message) = &seen[0];
    assert_eq!(target, "polyoxide_binance::spot::error");
    assert!(
        message.contains("418") && message.contains("IP banned until 1700000000000"),
        "{message:?} does not name the ban's end"
    );
    assert!(
        warnings
            .iter()
            .any(|(target, message)| target.starts_with("polyoxide_core")
                && message.starts_with("Status 418")
                && message.contains(" on /api/v3/time,")),
        "no warning names the path: {warnings:?}"
    );
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test ban_log_spot --test error_classes_spot --test failure_tags_spot --test headers_spot`

Expected before the files are saved: `error: no test target named \`ban_log_spot\` in \`polyoxide-binance\` package` (and the same for the other three). With them saved and the stanzas in place they pass at once, since the code they exercise is written; Step 3 shows they can fail.

- [ ] **Step 3: Implement**

No library code. Prove the ban log and the header pin can fail:

- In `polyoxide-binance/src/spot/error.rs` change the target-bearing line `tracing::warn!("418: IP banned: {}", truncate_for_log(&body));` to `tracing::warn!(target: "polyoxide_binance::usdm::error", "418: IP banned: {}", truncate_for_log(&body));` and run `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test ban_log_spot`. Expected: it fails (`assertion \`left == right\` failed` on the target, `left: "polyoxide_binance::usdm::error"`). Revert it.
- In `polyoxide-binance/src/spot/error.rs` change `Self::Venue { status, code, .. } => venue_class(*status, *code)` to `Self::Venue { status, .. } => venue_class(*status, 0)` and run `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test error_classes_spot`. Expected: `binance_s_body_carries_its_code_into_the_class` fails with `left: VenueRefusal { code: Some("0") }`. Revert it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test ban_log_spot --test error_classes_spot --test failure_tags_spot --test headers_spot`

Expected: `ban_log_spot` 1 passed, `error_classes_spot` 4 passed, `failure_tags_spot` 10 passed, `headers_spot` 1 passed. Then `nix develop --command cargo test -p polyoxide-binance --all-features` passes (every `usdm` suite unchanged) and `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` passes.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/headers_spot.rs polyoxide-binance/tests/error_classes_spot.rs polyoxide-binance/tests/failure_tags_spot.rs polyoxide-binance/tests/ban_log_spot.rs
git commit -m "test(binance): spot's headers, error classes, failure tags and 418 log (binance spot, Story 4)" -- polyoxide-binance/Cargo.toml polyoxide-binance/tests/headers_spot.rs polyoxide-binance/tests/error_classes_spot.rs polyoxide-binance/tests/failure_tags_spot.rs polyoxide-binance/tests/ban_log_spot.rs
```

### Task 4.8: The module README

**Files:**
- Create: `polyoxide-binance/src/spot/README.md`
- Modify: `polyoxide-binance/src/lib.rs`
- Test: the README's doctests, under `spot` alone

- [ ] **Step 1: Write the failing test**

Edit `polyoxide-binance/src/lib.rs`. Replace

```rust
struct UsdmReadmeDoctests;
```

with

```rust
struct UsdmReadmeDoctests;

#[cfg(all(doctest, feature = "spot"))]
#[doc = include_str!("spot/README.md")]
struct SpotReadmeDoctests;
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --doc`

Expected: the build fails with `error: couldn't read \`polyoxide-binance/src/spot/README.md\`: No such file or directory`.

- [ ] **Step 3: Implement**

Create `polyoxide-binance/src/spot/README.md` (the outer fence here is four backticks; the file holds the inner three-backtick fences):

````markdown
# polyoxide-binance: `spot`

Rust client for Binance spot public market data (`api.binance.com`), the `spot` module of
`polyoxide-binance`: liveness, the exchange listing with its symbols and filters, and the market
routes that follow in the `market()` namespace. No credentials are needed.

Every request is charged against a `WeightBudget`, because Binance limits each IP by request
*weight*, which varies with the route and its parameters, rather than by request count. The
budget is 5400 of the published 6000 per minute, follows the server's own count from the
`X-MBX-USED-WEIGHT-1M` header, and a `429` or `418` holds every request until the server's wait
is over. The venue's `RAW_REQUESTS` limit is not modelled: every request weighs at least 1, so
the budget keeps a client under it.

More information about this module can be found in the [crate documentation](https://docs.rs/polyoxide-binance/latest/polyoxide_binance/spot/).

## Installation

```toml
[dependencies]
polyoxide-binance = "0.44"
```

`spot` is on by default, and builds without `usdm`.

## Usage

```no_run
use polyoxide_binance::spot::{types::Symbol, Spot};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let spot = Spot::new()?;

let latency = spot.health().ping().await?;
println!("ping {latency:?}");

let btc = Symbol::new("BTCUSDT")?;
let info = spot.exchange().exchange_info().symbol(&btc).send().await?;
for symbol in &info.symbols {
    println!("{} is {}", symbol.symbol, symbol.status);
}
# Ok(())
# }
```

`exchangeInfo` weighs 20 with a symbol or without. Clients in one process should share one
budget, since Binance counts weight per IP:

```no_run
use polyoxide_binance::spot::{Spot, WeightBudget};

# fn example() -> Result<(), polyoxide_binance::spot::SpotError> {
let budget = WeightBudget::new();
let a = Spot::builder().weight_budget(budget.clone()).build()?;
let b = Spot::builder().weight_budget(budget).build()?;
# Ok(())
# }
```

Each client may have `max_concurrent` requests in flight (default 4), and the budget's reserve
absorbs in-flight weight only while it stays under 600: at the default, one client has at most
320 in flight on a route at its default (4 × the heaviest, `tickers_24h`'s 80). Four `depth`
calls with a `limit` above 1000 (250 each) pass it.
````

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --doc && nix develop --command cargo test -p polyoxide-binance --all-features --doc`

Expected: both pass; the README's three `no_run` fences compile under `spot` alone and under all features. Then `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_module_readmes.py -q'` passes.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/README.md
git commit -m "docs(binance): the spot module README, compiled as a doctest (binance spot, Story 4)" -- polyoxide-binance/src/spot/README.md polyoxide-binance/src/lib.rs
```

### Task 4.9: The dependency allowlist

**Files:**
- Modify: `.github/scripts/tests/test_dep_allowlists.py`
- Modify: `.claude/rules/ci-gates.md:79-81` (the `polyoxide-binance` deny-list sentence)
- Create: `ci/dep-allowlists/polyoxide-binance@spot.txt` (by `--write`)
- Test: `.github/scripts/tests/test_dep_allowlists.py`

- [ ] **Step 1: Write the failing test**

Edit `.github/scripts/tests/test_dep_allowlists.py`. Insert directly before the line `if __name__ == "__main__":`:

```python
def test_binances_spot_allowlist_is_the_rest_tree_and_builds_no_socket_stack() -> None:
    # `spot` is an HTTP module like `usdm`: it builds core and reqwest, and none of
    # the socket stack, which only `usdm-ws` admits. The file must list core, or a
    # rule that admitted nothing could not be told from one that holds.
    path = ALLOWLISTS / "polyoxide-binance@spot.txt"
    assert path.exists(), "run `uv run python -m tests.test_dep_allowlists --write`"
    listed = read_allowlist(path)
    assert "polyoxide-core" in listed, listed
    assert "reqwest" in listed, listed
    assert [n for n in SOCKET_STACK if n in listed] == [], listed
    assert denied(BINANCE, listed, ("spot",)) == []


def test_binances_spot_feature_stands_alone() -> None:
    # `cargo hack --each-feature` builds `spot` without `usdm`: neither turns the
    # other, nor the streams, on. The real table, not a stand-in.
    table = feature_table(BINANCE)
    assert enabled(("spot",), table) == {"spot"}
    assert "spot" not in enabled(("usdm",), table)
    assert "spot" not in enabled(("usdm-ws",), table)
    assert "usdm-ws" not in enabled(("spot",), table)
    assert "spot" in table["default"] and "usdm" in table["default"], table["default"]


```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q -k "spot"'`

Expected: `test_binances_spot_allowlist_is_the_rest_tree_and_builds_no_socket_stack` fails with `AssertionError: run \`uv run python -m tests.test_dep_allowlists --write\`` (`test_binances_spot_feature_stands_alone` passes: Task 4.1 wrote the features).

- [ ] **Step 3: Implement**

`--write` rewrites existing files only, so create the file first, then regenerate every allowlist from the tree cargo reports:

```bash
: > ci/dep-allowlists/polyoxide-binance@spot.txt
nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
git status --short ci/dep-allowlists
```

Expected: `git status` lists only `ci/dep-allowlists/polyoxide-binance@spot.txt` as new. No other allowlist changes, because no other crate takes `polyoxide-binance` with its default features. If any other file changed, stop and report: a crate depends on `polyoxide-binance` with defaults, and the new default `spot` widened its tree.

The file's header names the command it was generated with, `cargo tree -p polyoxide-binance -e normal --no-default-features --target x86_64-unknown-linux-gnu --features spot`; its crate list equals `polyoxide-binance@usdm.txt`'s, since `spot` lists the same dependencies as `usdm`. Say why in the commit: the file is new because the `spot` feature is new, and the tree is the REST client's with no socket stack.

Edit `.claude/rules/ci-gates.md`, in this commit (AD-21: the sentence states the rule the new test holds). Replace

```markdown
  - `polyoxide-binance`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with
    `usdm-ws` (files `@usdm` and `@usdm-ws`; `@usdm-ws` must hold `polyoxide-ws`). `rustls` is not
    on the rule, since core's HTTP client builds it for TLS.
```

with

```markdown
  - `polyoxide-binance`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with
    `usdm-ws` (files `@usdm`, `@usdm-ws` and `@spot`; `@usdm-ws` must hold `polyoxide-ws`, and
    `@spot` `polyoxide-core` and `reqwest`, so no rule can pass vacuously). `rustls` is not on the
    rule, since core's HTTP client builds it for TLS.
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_module_readmes.py -q'`

Expected: all pass, `polyoxide-binance@spot.txt` included in `test_each_allowlist_is_the_tree_cargo_builds` and `test_no_allowlist_admits_a_denied_crate`. Then the story's whole check:

- `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot`
- `nix develop --command cargo test -p polyoxide-binance --all-features`
- `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps`
- the profile's gates in order: `cargo fmt --all -- --check`, `cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings`, `cargo test -p polyoxide-binance --all-features`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features`, `python3 scripts/gen_registry.py --check`, `python3 scripts/venue_gate.py`, `python3 scripts/live_unwraps.py` and `python3 scripts/test_body_diff.py --baseline v0.40.0`, each behind `nix develop --command`. The packages that depend on `polyoxide-binance` (`polyoxide`, `polyoxide-cli`) take `-p polyoxide -p polyoxide-cli` in the clippy and test gates too; `Cargo.lock` is unchanged by this story, and `git status` must show it unmodified.

- [ ] **Step 5: Commit**

```bash
git add -N ci/dep-allowlists/polyoxide-binance@spot.txt
git commit -m "build(binance): pin the spot feature's dependency tree; the file is new because the feature is, and the tree is the REST client's with no socket stack (binance spot, Story 4)" -- .github/scripts/tests/test_dep_allowlists.py ci/dep-allowlists/polyoxide-binance@spot.txt .claude/rules/ci-gates.md
```

### Task 4.10: A spot-only build is warning-free

**Files:**
- Modify: `polyoxide-binance/src/shared/weight.rs` (attributes above the items the build reports; none if it reports none)

`shared` was written for `usdm` alone, and Task 4.1 widened its `cfg` to `spot`. A build without `usdm` reports each crate-private `shared` item that only `usdm` reaches as never used, and `cargo hack check --each-feature` builds exactly that. Which items those are is for the build to say: `Engine::charge` reaches both acquire paths, so it may report none. Story 2's `shared/weight.rs` is not in the tree when this draft is written, so no edit below is anchored on a quoted block; each is anchored on the name the build prints.

- [ ] **Step 1: Write the check**

The check is a build of the crate's library with `spot` alone, where a warning is an error. No file is written for it; it is this command:

```bash
nix develop --command cargo rustc -p polyoxide-binance --no-default-features --features spot --lib -- -D warnings
```

- [ ] **Step 2: Run it**

Run the command above. Do not expect it to fail.

- If it finishes with `Finished` and no warning, no `shared` item needs a mark: change nothing in `shared/weight.rs`, skip Step 3 and Step 5, and record in the task's report that the spot-only build is clean. The task commits nothing.
- If it fails with `error: ... is never used` (or `never read`, or `never constructed`) and `= note: \`-D dead-code\` implied by \`-D warnings\``, the items it names are the list to mark. Each has a span in `src/shared/weight.rs`.

- [ ] **Step 3: Implement**

Only when Step 2 named items. In `polyoxide-binance/src/shared/weight.rs`, add this line directly above the declaration of each item Step 2 named (below its doc comment, indented as the item is), and above no other item:

```rust
#[cfg_attr(not(feature = "usdm"), allow(dead_code))]
```

Change nothing else: no signature, no body, no `RESERVED_FRACTION` or any line that uses it, no other `shared` file. If the output names an item in another `shared` file, stop and report it to the supervisor: the names contract lists only `shared/weight.rs` for this story.

Run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo rustc -p polyoxide-binance --no-default-features --features spot --lib -- -D warnings`

Expected: it finishes with `Finished` and no warning (Step 2's output already, when it named nothing). Then `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` (the attribute is inert with `usdm` on, so nothing new), `nix develop --command cargo test -p polyoxide-binance --all-features` and `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps`: all pass, the last with no warning in its `spot` leg.

- [ ] **Step 5: Commit**

Only when Step 3 changed the file; otherwise there is nothing to commit.

```bash
git commit -m "fix(binance): a spot-only build marks the shared items only usdm uses (binance spot, Story 4)" -- polyoxide-binance/src/shared/weight.rs
```

## Story 5: Spot market routes

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/src/spot/api/market.rs`, `polyoxide-binance/src/spot/api/mod.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/spot/types.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/wire_agreement_spot.rs`, `polyoxide-binance/tests/query_setters_spot.rs`, `polyoxide-binance/examples/weight_probe_spot.rs`, `.claude/rules/clients.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`
- **Interfaces:** exposes "Story 5 → Stories 6 and 7"; uses Story 1's fixtures and Story 4's client, `Route`, `Cost`, `SpotError` and the `mock_spot.rs` helper
- **Verify:** `cargo test -p polyoxide-binance --no-default-features --features spot && cargo test -p polyoxide-binance --all-features --example weight_probe_spot && cargo hack check -p polyoxide-binance --each-feature --no-dev-deps && bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`
- **Review:** deep (the weight table every request is charged by)

This story adds the spot market routes to the client Story 4 built. `spot::types` gains `Interval`, `Depth`, `Level`, `Trade`, `AggTrade`, `Kline`, `AvgPrice`, `Ticker24h`, `PriceTicker` and `BookTicker`; `spot/api/market.rs` gains `MarketApi` with its eleven builders, of which `GetDepth::limit` is hand-written because it re-prices the route by its band (5 up to 100 levels, 25 up to 500, 50 up to 1000, 250 above); and `Spot` gains the `market()` namespace. The suites hold each route's path, query and `Cost` on a mock server (appended to `mock_spot.rs`), every setter's exact query pair (`query_setters_spot.rs`), and every captured fixture decoding with no unmodelled key and serializing back (`wire_agreement_spot.rs`). A new example, `weight_probe_spot`, probes the spot routes against the live host; `examples/weight_probe.rs` and its `[[example]]` stanza are not touched, because the example is `test = true` and the test-bodies gate counts every item in it. `clients.md` gains the setter's row and `MUTANTS.md` gains spot's depth re-pricing row, proved, citing `file:line` as every other row does, with one `SNIPPETS` entry per citation in `test_mutants_ledger.py`. The one landmine marker in this story's code is `.gzip(`: `weight_probe_spot`'s bare client copies `weight_probe.rs:126`'s `.gzip(true)`, copied and never changed, and no other client gains one (the `Spot` client is built through `ClientConfig` and sets nothing of the kind); the supervisor should dispatch Task 5.3's developer with `effort: xhigh`. Nothing else here holds a marker. The fixtures do not exist when this plan is written (Story 1 captures them), so the types follow Binance's documented spot shapes and the names contract, and `wire_agreement_spot` against the captures is what settles any difference: if it reports an unmodelled or invented key, fix the type to the capture, never the capture.

### Task 5.1: The market types and their wire agreement

**Files:**
- Modify: `polyoxide-binance/src/spot/types.rs` (insert before any `#[cfg(test)] mod tests`, else at the end)
- Modify: `polyoxide-binance/Cargo.toml` (one `[[test]]` stanza)
- Create: `polyoxide-binance/tests/wire_agreement_spot.rs`
- Test: `polyoxide-binance/tests/wire_agreement_spot.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide-binance/Cargo.toml`, insert this stanza directly after the `[[test]]` stanza named `wire_agreement_usdm`:

```toml
[[test]]
name = "wire_agreement_spot"
required-features = ["spot"]
```

Create `polyoxide-binance/tests/wire_agreement_spot.rs`:

```rust
//! Agreement between the spot types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/spot/PROVENANCE.md`.
//!
//! Each fixture is decoded into its type and encoded back, and:
//!
//! 1. **Nothing unmodelled.** Every key path the server sent is emitted by the
//!    type.
//! 2. **Nothing invented.** Every key path the type emits was sent by the
//!    server. An `Option` encodes as `null`, so a field modelled but absent from
//!    the wire shows up here instead of hiding.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a
//!    price with more digits than an `f64` keeps survives exactly or fails.
//!
//! A kline's twelfth element, which Binance documents as "ignore", is the one
//! thing dropped on purpose; positional arrays are compared by index only as
//! far as the shorter side goes.

use polyoxide_test_support::agreement as common;

use polyoxide_binance::spot::types::{
    AggTrade, AvgPrice, BookTicker, Depth, ExchangeInfo, Interval, Kline, PriceTicker,
    ServerTime, SymbolStatus, Ticker24h, Trade,
};
use polyoxide_test_support::fixtures;
use serde::{de::DeserializeOwned, Serialize};

fn check<T: DeserializeOwned + Serialize>(fixture: &str) {
    let text = fixtures!("spot/rest").text(fixture);
    let diff = common::compare::<T>(fixture, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{fixture}: the server sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{fixture}: the type emits {:?}, which the server did not send",
        diff.invented
    );
}

fn read<T: DeserializeOwned>(fixture: &str) -> T {
    serde_json::from_str(&fixtures!("spot/rest").text(fixture)).unwrap()
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    check::<ServerTime>("time");
    check::<ExchangeInfo>("exchange_info");
    check::<Depth>("depth");
    check::<Vec<Trade>>("trades");
    check::<Vec<AggTrade>>("agg_trades");
    check::<Vec<Kline>>("klines");
    check::<Vec<Kline>>("klines_1s");
    check::<AvgPrice>("avg_price");
    check::<Ticker24h>("ticker_24hr");
    check::<Vec<Ticker24h>>("tickers_24hr");
    check::<PriceTicker>("price");
    check::<Vec<PriceTicker>>("prices");
    check::<BookTicker>("book_ticker");
    check::<Vec<BookTicker>>("book_tickers");
}

#[test]
fn the_fixtures_hold_what_the_types_are_for() {
    // The book is best first on both sides: bids fall, asks rise.
    let book: Depth = read("depth");
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert!(book.bids.windows(2).all(|w| w[0].price > w[1].price));
    assert!(book.asks.windows(2).all(|w| w[0].price < w[1].price));
    assert!(book.bids[0].price < book.asks[0].price, "a crossed book");

    // Trade ids rise, oldest first.
    let trades: Vec<Trade> = read("trades");
    assert!(trades.len() > 1);
    assert!(trades.windows(2).all(|w| w[0].id < w[1].id));
    let aggregates: Vec<AggTrade> = read("agg_trades");
    assert!(aggregates.len() > 1);
    assert!(aggregates.windows(2).all(|w| w[0].id < w[1].id));
    assert!(aggregates.iter().all(|t| t.first_trade_id <= t.last_trade_id));

    // The candles are one interval apart: 60 s for `1m`, one second for `1s`,
    // which this host serves (the USDⓈ-M host refuses it).
    let minutes: Vec<Kline> = read("klines");
    assert!(minutes.len() > 1);
    assert_eq!(minutes[1].open_time - minutes[0].open_time, 60_000);
    let seconds: Vec<Kline> = read("klines_1s");
    assert!(seconds.len() > 1);
    assert_eq!(seconds[1].open_time - seconds[0].open_time, 1_000);
    assert!(minutes.iter().all(|k| k.close_time >= k.open_time));

    // The single-symbol and all-symbol forms agree on the symbols they name.
    let one: Ticker24h = read("ticker_24hr");
    assert_eq!(one.symbol, "BTCUSDT");
    let all: Vec<Ticker24h> = read("tickers_24hr");
    for symbol in ["BTCUSDT", "ETHUSDT"] {
        assert!(all.iter().any(|t| t.symbol == symbol), "{symbol}");
    }
    let prices: Vec<PriceTicker> = read("prices");
    assert!(prices.iter().any(|t| t.symbol == "BTCUSDT"));
    let books: Vec<BookTicker> = read("book_tickers");
    assert!(books.iter().any(|t| t.symbol == "BTCUSDT"));
    let book_one: BookTicker = read("book_ticker");
    assert!(book_one.bid_price <= book_one.ask_price);
}

#[test]
fn the_exchange_info_fixture_holds_a_symbol_that_is_not_trading() {
    // An unknown status would land in `Other` and decode fine; failing here is
    // how a status Binance adds gets modelled instead of passing silently.
    let info: ExchangeInfo = read("exchange_info");
    assert!(
        info.symbols
            .iter()
            .any(|s| s.status != SymbolStatus::Trading),
        "a symbol that is not TRADING"
    );
    let unnamed: Vec<_> = info
        .symbols
        .iter()
        .filter(|s| matches!(s.status, SymbolStatus::Other(_)))
        .map(|s| (&s.symbol, &s.status))
        .collect();
    assert!(
        unnamed.is_empty(),
        "statuses SymbolStatus does not name, model them: {unnamed:?}"
    );
}

#[test]
fn every_interval_the_host_takes_is_named() {
    let wire: Vec<&str> = Interval::ALL.iter().map(|i| i.as_str()).collect();
    assert_eq!(
        wire,
        [
            "1s", "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d",
            "3d", "1w", "1M"
        ]
    );
}
```

- [ ] **Step 2: Run it to see it fail**

```sh
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test wire_agreement_spot
```

It fails to compile: `error[E0432]: unresolved imports `polyoxide_binance::spot::types::AggTrade``, and the same for `AvgPrice`, `BookTicker`, `Depth`, `Interval`, `Kline`, `PriceTicker`, `Ticker24h` and `Trade`.

- [ ] **Step 3: Implement**

In `polyoxide-binance/src/spot/types.rs`, insert the block below directly before the file's `#[cfg(test)] mod tests` if it has one (clippy's `items_after_test_module` forbids items after it), else at the end of the file. It sits in a private module of its own so its imports cannot collide with the exchange part's; the items are re-exported at `spot::types`.

```rust
pub use rows::{
    AggTrade, AvgPrice, BookTicker, Depth, Interval, Kline, Level, PriceTicker, Ticker24h, Trade,
};

/// The market routes' rows. Prices and quantities are `Decimal`, decoded from
/// the decimal strings Binance sends, so no value passes through an `f64`;
/// timestamps are Unix milliseconds. Where the wire uses one-letter keys
/// (`aggTrades`), fields take spelled-out names through serde renames.
mod rows {
    use std::fmt;

    use polyoxide_venue::positional::{drain, element, DecimalStr};
    use rust_decimal::Decimal;
    use serde::{
        de::{SeqAccess, Visitor},
        ser::SerializeSeq,
        Deserialize, Deserializer, Serialize, Serializer,
    };

    polyoxide_venue::wire_enum! {
        /// Kline width, for `klines`. This host takes `1s`, which the USDⓈ-M
        /// host refuses.
        pub enum Interval {
            /// One second.
            Second1 => "1s",
            /// One minute.
            Minute1 => "1m",
            /// Three minutes.
            Minute3 => "3m",
            /// Five minutes.
            Minute5 => "5m",
            /// Fifteen minutes.
            Minute15 => "15m",
            /// Thirty minutes.
            Minute30 => "30m",
            /// One hour.
            Hour1 => "1h",
            /// Two hours.
            Hour2 => "2h",
            /// Four hours.
            Hour4 => "4h",
            /// Six hours.
            Hour6 => "6h",
            /// Eight hours.
            Hour8 => "8h",
            /// Twelve hours.
            Hour12 => "12h",
            /// One day.
            Day1 => "1d",
            /// Three days.
            Day3 => "3d",
            /// One week.
            Week1 => "1w",
            /// One calendar month.
            Month1 => "1M",
        }
    }

    /// `GET /api/v3/depth`: an order book snapshot. It carries no time.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    pub struct Depth {
        /// Book update id of the snapshot.
        pub last_update_id: u64,
        /// Bids, best first.
        pub bids: Vec<Level>,
        /// Asks, best first.
        pub asks: Vec<Level>,
    }

    /// A price level, a `[price, quantity]` pair on the wire.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[non_exhaustive]
    pub struct Level {
        /// Price.
        pub price: Decimal,
        /// Quantity at the price.
        pub quantity: Decimal,
    }

    impl Level {
        /// A level from a price and a quantity.
        pub fn new(price: Decimal, quantity: Decimal) -> Self {
            Self { price, quantity }
        }
    }

    impl<'de> Deserialize<'de> for Level {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct LevelVisitor;

            impl<'de> Visitor<'de> for LevelVisitor {
                type Value = Level;

                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a [price, quantity] array")
                }

                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Level, A::Error> {
                    const WHAT: &str = "a [price, quantity] array";
                    let level = Level {
                        price: element::<DecimalStr, _>(&mut seq, 0, WHAT)?.0,
                        quantity: element::<DecimalStr, _>(&mut seq, 1, WHAT)?.0,
                    };
                    drain(&mut seq)?;
                    Ok(level)
                }
            }

            deserializer.deserialize_seq(LevelVisitor)
        }
    }

    impl Serialize for Level {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut seq = serializer.serialize_seq(Some(2))?;
            seq.serialize_element(&self.price.to_string())?;
            seq.serialize_element(&self.quantity.to_string())?;
            seq.end()
        }
    }

    /// One row of `GET /api/v3/trades`, oldest first.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    pub struct Trade {
        /// Trade id.
        pub id: u64,
        /// Price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price: Decimal,
        /// Base-asset quantity.
        #[serde(with = "rust_decimal::serde::str")]
        pub qty: Decimal,
        /// Quote-asset quantity.
        #[serde(with = "rust_decimal::serde::str")]
        pub quote_qty: Decimal,
        /// Trade time.
        pub time: u64,
        /// Whether the buyer was the maker, so the taker sold.
        pub is_buyer_maker: bool,
        /// Whether the trade was the best price match.
        pub is_best_match: bool,
    }

    /// One row of `GET /api/v3/aggTrades`: trades at one price, taken by one
    /// order, merged.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct AggTrade {
        /// Aggregate trade id.
        #[serde(rename = "a")]
        pub id: u64,
        /// Price.
        #[serde(rename = "p", with = "rust_decimal::serde::str")]
        pub price: Decimal,
        /// Quantity.
        #[serde(rename = "q", with = "rust_decimal::serde::str")]
        pub quantity: Decimal,
        /// First trade id merged.
        #[serde(rename = "f")]
        pub first_trade_id: u64,
        /// Last trade id merged.
        #[serde(rename = "l")]
        pub last_trade_id: u64,
        /// Trade time.
        #[serde(rename = "T")]
        pub time: u64,
        /// Whether the buyer was the maker, so the taker sold.
        #[serde(rename = "m")]
        pub is_buyer_maker: bool,
        /// Whether the trade was the best price match.
        #[serde(rename = "M")]
        pub is_best_match: bool,
    }

    /// One candle of `GET /api/v3/klines`, a positional array on the wire.
    ///
    /// The wire sends a twelfth element Binance documents as "ignore"; it is
    /// dropped, and any further elements are tolerated.
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[non_exhaustive]
    pub struct Kline {
        /// Candle start.
        pub open_time: u64,
        /// Open price.
        pub open: Decimal,
        /// High price.
        pub high: Decimal,
        /// Low price.
        pub low: Decimal,
        /// Close price.
        pub close: Decimal,
        /// Base-asset volume.
        pub volume: Decimal,
        /// Candle end, inclusive.
        pub close_time: u64,
        /// Quote-asset volume.
        pub quote_volume: Decimal,
        /// Trades in the candle.
        pub trade_count: u64,
        /// Base-asset volume bought by takers.
        pub taker_buy_base_volume: Decimal,
        /// Quote-asset volume bought by takers.
        pub taker_buy_quote_volume: Decimal,
    }

    impl<'de> Deserialize<'de> for Kline {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct KlineVisitor;

            impl<'de> Visitor<'de> for KlineVisitor {
                type Value = Kline;

                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("a kline array of at least 11 elements")
                }

                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Kline, A::Error> {
                    const WHAT: &str = "a kline array of at least 11 elements";
                    let kline = Kline {
                        open_time: element(&mut seq, 0, WHAT)?,
                        open: element::<DecimalStr, _>(&mut seq, 1, WHAT)?.0,
                        high: element::<DecimalStr, _>(&mut seq, 2, WHAT)?.0,
                        low: element::<DecimalStr, _>(&mut seq, 3, WHAT)?.0,
                        close: element::<DecimalStr, _>(&mut seq, 4, WHAT)?.0,
                        volume: element::<DecimalStr, _>(&mut seq, 5, WHAT)?.0,
                        close_time: element(&mut seq, 6, WHAT)?,
                        quote_volume: element::<DecimalStr, _>(&mut seq, 7, WHAT)?.0,
                        trade_count: element(&mut seq, 8, WHAT)?,
                        taker_buy_base_volume: element::<DecimalStr, _>(&mut seq, 9, WHAT)?.0,
                        taker_buy_quote_volume: element::<DecimalStr, _>(&mut seq, 10, WHAT)?.0,
                    };
                    drain(&mut seq)?;
                    Ok(kline)
                }
            }

            deserializer.deserialize_seq(KlineVisitor)
        }
    }

    impl Serialize for Kline {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut seq = serializer.serialize_seq(Some(11))?;
            seq.serialize_element(&self.open_time)?;
            for value in [&self.open, &self.high, &self.low, &self.close, &self.volume] {
                seq.serialize_element(&value.to_string())?;
            }
            seq.serialize_element(&self.close_time)?;
            seq.serialize_element(&self.quote_volume.to_string())?;
            seq.serialize_element(&self.trade_count)?;
            seq.serialize_element(&self.taker_buy_base_volume.to_string())?;
            seq.serialize_element(&self.taker_buy_quote_volume.to_string())?;
            seq.end()
        }
    }

    /// `GET /api/v3/avgPrice`: the average price over the last `mins` minutes.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    pub struct AvgPrice {
        /// The window, in minutes.
        pub mins: u32,
        /// The average price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price: Decimal,
        /// When the average was computed.
        pub close_time: u64,
    }

    /// `GET /api/v3/ticker/24hr`: rolling 24-hour statistics.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    pub struct Ticker24h {
        /// The pair.
        pub symbol: String,
        /// Last price less the open price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price_change: Decimal,
        /// The change as a percentage.
        #[serde(with = "rust_decimal::serde::str")]
        pub price_change_percent: Decimal,
        /// Volume-weighted average price.
        #[serde(with = "rust_decimal::serde::str")]
        pub weighted_avg_price: Decimal,
        /// The previous day's close.
        #[serde(with = "rust_decimal::serde::str")]
        pub prev_close_price: Decimal,
        /// Last traded price.
        #[serde(with = "rust_decimal::serde::str")]
        pub last_price: Decimal,
        /// Last traded quantity.
        #[serde(with = "rust_decimal::serde::str")]
        pub last_qty: Decimal,
        /// Best bid price.
        #[serde(with = "rust_decimal::serde::str")]
        pub bid_price: Decimal,
        /// Quantity at the best bid.
        #[serde(with = "rust_decimal::serde::str")]
        pub bid_qty: Decimal,
        /// Best ask price.
        #[serde(with = "rust_decimal::serde::str")]
        pub ask_price: Decimal,
        /// Quantity at the best ask.
        #[serde(with = "rust_decimal::serde::str")]
        pub ask_qty: Decimal,
        /// Price 24 hours ago.
        #[serde(with = "rust_decimal::serde::str")]
        pub open_price: Decimal,
        /// Highest price.
        #[serde(with = "rust_decimal::serde::str")]
        pub high_price: Decimal,
        /// Lowest price.
        #[serde(with = "rust_decimal::serde::str")]
        pub low_price: Decimal,
        /// Base-asset volume.
        #[serde(with = "rust_decimal::serde::str")]
        pub volume: Decimal,
        /// Quote-asset volume.
        #[serde(with = "rust_decimal::serde::str")]
        pub quote_volume: Decimal,
        /// Window start.
        pub open_time: u64,
        /// Window end.
        pub close_time: u64,
        /// First trade id in the window, `-1` when there is none.
        pub first_id: i64,
        /// Last trade id in the window, `-1` when there is none.
        pub last_id: i64,
        /// Trades in the window.
        pub count: u64,
    }

    /// `GET /api/v3/ticker/price`: the last price.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[non_exhaustive]
    pub struct PriceTicker {
        /// The pair.
        pub symbol: String,
        /// Last traded price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price: Decimal,
    }

    /// `GET /api/v3/ticker/bookTicker`: the best bid and ask. It carries no
    /// time.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    pub struct BookTicker {
        /// The pair.
        pub symbol: String,
        /// Best bid price.
        #[serde(with = "rust_decimal::serde::str")]
        pub bid_price: Decimal,
        /// Quantity at the best bid.
        #[serde(with = "rust_decimal::serde::str")]
        pub bid_qty: Decimal,
        /// Best ask price.
        #[serde(with = "rust_decimal::serde::str")]
        pub ask_price: Decimal,
        /// Quantity at the best ask.
        #[serde(with = "rust_decimal::serde::str")]
        pub ask_qty: Decimal,
    }
}
```

- [ ] **Step 4: Run it to see it pass**

```sh
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test wire_agreement_spot
nix develop --command cargo clippy -p polyoxide-binance --no-default-features --features spot --all-targets -- -D warnings
```

The first prints `test result: ok. 4 passed; 0 failed`; the second prints no warning. If `every_rest_fixture_agrees_with_its_type` names a key the type does not model or invents, change the type to the capture (a field renamed, added, made an `Option` where the wire sends `""` or `null`) and run it again; do not touch the fixture.

- [ ] **Step 5: Commit**

```sh
git add -N polyoxide-binance/tests/wire_agreement_spot.rs
git commit -m "feat(binance): spot's market row types and their wire agreement (binance spot, Story 5)" -- polyoxide-binance/src/spot/types.rs polyoxide-binance/Cargo.toml polyoxide-binance/tests/wire_agreement_spot.rs
```

### Task 5.2: The market builders, the `market()` namespace and their suites

**Files:**
- Create: `polyoxide-binance/src/spot/api/market.rs`
- Modify: `polyoxide-binance/src/spot/api/mod.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs`
- Modify: `polyoxide-binance/Cargo.toml` (one `[[test]]` stanza)
- Modify: `polyoxide-binance/tests/mock_spot.rs` (append only)
- Create: `polyoxide-binance/tests/query_setters_spot.rs`
- Modify: `.claude/rules/clients.md`
- Test: `polyoxide-binance/tests/mock_spot.rs`, `polyoxide-binance/tests/query_setters_spot.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/Cargo.toml`, insert this stanza directly after the `[[test]]` stanza named `query_setters_usdm`:

```toml
[[test]]
name = "query_setters_spot"
required-features = ["spot"]
```

Create `polyoxide-binance/tests/query_setters_spot.rs`:

```rust
//! Every spot query setter, called with a typed value, sends its key and value
//! in the order it was called. A changed name or argument type fails to
//! compile; a changed key, value or order fails the test.

use polyoxide_binance::spot::{
    types::{Interval, Symbol},
    Spot,
};
use polyoxide_test_support::query::{assert_cases, Case};

fn client(base: &str) -> Spot {
    Spot::builder().base_url(base).build().unwrap()
}

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

const CASES: &[Case] = &[
    Case {
        builder: "GetTrades",
        path: "/api/v3/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .trades(&btc())
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "5")],
    },
    Case {
        builder: "GetAggTrades",
        path: "/api/v3/aggTrades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .agg_trades(&btc())
                    .from_id(6u64)
                    .start_time(6u64)
                    .end_time(6u64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("symbol", "BTCUSDT"),
            ("fromId", "6"),
            ("startTime", "6"),
            ("endTime", "6"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "GetKlines",
        path: "/api/v3/klines",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .klines(&btc(), Interval::Minute1)
                    .start_time(6u64)
                    .end_time(6u64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("symbol", "BTCUSDT"),
            ("interval", "1m"),
            ("startTime", "6"),
            ("endTime", "6"),
            ("limit", "5"),
        ],
    },
    // `GetDepth::limit` is hand-written; it sends the limit as given, past the
    // venue's maximum included.
    Case {
        builder: "GetDepth",
        path: "/api/v3/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(7u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "7")],
    },
    Case {
        builder: "GetDepth",
        path: "/api/v3/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(5001u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "5001")],
    },
    // a repeated parameter replaces the first.
    Case {
        builder: "GetKlines",
        path: "/api/v3/klines",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .klines(&btc(), Interval::Second1)
                    .limit(5u32)
                    .limit(9u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("interval", "1s"), ("limit", "9")],
    },
    Case {
        builder: "GetDepth",
        path: "/api/v3/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(5u32)
                    .limit(9u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "9")],
    },
    // Story 4's `GetExchangeInfo::symbol`, a `query_setters!` setter, pinned
    // here beside the market setters.
    Case {
        builder: "GetExchangeInfo",
        path: "/api/v3/exchangeInfo",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .exchange()
                    .exchange_info()
                    .symbol(&btc())
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT")],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value() {
    assert_cases(CASES).await;
}
```

Append to `polyoxide-binance/tests/mock_spot.rs`, after its last test (the file's existing imports and helpers are left alone; this block names everything in full and prefixes its helpers with `market_`):

```rust
// ── market ──────────────────────────────────────────────────────

fn market_fixture(name: &str) -> String {
    polyoxide_test_support::fixtures!("spot/rest").text(name)
}

fn market_client(server: &mockito::ServerGuard) -> polyoxide_binance::spot::Spot {
    polyoxide_binance::spot::Spot::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

fn market_btc() -> polyoxide_binance::spot::types::Symbol {
    polyoxide_binance::spot::types::Symbol::new("BTCUSDT").unwrap()
}

/// A route answering `body`, matched on its whole query string so a missing
/// or extra parameter fails the request.
async fn market_route(
    server: &mut mockito::ServerGuard,
    path: &str,
    query: &str,
    body: &str,
) -> mockito::Mock {
    let query = if query.is_empty() {
        mockito::Matcher::Missing
    } else {
        mockito::Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .create_async()
        .await
}

#[tokio::test]
async fn depth_sends_a_limit_only_when_asked_and_decodes_the_capture() {
    let mut server = mockito::Server::new_async().await;
    let raw = market_fixture("depth");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let limited = market_route(&mut server, "/api/v3/depth", "symbol=BTCUSDT&limit=5", &raw).await;
    let bare = market_route(&mut server, "/api/v3/depth", "symbol=BTCUSDT", &raw).await;

    let client = market_client(&server);
    let book = client
        .market()
        .depth(&market_btc())
        .limit(5)
        .send()
        .await
        .unwrap();
    assert_eq!(book.last_update_id, wire["lastUpdateId"].as_u64().unwrap());
    assert_eq!(book.bids.len(), wire["bids"].as_array().unwrap().len());
    assert_eq!(book.asks.len(), wire["asks"].as_array().unwrap().len());
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert_eq!(book.bids[0].price.to_string(), wire["bids"][0][0].as_str().unwrap());
    assert_eq!(
        book.asks[0].quantity.to_string(),
        wire["asks"][0][1].as_str().unwrap()
    );

    client.market().depth(&market_btc()).send().await.unwrap();
    limited.assert_async().await;
    bare.assert_async().await;
}

#[tokio::test]
async fn trades_send_their_limit_and_decode_the_capture() {
    let mut server = mockito::Server::new_async().await;
    let raw = market_fixture("trades");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mock = market_route(&mut server, "/api/v3/trades", "symbol=BTCUSDT&limit=3", &raw).await;
    let trades = market_client(&server)
        .market()
        .trades(&market_btc())
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(trades.len(), wire.as_array().unwrap().len());
    assert_eq!(trades[0].id, wire[0]["id"].as_u64().unwrap());
    assert_eq!(trades[0].qty.to_string(), wire[0]["qty"].as_str().unwrap());
}

#[tokio::test]
async fn agg_trades_send_their_filters() {
    let mut server = mockito::Server::new_async().await;
    let raw = market_fixture("agg_trades");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let first = wire[0]["a"].as_u64().unwrap();
    let by_id = market_route(
        &mut server,
        "/api/v3/aggTrades",
        &format!("symbol=BTCUSDT&fromId={first}&limit=3"),
        &raw,
    )
    .await;
    let by_time = market_route(
        &mut server,
        "/api/v3/aggTrades",
        "symbol=BTCUSDT&startTime=1791350000000&endTime=1791353600000&limit=3",
        &raw,
    )
    .await;

    let client = market_client(&server);
    let trades = client
        .market()
        .agg_trades(&market_btc())
        .from_id(first)
        .limit(3)
        .send()
        .await
        .unwrap();
    assert_eq!(trades.len(), wire.as_array().unwrap().len());
    assert_eq!(trades[0].id, first);
    assert_eq!(trades[0].price.to_string(), wire[0]["p"].as_str().unwrap());
    client
        .market()
        .agg_trades(&market_btc())
        .start_time(1_791_350_000_000)
        .end_time(1_791_353_600_000)
        .limit(3)
        .send()
        .await
        .unwrap();
    by_id.assert_async().await;
    by_time.assert_async().await;
}

#[tokio::test]
async fn klines_send_every_parameter_once_and_the_interval_by_its_wire_name() {
    use polyoxide_binance::spot::types::Interval;

    let mut server = mockito::Server::new_async().await;
    let minutes = market_fixture("klines");
    let seconds = market_fixture("klines_1s");
    let hourly = market_route(
        &mut server,
        "/api/v3/klines",
        "symbol=BTCUSDT&interval=1h&startTime=1791350000000&endTime=1791354000000&limit=2",
        &minutes,
    )
    .await;
    let second = market_route(
        &mut server,
        "/api/v3/klines",
        "symbol=BTCUSDT&interval=1s&limit=2",
        &seconds,
    )
    .await;

    let client = market_client(&server);
    let candles = client
        .market()
        .klines(&market_btc(), Interval::Hour1)
        .start_time(1_791_350_000_000)
        .end_time(1_791_354_000_000)
        .limit(100)
        .limit(2)
        .send()
        .await
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&minutes).unwrap();
    assert_eq!(candles.len(), wire.as_array().unwrap().len());
    assert_eq!(candles[0].open_time, wire[0][0].as_u64().unwrap());
    assert_eq!(candles[0].close.to_string(), wire[0][4].as_str().unwrap());

    let candles = client
        .market()
        .klines(&market_btc(), Interval::Second1)
        .limit(2)
        .send()
        .await
        .unwrap();
    assert_eq!(candles[1].open_time - candles[0].open_time, 1_000);
    hourly.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn avg_price_sends_the_symbol_and_decodes_the_capture() {
    let mut server = mockito::Server::new_async().await;
    let raw = market_fixture("avg_price");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mock = market_route(&mut server, "/api/v3/avgPrice", "symbol=BTCUSDT", &raw).await;
    let average = market_client(&server)
        .market()
        .avg_price(&market_btc())
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(u64::from(average.mins), wire["mins"].as_u64().unwrap());
    assert_eq!(average.price.to_string(), wire["price"].as_str().unwrap());
    assert_eq!(average.close_time, wire["closeTime"].as_u64().unwrap());
}

#[tokio::test]
async fn ticker_24h_sends_the_symbol_and_tickers_24h_sends_nothing() {
    let mut server = mockito::Server::new_async().await;
    let one_raw = market_fixture("ticker_24hr");
    let all_raw = market_fixture("tickers_24hr");
    let one_wire: serde_json::Value = serde_json::from_str(&one_raw).unwrap();
    let all_wire: serde_json::Value = serde_json::from_str(&all_raw).unwrap();
    let single = market_route(&mut server, "/api/v3/ticker/24hr", "symbol=BTCUSDT", &one_raw).await;
    let every = market_route(&mut server, "/api/v3/ticker/24hr", "", &all_raw).await;

    let client = market_client(&server);
    let ticker = client
        .market()
        .ticker_24h(&market_btc())
        .send()
        .await
        .unwrap();
    assert_eq!(ticker.symbol, "BTCUSDT");
    assert_eq!(
        ticker.last_price.to_string(),
        one_wire["lastPrice"].as_str().unwrap()
    );
    let tickers = client.market().tickers_24h().send().await.unwrap();
    assert_eq!(tickers.len(), all_wire.as_array().unwrap().len());
    single.assert_async().await;
    every.assert_async().await;
}

#[tokio::test]
async fn price_and_prices_send_the_symbol_or_nothing() {
    let mut server = mockito::Server::new_async().await;
    let one_raw = market_fixture("price");
    let all_raw = market_fixture("prices");
    let one_wire: serde_json::Value = serde_json::from_str(&one_raw).unwrap();
    let all_wire: serde_json::Value = serde_json::from_str(&all_raw).unwrap();
    let single = market_route(&mut server, "/api/v3/ticker/price", "symbol=BTCUSDT", &one_raw).await;
    let every = market_route(&mut server, "/api/v3/ticker/price", "", &all_raw).await;

    let client = market_client(&server);
    let price = client.market().price(&market_btc()).send().await.unwrap();
    assert_eq!(price.symbol, "BTCUSDT");
    assert_eq!(price.price.to_string(), one_wire["price"].as_str().unwrap());
    let prices = client.market().prices().send().await.unwrap();
    assert_eq!(prices.len(), all_wire.as_array().unwrap().len());
    single.assert_async().await;
    every.assert_async().await;
}

#[tokio::test]
async fn book_ticker_and_book_tickers_send_the_symbol_or_nothing() {
    let mut server = mockito::Server::new_async().await;
    let one_raw = market_fixture("book_ticker");
    let all_raw = market_fixture("book_tickers");
    let one_wire: serde_json::Value = serde_json::from_str(&one_raw).unwrap();
    let all_wire: serde_json::Value = serde_json::from_str(&all_raw).unwrap();
    let single =
        market_route(&mut server, "/api/v3/ticker/bookTicker", "symbol=BTCUSDT", &one_raw).await;
    let every = market_route(&mut server, "/api/v3/ticker/bookTicker", "", &all_raw).await;

    let client = market_client(&server);
    let book = client
        .market()
        .book_ticker(&market_btc())
        .send()
        .await
        .unwrap();
    assert_eq!(book.symbol, "BTCUSDT");
    assert_eq!(
        book.ask_qty.to_string(),
        one_wire["askQty"].as_str().unwrap()
    );
    let books = client.market().book_tickers().send().await.unwrap();
    assert_eq!(books.len(), all_wire.as_array().unwrap().len());
    single.assert_async().await;
    every.assert_async().await;
}

/// Spec §3.1's table, builder by builder: what a request will be charged
/// before it is sent.
#[test]
fn every_market_builder_charges_its_route() {
    use polyoxide_binance::spot::{types::Interval, weight::Cost, Spot};

    let spot = Spot::new().unwrap();
    let market = spot.market();
    let btc = market_btc();
    let w = Cost::Weight;

    // depth: 5 without a limit and up to 100, then 25, 50 and 250 by band.
    assert_eq!(market.depth(&btc).cost(), w(5));
    for (limit, weight) in [
        (1, 5),
        (100, 5),
        (101, 25),
        (500, 25),
        (501, 50),
        (1000, 50),
        (1001, 250),
        (5000, 250),
        (5001, 250),
    ] {
        assert_eq!(market.depth(&btc).limit(limit).cost(), w(weight), "limit {limit}");
    }
    // The limit set again re-prices again.
    assert_eq!(market.depth(&btc).limit(1001).limit(100).cost(), w(5));

    // These do not change with a limit.
    assert_eq!(market.trades(&btc).cost(), w(25));
    assert_eq!(market.trades(&btc).limit(1000).cost(), w(25));
    assert_eq!(market.agg_trades(&btc).cost(), w(4));
    assert_eq!(market.agg_trades(&btc).limit(1).cost(), w(4));
    assert_eq!(market.klines(&btc, Interval::Minute1).cost(), w(2));
    assert_eq!(market.klines(&btc, Interval::Minute1).limit(1000).cost(), w(2));
    assert_eq!(market.avg_price(&btc).cost(), w(2));

    // The tickers: 2 for one symbol; 80, 4 and 4 for all of them.
    assert_eq!(market.ticker_24h(&btc).cost(), w(2));
    assert_eq!(market.tickers_24h().cost(), w(80));
    assert_eq!(market.price(&btc).cost(), w(2));
    assert_eq!(market.prices().cost(), w(4));
    assert_eq!(market.book_ticker(&btc).cost(), w(2));
    assert_eq!(market.book_tickers().cost(), w(4));
}

/// The cost a builder reports is the cost the budget is charged: the re-priced
/// route reaches the send loop, not only `cost()`.
#[tokio::test]
async fn a_deep_depth_is_charged_its_band() {
    polyoxide_test_support::minute::wait_unless_within(0..=57_000, 100).await;
    let mut server = mockito::Server::new_async().await;
    let _deep = market_route(
        &mut server,
        "/api/v3/depth",
        "symbol=BTCUSDT&limit=1001",
        &market_fixture("depth"),
    )
    .await;
    let client = market_client(&server);
    client
        .market()
        .depth(&market_btc())
        .limit(1001)
        .send()
        .await
        .unwrap();
    assert_eq!(client.weight_budget().used(), 250);
}

#[tokio::test]
async fn all_tickers_are_charged_their_weight() {
    polyoxide_test_support::minute::wait_unless_within(0..=57_000, 100).await;
    let mut server = mockito::Server::new_async().await;
    let _every = market_route(
        &mut server,
        "/api/v3/ticker/24hr",
        "",
        &market_fixture("tickers_24hr"),
    )
    .await;
    let client = market_client(&server);
    client.market().tickers_24h().send().await.unwrap();
    assert_eq!(client.weight_budget().used(), 80);
}

#[tokio::test]
async fn an_unknown_symbol_on_a_market_route_is_a_venue_error() {
    use polyoxide_binance::spot::{types::Symbol, SpotError};

    let mut server = mockito::Server::new_async().await;
    let raw = market_fixture("error_invalid_symbol");
    let wire: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mock = server
        .mock("GET", "/api/v3/depth")
        .match_query(mockito::Matcher::Exact("symbol=NOPEUSDT".to_owned()))
        .with_status(400)
        .with_body(&raw)
        .create_async()
        .await;
    let error = market_client(&server)
        .market()
        .depth(&Symbol::new("NOPEUSDT").unwrap())
        .send()
        .await
        .unwrap_err();
    mock.assert_async().await;
    match error {
        SpotError::Venue { status, code, msg } => {
            assert_eq!(status, 400);
            assert_eq!(code, wire["code"].as_i64().unwrap());
            assert_eq!(msg, wire["msg"].as_str().unwrap());
        }
        other => panic!("expected a venue error, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run them to see them fail**

```sh
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot --test query_setters_spot
```

Neither compiles: `error[E0599]: no method named `market` found for struct `Spot` in the current scope`, once per use in each file.

- [ ] **Step 3: Implement**

Create `polyoxide-binance/src/spot/api/market.rs`:

```rust
//! Market data: depth, trades, aggregate trades, klines, the average price and
//! the three tickers.

use polyoxide_core::HttpClient;

use crate::spot::{
    error::SpotError,
    request::{route_builder, Routed},
    types::{
        AggTrade, AvgPrice, BookTicker, Depth, Interval, Kline, PriceTicker, Symbol, Ticker24h,
        Trade,
    },
    weight::{Cost, Route},
};

/// Market namespace.
#[derive(Debug, Clone)]
pub struct MarketApi {
    pub(crate) http: HttpClient,
}

impl MarketApi {
    fn request<T>(&self, route: Route) -> Routed<T> {
        Routed::new(&self.http, route)
    }

    /// `GET /api/v3/depth`: an order book snapshot. Without a limit it returns
    /// 100 levels for weight 5; see [`GetDepth::limit`] for the rest.
    pub fn depth(&self, symbol: &Symbol) -> GetDepth {
        GetDepth {
            request: self
                .request(Route::Depth { limit: None })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/trades`: recent trades, oldest first (weight 25 at any
    /// limit).
    pub fn trades(&self, symbol: &Symbol) -> GetTrades {
        GetTrades {
            request: self.request(Route::Trades).query("symbol", symbol),
        }
    }

    /// `GET /api/v3/aggTrades`: aggregate trades, oldest first (weight 4 at
    /// any limit).
    pub fn agg_trades(&self, symbol: &Symbol) -> GetAggTrades {
        GetAggTrades {
            request: self.request(Route::AggTrades).query("symbol", symbol),
        }
    }

    /// `GET /api/v3/klines`: candles, oldest first (weight 2 at any limit).
    pub fn klines(&self, symbol: &Symbol, interval: Interval) -> GetKlines {
        GetKlines {
            request: self
                .request(Route::Klines)
                .query("symbol", symbol)
                .query("interval", interval),
        }
    }

    /// `GET /api/v3/avgPrice`: the current average price (weight 2).
    pub fn avg_price(&self, symbol: &Symbol) -> GetAvgPrice {
        GetAvgPrice {
            request: self.request(Route::AvgPrice).query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/24hr` for one symbol (weight 2).
    pub fn ticker_24h(&self, symbol: &Symbol) -> GetTicker24h {
        GetTicker24h {
            request: self
                .request(Route::Ticker24h { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/24hr` for every symbol (weight 80).
    pub fn tickers_24h(&self) -> GetTickers24h {
        GetTickers24h {
            request: self.request(Route::Ticker24h { all: true }),
        }
    }

    /// `GET /api/v3/ticker/price` for one symbol (weight 2).
    pub fn price(&self, symbol: &Symbol) -> GetPrice {
        GetPrice {
            request: self
                .request(Route::Price { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/price` for every symbol (weight 4).
    pub fn prices(&self) -> GetPrices {
        GetPrices {
            request: self.request(Route::Price { all: true }),
        }
    }

    /// `GET /api/v3/ticker/bookTicker` for one symbol (weight 2).
    pub fn book_ticker(&self, symbol: &Symbol) -> GetBookTicker {
        GetBookTicker {
            request: self
                .request(Route::BookTicker { all: false })
                .query("symbol", symbol),
        }
    }

    /// `GET /api/v3/ticker/bookTicker` for every symbol (weight 4).
    pub fn book_tickers(&self) -> GetBookTickers {
        GetBookTickers {
            request: self.request(Route::BookTicker { all: true }),
        }
    }
}

route_builder! {
    /// Request builder for `GET /api/v3/avgPrice`.
    GetAvgPrice => AvgPrice
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/24hr` for one symbol.
    GetTicker24h => Ticker24h
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/24hr` for every symbol.
    GetTickers24h => Vec<Ticker24h>
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/price` for one symbol.
    GetPrice => PriceTicker
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/price` for every symbol.
    GetPrices => Vec<PriceTicker>
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/bookTicker` for one symbol.
    GetBookTicker => BookTicker
}

route_builder! {
    /// Request builder for `GET /api/v3/ticker/bookTicker` for every symbol.
    GetBookTickers => Vec<BookTicker>
}

/// Request builder for `GET /api/v3/depth`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetDepth {
    request: Routed<Depth>,
}

impl GetDepth {
    /// Levels per side, sent as given. Any value is accepted, and one past
    /// 5000 is truncated to the maximum. Sets the weight: up to 100 costs 5,
    /// up to 500 costs 25, up to 1000 costs 50, more costs 250.
    pub fn limit(mut self, limit: u32) -> Self {
        self.request = self
            .request
            .route(Route::Depth { limit: Some(limit) })
            .query("limit", limit);
        self
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Depth, SpotError> {
        self.request.send().await
    }
}

/// Request builder for `GET /api/v3/trades`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetTrades {
    request: Routed<Vec<Trade>>,
}

impl GetTrades {
    polyoxide_core::query_setters! {
        /// Rows to return, 500 by default and up to 1000; the venue truncates
        /// past 1000. The weight is 25 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Trade>, SpotError> {
        self.request.send().await
    }
}

/// Request builder for `GET /api/v3/aggTrades`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetAggTrades {
    request: Routed<Vec<AggTrade>>,
}

impl GetAggTrades {
    polyoxide_core::query_setters! {
        /// Start from this aggregate trade id, inclusive.
        from_id: u64 => "fromId",
        /// Earliest trade time, Unix milliseconds.
        start_time: u64 => "startTime",
        /// Latest trade time, Unix milliseconds.
        end_time: u64 => "endTime",
        /// Rows to return, 500 by default and up to 1000; the venue truncates
        /// past 1000. The weight is 4 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<AggTrade>, SpotError> {
        self.request.send().await
    }
}

/// Request builder for `GET /api/v3/klines`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetKlines {
    request: Routed<Vec<Kline>>,
}

impl GetKlines {
    polyoxide_core::query_setters! {
        /// Earliest candle start, Unix milliseconds.
        start_time: u64 => "startTime",
        /// Latest candle start, Unix milliseconds.
        end_time: u64 => "endTime",
        /// Candles to return, 500 by default and up to 1000; the venue
        /// truncates past 1000. The weight is 2 whatever the limit.
        limit: u32 => "limit",
    }

    /// What sending this request will cost.
    pub fn cost(&self) -> Cost {
        self.request.cost()
    }

    /// Execute the request.
    pub async fn send(self) -> Result<Vec<Kline>, SpotError> {
        self.request.send().await
    }
}
```

In `polyoxide-binance/src/spot/api/mod.rs`, replace

```rust
pub mod exchange;
pub mod health;
```

with

```rust
pub mod exchange;
pub mod health;
pub mod market;
```

In `polyoxide-binance/src/spot/mod.rs`, make two edits:

1. In the file's `use crate::spot::{ … };` statement, add `market::MarketApi` to the `api::{ … }` list, so it reads `api::{exchange::ExchangeApi, health::Health, market::MarketApi}`.
2. In `impl Spot`'s `polyoxide_core::namespaces! { http; … }` invocation, add this entry after the last one (`exchange: ExchangeApi,`):

```rust
        /// Market data: depth, trades, aggregate trades, klines, the average
        /// price and the tickers.
        market: MarketApi,
```

In `.claude/rules/clients.md`, make three replacements.

Replace

```
Ten setters stay hand-written because they are not single query writes:

- Binance's `GetKlines::limit` and `GetDepth::limit`, which also re-price the route;
```

with

```
Eleven setters stay hand-written because they are not single query writes:

- Binance's `GetKlines::limit` and `GetDepth::limit`, and spot's
  `spot::api::market::GetDepth::limit`, which also re-price the route (spot's
  `GetKlines::limit` does not, since a spot `klines` costs 2 at any limit);
```

and replace the line

```
`tests/query_setters_usdm.rs` (Binance), `tests/query_setters_clob.rs`,
```

with

```
`tests/query_setters_usdm.rs` and `tests/query_setters_spot.rs` (Binance), `tests/query_setters_clob.rs`,
```

- [ ] **Step 4: Run them to see them pass**

```sh
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot --test query_setters_spot --test wire_agreement_spot
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command cargo fmt --all -- --check
```

The first prints `test result: ok` for each of the three targets, with every `mock_spot` test Story 4 wrote still passing beside the new ones; the second, the third and the fourth print no failure, warning or diff. If `cargo fmt --all -- --check` prints a diff, run `nix develop --command cargo fmt --all` and check again.

- [ ] **Step 5: Commit**

```sh
git add -N polyoxide-binance/src/spot/api/market.rs polyoxide-binance/tests/query_setters_spot.rs
git commit -m "feat(binance): spot's market routes and their suites (binance spot, Story 5)" -- polyoxide-binance/src/spot/api/market.rs polyoxide-binance/src/spot/api/mod.rs polyoxide-binance/src/spot/mod.rs polyoxide-binance/Cargo.toml polyoxide-binance/tests/mock_spot.rs polyoxide-binance/tests/query_setters_spot.rs .claude/rules/clients.md
```

### Task 5.3: The `weight_probe_spot` example

**Files:**
- Create: `polyoxide-binance/examples/weight_probe_spot.rs`
- Modify: `polyoxide-binance/Cargo.toml` (one `[[example]]` stanza)
- Test: `polyoxide-binance/examples/weight_probe_spot.rs` (its own unit tests, run with the example)

`examples/weight_probe.rs` and its `[[example]]` stanza are not touched.

This task's code holds the landmine marker `.gzip(`: the example's bare client keeps `.gzip(true)` exactly as `examples/weight_probe.rs:126` has it (a bare client that asks for gzip, as the measured probe does), and no other client in this story gains a `.gzip(` call. The supervisor should dispatch this task's developer with `effort: xhigh`.

- [ ] **Step 1: Write the failing test**

In `polyoxide-binance/Cargo.toml`, insert this stanza directly after the `weight_probe` stanza, which reads

```toml
[[example]]
name = "weight_probe"
path = "examples/weight_probe.rs"
test = true
required-features = ["usdm"]
```

```toml
# The spot module's probe. Its own example, not a `--product` of the one above:
# that example is `test = true`, so the test-bodies gate counts every item in it.
[[example]]
name = "weight_probe_spot"
path = "examples/weight_probe_spot.rs"
test = true
required-features = ["spot"]
```

Create `polyoxide-binance/examples/weight_probe_spot.rs` holding only its test module for now, so the test can fail first:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn every_case_requests_the_route_it_checks() {
        for (route, path) in super::cases() {
            assert_eq!(path.split('?').next(), Some(route.path()), "{path}");
        }
    }
}
```

- [ ] **Step 2: Run it to see it fail**

```sh
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --example weight_probe_spot
```

It fails to compile: `error[E0425]: cannot find function `cases` in module `super``, and `error[E0601]: `main` function not found in crate `weight_probe_spot``.

- [ ] **Step 3: Implement**

Replace the whole of `polyoxide-binance/examples/weight_probe_spot.rs` with:

```rust
//! Measures each spot route's weight on the live host and compares it with
//! `Route::cost`, the table the client charges.
//!
//! ```sh
//! cargo run -p polyoxide-binance --no-default-features --features spot --example weight_probe_spot
//! ```
//!
//! A route's weight is the rise in `X-MBX-USED-WEIGHT-1M` across its request,
//! so every case runs back to back inside one UTC minute; the probe waits for a
//! fresh minute first. Another process spending weight on the same IP makes a
//! row read high, and so does one calling `data-api.binance.vision`, which
//! shares the counter. Costs about 870 weight. Exits 1 if any row differs from
//! the table.

use std::time::Duration;

use polyoxide_binance::spot::weight::{Cost, Route};
use polyoxide_test_support::minute;

const BASE: &str = "https://api.binance.com";

/// The depth limits probed: the edges of every band, and one past the
/// documented maximum.
const DEPTH_LIMITS: [u32; 8] = [1, 100, 101, 500, 501, 1000, 1001, 5001];

/// `(route, path and query)`. The query carries exactly the parameters the
/// route names, which `every_case_requests_the_route_it_checks` holds.
fn cases() -> Vec<(Route, String)> {
    let mut cases = vec![
        (Route::Ping, "/api/v3/ping".to_owned()),
        (Route::Time, "/api/v3/time".to_owned()),
        (Route::ExchangeInfo, "/api/v3/exchangeInfo".to_owned()),
        (
            Route::ExchangeInfo,
            "/api/v3/exchangeInfo?symbol=BTCUSDT".to_owned(),
        ),
        (Route::Trades, "/api/v3/trades?symbol=BTCUSDT".to_owned()),
        (
            Route::Trades,
            "/api/v3/trades?symbol=BTCUSDT&limit=1000".to_owned(),
        ),
        (
            Route::AggTrades,
            "/api/v3/aggTrades?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::AggTrades,
            "/api/v3/aggTrades?symbol=BTCUSDT&limit=1000".to_owned(),
        ),
        (
            Route::Klines,
            "/api/v3/klines?symbol=BTCUSDT&interval=1m".to_owned(),
        ),
        (
            Route::Klines,
            "/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=1000".to_owned(),
        ),
        (
            Route::AvgPrice,
            "/api/v3/avgPrice?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Ticker24h { all: false },
            "/api/v3/ticker/24hr?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Ticker24h { all: true },
            "/api/v3/ticker/24hr".to_owned(),
        ),
        (
            Route::Price { all: false },
            "/api/v3/ticker/price?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::Price { all: true },
            "/api/v3/ticker/price".to_owned(),
        ),
        (
            Route::BookTicker { all: false },
            "/api/v3/ticker/bookTicker?symbol=BTCUSDT".to_owned(),
        ),
        (
            Route::BookTicker { all: true },
            "/api/v3/ticker/bookTicker".to_owned(),
        ),
        (
            Route::Depth { limit: None },
            "/api/v3/depth?symbol=BTCUSDT".to_owned(),
        ),
    ];
    for limit in DEPTH_LIMITS {
        cases.push((
            Route::Depth { limit: Some(limit) },
            format!("/api/v3/depth?symbol=BTCUSDT&limit={limit}"),
        ));
    }
    cases
}

/// Sends one request and returns the minute's count after it. Stops the probe
/// on any answer but 200: a refused request is not a call the client makes,
/// and sending on after a 429 is how a ban starts.
async fn used_after(client: &polyoxide_core::reqwest::Client, path: &str) -> u32 {
    let response = client
        .get(format!("{BASE}{path}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{path}: {e}"));
    let status = response.status();
    if status != polyoxide_core::reqwest::StatusCode::OK {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("none")
            .to_owned();
        let body = response.text().await.unwrap_or_default();
        eprintln!("{path} answered {status} (Retry-After {retry_after}): {body:.300}");
        eprintln!("stopping: nothing further is sent");
        std::process::exit(2);
    }
    response
        .headers()
        .get("x-mbx-used-weight-1m")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{path} answered {status} with no weight header"))
}

async fn wait_for_a_fresh_minute() {
    // Three seconds past the boundary: the server's count was seen to fall to
    // 1 only within 3 s of it, and the local clock is not the server's.
    if let Some(wait) = minute::wait_needed(3_000..=20_000, 3_000, minute::ms_into_minute()) {
        eprintln!(
            "waiting {} s for a fresh minute",
            wait.as_millis().div_ceil(1000)
        );
        tokio::time::sleep(wait).await;
    }
}

#[tokio::main]
async fn main() {
    let client = polyoxide_core::reqwest::Client::builder()
        .gzip(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    wait_for_a_fresh_minute().await;
    let mut previous = used_after(&client, "/api/v3/ping").await;
    if previous > 1 {
        eprintln!(
            "the minute's count is {previous} after one ping: another process on this IP is \
             spending weight, and rows may differ"
        );
    }
    let mut differing = 0;
    for (route, path) in cases() {
        let now = used_after(&client, &path).await;
        if now < previous {
            eprintln!(
                "{path}: the minute's count fell from {previous} to {now}, so the minute \
                 rolled over mid-run; run the probe again"
            );
            std::process::exit(2);
        }
        let measured = now - previous;
        previous = now;
        let Cost::Weight(table) = route.cost();
        let verdict = if measured == table {
            "ok"
        } else {
            differing += 1;
            "DIFFERS"
        };
        println!("{path:58} table {table:>3}  measured {measured:>3}  {verdict}");
    }
    if differing > 0 {
        eprintln!("{differing} rows differ from Route::cost; run the probe again, and if they still differ, update it and docs/specs/binance/OBSERVED.md");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_requests_the_route_it_checks() {
        for (route, path) in cases() {
            assert_eq!(path.split('?').next(), Some(route.path()), "{path}");
            let limit = path
                .split(['?', '&'])
                .find_map(|kv| kv.strip_prefix("limit="));
            let one_symbol = path.contains("symbol=");
            match route {
                Route::Ping | Route::Time => assert!(!path.contains('?'), "{path}"),
                Route::Depth { limit: expected } => {
                    assert_eq!(limit.map(|l| l.parse::<u32>().unwrap()), expected, "{path}")
                }
                Route::Ticker24h { all } | Route::Price { all } | Route::BookTicker { all } => {
                    assert_eq!(one_symbol, !all, "{path}")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn every_depth_band_edge_is_probed() {
        let routes: Vec<Route> = cases().into_iter().map(|(route, _)| route).collect();
        assert!(routes.contains(&Route::Depth { limit: None }));
        for limit in [1, 100, 101, 500, 501, 1000, 1001] {
            assert!(
                routes.contains(&Route::Depth { limit: Some(limit) }),
                "{limit}"
            );
        }
    }

    #[test]
    fn every_route_is_probed_with_and_without_what_does_not_change_its_weight() {
        let cases = cases();
        let probed = |route: Route| cases.iter().any(|(r, _)| *r == route);
        for route in [
            Route::Ping,
            Route::Time,
            Route::ExchangeInfo,
            Route::Trades,
            Route::AggTrades,
            Route::Klines,
            Route::AvgPrice,
            Route::Ticker24h { all: false },
            Route::Ticker24h { all: true },
            Route::Price { all: false },
            Route::Price { all: true },
            Route::BookTicker { all: false },
            Route::BookTicker { all: true },
        ] {
            assert!(probed(route), "{route:?}");
        }
        // The routes whose weight Binance says does not depend on `limit` or
        // `symbol` are probed both ways, so a change in either shows.
        for (route, with) in [
            (Route::Trades, "limit="),
            (Route::AggTrades, "limit="),
            (Route::Klines, "limit="),
            (Route::ExchangeInfo, "symbol="),
        ] {
            let paths: Vec<&String> = cases
                .iter()
                .filter(|(r, _)| *r == route)
                .map(|(_, p)| p)
                .collect();
            assert!(paths.iter().any(|p| p.contains(with)), "{route:?} with");
            assert!(paths.iter().any(|p| !p.contains(with)), "{route:?} without");
        }
    }
}
```

- [ ] **Step 4: Run it to see it pass**

```sh
nix develop --command cargo test -p polyoxide-binance --all-features --example weight_probe_spot
nix develop --command cargo test -p polyoxide-binance --all-features --example weight_probe
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
```

The first prints `test result: ok. 3 passed; 0 failed`; the second still prints `test result: ok. 2 passed; 0 failed`, since that example is untouched; the third prints no warning; the fourth exits 0 and lists `examples/weight_probe_spot.rs`'s items as new at HEAD, with no changed or lost item.

The probe is not run against the live host in this task. Running it spends about 870 weight of the IP's minute and is Story 1's measurement repeated: the owner or the supervisor runs `cargo run -p polyoxide-binance --no-default-features --features spot --example weight_probe_spot` once, and a row that prints `DIFFERS` goes to `docs/specs/binance/OBSERVED.md` (above its anchor) and to the owner.

- [ ] **Step 5: Commit**

```sh
git add -N polyoxide-binance/examples/weight_probe_spot.rs
git commit -m "feat(binance): a weight probe for the spot routes (binance spot, Story 5)" -- polyoxide-binance/examples/weight_probe_spot.rs polyoxide-binance/Cargo.toml
```

### Task 5.4: The depth re-pricing row in `MUTANTS.md`, proved

**Files:**
- Modify: `docs/MUTANTS.md`
- Modify: `.github/scripts/tests/test_mutants_ledger.py` (`SNIPPETS`, three entries appended after its last)
- Test: `polyoxide-binance/tests/mock_spot.rs`, `.github/scripts/tests/test_mutants_ledger.py`

The row cites `file:line` as every other row does: the setter's line in `src/spot/api/market.rs` and each failing test's line in `tests/mock_spot.rs`. `test_mutants_ledger.py` holds a snippet for every `file:line` the ledger names and fails when the two sets differ, so the ledger test gains one `SNIPPETS` entry per citation.

- [ ] **Step 1: Prove the mutant fails the tests**

In `polyoxide-binance/src/spot/api/market.rs`, `GetDepth::limit`, delete the line `.route(Route::Depth { limit: Some(limit) })`, so the chain reads `self.request = self.request.query("limit", limit);` and the route stays at `Depth { limit: None }`. Then run:

```sh
nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot -j 4 -- every_market_builder_charges_its_route a_deep_depth_is_charged_its_band
```

Both fail: `every_market_builder_charges_its_route` with `limit 101` reading `left: Weight(5)`, `right: Weight(25)`, and `a_deep_depth_is_charged_its_band` with `left: 5`, `right: 250`. Restore the line and run the same command again: `test result: ok. 2 passed`. Then:

```sh
git diff --exit-code -- polyoxide-binance/src/spot/api/market.rs
```

exits 0.

- [ ] **Step 2: Add the row, its ledger snippets and its proof note**

Find the three lines to cite, with the file restored:

```sh
grep -n 'route(Route::Depth { limit: Some(limit) })' polyoxide-binance/src/spot/api/market.rs
grep -n 'fn every_market_builder_charges_its_route(' polyoxide-binance/tests/mock_spot.rs
grep -n 'fn a_deep_depth_is_charged_its_band(' polyoxide-binance/tests/mock_spot.rs
```

Each prints exactly one line. Call the three line numbers `S`, `T1` and `T2`, in that order.

In `docs/MUTANTS.md`, insert this row directly after the row beginning `| (k) A `418` holds every request on the weight budget`, with `S`, `T1` and `T2` written as the numbers the greps printed:

```
| (l) `GetDepth::limit` re-prices the route by its band, so a deeper spot book is charged its weight | `polyoxide-binance/src/spot/api/market.rs:S`, `GetDepth::limit`'s `.route(Route::Depth { limit: Some(limit) })` | Delete that call, leaving the route at `Depth { limit: None }` and the `limit` query still sent | `polyoxide-binance/tests/mock_spot.rs:T1` `every_market_builder_charges_its_route`; `polyoxide-binance/tests/mock_spot.rs:T2` `a_deep_depth_is_charged_its_band` |
```

In `.github/scripts/tests/test_mutants_ledger.py`, replace the last entry of `SNIPPETS` and the brace that closes it:

```python
    ("polyoxide-polymarket/src/sports/supervised.rs", 419):
        "async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11(",
}
```

with the same two entries followed by three more (again with `S`, `T1` and `T2` as numbers), then the brace:

```python
    ("polyoxide-polymarket/src/sports/supervised.rs", 419):
        "async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11(",
    ("polyoxide-binance/src/spot/api/market.rs", S): ".route(Route::Depth { limit: Some(limit) })",
    ("polyoxide-binance/tests/mock_spot.rs", T1): "fn every_market_builder_charges_its_route(",
    ("polyoxide-binance/tests/mock_spot.rs", T2): "async fn a_deep_depth_is_charged_its_band(",
}
```

Insert a paragraph directly before the line `## The rules`, with today's date from `date +%F` and the short hash `git rev-parse --short HEAD` prints:

```
Row (l), spot's depth re-pricing, was proved on <the date> on top of `<the hash>`, on Rust 1.99.0 with `-j 4`: the two tests passed, then failed with the mutant in place, then passed again once the line was restored, with `git diff --exit-code` clean, through the command in Story 5 of the Binance spot plan (`cargo test -p polyoxide-binance --no-default-features --features spot --test mock_spot -j 4 -- every_market_builder_charges_its_route a_deep_depth_is_charged_its_band`).
```

- [ ] **Step 3: Run the ledger and the gates**

```sh
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps
```

The ledger test passes: the row cites three new lines, `SNIPPETS` holds each of them, and each snippet is found on its line. (With `T1` changed by one in the row only, `test_every_citation_has_a_recorded_snippet_and_every_snippet_is_cited` fails, which is the ledger doing its work; put it back.) The body diff exits 0, and `cargo hack` builds `spot` alone and `usdm` alone.

- [ ] **Step 4: Commit**

```sh
git commit -m "docs(mutants): spot's depth re-pricing row, proved (binance spot, Story 5)" -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```

## Story 6: `spot::venue`, keys, registration, `live_spot` and the umbrella's `binance-spot`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/src/spot/key.rs`, `polyoxide-binance/src/spot/venue.rs`, `polyoxide-binance/src/spot/mod.rs`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/tests/venue_spot.rs`, `polyoxide-binance/tests/live_spot.rs`, `polyoxide/Cargo.toml`, `polyoxide/src/lib.rs`, `polyoxide/tests/parse_key.rs`, `polyoxide/tests/live_venue.rs`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/polyoxide@binance-spot.txt`, `ci/venue-gate-skip.txt`, `.claude/rules/venue-traits.md`, `.claude/rules/ci-gates.md`, `.github/workflows/ci.yml`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`, `Cargo.lock`
- **Interfaces:** exposes "Story 6 → Story 7"; uses Story 3's `ProductClass::Spot` and `SpotMarket`, and Stories 4 and 5's client, routes and types
- **Verify:** `cargo test -p polyoxide-binance --all-features && cargo test -p polyoxide --all-features && cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests -q'`
- **Review:** deep (the venue mapping every trait consumer reads, and the umbrella's public surface)

Gives spot its market keys and its place in the venue-neutral traits, registers it, and puts it under the umbrella. Task 6.1 writes `spot::key` (`binance.spot:<SYMBOL>`, `ProductClass::Spot`) and registers the product, its class and its identifier in the crate's metadata, then regenerates the generated regions; the registration also makes `spot` a product id the venue gate looks for in the foundation crates, where `polyoxide-venue`'s tests already use "spot" as a made-up product's name, so the task adds `spot` to the gate's generic-word list. Task 6.2 writes `spot::venue`: `MarketData`, `Instruments`, `Trades` and `Candles` for `Spot` with `InstrumentExt`, `BookExt`, `TradeExt` and `CandleExt` (a quote has no extension), the skipped-row WARN, and `lib.rs`'s pin of `Spot` to `SpotMarket`; `venue_spot` holds each method to the native client's values on Story 1's fixtures, the weight each method is charged, the refusals made before anything is sent, and the WARN. Task 6.3 writes `live_spot`, the live entry and the nightly job it becomes, with the drift detector `live_responses_carry_no_unmodelled_keys` and a test that fails when `instruments` skips a row. Task 6.4 adds the umbrella's `binance-spot` feature, `parse_key`'s branch, the prelude names, the passthrough and bundle pins, new tests appended to `parse_key.rs` and `live_venue.rs` (AD-12: no existing test changes), and the umbrella's `binance-spot` dependency allowlist, regenerated. The rules files that state these facts change in the commits that make them true (AD-21): `ci-gates.md` and `venue-traits.md` in Tasks 6.1 and 6.2, and in Task 6.4 `ci-gates.md` and the `features` job's two header-pin steps in `ci.yml`. The code holds none of the profile's landmine markers.

### Task 6.1: `spot::key` and the product's registration

**Files:**
- Create: `polyoxide-binance/src/spot/key.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs` (the module block, to its first form here)
- Modify: `polyoxide-binance/Cargo.toml` (the `[package.metadata.polyoxide]` keys `readme`, `products`, `classes`, `identifiers`)
- Modify: `ci/venue-gate-skip.txt`
- Modify: `.claude/rules/ci-gates.md` (the venue gate's generic words and Binance's identifiers, ~lines 110-113)
- Modify: `.claude/rules/venue-traits.md` (the key-space bullets and the `ProductClass` bullet, ~lines 26-28 and 42-43)
- Modify (generated, by `gen_registry.py --write`): `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-behavioral.yml`, `.github/workflows/nightly-schema.yml`
- Test: `polyoxide-binance/src/spot/key.rs` (its unit tests)

- [ ] **Step 1: Write the key module and its tests**

Create `polyoxide-binance/src/spot/key.rs`. Its last test, `the_ids_are_the_ones_the_crate_registers`, is the failing one until Step 3 registers the product.

````rust
//! Spot's market keys: `binance.spot:<SYMBOL>`, one per trading pair.
//!
//! The id is the pair's [`Symbol`], whose ASCII letters are uppercased, so
//! `binance.spot:btcusdt` and `binance.spot:BTCUSDT` are one key.
//!
//! ```
//! use polyoxide_binance::spot::{key, types::Symbol};
//! use polyoxide_venue::RawKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parsed = key::parse(&RawKey::parse("binance.spot:btcusdt")?)?;
//! assert_eq!(parsed, key::key(&Symbol::new("BTCUSDT")?));
//! assert_eq!(parsed.to_string(), "binance.spot:BTCUSDT");
//! assert_eq!(key::symbol(&parsed)?.as_str(), "BTCUSDT");
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;

use polyoxide_venue::{KeyError, KeySpace, MarketKey, ProductClass, ProductId, RawKey, VenueId};

use crate::spot::types::Symbol;

/// Binance's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("binance");

/// Spot's product id, as the crate registers it.
pub const PRODUCT: ProductId = ProductId::from_static("spot");

/// Spot's keys. Each names one trading pair by its [`Symbol`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpotKeys;

impl KeySpace for SpotKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PRODUCT;
    const CLASS: ProductClass = ProductClass::Spot;
    type Id = Symbol;

    fn parse_id(text: &str) -> Result<Symbol, String> {
        Symbol::new(text).map_err(|err| err.to_string())
    }

    fn id_text(id: &Symbol) -> Cow<'_, str> {
        Cow::Borrowed(id.as_str())
    }
}

/// The key of the pair `symbol`.
pub fn key(symbol: &Symbol) -> MarketKey {
    SpotKeys::key(symbol)
}

/// The key `raw` names, with its symbol's ASCII letters uppercased.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `raw` is not `binance.spot`, and
/// [`KeyError::InvalidId`] when its id is not a symbol.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    SpotKeys::parse(raw)
}

/// The symbol `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `binance.spot` key.
pub fn symbol(key: &MarketKey) -> Result<Symbol, KeyError> {
    SpotKeys::id(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    #[test]
    fn a_lowercase_symbol_names_the_same_key() {
        let lower = parse(&raw("binance.spot:btcusdt")).unwrap();
        let upper = parse(&raw("binance.spot:BTCUSDT")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.to_string(), "binance.spot:BTCUSDT");
        assert_eq!(lower, key(&Symbol::new("BTCUSDT").unwrap()));
        assert_eq!(
            parse(&raw("binance.spot:1000satsusdt")).unwrap().id(),
            "1000SATSUSDT"
        );
    }

    #[test]
    fn a_key_gives_back_its_symbol() {
        let btc = Symbol::new("BTCUSDT").unwrap();
        assert_eq!(symbol(&key(&btc)), Ok(btc));
    }

    #[test]
    fn a_symbol_binance_cannot_carry_and_another_product_are_refused() {
        assert!(matches!(
            parse(&raw("binance.spot:BTC/USDT")),
            Err(KeyError::InvalidId { .. })
        ));
        for other in ["binance.usdm:BTCUSDT", "binance.margin:BTCUSDT"] {
            assert!(
                matches!(parse(&raw(other)), Err(KeyError::WrongProduct { .. })),
                "{other}"
            );
        }
    }

    #[test]
    fn the_ids_are_the_ones_the_crate_registers() {
        let declared = polyoxide_test_support::registration!();
        assert_eq!(declared.venue.as_deref(), Some(VENUE.as_str()));
        assert!(
            declared
                .products
                .iter()
                .any(|product| product == PRODUCT.as_str()),
            "{declared:?}"
        );
        assert_eq!(
            declared.classes.get(PRODUCT.as_str()).map(String::as_str),
            Some(SpotKeys::CLASS.as_str()),
            "{declared:?}"
        );
        assert_eq!(SpotKeys::CLASS.as_str(), "spot");
    }
}
````

Now replace Story 4's module block in `polyoxide-binance/src/spot/mod.rs`. Quote this block (it sits after the module docs):

```rust
pub mod api;
pub mod error;
mod policy;
pub mod request;
pub mod types;
pub mod weight;

pub use error::SpotError;
pub use weight::WeightBudget;
```

and put in its place (Task 6.2 adds `venue` to it):

```rust
pub mod api;
pub mod error;
pub mod key;
mod policy;
pub mod request;
pub mod types;
pub mod weight;

pub use error::SpotError;
pub use key::key;
pub use weight::WeightBudget;
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::key`

Expected: the crate builds, and the run ends `test result: FAILED. 3 passed; 1 failed`, the failure `spot::key::tests::the_ids_are_the_ones_the_crate_registers`, whose assertion on `declared.products` prints the registration (`products: ["usdm"]`, no `spot`). The other three pass: the key code needs no registration.

- [ ] **Step 3: Register the product**

In `polyoxide-binance/Cargo.toml`, replace the registration lines (these are the current ones):

```toml
readme = "Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`); re-exported by the umbrella as `polyoxide::binance`"
venue = "binance"
products = ["usdm"]
classes = { usdm = "perpetual-future" }
mirrors = ["binance"]
identifiers = ["fapi", "fstream", "X-MBX"]
```

with:

```toml
readme = "Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`) and `spot` (spot market data); re-exported by the umbrella as `polyoxide::binance`"
venue = "binance"
products = ["usdm", "spot"]
classes = { usdm = "perpetual-future", spot = "spot" }
mirrors = ["binance"]
identifiers = ["fapi", "fstream", "X-MBX", "api/v3"]
```

`spot` is now a product id the venue gate looks for in `polyoxide-venue`, `-core`, `-ws` and `-test-support`, where the made-up venue in their tests names its product "spot" and `ProductClass::Spot` is a variant. It is an ordinary word there, as `data` and `events` are, so it joins the gate's generic-word list. In `ci/venue-gate-skip.txt`, append a line holding `spot` after the last word (`events`), and add one comment line above the first word saying why:

```text
# `spot` is also a product class (`ProductClass::Spot`) and the name a made-up
# product takes in the foundation crates' tests.
```

Put that comment directly after the file's last existing comment line, before `data`.

The rules files state both facts (AD-21), so they change in this commit. In `.claude/rules/ci-gates.md`, replace

```markdown
  `ci/venue-gate-skip.txt` (`data` and `events`);
```

with

```markdown
  `ci/venue-gate-skip.txt` (`data`, `events` and `spot`);
```

and replace

```markdown
- each venue's `identifiers` (Polymarket's `Poly-RateLimit`; Binance's `fapi`, `fstream` and
  `X-MBX`, its hosts and weight header);
```

with

```markdown
- each venue's `identifiers` (Polymarket's `Poly-RateLimit`; Binance's `fapi`, `fstream`,
  `X-MBX` and `api/v3`, its hosts, weight header and spot's path prefix);
```

In `.claude/rules/venue-traits.md`, add spot's key space beside USDⓈ-M's. Replace

```markdown
  - Binance's `usdm::key`: `UsdmKeys`, over `Symbol` (uppercases ASCII); `usdm::key(&symbol)`,
    `key::parse`, `key::symbol`.
```

with

```markdown
  - Binance's `usdm::key`: `UsdmKeys`, over `Symbol` (uppercases ASCII); `usdm::key(&symbol)`,
    `key::parse`, `key::symbol`.
  - Binance's `spot::key`: `SpotKeys`, over `Symbol` (uppercases ASCII); `spot::key(&symbol)`,
    `key::parse`, `key::symbol`.
```

and name the product under the `Spot` class. Story 3 left the `ProductClass` bullet reading

```markdown
- Each `KeySpace` declares its product's `ProductClass` as `CLASS` (ADR 0001): `EventContract`
  (the clob, Kalshi's `predictions`), `PerpetualFuture` (perps, USDⓈ-M) or `Spot` (no product yet),
  `#[non_exhaustive]`, a variant added only when a product of that class lands. Every key carries it as
```

replace those three lines with

```markdown
- Each `KeySpace` declares its product's `ProductClass` as `CLASS` (ADR 0001): `EventContract`
  (the clob, Kalshi's `predictions`), `PerpetualFuture` (perps, USDⓈ-M) or `Spot` (Binance's
  `spot`), `#[non_exhaustive]`, a variant added only when a product of that class lands. Every key
  carries it as
```

Regenerate what the registration feeds:

Run: `nix develop --command python3 scripts/gen_registry.py --write`

It rewrites the generated regions of `CLAUDE.md` (the workspace line of `polyoxide-binance`), `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md` and the two nightly workflows, and only those; `git diff --stat` after it shows which. Do not edit any of them by hand.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::key`
Expected: `test result: ok. 4 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --doc spot::key`
Expected: `test result: ok. 1 passed`.

Then the gates that read the registration:

Run: `nix develop --command cargo fmt --all -- --check`
Run: `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings`
Run: `nix develop --command python3 scripts/gen_registry.py --check`
Run: `nix develop --command python3 scripts/venue_gate.py`
Expected: each exits 0 (the gate prints no hit and no stale exception). If the gate still reports "spot" in a foundation file, the skip line is missing or misspelt.

Do not run `.github/scripts/tests/test_ci_workflow.py` here, nor the whole `.github/scripts` suite: it derives the expected `headers_spot` pin commands from the `products` this task registers and requires the umbrella's `full` to turn on `spot`, and both land in Task 6.4 (`ci.yml` and `full`). From this commit until Task 6.4's, `test_the_features_job_runs_each_header_pin_in_its_minimal_build` and `test_the_features_job_runs_each_header_pin_under_the_umbrellas_full` are red by design, and the story's `pytest tests -q` runs only in Task 6.4 Step 4, where it is green. Do not reorder the tasks.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/key.rs
git commit -m "feat(binance): spot market keys and the product's registration (binance spot, Story 6)" -- polyoxide-binance/src/spot/key.rs polyoxide-binance/src/spot/mod.rs polyoxide-binance/Cargo.toml ci/venue-gate-skip.txt .claude/rules/ci-gates.md .claude/rules/venue-traits.md CLAUDE.md README.md SELF-HEALING.md docs/specs/INDEX.md docs/ARCHITECTURE.md .github/workflows/nightly-behavioral.yml .github/workflows/nightly-schema.yml
```

### Task 6.2: `spot::venue` and the bundle pin

**Files:**
- Create: `polyoxide-binance/src/spot/venue.rs`
- Modify: `polyoxide-binance/src/spot/mod.rs` (the module block, to its final form)
- Modify: `polyoxide-binance/src/lib.rs` (the bundle pin)
- Modify: `.claude/rules/venue-traits.md` (the pins sentence, ~lines 107-109)
- Modify: `polyoxide-binance/Cargo.toml` (the `venue_spot` `[[test]]` stanza)
- Test: `polyoxide-binance/tests/venue_spot.rs`

- [ ] **Step 1: Write the failing test**

Add the stanza to `polyoxide-binance/Cargo.toml`, directly above the existing

```toml
[[test]]
name = "venue_usdm"
required-features = ["usdm"]
```

as

```toml
[[test]]
name = "venue_spot"
required-features = ["spot"]

```

Create `polyoxide-binance/tests/venue_spot.rs`. It holds each trait method to the native client's value on Story 1's fixtures. The mock routes match the whole query string, in the order a builder appends its pairs (`symbol`, then `interval`, then the time bounds, then `limit`).

```rust
//! Spot through the venue-neutral traits, each held as its `Dyn` wrapper,
//! against mock servers serving the captured fixtures: every method returns
//! the values the native client decodes from the same body.

use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_binance::spot::{
    self,
    types::{Filter, Interval, Symbol},
    venue::{BookExt, CandleExt, InstrumentExt, TradeExt},
    Spot, SpotError,
};
use polyoxide_test_support::{fixtures, minute};
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, Classify, DynCandles, DynInstruments,
    DynMarketData, DynTrades, Instruments, KeySpace, Level, MarketData, MarketKey, NotListed,
    ProductClass, ProductId, Quote, RawKey, Side, SizeUnit, TradeId, Trades, UnixMillis, VenueId,
};
use serde_json::Value;

fn client(server: &ServerGuard) -> Spot {
    Spot::builder().base_url(server.url()).build().unwrap()
}

fn fixture(name: &str) -> String {
    fixtures!("spot/rest").text(name)
}

fn btc_symbol() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

fn btc() -> MarketKey {
    spot::key(&btc_symbol())
}

/// A route answering `body` `hits` times, matched on its whole query string.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str, hits: usize) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("x-mbx-used-weight-1m", "1")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

#[tokio::test]
async fn quote_is_the_native_book_ticker_and_carries_no_extension() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/ticker/bookTicker",
        "symbol=BTCUSDT",
        &fixture("book_ticker"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .book_ticker(&btc_symbol())
        .send()
        .await
        .unwrap();
    assert!(native.bid_price < native.ask_price, "a real quote");

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let quote = venue.quote(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(quote.key, btc());
    assert_eq!(
        quote.bid,
        Some(Level::new(native.bid_price, native.bid_qty))
    );
    assert_eq!(
        quote.ask,
        Some(Level::new(native.ask_price, native.ask_qty))
    );
    // `ticker/bookTicker` carries no time and nothing beyond the record's fields.
    assert_eq!(quote.time, None);
    assert_eq!(quote.extensions, Quote::new(btc()).extensions);
}

#[tokio::test]
async fn book_returns_every_level_of_the_native_depth_best_first() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.market().depth(&btc_symbol()).send().await.unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let book = venue.book(&btc()).await.unwrap();
    mock.assert_async().await;

    let bids: Vec<Level> = native
        .bids
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    let asks: Vec<Level> = native
        .asks
        .iter()
        .map(|l| Level::new(l.price, l.quantity))
        .collect();
    assert!(!bids.is_empty() && !asks.is_empty());
    assert_eq!(book.bids, bids);
    assert_eq!(book.asks, asks);
    assert!(
        book.bids.windows(2).all(|w| w[0].price > w[1].price),
        "bids best first"
    );
    assert!(
        book.asks.windows(2).all(|w| w[0].price < w[1].price),
        "asks best first"
    );
    // The default depth carries no time.
    assert_eq!(book.time, None);
    let ext = book.extensions.get::<BookExt>().unwrap();
    assert_eq!(ext.last_update_id, Some(native.last_update_id));
}

#[tokio::test]
async fn instrument_reads_its_pair_from_exchange_info() {
    let mut server = Server::new_async().await;
    // The host answers `?symbol=` with that one row; the fixture holds several,
    // so the read has to pick the right one out.
    let mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "symbol=BTCUSDT",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .exchange()
        .exchange_info()
        .symbol(&btc_symbol())
        .send()
        .await
        .unwrap();
    let row = native
        .symbols
        .iter()
        .find(|row| row.symbol == "BTCUSDT")
        .unwrap();

    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);
    let instrument = venue.instrument(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(instrument.key, btc());
    assert_eq!(instrument.name, "BTCUSDT");
    assert_eq!(instrument.size_unit, SizeUnit::Base);
    assert_eq!(
        instrument.base_asset.as_deref(),
        Some(row.base_asset.as_str())
    );
    assert_eq!(
        instrument.quote_asset.as_deref(),
        Some(row.quote_asset.as_str())
    );
    assert_eq!(instrument.tradable, Some(true));
    assert!(
        row.filters
            .iter()
            .any(|filter| matches!(filter, Filter::Notional { .. })),
        "the fixture's BTCUSDT carries a NOTIONAL filter"
    );
    for filter in &row.filters {
        match filter {
            Filter::PriceFilter { tick_size, .. } => {
                assert_eq!(instrument.tick_size, Some(*tick_size))
            }
            Filter::LotSize {
                min_qty, step_size, ..
            } => {
                assert_eq!(instrument.min_size, Some(*min_qty));
                assert_eq!(instrument.size_increment, Some(*step_size));
            }
            Filter::Notional { min_notional, .. } => {
                assert_eq!(instrument.min_notional, Some(*min_notional))
            }
            _ => {}
        }
    }
    assert!(instrument.tick_size.is_some() && instrument.min_size.is_some());
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.status.as_ref(), Some(&row.status));
    assert_eq!(ext.permission_sets.as_ref(), Some(&row.permission_sets));
    assert_eq!(ext.order_types.as_ref(), Some(&row.order_types));
    assert_eq!(
        ext.is_spot_trading_allowed,
        Some(row.is_spot_trading_allowed)
    );
    assert_eq!(
        ext.is_margin_trading_allowed,
        Some(row.is_margin_trading_allowed)
    );
    assert_eq!(ext.filters.as_ref(), Some(&row.filters));
}

#[tokio::test]
async fn a_pair_the_answer_does_not_hold_is_not_listed() {
    let mut server = Server::new_async().await;
    let mut body: Value = serde_json::from_str(&fixture("exchange_info")).unwrap();
    body["symbols"] = Value::Array(Vec::new());
    let _mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "symbol=BTCUSDT",
        &body.to_string(),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.instrument(&btc()).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(
        err.source.downcast_ref::<NotListed>(),
        Some(&NotListed::new(btc()))
    );
}

#[tokio::test]
async fn a_pair_the_host_refuses_is_the_hosts_own_refusal() {
    // The captured answer to `exchangeInfo?symbol=NOPEUSDT`: a 400 with the
    // host's `{code, msg}`.
    let body = fixture("error_invalid_symbol");
    let code = serde_json::from_str::<Value>(&body).unwrap()["code"]
        .as_i64()
        .unwrap();
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/exchangeInfo")
        .match_query(Matcher::Exact("symbol=NOPEUSDT".to_owned()))
        .with_status(400)
        .with_body(body)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let missing = spot::key(&Symbol::new("NOPEUSDT").unwrap());
    let err = venue.instrument(&missing).await.unwrap_err();
    assert_eq!(
        err.class,
        Class::VenueRefusal {
            code: Some(code.to_string())
        }
    );
    assert!(matches!(
        err.source.downcast_ref::<SpotError>(),
        Some(SpotError::Venue { status: 400, .. })
    ));
}

#[tokio::test]
async fn instruments_lists_every_pair_with_its_status() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "",
        &fixture("exchange_info"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    mock.assert_async().await;

    let keys: Vec<String> = instruments.iter().map(|i| i.key.to_string()).collect();
    let expected: Vec<String> = native
        .symbols
        .iter()
        .map(|row| format!("binance.spot:{}", row.symbol))
        .collect();
    assert_eq!(keys, expected);
    for (instrument, row) in instruments.iter().zip(&native.symbols) {
        assert_eq!(
            instrument.tradable,
            Some(row.status.as_str() == "TRADING"),
            "{}",
            row.symbol
        );
    }
    assert!(
        instruments.iter().any(|i| i.tradable == Some(false)),
        "the fixture holds a symbol that is not TRADING"
    );
}

/// Records the target, level and fields of every event, for the skipped row's
/// WARN.
#[derive(Clone, Default)]
struct Events(Arc<std::sync::Mutex<Vec<(String, tracing::Level, String)>>>);

struct Fields(String);

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!("{}={value:?} ", field.name()));
    }
}

impl tracing::Subscriber for Events {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let metadata = event.metadata();
        let mut fields = Fields(String::new());
        event.record(&mut fields);
        self.0
            .lock()
            .unwrap()
            .push((metadata.target().to_owned(), *metadata.level(), fields.0));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

#[tokio::test]
async fn instruments_skips_a_row_that_does_not_convert_and_returns_the_rest() {
    // The current-thread runtime runs the call on this thread, so the scoped
    // subscriber sees its events.
    let events = Events::default();
    let _scope = tracing::subscriber::set_default(events.clone());
    // With one dispatcher registered, tracing-core works out the interest of
    // a callsite another test's thread hits first from that thread's
    // default, which has none, and caches it as never for every thread. With
    // a second one registered it asks each registered dispatcher instead.
    let _second = tracing::Dispatch::new(Events::default());
    let mut server = Server::new_async().await;
    let mut info: Value = serde_json::from_str(&fixture("exchange_info")).unwrap();
    let rows = info["symbols"].as_array_mut().unwrap();
    let valid = rows.len();
    let mut bad = rows[0].clone();
    bad["symbol"] = Value::from("BAD-SYMBOL");
    rows.insert(1, bad);
    let mock = route(
        &mut server,
        "/api/v3/exchangeInfo",
        "",
        &info.to_string(),
        2,
    )
    .await;
    let client = client(&server);
    let native = client.exchange().exchange_info().send().await.unwrap();
    assert_eq!(native.symbols.len(), valid + 1);

    let venue: Arc<DynInstruments<'static>> = DynInstruments::new_arc(client);
    let instruments = venue.instruments().await.unwrap();
    mock.assert_async().await;

    assert_eq!(instruments.len(), valid);
    let keys: Vec<String> = instruments.iter().map(|i| i.key.to_string()).collect();
    let expected: Vec<String> = native
        .symbols
        .iter()
        .filter(|row| row.symbol != "BAD-SYMBOL")
        .map(|row| format!("binance.spot:{}", row.symbol))
        .collect();
    assert_eq!(keys, expected);

    // One WARN, under the venue module's own target, naming the symbol.
    let warnings: Vec<(String, String)> = events
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, level, _)| *level == tracing::Level::WARN)
        .map(|(target, _, fields)| (target.clone(), fields.clone()))
        .collect();
    assert_eq!(
        warnings.len(),
        1,
        "the skipped row's warnings: {warnings:?}"
    );
    let (target, fields) = &warnings[0];
    assert_eq!(target, "polyoxide_binance::spot::venue");
    assert!(fields.contains("symbol=BAD-SYMBOL"), "{fields}");
    assert!(fields.contains("error="), "{fields}");
}

#[tokio::test]
async fn trades_are_the_native_aggregate_trades_with_the_takers_side() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/aggTrades",
        "symbol=BTCUSDT&limit=1000",
        &fixture("agg_trades"),
        2,
    )
    .await;
    let client = client(&server);
    let native = client
        .market()
        .agg_trades(&btc_symbol())
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert!(!native.is_empty());

    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);
    let trades = venue.trades(&btc()).await.unwrap();
    mock.assert_async().await;

    assert_eq!(trades.len(), native.len());
    for (trade, row) in trades.iter().zip(&native) {
        assert_eq!(trade.key, btc());
        assert_eq!(trade.id, Some(TradeId::new(row.id.to_string())));
        assert_eq!(trade.price, row.price);
        assert_eq!(trade.size, row.quantity);
        assert_eq!(trade.notional, None);
        assert_eq!(
            trade.taker_side,
            Some(if row.is_buyer_maker {
                Side::Sell
            } else {
                Side::Buy
            })
        );
        assert_eq!(trade.time, UnixMillis(row.time));
        let ext = trade.extensions.get::<TradeExt>().unwrap();
        assert_eq!(ext.first_trade_id, Some(row.first_trade_id));
        assert_eq!(ext.last_trade_id, Some(row.last_trade_id));
        assert_eq!(ext.is_best_match, Some(row.is_best_match));
    }
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );
}

#[tokio::test]
async fn candles_are_the_native_klines_from_start() {
    let mut server = Server::new_async().await;
    let query =
        "symbol=BTCUSDT&interval=1h&startTime=1791380000000&endTime=1791390000000&limit=1000";
    let mock = route(&mut server, "/api/v3/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::Hour1)
        .start_time(1_791_380_000_000)
        .end_time(1_791_390_000_000)
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert!(!native.is_empty());

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000))
        .end(UnixMillis(1_791_390_000_000));
    let candles = venue.candles(&btc(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.len());
    for (candle, row) in candles.iter().zip(&native) {
        assert_eq!(candle.interval, CandleInterval::Hour1);
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.open, Some(row.open));
        assert_eq!(candle.high, Some(row.high));
        assert_eq!(candle.low, Some(row.low));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.notional, Some(row.quote_volume));
        assert_eq!(candle.trades, Some(row.trade_count));
        let ext = candle.extensions.get::<CandleExt>().unwrap();
        assert_eq!(ext.close_time, Some(UnixMillis(row.close_time)));
        assert_eq!(ext.taker_buy_volume, Some(row.taker_buy_base_volume));
        assert_eq!(ext.taker_buy_notional, Some(row.taker_buy_quote_volume));
    }
}

#[tokio::test]
async fn candles_with_no_end_send_no_end_time() {
    let mut server = Server::new_async().await;
    let query = "symbol=BTCUSDT&interval=1h&startTime=1791380000000&limit=1000";
    let mock = route(&mut server, "/api/v3/klines", query, &fixture("klines"), 2).await;
    let client = client(&server);
    let native = client
        .market()
        .klines(&btc_symbol(), Interval::Hour1)
        .start_time(1_791_380_000_000)
        .limit(1000)
        .send()
        .await
        .unwrap();

    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
    assert_eq!(request.end, None);
    let candles = venue.candles(&btc(), &request).await.unwrap();
    mock.assert_async().await;

    assert_eq!(candles.len(), native.len());
    assert!(!candles.is_empty());
    for (candle, row) in candles.iter().zip(&native) {
        assert_eq!(candle.open_time, UnixMillis(row.open_time));
        assert_eq!(candle.close, row.close);
        assert_eq!(candle.volume, Some(row.volume));
    }
}

#[tokio::test]
async fn a_lowercase_key_reaches_the_host_uppercased() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/api/v3/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let lower = spot::key::parse(&RawKey::parse("binance.spot:btcusdt").unwrap()).unwrap();
    venue.book(&lower).await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn a_venue_error_reaches_the_caller_classified_with_its_native_error_as_source() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/api/v3/depth")
        .match_query(Matcher::Any)
        .with_status(400)
        .with_body(r#"{"code":-1121,"msg":"Invalid symbol."}"#)
        .create_async()
        .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue.book(&btc()).await.unwrap_err();
    assert_eq!(
        err.class,
        Class::VenueRefusal {
            code: Some("-1121".into())
        }
    );
    assert!(err.source.downcast_ref::<SpotError>().is_some());
}

/// A product of another venue that is also a spot product, to hand Spot a key
/// of its own class it does not own.
struct Elsewhere;

impl KeySpace for Elsewhere {
    const VENUE: VenueId = VenueId::from_static("elsewhere");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    const CLASS: ProductClass = ProductClass::Spot;
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

#[tokio::test]
async fn a_key_of_another_product_is_refused_by_every_trait_before_anything_is_sent() {
    let mut server = Server::new_async().await;
    let untouched = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let client = client(&server);
    let foreign = Elsewhere::key(&"BTCUSDT".to_owned());
    let market: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client.clone());
    let trades: Arc<DynTrades<'static>> = DynTrades::new_arc(client.clone());
    let candles: Arc<DynCandles<'static>> = DynCandles::new_arc(client);
    let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
    for err in [
        market.quote(&foreign).await.unwrap_err(),
        market.book(&foreign).await.unwrap_err(),
        market.instrument(&foreign).await.unwrap_err(),
        trades.trades(&foreign).await.unwrap_err(),
        candles.candles(&foreign, &request).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest, "{err}");
    }
    untouched.assert_async().await;
}

/// The weight a client has charged after `call` made its one request, on a
/// fresh client, clear of a UTC minute boundary. The mock reports a used
/// weight of 1, below every charge here, so the budget's count is the charge.
async fn weight_of<F, Fut>(path: &str, query: &str, body: &str, call: F) -> u32
where
    F: FnOnce(Spot) -> Fut,
    Fut: Future<Output = ()>,
{
    minute::wait_unless_within(0..=57_000, 100).await;
    let mut server = Server::new_async().await;
    let mock = route(&mut server, path, query, body, 1).await;
    let client = client(&server);
    call(client.clone()).await;
    mock.assert_async().await;
    client.weight_budget().used()
}

#[tokio::test]
async fn each_method_is_charged_the_weight_its_route_costs() {
    let info = fixture("exchange_info");
    let by_symbol = weight_of(
        "/api/v3/exchangeInfo",
        "symbol=BTCUSDT",
        &info,
        |venue| async move {
            venue.instrument(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(by_symbol, 20, "instrument");
    let listing = weight_of("/api/v3/exchangeInfo", "", &info, |venue| async move {
        venue.instruments().await.unwrap();
    })
    .await;
    assert_eq!(listing, 20, "instruments");
    let quote = weight_of(
        "/api/v3/ticker/bookTicker",
        "symbol=BTCUSDT",
        &fixture("book_ticker"),
        |venue| async move {
            venue.quote(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(quote, 2, "quote");
    let book = weight_of(
        "/api/v3/depth",
        "symbol=BTCUSDT",
        &fixture("depth"),
        |venue| async move {
            venue.book(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(book, 5, "book: the default depth");
    let trades = weight_of(
        "/api/v3/aggTrades",
        "symbol=BTCUSDT&limit=1000",
        &fixture("agg_trades"),
        |venue| async move {
            venue.trades(&btc()).await.unwrap();
        },
    )
    .await;
    assert_eq!(trades, 4, "trades");
    let candles = weight_of(
        "/api/v3/klines",
        "symbol=BTCUSDT&interval=1h&startTime=1791380000000&limit=1000",
        &fixture("klines"),
        |venue| async move {
            let request = CandleQuery::new(CandleInterval::Hour1, UnixMillis(1_791_380_000_000));
            venue.candles(&btc(), &request).await.unwrap();
        },
    )
    .await;
    assert_eq!(candles, 2, "candles");
}

#[tokio::test]
async fn a_trait_error_carries_the_native_errors_class_fault_and_wait() {
    let cases: [(usize, &str, Option<&str>); 3] = [
        (400, r#"{"code":-1121,"msg":"Invalid symbol."}"#, None),
        (418, r#"{"code":-1003,"msg":"Banned."}"#, Some("5")),
        (451, r#"{"code":0,"msg":"Restricted location."}"#, Some("3")),
    ];
    for (status, body, retry_after) in cases {
        let mut server = Server::new_async().await;
        let mut mock = server
            .mock("GET", "/api/v3/depth")
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(body);
        if let Some(wait) = retry_after {
            mock = mock.with_header("retry-after", wait);
        }
        let _mock = mock.create_async().await;

        // One client apiece, so the native call's hold is not the trait call's.
        let native = client(&server)
            .market()
            .depth(&btc_symbol())
            .send()
            .await
            .unwrap_err();
        let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
        let err = venue.book(&btc()).await.unwrap_err();

        assert_eq!(err.class, native.class(), "{status}");
        assert_eq!(err.is_fault(), native.is_fault(), "{status}");
        assert_eq!(err.retry_after(), native.retry_after(), "{status}");
        assert!(err.source.downcast_ref::<SpotError>().is_some(), "{status}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test venue_spot`

Expected: it does not compile. Rustc prints `error[E0432]: unresolved import polyoxide_binance::spot::venue` (and `error[E0599]` for `quote`, `book` and the other trait methods on `Spot`, which has no impl of the traits yet).

- [ ] **Step 3: Implement**

Create `polyoxide-binance/src/spot/venue.rs`:

````rust
//! Spot through the venue-neutral market-data traits.
//!
//! [`Spot`] implements [`MarketData`], [`Instruments`], [`Trades`] and
//! [`Candles`] over its own routes, keyed `binance.spot:<SYMBOL>` by the
//! constructors in `spot::key`. Spot has no funding, so it implements no
//! `Funding`. What Binance alone publishes rides in this module's
//! `<Record>Ext` types, one per record, in each record's
//! [`Extensions`](polyoxide_venue::Extensions); a quote has none, since
//! `ticker/bookTicker` carries nothing beyond the record's fields.
//!
//! `instruments` skips a pair whose `exchangeInfo` row does not convert, with
//! a WARN naming the symbol and the error, rather than failing the whole
//! listing.
//!
//! | Method | Route | Weight |
//! | --- | --- | --- |
//! | `instrument` | `exchangeInfo?symbol=` | 20 |
//! | `instruments` | `exchangeInfo` | 20 |
//! | `quote` | `ticker/bookTicker` | 2 |
//! | `book` | `depth`, no limit (100 levels) | 5 |
//! | `trades` | `aggTrades` (the last 1000) | 4 |
//! | `candles` | `klines`, from `start` (up to 1000) | 2 |
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use polyoxide_binance::spot::{self, types::Symbol, Spot};
//! use polyoxide_venue::{DynMarketData, MarketData};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(Spot::new()?);
//! let quote = venue.quote(&spot::key(&Symbol::new("BTCUSDT")?)).await?;
//! println!("bid {:?} ask {:?}", quote.bid, quote.ask);
//! # Ok(())
//! # }
//! ```

use polyoxide_venue::{
    Book, Candle, CandleInterval, CandleQuery, Candles, Class, ClassifiedError, Instrument,
    Instruments, Level, MarketData, MarketKey, NotListed, Quote, Side, SizeUnit, Trade, TradeId,
    Trades, UnixMillis,
};
use rust_decimal::Decimal;

use crate::spot::{
    key,
    types::{self, Filter, Interval, Symbol, SymbolInfo, SymbolStatus},
    Spot,
};

/// Binance's facts about a trading pair, beside [`Instrument`]'s fields.
///
/// It is not `Eq`: [`Filter`] is `PartialEq` alone.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// Trading, halted and so on.
    pub status: Option<SymbolStatus>,
    /// The permission groups that may trade the pair.
    pub permission_sets: Option<Vec<Vec<String>>>,
    /// The order types the pair accepts.
    pub order_types: Option<Vec<String>>,
    /// Whether spot trading is allowed.
    pub is_spot_trading_allowed: Option<bool>,
    /// Whether margin trading is allowed.
    pub is_margin_trading_allowed: Option<bool>,
    /// Every order filter, the ones [`Instrument`]'s fields come from
    /// included.
    pub filters: Option<Vec<Filter>>,
}

/// Binance's facts about a book, beside [`Book`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BookExt {
    /// The book update id of the snapshot, to align it with the depth stream.
    pub last_update_id: Option<u64>,
}

/// Binance's facts about an aggregate trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// The first trade merged into it.
    pub first_trade_id: Option<u64>,
    /// The last trade merged into it.
    pub last_trade_id: Option<u64>,
    /// Whether the trade was the best price match.
    pub is_best_match: Option<bool>,
}

/// Binance's facts about a candle, beside [`Candle`]'s fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct CandleExt {
    /// When the candle closes, inclusive.
    pub close_time: Option<UnixMillis>,
    /// The size takers bought.
    pub taker_buy_volume: Option<Decimal>,
    /// The quote-currency amount takers bought.
    pub taker_buy_notional: Option<Decimal>,
}

fn level(level: &types::Level) -> Level {
    Level::new(level.price, level.quantity)
}

/// The instrument a pair's row describes.
fn instrument(info: &SymbolInfo) -> Result<Instrument, ClassifiedError> {
    // A row whose symbol no key can carry is a response this client cannot read.
    let symbol = Symbol::new(info.symbol.as_str())
        .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
    let mut instrument = Instrument::new(key::key(&symbol), info.symbol.clone(), SizeUnit::Base);
    instrument.base_asset = Some(info.base_asset.clone());
    instrument.quote_asset = Some(info.quote_asset.clone());
    instrument.tradable = Some(info.status == SymbolStatus::Trading);
    for filter in &info.filters {
        match filter {
            Filter::PriceFilter { tick_size, .. } => instrument.tick_size = Some(*tick_size),
            Filter::LotSize {
                min_qty, step_size, ..
            } => {
                instrument.min_size = Some(*min_qty);
                instrument.size_increment = Some(*step_size);
            }
            Filter::Notional { min_notional, .. } => {
                instrument.min_notional = Some(*min_notional)
            }
            _ => {}
        }
    }
    instrument.extensions = polyoxide_venue::Extensions::of(InstrumentExt {
        status: Some(info.status.clone()),
        permission_sets: Some(info.permission_sets.clone()),
        order_types: Some(info.order_types.clone()),
        is_spot_trading_allowed: Some(info.is_spot_trading_allowed),
        is_margin_trading_allowed: Some(info.is_margin_trading_allowed),
        filters: Some(info.filters.clone()),
    });
    Ok(instrument)
}

/// The kline width a candle interval is.
fn interval(interval: CandleInterval) -> Interval {
    match interval {
        CandleInterval::Minute1 => Interval::Minute1,
        CandleInterval::Minute5 => Interval::Minute5,
        CandleInterval::Minute15 => Interval::Minute15,
        CandleInterval::Hour1 => Interval::Hour1,
        CandleInterval::Hour4 => Interval::Hour4,
        CandleInterval::Day1 => Interval::Day1,
        CandleInterval::Week1 => Interval::Week1,
    }
}

impl MarketData for Spot {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let info = self
            .exchange()
            .exchange_info()
            .symbol(&symbol)
            .send()
            .await?;
        match info
            .symbols
            .iter()
            .find(|row| row.symbol == symbol.as_str())
        {
            Some(row) => instrument(row),
            None => Err(NotListed::new(key.clone()).into()),
        }
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let ticker = self.market().book_ticker(&symbol).send().await?;
        let mut quote = Quote::new(key.clone());
        quote.bid = Some(Level::new(ticker.bid_price, ticker.bid_qty));
        quote.ask = Some(Level::new(ticker.ask_price, ticker.ask_qty));
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let depth = self.market().depth(&symbol).send().await?;
        let mut book = Book::new(key.clone());
        book.bids = depth.bids.iter().map(level).collect();
        book.asks = depth.asks.iter().map(level).collect();
        book.extensions = polyoxide_venue::Extensions::of(BookExt {
            last_update_id: Some(depth.last_update_id),
        });
        Ok(book)
    }
}

impl Instruments for Spot {
    /// Every pair `exchangeInfo` lists. A row that does not convert (a symbol
    /// no key can carry) is logged at WARN with its symbol and the error, and
    /// skipped: the rest are returned.
    async fn instruments(&self) -> Result<Vec<Instrument>, ClassifiedError> {
        let info = self.exchange().exchange_info().send().await?;
        Ok(info
            .symbols
            .iter()
            .filter_map(|row| match instrument(row) {
                Ok(instrument) => Some(instrument),
                Err(err) => {
                    tracing::warn!(symbol = %row.symbol, error = %err, "skipping an exchangeInfo row that does not convert");
                    None
                }
            })
            .collect())
    }
}

impl Trades for Spot {
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let rows = self.market().agg_trades(&symbol).limit(1000).send().await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut trade =
                    Trade::new(key.clone(), row.price, row.quantity, UnixMillis(row.time));
                trade.id = Some(TradeId::new(row.id.to_string()));
                // The buyer made the market, so the taker sold.
                trade.taker_side = Some(if row.is_buyer_maker {
                    Side::Sell
                } else {
                    Side::Buy
                });
                trade.extensions = polyoxide_venue::Extensions::of(TradeExt {
                    first_trade_id: Some(row.first_trade_id),
                    last_trade_id: Some(row.last_trade_id),
                    is_best_match: Some(row.is_best_match),
                });
                trade
            })
            .collect())
    }
}

impl Candles for Spot {
    async fn candles(
        &self,
        key: &MarketKey,
        query: &CandleQuery,
    ) -> Result<Vec<Candle>, ClassifiedError> {
        let symbol = key::symbol(key)?;
        let mut request = self
            .market()
            .klines(&symbol, interval(query.interval))
            .start_time(query.start.0);
        if let Some(end) = query.end {
            request = request.end_time(end.0);
        }
        // 1000 candles cost weight 2, as the default 500 does.
        let rows = request.limit(1000).send().await?;
        Ok(rows
            .iter()
            .map(|row| {
                let mut candle = Candle::new(
                    key.clone(),
                    query.interval,
                    UnixMillis(row.open_time),
                    row.close,
                );
                candle.open = Some(row.open);
                candle.high = Some(row.high);
                candle.low = Some(row.low);
                candle.volume = Some(row.volume);
                candle.notional = Some(row.quote_volume);
                candle.trades = Some(row.trade_count);
                candle.extensions = polyoxide_venue::Extensions::of(CandleExt {
                    close_time: Some(UnixMillis(row.close_time)),
                    taker_buy_volume: Some(row.taker_buy_base_volume),
                    taker_buy_notional: Some(row.taker_buy_quote_volume),
                });
                candle
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_candle_interval_is_a_kline_width() {
        let widths: Vec<&str> = CandleInterval::ALL
            .iter()
            .map(|each| interval(*each).as_str())
            .collect();
        assert_eq!(widths, ["1m", "5m", "15m", "1h", "4h", "1d", "1w"]);
    }

    #[test]
    fn a_row_whose_symbol_no_key_carries_is_a_decode_failure() {
        let info: types::ExchangeInfo = serde_json::from_str(include_str!(
            "../../tests/fixtures/spot/rest/exchange_info.json"
        ))
        .unwrap();
        let mut row = info.symbols[0].clone();
        row.symbol = "BTC/USDT".to_owned();
        let err = instrument(&row).unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }
}
````

Now bring the module block of `polyoxide-binance/src/spot/mod.rs` to its final form. Replace Task 6.1's block:

```rust
pub mod api;
pub mod error;
pub mod key;
mod policy;
pub mod request;
pub mod types;
pub mod weight;

pub use error::SpotError;
pub use key::key;
pub use weight::WeightBudget;
```

with:

```rust
pub mod api;
pub mod error;
pub mod key;
mod policy;
pub mod request;
pub mod types;
pub mod venue;
pub mod weight;

pub use error::SpotError;
pub use key::key;
pub use weight::WeightBudget;
```

In `polyoxide-binance/src/lib.rs`, pin `Spot` to its bundle. The file ends with this block:

```rust
// Every product implements its class's bundle; one that drops a defining
// capability fails the build here.
#[cfg(feature = "usdm")]
const _: fn() = || {
    fn bundle<T: polyoxide_venue::PerpetualMarket>() {}
    bundle::<usdm::Usdm>();
};
```

Append after it:

```rust
#[cfg(feature = "spot")]
const _: fn() = || {
    fn bundle<T: polyoxide_venue::SpotMarket>() {}
    bundle::<spot::Spot>();
};
```

The rules file states the pins (AD-21), so it changes in this commit. In `.claude/rules/venue-traits.md`, replace

```markdown
`Classify` assertions: `Perps` (`perps`) and `Usdm` (`usdm`) to `PerpetualMarket`, `Clob`
(`all(clob, gamma)`) and `Predictions` (`predictions`) to `EventContractMarket`.
```

with

```markdown
`Classify` assertions: `Perps` (`perps`) and `Usdm` (`usdm`) to `PerpetualMarket`, `Clob`
(`all(clob, gamma)`) and `Predictions` (`predictions`) to `EventContractMarket`, and `Spot`
(`spot`) to `SpotMarket`.
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all` first: the code above is written by hand and rustfmt may wrap a line differently.

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test venue_spot`
Expected: `test result: ok. 15 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --lib spot::venue`
Expected: `test result: ok. 2 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-binance --all-features`
Expected: every `usdm` and `spot` suite passes, doctests included (`spot::venue` and `spot::key` are `no_run` and run examples).

Then the gates:

Run: `nix develop --command cargo fmt --all -- --check`
Run: `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings`
Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'`
Run: `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps`
Expected: each exits 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/src/spot/venue.rs polyoxide-binance/tests/venue_spot.rs
git commit -m "feat(binance): spot through the market-data traits (binance spot, Story 6)" -- polyoxide-binance/src/spot/venue.rs polyoxide-binance/src/spot/mod.rs polyoxide-binance/src/lib.rs polyoxide-binance/Cargo.toml polyoxide-binance/tests/venue_spot.rs .claude/rules/venue-traits.md
```

### Task 6.3: `live_spot` and its nightly job

**Files:**
- Create: `polyoxide-binance/tests/live_spot.rs`
- Modify: `polyoxide-binance/Cargo.toml` (the `live_spot` `[[test]]` stanza and the live entry)
- Modify (generated, by `gen_registry.py --write`): `CLAUDE.md`, `.github/workflows/nightly-behavioral.yml`, and any other file `git status` shows
- Test: `polyoxide-binance/tests/live_spot.rs`

- [ ] **Step 1: Write the live suite**

Add the stanza to `polyoxide-binance/Cargo.toml`, directly above the existing

```toml
[[test]]
name = "live_usdm"
required-features = ["usdm"]
```

as

```toml
[[test]]
name = "live_spot"
required-features = ["spot"]

```

Create `polyoxide-binance/tests/live_spot.rs`. It fails only through test-support's tags, and each line that must keep an `unwrap` or `expect` ends with its `// live-unwraps:` reason.

```rust
//! Live integration tests against `api.binance.com`.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector:
//! the published spot OpenAPI is stale and lacks fields the wire sends, so
//! nothing else notices a new field. It sees keys, and the length of the
//! positional kline and book rows. It does not see a new value of an enum,
//! which decodes as that enum's `Other` variant.

use polyoxide_test_support::agreement as common;

use std::collections::BTreeSet;
use std::time::Duration;

use polyoxide_binance::spot::{
    self,
    types::{
        AggTrade, AvgPrice, BookTicker, Depth, ExchangeInfo, Interval, Kline, PriceTicker,
        ServerTime, Symbol, SymbolStatus, Ticker24h, Trade,
    },
    Spot, SpotError,
};
use polyoxide_core::ApiError;
use polyoxide_test_support::{environmental, fail, transient, ClassifiedResultExt, ResultExt};
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Instruments, MarketData, Trades, UnixMillis,
};
use serde::{de::DeserializeOwned, Serialize};

fn client() -> Spot {
    Spot::new().or_fail("binance spot client")
}

/// BTCUSDT and, when one is listed and trading, a pair whose symbol has a
/// non-ASCII character, chosen from today's `exchangeInfo` so a delisting
/// cannot break the suite.
async fn symbols(spot: &Spot) -> Vec<Symbol> {
    let info = spot
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo");
    let mut chosen = vec![Symbol::new("BTCUSDT").or_fail("BTCUSDT")];
    if let Some(row) = info
        .symbols
        .iter()
        .find(|row| row.status == SymbolStatus::Trading && !row.symbol.is_ascii())
    {
        chosen.push(Symbol::new(row.symbol.as_str()).or_fail(&row.symbol));
    }
    chosen
}

fn unix_minute() -> u64 {
    UnixMillis::now().0 / 60_000
}

#[tokio::test]
#[ignore]
async fn live_ping_time_and_the_weight_header() {
    let spot = client();
    let latency = spot.health().ping().await.or_fail("ping");
    assert!(latency < Duration::from_secs(10), "latency {latency:?}");

    // Two clients, each with its own budget: the second has charged only its
    // own request, so a count of 2 or more can only be the server's header,
    // which counts the first client's request too. Both counts reset on the
    // minute, so a pair that straddles one is sent again.
    for _ in 0..3 {
        let before = unix_minute();
        let (first, second) = (client(), client());
        let a = first.health().time().send().await.or_fail("time");
        let b = second.health().time().send().await.or_fail("time");
        assert!(b.server_time > 1_790_000_000_000);
        if a.server_time / 60_000 != b.server_time / 60_000 || unix_minute() != before {
            continue;
        }
        let used = second.weight_budget().used();
        assert!(
            used >= 2,
            "X-MBX-USED-WEIGHT-1M was not recorded: the budget counts {used}, and its own charge is 1"
        );
        return;
    }
    // Each pair takes a fraction of a second, so three straddles in a row is a
    // stalled host rather than bad luck, and a retry runs on fresh minutes.
    transient("three pairs of requests each straddled a minute boundary");
}

#[tokio::test]
#[ignore]
async fn live_every_market_route_answers() {
    let spot = client();
    let market = spot.market();
    for symbol in symbols(&spot).await {
        let name = symbol.as_str();
        let ticker = market.ticker_24h(&symbol).send().await.or_fail("ticker/24hr");
        // The record serializes to the wire's own keys.
        let sent = serde_json::to_value(&ticker).expect("a serializable record"); // live-unwraps: serializing a decoded record
        assert_eq!(sent["symbol"], name);
        let avg = market.avg_price(&symbol).send().await.or_fail("avgPrice");
        assert!(avg.price > 0.into(), "{symbol} average {}", avg.price);
        let price = market.price(&symbol).send().await.or_fail("ticker/price");
        assert_eq!(price.symbol, name);
        let book = market
            .book_ticker(&symbol)
            .send()
            .await
            .or_fail("ticker/bookTicker");
        assert_eq!(book.symbol, name);
        let depth = market
            .depth(&symbol)
            .limit(5)
            .send()
            .await
            .or_fail("depth");
        assert!(depth.bids.len() <= 5 && depth.asks.len() <= 5, "{symbol}");
        let trades = market
            .trades(&symbol)
            .limit(2)
            .send()
            .await
            .or_fail("trades");
        assert!(trades.len() <= 2, "{symbol}");
        let agg = market
            .agg_trades(&symbol)
            .limit(2)
            .send()
            .await
            .or_fail("aggTrades");
        assert!(agg.len() <= 2, "{symbol}");
        for interval in [Interval::Minute1, Interval::Second1] {
            let candles = market
                .klines(&symbol, interval)
                .limit(2)
                .send()
                .await
                .or_fail("klines");
            assert!(candles.len() <= 2, "{symbol} {}", interval.as_str());
            if name == "BTCUSDT" {
                assert_eq!(candles.len(), 2, "{}", interval.as_str());
            }
        }
        let info = spot
            .exchange()
            .exchange_info()
            .symbol(&symbol)
            .send()
            .await
            .or_fail("exchangeInfo?symbol=");
        assert!(info.symbols.iter().any(|row| row.symbol == name), "{symbol}");
    }

    // The routes that answer for every pair at once.
    let all = market.tickers_24h().send().await.or_fail("ticker/24hr, all");
    assert!(!all.is_empty());
    let prices = market.prices().send().await.or_fail("ticker/price, all");
    assert!(prices.iter().any(|row| row.symbol == "BTCUSDT"));
    let books = market
        .book_tickers()
        .send()
        .await
        .or_fail("ticker/bookTicker, all");
    assert!(books.iter().any(|row| row.symbol == "BTCUSDT"));
}

#[tokio::test]
#[ignore]
async fn live_an_unknown_symbol_is_venue_error_1121() {
    let err = client()
        .exchange()
        .exchange_info()
        .symbol(&Symbol::new("NOTASYMBOLUSDT").or_fail("NOTASYMBOLUSDT"))
        .send()
        .await
        .unwrap_err(); // live-unwraps: the venue must refuse an unknown symbol

    // Any other error fails by its class, so a 5xx or a dropped connection is
    // retried rather than filed.
    if !matches!(
        err,
        SpotError::Venue {
            status: 400,
            code: -1121,
            ..
        }
    ) {
        fail("expected venue error -1121", &err);
    }
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("https://api.binance.com{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    let status = response.status();
    // A refusal fails as core classifies its status, so its tag says what the
    // nightly does with it: a 451 is environmental, a 429 or a 5xx transient,
    // and a 403 from the firewall or a 418 ban real.
    if !status.is_success() {
        fail(path, &ApiError::from_response(response).await);
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).expect("a JSON body") // live-unwraps: an assertion on the response
}

/// Positional rows of any length but `len`. The key comparison cannot see a
/// value appended to a kline or a book level, since neither has keys.
fn rows_not_of_length(path: &str, rows: &serde_json::Value, len: usize) -> Vec<String> {
    rows.as_array()
        .expect("an array of rows") // live-unwraps: an assertion on the response
        .iter()
        .map(|row| row.as_array().expect("a positional row").len()) // live-unwraps: an assertion on the response
        .filter(|&n| n != len)
        .map(|n| format!("{path}: a row of {n} values, not {len}"))
        .collect()
}

fn agrees<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = common::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may
    // be sent only sometimes. A key the type does not model is.
    if !diff.invented.is_empty() {
        eprintln!(
            "{path}: modelled but not sent this time: {:?}",
            diff.invented
        );
    }
    diff.unmodelled
        .into_iter()
        .map(|key| format!("{path}: {key}"))
        .collect()
}

#[tokio::test]
#[ignore]
async fn live_responses_carry_no_unmodelled_keys() {
    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("a reqwest client"); // live-unwraps: builds a local client
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/api/v3/time";
    check(agrees::<ServerTime>(path, &raw(&http, path).await));
    let path = "/api/v3/exchangeInfo";
    check(agrees::<ExchangeInfo>(path, &raw(&http, path).await));
    let path = "/api/v3/trades?symbol=BTCUSDT&limit=3";
    check(agrees::<Vec<Trade>>(path, &raw(&http, path).await));
    let path = "/api/v3/aggTrades?symbol=BTCUSDT&limit=3";
    check(agrees::<Vec<AggTrade>>(path, &raw(&http, path).await));
    for interval in ["1m", "1s"] {
        let path = format!("/api/v3/klines?symbol=BTCUSDT&interval={interval}&limit=3");
        let text = raw(&http, &path).await;
        check(agrees::<Vec<Kline>>(&path, &text));
        check(rows_not_of_length(&path, &json(&text), 12));
    }
    let path = "/api/v3/avgPrice?symbol=BTCUSDT";
    check(agrees::<AvgPrice>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/24hr?symbol=BTCUSDT";
    check(agrees::<Ticker24h>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/24hr";
    check(agrees::<Vec<Ticker24h>>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/price?symbol=BTCUSDT";
    check(agrees::<PriceTicker>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/price";
    check(agrees::<Vec<PriceTicker>>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/bookTicker?symbol=BTCUSDT";
    check(agrees::<BookTicker>(path, &raw(&http, path).await));
    let path = "/api/v3/ticker/bookTicker";
    check(agrees::<Vec<BookTicker>>(path, &raw(&http, path).await));
    let path = "/api/v3/depth?symbol=BTCUSDT&limit=5";
    let text = raw(&http, path).await;
    check(agrees::<Depth>(path, &text));
    let book = json(&text);
    check(rows_not_of_length(path, &book["bids"], 2));
    check(rows_not_of_length(path, &book["asks"], 2));

    assert!(
        unmodelled.is_empty(),
        "the host sent what the types do not model; model it and record it in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}

/// Every venue method answers for BTCUSDT, and returns what its trait says:
/// the best side first, the oldest row first.
#[tokio::test]
#[ignore]
async fn live_every_venue_method_answers_for_btcusdt() {
    let spot = client();
    let key = spot::key(&Symbol::new("BTCUSDT").or_fail("BTCUSDT"));

    let instrument = ClassifiedResultExt::or_fail(spot.instrument(&key).await, "instrument");
    assert_eq!(instrument.name, "BTCUSDT");
    assert!(instrument.tick_size.is_some() && instrument.min_size.is_some());

    let quote = ClassifiedResultExt::or_fail(spot.quote(&key).await, "quote");
    assert!(
        quote.bid.is_some() && quote.ask.is_some(),
        "BTCUSDT quotes both sides"
    );
    if let (Some(bid), Some(ask)) = (&quote.bid, &quote.ask) {
        assert!(bid.price < ask.price, "{bid:?} crosses {ask:?}");
    }

    let book = ClassifiedResultExt::or_fail(spot.book(&key).await, "book");
    assert!(!book.bids.is_empty() && !book.asks.is_empty());
    assert!(
        book.bids.windows(2).all(|w| w[0].price > w[1].price),
        "bids best first"
    );
    assert!(
        book.asks.windows(2).all(|w| w[0].price < w[1].price),
        "asks best first"
    );

    let trades = ClassifiedResultExt::or_fail(spot.trades(&key).await, "trades");
    assert!(!trades.is_empty());
    assert!(
        trades.windows(2).all(|w| w[0].time <= w[1].time),
        "oldest first"
    );

    let day_ago = UnixMillis(UnixMillis::now().0 - 24 * 60 * 60 * 1000);
    let query = CandleQuery::new(CandleInterval::Hour1, day_ago);
    let candles = ClassifiedResultExt::or_fail(spot.candles(&key, &query).await, "candles");
    assert!(!candles.is_empty());
    assert!(
        candles.windows(2).all(|w| w[0].open_time < w[1].open_time),
        "oldest first"
    );
}

/// The trait's `instruments()` skips a row that does not convert, with a WARN;
/// here a skipped row fails the nightly. `instruments()` reads `exchangeInfo`
/// by itself, so a pair listed or delisted between the test's read and its own
/// makes the two disagree: the test reads the listing once more, and if that
/// one is what `instruments()` returned, the listing moved.
#[tokio::test]
#[ignore]
async fn live_instruments_returns_one_instrument_per_exchange_info_symbol() {
    let spot = client();
    let listed = spot
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo");
    let instruments = ClassifiedResultExt::or_fail(spot.instruments().await, "instruments");

    let have: BTreeSet<&str> = instruments.iter().map(|i| i.name.as_str()).collect();
    let first: BTreeSet<&str> = listed.symbols.iter().map(|r| r.symbol.as_str()).collect();
    if have == first {
        return;
    }
    let again = spot
        .exchange()
        .exchange_info()
        .send()
        .await
        .or_fail("exchangeInfo, again");
    let second: BTreeSet<&str> = again.symbols.iter().map(|r| r.symbol.as_str()).collect();
    if have == second {
        environmental(
            "exchangeInfo changed while the test ran: instruments() read the second listing",
        );
    }
    let missing: Vec<&&str> = second.difference(&have).collect();
    let unlisted: Vec<&&str> = have.difference(&second).collect();
    assert!(
        missing.is_empty() && unlisted.is_empty(),
        "instruments() skipped these exchangeInfo symbols: {missing:?}; \
         it returned these that exchangeInfo does not list: {unlisted:?}"
    );
}
```

- [ ] **Step 2: Run it to see it fail**

The live target is not registered yet, so the registration check fails.

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: it reports that `polyoxide-binance` has a live test target `live_spot` (the `[[test]]` stanza) with no `[package.metadata.polyoxide.live.live_spot]` entry, and exits 1.

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot --no-run`
Expected: it builds. This is the check that the suite names Stories 4 and 5's items exactly; a compile error here is a mismatch at a story boundary, so stop and report it to the supervisor, and do not rename anything.

- [ ] **Step 3: Register the live entry**

In `polyoxide-binance/Cargo.toml`, directly after the `[package.metadata.polyoxide.live.live_usdm_ws]` table:

```toml
[package.metadata.polyoxide.live.live_usdm_ws]
suite = "usdm"
timeout = 15
features = ["usdm-ws"]
secrets = []
```

add:

```toml

[package.metadata.polyoxide.live.live_spot]
suite = "spot"
timeout = 15
features = ["spot"]
secrets = []
```

Run: `nix develop --command python3 scripts/gen_registry.py --write`

It adds the nightly job `live-polyoxide-binance-spot` to `.github/workflows/nightly-behavioral.yml` and its line to the generated list in `CLAUDE.md`. Do not edit either by hand.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all` first.

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: exit 0.

Run: `nix develop --command python3 scripts/live_unwraps.py`
Expected: exit 0; `live_spot.rs` counts zero un-opted sites, and its opted-out lines carry their reasons.

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot`
Expected: `test result: ok. 0 passed; 0 failed; 6 ignored`.

Run: `nix develop --command cargo fmt --all -- --check`
Run: `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings`
Run: `nix develop --command python3 scripts/venue_gate.py`
Expected: each exits 0.

The suite itself reads production and needs no credentials. When the owner has network access:

Run: `nix develop --command cargo test -p polyoxide-binance --no-default-features --features spot --test live_spot -- --ignored`
Expected: `test result: ok. 6 passed`. A failure prints a `polyoxide-class=<tag>` line (`.github/scripts/classify_failures.py` reads it): `environmental` and `transient` are the world's, a `real` one is a type that does not model what the host sends (record it in `docs/specs/binance/OBSERVED.md` above its spot anchor, which is Story 1's file) or a venue method that skips a row.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/live_spot.rs
git commit -m "test(binance): live_spot, spot's drift detector and nightly job (binance spot, Story 6)" -- polyoxide-binance/tests/live_spot.rs polyoxide-binance/Cargo.toml CLAUDE.md README.md SELF-HEALING.md docs/specs/INDEX.md docs/ARCHITECTURE.md .github/workflows/nightly-behavioral.yml .github/workflows/nightly-schema.yml
```

### Task 6.4: The umbrella's `binance-spot`

**Files:**
- Modify: `polyoxide/Cargo.toml`
- Modify: `polyoxide/src/lib.rs`
- Modify: `polyoxide/tests/parse_key.rs` (append only)
- Modify: `polyoxide/tests/live_venue.rs` (append only)
- Modify: `.github/scripts/tests/test_dep_allowlists.py`
- Create: `ci/dep-allowlists/polyoxide@binance-spot.txt` (by `--write`)
- Modify: `.claude/rules/ci-gates.md` (the features job's header pins, ~line 33, and the `polyoxide` deny-list sentence, ~lines 86-87)
- Modify: `.github/workflows/ci.yml` (the two header-pin steps of the `features` job, ~lines 131 and 147)
- Modify (generated, by `gen_registry.py --write`): `CLAUDE.md`, `.github/workflows/nightly-behavioral.yml`, and any other file `git status` shows
- Test: `polyoxide/tests/parse_key.rs`, `.github/scripts/tests/test_dep_allowlists.py`

- [ ] **Step 1: Write the failing tests**

Append to `polyoxide/tests/parse_key.rs`, after its last test (`each_key_carries_the_class_its_product_declares`). The `use` lines sit inside each function so no existing line changes.

```rust

#[test]
fn binance_spots_keys_come_from_its_own_constructors() {
    use polyoxide::binance::spot::{self, types::Symbol};

    let btc = parse("binance.spot:btcusdt").unwrap();
    assert_eq!(btc, spot::key(&Symbol::new("BTCUSDT").unwrap()));
    assert_eq!(btc.to_string(), "binance.spot:BTCUSDT");
    // The same symbol on USDⓈ-M is another product's key.
    assert_ne!(btc, parse("binance.usdm:btcusdt").unwrap());
}

#[test]
fn binance_spot_refuses_an_id_it_cannot_read_and_leaves_its_neighbours_unknown() {
    assert!(matches!(
        parse("binance.spot:BTC/USDT"),
        Err(KeyError::InvalidId { .. })
    ));
    let parsed = parse("binance.margin:BTCUSDT");
    assert!(
        matches!(parsed, Err(KeyError::UnknownProduct { .. })),
        "{parsed:?}"
    );
}

#[test]
fn a_spot_key_carries_the_spot_class_and_a_usdm_key_does_not() {
    use polyoxide::binance::spot::key::SpotKeys;
    use polyoxide::binance::usdm::key::UsdmKeys;
    use polyoxide::venue::{KeySpace, ProductClass};

    let spot = parse("binance.spot:btcusdt").unwrap();
    assert_eq!(spot.class(), SpotKeys::CLASS);
    assert_eq!(spot.class(), ProductClass::Spot);
    let usdm = parse("binance.usdm:BTCUSDT").unwrap();
    assert_eq!(usdm.class(), UsdmKeys::CLASS);
    assert_ne!(spot.class(), usdm.class());
}
```

Append to `polyoxide/tests/live_venue.rs`, after its last test (`best_bid_ask_reads_kalshi_predictions`):

```rust

#[tokio::test]
#[ignore = "hits the live hosts"]
async fn best_bid_ask_reads_binance_spot() {
    use polyoxide::binance::spot::{self, types::Symbol, Spot};

    let venue = DynMarketData::new_arc(Spot::new().or_fail("build the spot client"));
    let key = spot::key(&Symbol::new("BTCUSDT").or_fail("name BTCUSDT"));
    let (bid, ask) = read(venue, &key).await;
    assert!(
        bid.is_some() && ask.is_some(),
        "BTCUSDT always has both sides"
    );
}
```

In `.github/scripts/tests/test_dep_allowlists.py`, make these edits.

(a) The umbrella's HTTP rule admits the new module. Replace

```python
                        "polymarket-relay", "polymarket-perps", "binance-usdm",
                        "kalshi-predictions"}),
```

with

```python
                        "polymarket-relay", "polymarket-perps", "binance-usdm",
                        "binance-spot", "kalshi-predictions"}),
```

(b) The file must hold `reqwest`. Replace the line

```python
    "polyoxide@kalshi-predictions-ws.txt": "polyoxide-ws",
```

with

```python
    "polyoxide@kalshi-predictions-ws.txt": "polyoxide-ws",
    "polyoxide@binance-spot.txt": "reqwest",
```

(c) The existence check names it. Replace

```python
    umbrella = ("polymarket-rtds", "polymarket-sports", "polymarket-perps", "polymarket-clob",
                "polymarket-clob,keychain")
```

with

```python
    umbrella = ("polymarket-rtds", "polymarket-sports", "polymarket-perps", "polymarket-clob",
                "polymarket-clob,keychain", "binance-spot")
```

(d) The HTTP-module loop covers it. Replace

```python
                     ("polymarket-perps-ws",), ("binance-usdm",), ("binance-usdm-ws",),
                     ("binance",), ("polymarket-rtds", "binance-usdm")):
```

with

```python
                     ("polymarket-perps-ws",), ("binance-usdm",), ("binance-usdm-ws",),
                     ("binance-spot",), ("binance",), ("polymarket-rtds", "binance-usdm")):
```

(e) A new test after the last of the umbrella's Kalshi tests. Replace the line

```python
    assert [n for n in SOCKET_STACK if n not in stream] == [], stream
```

with

```python
    assert [n for n in SOCKET_STACK if n not in stream] == [], stream


def test_the_umbrellas_binance_spot_feature_is_http_only_and_part_of_binance_and_full() -> None:
    table = feature_table(UMBRELLA)
    # The HTTP stack is admitted, the signing stack never, and no socket stack
    # comes with spot's REST module.
    assert denied(UMBRELLA, HTTP_STACK, ("binance-spot",), table) == []
    assert denied(UMBRELLA, ["alloy", "hmac"], ("binance-spot",), table) == [
        f"alloy ({UMBRELLA_SIGNING})", f"hmac ({UMBRELLA_SIGNING})"]
    listed = read_allowlist(ALLOWLISTS / "polyoxide@binance-spot.txt")
    assert "polyoxide-binance" in listed, listed
    assert [n for n in SOCKET_STACK if n in listed] == [], listed
    # The real table: `binance` and `full` turn spot on, and spot turns on no stream.
    assert "binance-spot" in enabled(("binance",), table)
    assert "binance-spot" in enabled(("full",), table)
    assert "binance-usdm-ws" not in enabled(("binance-spot",), table)
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'`
Expected: failures that name the missing file: `test_each_per_feature_rule_admits_what_it_guards[polyoxide@binance-spot.txt]` and `test_the_allowlists_exist` (`polyoxide@binance-spot.txt` is not in the directory), and `test_the_umbrellas_binance_spot_feature_is_http_only_and_part_of_binance_and_full` (no such feature in the table).

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,binance-spot,kalshi-predictions --test parse_key`
Expected: cargo refuses before compiling: `the package 'polyoxide' does not contain this feature: binance-spot`.

- [ ] **Step 3: Implement**

`polyoxide/Cargo.toml`:

- Replace `binance = ["binance-usdm"]` with `binance = ["binance-usdm", "binance-spot"]`.
- After the line `binance-usdm-ws = ["binance-usdm", "polyoxide-binance/usdm-ws"]` add:

```toml
binance-spot = ["dep:polyoxide-binance", "polyoxide-binance/spot", "polyoxide-venue/market"]
```

- In `full`, replace

```toml
    "binance-usdm-ws",
    "kalshi",
```

with

```toml
    "binance-usdm-ws",
    "binance-spot",
    "kalshi",
```

- In the package description, replace

```toml
description = "Multi-venue Rust toolkit: Polymarket (CLOB, Gamma, Data, Relay, Perps, RTDS, Sports), Binance USDⓈ-M futures and Kalshi's event-contract market data, each venue behind features named for it"
```

with

```toml
description = "Multi-venue Rust toolkit: Polymarket (CLOB, Gamma, Data, Relay, Perps, RTDS, Sports), Binance USDⓈ-M futures and spot and Kalshi's event-contract market data, each venue behind features named for it"
```

- The suites that use spot require it. Replace the `live_venue` and `parse_key` stanzas' `required-features`. The current stanzas are

```toml
[[test]]
name = "live_venue"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
```

and

```toml
[[test]]
name = "parse_key"
required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]
```

and each becomes the same two lines with `required-features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "binance-spot", "kalshi-predictions"]`. The `venue_neutral` example and test stanzas stay as they are.

- In the live entry, replace the one line `features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "kalshi-predictions"]` (it sits under `[package.metadata.polyoxide.live.live_venue]`) with `features = ["polymarket-clob", "polymarket-perps", "binance-usdm", "binance-spot", "kalshi-predictions"]`.

`polyoxide/src/lib.rs`, edits in file order:

1. The crate docs. Replace

```rust
//!   on. With any module that implements them (`polymarket-clob`,
//!   `polymarket-perps`, `binance-usdm`, `kalshi-predictions`) it also holds
```

with

```rust
//!   on. With any module that implements them (`polymarket-clob`,
//!   `polymarket-perps`, `binance-usdm`, `binance-spot`, `kalshi-predictions`) it also holds
```

and replace

```rust
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm`), `binance-usdm` or `binance-usdm-ws`.
```

with

```rust
//! - `polyoxide::binance` is `polyoxide-binance`, under `binance` (its
//!   defaults: `usdm` and `spot`), `binance-usdm`, `binance-usdm-ws` or
//!   `binance-spot`.
```

2. The re-export. Replace

```rust
#[cfg(any(
    feature = "binance",
    feature = "binance-usdm",
    feature = "binance-usdm-ws"
))]
pub use polyoxide_binance as binance;
```

with

```rust
#[cfg(any(
    feature = "binance",
    feature = "binance-usdm",
    feature = "binance-usdm-ws",
    feature = "binance-spot"
))]
pub use polyoxide_binance as binance;
```

3. `parse_key`'s docs. Replace

```rust
/// Binance's `usdm` (`binance-usdm`), and Kalshi's `predictions`
/// (`kalshi-predictions`).
```

with

```rust
/// Binance's `usdm` (`binance-usdm`) and `spot` (`binance-spot`), and Kalshi's
/// `predictions` (`kalshi-predictions`).
```

and, after the USDⓈ-M doctest, whose last lines are

```rust
/// # #[cfg(not(feature = "binance-usdm"))]
/// # fn main() {}
/// ```
```

add a doctest for spot directly below that closing fence:

````rust
///
/// ```
/// # #[cfg(feature = "binance-spot")]
/// # fn main() -> Result<(), polyoxide::venue::KeyError> {
/// use polyoxide::venue::RawKey;
///
/// let btc = polyoxide::parse_key(&RawKey::parse("binance.spot:btcusdt")?)?;
/// assert_eq!(btc.to_string(), "binance.spot:BTCUSDT");
/// # Ok(())
/// # }
/// # #[cfg(not(feature = "binance-spot"))]
/// # fn main() {}
/// ```
````

4. `parse_key`'s `cfg` and body. Replace

```rust
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "kalshi-predictions"
))]
pub fn parse_key(raw: &venue::RawKey) -> Result<venue::MarketKey, venue::KeyError> {
```

with

```rust
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "binance-spot",
    feature = "kalshi-predictions"
))]
pub fn parse_key(raw: &venue::RawKey) -> Result<venue::MarketKey, venue::KeyError> {
```

and replace

```rust
        return binance::usdm::key::parse(raw);
    }
```

with

```rust
        return binance::usdm::key::parse(raw);
    }
    #[cfg(feature = "binance-spot")]
    if raw.venue() == &binance::spot::key::VENUE && raw.product() == &binance::spot::key::PRODUCT {
        return binance::spot::key::parse(raw);
    }
```

5. The prelude. After the block whose last line is

```rust
        Event as UsdmEvent, StreamName, UsdmWs, UsdmWsBuilder, UsdmWsError,
    };
```

add, separated by a blank line:

```rust

    // Aliased: usdm's `WeightBudget` keeps the bare name.
    #[cfg(feature = "binance-spot")]
    pub use polyoxide_binance::spot::{
        Spot, SpotBuilder, SpotError, WeightBudget as SpotWeightBudget,
    };
```

6. The passthrough. After

```rust
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::binance::usdm::ws::UsdmWs) = drop;
```

add:

```rust
#[cfg(feature = "binance-spot")]
const _: fn(crate::binance::spot::Spot) = drop;
```

7. The prelude pin. After

```rust
#[cfg(feature = "binance-usdm-ws")]
const _: fn(crate::prelude::UsdmWs) = drop;
```

add:

```rust
#[cfg(feature = "binance-spot")]
const _: fn(crate::prelude::Spot) = drop;
```

8. The trait pins. Replace

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "kalshi-predictions"
))]
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
```

with

```rust
#[cfg(any(
    feature = "polymarket-clob",
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "binance-spot",
    feature = "kalshi-predictions"
))]
const _: fn(&crate::venue::DynMarketData<'static>) = |_| {};
// Spot is a `SpotMarket`, a bundle that is at the venue crate's root.
#[cfg(feature = "binance-spot")]
const _: fn() = || {
    fn bundle<T: crate::venue::SpotMarket>() {}
    bundle::<crate::binance::spot::Spot>();
};
```

and replace

```rust
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "kalshi-predictions"
))]
const _: fn(&crate::venue::RawKey) -> Result<crate::venue::MarketKey, crate::venue::KeyError> =
    crate::parse_key;
```

with

```rust
    feature = "polymarket-perps",
    feature = "binance-usdm",
    feature = "binance-spot",
    feature = "kalshi-predictions"
))]
const _: fn(&crate::venue::RawKey) -> Result<crate::venue::MarketKey, crate::venue::KeyError> =
    crate::parse_key;
```

The umbrella's dependency allowlist for the new feature. `--write` rewrites existing files only, so seed the file under its name from a sibling, then let `--write` replace its contents with `cargo tree`'s (the seed's text does not survive):

Run: `cp 'ci/dep-allowlists/polyoxide@kalshi-predictions.txt' 'ci/dep-allowlists/polyoxide@binance-spot.txt'`
Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`

Check `ci/dep-allowlists/polyoxide@binance-spot.txt`: its header must read `--no-default-features ... --features binance-spot`, and the list must hold `polyoxide-binance` and `reqwest` and none of `polyoxide-ws`, `tokio-tungstenite`, `tungstenite`. `git diff --stat ci/dep-allowlists` must show no other allowlist changed; if one did, a feature of Stories 4 or 5 changed a tree and the supervisor decides whether that is expected.

The features job pins every registered HTTP module's headers in two steps of `.github/workflows/ci.yml`, and `.github/scripts/tests/test_ci_workflow.py` derives the commands from cargo metadata and compares them with the steps; registering `spot` (Task 6.1) and turning it on under the umbrella's `full` (this task) make it a module both steps must name, so this commit adds its two lines. In the `Header pins, minimal builds` step, replace

```yaml
          cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm
```

with

```yaml
          cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm
          cargo test -p polyoxide-binance --no-default-features --features spot --test headers_spot
```

and in the `Header pins, umbrella full build` step, replace

```yaml
          cargo test -p polyoxide -p polyoxide-binance --features polyoxide/full --test headers_usdm
```

with

```yaml
          cargo test -p polyoxide -p polyoxide-binance --features polyoxide/full --test headers_usdm
          cargo test -p polyoxide -p polyoxide-binance --features polyoxide/full --test headers_spot
```

(If the first Edit of a file under `.github/workflows/` is refused by the environment's security hook, repeat it unchanged, then check with `git diff .github/workflows/ci.yml`.)

The rules file states both lists (AD-21), so it changes in the same commit. In `.claude/rules/ci-gates.md`, replace

```markdown
   for clob, gamma, data, relay and perps; `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm`;
   `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`.
```

with

```markdown
   for clob, gamma, data, relay and perps; `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm`
   and `cargo test -p polyoxide-binance --no-default-features --features spot --test headers_spot`;
   `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`.
```

and replace

```markdown
  - `polyoxide`, per feature on its own names: the HTTP stack only with an HTTP module of any
    venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps`, `binance-usdm` or
    `kalshi-predictions`); `alloy*` and `hmac` only with `polymarket-clob` or `polymarket-relay`;
```

with

```markdown
  - `polyoxide`, per feature on its own names: the HTTP stack only with an HTTP module of any
    venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps`, `binance-usdm`,
    `binance-spot` or `kalshi-predictions`); `alloy*` and `hmac` only with `polymarket-clob` or
    `polymarket-relay`;
```

Regenerate what the umbrella's live entry feeds:

Run: `nix develop --command python3 scripts/gen_registry.py --write`

It rewrites the `live-polyoxide-venue` line of `CLAUDE.md` and its job in `nightly-behavioral.yml`, and nothing by hand.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all` first.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'`
Expected: all pass, the new test included.

Run: `nix develop --command cargo test -p polyoxide --no-default-features --features polymarket-clob,polymarket-perps,binance-usdm,binance-spot,kalshi-predictions --test parse_key`
Expected: `test result: ok. 9 passed; 0 failed` (the six existing tests and the three new ones).

Run: `nix develop --command cargo test -p polyoxide --all-features`
Expected: every suite and doctest passes (`live_venue` is `ignored`).

Run: `nix develop --command cargo hack check -p polyoxide --each-feature --no-dev-deps`
Expected: exit 0, `binance-spot` alone included: the re-export, `parse_key`, the prelude and the pins compile with that feature alone.

Then the gates:

Run: `nix develop --command cargo fmt --all -- --check`
Run: `nix develop --command cargo clippy -p polyoxide -p polyoxide-binance --all-targets --all-features -- -D warnings`
Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide -p polyoxide-binance --all-features'`
Run: `nix develop --command python3 scripts/gen_registry.py --check`
Run: `nix develop --command python3 scripts/venue_gate.py`
Run: `nix develop --command python3 scripts/live_unwraps.py`
Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`
Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests -q'`
Expected: each exits 0. `test_body_diff.py` finds no changed test: the Task 6.4 tests are appended, and the umbrella's `[[test]]` stanzas did not exist at the baseline.

- [ ] **Step 5: Commit**

The allowlist is regenerated because the umbrella gained a feature, `binance-spot`, whose tree (the HTTP stack and `polyoxide-binance` without a socket) had no file.

Add `Cargo.lock` to the pathspec if `git status` shows cargo changed it.

```bash
git add -N ci/dep-allowlists/polyoxide@binance-spot.txt
git commit -m "feat(polyoxide): the umbrella's binance-spot, parse_key and prelude; allowlist added for the new feature (binance spot, Story 6)" -- polyoxide/Cargo.toml polyoxide/src/lib.rs polyoxide/tests/parse_key.rs polyoxide/tests/live_venue.rs .github/scripts/tests/test_dep_allowlists.py ci/dep-allowlists/polyoxide@binance-spot.txt .claude/rules/ci-gates.md .github/workflows/ci.yml CLAUDE.md README.md SELF-HEALING.md docs/specs/INDEX.md docs/ARCHITECTURE.md .github/workflows/nightly-behavioral.yml .github/workflows/nightly-schema.yml
```

## Story 7: Docs

- **Developer:** junior-dev
- **Files:** `docs/specs/binance/INDEX.md`, `polyoxide-binance/README.md`, `polyoxide-binance/src/lib.rs`, `polyoxide/README.md`, `README.md`, `.claude/rules/binance.md`, `.claude/rules/umbrella.md`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/live-tests.md`, `.claude/rules/testing.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`, `.claude/rules/wire-enums.md`, `.claude/rules/specs.md`, `Cargo.toml`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md`, `CLAUDE.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `docs/ARCHITECTURE.md`, `.github/workflows/nightly-schema.yml`
- **Interfaces:** uses every earlier story's names; exposes none
- **Verify:** `cargo test --doc -p polyoxide-binance --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_module_readmes.py -q' && bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide --all-features'`
- **Review:** light (docs and records; no library code)

This story writes the hand-written text that says spot exists: the Binance spec index (a spot section with the routes table and the drift detectors), the crate README and `src/lib.rs`'s crate docs, the umbrella README and the root README's hand-written lines, the hand-written lines of `CLAUDE.md`, `SELF-HEALING.md` and `docs/specs/INDEX.md`, the two rules files `.claude/rules/binance.md` (a `shared` section and a `spot` section, and the capture script in its `paths:`) and `.claude/rules/umbrella.md`, and the sentences in eight other rules files that list Binance's `usdm` counterparts (`rate-limits.md`, `send-loop.md`, `live-tests.md`, `testing.md`, `errors.md`, `clients.md`, `wire-enums.md`, `specs.md`), which gain spot's. It updates `[workspace.metadata.polyoxide.mirrors.binance]` in the root manifest and runs `python3 scripts/gen_registry.py --write`, which rewrites the generated regions that feed on it, and it writes the spine amendment that records `ProductClass::Spot` and `SpotMarket`. No story before it changes a hand-written line of those files (Story 6 only ran the generator). No code changes, no task touches a generated region by hand, and none of the text holds a landmine marker. The gates that bite are `gen_registry.py --check`, the rules-file test, the module-README test and the doc build.

### Task 7.1: The mirror's metadata and the regenerated regions

**Files:**
- Modify: `Cargo.toml:265-271`
- Modify (by `python3 scripts/gen_registry.py --write`, never by hand): `CLAUDE.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `.github/workflows/nightly-schema.yml`, `docs/ARCHITECTURE.md` if the generator rewrites it, and any other file the generator rewrites

- [ ] **Step 1: Write the failing check**

Nothing in the four files says spot's swagger is stale until the manifest does. The check is a count of the phrase the new `exclude` carries, plus the new mirror name in the spec index:

```bash
grep -c 'spot_api.yaml' CLAUDE.md SELF-HEALING.md docs/specs/INDEX.md .github/workflows/nightly-schema.yml
grep -c 'Binance USDⓈ-M and spot' docs/specs/INDEX.md
```

- [ ] **Step 2: Run it to see it fail**

Run the two commands above. Expected: every count is `0` (`CLAUDE.md:0`, `SELF-HEALING.md:0`, `docs/specs/INDEX.md:0`, `.github/workflows/nightly-schema.yml:0`, then `0`), and each `grep` exits `1`.

- [ ] **Step 3: Implement**

In `Cargo.toml`, replace the whole `binance` mirror table. The current text is:

```toml
[workspace.metadata.polyoxide.mirrors.binance]
name = "Binance USDⓈ-M"
section = "other-venue"
base_urls = ["https://fapi.binance.com", "wss://fstream.binance.com"]
description = "Futures public market data and market streams. No published spec, so not a mirror"
crate_note = "streams behind `usdm-ws`"
exclude = "Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites in `polyoxide-binance` are the drift check."
```

The new text (`section` and `crate_note` stay as they are):

```toml
[workspace.metadata.polyoxide.mirrors.binance]
name = "Binance USDⓈ-M and spot"
section = "other-venue"
base_urls = ["https://fapi.binance.com", "https://api.binance.com", "wss://fstream.binance.com"]
description = "Futures and spot public market data, and the futures market streams. No published spec, so not a mirror"
crate_note = "streams behind `usdm-ws`"
exclude = "Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; spot's `spot_api.yaml` (in `binance-api-swagger`) was last released on 2024-10-08 and lacks fields the wire sends today, so a diff of it would never fire. The live suites in `polyoxide-binance` are the drift check."
```

Then regenerate:

```bash
python3 scripts/gen_registry.py --write
git status --short
```

`git status --short` must list `Cargo.toml` and only the files that hold a generated region fed by the mirror table: `CLAUDE.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md` and `.github/workflows/nightly-schema.yml`. If it lists another file (`docs/ARCHITECTURE.md` is the likely one), the generator rewrote it from this change; keep it and add it to the commit in Step 5. Do not edit any region by hand.

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'spot_api.yaml' CLAUDE.md SELF-HEALING.md docs/specs/INDEX.md .github/workflows/nightly-schema.yml
grep -c 'Binance USDⓈ-M and spot' docs/specs/INDEX.md
python3 scripts/gen_registry.py --check
```

Expected: each of the four files has a count of `1` (the schema-exclusion entry), `docs/specs/INDEX.md` shows `1` for the name, and `--check` exits `0` with no output about a differing region.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(binance): the spot swagger is stale, so spot's drift check is the live suites (binance spot, Story 7)" -- Cargo.toml CLAUDE.md SELF-HEALING.md docs/specs/INDEX.md .github/workflows/nightly-schema.yml
```

### Task 7.2: The Binance spec index

**Files:**
- Modify: `docs/specs/binance/INDEX.md` (the whole file is rewritten below; the USDⓈ-M text is kept word for word)

- [ ] **Step 1: Write the failing check**

```bash
grep -c 'api/v3/depth' docs/specs/binance/INDEX.md
grep -c 'live_spot.rs' docs/specs/binance/INDEX.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: `0` and `0`, each `grep` exiting `1`.

- [ ] **Step 3: Implement**

Replace the whole content of `docs/specs/binance/INDEX.md` with the text below. The weights in the spot table are `spot::weight::Route::cost`'s; `depth` above 1000 levels is documented, not measured. The shared-counter and minute-window findings live in `OBSERVED.md`'s `## Spot` section, which this file links and does not repeat.

````markdown
# Binance USDⓈ-M futures and spot

Binance is not a Polymarket host. `polyoxide-binance`'s `usdm` module reads its USDⓈ-M
futures public market data, and its `spot` module reads its spot public market data, for
consumers that trade both venues. The `polyoxide` umbrella re-exports them as
`polyoxide::binance`, under its `binance-usdm`, `binance-usdm-ws` and `binance-spot` features
and `full`.

| Surface | Host | Crate |
|---|---|---|
| USDⓈ-M REST, public market data | `https://fapi.binance.com` | `polyoxide-binance` with the `usdm` feature (`usdm::Usdm`) |
| USDⓈ-M market streams | `wss://fstream.binance.com/{market,public}/stream` | `polyoxide-binance` with the `usdm-ws` feature (`usdm::ws::UsdmWs`, `usdm::ws::UsdmWsBuilder`) |
| Spot REST, public market data | `https://api.binance.com` | `polyoxide-binance` with the `spot` feature (`spot::Spot`) |

Spot has no streams yet.

**This directory is not a mirror.** Binance publishes no OpenAPI or AsyncAPI document for
USDⓈ-M futures (`github.com/binance/binance-api-swagger` holds `spot_api.yaml` only), so
there is nothing to vendor and nothing for `nightly-schema.yml` to diff. Spot's
`spot_api.yaml` was last released on 2024-10-08 (v1.22.0) and lacks fields the wire sends
today (`amendAllowed`, `opoAllowed`, `pegInstructionsAllowed`), so a diff of it would never
fire either. The sources are the prose pages and the wire:

- USDⓈ-M REST pages: `https://developers.binance.com/docs/derivatives/usds-margined-futures/`
- USDⓈ-M stream pages: `https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/`
  (`market` and `public`). The old stream URLs under the REST prefix land on a generic
  page as of 2026-10-07.
- Spot pages: `https://github.com/binance/binance-spot-api-docs`.

Where the pages and the wire disagree, the wire wins and [OBSERVED.md](OBSERVED.md)
records it. The drift detectors are the live suites:
`polyoxide-binance/tests/live_usdm.rs::live_responses_carry_no_unmodelled_keys`,
`live_usdm_ws.rs::live_frames_carry_no_unmodelled_keys` and
`live_spot.rs::live_responses_carry_no_unmodelled_keys` fail on any key the types do not model.
A new enum value or filter type decodes as `Other` and is not seen, and a changed weight
goes unseen until `weight_probe` (USDⓈ-M) or `weight_probe_spot` (spot) is run by hand.

## USDⓈ-M routes covered

| Route | Method | Weight |
|---|---|---|
| `/fapi/v1/ping` | `usdm.health().ping()` | 1 |
| `/fapi/v1/time` | `usdm.health().time()` | 1 |
| `/fapi/v1/exchangeInfo` | `usdm.exchange().exchange_info()` | 1 |
| `/fapi/v1/fundingInfo` | `usdm.exchange().funding_info()` | funding limit, 500 per 5 minutes |
| `/fapi/v1/ticker/24hr` | `usdm.market().ticker_24h(&s)` / `tickers_24h()` | 1 / 40 |
| `/fapi/v1/premiumIndex` | `usdm.market().premium_index(&s)` / `premium_indices()` | 1 / 10 |
| `/fapi/v1/klines` | `usdm.market().klines(&s, interval)` | 1 to 10 by `limit`, 5 without |
| `/fapi/v1/fundingRate` | `usdm.market().funding_rate()` | funding limit, 500 per 5 minutes |
| `/fapi/v1/openInterest` | `usdm.market().open_interest(&s)` | 1 |
| `/fapi/v1/aggTrades` | `usdm.market().agg_trades(&s)` | 20 |
| `/fapi/v1/depth` | `usdm.market().depth(&s)` | 1 without `limit`, 2 to 20 with |

The weight table is `Route::cost` in `polyoxide-binance/src/usdm/weight.rs`, pinned by its
`documented_weights` test and re-measured live by
`cargo run -p polyoxide-binance --example weight_probe`.

## Spot routes covered

| Route | Method | Weight |
|---|---|---|
| `/api/v3/ping` | `spot.health().ping()` | 1 |
| `/api/v3/time` | `spot.health().time()` | 1 |
| `/api/v3/exchangeInfo` | `spot.exchange().exchange_info()`, with `.symbol(&s)` or without | 20 either way |
| `/api/v3/depth` | `spot.market().depth(&s)` | 5 without `limit` and for 1 to 100; 25 for 101 to 500; 50 for 501 to 1000; 250 above 1000 (documented, not measured) |
| `/api/v3/trades` | `spot.market().trades(&s)` | 25, with or without `limit` |
| `/api/v3/aggTrades` | `spot.market().agg_trades(&s)` | 4, with or without `limit` |
| `/api/v3/klines` | `spot.market().klines(&s, interval)` | 2, with or without `limit` |
| `/api/v3/avgPrice` | `spot.market().avg_price(&s)` | 2 |
| `/api/v3/ticker/24hr` | `spot.market().ticker_24h(&s)` / `tickers_24h()` | 2 / 80 |
| `/api/v3/ticker/price` | `spot.market().price(&s)` / `prices()` | 2 / 4 |
| `/api/v3/ticker/bookTicker` | `spot.market().book_ticker(&s)` / `book_tickers()` | 2 / 4 |

The weight table is `Route::cost` in `polyoxide-binance/src/spot/weight.rs`, pinned by its
`documented_weights` test and re-measured live by the `weight_probe_spot` example
(`cargo run -p polyoxide-binance --example weight_probe_spot`, which takes no `--product`
argument; `weight_probe` measures USDⓈ-M only). A `limit` past a route's maximum is truncated
by the venue, not refused, and the cost does not change (`depth` is priced by its band). `api.binance.com` and
`data-api.binance.vision` and the weight window are covered in [OBSERVED.md](OBSERVED.md)'s
`## Spot` section.

## USDⓈ-M streams covered

| Stream | `StreamName` | Path | `Payload` |
|---|---|---|---|
| `!ticker@arr` | `AllTickers` | market | `Tickers(Vec<TickerEvent>)` |
| `!markPrice@arr@1s` | `AllMarkPrices` | market | `MarkPrices(Vec<MarkPriceEvent>)` |
| `<s>@aggTrade` | `AggTrade(symbol)` | market | `AggTrade(AggTradeEvent)` |
| `<s>@kline_<interval>` | `Kline(symbol, interval)` | market | `Kline(KlineEvent)` |
| `<s>@markPrice@1s` | `MarkPrice(symbol)` | market | `MarkPrice(MarkPriceEvent)` |
| `<s>@ticker` | `Ticker(symbol)` | market | `Ticker(TickerEvent)` |
| `<s>@depth<5\|10\|20>`, then `@100ms`, `@500ms` or nothing (250 ms) | `PartialDepth(symbol, levels, speed)` | public | `PartialDepth(PartialDepthEvent)` |
| `<s>@bookTicker` | `BookTicker(symbol)` | public | `BookTicker(BookTickerEvent)` |

## Fixtures and probes

- `polyoxide-binance/tests/fixtures/usdm/rest/`: refreshed by
  `uv run scripts/capture_binance_usdm.py polyoxide-binance/tests/fixtures/usdm`.
- `polyoxide-binance/tests/fixtures/usdm/ws/`: stream envelopes, refreshed by the same script
  and compiled into `polyoxide_binance::usdm::ws::fixtures` under `usdm-ws` and `test-server`.
- `polyoxide-binance/tests/fixtures/spot/rest/`: refreshed by
  `python3 scripts/capture_binance_spot.py rest`, with each file's request and trimming in
  `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`.
- `probes/`: the stdlib scripts behind most of the design spec's measurements. Its
  `capture.py` and `capture_ws.py` are superseded by `scripts/capture_binance_usdm.py`.
  `probes/probe_rest.py --product {usdm,spot}` measures the REST weights and limits of either
  product (`usdm` is the default).
````

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'api/v3/depth' docs/specs/binance/INDEX.md
grep -c 'live_spot.rs' docs/specs/binance/INDEX.md
python3 scripts/gen_registry.py --check
```

Expected: `1`, `1`, and `--check` exits `0` (this file holds no generated region; the check proves the page the generated index links to is still the one it names).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(binance): spot's routes, drift detectors and fixtures in the Binance spec index (binance spot, Story 7)" -- docs/specs/binance/INDEX.md
```

### Task 7.3: The crate README and the crate docs

**Files:**
- Modify: `polyoxide-binance/README.md:1-18`
- Modify: `polyoxide-binance/src/lib.rs:1-8`

- [ ] **Step 1: Write the failing check**

```bash
grep -c '`spot`' polyoxide-binance/README.md
grep -c '`spot`' polyoxide-binance/src/lib.rs
```

- [ ] **Step 2: Run it to see it fail**

Expected: `0` and `0`.

- [ ] **Step 3: Implement**

In `polyoxide-binance/README.md`, replace the opening paragraph and the module table. Current text:

```markdown
Rust client library for Binance's APIs, one feature per module: `usdm`. Part of the
[polyoxide](https://crates.io/crates/polyoxide) workspace, whose umbrella crate, `polyoxide`,
re-exports it as `polyoxide::binance` under its `binance`, `binance-usdm` and `binance-usdm-ws`
features.

Each module has its own README:

| Module | Feature | API |
|---|---|---|
| [`usdm`](src/usdm/README.md) | `usdm`; its market streams, `usdm::ws`, under `usdm-ws` | USDⓈ-M futures public market data on `fapi.binance.com`, and its market streams on `fstream.binance.com`; read-only, no credentials |

`usdm` is on by default, and `usdm-ws` turns it on. `test-server` exposes the captured stream
frames, `usdm::ws::fixtures`, to integration tests with `usdm-ws`; it is not for consumers.
```

New text:

```markdown
Rust client library for Binance's APIs, one feature per module: `usdm` and `spot`. Part of the
[polyoxide](https://crates.io/crates/polyoxide) workspace, whose umbrella crate, `polyoxide`,
re-exports it as `polyoxide::binance` under its `binance`, `binance-usdm`, `binance-usdm-ws` and
`binance-spot` features.

Each module has its own README:

| Module | Feature | API |
|---|---|---|
| [`usdm`](src/usdm/README.md) | `usdm`; its market streams, `usdm::ws`, under `usdm-ws` | USDⓈ-M futures public market data on `fapi.binance.com`, and its market streams on `fstream.binance.com`; read-only, no credentials |
| [`spot`](src/spot/README.md) | `spot` | Spot public market data on `api.binance.com`; read-only, no credentials |

`usdm` and `spot` are on by default, and `usdm-ws` turns `usdm` on. `test-server` exposes the
captured stream frames, `usdm::ws::fixtures`, to integration tests with `usdm-ws`; it is not for
consumers.
```

In `polyoxide-binance/src/lib.rs`, replace the crate docs. Current text:

```rust
//! # polyoxide-binance
//!
//! Rust client library for Binance's APIs, one feature per module: `usdm`.
//!
//! `usdm` (feature `usdm`, on by default) reads USDⓈ-M futures public market
//! data (`fapi.binance.com`), and with `usdm-ws` its market streams
//! (`fstream.binance.com`). No credentials are needed. The module's own docs
//! carry its example.
```

New text:

```rust
//! # polyoxide-binance
//!
//! Rust client library for Binance's APIs, one feature per module: `usdm` and `spot`.
//!
//! `usdm` (feature `usdm`, on by default) reads USDⓈ-M futures public market
//! data (`fapi.binance.com`), and with `usdm-ws` its market streams
//! (`fstream.binance.com`). `spot` (feature `spot`, on by default) reads spot
//! public market data (`api.binance.com`). No credentials are needed. Each
//! module's own docs carry its example.
```

Leave every code line of `lib.rs` as Stories 2, 4 and 6 left it.

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c '`spot`' polyoxide-binance/README.md
grep -c '`spot`' polyoxide-binance/src/lib.rs
nix develop --command cargo test --doc -p polyoxide-binance --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance --all-features'
```

Expected: `3` (the opening paragraph, the table's feature cell and the paragraph below it; the number only has to be above `0`) and `1`, then the doctests pass (`test result: ok`, the crate README and both module READMEs among them) and the doc build prints no warning.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(binance): the crate README and crate docs name both modules (binance spot, Story 7)" -- polyoxide-binance/README.md polyoxide-binance/src/lib.rs
```

### Task 7.4: The umbrella README, the root README and the other hand-written lines

**Files:**
- Modify: `polyoxide/README.md:11`, `polyoxide/README.md:43-44`, `polyoxide/README.md:69-71`
- Modify: `README.md:3`, `README.md:54-55`
- Modify: `docs/specs/INDEX.md:6`
- Modify: `SELF-HEALING.md:7-8`
- Modify: `CLAUDE.md:7`, `CLAUDE.md:87`, `CLAUDE.md:116`

- [ ] **Step 1: Write the failing check**

```bash
grep -c 'binance-spot' polyoxide/README.md
grep -c 'binance-spot' README.md
grep -c '`X-MBX` and `api/v3`' CLAUDE.md
grep -c 'none current for spot' SELF-HEALING.md docs/specs/INDEX.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: `0`, `0`, `0`, then `SELF-HEALING.md:0` and `docs/specs/INDEX.md:0`.

- [ ] **Step 3: Implement**

`polyoxide/README.md`, three edits.

(a) The path table row. Current:

```markdown
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures market data and streams | `binance` and `binance-<module>` |
```

New:

```markdown
| `polyoxide::binance` | `polyoxide-binance`: Binance's USDⓈ-M futures market data and streams, and its spot market data | `binance` and `binance-<module>` |
```

(b) The install block. Current:

```markdown
# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws
```

New (the second block is added):

```markdown
# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws

# Binance spot market data
cargo add polyoxide --no-default-features --features binance-spot
```

(c) The feature table. Current:

```markdown
| `binance` | no | `binance-usdm`, `polyoxide-binance`'s defaults |
| `binance-usdm` | no | Binance USDⓈ-M futures market data via `polyoxide-binance`'s `usdm` module |
| `binance-usdm-ws` | no | Binance USDⓈ-M market streams via `polyoxide-binance`'s `usdm-ws` feature |
```

New:

```markdown
| `binance` | no | `binance-usdm` + `binance-spot`, `polyoxide-binance`'s defaults |
| `binance-usdm` | no | Binance USDⓈ-M futures market data via `polyoxide-binance`'s `usdm` module |
| `binance-usdm-ws` | no | Binance USDⓈ-M market streams via `polyoxide-binance`'s `usdm-ws` feature |
| `binance-spot` | no | Binance spot market data via `polyoxide-binance`'s `spot` module |
```

`README.md` (root), two edits.

(a) Line 3. Current:

```markdown
Multi-venue Rust SDK toolkit: Polymarket's APIs, Binance's USDⓈ-M futures market data and Kalshi's event-contract market data, one
```

New:

```markdown
Multi-venue Rust SDK toolkit: Polymarket's APIs, Binance's USDⓈ-M futures and spot market data and Kalshi's event-contract market data, one
```

(b) The install block. Current:

```markdown
# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws
```

New:

```markdown
# Binance USDⓈ-M futures market data and its streams
cargo add polyoxide --no-default-features --features binance-usdm-ws

# Binance spot market data
cargo add polyoxide --no-default-features --features binance-spot
```

Leave the generated `readme-crates` table (lines 12-25) alone: it was regenerated from the crates' metadata by Story 6.

`docs/specs/INDEX.md` line 6 (the intro, outside the generated regions). Current:

```markdown
for USDⓈ-M, as its hosts were observed to behave, and Kalshi's, which publishes an OpenAPI and an AsyncAPI, as it publishes them, with what its hosts were observed to do beside them ([Other venues](#other-venues)).
```

New:

```markdown
for USDⓈ-M and none current for spot, as its hosts were observed to behave, and Kalshi's, which publishes an OpenAPI and an AsyncAPI, as it publishes them, with what its hosts were observed to do beside them ([Other venues](#other-venues)).
```

`SELF-HEALING.md` lines 7-8 (outside the generated regions). Current:

```markdown
ways, in the live tests and against the vendored mirrors, and so does Kalshi's drift; Binance publishes none
for USDⓈ-M, so its live suites are its only drift check.
```

New:

```markdown
ways, in the live tests and against the vendored mirrors, and so does Kalshi's drift; Binance publishes none
for USDⓈ-M and none current for spot, so its live suites are its only drift check.
```

`CLAUDE.md`, three edits (all outside the generated regions).

(a) Line 7, the project overview. Replace

```
`polyoxide-binance` for Binance's USDⓈ-M futures market data and streams and `polyoxide-kalshi`
```

with

```
`polyoxide-binance` for Binance's USDⓈ-M futures market data and streams and its spot market data, and `polyoxide-kalshi`
```

(b) Line 87, the venue gate rule. Replace

```
Binance's `fapi`, `fstream` and `X-MBX`)
```

with

```
Binance's `fapi`, `fstream`, `X-MBX` and `api/v3`)
```

(c) Line 116, the module layout. Replace

```
`polyoxide-binance`'s `usdm`; `polyoxide-kalshi`'s `predictions`)
```

with

```
`polyoxide-binance`'s `usdm` and `spot`; `polyoxide-kalshi`'s `predictions`)
```

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'binance-spot' polyoxide/README.md
grep -c 'binance-spot' README.md
grep -c '`X-MBX` and `api/v3`' CLAUDE.md
grep -c 'none current for spot' SELF-HEALING.md docs/specs/INDEX.md
python3 scripts/gen_registry.py --check
```

Expected: `3` (the table row, the install block, the feature row; at least `1` is the bar), `1`, `1` (only the line edited in (b): the count is of its exact words, so a generated region that names `api/v3` does not change it), `SELF-HEALING.md:1` and `docs/specs/INDEX.md:1`, and `--check` exits `0` (the edits left every generated region as it was).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(binance): spot in the umbrella and root READMEs and the hand-written lines of CLAUDE.md, SELF-HEALING.md and the spec index (binance spot, Story 7)" -- polyoxide/README.md README.md docs/specs/INDEX.md SELF-HEALING.md CLAUDE.md
```

### Task 7.5: The rules files

**Files:**
- Modify: `.claude/rules/binance.md:1-17`, plus two inserted sections
- Modify: `.claude/rules/umbrella.md`
- Modify: `CLAUDE.md:40` (the rules table row for `binance.md`)

- [ ] **Step 1: Write the failing check**

```bash
grep -c 'capture_binance_spot' .claude/rules/binance.md
grep -c '^## `spot`' .claude/rules/binance.md
grep -c '^## `shared`' .claude/rules/binance.md
grep -c 'binance-spot' .claude/rules/umbrella.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: four `0`s.

- [ ] **Step 3: Implement**

**`.claude/rules/binance.md`**, four edits.

(a) The `paths:` list. Current:

```yaml
  - "scripts/capture_binance_usdm.py"
```

New (the capture script is added; `polyoxide-binance/**` already covers `polyoxide-binance/src/spot/**`):

```yaml
  - "scripts/capture_binance_usdm.py"
  - "scripts/capture_binance_spot.py"
```

(b) The title and the opening paragraph. Current:

```markdown
# Binance USDⓈ-M

`polyoxide-binance`'s `usdm` module (feature `usdm`, on by default) reads public USDⓈ-M futures
market data on `fapi.binance.com` (`usdm::Usdm::new()`; namespaces `usdm.health()`, `exchange()`,
`market()`) for consumers that trade both venues. Binance is not a Polymarket host. The umbrella
re-exports the crate as `polyoxide::binance` under its `binance`, `binance-usdm`,
`binance-usdm-ws` and `full` features, none of which its default, `polymarket`, turns on.
```

New:

```markdown
# Binance: USDⓈ-M and spot

`polyoxide-binance`'s `usdm` module (feature `usdm`, on by default) reads public USDⓈ-M futures
market data on `fapi.binance.com` (`usdm::Usdm::new()`; namespaces `usdm.health()`, `exchange()`,
`market()`) for consumers that trade both venues. Binance is not a Polymarket host. The umbrella
re-exports the crate as `polyoxide::binance` under its `binance`, `binance-usdm`,
`binance-usdm-ws`, `binance-spot` and `full` features, none of which its default, `polymarket`,
turns on. Its `spot` module (feature `spot`, on by default) reads public spot market data on
`api.binance.com`; the `shared` and `spot` sections below hold the rules both modules share and
spot's own.
```

(c) Insert a `shared` section directly before `## The weight budget` (the anchor line is `## The weight budget`, which appears once). The new section, then a blank line, then the anchor line again:

````markdown
## `shared`

`src/shared/` is crate-private and built under `any(feature = "usdm", feature = "spot")`. It holds
what the two products do the same way, so one fix serves both, and `cargo hack --each-feature`
builds each product without the other, so every item in it compiles under either.

- `shared::weight`: the minute `Engine` (the UTC clock minute at the published count less a tenth,
  raised by `X-MBX-USED-WEIGHT-1M` only for the minute its request was charged in; the client-wide
  `Hold` for a `429` or `418`, only ever extended, with the 3-day ceiling; an optional paced layer,
  which only `usdm`'s funding routes use), its `Clock` and the `USED_WEIGHT_HEADER`, `DEFAULT_BAN`
  and `MAX_COOLDOWN` constants. A product's `WeightBudget` is a handle to one `Engine` with its own
  layers and its own `Throttle` impl.
- `shared::policy::decide`: the retry decision as a function of the response, the attempt, the
  schedule and the time to the next minute. "The retry policy" below is its rules.
- `shared::error`: `decode` by status first into `Decoded`, `retry_after_secs`, `clip` and
  `venue_class`. A product's one `From<ApiError>` maps `Decoded` one to one onto its error enum and
  logs the `418` body at WARN itself, so the WARN's target is `polyoxide_binance::<module>::error`.
- `shared::request`: `Routed<T, R, E>`, the `Priced` trait a route table implements, and the
  `route_builder!` macro every route builder is written with.
- `shared::symbol`: `Symbol` and `InvalidSymbol`. Nothing else is shared: response types,
  `Interval` and `Filter` stay per product, so one product's wire change never reaches the other.
- **A product keeps** its `WeightBudget`, its `<Module>RetryPolicy`, its `<Module>Error`, its
  route table and `Route::cost`, its types, its key space, its venue impls and its
  `DEFAULT_BASE_URL`.
- **`usdm`'s public paths do not change.** `usdm::types::Symbol`, `usdm::weight::DEFAULT_BAN` and
  the rest are `pub use` of the shared items. `usdm`'s unit tests stay in `usdm`'s files, where
  they reach the shared code through `usdm`'s wrappers, because the test-bodies gate keys a test by
  its file and module path: a moved test is a lost one.
- `docs/MUTANTS.md`'s rows (a) and (k) name `src/shared/policy.rs`.
````

(d) Insert a `spot` section directly before `## Streams (`usdm-ws`)` (that heading appears once). The new section, then a blank line, then the heading again:

````markdown
## `spot`

`polyoxide-binance`'s `spot` module (feature `spot`, on by default) reads public spot market data
on `api.binance.com` (`spot::Spot::new()`; namespaces `spot.health()`, `exchange()`, `market()`),
fourteen methods over eleven routes, with no credentials. It is the first product of
`ProductClass::Spot`.

- **No spec to diff.** Binance's `spot_api.yaml` was last released on 2024-10-08, lacks fields the
  wire sends today (`amendAllowed`, `opoAllowed`, `pegInstructionsAllowed`) and is mostly `/sapi`
  paths, so `nightly-schema.yml` does not watch it.
  `tests/live_spot.rs::live_responses_carry_no_unmodelled_keys` is the drift detector, and a
  measured fact goes in `docs/specs/binance/OBSERVED.md`'s `## Spot` section.
- **The weight budget.** `spot::WeightBudget` is the shared `Engine` at 5400 of
  `PUBLISHED_WEIGHT_PER_MINUTE = 6000`, on `WEIGHT_LAYER`, `binance-spot-weight`, with no paced
  layer. The `RAW_REQUESTS` limit (300,000 per 5 minutes) is not modelled: every request weighs at
  least 1, so the budget caps a client at 27,000 requests per 5 minutes, and the module docs say
  so. Pass a budget with `SpotBuilder::weight_budget`; every `Spot` built with one shares its hold.
- **The weight table**, `spot::weight::Route::cost`, is measured over distinct URLs, not copied,
  and pinned by its `documented_weights` test: `ping` and `time` 1, `exchangeInfo` 20, `depth` 5
  without `limit` and for 1 to 100, 25 for 101 to 500, 50 for 501 to 1000 and 250 above (the last
  band documented, not measured), `trades` 25, `aggTrades` 4, `klines`, `avgPrice` and the
  one-symbol tickers 2, `ticker/24hr` without a symbol 80, `ticker/price` and `ticker/bookTicker`
  without one 4. `weight_probe_spot` re-measures it (`cargo run -p polyoxide-binance --example weight_probe_spot`, no
  `--product` argument; `weight_probe` is unchanged and measures USDⓈ-M).
  `GetDepth::limit` is a hand-written setter because it also re-prices the route (`clients.md`).
- **A `limit` past a route's maximum is truncated by the venue, not refused**, and is sent as the
  caller gave it; the cost does not change but for `depth`'s bands. Only `Symbol` is validated by
  the client.
- **In flight.** One `Spot` at the default 4 concurrent requests has at most 320 weight in flight
  on a route at its default (4 × `tickers_24h`'s 80), under the reserve of 600; four `depth`
  calls above 1000 levels (1000) pass it. The builder's docs say so.
- **The retry policy** is `UsdmRetryPolicy`'s, below: the private `SpotRetryPolicy`
  (`src/spot/policy.rs`) decides through `shared::policy::decide`, and `SpotError` has
  `UsdmError`'s variants, `code()` and `Classify`. A `418` logs its body at WARN under
  `polyoxide_binance::spot::error`.
- `Interval` is exhaustive (`1s` through `1M`) and separate from `usdm`'s. `Symbol` is `usdm`'s
  (`shared`), so it accepts `_` though spot lists no such symbol; the venue answers an unknown
  symbol `400` with `-1121`. `Filter` is an open enum. Klines and book levels decode through
  `polyoxide_venue::positional` in visitors that tolerate extra elements. Response rows carry
  symbols as `String`.
- **`spot::venue`:** `MarketData`, `Instruments`, `Trades` and `Candles` on `Spot`, with
  `InstrumentExt`, `BookExt`, `TradeExt` and `CandleExt`; a quote has no extension. Its `quote` is
  `ticker/bookTicker` (weight 2, `time` `None`), its `book` the default `depth` (100 levels,
  weight 5), its `instrument` and `instruments` `exchangeInfo` (weight 20), its `trades` `aggTrades`
  with `limit=1000` (weight 4) and its `candles` `klines` with `limit=1000` (weight 2). Its
  `instruments` skips a row that does not convert (a WARN naming the symbol and the error), and
  the live suite fails when it skips one. `lib.rs` pins `Spot` to `SpotMarket`. `spot::key`
  makes `binance.spot:<SYMBOL>` keys with `CLASS = ProductClass::Spot`.
- A 451, a `418` and the firewall `403` are `Restricted`, classified by status as on `usdm`.
- `tests/venue_spot.rs` holds each method to the native client's values on the captured fixtures;
  the fixtures are captured, never written by hand, by `scripts/capture_binance_spot.py rest`.
````

**`.claude/rules/umbrella.md`**, three edits.

(a) The market-trait bullet. Current:

```markdown
- A module that implements the market-data traits (`polymarket-clob`, `polymarket-perps`,
  `binance-usdm`, `kalshi-predictions`) also turns on `polyoxide-venue/market`, and one that only
```

New:

```markdown
- A module that implements the market-data traits (`polymarket-clob`, `polymarket-perps`,
  `binance-usdm`, `binance-spot`, `kalshi-predictions`) also turns on `polyoxide-venue/market`, and one that only
```

(b) The features bullet. Current:

```markdown
- The features `polymarket` and `binance` turn on their venue crate's defaults; each
```

New:

```markdown
- The features `polymarket` and `binance` turn on their venue crate's defaults (`binance` is
  `binance-usdm` and `binance-spot`); each
```

(c) A new bullet directly after the Kalshi prelude bullet (the one that ends "`parse_key` also names\n  `kalshi.predictions` under `kalshi-predictions`."):

```markdown
- Binance spot's prelude names, under `binance-spot`, are `Spot`, `SpotBuilder`, `SpotError` and
  `WeightBudget` as `SpotWeightBudget` (`USDⓈ-M`'s keeps the bare `WeightBudget`). `parse_key` also
  names `binance.spot` under `binance-spot`, and its passthrough `const _` names
  `crate::binance::spot::Spot`. `SpotMarket` is at `polyoxide::venue` beside the other bundles.
```

**`CLAUDE.md` line 40**, the rules table row. Current:

```markdown
| `binance.md` | Binance USDⓈ-M: the weight budget, its policy, market-data impl and streams |
```

New:

```markdown
| `binance.md` | Binance: the `shared` plumbing, USDⓈ-M and spot, their weight budgets, policy and market-data impls, and USDⓈ-M's streams |
```

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'capture_binance_spot' .claude/rules/binance.md
grep -c '^## `spot`' .claude/rules/binance.md
grep -c '^## `shared`' .claude/rules/binance.md
grep -c 'binance-spot' .claude/rules/umbrella.md
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
python3 scripts/gen_registry.py --check
```

Expected: `2` (the `paths:` line and the `spot` section; at least `1` is the bar), `1`, `1`, `4`, then the rules test passes (`N passed`, no failure naming a glob that matches no file or a table that disagrees with the directory) and `--check` exits `0`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(binance): the shared and spot rules, and the umbrella's binance-spot (binance spot, Story 7)" -- .claude/rules/binance.md .claude/rules/umbrella.md CLAUDE.md
```

### Task 7.6: The spine amendment

**Files:**
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md`

- [ ] **Step 1: Write the failing check**

```bash
test -f _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md && grep -c 'ProductClass::Spot' _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: no output, exit status `1` (the file does not exist).

- [ ] **Step 3: Implement**

Create the file with this content (the format is `adr-0001.md`'s, in the same directory):

````markdown
# Proposed spine amendments: Binance spot

Recorded per AD-21 from `docs/superpowers/specs/2026-11-02-binance-spot-design.md`. One session later merges this file into the spine, after ADR 0001's amendments (`adr-0001.md`), and regenerates the guide.

## BSPOT-1: `ProductClass::Spot` (amends ADR1-1)

- **Proposed by:** the owner, 2026-11-02, in brainstorming (spec §2, the "Product class" row).
- **Current rule:** ADR1-1 makes `polyoxide-venue`'s `ProductClass` (feature `keys`) `#[non_exhaustive]`, with `EventContract` and `PerpetualFuture`, and adds a variant only when a product of that class lands. Each `KeySpace` declares `const CLASS: ProductClass`, and a crate registers it in `classes = { <product> = "<class>" }`.
- **Proposed rule:**
  - `ProductClass` gains `Spot`, after `PerpetualFuture`, with `as_str()` `"spot"`: buying and selling the asset itself; a price is unbounded, a position is a balance of the base asset and so never negative, and there is no funding.
  - Binance spot is the class's first product: `polyoxide-binance` registers `products = ["usdm", "spot"]` and `classes = { usdm = "perpetual-future", spot = "spot" }`, and `spot::key::SpotKeys` declares `CLASS = ProductClass::Spot`.
  - The foundation crate names no venue: the variant's docs and its tests use made-up instruments.
- **Why:**
  - **Spot is a class of its own.** A perpetual future has funding and a signed position; a spot product has neither, so a consumer that reads a key's class without a request must tell them apart (ADR1-1's reason).
  - **The rule's condition is met.** ADR1-1 adds a variant only when a product of that class lands, and Binance spot lands with this slice.
- **Follow-up in the code:** Story 3 of `docs/plans/2026-11-02-binance-spot-rest.md` adds the variant; Story 6 registers the class and the key space. The merge session edits ADR1-1's text and the guide's list of classes.

## BSPOT-2: the `SpotMarket` bundle (amends ADR1-2)

- **Proposed by:** the owner, 2026-11-02, in brainstorming (spec §2, the "Product class" row, and §5.3).
- **Current rule:** ADR1-2 makes a class bundle name `MarketData` plus the capabilities that every product of the class offers and that mean nothing outside it (`PerpetualMarket: MarketData + Funding`, `EventContractMarket: MarketData + EventGroups`), each with a blanket impl and no `Dyn` wrapper, and has each venue crate pin its products to their bundles in a `const _`.
- **Proposed rule:**
  - `polyoxide-venue` (feature `market`) holds `pub trait SpotMarket: MarketData {}` with `impl<T: MarketData> SpotMarket for T {}`, re-exported at the crate root beside the other two bundles, with no `Dyn` wrapper.
  - The bundle adds nothing to `MarketData`: no capability is defined by spot alone. `Instruments`, `Trades` and `Candles` are shared with other classes and stay out, as ADR1-2 says they do.
  - `polyoxide-binance`'s `lib.rs` pins `spot::Spot` to `SpotMarket` in a `const _`, beside `usdm::Usdm`'s pin to `PerpetualMarket`.
- **Why:**
  - **Every class has one bound.** A function generic over a class takes one bound instead of a list each consumer writes and lets drift, and the bundle names only what defines its class. Nothing beyond `MarketData` defines spot, so the bundle is that alone.
  - **It has one implementer today.** ADR1-2 justifies a bundle by two implementers; this one holds no capability that needs the justification, and a capability that spot and a second spot product both offer and no other class does joins it under ADR1-2's promotion rule, citing both products.
- **Follow-up in the code:** Story 3 of `docs/plans/2026-11-02-binance-spot-rest.md` adds the trait and its re-export; Story 6 adds the pin in `polyoxide-binance/src/lib.rs`. The merge session edits ADR1-2's text and the guide's list of bundles.
````

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'ProductClass::Spot' _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-binance -p polyoxide --all-features'
python3 scripts/gen_registry.py --check
```

Expected: a count above `0`, the doc build ends with `Finished` and no warning, and `--check` exits `0`.

- [ ] **Step 5: Commit**

```bash
git add -N _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md
git commit -m "docs(binance): the spine amendment for ProductClass::Spot and SpotMarket (binance spot, Story 7)" -- _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/binance-spot.md
```

### Task 7.7: The other rules files' body text

**Files:**
- Modify: `.claude/rules/rate-limits.md` (the Binance bullet)
- Modify: `.claude/rules/send-loop.md` (four sentences)
- Modify: `.claude/rules/live-tests.md` (the failure-tags sentence)
- Modify: `.claude/rules/testing.md` (the mock-suite list)
- Modify: `.claude/rules/errors.md` (the module-error list)
- Modify: `.claude/rules/clients.md` (the builder list)
- Modify: `.claude/rules/wire-enums.md` (two sentences)
- Modify: `.claude/rules/specs.md` (the Binance sentence)

Each edit anchors on the sentence's text, not on a line number, and changes that sentence only. Stories 2, 4 and 5 have already changed these files' `paths:` globs, and Story 5 has changed `clients.md`'s setter count, Binance bullet and `query_setters_*` sentence; none of those lines is quoted or touched here. Read each sentence in the file before replacing it, and stop if it no longer reads as quoted.

- [ ] **Step 1: Write the failing check**

```bash
grep -c 'spot::WeightBudget' .claude/rules/rate-limits.md
grep -c 'SpotRetryPolicy' .claude/rules/send-loop.md
grep -c '/api/v3/ping' .claude/rules/send-loop.md
grep -c 'failure_tags_spot' .claude/rules/live-tests.md
grep -c 'mock_spot' .claude/rules/testing.md
grep -c 'SpotError' .claude/rules/errors.md
grep -c 'Spot::builder()' .claude/rules/clients.md
grep -c 'spot/types.rs' .claude/rules/wire-enums.md
grep -c "Binance's .spot. keep" .claude/rules/wire-enums.md
grep -c 'none current for spot' .claude/rules/specs.md
```

- [ ] **Step 2: Run it to see it fail**

Expected: ten `0`s.

- [ ] **Step 3: Implement**

**`.claude/rules/rate-limits.md`.** Current:

```markdown
- **Binance**: `usdm::WeightBudget` (`binance.md`).
```

New:

```markdown
- **Binance**: `usdm::WeightBudget` and `spot::WeightBudget` (`binance.md`).
```

**`.claude/rules/send-loop.md`**, four edits.

(a) The route builders. Current:

```markdown
- Binance's route builders send core's `Request` over its `WeightBudget` throttle and
  `UsdmRetryPolicy`.
```

New:

```markdown
- Binance's route builders send core's `Request` over its `WeightBudget` throttle and
  `UsdmRetryPolicy` (spot's, over its own `WeightBudget` and `SpotRetryPolicy`).
```

(b) The ping path. Current:

```markdown
keeps a prefix; perps `/v1/info/ping`; Binance `/fapi/v1/ping`, costed from `Route::Ping`) and its
body check (perps `status == "ok"`, Binance `{}`).
```

New:

```markdown
keeps a prefix; perps `/v1/info/ping`; Binance `/fapi/v1/ping`, and spot's `/api/v3/ping`, each
costed from its own `Route::Ping`) and its body check (perps `status == "ok"`, Binance `{}`).
```

(c) The retry decision. Current:

```markdown
- Binance's `UsdmRetryPolicy` retries a `429` and never a `425` (`binance.md`).
```

New:

```markdown
- Binance's `UsdmRetryPolicy` and `SpotRetryPolicy` retry a `429` and never a `425` (`binance.md`).
```

(d) The headers suites. Current:

```markdown
  `relay` and `perps`, Binance's `usdm`, Kalshi's `predictions`) that sends one unauthenticated
```

New:

```markdown
  `relay` and `perps`, Binance's `usdm` and `spot`, Kalshi's `predictions`) that sends one unauthenticated
```

**`.claude/rules/live-tests.md`.** Current:

```markdown
`polyoxide-binance`'s `tests/failure_tags_usdm.rs`, and `polyoxide-polymarket`'s
```

New:

```markdown
`polyoxide-binance`'s `tests/failure_tags_usdm.rs` and `tests/failure_tags_spot.rs`, and `polyoxide-polymarket`'s
```

**`.claude/rules/testing.md`.** Current:

```markdown
  `mock_data_v2.rs` beside it), `mock_relay` and `mock_perps`; Binance's `mock_usdm`; Kalshi's
```

New:

```markdown
  `mock_data_v2.rs` beside it), `mock_relay` and `mock_perps`; Binance's `mock_usdm` and
  `mock_spot`; Kalshi's
```

**`.claude/rules/errors.md`.** Current:

```markdown
  `DataError`, `RelayError`, Binance's `UsdmError`, Kalshi's `PredictionsError`. A module's socket
```

New:

```markdown
  `DataError`, `RelayError`, Binance's `UsdmError` and `SpotError`, Kalshi's `PredictionsError`. A module's socket
```

**`.claude/rules/clients.md`.** Current:

```markdown
  `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`,
  `Predictions::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`.
```

New:

```markdown
  `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`,
  `Spot::builder()`, `Predictions::builder()`, `RelayClient::default_builder()`,
  `Polymarket::builder(account)`.
```

**`.claude/rules/wire-enums.md`**, two edits.

(a) The Binance types file. Current:

```markdown
`session_signers.rs`), all in `polyoxide-polymarket`, and of Binance (`usdm/types.rs`):
```

New:

```markdown
`session_signers.rs`), all in `polyoxide-polymarket`, and of Binance (`usdm/types.rs` and
`spot/types.rs`):
```

(b) The `Interval` enums. Current:

```markdown
- Perps and Binance keep separate `Interval` enums (D16).
```

New:

```markdown
- Perps, Binance's `usdm` and Binance's `spot` keep separate `Interval` enums (D16).
```

**`.claude/rules/specs.md`.** Current:

```markdown
OpenAPI specs; Kalshi's OpenAPI and AsyncAPI are mirrored in `docs/specs/kalshi/`; Binance
publishes none for USDⓈ-M, so `docs/specs/binance/` holds what its hosts were observed to do.
```

New:

```markdown
OpenAPI specs; Kalshi's OpenAPI and AsyncAPI are mirrored in `docs/specs/kalshi/`; Binance
publishes none for USDⓈ-M and none current for spot (spot's `spot_api.yaml` has been stale since
2024-10-08 and is not watched), so `docs/specs/binance/` holds what its hosts were observed to do.
```

- [ ] **Step 4: Run it to see it pass**

```bash
grep -c 'spot::WeightBudget' .claude/rules/rate-limits.md
grep -c 'SpotRetryPolicy' .claude/rules/send-loop.md
grep -c '/api/v3/ping' .claude/rules/send-loop.md
grep -c 'failure_tags_spot' .claude/rules/live-tests.md
grep -c 'mock_spot' .claude/rules/testing.md
grep -c 'SpotError' .claude/rules/errors.md
grep -c 'Spot::builder()' .claude/rules/clients.md
grep -c 'spot/types.rs' .claude/rules/wire-enums.md
grep -c "Binance's .spot. keep" .claude/rules/wire-enums.md
grep -c 'none current for spot' .claude/rules/specs.md
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
```

Expected: `1` for every grep except `SpotRetryPolicy`, which is `2` (the route-builder and retry sentences), then the rules test passes (`N passed`, no failure naming a glob that matches no file or a table that disagrees with the directory).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(rules): spot's counterpart in each sentence that lists Binance's usdm files (binance spot, Story 7)" -- .claude/rules/rate-limits.md .claude/rules/send-loop.md .claude/rules/live-tests.md .claude/rules/testing.md .claude/rules/errors.md .claude/rules/clients.md .claude/rules/wire-enums.md .claude/rules/specs.md
```
