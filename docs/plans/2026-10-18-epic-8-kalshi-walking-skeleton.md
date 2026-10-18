# Epic 8: the Kalshi Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** Kalshi lands as a third venue by the guide's recipe: `polyoxide-kalshi` (`publish = false`) with signed, throttled REST, exchange status, `MarketData`, `Trades` and `EventGroups`, and a supervised order-book socket whose handshake is signed on every attempt. It touches only the venue's files and the edits its spec pre-approves, and records every gap in the recipe as an amendment (Epic 8, Stories 8.1 to 8.5).

**Architecture:** One crate, `polyoxide-kalshi`, with one module, `predictions`. Its REST client (feature `predictions`) runs on core's send loop with core's `DefaultRetryPolicy`, a provisional token-bucket throttle confirmed at first use, and an Ed25519 or RSA-PSS authenticator over `ring`. Its socket (feature `predictions-ws`) is a `Protocol` on `polyoxide-ws`'s `Supervisor`. The market-data traits turn Kalshi's YES and NO bid ladders into one book per outcome. The kit's scripted server gains a handshake-header recorder. CI's derived header pins, the dependency allowlists and CLAUDE.md learn the unpublished venue.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), tokio, ring 0.17 (a new direct pin), base64, chrono, rust_decimal, serde, mockito, the kit's scripted WebSocket server, a dev-only YAML parser (`serde-saphyr`, spec §4.7), and Python capture scripts (PEP 723, `cryptography`, `websockets`).

**Spec:** `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`, read with `_bmad-output/planning-artifacts/epics.md` (Epic 8) and the architecture spine (AD-4, AD-7, AD-8, AD-10, AD-11, AD-13, AD-15). The spec binds where it and the epic's criteria differ (its §1.4).

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `nix develop --command`. A story's gates are the profile's (spec §10); its **Verify** line is the developer's done-check, and it ends with the touch-set gate.
- **The touch-set gate.** `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh` (Story 1 writes it, names contract) prints every path the branch has changed, staged, left unstaged or left untracked since `git merge-base main HEAD` that the allowlist does not admit. It fails when it prints one. An edit outside the allowlist stops the story and goes to the owner (spec D20).
- **Put code in place from this file by line range.** A new file is the block after its `Create` line. Each edit replaces the block after a `Current (...)` line with the block after the `Replace with:` line that follows it. Each `Current` block occurs exactly once in its file at that point, and a task's edits apply top to bottom. In CLAUDE.md a `Current` block may be part of one long line.
- **Shared files grow by anchors, never by quoting another story's text.** In `polyoxide-kalshi/Cargo.toml` and `docs/specs/kalshi/OBSERVED.md`, a later story inserts directly before or after an anchor line that Story 1 writes and the names contract gives verbatim. The Rust module blocks are the one exception: each story replaces the whole block the contract gives for the story before it.
- **Add tests; never change an existing test's body** (AD-12, the test-bodies job against `v0.40.0`). Each story's suites are files of its own. Story 5 appends its test after the last test in `polyoxide-ws/src/test_server/tests.rs`.
- **The foundation crates name no venue** (`python3 scripts/venue_gate.py`). Story 5's doc and test use made-up words, and never "kalshi", "predictions", "clob", "Polymarket", "Binance" or "perps".
- **Generated regions are regenerated, never edited.** After a `[package.metadata.polyoxide]` change or a mirrors change (Stories 1, 2, 3, 4 and 6), run `python3 scripts/gen_registry.py --write` and commit what it rewrites. `docs/ARCHITECTURE.md` is never edited.
- **README text is prose with no code fence**, since the crate and module READMEs are doctests.
- **Mirrors stay byte for byte as served.** Observations go in `OBSERVED.md`.
- **Live suites:**
  - credentials come only through `polyoxide-test-support`'s loaders, with the env names as string literals;
  - failures go through `or_fail`, `environmental` and `transient`;
  - no `.unwrap()` or `.expect(`, and every test is `#[ignore]`d.
- **The demo captures need the owner's key.** Stories 4 and 6 capture from the demo host with `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`. When either is absent, the story stops at its capture and asks; no fixture is ever written by hand (spec D19).
- **A bare `D<n>` in code, docs, READMEs and CLAUDE.md names a row of `divergences.md`**, as CLAUDE.md already uses it. The spec's decisions D1 to D25 share those numbers, so code and docs state a decision in words. Story 7's planning documents may cite one as "spec D<n>".
- **Commit by pathspec**, with a conventional message naming the story, and include `Cargo.lock` when cargo changes it.

## File structure

Paths under `polyoxide-kalshi/` are listed from the crate root.

**Story 1: register the crate.**

- `Cargo.toml` (modify): the `members` line, the `polyoxide-kalshi` and `ring` pins, and the `kalshi` mirrors table.
- `polyoxide-kalshi/Cargo.toml` (create): the package, the `predictions` feature, its dependencies, Story 1's suites, the registration metadata and the docs.rs list, with the anchors the names contract gives.
- `polyoxide-kalshi/README.md` (create) and `src/predictions/README.md` (create): prose.
- `src/lib.rs` (create): the crate docs, the README doctest structs, `pub mod predictions;` and the `Classify` assertion.
- `src/predictions/mod.rs` (create): the module block, the consts, `Predictions`, `Inner`, `PredictionsBuilder` and the `namespaces!` block.
- `src/predictions/request.rs` (create): `READ`, `DEFAULT_COST`, `Route`, the whole route table, the doc-hidden accessors and `Routed<T>`.
- `src/predictions/throttle.rs` (create): `PredictionsThrottle`, `RouteCosts` and the `before_send` stub.
- `src/predictions/error.rs` (create): `PredictionsError`, `VenueError` and the one decode.
- `src/predictions/key.rs` (create): the key space and its tests.
- `src/predictions/api/{mod,health,exchange}.rs` (create): `Health::ping` and `ExchangeApi::status`.
- `src/predictions/types/{mod,exchange}.rs` (create): `ExchangeStatus` and `ExchangeIndexStatus`.
- `tests/headers_predictions.rs`, `tests/mock_predictions.rs` and `tests/error_classes_predictions.rs` (create).
- `docs/specs/kalshi/openapi.yaml` and `docs/specs/kalshi/asyncapi.yaml` (create, downloaded as served), and `docs/specs/kalshi/OBSERVED.md` (create).
- `ci/dep-allowlists/polyoxide-kalshi@predictions.txt` (create, written by the test's `--write`).
- `.github/workflows/ci.yml`, `.github/scripts/tests/test_ci_workflow.py` and `.github/scripts/tests/test_dep_allowlists.py` (modify): spec §3.7.
- `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt` and `epic-8-touch-set.sh` (create): the touch-set gate.
- `CLAUDE.md` (modify): Story 1's paragraphs, and the regenerated regions.
- `README.md`, `docs/specs/INDEX.md` and `.github/workflows/nightly-schema.yml` (regenerated).
- `Cargo.lock`.

**Story 2: the public REST surface.**

- `Cargo.toml` (modify): the YAML parser's pin, after `ring = "0.17"`.
- `polyoxide-kalshi/Cargo.toml` (modify): the YAML dev-dependency, Story 2's suites and the `live_predictions` table.
- `src/predictions/mod.rs` (modify): the `namespaces!` block.
- `src/predictions/api/{markets,events,account}.rs` (create), and `src/predictions/api/mod.rs` (modify).
- `src/predictions/types/{markets,events,account}.rs` (create), and `src/predictions/types/mod.rs` (modify).
- `scripts/capture_kalshi_predictions.py` (create).
- `tests/fixtures/predictions/` (create, captured): `exchange_status.json`, `endpoint_costs.json`, `market.json`, `markets_page.json`, `orderbook.json`, `orderbook_depth1.json`, `trades.json`, `event.json`, `error_not_found.json`, `error_validation.json`, `error_unauthenticated.json`, `error_rate_limited.json` and `PROVENANCE.md`.
- `tests/common/mod.rs` (create): the live suites' market selection.
- `tests/mock_predictions_routes.rs`, `tests/query_setters_predictions.rs`, `tests/wire_agreement_predictions.rs`, `tests/spec_agreement_predictions.rs` and `tests/live_predictions.rs` (create).
- `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md` and `CLAUDE.md` (regenerated).
- `Cargo.lock`.

**Story 3: the traits.**

- `polyoxide-kalshi/Cargo.toml` (modify): the umbrella dev-dependency, Story 3's suites and the `live_predictions_venue` table.
- `src/predictions/mod.rs` (modify): the module block gains `venue`.
- `src/predictions/venue.rs` (create): `MarketData`, `Trades` and `EventGroups` on `Predictions`, the conversion, and the three ext types.
- `tests/venue_predictions.rs` and `tests/live_predictions_venue.rs` (create).
- `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md` and `CLAUDE.md` (regenerated).
- `Cargo.lock`.

**Story 4: signing and the first-use confirmation.**

- `polyoxide-kalshi/Cargo.toml` (modify): Story 4's suites and the `live_predictions_account` table.
- `src/lib.rs` (modify): the assertion gains `CredentialsError`.
- `src/predictions/mod.rs` (modify): the module block gains `auth`.
- `src/predictions/auth.rs` (create): `Credentials`, `CredentialsError`, the signer, the authenticator and `PredictionsBuilder::credentials`.
- `src/predictions/throttle.rs` (modify): `before_send` becomes the confirmation.
- `src/predictions/api/limits.rs` (create), and `src/predictions/api/mod.rs` (modify).
- `src/predictions/types/limits.rs` (create), and `src/predictions/types/mod.rs` (modify).
- `scripts/capture_kalshi_account.py` (create).
- `tests/fixtures/predictions/signing/vectors.json` and its `PROVENANCE.md` (create, generated).
- `tests/fixtures/predictions/account/limits.json` and its `PROVENANCE.md` (create, captured from the demo host).
- `tests/signing_predictions.rs`, `tests/mock_predictions_signed.rs` and `tests/live_predictions_account.rs` (create).
- `docs/specs/kalshi/OBSERVED.md` (modify, by its anchor).
- `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md` and `CLAUDE.md` (regenerated).
- `Cargo.lock`.

**Story 5: the kit records handshake headers.**

- `polyoxide-ws/src/test_server/mod.rs` (modify): the recorder.
- `polyoxide-ws/src/test_server/tests.rs` (modify, append only).
- `CLAUDE.md` (modify): the scripted-server paragraph.

**Story 6: the supervised socket.**

- `polyoxide-kalshi/Cargo.toml` (modify): `predictions-ws`, its dependencies, the kit's dev-dependency, Story 6's suites, the `live_predictions_ws` table and the docs.rs list.
- `src/lib.rs` (modify): the assertion gains `PredictionsWsError`.
- `src/predictions/mod.rs` (modify): the module block gains `ws`.
- `src/predictions/ws/{mod,protocol,event,error,builder,scripted}.rs` (create).
- `scripts/capture_kalshi_predictions_ws.py` (create).
- `tests/fixtures/predictions/ws/` (create, captured): the frames and `PROVENANCE.md`.
- `tests/supervision_predictions.rs`, `tests/wire_agreement_predictions_ws.rs` and `tests/live_predictions_ws.rs` (create).
- `ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt` (create, written by `--write`).
- `.github/scripts/tests/test_dep_allowlists.py` (modify): the `MUST_BUILD` row.
- `docs/specs/kalshi/OBSERVED.md` (modify, by its anchor).
- `CLAUDE.md` (modify): the live-suites paragraph and Module Organization's WebSocket paragraph, and the regenerated regions.
- `.github/workflows/nightly-behavioral.yml` and `SELF-HEALING.md` (regenerated).
- `Cargo.lock`.

**Story 7: validation, amendments and records.**

- `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md` (create).
- `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` (modify).
- `_bmad-output/implementation-artifacts/deferred-work.md` (modify).
- `docs/specs/kalshi/OBSERVED.md` (modify, by its anchor, only if the audit finds a gap).

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story quotes. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Paths are from `polyoxide-kalshi/src/` unless they start with a crate or a top-level directory. Each Rust block is in the form `cargo fmt` leaves (rustfmt, edition 2021).

**Story 1 → Stories 2 to 7.**

- **The module block of `predictions/mod.rs`.** Story 1 writes:

      pub mod api;
      pub mod error;
      pub mod key;
      pub mod request;
      mod throttle;
      pub mod types;

      pub use error::{PredictionsError, VenueError};
      pub use key::key;

  Story 3 replaces it with the same block plus `pub mod venue;` directly after `pub mod types;`. Story 4 replaces Story 3's with:

      pub mod api;
      pub mod auth;
      pub mod error;
      pub mod key;
      pub mod request;
      mod throttle;
      pub mod types;
      pub mod venue;

      pub use auth::{Credentials, CredentialsError};
      pub use error::{PredictionsError, VenueError};
      pub use key::key;

  Story 6 replaces Story 4's with the same block plus these two lines directly after `pub mod venue;`:

      #[cfg(feature = "predictions-ws")]
      pub mod ws;

- **Consts in `predictions/mod.rs`:** `pub const PRODUCTION: &str = "https://external-api.kalshi.com";`, `pub const DEMO: &str = "https://external-api.demo.kalshi.co";` and `pub const DEFAULT_MAX_CONCURRENT: usize = 4;`.
- **`pub struct Predictions`**: `Clone`, with a hand-written `Debug` that prints no key material.
  - Its one field is `pub(crate) inner: std::sync::Arc<Inner>`.
  - `pub fn new() -> Result<Self, PredictionsError>` and `pub fn builder() -> PredictionsBuilder`.
- **`pub(crate) struct Inner`**, in `predictions/mod.rs`, with a hand-written `Debug`:
  - `pub(crate) http: polyoxide_core::HttpClient`;
  - `pub(crate) throttle: throttle::PredictionsThrottle`;
  - `pub(crate) costs: throttle::RouteCosts`;
  - `pub(crate) confirmation: tokio::sync::OnceCell<()>`;
  - `pub(crate) auth: Option<std::sync::Arc<polyoxide_core::DynAuthenticator<'static>>>`.
- **The `namespaces!` block in `impl Predictions`.** Story 1 writes:

          polyoxide_core::namespaces! { inner;
              /// Liveness: a ping of the exchange-status route.
              health: Health,
              /// The exchange's status.
              exchange: ExchangeApi,
          }

  Story 2 replaces it with:

          polyoxide_core::namespaces! { inner;
              /// Liveness: a ping of the exchange-status route.
              health: Health,
              /// The exchange's status.
              exchange: ExchangeApi,
              /// Markets, their order books and their public trades.
              markets: MarketsApi,
              /// Events and their markets.
              events: EventsApi,
              /// The account's limits and the routes' token costs.
              account: AccountApi,
          }

  Each namespace struct is `#[derive(Debug, Clone)] pub struct <Name> { pub(crate) inner: std::sync::Arc<Inner> }`. The `Debug` derive needs `Inner: Debug`, which its hand-written impl gives.
- **`pub struct PredictionsBuilder`**: `Clone` and `Default`, with a hand-written `Debug` that prints only whether a key is set.
  - Fields: `config: polyoxide_core::ClientConfig` and `pub(crate) auth: Option<std::sync::Arc<polyoxide_core::DynAuthenticator<'static>>>`.
  - `pub fn new() -> Self`, the five setters of `polyoxide_core::client_config_setters!(config);`, and `pub fn build(self) -> Result<Predictions, PredictionsError>`.
  - `build` refuses a base URL with a path other than `/` with exactly
    `PredictionsError::Api(ApiError::Validation(format!("the base URL must be a scheme and host with no path: {url}")))`.
  - Story 4 adds `pub fn credentials(self, credentials: Credentials) -> Self` in its own `impl PredictionsBuilder` block, in `auth.rs`. It sets `auth`.
- **`request.rs`** (`pub mod request`):
  - `pub(crate) const READ: polyoxide_core::LayerId = polyoxide_core::LayerId("predictions.read");`
  - `#[doc(hidden)] pub const DEFAULT_COST: u32 = 10;`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub(crate) struct Route { pub(crate) method: &'static str, pub(crate) template: &'static str }`, with:
    - `pub(crate) const fn get(template: &'static str) -> Self`;
    - `pub(crate) fn path(&self, params: &[&str]) -> String`, which replaces each `:name` segment with the next parameter, in order.
  - `pub(crate) mod routes` holds `EXCHANGE_STATUS` (`/trade-api/v2/exchange/status`), `MARKETS` (`/trade-api/v2/markets`), `MARKET` (`/trade-api/v2/markets/:ticker`), `ORDERBOOK` (`/trade-api/v2/markets/:ticker/orderbook`), `TRADES` (`/trade-api/v2/markets/trades`), `EVENT` (`/trade-api/v2/events/:event_ticker`), `ENDPOINT_COSTS` (`/trade-api/v2/account/endpoint_costs`) and `ACCOUNT_LIMITS` (`/trade-api/v2/account/limits`). Each is a `pub(crate) const` `Route::get(..)`, and `pub(crate) const ALL: &[Route]` lists all eight. No later story adds a route.
  - `#[doc(hidden)] pub fn route_templates() -> impl Iterator<Item = (&'static str, &'static str)>`: each route's method and template, from `routes::ALL`.
  - `#[doc(hidden)] pub fn template_matches(ours: &str, listed: &str) -> bool`. Two templates match when:
    - they have as many `/` segments, and each pair is equal or both begin with `:`;
    - or, as an exception, a listed segment beginning with `*` matches all the remaining segments.
  - `pub(crate) struct Routed<T>` (`inner`, `route`, `path`, `query: Vec<(&'static str, String)>`, a `PhantomData<fn() -> T>`):
    - `pub(crate) fn new(inner: &std::sync::Arc<Inner>, route: Route, params: &[&str]) -> Self`;
    - `pub(crate) fn query(self, key: &'static str, value: impl ToString) -> Self`, which replaces a key already set;
    - `pub(crate) fn has_key(&self) -> bool`, whether the client holds an authenticator;
    - `pub(crate) async fn send(self) -> Result<T, PredictionsError>` where `T: serde::de::DeserializeOwned`, following spec §3.5's four steps.
  - Every route builder holds `pub(crate) request: Routed<Output>`, so `polyoxide_core::query_setters!`'s default target writes it, and has `pub async fn send(self) -> Result<Output, PredictionsError>`.
- **`throttle.rs`** (`mod throttle`):
  - `pub(crate) const BASIC_READ_CAPACITY: u32 = 600;` and `pub(crate) const BASIC_READ_REFILL: u32 = 200;`
  - `#[derive(Debug, Clone)] pub(crate) struct PredictionsThrottle`, with `pub(crate) fn new() -> Self` (`CapacityBucket::provisional(READ, BASIC_READ_CAPACITY, BASIC_READ_REFILL, hold)`, `hold = Hold::unbounded()`), `pub(crate) fn bucket(&self) -> &polyoxide_core::CapacityBucket`, and `impl polyoxide_core::Throttle`.
  - `#[derive(Debug, Default)] pub(crate) struct RouteCosts`, with `pub(crate) fn cost(&self, route: &Route) -> u32` (the override, else `DEFAULT_COST`) and `pub(crate) fn set(&self, route: &Route, cost: u32)`. A `std::sync::RwLock` keyed by template holds the overrides.
  - The stub Story 1 writes exactly, and Story 4 replaces as a whole with the confirmation:

        impl Inner {
            /// Runs before every send.
            pub(crate) async fn before_send(&self) {}
        }

- **`error.rs`:**
  - `#[derive(Debug, thiserror::Error)] #[non_exhaustive] pub enum PredictionsError { #[error(transparent)] Api(polyoxide_core::ApiError), #[error(transparent)] Venue(VenueError) }`.
  - `#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] #[error("Kalshi answered {status}: {message}")] pub struct VenueError { pub status: u16, pub code: Option<String>, pub message: String, pub details: Option<String>, pub retry_after: Option<std::time::Duration> }`.
  - `impl From<polyoxide_core::ApiError> for PredictionsError`, the one decode (spec D7).
  - `impl polyoxide_venue::Classify` for both.
- **`key.rs`** (`pub mod key`):
  - `pub const VENUE: polyoxide_venue::VenueId` and `pub const PRODUCT: polyoxide_venue::ProductId`.
  - `pub struct Ticker` (`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `Display`), with `pub fn new(text: &str) -> Result<Self, InvalidTicker>` and `pub fn as_str(&self) -> &str`.
  - `pub enum Outcome { Yes, No }` (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`), with `pub fn as_str(self) -> &'static str` (`"yes"`, `"no"`) and `pub fn other(self) -> Self`.
  - `pub struct OutcomeId { pub ticker: Ticker, pub outcome: Outcome }` (`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`).
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct PredictionsKeys;`, with `impl KeySpace` and `type Id = OutcomeId`.
  - `pub fn key(ticker: &Ticker, outcome: Outcome) -> MarketKey`, `pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError>` and `pub fn outcome(key: &MarketKey) -> Result<OutcomeId, KeyError>`.
  - `#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] #[error("{text:?} is not a Kalshi ticker: {reason}")] pub struct InvalidTicker { pub text: String, pub reason: &'static str }`, `Classify` as `InvalidRequest`.
- **`api/health.rs`**: `Health` with `pub async fn ping(&self) -> Result<polyoxide_core::Pong, PredictionsError>`.
- **`api/exchange.rs`**: `ExchangeApi` with `pub fn status(&self) -> GetExchangeStatus`.
- **`api/mod.rs`.** Story 1 writes:

      pub mod exchange;
      pub mod health;

      pub use exchange::{ExchangeApi, GetExchangeStatus};
      pub use health::Health;

  Story 2 replaces it with:

      pub mod account;
      pub mod events;
      pub mod exchange;
      pub mod health;
      pub mod markets;

      pub use account::{AccountApi, GetEndpointCosts};
      pub use events::{EventsApi, GetEvent};
      pub use exchange::{ExchangeApi, GetExchangeStatus};
      pub use health::Health;
      pub use markets::{GetMarket, GetOrderbook, ListMarkets, ListTrades, MarketsApi};

  Story 4 replaces Story 2's with the same block plus `pub mod limits;` directly after `pub mod health;`, and `pub use limits::GetAccountLimits;` directly after `pub use health::Health;`.
- **`types/mod.rs`.** Story 1 writes:

      mod exchange;

      pub use exchange::{ExchangeIndexStatus, ExchangeStatus};

  Story 2 replaces it with:

      mod account;
      mod events;
      mod exchange;
      mod markets;

      pub use account::{EndpointCost, EndpointCosts};
      pub use events::{Event, EventResponse};
      pub use exchange::{ExchangeIndexStatus, ExchangeStatus};
      pub use markets::{
          BookSide, Market, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, OrderbookFp,
          OrderbookResponse, OutcomeSide, PriceLevel, PriceRange, Trade, TradesPage,
      };

  Story 4 replaces Story 2's with the same block plus `mod limits;` directly after `mod exchange;`, and `pub use limits::{AccountLimits, ApiUsageGrant, BucketLimit};` directly after the `pub use exchange` line.
- **`ExchangeStatus`** has `pub exchange_active: bool`, `pub trading_active: bool`, `pub intra_exchange_transfers_active: Option<bool>`, `pub exchange_estimated_resume_time: Option<String>` and `pub exchange_index_statuses: Vec<ExchangeIndexStatus>` (`#[serde(default)]`). **`ExchangeIndexStatus`** has `pub description: String`, `pub exchange_active: bool`, `pub exchange_index: u32`, `pub intra_exchange_transfers_active: bool` and `pub trading_active: bool`.
- **The `Classify` assertion in `lib.rs`.** Story 1 writes:

      #[cfg(feature = "predictions")]
      const _: fn() = || {
          fn is<T: polyoxide_venue::Classify>() {}
          is::<predictions::PredictionsError>();
          is::<predictions::VenueError>();
          is::<predictions::key::InvalidTicker>();
      };

  Story 4 replaces it with the same block plus `is::<predictions::CredentialsError>();` directly after the `InvalidTicker` line. Story 6 replaces Story 4's with the same block plus these two lines directly after the `CredentialsError` line:

          #[cfg(feature = "predictions-ws")]
          is::<predictions::ws::PredictionsWsError>();

- **Anchors in `polyoxide-kalshi/Cargo.toml`**, lines Story 1 writes verbatim:
  - `[dependencies]` and `[dev-dependencies]`, each a header line of its own. Story 6 inserts its `predictions-ws` feature directly before `[dependencies]`, and its two optional dependencies directly before `[dev-dependencies]`.
  - `mockito = { workspace = true }`, a line of `[dev-dependencies]`. Stories 2, 3 and 6 each insert their dev-dependency directly after it.
  - `# Registration, read by scripts/gen_registry.py; run it with --write after an edit.`, the one line directly above `[package.metadata.polyoxide]`. Stories 2, 3, 4 and 6 each insert their `[[test]]` stanzas directly before it.
  - `[package.metadata.docs.rs]`, followed by `features = ["predictions"]`, the file's last two lines. Stories 2, 3 and 4 each insert their `[package.metadata.polyoxide.live.<target>]` table directly before `[package.metadata.docs.rs]`. Story 6 replaces the two lines with its live table, then `[package.metadata.docs.rs]`, then `features = ["predictions", "predictions-ws"]`.
  - The `predictions` feature turns on every optional dependency the REST module uses, from Story 1: `dep:polyoxide-core`, `polyoxide-venue/decimal`, `polyoxide-venue/market`, `dep:base64`, `dep:chrono`, `dep:ring`, `dep:rust_decimal`, `dep:serde`, `dep:serde_json`, `dep:thiserror`, `dep:tokio` (with its `time` and `sync` features), `dep:tracing` and `dep:url`. No later story edits it.
- **The root `Cargo.toml`.** Story 1 inserts `polyoxide-kalshi = { path = "polyoxide-kalshi" }` directly after the `polyoxide-core = …` pin, and `ring = "0.17"` directly after the `rustls = …` pin. Story 2 replaces the line `ring = "0.17"` with that line and then its YAML parser's pin.
- **The anchor in `docs/specs/kalshi/OBSERVED.md`**: its last line, `<!-- capture findings go above this line -->`. Stories 4, 6 and 7 each insert a section directly above it.
- **The touch-set gate.**
  - `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh` holds exactly
    `{ git diff --name-only "$(git merge-base main HEAD)"; git ls-files --others --exclude-standard; } | sort -u | grep -v -E -f "$(dirname "$0")/epic-8-touch-set.txt"; test $? -eq 1`.
    Its first two lines are `#!/usr/bin/env bash` and `cd "$(git rev-parse --show-toplevel)" || exit 2`, so every path is the repository's. `set -o pipefail` is not set, so `test` reads `grep`'s status.
  - `epic-8-touch-set.txt` holds one extended regex per line, these and no others:

        ^Cargo\.toml$
        ^Cargo\.lock$
        ^CLAUDE\.md$
        ^README\.md$
        ^SELF-HEALING\.md$
        ^docs/specs/INDEX\.md$
        ^\.github/workflows/ci\.yml$
        ^\.github/workflows/nightly-behavioral\.yml$
        ^\.github/workflows/nightly-schema\.yml$
        ^\.github/scripts/tests/test_ci_workflow\.py$
        ^\.github/scripts/tests/test_dep_allowlists\.py$
        ^polyoxide-kalshi/
        ^docs/specs/kalshi/
        ^scripts/capture_kalshi_[a-z_]+\.py$
        ^ci/dep-allowlists/polyoxide-kalshi(@[a-z0-9,-]+)?\.txt$
        ^polyoxide-ws/src/test_server/(mod|tests)\.rs$
        ^docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design\.md$
        ^docs/plans/2026-10-18-epic-8-kalshi-walking-skeleton\.md$
        ^_bmad-output/specs/
        ^_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8\.md$
        ^_bmad-output/implementation-artifacts/deferred-work\.md$

- **CLAUDE.md, by paragraph.** No two stories edit one paragraph:
  - Story 1 edits the overview (`Polyoxide is a multi-venue Rust SDK toolkit`), the CI paragraph (`CI runs nine jobs`), the header-pins sentence (`Each of those six HTTP modules`), the mock-suites paragraph (`Mock HTTP tests use mockito`) and Module Organization's first paragraph (`Most crates follow a consistent layout`).
  - Story 5 edits the scripted-server paragraph (`**One scripted WebSocket server lives in polyoxide-ws**`).
  - Story 6 edits the live-suites paragraph (`The CLI has live integration tests`), naming all three Kalshi live targets, and Module Organization's WebSocket paragraph (`**WebSocket** support for the CLOB`).

**Story 2 → Stories 3, 4, 6 and 7.**

- **Namespaces and builders.** Each builder has `pub async fn send(self) -> Result<…, PredictionsError>`, and its setters come from `query_setters!`:
  - `MarketsApi`:
    - `pub fn list(&self) -> ListMarkets`, with setters `status(MarketStatusFilter)`, `event_ticker(impl Into<String>)`, `series_ticker(impl Into<String>)`, `limit(u32)` and `cursor(impl Into<String>)`, sending `MarketsPage`;
    - `pub fn get(&self, ticker: &Ticker) -> GetMarket`, sending `MarketResponse`;
    - `pub fn orderbook(&self, ticker: &Ticker) -> GetOrderbook`, with setter `depth(u32)`, sending `OrderbookResponse`;
    - `pub fn trades(&self) -> ListTrades`, with setters `ticker(&Ticker)`, `limit(u32)`, `cursor(impl Into<String>)`, `min_ts(i64)` and `max_ts(i64)`, sending `TradesPage`.
  - `EventsApi`: `pub fn get(&self, event_ticker: &str) -> GetEvent`, with setter `with_nested_markets(bool)`, sending `EventResponse`.
  - `AccountApi`: `pub fn endpoint_costs(&self) -> GetEndpointCosts`, sending `EndpointCosts`.
- **Types**, each deriving `Debug`, `Clone`, `PartialEq`, `serde::Deserialize` and `serde::Serialize`, every field `pub`. Money and counts are `rust_decimal::Decimal` through `rust_decimal::serde::str`. The fields other stories read:
  - **Markets.** `Market` has `ticker: String`, `event_ticker: String`, `status: MarketStatus`, `yes_sub_title: String`, `no_sub_title: String`, `yes_bid_dollars: Decimal`, `yes_ask_dollars: Decimal`, `price_ranges: Vec<PriceRange>`, `price_level_structure: String`, `notional_value_dollars: Decimal`, `close_time: String`, `expected_expiration_time: Option<String>`, `latest_expiration_time: String`, `settlement_bounds_type: String` and `exchange_index: Option<u32>`, and every other key the wire carries.
    - `PriceRange { start: Decimal, end: Decimal, step: Decimal }`.
    - `MarketResponse { market: Market }` and `MarketsPage { markets: Vec<Market>, cursor: String }`. The empty cursor ends a walk.
  - **The orderbook.** `OrderbookResponse { orderbook_fp: OrderbookFp }` and `OrderbookFp { yes_dollars: Vec<PriceLevel>, no_dollars: Vec<PriceLevel> }`, both lists `#[serde(default)]`. `PriceLevel { price: Decimal, count: Decimal }` is read from and written as the positional `[price, count]` pair of strings.
  - **Trades.** `Trade` has `trade_id: String`, `ticker: String`, `count_fp: Decimal`, `yes_price_dollars: Decimal`, `no_price_dollars: Decimal`, `taker_outcome_side: OutcomeSide`, `taker_book_side: BookSide`, `created_time: String` and `is_block_trade: bool`, and every other key the wire carries. `TradesPage { trades: Vec<Trade>, cursor: String }`.
  - **Events.** `Event` has `event_ticker: String`, `series_ticker: String`, `sub_title: String`, `title: String`, `collateral_return_type: String`, `mutually_exclusive: bool` and `markets: Vec<Market>` (`#[serde(default)]`), and every other key the wire carries. `EventResponse { event: Event, markets: Vec<Market> }`, `markets` `#[serde(default)]`.
  - **Endpoint costs.** `EndpointCosts { default_cost: u32, endpoint_costs: Vec<EndpointCost> }` and `EndpointCost { method: String, path: String, cost: u32 }`.
  - **Enums.**
    - `MarketStatus` is a `polyoxide_venue::open_enum!`: `Initialized` `"initialized"`, `Inactive`, `Active`, `Closed`, `Determined`, `Disputed`, `Amended` and `Finalized`, each its lowercase name.
    - `OutcomeSide` is an `open_enum!` of `Yes` `"yes"` and `No` `"no"`, and `BookSide` one of `Bid` `"bid"` and `Ask` `"ask"`.
    - `MarketStatusFilter` is a `polyoxide_venue::wire_enum!`: `Unopened` `"unopened"`, `Open` `"open"`, `Closed` `"closed"` and `Settled` `"settled"`.
- **Fixtures** (`fixtures!("predictions")`): Story 2's twelve JSON files, by the names in the file structure, and `PROVENANCE.md`.
- **`tests/common/mod.rs`**: `pub async fn two_sided_market(client: &Predictions) -> Market`, which picks an open market with a two-sided book (spec §4.6) and is an `environmental(..)` skip when none qualifies. Story 3's live target uses it by `mod common;`.

**Story 3 → Story 7.** `predictions::venue::{InstrumentExt, TradeExt, EventGroupExt}`. Each derives `Debug`, `Clone`, `PartialEq` and `Default`, is `#[non_exhaustive]`, and has every field an `Option`, the fields of spec §5.2 to §5.4.

**Story 4 → Story 6.**

- **`pub struct Credentials`**: `Clone`, with a hand-written `Debug` that prints the key id and `Secret`'s redaction.
  - `pub fn from_pem(key_id: impl Into<String>, pem: &str) -> Result<Self, CredentialsError>`.
  - `pub fn from_pem_file(key_id: impl Into<String>, path: impl AsRef<std::path::Path>) -> Result<Self, CredentialsError>`.
  - `pub fn key_id(&self) -> &str`.
  - `pub(crate) fn sign_request(&self, method: &str, path: &str) -> Result<SignedHeaders, CredentialsError>`, which signs `timestamp_ms + method + path`.
- **`pub(crate) struct SignedHeaders`** has `pub(crate) key: String`, `pub(crate) timestamp: String` and `pub(crate) signature: String`, and `pub(crate) fn pairs(&self) -> [(&'static str, &str); 3]`, in the order key, timestamp, signature.
- **Header names**, in `auth.rs`: `pub(crate) const ACCESS_KEY: &str = "KALSHI-ACCESS-KEY";`, `pub(crate) const ACCESS_TIMESTAMP: &str = "KALSHI-ACCESS-TIMESTAMP";` and `pub(crate) const ACCESS_SIGNATURE: &str = "KALSHI-ACCESS-SIGNATURE";`.
- **`#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)] #[non_exhaustive] pub enum CredentialsError`**, each variant `Classify` as `InvalidRequest`:
  - `Pem { reason: String }`: `the key is not valid PEM: {reason}`;
  - `Label { label: String }`: `a PEM labelled {label} is not a supported key; Kalshi keys are Ed25519 or RSA`;
  - `Key { reason: String }`: `the PEM holds no Ed25519 or RSA key ring can sign with: {reason}`;
  - `File { path: String, reason: String }`: `could not read the key file {path}: {reason}`;
  - `Sign { reason: String }`: `signing failed: {reason}`.
- **`impl PredictionsBuilder { pub fn credentials(self, credentials: Credentials) -> Self }`**, in `auth.rs`.
- **`AccountApi::limits(&self) -> GetAccountLimits`**, in `api/limits.rs`, sending `AccountLimits`. Without credentials it sends nothing and fails with exactly `PredictionsError::Api(ApiError::Validation("account().limits() needs credentials: build the client with PredictionsBuilder::credentials".into()))`.
- **Types:**
  - `AccountLimits { usage_tier: String, read: BucketLimit, write: BucketLimit, grants: Vec<ApiUsageGrant> }`;
  - `BucketLimit { refill_rate: u32, bucket_capacity: u32 }`;
  - `ApiUsageGrant { exchange_instance: String, level: String, expires_ts: Option<i64>, source: String }`.

  Each derives as Story 2's do.
- **`tests/fixtures/predictions/signing/vectors.json`**: `{"message": …, "ed25519": {"pkcs8_pem", "public_key_hex", "signature_b64"}, "rsa": {"pkcs1_pem", "pkcs8_pem", "public_key_pkcs1_der_b64", "pss_signature_b64"}}`. The keys are throwaway, made by `scripts/capture_kalshi_account.py`'s `signing` step. Story 6 builds its `Credentials` from `ed25519.pkcs8_pem` and verifies handshakes with `ed25519.public_key_hex`.
- **`scripts/capture_kalshi_account.py`**: `def signed_headers(method: str, path: str) -> dict[str, str]`, which reads `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` and returns the three headers. Story 6's capture imports it.

**Story 5 → Story 6.** `polyoxide_ws::test_server::ScriptedServer::handshake_headers(&self) -> Vec<Vec<(String, String)>>`: each accepted handshake's request headers, names lowercase, in accept order.

**Story 6's items**, which Story 7 names in prose: `predictions::ws::{PredictionsWsBuilder, SupervisedPredictionsWs, PredictionsProtocol, PredictionsMembership, Change, Event, OrderbookSnapshot, OrderbookDelta, DisconnectReason, PredictionsWsError, PRODUCTION_WS, DEMO_WS}` (spec §8).

**Live targets** (each `[[test]]` `required-features` equals its table's `features`):

| Target | Story | `features` | `secrets` |
|---|---|---|---|
| `live_predictions` | 2 | `["predictions"]` | `[]` |
| `live_predictions_venue` | 3 | `["predictions"]` | `[]` |
| `live_predictions_account` | 4 | `["predictions"]` | `["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"]` |
| `live_predictions_ws` | 6 | `["predictions-ws"]` | `["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"]` |

Each has `suite = "predictions"` and `timeout = 15`.

## Stories

- **Story 1** — Register `polyoxide-kalshi` by the recipe (Story 8.1, and 8.2's exchange status).

  Creates the crate: a `members` line, the pins, `[package.metadata.polyoxide]` (venue `kalshi`, product `predictions`, mirror `kalshi`), and the ids test. It downloads the OpenAPI and AsyncAPI mirrors byte for byte, writes `OBSERVED.md` from the spec's probes, and adds the `kalshi` mirrors table. It writes the key space (spec D8), the client and its builder (D4), the whole route table with its doc-hidden accessors, and the provisional Read throttle (D5) with the `before_send` stub Story 4 replaces. It writes `PredictionsError` with its one decode of Kalshi's three error shapes (D7), and `health().ping()` and `exchange().status()`. It makes the CI wiring the spec pre-approves (§3.7: two header-pin lines, the full-pin assertion scoped to the umbrella's crates, a `PER_FEATURE` rule) and Story 1's CLAUDE.md paragraphs. It writes the touch-set gate every later story ends with, and regenerates the registry regions.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/README.md`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/src/predictions/key.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/api/exchange.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/exchange.rs`, `polyoxide-kalshi/tests/headers_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-kalshi/tests/error_classes_predictions.rs`, `docs/specs/kalshi/openapi.yaml`, `docs/specs/kalshi/asyncapi.yaml`, `docs/specs/kalshi/OBSERVED.md`, `ci/dep-allowlists/polyoxide-kalshi@predictions.txt`, `.github/workflows/ci.yml`, `.github/scripts/tests/test_ci_workflow.py`, `.github/scripts/tests/test_dep_allowlists.py`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`, `CLAUDE.md`, `README.md`, `docs/specs/INDEX.md`, `.github/workflows/nightly-schema.yml`
  - **Interfaces:** exposes everything under "Story 1 → Stories 2 to 7" in the names contract (the module blocks, `Predictions`, `Inner`, `PredictionsBuilder`, the route table, `Routed`, the throttle and its stub, the errors, the keys, the anchors and the touch-set gate); uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py tests/test_dep_allowlists.py tests/test_module_readmes.py tests/test_classify_coverage.py tests/test_gen_registry.py tests/test_venue_gate.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (a new crate's registration, and CI gates changed)
  - **Reading:** spec §1, §2 (D1 to D8, D20, D23, D24), §3, §10, §11; `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/key.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/tests/headers_usdm.rs`, `polyoxide-test-support/tests/token_cost_throttle.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/hooks.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/error.rs`, `polyoxide-venue/src/key.rs:290-410`, `Cargo.toml:1-60,77-267`, `scripts/gen_registry.py:90-130,360-460,530-640`, `.github/workflows/ci.yml:106-166`, `.github/scripts/tests/test_ci_workflow.py:155-320`, `.github/scripts/tests/test_dep_allowlists.py:40-260`; CLAUDE.md's paragraphs named in the contract

- **Story 2** — The public REST surface (Story 8.2).

  Adds the markets, events and endpoint-costs namespaces and their builders over Story 1's route table, every query setter from `query_setters!`, and the types of every key the wire carries (spec §4). The capture script reads production unsigned and writes the fixtures and their provenance. The suites cover the routes on a mock server, the setters' exact pairs, wire agreement both ways, and spec agreement against the YAML mirror through a dev-only parser (§4.7). `live_predictions` runs against production with no secrets: it picks a two-sided market, checks no unmodelled keys, and checks that the route table agrees with `/account/endpoint_costs` (D5's drift test). Every route is sent at the compiled cost; the confirmation is Story 4's.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/api/events.rs`, `polyoxide-kalshi/src/predictions/api/account.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/types/events.rs`, `polyoxide-kalshi/src/predictions/types/account.rs`, `scripts/capture_kalshi_predictions.py`, `polyoxide-kalshi/tests/fixtures/predictions/exchange_status.json`, `polyoxide-kalshi/tests/fixtures/predictions/endpoint_costs.json`, `polyoxide-kalshi/tests/fixtures/predictions/market.json`, `polyoxide-kalshi/tests/fixtures/predictions/markets_page.json`, `polyoxide-kalshi/tests/fixtures/predictions/orderbook.json`, `polyoxide-kalshi/tests/fixtures/predictions/orderbook_depth1.json`, `polyoxide-kalshi/tests/fixtures/predictions/trades.json`, `polyoxide-kalshi/tests/fixtures/predictions/event.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_not_found.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_validation.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_unauthenticated.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_rate_limited.json`, `polyoxide-kalshi/tests/fixtures/predictions/PROVENANCE.md`, `polyoxide-kalshi/tests/common/mod.rs`, `polyoxide-kalshi/tests/mock_predictions_routes.rs`, `polyoxide-kalshi/tests/query_setters_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
  - **Interfaces:** exposes the namespaces, builders, setters, types, enums and fixtures under "Story 2 → Stories 3, 4, 6 and 7", and `tests/common/mod.rs`'s `two_sided_market`; uses Story 1's `Inner`, `Routed`, `routes`, `route_templates`, `template_matches`, `DEFAULT_COST`, `Ticker`, `PredictionsError` and the anchors
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (the drift detectors and the types every later story reads)
  - **Reading:** spec §1.3, §2 (D5, D7, D18, D19), §4, §10, §11; `docs/specs/kalshi/openapi.yaml` (the `Market`, `PriceRange`, `GetMarketOrderbookResponse`, `Trade`, `EventData`, `GetAccountEndpointCostsResponse` schemas and the routes of §4.1); `polyoxide-binance/src/usdm/api/market.rs`, `polyoxide-binance/src/usdm/types.rs`, `polyoxide-binance/tests/mock_usdm.rs:1-120`, `polyoxide-binance/tests/query_setters_usdm.rs`, `polyoxide-binance/tests/wire_agreement_usdm.rs`, `polyoxide-binance/tests/live_usdm.rs:1-140`, `polyoxide-polymarket/tests/spec_agreement_perps.rs:1-120`, `polyoxide-polymarket/tests/common/mod.rs`, `polyoxide-polymarket/src/perps/types.rs` (its positional levels), `polyoxide-venue/src/positional.rs`, `polyoxide-venue/src/wire.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/src/agreement/mod.rs`, `polyoxide-test-support/src/openapi.rs`, `polyoxide-test-support/src/fixtures.rs`, `scripts/capture_common.py`, `scripts/capture_binance_usdm.py`

- **Story 3** — Kalshi market data through the traits (Story 8.3).

  Implements `MarketData`, `Trades` and `EventGroups` on `Predictions` (spec §5), keyed by Story 1's key space. One conversion gives each outcome its book: a NO bid at *q* is a YES ask at *1 − q* (D9). `instrument` turns a 404 into `NotListed`, while `quote` and `book` of an unknown ticker are empty (D11). Instruments are `SizeUnit::Shares` (D12). Times are read with `chrono` (D25). Kalshi-only facts go in `InstrumentExt`, `TradeExt` and `EventGroupExt`. `venue_predictions` holds each method to the native client's values on Story 2's fixtures. It also runs Story 6.6's generic `best_bid_ask` through the umbrella's example, included by `#[path]`, with the umbrella's dev-dependency on the example's three features (D22). `live_predictions_venue` checks the traits against production, unsigned.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/venue.rs`, `polyoxide-kalshi/tests/venue_predictions.rs`, `polyoxide-kalshi/tests/live_predictions_venue.rs`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
  - **Interfaces:** exposes `impl MarketData`, `impl Trades` and `impl EventGroups` for `Predictions`, and `predictions::venue::{InstrumentExt, TradeExt, EventGroupExt}`; uses Story 1's keys and module block, Story 2's namespaces, types and fixtures, and `tests/common/mod.rs`
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (the YES/NO conversion every consumer's book depends on)
  - **Reading:** spec §2 (D8 to D12, D22, D25), §5; `polyoxide-binance/src/usdm/venue.rs`, `polyoxide-binance/tests/venue_usdm.rs:1-120`, `polyoxide-polymarket/src/clob/venue.rs:120-200,440-520`, `polyoxide-polymarket/src/perps/venue.rs:150-260`, `polyoxide-venue/src/market.rs`, `polyoxide-venue/src/records.rs`, `polyoxide-venue/src/extensions.rs`, `polyoxide-venue/src/class.rs:140-220`, `polyoxide/examples/venue_neutral.rs`, `polyoxide/tests/venue_neutral.rs:1-130`, `polyoxide/Cargo.toml:55-95`

- **Story 4** — Signed requests and the first-use confirmation (Story 8.2).

  Adds `Credentials` over `ring` (spec §6, D2, D3). PEM detection reads the parsed key: PKCS#1 RSA, or PKCS#8 tried as Ed25519 and then as RSA. Refusals never quote key material. The authenticator signs `ts_ms + METHOD + path` on every attempt, and `PredictionsBuilder::credentials` installs it, so a keyed client signs every route but the ping. It replaces Story 1's `before_send` stub with the whole confirmation (D5): costs from `/account/endpoint_costs`; with a key, `/account/limits` resizing the bucket; and always ending confirmed, with a zero refill ignored. `account().limits()` refuses without a key. The account capture script writes the signing vectors from throwaway keys, then, with the owner's demo key, `/account/limits` (if the key is absent, the story stops there and asks). The suites pin the Ed25519 vector, verify RSA-PSS both ways, prove the signed headers on a mock server, and run `live_predictions_account` against the demo host, auth-gated without secrets.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/auth.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/limits.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/limits.rs`, `scripts/capture_kalshi_account.py`, `polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json`, `polyoxide-kalshi/tests/fixtures/predictions/signing/PROVENANCE.md`, `polyoxide-kalshi/tests/fixtures/predictions/account/limits.json`, `polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md`, `polyoxide-kalshi/tests/signing_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions_signed.rs`, `polyoxide-kalshi/tests/live_predictions_account.rs`, `docs/specs/kalshi/OBSERVED.md`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
  - **Interfaces:** exposes everything under "Story 4 → Story 6" (`Credentials`, `SignedHeaders`, the header consts, `CredentialsError`, `PredictionsBuilder::credentials`, `AccountApi::limits`, the limits types, the vectors and `signed_headers`); uses Story 1's `Inner`, `before_send` stub, `RouteCosts`, `PredictionsThrottle::bucket`, `routes`, `template_matches`, `Routed::has_key` and module blocks, and Story 2's `AccountApi`, `EndpointCosts`, `types/mod.rs` and `api/mod.rs` blocks
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py tests/test_classify_coverage.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (signing and secrets)
  - **Reading:** spec §1.3 (keys, rate limits), §2 (D2, D3, D5, D18, D19), §6, §10; `polyoxide-kalshi/src/predictions/throttle.rs` and `request.rs` as Story 1 leaves them (names contract), `polyoxide-polymarket/src/clob/authenticator.rs`, `polyoxide-polymarket/src/clob/error.rs:130-150`, `polyoxide-core/src/capacity.rs`, `polyoxide-test-support/tests/token_cost_throttle.rs`, `polyoxide-test-support/src/creds.rs`, `polyoxide-test-support/src/tag.rs`, `polyoxide-venue/src/secret.rs`, `scripts/capture_common.py`; ring 0.17's `signature` module (`Ed25519KeyPair::from_pkcs8_maybe_unchecked`, `RsaKeyPair::{from_der, from_pkcs8, sign}`, `RSA_PSS_SHA256`, `RSA_PSS_2048_8192_SHA256`, `UnparsedPublicKey`)

- **Story 5** — The kit's scripted server records handshake headers (Story 8.4's prerequisite).

  Adds `ScriptedServer::handshake_headers()` to `polyoxide-ws`'s doc-hidden test server (spec §7, D17): the `accept_hdr_async` callback copies the request's headers beside the path it already records, and `serve()` pushes them after `paths`. A refused upgrade records nothing. The test is appended after the file's last test: two connections with a header of different values are recorded in order. It names no venue. CLAUDE.md's scripted-server paragraph lists the new recorder. This is the foundation edit the owner approved (A8-4).

  - **Developer:** junior-dev
  - **Files:** `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/tests.rs`, `CLAUDE.md`
  - **Interfaces:** exposes `polyoxide_ws::test_server::ScriptedServer::handshake_headers`; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-ws --features test-server --lib test_server && python3 scripts/venue_gate.py && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (a foundation crate)
  - **Reading:** spec §7; `polyoxide-ws/src/test_server/mod.rs:230-340,515-580`, `polyoxide-ws/src/test_server/tests.rs` (its last 80 lines and lines 700-760); CLAUDE.md's scripted-server paragraph

- **Story 6** — A supervised Kalshi socket with a signed handshake (Story 8.4).

  Adds `predictions-ws` and `predictions::ws` (spec §8, D13 to D17): `PredictionsProtocol` on the kit's `Supervisor`, whose handshake is signed afresh on every attempt. One `subscribe` to `orderbook_delta` with `use_yes_price: true` is replayed on each connection, and changes go out as `update_subscription` `add_markets` and `delete_markets` on the live `sid`. Snapshots and deltas decode into YES bids and asks, `seq` is checked per `sid` (its answers included), and a gap reconnects. The client pings every 10 s and counts every frame as liveness. Socket errors go through `impl_ws_classification!`, with Kalshi's error codes mapped as D16 says. The scripted suite proves a fresh signature on every attempt through Story 5's `handshake_headers()`, replay after a reconnect, the membership frames, a gap's reconnect and the recovery rows. The WebSocket capture (owner's demo key; stop and ask without it) writes the frames for the wire agreement and records its findings in `OBSERVED.md`. `live_predictions_ws` runs against the demo host, auth-gated without secrets. It adds the `@predictions-ws` allowlist and its `MUST_BUILD` row, and Story 6's CLAUDE.md paragraphs.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-kalshi/Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `scripts/capture_kalshi_predictions_ws.py`, `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/snapshot.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/deltas.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/ok.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/unsubscribed.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/error.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt`, `.github/scripts/tests/test_dep_allowlists.py`, `docs/specs/kalshi/OBSERVED.md`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
  - **Interfaces:** exposes `predictions::ws`'s items (names contract, Story 6); uses Story 1's keys, module block, assertion and anchors, Story 4's `Credentials::sign_request`, `SignedHeaders`, the header consts, the vectors and `signed_headers`, and Story 5's `handshake_headers`
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_live_registry.py tests/test_ci_workflow.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (a supervised socket carrying credentials)
  - **Reading:** spec §1.3 (the socket), §2 (D13 to D17, D19), §8, §11; `docs/specs/kalshi/asyncapi.yaml` (the `subscribe`, `update_subscription`, `unsubscribe`, `subscribed`, `ok`, `unsubscribed`, `error`, `orderbook_snapshot` and `orderbook_delta` schemas, and the keep-alive); `polyoxide-polymarket/src/perps/ws/protocol.rs`, `polyoxide-polymarket/src/perps/ws/builder.rs`, `polyoxide-polymarket/src/perps/ws/scripted.rs`, `polyoxide-polymarket/src/perps/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/tests/supervision_usdm.rs:1-80`, `polyoxide-ws/src/supervisor/mod.rs:1-500`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs`, `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/helpers.rs`, `polyoxide-venue/src/ws_table.rs`, `scripts/capture_polymarket_perps_ws.py`, `.github/scripts/tests/test_dep_allowlists.py:90-140`

- **Story 7** — The guide, validated (Story 8.5).

  Runs the audit of spec §9.1 over the epic's diff since `git merge-base main HEAD`: the touch-set gate passes, every changed hunk of the files with generated regions lies inside a region, the only foundation change is Story 5's, and `polyoxide-kalshi` defines none of the duplication inventory's helpers. It writes `spine-amendments/epic-8.md` in `epic-6.md`'s format, with A8-1 to A8-10 and any gap the run met. It renames the trading mapping's keys to `kalshi.predictions:…` and its size unit to `SizeUnit::Shares` (D21). It adds §9.5's deferred-work entries. Documents only: no code.

  - **Developer:** junior-dev
  - **Files:** `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md`, `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `docs/specs/kalshi/OBSERVED.md`
  - **Interfaces:** uses the names of Stories 1 to 6 in prose; exposes nothing
  - **Verify:** `python3 scripts/gen_registry.py --check && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh && git diff --check HEAD~1 HEAD`
  - **Review:** light (planning documents; no code)
  - **Reading:** spec §1.4, §2, §9; `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`, `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md`, `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/implementation-artifacts/deferred-work.md:400-420`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->
