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
- `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md` and `.github/workflows/nightly-schema.yml` (regenerated).
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
- `docs/specs/kalshi/OBSERVED.md` (modify, by its anchor): the capture's findings.
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
              health: api::Health,
              /// The exchange's status.
              exchange: api::ExchangeApi,
          }

  Story 2 replaces it with:

          polyoxide_core::namespaces! { inner;
              /// Liveness: a ping of the exchange-status route.
              health: api::Health,
              /// The exchange's status.
              exchange: api::ExchangeApi,
              /// Markets, their order books and their public trades.
              markets: api::MarketsApi,
              /// Events and their markets.
              events: api::EventsApi,
              /// The account's limits and the routes' token costs.
              account: api::AccountApi,
          }

  Each namespace is named by its path through `api` (the macro takes a `path`), so `mod.rs` holds no `use` line for the namespaces and no story edits one. Each namespace struct is `#[derive(Debug, Clone)] pub struct <Name> { pub(crate) inner: std::sync::Arc<Inner> }`. The `Debug` derive needs `Inner: Debug`, which its hand-written impl gives.
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

  - **Item order in `throttle.rs`.** The stub is the file's last item but one: Story 1's inline tests, in one `#[cfg(test)] mod tests` directly after the stub, end the file (with no inline tests, the stub ends it). Story 4 replaces the stub with its `impl Inner` block followed by its own inline tests in `#[cfg(test)] mod confirmation_tests`, so the two test modules never share a name and `mod tests` stays the file's last item (clippy's `items_after_test_module`).

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
          BookSide, Market, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, MveFilter,
          OrderbookFp, OrderbookResponse, OutcomeSide, PriceLevel, PriceRange, Trade, TradesPage,
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
  - `[package.metadata.docs.rs]`, followed by `features = ["predictions"]`, the file's last two lines. Stories 2, 3, 4 and 6 each insert their `[package.metadata.polyoxide.live.<target>]` table directly before `[package.metadata.docs.rs]`. Story 6 also replaces `features = ["predictions"]` with `features = ["predictions", "predictions-ws"]` in the task that adds the `predictions-ws` feature, since `test_ci_workflow.py`'s docs.rs test fails while the two disagree.
  - The `predictions` feature turns on every optional dependency the REST module uses, from Story 1: `dep:polyoxide-core`, `polyoxide-venue/decimal`, `polyoxide-venue/market`, `dep:base64`, `dep:chrono`, `dep:ring`, `dep:rust_decimal`, `dep:serde`, `dep:serde_json`, `dep:thiserror`, `dep:tokio` (with its `time` and `sync` features), `dep:tracing` and `dep:url`. No later story edits it.
- **The root `Cargo.toml`.** Story 1 inserts `polyoxide-kalshi = { path = "polyoxide-kalshi" }` directly after the `polyoxide-core = …` pin, and `ring = "0.17"` directly after the `rustls = …` pin. Story 2 replaces the line `ring = "0.17"` with that line and then its YAML parser's pin.
- **The anchor in `docs/specs/kalshi/OBSERVED.md`**: its last line, `<!-- capture findings go above this line -->`. Stories 2, 4, 6 and 7 each insert a section directly above it.
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

- **CLAUDE.md, by paragraph.** No two stories edit one paragraph, but for one named text:
  - Story 1 edits the overview (`Polyoxide is a multi-venue Rust SDK toolkit`), the CI paragraph (`CI runs nine jobs`), the header-pins sentence (`Each of those six HTTP modules`), the mock-suites paragraph (`Mock HTTP tests use mockito`) and Module Organization's first paragraph (`Most crates follow a consistent layout`).
  - **The named text in the CI paragraph.** Story 1's clause for `polyoxide-kalshi`'s allowlist rule (D23) ends with exactly ``(its `@predictions` file exists)``, the only occurrence in CLAUDE.md. Story 6 replaces that text, and nothing else in the CI paragraph, with ``(its `@predictions` and `@predictions-ws` files exist, and `@predictions-ws` must hold `polyoxide-ws`)``.
  - Story 5 edits the scripted-server paragraph (`**One scripted WebSocket server lives in polyoxide-ws**`).
  - Story 6 edits the live-suites paragraph (`The CLI has live integration tests`), naming all four Kalshi live targets of the table below, and Module Organization's WebSocket paragraph (`**WebSocket** support for the CLOB`).

**Story 2 → Stories 3, 4, 6 and 7.**

- **Namespaces and builders.** Each builder has `pub async fn send(self) -> Result<…, PredictionsError>`, and its setters come from `query_setters!`:
  - `MarketsApi`:
    - `pub fn list(&self) -> ListMarkets`, with setters `status(MarketStatusFilter)`, `event_ticker(impl Into<String>)`, `series_ticker(impl Into<String>)`, `mve_filter(MveFilter)`, `limit(u32)` and `cursor(impl Into<String>)`, sending `MarketsPage`. The capture and `two_sided_market` page `status(MarketStatusFilter::Open)` with `mve_filter(MveFilter::Exclude)` (spec §4.4, §4.6);
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
    - `MarketStatusFilter` is a `polyoxide_venue::wire_enum!`: `Unopened` `"unopened"`, `Open` `"open"`, `Paused` `"paused"`, `Closed` `"closed"` and `Settled` `"settled"`.
    - `MveFilter` is a `polyoxide_venue::wire_enum!`: `Only` `"only"` and `Exclude` `"exclude"`.
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
- **`tests/fixtures/predictions/signing/vectors.json`**: `{"message": …, "ed25519": {"pkcs8_body", "public_key_hex", "signature_b64"}, "rsa": {"pkcs1_body", "pkcs8_body", "public_key_pkcs1_der_b64", "pss_signature_b64"}}`. The keys are throwaway, made by `scripts/capture_kalshi_account.py`'s `signing` step.
  - Each `*_body` is a PEM's base64 lines between its armour lines, joined by `\n`, with no armour line, so no push protection can refuse the file (spec §6.4). A test armours it as `format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")`, with `label` `PRIVATE KEY` for a `pkcs8_body` and `RSA PRIVATE KEY` for a `pkcs1_body`.
  - Story 6 builds its `Credentials` from `ed25519.pkcs8_body`, armoured as `PRIVATE KEY`, and verifies handshakes with `ed25519.public_key_hex`.
- **`#[doc(hidden)] pub fn sign_message(&self, message: &[u8]) -> Result<String, CredentialsError>`** on `Credentials`: the padded `STANDARD` base64 signature over `message`, so an integration suite can sign the vectors' fixed message (`sign_request` is `pub(crate)`).
- **`scripts/capture_kalshi_account.py`**: `def signed_headers(method: str, path: str) -> dict[str, str]`, which reads `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` and returns the three headers. Story 6's capture imports it, so importing the module runs nothing: its steps run only under `if __name__ == "__main__":`. Story 6's script declares `cryptography` in its own PEP 723 header, since the import needs it.

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

**Design stage:** /flow-draft session 10ca562f-2d8b-4386-ac6f-5bbb6c061eb4, 2026-10-18T10:22:21Z to 2026-10-18T12:16:29Z UTC: $86.66 logged, $97.33 with estimated thinking; 18,305 plan lines, $4.73 per 1,000 lines.

## Stories

- **Story 1** — Register `polyoxide-kalshi` by the recipe (Story 8.1, and 8.2's exchange status).

  Creates the crate: a `members` line, the pins, `[package.metadata.polyoxide]` (venue `kalshi`, product `predictions`, mirror `kalshi`), and the ids test. It downloads the OpenAPI and AsyncAPI mirrors byte for byte, writes `OBSERVED.md` from the spec's probes, and adds the `kalshi` mirrors table. It writes the key space (spec D8), the client and its builder (D4), the whole route table with its doc-hidden accessors, and the provisional Read throttle (D5) with the `before_send` stub Story 4 replaces. It writes `PredictionsError` with its one decode of Kalshi's three error shapes (D7), and `health().ping()` and `exchange().status()`. It makes the CI wiring the spec pre-approves (§3.7: two header-pin lines, the full-pin assertion scoped to the umbrella's crates, a `PER_FEATURE` rule) and Story 1's CLAUDE.md paragraphs. It writes the touch-set gate every later story ends with, and regenerates the registry regions.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/README.md`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/src/predictions/key.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/api/exchange.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/exchange.rs`, `polyoxide-kalshi/tests/headers_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-kalshi/tests/error_classes_predictions.rs`, `docs/specs/kalshi/openapi.yaml`, `docs/specs/kalshi/asyncapi.yaml`, `docs/specs/kalshi/OBSERVED.md`, `ci/dep-allowlists/polyoxide-kalshi@predictions.txt`, `.github/workflows/ci.yml`, `.github/scripts/tests/test_ci_workflow.py`, `.github/scripts/tests/test_dep_allowlists.py`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`, `CLAUDE.md`, `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-schema.yml`
  - **Interfaces:** exposes everything under "Story 1 → Stories 2 to 7" in the names contract (the module blocks, `Predictions`, `Inner`, `PredictionsBuilder`, the route table, `Routed`, the throttle and its stub, the errors, the keys, the anchors and the touch-set gate); uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py tests/test_dep_allowlists.py tests/test_module_readmes.py tests/test_classify_coverage.py tests/test_gen_registry.py tests/test_venue_gate.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (a new crate's registration, and CI gates changed)
  - **Reading:** spec §1, §2 (D1 to D8, D20, D23, D24), §3, §10, §11; `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/lib.rs`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/key.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/tests/headers_usdm.rs`, `polyoxide-test-support/tests/token_cost_throttle.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/hooks.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/error.rs`, `polyoxide-venue/src/key.rs:290-410`, `Cargo.toml:1-60,77-267`, `scripts/gen_registry.py:90-130,360-460,530-640`, `.github/workflows/ci.yml:106-166`, `.github/scripts/tests/test_ci_workflow.py:155-320`, `.github/scripts/tests/test_dep_allowlists.py:40-260`; CLAUDE.md's paragraphs named in the contract

- **Story 2** — The public REST surface (Story 8.2).

  Adds the markets, events and endpoint-costs namespaces and their builders over Story 1's route table, every query setter from `query_setters!`, and the types of every key the wire carries (spec §4). The capture script reads production unsigned and writes the fixtures and their provenance. The suites cover the routes on a mock server, the setters' exact pairs, wire agreement both ways, and spec agreement against the YAML mirror through a dev-only parser (§4.7). `live_predictions` runs against production with no secrets: it picks a two-sided market, checks no unmodelled keys, and checks that the route table agrees with `/account/endpoint_costs` (D5's drift test). Every route is sent at the compiled cost; the confirmation is Story 4's.

  - **Developer:** intermediate-dev
  - **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/api/events.rs`, `polyoxide-kalshi/src/predictions/api/account.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/types/events.rs`, `polyoxide-kalshi/src/predictions/types/account.rs`, `scripts/capture_kalshi_predictions.py`, `polyoxide-kalshi/tests/fixtures/predictions/exchange_status.json`, `polyoxide-kalshi/tests/fixtures/predictions/endpoint_costs.json`, `polyoxide-kalshi/tests/fixtures/predictions/market.json`, `polyoxide-kalshi/tests/fixtures/predictions/markets_page.json`, `polyoxide-kalshi/tests/fixtures/predictions/orderbook.json`, `polyoxide-kalshi/tests/fixtures/predictions/orderbook_depth1.json`, `polyoxide-kalshi/tests/fixtures/predictions/trades.json`, `polyoxide-kalshi/tests/fixtures/predictions/event.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_not_found.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_validation.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_unauthenticated.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_rate_limited.json`, `polyoxide-kalshi/tests/fixtures/predictions/PROVENANCE.md`, `polyoxide-kalshi/tests/common/mod.rs`, `polyoxide-kalshi/tests/mock_predictions_routes.rs`, `polyoxide-kalshi/tests/query_setters_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `docs/specs/kalshi/OBSERVED.md`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
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
  - **Interfaces:** exposes `predictions::ws`'s items (names contract, Story 6); uses Story 1's keys, `Predictions`, `DEMO`, module block, assertion, anchors and the CI paragraph's named text, Story 2's `MarketStatusFilter`, `MveFilter` and `tests/common/mod.rs`'s `two_sided_market`, Story 4's `Credentials::sign_request`, `SignedHeaders`, the header consts, the vectors and `signed_headers`, and Story 5's `handshake_headers`
  - **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_live_registry.py tests/test_ci_workflow.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
  - **Review:** deep (a supervised socket carrying credentials)
  - **Reading:** spec §1.3 (the socket), §2 (D13 to D17, D19), §8, §11; `docs/specs/kalshi/asyncapi.yaml` (the `subscribe`, `update_subscription`, `unsubscribe`, `subscribed`, `ok`, `unsubscribed`, `error`, `orderbook_snapshot` and `orderbook_delta` schemas, and the keep-alive); `polyoxide-polymarket/src/perps/ws/protocol.rs`, `polyoxide-polymarket/src/perps/ws/builder.rs`, `polyoxide-polymarket/src/perps/ws/scripted.rs`, `polyoxide-polymarket/src/perps/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/tests/supervision_usdm.rs:1-80`, `polyoxide-ws/src/supervisor/mod.rs:1-486`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs`, `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/helpers.rs`, `polyoxide-venue/src/ws_table.rs`, `scripts/capture_polymarket_perps_ws.py`, `.github/scripts/tests/test_dep_allowlists.py:90-140`

- **Story 7** — The guide, validated (Story 8.5).

  Runs the audit of spec §9.1 over the epic's diff since `git merge-base main HEAD`: the touch-set gate passes, every changed hunk of the files with generated regions lies inside a region, the only foundation change is Story 5's, and `polyoxide-kalshi` defines none of the duplication inventory's helpers. It writes `spine-amendments/epic-8.md` in `epic-6.md`'s format, with A8-1 to A8-10 and any gap the run met. It renames the trading mapping's keys to `kalshi.predictions:…` and its size unit to `SizeUnit::Shares` (D21). It adds §9.5's deferred-work entries. Documents only: no code.

  - **Developer:** junior-dev
  - **Files:** `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md`, `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `docs/specs/kalshi/OBSERVED.md`
  - **Interfaces:** uses the names of Stories 1 to 6 in prose; exposes nothing
  - **Verify:** `python3 scripts/gen_registry.py --check && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh && git diff --check HEAD~1 HEAD`
  - **Review:** light (planning documents; no code)
  - **Reading:** spec §1.4, §2, §9; `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-6.md`, `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md`, `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/implementation-artifacts/deferred-work.md:400-417`

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->

## Story 1: Register `polyoxide-kalshi` by the recipe (Story 8.1, and 8.2's exchange status)

- **Developer:** intermediate-dev
- **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/README.md`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/src/predictions/key.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/api/exchange.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/exchange.rs`, `polyoxide-kalshi/tests/headers_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-kalshi/tests/error_classes_predictions.rs`, `docs/specs/kalshi/openapi.yaml`, `docs/specs/kalshi/asyncapi.yaml`, `docs/specs/kalshi/OBSERVED.md`, `ci/dep-allowlists/polyoxide-kalshi@predictions.txt`, `.github/workflows/ci.yml`, `.github/scripts/tests/test_ci_workflow.py`, `.github/scripts/tests/test_dep_allowlists.py`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`, `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`, `CLAUDE.md`, `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-schema.yml`
- **Interfaces:** exposes everything under "Story 1 → Stories 2 to 7" in the names contract (the module blocks, `Predictions`, `Inner`, `PredictionsBuilder`, the route table, `Routed`, the throttle and its stub, the errors, the keys, the anchors and the touch-set gate); uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py tests/test_dep_allowlists.py tests/test_module_readmes.py tests/test_classify_coverage.py tests/test_gen_registry.py tests/test_venue_gate.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
- **Review:** deep (a new crate's registration, and CI gates changed)

This story creates `polyoxide-kalshi` (`publish = false`) and registers it by the architecture guide's recipe: the manifest and its metadata, Kalshi's two vendored specs and an `OBSERVED.md` of what the host does, the `kalshi` mirrors table, the crate's dependency rule and allowlist, and the CI wiring the spec pre-approves. It then writes the module's foundation for the stories after it: the key space, `PredictionsError` with its one decode of Kalshi's three error shapes, the whole route table with its doc-hidden accessors, the provisional Read throttle with the `before_send` stub that Story 4 replaces, the client and its builder, `health().ping()` and `exchange().status()`; and it writes the touch-set gate every later story ends with. The tasks run in order: 1.1 the gate, 1.2 the registration, 1.3 the keys, 1.4 the errors, 1.5 the status types, 1.6 the route table and the throttle, 1.7 the client, 1.8 the header pin in CI, 1.9 the story's gates. This story writes two landmine shapes, so the supervisor should dispatch its developer with `effort: xhigh`. Task 1.6 sizes a `CapacityBucket` at Kalshi's published Basic tier, capacity and refill both: Kalshi publishes a token bucket with its capacity and the server is one, so the bucket keeps its published burst, the opposite of the window quotas of Cloudflare's hosts, which are never given depth, so no `allow_burst` call or burst setting belongs here and no quota is aimed at its published count. Task 1.4 writes the module's one `From<ApiError>`: every foreign error enters through `ApiError`, and no `From<reqwest::Error>`, `From<serde_json::Error>` or `From<url::ParseError>` joins it. Every Rust block is in the form rustfmt leaves, but where a task's `cargo fmt --all -- --check` prints a diff, run `nix develop --command cargo fmt -p polyoxide-kalshi` and commit the result with that task. Registering the crate in Task 1.2 puts it in `cargo metadata`, so two tests of `.github/scripts/tests/test_ci_workflow.py` (the header-pin tests) fail from then until Task 1.8 adds Kalshi's pin steps; that is expected, and no other CI-script test is left red by a task.

### Task 1.1: The touch-set gate

Every story of the epic ends with this gate, so it lands first. It prints each path the branch has changed since `git merge-base main HEAD`, staged, unstaged or untracked, that the allowlist does not admit, and it fails when it prints one. The two files hold exactly the text the names contract gives.

**Files:**
- Create: `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`
- Create: `_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`

- [ ] **Step 1: Write the failing check.** The check is the gate itself, run three ways: on the branch as it stands (it must pass), with an untracked file outside the allowlist (it must print that file and fail), and with the allowlist missing (it must fail closed). There is no file to write for it.

- [ ] **Step 2: Run it to see it fail.**

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh; echo "exit $?"`
Expected: `bash: _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh: No such file or directory`, then `exit 127`.

- [ ] **Step 3: Implement.** Create the allowlist, one extended regular expression per line, these and no others.

`_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`:

```text
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
```

Create the gate. It does not set `pipefail`, so `test` reads `grep`'s status: `grep -v` exits 0 when it printed a path the allowlist does not admit, 1 when it printed none, and 2 when it could not run, and only 1 passes.

`_bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`:

```bash
#!/usr/bin/env bash
cd "$(git rev-parse --show-toplevel)" || exit 2
{ git diff --name-only "$(git merge-base main HEAD)"; git ls-files --others --exclude-standard; } | sort -u | grep -v -E -f "$(dirname "$0")/epic-8-touch-set.txt"; test $? -eq 1
```

- [ ] **Step 4: Run the check three ways.**

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`. The branch's changes since the merge base are this plan, its spec and the two new files, all admitted.

Run: `touch touch-set-control.txt && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh; echo "exit $?"; rm touch-set-control.txt`
Expected: `touch-set-control.txt`, then `exit 1`.

Run: `mv _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt.aside; bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh; echo "exit $?"; mv _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt.aside _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt`
Expected: a `grep: …/epic-8-touch-set.txt: No such file or directory` line, then `exit 1`: a missing allowlist fails the gate. Check `git status --short` shows both new files and nothing else.

- [ ] **Step 5: Commit.**

```bash
git add -N _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh
git commit -m "chore(epic-8): the touch-set gate every story ends with (Story 1)" -- _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.txt _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh
```

### Task 1.2: Register the crate

The crate joins the workspace with its manifest and metadata, its two vendored specs and `OBSERVED.md`, its `kalshi` mirrors table, a README for the crate and one for the module, and a `predictions` module that holds only its docs and its three constants. Its dependency rule and allowlist land with it, since `test_dep_allowlists.py` checks every file under `ci/dep-allowlists/` against `cargo tree`. Kalshi's rule is the one D23 gives: `alloy*`, `hmac` and `keyring` are never built, the socket stack only with `predictions-ws`, and the HTTP stack with `predictions`, which every file names. The generated regions of `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md` and `.github/workflows/nightly-schema.yml` are regenerated, never edited. Story 1 edits five paragraphs of `CLAUDE.md`, each in the commit that changes the rule it states (AD-21); this commit edits three of them: the overview, the allowlist rules in the CI paragraph, and Module Organization's first paragraph.

**Files:**
- Create: `docs/specs/kalshi/openapi.yaml`, `docs/specs/kalshi/asyncapi.yaml`, `docs/specs/kalshi/OBSERVED.md`
- Modify: `Cargo.toml:2-12`, `Cargo.toml:25`, `Cargo.toml:51`, `Cargo.toml:267`
- Create: `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/README.md`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/README.md`
- Modify: `.github/scripts/tests/test_dep_allowlists.py:101-106`, `.github/scripts/tests/test_dep_allowlists.py:438`
- Create: `ci/dep-allowlists/polyoxide-kalshi@predictions.txt`
- Modify: `CLAUDE.md:7`, `CLAUDE.md:58`, `CLAUDE.md:749-750`
- Regenerated: `Cargo.lock`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `.github/workflows/nightly-schema.yml`
- Test: `.github/scripts/tests/test_dep_allowlists.py`

- [ ] **Step 1: Write the failing tests.** Two checks on Kalshi's dependency rule, in `test_dep_allowlists.py`, on a stand-in for the crate's `[features]` as the file does for Binance's. They are written before the rule.

Current (`.github/scripts/tests/test_dep_allowlists.py`):

```python
    assert "usdm" in enabled(("usdm-ws",), table)


UMBRELLA = "polyoxide"
```

Replace with:

```python
    assert "usdm" in enabled(("usdm-ws",), table)


# A trimmed stand-in for Kalshi's `[features]`: `predictions` is the REST module, and
# `predictions-ws`, which a later story adds, its order-book stream on the socket kit.
KALSHI_TABLE = {
    "default": ("predictions",),
    "predictions": ("dep:polyoxide-core", "polyoxide-venue/market", "dep:ring"),
    "predictions-ws": ("predictions", "dep:polyoxide-ws"),
}
KALSHI = "polyoxide-kalshi"
KALSHI_HTTP = "reqwest, polyoxide-core, governor and sha2 only with predictions"
KALSHI_NEVER = "alloy*, hmac and keyring are never built"
KALSHI_SOCKETS = "polyoxide-ws, tokio-tungstenite and tungstenite only with predictions-ws"


def test_kalshis_http_stack_comes_with_predictions_and_its_sockets_with_predictions_ws() -> None:
    names = [*HTTP_STACK, *SOCKET_STACK]
    # A crate's defaults admit nothing: a file names every feature it builds.
    assert denied(KALSHI, names, (), KALSHI_TABLE) == sorted(
        [f"{n} ({KALSHI_HTTP})" for n in HTTP_STACK]
        + [f"{n} ({KALSHI_SOCKETS})" for n in SOCKET_STACK])
    # The REST module builds the HTTP stack and no socket.
    assert denied(KALSHI, names, ("predictions",), KALSHI_TABLE) == [
        f"{n} ({KALSHI_SOCKETS})" for n in SOCKET_STACK]
    # The stream builds the socket kit on top of it.
    assert denied(KALSHI, names, ("predictions-ws",), KALSHI_TABLE) == []


def test_no_kalshi_file_may_build_the_signing_stack_or_the_keychain() -> None:
    names = ["alloy", "alloy-primitives", "hmac", "keyring", "ring"]
    never = [f"{n} ({KALSHI_NEVER})" for n in names[:4]]
    # Kalshi signs with `ring`, which no rule refuses.
    for features in ((), ("predictions",), ("predictions-ws",)):
        assert denied(KALSHI, names, features, KALSHI_TABLE) == never, features


UMBRELLA = "polyoxide"
```

Then create the empty allowlist, which the file-parametrized checks pick up by its name:

Run: `touch 'ci/dep-allowlists/polyoxide-kalshi@predictions.txt'`

- [ ] **Step 2: Run them to see them fail.**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q -k kalshi'`
Expected: FAIL. Both new tests fail on an assertion, because a crate with no `PER_FEATURE` entry falls to the socket-only deny-list, which refuses `reqwest` and `polyoxide-core` with that list's wording. `test_each_allowlist_is_the_tree_cargo_builds[polyoxide-kalshi@predictions]` fails too: `cargo tree -p polyoxide-kalshi …` exits with `error: package ID specification `polyoxide-kalshi` did not match any packages`.

- [ ] **Step 3: Implement.** Eight parts, in this order: the rule, the specs, `OBSERVED.md`, the crate, the root manifest, the lock file and the allowlist, the generated regions, and `CLAUDE.md`.

**3a. The rule.**

Current (`.github/scripts/tests/test_dep_allowlists.py`):

```python
    "polyoxide-binance": (
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"usdm-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws"),
    ),
```

Replace with:

```python
    "polyoxide-binance": (
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"usdm-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with usdm-ws"),
    ),
    # Kalshi's REST module builds the HTTP stack and signs with ring, so no file of it admits
    # the Polymarket signing stack or the keychain; the socket stack is its order-book
    # stream's alone.
    "polyoxide-kalshi": (
        Rule(frozenset({"reqwest", "polyoxide-core", "governor", "sha2"}), (),
             frozenset({"predictions"}),
             "reqwest, polyoxide-core, governor and sha2 only with predictions"),
        Rule(frozenset({"hmac", "keyring"}), ("alloy",), frozenset(),
             "alloy*, hmac and keyring are never built"),
        Rule(frozenset({"polyoxide-ws", "tokio-tungstenite", "tungstenite"}), (),
             frozenset({"predictions-ws"}),
             "polyoxide-ws, tokio-tungstenite and tungstenite only with predictions-ws"),
    ),
```

**3b. The specs, byte for byte as served.** They are never edited by hand.

Run: `mkdir -p docs/specs/kalshi && curl -sSf -o docs/specs/kalshi/openapi.yaml https://docs.kalshi.com/openapi.yaml && curl -sSf -o docs/specs/kalshi/asyncapi.yaml https://docs.kalshi.com/asyncapi.yaml`
Expected: no output.

Run: `grep -m1 '^  version:' docs/specs/kalshi/openapi.yaml docs/specs/kalshi/asyncapi.yaml`
Expected: `docs/specs/kalshi/openapi.yaml:  version: 3.34.0` and `docs/specs/kalshi/asyncapi.yaml:  version: 2.0.0`. Any other version stops the task: the plan, the spec and `OBSERVED.md` were written against these two, so ask.

**3c. `OBSERVED.md`.** It records the spec's probes, the error shapes, the ticker pattern and the rate limits. It ends with the anchor line that Stories 4, 6 and 7 insert their sections above.

`docs/specs/kalshi/OBSERVED.md`:

```markdown
# Kalshi: what the hosts do

Measured against `external-api.kalshi.com` (production) and `external-api.demo.kalshi.co` (demo) on 2026-10-18, with unsigned `GET`s, unless an entry says otherwise. `openapi.yaml` and `asyncapi.yaml` beside this file are Kalshi's served OpenAPI (`info.version` 3.34.0) and AsyncAPI (`info.version` 2.0.0), byte for byte. Kalshi publishes neither as JSON: `openapi.json` and `asyncapi.json` answer 404. Kalshi announced schema corrections dated 2026-10-22, so a drift issue will follow, and adopting it is that issue's `curl`.

This file records what the specs leave out or get wrong. A later story's findings go in a section of their own, directly above the last line.

## Probes of the production host

Read-only requests at about 09:10 UTC. Rows 2 and 3 and the error bodies below were repeated at 10:30 UTC.

1. `GET /trade-api/v2/account/endpoint_costs` answers with no key. `default_cost` is 10, and the listed paths are full templates (`/trade-api/v2/portfolio/orders/:order_id`, `/trade-api/v2/cfbenchmarks/*endpoint`). None of the Predictions client's routes is listed, so each costs 10.
2. `GET /trade-api/v2/exchange/status` answers with no key, with `exchange_index_statuses` for indexes 0 to 3 (Default, Combos, Crypto & Commodities, and Tennis, Baseball, Basketball). The response is cacheable for a second (`cache-control: public, max-age=1`). The demo host answers it with 200 too.
3. For an unknown ticker, `/markets/{t}` and `/events/{e}` answer `404 {"error":{"code":"not_found","message":"not found"}}`, but `/markets/{t}/orderbook` answers `200 {"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}`. The orderbook route cannot tell an unlisted ticker from an empty book, so a listing is confirmed on `/markets/{t}`.
4. Tickers are case-sensitive: a lowercase ticker is a 404 on `/markets/{t}` and an empty book on `/orderbook`. Tickers contain `-` and `.` (`KXBTCD-26OCT1806-T82099.99`). The longest of 1,000 open tickers was 51 characters.
5. The REST orderbook lists each side's levels in ascending price, so worst first, where the OpenAPI's description says best to worst. `depth=n` keeps the n best levels, and a `depth` of 0 or less keeps every level.
6. Error bodies: see below.
7. Of 1,000 open markets, every one has `notional_value_dollars` of `1.0000` and three `price_ranges` (steps `0.0001`, `0.0010` and `0.0001`, with `price_level_structure` `center_deci_edge_centi_cent`). Trades come newest first, and `count_fp` can be fractional (`46.91`).
8. `GET /trade-api/v2/events/{e}?with_nested_markets=true` returned all 188 markets of `KXBTCD-26OCT1806`, as many as `/markets?event_ticker=` paged. The OpenAPI says markets settled before the historical cutoff are left out.

## Error bodies

Kalshi answers with three shapes, which the client's one decode reads (`PredictionsError`'s `From<ApiError>`).

- `{"error":{"code":"…","message":"…","details":"…"}}`, with `details` optional. A 404 is `{"error":{"code":"not_found","message":"not found"}}`, and an unsigned request to a route that needs a key, `GET /account/limits`, is a 401 with `{"error":{"code":"token_authentication_failure","message":"token authentication failure"}}`.
- `{"error":"<text>"}`. The documentation gives a 429 the body `{"error":"too many requests"}`. A 429 was not provoked.
- `{"msg":"<text>"}`: a parameter-validation 400. The wording varies. The first probe recorded `Parameter validation failed for …`, and `GET /markets?limit=abc` at 10:30 UTC answered `{"msg":"Invalid format for parameter limit: error binding string parameter: strconv.ParseInt: parsing \"abc\": invalid syntax"}`. The client reads `msg` whole.

The documentation says a 429 carries no `Retry-After` and no `X-RateLimit-*` header.

## The AsyncAPI's ticker pattern

`asyncapi.yaml` gives `marketTicker` the pattern `^[A-Z0-9-]+$`, and its own examples carry dots (`FED-23DEC-T3.00`), as production's tickers do. The client accepts `[A-Z0-9.-]`, 1 to 200 characters (the OpenAPI's `maxLength` for the tickers of `GET /markets/orderbooks`), at least one of them a letter or digit, so `.` and `..`, which a URL resolves as dot segments, are refused, and uppercases ASCII letters.

## Rate limits

Published (`getting_started/rate_limits`, read 2026-10-18):

- every authenticated request costs tokens, 10 by default, and `GET /account/endpoint_costs` lists the exceptions;
- Read and Write are separate token buckets. Basic's Read bucket refills 200 tokens a second and holds 3 seconds of it, 600. Its Write bucket refills 100 a second and holds 1 second. No tier's Read capacity or refill is below Basic's;
- `GET /portfolio/balance` has a bucket of its own (since 2026-10-15), and Write buckets are per shard;
- a 429 carries no penalty or cooldown, and the client should back off;
- no limit for unauthenticated requests is published.

What the client builds, and what is provisional:

- One Read layer, `predictions.read`, a token bucket sized at Basic's published capacity (600) and refill (200 a second). Kalshi publishes a bucket's capacity and the server is a bucket, so the client's bucket holds the published burst. It is provisional until the account's own size is read from `GET /account/limits` (Story 4 of the epic). Basic is the floor of every tier, so a client that cannot read its account's size under-sends and never over-sends.
- A client with no credentials keeps that sizing. No anonymous limit is published, so for it the sizing is a guess. A soak that measures the anonymous limit is deferred.
- Every route costs the default, 10. The first-use confirmation (Story 4) replaces a cost with what `GET /account/endpoint_costs` lists.
- An instrument's `min_size` and `size_increment` will be 0.01, the granularity the OpenAPI gives `FixedPointCount` for orders. They are provisional until Kalshi's `Trading` probes them.

Recorded, not built: the Write bucket, the balance bucket and the per-shard Write buckets come with Kalshi's `Trading`.

<!-- capture findings go above this line -->
```

**3d. The crate.**

`polyoxide-kalshi/Cargo.toml`. Its anchor lines are the names contract's: `[dependencies]` and `[dev-dependencies]` each on a line of its own, the `mockito` line, the `# Registration…` comment directly above `[package.metadata.polyoxide]`, and `[package.metadata.docs.rs]` with its `features` line as the file's last two lines. Later stories insert before or after them.

```toml
[package]
name = "polyoxide-kalshi"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "Rust client library for Kalshi's event-contract market data (a walking skeleton, not published)"
publish = false

# One feature per module, as polyoxide-binance has: `predictions`, the REST client
# for Kalshi's Predictions exchange and its Read throttle. The order-book stream
# arrives as `predictions-ws`.
[features]
default = ["predictions"]
predictions = [
    "dep:polyoxide-core",
    "polyoxide-venue/decimal",
    "polyoxide-venue/market",
    "dep:base64",
    "dep:chrono",
    "dep:ring",
    "dep:rust_decimal",
    "dep:serde",
    "dep:serde_json",
    "dep:thiserror",
    "dep:tokio",
    "dep:tracing",
    "dep:url",
]

[dependencies]
polyoxide-core = { workspace = true, optional = true }
polyoxide-venue = { workspace = true }
base64 = { workspace = true, optional = true }
chrono = { workspace = true, optional = true }
ring = { workspace = true, optional = true }
rust_decimal = { workspace = true, optional = true }
serde = { workspace = true, optional = true }
serde_json = { workspace = true, optional = true }
thiserror = { workspace = true, optional = true }
tokio = { workspace = true, features = ["time", "sync"], optional = true }
tracing = { workspace = true, optional = true }
url = { workspace = true, optional = true }

[dev-dependencies]
# The tests' query and header helpers, the failure tags and the credential
# loaders. Path-only: it is never published, and cargo strips a path-only
# dev-dependency on publish.
polyoxide-test-support = { path = "../polyoxide-test-support", features = ["query"] }
mockito = { workspace = true }
# `test-util` gives the throttle's tests a paused clock.
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time", "test-util"] }

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
[package.metadata.polyoxide]
readme = "Client library for Kalshi's APIs, one feature per module: `predictions` (event-contract market data, with its order-book stream under `predictions-ws`); a walking skeleton, not published"
venue = "kalshi"
products = ["predictions"]
mirrors = ["kalshi"]

[package.metadata.docs.rs]
features = ["predictions"]
```

`polyoxide-kalshi/README.md` is prose with no code fence, since it is compiled as a doctest, and no four-space indent, which a doctest reads as code:

```markdown
# polyoxide-kalshi

A Rust client library for Kalshi's event-contract exchange, the third venue of the polyoxide workspace after Polymarket and Binance.

This crate is a walking skeleton and it is not published. It exists to show that a venue lands by the architecture guide's recipe: a crate of its own, registered in its manifest, touching the workspace only in the few places the recipe names.

It has one module, `predictions`, behind the `predictions` feature, which is on by default. The module covers Kalshi's Predictions exchange on `external-api.kalshi.com`, and on the demo host `external-api.demo.kalshi.co`. Its documentation says what it offers.
```

`polyoxide-kalshi/src/predictions/README.md`, likewise:

```markdown
# Predictions

The module of `polyoxide-kalshi` for Kalshi's Predictions exchange, on `external-api.kalshi.com` and, for keys made there, on the demo host `external-api.demo.kalshi.co`. The crate is a walking skeleton and it is not published. Kalshi's other exchange, Margin, is not covered, and the name `margin` is left free for it.

A market is binary, and its two outcomes trade as two books, so a market key names one outcome: `kalshi.predictions:<TICKER>:yes` or `kalshi.predictions:<TICKER>:no`. Tickers are case-sensitive and uppercase, so the key space uppercases the ticker it is given.

Every request runs on the send loop of `polyoxide-core` and costs tokens from a Read throttle, sized at Kalshi's Basic tier, which no tier falls below. The module's documentation says what it offers, and `docs/specs/kalshi/OBSERVED.md` records what the host does.
```

`polyoxide-kalshi/src/lib.rs`:

```rust
//! # polyoxide-kalshi
//!
//! Rust client library for Kalshi's APIs, one feature per module: `predictions`.
//!
//! The crate is a walking skeleton and it is not published. It is the workspace's third venue,
//! built by the architecture guide's recipe to show that a venue lands by editing its own files
//! and the few places the recipe names.
//!
//! `predictions` (feature `predictions`, on by default) reads Kalshi's Predictions exchange
//! (`external-api.kalshi.com`, and the demo host `external-api.demo.kalshi.co`). Public reads need
//! no credentials. The module's own docs carry its example.

// The crate README is compiled as a doctest, so a code fence that is ever added
// to it cannot rot. `doctest` keeps this out of normal builds and `cargo doc`.
#[cfg(all(doctest, feature = "predictions"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

// Each module's README is a doctest under its own feature, as the crate's is.
// `.github/scripts/tests/test_module_readmes.py` fails when a module README
// has no include here, or an include names no README.
#[cfg(all(doctest, feature = "predictions"))]
#[doc = include_str!("predictions/README.md")]
struct PredictionsReadmeDoctests;

#[cfg(feature = "predictions")]
pub mod predictions;
```

`polyoxide-kalshi/src/predictions/mod.rs`, holding the module docs and the three constants. Tasks 1.3 to 1.7 build the rest of it up to the names contract's module block.

```rust
//! # Predictions
//!
//! Client for Kalshi's Predictions exchange (`external-api.kalshi.com`), behind the `predictions`
//! feature, which is on by default. The module is a walking skeleton: it grows a part at a time,
//! and what it offers today is in its namespaces.
//!
//! Every request runs on core's send loop and costs tokens from a Read throttle. The throttle
//! starts at the size Kalshi gives its Basic tier, and a client confirms the account's own size
//! when it first sends. The hosts are [`PRODUCTION`] and [`DEMO`], and keys do not cross between
//! them.

/// Production REST host: a scheme and host, with no path.
pub const PRODUCTION: &str = "https://external-api.kalshi.com";

/// Demo REST host. Keys do not cross environments.
pub const DEMO: &str = "https://external-api.demo.kalshi.co";

/// In-flight requests the client allows by default.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;
```

**3e. The root manifest.** Four edits.

Current (`Cargo.toml`):

```toml
  "polyoxide-core",
  "polyoxide-polymarket",
```

Replace with:

```toml
  "polyoxide-core",
  "polyoxide-kalshi",
  "polyoxide-polymarket",
```

Current (`Cargo.toml`):

```toml
polyoxide-core = { path = "polyoxide-core", version = "0.41.0" }
```

Replace with:

```toml
polyoxide-core = { path = "polyoxide-core", version = "0.41.0" }
polyoxide-kalshi = { path = "polyoxide-kalshi" }
```

Current (`Cargo.toml`):

```toml
rustls = { version = "0.23", default-features = false, features = ["ring", "std"] }
```

Replace with:

```toml
rustls = { version = "0.23", default-features = false, features = ["ring", "std"] }
ring = "0.17"
```

Current (`Cargo.toml`, the file's last line):

```toml
exclude = "Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites in `polyoxide-binance` are the drift check."
```

Replace with:

```toml
exclude = "Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites in `polyoxide-binance` are the drift check."

[workspace.metadata.polyoxide.mirrors.kalshi]
name = "Kalshi"
section = "other-venue"
base_urls = ["https://external-api.kalshi.com", "https://external-api.demo.kalshi.co", "wss://external-api-ws.kalshi.com/trade-api/ws/v2", "wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2"]
description = "Event-contract market data and its order-book stream. A walking skeleton, in a crate that is not published"

[[workspace.metadata.polyoxide.mirrors.kalshi.specs]]
id = "kalshi"
kind = "openapi"
url = "https://docs.kalshi.com/openapi.yaml"
vendored = "docs/specs/kalshi/openapi.yaml"

[[workspace.metadata.polyoxide.mirrors.kalshi.specs]]
id = "kalshi-ws"
kind = "asyncapi"
url = "https://docs.kalshi.com/asyncapi.yaml"
vendored = "docs/specs/kalshi/asyncapi.yaml"
covers = "Trade API WebSocket (15 channels); `orderbook_delta` is implemented"
```

**3f. The lock file and the allowlist.** The first build adds the crate to `Cargo.lock`; the second command writes the allowlist from the tree cargo reports.

Run: `nix develop --command cargo build -p polyoxide-kalshi --no-default-features --features predictions`
Expected: `Compiling polyoxide-kalshi v0.41.0 (…)` and a clean finish. `git diff --stat Cargo.lock` shows only the new `polyoxide-kalshi` package.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`
Expected: no output. Then `git status --short ci/dep-allowlists` lists only `polyoxide-kalshi@predictions.txt`. Any other allowlist changing means the tree moved under it: stop and ask. Open the new file and check that it lists `polyoxide-core`, `reqwest` and `ring`, and none of `alloy`, `hmac`, `keyring`, `polyoxide-ws` and `tokio-tungstenite`.

**3g. The generated regions.**

Run: `nix develop --command python3 scripts/gen_registry.py --write`
Expected: exit 0. `git status --short` then shows changes in `Cargo.toml`, `Cargo.lock`, `CLAUDE.md`, `README.md`, `SELF-HEALING.md`, `docs/specs/INDEX.md`, `.github/workflows/nightly-schema.yml`, the test file and the new paths, and in nothing else. `CLAUDE.md` now counts ten crates and lists `polyoxide-kalshi` among the unpublished ones, before `polyoxide-py`, and the schema-watch regions of `CLAUDE.md`, `SELF-HEALING.md` and `nightly-schema.yml` list `kalshi` and `kalshi-ws`.

**3h. The hand-written paragraphs of `CLAUDE.md`.** Three edits, each a part of one long line or paragraph.

Current (`CLAUDE.md`):

```markdown
and `polyoxide-binance` for Binance's USDⓈ-M futures market data and streams, under a `polyoxide` umbrella with a module per venue. It also provides
```

Replace with:

```markdown
and `polyoxide-binance` for Binance's USDⓈ-M futures market data and streams, under a `polyoxide` umbrella with a module per venue (a third crate, `polyoxide-kalshi`, is a walking skeleton for Kalshi's event-contract market data: it is not published, and the umbrella does not include it). It also provides
```

Current (`CLAUDE.md`):

```markdown
(and `--no-default-features` for `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide`, whose files name every feature they build; the socket kit's bare file is its default build)
```

Replace with:

```markdown
(and `--no-default-features` for `polyoxide-polymarket`, `polyoxide-binance`, `polyoxide-kalshi` and `polyoxide`, whose files name every feature they build; the socket kit's bare file is its default build)
```

Current (`CLAUDE.md`):

```markdown
`rustls` is not on the rule, since core's HTTP client builds it for TLS), and the umbrella `polyoxide`'s per feature on its own names,
```

Replace with:

```markdown
`rustls` is not on the rule, since core's HTTP client builds it for TLS), and `polyoxide-kalshi`'s per feature too, so `reqwest`, `polyoxide-core`, `governor` and `sha2` only with `predictions`, which every file names, `alloy*`, `hmac` and `keyring` never, and `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `predictions-ws` (its `@predictions` file exists), and the umbrella `polyoxide`'s per feature on its own names,
```

Current (`CLAUDE.md`):

```markdown
`src/<module>/` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay`, `perps`, `rtds` and
`sports`, and `polyoxide-binance`'s `usdm`, whose `mod.rs` holds its client), with `mod.rs` in
```

Replace with:

```markdown
`src/<module>/` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay`, `perps`, `rtds` and
`sports`, `polyoxide-binance`'s `usdm`, whose `mod.rs` holds its client, and `polyoxide-kalshi`'s
`predictions`), with `mod.rs` in
```

- [ ] **Step 4: Run the tests again.**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_module_readmes.py tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_gen_registry.py tests/test_venue_gate.py tests/test_spec_docs.py -q'`
Expected: PASS. The two new rule tests pass, and so do the parametrized checks of `polyoxide-kalshi@predictions`: the tree matches the file, and the file admits nothing its rule refuses. The module README check finds the one include of `predictions/README.md` under `feature = "predictions"`.

Run: `nix develop --command python3 scripts/gen_registry.py --check && nix develop --command python3 scripts/venue_gate.py`
Expected: both exit 0, `gen_registry.py` with no region differing.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: clean, with `running 0 tests` for every target.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py -q'`
Expected: exactly two failures, `test_the_features_job_runs_each_header_pin_in_its_minimal_build` and `test_the_features_job_runs_each_header_pin_under_the_umbrellas_full`, because Kalshi is now an HTTP module with no pin step and no `headers_predictions.rs`. Task 1.8 clears them.

- [ ] **Step 5: Commit.**

```bash
git add -N docs/specs/kalshi polyoxide-kalshi 'ci/dep-allowlists/polyoxide-kalshi@predictions.txt'
git commit -m "feat(kalshi): register polyoxide-kalshi, its mirrors and its dependency rule (Story 1)" -- Cargo.toml Cargo.lock CLAUDE.md README.md SELF-HEALING.md docs/specs/INDEX.md docs/specs/kalshi .github/workflows/nightly-schema.yml .github/scripts/tests/test_dep_allowlists.py 'ci/dep-allowlists/polyoxide-kalshi@predictions.txt' polyoxide-kalshi
```

### Task 1.3: The key space

`key.rs` is the module's key space (D8): a `Ticker` that uppercases and checks what a caller sends, the two outcomes of a binary market, and `PredictionsKeys`, whose ids are `<TICKER>:yes` and `<TICKER>:no`, split at the last `:`. It also adds the crate's `Classify` assertion, listing `InvalidTicker`; Task 1.4 completes the list.

**Files:**
- Create: `polyoxide-kalshi/src/predictions/key.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/lib.rs`
- Test: `polyoxide-kalshi/src/predictions/key.rs`

- [ ] **Step 1: Write the failing tests.** `key.rs` holds only its tests for now. They cover the registration test (the ids the crate declares in its manifest), canonicalization, a dotted ticker, the round trip through `RawKey::parse`, `parse` and `Display`, the 1 to 200 character bound and the refused characters, the refusal of a ticker with no letter or digit (`.` and `..` among them, also as the ticker of a key), an id with no ticker or no outcome, and a key of another venue or product.

`polyoxide-kalshi/src/predictions/key.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> RawKey {
        RawKey::parse(text).unwrap()
    }

    fn ticker(text: &str) -> Ticker {
        Ticker::new(text).unwrap()
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
    }

    #[test]
    fn a_lowercase_ticker_and_outcome_name_the_same_key() {
        let lower = parse(&raw("kalshi.predictions:kxbtcd-26oct1806-t82099.99:YES")).unwrap();
        let upper = parse(&raw("kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes")).unwrap();
        assert_eq!(lower, upper);
        assert_eq!(lower.id(), "KXBTCD-26OCT1806-T82099.99:yes");
        assert_eq!(
            lower,
            key(&ticker("KXBTCD-26OCT1806-T82099.99"), Outcome::Yes)
        );
    }

    #[test]
    fn a_dotted_ticker_parses_and_keeps_its_dot() {
        // Production serves tickers such as this one, though the AsyncAPI's
        // pattern is `^[A-Z0-9-]+$`.
        let parsed = parse(&raw("kalshi.predictions:FED-23DEC-T3.00:no")).unwrap();
        let id = outcome(&parsed).unwrap();
        assert_eq!(id.ticker.as_str(), "FED-23DEC-T3.00");
        assert_eq!(id.outcome, Outcome::No);
    }

    #[test]
    fn a_key_round_trips_through_its_text() {
        let text = "kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes";
        let parsed = parse(&RawKey::parse(text).unwrap()).unwrap();
        assert_eq!(parsed.to_string(), text);
        let id = outcome(&parsed).unwrap();
        assert_eq!(id.ticker.as_str(), "KXBTCD-26OCT1806-T82099.99");
        assert_eq!(id.outcome, Outcome::Yes);
        assert_eq!(key(&id.ticker, id.outcome), parsed);
    }

    #[test]
    fn the_two_outcomes_of_a_market_are_two_keys() {
        let market = ticker("KXBTCD-26OCT1806-T82099.99");
        let yes = key(&market, Outcome::Yes);
        let no = key(&market, Outcome::No);
        assert_ne!(yes, no);
        assert_eq!(no, key(&market, Outcome::Yes.other()));
        assert_eq!(Outcome::No.other(), Outcome::Yes);
    }

    #[test]
    fn a_ticker_has_one_to_200_characters_of_the_alphabet() {
        assert_eq!(ticker(&"A".repeat(200)).as_str().len(), 200);

        let empty = Ticker::new("").unwrap_err();
        assert_eq!(empty.text, "");
        assert!(empty.reason.contains("at least one"), "{empty:?}");

        let long = Ticker::new(&"A".repeat(201)).unwrap_err();
        assert!(long.reason.contains("at most 200"), "{long:?}");

        for text in [
            "BTC/USD", "BTC USD", "BTC_USD", "BTC:YES", "BTC*", "BTC%41", "BTCé",
        ] {
            let refused = Ticker::new(text).unwrap_err();
            assert_eq!(refused.text, text);
            assert!(refused.reason.contains("holds only"), "{text}: {refused:?}");
        }
    }

    #[test]
    fn a_refused_ticker_names_the_text_and_is_an_invalid_request() {
        let err = Ticker::new("BTC/USD").unwrap_err();
        assert_eq!(
            err.to_string(),
            "\"BTC/USD\" is not a Kalshi ticker: a ticker holds only A-Z, 0-9, '.' and '-'"
        );
        assert_eq!(err.class(), Class::InvalidRequest);
    }

    #[test]
    fn a_ticker_needs_a_letter_or_a_digit_so_dot_segments_are_refused() {
        // `.` and `..` are dot segments, which a URL resolves before it is sent.
        for text in [".", "..", "-", "...", ".-."] {
            let refused = Ticker::new(text).unwrap_err();
            assert_eq!(refused.text, text);
            assert!(
                refused.reason.contains("letter or digit"),
                "{text}: {refused:?}"
            );
            assert!(
                refused.reason.contains("dot segments"),
                "{text}: {refused:?}"
            );
            assert_eq!(refused.class(), Class::InvalidRequest);
        }

        // The same text as the ticker of a key is a key error, not a dot segment in a path.
        for text in ["kalshi.predictions:.:yes", "kalshi.predictions:..:no"] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::InvalidId { .. })),
                "{text}"
            );
        }

        // One letter or digit is enough, wherever it sits.
        for text in ["A", "0", ".A", "A..", "-9", "a.b"] {
            assert!(Ticker::new(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn an_id_without_a_ticker_or_an_outcome_is_refused() {
        for text in [
            "kalshi.predictions:KXBTCD",
            "kalshi.predictions:KXBTCD:maybe",
            "kalshi.predictions::yes",
            "kalshi.predictions:KX BTC:yes",
        ] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::InvalidId { .. })),
                "{text}"
            );
        }
    }

    #[test]
    fn a_key_of_another_venue_or_product_is_refused() {
        for text in ["binance.usdm:BTCUSDT:yes", "kalshi.margin:KXBTCD:yes"] {
            assert!(
                matches!(parse(&raw(text)), Err(KeyError::WrongProduct { .. })),
                "{text}"
            );
        }
    }
}
```

Declare the module and its re-export, and start the assertion.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
/// Production REST host: a scheme and host, with no path.
pub const PRODUCTION: &str = "https://external-api.kalshi.com";
```

Replace with:

```rust
pub mod key;

pub use key::key;

/// Production REST host: a scheme and host, with no path.
pub const PRODUCTION: &str = "https://external-api.kalshi.com";
```

Current (`polyoxide-kalshi/src/lib.rs`):

```rust
#[cfg(feature = "predictions")]
pub mod predictions;
```

Replace with:

```rust
#[cfg(feature = "predictions")]
pub mod predictions;

// Every public error type implements `Classify`; one without it fails the
// build here. `.github/scripts/tests/test_classify_coverage.py` fails when a
// public error type is missing from this list.
#[cfg(feature = "predictions")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<predictions::key::InvalidTicker>();
};
```

- [ ] **Step 2: Run them to see them fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib`
Expected: a compile error, not a test failure: `error[E0412]: cannot find type `Ticker` in this scope` and `cannot find type `RawKey``, `error[E0425]: cannot find function `parse``, `error[E0432]: unresolved import `key::key``, and `error[E0412]: cannot find type `InvalidTicker` in module `predictions::key``.

- [ ] **Step 3: Implement.** Put the key space above the tests. The ticker's grammar is D8's: `[A-Z0-9.-]`, 1 to 200 characters, at least one of them a letter or digit, ASCII letters uppercased. `Ticker::new` is the one check, and `parse_id` calls it. `parse_id` splits at the last `:`, since a ticker has none.

Current (`polyoxide-kalshi/src/predictions/key.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
//! Predictions' market keys: `kalshi.predictions:<TICKER>:<outcome>`, one per outcome of a market.
//!
//! A Kalshi market is binary, and its two outcomes trade as two books, so a key names one outcome
//! of one market: the market's [`Ticker`] and `yes` or `no`. A ticker's ASCII letters are
//! uppercased and an outcome's lowercased, so
//! `kalshi.predictions:kxbtcd-26oct1806-t82099.99:YES` and
//! `kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes` are one key.
//!
//! ```
//! use polyoxide_kalshi::predictions::key::{self, Outcome, Ticker};
//! use polyoxide_venue::RawKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let raw = RawKey::parse("kalshi.predictions:kxbtcd-26oct1806-t82099.99:YES")?;
//! let parsed = key::parse(&raw)?;
//! let ticker = Ticker::new("KXBTCD-26OCT1806-T82099.99")?;
//! assert_eq!(parsed, key::key(&ticker, Outcome::Yes));
//! assert_eq!(
//!     parsed.to_string(),
//!     "kalshi.predictions:KXBTCD-26OCT1806-T82099.99:yes"
//! );
//! assert_eq!(key::outcome(&parsed)?.outcome, Outcome::Yes);
//! # Ok(())
//! # }
//! ```

use std::borrow::Cow;
use std::fmt;

use polyoxide_venue::{Class, Classify, KeyError, KeySpace, MarketKey, ProductId, RawKey, VenueId};
use thiserror::Error;

/// Kalshi's venue id, as the crate registers it.
pub const VENUE: VenueId = VenueId::from_static("kalshi");

/// Predictions' product id, as the crate registers it.
pub const PRODUCT: ProductId = ProductId::from_static("predictions");

/// The most characters a ticker has: the length the OpenAPI gives the tickers of
/// `GET /markets/orderbooks`.
const MAX_TICKER_LEN: usize = 200;

/// A market's ticker, such as `KXBTCD-26OCT1806-T82099.99`: 1 to 200 characters of `A-Z`, `0-9`,
/// `.` and `-`, at least one of them a letter or digit, with ASCII letters uppercased.
///
/// A ticker of dots and hyphens alone is refused, since `.` and `..` are dot segments, which a URL
/// resolves before the request is sent.
///
/// Kalshi's tickers are case-sensitive and every one it serves is uppercase, so a lowercase ticker
/// would name nothing at the venue; uppercasing it names the market the caller meant. A response
/// row keeps its ticker as a `String`: this type is what a caller sends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ticker(String);

impl Ticker {
    /// Checks and canonicalizes `text`.
    ///
    /// # Errors
    ///
    /// [`InvalidTicker`] when `text` is empty, holds a character outside `A-Z`, `0-9`, `.` and `-`
    /// once its letters are uppercased, is longer than 200 characters, or holds no letter or digit.
    pub fn new(text: &str) -> Result<Self, InvalidTicker> {
        let refuse = |reason: &'static str| InvalidTicker {
            text: text.to_owned(),
            reason,
        };
        let ticker = text.to_ascii_uppercase();
        if ticker.is_empty() {
            return Err(refuse("a ticker has at least one character"));
        }
        if !ticker.bytes().all(is_ticker_byte) {
            return Err(refuse("a ticker holds only A-Z, 0-9, '.' and '-'"));
        }
        if ticker.len() > MAX_TICKER_LEN {
            return Err(refuse("a ticker has at most 200 characters"));
        }
        if !ticker.bytes().any(|byte| byte.is_ascii_alphanumeric()) {
            return Err(refuse(
                "a ticker has at least one letter or digit ('.' and '..' are dot segments)",
            ));
        }
        Ok(Self(ticker))
    }

    /// The ticker's text, in its canonical form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether `byte` may appear in an uppercased ticker.
fn is_ticker_byte(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
}

impl fmt::Display for Ticker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Text that is not a ticker.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
#[error("{text:?} is not a Kalshi ticker: {reason}")]
pub struct InvalidTicker {
    /// The text, as given.
    pub text: String,
    /// What is wrong with it.
    pub reason: &'static str,
}

/// [`Class::InvalidRequest`]: the caller named something no request can carry.
impl Classify for InvalidTicker {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

/// One of a binary market's two outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// The outcome the market's question names.
    Yes,
    /// The other one.
    No,
}

impl Outcome {
    /// The outcome's spelling in a key: `yes` or `no`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
        }
    }

    /// The market's other outcome.
    pub fn other(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
        }
    }
}

/// The id of a Predictions key: one outcome of one market.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OutcomeId {
    /// The market.
    pub ticker: Ticker,
    /// Which of its two outcomes.
    pub outcome: Outcome,
}

/// Predictions' keys. Each names one outcome of one market, as an [`OutcomeId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PredictionsKeys;

impl KeySpace for PredictionsKeys {
    const VENUE: VenueId = VENUE;
    const PRODUCT: ProductId = PRODUCT;
    type Id = OutcomeId;

    /// Splits the id at its last `:`: the ticker before it, uppercased and checked, and the
    /// outcome after it, lowercased. A ticker has no `:`, so the last one is the outcome's.
    fn parse_id(text: &str) -> Result<OutcomeId, String> {
        let (ticker, outcome) = text
            .rsplit_once(':')
            .ok_or_else(|| "expected <TICKER>:yes or <TICKER>:no".to_owned())?;
        let ticker = Ticker::new(ticker).map_err(|err| err.to_string())?;
        let outcome = match outcome.to_ascii_lowercase().as_str() {
            "yes" => Outcome::Yes,
            "no" => Outcome::No,
            _ => return Err(format!("{outcome:?} is not an outcome: expected yes or no")),
        };
        Ok(OutcomeId { ticker, outcome })
    }

    fn id_text(id: &OutcomeId) -> Cow<'_, str> {
        Cow::Owned(format!("{}:{}", id.ticker, id.outcome.as_str()))
    }
}

/// The key of `outcome` of the market `ticker`.
pub fn key(ticker: &Ticker, outcome: Outcome) -> MarketKey {
    PredictionsKeys::key(&OutcomeId {
        ticker: ticker.clone(),
        outcome,
    })
}

/// The key `raw` names, with its ticker uppercased and its outcome lowercased.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `raw` is not `kalshi.predictions`, and
/// [`KeyError::InvalidId`] when its id is not `<TICKER>:yes` or `<TICKER>:no`.
pub fn parse(raw: &RawKey) -> Result<MarketKey, KeyError> {
    PredictionsKeys::parse(raw)
}

/// The market and outcome `key` names.
///
/// # Errors
///
/// [`KeyError::WrongProduct`] when `key` is not a `kalshi.predictions` key.
pub fn outcome(key: &MarketKey) -> Result<OutcomeId, KeyError> {
    PredictionsKeys::id(key)
}

#[cfg(test)]
mod tests {
    use super::*;
```

- [ ] **Step 4: Run them to see them pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: PASS: 10 tests in `predictions::key::tests` and the module's one doctest.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: clean.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'`
Expected: PASS: `InvalidTicker` is public, an error type and listed in the assertion.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-kalshi/src/predictions/key.rs
git commit -m "feat(kalshi): the predictions key space (Story 1)" -- polyoxide-kalshi/src/predictions/key.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/lib.rs
```

### Task 1.4: The error and its one decode

`PredictionsError` is `Api(ApiError)` or `Venue(VenueError)`, and its one `From<ApiError>` (D7) is the only place Kalshi's error bodies are read. An unsuccessful response whose body is one of Kalshi's three shapes becomes `Venue`; every other error, an unsuccessful response with another body included, stays core's. This is the landmine task of the story: a foreign error reaches the module only through `ApiError` (`.map_err(ApiError::from)?`), so no other `From` impl is written here, and no inherent `is_retriable` or `retry_after` exists. Classification is by status before body (AD-15), through the workspace's status map.

**Files:**
- Create: `polyoxide-kalshi/src/predictions/error.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/lib.rs`
- Test: `polyoxide-kalshi/src/predictions/error.rs`

- [ ] **Step 1: Write the failing tests.** `error.rs` holds only its tests for now. The bodies are the ones the production host sent on 2026-10-18 (a 404, a 401 and a parameter-validation 400) and the one Kalshi's documentation gives a 429. The tests cover each shape's fields, the class of every status in the map with the body it came with, the `Retry-After` and the zero that is no wait, a body in no known shape, an error that is not a response, and the 451 that is not a fault.

`polyoxide-kalshi/src/predictions/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use polyoxide_core::reqwest::header::{HeaderMap, RETRY_AFTER};
    use polyoxide_core::reqwest::StatusCode;

    use super::*;

    /// A response with `status`, `body` and, when given, a `Retry-After`, as core builds it.
    fn response(status: u16, retry_after: Option<&str>, body: &str) -> ApiError {
        let mut headers = HeaderMap::new();
        if let Some(value) = retry_after {
            headers.insert(RETRY_AFTER, value.parse().unwrap());
        }
        ErrorResponse::new(StatusCode::from_u16(status).unwrap(), headers, body).into()
    }

    /// That response, through Kalshi's decode.
    fn decoded(status: u16, retry_after: Option<&str>, body: &str) -> PredictionsError {
        PredictionsError::from(response(status, retry_after, body))
    }

    /// The `VenueError` a response decodes to.
    fn venue(status: u16, retry_after: Option<&str>, body: &str) -> VenueError {
        match decoded(status, retry_after, body) {
            PredictionsError::Venue(venue) => venue,
            other => panic!("expected a Kalshi error, got {other:?}"),
        }
    }

    // Captured from the production host on 2026-10-18.
    const NOT_FOUND: &str = r#"{"error":{"code":"not_found","message":"not found"}}"#;
    const UNAUTHENTICATED: &str = concat!(
        r#"{"error":{"code":"token_authentication_failure","#,
        r#""message":"token authentication failure"}}"#,
    );
    const BAD_PARAMETER: &str = concat!(
        r#"{"msg":"Invalid format for parameter limit: error binding string parameter: "#,
        r#"strconv.ParseInt: parsing \"abc\": invalid syntax"}"#,
    );
    // The body Kalshi's documentation gives a `429`.
    const TOO_MANY: &str = r#"{"error":"too many requests"}"#;

    #[test]
    fn a_nested_error_gives_its_code_message_and_details() {
        let err = venue(404, None, NOT_FOUND);
        assert_eq!(err.status, 404);
        assert_eq!(err.code.as_deref(), Some("not_found"));
        assert_eq!(err.message, "not found");
        assert_eq!(err.details, None);
        assert_eq!(err.retry_after, None);
        assert_eq!(err.to_string(), "Kalshi answered 404: not found");

        let detailed = venue(
            400,
            None,
            r#"{"error":{"code":"invalid_order","message":"bad price","details":"price is 0"}}"#,
        );
        assert_eq!(detailed.code.as_deref(), Some("invalid_order"));
        assert_eq!(detailed.message, "bad price");
        assert_eq!(detailed.details.as_deref(), Some("price is 0"));
    }

    #[test]
    fn a_string_error_gives_its_message_alone() {
        let err = venue(429, None, TOO_MANY);
        assert_eq!(err.status, 429);
        assert_eq!(err.code, None);
        assert_eq!(err.message, "too many requests");
        assert_eq!(err.details, None);
    }

    #[test]
    fn a_msg_body_gives_its_message_alone() {
        let err = venue(400, None, BAD_PARAMETER);
        assert_eq!(err.code, None);
        assert_eq!(err.details, None);
        assert!(
            err.message
                .starts_with("Invalid format for parameter limit"),
            "{err:?}"
        );
        assert!(err.message.ends_with("invalid syntax"), "{err:?}");
        // The shape the spec's first probe recorded.
        let probed = venue(
            400,
            None,
            r#"{"msg":"Parameter validation failed for limit"}"#,
        );
        assert_eq!(probed.message, "Parameter validation failed for limit");
    }

    #[test]
    fn the_status_decides_the_class_before_the_body() {
        let code = |text: &str| Some(Arc::<str>::from(text));
        let refusal = |code| Class::VenueRefusal { code };
        let unavailable = |code| Class::Unavailable { code };
        // (status, body, class, is_fault)
        let rows = [
            (400, BAD_PARAMETER, refusal(None), true),
            (401, UNAUTHENTICATED, Class::Unauthorized, true),
            (403, NOT_FOUND, Class::Unauthorized, true),
            (404, NOT_FOUND, refusal(code("not_found")), true),
            (408, NOT_FOUND, unavailable(code("not_found")), true),
            (418, TOO_MANY, Class::Restricted, true),
            (425, TOO_MANY, unavailable(None), true),
            (
                429,
                TOO_MANY,
                Class::RateLimited { retry_after: None },
                true,
            ),
            (451, NOT_FOUND, Class::Restricted, false),
            (500, NOT_FOUND, unavailable(code("not_found")), true),
            (503, BAD_PARAMETER, unavailable(None), true),
        ];
        for (status, body, class, fault) in rows {
            let err = decoded(status, None, body);
            assert!(
                matches!(err, PredictionsError::Venue(_)),
                "{status}: {err:?}"
            );
            assert_eq!(err.class(), class, "{status}: {err:?}");
            assert_eq!(err.is_fault(), fault, "{status}: {err:?}");
            assert_eq!(err.is_retriable(), class.is_retriable(), "{status}");
        }
    }

    #[test]
    fn a_wait_is_the_responses_and_a_zero_is_none() {
        let limited = decoded(429, Some("7"), TOO_MANY);
        assert_eq!(limited.retry_after(), Some(Duration::from_secs(7)));
        assert_eq!(
            limited.class(),
            Class::RateLimited {
                retry_after: Some(Duration::from_secs(7))
            }
        );

        // Any status's `Retry-After` is the error's wait, though only a 429's class carries it.
        let down = decoded(503, Some("2"), NOT_FOUND);
        assert_eq!(down.retry_after(), Some(Duration::from_secs(2)));
        assert_eq!(down.class().retry_after(), None);

        // Core reads a zero as no wait, and a hand-built zero is no wait either.
        assert_eq!(decoded(429, Some("0"), TOO_MANY).retry_after(), None);
        let mut zero = venue(429, None, TOO_MANY);
        zero.retry_after = Some(Duration::ZERO);
        assert_eq!(Classify::retry_after(&zero), None);
        assert_eq!(zero.class(), Class::RateLimited { retry_after: None });
    }

    #[test]
    fn a_body_in_no_known_shape_is_left_to_core() {
        let bodies = [
            "<html>bad gateway</html>",
            "",
            "[]",
            r#"{"message":"down"}"#,
            r#"{"error":5}"#,
            r#"{"error":{"code":"x"}}"#,
            r#"{"error":{"message":5}}"#,
            r#"{"msg":5}"#,
            r#"{"exchange_active":true,"trading_active":true}"#,
        ];
        for body in bodies {
            let err = decoded(502, None, body);
            assert!(
                matches!(&err, PredictionsError::Api(ApiError::Response(r))
                    if r.status.as_u16() == 502 && r.body == body),
                "{body:?}: {err:?}"
            );
            assert_eq!(err.class(), Class::Unavailable { code: None }, "{body:?}");
            assert!(err.is_retriable(), "{body:?}");
        }
    }

    #[test]
    fn an_error_that_is_not_a_response_is_left_to_core() {
        let sign = ApiError::Sign(Box::new(std::io::Error::other("no key")));
        let rows = [
            ApiError::Validation("refused".into()),
            ApiError::Url(url::ParseError::EmptyHost),
            sign,
        ];
        for err in rows {
            let text = err.to_string();
            let err = PredictionsError::from(err);
            assert!(matches!(err, PredictionsError::Api(_)), "{text}");
            assert_eq!(err.to_string(), text);
            assert_eq!(err.class(), Class::InvalidRequest, "{text}");
            assert!(err.is_fault(), "{text}");
        }
    }

    #[test]
    fn a_451_is_a_region_block_and_not_a_fault_whatever_its_body() {
        let blocked = venue(451, None, NOT_FOUND);
        assert_eq!(blocked.class(), Class::Restricted);
        assert!(!blocked.is_fault());
        assert!(!Classify::is_fault(&decoded(451, None, "")));
    }

    #[test]
    fn predictions_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PredictionsError>();
        assert_send_sync::<VenueError>();
    }
}
```

Declare the module and its re-exports, and complete the `Classify` assertion with the two error types.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
pub mod key;

pub use key::key;
```

Replace with:

```rust
pub mod error;
pub mod key;

pub use error::{PredictionsError, VenueError};
pub use key::key;
```

Current (`polyoxide-kalshi/src/lib.rs`):

```rust
    is::<predictions::key::InvalidTicker>();
```

Replace with:

```rust
    is::<predictions::PredictionsError>();
    is::<predictions::VenueError>();
    is::<predictions::key::InvalidTicker>();
```

- [ ] **Step 2: Run them to see them fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib`
Expected: a compile error: `error[E0432]: unresolved import `error::PredictionsError``, and in the tests `error[E0433]: failed to resolve: use of undeclared type `ApiError``, `cannot find type `VenueError``, `cannot find trait `Classify``.

- [ ] **Step 3: Implement.** The decode reads the response's `body`, never the `message` core derived from it, since core keeps only a string `error` field. A nested `error` needs a string `message`; the other two shapes give a message alone.

Current (`polyoxide-kalshi/src/predictions/error.rs`):

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;
```

Replace with:

```rust
//! Errors of the Predictions module.

use std::time::Duration;

use polyoxide_core::{ApiError, ErrorResponse};
use polyoxide_venue::{class_for_status, Class, Classify};
use serde_json::Value;
use thiserror::Error;

/// Error type for Predictions operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PredictionsError {
    /// Transport, decoding, a refusal made before sending, or an error body in a shape Kalshi does
    /// not use.
    #[error(transparent)]
    Api(ApiError),

    /// Kalshi refused the request with one of its three error bodies.
    #[error(transparent)]
    Venue(VenueError),
}

/// An unsuccessful response whose body is one of Kalshi's three error shapes.
///
/// The status and the `Retry-After` are the response's; the rest is read from the body, which is
/// `{"error": {"code", "message", "details"}}` (a `404` or a `401`), `{"error": "<text>"}` (a
/// `429`) or `{"msg": "<text>"}` (a parameter-validation `400`). The last two give a message
/// alone.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
#[error("Kalshi answered {status}: {message}")]
pub struct VenueError {
    /// The HTTP status.
    pub status: u16,
    /// Kalshi's error code, such as `not_found`, when the body carries one.
    pub code: Option<String>,
    /// Kalshi's message.
    pub message: String,
    /// Kalshi's additional details, when the body carries them.
    pub details: Option<String>,
    /// The response's `Retry-After`, as core reads it: unclamped, and `None` for a zero. Kalshi
    /// sends none with a `429`.
    pub retry_after: Option<Duration>,
}

impl VenueError {
    /// The error `response` carries, when its body is one of the three shapes.
    ///
    /// The body is read, never the message core derived from it, since core keeps only a string
    /// `error` field. A nested `error` without a `message`, an `error` of another type and a body
    /// of any other shape are not Kalshi's, and give `None`.
    fn read(response: &ErrorResponse) -> Option<Self> {
        let body: Value = serde_json::from_str(&response.body).ok()?;
        let fields = body.as_object()?;
        let (code, message, details) = match fields.get("error") {
            Some(Value::Object(error)) => (
                text(error.get("code")),
                text(error.get("message"))?,
                text(error.get("details")),
            ),
            Some(Value::String(message)) => (None, message.clone(), None),
            Some(_) => return None,
            None => (None, text(fields.get("msg"))?, None),
        };
        Some(Self {
            status: response.status.as_u16(),
            code,
            message,
            details,
            retry_after: response.retry_after,
        })
    }
}

/// A body field, when it is a string.
fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

/// Kalshi's one decode: an unsuccessful response whose body is one of its three error shapes is
/// [`Venue`](PredictionsError::Venue), and every other error stays core's
/// [`Api`](PredictionsError::Api), an unsuccessful response with any other body included.
///
/// A signing failure stays `Api(ApiError::Sign(..))`. Foreign errors enter the module only through
/// [`ApiError`], so a `?` on core's error anywhere decodes the same way.
impl From<ApiError> for PredictionsError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Response(response) => match VenueError::read(&response) {
                Some(venue) => Self::Venue(venue),
                None => Self::Api(ApiError::Response(response)),
            },
            other => Self::Api(other),
        }
    }
}

/// `Api` delegates to core.
impl Classify for PredictionsError {
    fn class(&self) -> Class {
        match self {
            Self::Api(err) => err.class(),
            Self::Venue(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        match self {
            Self::Api(err) => err.is_fault(),
            Self::Venue(err) => err.is_fault(),
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Api(err) => Classify::retry_after(err),
            Self::Venue(err) => Classify::retry_after(err),
        }
    }
}

/// The class goes by the status before the body (AD-15), through the workspace's status map:
/// `401` and `403` are `Unauthorized`, `418` and `451` `Restricted`, `429` `RateLimited`, `408`,
/// `425` and every 5xx `Unavailable`, and any other 4xx a `VenueRefusal`; a status outside 400 to
/// 599 is a `Decode`. Kalshi's `code` is the class's code where the class has room for one, and a
/// `429` carries the response's `Retry-After`.
///
/// A `451` is the venue not serving the caller's region, as designed, so it is not a fault; every
/// other status is.
impl Classify for VenueError {
    fn class(&self) -> Class {
        let class = class_for_status(self.status).unwrap_or(Class::Decode);
        let class = match &self.code {
            Some(code) => class.with_code(code.as_str()),
            None => class,
        };
        class.with_retry_after(self.retry_after)
    }

    fn is_fault(&self) -> bool {
        self.status != 451
    }

    fn retry_after(&self) -> Option<Duration> {
        self.retry_after.filter(|wait| !wait.is_zero())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
```

- [ ] **Step 4: Run them to see them pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: PASS: the 9 tests of `predictions::error::tests`, the 10 of `predictions::key::tests` and the key module's doctest.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: clean.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py -q'`
Expected: PASS: `PredictionsError`, `VenueError` and `InvalidTicker` are the public error types of the crate, and all three are listed.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-kalshi/src/predictions/error.rs
git commit -m "feat(kalshi): PredictionsError and its one decode of Kalshi's error bodies (Story 1)" -- polyoxide-kalshi/src/predictions/error.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/lib.rs
```

### Task 1.5: The exchange-status types

`ExchangeStatus` and `ExchangeIndexStatus` model every key of `GET /trade-api/v2/exchange/status` (OpenAPI 3.34.0). Fields the schema makes optional are `Option`, and the index list defaults to empty. Story 2's `types/mod.rs` replaces the two-line block this task writes.

**Files:**
- Create: `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/exchange.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs`
- Test: `polyoxide-kalshi/src/predictions/types/exchange.rs`

- [ ] **Step 1: Write the failing tests.** `exchange.rs` holds only its tests for now: the optional keys' defaults, a null or a given resume time, a body missing a required key (the error names it), and a round trip.

`polyoxide-kalshi/src/predictions/types/mod.rs`:

```rust
mod exchange;

pub use exchange::{ExchangeIndexStatus, ExchangeStatus};
```

`polyoxide-kalshi/src/predictions/types/exchange.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_with_only_the_required_keys_gives_the_optional_ones_their_defaults() {
        let status: ExchangeStatus =
            serde_json::from_str(r#"{"exchange_active":false,"trading_active":false}"#).unwrap();
        assert!(!status.exchange_active);
        assert!(!status.trading_active);
        assert_eq!(status.intra_exchange_transfers_active, None);
        assert_eq!(status.exchange_estimated_resume_time, None);
        assert!(status.exchange_index_statuses.is_empty());
    }

    #[test]
    fn a_null_or_a_given_resume_time_is_read() {
        let body = |resume: &str| {
            format!(
                r#"{{"exchange_active":false,"trading_active":false,"exchange_estimated_resume_time":{resume}}}"#
            )
        };
        let null: ExchangeStatus = serde_json::from_str(&body("null")).unwrap();
        assert_eq!(null.exchange_estimated_resume_time, None);
        let given: ExchangeStatus =
            serde_json::from_str(&body(r#""2026-10-19T14:00:00Z""#)).unwrap();
        assert_eq!(
            given.exchange_estimated_resume_time.as_deref(),
            Some("2026-10-19T14:00:00Z")
        );
    }

    #[test]
    fn a_body_without_a_required_key_is_refused_naming_it() {
        let err =
            serde_json::from_str::<ExchangeStatus>(r#"{"exchange_active":true}"#).unwrap_err();
        assert!(err.to_string().contains("trading_active"), "{err}");
        let err = serde_json::from_str::<ExchangeIndexStatus>(
            r#"{"description":"Default","exchange_active":true,"exchange_index":0,"trading_active":true}"#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("intra_exchange_transfers_active"),
            "{err}"
        );
    }

    #[test]
    fn a_status_serializes_back_to_what_it_was_read_from() {
        let status = ExchangeStatus {
            exchange_active: true,
            trading_active: false,
            intra_exchange_transfers_active: Some(true),
            exchange_estimated_resume_time: Some("2026-10-19T14:00:00Z".to_owned()),
            exchange_index_statuses: vec![ExchangeIndexStatus {
                description: "Default".to_owned(),
                exchange_active: true,
                exchange_index: 0,
                intra_exchange_transfers_active: true,
                trading_active: false,
            }],
        };
        let text = serde_json::to_string(&status).unwrap();
        assert_eq!(
            serde_json::from_str::<ExchangeStatus>(&text).unwrap(),
            status
        );
    }
}
```

Declare the module.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
pub mod error;
pub mod key;

pub use error::{PredictionsError, VenueError};
```

Replace with:

```rust
pub mod error;
pub mod key;
pub mod types;

pub use error::{PredictionsError, VenueError};
```

- [ ] **Step 2: Run them to see them fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib`
Expected: a compile error: `error[E0412]: cannot find type `ExchangeStatus` in this scope` and the same for `ExchangeIndexStatus`.

- [ ] **Step 3: Implement.**

Current (`polyoxide-kalshi/src/predictions/types/exchange.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
//! The exchange's status: `GET /trade-api/v2/exchange/status`.

use serde::{Deserialize, Serialize};

/// The exchange's status, and the status of each of its exchange indexes.
///
/// The top-level fields are the default index's (index 0). Kalshi sends the status route's body
/// with a 500, 503 or 504 too, so a body of this shape does not mean the exchange is up: read
/// [`exchange_active`](Self::exchange_active) and [`trading_active`](Self::trading_active).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExchangeStatus {
    /// False if the exchange is taking no state changes at all: under maintenance.
    pub exchange_active: bool,
    /// True while trading is permitted: during trading hours, and not during a pause.
    pub trading_active: bool,
    /// True while intra-exchange transfers are permitted.
    pub intra_exchange_transfers_active: Option<bool>,
    /// When the current maintenance window is expected to end, as an RFC 3339 time. Not a
    /// guarantee: the window can be extended.
    pub exchange_estimated_resume_time: Option<String>,
    /// The status of each exchange index; empty when Kalshi sends no breakdown.
    #[serde(default)]
    pub exchange_index_statuses: Vec<ExchangeIndexStatus>,
}

/// The status of one exchange index (a shard of the exchange), such as `Default` or `Combos`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExchangeIndexStatus {
    /// What the index is.
    pub description: String,
    /// False if the index is taking no state changes at all.
    pub exchange_active: bool,
    /// The index's number; markets name theirs in `exchange_index`.
    pub exchange_index: u32,
    /// True while the index permits intra-exchange transfers.
    pub intra_exchange_transfers_active: bool,
    /// True while the index permits trading.
    pub trading_active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
```

- [ ] **Step 4: Run them to see them pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: PASS: the 4 tests of `predictions::types::exchange::tests` beside the earlier tasks' 19 and the key doctest.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: clean.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-kalshi/src/predictions/types
git commit -m "feat(kalshi): the exchange-status types (Story 1)" -- polyoxide-kalshi/src/predictions/types polyoxide-kalshi/src/predictions/mod.rs
```

### Task 1.6: The route table and the Read throttle

`request.rs` gets its route table: the `READ` layer, the default cost, `Route`, the eight routes by name, the two doc-hidden accessors that the live drift test (Story 2) reads, and `Route::path`. `throttle.rs` gets the provisional Read throttle (D5) and the per-route costs. This is the landmine task for the throttle's sizing. `PredictionsThrottle` is one `CapacityBucket` made with `CapacityBucket::provisional` at Kalshi's published Basic Read tier, capacity 600 and refill 200 a second, over one unbounded `Hold`. The bucket holds the published burst because Kalshi publishes a bucket and the server is one; a window quota, which core's `WindowQuotaTable` models for Cloudflare's hosts, is the opposite case, and nothing here calls `allow_burst` or aims at a published count. The tests pin the bucket's behaviour against the published numbers, not against the constants: 60 reads of ten tokens are the 600-token burst, the 61st waits 50 ms, and 40 more take two seconds.

The items are used by Task 1.7, not by this task's public API, so the two modules carry `#[allow(dead_code)]` in `mod.rs` until then. Task 1.7 removes both allows.

**Files:**
- Create: `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs`
- Test: `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`

- [ ] **Step 1: Write the failing tests.** Each file holds only its tests for now. The route table's tests pin the eight routes by their method and template, that every one is written from the API root and none twice, that `Route::path` fills and encodes its parameters, and that `template_matches` is segment by segment, with a literal never matching a parameter and a listed `*name` taking at least one segment. The throttle's tests run on a paused clock: the published burst and refill rate, a cap on an idle bucket, the charge each request leaves, a cost on another layer, the provisional wait and its two ends (confirmed, refused; resized, charged), both orders of a hold against a wait for tokens, and the per-route costs.

`polyoxide-kalshi/src/predictions/request.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_route_table_is_the_epics_eight_routes() {
        let templates: Vec<(&str, &str)> = route_templates().collect();
        assert_eq!(
            templates,
            [
                ("GET", "/trade-api/v2/exchange/status"),
                ("GET", "/trade-api/v2/markets"),
                ("GET", "/trade-api/v2/markets/:ticker"),
                ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
                ("GET", "/trade-api/v2/markets/trades"),
                ("GET", "/trade-api/v2/events/:event_ticker"),
                ("GET", "/trade-api/v2/account/endpoint_costs"),
                ("GET", "/trade-api/v2/account/limits"),
            ]
        );
    }

    #[test]
    fn every_route_is_written_from_the_api_root_and_none_is_listed_twice() {
        let mut seen = std::collections::HashSet::new();
        for route in routes::ALL {
            assert!(route.template.starts_with("/trade-api/v2/"), "{route:?}");
            assert!(seen.insert((route.method, route.template)), "{route:?}");
        }
    }

    #[test]
    fn a_route_costs_the_ten_tokens_kalshi_publishes_as_its_default() {
        // `default_cost` of `GET /account/endpoint_costs`, probed on 2026-10-18.
        assert_eq!(DEFAULT_COST, 10);
    }

    #[test]
    fn a_path_replaces_each_parameter_segment_in_order() {
        let orderbook = Route::get("/trade-api/v2/markets/:ticker/orderbook");
        assert_eq!(
            orderbook.path(&["KXBTCD-26OCT1806-T82099.99"]),
            "/trade-api/v2/markets/KXBTCD-26OCT1806-T82099.99/orderbook"
        );
        assert_eq!(
            Route::get("/a/:first/b/:second").path(&["1", "2"]),
            "/a/1/b/2"
        );
        assert_eq!(
            routes::EXCHANGE_STATUS.path(&[]),
            "/trade-api/v2/exchange/status"
        );
    }

    #[test]
    fn a_parameter_cannot_add_a_segment_or_start_a_query() {
        let event = routes::EVENT;
        assert_eq!(
            event.path(&["a/b?c d%é"]),
            "/trade-api/v2/events/a%2Fb%3Fc%20d%25%C3%A9"
        );
        // The unreserved characters stay as they are.
        assert_eq!(event.path(&["Az09-._~"]), "/trade-api/v2/events/Az09-._~");
    }

    #[test]
    fn a_missing_parameter_leaves_its_segment_and_an_extra_one_is_ignored() {
        let two = Route::get("/a/:first/b/:second");
        assert_eq!(two.path(&["1"]), "/a/1/b/:second");
        assert_eq!(two.path(&["1", "2", "3"]), "/a/1/b/2");
        assert_eq!(Route::get("/a/b").path(&["1"]), "/a/b");
    }

    #[test]
    fn templates_match_segment_by_segment() {
        let rows = [
            ("/markets/:ticker", "/markets/:ticker", true),
            // A parameter Kalshi spells differently still matches.
            ("/markets/:ticker/orderbook", "/markets/:id/orderbook", true),
            // A literal segment is not a parameter, either way round.
            ("/markets/trades", "/markets/:ticker", false),
            ("/markets/:ticker", "/markets/trades", false),
            ("/markets/a", "/markets/b", false),
            // The segments must be as many.
            ("/markets", "/markets/:ticker", false),
            ("/markets/:ticker", "/markets", false),
            ("/markets/:ticker/orderbook", "/markets/:ticker", false),
            // A listed `*name` takes every remaining segment, and at least one.
            ("/cf/btc", "/cf/*endpoint", true),
            ("/cf/btc/spot", "/cf/*endpoint", true),
            ("/cf/:name/spot", "/cf/*endpoint", true),
            ("/cf", "/cf/*endpoint", false),
            ("/markets/btc", "/cf/*endpoint", false),
        ];
        for (ours, listed, matches) in rows {
            assert_eq!(template_matches(ours, listed), matches, "{ours} / {listed}");
        }
    }

    #[test]
    fn our_routes_find_themselves_and_not_each_other() {
        for route in routes::ALL {
            for other in routes::ALL {
                assert_eq!(
                    template_matches(route.template, other.template),
                    route == other,
                    "{} / {}",
                    route.template,
                    other.template
                );
            }
        }
    }
}
```

`polyoxide-kalshi/src/predictions/throttle.rs`:

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
    use polyoxide_core::{Cost, LayerId};
    use tokio::time::Instant;

    use super::*;

    fn read(units: u32) -> Cost {
        Cost {
            layer: READ,
            units,
            exact: true,
        }
    }

    /// Charge one request costing `costs`, as the send loop would.
    async fn charge(throttle: &PredictionsThrottle, costs: &[Cost]) -> Result<Charge, Refused> {
        let method = Method::GET;
        throttle
            .acquire(&RequestMeta {
                method: &method,
                path: "/trade-api/v2/markets",
                query: &[],
                costs,
            })
            .await
    }

    /// Charge `count` reads of ten tokens.
    async fn spend(throttle: &PredictionsThrottle, count: u32) {
        for _ in 0..count {
            charge(throttle, &[read(10)]).await.unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_bucket_holds_basics_published_burst_and_refills_at_its_published_rate() {
        // Basic's Read refills 200 tokens a second and holds 3 seconds of it: 600.
        let throttle = PredictionsThrottle::new();
        let start = Instant::now();
        spend(&throttle, 60).await;
        assert_eq!(
            start.elapsed(),
            Duration::ZERO,
            "60 reads are the 600 tokens"
        );

        // The 61st finds it empty, and ten tokens at 200 a second take 50 ms.
        spend(&throttle, 1).await;
        let waited = start.elapsed();
        assert!(
            (Duration::from_millis(50)..Duration::from_millis(60)).contains(&waited),
            "the 61st read waited {waited:?}"
        );

        // Then 400 more tokens take two seconds, not one: the burst is not spent twice.
        let resumed = Instant::now();
        spend(&throttle, 40).await;
        let waited = resumed.elapsed();
        assert!(
            (Duration::from_millis(1900)..Duration::from_millis(2100)).contains(&waited),
            "40 reads waited {waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_idle_bucket_never_holds_more_than_its_capacity() {
        let throttle = PredictionsThrottle::new();
        spend(&throttle, 60).await;
        tokio::time::sleep(Duration::from_secs(60)).await;

        let start = Instant::now();
        spend(&throttle, 60).await;
        assert_eq!(start.elapsed(), Duration::ZERO);
        spend(&throttle, 1).await;
        assert!(
            start.elapsed() >= Duration::from_millis(50),
            "a minute idle filled the bucket past 600"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_charge_names_the_layer_and_the_units() {
        let throttle = PredictionsThrottle::new();
        let charged = charge(&throttle, &[read(30)]).await.unwrap();
        assert_eq!(
            charged.layers(),
            [LayerCharge {
                layer: READ,
                units: 30,
                window: None
            }]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cost_on_another_layer_is_not_this_throttles() {
        let throttle = PredictionsThrottle::new();
        let elsewhere = Cost {
            layer: LayerId("elsewhere"),
            units: 1_000_000,
            exact: true,
        };
        let start = Instant::now();
        let charged = charge(&throttle, &[elsewhere]).await.unwrap();
        assert!(charged.layers().is_empty());
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cost_above_the_provisional_capacity_waits_for_the_sizing() {
        let throttle = PredictionsThrottle::new();
        let costs = [read(BASIC_READ_CAPACITY + 1)];

        // Neither charged nor refused while the sizing is a guess.
        let early = tokio::time::timeout(Duration::from_secs(5), charge(&throttle, &costs)).await;
        assert!(early.is_err(), "settled before the sizing was confirmed");

        // Confirmed at 600, it is a cost the bucket can never hold: refused at once.
        throttle.bucket().confirm();
        assert_eq!(
            charge(&throttle, &costs).await.unwrap_err(),
            Refused {
                layer: READ,
                units: 601,
                capacity: 600
            }
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_resize_releases_a_cost_waiting_for_the_sizing() {
        let throttle = PredictionsThrottle::new();
        let costs = [read(BASIC_READ_CAPACITY + 1)];
        let waiting = tokio::spawn({
            let throttle = throttle.clone();
            async move { charge(&throttle, &costs).await }
        });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        throttle.bucket().resize(1_000, 400);
        let charged = waiting.await.unwrap().unwrap();
        assert_eq!(charged.layers().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_is_waited_out_before_a_request_is_charged() {
        let throttle = PredictionsThrottle::new();
        throttle.hold(Duration::from_secs(1));
        let start = Instant::now();
        spend(&throttle, 1).await;
        let waited = start.elapsed();
        assert!(
            (Duration::from_secs(1)..Duration::from_millis(1010)).contains(&waited),
            "waited {waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_set_while_a_request_waits_for_tokens_is_waited_out_after_the_wait() {
        let throttle = PredictionsThrottle::new();
        spend(&throttle, 60).await;
        let start = Instant::now();
        let waiting = tokio::spawn({
            let throttle = throttle.clone();
            async move { spend(&throttle, 1).await }
        });

        // The request is waiting 50 ms for tokens when a 429 holds everyone for a second.
        tokio::time::sleep(Duration::from_millis(10)).await;
        throttle.hold(Duration::from_secs(1));
        waiting.await.unwrap();
        let waited = start.elapsed();
        assert!(
            (Duration::from_millis(1010)..Duration::from_millis(1030)).contains(&waited),
            "waited {waited:?}"
        );
    }

    #[test]
    fn a_route_costs_the_default_until_it_is_set() {
        let costs = RouteCosts::default();
        let status = Route::get("/trade-api/v2/exchange/status");
        let markets = Route::get("/trade-api/v2/markets");
        assert_eq!(costs.cost(&status), 10);

        costs.set(&status, 30);
        assert_eq!(costs.cost(&status), 30);
        assert_eq!(costs.cost(&markets), 10, "another route's cost moved");

        costs.set(&status, 2);
        assert_eq!(costs.cost(&status), 2);
    }

    #[test]
    fn a_cost_belongs_to_a_method_and_a_template() {
        let costs = RouteCosts::default();
        let get = Route::get("/x");
        let post = Route {
            method: "POST",
            template: "/x",
        };
        costs.set(&post, 7);
        assert_eq!(costs.cost(&post), 7);
        assert_eq!(costs.cost(&get), 10);
    }
}
```

Declare the two modules. Both carry `#[allow(dead_code)]` until Task 1.7.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
pub mod error;
pub mod key;
pub mod types;

pub use error::{PredictionsError, VenueError};
```

Replace with:

```rust
pub mod error;
pub mod key;
// Task 1.7 uses what these two modules define, and removes both allows.
#[allow(dead_code)]
pub mod request;
#[allow(dead_code)]
mod throttle;
pub mod types;

pub use error::{PredictionsError, VenueError};
```

- [ ] **Step 2: Run them to see them fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib`
Expected: a compile error: `error[E0425]: cannot find function `route_templates` in this scope`, `cannot find value `routes``, `cannot find type `Route``, `cannot find type `PredictionsThrottle``, `cannot find type `RouteCosts``, `cannot find value `READ``.

- [ ] **Step 3: Implement.** The route table first.

Current (`polyoxide-kalshi/src/predictions/request.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
//! What every route builder sends through: the route table, and core's request builder with the
//! route's cost on the Read layer.

use std::fmt::Write as _;

use polyoxide_core::LayerId;

/// The throttle layer every request of this module is charged to: Kalshi's Read bucket.
pub(crate) const READ: LayerId = LayerId("predictions.read");

/// What a route costs, in tokens, until `GET /account/endpoint_costs` says otherwise: the default
/// cost Kalshi publishes there.
#[doc(hidden)]
pub const DEFAULT_COST: u32 = 10;

/// A route: an HTTP method and a path template in the spelling of `GET /account/endpoint_costs`,
/// where `:name` stands for a segment that varies.
///
/// Every path begins `/trade-api/v2/`, the API root, whatever the host, because that is the path
/// Kalshi's signature covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) method: &'static str,
    pub(crate) template: &'static str,
}

impl Route {
    /// A `GET` route.
    pub(crate) const fn get(template: &'static str) -> Self {
        Self {
            method: "GET",
            template,
        }
    }

    /// The route's path with each `:name` segment replaced by the next of `params`, in order.
    ///
    /// A parameter is percent-encoded, every byte but an ASCII letter, a digit and `-._~`, so a
    /// caller's text can never add a segment or start a query. A `:name` segment with no parameter
    /// is left as it is, and a parameter with no segment is ignored.
    pub(crate) fn path(&self, params: &[&str]) -> String {
        let mut params = params.iter();
        let segments: Vec<String> = self
            .template
            .split('/')
            .map(|segment| {
                if segment.starts_with(':') {
                    params
                        .next()
                        .map_or_else(|| segment.to_owned(), |param| encode(param))
                } else {
                    segment.to_owned()
                }
            })
            .collect();
        segments.join("/")
    }
}

/// `param`, percent-encoded as one path segment.
fn encode(param: &str) -> String {
    let mut encoded = String::with_capacity(param.len());
    for byte in param.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

/// The eight routes of this module, by name. No later story adds one.
pub(crate) mod routes {
    use super::Route;

    pub(crate) const EXCHANGE_STATUS: Route = Route::get("/trade-api/v2/exchange/status");
    pub(crate) const MARKETS: Route = Route::get("/trade-api/v2/markets");
    pub(crate) const MARKET: Route = Route::get("/trade-api/v2/markets/:ticker");
    pub(crate) const ORDERBOOK: Route = Route::get("/trade-api/v2/markets/:ticker/orderbook");
    pub(crate) const TRADES: Route = Route::get("/trade-api/v2/markets/trades");
    pub(crate) const EVENT: Route = Route::get("/trade-api/v2/events/:event_ticker");
    pub(crate) const ENDPOINT_COSTS: Route = Route::get("/trade-api/v2/account/endpoint_costs");
    pub(crate) const ACCOUNT_LIMITS: Route = Route::get("/trade-api/v2/account/limits");

    /// Every route above.
    pub(crate) const ALL: &[Route] = &[
        EXCHANGE_STATUS,
        MARKETS,
        MARKET,
        ORDERBOOK,
        TRADES,
        EVENT,
        ENDPOINT_COSTS,
        ACCOUNT_LIMITS,
    ];
}

/// Every route this module sends, as its method and its path template.
///
/// The live suite's drift test reads the table through this and `template_matches`; nothing else
/// should.
#[doc(hidden)]
pub fn route_templates() -> impl Iterator<Item = (&'static str, &'static str)> {
    routes::ALL
        .iter()
        .map(|route| (route.method, route.template))
}

/// Whether two path templates name the same route, segment by segment.
///
/// Two templates match when they have as many `/`-separated segments and each pair is equal or both
/// begin with `:`, so a parameter Kalshi spells differently still matches. As an exception, a
/// segment of `listed` beginning with `*` matches all the remaining segments of `ours`, at least
/// one, as `/trade-api/v2/cfbenchmarks/*endpoint` does.
#[doc(hidden)]
pub fn template_matches(ours: &str, listed: &str) -> bool {
    let ours: Vec<&str> = ours.split('/').collect();
    let listed: Vec<&str> = listed.split('/').collect();
    for (index, theirs) in listed.iter().enumerate() {
        if theirs.starts_with('*') {
            return ours.len() > index;
        }
        match ours.get(index) {
            Some(mine) if mine == theirs || (mine.starts_with(':') && theirs.starts_with(':')) => {}
            _ => return false,
        }
    }
    ours.len() == listed.len()
}

#[cfg(test)]
mod tests {
    use super::*;
```

Then the throttle.

Current (`polyoxide-kalshi/src/predictions/throttle.rs`):

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
```

Replace with:

```rust
//! The Read throttle: one token bucket, sized at Kalshi's Basic tier until the client has read the
//! account's own size, and the per-route costs it charges.
//!
//! Kalshi charges every authenticated request tokens from a Read or a Write bucket, 10 by default.
//! This module sends reads only. A bucket here holds its published capacity: Kalshi publishes a
//! token bucket's capacity and refill rate (Basic's Read refills 200 tokens a second and holds 3
//! seconds of it), and the server implements a bucket, so a bucket of that capacity is a faithful
//! model. That is the opposite of Cloudflare's window quotas, which publish a count per window with
//! no capacity term and must not be given a depth.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, CapacityBucket, Charge, Hold, LayerCharge, Refused, RequestMeta, ResponseMeta,
    Throttle,
};

use crate::predictions::request::{Route, DEFAULT_COST, READ};

/// Tokens the Basic tier's Read bucket holds: 3 seconds of its refill.
pub(crate) const BASIC_READ_CAPACITY: u32 = 600;

/// Tokens a second the Basic tier's Read bucket refills. No tier's is lower.
pub(crate) const BASIC_READ_REFILL: u32 = 200;

/// Charges each request its cost on the Read layer, and holds every request when the retry policy
/// says to.
///
/// The bucket is [provisional](CapacityBucket::provisional): a cost above its capacity waits for
/// the sizing to be confirmed or replaced instead of being refused. The hold is unbounded, since
/// every hold this module's policy sets is the loop's own `retry_delay(0)`, which
/// `RetryConfig::max_backoff_ms` bounds.
///
/// A clone shares the bucket and the hold.
#[derive(Debug, Clone)]
pub(crate) struct PredictionsThrottle {
    read: CapacityBucket,
    hold: Hold,
}

impl PredictionsThrottle {
    /// A throttle with a full Read bucket at Basic's sizing.
    pub(crate) fn new() -> Self {
        let hold = Hold::unbounded();
        Self {
            read: CapacityBucket::provisional(
                READ,
                BASIC_READ_CAPACITY,
                BASIC_READ_REFILL,
                hold.clone(),
            ),
            hold,
        }
    }

    /// The Read bucket, for resizing or confirming it in place.
    pub(crate) fn bucket(&self) -> &CapacityBucket {
        &self.read
    }
}

impl Throttle for PredictionsThrottle {
    /// Waits out the hold, charges each cost on the Read layer (a cost on any other layer is not
    /// this throttle's), and waits out the hold again, since one set while the bucket made this
    /// request wait for tokens still applies (AD-23).
    async fn acquire(&self, meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        self.hold.wait().await;
        let mut charge = Charge::none();
        for cost in meta.costs {
            if cost.layer != READ {
                continue;
            }
            self.read.acquire(cost.units, cost.exact).await?;
            charge = charge.with(LayerCharge {
                layer: READ,
                units: cost.units,
                window: None,
            });
        }
        self.hold.wait().await;
        Ok(charge)
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {}

    fn hold(&self, delay: Duration) {
        self.hold.extend(delay);
    }
}

/// What each route costs the Read layer, when that is not the default.
///
/// The compiled costs are the default, 10, for every route. The first-use confirmation replaces
/// them with what `GET /account/endpoint_costs` lists, so a cost is read when a request is sent,
/// not when its builder is made.
#[derive(Debug, Default)]
pub(crate) struct RouteCosts {
    overrides: RwLock<HashMap<(&'static str, &'static str), u32>>,
}

impl RouteCosts {
    /// What `route` costs: its override, else the default cost.
    pub(crate) fn cost(&self, route: &Route) -> u32 {
        self.overrides
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&(route.method, route.template))
            .copied()
            .unwrap_or(DEFAULT_COST)
    }

    /// Makes `route` cost `cost` from now on.
    pub(crate) fn set(&self, route: &Route, cost: u32) {
        self.overrides
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert((route.method, route.template), cost);
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
```

- [ ] **Step 4: Run them to see them pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: PASS: the 8 tests of `predictions::request::tests` and the 10 of `predictions::throttle::tests`, beside the earlier tasks' 23 and the key doctest. The throttle's tests run on a paused clock, so they take well under a second.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: clean.

- [ ] **Step 5: Commit.**

```bash
git add -N polyoxide-kalshi/src/predictions/request.rs polyoxide-kalshi/src/predictions/throttle.rs
git commit -m "feat(kalshi): the route table and the provisional Read throttle (Story 1)" -- polyoxide-kalshi/src/predictions/request.rs polyoxide-kalshi/src/predictions/throttle.rs polyoxide-kalshi/src/predictions/mod.rs
```

### Task 1.7: The client, `health().ping()` and `exchange().status()`

This task builds `Predictions` and its shared `Inner`, the builder, `Routed<T>`, the `before_send` stub, the two namespaces and their builders, and drops the temporary allows of Task 1.6. The builder refuses a base URL with a path (D4) and installs the throttle on core's `HttpClient` builder, which leaves core's `DefaultRetryPolicy` in place (D6): it retries a `429`, holds every request, and fails every other status. `Routed::send` follows the spec's four steps: await `before_send`, build core's request with the route's cost read at that point, add the authenticator when the client holds one and the query pairs, send. `health().ping()` calls `HttpClient::health`, which takes no authenticator of the venue's, so a ping is never signed. `mod.rs` names each namespace by its path through `api` in the `namespaces!` block (the macro takes a `path`), so it holds no `use` line for the namespaces and Story 2's replacement of the block adds none. `Route::get`'s routes are all `GET`, which core's `Request` is until told otherwise.

The `before_send` stub is exactly the text the names contract gives, and Story 4 replaces it as a whole. `RouteCosts::set` and `Routed::has_key` are used only by Story 4, and `Routed::query` only by Story 2's query setters, so each keeps a targeted `#[allow(dead_code)]` with its reason.

**Files:**
- Modify: `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`
- Create: `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/api/exchange.rs`
- Create: `polyoxide-kalshi/tests/headers_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-kalshi/tests/error_classes_predictions.rs`
- Modify: `CLAUDE.md:695`
- Test: the three integration files, and the inline tests of `mod.rs` and `request.rs`

- [ ] **Step 1: Write the failing tests.** Three integration suites, each with a `[[test]]` stanza, and inline tests in `mod.rs` and `request.rs`.

`headers_predictions.rs` is the header pin: one unauthenticated `GET` of the status route sends exactly `accept: */*`, `accept-encoding: gzip` and `host`, nothing else, as a literal that is never read from the client. CI runs it in three builds in Task 1.8.

`mock_predictions.rs` reads the status body that the production host answered on 2026-10-18, byte for byte, through `exchange().status()` and `health().ping()`; refuses a base URL with a path; decodes each of Kalshi's error shapes into its `VenueError` and class; and holds that a `503` keeps the status body as core's error and is not retried (AD-17).

`error_classes_predictions.rs` runs every status of the map, with a body Kalshi does not use and with one it does, and reads the class, the fault flag and the wait through `Classify` alone.

Current (`polyoxide-kalshi/Cargo.toml`):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
[package.metadata.polyoxide]
```

Replace with:

```toml
# The predictions module's suites. `required-features` keeps a plain
# `cargo test --all-targets` from building them without the module.
[[test]]
name = "error_classes_predictions"
required-features = ["predictions"]

[[test]]
name = "headers_predictions"
required-features = ["predictions"]

[[test]]
name = "mock_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
[package.metadata.polyoxide]
```

`polyoxide-kalshi/tests/headers_predictions.rs`:

```rust
//! The full header set one request goes out with (AD-18).
//!
//! The expected set is a literal, never read from the client under test. CI runs this file in this
//! crate alone with `--no-default-features --features predictions`, in the workspace build, and
//! again beside the umbrella with `--features polyoxide/full`, so a change to what a client sends
//! fails here in one of the three builds, whether a code default (0.37.0's gzip regression was
//! core's builder turning gzip off) or a reqwest feature another member, or another venue's
//! module, switches on.

use polyoxide_kalshi::predictions::Predictions;
use polyoxide_test_support::query::headers_sent;

/// One unauthenticated `GET /trade-api/v2/exchange/status`, and the headers it carried.
#[tokio::test]
async fn a_status_request_sends_exactly_the_default_headers() {
    let mut base = String::new();
    let headers = headers_sent("/trade-api/v2/exchange/status", |url| {
        base = url.clone();
        async move {
            let kalshi = Predictions::builder().base_url(url).build().unwrap();
            let _ = kalshi.exchange().status().send().await;
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
        "`exchange().status()` sent a header set other than reqwest's defaults with gzip"
    );
}
```

`polyoxide-kalshi/tests/mock_predictions.rs`:

```rust
//! The exchange-status route, the ping, the builder's refusal of a base URL with a path, and
//! Kalshi's three error bodies, on a mock server.
//!
//! The status body is the one the production host answered on 2026-10-18, recorded in
//! `docs/specs/kalshi/OBSERVED.md`.

use std::sync::Arc;

use mockito::{Matcher, Server, ServerGuard};
use polyoxide_core::{ApiError, RetryConfig};
use polyoxide_kalshi::predictions::{Predictions, PredictionsError};
use polyoxide_venue::{Class, Classify};

const STATUS: &str = "/trade-api/v2/exchange/status";

/// `GET /trade-api/v2/exchange/status`, byte for byte as the production host sent it.
const STATUS_BODY: &str = concat!(
    r#"{"exchange_active":true,"exchange_index_statuses":["#,
    r#"{"description":"Default","exchange_active":true,"exchange_index":0,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true},"#,
    r#"{"description":"Combos","exchange_active":true,"exchange_index":1,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true},"#,
    r#"{"description":"Crypto & Commodities","exchange_active":true,"exchange_index":2,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true},"#,
    r#"{"description":"Tennis, Baseball, Basketball","exchange_active":true,"exchange_index":3,"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true}],"#,
    r#""intra_exchange_transfers_active":true,"trading_active":true}"#,
);

const NOT_FOUND: &str = r#"{"error":{"code":"not_found","message":"not found"}}"#;
const UNAUTHENTICATED: &str = concat!(
    r#"{"error":{"code":"token_authentication_failure","#,
    r#""message":"token authentication failure"}}"#,
);
const BAD_PARAMETER: &str = concat!(
    r#"{"msg":"Invalid format for parameter limit: error binding string parameter: "#,
    r#"strconv.ParseInt: parsing \"abc\": invalid syntax"}"#,
);
const TOO_MANY: &str = r#"{"error":"too many requests"}"#;

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

/// A client that does not retry, so a `429` or a hold cannot reach the next case.
fn client_without_retries(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap()
}

/// `GET` the status route, answered with `status` and `body`, on a client that does not retry.
async fn fail(status: usize, body: &str) -> PredictionsError {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", STATUS)
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body)
        .create_async()
        .await;
    client_without_retries(&server)
        .exchange()
        .status()
        .send()
        .await
        .unwrap_err()
}

#[tokio::test]
async fn exchange_status_reads_the_body_the_host_sent() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_header("content-type", "application/json; charset=utf-8")
        .with_body(STATUS_BODY)
        .create_async()
        .await;

    let status = client(&server).exchange().status().send().await.unwrap();
    mock.assert_async().await;

    assert!(status.exchange_active);
    assert!(status.trading_active);
    assert_eq!(status.intra_exchange_transfers_active, Some(true));
    assert_eq!(status.exchange_estimated_resume_time, None);
    let rows: Vec<_> = status
        .exchange_index_statuses
        .iter()
        .map(|row| (row.exchange_index, row.description.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (0, "Default"),
            (1, "Combos"),
            (2, "Crypto & Commodities"),
            (3, "Tennis, Baseball, Basketball"),
        ]
    );
    assert!(status.exchange_index_statuses.iter().all(|row| {
        row.exchange_active && row.trading_active && row.intra_exchange_transfers_active
    }));
}

#[tokio::test]
async fn ping_answers_from_the_status_route_and_leaves_the_body_unread() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_body(STATUS_BODY)
        .expect(1)
        .create_async()
        .await;

    let pong = client(&server).health().ping().await.unwrap();
    assert_eq!(pong.response.status().as_u16(), 200);
    assert_eq!(pong.response.text().await.unwrap(), STATUS_BODY);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_ping_the_host_refuses_is_the_error_kalshis_decode_makes_of_it() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", STATUS)
        .with_status(404)
        .with_body(NOT_FOUND)
        .create_async()
        .await;

    let err = client_without_retries(&server)
        .health()
        .ping()
        .await
        .unwrap_err();
    assert!(matches!(err, PredictionsError::Venue(_)), "{err:?}");
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(Arc::from("not_found"))
        }
    );
}

#[test]
fn building_with_a_path_in_the_base_url_is_refused() {
    for url in [
        "http://127.0.0.1:1/proxy",
        "http://127.0.0.1:1/proxy/",
        "https://external-api.kalshi.com/trade-api/v2",
    ] {
        let err = Predictions::builder().base_url(url).build().unwrap_err();
        let expected = format!("the base URL must be a scheme and host with no path: {url}");
        assert!(
            matches!(&err, PredictionsError::Api(ApiError::Validation(message))
                if *message == expected),
            "{url}: {err:?}"
        );
        assert_eq!(err.class(), Class::InvalidRequest, "{url}");
    }
}

#[test]
fn a_bare_host_is_a_base_url_with_or_without_its_slash() {
    for url in ["http://127.0.0.1:1", "http://127.0.0.1:1/"] {
        assert!(
            Predictions::builder().base_url(url).build().is_ok(),
            "{url}"
        );
    }
}

#[tokio::test]
async fn each_error_shape_decodes_into_a_venue_error_with_its_class() {
    // (status, body, code, message, class)
    let rows = [
        (
            401,
            UNAUTHENTICATED,
            Some("token_authentication_failure"),
            "token authentication failure",
            Class::Unauthorized,
        ),
        (
            404,
            NOT_FOUND,
            Some("not_found"),
            "not found",
            Class::VenueRefusal {
                code: Some(Arc::from("not_found")),
            },
        ),
        (
            400,
            BAD_PARAMETER,
            None,
            "Invalid format for parameter limit: error binding string parameter: \
             strconv.ParseInt: parsing \"abc\": invalid syntax",
            Class::VenueRefusal { code: None },
        ),
        (
            429,
            TOO_MANY,
            None,
            "too many requests",
            Class::RateLimited { retry_after: None },
        ),
    ];
    for (status, body, code, message, class) in rows {
        let err = fail(status, body).await;
        let PredictionsError::Venue(venue) = &err else {
            panic!("{status}: not a Kalshi error: {err:?}");
        };
        assert_eq!(usize::from(venue.status), status);
        assert_eq!(venue.code.as_deref(), code, "{status}");
        assert_eq!(venue.message, message, "{status}");
        assert_eq!(venue.details, None, "{status}");
        assert_eq!(venue.retry_after, None, "{status}");
        assert_eq!(err.class(), class, "{status}");
    }
}

#[tokio::test]
async fn a_503_with_the_status_body_stays_cores_and_is_unavailable() {
    let err = fail(503, STATUS_BODY).await;
    assert!(
        matches!(&err, PredictionsError::Api(ApiError::Response(response))
            if response.status.as_u16() == 503 && response.body == STATUS_BODY),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::Unavailable { code: None });
    assert!(err.is_retriable());
}

#[tokio::test]
async fn a_503_is_not_retried() {
    // Core's default policy retries a 429 and fails every other status (AD-17).
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", STATUS)
        .with_status(503)
        .with_body(NOT_FOUND)
        .expect(1)
        .create_async()
        .await;

    let err = client(&server)
        .exchange()
        .status()
        .send()
        .await
        .unwrap_err();
    assert!(matches!(err, PredictionsError::Venue(_)), "{err:?}");
    mock.assert_async().await;
}
```

`polyoxide-kalshi/tests/error_classes_predictions.rs`:

```rust
//! The class each status reaches a caller with, read through `Classify` alone.
//!
//! One request per status, each on a fresh client with no retries, so a `429` hold cannot reach
//! the next case. A status decides the class before the body does (AD-15), so every row is run
//! with a body Kalshi does not use as well as with one it does.

use std::sync::Arc;
use std::time::Duration;

use mockito::{Matcher, Server};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::key::{InvalidTicker, Ticker};
use polyoxide_kalshi::predictions::{Predictions, PredictionsError};
use polyoxide_venue::{Class, Classify};

const STATUS: &str = "/trade-api/v2/exchange/status";

/// `GET` the status route, answered with `status`, `body` and, when given, a `Retry-After`, on a
/// client that does not retry.
async fn fail(status: usize, body: &str, retry_after: Option<&str>) -> PredictionsError {
    let mut server = Server::new_async().await;
    let mut mock = server
        .mock("GET", STATUS)
        .match_query(Matcher::Any)
        .with_status(status)
        .with_body(body);
    if let Some(wait) = retry_after {
        mock = mock.with_header("retry-after", wait);
    }
    let _mock = mock.create_async().await;
    let kalshi = Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap();
    kalshi.exchange().status().send().await.unwrap_err()
}

#[tokio::test]
async fn every_status_reaches_the_caller_with_its_class() {
    let refusal = Class::VenueRefusal { code: None };
    let unavailable = Class::Unavailable { code: None };
    // (status, class, is_fault). Only a 451 is the venue answering as designed.
    let rows = [
        (400, refusal.clone(), true),
        (401, Class::Unauthorized, true),
        (403, Class::Unauthorized, true),
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
        // A body that is not Kalshi's: core's error, classed by its status.
        let plain = fail(status, "<html>x</html>", None).await;
        assert!(
            matches!(plain, PredictionsError::Api(_)),
            "{status}: {plain:?}"
        );
        assert_eq!(plain.class(), class, "{status}: {plain:?}");
        assert_eq!(plain.is_fault(), fault, "{status}: {plain:?}");

        // One that is: Kalshi's error, classed by the same status.
        let venue = fail(status, r#"{"error":"x"}"#, None).await;
        assert!(
            matches!(venue, PredictionsError::Venue(_)),
            "{status}: {venue:?}"
        );
        assert_eq!(venue.class(), class, "{status}: {venue:?}");
        assert_eq!(venue.is_fault(), fault, "{status}: {venue:?}");
        assert_eq!(venue.is_retriable(), class.is_retriable(), "{status}");
    }
}

#[tokio::test]
async fn kalshis_code_reaches_the_class_where_the_class_has_room_for_one() {
    let err = fail(
        404,
        r#"{"error":{"code":"not_found","message":"not found"}}"#,
        None,
    )
    .await;
    assert_eq!(
        err.class(),
        Class::VenueRefusal {
            code: Some(Arc::from("not_found"))
        },
        "{err:?}"
    );
    assert!(err.is_fault(), "{err:?}");
    assert!(!err.is_retriable(), "{err:?}");

    // `Unauthorized` has nowhere to put a code.
    let denied = fail(
        401,
        r#"{"error":{"code":"token_authentication_failure","message":"x"}}"#,
        None,
    )
    .await;
    assert_eq!(denied.class(), Class::Unauthorized, "{denied:?}");
}

#[tokio::test]
async fn a_retry_after_is_the_errors_wait_whichever_shape_the_body_has() {
    let wait = Some(Duration::from_secs(7));
    for body in [r#"{"error":"too many requests"}"#, "<html>x</html>"] {
        let limited = fail(429, body, Some("7")).await;
        assert_eq!(limited.retry_after(), wait, "{body}");
        assert_eq!(
            limited.class(),
            Class::RateLimited { retry_after: wait },
            "{body}"
        );
        assert!(limited.is_retriable(), "{body}");
    }

    // A 503 carries the wait too, though its class has no room for it.
    let down = fail(503, r#"{"error":"x"}"#, Some("2")).await;
    assert_eq!(down.retry_after(), Some(Duration::from_secs(2)));
    assert_eq!(down.class(), Class::Unavailable { code: None });

    // Kalshi sends none with a 429, and a zero is no wait.
    assert_eq!(fail(429, "x", None).await.retry_after(), None);
    assert_eq!(fail(429, "x", Some("0")).await.retry_after(), None);
}

#[tokio::test]
async fn a_2xx_body_that_does_not_decode_is_decode() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", STATUS)
        .with_status(200)
        .with_body("not json")
        .create_async()
        .await;
    let kalshi = Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap();
    let err = kalshi.exchange().status().send().await.unwrap_err();
    assert_eq!(err.class(), Class::Decode, "{err:?}");
    assert!(err.is_fault(), "{err:?}");
}

#[test]
fn a_refusal_made_before_sending_is_an_invalid_request() {
    let path = Predictions::builder()
        .base_url("http://127.0.0.1:1/proxy")
        .build()
        .unwrap_err();
    assert_eq!(path.class(), Class::InvalidRequest, "{path:?}");
    assert!(path.is_fault(), "{path:?}");
    assert_eq!(path.retry_after(), None);

    let url = Predictions::builder()
        .base_url("not a url")
        .build()
        .unwrap_err();
    assert_eq!(url.class(), Class::InvalidRequest, "{url:?}");

    let ticker: InvalidTicker = Ticker::new("BTC/USD").unwrap_err();
    assert_eq!(ticker.class(), Class::InvalidRequest);
    assert!(ticker.is_fault());
    assert!(!ticker.is_retriable());
}
```

The inline tests of the client go after the constants of `mod.rs`: the default and demo hosts, the base URL's refusal, the default concurrency of 4, shared state, the provisional Basic bucket, and that the client and its futures are `Send`.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
/// In-flight requests the client allows by default.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;
```

Replace with:

```rust
/// In-flight requests the client allows by default.
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_client_targets_production_and_the_demo_host_is_a_builder_away() {
        let production = Predictions::new().unwrap();
        assert_eq!(
            production.inner.http.base_url.as_str(),
            "https://external-api.kalshi.com/"
        );
        let demo = Predictions::builder().base_url(DEMO).build().unwrap();
        assert_eq!(
            demo.inner.http.base_url.as_str(),
            "https://external-api.demo.kalshi.co/"
        );
    }

    #[test]
    fn a_base_url_is_a_scheme_and_host() {
        for url in [
            "http://127.0.0.1:8080",
            "http://127.0.0.1:8080/",
            "https://external-api.kalshi.com",
        ] {
            assert!(
                Predictions::builder().base_url(url).build().is_ok(),
                "{url}"
            );
        }
        for url in [
            "http://127.0.0.1:8080/proxy",
            "http://127.0.0.1:8080/proxy/",
            "https://external-api.kalshi.com/trade-api/v2",
        ] {
            let err = Predictions::builder().base_url(url).build().unwrap_err();
            let message = match err {
                PredictionsError::Api(ApiError::Validation(message)) => message,
                other => panic!("{url}: {other:?}"),
            };
            assert_eq!(
                message,
                format!("the base URL must be a scheme and host with no path: {url}")
            );
        }
    }

    #[test]
    fn a_base_url_that_is_not_a_url_is_a_url_error() {
        let err = Predictions::builder()
            .base_url("not a url")
            .build()
            .unwrap_err();
        assert!(matches!(err, PredictionsError::Api(ApiError::Url(_))));
    }

    #[tokio::test]
    async fn the_default_concurrency_limit_is_4() {
        let kalshi = Predictions::new().unwrap();
        let mut permits = Vec::new();
        for _ in 0..DEFAULT_MAX_CONCURRENT {
            permits.push(kalshi.inner.http.acquire_concurrency().await);
        }
        assert!(permits.iter().all(|permit| permit.is_some()));

        let fifth = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            kalshi.inner.http.acquire_concurrency(),
        )
        .await;
        assert!(
            fifth.is_err(),
            "a 5th permit should wait at the default of 4"
        );
    }

    #[test]
    fn a_clone_and_its_namespaces_share_one_state() {
        let kalshi = Predictions::new().unwrap();
        let clone = kalshi.clone();
        assert!(Arc::ptr_eq(&kalshi.inner, &clone.inner));
        assert!(Arc::ptr_eq(&kalshi.inner, &kalshi.health().inner));
        assert!(Arc::ptr_eq(&kalshi.inner, &kalshi.exchange().inner));
        let other = Predictions::new().unwrap();
        assert!(!Arc::ptr_eq(&kalshi.inner, &other.inner));
    }

    #[test]
    fn a_new_client_has_the_provisional_basic_read_bucket_and_no_key() {
        let kalshi = Predictions::new().unwrap();
        assert_eq!(
            kalshi.inner.throttle.bucket().capacity(),
            throttle::BASIC_READ_CAPACITY
        );
        assert_eq!(kalshi.inner.throttle.bucket().layer(), request::READ);
        assert!(kalshi.inner.auth.is_none());
        assert!(!kalshi.inner.confirmation.initialized());
        let text = format!("{kalshi:?}");
        assert!(text.contains("keyed: false"), "{text}");
        assert!(text.contains("provisional: true"), "{text}");
    }

    #[test]
    fn the_client_and_its_futures_are_send() {
        fn send<T: Send>(_: T) {}
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Predictions>();
        send_sync::<PredictionsBuilder>();
        let kalshi = Predictions::new().unwrap();
        send(kalshi.exchange().status().send());
        send(kalshi.health().ping());
    }
}
```

The inline tests of `Routed` go at the end of `request.rs`. They cover the read's cost, a query parameter set twice, the path a builder fills, the key and its redacted `Debug`, the query pairs on the wire, a send signed by an installed authenticator while a ping is not, and a read the confirmed bucket cannot hold being refused before anything is sent.

Current (`polyoxide-kalshi/src/predictions/request.rs`):

```rust
                    route.template,
                    other.template
                );
            }
        }
    }
}
```

Replace with:

```rust
                    route.template,
                    other.template
                );
            }
        }
    }
}

#[cfg(test)]
mod routed_tests {
    use mockito::{Matcher, Server};
    use polyoxide_core::reqwest::header::{HeaderName, HeaderValue};
    use polyoxide_core::{ApiError, Authenticator, DynAuthenticator, RequestParts};
    use polyoxide_test_support::query::{headers_sent, pairs_sent};
    use serde_json::Value;

    use super::*;
    use crate::predictions::Predictions;

    const STATUS: &str = "/trade-api/v2/exchange/status";

    /// Signs by adding a header, so a test can see which requests were signed.
    struct Marker;

    impl Authenticator for Marker {
        async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
            parts.headers.insert(
                HeaderName::from_static("x-marker"),
                HeaderValue::from_static("signed"),
            );
            Ok(())
        }
    }

    /// A client for `url` that signs with [`Marker`].
    fn keyed(url: String) -> Predictions {
        let mut builder = Predictions::builder().base_url(url);
        builder.auth = Some(DynAuthenticator::new_arc(Marker));
        builder.build().unwrap()
    }

    fn marked(headers: &[(String, String)]) -> bool {
        headers.contains(&("x-marker".to_owned(), "signed".to_owned()))
    }

    #[test]
    fn a_read_costs_ten_exact_tokens_on_the_read_layer_until_a_cost_is_set() {
        let kalshi = Predictions::new().unwrap();
        assert_eq!(
            kalshi.inner.read_cost(&routes::MARKETS),
            Cost {
                layer: READ,
                units: 10,
                exact: true
            }
        );

        kalshi.inner.costs.set(&routes::MARKETS, 30);
        assert_eq!(kalshi.inner.read_cost(&routes::MARKETS).units, 30);
        assert_eq!(kalshi.inner.read_cost(&routes::TRADES).units, 10);
    }

    #[test]
    fn a_query_parameter_set_twice_is_replaced_in_place() {
        let kalshi = Predictions::new().unwrap();
        let request = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .query("a", 1)
            .query("b", "x")
            .query("a", 2);
        assert_eq!(
            request.query,
            [("a", "2".to_owned()), ("b", "x".to_owned())]
        );
        assert_eq!(request.path, "/trade-api/v2/markets");
    }

    #[test]
    fn a_route_builder_fills_its_path_from_its_parameters() {
        let kalshi = Predictions::new().unwrap();
        let request = Routed::<Value>::new(&kalshi.inner, routes::ORDERBOOK, &["FED-23DEC-T3.00"]);
        assert_eq!(
            request.path,
            "/trade-api/v2/markets/FED-23DEC-T3.00/orderbook"
        );
    }

    #[test]
    fn a_client_has_a_key_only_when_an_authenticator_was_installed() {
        let plain = Predictions::new().unwrap();
        assert!(!Routed::<Value>::new(&plain.inner, routes::MARKETS, &[]).has_key());
        let signed = keyed("http://127.0.0.1:1".to_owned());
        assert!(Routed::<Value>::new(&signed.inner, routes::MARKETS, &[]).has_key());
        // Neither prints what signs, only whether something does.
        assert!(format!("{signed:?}").contains("keyed: true"));
        assert!(format!("{:?}", Predictions::builder()).contains("keyed: false"));
        let mut builder = Predictions::builder();
        builder.auth = Some(DynAuthenticator::new_arc(Marker));
        assert!(format!("{builder:?}").contains("keyed: true"));
    }

    #[tokio::test]
    async fn a_send_carries_the_query_pairs_in_order() {
        let pairs = pairs_sent(STATUS, |url| async move {
            let kalshi = Predictions::builder().base_url(url).build().unwrap();
            let _ = Routed::<Value>::new(&kalshi.inner, routes::EXCHANGE_STATUS, &[])
                .query("a", 1)
                .query("b", "x y")
                .query("a", 2)
                .send()
                .await;
        })
        .await;
        assert_eq!(
            pairs,
            [("a", "2"), ("b", "x y")].map(|(k, v)| (k.to_owned(), v.to_owned()))
        );
    }

    #[tokio::test]
    async fn a_send_is_signed_by_the_clients_authenticator_and_a_ping_is_not() {
        let sent = headers_sent(STATUS, |url| async move {
            let _ = keyed(url).exchange().status().send().await;
        })
        .await;
        assert!(marked(&sent), "{sent:?}");

        let pinged = headers_sent(STATUS, |url| async move {
            let _ = keyed(url).health().ping().await;
        })
        .await;
        assert!(!marked(&pinged), "{pinged:?}");
    }

    #[tokio::test]
    async fn a_read_the_confirmed_bucket_cannot_hold_is_refused_before_anything_is_sent() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", STATUS)
            .match_query(Matcher::Any)
            .expect(0)
            .create_async()
            .await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();
        kalshi.inner.throttle.bucket().resize(5, 1);

        let err = Routed::<Value>::new(&kalshi.inner, routes::EXCHANGE_STATUS, &[])
            .send()
            .await
            .unwrap_err();
        assert!(
            matches!(&err, PredictionsError::Api(ApiError::Refused(refused))
                if refused.layer == READ && refused.units == 10 && refused.capacity == 5),
            "{err:?}"
        );
        mock.assert_async().await;
    }
}
```

- [ ] **Step 2: Run them to see them fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --no-run`
Expected: compile errors, from the library's tests and from each integration suite: `error[E0599]: no function or associated item named `new` found for struct `Predictions``, `cannot find type `Predictions` in this scope`, `unresolved import `polyoxide_kalshi::predictions::Predictions``, `cannot find struct, variant or union type `Routed`` and `no method named `read_cost``.

- [ ] **Step 3: Implement.** Five parts: the module block and the client, `Routed`, the stub, and the API.

**3a. The module block and its imports.** This is the names contract's block for Story 1, and it drops Task 1.6's two allows.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
pub mod error;
pub mod key;
// Task 1.7 uses what these two modules define, and removes both allows.
#[allow(dead_code)]
pub mod request;
#[allow(dead_code)]
mod throttle;
pub mod types;

pub use error::{PredictionsError, VenueError};
pub use key::key;
```

Replace with:

```rust
pub mod api;
pub mod error;
pub mod key;
pub mod request;
mod throttle;
pub mod types;

pub use error::{PredictionsError, VenueError};
pub use key::key;

use std::fmt;
use std::sync::Arc;

use polyoxide_core::{ApiError, ClientConfig, DynAuthenticator, HttpClient};
use tokio::sync::OnceCell;

use throttle::{PredictionsThrottle, RouteCosts};
```

**3b. The client, its shared state and its builder.**

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
pub const DEFAULT_MAX_CONCURRENT: usize = 4;

/// Client for Kalshi's Predictions exchange.
///
/// A clone shares the client's connection pool, throttle and confirmation. A client built with no
/// credentials sends unsigned requests, which is all the public routes need.
///
/// ```no_run
/// use polyoxide_kalshi::predictions::Predictions;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let kalshi = Predictions::new()?;
/// let status = kalshi.exchange().status().send().await?;
/// println!("trading is active: {}", status.trading_active);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Predictions {
    pub(crate) inner: Arc<Inner>,
}

impl Predictions {
    /// A client for the production host, with default settings and no credentials.
    pub fn new() -> Result<Self, PredictionsError> {
        Self::builder().build()
    }

    /// Start configuring a client.
    pub fn builder() -> PredictionsBuilder {
        PredictionsBuilder::new()
    }

    polyoxide_core::namespaces! { inner;
        /// Liveness: a ping of the exchange-status route.
        health: api::Health,
        /// The exchange's status.
        exchange: api::ExchangeApi,
    }
}

/// Prints no key material.
impl fmt::Debug for Predictions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Predictions")
            .field("inner", &self.inner)
            .finish()
    }
}

/// What a client and every clone and namespace of it share.
pub(crate) struct Inner {
    pub(crate) http: HttpClient,
    pub(crate) throttle: PredictionsThrottle,
    pub(crate) costs: RouteCosts,
    pub(crate) confirmation: OnceCell<()>,
    pub(crate) auth: Option<Arc<DynAuthenticator<'static>>>,
}

/// Prints whether a key is set, never the key.
impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner")
            .field("http", &self.http)
            .field("read", self.throttle.bucket())
            .field("costs", &self.costs)
            .field("confirmed", &self.confirmation.initialized())
            .field("keyed", &self.auth.is_some())
            .finish()
    }
}

/// Builder for [`Predictions`].
///
/// It allows [`DEFAULT_MAX_CONCURRENT`] in-flight requests unless told otherwise.
#[derive(Clone)]
pub struct PredictionsBuilder {
    config: ClientConfig,
    pub(crate) auth: Option<Arc<DynAuthenticator<'static>>>,
}

impl Default for PredictionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Prints whether a key is set, never the key.
impl fmt::Debug for PredictionsBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PredictionsBuilder")
            .field("config", &self.config)
            .field("keyed", &self.auth.is_some())
            .finish()
    }
}

impl PredictionsBuilder {
    /// A builder for the production host, with no credentials.
    pub fn new() -> Self {
        Self {
            config: ClientConfig::new(PRODUCTION, DEFAULT_MAX_CONCURRENT),
            auth: None,
        }
    }

    polyoxide_core::client_config_setters!(config);

    /// Build the client.
    ///
    /// The base URL is a scheme and host: every route's path is written from the API root, and the
    /// signed path is the request's path as written, so a base URL with a path would be silently
    /// dropped and is refused instead.
    ///
    /// The client's throttle is its Read layer, and its retry policy is core's default, which
    /// retries a `429` and holds every request, and fails every other status.
    ///
    /// # Errors
    ///
    /// [`ApiError::Url`] when the base URL does not parse, and [`ApiError::Validation`] when it
    /// has a path other than `/`.
    pub fn build(self) -> Result<Predictions, PredictionsError> {
        let base = &self.config.base_url;
        let parsed = url::Url::parse(base).map_err(ApiError::from)?;
        if parsed.path() != "/" {
            return Err(PredictionsError::Api(ApiError::Validation(format!(
                "the base URL must be a scheme and host with no path: {base}"
            ))));
        }
        let throttle = PredictionsThrottle::new();
        let http = self
            .config
            .http_builder()
            .with_throttle(throttle.clone())
            .build()?;
        Ok(Predictions {
            inner: Arc::new(Inner {
                http,
                throttle,
                costs: RouteCosts::default(),
                confirmation: OnceCell::new(),
                auth: self.auth,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
```

**3c. `Routed` and the cost of a read.** The imports first.

Current (`polyoxide-kalshi/src/predictions/request.rs`):

```rust
use std::fmt::Write as _;

use polyoxide_core::LayerId;
```

Replace with:

```rust
use std::fmt::Write as _;
use std::marker::PhantomData;
use std::sync::Arc;

use polyoxide_core::{Cost, LayerId, QueryBuilder, Request};
use serde::de::DeserializeOwned;

use crate::predictions::{error::PredictionsError, Inner};
```

Current (`polyoxide-kalshi/src/predictions/request.rs`):

```rust
    ours.len() == listed.len()
}

#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
    ours.len() == listed.len()
}

/// A route and its query, sent as core's request with the route's cost on the Read layer.
///
/// The route builders wrap one. Setting a query parameter twice replaces it. Every route of this
/// module is a `GET`, which core's request is until told otherwise.
pub(crate) struct Routed<T> {
    inner: Arc<Inner>,
    route: Route,
    path: String,
    query: Vec<(&'static str, String)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Routed<T> {
    /// A request for `route`, its `:name` segments filled from `params` in order.
    pub(crate) fn new(inner: &Arc<Inner>, route: Route, params: &[&str]) -> Self {
        Self {
            inner: Arc::clone(inner),
            route,
            path: route.path(params),
            query: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Sets a query parameter, replacing an earlier value for the same key.
    #[allow(
        dead_code,
        reason = "Story 2's query setters write through it, and the tests below use it"
    )]
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Whether the client holds an authenticator, so a route that needs a key can refuse before it
    /// sends.
    #[allow(
        dead_code,
        reason = "Story 4's account().limits() refuses without a key through it"
    )]
    pub(crate) fn has_key(&self) -> bool {
        self.inner.auth.is_some()
    }
}

impl<T: DeserializeOwned> Routed<T> {
    /// Sends on the client's send loop, which waits for the Read bucket, and decodes the response.
    ///
    /// The steps, in order: wait for the client's `before_send`; build core's request with the
    /// route's cost, read now, after any confirmation has replaced it; add the client's
    /// authenticator when it has one, and the query pairs; send.
    pub(crate) async fn send(self) -> Result<T, PredictionsError> {
        self.inner.before_send().await;
        let cost = self.inner.read_cost(&self.route);
        let mut request =
            Request::<T, PredictionsError>::new(self.inner.http.clone(), self.path).with_cost(cost);
        if let Some(auth) = &self.inner.auth {
            request = request.authenticator(Arc::clone(auth));
        }
        for (key, value) in self.query {
            request = request.query(key, value);
        }
        request.send().await
    }
}

impl Inner {
    /// What `route` costs the Read layer now: the confirmed cost when the first-use confirmation
    /// has replaced it, else the compiled one. The cost is exact: Kalshi charges it.
    pub(crate) fn read_cost(&self, route: &Route) -> Cost {
        Cost {
            layer: READ,
            units: self.costs.cost(route),
            exact: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
```

**3d. The `before_send` stub and `RouteCosts::set`'s allow.** The stub is the names contract's text, and Story 4 replaces it as a whole.

Current (`polyoxide-kalshi/src/predictions/throttle.rs`):

```rust
use crate::predictions::request::{Route, DEFAULT_COST, READ};
```

Replace with:

```rust
use crate::predictions::request::{Route, DEFAULT_COST, READ};
use crate::predictions::Inner;
```

Current (`polyoxide-kalshi/src/predictions/throttle.rs`):

```rust
    /// Makes `route` cost `cost` from now on.
    pub(crate) fn set(&self, route: &Route, cost: u32) {
        self.overrides
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert((route.method, route.template), cost);
    }
}

#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
```

Replace with:

```rust
    /// Makes `route` cost `cost` from now on.
    #[allow(
        dead_code,
        reason = "Story 4's first-use confirmation replaces the compiled costs with it"
    )]
    pub(crate) fn set(&self, route: &Route, cost: u32) {
        self.overrides
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert((route.method, route.template), cost);
    }
}

impl Inner {
    /// Runs before every send.
    pub(crate) async fn before_send(&self) {}
}

#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
```

**3e. The API.** `Health::ping` and `ExchangeApi::status`, and `api/mod.rs` as the names contract gives it.

`polyoxide-kalshi/src/predictions/api/mod.rs`:

```rust
pub mod exchange;
pub mod health;

pub use exchange::{ExchangeApi, GetExchangeStatus};
pub use health::Health;
```

`polyoxide-kalshi/src/predictions/api/health.rs`:

```rust
//! Liveness: a ping of the exchange-status route.

use std::sync::Arc;

use polyoxide_core::Pong;

use crate::predictions::{error::PredictionsError, request::routes, Inner};

/// Health namespace.
#[derive(Debug, Clone)]
pub struct Health {
    pub(crate) inner: Arc<Inner>,
}

impl Health {
    /// Pings the host with `GET /trade-api/v2/exchange/status`, which is public: a ping is never
    /// signed, whether or not the client holds a key.
    ///
    /// The ping runs on the client's send loop, so it waits for the Read bucket and is retried and
    /// held as every request is, and its round trip is that of the attempt that answered. Its body
    /// is not read: the route answers whether the host does, and
    /// [`exchange().status()`](crate::predictions::Predictions::exchange) says whether the exchange
    /// is trading. A non-2xx answer is the error Kalshi's decode makes of it.
    pub async fn ping(&self) -> Result<Pong, PredictionsError> {
        self.inner.before_send().await;
        let route = routes::EXCHANGE_STATUS;
        let cost = self.inner.read_cost(&route);
        self.inner
            .http
            .health::<PredictionsError>(&route.path(&[]), &[cost])
            .await
    }
}
```

`polyoxide-kalshi/src/predictions/api/exchange.rs`:

```rust
//! Reference data: the exchange's status.

use std::sync::Arc;

use crate::predictions::{
    error::PredictionsError,
    request::{routes, Routed},
    types::ExchangeStatus,
    Inner,
};

/// Exchange namespace.
#[derive(Debug, Clone)]
pub struct ExchangeApi {
    pub(crate) inner: Arc<Inner>,
}

impl ExchangeApi {
    /// The exchange's status, via `GET /trade-api/v2/exchange/status`.
    pub fn status(&self) -> GetExchangeStatus {
        GetExchangeStatus {
            request: Routed::new(&self.inner, routes::EXCHANGE_STATUS, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/exchange/status`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetExchangeStatus {
    pub(crate) request: Routed<ExchangeStatus>,
}

impl GetExchangeStatus {
    /// Waits for the Read bucket, sends, and decodes the response.
    ///
    /// Kalshi sends this route's body with a 500, 503 or 504 too, which is an error here, as every
    /// non-2xx answer is.
    pub async fn send(self) -> Result<ExchangeStatus, PredictionsError> {
        self.request.send().await
    }
}
```

- [ ] **Step 4: Run them to see them pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: PASS: 55 library tests, the integration suites `mock_predictions` (8), `error_classes_predictions` (5) and `headers_predictions` (1), and 2 doctests (the key module's, and `Predictions`' `no_run` example, which is compiled).

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions`
Expected: the same, in the module's own build.

Run: `nix develop --command cargo fmt --all -- --check && nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: clean. A `dead_code` warning here names an item no one uses: Task 1.6's allows are gone, so it is a mistake in this task.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_module_readmes.py tests/test_dependency_fences.py -q'`
Expected: PASS.

- [ ] **Step 5: Say what the mock suites are in `CLAUDE.md`.**

Current (`CLAUDE.md`):

```markdown
and `polyoxide-binance`'s `tests/mock_usdm.rs`, which stands alone.
```

Replace with:

```markdown
`polyoxide-binance`'s `tests/mock_usdm.rs`, which stands alone, and `polyoxide-kalshi`'s `tests/mock_predictions.rs`, whose later mock suites are files of their own, named `mock_predictions_<what>`.
```

- [ ] **Step 6: Commit.**

```bash
git add -N polyoxide-kalshi/src/predictions/api polyoxide-kalshi/tests/headers_predictions.rs polyoxide-kalshi/tests/mock_predictions.rs polyoxide-kalshi/tests/error_classes_predictions.rs
git commit -m "feat(kalshi): the client, health().ping() and exchange().status() (Story 1)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/request.rs polyoxide-kalshi/src/predictions/throttle.rs polyoxide-kalshi/src/predictions/api polyoxide-kalshi/tests/headers_predictions.rs polyoxide-kalshi/tests/mock_predictions.rs polyoxide-kalshi/tests/error_classes_predictions.rs CLAUDE.md
```

### Task 1.8: The header pin in CI

Kalshi is an HTTP module on core's client, so CI's features job pins its header set in the module's minimal build and again beside the umbrella's `full` (spec §3.7, D20). The pin test derives the modules from `cargo metadata`, so it has been red since Task 1.2. The full-pin test also asserts that `full` turns on every module's feature, which a published umbrella cannot do for an unpublished crate it does not depend on, so that assertion is scoped to the venue crates the umbrella depends on, and a stand-in test pins the rule both ways. The command lists of both steps are still asserted in full, so Kalshi's full pin is required. `CLAUDE.md` says so in the commit that changes the rule.

**Files:**
- Modify: `.github/workflows/ci.yml:131`, `.github/workflows/ci.yml:135`, `.github/workflows/ci.yml:143`
- Modify: `.github/scripts/tests/test_ci_workflow.py:235`, `.github/scripts/tests/test_ci_workflow.py:261-285`, `.github/scripts/tests/test_ci_workflow.py:304`
- Modify: `CLAUDE.md:58`, `CLAUDE.md:552-554`
- Test: `.github/scripts/tests/test_ci_workflow.py`

- [ ] **Step 1: Write the failing test.** A stand-in test of the rule, both ways: a venue crate outside the umbrella needs no umbrella feature, a dev-dependency does not put one inside, and a crate the umbrella does depend on still needs its feature turned on by `full`. It calls `umbrella_missing`, which Step 3 writes.

Current (`.github/scripts/tests/test_ci_workflow.py`):

```python
    assert umbrella_enables(features, "full") == {
        "polyoxide-a/ws", "polyoxide-a/a", "polyoxide-a/keychain"}
    assert umbrella_enables(features, "b") == {"polyoxide-b/b"}
```

Replace with:

```python
    assert umbrella_enables(features, "full") == {
        "polyoxide-a/ws", "polyoxide-a/a", "polyoxide-a/keychain"}
    assert umbrella_enables(features, "b") == {"polyoxide-b/b"}


def test_a_venue_crate_outside_the_umbrella_needs_no_umbrella_feature() -> None:
    """The umbrella is published, so it cannot depend on an unpublished venue crate. Such a
    crate's pin still runs under `full`, but `full` has no feature of the crate's to turn on."""
    umbrella = {"dependencies": [
        {"name": "polyoxide-polymarket", "kind": None},
        # A dev-dependency is not built into `full`.
        {"name": "polyoxide-kalshi", "kind": "dev"}]}
    modules = [("polyoxide-kalshi", "predictions"), ("polyoxide-polymarket", "gamma")]
    # Outside the umbrella: nothing to turn on.
    assert umbrella_missing(umbrella, modules, {"polyoxide-polymarket/gamma"}) == []
    # Inside it, the module's feature is still required.
    assert umbrella_missing(umbrella, modules, set()) == ["polyoxide-polymarket/gamma"]
    # A crate the umbrella builds is checked whatever its name.
    built = {"dependencies": [{"name": "polyoxide-kalshi", "kind": None}]}
    assert umbrella_missing(built, modules, set()) == ["polyoxide-kalshi/predictions"]
```

- [ ] **Step 2: Run the CI-workflow tests to see them fail.**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py -q'`
Expected: FAIL: `test_a_venue_crate_outside_the_umbrella_needs_no_umbrella_feature` with `NameError: name 'umbrella_missing' is not defined`, and the two header-pin tests, `test_the_features_job_runs_each_header_pin_in_its_minimal_build` and `test_the_features_job_runs_each_header_pin_under_the_umbrellas_full`, on `assert sorted(commands) == sorted(…)`: the expected commands hold `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions` and `cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`, and the steps do not.

- [ ] **Step 3: Implement.** The helper, the full-pin test's assertion, the workflow's two lines, and `CLAUDE.md`'s sentences. The first edit to a file under `.github/workflows/` can be refused by a security hook: retry the same edit unchanged, then check the file with `grep`.

Current (`.github/scripts/tests/test_ci_workflow.py`):

```python
    return found


def test_the_features_job_runs_each_header_pin_in_its_minimal_build() -> None:
```

Replace with:

```python
    return found


def umbrella_missing(umbrella: dict, modules: list[tuple[str, str]],
                     enabled: set[str]) -> list[str]:
    """The `<crate>/<product>` of each module whose crate the umbrella depends on and whose
    feature its `full` (`enabled`) does not turn on. A venue crate the umbrella does not depend
    on is outside it: an unpublished crate, which a published umbrella cannot name, is pinned
    under `full` too, but has no umbrella feature to turn on."""
    built = {dep["name"] for dep in umbrella["dependencies"] if dep["kind"] is None}
    return [f"{crate}/{product}" for crate, product in modules
            if crate in built and f"{crate}/{product}" not in enabled]


def test_the_features_job_runs_each_header_pin_in_its_minimal_build() -> None:
```

Current (`.github/scripts/tests/test_ci_workflow.py`):

```python
    literal. The modules are the minimal step's, and `full` must turn each one on, or
    its pin would run without the module the build is meant to unify."""
```

Replace with:

```python
    literal. The modules are the minimal step's, and `full` must turn on each one of a crate
    the umbrella depends on, or its pin would run without the module the build is meant to
    unify. A venue crate outside the umbrella (`publish = false`) is pinned under `full` too."""
```

Current (`.github/scripts/tests/test_ci_workflow.py`):

```python
    enabled = umbrella_enables(packages[UMBRELLA]["features"], "full")
    for crate, product in modules:
        assert f"{crate}/{product}" in enabled, (
            f"the umbrella's full does not turn on {crate}/{product}")
```

Replace with:

```python
    enabled = umbrella_enables(packages[UMBRELLA]["features"], "full")
    missing = umbrella_missing(packages[UMBRELLA], modules, enabled)
    assert not missing, f"the umbrella's full does not turn on {missing}"
```

Current (`.github/workflows/ci.yml`):

```yaml
          cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm
```

Replace with:

```yaml
          cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm
          cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions
```

Current (`.github/workflows/ci.yml`):

```yaml
      # literal (AD-18's third build).
      - name: Header pins, umbrella full build
```

Replace with:

```yaml
      # literal (AD-18's third build). `polyoxide-kalshi` is `publish = false`, so
      # the umbrella does not include it: its pin runs beside `polyoxide/full` all
      # the same, with no umbrella feature of its own to turn on.
      - name: Header pins, umbrella full build
```

Current (`.github/workflows/ci.yml`):

```yaml
          cargo test -p polyoxide -p polyoxide-binance --features polyoxide/full --test headers_usdm
```

Replace with:

```yaml
          cargo test -p polyoxide -p polyoxide-binance --features polyoxide/full --test headers_usdm
          cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions
```

Current (`CLAUDE.md`):

```markdown
, and `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm` for Binance's `usdm`, one `-p` per command so their features never unify
```

Replace with:

```markdown
, `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm` for Binance's `usdm`, and `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions` for Kalshi's `predictions`, one `-p` per command so their features never unify
```

Current (`CLAUDE.md`):

```markdown
(the pin test derives that step too, and fails unless `full` turns each module on)
```

Replace with:

```markdown
(the pin test derives that step too, and fails unless `full` turns on each module of a crate the umbrella depends on; `polyoxide-kalshi` is `publish = false`, so the umbrella does not include it, and its pin runs under `full` with no umbrella feature of its own to turn on)
```

Current (`CLAUDE.md`):

```markdown
Each of those six HTTP modules has a
`tests/headers_<module>.rs` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay` and
`perps`, and `polyoxide-binance`'s `usdm`)
```

Replace with:

```markdown
Each of those seven HTTP modules has a
`tests/headers_<module>.rs` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay` and
`perps`, `polyoxide-binance`'s `usdm`, and `polyoxide-kalshi`'s `predictions`)
```

- [ ] **Step 4: Run the tests to see them pass.**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py -q'`
Expected: PASS, the new test and the two header-pin tests among them. The minimal-pin test finds `polyoxide-kalshi/tests/headers_predictions.rs`; the full-pin test requires Kalshi's full command and asks `full` for the features of the venue crates the umbrella depends on only.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`
Expected: PASS, one test. The set under `polyoxide/full` runs in Task 1.9.

- [ ] **Step 5: Commit.**

```bash
git commit -m "ci(kalshi): pin the header set of Kalshi's requests in the features job (Story 1)" -- .github/workflows/ci.yml .github/scripts/tests/test_ci_workflow.py CLAUDE.md
```

### Task 1.9: The story's gates

No code changes here. The profile's story gates run in order, then the story's own check, which ends with the touch-set gate. A gate that fails is fixed in the task that introduced the fault, not here; a formatting diff is the one exception.

**Files:**
- Test: the whole story

- [ ] **Step 1: Run the story gates, in order.**

Run: `nix develop --command cargo fmt --all -- --check`
Expected: no output.

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`
Expected: no warning. `polyoxide-kalshi` is the one package this story changes, and nothing depends on it.

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: PASS: 55 library tests, 14 integration tests (`mock_predictions` 8, `error_classes_predictions` 5, `headers_predictions` 1) and 2 doctests.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: no warning.

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: exit 0, and no generated region differs.

Run: `nix develop --command python3 scripts/venue_gate.py`
Expected: exit 0. The foundation crates name neither `kalshi` nor `predictions`, which are now a venue id and a product id the gate looks for.

Run: `nix develop --command python3 scripts/live_unwraps.py`
Expected: exit 0. The story has no live target.

Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`
Expected: exit 0. Every test of the story is new, in a crate that is not in the baseline.

- [ ] **Step 2: Run the story's own check.** It is the **Verify** line above, in the dev shell: Kalshi's tests in the module's own build, its header pin under the umbrella's `full`, the registry check and the venue gate, the CI-script tests the story touches, and the touch-set gate.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions`
Expected: PASS, as in Step 1.

Run: `nix develop --command cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`
Expected: PASS, one test: the header set is unchanged when the umbrella's `full` unifies Polymarket's and Binance's modules into the build. This builds the whole umbrella and takes a few minutes.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_ci_workflow.py tests/test_dep_allowlists.py tests/test_module_readmes.py tests/test_classify_coverage.py tests/test_gen_registry.py tests/test_venue_gate.py -q'`
Expected: PASS.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh; echo "exit $?"`
Expected: no path printed, then `exit 0`. A path printed is a change outside the allowlist: stop the story and ask the owner (spec D20).

- [ ] **Step 3: Commit anything the gates changed.** Only a formatting fix can be left: if `cargo fmt --all -- --check` printed a diff, run `nix develop --command cargo fmt -p polyoxide-kalshi` and commit the files it changed. Otherwise there is nothing to commit and the story is done.

```bash
git status --short
git commit -m "style(kalshi): rustfmt (Story 1)" -- polyoxide-kalshi
```

## Story 2: The public REST surface (Story 8.2)

- **Developer:** intermediate-dev
- **Files:** `Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs`, `polyoxide-kalshi/src/predictions/api/events.rs`, `polyoxide-kalshi/src/predictions/api/account.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/markets.rs`, `polyoxide-kalshi/src/predictions/types/events.rs`, `polyoxide-kalshi/src/predictions/types/account.rs`, `scripts/capture_kalshi_predictions.py`, `polyoxide-kalshi/tests/fixtures/predictions/exchange_status.json`, `polyoxide-kalshi/tests/fixtures/predictions/endpoint_costs.json`, `polyoxide-kalshi/tests/fixtures/predictions/market.json`, `polyoxide-kalshi/tests/fixtures/predictions/markets_page.json`, `polyoxide-kalshi/tests/fixtures/predictions/orderbook.json`, `polyoxide-kalshi/tests/fixtures/predictions/orderbook_depth1.json`, `polyoxide-kalshi/tests/fixtures/predictions/trades.json`, `polyoxide-kalshi/tests/fixtures/predictions/event.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_not_found.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_validation.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_unauthenticated.json`, `polyoxide-kalshi/tests/fixtures/predictions/error_rate_limited.json`, `polyoxide-kalshi/tests/fixtures/predictions/PROVENANCE.md`, `polyoxide-kalshi/tests/common/mod.rs`, `polyoxide-kalshi/tests/mock_predictions_routes.rs`, `polyoxide-kalshi/tests/query_setters_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions.rs`, `polyoxide-kalshi/tests/spec_agreement_predictions.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `docs/specs/kalshi/OBSERVED.md`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- **Interfaces:** exposes the namespaces, builders, setters, types, enums and fixtures under "Story 2 → Stories 3, 4, 6 and 7", and `tests/common/mod.rs`'s `two_sided_market`; uses Story 1's `Inner`, `Routed`, `routes`, `route_templates`, `template_matches`, `DEFAULT_COST`, `Ticker`, `PredictionsError` and the anchors
- **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
- **Review:** deep (the drift detectors and the types every later story reads)

Adds the markets, events and endpoint-costs namespaces and their builders over Story 1's route table, with every query setter written by `polyoxide_core::query_setters!`, and the types of every key the wire carries. A capture script reads production unsigned and writes the twelve fixtures and their provenance, and every later test reads them. The suites hold the story in place: `mock_predictions_routes` pins each route's path, exact query and decode, a 429 retried and then holding every request, the captured error bodies and the route table's accessors; `query_setters_predictions` pins every setter's exact pairs; `wire_agreement_predictions` compares each fixture with its type both ways; `spec_agreement_predictions` reads the YAML mirror through a dev-only parser and holds each type's field names and optionality, each enum and each builder's query keys to it; and `live_predictions` runs against production with no secrets, picks a two-sided market, looks for unmodelled keys, and checks the route table against `/account/endpoint_costs` (D5's drift test). Every route is sent at the compiled cost: the first-use confirmation is Story 4's. No landmine marker is in this story's code.

Two facts about the host that the spec states shape the code, and both are in its section of `docs/specs/kalshi/OBSERVED.md`, which a last task inserts above the file's anchor line. On 2026-10-18 the unfiltered `status=open` listing's first ten pages held 9,996 multivariate combinations of 10,000 rows and no quoted single market, while `mve_filter=exclude`'s first page held 579 quoted rows, so `ListMarkets` has an `mve_filter(MveFilter)` setter and both the capture and `two_sided_market` page `markets?status=open&mve_filter=exclude&limit=1000` (10 pages for the capture, 5 for the live helper) to a row with a YES bid above 0 and a YES ask below 1, then confirm both sides on that market's orderbook route. And Kalshi's `status` filter takes five values, `unopened`, `open`, `paused`, `closed` and `settled`, which `MarketStatusFilter` carries, the enum test checking all five against the mirror's parameter enum. A key the capture finds that the schema lacks is modelled as an optional field, excused in `OBSERVED_EXTRA` and recorded in that section, as perps' are.

### Task 2.1: The capture script and the fixtures

**Files:**
- Create: `scripts/capture_kalshi_predictions.py`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/exchange_status.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/endpoint_costs.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/market.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/markets_page.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/orderbook.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/orderbook_depth1.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/trades.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/event.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/error_not_found.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/error_validation.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/error_unauthenticated.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/error_rate_limited.json`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/PROVENANCE.md`

The fixtures are captured, never written by hand (spec D19). The script reads production with no credentials, at the anonymous default of 10 tokens a request, and writes nothing until every capture has succeeded.

- [ ] **Step 1: Write the script**

Create `scripts/capture_kalshi_predictions.py`:

```python
#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["certifi"]
# ///
"""Capture Kalshi's public REST responses as test fixtures for polyoxide-kalshi's
`predictions` module.

Usage: uv run scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions

Reads production (https://external-api.kalshi.com) with no credentials, at the anonymous
default of 10 tokens a request, about a hundred requests in all. It picks an active market with
a quote on both sides and a two-sided book by paging `markets?status=open&mve_filter=exclude`:
the filter is needed, since on 2026-10-18 the unfiltered open listing's first ten pages held
9,996 multivariate combinations of 10,000 rows and no quoted single market, while the filtered
listing's first page held 579 quoted rows. It then writes the fixtures and PROVENANCE.md
into OUT_DIR. Nothing is written until every capture has succeeded, so a failed run leaves the
fixtures as they were. Every key is kept; only list lengths are trimmed, so a wire-agreement test
sees each field the host sends. Review the diff before committing.
"""
import sys
from pathlib import Path

import capture_common

BASE = "https://external-api.kalshi.com/trade-api/v2"
PAUSE = 0.25
# Pages of the filtered listing to walk before giving up, and the rows of each.
PAGES = 10
PAGE_SIZE = 1000
# How many quoted candidates to confirm on the orderbook route before giving up.
CANDIDATES = 40
# Levels kept on each side of orderbook.json, the best ones.
BOOK_LEVELS = 5
# Markets kept in event.json's nested list, the picked one first.
EVENT_MARKETS = 3
UNKNOWN_TICKER = "NOSUCHMARKET-0"
NO_SIGNATURE_PATH = "/account/limits"
# Kalshi's rate-limit page gives a 429's body as this; provoking one would spend the quota
# the nightly runs share.
RATE_LIMITED_BODY = {"error": "too many requests"}


def identity(_method, _url):
    """Asks for an uncompressed body."""
    return {"Accept-Encoding": "identity"}


def fetch(path, **params):
    return capture_common.get(BASE + path, params=params, headers=identity, pause=PAUSE, timeout=60)


def fetch_ok(path, **params):
    return capture_common.require_ok(fetch(path, **params), f"GET {path}")


def refused(path, status, **params):
    """The JSON body of a reply the host refuses with `status`."""
    reply = fetch(path, **params)
    if reply.status != status or not reply.is_json:
        sys.exit(f"GET {path}: expected a JSON body with HTTP {status}, got HTTP {reply.status}: {reply.body!r}")
    return reply.body


def is_two_sided(market):
    """Active, with a YES bid above 0 and a YES ask below 1."""
    return (
        market["status"] == "active"
        and float(market["yes_bid_dollars"]) > 0
        and float(market["yes_ask_dollars"]) < 1
    )


def listing_rows():
    """The rows of `markets?status=open&mve_filter=exclude&limit=1000`, page by page, at most
    PAGES pages."""
    cursor = ""
    for _ in range(PAGES):
        params = {"status": "open", "mve_filter": "exclude", "limit": PAGE_SIZE}
        if cursor:
            params["cursor"] = cursor
        page = fetch_ok("/markets", **params)
        yield from page["markets"]
        cursor = page.get("cursor") or ""
        if not cursor:
            return


def pick_market():
    """The first listed market that quotes both sides and whose order book, read on the route the
    tests assert on, has levels on both and more than one on some side, so that `depth=1` differs
    from the full book. Returns the ticker, the market response and the book."""
    tried = 0
    for row in listing_rows():
        if not is_two_sided(row):
            continue
        tried += 1
        if tried > CANDIDATES:
            break
        ticker = row["ticker"]
        book = fetch_ok(f"/markets/{ticker}/orderbook")
        sides = book["orderbook_fp"]
        if sides["yes_dollars"] and sides["no_dollars"] and max(len(v) for v in sides.values()) >= 2:
            return ticker, fetch_ok(f"/markets/{ticker}"), book
    sys.exit(f"none of the first {CANDIDATES} quoted open markets has a two-sided book; run again")


def best_levels(book, count):
    """Keeps the `count` best levels of each side. The host lists a side worst first, so those
    are the last `count`."""
    return {"orderbook_fp": {side: levels[-count:] for side, levels in book["orderbook_fp"].items()}}


def provenance(ticker, event_ticker):
    today = capture_common.stamp("%Y-%m-%d")
    rows = [
        ("exchange_status.json", "GET /exchange/status", "as fetched"),
        ("endpoint_costs.json", "GET /account/endpoint_costs", "as fetched"),
        ("market.json", f"GET /markets/{ticker}", "as fetched"),
        ("markets_page.json", "GET /markets?status=open&mve_filter=exclude&limit=5", "as fetched"),
        (
            "orderbook.json",
            f"GET /markets/{ticker}/orderbook",
            f"the {BOOK_LEVELS} best levels of each side (the host lists a side worst first, so the last {BOOK_LEVELS})",
        ),
        ("orderbook_depth1.json", f"GET /markets/{ticker}/orderbook?depth=1", "as fetched"),
        ("trades.json", f"GET /markets/trades?ticker={ticker}&limit=100", "as fetched"),
        (
            "event.json",
            f"GET /events/{event_ticker}?with_nested_markets=true",
            f"`event.markets`: the market above first, then up to {EVENT_MARKETS - 1} more",
        ),
        ("error_not_found.json", f"GET /markets/{UNKNOWN_TICKER}", "the 404 body"),
        ("error_validation.json", "GET /markets/trades?limit=5000", "the 400 body"),
        ("error_unauthenticated.json", f"GET {NO_SIGNATURE_PATH}", "the 401 body, sent with no signature"),
        (
            "error_rate_limited.json",
            "none",
            "written from the 429 body on Kalshi's rate-limit page (https://docs.kalshi.com/getting_started/rate_limits), not captured",
        ),
    ]
    table = capture_common.provenance_table(
        ["File", "Request", "Trimmed to"],
        [(f"`{name}`", request if request == "none" else f"`{request}`", note) for name, request, note in rows],
    )
    return [
        "# Provenance",
        "",
        f"Captured {today} from `https://external-api.kalshi.com/trade-api/v2` by",
        "`scripts/capture_kalshi_predictions.py`. No credentials. Every key is kept; only list",
        "lengths are trimmed.",
        "",
        f"The market is `{ticker}`, picked from `markets?status=open&mve_filter=exclude` because it was",
        "active, quoted both sides and had an order book with levels on both. The filter is needed: the",
        "unfiltered open listing serves multivariate combinations first (see `docs/specs/kalshi/OBSERVED.md`).",
        "",
        *table,
    ]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    ticker, market, book = pick_market()
    event_ticker = market["market"]["event_ticker"]

    event = fetch_ok(f"/events/{event_ticker}", with_nested_markets="true")
    nested = sorted(event["event"].get("markets") or [], key=lambda m: m["ticker"] != ticker)
    if not nested or nested[0]["ticker"] != ticker:
        sys.exit(f"/events/{event_ticker}?with_nested_markets=true does not list {ticker}")
    event["event"]["markets"] = nested[:EVENT_MARKETS]

    trades = fetch_ok("/markets/trades", ticker=ticker, limit=100)
    if not trades["trades"]:
        sys.exit(f"no trades for {ticker}; run again")

    files = {
        "exchange_status.json": fetch_ok("/exchange/status"),
        "endpoint_costs.json": fetch_ok("/account/endpoint_costs"),
        "market.json": market,
        "markets_page.json": fetch_ok("/markets", status="open", mve_filter="exclude", limit=5),
        "orderbook.json": best_levels(book, BOOK_LEVELS),
        "orderbook_depth1.json": fetch_ok(f"/markets/{ticker}/orderbook", depth=1),
        "trades.json": trades,
        "event.json": event,
        "error_not_found.json": refused(f"/markets/{UNKNOWN_TICKER}", 404),
        "error_validation.json": refused("/markets/trades", 400, limit=5000),
        "error_unauthenticated.json": refused(NO_SIGNATURE_PATH, 401),
        "error_rate_limited.json": RATE_LIMITED_BODY,
    }
    for name, value in files.items():
        capture_common.write_json(out / name, value)
    capture_common.write_provenance(out, provenance(ticker, event_ticker))
    print(f"wrote {len(files)} fixtures and PROVENANCE.md to {out} (market {ticker})")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it with no argument to see it refuse**

```bash
nix develop --command uv run scripts/capture_kalshi_predictions.py
```

It writes nothing and exits 1, printing the script's docstring:

```text
Capture Kalshi's public REST responses as test fixtures for polyoxide-kalshi's
`predictions` module.

Usage: uv run scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions
...
```

- [ ] **Step 3: Capture**

```bash
nix develop --command uv run scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions
```

It prints `wrote 12 fixtures and PROVENANCE.md to polyoxide-kalshi/tests/fixtures/predictions (market <TICKER>)`. A run that finds no two-sided market among the first 40 quoted open markets of the filtered listing (at most 10 pages) exits with that message and writes nothing; run it again.

- [ ] **Step 4: Check what it wrote**

```bash
nix develop --command python3 -c "
import json, pathlib
d = pathlib.Path('polyoxide-kalshi/tests/fixtures/predictions')
names = ['exchange_status', 'endpoint_costs', 'market', 'markets_page', 'orderbook', 'orderbook_depth1', 'trades', 'event', 'error_not_found', 'error_validation', 'error_unauthenticated', 'error_rate_limited']
for n in names:
    json.loads((d / f'{n}.json').read_text())
m = json.loads((d / 'market.json').read_text())['market']
assert m['status'] == 'active' and float(m['yes_bid_dollars']) > 0 and float(m['yes_ask_dollars']) < 1
assert len(json.loads((d / 'markets_page.json').read_text())['markets']) == 5
assert (d / 'PROVENANCE.md').read_text().startswith('# Provenance')
print('ok')"
```

Expected output: `ok`.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions/exchange_status.json polyoxide-kalshi/tests/fixtures/predictions/endpoint_costs.json polyoxide-kalshi/tests/fixtures/predictions/market.json polyoxide-kalshi/tests/fixtures/predictions/markets_page.json polyoxide-kalshi/tests/fixtures/predictions/orderbook.json polyoxide-kalshi/tests/fixtures/predictions/orderbook_depth1.json polyoxide-kalshi/tests/fixtures/predictions/trades.json polyoxide-kalshi/tests/fixtures/predictions/event.json polyoxide-kalshi/tests/fixtures/predictions/error_not_found.json polyoxide-kalshi/tests/fixtures/predictions/error_validation.json polyoxide-kalshi/tests/fixtures/predictions/error_unauthenticated.json polyoxide-kalshi/tests/fixtures/predictions/error_rate_limited.json polyoxide-kalshi/tests/fixtures/predictions/PROVENANCE.md
git commit -m "feat(kalshi): capture the public REST fixtures (Story 8.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions
```

### Task 2.2: The types, held to the captured payloads

**Files:**
- Create: `polyoxide-kalshi/src/predictions/types/markets.rs`
- Create: `polyoxide-kalshi/src/predictions/types/events.rs`
- Create: `polyoxide-kalshi/src/predictions/types/account.rs`
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs`
- Modify: `polyoxide-kalshi/Cargo.toml`
- Test: `polyoxide-kalshi/tests/wire_agreement_predictions.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-kalshi/tests/wire_agreement_predictions.rs`:

```rust
//! Agreement between the types and payloads captured from production. Provenance is in
//! `tests/fixtures/predictions/PROVENANCE.md`.
//!
//! Each fixture is decoded into its type and encoded back, and:
//!
//! 1. **Nothing unmodelled.** Every key path the host sent is emitted by the type.
//! 2. **Nothing invented.** Every key path the type emits was sent by the host, but for the
//!    paths of `SOMETIMES_ABSENT`.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a price keeps every
//!    digit it was sent with.

use polyoxide_kalshi::predictions::{
    request::DEFAULT_COST,
    types::{
        BookSide, EndpointCosts, EventResponse, ExchangeStatus, MarketResponse, MarketStatus,
        MarketsPage, OrderbookResponse, OutcomeSide, TradesPage,
    },
};
use polyoxide_test_support::{agreement, fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};

/// `(fixture, path)` pairs the host sends in some responses and not others, which a type that
/// models them may emit as `null` where a capture lacks them.
const SOMETIMES_ABSENT: &[(&str, &str)] = &[(
    "exchange_status",
    "/exchange_estimated_resume_time",
)];

fn read<T: DeserializeOwned>(fixture: &str) -> T {
    serde_json::from_str(&fixtures!("predictions").text(fixture)).unwrap()
}

fn hold_to_fixture<T: DeserializeOwned + Serialize>(fixture: &str) {
    let text = fixtures!("predictions").text(fixture);
    let diff = agreement::compare::<T>(fixture, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{fixture}: the host sent {:?}, which the type does not model",
        diff.unmodelled
    );
    let invented: Vec<&String> = diff
        .invented
        .iter()
        .filter(|path| !SOMETIMES_ABSENT.contains(&(fixture, path.as_str())))
        .collect();
    assert!(
        invented.is_empty(),
        "{fixture}: the type emits {invented:?}, which the host did not send"
    );
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    hold_to_fixture::<ExchangeStatus>("exchange_status");
    hold_to_fixture::<EndpointCosts>("endpoint_costs");
    hold_to_fixture::<MarketResponse>("market");
    hold_to_fixture::<MarketsPage>("markets_page");
    hold_to_fixture::<OrderbookResponse>("orderbook");
    hold_to_fixture::<OrderbookResponse>("orderbook_depth1");
    hold_to_fixture::<TradesPage>("trades");
    hold_to_fixture::<EventResponse>("event");
}

#[test]
fn the_fixtures_cover_the_cases_the_types_exist_for() {
    let market = read::<MarketResponse>("market").market;
    assert!(
        market.yes_bid_dollars > Decimal::ZERO && market.yes_ask_dollars < Decimal::ONE,
        "market.json is a market with a quote on both sides"
    );
    assert!(!market.price_ranges.is_empty());

    let page: MarketsPage = read("markets_page");
    assert_eq!(page.markets.len(), 5);
    assert!(!page.cursor.is_empty(), "a page with more behind it");

    let book = read::<OrderbookResponse>("orderbook").orderbook_fp;
    assert!(!book.yes_dollars.is_empty() && !book.no_dollars.is_empty());
    let shallow = read::<OrderbookResponse>("orderbook_depth1").orderbook_fp;
    assert!(shallow.yes_dollars.len() <= 1 && shallow.no_dollars.len() <= 1);

    let trades: TradesPage = read("trades");
    assert!(!trades.trades.is_empty());
    assert!(
        trades.trades.iter().all(|t| t.ticker == market.ticker),
        "trades.json is the market's"
    );

    let event: EventResponse = read("event");
    assert!(
        event.event.markets.iter().any(|m| m.ticker == market.ticker),
        "event.json nests the market"
    );

    // An unknown value would land in `Other` and decode fine; failing here is how a value
    // Kalshi adds gets modelled instead of passing silently.
    let unnamed: Vec<&MarketStatus> = page
        .markets
        .iter()
        .chain(&event.event.markets)
        .chain(std::iter::once(&market))
        .map(|m| &m.status)
        .filter(|s| matches!(s, MarketStatus::Other(_)))
        .collect();
    assert!(unnamed.is_empty(), "market states the enum does not name: {unnamed:?}");
    let unnamed: Vec<_> = trades
        .trades
        .iter()
        .filter(|t| {
            matches!(t.taker_outcome_side, OutcomeSide::Other(_))
                || matches!(t.taker_book_side, BookSide::Other(_))
                || matches!(t.taker_side, Some(OutcomeSide::Other(_)))
        })
        .map(|t| (&t.taker_outcome_side, &t.taker_book_side))
        .collect();
    assert!(unnamed.is_empty(), "trade sides the enums do not name: {unnamed:?}");
}

#[test]
fn the_endpoint_costs_default_is_the_compiled_default() {
    let costs: EndpointCosts = read("endpoint_costs");
    assert_eq!(costs.default_cost, DEFAULT_COST);
    assert!(!costs.endpoint_costs.is_empty());
}
```

Add its `[[test]]` stanza to `polyoxide-kalshi/Cargo.toml`. Current (the one line above `[package.metadata.polyoxide]`):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "wire_agreement_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test wire_agreement_predictions
```

It does not compile: `error[E0432]: unresolved imports` for `polyoxide_kalshi::predictions::types::{BookSide, EndpointCosts, EventResponse, MarketResponse, MarketStatus, MarketsPage, OrderbookResponse, OutcomeSide, TradesPage}`, none of which exists yet.

- [ ] **Step 3: Implement**

Create `polyoxide-kalshi/src/predictions/types/markets.rs`:

```rust
//! Markets, their order books and their public trades.
//!
//! Money (`*_dollars`) and counts (`*_fp`) are [`Decimal`], decoded from the fixed-point
//! strings Kalshi sends, so no value passes through an `f64` and every digit survives a round
//! trip. Times are the RFC 3339 strings as sent. A field Kalshi leaves out of a response is an
//! `Option`, and an absent one is left out again when the type is written back. Free-form
//! parts (`custom_strike`, a combination's legs) are kept as raw JSON.

use polyoxide_venue::positional::DecimalStr;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Number, Value};

polyoxide_venue::open_enum! {
    /// A market's place in its lifecycle. Kalshi extends the set, so a value this version does
    /// not know is kept as `Other`.
    pub enum MarketStatus {
        /// Wire value `initialized`.
        Initialized => "initialized",
        /// Wire value `inactive`.
        Inactive => "inactive",
        /// Wire value `active`: the market trades.
        Active => "active",
        /// Wire value `closed`.
        Closed => "closed",
        /// Wire value `determined`.
        Determined => "determined",
        /// Wire value `disputed`.
        Disputed => "disputed",
        /// Wire value `amended`.
        Amended => "amended",
        /// Wire value `finalized`.
        Finalized => "finalized",
    }
}

polyoxide_venue::open_enum! {
    /// An outcome: the side of a binary market a trade took or a level rests on.
    pub enum OutcomeSide {
        /// Wire value `yes`.
        Yes => "yes",
        /// Wire value `no`.
        No => "no",
    }
}

polyoxide_venue::open_enum! {
    /// A side of the book, in the YES leg's vocabulary: `bid` buys YES and `ask` sells it.
    pub enum BookSide {
        /// Wire value `bid`.
        Bid => "bid",
        /// Wire value `ask`.
        Ask => "ask",
    }
}

polyoxide_venue::wire_enum! {
    /// The lifecycle states `GET /markets` filters by. The OpenAPI's parameter enum lists all
    /// five; its prose leaves out `paused`.
    #[non_exhaustive]
    pub enum MarketStatusFilter {
        /// Not yet open.
        Unopened => "unopened",
        /// Open for trading.
        Open => "open",
        /// Paused.
        Paused => "paused",
        /// Closed.
        Closed => "closed",
        /// Settled.
        Settled => "settled",
    }
}

polyoxide_venue::wire_enum! {
    /// The `mve_filter` of `GET /markets`: whether the listing holds multivariate combinations.
    /// The unfiltered open listing serves them first, 9,996 of its first 10,000 rows on
    /// 2026-10-18, so a walk for a single quoted market asks to exclude them.
    #[non_exhaustive]
    pub enum MveFilter {
        /// Only the multivariate combinations.
        Only => "only",
        /// Everything but the multivariate combinations.
        Exclude => "exclude",
    }
}

/// Reads `null` as the type's default, for a list the host may send as `null` or leave out.
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// One range of valid order prices on a market, and the step within it, in dollars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceRange {
    /// Where the range starts.
    #[serde(with = "rust_decimal::serde::str")]
    pub start: Decimal,
    /// Where the range ends.
    #[serde(with = "rust_decimal::serde::str")]
    pub end: Decimal,
    /// The tick size within the range.
    #[serde(with = "rust_decimal::serde::str")]
    pub step: Decimal,
}

/// A market: one binary outcome of an event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Market {
    /// The market's ticker.
    pub ticker: String,
    /// The event it belongs to.
    pub event_ticker: String,
    /// `binary` or `scalar`.
    pub market_type: String,
    /// Deprecated: the market's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Deprecated: the market's subtitle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// A short title for the YES side.
    pub yes_sub_title: String,
    /// A short title for the NO side.
    pub no_sub_title: String,
    /// When the market was created.
    pub created_time: String,
    /// When its non-trading metadata last changed.
    pub updated_time: String,
    /// When it opens.
    pub open_time: String,
    /// When it closes.
    pub close_time: String,
    /// When it is expected to expire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_expiration_time: Option<String>,
    /// Deprecated: when it expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiration_time: Option<String>,
    /// The latest it can expire.
    pub latest_expiration_time: String,
    /// Seconds between determination and settlement.
    pub settlement_timer_seconds: u32,
    /// The market's lifecycle state.
    pub status: MarketStatus,
    /// The best YES bid, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_bid_dollars: Decimal,
    /// Contracts resting at the best YES bid.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_bid_size_fp: Decimal,
    /// The best YES ask, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_ask_dollars: Decimal,
    /// Contracts resting at the best YES ask.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_ask_size_fp: Decimal,
    /// The best NO bid, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub no_bid_dollars: Decimal,
    /// The best NO ask, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub no_ask_dollars: Decimal,
    /// The last YES trade's price, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub last_price_dollars: Decimal,
    /// Contracts traded.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_fp: Decimal,
    /// Contracts traded in the last 24 hours.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume_24h_fp: Decimal,
    /// `yes`, `no`, `scalar`, or empty before a result.
    pub result: String,
    /// Whether the market can close early.
    pub can_close_early: bool,
    /// Contracts bought, without netting.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest_fp: Decimal,
    /// What one contract pays at settlement, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub notional_value_dollars: Decimal,
    /// The best YES bid a day ago, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_yes_bid_dollars: Decimal,
    /// The best YES ask a day ago, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_yes_ask_dollars: Decimal,
    /// The last YES trade's price a day ago, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub previous_price_dollars: Decimal,
    /// What the YES side settled at, in dollars; set after determination.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub settlement_value_dollars: Option<Decimal>,
    /// When the market settled; set for a settled market.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_ts: Option<String>,
    /// Which settlement bounds apply: `default` (none) or `floor`.
    pub settlement_bounds_type: String,
    /// The lowest the YES side can settle at, in dollars; set with a `floor` bound.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub settlement_floor_dollars: Option<Decimal>,
    /// The value that was considered for the settlement.
    pub expiration_value: String,
    /// When the underlying event occurred, if recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_datetime: Option<String>,
    /// When the market's fee waiver expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_waiver_expiration_time: Option<String>,
    /// The condition under which the market can close early.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_close_condition: Option<String>,
    /// How the strike is defined and evaluated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike_type: Option<String>,
    /// The lowest expiration value that settles YES.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_strike: Option<Number>,
    /// The highest expiration value that settles YES.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_strike: Option<Number>,
    /// A mapping from expiration values to settlement values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub functional_strike: Option<String>,
    /// The expiration value of each target that settles YES, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_strike: Option<Value>,
    /// A plain-language statement of the main terms.
    pub rules_primary: String,
    /// A plain-language statement of the secondary terms.
    pub rules_secondary: String,
    /// A combination's collection ticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mve_collection_ticker: Option<String>,
    /// A combination's selected legs, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mve_selected_legs: Option<Vec<Value>>,
    /// The market's primary participant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_participant_key: Option<String>,
    /// The market's price level structure, which defines `price_ranges`.
    pub price_level_structure: String,
    /// The valid price ranges for orders.
    pub price_ranges: Vec<PriceRange>,
    /// Whether the market may be removed after determination if nothing traded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_provisional: Option<bool>,
    /// The exchange shard the market trades on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<u32>,
}

/// `GET /markets/{ticker}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketResponse {
    /// The market.
    pub market: Market,
}

/// `GET /markets`: one page. An empty `cursor` ends a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketsPage {
    /// The page's markets.
    pub markets: Vec<Market>,
    /// The cursor of the next page, empty on the last.
    pub cursor: String,
}

/// One price level of an order book side, `[price, count]` on the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceLevel {
    /// The price, in dollars.
    pub price: Decimal,
    /// Contracts resting at the price.
    pub count: Decimal,
}

/// A level's wire form: a tuple of exactly two, so a longer or shorter array is refused.
#[derive(Serialize, Deserialize)]
struct PriceLevelWire(DecimalStr, DecimalStr);

impl<'de> Deserialize<'de> for PriceLevel {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let PriceLevelWire(price, count) = PriceLevelWire::deserialize(deserializer)?;
        Ok(Self {
            price: price.0,
            count: count.0,
        })
    }
}

impl Serialize for PriceLevel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PriceLevelWire(DecimalStr(self.price), DecimalStr(self.count)).serialize(serializer)
    }
}

/// The two sides of an order book. Each lists its bids worst (lowest) first, and a NO bid at
/// `q` is a YES ask at `1 - q`. A side with no levels is an empty list, whether the host sends
/// `[]`, `null` or nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbookFp {
    /// YES bids.
    #[serde(default, deserialize_with = "null_as_default")]
    pub yes_dollars: Vec<PriceLevel>,
    /// NO bids.
    #[serde(default, deserialize_with = "null_as_default")]
    pub no_dollars: Vec<PriceLevel>,
}

/// `GET /markets/{ticker}/orderbook`. An unlisted ticker answers `200` with both sides empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbookResponse {
    /// The book.
    pub orderbook_fp: OrderbookFp,
}

/// One public trade.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    /// The trade's id.
    pub trade_id: String,
    /// The market's ticker.
    pub ticker: String,
    /// Contracts traded; a trade can be fractional.
    #[serde(with = "rust_decimal::serde::str")]
    pub count_fp: Decimal,
    /// The YES price, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub yes_price_dollars: Decimal,
    /// The NO price, in dollars.
    #[serde(with = "rust_decimal::serde::str")]
    pub no_price_dollars: Decimal,
    /// Deprecated: the taker's side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taker_side: Option<OutcomeSide>,
    /// The outcome the taker is positioned for.
    pub taker_outcome_side: OutcomeSide,
    /// The same direction in book vocabulary.
    pub taker_book_side: BookSide,
    /// When the trade executed.
    pub created_time: String,
    /// Whether it matched off the book as a block trade.
    pub is_block_trade: bool,
}

/// `GET /markets/trades`: one page, newest first. An empty `cursor` ends a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradesPage {
    /// The page's trades.
    pub trades: Vec<Trade>,
    /// The cursor of the next page, empty on the last.
    pub cursor: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_a_positional_pair_of_strings_that_keeps_every_digit() {
        let level: PriceLevel = serde_json::from_str(r#"["0.0100","402.00"]"#).unwrap();
        assert_eq!(level.price.to_string(), "0.0100");
        assert_eq!(level.count.to_string(), "402.00");
        assert_eq!(
            serde_json::to_string(&level).unwrap(),
            r#"["0.0100","402.00"]"#
        );
    }

    #[test]
    fn a_level_of_another_arity_or_with_a_number_is_refused() {
        assert!(serde_json::from_str::<PriceLevel>(r#"["0.01"]"#).is_err());
        assert!(serde_json::from_str::<PriceLevel>(r#"["0.01","1.00","x"]"#).is_err());
        assert!(serde_json::from_str::<PriceLevel>(r#"["0.01",1]"#).is_err());
    }

    #[test]
    fn a_side_that_is_null_or_missing_is_empty() {
        for body in [
            r#"{"yes_dollars":null,"no_dollars":[]}"#,
            r#"{"no_dollars":[]}"#,
            r#"{}"#,
        ] {
            let book: OrderbookFp = serde_json::from_str(body).unwrap();
            assert!(book.yes_dollars.is_empty() && book.no_dollars.is_empty(), "{body}");
        }
    }

    #[test]
    fn a_state_the_enum_does_not_name_is_kept_verbatim() {
        let status: MarketStatus = serde_json::from_str(r#""suspended""#).unwrap();
        assert_eq!(status, MarketStatus::Other("suspended".into()));
        assert_eq!(serde_json::to_string(&status).unwrap(), r#""suspended""#);
        assert_eq!(MarketStatus::from_wire("active"), MarketStatus::Active);
    }
}
```

Create `polyoxide-kalshi/src/predictions/types/events.rs`:

```rust
//! Events: a set of related markets.

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

use super::markets::{null_as_default, Market};

/// An event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The event's ticker.
    pub event_ticker: String,
    /// The series it belongs to.
    pub series_ticker: String,
    /// A short descriptive title.
    pub sub_title: String,
    /// The full title.
    pub title: String,
    /// The collateral-return netting type: `MECNET`, `DIRECNET`, or empty for none.
    pub collateral_return_type: String,
    /// Whether at most one of its markets can resolve YES (`MECNET`).
    pub mutually_exclusive: bool,
    /// Deprecated: the event's category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// The date the event is based on, when it has a date strike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike_date: Option<String>,
    /// The period the event covers, when it has a period strike.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike_period: Option<String>,
    /// Its markets; filled only when the request asked for them nested, and empty otherwise.
    #[serde(
        default,
        deserialize_with = "null_as_default",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub markets: Vec<Market>,
    /// Additional metadata, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_metadata: Option<Value>,
    /// The sources that determine its markets, as sent.
    #[serde(default)]
    pub settlement_sources: Option<Vec<Value>>,
    /// When its metadata last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_ts: Option<String>,
    /// A fee type that overrides its series'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_type_override: Option<String>,
    /// A fee multiplier that overrides its series'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fee_multiplier_override: Option<Number>,
    /// The exchange shard the event trades on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<u32>,
}

/// `GET /events/{event_ticker}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventResponse {
    /// The event.
    pub event: Event,
    /// Deprecated: the event's markets, which the host leaves empty when they are nested.
    #[serde(default, deserialize_with = "null_as_default")]
    pub markets: Vec<Market>,
}
```

Create `polyoxide-kalshi/src/predictions/types/account.rs`:

```rust
//! The account routes that need no key.

use serde::{Deserialize, Serialize};

/// One route whose token cost differs from the default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointCost {
    /// The HTTP method.
    pub method: String,
    /// The route's full template, such as `/trade-api/v2/markets/:ticker`.
    pub path: String,
    /// Its cost in tokens.
    pub cost: u32,
}

/// `GET /account/endpoint_costs`, which answers without a key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointCosts {
    /// What a route not listed costs, in tokens.
    pub default_cost: u32,
    /// The routes whose cost differs from the default.
    pub endpoint_costs: Vec<EndpointCost>,
}
```

Replace Story 1's `types/mod.rs` block in `polyoxide-kalshi/src/predictions/types/mod.rs`. Current:

```rust
mod exchange;

pub use exchange::{ExchangeIndexStatus, ExchangeStatus};
```

Replace with:

```rust
mod account;
mod events;
mod exchange;
mod markets;

pub use account::{EndpointCost, EndpointCosts};
pub use events::{Event, EventResponse};
pub use exchange::{ExchangeIndexStatus, ExchangeStatus};
pub use markets::{
    BookSide, Market, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, MveFilter,
    OrderbookFp, OrderbookResponse, OutcomeSide, PriceLevel, PriceRange, Trade, TradesPage,
};
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test wire_agreement_predictions
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib predictions::types
```

The first prints `test result: ok. 3 passed; 0 failed`, the second `test result: ok. 4 passed; 0 failed`. A failure of `every_rest_fixture_agrees_with_its_type` names the fixture and the key path: a key the host sent that no type emits is a field to add (as an `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]`), and one the type emits that the host did not send is a field to make optional. A key the schema in `docs/specs/kalshi/openapi.yaml` lacks is still modelled, as an optional field of its type: excuse it in `OBSERVED_EXTRA` in `spec_agreement_predictions.rs` (Task 2.4) and record it in Task 2.6's `OBSERVED.md` section.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git add -N polyoxide-kalshi/src/predictions/types/markets.rs polyoxide-kalshi/src/predictions/types/events.rs polyoxide-kalshi/src/predictions/types/account.rs polyoxide-kalshi/tests/wire_agreement_predictions.rs
git commit -m "feat(kalshi): the market, event and endpoint-cost types (Story 8.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-kalshi/src/predictions/types polyoxide-kalshi/tests/wire_agreement_predictions.rs polyoxide-kalshi/Cargo.toml Cargo.lock
```

### Task 2.3: The namespaces and builders, with their mock and setter suites

**Files:**
- Create: `polyoxide-kalshi/src/predictions/api/markets.rs`
- Create: `polyoxide-kalshi/src/predictions/api/events.rs`
- Create: `polyoxide-kalshi/src/predictions/api/account.rs`
- Modify: `polyoxide-kalshi/src/predictions/api/mod.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs`
- Modify: `polyoxide-kalshi/Cargo.toml`
- Test: `polyoxide-kalshi/tests/mock_predictions_routes.rs`
- Test: `polyoxide-kalshi/tests/query_setters_predictions.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-kalshi/tests/mock_predictions_routes.rs`:

```rust
//! Mock-server tests for the public routes: each route's path and exact query, the decoding of
//! its captured body, the captured error bodies, a 429 retried and then holding every request,
//! and the route table's accessors.

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    request::{route_templates, template_matches},
    types::{MarketStatus, MarketStatusFilter, MveFilter, OutcomeSide},
    Predictions, PredictionsError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

/// A schedule that retries nothing and holds the client for about a millisecond after a 429.
const NO_RETRIES: RetryConfig = RetryConfig {
    max_retries: 0,
    initial_backoff_ms: 1,
    max_backoff_ms: 10,
};

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    client_with_retries(server, RetryConfig::default())
}

fn client_with_retries(server: &ServerGuard, config: RetryConfig) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(config)
        .build()
        .unwrap()
}

fn ticker(text: &str) -> Ticker {
    Ticker::new(text).unwrap()
}

/// A route answering `body`, matched on its whole query string so a missing or extra parameter
/// fails the request.
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
        .with_body(body)
        .create_async()
        .await
}

// ── markets ─────────────────────────────────────────────────────

#[tokio::test]
async fn the_market_listing_sends_its_filters_and_decodes_the_captured_page() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets",
        "status=open&event_ticker=EV-1&series_ticker=SER-1&mve_filter=exclude&limit=5&cursor=c1",
        &fixture("markets_page"),
    )
    .await;
    let page = client(&server)
        .markets()
        .list()
        .status(MarketStatusFilter::Open)
        .event_ticker("EV-1")
        .series_ticker("SER-1")
        .mve_filter(MveFilter::Exclude)
        .limit(5)
        .cursor("c1")
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire("markets_page");
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    let rows = sent["markets"].as_array().unwrap();
    assert_eq!(page.markets.len(), rows.len());
    for (market, row) in page.markets.iter().zip(rows) {
        assert_eq!(market.ticker, row["ticker"].as_str().unwrap());
        assert_eq!(
            market.status,
            MarketStatus::from_wire(row["status"].as_str().unwrap())
        );
        assert_eq!(
            market.yes_bid_dollars.to_string(),
            row["yes_bid_dollars"].as_str().unwrap()
        );
        assert_eq!(
            market.price_ranges.len(),
            row["price_ranges"].as_array().unwrap().len()
        );
    }
}

#[tokio::test]
async fn a_market_is_read_by_ticker_and_decodes_the_captured_body() {
    let sent = wire("market");
    let text = sent["market"]["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/markets/{text}"),
        "",
        &fixture("market"),
    )
    .await;
    let read = client(&server)
        .markets()
        .get(&ticker(text))
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.market.ticker, text);
    assert_eq!(
        read.market.notional_value_dollars.to_string(),
        sent["market"]["notional_value_dollars"].as_str().unwrap()
    );
    assert_eq!(
        read.market.yes_ask_dollars.to_string(),
        sent["market"]["yes_ask_dollars"].as_str().unwrap()
    );
}

#[tokio::test]
async fn a_dotted_ticker_is_sent_as_written_and_in_capitals() {
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/KXBTCD-26OCT1806-T82099.99",
        "",
        &fixture("market"),
    )
    .await;
    client(&server)
        .markets()
        .get(&ticker("kxbtcd-26oct1806-t82099.99"))
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn an_order_book_sends_its_depth_and_keeps_every_digit_of_each_level() {
    let sent = wire("orderbook");
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/KXTEST-1/orderbook",
        "",
        &fixture("orderbook"),
    )
    .await;
    let book = client(&server)
        .markets()
        .orderbook(&ticker("KXTEST-1"))
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    mock.assert_async().await;
    for (side, levels) in [("yes_dollars", &book.yes_dollars), ("no_dollars", &book.no_dollars)] {
        let rows = sent["orderbook_fp"][side].as_array().unwrap();
        assert_eq!(levels.len(), rows.len(), "{side}");
        for (level, row) in levels.iter().zip(rows) {
            assert_eq!(level.price.to_string(), row[0].as_str().unwrap(), "{side}");
            assert_eq!(level.count.to_string(), row[1].as_str().unwrap(), "{side}");
        }
    }

    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/KXTEST-1/orderbook",
        "depth=1",
        &fixture("orderbook_depth1"),
    )
    .await;
    let shallow = client(&server)
        .markets()
        .orderbook(&ticker("KXTEST-1"))
        .depth(1)
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    mock.assert_async().await;
    assert!(shallow.yes_dollars.len() <= 1 && shallow.no_dollars.len() <= 1);
}

#[tokio::test]
async fn an_unlisted_ticker_has_an_empty_book_whatever_form_the_empty_sides_take() {
    for body in [
        r#"{"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}"#,
        r#"{"orderbook_fp":{"no_dollars":null,"yes_dollars":[]}}"#,
        r#"{"orderbook_fp":{}}"#,
    ] {
        let mut server = Server::new_async().await;
        let mock = route(
            &mut server,
            "/trade-api/v2/markets/NOSUCHMARKET-0/orderbook",
            "",
            body,
        )
        .await;
        let book = client(&server)
            .markets()
            .orderbook(&ticker("NOSUCHMARKET-0"))
            .send()
            .await
            .unwrap()
            .orderbook_fp;
        mock.assert_async().await;
        assert!(book.yes_dollars.is_empty() && book.no_dollars.is_empty(), "{body}");
    }
}

#[tokio::test]
async fn trades_send_their_query_and_decode_the_captured_page() {
    let sent = wire("trades");
    let text = sent["trades"][0]["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/trades",
        &format!("ticker={text}&limit=3&cursor=c1&min_ts=6&max_ts=7"),
        &fixture("trades"),
    )
    .await;
    let page = client(&server)
        .markets()
        .trades()
        .ticker(&ticker(text))
        .limit(3)
        .cursor("c1")
        .min_ts(6)
        .max_ts(7)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let rows = sent["trades"].as_array().unwrap();
    assert_eq!(page.trades.len(), rows.len());
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    for (trade, row) in page.trades.iter().zip(rows) {
        assert_eq!(trade.trade_id, row["trade_id"].as_str().unwrap());
        assert_eq!(trade.count_fp.to_string(), row["count_fp"].as_str().unwrap());
        assert_eq!(
            trade.yes_price_dollars.to_string(),
            row["yes_price_dollars"].as_str().unwrap()
        );
        assert_eq!(
            trade.taker_outcome_side,
            OutcomeSide::from_wire(row["taker_outcome_side"].as_str().unwrap())
        );
    }
}

// ── events and account ──────────────────────────────────────────

#[tokio::test]
async fn an_event_sends_nested_markets_and_decodes_the_captured_body() {
    let sent = wire("event");
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/events/EV-1",
        "with_nested_markets=true",
        &fixture("event"),
    )
    .await;
    let read = client(&server)
        .events()
        .get("EV-1")
        .with_nested_markets(true)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(
        read.event.event_ticker,
        sent["event"]["event_ticker"].as_str().unwrap()
    );
    let nested = sent["event"]["markets"].as_array().unwrap();
    assert!(!nested.is_empty());
    assert_eq!(read.event.markets.len(), nested.len());
    assert_eq!(
        read.event.markets[0].ticker,
        nested[0]["ticker"].as_str().unwrap()
    );
}

#[tokio::test]
async fn the_endpoint_costs_decode_the_captured_body() {
    let sent = wire("endpoint_costs");
    let mut server = Server::new_async().await;
    // At least once: a client may read this route itself before its first send.
    let mock = server
        .mock("GET", "/trade-api/v2/account/endpoint_costs")
        .match_query(Matcher::Missing)
        .with_status(200)
        .with_body(fixture("endpoint_costs"))
        .expect_at_least(1)
        .create_async()
        .await;
    let costs = client(&server)
        .account()
        .endpoint_costs()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(costs.default_cost, sent["default_cost"].as_u64().unwrap() as u32);
    let rows = sent["endpoint_costs"].as_array().unwrap();
    assert_eq!(costs.endpoint_costs.len(), rows.len());
    for (cost, row) in costs.endpoint_costs.iter().zip(rows) {
        assert_eq!(cost.method, row["method"].as_str().unwrap());
        assert_eq!(cost.path, row["path"].as_str().unwrap());
        assert_eq!(u64::from(cost.cost), row["cost"].as_u64().unwrap());
    }
}

// ── errors ──────────────────────────────────────────────────────

#[tokio::test]
async fn each_captured_error_body_decodes_into_its_class() {
    let cases: [(&str, usize, fn(&Class) -> bool); 4] = [
        ("error_not_found", 404, |c| matches!(c, Class::VenueRefusal { .. })),
        ("error_validation", 400, |c| matches!(c, Class::VenueRefusal { .. })),
        ("error_unauthenticated", 401, |c| matches!(c, Class::Unauthorized)),
        ("error_rate_limited", 429, |c| matches!(c, Class::RateLimited { .. })),
    ];
    for (name, status, is_class) in cases {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("GET", "/trade-api/v2/markets/trades")
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(fixture(name))
            .create_async()
            .await;
        let err = client_with_retries(&server, NO_RETRIES)
            .markets()
            .trades()
            .send()
            .await
            .unwrap_err();
        let PredictionsError::Venue(venue) = &err else {
            panic!("{name}: not a venue error: {err:?}");
        };

        // The body's own values: `{"msg": …}`, `{"error": "…"}` or `{"error": {"code": …,
        // "message": …}}`.
        let body = wire(name);
        let (code, message) = if let Some(text) = body["msg"].as_str() {
            (None, text)
        } else if let Some(text) = body["error"].as_str() {
            (None, text)
        } else {
            (
                body["error"]["code"].as_str(),
                body["error"]["message"].as_str().unwrap(),
            )
        };
        assert_eq!(usize::from(venue.status), status, "{name}");
        assert_eq!(venue.code.as_deref(), code, "{name}");
        assert_eq!(venue.message, message, "{name}");
        assert!(is_class(&err.class()), "{name}: {:?}", err.class());
    }
}

#[tokio::test]
async fn a_429_is_retried_and_the_retry_waits_out_the_backoff() {
    let mut server = Server::new_async().await;
    let served = AtomicUsize::new(0);
    let mock = server
        .mock("GET", "/trade-api/v2/markets/trades")
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |_| match served.fetch_add(1, Ordering::SeqCst) {
            0 => 429,
            _ => 200,
        })
        .with_body(fixture("trades"))
        .expect(2)
        .create_async()
        .await;
    let client = client_with_retries(
        &server,
        RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        },
    );

    let start = Instant::now();
    let page = client.markets().trades().send().await.unwrap();
    let elapsed = start.elapsed();
    mock.assert_async().await;
    assert!(!page.trades.is_empty());
    // The first backoff is 400 ms less up to a quarter for jitter.
    assert!(
        elapsed >= Duration::from_millis(300) && elapsed < Duration::from_secs(5),
        "{elapsed:?}"
    );
}

#[tokio::test]
async fn a_429_with_no_retry_left_still_holds_the_next_request() {
    let mut server = Server::new_async().await;
    let limited = server
        .mock("GET", "/trade-api/v2/markets/trades")
        .match_query(Matcher::Any)
        .with_status(429)
        .with_body(fixture("error_rate_limited"))
        .expect(1)
        .create_async()
        .await;
    let event = route(&mut server, "/trade-api/v2/events/EV-1", "", &fixture("event")).await;
    let client = client_with_retries(
        &server,
        RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        },
    );

    let err = client.markets().trades().send().await.unwrap_err();
    limited.assert_async().await;
    assert!(
        matches!(err.class(), Class::RateLimited { .. }),
        "{err:?}"
    );

    // A different route of the same client waits out the hold the 429 set: 400 ms less up to a
    // quarter for jitter, and the 429 was a moment ago.
    let start = Instant::now();
    client.events().get("EV-1").send().await.unwrap();
    let held = start.elapsed();
    event.assert_async().await;
    assert!(
        held >= Duration::from_millis(250) && held < Duration::from_secs(5),
        "{held:?}"
    );
}

// ── the route table ─────────────────────────────────────────────

#[test]
fn the_route_table_holds_the_eight_routes_in_endpoint_costs_spelling() {
    let mut routes: Vec<(String, &str)> = route_templates()
        .map(|(method, template)| (method.to_ascii_uppercase(), template))
        .collect();
    routes.sort();
    let expected = [
        ("GET", "/trade-api/v2/account/endpoint_costs"),
        ("GET", "/trade-api/v2/account/limits"),
        ("GET", "/trade-api/v2/events/:event_ticker"),
        ("GET", "/trade-api/v2/exchange/status"),
        ("GET", "/trade-api/v2/markets"),
        ("GET", "/trade-api/v2/markets/:ticker"),
        ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
        ("GET", "/trade-api/v2/markets/trades"),
    ];
    let expected: Vec<(String, &str)> = expected
        .into_iter()
        .map(|(method, template)| (method.to_owned(), template))
        .collect();
    assert_eq!(routes, expected);
}

#[test]
fn templates_match_segment_by_segment_and_a_star_takes_the_rest() {
    let ours = "/trade-api/v2/markets/:ticker/orderbook";
    // A parameter Kalshi spells differently still matches.
    assert!(template_matches(ours, "/trade-api/v2/markets/:market_ticker/orderbook"));
    assert!(template_matches(ours, ours));
    // A literal segment must be equal, and the segment counts must agree.
    assert!(!template_matches(ours, "/trade-api/v2/markets/:ticker/trades"));
    assert!(!template_matches(ours, "/trade-api/v2/markets/:ticker"));
    assert!(!template_matches(ours, "/trade-api/v2/markets/:ticker/orderbook/depth"));
    // A parameter does not match a literal.
    assert!(!template_matches("/trade-api/v2/markets/trades", "/trade-api/v2/markets/:ticker"));
    // A listed `*segment` matches everything left, and only there.
    assert!(template_matches(
        "/trade-api/v2/cfbenchmarks/a/b",
        "/trade-api/v2/cfbenchmarks/*endpoint"
    ));
    assert!(!template_matches(
        "/trade-api/v2/other/a/b",
        "/trade-api/v2/cfbenchmarks/*endpoint"
    ));
}
```

Create `polyoxide-kalshi/tests/query_setters_predictions.rs`:

```rust
//! Every query setter, called with a typed value, sends its key and value in the order it was
//! called: the golden test that holds the setters in place. A changed name or argument type
//! fails to compile; a changed key, value or order fails the test.

use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketStatusFilter, MveFilter},
    Predictions,
};
use polyoxide_test_support::query::{assert_cases, Case};

fn client(base: &str) -> Predictions {
    Predictions::builder().base_url(base).build().unwrap()
}

fn ticker() -> Ticker {
    Ticker::new("KXTEST-1").unwrap()
}

const CASES: &[Case] = &[
    Case {
        builder: "ListMarkets",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list()
                    .status(MarketStatusFilter::Settled)
                    .event_ticker("EV-1")
                    .series_ticker("SER-1")
                    .mve_filter(MveFilter::Only)
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "settled"),
            ("event_ticker", "EV-1"),
            ("series_ticker", "SER-1"),
            ("mve_filter", "only"),
            ("limit", "5"),
            ("cursor", "c1"),
        ],
    },
    Case {
        builder: "ListMarkets, a setter called twice",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).markets().list().limit(1).limit(2).send().await;
            })
        },
        sends: &[("limit", "2")],
    },
    Case {
        builder: "GetMarket",
        path: "/trade-api/v2/markets/KXTEST-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).markets().get(&ticker()).send().await;
            })
        },
        sends: &[],
    },
    Case {
        builder: "GetOrderbook",
        path: "/trade-api/v2/markets/KXTEST-1/orderbook",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .orderbook(&ticker())
                    .depth(3)
                    .send()
                    .await;
            })
        },
        sends: &[("depth", "3")],
    },
    Case {
        builder: "ListTrades",
        path: "/trade-api/v2/markets/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .trades()
                    .ticker(&ticker())
                    .limit(5)
                    .cursor("c1")
                    .min_ts(6)
                    .max_ts(7)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("ticker", "KXTEST-1"),
            ("limit", "5"),
            ("cursor", "c1"),
            ("min_ts", "6"),
            ("max_ts", "7"),
        ],
    },
    Case {
        builder: "GetEvent",
        path: "/trade-api/v2/events/EV-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .get("EV-1")
                    .with_nested_markets(true)
                    .send()
                    .await;
            })
        },
        sends: &[("with_nested_markets", "true")],
    },
    Case {
        builder: "GetEndpointCosts",
        path: "/trade-api/v2/account/endpoint_costs",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).account().endpoint_costs().send().await;
            })
        },
        sends: &[],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(CASES).await;
}
```

Add both `[[test]]` stanzas to `polyoxide-kalshi/Cargo.toml`. Current:

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "mock_predictions_routes"
required-features = ["predictions"]

[[test]]
name = "query_setters_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

- [ ] **Step 2: Run them to see them fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_routes --test query_setters_predictions
```

They do not compile: `error[E0599]: no method named `markets` found for struct `Predictions` in the current scope`, and the same for `events` and `account`.

- [ ] **Step 3: Implement**

Create `polyoxide-kalshi/src/predictions/api/markets.rs`:

```rust
//! Markets: the listing, one market, its order book and the public trades.

use std::sync::Arc;

use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        MarketResponse, MarketStatusFilter, MarketsPage, MveFilter, OrderbookResponse, TradesPage,
    },
    Inner, PredictionsError,
};

/// Markets namespace.
#[derive(Debug, Clone)]
pub struct MarketsApi {
    pub(crate) inner: Arc<Inner>,
}

impl MarketsApi {
    /// `GET /trade-api/v2/markets`: one page of markets.
    ///
    /// The host lists multivariate combinations first, thousands of them (9,996 of the first
    /// 10,000 open rows on 2026-10-18), so a walk for a single market sends
    /// `mve_filter(MveFilter::Exclude)`.
    pub fn list(&self) -> ListMarkets {
        ListMarkets {
            request: Routed::new(&self.inner, routes::MARKETS, &[]),
        }
    }

    /// `GET /trade-api/v2/markets/{ticker}`: a ticker the host does not list is a `404`.
    pub fn get(&self, ticker: &Ticker) -> GetMarket {
        GetMarket {
            request: Routed::new(&self.inner, routes::MARKET, &[ticker.as_str()]),
        }
    }

    /// `GET /trade-api/v2/markets/{ticker}/orderbook`: the YES and NO bids, each side worst
    /// first. A ticker the host does not list answers `200` with both sides empty.
    pub fn orderbook(&self, ticker: &Ticker) -> GetOrderbook {
        GetOrderbook {
            request: Routed::new(&self.inner, routes::ORDERBOOK, &[ticker.as_str()]),
        }
    }

    /// `GET /trade-api/v2/markets/trades`: public trades across markets, newest first.
    pub fn trades(&self) -> ListTrades {
        ListTrades {
            request: Routed::new(&self.inner, routes::TRADES, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/markets`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListMarkets {
    pub(crate) request: Routed<MarketsPage>,
}

impl ListMarkets {
    polyoxide_core::query_setters! {
        /// Only markets in this lifecycle state.
        status: MarketStatusFilter => "status",
        /// Only the markets of this event.
        event_ticker: impl Into<String> => "event_ticker",
        /// Only the markets of this series.
        series_ticker: impl Into<String> => "series_ticker",
        /// Only the multivariate combinations, or everything but them.
        mve_filter: MveFilter => "mve_filter",
        /// Markets per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketsPage, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/{ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetMarket {
    pub(crate) request: Routed<MarketResponse>,
}

impl GetMarket {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/{ticker}/orderbook`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetOrderbook {
    pub(crate) request: Routed<OrderbookResponse>,
}

impl GetOrderbook {
    polyoxide_core::query_setters! {
        /// Levels kept on each side, the best ones; 0 keeps every level.
        depth: u32 => "depth",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<OrderbookResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/trades`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListTrades {
    pub(crate) request: Routed<TradesPage>,
}

impl ListTrades {
    polyoxide_core::query_setters! {
        /// Only the trades of this market.
        ticker: &Ticker => "ticker",
        /// Trades per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
        /// Only trades at or after this Unix time, in seconds.
        min_ts: i64 => "min_ts",
        /// Only trades at or before this Unix time, in seconds.
        max_ts: i64 => "max_ts",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<TradesPage, PredictionsError> {
        self.request.send().await
    }
}
```

Create `polyoxide-kalshi/src/predictions/api/events.rs`:

```rust
//! Events: the sets of markets Kalshi groups.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::EventResponse,
    Inner, PredictionsError,
};

/// Events namespace.
#[derive(Debug, Clone)]
pub struct EventsApi {
    pub(crate) inner: Arc<Inner>,
}

impl EventsApi {
    /// `GET /trade-api/v2/events/{event_ticker}`: an event the host does not list is a `404`.
    pub fn get(&self, event_ticker: &str) -> GetEvent {
        GetEvent {
            request: Routed::new(&self.inner, routes::EVENT, &[event_ticker]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/events/{event_ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetEvent {
    pub(crate) request: Routed<EventResponse>,
}

impl GetEvent {
    polyoxide_core::query_setters! {
        /// Nest the event's markets in `event.markets`, as many as the event has.
        with_nested_markets: bool => "with_nested_markets",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<EventResponse, PredictionsError> {
        self.request.send().await
    }
}
```

Create `polyoxide-kalshi/src/predictions/api/account.rs`:

```rust
//! The account namespace: the routes' token costs, which answer without a key.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::EndpointCosts,
    Inner, PredictionsError,
};

/// Account namespace.
#[derive(Debug, Clone)]
pub struct AccountApi {
    pub(crate) inner: Arc<Inner>,
}

impl AccountApi {
    /// `GET /trade-api/v2/account/endpoint_costs`: the routes whose token cost differs from the
    /// default.
    pub fn endpoint_costs(&self) -> GetEndpointCosts {
        GetEndpointCosts {
            request: Routed::new(&self.inner, routes::ENDPOINT_COSTS, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/account/endpoint_costs`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetEndpointCosts {
    pub(crate) request: Routed<EndpointCosts>,
}

impl GetEndpointCosts {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<EndpointCosts, PredictionsError> {
        self.request.send().await
    }
}
```

Replace Story 1's `api/mod.rs` block in `polyoxide-kalshi/src/predictions/api/mod.rs`. Current:

```rust
pub mod exchange;
pub mod health;

pub use exchange::{ExchangeApi, GetExchangeStatus};
pub use health::Health;
```

Replace with:

```rust
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
```

Replace the `namespaces!` block in `impl Predictions` in `polyoxide-kalshi/src/predictions/mod.rs`. Current:

```rust
    polyoxide_core::namespaces! { inner;
        /// Liveness: a ping of the exchange-status route.
        health: api::Health,
        /// The exchange's status.
        exchange: api::ExchangeApi,
    }
```

Replace with (every namespace is named by its path through `api`, so `mod.rs` holds no `use` line for the namespaces and this task edits none):

```rust
    polyoxide_core::namespaces! { inner;
        /// Liveness: a ping of the exchange-status route.
        health: api::Health,
        /// The exchange's status.
        exchange: api::ExchangeApi,
        /// Markets, their order books and their public trades.
        markets: api::MarketsApi,
        /// Events and their markets.
        events: api::EventsApi,
        /// The account's limits and the routes' token costs.
        account: api::AccountApi,
    }
```

- [ ] **Step 4: Run them to see them pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_routes --test query_setters_predictions
```

`mock_predictions_routes` prints `test result: ok. 13 passed; 0 failed` and `query_setters_predictions` `test result: ok. 1 passed; 0 failed`. The two 429 tests take about half a second each.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git add -N polyoxide-kalshi/src/predictions/api/markets.rs polyoxide-kalshi/src/predictions/api/events.rs polyoxide-kalshi/src/predictions/api/account.rs polyoxide-kalshi/tests/mock_predictions_routes.rs polyoxide-kalshi/tests/query_setters_predictions.rs
git commit -m "feat(kalshi): the markets, events and account namespaces (Story 8.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-kalshi/src/predictions/api polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/tests/mock_predictions_routes.rs polyoxide-kalshi/tests/query_setters_predictions.rs polyoxide-kalshi/Cargo.toml Cargo.lock
```

### Task 2.4: The spec agreement, through a dev-only YAML parser

**Files:**
- Modify: `Cargo.toml`
- Modify: `polyoxide-kalshi/Cargo.toml`
- Test: `polyoxide-kalshi/tests/spec_agreement_predictions.rs`

Kalshi publishes no JSON spec, and no YAML crate is in the tree, so the suite reads `docs/specs/kalshi/openapi.yaml` through `serde-saphyr` (MIT or Apache-2.0, Rust 1.89), added as a dev-dependency only: it is in no `cargo tree -e normal`, so `ci/dep-allowlists/` does not change.

The shared checks (`polyoxide_test_support::openapi::check`, `query::documented_parameters`) read a document in perps' form, and Kalshi's is not, so the suite normalises its own copy of the parsed mirror in memory and never touches the file. It inlines each route's `$ref` parameters and keeps the query ones; reads a single-arm `allOf` of a `$ref` as the `$ref`; gives an object with no `properties` (`custom_strike`) an empty set; gives `PriceRange`'s string fields a decimal `example`, which the helper's own documentation says a decimal field typed `string` needs; and drops from `required` the fields the types keep a `serde(default)` for (`DEFAULTED`, each row with its reason, and a test that the mirror still requires each).

- [ ] **Step 1: Write the failing test**

Create `polyoxide-kalshi/tests/spec_agreement_predictions.rs`:

```rust
//! Agreement between the types and builders and `docs/specs/kalshi/openapi.yaml`.
//!
//! Kalshi publishes YAML only, so the mirror is read through a dev-only parser and normalised
//! in memory into the form the shared helpers read (see `mirror`); the file is never changed.
//! Only the schemas the six public routes reach are checked. A key the host sends and the
//! schema lacks is modelled as an optional field, excused in `OBSERVED_EXTRA` and recorded in
//! `docs/specs/kalshi/OBSERVED.md`, as perps' are.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};

use polyoxide_kalshi::predictions::{
    request::route_templates,
    types::{
        BookSide, EndpointCost, EndpointCosts, Event, EventResponse, Market, MarketResponse,
        MarketStatus, MarketStatusFilter, MarketsPage, MveFilter, OrderbookFp, OrderbookResponse,
        OutcomeSide, PriceRange, Trade, TradesPage,
    },
    Predictions, PredictionsError,
};
use polyoxide_test_support::{fixtures, openapi, query};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/kalshi/openapi.yaml");

/// What the mirror's paths are relative to (its `servers`), and what the crate's routes begin
/// with.
const PREFIX: &str = "/trade-api/v2";

/// `(schema, field, reason)`: fields the mirror requires and the types still read with a
/// `serde(default)`. The normalised copy lists them as optional, and
/// `every_defaulted_field_is_required_in_the_mirror` fails once the mirror stops requiring one.
const DEFAULTED: &[(&str, &str, &str)] = &[
    (
        "OrderbookCountFp",
        "yes_dollars",
        "a side with no levels is an empty list, however the host spells it",
    ),
    (
        "OrderbookCountFp",
        "no_dollars",
        "a side with no levels is an empty list, however the host spells it",
    ),
    (
        "GetEventResponse",
        "markets",
        "the deprecated list, which a caller that reads the nested markets never needs",
    ),
];

/// `(schema, field)` pairs on the wire but not in the spec, each an optional field of its type
/// and recorded in `docs/specs/kalshi/OBSERVED.md`. Empty until the capture sends a key the
/// schema lacks; a developer who meets one adds its row here.
const OBSERVED_EXTRA: &[(&str, &str)] = &[];

fn raw_spec() -> Value {
    let options = serde_saphyr::options! { budget: None };
    serde_saphyr::from_str_with_options(SPEC, options).expect("the mirror is YAML")
}

fn walk(node: &mut Value, visit: &mut impl FnMut(&mut Value)) {
    visit(node);
    match node {
        Value::Object(map) => map.values_mut().for_each(|child| walk(child, visit)),
        Value::Array(items) => items.iter_mut().for_each(|child| walk(child, visit)),
        _ => {}
    }
}

/// Each route's `parameters` as the query parameters it documents, with `$ref`s inlined.
fn inline_query_parameters(spec: &mut Value) {
    let shared = spec["components"]["parameters"].clone();
    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };
    for item in paths.values_mut() {
        let Some(parameters) = item
            .get_mut("get")
            .and_then(|get| get.get_mut("parameters"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let inlined: Vec<Value> = parameters
            .iter()
            .map(|parameter| match parameter["$ref"].as_str() {
                Some(r) => shared[openapi::ref_name(r)].clone(),
                None => parameter.clone(),
            })
            .filter(|parameter| parameter["in"] == "query")
            .collect();
        *parameters = inlined;
    }
}

/// `{allOf: [{$ref: X}], ..}` is `{$ref: X, ..}`.
fn flatten_single_all_of(node: &mut Value) {
    let Some(map) = node.as_object_mut() else {
        return;
    };
    let reference = match map.get("allOf").and_then(Value::as_array) {
        Some(arms) if arms.len() == 1 => arms[0].get("$ref").cloned(),
        _ => None,
    };
    if let Some(reference) = reference {
        map.remove("allOf");
        map.insert("$ref".to_owned(), reference);
    }
}

/// An object with no properties is the empty object, which the helper can synthesise.
fn give_bare_objects_properties(node: &mut Value) {
    let Some(map) = node.as_object_mut() else {
        return;
    };
    if map.get("type").and_then(Value::as_str) == Some("object")
        && !map.contains_key("properties")
        && !map.contains_key("allOf")
    {
        map.insert("properties".to_owned(), Value::Object(Map::new()));
    }
}

fn schema_mut<'a>(spec: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    spec.get_mut("components")?.get_mut("schemas")?.get_mut(name)
}

/// The mirror in the form the shared helpers read.
fn mirror() -> Value {
    let mut spec = raw_spec();
    inline_query_parameters(&mut spec);
    walk(&mut spec, &mut |node| {
        flatten_single_all_of(node);
        give_bare_objects_properties(node);
    });
    // `PriceRange`'s fields are strings with no example, and its type reads decimals.
    if let Some(properties) = schema_mut(&mut spec, "PriceRange")
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
    {
        for property in properties.values_mut() {
            property["example"] = Value::from("0.0100");
        }
    }
    for (schema, field, _) in DEFAULTED {
        if let Some(required) = schema_mut(&mut spec, schema)
            .and_then(|schema| schema.get_mut("required"))
            .and_then(Value::as_array_mut)
        {
            required.retain(|name| name != field);
        }
    }
    spec
}

fn mirror_schemas() -> Map<String, Value> {
    openapi::schemas(&mirror())
}

#[track_caller]
fn check_schema<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    openapi::check::<T>(schemas, name, OBSERVED_EXTRA);
}

macro_rules! modelled_schemas {
    ($($schema:literal => $ty:ty),+ $(,)?) => {
        const MODELLED: &[&str] = &[$($schema),+];
        #[test]
        fn every_modelled_schema_agrees_with_the_spec() {
            let schemas = mirror_schemas();
            $( check_schema::<$ty>(&schemas, $schema); )+
        }
    };
}

modelled_schemas! {
    "Market" => Market,
    "PriceRange" => PriceRange,
    "GetMarketResponse" => MarketResponse,
    "GetMarketsResponse" => MarketsPage,
    "OrderbookCountFp" => OrderbookFp,
    "GetMarketOrderbookResponse" => OrderbookResponse,
    "Trade" => Trade,
    "GetTradesResponse" => TradesPage,
    "EventData" => Event,
    "GetEventResponse" => EventResponse,
    "EndpointTokenCost" => EndpointCost,
    "GetAccountEndpointCostsResponse" => EndpointCosts,
}

/// Reachable object schemas deliberately not in the table, each with a reason.
const NOT_MODELLED: &[(&str, &str)] = &[
    (
        "MveSelectedLeg",
        "a combination's legs, kept as raw JSON in Market::mve_selected_legs",
    ),
    (
        "SettlementSource",
        "an event's settlement sources, kept as raw JSON in Event::settlement_sources",
    ),
];

/// The mirror's paths for the six routes this story implements.
const SPEC_ROUTES: &[&str] = &[
    "/markets",
    "/markets/{ticker}",
    "/markets/{ticker}/orderbook",
    "/markets/trades",
    "/events/{event_ticker}",
    "/account/endpoint_costs",
];

/// Every named schema reachable by `$ref` from those routes' responses.
fn reachable_from_the_routes() -> BTreeSet<String> {
    fn follow(schemas: &Map<String, Value>, node: &Value, seen: &mut BTreeSet<String>) {
        match node {
            Value::Object(map) => {
                if let Some(r) = map.get("$ref").and_then(Value::as_str) {
                    let name = openapi::ref_name(r).to_owned();
                    if seen.insert(name.clone()) {
                        follow(schemas, &schemas[&name], seen);
                    }
                }
                for child in map.values() {
                    follow(schemas, child, seen);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| follow(schemas, child, seen)),
            _ => {}
        }
    }
    let spec = mirror();
    let schemas = mirror_schemas();
    let mut seen = BTreeSet::new();
    for path in SPEC_ROUTES {
        follow(&schemas, &spec["paths"][*path]["get"]["responses"]["200"], &mut seen);
    }
    seen
}

#[test]
fn every_reachable_object_schema_is_modelled_or_excused() {
    let schemas = mirror_schemas();
    let excused: BTreeSet<&str> = NOT_MODELLED.iter().map(|(name, _)| *name).collect();
    let modelled: BTreeSet<&str> = MODELLED.iter().copied().collect();
    let unaccounted: Vec<String> = reachable_from_the_routes()
        .into_iter()
        .filter(|name| {
            let schema = &schemas[name];
            schema["type"] == "object"
                || schema.get("allOf").is_some()
                || schema.get("properties").is_some()
        })
        .filter(|name| !modelled.contains(name.as_str()) && !excused.contains(name.as_str()))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "schemas neither modelled nor excused: {unaccounted:?}"
    );
}

#[test]
fn every_defaulted_field_is_required_in_the_mirror() {
    let raw = raw_spec();
    for (schema, field, reason) in DEFAULTED {
        let required = raw["components"]["schemas"][*schema]["required"]
            .as_array()
            .unwrap_or_else(|| panic!("{schema} has no required list"));
        assert!(
            required.iter().any(|name| name == field),
            "{schema}.{field} is no longer required in the mirror; drop the DEFAULTED row ({reason})"
        );
    }
}

// ── Enums ───────────────────────────────────────────────────────────

fn wire<T: Serialize>(all: &[T]) -> BTreeSet<String> {
    all.iter()
        .map(|variant| {
            serde_json::to_value(variant)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn documented(values: &Value, what: &str) -> BTreeSet<String> {
    values
        .as_array()
        .unwrap_or_else(|| panic!("{what} has no enum"))
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn every_enum_matches_the_spec() {
    let spec = mirror();
    let schemas = mirror_schemas();
    assert_eq!(
        wire(MarketStatus::ALL),
        documented(&schemas["Market"]["properties"]["status"]["enum"], "Market.status")
    );
    assert_eq!(
        wire(OutcomeSide::ALL),
        documented(&schemas["OutcomeSide"]["enum"], "OutcomeSide")
    );
    assert_eq!(
        wire(OutcomeSide::ALL),
        documented(
            &schemas["Trade"]["properties"]["taker_outcome_side"]["enum"],
            "Trade.taker_outcome_side"
        )
    );
    assert_eq!(
        wire(OutcomeSide::ALL),
        documented(&schemas["Trade"]["properties"]["taker_side"]["enum"], "Trade.taker_side")
    );
    assert_eq!(wire(BookSide::ALL), documented(&schemas["BookSide"]["enum"], "BookSide"));

    let filter = documented(
        &spec["components"]["parameters"]["MarketStatusQuery"]["schema"]["enum"],
        "MarketStatusQuery",
    );
    assert_eq!(wire(MarketStatusFilter::ALL), filter);
}

// ── Query parameters ────────────────────────────────────────────────

type Dispatch = fn(Predictions) -> Pin<Box<dyn Future<Output = Result<(), PredictionsError>> + Send>>;

/// Sends one request through `fire`, requires its response to decode, and returns the query
/// keys it carried.
async fn keys_of(mock_path: &str, fixture: &str, fire: Dispatch) -> BTreeSet<String> {
    let body = fixtures!("predictions").text(fixture);
    query::keys_sent(&format!("{PREFIX}{mock_path}"), fixture, body, |url| {
        fire(Predictions::builder().base_url(url).build().unwrap())
    })
    .await
}

/// One entry per builder with a query: the mirror's path, the path it is sent to, the fixture
/// it must decode, and a call using every setter it has. The endpoint-costs route documents no
/// query and is checked in `routes_that_document_no_query_send_none`.
const ROUTES: &[(&str, &str, &str, Dispatch)] = &[
    ("/markets", "/markets", "markets_page", |p| {
        Box::pin(async move {
            p.markets()
                .list()
                .status(MarketStatusFilter::Open)
                .event_ticker("EV-1")
                .series_ticker("SER-1")
                .mve_filter(MveFilter::Exclude)
                .limit(5)
                .cursor("c1")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/markets/{ticker}", "/markets/KXTEST-1", "market", |p| {
        Box::pin(async move {
            let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
            p.markets().get(&ticker).send().await.map(|_| ())
        })
    }),
    (
        "/markets/{ticker}/orderbook",
        "/markets/KXTEST-1/orderbook",
        "orderbook",
        |p| {
            Box::pin(async move {
                let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
                p.markets()
                    .orderbook(&ticker)
                    .depth(1)
                    .send()
                    .await
                    .map(|_| ())
            })
        },
    ),
    ("/markets/trades", "/markets/trades", "trades", |p| {
        Box::pin(async move {
            let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
            p.markets()
                .trades()
                .ticker(&ticker)
                .limit(5)
                .cursor("c1")
                .min_ts(6)
                .max_ts(7)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/events/{event_ticker}", "/events/EV-1", "event", |p| {
        Box::pin(async move {
            p.events()
                .get("EV-1")
                .with_nested_markets(true)
                .send()
                .await
                .map(|_| ())
        })
    }),
];

/// The parameters a mirror path documents that no setter sends, each with a reason.
const NOT_SENT: &[(&str, &str, &str)] = &[
    ("/markets", "min_created_ts", "a filter the skeleton does not model"),
    ("/markets", "max_created_ts", "a filter the skeleton does not model"),
    ("/markets", "min_updated_ts", "a filter the skeleton does not model"),
    ("/markets", "max_updated_ts", "a filter the skeleton does not model"),
    ("/markets", "max_close_ts", "a filter the skeleton does not model"),
    ("/markets", "min_close_ts", "a filter the skeleton does not model"),
    ("/markets", "min_settled_ts", "a filter the skeleton does not model"),
    ("/markets", "max_settled_ts", "a filter the skeleton does not model"),
    ("/markets", "tickers", "a filter the skeleton does not model"),
    (
        "/markets/trades",
        "is_block_trade",
        "a filter the skeleton does not model",
    ),
    (
        "/events/{event_ticker}",
        "exclude_markets",
        "the skeleton asks for the markets nested or not at all",
    ),
];

#[tokio::test]
async fn every_builder_sends_only_documented_query_keys_and_every_other_one_is_excused() {
    let spec = mirror();
    let mut by_path: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (spec_path, mock_path, fixture, fire) in ROUTES {
        by_path
            .entry(spec_path)
            .or_default()
            .extend(keys_of(mock_path, fixture, *fire).await);
    }
    for (spec_path, sent) in by_path {
        let documented: BTreeSet<String> =
            query::documented_parameters(&spec, spec_path).unwrap_or_default();
        let excused: BTreeSet<String> = NOT_SENT
            .iter()
            .filter(|(path, _, _)| *path == spec_path)
            .map(|(_, parameter, _)| (*parameter).to_owned())
            .collect();
        let unknown: Vec<&String> = sent.difference(&documented).collect();
        assert!(
            unknown.is_empty(),
            "{spec_path}: query keys sent that the mirror does not document: {unknown:?}"
        );
        let unsent: BTreeSet<String> = documented.difference(&sent).cloned().collect();
        assert_eq!(
            unsent, excused,
            "{spec_path}: documented parameters no setter sends differ from NOT_SENT"
        );
    }
}

#[test]
fn routes_that_document_no_query_send_none() {
    let spec = mirror();
    for path in ["/account/endpoint_costs", "/exchange/status", "/account/limits"] {
        assert_eq!(
            query::documented_parameters(&spec, path),
            Some(BTreeSet::new()),
            "{path} documents a query, so its builder needs a setter or an entry in ROUTES"
        );
    }
}

// ── The route table ─────────────────────────────────────────────────

/// `/trade-api/v2/markets/:ticker` as the mirror writes it, without the server prefix:
/// `/markets/{ticker}`.
fn mirror_path(template: &str) -> String {
    let relative = template
        .strip_prefix(PREFIX)
        .unwrap_or_else(|| panic!("{template} does not begin with {PREFIX}"));
    relative
        .split('/')
        .map(|segment| match segment.strip_prefix(':') {
            Some(name) => format!("{{{name}}}"),
            None => segment.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn every_route_the_crate_holds_is_a_documented_operation() {
    let spec = mirror();
    let mut count = 0;
    for (method, template) in route_templates() {
        count += 1;
        let path = mirror_path(template);
        assert!(
            spec["paths"][&path][method.to_ascii_lowercase()].is_object(),
            "{method} {template} is not an operation of the mirror at {path}"
        );
    }
    assert_eq!(count, 8, "the route table holds eight routes");
}
```

Add its `[[test]]` stanza and the dev-dependency. In `polyoxide-kalshi/Cargo.toml`, current:

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "spec_agreement_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
```

It does not compile: `error[E0433]: failed to resolve: use of undeclared crate or module `serde_saphyr``.

- [ ] **Step 3: Implement**

Add the pin to the root `Cargo.toml`. Current:

```toml
ring = "0.17"
```

Replace with:

```toml
ring = "0.17"
serde-saphyr = "2.0"
```

Add the dev-dependency to `polyoxide-kalshi/Cargo.toml`. Current:

```toml
mockito = { workspace = true }
```

Replace with:

```toml
mockito = { workspace = true }
# Reads the OpenAPI mirror, which Kalshi publishes as YAML only; dev-only, so it is in no
# normal build and no dependency allowlist.
serde-saphyr = { workspace = true }
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test spec_agreement_predictions
```

It prints `test result: ok. 7 passed; 0 failed`. When a check fails, the message names the schema and the field. A field the type lacks or misnames is a type to fix; a schema neither modelled nor excused is a type to add or a row for `NOT_MODELLED`; a key the wire carries and the schema lacks is a row of `OBSERVED_EXTRA` (the field is an optional one of its type, as `openapi::check` requires) and an entry in Task 2.6's `OBSERVED.md` section. If the parser reports a budget or an alias error on the mirror, stop and ask: the options in `raw_spec` already turn the budget off.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git add -N polyoxide-kalshi/tests/spec_agreement_predictions.rs
git commit -m "test(kalshi): hold the types and query keys to the OpenAPI mirror (Story 8.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/Cargo.toml Cargo.toml Cargo.lock
```

### Task 2.5: The live suite and the registry

**Files:**
- Create: `polyoxide-kalshi/tests/common/mod.rs`
- Create: `polyoxide-kalshi/tests/live_predictions.rs`
- Modify: `polyoxide-kalshi/Cargo.toml`
- Modify (regenerated): `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`

`live_predictions` runs against production, unsigned and with no secrets, so every nightly run reads real markets. Each test is `#[ignore]`d. A failure goes through `or_fail`, `fail` or `environmental`, so the nightly classifier reads its tag; there is no `.unwrap()`, `.expect(` or bare `panic!`, and no assertion message names an error or a status, which `scripts/live_unwraps.py` counts (the suite adds nothing to its baseline).

- [ ] **Step 1: Write the live suite**

Create `polyoxide-kalshi/tests/common/mod.rs`:

```rust
//! The market the live suites read, chosen by the precondition they assert on. Shared by
//! `live_predictions.rs` and, by `mod common;`, the live suites of later stories.

use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{Market, MarketStatus, MarketStatusFilter, MveFilter},
    Predictions,
};
use polyoxide_test_support::{environmental, ResultExt};
use rust_decimal::Decimal;

/// Pages of the filtered open listing to walk.
const LISTING_PAGES: usize = 5;
/// Quoted markets to confirm on the order book route.
const CANDIDATES: usize = 20;

/// Whether the market trades with a YES bid above 0 and a YES ask below 1.
fn quotes_both_sides(market: &Market) -> bool {
    market.status == MarketStatus::Active
        && market.yes_bid_dollars > Decimal::ZERO
        && market.yes_ask_dollars < Decimal::ONE
}

/// Whether the market's order book, read on the route the suites assert on (a CDN caches per
/// URL), has levels on both sides.
async fn has_both_sides(client: &Predictions, market: &Market) -> bool {
    // A ticker this client would refuse to send is skipped here and caught by
    // `live_every_ticker_the_host_lists_is_one_the_client_accepts`.
    let Ok(ticker) = Ticker::new(&market.ticker) else {
        return false;
    };
    let book = client
        .markets()
        .orderbook(&ticker)
        .send()
        .await
        .or_fail("orderbook")
        .orderbook_fp;
    !book.yes_dollars.is_empty() && !book.no_dollars.is_empty()
}

/// An open market with a quote and an order book on both sides.
///
/// Candidates come from `markets?status=open&mve_filter=exclude&limit=1000`, at most
/// `LISTING_PAGES` pages: the unfiltered open listing serves multivariate combinations first
/// (9,996 of its first 10,000 rows on 2026-10-18), none of them quoted, while the filtered
/// listing's first page held 579 quoted rows. A row with a YES bid above 0 and a YES ask below 1
/// is confirmed on the order book route. With none, the test is an `environmental` skip.
pub async fn two_sided_market(client: &Predictions) -> Market {
    let mut tried = 0;
    let mut cursor = String::new();
    for _ in 0..LISTING_PAGES {
        let list = client
            .markets()
            .list()
            .status(MarketStatusFilter::Open)
            .mve_filter(MveFilter::Exclude)
            .limit(1000);
        let list = if cursor.is_empty() {
            list
        } else {
            list.cursor(cursor.clone())
        };
        let page = list.send().await.or_fail("markets");
        for row in page.markets.iter().filter(|m| quotes_both_sides(m)) {
            if tried >= CANDIDATES {
                return environmental(
                    "no suitable market: no quoted open market with a book on both sides",
                );
            }
            tried += 1;
            if has_both_sides(client, row).await {
                return row.clone();
            }
        }
        if page.cursor.is_empty() {
            break;
        }
        cursor = page.cursor;
    }
    environmental("no suitable market: no open market with a quote and a book on both sides")
}
```

Create `polyoxide-kalshi/tests/live_predictions.rs`:

```rust
//! Live integration tests against production Kalshi (`external-api.kalshi.com`), unsigned.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d. No credentials
//! are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector for the types, and
//! `live_the_route_table_agrees_with_the_endpoint_costs` the table of costs' (D5). The first
//! sees keys and the length of the positional order-book rows. It does not see a new value of an
//! enum, which decodes as that enum's `Other` variant.

mod common;

use std::time::Duration;

use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    request::{route_templates, template_matches, DEFAULT_COST},
    types::{
        EndpointCosts, EventResponse, ExchangeStatus, MarketResponse, MarketsPage,
        OrderbookResponse, TradesPage,
    },
    Predictions, PredictionsError, PRODUCTION,
};
use polyoxide_test_support::{agreement, fail, ResultExt};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

fn client() -> Predictions {
    Predictions::new().or_fail("kalshi client")
}

#[tokio::test]
#[ignore]
async fn live_the_exchange_status_and_a_ping_answer() {
    let client = client();
    let exchange = client
        .exchange()
        .status()
        .send()
        .await
        .or_fail("exchange status");
    assert!(
        exchange
            .exchange_index_statuses
            .iter()
            .any(|index| index.exchange_index == 0),
        "the default exchange index (0) is listed"
    );
    let pong = client.health().ping().await.or_fail("ping");
    assert!(
        pong.round_trip < Duration::from_secs(10),
        "round trip {:?}",
        pong.round_trip
    );
}

#[tokio::test]
#[ignore]
async fn live_a_two_sided_market_reads_through_every_route() {
    let client = client();
    let market = common::two_sided_market(&client).await;
    let ticker = Ticker::new(&market.ticker).or_fail(&market.ticker);

    let read = client.markets().get(&ticker).send().await.or_fail("market");
    assert_eq!(read.market.ticker, market.ticker);

    // The host lists each side worst first.
    let book = client
        .markets()
        .orderbook(&ticker)
        .send()
        .await
        .or_fail("orderbook")
        .orderbook_fp;
    assert!(
        !book.yes_dollars.is_empty() && !book.no_dollars.is_empty(),
        "{ticker}"
    );
    for levels in [&book.yes_dollars, &book.no_dollars] {
        assert!(
            levels.windows(2).all(|pair| pair[0].price < pair[1].price),
            "{ticker}: levels are not in ascending price"
        );
    }
    let shallow = client
        .markets()
        .orderbook(&ticker)
        .depth(1)
        .send()
        .await
        .or_fail("orderbook, depth 1")
        .orderbook_fp;
    assert!(
        shallow.yes_dollars.len() <= 1 && shallow.no_dollars.len() <= 1,
        "{ticker}"
    );

    let trades = client
        .markets()
        .trades()
        .ticker(&ticker)
        .limit(10)
        .send()
        .await
        .or_fail("trades");
    assert!(trades.trades.iter().all(|t| t.ticker == market.ticker));

    let event = client
        .events()
        .get(&market.event_ticker)
        .with_nested_markets(true)
        .send()
        .await
        .or_fail("event");
    assert_eq!(event.event.event_ticker, market.event_ticker);
    assert!(
        event.event.markets.iter().any(|m| m.ticker == market.ticker),
        "{ticker} is not among its event's nested markets"
    );
}

/// An unlisted ticker is a `404` on the market and a `200` with an empty book on the order book.
#[tokio::test]
#[ignore]
async fn live_an_unknown_ticker_is_a_404_on_the_market_and_an_empty_book() {
    let client = client();
    let missing = Ticker::new("NOSUCHMARKET-0").or_fail("a ticker");

    let result = client.markets().get(&missing).send().await;
    assert!(result.is_err(), "{missing} is listed");
    let Err(err) = result else { return };
    if !matches!(
        &err,
        PredictionsError::Venue(venue)
            if venue.status == 404 && venue.code.as_deref() == Some("not_found")
    ) {
        fail("expected a 404 not_found", &err);
    }

    let book = client
        .markets()
        .orderbook(&missing)
        .send()
        .await
        .or_fail("orderbook of an unknown ticker")
        .orderbook_fp;
    assert!(book.yes_dollars.is_empty() && book.no_dollars.is_empty());
}

/// Every ticker the host serves must be one `Ticker::new` accepts, or a market cannot be read.
#[tokio::test]
#[ignore]
async fn live_every_ticker_the_host_lists_is_one_the_client_accepts() {
    let client = client();
    let listed = client
        .markets()
        .list()
        .limit(1000)
        .send()
        .await
        .or_fail("markets");
    for market in &listed.markets {
        Ticker::new(&market.ticker).or_fail(&market.ticker);
    }
    let traded = client
        .markets()
        .trades()
        .limit(1000)
        .send()
        .await
        .or_fail("trades");
    for trade in &traded.trades {
        Ticker::new(&trade.ticker).or_fail(&trade.ticker);
    }
}

/// D5's drift test: the crate charges every route the default cost, so the host's default is
/// the crate's, and a route it lists must be listed at that cost.
#[tokio::test]
#[ignore]
async fn live_the_route_table_agrees_with_the_endpoint_costs() {
    let costs = client()
        .account()
        .endpoint_costs()
        .send()
        .await
        .or_fail("endpoint costs");
    assert_eq!(costs.default_cost, DEFAULT_COST, "the host's default cost");
    assert!(!costs.endpoint_costs.is_empty());

    let mut checked = 0;
    for (method, template) in route_templates() {
        checked += 1;
        for listed in costs
            .endpoint_costs
            .iter()
            .filter(|c| c.method.eq_ignore_ascii_case(method) && template_matches(template, &c.path))
        {
            assert_eq!(
                listed.cost, DEFAULT_COST,
                "{method} {template} is listed at {} tokens as {}, and the crate charges {DEFAULT_COST}",
                listed.cost, listed.path
            );
        }
    }
    assert_eq!(checked, 8, "the route table holds eight routes");
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("{PRODUCTION}{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    // A refusal fails as core classifies its status, so its tag says what the nightly does with
    // it: a 429 or a 5xx is transient.
    if !response.status().is_success() {
        fail(path, &ApiError::from_response(response).await);
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}

fn json(path: &str, text: &str) -> Value {
    serde_json::from_str(text)
        .map_err(ApiError::Serialization)
        .or_fail(path)
}

/// Positional rows of any length but `len`. The key comparison cannot see a value appended to an
/// order-book level, which has no keys.
fn rows_not_of_length(path: &str, rows: &Value, len: usize) -> Vec<String> {
    rows.as_array()
        .into_iter()
        .flatten()
        .map(|row| row.as_array().map_or(0, Vec::len))
        .filter(|&n| n != len)
        .map(|n| format!("{path}: a row of {n} values, not {len}"))
        .collect()
}

fn compare_live<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = agreement::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may be sent only
    // sometimes. A key the type does not model is.
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
    let client = client();
    let market = common::two_sided_market(&client).await;
    let ticker = &market.ticker;
    let event_ticker = &market.event_ticker;

    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(ApiError::Network)
        .or_fail("a reqwest client");
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/trade-api/v2/exchange/status";
    check(compare_live::<ExchangeStatus>(path, &raw(&http, path).await));
    let path = "/trade-api/v2/account/endpoint_costs";
    check(compare_live::<EndpointCosts>(path, &raw(&http, path).await));
    // The same filtered listing the capture and `two_sided_market` page.
    let path = "/trade-api/v2/markets?status=open&mve_filter=exclude&limit=5";
    check(compare_live::<MarketsPage>(path, &raw(&http, path).await));
    let path = format!("/trade-api/v2/markets/{ticker}");
    check(compare_live::<MarketResponse>(&path, &raw(&http, &path).await));
    let path = format!("/trade-api/v2/markets/{ticker}/orderbook");
    let text = raw(&http, &path).await;
    check(compare_live::<OrderbookResponse>(&path, &text));
    let book = json(&path, &text);
    check(rows_not_of_length(&path, &book["orderbook_fp"]["yes_dollars"], 2));
    check(rows_not_of_length(&path, &book["orderbook_fp"]["no_dollars"], 2));
    let path = format!("/trade-api/v2/markets/trades?ticker={ticker}&limit=10");
    check(compare_live::<TradesPage>(&path, &raw(&http, &path).await));
    let path = format!("/trade-api/v2/events/{event_ticker}?with_nested_markets=true");
    check(compare_live::<EventResponse>(&path, &raw(&http, &path).await));

    assert!(
        unmodelled.is_empty(),
        "the host sent what the types do not model; model it as an optional field, excuse it \
         in OBSERVED_EXTRA and record it in docs/specs/kalshi/OBSERVED.md: {unmodelled:#?}"
    );
}
```

Add the Cargo entries to `polyoxide-kalshi/Cargo.toml`. The stanza: current

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "live_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

The live table, directly before the docs.rs table. Current:

```toml
[package.metadata.docs.rs]
```

Replace with:

```toml
[package.metadata.polyoxide.live.live_predictions]
suite = "predictions"
timeout = 15
features = ["predictions"]
secrets = []

[package.metadata.docs.rs]
```

- [ ] **Step 2: Run the registry check to see it fail**

```bash
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions --no-run
nix develop --command python3 scripts/gen_registry.py --check
```

The suite compiles. The check fails, naming the generated regions that now differ from what the metadata produces: the nightly job list in `CLAUDE.md`, the same list in `.github/workflows/nightly-behavioral.yml`, and `SELF-HEALING.md`.

- [ ] **Step 3: Regenerate the regions**

```bash
nix develop --command python3 scripts/gen_registry.py --write
```

It adds a `live-polyoxide-kalshi-predictions` job (15 min, `live_predictions` with `--features predictions`) to those three files, and changes nothing outside their generated regions. Read the diff before committing.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/live_unwraps.py
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored
```

The check and the unwrap script exit 0. The live run prints `test result: ok. 6 passed; 0 failed`. A test that fails prints `polyoxide-class=<tag>` on a line of its own before its panic; `real` means the host disagrees with the crate. `live_responses_carry_no_unmodelled_keys` listing a key means a field to model as an optional field of its type, to excuse in `OBSERVED_EXTRA` in `spec_agreement_predictions.rs` and to record in Task 2.6's `OBSERVED.md` section. `live_the_route_table_agrees_with_the_endpoint_costs` failing means the compiled cost of a route is wrong, which is `request.rs`'s, Story 1's.

- [ ] **Step 5: Commit**

```bash
nix develop --command cargo fmt --all
git add -N polyoxide-kalshi/tests/common/mod.rs polyoxide-kalshi/tests/live_predictions.rs
git commit -m "test(kalshi): the public REST live suite against production (Story 8.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-kalshi/tests/common/mod.rs polyoxide-kalshi/tests/live_predictions.rs polyoxide-kalshi/Cargo.toml .github/workflows/nightly-behavioral.yml SELF-HEALING.md CLAUDE.md
```

### Task 2.6: The public REST findings in OBSERVED.md

**Files:**
- Modify: `docs/specs/kalshi/OBSERVED.md` (by its anchor)

Story 1 wrote `docs/specs/kalshi/OBSERVED.md` and ended it with the anchor line `<!-- capture findings go above this line -->`. This task inserts a section directly above that line and edits nothing else in the file. The section records what the capture and the spec-agreement suite found, which is why the walks filter and which keys the schema lacks.

- [ ] **Step 1: Write the failing check**

```bash
nix develop --command python3 - <<'PY'
import pathlib

lines = pathlib.Path("docs/specs/kalshi/OBSERVED.md").read_text().rstrip("\n").split("\n")
text = "\n".join(lines)
assert lines[-1] == "<!-- capture findings go above this line -->", "the anchor is not the last line"
assert "## Public REST" in text, "no Public REST section"
assert "mve_filter=exclude" in text and "9,996" in text and "579" in text, "the listing finding is missing"
print("ok")
PY
```

- [ ] **Step 2: Run it to see it fail**

Run the block above. It fails with `AssertionError: no Public REST section`.

- [ ] **Step 3: Write the section**

Insert the section directly above the anchor line. In the file, replace the anchor line:

```text
<!-- capture findings go above this line -->
```

with the section below followed by the anchor line again:

````markdown
## Public REST (Story 2, captured 2026-10-18)

Read from `https://external-api.kalshi.com/trade-api/v2` with no credentials by
`scripts/capture_kalshi_predictions.py`; the fixtures are in
`polyoxide-kalshi/tests/fixtures/predictions/` and their `PROVENANCE.md` says how each was trimmed.

### The open listing serves multivariate combinations first

`GET /markets?status=open&limit=1000`, paged by cursor, returned 9,996 multivariate combinations
(`mve_collection_ticker` set) in its first ten pages of 10,000 rows, and no single market with a
quote on both sides. With `mve_filter=exclude` the first page held 579 rows with a YES bid above 0
and a YES ask below 1. So a walk for a quoted single market filters: the capture script and the live
suites' `two_sided_market` page `markets?status=open&mve_filter=exclude&limit=1000` (at most 10 pages
for the capture, 5 for the live helper) and confirm both sides on that market's orderbook route, since
a CDN caches per URL. `mve_filter` takes `only` and `exclude`, and the client's `MveFilter` carries
both.

### The `status` filter takes five values

The OpenAPI's parameter enum for `GET /markets`'s `status` lists `unopened`, `open`, `paused`,
`closed` and `settled`; its prose leaves out `paused`. `MarketStatusFilter` carries all five, and
`spec_agreement_predictions` checks them against the mirror's parameter enum.

### Keys the wire carries and the schema lacks

Each is modelled as an optional field of its type and excused in `OBSERVED_EXTRA` in
`spec_agreement_predictions.rs`, as perps' are. The capture found no key the schema lacks.
<!-- capture findings go above this line -->
````

That last sentence is true only if `OBSERVED_EXTRA` is empty. For each `(schema, field)` row a developer added to `OBSERVED_EXTRA` while the earlier tasks ran, replace the sentence with a table of one row per excused key, with the header `| Schema | Field | What the host sends |` and, in the last column, an example value and the fixture it appears in.

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 block again. It prints `ok`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(specs): Kalshi's public REST findings, the filtered listing and the five status values (Story 8.2)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>" -m "Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- docs/specs/kalshi/OBSERVED.md
```

## Story 3: Kalshi market data through the traits (Story 8.3)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/venue.rs`, `polyoxide-kalshi/tests/venue_predictions.rs`, `polyoxide-kalshi/tests/live_predictions_venue.rs`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- **Interfaces:** exposes `impl MarketData`, `impl Trades` and `impl EventGroups` for `Predictions`, and `predictions::venue::{InstrumentExt, TradeExt, EventGroupExt}`; uses Story 1's keys and module block, Story 2's namespaces, types and fixtures, and `tests/common/mod.rs`
- **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
- **Review:** deep (the YES/NO conversion every consumer's book depends on)

Implements `MarketData`, `Trades` and `EventGroups` on `Predictions` in `predictions/venue.rs`, keyed by Story 1's key space, over Story 2's namespaces and types. One function, `sides`, turns Kalshi's two lists of resting bids (YES and NO, served worst first) into an outcome's book: its own bids best first, and the other outcome's bids as asks at `1 - price`, best first (D9). `instrument` turns `/markets/{t}`'s 404 into `NotListed` (D11), while `quote` and `book` of an unknown ticker come back empty because Kalshi answers `200` with two empty lists for it. Instruments are `SizeUnit::Shares` with D12's fields. Times are RFC 3339 on the REST side and are read with `chrono` into `UnixMillis`; one that does not parse is a `Decode` error that names its field (D25). Kalshi-only facts ride in `InstrumentExt`, `TradeExt` and `EventGroupExt`. Task 3.1 adds the umbrella dev-dependency, the `venue_predictions` suite and `MarketData`; Task 3.2 adds `Trades`; Task 3.3 adds `EventGroups`; Task 3.4 adds the `live_predictions_venue` target and regenerates the registry regions. `venue_predictions` holds each method to the native client's values on Story 2's fixtures, holds the conversion to literal prices (the fixtures' own numbers are unknown to this plan), and runs Story 6.6's generic `best_bid_ask` through the umbrella's example, included by `#[path]`, with the umbrella's dev-dependency on the example's three features (D22). No code of this story holds a landmine marker.

### Task 3.1: `MarketData` on `Predictions`

**Files:**
- Modify: `polyoxide-kalshi/Cargo.toml` (the `mockito` dev-dependency line, and the line above `[package.metadata.polyoxide]`)
- Modify: `polyoxide-kalshi/src/predictions/mod.rs` (the module block at the top of the file)
- Create: `polyoxide-kalshi/src/predictions/venue.rs`
- Test: `polyoxide-kalshi/tests/venue_predictions.rs`, `polyoxide-kalshi/src/predictions/venue.rs`

- [ ] **Step 1: Write the failing tests.** First the umbrella dev-dependency and the suite's `[[test]]` stanza, so the suite builds only with the feature it needs. The umbrella is path-only (cargo strips it on publish, and the crate is `publish = false`), and its three features are the example's `required-features`. A dev-dependency is in no `cargo tree -e normal`, so `ci/dep-allowlists/` does not change. The suite also needs the tokio dev-dependency to carry `macros` and `rt-multi-thread` (the example's `#[tokio::main]`); Story 1's `[dev-dependencies]` already declares it that way, as `polyoxide-binance`'s does, and this task does not declare it again.

Current (`polyoxide-kalshi/Cargo.toml`):

```toml
mockito = { workspace = true }
```

Replace with:

```toml
mockito = { workspace = true }
# `tests/venue_predictions.rs` includes the umbrella's `examples/venue_neutral.rs`
# by `#[path]`. The three features are that example's `required-features`; they
# build the CLOB, perps and USDⓈ-M stacks into Kalshi's test builds only.
# Path-only, so cargo strips it on publish.
polyoxide = { path = "../polyoxide", default-features = false, features = ["polymarket-clob", "polymarket-perps", "binance-usdm"] }
```

Current (`polyoxide-kalshi/Cargo.toml`):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "venue_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Declare the module, so the suite's `venue::InstrumentExt` import has a module to fail on. The module block is Story 1's.

Current (`polyoxide-kalshi/src/predictions/mod.rs`):

```rust
pub mod api;
pub mod error;
pub mod key;
pub mod request;
mod throttle;
pub mod types;

pub use error::{PredictionsError, VenueError};
pub use key::key;
```

Replace with:

```rust
pub mod api;
pub mod error;
pub mod key;
pub mod request;
mod throttle;
pub mod types;
pub mod venue;

pub use error::{PredictionsError, VenueError};
pub use key::key;
```

Create `polyoxide-kalshi/tests/venue_predictions.rs` with the tests below. Its numbers are literal wherever the conversion is under test: a NO bid at 0.52 must come out as a YES ask at 0.48, and a test that recomputed that with the code under test would pass on any rule. The tests that read Story 2's fixtures take their expected values from the native client's decode of the same body, and each asserts the fixture is not one-sided, so none passes over an empty book. The fixtures are Kalshi's responses as served (`market.json` is `{"market": {...}}`, `orderbook.json` is `{"orderbook_fp": {...}}`), and a test that needs a market with another status, price ranges or time edits a copy of that JSON. `start()` mounts the account's `endpoint_costs` fixture on every server that serves a trait call, never asserted: Story 4 makes a client's first send read that route, and the suite is the same before and after it.

```rust
//! Kalshi's `predictions` module through the venue-neutral traits, each held as
//! its `Dyn` wrapper, against mock servers serving the captured fixtures: every
//! method returns the values the native client decodes from the same body, and
//! the YES/NO conversion is held to literal prices.

#[allow(dead_code)]
#[path = "../../polyoxide/examples/venue_neutral.rs"]
mod example;

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use example::{best_bid_ask, classify, classify_classified, Handling};
use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    types::{MarketStatus, PriceLevel},
    venue::InstrumentExt,
    Predictions,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    Book, Class, ClassifiedError, DynMarketData, Instrument, KeySpace, Level, MarketData, NotListed,
    ProductId, SizeUnit, UnixMillis, VenueId,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};

/// A ticker of this suite's own, for the tests that serve an inline body.
const TICKER: &str = "KXTEST-26OCT18-T50.5";

/// A book as Kalshi serves it: each side's levels in ascending price, worst
/// first. Four bids: YES 0.40 and 0.45, NO 0.50 and 0.52.
const BOOK: &str = r#"{"orderbook_fp":{"yes_dollars":[["0.4000","10.00"],["0.4500","5.00"]],"no_dollars":[["0.5000","7.00"],["0.5200","3.00"]]}}"#;

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn fixture_json(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

/// No retries, so a refused request answers at once.
fn once() -> RetryConfig {
    RetryConfig {
        max_retries: 0,
        ..RetryConfig::default()
    }
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(once())
        .build()
        .unwrap()
}

/// A server with the account's `endpoint_costs` mounted and never asserted: a
/// client built with Story 4 reads it on its first send, and one built before
/// never asks.
async fn start() -> ServerGuard {
    let mut server = Server::new_async().await;
    let _costs = server
        .mock("GET", "/trade-api/v2/account/endpoint_costs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fixture("endpoint_costs"))
        .create_async()
        .await;
    server
}

/// A route answering 200 with `body` `hits` times, matched on its whole query.
async fn route(server: &mut ServerGuard, path: &str, query: Matcher, body: &str, hits: usize) -> Mock {
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

/// A server that fails the test if it is asked for anything.
async fn silent() -> (ServerGuard, Mock) {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    (server, nothing)
}

fn test_ticker() -> Ticker {
    Ticker::new(TICKER).unwrap()
}

/// The ticker of a `{"market": {...}}` body.
fn ticker_of(body: &Value) -> Ticker {
    Ticker::new(body["market"]["ticker"].as_str().unwrap()).unwrap()
}

fn market_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/markets/{ticker}")
}

fn book_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/markets/{ticker}/orderbook")
}

fn lv(price: &str, size: &str) -> Level {
    Level::new(
        price.parse::<Decimal>().unwrap(),
        size.parse::<Decimal>().unwrap(),
    )
}

/// The levels an opposite book's bids are, as asks: each price `p` is `1 - p`.
fn flipped(levels: &[Level]) -> Vec<Level> {
    levels
        .iter()
        .map(|level| Level::new(Decimal::ONE - level.price, level.size))
        .collect()
}

/// The two outcomes' books of one market are one book read from both sides.
fn assert_mirrored(yes: &Book, no: &Book) {
    assert_eq!(yes.bids, flipped(&no.asks));
    assert_eq!(yes.asks, flipped(&no.bids));
}

/// A book with both sides, whose best bid is below its best ask.
fn assert_uncrossed(book: &Book) {
    let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) else {
        panic!("{}: a side of the book is empty: {book:?}", book.key);
    };
    assert!(bid.price < ask.price, "{}: {bid:?} crosses {ask:?}", book.key);
}

/// The highest-priced level of one side, as (price, count).
fn best(levels: &[PriceLevel]) -> (Decimal, Decimal) {
    let top = levels
        .iter()
        .max_by_key(|level| level.price)
        .expect("the captured book has both sides");
    (top.price, top.count)
}

/// `instrument` on a mock server serving `body` for the test ticker's market.
async fn instrument_of(body: &Value, outcome: Outcome) -> Result<Instrument, ClassifiedError> {
    let ticker = ticker_of(body);
    let mut server = start().await;
    let _market = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &body.to_string(),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    venue.instrument(&key::key(&ticker, outcome)).await
}

/// A key of a product that is not Kalshi's.
struct Other;

impl KeySpace for Other {
    const VENUE: VenueId = VenueId::from_static("other");
    const PRODUCT: ProductId = ProductId::from_static("spot");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        Ok(text.to_owned())
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

fn foreign_key() -> polyoxide_venue::MarketKey {
    Other::key(&"ABC".to_owned())
}

#[tokio::test]
async fn a_dyn_market_data_returns_the_native_instrument_of_both_outcomes() {
    let body = fixture_json("market");
    let ticker = ticker_of(&body);
    let mut server = start().await;
    let mock = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &body.to_string(),
        3,
    )
    .await;
    let client = client(&server);
    let native = client.markets().get(&ticker).send().await.unwrap().market;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);

    for outcome in [Outcome::Yes, Outcome::No] {
        let k = key::key(&ticker, outcome);
        let instrument = venue.instrument(&k).await.unwrap();
        assert_eq!(instrument.key, k);
        let subtitle = match outcome {
            Outcome::Yes => &native.yes_sub_title,
            Outcome::No => &native.no_sub_title,
        };
        let name = if subtitle.is_empty() {
            format!("{ticker} {}", outcome.as_str())
        } else {
            subtitle.clone()
        };
        assert_eq!(instrument.name, name);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.base_asset, None);
        assert_eq!(instrument.quote_asset.as_deref(), Some("USD"));
        assert_eq!(instrument.min_size, Some(Decimal::new(1, 2)));
        assert_eq!(instrument.size_increment, Some(Decimal::new(1, 2)));
        assert_eq!(instrument.min_notional, None);
        assert_eq!(
            instrument.tick_size,
            match native.price_ranges.as_slice() {
                [only] => Some(only.step),
                _ => None,
            }
        );
        assert_eq!(
            instrument.tradable,
            Some(native.status == MarketStatus::Active)
        );

        let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
        assert_eq!(
            ext.event_ticker.as_deref(),
            Some(native.event_ticker.as_str())
        );
        assert_eq!(ext.status.as_ref(), Some(&native.status));
        assert_eq!(ext.price_ranges.as_ref(), Some(&native.price_ranges));
        assert_eq!(
            ext.price_level_structure.as_deref(),
            Some(native.price_level_structure.as_str())
        );
        assert_eq!(ext.notional_value, Some(native.notional_value_dollars));
        assert_eq!(
            ext.settlement_bounds_type.as_deref(),
            Some(native.settlement_bounds_type.as_str())
        );
        assert_eq!(ext.exchange_index, native.exchange_index);
        assert!(ext.close_time.is_some() && ext.latest_expiration_time.is_some());
        assert_eq!(
            ext.expected_expiration_time.is_some(),
            native.expected_expiration_time.is_some()
        );
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn a_markets_status_and_price_ranges_decide_tradable_and_tick_size() {
    let mut body = fixture_json("market");
    body["market"]["status"] = json!("active");
    body["market"]["price_ranges"] = json!([{"start": "0.0000", "end": "1.0000", "step": "0.0100"}]);
    let one = instrument_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(one.tick_size, Some(Decimal::new(1, 2)));
    assert_eq!(one.tradable, Some(true));
    let ext = one.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.status, Some(MarketStatus::Active));

    // Two ranges have no one step, and a closed market takes no orders.
    body["market"]["status"] = json!("closed");
    body["market"]["price_ranges"] = json!([
        {"start": "0.0000", "end": "0.1000", "step": "0.0010"},
        {"start": "0.1000", "end": "1.0000", "step": "0.0100"}
    ]);
    let two = instrument_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(two.tick_size, None);
    assert_eq!(two.tradable, Some(false));

    // An empty subtitle falls back to the ticker and the outcome.
    body["market"]["no_sub_title"] = json!("");
    let named = instrument_of(&body, Outcome::No).await.unwrap();
    assert_eq!(named.name, format!("{} no", ticker_of(&body)));
}

#[tokio::test]
async fn the_instruments_times_are_rfc_3339_and_one_that_is_not_is_a_decode_failure() {
    let mut body = fixture_json("market");
    body["market"]["close_time"] = json!("2026-10-18T09:10:11.250Z");
    body["market"]["latest_expiration_time"] = json!("2026-10-18T11:10:12+02:00");
    body["market"]["expected_expiration_time"] = json!("2026-10-18T10:10:11Z");
    let instrument = instrument_of(&body, Outcome::Yes).await.unwrap();
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.close_time, Some(UnixMillis(1_792_314_611_250)));
    assert_eq!(ext.latest_expiration_time, Some(UnixMillis(1_792_314_612_000)));
    assert_eq!(
        ext.expected_expiration_time,
        Some(UnixMillis(1_792_318_211_000))
    );

    // No expected expiration is none, not a failure.
    body["market"]["expected_expiration_time"] = Value::Null;
    let instrument = instrument_of(&body, Outcome::Yes).await.unwrap();
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.expected_expiration_time, None);

    // A time that does not parse names its field; it is never a silent zero.
    body["market"]["close_time"] = json!("tomorrow");
    let err = instrument_of(&body, Outcome::Yes).await.unwrap_err();
    assert_eq!(err.class, Class::Decode);
    assert!(err.to_string().contains("close_time"), "{err}");
}

#[tokio::test]
async fn a_dyn_market_data_returns_the_native_quote_of_both_outcomes() {
    let body = fixture("orderbook_depth1");
    let ticker = ticker_of(&fixture_json("market"));
    let mut server = start().await;
    let mock = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Exact("depth=1".to_owned()),
        &body,
        3,
    )
    .await;
    let client = client(&server);
    let native = client
        .markets()
        .orderbook(&ticker)
        .depth(1)
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    assert!(
        !native.yes_dollars.is_empty() && !native.no_dollars.is_empty(),
        "the captured depth-1 book has both sides"
    );
    let (yes_price, yes_count) = best(&native.yes_dollars);
    let (no_price, no_count) = best(&native.no_dollars);
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);

    let yes = venue.quote(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert_eq!(yes.bid, Some(Level::new(yes_price, yes_count)));
    assert_eq!(
        yes.ask,
        Some(Level::new(Decimal::ONE - no_price, no_count))
    );
    assert_eq!(yes.time, None);
    let no = venue.quote(&key::key(&ticker, Outcome::No)).await.unwrap();
    assert_eq!(no.bid, Some(Level::new(no_price, no_count)));
    assert_eq!(
        no.ask,
        Some(Level::new(Decimal::ONE - yes_price, yes_count))
    );

    // Neither quote crosses, and the two outcomes are one quote read twice.
    assert!(yes.bid.unwrap().price < yes.ask.unwrap().price);
    assert!(no.bid.unwrap().price < no.ask.unwrap().price);
    assert_eq!(yes.bid, no.ask.map(|ask| Level::new(Decimal::ONE - ask.price, ask.size)));
    mock.assert_async().await;
}

#[tokio::test]
async fn a_dyn_market_data_returns_the_native_book_of_both_outcomes() {
    let body = fixture("orderbook");
    let ticker = ticker_of(&fixture_json("market"));
    let mut server = start().await;
    let mock = route(&mut server, &book_path(&ticker), Matcher::Missing, &body, 3).await;
    let client = client(&server);
    let native = client
        .markets()
        .orderbook(&ticker)
        .send()
        .await
        .unwrap()
        .orderbook_fp;
    assert!(
        !native.yes_dollars.is_empty() && !native.no_dollars.is_empty(),
        "the captured book has both sides"
    );
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client);

    let yes_levels: Vec<Level> = native
        .yes_dollars
        .iter()
        .map(|level| Level::new(level.price, level.count))
        .collect();
    let no_levels: Vec<Level> = native
        .no_dollars
        .iter()
        .map(|level| Level::new(level.price, level.count))
        .collect();
    let highest_first = |mut levels: Vec<Level>| {
        levels.sort_by(|a, b| b.price.cmp(&a.price));
        levels
    };
    let lowest_first = |mut levels: Vec<Level>| {
        levels.sort_by(|a, b| a.price.cmp(&b.price));
        levels
    };

    let yes = venue.book(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert_eq!(yes.bids, highest_first(yes_levels.clone()));
    assert_eq!(yes.asks, lowest_first(flipped(&no_levels)));
    let no = venue.book(&key::key(&ticker, Outcome::No)).await.unwrap();
    assert_eq!(no.bids, highest_first(no_levels.clone()));
    assert_eq!(no.asks, lowest_first(flipped(&yes_levels)));

    assert_uncrossed(&yes);
    assert_uncrossed(&no);
    assert_mirrored(&yes, &no);
    mock.assert_async().await;
}

#[tokio::test]
async fn a_no_bid_is_a_yes_ask_at_one_minus_its_price() {
    let ticker = test_ticker();
    let mut server = start().await;
    let depth1 = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Exact("depth=1".to_owned()),
        BOOK,
        2,
    )
    .await;
    let full = route(&mut server, &book_path(&ticker), Matcher::Missing, BOOK, 2).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let yes_key = key::key(&ticker, Outcome::Yes);
    let no_key = key::key(&ticker, Outcome::No);

    // YES bids 0.45 and 0.40; NO bids 0.52 and 0.50 are YES asks at 0.48 and 0.50.
    let yes = venue.book(&yes_key).await.unwrap();
    assert_eq!(yes.bids, [lv("0.45", "5"), lv("0.40", "10")]);
    assert_eq!(yes.asks, [lv("0.48", "3"), lv("0.50", "7")]);
    // NO bids 0.52 and 0.50; YES bids 0.45 and 0.40 are NO asks at 0.55 and 0.60.
    let no = venue.book(&no_key).await.unwrap();
    assert_eq!(no.bids, [lv("0.52", "3"), lv("0.50", "7")]);
    assert_eq!(no.asks, [lv("0.55", "5"), lv("0.60", "10")]);
    assert_mirrored(&yes, &no);

    let yes_quote = venue.quote(&yes_key).await.unwrap();
    assert_eq!(yes_quote.bid, Some(lv("0.45", "5")));
    assert_eq!(yes_quote.ask, Some(lv("0.48", "3")));
    let no_quote = venue.quote(&no_key).await.unwrap();
    assert_eq!(no_quote.bid, Some(lv("0.52", "3")));
    assert_eq!(no_quote.ask, Some(lv("0.55", "5")));
    depth1.assert_async().await;
    full.assert_async().await;
}

#[tokio::test]
async fn a_one_sided_book_leaves_the_other_side_of_the_quote_empty() {
    let ticker = test_ticker();
    let mut server = start().await;
    let _book = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Any,
        r#"{"orderbook_fp":{"yes_dollars":[["0.4000","10.00"]],"no_dollars":[]}}"#,
        2,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));

    let yes = venue.quote(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert_eq!(yes.bid, Some(lv("0.40", "10")));
    assert_eq!(yes.ask, None);
    let no = venue.quote(&key::key(&ticker, Outcome::No)).await.unwrap();
    assert_eq!(no.bid, None);
    assert_eq!(no.ask, Some(lv("0.60", "10")));
}

#[tokio::test]
async fn an_unknown_ticker_is_not_listed_but_its_quote_and_book_are_empty() {
    let ticker = test_ticker();
    let mut server = start().await;
    let market = server
        .mock("GET", market_path(&ticker).as_str())
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .expect(2)
        .create_async()
        .await;
    // Kalshi answers an unknown ticker's orderbook with 200 and two empty lists.
    let books = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Any,
        r#"{"orderbook_fp":{"no_dollars":[],"yes_dollars":[]}}"#,
        4,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));

    for outcome in [Outcome::Yes, Outcome::No] {
        let k = key::key(&ticker, outcome);
        let err = venue.instrument(&k).await.unwrap_err();
        assert_eq!(err.class, Class::VenueRefusal { code: None });
        assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, k);

        let quote = venue.quote(&k).await.unwrap();
        assert_eq!((quote.bid, quote.ask), (None, None));
        let book = venue.book(&k).await.unwrap();
        assert!(book.bids.is_empty() && book.asks.is_empty(), "{book:?}");
    }
    market.assert_async().await;
    books.assert_async().await;
}

#[tokio::test]
async fn only_a_404_is_not_listed() {
    let ticker = test_ticker();
    let k = key::key(&ticker, Outcome::Yes);

    // A 404 whose body is none of Kalshi's three shapes stays a core error and
    // is still a missing market.
    let mut server = start().await;
    let _plain = server
        .mock("GET", market_path(&ticker).as_str())
        .with_status(404)
        .with_body("not found")
        .create_async()
        .await;
    let venue = DynMarketData::new_arc(client(&server));
    let err = venue.instrument(&k).await.unwrap_err();
    assert!(err.source.downcast_ref::<NotListed>().is_some(), "{err}");

    for status in [400, 401, 429, 503] {
        let mut server = start().await;
        let _refusal = server
            .mock("GET", market_path(&ticker).as_str())
            .with_status(status)
            .with_body(r#"{"error":{"code":"refused","message":"refused"}}"#)
            .create_async()
            .await;
        let venue = DynMarketData::new_arc(client(&server));
        let err = venue.instrument(&k).await.unwrap_err();
        assert!(
            err.source.downcast_ref::<NotListed>().is_none(),
            "{status} is not a missing market: {err}"
        );
    }
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let foreign = foreign_key();

    for err in [
        venue.instrument(&foreign).await.unwrap_err(),
        venue.quote(&foreign).await.unwrap_err(),
        venue.book(&foreign).await.unwrap_err(),
    ] {
        assert_eq!(err.class, Class::InvalidRequest);
    }
    nothing.assert_async().await;
}

#[tokio::test]
async fn best_bid_ask_runs_on_a_dyn_market_data_over_kalshi() {
    let ticker = test_ticker();
    let mut server = start().await;
    let mock = route(
        &mut server,
        &book_path(&ticker),
        Matcher::Exact("depth=1".to_owned()),
        BOOK,
        2,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));

    let yes = best_bid_ask(&venue, &key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    assert_eq!(yes, (Some(lv("0.45", "5")), Some(lv("0.48", "3"))));
    let no = best_bid_ask(&venue, &key::key(&ticker, Outcome::No))
        .await
        .unwrap();
    assert_eq!(no, (Some(lv("0.52", "3")), Some(lv("0.55", "5"))));
    mock.assert_async().await;
}

#[tokio::test]
async fn one_classifier_reads_the_errors_the_traits_return_as_it_reads_the_native_ones() {
    let ticker = test_ticker();
    let cases = [
        (
            429,
            fixture("error_rate_limited"),
            None,
            Handling::Backoff(None),
        ),
        (
            429,
            fixture("error_rate_limited"),
            Some("7"),
            Handling::Backoff(Some(Duration::from_secs(7))),
        ),
        (
            503,
            r#"{"error":{"code":"service_unavailable","message":"try later"}}"#.to_owned(),
            None,
            Handling::Retry,
        ),
        (
            401,
            fixture("error_unauthenticated"),
            None,
            Handling::Reauthenticate,
        ),
        (
            400,
            fixture("error_validation"),
            None,
            Handling::Rejected { alert: true },
        ),
        (
            404,
            fixture("error_not_found"),
            None,
            Handling::Rejected { alert: true },
        ),
    ];
    for (status, body, retry_after, expected) in cases {
        let mut server = Server::new_async().await;
        let mut mock = server
            .mock("GET", book_path(&ticker).as_str())
            .match_query(Matcher::Any)
            .with_status(status)
            .with_body(body);
        if let Some(wait) = retry_after {
            mock = mock.with_header("retry-after", wait);
        }
        let _mock = mock.create_async().await;

        let native = client(&server)
            .markets()
            .orderbook(&ticker)
            .depth(1)
            .send()
            .await
            .unwrap_err();
        assert_eq!(classify(&native), expected, "{status}, native");

        let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
        let err = best_bid_ask(&venue, &key::key(&ticker, Outcome::Yes))
            .await
            .unwrap_err();
        assert_eq!(
            classify_classified(&err),
            expected,
            "{status}, through the traits"
        );
    }
}
```

Create `polyoxide-kalshi/src/predictions/venue.rs` holding only its first doc line, so the module exists and the suite fails on the name it lacks, not on a missing file:

```rust
//! Kalshi's predictions exchange through the venue-neutral market-data traits.
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: the build fails. The suite imports `polyoxide_kalshi::predictions::venue::InstrumentExt`, which the empty module lacks (once that resolves, `DynMarketData::new_arc(Predictions)` fails for want of a `MarketData` impl):

```
error[E0432]: unresolved import `polyoxide_kalshi::predictions::venue::InstrumentExt`
```

- [ ] **Step 3: Implement.** Replace the one-line module with the whole file. `sides` is the YES/NO conversion (D9): every outcome's bids are its own list, and its asks are the other list's prices mapped to `1 - price`. `millis` is D25's one parse. `Predictions::listed` is the private read of a key's market that `instrument` and, in Task 3.3, `event_group` share; its 404 is the one `NotListed` (D11), found on either of the error's two shapes, since a 404 whose body is none of Kalshi's three stays a core `ApiError::Response`. The module docs name no code fence, as the crate READMEs do not.

Replace the contents of `polyoxide-kalshi/src/predictions/venue.rs` (it holds only `//! Kalshi's predictions exchange through the venue-neutral market-data traits.`) with:

```rust
//! Kalshi's predictions exchange through the venue-neutral market-data traits.
//!
//! [`Predictions`] implements `MarketData` over its own routes, keyed
//! `kalshi.predictions:<TICKER>:yes` and `kalshi.predictions:<TICKER>:no` by the
//! constructors in `predictions::key`: a key names one outcome of one market,
//! never the market. What Kalshi alone publishes rides in this module's
//! `<Record>Ext` types, one per record, in each record's
//! [`Extensions`](polyoxide_venue::Extensions).
//!
//! Kalshi lists each market's resting bids twice, once for YES and once for NO,
//! each side in ascending price. A NO bid at price *q* is a YES ask at *1 - q*,
//! and a YES bid at *p* is a NO ask at *1 - p*. So a key's book is its own
//! outcome's bids, best first, and the other outcome's bids turned into asks,
//! best first, on every path: `quote` and `book` alike, and the two outcomes of
//! one market are one book read from its two sides.
//!
//! An unknown ticker is a `404` on the market route but a `200` with two empty
//! lists on the orderbook route, so `instrument` is `NotListed` for it while
//! `quote` and `book` return empty ones: use `instrument` to confirm a listing.
//! A time Kalshi sends is RFC 3339 and is read into `UnixMillis`; one that does
//! not parse is a `Decode` error naming the field, never a zero.
//!
//! | Method | Route | Requests |
//! | --- | --- | --- |
//! | `instrument` | `GET /trade-api/v2/markets/{ticker}` | 1 |
//! | `quote` | `GET /trade-api/v2/markets/{ticker}/orderbook`, `depth=1` | 1 |
//! | `book` | `GET /trade-api/v2/markets/{ticker}/orderbook`, every level | 1 |
//!
//! Each request costs its route's tokens on the client's Read layer, 10 today.

use polyoxide_core::ApiError;
use polyoxide_venue::{
    Book, Class, ClassifiedError, Extensions, Instrument, Level, MarketData, MarketKey, NotListed,
    Quote, SizeUnit, UnixMillis,
};
use rust_decimal::Decimal;

use super::key::{self, Outcome, OutcomeId};
use super::types::{self, MarketStatus, OrderbookFp, PriceRange};
use super::{Predictions, PredictionsError};

/// The smallest size step and the smallest order, in contracts: the
/// granularity of the `FixedPointCount` the OpenAPI gives for orders.
/// Provisional until Kalshi's `Trading` probes them (`OBSERVED.md`).
const GRANULARITY: Decimal = Decimal::new(1, 2);

/// Kalshi's facts about a market, beside [`Instrument`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct InstrumentExt {
    /// The event the market belongs to.
    pub event_ticker: Option<String>,
    /// Where the market is in its life: active, closed, finalized and so on.
    pub status: Option<MarketStatus>,
    /// The price ranges the market trades in, each with its own step.
    pub price_ranges: Option<Vec<PriceRange>>,
    /// The name of the structure those ranges follow.
    pub price_level_structure: Option<String>,
    /// When trading closes.
    pub close_time: Option<UnixMillis>,
    /// When the market is expected to be determined, when Kalshi says.
    pub expected_expiration_time: Option<UnixMillis>,
    /// The latest time the market can be determined.
    pub latest_expiration_time: Option<UnixMillis>,
    /// What one contract pays on its outcome, in dollars.
    pub notional_value: Option<Decimal>,
    /// How the settlement value is bounded.
    pub settlement_bounds_type: Option<String>,
    /// The index the market settles against, when it has one.
    pub exchange_index: Option<u32>,
}

/// A time Kalshi sent as RFC 3339, in milliseconds since the Unix epoch.
///
/// A time that does not parse, or is before 1970, is a `Decode` error naming
/// `field`.
fn millis(field: &str, text: &str) -> Result<UnixMillis, ClassifiedError> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text).map_err(|err| {
        ClassifiedError::new(
            Class::Decode,
            format!("{field} {text:?} is not an RFC 3339 time: {err}"),
        )
    })?;
    u64::try_from(parsed.timestamp_millis())
        .map(UnixMillis)
        .map_err(|_| {
            ClassifiedError::new(
                Class::Decode,
                format!("{field} {text:?} is before 1970"),
            )
        })
}

/// Whether the venue answered `404`, in either of the error's two shapes.
fn is_not_found(err: &PredictionsError) -> bool {
    match err {
        PredictionsError::Venue(venue) => venue.status == 404,
        PredictionsError::Api(ApiError::Response(response)) => response.status.as_u16() == 404,
        _ => false,
    }
}

/// An outcome's bids and asks, each best first (D9): its own bids from the
/// highest, and the other outcome's bids at `1 - price` from the lowest.
fn sides(book: &OrderbookFp, outcome: Outcome) -> (Vec<Level>, Vec<Level>) {
    let (own, other) = match outcome {
        Outcome::Yes => (&book.yes_dollars, &book.no_dollars),
        Outcome::No => (&book.no_dollars, &book.yes_dollars),
    };
    let mut bids: Vec<Level> = own
        .iter()
        .map(|level| Level::new(level.price, level.count))
        .collect();
    let mut asks: Vec<Level> = other
        .iter()
        .map(|level| Level::new(Decimal::ONE - level.price, level.count))
        .collect();
    bids.sort_by(|a, b| b.price.cmp(&a.price));
    asks.sort_by(|a, b| a.price.cmp(&b.price));
    (bids, asks)
}

/// The instrument a market's row describes, for one of its two outcomes.
fn instrument(
    key: &MarketKey,
    id: &OutcomeId,
    market: &types::Market,
) -> Result<Instrument, ClassifiedError> {
    let subtitle = match id.outcome {
        Outcome::Yes => &market.yes_sub_title,
        Outcome::No => &market.no_sub_title,
    };
    let name = if subtitle.trim().is_empty() {
        format!("{} {}", id.ticker, id.outcome.as_str())
    } else {
        subtitle.clone()
    };
    let mut instrument = Instrument::new(key.clone(), name, SizeUnit::Shares);
    instrument.quote_asset = Some("USD".to_owned());
    // A fixed step exists only when the market has one price range.
    instrument.tick_size = match market.price_ranges.as_slice() {
        [only] => Some(only.step),
        _ => None,
    };
    instrument.min_size = Some(GRANULARITY);
    instrument.size_increment = Some(GRANULARITY);
    instrument.tradable = Some(market.status == MarketStatus::Active);
    instrument.extensions = Extensions::of(InstrumentExt {
        event_ticker: Some(market.event_ticker.clone()),
        status: Some(market.status.clone()),
        price_ranges: Some(market.price_ranges.clone()),
        price_level_structure: Some(market.price_level_structure.clone()),
        close_time: Some(millis("close_time", &market.close_time)?),
        expected_expiration_time: market
            .expected_expiration_time
            .as_deref()
            .map(|text| millis("expected_expiration_time", text))
            .transpose()?,
        latest_expiration_time: Some(millis(
            "latest_expiration_time",
            &market.latest_expiration_time,
        )?),
        notional_value: Some(market.notional_value_dollars),
        settlement_bounds_type: Some(market.settlement_bounds_type.clone()),
        exchange_index: market.exchange_index,
    });
    Ok(instrument)
}

impl Predictions {
    /// The outcome a key names and its market's row; `NotListed` when Kalshi
    /// answers `404` for the ticker (D11).
    async fn listed(&self, key: &MarketKey) -> Result<(OutcomeId, types::Market), ClassifiedError> {
        let id = key::outcome(key)?;
        match self.markets().get(&id.ticker).send().await {
            Ok(response) => Ok((id, response.market)),
            Err(err) if is_not_found(&err) => Err(NotListed::new(key.clone()).into()),
            Err(err) => Err(err.into()),
        }
    }
}

impl MarketData for Predictions {
    async fn instrument(&self, key: &MarketKey) -> Result<Instrument, ClassifiedError> {
        let (id, market) = self.listed(key).await?;
        instrument(key, &id, &market)
    }

    async fn quote(&self, key: &MarketKey) -> Result<Quote, ClassifiedError> {
        let id = key::outcome(key)?;
        let response = self.markets().orderbook(&id.ticker).depth(1).send().await?;
        let (bids, asks) = sides(&response.orderbook_fp, id.outcome);
        let mut quote = Quote::new(key.clone());
        quote.bid = bids.first().copied();
        quote.ask = asks.first().copied();
        Ok(quote)
    }

    async fn book(&self, key: &MarketKey) -> Result<Book, ClassifiedError> {
        let id = key::outcome(key)?;
        let response = self.markets().orderbook(&id.ticker).send().await?;
        let (bids, asks) = sides(&response.orderbook_fp, id.outcome);
        let mut book = Book::new(key.clone());
        book.bids = bids;
        book.asks = asks;
        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book(json: &str) -> OrderbookFp {
        serde_json::from_str(json).unwrap()
    }

    fn lv(price: &str, size: &str) -> Level {
        Level::new(
            price.parse::<Decimal>().unwrap(),
            size.parse::<Decimal>().unwrap(),
        )
    }

    #[test]
    fn the_other_outcomes_bids_are_asks_at_one_minus_their_price() {
        let book = book(
            r#"{"yes_dollars":[["0.4000","10.00"],["0.4500","5.00"]],"no_dollars":[["0.5000","7.00"],["0.5200","3.00"]]}"#,
        );
        let (bids, asks) = sides(&book, Outcome::Yes);
        assert_eq!(bids, [lv("0.45", "5"), lv("0.40", "10")]);
        assert_eq!(asks, [lv("0.48", "3"), lv("0.50", "7")]);
        let (bids, asks) = sides(&book, Outcome::No);
        assert_eq!(bids, [lv("0.52", "3"), lv("0.50", "7")]);
        assert_eq!(asks, [lv("0.55", "5"), lv("0.60", "10")]);
    }

    #[test]
    fn a_missing_side_is_an_empty_list_and_the_complement_is_exact() {
        let book = book(r#"{"yes_dollars":[["0.0001","1.00"]]}"#);
        let (bids, asks) = sides(&book, Outcome::Yes);
        assert_eq!(bids, [lv("0.0001", "1")]);
        assert!(asks.is_empty());
        let (bids, asks) = sides(&book, Outcome::No);
        assert!(bids.is_empty());
        assert_eq!(asks, [lv("0.9999", "1")]);
    }

    #[test]
    fn a_time_is_read_with_any_offset_and_before_1970_is_refused() {
        assert_eq!(
            millis("t", "2026-10-18T09:10:11.250Z").unwrap(),
            UnixMillis(1_792_314_611_250)
        );
        assert_eq!(
            millis("t", "2026-10-18T11:10:11.250+02:00").unwrap(),
            UnixMillis(1_792_314_611_250)
        );
        let err = millis("created_time", "yesterday").unwrap_err();
        assert_eq!(err.class, Class::Decode);
        assert!(err.to_string().contains("created_time"), "{err}");
        let err = millis("created_time", "1960-01-01T00:00:00Z").unwrap_err();
        assert_eq!(err.class, Class::Decode);
    }
}
```

- [ ] **Step 4: Run it to see it pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: `test result: ok. 12 passed; 0 failed`, among them `a_no_bid_is_a_yes_ask_at_one_minus_its_price`, `only_a_404_is_not_listed` and `one_classifier_reads_the_errors_the_traits_return_as_it_reads_the_native_ones`.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib venue`
Expected: `test result: ok. 3 passed; 0 failed`.

To see the conversion test bite, edit `sides` so the asks are not complemented (`level.price` for `Decimal::ONE - level.price`) and run `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions a_no_bid_is_a_yes_ask`: it fails on the asks `[0.50, 0.52]` against `[0.48, 0.50]`. Put the line back.

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: no warning.

- [ ] **Step 5: Commit.**

```bash
nix develop --command cargo fmt --all
git add -N polyoxide-kalshi/src/predictions/venue.rs polyoxide-kalshi/tests/venue_predictions.rs
git commit -m "feat(kalshi): MarketData on Predictions, one YES/NO conversion (Story 8.3)" -- polyoxide-kalshi/Cargo.toml Cargo.lock polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/venue.rs polyoxide-kalshi/tests/venue_predictions.rs
```

### Task 3.2: `Trades` on `Predictions`

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/venue.rs` (the imports, the module docs' table, and the code before the `tests` module)
- Test: `polyoxide-kalshi/tests/venue_predictions.rs`

- [ ] **Step 1: Write the failing tests.** The suite needs `TradeExt`, `DynTrades`, `Side`, `TradeId` and `Trades`. Edit the imports, then append the tests. The trade names go in a second `use` of `polyoxide_venue` after the first, in the order `cargo fmt` keeps.

Current (`polyoxide-kalshi/tests/venue_predictions.rs`):

```rust
    venue::InstrumentExt,
```

Replace with:

```rust
    venue::{InstrumentExt, TradeExt},
```

Current (`polyoxide-kalshi/tests/venue_predictions.rs`):

```rust
use polyoxide_venue::{
    Book, Class, ClassifiedError, DynMarketData, Instrument, KeySpace, Level, MarketData, NotListed,
    ProductId, SizeUnit, UnixMillis, VenueId,
};
use rust_decimal::Decimal;
use serde_json::{json, Value};
```

Replace with:

```rust
use polyoxide_venue::{
    Book, Class, ClassifiedError, DynMarketData, DynTrades, Instrument, KeySpace, Level, MarketData,
    NotListed, ProductId, SizeUnit, UnixMillis, VenueId,
};
use polyoxide_venue::{Side, TradeId, Trades};
use rust_decimal::Decimal;
use serde_json::{json, Value};
```

Append to `polyoxide-kalshi/tests/venue_predictions.rs`. The first test reads Story 2's `trades.json`, which Kalshi serves newest first, and holds every row to the native client's values in reverse. The second serves four rows with literal prices, times and sides: a row Kalshi serves with a taker side this client does not know is `None`, never a guess. The route is matched on both query pairs the trait sends, `ticker` and `limit=1000`, in either order.

```rust
/// A trades route: the ticker and the 1000-row limit, in either order.
fn trades_query(ticker: &Ticker) -> Matcher {
    Matcher::AllOf(vec![
        Matcher::UrlEncoded("ticker".to_owned(), ticker.to_string()),
        Matcher::UrlEncoded("limit".to_owned(), "1000".to_owned()),
    ])
}

/// A copy of the fixture's first trade with the fields a test sets.
fn trade_row(
    template: &Value,
    id: &str,
    time: &str,
    yes: &str,
    no: &str,
    taker: &str,
    block: bool,
) -> Value {
    let mut row = template.clone();
    row["trade_id"] = json!(id);
    row["created_time"] = json!(time);
    row["yes_price_dollars"] = json!(yes);
    row["no_price_dollars"] = json!(no);
    row["count_fp"] = json!("5.00");
    row["taker_outcome_side"] = json!(taker);
    row["is_block_trade"] = json!(block);
    row
}

/// `trades` on a mock server serving `body`, for the test ticker.
async fn trades_of(
    body: &Value,
    outcome: Outcome,
) -> Result<Vec<polyoxide_venue::Trade>, ClassifiedError> {
    let ticker = test_ticker();
    let mut server = start().await;
    let _trades = route(
        &mut server,
        "/trade-api/v2/markets/trades",
        trades_query(&ticker),
        &body.to_string(),
        1,
    )
    .await;
    let venue = DynTrades::new_arc(client(&server));
    venue.trades(&key::key(&ticker, outcome)).await
}

#[tokio::test]
async fn a_dyn_trades_returns_the_native_trades_oldest_first() {
    let body = fixture("trades");
    let ticker = ticker_of(&fixture_json("market"));
    let mut server = start().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/trades",
        trades_query(&ticker),
        &body,
        3,
    )
    .await;
    let client = client(&server);
    let native = client
        .markets()
        .trades()
        .ticker(&ticker)
        .limit(1000)
        .send()
        .await
        .unwrap();
    assert!(native.trades.len() > 1, "the captured page holds several trades");
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client);

    for outcome in [Outcome::Yes, Outcome::No] {
        let k = key::key(&ticker, outcome);
        let trades = venue.trades(&k).await.unwrap();
        assert_eq!(trades.len(), native.trades.len());
        // Kalshi serves newest first; the trait is oldest first.
        for (trade, row) in trades.iter().zip(native.trades.iter().rev()) {
            assert_eq!(trade.key, k);
            assert_eq!(
                trade.price,
                match outcome {
                    Outcome::Yes => row.yes_price_dollars,
                    Outcome::No => row.no_price_dollars,
                }
            );
            assert_eq!(trade.size, row.count_fp);
            assert_eq!(trade.id, Some(TradeId::new(row.trade_id.clone())));
            assert!(trade.taker_side.is_some(), "{row:?}");
            let ext = trade.extensions.get::<TradeExt>().unwrap();
            assert_eq!(ext.is_block_trade, Some(row.is_block_trade));
        }
        assert!(
            trades.windows(2).all(|pair| pair[0].time <= pair[1].time),
            "the trades are not oldest first"
        );
        // 2023-11-14 in milliseconds: a time read as seconds would be far below.
        assert!(trades[0].time > UnixMillis(1_700_000_000_000), "{:?}", trades[0].time);
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn a_trades_price_time_and_taker_side_follow_the_keys_outcome() {
    let template = fixture_json("trades")["trades"][0].clone();
    // Newest first, as Kalshi serves them.
    let mut body = fixture_json("trades");
    body["trades"] = json!([
        trade_row(&template, "t4", "2026-10-18T09:10:12Z", "0.6200", "0.3800", "maybe", false),
        trade_row(&template, "t3", "2026-10-18T09:10:11.250Z", "0.6000", "0.4000", "no", false),
        trade_row(&template, "t2", "2026-10-18T09:10:11Z", "0.5900", "0.4100", "yes", false),
        trade_row(&template, "t1", "2026-10-18T09:10:10Z", "0.5800", "0.4200", "yes", true),
    ]);

    let yes = trades_of(&body, Outcome::Yes).await.unwrap();
    assert_eq!(
        yes.iter().map(|t| t.id.clone().unwrap()).collect::<Vec<_>>(),
        ["t1", "t2", "t3", "t4"].map(TradeId::new)
    );
    assert_eq!(
        yes.iter().map(|t| t.time).collect::<Vec<_>>(),
        [
            UnixMillis(1_792_314_610_000),
            UnixMillis(1_792_314_611_000),
            UnixMillis(1_792_314_611_250),
            UnixMillis(1_792_314_612_000),
        ]
    );
    assert_eq!(
        yes.iter().map(|t| t.price).collect::<Vec<_>>(),
        ["0.58", "0.59", "0.60", "0.62"].map(|p| p.parse::<Decimal>().unwrap())
    );
    // The taker bought the outcome when its outcome side is the key's.
    assert_eq!(
        yes.iter().map(|t| t.taker_side).collect::<Vec<_>>(),
        [Some(Side::Buy), Some(Side::Buy), Some(Side::Sell), None]
    );
    assert!(yes.iter().all(|t| t.size == Decimal::new(5, 0)));
    assert_eq!(
        yes.iter()
            .map(|t| t.extensions.get::<TradeExt>().unwrap().is_block_trade)
            .collect::<Vec<_>>(),
        [Some(true), Some(false), Some(false), Some(false)]
    );

    let no = trades_of(&body, Outcome::No).await.unwrap();
    assert_eq!(
        no.iter().map(|t| t.price).collect::<Vec<_>>(),
        ["0.42", "0.41", "0.40", "0.38"].map(|p| p.parse::<Decimal>().unwrap())
    );
    assert_eq!(
        no.iter().map(|t| t.taker_side).collect::<Vec<_>>(),
        [Some(Side::Sell), Some(Side::Sell), Some(Side::Buy), None]
    );
}

#[tokio::test]
async fn a_trade_time_that_does_not_parse_is_a_decode_failure() {
    let template = fixture_json("trades")["trades"][0].clone();
    let mut body = fixture_json("trades");
    body["trades"] = json!([
        trade_row(&template, "t2", "2026-10-18T09:10:11Z", "0.5900", "0.4100", "yes", false),
        trade_row(&template, "t1", "last Tuesday", "0.5800", "0.4200", "yes", false),
    ]);
    let err = trades_of(&body, Outcome::Yes).await.unwrap_err();
    assert_eq!(err.class, Class::Decode);
    assert!(err.to_string().contains("created_time"), "{err}");
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_by_trades_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let err = venue.trades(&foreign_key()).await.unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    nothing.assert_async().await;
}
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: the build fails on the missing name (once it resolves, `DynTrades::new_arc(Predictions)` fails for want of a `Trades` impl):

```
error[E0432]: unresolved import `polyoxide_kalshi::predictions::venue::TradeExt`
```

- [ ] **Step 3: Implement.** `Trades` reads the market-wide `/markets/trades` page for the key's ticker, 1000 rows, and reverses it. A row's price is the key's outcome's; its size is `count_fp`; its time is `created_time`; its id is `trade_id`. The taker bought the key's outcome when `taker_outcome_side` is that outcome, and sold it when it is the other; a side this client does not know is `None`.

Current (`polyoxide-kalshi/src/predictions/venue.rs`):

```rust
//! | `book` | `GET /trade-api/v2/markets/{ticker}/orderbook`, every level | 1 |
```

Replace with:

```rust
//! | `book` | `GET /trade-api/v2/markets/{ticker}/orderbook`, every level | 1 |
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows | 1 |
```

Current (`polyoxide-kalshi/src/predictions/venue.rs`):

```rust
use rust_decimal::Decimal;

use super::key::{self, Outcome, OutcomeId};
use super::types::{self, MarketStatus, OrderbookFp, PriceRange};
```

Replace with:

```rust
use polyoxide_venue::{Side, Trade, TradeId, Trades};
use rust_decimal::Decimal;

use super::key::{self, Outcome, OutcomeId};
use super::types::{self, MarketStatus, OrderbookFp, OutcomeSide, PriceRange};
```

Current (`polyoxide-kalshi/src/predictions/venue.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
/// Kalshi's facts about a trade, beside [`Trade`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct TradeExt {
    /// Whether it was a block trade.
    pub is_block_trade: Option<bool>,
}

/// Whether the taker bought `own`, the outcome a key names: when its outcome
/// side is `own` it took that outcome, and when it is the other it sold it. A
/// side this client does not know is none.
fn taker_side(taker: &OutcomeSide, own: Outcome) -> Option<Side> {
    let taker = match taker {
        OutcomeSide::Yes => Outcome::Yes,
        OutcomeSide::No => Outcome::No,
        _ => return None,
    };
    Some(if taker == own { Side::Buy } else { Side::Sell })
}

/// The trade a row describes, priced for the key's outcome.
fn trade(key: &MarketKey, outcome: Outcome, row: &types::Trade) -> Result<Trade, ClassifiedError> {
    let price = match outcome {
        Outcome::Yes => row.yes_price_dollars,
        Outcome::No => row.no_price_dollars,
    };
    let time = millis("created_time", &row.created_time)?;
    let mut trade = Trade::new(key.clone(), price, row.count_fp, time);
    trade.id = Some(TradeId::new(row.trade_id.clone()));
    trade.taker_side = taker_side(&row.taker_outcome_side, outcome);
    trade.extensions = Extensions::of(TradeExt {
        is_block_trade: Some(row.is_block_trade),
    });
    Ok(trade)
}

impl Trades for Predictions {
    /// The last 1000 trades of the market, oldest first. Kalshi serves them
    /// newest first, with a cursor this method does not follow.
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let id = key::outcome(key)?;
        let page = self
            .markets()
            .trades()
            .ticker(&id.ticker)
            .limit(1000)
            .send()
            .await?;
        page.trades
            .iter()
            .rev()
            .map(|row| trade(key, id.outcome, row))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
```

- [ ] **Step 4: Run it to see it pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: `test result: ok. 16 passed; 0 failed`, the four new ones among them.

To see the side rule bite, swap `Side::Buy` and `Side::Sell` in `taker_side` and run `... --test venue_predictions a_trades_price_time_and_taker_side`: it fails on the first trade's side. Put them back.

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`
Expected: no warning.

- [ ] **Step 5: Commit.**

```bash
nix develop --command cargo fmt --all
git commit -m "feat(kalshi): Trades on Predictions (Story 8.3)" -- polyoxide-kalshi/src/predictions/venue.rs polyoxide-kalshi/tests/venue_predictions.rs
```

### Task 3.3: `EventGroups` on `Predictions`

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/venue.rs` (the imports, the module docs' table, and the code before the `tests` module)
- Test: `polyoxide-kalshi/tests/venue_predictions.rs`

- [ ] **Step 1: Write the failing tests.** The suite gains `EventGroupExt` and `EventGroups`.

Current (`polyoxide-kalshi/tests/venue_predictions.rs`):

```rust
    venue::{InstrumentExt, TradeExt},
```

Replace with:

```rust
    venue::{EventGroupExt, InstrumentExt, TradeExt},
```

Current (`polyoxide-kalshi/tests/venue_predictions.rs`):

```rust
use polyoxide_venue::{Side, TradeId, Trades};
```

Replace with:

```rust
use polyoxide_venue::{DynEventGroups, EventGroups};
use polyoxide_venue::{Side, TradeId, Trades};
```

Append to `polyoxide-kalshi/tests/venue_predictions.rs`. The first test serves the captured market and the captured nested event and holds the group to the native decode: two outcomes per nested market, `Yes` then `No`, in the order Kalshi sends the markets. A group of no nested markets would pass an empty comparison, so the test asserts the fixture nests some. The unlisted test serves the events route with `expect(0)`: a key whose market is a 404 never reaches it.

```rust
/// A `{"event": ..}` body with one nested market's ticker replaced.
fn event_with_ticker(ticker: &str) -> Value {
    let mut body = fixture_json("event");
    body["event"]["markets"][0]["ticker"] = json!(ticker);
    body
}

#[tokio::test]
async fn a_dyn_event_groups_returns_both_outcomes_of_each_nested_market() {
    let market_body = fixture_json("market");
    let ticker = ticker_of(&market_body);
    let event_ticker = market_body["market"]["event_ticker"].as_str().unwrap().to_owned();
    let event_path = format!("/trade-api/v2/events/{event_ticker}");
    let nested = Matcher::UrlEncoded("with_nested_markets".to_owned(), "true".to_owned());

    let mut server = start().await;
    let market = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &market_body.to_string(),
        2,
    )
    .await;
    let event = route(&mut server, &event_path, nested, &fixture("event"), 3).await;
    let client = client(&server);
    let native = client
        .events()
        .get(&event_ticker)
        .with_nested_markets(true)
        .send()
        .await
        .unwrap()
        .event;
    assert!(!native.markets.is_empty(), "the captured event nests markets");
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client);

    for outcome in [Outcome::Yes, Outcome::No] {
        let group = venue
            .event_group(&key::key(&ticker, outcome))
            .await
            .unwrap();
        assert_eq!(group.id, native.event_ticker);
        assert_eq!(group.title.as_deref(), Some(native.title.as_str()));
        assert_eq!(group.outcomes.len(), 2 * native.markets.len());
        for (pair, nested) in group.outcomes.chunks(2).zip(&native.markets) {
            let nested_ticker = Ticker::new(&nested.ticker).unwrap();
            assert_eq!(pair[0].key, key::key(&nested_ticker, Outcome::Yes));
            assert_eq!(pair[0].label, "Yes");
            assert_eq!(pair[1].key, key::key(&nested_ticker, Outcome::No));
            assert_eq!(pair[1].label, "No");
            for each in pair {
                assert_eq!(each.market, nested.ticker);
                assert_eq!(each.market_title.as_deref(), Some(nested.yes_sub_title.as_str()));
            }
        }
        let ext = group.extensions.get::<EventGroupExt>().unwrap();
        assert_eq!(ext.series_ticker.as_deref(), Some(native.series_ticker.as_str()));
        assert_eq!(ext.sub_title.as_deref(), Some(native.sub_title.as_str()));
        assert_eq!(ext.mutually_exclusive, Some(native.mutually_exclusive));
        assert_eq!(
            ext.collateral_return_type.as_deref(),
            Some(native.collateral_return_type.as_str())
        );
    }
    market.assert_async().await;
    event.assert_async().await;
}

#[tokio::test]
async fn an_unlisted_key_has_no_event_group_and_asks_for_no_event() {
    let ticker = test_ticker();
    let mut server = start().await;
    let _market = server
        .mock("GET", market_path(&ticker).as_str())
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .create_async()
        .await;
    let events = server
        .mock("GET", Matcher::Regex("^/trade-api/v2/events/".to_owned()))
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));

    let k = key::key(&ticker, Outcome::No);
    let err = venue.event_group(&k).await.unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, k);
    events.assert_async().await;
}

#[tokio::test]
async fn a_nested_market_ticker_no_key_can_carry_is_a_decode_failure() {
    let market_body = fixture_json("market");
    let ticker = ticker_of(&market_body);
    let event_ticker = market_body["market"]["event_ticker"].as_str().unwrap().to_owned();
    let mut server = start().await;
    let _market = route(
        &mut server,
        &market_path(&ticker),
        Matcher::Missing,
        &market_body.to_string(),
        1,
    )
    .await;
    let _event = route(
        &mut server,
        &format!("/trade-api/v2/events/{event_ticker}"),
        Matcher::Any,
        &event_with_ticker("not a ticker!").to_string(),
        1,
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));

    let err = venue
        .event_group(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap_err();
    assert_eq!(err.class, Class::Decode);
}

#[tokio::test]
async fn a_key_of_another_venue_is_refused_by_event_group_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let err = venue.event_group(&foreign_key()).await.unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    nothing.assert_async().await;
}
```

- [ ] **Step 2: Run it to see it fail.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: the build fails on the missing name (once it resolves, `DynEventGroups::new_arc(Predictions)` fails for want of an `EventGroups` impl):

```
error[E0432]: unresolved import `polyoxide_kalshi::predictions::venue::EventGroupExt`
```

- [ ] **Step 3: Implement.** `event_group` is two requests: the key's market, for its event ticker (an unlisted key stops there as `NotListed`), then that event with its markets nested. Each nested market gives two outcomes, `:yes` labelled `Yes` and `:no` labelled `No`, both titled with the market's `yes_sub_title`, in the order Kalshi sends the markets. A nested market whose ticker no key can carry is a response this client cannot read: a `Decode` error, not the `InvalidRequest` of a caller's bad ticker.

Current (`polyoxide-kalshi/src/predictions/venue.rs`):

```rust
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows | 1 |
```

Replace with:

```rust
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows | 1 |
//! | `event_group` | `GET /trade-api/v2/markets/{ticker}`, then `GET /trade-api/v2/events/{event}` with `with_nested_markets=true` | 2 |
```

Current (`polyoxide-kalshi/src/predictions/venue.rs`):

```rust
use polyoxide_venue::{Side, Trade, TradeId, Trades};
use rust_decimal::Decimal;

use super::key::{self, Outcome, OutcomeId};
```

Replace with:

```rust
use polyoxide_venue::{EventGroup, EventGroups, EventOutcome};
use polyoxide_venue::{Side, Trade, TradeId, Trades};
use rust_decimal::Decimal;

use super::key::{self, Outcome, OutcomeId, Ticker};
```

Current (`polyoxide-kalshi/src/predictions/venue.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
/// Kalshi's facts about an event, beside [`EventGroup`]'s fields.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct EventGroupExt {
    /// The series the event belongs to.
    pub series_ticker: Option<String>,
    /// The event's subtitle.
    pub sub_title: Option<String>,
    /// Whether exactly one of the event's markets can resolve YES.
    pub mutually_exclusive: Option<bool>,
    /// What a contract returns as collateral when the market is settled.
    pub collateral_return_type: Option<String>,
}

impl EventGroups for Predictions {
    /// The event of the key's market, with both outcomes of each of its
    /// markets. The event is read with its markets nested, so a settled market
    /// Kalshi leaves out of an event is left out here.
    async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
        let (_, market) = self.listed(key).await?;
        let response = self
            .events()
            .get(&market.event_ticker)
            .with_nested_markets(true)
            .send()
            .await?;
        let event = response.event;
        let mut group = EventGroup::new(event.event_ticker.clone());
        group.title = Some(event.title.clone());
        for nested in &event.markets {
            // A ticker no key can carry is a response this client cannot read.
            let ticker = Ticker::new(&nested.ticker)
                .map_err(|err| ClassifiedError::new(Class::Decode, err))?;
            for (outcome, label) in [(Outcome::Yes, "Yes"), (Outcome::No, "No")] {
                let mut each =
                    EventOutcome::new(key::key(&ticker, outcome), nested.ticker.clone(), label);
                each.market_title = Some(nested.yes_sub_title.clone());
                group.outcomes.push(each);
            }
        }
        group.extensions = Extensions::of(EventGroupExt {
            series_ticker: Some(event.series_ticker.clone()),
            sub_title: Some(event.sub_title.clone()),
            mutually_exclusive: Some(event.mutually_exclusive),
            collateral_return_type: Some(event.collateral_return_type.clone()),
        });
        Ok(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
```

- [ ] **Step 4: Run it to see it pass.**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test venue_predictions`
Expected: `test result: ok. 20 passed; 0 failed`, the four new ones among them.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib venue`
Expected: `test result: ok. 3 passed; 0 failed`.

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: no warning.

- [ ] **Step 5: Commit.**

```bash
nix develop --command cargo fmt --all
git commit -m "feat(kalshi): EventGroups on Predictions (Story 8.3)" -- polyoxide-kalshi/src/predictions/venue.rs polyoxide-kalshi/tests/venue_predictions.rs
```

### Task 3.4: `live_predictions_venue`, and the registry regions

**Files:**
- Create: `polyoxide-kalshi/tests/live_predictions_venue.rs`
- Modify: `polyoxide-kalshi/Cargo.toml` (the line above `[package.metadata.polyoxide]`, and the line `[package.metadata.docs.rs]`)
- Modify: `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md` (regenerated by `scripts/gen_registry.py --write`, never by hand)
- Test: `polyoxide-kalshi/tests/live_predictions_venue.rs`

- [ ] **Step 1: Write the live target.** It runs against production, unsigned, with no secrets (D18), and every test is `#[ignore]`d. It picks its market with Story 2's `common::two_sided_market`, which already confirmed a two-sided book on the orderbook route the traits read, and is an `environmental(..)` skip when none qualifies. The books of the two outcomes and their quotes are read more than once and compared only when the YES side did not move between two reads, since a live book moves between requests; five reads that all moved are `transient`. The checks assert the conversion's rule on the live host: every converted book is not crossed, which also catches Kalshi serving the NO side in YES pricing on REST, as the socket's `use_yes_price` migration does there (D9). No test uses `.unwrap()` or `.expect(`, and no assertion message names an error or a status.

Create `polyoxide-kalshi/tests/live_predictions_venue.rs` with:

```rust
//! Kalshi's market-data traits against production, unsigned: no credentials
//! and no secrets. Every test is `#[ignore]`d. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions \
//!     --test live_predictions_venue -- --ignored
//! ```
//!
//! The market is chosen by precondition (open, with a two-sided book on the
//! orderbook route the assertions read) by `common::two_sided_market`, so a
//! quiet night is an environmental skip. The conversion's rule is asserted on
//! the live host: no converted book or quote is crossed, and the two outcomes
//! of one market are one book read from both sides. A book that moved between
//! reads is read again, up to five times.

#[allow(dead_code)]
mod common;

use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    Predictions,
};
use polyoxide_test_support::{environmental, transient, ClassifiedResultExt, ResultExt};
use polyoxide_venue::{Book, EventGroups, Level, MarketData, MarketKey, Quote, SizeUnit, Trades};
use rust_decimal::Decimal;

/// How many times a read that moved is tried again.
const READS: usize = 5;

fn client() -> Predictions {
    Predictions::new().or_fail("build the predictions client")
}

/// The keys of both outcomes of the chosen market.
async fn chosen(client: &Predictions) -> (MarketKey, MarketKey) {
    let market = common::two_sided_market(client).await;
    let ticker = Ticker::new(&market.ticker).or_fail("name the chosen market");
    (
        key::key(&ticker, Outcome::Yes),
        key::key(&ticker, Outcome::No),
    )
}

/// The prices a side of one outcome's book is, as the other outcome's asks.
fn flipped(levels: &[Level]) -> Vec<Level> {
    levels
        .iter()
        .map(|level| Level::new(Decimal::ONE - level.price, level.size))
        .collect()
}

/// Both sides of a book, with the best bid below the best ask.
fn assert_uncrossed(key: &MarketKey, bids: &[Level], asks: &[Level]) {
    let (Some(bid), Some(ask)) = (bids.first(), asks.first()) else {
        environmental(&format!("{key}: a side of the book emptied between reads"));
    };
    assert!(
        bid.price < ask.price,
        "{key}: best bid {bid:?} is not below best ask {ask:?}"
    );
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_the_chosen_markets_outcomes_are_tradable_shares() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    for each in [yes, no] {
        let instrument = client.instrument(&each).await.or_fail("instrument");
        assert_eq!(instrument.key, each);
        assert_eq!(instrument.size_unit, SizeUnit::Shares);
        assert_eq!(instrument.tradable, Some(true), "{each} is not tradable");
    }
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_both_outcomes_books_are_uncrossed_and_mirror_each_other() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    for _ in 0..READS {
        let first: Book = client.book(&yes).await.or_fail("YES book");
        let other: Book = client.book(&no).await.or_fail("NO book");
        let again: Book = client.book(&yes).await.or_fail("YES book, again");
        if first.bids != again.bids || first.asks != again.asks {
            continue;
        }
        assert_uncrossed(&yes, &first.bids, &first.asks);
        assert_uncrossed(&no, &other.bids, &other.asks);
        assert_eq!(first.bids, flipped(&other.asks), "{yes}: bids against {no}'s asks");
        assert_eq!(first.asks, flipped(&other.bids), "{yes}: asks against {no}'s bids");
        return;
    }
    transient("the YES book moved in each of five reads");
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_both_outcomes_quotes_are_uncrossed_and_mirror_each_other() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    for _ in 0..READS {
        let first: Quote = client.quote(&yes).await.or_fail("YES quote");
        let other: Quote = client.quote(&no).await.or_fail("NO quote");
        let again: Quote = client.quote(&yes).await.or_fail("YES quote, again");
        if first.bid != again.bid || first.ask != again.ask {
            continue;
        }
        let side = |level: Option<Level>| level.into_iter().collect::<Vec<Level>>();
        assert_uncrossed(&yes, &side(first.bid), &side(first.ask));
        assert_uncrossed(&no, &side(other.bid), &side(other.ask));
        assert_eq!(side(first.bid), flipped(&side(other.ask)), "{yes}: bid against {no}'s ask");
        assert_eq!(side(first.ask), flipped(&side(other.bid)), "{yes}: ask against {no}'s bid");
        return;
    }
    transient("the YES quote moved in each of five reads");
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_trades_are_oldest_first() {
    let client = client();
    let (yes, _) = chosen(&client).await;
    let trades = client.trades(&yes).await.or_fail("trades");
    if trades.len() < 2 {
        environmental(&format!(
            "the chosen market has {} trades, too few to check their order",
            trades.len()
        ));
    }
    assert!(
        trades.windows(2).all(|pair| pair[0].time <= pair[1].time),
        "{yes}: the trades are not oldest first"
    );
    assert!(trades.iter().all(|trade| trade.id.is_some()), "{yes}: a trade has no id");
}

#[tokio::test]
#[ignore = "hits the live host"]
async fn live_the_event_group_holds_both_outcomes_of_the_market() {
    let client = client();
    let (yes, no) = chosen(&client).await;
    let group = client.event_group(&yes).await.or_fail("event group");
    for each in [yes, no] {
        assert!(
            group.outcomes.iter().any(|outcome| outcome.key == each),
            "the event {} does not hold {each}",
            group.id
        );
    }
}
```

Declare the target and its live entry. The `[[test]]` goes directly above the registration comment line, and the live table directly above `[package.metadata.docs.rs]`, as Story 1's anchors fix. The target's `required-features` equals its entry's `features`.

Current (`polyoxide-kalshi/Cargo.toml`):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "live_predictions_venue"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Current (`polyoxide-kalshi/Cargo.toml`):

```toml
[package.metadata.docs.rs]
```

Replace with:

```toml
[package.metadata.polyoxide.live.live_predictions_venue]
suite = "predictions"
timeout = 15
features = ["predictions"]
secrets = []

[package.metadata.docs.rs]
```

- [ ] **Step 2: Run the registry check to see it fail.**

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: it fails, naming the generated regions that differ from what the metadata now produces (the nightly job list in `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md` and `CLAUDE.md`), since the new live entry has no job in them yet.

- [ ] **Step 3: Regenerate the regions.** The generator writes them; nothing here is edited by hand.

Run: `nix develop --command python3 scripts/gen_registry.py --write`
Expected: it rewrites the regions that list live targets and nothing else. `git diff --stat` shows `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md` and `CLAUDE.md`, each with the new `live_predictions_venue` entry beside Story 2's `live_predictions`, and no change under `docs/ARCHITECTURE.md`.

- [ ] **Step 4: Run it to see it pass.**

Run: `nix develop --command python3 scripts/gen_registry.py --check`
Expected: it passes.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored`
Expected: `test result: ok. 5 passed; 0 failed`. It reads the production host, so a failure tagged `polyoxide-class=environmental` or `transient` (no market with a two-sided book that minute, or a book that moved in each of five reads) is the host and not the code: run it again before looking further.

Run: `nix develop --command python3 scripts/live_unwraps.py && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
Expected: both pass, the first with no entry for the new file (its count is zero) and the second printing no path.

- [ ] **Step 5: Commit.**

```bash
nix develop --command cargo fmt --all
git add -N polyoxide-kalshi/tests/live_predictions_venue.rs
git commit -m "test(kalshi): live_predictions_venue, the traits against production (Story 8.3)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/tests/live_predictions_venue.rs .github/workflows/nightly-behavioral.yml SELF-HEALING.md CLAUDE.md Cargo.lock
```

## Story 4: Signed requests and the first-use confirmation

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/auth.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/api/limits.rs`, `polyoxide-kalshi/src/predictions/types/mod.rs`, `polyoxide-kalshi/src/predictions/types/limits.rs`, `scripts/capture_kalshi_account.py`, `polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json`, `polyoxide-kalshi/tests/fixtures/predictions/signing/PROVENANCE.md`, `polyoxide-kalshi/tests/fixtures/predictions/account/limits.json`, `polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md`, `polyoxide-kalshi/tests/signing_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions_signed.rs`, `polyoxide-kalshi/tests/live_predictions_account.rs`, `docs/specs/kalshi/OBSERVED.md`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- **Interfaces:** exposes everything under "Story 4 → Story 6" (`Credentials`, `SignedHeaders`, the header consts, `CredentialsError`, `PredictionsBuilder::credentials`, `AccountApi::limits`, the limits types, the vectors and `signed_headers`); uses Story 1's `Inner`, `before_send` stub, `RouteCosts`, `PredictionsThrottle::bucket`, `routes`, `template_matches`, `Routed::has_key` and module blocks, and Story 2's `AccountApi`, `EndpointCosts`, `types/mod.rs` and `api/mod.rs` blocks
- **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py tests/test_classify_coverage.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
- **Review:** deep (signing and secrets)

Adds signing and the first-use confirmation. `Credentials` reads an Ed25519 or an RSA key from PEM over `ring` (spec D2): PKCS#1 under the label `RSA PRIVATE KEY`, and PKCS#8 under `PRIVATE KEY`, tried as Ed25519 and then as RSA, with the label deciding before the body is read. The key lives in a `polyoxide_venue::Secret`, and no refusal quotes the body, the base64 or a byte of the key. The authenticator signs `timestamp_ms + METHOD + path` afresh on every attempt, so a retried request carries a new timestamp, and `PredictionsBuilder::credentials` installs it: a keyed client signs every route but the ping, which core's `HttpClient::health` sends without a venue authenticator. `AccountApi::limits` reads `/account/limits` and refuses, before sending anything, when the client has no key. Task 4.4 replaces Story 1's `before_send` stub with the whole confirmation (spec D5): one read of `/account/endpoint_costs` and, with a key, one of `/account/limits` that resizes the Read `CapacityBucket` in place. It always ends with the bucket confirmed, and a failed read or a zero capacity or refill logs one WARN and keeps the compiled costs or Basic's sizing. No landmine marker of the profile appears in this story's code, but the dispatch named its two shapes (a bucket resized from an account's own numbers, and signing with key material), so the supervisor should dispatch its developer with `effort: xhigh`. The signing vectors come from throwaway keys that `scripts/capture_kalshi_account.py` makes, and the demo host's `/account/limits` comes from that script too, signed with the owner's demo key (`KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`). Without that key the story stops at Task 4.5's capture and asks the owner, and no fixture is ever written by hand.

### Task 4.1: The signing vectors and the account capture script

The script is Story 4's and Story 6's: its `signing` step writes the throwaway-key vectors, its `demo` step (run in Task 4.5) captures the demo host's `/account/limits`, and `signed_headers(method, path)` is the function Story 6's capture imports, so importing the script runs nothing: its steps run only under `if __name__ == "__main__":`. The vectors never hold an armour line: each private key is stored as its PEM's base64 lines, and the Rust tests armour it. Its `signing --check` is this task's test: it re-verifies the fixture on disk, so it fails before the fixture exists.

**Files:**
- Create: `scripts/capture_kalshi_account.py`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json` (written by the script)
- Create: `polyoxide-kalshi/tests/fixtures/predictions/signing/PROVENANCE.md` (written by the script)

- [ ] **Step 1: Write the script and its check**

Create `scripts/capture_kalshi_account.py`:

```python
#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["cryptography>=42", "certifi"]
# ///
"""Capture the account-side fixtures of polyoxide-kalshi's `predictions` module.

Run each step as a file with `uv run`, never with `python3 -I`:

    uv run scripts/capture_kalshi_account.py signing DIR
    uv run scripts/capture_kalshi_account.py signing --check DIR
    uv run scripts/capture_kalshi_account.py demo DIR

`signing` needs no account. It makes throwaway keys, an Ed25519 key and an RSA-2048 key, signs one
message with each, and writes DIR/vectors.json and DIR/PROVENANCE.md. The keys exist only for this
fixture, and each is stored as the base64 lines of its PEM with the armour lines left off, so the
file never holds an armour line and no push protection can refuse it; the Rust suite puts the
armour back before it reads a key. It refuses to run when DIR/vectors.json exists, so the keys the
Rust suite reads are never replaced by accident. `signing --check` makes no key and writes
nothing: it re-verifies the files on disk, and exits 1 naming what is wrong.

`demo` reads GET /trade-api/v2/account/limits from the demo host, signed with the owner's demo key,
and writes DIR/limits.json and DIR/PROVENANCE.md. It needs KALSHI_DEMO_API_KEY_ID and
KALSHI_DEMO_PRIVATE_KEY (the PEM text, with a literal backslash-n accepted for a line break), and it
stops without writing anything when either is absent. No fixture is ever written by hand.

`signed_headers(method, path)` is importable: another capture script that needs the demo host's
three KALSHI-ACCESS-* headers imports it from this file's directory. Importing this file runs
nothing: its steps run only under `if __name__ == "__main__":`.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import time
import urllib.parse
from pathlib import Path

import capture_common
from cryptography.exceptions import InvalidSignature, UnsupportedAlgorithm
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ed25519, padding, rsa

DEMO_BASE = "https://external-api.demo.kalshi.co"
LIMITS_PATH = "/trade-api/v2/account/limits"
# What every vector signs: a timestamp in milliseconds, the method, and the path.
MESSAGE = "1700000000000GET/trade-api/v2/portfolio/balance"
KEY_ID_VAR = "KALSHI_DEMO_API_KEY_ID"
PEM_VAR = "KALSHI_DEMO_PRIVATE_KEY"

ENCODING = serialization.Encoding
PRIVATE = serialization.PrivateFormat
PUBLIC = serialization.PublicFormat


def b64(data: bytes) -> str:
    return base64.b64encode(data).decode("ascii")


def pss() -> padding.PSS:
    """RSA-PSS as Kalshi asks for it: MGF1 over SHA-256 and a salt as long as the digest."""
    return padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=padding.PSS.DIGEST_LENGTH)


def sign(key, message: bytes) -> bytes:
    if isinstance(key, ed25519.Ed25519PrivateKey):
        return key.sign(message)
    if isinstance(key, rsa.RSAPrivateKey):
        return key.sign(message, pss(), hashes.SHA256())
    raise SystemExit(f"unsupported key type {type(key).__name__}: Kalshi keys are Ed25519 or RSA")


def body_of(key, fmt) -> str:
    """The base64 lines of `key`'s PEM, joined by newlines, with the armour lines left off."""
    pem = key.private_bytes(ENCODING.PEM, fmt, serialization.NoEncryption()).decode("ascii")
    return "\n".join(line for line in pem.splitlines() if not line.startswith("-----"))


def armour(label: str, body: str) -> bytes:
    """The PEM text around `body`, as the Rust suite builds it before it reads a key."""
    return f"-----BEGIN {label}-----\n{body}\n-----END {label}-----\n".encode("ascii")


def signed_headers(method: str, path: str) -> dict[str, str]:
    """The three KALSHI-ACCESS-* headers for `method path`, signed with the owner's demo key.

    The path is signed without its query. Stops the run, asking for the key, when the key is not
    in the environment.
    """
    key_id = os.environ.get(KEY_ID_VAR, "")
    pem = os.environ.get(PEM_VAR, "")
    if not key_id or not pem:
        raise SystemExit(
            f"{KEY_ID_VAR} and {PEM_VAR} are not both set. This capture needs the owner's demo key: "
            "the key id, and the PEM text (a literal backslash-n is read as a line break). "
            "Stop here and ask the owner; never write the fixture by hand."
        )
    try:
        key = serialization.load_pem_private_key(pem.replace("\\n", "\n").encode(), password=None)
    except (ValueError, TypeError, UnsupportedAlgorithm):
        raise SystemExit(f"{PEM_VAR} does not hold an unencrypted PEM private key") from None
    timestamp = str(int(time.time() * 1000))
    message = f"{timestamp}{method.upper()}{path.split('?')[0]}".encode()
    return {
        "KALSHI-ACCESS-KEY": key_id,
        "KALSHI-ACCESS-TIMESTAMP": timestamp,
        "KALSHI-ACCESS-SIGNATURE": b64(sign(key, message)),
    }


def signing_vectors() -> dict:
    """Throwaway keys, and one signature by each over MESSAGE."""
    message = MESSAGE.encode()
    ed_key = ed25519.Ed25519PrivateKey.generate()
    rsa_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    rsa_public = rsa_key.public_key().public_bytes(ENCODING.DER, PUBLIC.PKCS1)
    return {
        "message": MESSAGE,
        "ed25519": {
            "pkcs8_body": body_of(ed_key, PRIVATE.PKCS8),
            "public_key_hex": ed_key.public_key().public_bytes(ENCODING.Raw, PUBLIC.Raw).hex(),
            "signature_b64": b64(ed_key.sign(message)),
        },
        "rsa": {
            "pkcs1_body": body_of(rsa_key, PRIVATE.TraditionalOpenSSL),
            "pkcs8_body": body_of(rsa_key, PRIVATE.PKCS8),
            "public_key_pkcs1_der_b64": b64(rsa_public),
            "pss_signature_b64": b64(rsa_key.sign(message, pss(), hashes.SHA256())),
        },
    }


def check_vectors(vectors: dict) -> list[str]:
    """What is wrong with `vectors`, as sentences that quote no key material."""
    problems: list[str] = []
    if vectors.get("message") != MESSAGE:
        problems.append("message is not the one the Rust suite names")
    message = MESSAGE.encode()

    def verifies(public, signature_b64: str, *scheme) -> bool:
        try:
            public.verify(base64.b64decode(signature_b64), message, *scheme)
        except InvalidSignature:
            return False
        return True

    # The file never holds an armour line, so no push protection can refuse it.
    for section in ("ed25519", "rsa"):
        for name, value in vectors[section].items():
            if "-----" in value:
                problems.append(f"{section}.{name} holds an armour line")
    if problems:
        return problems

    ed = vectors["ed25519"]
    ed_key = serialization.load_pem_private_key(
        armour("PRIVATE KEY", ed["pkcs8_body"]), password=None
    )
    ed_public = ed25519.Ed25519PublicKey.from_public_bytes(bytes.fromhex(ed["public_key_hex"]))
    if ed_key.public_key().public_bytes(ENCODING.Raw, PUBLIC.Raw) != ed_public.public_bytes(
        ENCODING.Raw, PUBLIC.Raw
    ):
        problems.append("ed25519.public_key_hex is not the private key's public key")
    if not verifies(ed_public, ed["signature_b64"]):
        problems.append("ed25519.signature_b64 does not verify under ed25519.public_key_hex")

    rs = vectors["rsa"]
    # The label decides how a body is read: a PKCS#8 body under RSA PRIVATE KEY would not load.
    pkcs1 = serialization.load_pem_private_key(
        armour("RSA PRIVATE KEY", rs["pkcs1_body"]), password=None
    )
    pkcs8 = serialization.load_pem_private_key(
        armour("PRIVATE KEY", rs["pkcs8_body"]), password=None
    )
    public = pkcs1.public_key()
    if public.public_numbers() != pkcs8.public_key().public_numbers():
        problems.append("rsa.pkcs1_body and rsa.pkcs8_body are not one key")
    if public.public_bytes(ENCODING.DER, PUBLIC.PKCS1) != base64.b64decode(
        rs["public_key_pkcs1_der_b64"]
    ):
        problems.append("rsa.public_key_pkcs1_der_b64 is not the private key's public key")
    if not verifies(public, rs["pss_signature_b64"], pss(), hashes.SHA256()):
        problems.append("rsa.pss_signature_b64 does not verify under the public key")
    return problems


def generate(out: Path) -> None:
    target = out / "vectors.json"
    if target.exists():
        raise SystemExit(f"{target} exists. Delete it to make new keys; the Rust suite reads it.")
    vectors = signing_vectors()
    problems = check_vectors(vectors)
    if problems:
        raise SystemExit("the vectors just made are wrong: " + "; ".join(problems))
    capture_common.write_json(target, vectors)
    what = "throwaway Ed25519 and RSA-2048 keys, and one signature by each"
    rows = [["vectors.json", what, capture_common.stamp()]]
    capture_common.write_provenance(
        out,
        [
            "# Provenance",
            "",
            "`vectors.json` is made by `scripts/capture_kalshi_account.py signing`. Its keys are "
            "throwaway ones that exist only for this fixture: no Kalshi account has ever held them. "
            f"Every signature is over `{MESSAGE}`.",
            "",
            "The file never holds an armour line, so no push protection can refuse it. Each "
            "`*_body` is the base64 lines of a PEM, joined by newlines. The Rust suite puts the "
            "armour back before it reads a key, with the label `PRIVATE KEY` around a "
            "`pkcs8_body` and `RSA PRIVATE KEY` around the `pkcs1_body`.",
            "",
            *capture_common.provenance_table(["File", "What", "Generated"], rows),
            "",
            "Check it with `uv run scripts/capture_kalshi_account.py signing --check "
            "polyoxide-kalshi/tests/fixtures/predictions/signing`.",
        ],
    )
    print(f"wrote {target}")


def check(out: Path) -> None:
    target = out / "vectors.json"
    if not target.exists():
        raise SystemExit(f"{target} is missing: run the signing step without --check first")
    problems = check_vectors(json.loads(target.read_text()))
    if problems:
        print(f"{target}:")
        for problem in problems:
            print(f"  {problem}")
        raise SystemExit(1)
    print(f"signing vectors ok: {target}")


def demo(out: Path) -> None:
    def headers(method: str, url: str) -> dict[str, str]:
        return signed_headers(method, urllib.parse.urlparse(url).path)

    reply = capture_common.get(DEMO_BASE + LIMITS_PATH, headers=headers)
    body = capture_common.require_ok(reply, f"GET {LIMITS_PATH} on the demo host")
    capture_common.write_json(out / "limits.json", body)
    rows = [["limits.json", f"GET {LIMITS_PATH}", "the demo host", "yes", capture_common.stamp()]]
    capture_common.write_provenance(
        out,
        [
            "# Provenance",
            "",
            "`limits.json` is the body of `GET /trade-api/v2/account/limits` as the demo host "
            f"(`{DEMO_BASE}`) served it, signed with the owner's demo key ({KEY_ID_VAR} and "
            f"{PEM_VAR}). Only its indentation was changed.",
            "",
            *capture_common.provenance_table(["File", "Route", "Host", "Signed", "Captured"], rows),
            "",
            "Capture it again with `uv run scripts/capture_kalshi_account.py demo "
            "polyoxide-kalshi/tests/fixtures/predictions/account`.",
        ],
    )
    print(f"wrote {out / 'limits.json'}")


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    steps = parser.add_subparsers(dest="step", required=True)
    signing_step = steps.add_parser("signing", help="throwaway keys, and one signature by each")
    signing_step.add_argument("out", type=Path, help="the signing fixtures' directory")
    signing_step.add_argument("--check", action="store_true", help="re-verify the fixture on disk")
    demo_step = steps.add_parser("demo", help="the demo host's /account/limits, signed")
    demo_step.add_argument("out", type=Path, help="the account fixtures' directory")
    args = parser.parse_args(argv)
    if args.step == "demo":
        demo(args.out)
    elif args.check:
        check(args.out)
    else:
        generate(args.out)


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run the check to see it fail**

Run: `nix develop --command uv run scripts/capture_kalshi_account.py signing --check polyoxide-kalshi/tests/fixtures/predictions/signing`

Expected: exits 1 with `polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json is missing: run the signing step without --check first`.

- [ ] **Step 3: Make the vectors**

Run: `nix develop --command uv run scripts/capture_kalshi_account.py signing polyoxide-kalshi/tests/fixtures/predictions/signing`

Expected: prints `wrote polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json` and writes `vectors.json` and `PROVENANCE.md` beside it. `vectors.json` holds the keys `message`, `ed25519` (`pkcs8_body`, `public_key_hex`, `signature_b64`) and `rsa` (`pkcs1_body`, `pkcs8_body`, `public_key_pkcs1_der_b64`, `pss_signature_b64`), which is the shape the names contract gives Story 6. The keys are throwaway ones, and each `*_body` is a PEM's base64 lines between its armour lines, joined by `\n`, with no armour line in the file; a test armours it before it reads the key, and `signing --check` fails on a field that holds an armour line.

- [ ] **Step 4: Run the check to see it pass**

Run: `nix develop --command uv run scripts/capture_kalshi_account.py signing --check polyoxide-kalshi/tests/fixtures/predictions/signing`

Expected: prints `signing vectors ok: polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json` and exits 0.

Run: `nix develop --command uv run scripts/capture_kalshi_account.py signing polyoxide-kalshi/tests/fixtures/predictions/signing`

Expected: refuses with `... vectors.json exists. Delete it to make new keys; the Rust suite reads it.` and changes nothing.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/capture_kalshi_account.py polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json polyoxide-kalshi/tests/fixtures/predictions/signing/PROVENANCE.md
git commit -m "feat(kalshi): throwaway signing vectors and the account capture script (Epic 8, Story 4)" -- scripts/capture_kalshi_account.py polyoxide-kalshi/tests/fixtures/predictions/signing/vectors.json polyoxide-kalshi/tests/fixtures/predictions/signing/PROVENANCE.md
```

### Task 4.2: Credentials, the authenticator and `PredictionsBuilder::credentials`

`Credentials` is the key id and the parsed private key; `sign_request` makes the three headers for a request, and the authenticator calls it on every attempt. The doc-hidden `Credentials::sign_message`, which the names contract gives, signs given bytes and returns the padded `STANDARD` base64 signature: an integration suite cannot reach `sign_request`, and the Ed25519 vector needs a fixed message. The Ed25519 signature is checked against Python's, and an RSA-PSS signature both ways, so a wrong MGF1 or salt length fails.

**Files:**
- Create: `polyoxide-kalshi/src/predictions/auth.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs` (the module block)
- Modify: `polyoxide-kalshi/src/lib.rs` (the `Classify` assertion)
- Modify: `polyoxide-kalshi/Cargo.toml` (two `[[test]]` stanzas)
- Test: `polyoxide-kalshi/tests/signing_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions_signed.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-kalshi/tests/signing_predictions.rs`:

```rust
//! Signing: the vectors, both key types, the PEM forms Kalshi hands out, and what a refusal says.
//!
//! The keys in `tests/fixtures/predictions/signing/vectors.json` are throwaway ones, made by
//! `scripts/capture_kalshi_account.py signing`. No account has ever held them. The file holds each
//! key as the base64 lines of its PEM with no armour line, and `armour` puts the armour back.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use polyoxide_kalshi::predictions::{Credentials, CredentialsError};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use ring::signature::{UnparsedPublicKey, ED25519, RSA_PSS_2048_8192_SHA256};
use serde_json::Value;

/// The message the vectors sign: a timestamp in milliseconds, the method and the path.
const MESSAGE: &str = "1700000000000GET/trade-api/v2/portfolio/balance";

fn vectors() -> Value {
    fixtures!("predictions/signing").json("vectors")
}

/// A string field of the vectors.
fn field(vectors: &Value, key: &str, name: &str) -> String {
    vectors[key][name]
        .as_str()
        .unwrap_or_else(|| panic!("vectors.json has no {key}.{name}"))
        .to_owned()
}

/// The vectors' Ed25519 key as PEM.
fn ed25519_pem() -> String {
    armour("PRIVATE KEY", &field(&vectors(), "ed25519", "pkcs8_body"))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("the vectors hold valid hex"))
        .collect()
}

fn base64(text: &str) -> Vec<u8> {
    STANDARD
        .decode(text)
        .expect("the vectors hold valid base64")
}

fn rsa_public_key(vectors: &Value) -> Vec<u8> {
    base64(&field(vectors, "rsa", "public_key_pkcs1_der_b64"))
}

fn verifies_pss(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, public_key)
        .verify(message, signature)
        .is_ok()
}

/// A refusal is the caller's: `InvalidRequest`, which no retry fixes.
fn assert_invalid_request(error: &CredentialsError) {
    assert_eq!(error.class(), Class::InvalidRequest, "{error:?}");
    assert!(!error.is_retriable(), "{error:?}");
}

/// A PEM block around `body`: a vectors body, or a made-up one for a refusal.
fn armour(label: &str, body: &str) -> String {
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
}

#[test]
fn the_ed25519_signature_is_the_vectors() {
    let vectors = vectors();
    assert_eq!(
        vectors["message"], MESSAGE,
        "the vectors sign the message this suite names"
    );
    let credentials = Credentials::from_pem("key", &ed25519_pem()).unwrap();

    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors, "ed25519", "signature_b64"),
        "Ed25519 is deterministic: one key and message give one signature"
    );
}

#[test]
fn the_vectors_ed25519_signature_verifies_under_its_public_key() {
    let vectors = vectors();
    let public_key = hex(&field(&vectors, "ed25519", "public_key_hex"));
    let signature = base64(&field(&vectors, "ed25519", "signature_b64"));

    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(MESSAGE.as_bytes(), &signature)
        .expect("the vectors' signature verifies under the vectors' public key");
}

#[test]
fn ring_verifies_pythons_pss_signature_so_mgf1_and_the_salt_length_agree() {
    let vectors = vectors();
    let signature = base64(&field(&vectors, "rsa", "pss_signature_b64"));

    assert!(verifies_pss(
        &rsa_public_key(&vectors),
        MESSAGE.as_bytes(),
        &signature
    ));
}

#[test]
fn the_rsa_signature_verifies_for_both_pem_forms_and_only_for_its_message() {
    let vectors = vectors();
    let public_key = rsa_public_key(&vectors);

    let forms = [
        ("pkcs1_body", "RSA PRIVATE KEY"),
        ("pkcs8_body", "PRIVATE KEY"),
    ];
    for (form, label) in forms {
        let pem = armour(label, &field(&vectors, "rsa", form));
        let credentials = Credentials::from_pem("key", &pem).unwrap();
        let signature = base64(&credentials.sign_message(MESSAGE.as_bytes()).unwrap());

        assert!(
            verifies_pss(&public_key, MESSAGE.as_bytes(), &signature),
            "{form}"
        );
        assert!(
            !verifies_pss(&public_key, b"another message", &signature),
            "{form}"
        );
    }
}

#[test]
fn rsa_pss_is_salted_so_two_signatures_of_one_message_differ() {
    let vectors = vectors();
    let pem = armour("RSA PRIVATE KEY", &field(&vectors, "rsa", "pkcs1_body"));
    let credentials = Credentials::from_pem("key", &pem).unwrap();

    let first = credentials.sign_message(MESSAGE.as_bytes()).unwrap();
    let second = credentials.sign_message(MESSAGE.as_bytes()).unwrap();

    assert_ne!(first, second);
}

#[test]
fn a_key_on_one_line_with_literal_backslash_n_signs_as_the_pem_does() {
    let one_line = ed25519_pem().trim_end().replace('\n', "\\n");
    assert!(!one_line.contains('\n'), "the test's own premise");

    let credentials = Credentials::from_pem("key", &one_line).unwrap();

    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors(), "ed25519", "signature_b64")
    );
}

#[test]
fn windows_line_endings_and_blank_space_around_the_pem_parse() {
    let padded = format!("\n  {}\n\n", ed25519_pem().replace('\n', "\r\n"));

    let credentials = Credentials::from_pem("key", &padded).unwrap();

    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors(), "ed25519", "signature_b64")
    );
}

#[test]
fn an_ec_key_is_refused_by_its_label_alone() {
    let pem = armour("EC PRIVATE KEY", "c2VjcmV0LWVjLWJvZHk=");

    let error = Credentials::from_pem("key", &pem).unwrap_err();

    assert!(matches!(&error, CredentialsError::Label { label } if label == "EC PRIVATE KEY"));
    assert!(error.to_string().contains("EC PRIVATE KEY"), "{error}");
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn an_encrypted_key_is_refused_by_its_label() {
    let pem = armour("ENCRYPTED PRIVATE KEY", "c2VjcmV0LWVuY3J5cHRlZA==");

    let error = Credentials::from_pem("key", &pem).unwrap_err();

    assert!(
        matches!(&error, CredentialsError::Label { label } if label == "ENCRYPTED PRIVATE KEY")
    );
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn a_legacy_encrypted_pem_is_refused_before_its_body_is_read() {
    let pem = "-----BEGIN RSA PRIVATE KEY-----\nProc-Type: 4,ENCRYPTED\nDEK-Info: AES-128-CBC,00\n\nc2VjcmV0\n-----END RSA PRIVATE KEY-----\n";

    let error = Credentials::from_pem("key", pem).unwrap_err();

    assert!(matches!(error, CredentialsError::Pem { .. }), "{error:?}");
    assert!(error.to_string().contains("encrypted"), "{error}");
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn a_body_that_is_not_base64_is_a_pem_error_that_does_not_quote_it() {
    let pem = armour("PRIVATE KEY", "c2VjcmV0-body!!not base64");

    let error = Credentials::from_pem("key", &pem).unwrap_err();

    assert!(matches!(error, CredentialsError::Pem { .. }), "{error:?}");
    assert!(!error.to_string().contains("c2VjcmV0"), "{error}");
    assert_invalid_request(&error);
}

#[test]
fn a_body_that_is_base64_but_no_key_is_a_key_error_that_does_not_quote_it() {
    let body = STANDARD.encode(b"polyoxide-test: this is not a private key");

    for label in ["PRIVATE KEY", "RSA PRIVATE KEY"] {
        let error = Credentials::from_pem("key", &armour(label, &body)).unwrap_err();

        assert!(
            matches!(error, CredentialsError::Key { .. }),
            "{label}: {error:?}"
        );
        assert!(!error.to_string().contains(&body[..16]), "{label}: {error}");
        assert!(
            !error.to_string().contains("polyoxide-test"),
            "{label}: {error}"
        );
        assert_invalid_request(&error);
    }
}

#[test]
fn text_that_is_not_one_armoured_block_is_a_pem_error() {
    let texts = [
        "",
        "not a key",
        "-----BEGIN PRIVATE KEY-----\nAAAA\n",
        "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\nextra",
    ];

    for text in texts {
        let error = Credentials::from_pem("key", text).unwrap_err();

        assert!(
            matches!(error, CredentialsError::Pem { .. }),
            "{text:?}: {error:?}"
        );
        assert_invalid_request(&error);
    }
}

#[test]
fn debug_prints_the_key_id_and_no_key_material() {
    let pem = ed25519_pem();
    let body_line = pem.lines().nth(1).expect("a PEM has a body line");
    let credentials = Credentials::from_pem("my-key-id", &pem).unwrap();

    for printed in [format!("{credentials:?}"), format!("{credentials:#?}")] {
        assert!(printed.contains("my-key-id"), "{printed}");
        assert!(printed.contains("Secret([REDACTED])"), "{printed}");
        assert!(!printed.contains(body_line), "{printed}");
    }
}

#[test]
fn from_pem_file_reads_a_key_and_names_the_path_of_one_it_cannot() {
    let dir = std::env::temp_dir().join(format!("polyoxide-kalshi-signing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("key.pem");
    std::fs::write(&path, ed25519_pem()).unwrap();

    let credentials = Credentials::from_pem_file("file-key", path).unwrap();
    assert_eq!(credentials.key_id(), "file-key");
    assert_eq!(
        credentials.sign_message(MESSAGE.as_bytes()).unwrap(),
        field(&vectors(), "ed25519", "signature_b64")
    );

    let missing = dir.join("missing.pem");
    let error = Credentials::from_pem_file("file-key", &missing).unwrap_err();
    assert!(
        matches!(&error, CredentialsError::File { path, .. } if *path == missing.display().to_string()),
        "{error:?}"
    );
    assert_invalid_request(&error);

    std::fs::remove_dir_all(dir).unwrap();
}
```

Create `polyoxide-kalshi/tests/mock_predictions_signed.rs`:

```rust
//! Signed requests against a mock server: the three headers on every route but the ping, what
//! the signature covers, a fresh signature for every attempt, and an anonymous client's silence.
//!
//! Every test answers from one catch-all that records what it was sent. The request is what is
//! under test and the answer is not, so most routes answer `404`, and a test that only looks at
//! the request does not read the result.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::{ApiError, RetryConfig};
use polyoxide_kalshi::predictions::key::Ticker;
use polyoxide_kalshi::predictions::{Credentials, Predictions, PredictionsError};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use ring::signature::{UnparsedPublicKey, ED25519, RSA_PSS_2048_8192_SHA256};
use serde_json::Value;

const STATUS: &str = "/trade-api/v2/exchange/status";
const KEY_ID: &str = "test-key-id";

/// One request as the server saw it.
#[derive(Debug, Clone)]
struct Sent {
    path: String,
    query: Option<String>,
    key: Option<String>,
    timestamp: Option<String>,
    signature: Option<String>,
    /// How many of its headers start with `kalshi-`.
    kalshi_headers: usize,
}

impl Sent {
    fn of(request: &mockito::Request) -> Self {
        let header = |name: &str| {
            request
                .header(name)
                .first()
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        Self {
            path: request.path().to_owned(),
            query: request
                .path_and_query()
                .split_once('?')
                .map(|(_, query)| query.to_owned()),
            key: header("kalshi-access-key"),
            timestamp: header("kalshi-access-timestamp"),
            signature: header("kalshi-access-signature"),
            kalshi_headers: request
                .headers()
                .keys()
                .filter(|name| name.as_str().starts_with("kalshi-"))
                .count(),
        }
    }
}

/// What the server saw, in order.
#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<Sent>>>);

impl Seen {
    fn push(&self, sent: Sent) {
        self.0.lock().unwrap().push(sent);
    }

    fn all(&self) -> Vec<Sent> {
        self.0.lock().unwrap().clone()
    }

    fn last(&self) -> Sent {
        self.all().pop().expect("the server saw no request")
    }

    /// The requests that went to `path`.
    fn to(&self, path: &str) -> Vec<Sent> {
        let all = self.all();
        all.into_iter().filter(|sent| sent.path == path).collect()
    }
}

/// A catch-all that records every request. The status route answers `status_route` in turn, the
/// last repeating; any other path in `bodies` answers `200` with its body, and every other path
/// answers `404`.
async fn recording(
    server: &mut ServerGuard,
    status_route: &[usize],
    bodies: &[(&str, &str)],
    seen: &Seen,
) -> Mock {
    let status_route = status_route.to_vec();
    let served = AtomicUsize::new(0);
    let seen = seen.clone();
    let bodies: Vec<(String, String)> = bodies
        .iter()
        .map(|(path, body)| ((*path).to_owned(), (*body).to_owned()))
        .collect();
    let status_of = bodies.clone();
    server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |request| {
            seen.push(Sent::of(request));
            if request.path() == STATUS {
                let n = served.fetch_add(1, Ordering::SeqCst);
                status_route[n.min(status_route.len() - 1)]
            } else if status_of.iter().any(|(path, _)| path == request.path()) {
                200
            } else {
                404
            }
        })
        .with_header("content-type", "application/json")
        .with_body_from_request(move |request| {
            bodies
                .iter()
                .find(|(path, _)| path == request.path())
                .map_or_else(|| b"{}".to_vec(), |(_, body)| body.clone().into_bytes())
        })
        .expect_at_least(1)
        .create_async()
        .await
}

/// A client on the mock server with a short retry schedule.
fn client(server: &ServerGuard, credentials: Option<Credentials>) -> Predictions {
    let mut builder = Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 3,
            initial_backoff_ms: 20,
            max_backoff_ms: 50,
        });
    if let Some(credentials) = credentials {
        builder = builder.credentials(credentials);
    }
    builder.build().unwrap()
}

fn vectors() -> Value {
    fixtures!("predictions/signing").json("vectors")
}

fn field(vectors: &Value, key: &str, name: &str) -> String {
    vectors[key][name]
        .as_str()
        .unwrap_or_else(|| panic!("vectors.json has no {key}.{name}"))
        .to_owned()
}

/// A PEM block around `body`: the vectors hold each key as base64 lines with no armour line.
fn armour(label: &str, body: &str) -> String {
    format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n")
}

/// The vectors' Ed25519 key as PEM.
fn ed25519_pem() -> String {
    armour("PRIVATE KEY", &field(&vectors(), "ed25519", "pkcs8_body"))
}

fn ed25519() -> Credentials {
    Credentials::from_pem(KEY_ID, &ed25519_pem()).unwrap()
}

fn rsa() -> Credentials {
    let pem = armour("RSA PRIVATE KEY", &field(&vectors(), "rsa", "pkcs1_body"));
    Credentials::from_pem(KEY_ID, &pem).unwrap()
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("the vectors hold valid hex"))
        .collect()
}

/// Whether `(message, signature)` verifies under the vectors' Ed25519 public key.
fn ed25519_verifier() -> impl Fn(&[u8], &[u8]) -> bool {
    let public_key = hex(&field(&vectors(), "ed25519", "public_key_hex"));
    move |message, signature| {
        UnparsedPublicKey::new(&ED25519, &public_key)
            .verify(message, signature)
            .is_ok()
    }
}

/// Whether `(message, signature)` verifies under the vectors' RSA public key, in PSS.
fn rsa_verifier() -> impl Fn(&[u8], &[u8]) -> bool {
    let public_key = STANDARD
        .decode(field(&vectors(), "rsa", "public_key_pkcs1_der_b64"))
        .expect("the vectors hold valid base64");
    move |message, signature| {
        UnparsedPublicKey::new(&RSA_PSS_2048_8192_SHA256, &public_key)
            .verify(message, signature)
            .is_ok()
    }
}

/// Whether `sent` carries a signature over `timestamp + GET + path`, the path without its query.
fn verifies(sent: &Sent, verify: &dyn Fn(&[u8], &[u8]) -> bool) -> bool {
    let (Some(timestamp), Some(signature)) = (&sent.timestamp, &sent.signature) else {
        return false;
    };
    let Ok(signature) = STANDARD.decode(signature) else {
        return false;
    };
    verify(
        format!("{timestamp}GET{}", sent.path).as_bytes(),
        &signature,
    )
}

/// The last request seen went to `path` and is signed by the key `verify` holds.
fn assert_signed(seen: &Seen, path: &str, verify: &dyn Fn(&[u8], &[u8]) -> bool) {
    let sent = seen.last();
    assert_eq!(sent.path, path);
    assert_eq!(
        sent.kalshi_headers, 3,
        "{path} carries three KALSHI- headers"
    );
    assert_eq!(sent.key.as_deref(), Some(KEY_ID), "{path}");
    assert!(
        verifies(&sent, verify),
        "{path}: the signature does not verify over timestamp + GET + path"
    );
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_millis()
}

fn timestamp(sent: &Sent) -> u128 {
    sent.timestamp
        .as_deref()
        .expect("the request is signed")
        .parse()
        .expect("the timestamp is milliseconds")
}

#[tokio::test]
async fn a_keyed_client_signs_every_route_but_the_ping() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(ed25519()));
    let verify = ed25519_verifier();
    let ticker = Ticker::new("KXTEST-1").unwrap();

    // Each answer is a 404 nobody reads: the request is what is under test.
    let _ = client.exchange().status().send().await;
    assert_signed(&seen, STATUS, &verify);
    let _ = client.markets().list().limit(5).send().await;
    assert_signed(&seen, "/trade-api/v2/markets", &verify);
    let _ = client.markets().get(&ticker).send().await;
    assert_signed(&seen, "/trade-api/v2/markets/KXTEST-1", &verify);
    let _ = client.markets().orderbook(&ticker).send().await;
    assert_signed(&seen, "/trade-api/v2/markets/KXTEST-1/orderbook", &verify);
    let _ = client.markets().trades().send().await;
    assert_signed(&seen, "/trade-api/v2/markets/trades", &verify);
    let _ = client.events().get("KXEVENT-1").send().await;
    assert_signed(&seen, "/trade-api/v2/events/KXEVENT-1", &verify);
    let _ = client.account().endpoint_costs().send().await;
    assert_signed(&seen, "/trade-api/v2/account/endpoint_costs", &verify);
}

#[tokio::test]
async fn the_ping_is_never_signed_even_by_a_keyed_client() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[200], &[], &seen).await;
    let client = client(&server, Some(ed25519()));

    let _ = client.health().ping().await;

    let sent = seen.last();
    assert_eq!(sent.path, STATUS);
    assert_eq!(sent.kalshi_headers, 0, "the ping carried a KALSHI- header");
}

#[tokio::test]
async fn the_signature_covers_the_path_and_not_the_query() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(ed25519()));
    let verify = ed25519_verifier();

    let _ = client.markets().list().limit(5).send().await;

    let sent = seen.last();
    assert_eq!(
        sent.query.as_deref(),
        Some("limit=5"),
        "the test's own premise"
    );
    assert!(
        verifies(&sent, &verify),
        "signed over timestamp + GET + path"
    );
    // The same request with its query on the signed path is a different message.
    let with_query = Sent {
        path: format!("{}?limit=5", sent.path),
        ..sent.clone()
    };
    assert!(
        !verifies(&with_query, &verify),
        "the signature also verifies over a path with its query"
    );
}

#[tokio::test]
async fn an_rsa_key_signs_in_pss_with_a_new_salt_each_time() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(rsa()));
    let verify = rsa_verifier();

    let _ = client.exchange().status().send().await;
    let first = seen.last();
    let _ = client.exchange().status().send().await;
    let second = seen.last();

    assert!(verifies(&first, &verify), "{first:?}");
    assert!(verifies(&second, &verify), "{second:?}");
    assert_ne!(first.signature, second.signature);
}

#[tokio::test]
async fn the_timestamp_is_milliseconds_within_a_second_of_the_clock() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, Some(ed25519()));

    let before = now_ms();
    let _ = client.exchange().status().send().await;
    let after = now_ms();

    let stamp = timestamp(&seen.last());
    assert!(
        stamp + 1_000 >= before && stamp <= after + 1_000,
        "{stamp} is not within a second of {before}..{after}"
    );
}

#[tokio::test]
async fn a_retried_attempt_is_signed_afresh() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[429, 200], &[], &seen).await;
    let client = client(&server, Some(ed25519()));
    let verify = ed25519_verifier();

    let _ = client.exchange().status().send().await;

    let attempts = seen.to(STATUS);
    assert_eq!(attempts.len(), 2, "a 429, then the retry: {attempts:?}");
    for attempt in &attempts {
        assert!(verifies(attempt, &verify), "{attempt:?}");
    }
    assert!(
        timestamp(&attempts[1]) > timestamp(&attempts[0]),
        "the retry reused the first attempt's timestamp"
    );
    assert_ne!(attempts[0].signature, attempts[1].signature);
}

#[tokio::test]
async fn a_key_id_that_cannot_be_a_header_value_fails_signing_and_sends_nothing() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let credentials = Credentials::from_pem("bad\nid", &ed25519_pem()).unwrap();
    let client = client(&server, Some(credentials));

    let Err(error) = client.exchange().status().send().await else {
        panic!("an unsignable request succeeded");
    };

    assert!(
        matches!(&error, PredictionsError::Api(ApiError::Sign(_))),
        "{error:?}"
    );
    assert_eq!(error.class(), Class::InvalidRequest);
    assert!(seen.to(STATUS).is_empty(), "an unsignable request was sent");
}

#[tokio::test]
async fn an_anonymous_client_sends_no_kalshi_header() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[], &seen).await;
    let client = client(&server, None);

    let _ = client.exchange().status().send().await;
    let _ = client.markets().list().send().await;
    let _ = client.account().endpoint_costs().send().await;

    let all = seen.all();
    assert!(all.len() >= 3, "the server saw {all:?}");
    assert!(
        all.iter().all(|sent| sent.kalshi_headers == 0),
        "an anonymous client sent a KALSHI- header"
    );
}
```

Add the two stanzas to the manifest. Current (`polyoxide-kalshi/Cargo.toml`, the line Story 1 writes directly above `[package.metadata.polyoxide]`):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "mock_predictions_signed"
required-features = ["predictions"]

[[test]]
name = "signing_predictions"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test signing_predictions --test mock_predictions_signed`

Expected: neither compiles. `error[E0432]: unresolved imports` names `polyoxide_kalshi::predictions::Credentials` and `CredentialsError`, and the mock suite adds `no method named `credentials` found for struct `PredictionsBuilder``.

- [ ] **Step 3: Implement**

Create `polyoxide-kalshi/src/predictions/auth.rs`:

```rust
//! Signing: the three headers Kalshi reads, made over `ring`.
//!
//! [`Credentials`] holds an API key id and the private key it signs with, an Ed25519 key or an
//! RSA key, read once from PEM. [`PredictionsBuilder::credentials`] installs them. A keyed client
//! signs every request but its ping, with a fresh timestamp and signature on every attempt, and
//! a client without credentials signs none.
//!
//! The text signed is the timestamp in milliseconds, the method and the path, joined with
//! nothing between them, and the path has no query. An Ed25519 key signs it as it is. An RSA key
//! signs with RSA-PSS over SHA-256, with MGF1 over SHA-256 and a salt as long as the digest. The
//! signature is base64, padded.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use polyoxide_core::reqwest::header::{HeaderName, HeaderValue};
use polyoxide_core::{ApiError, Authenticator, DynAuthenticator, RequestParts};
use polyoxide_venue::{Class, Classify, Secret, UnixMillis};
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, RsaKeyPair, RSA_PSS_SHA256};

use crate::predictions::PredictionsBuilder;

/// The header that names the API key.
pub(crate) const ACCESS_KEY: &str = "KALSHI-ACCESS-KEY";
/// The header that carries the timestamp, in milliseconds.
pub(crate) const ACCESS_TIMESTAMP: &str = "KALSHI-ACCESS-TIMESTAMP";
/// The header that carries the base64 signature.
pub(crate) const ACCESS_SIGNATURE: &str = "KALSHI-ACCESS-SIGNATURE";

/// Why a key could not be read or used to sign.
///
/// Every variant is [`Class::InvalidRequest`]: the caller supplied a key Kalshi cannot use. No
/// message quotes the key's body, the base64 or a byte of it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialsError {
    /// The text is not one armoured PEM block.
    #[error("the key is not valid PEM: {reason}")]
    Pem {
        /// What is wrong with the armour or the encoding.
        reason: String,
    },
    /// The PEM block is labelled as something other than a key Kalshi issues.
    #[error("a PEM labelled {label} is not a supported key; Kalshi keys are Ed25519 or RSA")]
    Label {
        /// The label on the BEGIN line.
        label: String,
    },
    /// The body is not a key `ring` can sign with.
    #[error("the PEM holds no Ed25519 or RSA key ring can sign with: {reason}")]
    Key {
        /// What `ring` rejected, in its own words.
        reason: String,
    },
    /// The key file could not be read.
    #[error("could not read the key file {path}: {reason}")]
    File {
        /// The path that was asked for.
        path: String,
        /// The operating system's reason.
        reason: String,
    },
    /// The key could not sign, or a header could not carry the result.
    #[error("signing failed: {reason}")]
    Sign {
        /// What failed.
        reason: String,
    },
}

/// A key that cannot be used is the caller's mistake, which no retry fixes.
impl Classify for CredentialsError {
    fn class(&self) -> Class {
        Class::InvalidRequest
    }
}

fn pem_error(reason: &str) -> CredentialsError {
    CredentialsError::Pem {
        reason: reason.to_owned(),
    }
}

/// An RSA key with the generator its signatures draw their salt from.
struct RsaSigner {
    pair: RsaKeyPair,
    rng: SystemRandom,
}

/// The private key, parsed. The RSA pair is boxed: it is the larger by far.
enum SigningKey {
    Ed25519(Ed25519KeyPair),
    Rsa(Box<RsaSigner>),
}

impl SigningKey {
    /// Reads a key from PEM.
    ///
    /// The label decides before the body is read: `RSA PRIVATE KEY` is PKCS#1 RSA, and
    /// `PRIVATE KEY` is PKCS#8, which cannot say what it holds, so it is tried as Ed25519 and
    /// then as RSA. Any other label, an encrypted key included, is refused.
    fn from_pem(text: &str) -> Result<Self, CredentialsError> {
        let (label, body) = unarmour(text)?;
        if label != "RSA PRIVATE KEY" && label != "PRIVATE KEY" {
            return Err(CredentialsError::Label {
                label: label.chars().take(64).collect(),
            });
        }
        let der = STANDARD
            .decode(body.as_bytes())
            .map_err(|_| pem_error("the body is not base64"))?;
        if label == "RSA PRIVATE KEY" {
            let pair = RsaKeyPair::from_der(&der).map_err(|rejected| CredentialsError::Key {
                reason: rejected.to_string(),
            })?;
            return Ok(Self::rsa(pair));
        }
        match Ed25519KeyPair::from_pkcs8_maybe_unchecked(&der) {
            Ok(pair) => Ok(Self::Ed25519(pair)),
            Err(not_ed25519) => match RsaKeyPair::from_pkcs8(&der) {
                Ok(pair) => Ok(Self::rsa(pair)),
                Err(not_rsa) => Err(CredentialsError::Key {
                    reason: format!("not Ed25519 ({not_ed25519}) and not RSA ({not_rsa})"),
                }),
            },
        }
    }

    fn rsa(pair: RsaKeyPair) -> Self {
        Self::Rsa(Box::new(RsaSigner {
            pair,
            rng: SystemRandom::new(),
        }))
    }

    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CredentialsError> {
        match self {
            Self::Ed25519(pair) => Ok(pair.sign(message).as_ref().to_vec()),
            Self::Rsa(signer) => {
                let mut signature = vec![0; signer.pair.public().modulus_len()];
                signer
                    .pair
                    .sign(&RSA_PSS_SHA256, &signer.rng, message, &mut signature)
                    .map_err(|_| CredentialsError::Sign {
                        reason: "the RSA key could not sign".to_owned(),
                    })?;
                Ok(signature)
            }
        }
    }
}

/// The label and the still-encoded body of the one PEM block in `text`.
///
/// A literal backslash-n reads as a line break, so a key kept on one line in an env file parses.
fn unarmour(text: &str) -> Result<(String, String), CredentialsError> {
    let text = text.replace("\\n", "\n");
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let begin = lines.next().ok_or_else(|| pem_error("there is no text"))?;
    let label = begin
        .strip_prefix("-----BEGIN ")
        .and_then(|rest| rest.strip_suffix("-----"))
        .ok_or_else(|| pem_error("the text does not start with a BEGIN line"))?;
    let end = format!("-----END {label}-----");
    let mut body = String::new();
    let mut closed = false;
    for line in lines {
        if closed {
            return Err(pem_error("there is text after the END line"));
        }
        if line == end {
            closed = true;
        } else if line.contains(':') {
            return Err(pem_error(
                "the key is encrypted or carries headers; decrypt it first",
            ));
        } else {
            body.push_str(line);
        }
    }
    if !closed {
        return Err(pem_error("there is no END line for the BEGIN line"));
    }
    Ok((label.to_owned(), body))
}

/// An API key id and the private key that signs for it.
///
/// Build one from the PEM Kalshi issued, with [`from_pem`](Self::from_pem) or
/// [`from_pem_file`](Self::from_pem_file), and hand it to [`PredictionsBuilder::credentials`].
/// The key is held in a [`Secret`], so `Debug` prints the key id and never the key.
#[derive(Clone)]
pub struct Credentials {
    key_id: String,
    key: Secret<Arc<SigningKey>>,
}

impl Credentials {
    /// Reads an Ed25519 or an RSA private key from PEM text.
    ///
    /// Kalshi's keys come as PKCS#8 under `PRIVATE KEY` (Ed25519, or RSA from `openssl genpkey`)
    /// or as PKCS#1 under `RSA PRIVATE KEY`. A literal backslash-n reads as a line break.
    ///
    /// # Errors
    ///
    /// [`CredentialsError`] when the text is not one PEM block, its label is not one of those
    /// two, or `ring` rejects its body. No error quotes the key.
    pub fn from_pem(key_id: impl Into<String>, pem: &str) -> Result<Self, CredentialsError> {
        let key = SigningKey::from_pem(pem)?;
        Ok(Self {
            key_id: key_id.into(),
            key: Secret::new(Arc::new(key)),
        })
    }

    /// Reads the PEM from a file, as [`from_pem`](Self::from_pem) does.
    ///
    /// # Errors
    ///
    /// [`CredentialsError::File`] when the file cannot be read, and every error `from_pem` has.
    pub fn from_pem_file(
        key_id: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Self, CredentialsError> {
        let path = path.as_ref();
        let pem = std::fs::read_to_string(path).map_err(|error| CredentialsError::File {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        Self::from_pem(key_id, &pem)
    }

    /// The API key id, which Kalshi shows beside the key and which is not a secret.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Signs `message` and returns the signature as padded `STANDARD` base64.
    ///
    /// Not part of the API: it exists so a suite can sign a fixed message, which
    /// `sign_request` cannot do because it stamps the clock.
    #[doc(hidden)]
    pub fn sign_message(&self, message: &[u8]) -> Result<String, CredentialsError> {
        Ok(STANDARD.encode(self.key.expose().sign(message)?))
    }

    /// The three headers for a request: the key id, now in milliseconds, and the signature of
    /// `timestamp + method + path`. `path` has no query.
    pub(crate) fn sign_request(
        &self,
        method: &str,
        path: &str,
    ) -> Result<SignedHeaders, CredentialsError> {
        let timestamp = UnixMillis::now().0.to_string();
        let signature = self.sign_message(format!("{timestamp}{method}{path}").as_bytes())?;
        Ok(SignedHeaders {
            key: self.key_id.clone(),
            timestamp,
            signature,
        })
    }
}

/// Written by hand so the key is never printed.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("key_id", &self.key_id)
            .field("key", &self.key)
            .finish()
    }
}

/// The values of the three signing headers for one attempt.
#[derive(Debug, Clone)]
pub(crate) struct SignedHeaders {
    pub(crate) key: String,
    pub(crate) timestamp: String,
    pub(crate) signature: String,
}

impl SignedHeaders {
    /// Each header's name and value, in the order key, timestamp, signature.
    pub(crate) fn pairs(&self) -> [(&'static str, &str); 3] {
        [
            (ACCESS_KEY, &self.key),
            (ACCESS_TIMESTAMP, &self.timestamp),
            (ACCESS_SIGNATURE, &self.signature),
        ]
    }
}

/// Signs each attempt of a request with the account's key.
struct KalshiAuth {
    credentials: Credentials,
}

fn sign_failure(error: CredentialsError) -> ApiError {
    ApiError::Sign(Box::new(error))
}

/// A header value that cannot be sent, as the signing error it is.
fn unfit(name: &str) -> ApiError {
    sign_failure(CredentialsError::Sign {
        reason: format!("the {name} value is not a valid header value"),
    })
}

impl Authenticator for KalshiAuth {
    async fn sign(&self, parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        let signed = self
            .credentials
            .sign_request(parts.method.as_str(), &parts.path)
            .map_err(sign_failure)?;
        for (name, value) in signed.pairs() {
            let header = HeaderName::from_bytes(name.as_bytes()).map_err(|_| unfit(name))?;
            let mut value = HeaderValue::from_str(value).map_err(|_| unfit(name))?;
            // Not a secret once it is sent, but not for a log either.
            value.set_sensitive(name == ACCESS_SIGNATURE);
            parts.headers.insert(header, value);
        }
        Ok(())
    }
}

impl PredictionsBuilder {
    /// Sign every request the client sends but its ping with `credentials`.
    ///
    /// Each attempt is signed afresh, so a retry carries a new timestamp. A client built without
    /// credentials signs nothing, and a route that needs a key, such as `account().limits()`,
    /// fails before sending anything.
    pub fn credentials(mut self, credentials: Credentials) -> Self {
        self.auth = Some(DynAuthenticator::new_arc(KalshiAuth { credentials }));
        self
    }
}
```

Replace the module block in `polyoxide-kalshi/src/predictions/mod.rs`. Current (Story 3's block):

```rust
pub mod api;
pub mod error;
pub mod key;
pub mod request;
mod throttle;
pub mod types;
pub mod venue;

pub use error::{PredictionsError, VenueError};
pub use key::key;
```

Replace with:

```rust
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
```

Add `CredentialsError` to the assertion in `polyoxide-kalshi/src/lib.rs`. Current:

```rust
    is::<predictions::key::InvalidTicker>();
};
```

Replace with:

```rust
    is::<predictions::key::InvalidTicker>();
    is::<predictions::CredentialsError>();
};
```

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test signing_predictions --test mock_predictions_signed`

Expected: every test passes, among them `the_ed25519_signature_is_the_vectors`, `ring_verifies_pythons_pss_signature_so_mgf1_and_the_salt_length_agree`, `the_rsa_signature_verifies_for_both_pem_forms_and_only_for_its_message`, `a_keyed_client_signs_every_route_but_the_ping`, `the_signature_covers_the_path_and_not_the_query` and `a_retried_attempt_is_signed_afresh`, and each output ends `test result: ok.`.

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command cargo fmt --all -- --check`

Expected: no output. If it prints a diff, run `nix develop --command cargo fmt --all`, check that only whitespace and line breaks moved, and run the commands above again.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`

Expected: finishes with no warnings.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/auth.rs polyoxide-kalshi/tests/signing_predictions.rs polyoxide-kalshi/tests/mock_predictions_signed.rs
git commit -m "feat(kalshi): credentials over ring and a signing authenticator (Epic 8, Story 4)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/src/lib.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/auth.rs polyoxide-kalshi/tests/signing_predictions.rs polyoxide-kalshi/tests/mock_predictions_signed.rs
```

Include `Cargo.lock` in the pathspec if cargo changed it.

### Task 4.3: `account().limits()` and the limits types

`AccountApi::limits` is a second `impl AccountApi` block in `api/limits.rs`, so Story 2's `account.rs` is not edited. Its builder checks `Routed::has_key` before it sends anything, so a client without a key sends nothing and spends no token. The types are Story 2's kind: every field public, and the buckets' counts as the whole tokens the wire carries (`u32`).

**Files:**
- Create: `polyoxide-kalshi/src/predictions/api/limits.rs`
- Create: `polyoxide-kalshi/src/predictions/types/limits.rs`
- Modify: `polyoxide-kalshi/src/predictions/api/mod.rs` (the module block)
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs` (the module block)
- Test: `polyoxide-kalshi/tests/mock_predictions_signed.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-kalshi/tests/mock_predictions_signed.rs`, add the route's constants. Current:

```rust
const STATUS: &str = "/trade-api/v2/exchange/status";
const KEY_ID: &str = "test-key-id";
```

Replace with:

```rust
const STATUS: &str = "/trade-api/v2/exchange/status";
const LIMITS: &str = "/trade-api/v2/account/limits";
const KEY_ID: &str = "test-key-id";

/// An answer of `/account/limits` for a Premier account, with one grant that expires and one that
/// does not.
const LIMITS_BODY: &str = r#"{"usage_tier":"Premier","read":{"refill_rate":1200,"bucket_capacity":1200},"write":{"refill_rate":600,"bucket_capacity":600},"grants":[{"exchange_instance":"main","level":"premier","expires_ts":1790000000,"source":"volume"},{"exchange_instance":"main","level":"basic","source":"default"}]}"#;
```

Append after the last test of `polyoxide-kalshi/tests/mock_predictions_signed.rs`. Current (the end of `an_anonymous_client_sends_no_kalshi_header`):

```rust
    assert!(
        all.iter().all(|sent| sent.kalshi_headers == 0),
        "an anonymous client sent a KALSHI- header"
    );
}
```

Replace with:

```rust
    assert!(
        all.iter().all(|sent| sent.kalshi_headers == 0),
        "an anonymous client sent a KALSHI- header"
    );
}

#[tokio::test]
async fn limits_decodes_the_accounts_buckets_and_is_signed() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[(LIMITS, LIMITS_BODY)], &seen).await;
    let client = client(&server, Some(ed25519()));

    let limits = client.account().limits().send().await.unwrap();

    assert_eq!(limits.usage_tier, "Premier");
    assert_eq!(
        (limits.read.bucket_capacity, limits.read.refill_rate),
        (1200, 1200)
    );
    assert_eq!(
        (limits.write.bucket_capacity, limits.write.refill_rate),
        (600, 600)
    );
    assert_eq!(limits.grants.len(), 2);
    assert_eq!(limits.grants[0].level, "premier");
    assert_eq!(limits.grants[0].expires_ts, Some(1_790_000_000));
    assert_eq!(limits.grants[1].expires_ts, None);
    let verify = ed25519_verifier();
    let sent = seen.to(LIMITS);
    assert!(!sent.is_empty(), "the route was never read");
    for request in &sent {
        assert!(verifies(request, &verify), "{request:?}");
    }
}

#[tokio::test]
async fn limits_without_credentials_sends_nothing() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let _mock = recording(&mut server, &[404], &[(LIMITS, LIMITS_BODY)], &seen).await;
    let client = client(&server, None);

    let error = client.account().limits().send().await.unwrap_err();

    match error {
        PredictionsError::Api(ApiError::Validation(message)) => assert_eq!(
            message,
            "account().limits() needs credentials: build the client with PredictionsBuilder::credentials"
        ),
        other => panic!("expected a validation refusal, got {other:?}"),
    }
    assert!(
        seen.to(LIMITS).is_empty(),
        "the refused call reached the server"
    );
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed`

Expected: the suite does not compile: `no method named `limits` found for struct `AccountApi``.

- [ ] **Step 3: Implement**

Create `polyoxide-kalshi/src/predictions/types/limits.rs`:

```rust
//! The account's limits: `GET /trade-api/v2/account/limits`.

use serde::{Deserialize, Serialize};

/// An account's token buckets and the grants behind its tier.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountLimits {
    /// The account's usage tier.
    pub usage_tier: String,
    /// The Read bucket, which every read request is charged against.
    pub read: BucketLimit,
    /// The Write bucket. Recorded, and not built: it comes with Kalshi's `Trading`.
    pub write: BucketLimit,
    /// The grants the tier comes from.
    #[serde(default)]
    pub grants: Vec<ApiUsageGrant>,
}

/// One token bucket, as whole tokens.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BucketLimit {
    /// Tokens added a second.
    pub refill_rate: u32,
    /// The most tokens the bucket holds.
    pub bucket_capacity: u32,
}

/// A grant of API usage to the account.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ApiUsageGrant {
    /// The exchange instance the grant applies to.
    pub exchange_instance: String,
    /// The level the grant gives.
    pub level: String,
    /// When the grant expires, in seconds since the epoch, if it does.
    pub expires_ts: Option<i64>,
    /// What gave the grant.
    pub source: String,
}
```

Create `polyoxide-kalshi/src/predictions/api/limits.rs`:

```rust
//! The account's limits: `GET /trade-api/v2/account/limits`.

use polyoxide_core::ApiError;

use super::AccountApi;
use crate::predictions::{
    error::PredictionsError,
    request::{routes, Routed},
    types::AccountLimits,
};

impl AccountApi {
    /// `GET /trade-api/v2/account/limits`: the account's Read and Write token buckets and the
    /// grants behind its tier. It needs credentials.
    pub fn limits(&self) -> GetAccountLimits {
        GetAccountLimits {
            request: Routed::new(&self.inner, routes::ACCOUNT_LIMITS, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/account/limits`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetAccountLimits {
    pub(crate) request: Routed<AccountLimits>,
}

impl GetAccountLimits {
    /// Signs, sends and decodes the request.
    ///
    /// # Errors
    ///
    /// Without credentials it sends nothing and spends no token: the error is a validation
    /// refusal, an `InvalidRequest`.
    pub async fn send(self) -> Result<AccountLimits, PredictionsError> {
        if !self.request.has_key() {
            return Err(PredictionsError::Api(ApiError::Validation(
                "account().limits() needs credentials: build the client with PredictionsBuilder::credentials"
                    .into(),
            )));
        }
        self.request.send().await
    }
}
```

Replace the module block in `polyoxide-kalshi/src/predictions/api/mod.rs`. Current (Story 2's block):

```rust
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
```

Replace with:

```rust
pub mod account;
pub mod events;
pub mod exchange;
pub mod health;
pub mod limits;
pub mod markets;

pub use account::{AccountApi, GetEndpointCosts};
pub use events::{EventsApi, GetEvent};
pub use exchange::{ExchangeApi, GetExchangeStatus};
pub use health::Health;
pub use limits::GetAccountLimits;
pub use markets::{GetMarket, GetOrderbook, ListMarkets, ListTrades, MarketsApi};
```

Replace the module block in `polyoxide-kalshi/src/predictions/types/mod.rs`. Current (Story 2's block):

```rust
mod account;
mod events;
mod exchange;
mod markets;

pub use account::{EndpointCost, EndpointCosts};
pub use events::{Event, EventResponse};
pub use exchange::{ExchangeIndexStatus, ExchangeStatus};
pub use markets::{
    BookSide, Market, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, MveFilter,
    OrderbookFp, OrderbookResponse, OutcomeSide, PriceLevel, PriceRange, Trade, TradesPage,
};
```

Replace with:

```rust
mod account;
mod events;
mod exchange;
mod limits;
mod markets;

pub use account::{EndpointCost, EndpointCosts};
pub use events::{Event, EventResponse};
pub use exchange::{ExchangeIndexStatus, ExchangeStatus};
pub use limits::{AccountLimits, ApiUsageGrant, BucketLimit};
pub use markets::{
    BookSide, Market, MarketResponse, MarketStatus, MarketStatusFilter, MarketsPage, MveFilter,
    OrderbookFp, OrderbookResponse, OutcomeSide, PriceLevel, PriceRange, Trade, TradesPage,
};
```

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed`

Expected: every test passes, including `limits_decodes_the_accounts_buckets_and_is_signed` and `limits_without_credentials_sends_nothing`.

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command cargo fmt --all -- --check`

Expected: no output (if it prints a diff, run `nix develop --command cargo fmt --all` and the commands above again).

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`

Expected: finishes with no warnings.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/api/limits.rs polyoxide-kalshi/src/predictions/types/limits.rs
git commit -m "feat(kalshi): account().limits() and the limits types (Epic 8, Story 4)" -- polyoxide-kalshi/src/predictions/api/mod.rs polyoxide-kalshi/src/predictions/api/limits.rs polyoxide-kalshi/src/predictions/types/mod.rs polyoxide-kalshi/src/predictions/types/limits.rs polyoxide-kalshi/tests/mock_predictions_signed.rs
```

### Task 4.4: The first-use confirmation

This replaces Story 1's `before_send` stub, as a whole, with the confirmation of spec D5. Costs are read when a request is sent, after the confirmation, so no builder changes. The two reads are core `Request`s and not route builders, since a builder's send waits on the confirmation and would wait on itself. They are charged to the Read layer at their own cost, and signed when the client holds a key. The bucket always ends confirmed: `resize` from `/account/limits` when it answers with a capacity and a refill, and `confirm` otherwise, so a cost above Basic's capacity is refused and not left waiting. The confirmation lives in an `OnceCell` that every clone shares through `Inner`'s `Arc`. The file's items end in this order: the `impl Inner` block, this task's inline tests in `#[cfg(test)] mod confirmation_tests`, and Story 1's `#[cfg(test)] mod tests`, which stays the last item (clippy's `items_after_test_module`), so the two test modules never share a name. The suites of Tasks 4.2 and 4.3 keep passing after it, because each reads the last request seen, or the requests to one path, and never counts the first send's two reads.

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs` (the `before_send` stub, and a `confirmation_tests` module directly after it, ahead of Story 1's `mod tests`)
- Test: `polyoxide-kalshi/src/predictions/throttle.rs` (a new `confirmation_tests` module), `polyoxide-kalshi/tests/mock_predictions_signed.rs`

- [ ] **Step 1: Write the failing tests**

Add the inline tests directly after Story 1's stub in `polyoxide-kalshi/src/predictions/throttle.rs`, so they sit ahead of Story 1's `mod tests`, which stays the file's last item. The module is `confirmation_tests`, not `tests`. Current:

```rust
impl Inner {
    /// Runs before every send.
    pub(crate) async fn before_send(&self) {}
}
```

Replace with:

```rust
impl Inner {
    /// Runs before every send.
    pub(crate) async fn before_send(&self) {}
}

#[cfg(test)]
mod confirmation_tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use mockito::{Server, ServerGuard};
    use polyoxide_core::ApiError;
    use polyoxide_test_support::fixtures;

    use crate::predictions::request::{routes, DEFAULT_COST};
    use crate::predictions::{Credentials, Predictions, PredictionsError};

    const COSTS: &str = "/trade-api/v2/account/endpoint_costs";
    const LIMITS: &str = "/trade-api/v2/account/limits";
    const STATUS: &str = "/trade-api/v2/exchange/status";
    const STATUS_BODY: &str = r#"{"exchange_active":true,"trading_active":true}"#;

    /// The Ed25519 key of the signing vectors, which are throwaway ones.
    fn keyed() -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key body");
        // The fixture holds the base64 lines with no armour line.
        let label = "PRIVATE KEY";
        let pem = format!("-----BEGIN {label}-----\n{body}\n-----END {label}-----\n");
        Credentials::from_pem("test-key", &pem).unwrap()
    }

    fn client(server: &ServerGuard, credentials: Option<Credentials>) -> Predictions {
        let mut builder = Predictions::builder().base_url(server.url());
        if let Some(credentials) = credentials {
            builder = builder.credentials(credentials);
        }
        builder.build().unwrap()
    }

    /// A mock answering `path` with `status` and `body`, expected `hits` times.
    async fn answer(
        server: &mut ServerGuard,
        path: &str,
        status: usize,
        body: &str,
        hits: usize,
    ) -> mockito::Mock {
        server
            .mock("GET", path)
            .with_status(status)
            .with_header("content-type", "application/json")
            .with_body(body)
            .expect(hits)
            .create_async()
            .await
    }

    /// `/account/endpoint_costs` listing each `(path, cost)` for `GET`.
    fn costs_body(listed: &[(&str, u32)]) -> String {
        let rows: Vec<_> = listed
            .iter()
            .map(|(path, cost)| serde_json::json!({"method": "GET", "path": path, "cost": cost}))
            .collect();
        serde_json::json!({"default_cost": 10, "endpoint_costs": rows}).to_string()
    }

    fn limits_body(capacity: u32, refill: u32) -> String {
        serde_json::json!({
            "usage_tier": "Premier",
            "read": {"refill_rate": refill, "bucket_capacity": capacity},
            "write": {"refill_rate": 100, "bucket_capacity": 100},
            "grants": [],
        })
        .to_string()
    }

    /// Records the target, level and fields of every event.
    #[derive(Clone, Default)]
    struct Events(Arc<Mutex<Vec<(String, tracing::Level, String)>>>);

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
            self.0.lock().unwrap().push((
                metadata.target().to_owned(),
                *metadata.level(),
                fields.0,
            ));
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }

    impl Events {
        /// The target and fields of every WARN seen so far.
        fn warnings(&self) -> Vec<(String, String)> {
            let events = self.0.lock().unwrap();
            events
                .iter()
                .filter(|(_, level, _)| *level == tracing::Level::WARN)
                .map(|(target, _, fields)| (target.clone(), fields.clone()))
                .collect()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_keyed_client_reads_each_confirmation_route_once_across_clones_and_concurrent_sends()
    {
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 200, &costs_body(&[]), 1).await;
        let limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1200), 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 8).await;
        let client = client(&server, Some(keyed()));

        let sends: Vec<_> = (0..8)
            .map(|_| {
                let client = client.clone();
                tokio::spawn(async move { client.exchange().status().send().await })
            })
            .collect();
        for send in sends {
            send.await.unwrap().unwrap();
        }

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        assert_eq!(
            client.inner.throttle.bucket().capacity(),
            1200,
            "the Read bucket was not resized to the account's capacity"
        );
    }

    #[tokio::test]
    async fn a_listed_route_takes_the_listed_cost_and_an_unlisted_route_keeps_the_compiled_one() {
        let mut server = Server::new_async().await;
        // The listing spells the orderbook's parameter another way, and carries a wildcard entry
        // that is none of ours.
        let listed = costs_body(&[
            (STATUS, 25),
            ("/trade-api/v2/markets/:market_ticker/orderbook", 30),
            ("/trade-api/v2/cfbenchmarks/*endpoint", 99),
        ]);
        let costs = answer(&mut server, COSTS, 200, &listed, 1).await;
        let limits = answer(&mut server, LIMITS, 200, &limits_body(1200, 1200), 0).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, None);

        client.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        limits.assert_async().await;
        status.assert_async().await;
        let costs = &client.inner.costs;
        assert_eq!(costs.cost(&routes::EXCHANGE_STATUS), 25);
        assert_eq!(costs.cost(&routes::ORDERBOOK), 30);
        assert_eq!(costs.cost(&routes::MARKETS), DEFAULT_COST);
        assert_eq!(costs.cost(&routes::MARKET), DEFAULT_COST);
        assert_eq!(costs.cost(&routes::TRADES), DEFAULT_COST);
    }

    #[tokio::test]
    async fn an_anonymous_client_confirms_at_basic_so_a_cost_above_it_is_refused_not_waited_on() {
        let mut server = Server::new_async().await;
        let listed = costs_body(&[(STATUS, 700)]);
        let _costs = answer(&mut server, COSTS, 200, &listed, 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 0).await;
        let client = client(&server, None);

        let outcome =
            tokio::time::timeout(Duration::from_secs(5), client.exchange().status().send())
                .await
                .expect("the send waited for a resize that never comes");

        match outcome {
            Err(PredictionsError::Api(ApiError::Refused(refusal))) => {
                assert_eq!(refusal.units, 700);
                assert_eq!(refusal.capacity, 600);
                assert_eq!(refusal.layer, crate::predictions::request::READ);
            }
            Err(other) => panic!("expected a refusal, got {other:?}"),
            Ok(_) => panic!("expected a refusal, got a success"),
        }
        status.assert_async().await;
    }

    #[tokio::test]
    async fn a_failed_costs_read_warns_once_and_the_next_send_does_not_repeat_it() {
        let events = Events::default();
        let _scope = tracing::subscriber::set_default(events.clone());
        // With one dispatcher registered, tracing-core works out the interest of a callsite
        // another test's thread hits first from that thread's default, which has none, and
        // caches it as never for every thread. A second one makes it ask each dispatcher.
        let _second = tracing::Dispatch::new(Events::default());
        let mut server = Server::new_async().await;
        let costs = answer(&mut server, COSTS, 500, "{}", 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 2).await;
        let client = client(&server, None);

        client.exchange().status().send().await.unwrap();
        client.exchange().status().send().await.unwrap();

        costs.assert_async().await;
        status.assert_async().await;
        let warnings = events.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].0.starts_with("polyoxide_kalshi"),
            "{warnings:?}"
        );
        assert!(warnings[0].1.contains("endpoint_costs"), "{warnings:?}");
        assert_eq!(
            client.inner.costs.cost(&routes::EXCHANGE_STATUS),
            DEFAULT_COST
        );
        // The bucket is confirmed at Basic: a cost above it is refused and not waited on.
        let admitted = tokio::time::timeout(
            Duration::from_secs(5),
            client.inner.throttle.bucket().acquire(700, true),
        )
        .await
        .expect("the bucket was left provisional");
        assert!(
            admitted.is_err(),
            "a cost above Basic's capacity was admitted"
        );
    }

    #[tokio::test]
    async fn a_failed_limits_read_warns_once_and_keeps_basics_sizing() {
        let events = Events::default();
        let _scope = tracing::subscriber::set_default(events.clone());
        let _second = tracing::Dispatch::new(Events::default());
        let mut server = Server::new_async().await;
        let _costs = answer(&mut server, COSTS, 200, &costs_body(&[]), 1).await;
        let limits = answer(&mut server, LIMITS, 500, "{}", 1).await;
        let status = answer(&mut server, STATUS, 200, STATUS_BODY, 1).await;
        let client = client(&server, Some(keyed()));

        client.exchange().status().send().await.unwrap();

        limits.assert_async().await;
        status.assert_async().await;
        let warnings = events.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].0.starts_with("polyoxide_kalshi"),
            "{warnings:?}"
        );
        assert!(warnings[0].1.contains("account/limits"), "{warnings:?}");
        assert_eq!(client.inner.throttle.bucket().capacity(), 600);
        let admitted = tokio::time::timeout(
            Duration::from_secs(5),
            client.inner.throttle.bucket().acquire(700, true),
        )
        .await
        .expect("the bucket was left provisional");
        assert!(
            admitted.is_err(),
            "a cost above Basic's capacity was admitted"
        );
    }
}
```

In `polyoxide-kalshi/tests/mock_predictions_signed.rs`, widen two imports. Current:

```rust
use std::time::{SystemTime, UNIX_EPOCH};
```

Replace with:

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};
```

Current (`polyoxide-kalshi/tests/mock_predictions_signed.rs`):

```rust
use polyoxide_core::{ApiError, RetryConfig};
```

Replace with:

```rust
use polyoxide_core::{ApiError, Refused, RetryConfig};
```

Add the confirmation's constants to the same file. Current (`polyoxide-kalshi/tests/mock_predictions_signed.rs`):

```rust
const STATUS: &str = "/trade-api/v2/exchange/status";
const LIMITS: &str = "/trade-api/v2/account/limits";
const KEY_ID: &str = "test-key-id";
```

Replace with:

```rust
const STATUS: &str = "/trade-api/v2/exchange/status";
const LIMITS: &str = "/trade-api/v2/account/limits";
const COSTS: &str = "/trade-api/v2/account/endpoint_costs";
const KEY_ID: &str = "test-key-id";

/// `/account/endpoint_costs` listing none of our routes.
const NO_COSTS: &str = r#"{"default_cost":10,"endpoint_costs":[]}"#;

/// `/account/endpoint_costs` listing the status route at 700 tokens, more than Basic's Read
/// bucket holds.
const COSTLY_STATUS: &str = r#"{"default_cost":10,"endpoint_costs":[{"method":"GET","path":"/trade-api/v2/exchange/status","cost":700}]}"#;

/// What `/exchange/status` answers.
const STATUS_BODY: &str = r#"{"exchange_active":true,"trading_active":true}"#;
```

Append after the last test of the same file. Current (`polyoxide-kalshi/tests/mock_predictions_signed.rs`, the end of `limits_without_credentials_sends_nothing`):

```rust
    assert!(
        seen.to(LIMITS).is_empty(),
        "the refused call reached the server"
    );
}
```

Replace with:

```rust
    assert!(
        seen.to(LIMITS).is_empty(),
        "the refused call reached the server"
    );
}

/// An answer of `/account/limits` whose Read bucket is `capacity` tokens refilling `refill` a
/// second.
fn limits_body(capacity: u32, refill: u32) -> String {
    serde_json::json!({
        "usage_tier": "Premier",
        "read": {"refill_rate": refill, "bucket_capacity": capacity},
        "write": {"refill_rate": 100, "bucket_capacity": 100},
        "grants": [],
    })
    .to_string()
}

/// A send that never settles fails the test and does not hang it.
async fn within<T>(send: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), send)
        .await
        .expect("the send did not settle in 10 s")
}

/// The refusal a send ended in; any other outcome fails the test.
fn refusal<T>(outcome: Result<T, PredictionsError>) -> Refused {
    match outcome {
        Err(PredictionsError::Api(ApiError::Refused(refused))) => refused,
        Err(other) => panic!("expected a refusal, got {other:?}"),
        Ok(_) => panic!("expected a refusal, got a success"),
    }
}

/// Records the target, level and fields of every event.
#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<(String, tracing::Level, String)>>>);

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

impl Events {
    /// The target and fields of every WARN seen so far.
    fn warnings(&self) -> Vec<(String, String)> {
        let events = self.0.lock().unwrap();
        events
            .iter()
            .filter(|(_, level, _)| *level == tracing::Level::WARN)
            .map(|(target, _, fields)| (target.clone(), fields.clone()))
            .collect()
    }
}

#[tokio::test]
async fn the_confirmation_reads_the_costs_then_the_limits_and_signs_both_when_keyed() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(600, 200);
    let bodies = [
        (COSTS, NO_COSTS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let verify = ed25519_verifier();

    client(&server, Some(ed25519()))
        .exchange()
        .status()
        .send()
        .await
        .unwrap();

    let all = seen.all();
    let paths: Vec<&str> = all.iter().map(|request| request.path.as_str()).collect();
    assert_eq!(paths, [COSTS, LIMITS, STATUS]);
    for request in &all {
        assert!(verifies(request, &verify), "{request:?}");
    }
}

#[tokio::test]
async fn an_anonymous_confirmation_reads_only_the_costs_and_signs_nothing() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let bodies = [(COSTS, NO_COSTS), (STATUS, STATUS_BODY)];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;

    client(&server, None)
        .exchange()
        .status()
        .send()
        .await
        .unwrap();

    let all = seen.all();
    let paths: Vec<&str> = all.iter().map(|request| request.path.as_str()).collect();
    assert_eq!(paths, [COSTS, STATUS]);
    assert!(all.iter().all(|request| request.kalshi_headers == 0));
}

#[tokio::test]
async fn a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(1200, 1200);
    let bodies = [
        (COSTS, COSTLY_STATUS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    within(client.exchange().status().send()).await.unwrap();

    assert_eq!(seen.to(STATUS).len(), 1, "the 700-token send went out");
}

#[tokio::test]
async fn the_bucket_takes_the_capacity_the_account_reports() {
    let mut server = Server::new_async().await;
    let seen = Seen::default();
    let limits = limits_body(650, 100);
    let bodies = [
        (COSTS, COSTLY_STATUS),
        (LIMITS, limits.as_str()),
        (STATUS, STATUS_BODY),
    ];
    let _mock = recording(&mut server, &[200], &bodies, &seen).await;
    let client = client(&server, Some(ed25519()));

    let refused = refusal(within(client.exchange().status().send()).await);

    assert_eq!((refused.units, refused.capacity), (700, 650));
    assert!(seen.to(STATUS).is_empty(), "a refused cost was sent");
}

#[tokio::test]
async fn a_limits_answer_with_a_zero_refill_or_capacity_keeps_basic_and_warns() {
    for (capacity, refill) in [(1200, 0), (0, 1200)] {
        let events = Events::default();
        let _scope = tracing::subscriber::set_default(events.clone());
        // With one dispatcher registered, tracing-core works out the interest of a callsite
        // another test's thread hits first from that thread's default, which has none, and
        // caches it as never for every thread. A second one makes it ask each dispatcher.
        let _second = tracing::Dispatch::new(Events::default());
        let mut server = Server::new_async().await;
        let seen = Seen::default();
        let limits = limits_body(capacity, refill);
        let bodies = [
            (COSTS, COSTLY_STATUS),
            (LIMITS, limits.as_str()),
            (STATUS, STATUS_BODY),
        ];
        let _mock = recording(&mut server, &[200], &bodies, &seen).await;
        let client = client(&server, Some(ed25519()));

        let refused = refusal(within(client.exchange().status().send()).await);

        assert_eq!(
            (refused.units, refused.capacity),
            (700, 600),
            "capacity {capacity}, refill {refill}: Basic's sizing was not kept"
        );
        let warnings = events.warnings();
        assert_eq!(
            warnings.len(),
            1,
            "capacity {capacity}, refill {refill}: {warnings:?}"
        );
        assert!(
            warnings[0].0.starts_with("polyoxide_kalshi"),
            "{warnings:?}"
        );
        assert!(warnings[0].1.contains("account/limits"), "{warnings:?}");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib confirmation_tests`

Expected: the five inline tests fail while the stub reads nothing. `a_keyed_client_reads_each_confirmation_route_once_across_clones_and_concurrent_sends` and `a_listed_route_takes_the_listed_cost_and_an_unlisted_route_keeps_the_compiled_one` panic at `costs.assert_async()` with `Expected 1 request(s) to GET /trade-api/v2/account/endpoint_costs, but received 0`. `an_anonymous_client_confirms_at_basic_so_a_cost_above_it_is_refused_not_waited_on` panics with `expected a refusal, got a success`, and the two failed-read tests panic the same way at their `assert_async()`.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed`

Expected: the new tests fail and the earlier ones pass. `the_confirmation_reads_the_costs_then_the_limits_and_signs_both_when_keyed` panics with `left: ["/trade-api/v2/exchange/status"]` against `right: ["/trade-api/v2/account/endpoint_costs", "/trade-api/v2/account/limits", "/trade-api/v2/exchange/status"]`, and `the_bucket_takes_the_capacity_the_account_reports` and the zero-refill test panic with `expected a refusal, got a success`, since the status send goes out at 10 tokens.

- [ ] **Step 3: Implement**

Replace the stub in `polyoxide-kalshi/src/predictions/throttle.rs`, leaving `confirmation_tests` below it and Story 1's `mod tests` after that, as they are. Current:

```rust
impl Inner {
    /// Runs before every send.
    pub(crate) async fn before_send(&self) {}
}
```

Replace with:

```rust
impl Inner {
    /// Runs before every send, and confirms the client's costs and Read bucket the first time.
    ///
    /// The first send of a client, and of every clone of it, runs the confirmation, and every
    /// other send, a concurrent one included, waits for it. It reads `/account/endpoint_costs`
    /// and replaces the compiled cost of each of our routes the answer lists, matched on method
    /// and template. With credentials it then reads `/account/limits` and resizes the Read
    /// bucket in place to the account's capacity and refill. It always ends with the bucket
    /// confirmed, so a cost above its capacity is refused and not left waiting for a resize that
    /// never comes.
    ///
    /// It never fails a send. A read that fails, and a limits answer with a zero capacity or
    /// refill (core's bucket panics on a zero refill), log one WARN and keep the compiled costs
    /// or Basic's sizing for the client's life. Basic is the floor of every tier, so such a
    /// client under-sends and never over-sends. A send dropped mid-confirmation leaves it to
    /// the next one.
    ///
    /// The two reads are core requests sent here, not through the route builders, which wait on
    /// this themselves. Each is charged to the Read layer at its own cost.
    pub(crate) async fn before_send(&self) {
        self.confirmation.get_or_init(|| self.confirm()).await;
    }

    async fn confirm(&self) {
        self.read_costs().await;
        self.read_limits().await;
    }

    /// One of the confirmation's reads: signed when the client holds a key, and charged to the
    /// Read layer at the route's cost as it stands.
    fn confirmation_request<T>(
        &self,
        route: &crate::predictions::request::Route,
    ) -> polyoxide_core::Request<T, crate::predictions::PredictionsError> {
        let request = polyoxide_core::Request::new(self.http.clone(), route.path(&[])).with_cost(
            polyoxide_core::Cost {
                layer: crate::predictions::request::READ,
                units: self.costs.cost(route),
                exact: true,
            },
        );
        match &self.auth {
            Some(auth) => request.authenticator(std::sync::Arc::clone(auth)),
            None => request,
        }
    }

    /// Replaces the compiled cost of each route `/account/endpoint_costs` lists.
    ///
    /// Only listed routes change: an unlisted route keeps the compiled cost, and the list's
    /// `default_cost` is not applied, since the live suite compares it with the compiled one.
    async fn read_costs(&self) {
        use crate::predictions::request::{routes, template_matches};
        use crate::predictions::types::EndpointCosts;

        let route = routes::ENDPOINT_COSTS;
        let template = route.template;
        let listed = match self
            .confirmation_request::<EndpointCosts>(&route)
            .send()
            .await
        {
            Ok(listed) => listed,
            Err(error) => {
                tracing::warn!(
                    "could not read {template}: {error}; keeping the compiled route costs"
                );
                return;
            }
        };
        for entry in &listed.endpoint_costs {
            for ours in routes::ALL {
                if ours.method.eq_ignore_ascii_case(&entry.method)
                    && template_matches(ours.template, &entry.path)
                {
                    self.costs.set(ours, entry.cost);
                }
            }
        }
    }

    /// Sizes the Read bucket from `/account/limits` when the client holds a key, and confirms it
    /// in every case.
    async fn read_limits(&self) {
        use crate::predictions::request::routes;
        use crate::predictions::types::AccountLimits;

        let bucket = self.throttle.bucket();
        if self.auth.is_none() {
            // No limit for unauthenticated requests is published: Basic's sizing stands.
            bucket.confirm();
            return;
        }
        let route = routes::ACCOUNT_LIMITS;
        let template = route.template;
        match self
            .confirmation_request::<AccountLimits>(&route)
            .send()
            .await
        {
            Ok(limits) if limits.read.bucket_capacity > 0 && limits.read.refill_rate > 0 => {
                bucket.resize(limits.read.bucket_capacity, limits.read.refill_rate);
            }
            Ok(limits) => {
                let (capacity, refill) = (limits.read.bucket_capacity, limits.read.refill_rate);
                tracing::warn!(
                    "{template} reports a Read bucket of {capacity} tokens refilling {refill} a second; keeping Basic's sizing"
                );
                bucket.confirm();
            }
            Err(error) => {
                tracing::warn!("could not read {template}: {error}; keeping Basic's sizing");
                bucket.confirm();
            }
        }
    }
}
```

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --lib confirmation_tests`

Expected: the five `confirmation_tests` pass, among them `a_keyed_client_reads_each_confirmation_route_once_across_clones_and_concurrent_sends` (each `expect` asserted) and `an_anonymous_client_confirms_at_basic_so_a_cost_above_it_is_refused_not_waited_on`.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions`

Expected: every suite passes, the earlier ones of `mock_predictions_signed` and `signing_predictions` included, and the new `mock_predictions_signed` tests (`the_confirmation_reads_the_costs_then_the_limits_and_signs_both_when_keyed`, `a_premier_account_is_admitted_a_cost_the_basic_bucket_would_refuse`, `the_bucket_takes_the_capacity_the_account_reports` and `a_limits_answer_with_a_zero_refill_or_capacity_keeps_basic_and_warns`).

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command cargo fmt --all -- --check`

Expected: no output (if it prints a diff, run `nix develop --command cargo fmt --all`, and run the commands above again).

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`

Expected: finishes with no warnings.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(kalshi): the first-use confirmation of costs and the Read bucket (Epic 8, Story 4)" -- polyoxide-kalshi/src/predictions/throttle.rs polyoxide-kalshi/tests/mock_predictions_signed.rs
```

### Task 4.5: The demo capture, the account suite and the registry

The wire test holds `AccountLimits` to the demo host's real answer in both directions, so an unmodelled key fails it. `live_predictions_account` runs against the demo host with the owner's demo key and is auth-gated without it. **This task needs the owner's demo key in `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`. When either is absent, stop at Step 3 and ask the owner; never write `limits.json` by hand.** The OBSERVED section is written only after the live suite has passed, so it records what was seen.

**Files:**
- Create: `polyoxide-kalshi/tests/live_predictions_account.rs`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/account/limits.json` (written by the script)
- Create: `polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md` (written by the script)
- Modify: `polyoxide-kalshi/Cargo.toml` (the `[[test]]` stanza and the live table)
- Modify: `docs/specs/kalshi/OBSERVED.md` (a section above its anchor)
- Modify (regenerated): `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- Test: `polyoxide-kalshi/tests/mock_predictions_signed.rs`

- [ ] **Step 1: Write the failing test and the live suite**

Append the wire test to `polyoxide-kalshi/tests/mock_predictions_signed.rs`. Current (the end of `a_limits_answer_with_a_zero_refill_or_capacity_keeps_basic_and_warns`):

```rust
        assert!(warnings[0].1.contains("account/limits"), "{warnings:?}");
    }
}
```

Replace with:

```rust
        assert!(warnings[0].1.contains("account/limits"), "{warnings:?}");
    }
}

/// `value` with every `null` removed, so an absent key and a null one compare equal.
fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| (key, without_nulls(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(without_nulls).collect()),
        other => other,
    }
}

#[test]
fn the_captured_limits_fit_the_type_in_both_directions() {
    let wire = fixtures!("predictions/account").json("limits");

    let limits: polyoxide_kalshi::predictions::types::AccountLimits =
        serde_json::from_value(wire.clone()).unwrap();

    // A key the wire carries and the type lacks is lost on the way back, and a key the type adds
    // is not on the wire; either makes the two differ.
    assert_eq!(
        without_nulls(serde_json::to_value(&limits).unwrap()),
        without_nulls(wire)
    );
    // Basic is the floor of every tier (Kalshi's rate_limits page): Read holds at least 600
    // tokens and refills at least 200 a second.
    assert!(limits.read.bucket_capacity >= 600);
    assert!(limits.read.refill_rate >= 200);
}
```

Create `polyoxide-kalshi/tests/live_predictions_account.rs`:

```rust
//! Live tests against Kalshi's demo host, signed with the owner's demo key.
//!
//! These hit the real host and need network access and the demo key, so they are `#[ignore]`d.
//! The key comes from `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text). A
//! run without them fails as `auth-gated`, which the nightly skips. Run with:
//! ```sh
//! cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_account -- --ignored
//! ```

use polyoxide_kalshi::predictions::{Credentials, Predictions, DEMO};
use polyoxide_test_support::{load_env, ResultExt};

/// A client on the demo host that signs with the demo key. Fails the test as `auth-gated` when
/// either variable is absent or empty.
fn client() -> Predictions {
    let creds = load_env(&["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"])
        .unwrap_or_else(|missing| missing.or_auth_gated());
    let credentials = Credentials::from_pem(
        creds.get("KALSHI_DEMO_API_KEY_ID"),
        creds.get("KALSHI_DEMO_PRIVATE_KEY"),
    )
    .or_fail("the demo key parses");
    Predictions::builder()
        .base_url(DEMO)
        .credentials(credentials)
        .build()
        .or_fail("the demo client builds")
}

#[tokio::test]
#[ignore]
async fn live_account_limits_are_at_least_basics() {
    let limits = client()
        .account()
        .limits()
        .send()
        .await
        .or_fail("account limits");

    // Basic is the floor of every tier: Read holds 600 tokens and refills 200 a second.
    assert!(limits.read.bucket_capacity >= 600);
    assert!(limits.read.refill_rate >= 200);
    assert!(!limits.usage_tier.is_empty());
}

#[tokio::test]
#[ignore]
async fn live_a_signed_public_read_succeeds() {
    let page = client()
        .markets()
        .list()
        .limit(1)
        .send()
        .await
        .or_fail("a signed markets list");

    assert!(page.markets.len() <= 1, "the limit was not honoured");
}
```

Add the live target's stanza to `polyoxide-kalshi/Cargo.toml`, directly before the registration anchor. Current:

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "live_predictions_account"
required-features = ["predictions"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Add its live table to `polyoxide-kalshi/Cargo.toml`, directly before the docs.rs table. Current (`polyoxide-kalshi/Cargo.toml`):

```toml
[package.metadata.docs.rs]
```

Replace with:

```toml
[package.metadata.polyoxide.live.live_predictions_account]
suite = "predictions"
timeout = 15
features = ["predictions"]
secrets = ["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"]

[package.metadata.docs.rs]
```

- [ ] **Step 2: Run the wire test to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed the_captured_limits_fit_the_type_in_both_directions`

Expected: it fails with a panic naming `polyoxide-kalshi/tests/fixtures/predictions/account/limits.json: No such file or directory (os error 2)`.

- [ ] **Step 3: Capture the demo host, run the live suite and record the findings**

Run: `nix develop --command uv run scripts/capture_kalshi_account.py demo polyoxide-kalshi/tests/fixtures/predictions/account`

Expected: prints `wrote polyoxide-kalshi/tests/fixtures/predictions/account/limits.json` and writes it with `PROVENANCE.md`. If it stops with `KALSHI_DEMO_API_KEY_ID and KALSHI_DEMO_PRIVATE_KEY are not both set`, stop the story here and ask the owner for the demo key. Do not write the fixture by hand, and do not go on to the steps below.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test mock_predictions_signed the_captured_limits_fit_the_type_in_both_directions`

Expected: it passes. If it fails because the captured answer carries a key `AccountLimits` lacks, the failure shows the two values: add the field to `polyoxide-kalshi/src/predictions/types/limits.rs` (every key the wire carries is modelled, spec §4.2) with a doc comment, run it again, and say so in the commit message.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_account -- --ignored`

Expected: both tests pass. A failure prints `polyoxide-class=<tag>` first: `auth-gated` means the key is not in the environment, and `real` means the demo host refused the signature or answered something the types do not read. Stop and report a `real` failure; the next edit records only what this run saw.

Record the findings. Current (`docs/specs/kalshi/OBSERVED.md`, its last line, which Story 1 writes):

```markdown
<!-- capture findings go above this line -->
```

Replace with:

```markdown
## Account limits and signing (Epic 8, Story 4)

Captured from the demo host (`https://external-api.demo.kalshi.co`) with the owner's demo key. The answer of `GET /trade-api/v2/account/limits` is `polyoxide-kalshi/tests/fixtures/predictions/account/limits.json`, and `polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md` says when. The demo account's tier and sizing are in that file.

- **The signing scheme is accepted as the docs give it.** The demo host answered `200` to a request signed over `timestamp_ms + METHOD + path`, with the path written from the API root and without its query, and sent with `KALSHI-ACCESS-KEY`, `KALSHI-ACCESS-TIMESTAMP` and `KALSHI-ACCESS-SIGNATURE`. It also answered `200` to a signed public read, `GET /trade-api/v2/markets`, so a keyed client may sign every route.
- **`/account/limits` is modelled whole.** `AccountLimits` holds every key of the captured answer, and `the_captured_limits_fit_the_type_in_both_directions` fails on one it lacks. `read` and `write` are each a `refill_rate` and a `bucket_capacity`, in whole tokens, and the live suite holds the demo account's Read bucket at or above Basic's published 600 tokens refilling 200 a second.
- **What the client does with it.** The first send of a client reads `/account/endpoint_costs` and, with a key, `/account/limits`, and resizes the Read bucket in place to the answer's `read`. It keeps Basic's sizing, with one WARN, when a read fails or the answer reports a zero capacity or refill. A client without a key keeps Basic's sizing, since no anonymous limit is published.
- **Recorded, not built.** The Write bucket, the per-shard Write buckets and the separate bucket of `GET /portfolio/balance` (2026-10-15) are published and not modelled by the throttle. They come with Kalshi's `Trading`.

<!-- capture findings go above this line -->
```

- [ ] **Step 4: Regenerate the registry, and run every gate**

Run: `nix develop --command python3 scripts/gen_registry.py --write`

Expected: it rewrites the generated regions of `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md` and `CLAUDE.md` (the nightly job list gains `live_predictions_account` and the two `KALSHI_DEMO_*` secrets), and nothing else. Check `git status --short`: only those three files and this task's own.

Run: `nix develop --command cargo fmt --all -- --check`

Expected: no output (if it prints a diff, run `nix develop --command cargo fmt --all`, and run the rest again).

Run: `nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`

Expected: every suite passes, the live suites skipped as `ignored`.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`

Expected: finishes with no warnings.

Run: `nix develop --command python3 scripts/gen_registry.py --check`

Expected: passes (the regions equal what the generator writes).

Run: `nix develop --command python3 scripts/venue_gate.py`

Expected: passes (nothing in a foundation crate changed).

Run: `nix develop --command python3 scripts/live_unwraps.py`

Expected: passes, with no `live_unwraps.baseline.json` entry for `live_predictions_account.rs` (its counts are zero).

Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`

Expected: passes (the story adds tests and changes none).

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py tests/test_classify_coverage.py -q'`

Expected: passes: the live target and its two secrets match what the loader reads, and `CredentialsError` is in the assertion.

Run: `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`

Expected: prints nothing and exits 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/tests/live_predictions_account.rs polyoxide-kalshi/tests/fixtures/predictions/account/limits.json polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md
git commit -m "feat(kalshi): the demo account's limits, captured, and the signed live suite (Epic 8, Story 4)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/tests/mock_predictions_signed.rs polyoxide-kalshi/tests/live_predictions_account.rs polyoxide-kalshi/tests/fixtures/predictions/account/limits.json polyoxide-kalshi/tests/fixtures/predictions/account/PROVENANCE.md docs/specs/kalshi/OBSERVED.md .github/workflows/nightly-behavioral.yml SELF-HEALING.md CLAUDE.md
```

Include `Cargo.lock` in the pathspec if cargo changed it, and `polyoxide-kalshi/src/predictions/types/limits.rs` if Step 3 added a field to it.

## Story 5: The kit's scripted server records handshake headers

- **Developer:** junior-dev
- **Files:** `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/tests.rs`, `CLAUDE.md`
- **Interfaces:** exposes `polyoxide_ws::test_server::ScriptedServer::handshake_headers`; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-ws --features test-server --lib test_server && python3 scripts/venue_gate.py && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
- **Review:** deep (a foundation crate)

`polyoxide-ws`'s doc-hidden scripted test server records each accepted connection's request path but not its headers, so a suite cannot show that a client signed its handshake afresh on each attempt (spec §7, spec D17). This story makes the `accept_hdr_async` callback copy the request's headers beside the path it already records, makes `serve()` push them just after `paths`, and adds `ScriptedServer::handshake_headers(&self) -> Vec<Vec<(String, String)>>` beside `paths()`: each accepted handshake's request headers, names lowercase, in accept order. A refused upgrade records nothing, as for `paths`. The new test is appended after the file's last test, in a module of its own, and uses made-up words only (`x-sample-stamp`, `alpha`, `beta`), because the foundation crates name no venue (`python3 scripts/venue_gate.py`). `test_server` is `#[doc(hidden)]` and behind `test-server`, so `docs/s3-removals.md` gains nothing. This is the foundation edit the owner approved (A8-4); nothing outside the two `test_server` files and CLAUDE.md's scripted-server paragraph changes. The code holds none of the profile's landmine markers: the test connects with `connect_async`, which is allowed inside `polyoxide-ws` (its existing tests already do).

### Task 5.1: Record the handshake's request headers

**Files:**
- Modify: `polyoxide-ws/src/test_server/mod.rs:237-249`, `:327-330`, `:518-549`, `:571-575`
- Test: `polyoxide-ws/src/test_server/tests.rs` (append after the last test, `drain_reads_to_the_end`)

- [ ] **Step 1: Write the failing test**

Append the new module to `polyoxide-ws/src/test_server/tests.rs`. The edit quotes the file's last test and its module's closing brace, and keeps them exactly as they are.

Current (the end of `polyoxide-ws/src/test_server/tests.rs`):

```rust
    #[tokio::test]
    async fn drain_reads_to_the_end() {
        let mut drained = Vec::new();
        drain(stream::iter([1, 2, 3]).inspect(|item| drained.push(*item))).await;
        assert_eq!(drained, [1, 2, 3]);
    }
}
```

Replace with:

```rust
    #[tokio::test]
    async fn drain_reads_to_the_end() {
        let mut drained = Vec::new();
        drain(stream::iter([1, 2, 3]).inspect(|item| drained.push(*item))).await;
        assert_eq!(drained, [1, 2, 3]);
    }
}

mod handshake_headers_are_recorded {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    use super::*;

    /// Connects to `url` with one extra request header, `x-sample-stamp: value`.
    async fn connect_stamped(
        url: &str,
        value: &'static str,
    ) -> Result<Client, tokio_tungstenite::tungstenite::Error> {
        let mut request = url.into_client_request().expect("a client request");
        request
            .headers_mut()
            .insert("x-sample-stamp", HeaderValue::from_static(value));
        connect_async(request).await.map(|(ws, _)| ws)
    }

    /// The value of the `x-sample-stamp` header among a handshake's headers.
    fn stamp(headers: &[(String, String)]) -> Option<&str> {
        headers
            .iter()
            .find(|(name, _)| name.as_str() == "x-sample-stamp")
            .map(|(_, value)| value.as_str())
    }

    #[tokio::test]
    async fn each_handshake_is_recorded_with_its_headers_in_accept_order() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        assert!(server.handshake_headers().is_empty());

        let _first = connect_stamped(&server.url, "alpha").await.unwrap();
        server
            .wait_for("one handshake", |s| s.handshake_headers().len() == 1)
            .await;
        let _second = connect_stamped(&server.url, "beta").await.unwrap();
        server
            .wait_for("two handshakes", |s| s.handshake_headers().len() == 2)
            .await;

        let recorded = server.handshake_headers();
        assert_eq!(recorded.len(), 2);
        assert_eq!(stamp(&recorded[0]), Some("alpha"));
        assert_eq!(stamp(&recorded[1]), Some("beta"));
        // Every name is lowercase, and the client's own upgrade headers are
        // among them, so the record is the request and not only what a test
        // added to it.
        assert!(recorded
            .iter()
            .flatten()
            .all(|(name, _)| *name == name.to_lowercase()));
        for headers in &recorded {
            assert!(headers
                .iter()
                .any(|(name, _)| name.as_str() == "sec-websocket-key"));
        }
    }

    #[tokio::test]
    async fn a_refused_upgrade_records_nothing() {
        let server =
            ScriptedServer::start(vec![Script::refuse_upgrade(429), Script::hold()]).await;
        // The first connection is refused; the second is accepted.
        assert!(connect_stamped(&server.url, "refused").await.is_err());
        let _accepted = connect_stamped(&server.url, "accepted").await.unwrap();
        server
            .wait_for("the accepted handshake", |s| {
                !s.handshake_headers().is_empty()
            })
            .await;

        let recorded = server.handshake_headers();
        assert_eq!(recorded.len(), 1);
        assert_eq!(stamp(&recorded[0]), Some("accepted"));
        assert_eq!(server.paths().len(), 1);
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-ws --features test-server --lib test_server::tests::handshake_headers_are_recorded`

Expected: the build fails with `error[E0599]: no method named `handshake_headers` found for struct `ScriptedServer` in the current scope`, once for each use in the new module.

- [ ] **Step 3: Implement**

Four edits to `polyoxide-ws/src/test_server/mod.rs`, applied top to bottom.

Edit 1, the recorder gains its field after `paths`.

Current (`polyoxide-ws/src/test_server/mod.rs:240-241`):

```rust
    paths: Mutex<Vec<String>>,
    received: Mutex<Vec<Received>>,
```

Replace with:

```rust
    paths: Mutex<Vec<String>>,
    handshake_headers: Mutex<Vec<Vec<(String, String)>>>,
    received: Mutex<Vec<Received>>,
```

Edit 2, the accessor sits beside `paths()`.

Current (`polyoxide-ws/src/test_server/mod.rs:327-330`):

```rust
    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.recorder.paths.lock().unwrap().clone()
    }
```

Replace with:

```rust
    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.recorder.paths.lock().unwrap().clone()
    }

    /// The request headers of each connection that completed a handshake, in
    /// accept order: a name and a value for each header, names lowercase and
    /// values read as lossy UTF-8. A refused upgrade records nothing, as for
    /// [`paths`](Self::paths).
    pub fn handshake_headers(&self) -> Vec<Vec<(String, String)>> {
        self.recorder.handshake_headers.lock().unwrap().clone()
    }
```

Edit 3, the handshake callback copies the headers beside the path.

Current (`polyoxide-ws/src/test_server/mod.rs:518-549`):

```rust
/// Complete the handshake, recording the request's path, or refuse it.
async fn handshake(stream: TcpStream, script: &Script) -> Option<(Socket, String)> {
    let path = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&path);
    let refusal = script.refuse_upgrade.map(|status| {
        let headers = script.refuse_headers.clone();
        (status, headers)
    });
    // The return type is tungstenite's `Callback` contract; it cannot be
    // boxed.
    #[allow(clippy::result_large_err)]
    let callback =
        move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            *seen.lock().unwrap() = request.uri().path().to_owned();
            let Some((status, headers)) = refusal else {
                return Ok(response);
            };
            let mut refused = ErrorResponse::new(None);
            *refused.status_mut() = StatusCode::from_u16(status).expect("an HTTP status");
            for (name, value) in headers {
                refused.headers_mut().insert(
                    HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
                    HeaderValue::from_str(&value).expect("a header value"),
                );
            }
            Err(refused)
        };
    // A refusal writes its response, then reports the handshake as failed.
    let ws = accept_hdr_async(stream, callback).await.ok()?;
    let path = path.lock().unwrap().clone();
    Some((ws, path))
}
```

Replace with:

```rust
/// Complete the handshake, recording the request's path and headers, or
/// refuse it.
async fn handshake(
    stream: TcpStream,
    script: &Script,
) -> Option<(Socket, String, Vec<(String, String)>)> {
    let path = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&path);
    let request_headers: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let seen_headers = Arc::clone(&request_headers);
    let refusal = script.refuse_upgrade.map(|status| {
        let headers = script.refuse_headers.clone();
        (status, headers)
    });
    // The return type is tungstenite's `Callback` contract; it cannot be
    // boxed.
    #[allow(clippy::result_large_err)]
    let callback =
        move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            *seen.lock().unwrap() = request.uri().path().to_owned();
            *seen_headers.lock().unwrap() = request
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            let Some((status, headers)) = refusal else {
                return Ok(response);
            };
            let mut refused = ErrorResponse::new(None);
            *refused.status_mut() = StatusCode::from_u16(status).expect("an HTTP status");
            for (name, value) in headers {
                refused.headers_mut().insert(
                    HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
                    HeaderValue::from_str(&value).expect("a header value"),
                );
            }
            Err(refused)
        };
    // A refusal writes its response, then reports the handshake as failed.
    let ws = accept_hdr_async(stream, callback).await.ok()?;
    let path = path.lock().unwrap().clone();
    let request_headers = request_headers.lock().unwrap().clone();
    Some((ws, path, request_headers))
}
```

Edit 4, `serve()` pushes the headers just after the path.

Current (`polyoxide-ws/src/test_server/mod.rs:571-575`):

```rust
    let Some((mut ws, path)) = handshake(stream, &script).await else {
        return Ok(());
    };
    recorder.handshakes.fetch_add(1, Ordering::SeqCst);
    recorder.paths.lock().unwrap().push(path);
```

Replace with:

```rust
    let Some((mut ws, path, headers)) = handshake(stream, &script).await else {
        return Ok(());
    };
    recorder.handshakes.fetch_add(1, Ordering::SeqCst);
    recorder.paths.lock().unwrap().push(path);
    recorder.handshake_headers.lock().unwrap().push(headers);
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-ws --features test-server --lib test_server`

Expected: every `test_server::tests::` test passes, among them `handshake_headers_are_recorded::each_handshake_is_recorded_with_its_headers_in_accept_order` and `handshake_headers_are_recorded::a_refused_upgrade_records_nothing`, ending in `test result: ok.` with no failures.

Then the task's gates: `nix develop --command cargo fmt --all -- --check` (if it reports a difference in the new lines, run `nix develop --command cargo fmt --all` and rerun the test), then `nix develop --command cargo clippy -p polyoxide-ws --all-targets --all-features -- -D warnings`, and `python3 scripts/venue_gate.py` (the test and the doc name no venue).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(ws): the scripted server records each handshake's request headers (Epic 8, Story 5)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- polyoxide-ws/src/test_server/mod.rs polyoxide-ws/src/test_server/tests.rs
```

### Task 5.2: List the recorder in CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (the scripted-server paragraph, `**One scripted WebSocket server lives in `polyoxide-ws`**`)

- [ ] **Step 1: Edit the paragraph**

AD-21: the paragraph states the recorders the server has, so the commit that adds one edits it. There is no test for a prose edit; the check is the grep in Step 2.

Current (part of the one long line of the paragraph in `CLAUDE.md`):

```
the recorders cover frames per connection, first frames, paths, closes, close replies, pongs, pings received, accept times and end counts, and `wait_for` polls for 5 s.
```

Replace with:

```
the recorders cover frames per connection, first frames, paths, each handshake's request headers (`handshake_headers`: names lowercase, values as lossy UTF-8, in accept order, and a refused upgrade records none), closes, close replies, pongs, pings received, accept times and end counts, and `wait_for` polls for 5 s.
```

- [ ] **Step 2: Check it**

Run:

```bash
grep -c "each handshake's request headers (\`handshake_headers\`" CLAUDE.md
```

Expected: `1`. Then run `python3 scripts/gen_registry.py --check` (the paragraph is outside every generated region, so it prints nothing and exits 0).

- [ ] **Step 3: Run the story's checks**

Run: `nix develop --command cargo test -p polyoxide-ws --features test-server --lib test_server`, then `python3 scripts/venue_gate.py`, then `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`.

Expected: the tests pass, the venue gate prints nothing, and the touch-set gate prints nothing (it exits 0).

Then the profile's story gates for the packages this story changes and the packages that depend on them (`polyoxide-ws`, `polyoxide-polymarket`, `polyoxide-binance`, `polyoxide-cli`, `polyoxide-test-support`, `polyoxide`), in order: `cargo fmt --all -- --check`, `cargo clippy -p polyoxide-ws -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-cli -p polyoxide-test-support -p polyoxide --all-targets --all-features -- -D warnings`, `cargo test` over the same packages with `--all-features`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` over the same packages with `--all-features`, `python3 scripts/gen_registry.py --check`, `python3 scripts/venue_gate.py`, `python3 scripts/live_unwraps.py` and `python3 scripts/test_body_diff.py --baseline v0.40.0`, each run with the `nix develop --command` prefix. The test-bodies gate passes because the story only appends a test.

- [ ] **Step 4: Commit**

```bash
git commit -m "docs(claude): the scripted server records handshake headers (Epic 8, Story 5)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- CLAUDE.md
```

## Story 6: A supervised Kalshi socket with a signed handshake

- **Developer:** intermediate-dev
- **Files:** `polyoxide-kalshi/Cargo.toml`, `Cargo.lock`, `polyoxide-kalshi/src/lib.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `scripts/capture_kalshi_predictions_ws.py`, `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/snapshot.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/deltas.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/ok.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/unsubscribed.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/error.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt`, `.github/scripts/tests/test_dep_allowlists.py`, `docs/specs/kalshi/OBSERVED.md`, `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- **Interfaces:** exposes `predictions::ws`'s items (names contract, Story 6); uses Story 1's keys, `Predictions`, `DEMO`, module block, assertion, anchors and the CI paragraph's named text, Story 2's `MarketStatusFilter`, `MveFilter` and `tests/common/mod.rs`'s `two_sided_market`, Story 4's `Credentials::sign_request`, `SignedHeaders`, the header consts, the vectors and `signed_headers`, and Story 5's `handshake_headers`
- **Verify:** `cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_live_registry.py tests/test_ci_workflow.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh`
- **Review:** deep (a supervised socket carrying credentials)

Adds the `predictions-ws` feature and `predictions::ws` (spec §8, D13 to D17): `PredictionsProtocol` on the kit's `Supervisor`, whose handshake is signed afresh on every attempt (the story's load-bearing rule, proved on the scripted server through Story 5's `handshake_headers()`). It subscribes `orderbook_delta` with `use_yes_price: true`, replays the whole membership as one `subscribe` on every connection, changes it with `update_subscription` on the live `sid`, checks `seq` per `sid` over every frame that carries one (answers included: the kit's `request` consumes an answer before `decode` sees it, so the correlation closure counts what it reads, and counts a push it keeps only once) and reconnects on a gap, pings every 10 s and counts every frame as liveness. Its code sits next to the landmines: the socket connects only through the kit (never `connect_async`), every socket error is classified only through `polyoxide_venue::impl_ws_classification!` (no match over `tungstenite::Error`; the only matches are over the kit's own `Kind`), and a refused upgrade's `Retry-After` reaches the kit through `Protocol::retry_after` so it only ever lengthens a wait. The supervisor should dispatch this story's developer with `effort: xhigh`. Task 6.1 is the demo capture and needs the owner's demo key (`KALSHI_DEMO_API_KEY_ID`, `KALSHI_DEMO_PRIVATE_KEY`): without it the story stops there and asks, and no fixture is ever written by hand. The later tasks add the feature and the error type (with the `@predictions-ws` allowlist and its `MUST_BUILD` row, and the docs.rs entry), the frames and events (with the wire agreement), the protocol and builder, the scripted answer and the supervision suite, `live_predictions_ws` (with the registry), and Story 6's CLAUDE.md edits: its two paragraphs, and the one named text of the CI paragraph that adds the `@predictions-ws` file.

### Task 6.1: Capture the socket's frames from the demo host

**Files:**
- Create: `scripts/capture_kalshi_predictions_ws.py`
- Create (written by the script, never by hand): `polyoxide-kalshi/tests/fixtures/predictions/ws/subscribed.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/snapshot.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/deltas.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/ok.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/unsubscribed.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/error.json`, `polyoxide-kalshi/tests/fixtures/predictions/ws/PROVENANCE.md`
- Modify: `docs/specs/kalshi/OBSERVED.md` (the script inserts one section directly above the anchor line `<!-- capture findings go above this line -->`)

The script signs its handshake through Story 4's `scripts/capture_kalshi_account.py` (`signed_headers(method, path)`), which it imports. Importing that module runs nothing (its steps run only under `if __name__ == "__main__":`), and this script declares `cryptography`, which that import needs, with `websockets` in its own PEP 723 header. The tests of Tasks 6.3 and 6.4 read the fixtures this task writes, so this task comes first: if the owner's key is absent the story stops here.

- [ ] **Step 1: Write the script**

Create `scripts/capture_kalshi_predictions_ws.py`:

```python
#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["websockets>=13", "certifi", "cryptography"]
# ///
"""Capture the order-book socket's frames from Kalshi's demo host as test fixtures for
polyoxide-kalshi's `predictions-ws` feature, and record what the capture found in
docs/specs/kalshi/OBSERVED.md.

Usage: uv run scripts/capture_kalshi_predictions_ws.py [--ticker TICKER] [--seconds 60]

Needs the owner's demo key in KALSHI_DEMO_API_KEY_ID and KALSHI_DEMO_PRIVATE_KEY (the PEM
text): the handshake is signed through capture_kalshi_account.signed_headers. Without them
the run stops before it connects, and no fixture is ever written by hand. Keys do not cross
environments, so a production key does not work here.

It subscribes one open demo market with use_yes_price, reads its snapshot and up to
--seconds of deltas, adds a second market, unsubscribes, and sends a subscribe that names
no market. The markets come from the demo host's unsigned listing with status=open and
mve_filter=exclude: the open listing is dominated by multivariate combinations, which this
capture does not want, and the book of the one it subscribes is confirmed on the orderbook
route. Nothing is written until every step has succeeded, so a failed run leaves the
fixtures as they were. It writes subscribed, snapshot, deltas, ok, unsubscribed and error
under polyoxide-kalshi/tests/fixtures/predictions/ws/, with PROVENANCE.md, and writes one
section into docs/specs/kalshi/OBSERVED.md directly above its anchor line, replacing the
one an earlier run wrote. Review the diff before committing.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import os
import time
import urllib.parse
from dataclasses import dataclass
from decimal import Decimal
from pathlib import Path
from typing import Any

from websockets.exceptions import ConnectionClosed, InvalidHandshake

import capture_common
import capture_kalshi_account

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "polyoxide-kalshi" / "tests" / "fixtures" / "predictions" / "ws"
OBSERVED = ROOT / "docs" / "specs" / "kalshi" / "OBSERVED.md"
REST = "https://external-api.demo.kalshi.co"
WS = "wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2"
SECRETS = ("KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY")
# How long a command may wait for its answer, and how long the capture waits for the
# snapshot of a market it added.
ANSWER_SECONDS = 10.0
ADDED_SECONDS = 3.0
# At most this many deltas are kept, and this many open markets are checked for a
# two-sided book.
MAX_DELTAS = 100
CANDIDATES = 20
ANSWERS = ("subscribed", "ok", "unsubscribed", "error")
ANCHOR = "<!-- capture findings go above this line -->"
BEGIN = "<!-- kalshi-ws-capture:begin -->"
END = "<!-- kalshi-ws-capture:end -->"


def require_key() -> None:
    missing = [name for name in SECRETS if not os.environ.get(name)]
    if missing:
        raise SystemExit(
            f"{' and '.join(missing)} not set. This capture signs its handshake with the owner's "
            "demo key: stop here and ask the owner for it. No fixture is ever written by hand."
        )


def require_anchor() -> None:
    if ANCHOR not in OBSERVED.read_text(encoding="utf-8"):
        raise SystemExit(f"{OBSERVED} has no anchor line {ANCHOR!r}: Story 1 writes it")


def signed(_method: str, url: str) -> dict[str, str]:
    """The handshake's headers, signed over the URL's path (`ws_session` calls this for
    each connection)."""
    return capture_kalshi_account.signed_headers("GET", urllib.parse.urlparse(url).path)


def fetch_ok(path: str, **params: Any) -> Any:
    reply = capture_common.get(f"{REST}{path}", params=params)
    return capture_common.require_ok(reply, f"GET {path}")


def two_sided(ticker: str) -> bool:
    book = fetch_ok(f"/trade-api/v2/markets/{ticker}/orderbook")["orderbook_fp"]
    return bool(book.get("yes_dollars")) and bool(book.get("no_dollars"))


def choose(wanted: str | None) -> tuple[str, str]:
    """The market to subscribe (the first open one with a two-sided book unless one is
    given), and another to add to the subscription.

    The listing excludes multivariate combinations (`mve_filter=exclude`), which dominate
    the open listing."""
    listing = fetch_ok("/trade-api/v2/markets", status="open", mve_filter="exclude", limit=100)
    tickers = [market["ticker"] for market in listing["markets"]]
    first = wanted or next((t for t in tickers[:CANDIDATES] if two_sided(t)), None)
    if first is None:
        raise SystemExit(
            f"none of the first {CANDIDATES} open demo markets has a two-sided book: pass --ticker"
        )
    others = [t for t in tickers if t != first]
    if not others:
        raise SystemExit("the demo host lists no second open market to add to the subscription")
    return first, others[0]


@dataclass
class Seen:
    """One frame read: its text, its parsed form, and the last `seq` its subscription had
    before it (`None` for a frame without `sid` and `seq`, and for the first of its
    subscription)."""

    text: str
    frame: dict[str, Any]
    previous: int | None


class Tape:
    """Every frame read from one connection, in order, with each subscription's `seq`."""

    def __init__(self, ws: Any) -> None:
        self.ws = ws
        self.log: list[Seen] = []
        self.last: dict[int, int] = {}
        self.first: dict[int, tuple[int, str]] = {}

    async def read(self, timeout: float) -> Seen | None:
        """The next frame, or None when none arrives within `timeout` seconds."""
        try:
            text = await asyncio.wait_for(self.ws.recv(), timeout=max(0.05, timeout))
        except asyncio.TimeoutError:
            return None
        frame = json.loads(text)
        previous = None
        sid, seq = frame.get("sid"), frame.get("seq")
        if isinstance(sid, int) and isinstance(seq, int):
            previous = self.last.get(sid)
            self.first.setdefault(sid, (seq, str(frame.get("type"))))
            self.last[sid] = seq
        seen = Seen(text, frame, previous)
        self.log.append(seen)
        return seen

    async def command(self, command: dict[str, Any]) -> Seen:
        """Sends `command` and reads until the frame answering its id arrives."""
        await self.ws.send(json.dumps(command))
        deadline = time.monotonic() + ANSWER_SECONDS
        while True:
            seen = await self.read(deadline - time.monotonic())
            if seen is None:
                raise SystemExit(
                    f"no answer to {command['cmd']} (id {command['id']}) in {ANSWER_SECONDS:.0f} s"
                )
            if seen.frame.get("id") == command["id"] and seen.frame.get("type") in ANSWERS:
                return seen

    async def listen(self, seconds: float) -> tuple[Seen | None, list[Seen]]:
        """The first snapshot, and up to MAX_DELTAS deltas, read in `seconds`."""
        snapshot: Seen | None = None
        deltas: list[Seen] = []
        deadline = time.monotonic() + seconds
        while len(deltas) < MAX_DELTAS and time.monotonic() < deadline:
            seen = await self.read(deadline - time.monotonic())
            if seen is None:
                break
            kind = seen.frame.get("type")
            if kind == "orderbook_snapshot" and snapshot is None:
                snapshot = seen
            elif kind == "orderbook_delta":
                deltas.append(seen)
        return snapshot, deltas

    async def read_for(self, seconds: float) -> None:
        """Reads whatever arrives in `seconds`."""
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if await self.read(deadline - time.monotonic()) is None:
                break


@dataclass
class Capture:
    first: str
    second: str
    subscribed: Seen
    snapshot: Seen
    deltas: list[Seen]
    added: Seen
    unsubscribed: Seen
    refused: Seen
    first_seq: tuple[int, str]
    snapshot_after_add: bool
    limit_hits: list[str]


def expect(seen: Seen, kind: str, what: str) -> None:
    if seen.frame.get("type") != kind:
        raise SystemExit(f"{what} was answered with {seen.text}, not a {kind}")


def book(snapshot: Seen) -> tuple[list[Decimal], list[Decimal]]:
    """The prices of the snapshot's YES side and of its NO side (in YES pricing)."""
    msg = snapshot.frame["msg"]
    return (
        [Decimal(price) for price, _count in msg.get("yes_dollars_fp", [])],
        [Decimal(price) for price, _count in msg.get("no_dollars_fp", [])],
    )


async def run(first: str, second: str, seconds: float) -> Capture:
    async with capture_common.ws_session(WS, headers=signed) as ws:
        tape = Tape(ws)
        params = {"channels": ["orderbook_delta"], "market_tickers": [first], "use_yes_price": True}
        subscribed = await tape.command({"id": 1, "cmd": "subscribe", "params": params})
        expect(subscribed, "subscribed", "subscribe")
        sid = subscribed.frame["msg"]["sid"]
        snapshot, deltas = await tape.listen(seconds)
        if snapshot is None:
            raise SystemExit(f"no orderbook_snapshot for {first} in {seconds:.0f} s")
        bids, asks = book(snapshot)
        if not bids or not asks:
            raise SystemExit(f"the snapshot of {first} is one-sided: pass --ticker of a market with both sides")
        if not deltas:
            raise SystemExit(
                f"no orderbook_delta for {first} in {seconds:.0f} s: rerun with --ticker of a busier market"
            )
        mark = len(tape.log)
        update = {"sid": sid, "action": "add_markets", "market_tickers": [second]}
        added = await tape.command({"id": 2, "cmd": "update_subscription", "params": update})
        expect(added, "ok", "update_subscription")
        await tape.read_for(ADDED_SECONDS)
        snapshot_after_add = any(
            seen.frame.get("type") == "orderbook_snapshot"
            and (seen.frame.get("msg") or {}).get("market_ticker") == second
            for seen in tape.log[mark:]
        )
        unsubscribed = await tape.command({"id": 3, "cmd": "unsubscribe", "params": {"sids": [sid]}})
        expect(unsubscribed, "unsubscribed", "unsubscribe")
        nothing = {"channels": ["orderbook_delta"]}
        refused = await tape.command({"id": 4, "cmd": "subscribe", "params": nothing})
        expect(refused, "error", "a subscribe that names no market")
        limit_hits = [
            seen.text
            for seen in tape.log
            if seen.frame.get("type") == "error" and (seen.frame.get("msg") or {}).get("code") == 26
        ]
        return Capture(
            first, second, subscribed, snapshot, deltas, added, unsubscribed, refused,
            tape.first[sid], snapshot_after_add, limit_hits,
        )


def advance(seen: Seen) -> str:
    """Whether the frame took the next number in its subscription's `seq`."""
    seq = seen.frame.get("seq")
    if not isinstance(seq, int):
        return "no: the frame carries no `seq`"
    if seen.previous is None:
        return f"unknown: `seq` {seq} was the first the capture saw on that subscription"
    verdict = "yes" if seq == seen.previous + 1 else "no"
    return f"{verdict}: `seq` {seen.previous} before it, {seq} on it"


def section(capture: Capture, stamp: str) -> str:
    bids, asks = book(capture.snapshot)
    best_bid, best_ask = max(bids), min(asks)
    seq, kind = capture.first_seq
    uncrossed = "yes" if best_bid < best_ask else "NO, crossed"
    if capture.limit_hits:
        bound = f"met, code 26: `{capture.limit_hits[0]}`"
    else:
        bound = "none met: the capture subscribed one market, then added a second"
    rows = [
        ("First `seq` of a subscription", f"{seq}, on its `{kind}` frame"),
        ("`ok` to `update_subscription` advances `seq`", advance(capture.added)),
        ("`unsubscribed` advances `seq`", advance(capture.unsubscribed)),
        (
            "`add_markets` is followed by a snapshot of the added market",
            "yes" if capture.snapshot_after_add else "no",
        ),
        (
            "Snapshot book not crossed, so `use_yes_price` is honoured",
            f"{uncrossed}: best bid {best_bid}, best ask {best_ask}",
        ),
        ("Bound on markets per subscription", bound),
    ]
    lines = [
        "## The order-book socket, as the demo host answers it",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py` from `{WS}` on "
        f"`{capture.first}`, with `{capture.second}` added to the subscription. "
        "Fixtures: `polyoxide-kalshi/tests/fixtures/predictions/ws/`.",
        "",
        *capture_common.provenance_table(("Question", "Observed"), rows),
    ]
    return "\n".join(lines)


def record_findings(text: str) -> None:
    """Writes `text` into OBSERVED.md between its markers, replacing an earlier run's, or
    directly above the anchor line when there is none."""
    current = OBSERVED.read_text(encoding="utf-8")
    block = f"{BEGIN}\n{text.rstrip()}\n{END}"
    if BEGIN in current and END in current:
        head, rest = current.split(BEGIN, 1)
        _, tail = rest.split(END, 1)
        current = head + block + tail
    else:
        current = current.replace(ANCHOR, f"{block}\n\n{ANCHOR}", 1)
    capture_common.write_raw(OBSERVED, current)


def write_fixtures(capture: Capture, stamp: str, seconds: float) -> None:
    files = [
        ("subscribed", capture.subscribed.frame, "the answer to the `subscribe`"),
        ("snapshot", capture.snapshot.frame, "the first `orderbook_snapshot` of the subscription"),
        (
            "deltas",
            [seen.frame for seen in capture.deltas],
            f"{len(capture.deltas)} `orderbook_delta` frames, as an array, in wire order",
        ),
        ("ok", capture.added.frame, "the `ok` answering `update_subscription` with `add_markets`"),
        ("unsubscribed", capture.unsubscribed.frame, "the `unsubscribed` answering `unsubscribe`"),
        ("error", capture.refused.frame, "the `error` answering a `subscribe` that names no market"),
    ]
    for name, value, _description in files:
        capture_common.write_json(OUT / f"{name}.json", value, ensure_ascii=True)
    capture_common.write_provenance(
        OUT,
        [
            "# Kalshi order-book socket fixtures",
            "",
            f"Captured {stamp} by `scripts/capture_kalshi_predictions_ws.py` from `{WS}` (the demo "
            "host), with the owner's demo key signing the handshake through "
            "`scripts/capture_kalshi_account.py`'s `signed_headers`. No key appears in a fixture: "
            "the handshake's headers are not frames.",
            "",
            f"Market `{capture.first}`; `{capture.second}` was added to the subscription. The "
            f"exchange: a `subscribe` (`orderbook_delta`, `use_yes_price: true`), up to "
            f"{seconds:.0f} s of frames (at most {MAX_DELTAS} deltas), an `update_subscription` "
            "`add_markets`, an `unsubscribe`, and a `subscribe` that names no market.",
            "",
            *capture_common.provenance_table(
                ("Fixture", "What it is"), [(f"`{name}.json`", what) for name, _value, what in files]
            ),
        ],
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Capture the order-book socket's demo frames.")
    parser.add_argument("--ticker", help="the demo market to subscribe (default: the first open one with a two-sided book)")
    parser.add_argument("--seconds", type=float, default=60.0, help="how long to read deltas (default 60)")
    args = parser.parse_args()
    require_key()
    require_anchor()
    first, second = choose(args.ticker)
    stamp = capture_common.stamp()
    try:
        capture = asyncio.run(run(first, second, args.seconds))
    except InvalidHandshake as err:
        raise SystemExit(
            f"the demo host refused the signed handshake ({err}): check the key id, and that the key is the demo environment's"
        ) from err
    except ConnectionClosed as err:
        raise SystemExit(f"the demo host closed the connection: {err}") from err
    write_fixtures(capture, stamp, args.seconds)
    record_findings(section(capture, stamp))
    print(f"captured 6 fixtures ({len(capture.deltas)} deltas) for {first} into {OUT}")
    print(f"recorded the findings in {OBSERVED}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it without the key to see it refuse**

Run: `nix develop --command env -u KALSHI_DEMO_API_KEY_ID -u KALSHI_DEMO_PRIVATE_KEY uv run scripts/capture_kalshi_predictions_ws.py`

Expected: it exits with status 1 before it connects, printing `KALSHI_DEMO_API_KEY_ID and KALSHI_DEMO_PRIVATE_KEY not set. This capture signs its handshake with the owner's demo key: stop here and ask the owner for it. No fixture is ever written by hand.` Nothing is written: `git status --short polyoxide-kalshi docs/specs/kalshi` shows no change.

- [ ] **Step 3: Run it with the owner's demo key**

If `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` are not in the session's environment, **stop the story here and ask the owner for them** (spec D19); do not continue to Task 6.2 and never write a fixture by hand. With them set:

Run: `nix develop --command uv run scripts/capture_kalshi_predictions_ws.py`

Expected, after up to a minute of reading: two lines, `captured 6 fixtures (<n> deltas) for <ticker> into …/polyoxide-kalshi/tests/fixtures/predictions/ws` and `recorded the findings in …/docs/specs/kalshi/OBSERVED.md`. Three refusals are possible, each a clear message and no write: `no orderbook_delta for <ticker> in 60 s` (a quiet market: rerun with `--ticker` of a busier one, or `--seconds 120`), `the snapshot of <ticker> is one-sided` (pass a market with both sides) and `the demo host refused the signed handshake` (the key is not the demo environment's).

- [ ] **Step 4: Check what it wrote**

Run: `ls polyoxide-kalshi/tests/fixtures/predictions/ws && grep -rn 'PRIVATE KEY' polyoxide-kalshi/tests/fixtures/predictions/ws; git diff docs/specs/kalshi/OBSERVED.md`

Expected: the `ls` lists `PROVENANCE.md deltas.json error.json ok.json snapshot.json subscribed.json unsubscribed.json`; the `grep` prints nothing; the diff adds one section between `<!-- kalshi-ws-capture:begin -->` and `<!-- kalshi-ws-capture:end -->` directly above `<!-- capture findings go above this line -->`, holding the six rows of the findings table. Read the table: if its "not crossed" row says `NO, crossed`, `use_yes_price` was not honoured and Task 6.3's `the_captured_snapshot_is_not_crossed` will fail, which is the finding; stop and report it. No Rust changed in this task, so the Rust gate waits for Task 6.2; run the checks that apply: `nix develop --command bash -c 'python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: no output and exit status 0.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/capture_kalshi_predictions_ws.py polyoxide-kalshi/tests/fixtures/predictions/ws
git commit -m "feat(kalshi): capture the order-book socket's frames from the demo host (Story 6)" -- scripts/capture_kalshi_predictions_ws.py polyoxide-kalshi/tests/fixtures/predictions/ws docs/specs/kalshi/OBSERVED.md
```

### Task 6.2: The `predictions-ws` feature, `PredictionsWsError` and the allowlist

**Files:**
- Create: `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt` (written by `--write`)
- Modify: `polyoxide-kalshi/Cargo.toml` (by Story 1's anchors, and the docs.rs list), `polyoxide-kalshi/src/lib.rs` (the `Classify` assertion), `polyoxide-kalshi/src/predictions/mod.rs` (the module block), `.github/scripts/tests/test_dep_allowlists.py` (the Binance row of `MUST_BUILD`), `CLAUDE.md` (regenerated), `Cargo.lock`
- Test: `polyoxide-kalshi/src/predictions/ws/error.rs`

`PredictionsWsError` is the socket's one error. It wraps the kit's `WsError` and takes its class from `polyoxide_venue::impl_ws_classification!`, expanded in this file so `test_classify_coverage.py` finds the table beside the type that wraps it. Kalshi's error frames are classed by code (spec §8.9), and every other variant has its own class. It also carries a `Credentials` variant for a handshake that could not be signed, which spec §8.9's list does not name; it is `InvalidRequest` like `CredentialsError` itself and stops the stream.

- [ ] **Step 1: Write the failing test**

Current (`polyoxide-kalshi/Cargo.toml`, the `[dependencies]` header line Story 1 wrote; it is a line of its own):

```toml
[dependencies]
```

Replace with:

```toml
# The order-book socket, `predictions::ws`, on the socket kit. It turns on `predictions`,
# and the runtime and macros the supervisor and the suites' `#[tokio::test]` need.
predictions-ws = ["predictions", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]

[dependencies]
```

Current (`polyoxide-kalshi/Cargo.toml`, the `[dev-dependencies]` header line Story 1 wrote):

```toml
[dev-dependencies]
```

Replace with:

```toml
# The socket kit (`predictions-ws`): connect, the supervisor and the socket error. It owns
# the transport, tokio-tungstenite and rustls, so this crate declares none of them.
polyoxide-ws = { workspace = true, optional = true }
futures-util = { workspace = true, optional = true }

[dev-dependencies]
```

Current (`polyoxide-kalshi/Cargo.toml`, the `mockito` line of `[dev-dependencies]`):

```toml
mockito = { workspace = true }
```

Replace with:

```toml
mockito = { workspace = true }
# The shared scripted WebSocket server the `predictions-ws` suites run on, with Kalshi's
# answer (`src/predictions/ws/scripted.rs`). Path-only, so cargo strips it on publish.
polyoxide-ws = { path = "../polyoxide-ws", features = ["test-server"] }
```

Current (`polyoxide-kalshi/Cargo.toml`, the file's last two lines, below any live tables Stories 2, 3 and 4 inserted):

```toml
[package.metadata.docs.rs]
features = ["predictions"]
```

Replace with:

```toml
[package.metadata.docs.rs]
features = ["predictions", "predictions-ws"]
```

The CI test of the docs.rs list fails while it lacks a feature the manifest declares, so the task that adds `predictions-ws` also lists it; Task 6.6 then only adds its live table above this one.

Current (`polyoxide-kalshi/src/predictions/mod.rs`, the module block as Story 4 leaves it):

```rust
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
```

Replace with:

```rust
pub mod api;
pub mod auth;
pub mod error;
pub mod key;
pub mod request;
mod throttle;
pub mod types;
pub mod venue;
#[cfg(feature = "predictions-ws")]
pub mod ws;

pub use auth::{Credentials, CredentialsError};
pub use error::{PredictionsError, VenueError};
pub use key::key;
```

Current (`polyoxide-kalshi/src/lib.rs`, the `Classify` assertion as Story 4 leaves it):

```rust
#[cfg(feature = "predictions")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<predictions::PredictionsError>();
    is::<predictions::VenueError>();
    is::<predictions::key::InvalidTicker>();
    is::<predictions::CredentialsError>();
};
```

Replace with:

```rust
#[cfg(feature = "predictions")]
const _: fn() = || {
    fn is<T: polyoxide_venue::Classify>() {}
    is::<predictions::PredictionsError>();
    is::<predictions::VenueError>();
    is::<predictions::key::InvalidTicker>();
    is::<predictions::CredentialsError>();
    #[cfg(feature = "predictions-ws")]
    is::<predictions::ws::PredictionsWsError>();
};
```

Create `polyoxide-kalshi/src/predictions/ws/mod.rs`:

```rust
//! The order-book socket of the predictions exchange.
//!
//! One connection to [`PRODUCTION_WS`] or [`DEMO_WS`] carries the `orderbook_delta`
//! channel: a snapshot of each subscribed market's book, then the changes to it, with
//! every price in YES pricing (the subscription asks for `use_yes_price`, so a NO bid is
//! reported as the YES ask it is). Kalshi refuses a handshake that carries no API-key
//! headers, public data included, so a socket is built from `Credentials`, and every
//! connection attempt signs its own handshake: a signature carries its timestamp, and a
//! reconnect that replayed an old one would be refused.
//!
//! The socket is run by the socket kit's `Supervisor`. It pings every 10 s, counts every
//! frame as liveness, reconnects after a drop, a stall or a gap in a subscription's
//! `seq`, and replays the whole membership as one `subscribe`. Every outage yields one
//! `Event::Disconnected` and one `Event::Reconnected`, and what the socket carried in
//! between is lost, so a consumer rebuilds its books from the fresh snapshots that
//! follow.
//!
//! Errors are `PredictionsWsError`, classified through the one socket table.

mod error;

pub use error::PredictionsWsError;

/// The production WebSocket endpoint.
pub const PRODUCTION_WS: &str = "wss://external-api-ws.kalshi.com/trade-api/ws/v2";

/// The demo WebSocket endpoint. Keys do not cross environments: a demo socket takes the
/// demo key.
pub const DEMO_WS: &str = "wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2";
```

Create `polyoxide-kalshi/src/predictions/ws/error.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_ws::tungstenite::{self, http};

    use super::*;

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> PredictionsWsError {
        PredictionsWsError::from(WsError::from(err))
    }

    /// An upgrade refused with `status`, and a `Retry-After` when one is given.
    fn refused(status: u16, retry_after: Option<&str>) -> PredictionsWsError {
        let mut response = http::Response::builder().status(status);
        if let Some(wait) = retry_after {
            response = response.header("Retry-After", wait);
        }
        transport(tungstenite::Error::Http(response.body(None).unwrap()))
    }

    fn server(code: u32) -> PredictionsWsError {
        PredictionsWsError::Server {
            code,
            message: "a message".to_owned(),
        }
    }

    #[test]
    fn a_server_error_is_classed_by_its_code() {
        let rows = [
            (9, Class::Unauthorized, false),
            // Kalshi's AsyncAPI calls codes 10 and 25 terminal ("the user must
            // resubscribe"): both are the socket's to retry.
            (
                10,
                Class::Unavailable {
                    code: Some("10".into()),
                },
                true,
            ),
            (
                25,
                Class::Unavailable {
                    code: Some("25".into()),
                },
                true,
            ),
            (27, Class::RateLimited { retry_after: None }, true),
            (7, Class::InvalidRequest, false),
            (11, Class::InvalidRequest, false),
            (26, Class::InvalidRequest, false),
        ];
        for (code, class, retriable) in rows {
            let err = server(code);
            assert_eq!(err.class(), class, "{code}");
            assert_eq!(err.is_retriable(), retriable, "{code}");
            assert!(err.is_fault(), "{code}");
            assert_eq!(err.retry_after(), None, "{code}");
        }
    }

    #[test]
    fn a_sequence_gap_is_a_retriable_network_fault() {
        let err = PredictionsWsError::SequenceGap {
            sid: 1,
            expected: 2,
            received: 4,
        };
        assert_eq!(err.class(), Class::Network);
        assert!(err.is_retriable() && err.is_fault());
        assert_eq!(
            err.to_string(),
            "subscription 1 skipped from sequence 2 to 4"
        );
    }

    #[test]
    fn a_frame_that_did_not_decode_is_a_decode_error() {
        let err = PredictionsWsError::Json {
            text: "x".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        assert_eq!(err.class(), Class::Decode);
        assert!(!err.is_retriable());
        assert!(
            err.to_string().starts_with("a frame did not decode: "),
            "{err}"
        );
    }

    #[test]
    fn the_callers_own_mistakes_are_invalid_requests() {
        let url = PredictionsWsError::InvalidUrl {
            url: "http://x".to_owned(),
            reason: "it must be wss://, or ws:// to a loopback host",
        };
        let signing = PredictionsWsError::Credentials(CredentialsError::Sign {
            reason: "x".to_owned(),
        });
        assert!(
            signing
                .to_string()
                .starts_with("could not sign the handshake: "),
            "{signing}"
        );
        for err in [url, signing] {
            assert_eq!(err.class(), Class::InvalidRequest, "{err}");
            assert!(!err.is_retriable(), "{err}");
            assert_eq!(err.retry_after(), None, "{err}");
        }
    }

    #[test]
    fn a_refused_upgrade_follows_the_socket_table() {
        for (status, class) in [
            (401, Class::Unauthorized),
            (404, Class::VenueRefusal { code: None }),
            (429, Class::RateLimited { retry_after: None }),
            (503, Class::Unavailable { code: None }),
            (418, Class::Restricted),
            (451, Class::Restricted),
        ] {
            assert_eq!(refused(status, None).class(), class, "{status}");
        }
        // Only a region block is not a fault; a ban the client earned is one.
        assert!(!refused(451, None).is_fault());
        assert!(refused(418, None).is_fault());
    }

    #[test]
    fn a_refusals_retry_after_is_surfaced() {
        // As HTTP's errors do for every status; only a 429's class carries the wait.
        let wait = Some(Duration::from_secs(30));
        assert_eq!(
            refused(429, Some("30")).class(),
            Class::RateLimited { retry_after: wait }
        );
        assert_eq!(refused(429, Some("30")).retry_after(), wait);
        assert_eq!(
            refused(503, Some("30")).class(),
            Class::Unavailable { code: None }
        );
        assert_eq!(refused(503, Some("30")).retry_after(), wait);
        // A zero is no wait at all.
        assert_eq!(refused(429, Some("0")).retry_after(), None);
    }

    #[test]
    fn a_server_close_follows_the_close_code_table() {
        for (code, class) in [
            (Some(1000), Class::Network),
            (Some(1001), Class::Network),
            (Some(1006), Class::Network),
            (Some(1011), Class::Network),
            (None, Class::Network),
            (
                Some(1008),
                Class::VenueRefusal {
                    code: Some("1008".into()),
                },
            ),
        ] {
            let err = PredictionsWsError::from(WsError::closed(code, "a reason"));
            assert_eq!(err.class(), class, "{code:?}");
        }
    }

    #[test]
    fn the_transport_is_the_kits_table() {
        let rows = [
            (tungstenite::Error::ConnectionClosed, Class::Network),
            (
                tungstenite::Error::Io(std::io::ErrorKind::ConnectionReset.into()),
                Class::Network,
            ),
            (tungstenite::Error::AlreadyClosed, Class::InvalidRequest),
            (
                tungstenite::Error::Url(tungstenite::error::UrlError::NoHostName),
                Class::InvalidRequest,
            ),
        ];
        for (err, class) in rows {
            let err = transport(err);
            assert_eq!(err.class(), class, "{err:?}");
            assert!(err.is_fault(), "{err:?}");
        }
        for err in [
            WsError::connect_timeout(Duration::from_secs(10)),
            WsError::no_answer(Duration::from_secs(1)),
        ] {
            let err = PredictionsWsError::from(err);
            assert_eq!(err.class(), Class::Network, "{err:?}");
        }
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --lib ws::error`

Expected: it does not compile. `error[E0432]: unresolved import `error::PredictionsWsError`` in `ws/mod.rs` (the file holds only its tests), and `cannot find type `PredictionsWsError` in this scope` in the tests.

- [ ] **Step 3: Implement**

Step 2's cargo run also brought `Cargo.lock` up to date with the manifest, which the allowlist run below needs (it passes `--locked`).

Insert at the top of `polyoxide-kalshi/src/predictions/ws/error.rs`, directly above the line `#[cfg(test)]`:

```rust
//! The order-book socket's error, and its classes.

use std::time::Duration;

use polyoxide_venue::{Class, Classify};
use polyoxide_ws::WsError;
use thiserror::Error;

use crate::predictions::CredentialsError;

polyoxide_venue::impl_ws_classification!();

/// The error of the supervised order-book socket and of its membership calls.
///
/// Every socket failure arrives as the kit's [`WsError`](polyoxide_ws::WsError) first,
/// and is classed by the one socket table. Kalshi's error frames are classed by code.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PredictionsWsError {
    /// The connection could not be opened, or failed mid-stream: the kit's one socket
    /// error, read through its `Kind`. A refused upgrade, a server close and a connect
    /// timeout are all this.
    #[error("WebSocket transport error: {0}")]
    Connection(#[from] WsError),

    /// Kalshi sent an error frame: it answered a command with one, or reported a
    /// problem on a subscription. Its codes run from 1 to 28.
    #[error("Kalshi sent error {code}: {message}")]
    Server {
        /// Kalshi's error code.
        code: u32,
        /// Kalshi's message.
        message: String,
    },

    /// A frame carried a sequence number other than the one after the last of its
    /// subscription: a message was lost, so the book it builds cannot be trusted. The
    /// socket reconnects and takes fresh snapshots.
    #[error("subscription {sid} skipped from sequence {expected} to {received}")]
    SequenceGap {
        /// The subscription.
        sid: u64,
        /// The sequence number that should have come.
        expected: u64,
        /// The one that did.
        received: u64,
    },

    /// A frame did not decode: it is not JSON, or it is a frame this socket knows with a
    /// field it cannot read. The connection stays.
    #[error("a frame did not decode: {source}")]
    Json {
        /// The frame's text.
        text: String,
        /// Why it did not decode.
        #[source]
        source: serde_json::Error,
    },

    /// The socket's URL is neither `wss://` nor `ws://` to a loopback host, so the
    /// signed handshake would not travel encrypted to a remote host.
    #[error("invalid WebSocket URL {url}: {reason}")]
    InvalidUrl {
        /// The URL as given.
        url: String,
        /// Why it was refused.
        reason: &'static str,
    },

    /// The handshake could not be signed.
    #[error("could not sign the handshake: {0}")]
    Credentials(#[from] CredentialsError),
}

/// The transport by the one socket table (`polyoxide_venue::impl_ws_classification!`),
/// whose only non-fault is an upgrade refused with 451 and whose wait is a refusal's
/// `Retry-After`. Kalshi's error frames are classed by code: 9 (authentication
/// required) is `Unauthorized`, 27 (the command rate limit) `RateLimited`, 10 (a channel
/// error) and 25 (the subscription's buffer overflowed) `Unavailable`, and every other
/// code `InvalidRequest`. Kalshi's AsyncAPI calls codes 10 and 25 terminal ("the user
/// must resubscribe"), so both are retriable: the socket reconnects and its replay
/// resubscribes. A gap in the sequence is a lost message, `Network` and a fault; a
/// frame that did not decode is `Decode`; a URL or a key that cannot be used is
/// `InvalidRequest`.
impl Classify for PredictionsWsError {
    fn class(&self) -> Class {
        match self {
            Self::Connection(err) => ws_class(err),
            Self::Server { code, .. } => match code {
                9 => Class::Unauthorized,
                10 | 25 => Class::Unavailable {
                    code: Some(code.to_string()),
                },
                27 => Class::RateLimited { retry_after: None },
                _ => Class::InvalidRequest,
            },
            Self::SequenceGap { .. } => Class::Network,
            Self::Json { .. } => Class::Decode,
            Self::InvalidUrl { .. } => Class::InvalidRequest,
            Self::Credentials(err) => err.class(),
        }
    }

    fn is_fault(&self) -> bool {
        !matches!(self, Self::Connection(err) if !ws_is_fault(err))
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Connection(err) => ws_retry_after(err),
            _ => self.class().retry_after(),
        }
    }
}

```

Run: `nix develop --command cargo fmt --all`

Then create the allowlist, which `--write` fills in from the tree cargo reports, and add its `MUST_BUILD` row.

Run: `touch ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt && nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write' && git status --short ci/dep-allowlists`

Expected: `git status` lists only `?? ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt` (and nothing else under `ci/dep-allowlists`; if another file changed, stop: the tree moved under the allowlists, and the commit must say why). The new file lists `polyoxide-ws`, `tokio-tungstenite`, `tungstenite` and the rest of the socket stack on top of what `polyoxide-kalshi@predictions.txt` lists.

Current (`.github/scripts/tests/test_dep_allowlists.py`, the Binance row of `MUST_BUILD`):

```python
    "polyoxide-binance@usdm-ws.txt": "polyoxide-ws",
```

Replace with:

```python
    "polyoxide-binance@usdm-ws.txt": "polyoxide-ws",
    "polyoxide-kalshi@predictions-ws.txt": "polyoxide-ws",
```

Regenerate the registry regions, which now list the crate's `predictions-ws` dependency:

Run: `nix develop --command python3 scripts/gen_registry.py --write && git status --short CLAUDE.md README.md docs/specs/INDEX.md`

Expected: ` M CLAUDE.md` (the crate graph region), and no other file of the three.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --lib ws::error`

Expected: `test result: ok. 8 passed; 0 failed`.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_ci_workflow.py -q'`

Expected: all pass, the `polyoxide-kalshi@predictions-ws` parameters among them (the `MUST_BUILD` row finds `polyoxide-ws` in the file, so the socket rule cannot pass vacuously, and the coverage sweep finds `PredictionsWsError` in the assertion and its file expanding the table and calling `ws_class`, `ws_is_fault` and `ws_retry_after`), and `test_docs_rs_builds_every_venue_feature_but_test_server` finds `predictions-ws` in the docs.rs list. Without the docs.rs edit above it fails with `polyoxide-kalshi` as its message, the two sorted lists differing by `predictions-ws`.

Then the story gate: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && cargo test -p polyoxide-kalshi --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: exit status 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/ws ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt
git commit -m "feat(kalshi): the predictions-ws feature and PredictionsWsError (Story 6)" -m "ci/dep-allowlists: the socket stack (polyoxide-ws, tokio-tungstenite, tungstenite and what they build) joins polyoxide-kalshi's tree under predictions-ws; the socket rule keeps it out of predictions alone, and the MUST_BUILD row stops the rule passing vacuously." -- Cargo.lock polyoxide-kalshi/Cargo.toml polyoxide-kalshi/src/lib.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/ws ci/dep-allowlists/polyoxide-kalshi@predictions-ws.txt .github/scripts/tests/test_dep_allowlists.py CLAUDE.md
```

### Task 6.3: The frames, the events and their wire agreement

**Files:**
- Create: `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`
- Modify: `polyoxide-kalshi/src/predictions/ws/mod.rs` (the module block), `polyoxide-kalshi/Cargo.toml` (a `[[test]]` stanza, before Story 1's registration anchor)
- Test: `polyoxide-kalshi/src/predictions/ws/event.rs`, `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`

`event.rs` holds what the stream yields (`Event`, `OrderbookSnapshot`, `OrderbookDelta`, `DisconnectReason`) and, in a doc-hidden `frames` module, the frames Kalshi sends them in, typed as the wire writes them. The public events are the converted shape: a snapshot's bids are the `yes_dollars_fp` levels highest first and its asks the `no_dollars_fp` levels, already in YES pricing, lowest first; a delta's `side` `yes` is a bid and `no` an ask. The wire agreement decodes each captured frame into its `frames` type, encodes it back and compares key paths and values both ways, so an unmodelled key fails it. A frame type whose documented fields differ from the capture is fixed here; the AsyncAPI's `msg` shapes are the starting point, every optional key an `Option` that is not written when absent.

- [ ] **Step 1: Write the failing test**

Current (`polyoxide-kalshi/src/predictions/ws/mod.rs`, the module block Task 6.2 wrote):

```rust
mod error;

pub use error::PredictionsWsError;
```

Replace with:

```rust
mod error;
mod event;

pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot};
```

Current (`polyoxide-kalshi/Cargo.toml`, the registration anchor Story 1 wrote, one line directly above `[package.metadata.polyoxide]`):

```toml
# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Replace with:

```toml
[[test]]
name = "wire_agreement_predictions_ws"
required-features = ["predictions-ws"]

# Registration, read by scripts/gen_registry.py; run it with --write after an edit.
```

Create `polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs`:

```rust
//! Agreement between the socket's frame types and the frames captured from the demo
//! host. Provenance is in `tests/fixtures/predictions/ws/PROVENANCE.md`.
//!
//! The AsyncAPI mirror is the starting point for the types, not the oracle: the check is
//! against the wire. Every fixture decodes into its type, re-serialises with the same key
//! paths (a key the wire sent and the type does not model fails, and so does one the type
//! writes and the wire did not send), and every scalar compares equal.

use polyoxide_kalshi::predictions::ws::frames::{
    Delta, Rejected, Snapshot, Subscribed, Unsubscribed, Updated,
};
use polyoxide_test_support::{agreement, fixtures, Fixtures};
use rust_decimal::Decimal;
use serde::{de::DeserializeOwned, Serialize};

fn ws() -> Fixtures {
    fixtures!("predictions/ws")
}

fn assert_agrees<T: DeserializeOwned + Serialize>(what: &str, text: &str) {
    let disagreement = agreement::compare::<T>(what, text);
    assert!(
        disagreement.unmodelled.is_empty(),
        "{what}: the wire sent keys the type does not model: {:?}",
        disagreement.unmodelled
    );
    assert!(
        disagreement.invented.is_empty(),
        "{what}: the type writes keys the wire did not send: {:?}",
        disagreement.invented
    );
}

#[test]
fn the_subscribed_fixture_agrees() {
    assert_agrees::<Subscribed>("subscribed", &ws().text("subscribed"));
}

#[test]
fn the_snapshot_fixture_agrees() {
    assert_agrees::<Snapshot>("snapshot", &ws().text("snapshot"));
}

#[test]
fn the_ok_fixture_agrees() {
    assert_agrees::<Updated>("ok", &ws().text("ok"));
}

#[test]
fn the_unsubscribed_fixture_agrees() {
    assert_agrees::<Unsubscribed>("unsubscribed", &ws().text("unsubscribed"));
}

#[test]
fn the_error_fixture_agrees() {
    assert_agrees::<Rejected>("error", &ws().text("error"));
}

#[test]
fn every_captured_delta_agrees() {
    let deltas = ws().json("deltas");
    let Some(frames) = deltas.as_array() else {
        panic!("deltas.json is not an array");
    };
    assert!(
        !frames.is_empty(),
        "a capture with no delta proves nothing about the delta type"
    );
    for (n, frame) in frames.iter().enumerate() {
        assert_agrees::<Delta>(&format!("delta {n}"), &frame.to_string());
    }
}

#[test]
fn the_captured_snapshot_is_not_crossed() {
    // `use_yes_price` puts both sides on one scale. A book crossed here would mean the
    // flag was not honoured and the asks are still in NO pricing.
    let snapshot: Snapshot = serde_json::from_str(&ws().text("snapshot")).unwrap();
    let bids: Vec<Decimal> = snapshot
        .msg
        .yes_dollars_fp
        .iter()
        .map(|level| level.price)
        .collect();
    let asks: Vec<Decimal> = snapshot
        .msg
        .no_dollars_fp
        .iter()
        .map(|level| level.price)
        .collect();
    assert!(
        !bids.is_empty() && !asks.is_empty(),
        "a one-sided snapshot cannot show whether the book is crossed"
    );
    let (best_bid, best_ask) = (bids.iter().max().unwrap(), asks.iter().min().unwrap());
    assert!(
        best_bid < best_ask,
        "the best bid {best_bid} is not below the best ask {best_ask}"
    );
}

#[test]
fn sequence_numbers_count_by_one_on_the_wire() {
    // The premise of the socket's gap check, pinned to the capture: after the snapshot,
    // each delta of the subscription is the previous number plus one.
    let snapshot: Snapshot = serde_json::from_str(&ws().text("snapshot")).unwrap();
    let deltas: Vec<Delta> = serde_json::from_str(&ws().text("deltas")).unwrap();
    let mut expected = snapshot.seq;
    for delta in &deltas {
        expected += 1;
        assert_eq!(
            (delta.sid, delta.seq),
            (snapshot.sid, expected),
            "{delta:?}"
        );
    }
}

#[test]
fn the_answers_carry_the_ids_and_sequences_the_protocol_reads() {
    // The correlation matches an answer to its command by `id`, and an `ok` or an
    // `unsubscribed` counts in its subscription's sequence by `sid` and `seq`.
    let subscribed: Subscribed = serde_json::from_str(&ws().text("subscribed")).unwrap();
    assert!(subscribed.id.is_some(), "{subscribed:?}");
    let ok: Updated = serde_json::from_str(&ws().text("ok")).unwrap();
    assert!(
        ok.id.is_some() && ok.sid.is_some() && ok.seq.is_some(),
        "{ok:?}"
    );
    let unsubscribed: Unsubscribed = serde_json::from_str(&ws().text("unsubscribed")).unwrap();
    assert!(unsubscribed.id.is_some(), "{unsubscribed:?}");
    let rejected: Rejected = serde_json::from_str(&ws().text("error")).unwrap();
    assert!(rejected.id.is_some(), "{rejected:?}");
}
```

Create `polyoxide-kalshi/src/predictions/ws/event.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // The mirror's own example (`docs/specs/kalshi/asyncapi.yaml`, version 2.0.0), as the
    // `msg` of a snapshot, in the order Kalshi lists a side: ascending price.
    const SNAPSHOT_MSG: &str = r#"{"market_ticker":"FED-23DEC-T3.00","market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","yes_dollars_fp":[["0.0800","300.00"],["0.2200","333.00"]],"no_dollars_fp":[["0.5400","20.00"],["0.5600","146.00"]]}"#;

    fn level(price: &str, size: &str) -> Level {
        Level::new(price.parse().unwrap(), size.parse().unwrap())
    }

    fn snapshot(msg: &str) -> OrderbookSnapshot {
        OrderbookSnapshot::from(serde_json::from_str::<frames::SnapshotMsg>(msg).unwrap())
    }

    fn delta(msg: &str) -> OrderbookDelta {
        OrderbookDelta::from(serde_json::from_str::<frames::DeltaMsg>(msg).unwrap())
    }

    #[test]
    fn a_snapshot_is_the_yes_book_best_first() {
        let book = snapshot(SNAPSHOT_MSG);
        assert_eq!(book.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(book.market_id, "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1");
        // Bids highest first; asks, the NO side in YES pricing, lowest first.
        assert_eq!(
            book.bids,
            [level("0.2200", "333.00"), level("0.0800", "300.00")]
        );
        assert_eq!(
            book.asks,
            [level("0.5400", "20.00"), level("0.5600", "146.00")]
        );
    }

    #[test]
    fn a_book_sent_the_other_way_round_is_sorted_the_same() {
        let reversed = r#"{"market_ticker":"A-1","market_id":"id","yes_dollars_fp":[["0.2200","333.00"],["0.0800","300.00"]],"no_dollars_fp":[["0.5600","146.00"],["0.5400","20.00"]]}"#;
        let book = snapshot(reversed);
        assert_eq!(
            book.bids,
            [level("0.2200", "333.00"), level("0.0800", "300.00")]
        );
        assert_eq!(
            book.asks,
            [level("0.5400", "20.00"), level("0.5600", "146.00")]
        );
    }

    #[test]
    fn a_side_with_no_offers_is_an_empty_side() {
        // Kalshi leaves the key out when a side has no offers.
        let one_sided =
            r#"{"market_ticker":"A-1","market_id":"id","yes_dollars_fp":[["0.2200","333.00"]]}"#;
        let book = snapshot(one_sided);
        assert_eq!(book.bids, [level("0.2200", "333.00")]);
        assert!(book.asks.is_empty());
    }

    #[test]
    fn a_yes_delta_is_a_bid_and_a_no_delta_an_ask_in_yes_pricing() {
        // The mirror's example delta: a YES level losing 54 contracts at 0.96.
        let yes = r#"{"market_ticker":"FED-23DEC-T3.00","market_id":"id","price_dollars":"0.9600","delta_fp":"-54.00","side":"yes","ts":"2022-11-22T20:44:01Z","ts_ms":1669149841000}"#;
        let bid = delta(yes);
        assert_eq!(bid.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(bid.side, BookSide::Bid);
        assert_eq!(bid.price, "0.9600".parse::<Decimal>().unwrap());
        assert_eq!(bid.delta, "-54.00".parse::<Decimal>().unwrap());
        assert_eq!(bid.ts, Some(UnixMillis(1_669_149_841_000)));
        // `use_yes_price` reports the NO side at its YES price, so no flip is applied.
        let no = r#"{"market_ticker":"A-1","market_id":"id","price_dollars":"0.4000","delta_fp":"12.00","side":"no"}"#;
        let ask = delta(no);
        assert_eq!(ask.side, BookSide::Ask);
        assert_eq!(ask.price, "0.4000".parse::<Decimal>().unwrap());
        assert_eq!(ask.delta, "12.00".parse::<Decimal>().unwrap());
        assert_eq!(ask.ts, None);
    }

    #[test]
    fn a_deltas_optional_fields_are_kept_when_present() {
        let ours = r#"{"market_ticker":"A-1","market_id":"id","price_dollars":"0.5000","delta_fp":"1.00","side":"yes","client_order_id":"mine","subaccount":2}"#;
        assert_eq!(delta(ours).client_order_id.as_deref(), Some("mine"));
    }

    #[test]
    fn an_unknown_side_does_not_decode() {
        let sideways = r#"{"market_ticker":"A-1","market_id":"id","price_dollars":"0.5000","delta_fp":"1.00","side":"maybe"}"#;
        assert!(serde_json::from_str::<frames::DeltaMsg>(sideways).is_err());
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --test wire_agreement_predictions_ws`

Expected: it does not compile: `error[E0432]: unresolved imports `event::DisconnectReason`, `event::Event`, `event::OrderbookDelta`, `event::OrderbookSnapshot`, `event::frames`` in `ws/mod.rs`.

- [ ] **Step 3: Implement**

Insert at the top of `polyoxide-kalshi/src/predictions/ws/event.rs`, directly above the line `#[cfg(test)]`:

```rust
//! What the supervised socket yields, and the frames Kalshi sends it in.

use std::time::Duration;

use polyoxide_venue::{Level, UnixMillis};
use polyoxide_ws::Outage;
use rust_decimal::Decimal;

use crate::predictions::{
    types::{BookSide, PriceLevel},
    ws::PredictionsWsError,
};

/// What a supervised order-book socket yields.
///
/// Every price is in YES pricing, so a book is one ladder: the bids are YES offers to buy
/// and the asks are the NO bids, priced as the YES offers to sell they are.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// A market's whole book: the first frame of its subscription, and the first after
    /// each reconnect.
    Snapshot(OrderbookSnapshot),
    /// One change to one price level of a book.
    Delta(OrderbookDelta),
    /// The connection went down. One per outage; [`Reconnected`](Self::Reconnected)
    /// follows once the subscription has been replayed, unless the stream ends on an
    /// error it cannot recover from.
    Disconnected {
        /// Why the connection went down.
        reason: DisconnectReason,
    },
    /// The connection is back and the subscription replayed. A fresh snapshot of every
    /// market follows, and what the socket carried in between is lost, so a consumer
    /// rebuilds its books from those.
    Reconnected,
}

/// A market's whole book, in YES pricing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderbookSnapshot {
    /// The market's ticker.
    pub market_ticker: String,
    /// The market's id.
    pub market_id: String,
    /// The YES bids, highest price first.
    pub bids: Vec<Level>,
    /// The YES asks, lowest price first: the NO bids, in YES pricing.
    pub asks: Vec<Level>,
}

/// One change to one price level of a book, in YES pricing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderbookDelta {
    /// The market's ticker.
    pub market_ticker: String,
    /// The side of the ladder: a YES level is a bid, and a NO level, in YES pricing, an
    /// ask.
    pub side: BookSide,
    /// The level's price.
    pub price: Decimal,
    /// The change to the size resting there: negative when contracts left.
    pub delta: Decimal,
    /// When Kalshi recorded the change, when it said.
    pub ts: Option<UnixMillis>,
    /// The `client_order_id` of the account's own order that caused the change, when it
    /// did.
    pub client_order_id: Option<String>,
}

/// Why a connection went down, in a `Disconnected` event: the kit's `Outage`.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed the connection, with the code and reason it sent, or ended the
    /// stream with neither.
    Closed {
        /// The close code, when a close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A failure: a read, a ping, or a frame that recovery sent to a reconnect, such as a
    /// gap in a subscription's sequence.
    Error(PredictionsWsError),
    /// Nothing arrived for the staleness window, or a membership change went unanswered
    /// for it.
    Stale {
        /// The silence.
        silent: Duration,
    },
    /// Replaced on purpose at the connection's maximum age. This socket never rotates.
    Rotation,
}

impl From<Outage<PredictionsWsError>> for DisconnectReason {
    fn from(outage: Outage<PredictionsWsError>) -> Self {
        match outage {
            Outage::Closed { code, reason } => Self::Closed { code, reason },
            Outage::Error(err) => Self::Error(err),
            Outage::Stale { silent } => Self::Stale { silent },
            Outage::Rotation => Self::Rotation,
            // An outage a later kit adds reads as a stall of no length.
            _ => Self::Stale {
                silent: Duration::ZERO,
            },
        }
    }
}

fn levels(wire: Vec<PriceLevel>) -> Vec<Level> {
    wire.into_iter()
        .map(|level| Level::new(level.price, level.count))
        .collect()
}

impl From<frames::SnapshotMsg> for OrderbookSnapshot {
    fn from(msg: frames::SnapshotMsg) -> Self {
        let mut bids = levels(msg.yes_dollars_fp);
        bids.sort_by_key(|level| std::cmp::Reverse(level.price));
        let mut asks = levels(msg.no_dollars_fp);
        asks.sort_by_key(|level| level.price);
        Self {
            market_ticker: msg.market_ticker,
            market_id: msg.market_id,
            bids,
            asks,
        }
    }
}

impl From<frames::DeltaMsg> for OrderbookDelta {
    fn from(msg: frames::DeltaMsg) -> Self {
        let side = match msg.side {
            frames::WireSide::Yes => BookSide::Bid,
            frames::WireSide::No => BookSide::Ask,
        };
        Self {
            market_ticker: msg.market_ticker,
            side,
            price: msg.price_dollars,
            delta: msg.delta_fp,
            ts: msg.ts_ms.map(UnixMillis),
            client_order_id: msg.client_order_id,
        }
    }
}

/// The frames Kalshi sends, as the wire writes them, for the protocol's decode and for
/// the wire-agreement and live suites. Not API.
///
/// Each type is the AsyncAPI's schema (version 2.0.0) read against the captured frames in
/// `tests/fixtures/predictions/ws/`. Every optional key is an `Option` that is not written
/// when absent, so a frame round-trips with exactly the keys it came with, and a key the
/// types do not model shows up as unmodelled.
#[doc(hidden)]
pub mod frames {
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    use crate::predictions::types::PriceLevel;

    /// `subscribed`: the answer to a `subscribe`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Subscribed {
        /// Always `subscribed`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the `subscribe` it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The channel and the new subscription's id.
        pub msg: SubscribedMsg,
    }

    /// The body of `subscribed`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct SubscribedMsg {
        /// The channel subscribed.
        pub channel: String,
        /// The subscription's id.
        pub sid: u64,
    }

    /// `ok`: the answer to an `update_subscription`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Updated {
        /// Always `ok`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the command it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sid: Option<u64>,
        /// Its sequence number.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub seq: Option<u64>,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The markets the subscription holds after the update.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub msg: Option<UpdatedMsg>,
    }

    /// The body of `ok`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct UpdatedMsg {
        /// The tickers after the update.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub market_tickers: Option<Vec<String>>,
        /// The market ids after the update.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub market_ids: Option<Vec<String>>,
    }

    /// `unsubscribed`: the answer to an `unsubscribe`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Unsubscribed {
        /// Always `unsubscribed`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the `unsubscribe` it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription that ended.
        pub sid: u64,
        /// Its last sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
    }

    /// `error`: a command Kalshi refused, or a problem on a subscription.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Rejected {
        /// Always `error`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the command it answers; none for a problem on a subscription.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription, when the error is scoped to one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sid: Option<u64>,
        /// Its sequence number, on a scoped error.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub seq: Option<u64>,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The code and message.
        pub msg: RejectedMsg,
    }

    /// The body of `error`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct RejectedMsg {
        /// Kalshi's error code.
        pub code: u32,
        /// Kalshi's message.
        pub msg: String,
    }

    /// `orderbook_snapshot`: a market's whole book.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Snapshot {
        /// Always `orderbook_snapshot`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The command it answers, for a snapshot that was asked for.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription.
        pub sid: u64,
        /// Its sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The book.
        pub msg: SnapshotMsg,
    }

    /// The body of `orderbook_snapshot`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct SnapshotMsg {
        /// The market's ticker.
        pub market_ticker: String,
        /// The market's id.
        pub market_id: String,
        /// The YES levels; the key is absent when the side has no offers.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub yes_dollars_fp: Vec<PriceLevel>,
        /// The NO levels, in YES pricing under `use_yes_price`; absent when the side has
        /// no offers.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub no_dollars_fp: Vec<PriceLevel>,
    }

    /// `orderbook_delta`: one change to one price level.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Delta {
        /// Always `orderbook_delta`.
        #[serde(rename = "type")]
        pub kind: String,
        /// Not documented for a delta; kept if Kalshi sends one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription.
        pub sid: u64,
        /// Its sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The change.
        pub msg: DeltaMsg,
    }

    /// The side of a delta as the wire spells it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum WireSide {
        /// A YES level: a bid.
        Yes,
        /// A NO level: an ask, when `use_yes_price` puts it in YES pricing.
        No,
    }

    /// The body of `orderbook_delta`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct DeltaMsg {
        /// The market's ticker.
        pub market_ticker: String,
        /// The market's id.
        pub market_id: String,
        /// The level's price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price_dollars: Decimal,
        /// The change to the size resting there, signed.
        #[serde(with = "rust_decimal::serde::str")]
        pub delta_fp: Decimal,
        /// The side.
        pub side: WireSide,
        /// The account's own order that caused the change, when it did.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_order_id: Option<String>,
        /// Deprecated: the time as RFC 3339. `ts_ms` replaces it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ts: Option<String>,
        /// When Kalshi recorded the change, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ts_ms: Option<u64>,
        /// The subaccount of the account's own order that caused the change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subaccount: Option<u64>,
    }
}

```

Run: `nix develop --command cargo fmt --all`

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --lib ws::event`

Expected: `test result: ok. 6 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --test wire_agreement_predictions_ws`

Expected: `test result: ok. 9 passed; 0 failed`. If `assert_agrees` names an unmodelled key, the capture carries a key the AsyncAPI does not document: add it to the right `frames` type as an `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]`, add a line about it to `docs/specs/kalshi/OBSERVED.md` in the capture's section, and run again. If `the_captured_snapshot_is_not_crossed` or `sequence_numbers_count_by_one_on_the_wire` fails, that is the finding (`use_yes_price` not honoured, or `seq` not counting by one): stop and report it, since the protocol's gap check depends on the second.

Then the story gate: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && cargo test -p polyoxide-kalshi --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: exit status 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/ws/event.rs polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs
git commit -m "feat(kalshi): the order-book socket's frames and events, held to the capture (Story 6)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/src/predictions/ws polyoxide-kalshi/tests/wire_agreement_predictions_ws.rs
```

### Task 6.4: The protocol and its builder

**Files:**
- Create: `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`
- Modify: `polyoxide-kalshi/src/predictions/ws/mod.rs` (the module block, and the alias beside the constants)
- Test: `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`

`PredictionsProtocol` is Kalshi's socket as the kit's `Supervisor` drives it. Its divergence rows: the ping is an empty protocol Ping on the wall clock; every frame is liveness, the server's own Pings and the Pongs to the client's included; a connection that decoded a book frame delivered; `Disconnected` and `Reconnected` mark each outage; a connection used after it closed reconnects. Its membership is a set of tickers. `signed_request` builds the handshake for one attempt and signs it over the URL's path (the same signer as REST), so the supervisor, which calls `handshake` for every attempt, signs afresh each time; the socket connects only through the kit's `connect`, which `Protocol::connect` calls by default.

The sequence check is the part to read twice. Kalshi counts `seq` per subscription over snapshots, deltas, `ok`, `unsubscribed` and subscription-scoped errors. The kit's `request` reads frames while it waits for an answer, returns the answer, and keeps every other frame for the stream, so the answer never reaches `decode`: the correlation closure therefore observes every sequenced frame it reads, answers and kept pushes alike, in wire order, and counts each push it keeps in `Sequences::ahead` so that `decode`, which meets the kept push later, does not observe it a second time. A gap the closure finds cannot be returned from the request that found it (the request is for another command), so it is held in `Sequences::gap` and raised by the next frame `decode` sees, whatever its kind.

- [ ] **Step 1: Write the failing test**

Current (`polyoxide-kalshi/src/predictions/ws/mod.rs`, the module block as Task 6.3 left it):

```rust
mod error;
mod event;

pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot};
```

Replace with:

```rust
mod builder;
mod error;
mod event;
mod protocol;

pub use builder::PredictionsWsBuilder;
pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot};
#[doc(hidden)]
pub use protocol::signed_request;
pub use protocol::{Change, PredictionsMembership, PredictionsProtocol};
```

Insert directly above the line `/// The production WebSocket endpoint.` in `polyoxide-kalshi/src/predictions/ws/mod.rs`:

```rust
/// A supervised connection: a `Stream` of [`Event`]s that survives drops, stalls and gaps
/// by reconnecting and replaying its subscription, run by the kit's `Supervisor` on
/// [`PredictionsProtocol`]. Build one with [`PredictionsWsBuilder`].
///
/// It pings every 10 s on the wall clock, even while the consumer is behind; the queue
/// holds 1024 unread events, after which the task reads nothing more until the consumer
/// catches up, and that wait does not count as silence. It ends after an error it cannot
/// recover from (a handshake Kalshi refuses with a 401 or 403, or an error frame with code
/// 9), which it yields first; a frame that did not decode is yielded as an error and does
/// not end it. Change its markets through `membership()` and [`PredictionsMembership`],
/// from a task other than the one reading the stream.
pub type SupervisedPredictionsWs = polyoxide_ws::Supervised<PredictionsProtocol>;

```

Create `polyoxide-kalshi/src/predictions/ws/builder.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use polyoxide_venue::{Class, Classify};

    use super::*;
    use crate::predictions::ws::DEMO_WS;

    #[test]
    fn defaults_match_kalshis_cadences() {
        let builder = PredictionsWsBuilder::new();
        assert_eq!(builder.url, PRODUCTION_WS);
        assert_eq!(builder.timing.ping_interval, Some(Duration::from_secs(10)));
        assert_eq!(builder.timing.stale_after, Duration::from_secs(30));
        assert_eq!(builder.timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(builder.timing.max_backoff, Duration::from_secs(60));
        assert_eq!(builder.timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(builder.timing.max_connection_age, None);
    }

    #[test]
    fn setters_write_the_timing() {
        let builder = PredictionsWsBuilder::new()
            .ping_interval(Duration::from_secs(3))
            .stale_after(Duration::from_secs(9))
            .backoff(Duration::from_millis(5), Duration::from_millis(50))
            .connect_timeout(Duration::from_secs(2));
        assert_eq!(builder.timing.ping_interval, Some(Duration::from_secs(3)));
        assert_eq!(builder.timing.stale_after, Duration::from_secs(9));
        assert_eq!(builder.timing.initial_backoff, Duration::from_millis(5));
        assert_eq!(builder.timing.max_backoff, Duration::from_millis(50));
        assert_eq!(builder.timing.connect_timeout, Duration::from_secs(2));
    }

    #[test]
    fn the_url_rule_is_wss_or_a_loopback_ws() {
        for good in [
            PRODUCTION_WS,
            DEMO_WS,
            "ws://127.0.0.1:8080/trade-api/ws/v2",
            "ws://localhost:9/x",
            "ws://[::1]:9/x",
        ] {
            let builder = PredictionsWsBuilder::new().url(good).unwrap();
            assert_eq!(builder.url, good);
        }
        for bad in [
            "ws://example.com/trade-api/ws/v2",
            "ws://192.168.0.1:9/x",
            "http://127.0.0.1:9/x",
            "not a url",
        ] {
            let err = PredictionsWsBuilder::new().url(bad).unwrap_err();
            assert!(
                matches!(&err, PredictionsWsError::InvalidUrl { url, .. } if url == bad),
                "{bad}: {err:?}"
            );
            assert_eq!(err.class(), Class::InvalidRequest);
        }
    }
}
```

Create `polyoxide-kalshi/src/predictions/ws/protocol.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use polyoxide_test_support::fixtures;
    use polyoxide_venue::{Level, UnixMillis};
    use polyoxide_ws::tungstenite::http;
    use ring::signature::{UnparsedPublicKey, ED25519};
    use rust_decimal::Decimal;
    use serde_json::{json, Value};

    use super::*;
    use crate::predictions::{
        auth::{ACCESS_KEY, ACCESS_SIGNATURE, ACCESS_TIMESTAMP},
        types::BookSide,
    };

    // The mirror's own examples (`docs/specs/kalshi/asyncapi.yaml`, version 2.0.0).
    const SNAPSHOT: &str = r#"{"type":"orderbook_snapshot","sending_ts_ms":1669149841234,"sid":2,"seq":2,"msg":{"market_ticker":"FED-23DEC-T3.00","market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","yes_dollars_fp":[["0.0800","300.00"],["0.2200","333.00"]],"no_dollars_fp":[["0.5400","20.00"],["0.5600","146.00"]]}}"#;
    const DELTA: &str = r#"{"type":"orderbook_delta","sending_ts_ms":1669149841123,"sid":2,"seq":3,"msg":{"market_ticker":"FED-23DEC-T3.00","market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","price_dollars":"0.9600","delta_fp":"-54.00","side":"yes","ts":"2022-11-22T20:44:01Z","ts_ms":1669149841000}}"#;
    const SUBSCRIBED: &str = r#"{"id":1,"type":"subscribed","sending_ts_ms":1669149841234,"msg":{"channel":"orderbook_delta","sid":1}}"#;
    const UNSUBSCRIBED: &str =
        r#"{"id":102,"sid":2,"seq":7,"type":"unsubscribed","sending_ts_ms":1669149841234}"#;
    const ERROR: &str = r#"{"id":123,"type":"error","sending_ts_ms":1669149841234,"msg":{"code":7,"msg":"Unknown subscription ID"}}"#;

    fn credentials() -> Credentials {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let body = vectors["ed25519"]["pkcs8_body"]
            .as_str()
            .expect("the vectors hold an Ed25519 key");
        // The fixture stores the key body without its armour lines (spec §6.4).
        let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n");
        Credentials::from_pem("test-key-id", &pem).expect("the vector key parses")
    }

    fn protocol() -> PredictionsProtocol {
        PredictionsProtocol::new("ws://127.0.0.1:9/trade-api/ws/v2".to_owned(), credentials())
    }

    fn text(frame: &str) -> Message {
        Message::Text(frame.into())
    }

    fn decoded(
        protocol: &mut PredictionsProtocol,
        frame: &str,
    ) -> Decoded<Event, PredictionsWsError> {
        protocol.decode(&text(frame))
    }

    fn answer(
        sequences: &mut Sequences,
        id: u64,
        frame: &str,
    ) -> Correlated<Result<Answered, PredictionsWsError>> {
        correlate(sequences, id, &text(frame))
    }

    fn snapshot_frame(sid: u64, seq: u64) -> String {
        json!({"type": "orderbook_snapshot", "sid": sid, "seq": seq, "msg": {
            "market_ticker": "A-1", "market_id": "id",
            "yes_dollars_fp": [["0.1000", "1.00"]], "no_dollars_fp": [["0.9000", "2.00"]],
        }})
        .to_string()
    }

    fn delta_frame(sid: u64, seq: u64, side: &str) -> String {
        json!({"type": "orderbook_delta", "sid": sid, "seq": seq, "msg": {
            "market_ticker": "A-1", "market_id": "id",
            "price_dollars": "0.5000", "delta_fp": "1.00", "side": side,
        }})
        .to_string()
    }

    fn ok_frame(id: u64, sid: u64, seq: u64) -> String {
        json!({"id": id, "sid": sid, "seq": seq, "type": "ok", "msg": {"market_tickers": ["A-1"]}})
            .to_string()
    }

    fn level(price: &str, size: &str) -> Level {
        Level::new(price.parse().unwrap(), size.parse().unwrap())
    }

    fn server(code: u32) -> PredictionsWsError {
        PredictionsWsError::Server {
            code,
            message: "a message".to_owned(),
        }
    }

    /// An upgrade refused with `status`, and a `Retry-After` when one is given.
    fn refused(status: u16, retry_after: Option<&str>) -> PredictionsWsError {
        let mut response = http::Response::builder().status(status);
        if let Some(wait) = retry_after {
            response = response.header("Retry-After", wait);
        }
        PredictionsWsError::from(WsError::from(tungstenite::Error::Http(
            response.body(None).unwrap(),
        )))
    }

    fn fixture(name: &str) -> String {
        fixtures!("predictions/ws").text(name)
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
            .collect()
    }

    #[test]
    fn recovery_follows_the_rows_of_the_error_codes() {
        let protocol = protocol();
        assert_eq!(protocol.recover(&server(9)), Recovery::Stop);
        // Kalshi calls codes 10 and 25 terminal ("the user must resubscribe"): the
        // reconnect's replay does.
        assert_eq!(protocol.recover(&server(10)), Recovery::Reconnect);
        assert_eq!(protocol.recover(&server(25)), Recovery::Reconnect);
        assert_eq!(protocol.recover(&server(27)), Recovery::Reconnect);
        for code in [1, 7, 8, 11, 14, 26] {
            assert_eq!(protocol.recover(&server(code)), Recovery::Skip, "{code}");
        }
        let json = PredictionsWsError::Json {
            text: "x".to_owned(),
            source: serde_json::from_str::<u8>("x").unwrap_err(),
        };
        assert_eq!(protocol.recover(&json), Recovery::Skip);
        let gap = PredictionsWsError::SequenceGap {
            sid: 1,
            expected: 2,
            received: 4,
        };
        assert_eq!(protocol.recover(&gap), Recovery::Reconnect);
        let url = PredictionsWsError::InvalidUrl {
            url: "x".to_owned(),
            reason: "y",
        };
        assert_eq!(protocol.recover(&url), Recovery::Stop);
        let signing = PredictionsWsError::Credentials(CredentialsError::Sign {
            reason: "x".to_owned(),
        });
        assert_eq!(protocol.recover(&signing), Recovery::Stop);
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_but_a_refused_request_stops() {
        let protocol = protocol();
        for status in [408, 425, 429, 500, 503] {
            let err = refused(status, None);
            assert_eq!(protocol.recover(&err), Recovery::Reconnect, "{status}");
            assert!(err.is_retriable(), "{status}");
        }
        for status in [400, 401, 403, 404, 418, 451] {
            assert_eq!(
                protocol.recover(&refused(status, None)),
                Recovery::Stop,
                "{status}"
            );
        }
    }

    #[test]
    fn the_kits_own_failures_follow_their_class_and_a_use_after_close_reconnects() {
        let protocol = protocol();
        for err in [
            WsError::connect_timeout(Duration::from_secs(10)),
            WsError::no_answer(Duration::from_secs(1)),
            WsError::closed(Some(1000), "bye"),
            WsError::closed(Some(1001), "going away"),
            WsError::closed(Some(1011), "internal"),
            WsError::closed(None, ""),
            WsError::from(tungstenite::Error::ConnectionClosed),
            // A ping that raced a server close: the class is InvalidRequest, and the
            // connection used after it closed still reconnects.
            WsError::from(tungstenite::Error::AlreadyClosed),
        ] {
            let err = PredictionsWsError::from(err);
            assert_eq!(protocol.recover(&err), Recovery::Reconnect, "{err:?}");
        }
        for err in [
            WsError::closed(Some(1008), "policy"),
            WsError::from(tungstenite::Error::AttackAttempt),
            WsError::from(tungstenite::Error::Capacity(
                tungstenite::error::CapacityError::TooManyHeaders,
            )),
        ] {
            let err = PredictionsWsError::from(err);
            assert_eq!(protocol.recover(&err), Recovery::Stop, "{err:?}");
        }
    }

    #[test]
    fn a_refusals_retry_after_is_the_wait_the_reconnect_floors() {
        // The kit waits at least what this returns, up to its ceiling (DRIFT R11). A
        // protocol that returned nothing would reconnect on its own schedule into a 429.
        let protocol = protocol();
        let wait = Some(Duration::from_secs(30));
        assert_eq!(protocol.retry_after(&refused(429, Some("30"))), wait);
        assert_eq!(protocol.retry_after(&refused(503, Some("30"))), wait);
        assert_eq!(protocol.retry_after(&refused(503, None)), None);
        // A zero is no wait: the backoff's own step is the floor.
        assert_eq!(protocol.retry_after(&refused(429, Some("0"))), None);
        assert_eq!(protocol.retry_after(&server(27)), None);
    }

    #[test]
    fn commands_are_written_as_the_asyncapi_gives_them() {
        let parsed = |command: &Command, id: u64| -> Value {
            serde_json::from_str(&command.frame(id)).unwrap()
        };
        let subscribe = Command::Subscribe {
            tickers: vec!["A-1".to_owned(), "B-2".to_owned()],
        };
        assert_eq!(
            parsed(&subscribe, 1),
            json!({"id": 1, "cmd": "subscribe", "params": {
                "channels": ["orderbook_delta"],
                "market_tickers": ["A-1", "B-2"],
                "use_yes_price": true,
            }})
        );
        let add = Command::Update {
            sid: 4,
            action: "add_markets",
            tickers: vec!["C-3".to_owned()],
        };
        assert_eq!(
            parsed(&add, 2),
            json!({"id": 2, "cmd": "update_subscription", "params": {
                "sid": 4, "action": "add_markets", "market_tickers": ["C-3"],
            }})
        );
        assert_eq!(
            parsed(&Command::Unsubscribe { sid: 4 }, 3),
            json!({"id": 3, "cmd": "unsubscribe", "params": {"sids": [4]}})
        );
    }

    #[test]
    fn an_answer_is_matched_by_its_id() {
        let mut sequences = Sequences::default();
        assert!(matches!(
            answer(&mut sequences, 1, SUBSCRIBED),
            Correlated::Answer(Ok(Answered::Subscribed(1)))
        ));
        // An answer to another command nobody awaits any more.
        assert!(matches!(
            answer(&mut sequences, 2, SUBSCRIBED),
            Correlated::Uncorrelated
        ));
        assert!(matches!(
            answer(&mut sequences, 8, &ok_frame(8, 3, 1)),
            Correlated::Answer(Ok(Answered::Done))
        ));
        assert!(matches!(
            answer(&mut sequences, 123, ERROR),
            Correlated::Answer(Err(PredictionsWsError::Server { code: 7, .. }))
        ));
        // An error with no id is Kalshi reporting on a subscription: the stream's.
        let scoped = r#"{"type":"error","sid":3,"seq":2,"msg":{"code":25,"msg":"Subscription buffer overflow"}}"#;
        assert!(matches!(
            answer(&mut sequences, 9, scoped),
            Correlated::Push
        ));
        // A frame that is not JSON is left for the stream to report.
        assert!(matches!(
            answer(&mut sequences, 9, "not json"),
            Correlated::Push
        ));
    }

    #[test]
    fn a_push_read_while_a_request_waits_is_counted_once() {
        // Bugs: counting a kept push twice, which reads the next frame as a gap, and not
        // counting it at all, which reads the answer behind it as one.
        let mut protocol = protocol();
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(Event::Snapshot(_))
        ));
        // On the wire: a delta (2), then the ok (3) the request waits for.
        let mut sequences = std::mem::take(&mut protocol.sequences);
        assert!(matches!(
            answer(&mut sequences, 5, &delta_frame(1, 2, "yes")),
            Correlated::Push
        ));
        assert!(matches!(
            answer(&mut sequences, 5, &ok_frame(5, 1, 3)),
            Correlated::Answer(Ok(Answered::Done))
        ));
        protocol.sequences = sequences;
        assert!(protocol.sequences.gap.is_none());
        // The kept delta is decoded afterwards, and is not counted again.
        assert!(matches!(
            decoded(&mut protocol, &delta_frame(1, 2, "yes")),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_frame(1, 4, "no")),
            Decoded::Event(Event::Delta(_))
        ));
        assert!(protocol.sequences.gap.is_none());
    }

    #[test]
    fn a_gap_in_an_answer_is_raised_by_the_next_frame() {
        // The request that read the ok is for another command, so it cannot return the
        // gap; the next frame does, whatever its kind.
        let mut protocol = protocol();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        assert!(matches!(
            correlate(&mut protocol.sequences, 5, &text(&ok_frame(5, 1, 3))),
            Correlated::Answer(Ok(Answered::Done))
        ));
        match protocol.decode(&Message::Ping(Vec::new().into())) {
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 2,
                received: 3,
            }) => {}
            other => panic!("expected the gap, got {other:?}"),
        }
        // It is raised once.
        assert!(matches!(
            protocol.decode(&Message::Ping(Vec::new().into())),
            Decoded::Nothing
        ));
    }

    #[test]
    fn a_snapshot_is_the_yes_book_best_first() {
        let mut protocol = protocol();
        let Decoded::Event(Event::Snapshot(book)) = decoded(&mut protocol, SNAPSHOT) else {
            panic!("not a snapshot");
        };
        assert_eq!(book.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(book.market_id, "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1");
        assert_eq!(
            book.bids,
            [level("0.2200", "333.00"), level("0.0800", "300.00")]
        );
        assert_eq!(
            book.asks,
            [level("0.5400", "20.00"), level("0.5600", "146.00")]
        );
    }

    #[test]
    fn a_delta_is_a_bid_or_an_ask_in_yes_pricing() {
        let mut protocol = protocol();
        let Decoded::Event(Event::Delta(bid)) = decoded(&mut protocol, DELTA) else {
            panic!("not a delta");
        };
        assert_eq!(bid.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(bid.side, BookSide::Bid);
        assert_eq!(bid.price, "0.9600".parse::<Decimal>().unwrap());
        assert_eq!(bid.delta, "-54.00".parse::<Decimal>().unwrap());
        assert_eq!(bid.ts, Some(UnixMillis(1_669_149_841_000)));
        let Decoded::Event(Event::Delta(ask)) = decoded(&mut protocol, &delta_frame(2, 4, "no"))
        else {
            panic!("not a delta");
        };
        assert_eq!(ask.side, BookSide::Ask);
    }

    #[test]
    fn answers_nobody_awaits_and_unknown_types_are_nothing() {
        for frame in [
            SUBSCRIBED,
            UNSUBSCRIBED,
            ERROR,
            r#"{"type":"ok","id":9,"msg":{}}"#,
            r#"{"type":"ticker","sid":5,"msg":{}}"#,
            "{}",
        ] {
            let mut protocol = protocol();
            assert!(
                matches!(decoded(&mut protocol, frame), Decoded::Nothing),
                "{frame}"
            );
        }
    }

    #[test]
    fn an_error_with_no_id_is_kalshi_reporting_on_a_subscription() {
        let mut protocol = protocol();
        let scoped = r#"{"type":"error","sid":3,"seq":1,"msg":{"code":25,"msg":"Subscription buffer overflow"}}"#;
        match decoded(&mut protocol, scoped) {
            Decoded::Error(PredictionsWsError::Server { code, message }) => {
                assert_eq!(code, 25);
                assert_eq!(message, "Subscription buffer overflow");
            }
            other => panic!("expected the server's error, got {other:?}"),
        }
    }

    #[test]
    fn a_frame_that_is_not_json_is_a_decode_error() {
        let mut protocol = protocol();
        match decoded(&mut protocol, "not json") {
            Decoded::Error(PredictionsWsError::Json { text, .. }) => {
                assert_eq!(text, "not json");
            }
            other => panic!("expected a decode error, got {other:?}"),
        }
        // A frame of a known type that cannot be read is the same error.
        let no_ticker = r#"{"type":"orderbook_snapshot","sid":1,"seq":1,"msg":{"market_id":"id"}}"#;
        assert!(matches!(
            decoded(&mut protocol, no_ticker),
            Decoded::Error(PredictionsWsError::Json { .. })
        ));
    }

    #[test]
    fn control_frames_are_nothing() {
        let mut protocol = protocol();
        for frame in [
            Message::Ping(b"heartbeat".to_vec().into()),
            Message::Pong(Vec::new().into()),
            Message::Binary(vec![1, 2, 3].into()),
            Message::Close(None),
        ] {
            assert!(
                matches!(protocol.decode(&frame), Decoded::Nothing),
                "{frame:?}"
            );
            // Every frame is liveness, and none is a delivered book.
            assert!(protocol.is_live(&frame, &Decoded::Nothing));
            assert!(!protocol.delivered(&frame, &Decoded::Nothing));
        }
    }

    #[test]
    fn a_skipped_sequence_number_is_a_gap_per_subscription() {
        let mut protocol = protocol();
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(_)
        ));
        // Another subscription counts on its own, from whatever number it starts at.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(2, 7)),
            Decoded::Event(_)
        ));
        assert!(matches!(
            decoded(&mut protocol, &delta_frame(2, 8, "yes")),
            Decoded::Event(_)
        ));
        match decoded(&mut protocol, &delta_frame(1, 3, "yes")) {
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 2,
                received: 3,
            }) => {}
            other => panic!("expected a gap, got {other:?}"),
        }
        // A number seen twice is a gap too: the next after 3 is 4.
        match decoded(&mut protocol, &delta_frame(1, 3, "yes")) {
            Decoded::Error(PredictionsWsError::SequenceGap {
                sid: 1,
                expected: 4,
                received: 3,
            }) => {}
            other => panic!("expected a gap, got {other:?}"),
        }
    }

    #[test]
    fn a_fresh_connection_counts_from_scratch() {
        let mut protocol = protocol();
        decoded(&mut protocol, &snapshot_frame(1, 1));
        decoded(&mut protocol, &delta_frame(1, 2, "yes"));
        protocol.reset();
        assert!(protocol.sid.is_none());
        // The same subscription number comes back on the next connection, from 1.
        assert!(matches!(
            decoded(&mut protocol, &snapshot_frame(1, 1)),
            Decoded::Event(Event::Snapshot(_))
        ));
    }

    #[test]
    fn the_captured_snapshot_decodes_to_a_sorted_book() {
        let wire: Value = serde_json::from_str(&fixture("snapshot")).unwrap();
        let mut protocol = protocol();
        let Decoded::Event(Event::Snapshot(book)) = decoded(&mut protocol, &fixture("snapshot"))
        else {
            panic!("the fixture is not a snapshot");
        };
        let count = |key: &str| wire["msg"][key].as_array().map_or(0, Vec::len);
        assert_eq!(book.bids.len(), count("yes_dollars_fp"));
        assert_eq!(book.asks.len(), count("no_dollars_fp"));
        assert!(
            book.bids
                .windows(2)
                .all(|pair| pair[0].price > pair[1].price),
            "{:?}",
            book.bids
        );
        assert!(
            book.asks
                .windows(2)
                .all(|pair| pair[0].price < pair[1].price),
            "{:?}",
            book.asks
        );
        assert_eq!(
            book.market_ticker,
            wire["msg"]["market_ticker"].as_str().unwrap()
        );
    }

    #[test]
    fn every_captured_delta_decodes_with_its_side_price_and_size() {
        let deltas: Value = serde_json::from_str(&fixture("deltas")).unwrap();
        let rows = deltas.as_array().unwrap();
        assert!(!rows.is_empty());
        let mut protocol = protocol();
        for wire in rows {
            let Decoded::Event(Event::Delta(delta)) = decoded(&mut protocol, &wire.to_string())
            else {
                panic!("not a delta: {wire}");
            };
            let msg = &wire["msg"];
            let side = if msg["side"] == "yes" {
                BookSide::Bid
            } else {
                BookSide::Ask
            };
            assert_eq!(delta.side, side, "{wire}");
            let price = msg["price_dollars"].as_str().unwrap().parse::<Decimal>();
            assert_eq!(delta.price, price.unwrap(), "{wire}");
            let size = msg["delta_fp"].as_str().unwrap().parse::<Decimal>();
            assert_eq!(delta.delta, size.unwrap(), "{wire}");
        }
    }

    #[test]
    fn the_captured_answers_are_nothing_to_the_stream_and_carry_what_it_reads() {
        for name in ["subscribed", "ok", "unsubscribed", "error"] {
            let wire: Value = serde_json::from_str(&fixture(name)).unwrap();
            // The correlation matches an answer by its id.
            assert!(wire["id"].is_u64(), "{name}: {wire}");
            let mut protocol = protocol();
            assert!(
                matches!(decoded(&mut protocol, &fixture(name)), Decoded::Nothing),
                "{name}"
            );
        }
        // An ok and an unsubscribed count in their subscription's sequence.
        for name in ["ok", "unsubscribed"] {
            let wire: Value = serde_json::from_str(&fixture(name)).unwrap();
            assert!(
                wire["sid"].is_u64() && wire["seq"].is_u64(),
                "{name}: {wire}"
            );
        }
    }

    #[test]
    fn the_handshake_is_signed_over_the_urls_path() {
        let vectors = fixtures!("predictions/signing").json("vectors");
        let public = hex(vectors["ed25519"]["public_key_hex"].as_str().unwrap());
        for (url, path) in [
            (
                "wss://external-api-ws.kalshi.com/trade-api/ws/v2",
                "/trade-api/ws/v2",
            ),
            ("ws://127.0.0.1:9/", "/"),
            ("ws://127.0.0.1:9", "/"),
        ] {
            let request = signed_request(&credentials(), url).unwrap();
            let header = |name: &str| -> String {
                request
                    .headers()
                    .get(name)
                    .map(|value| value.to_str().unwrap().to_owned())
                    .unwrap_or_else(|| panic!("no {name} header on {url}"))
            };
            assert_eq!(header(ACCESS_KEY), "test-key-id");
            let timestamp = header(ACCESS_TIMESTAMP);
            assert!(timestamp.chars().all(|c| c.is_ascii_digit()), "{timestamp}");
            let message = format!("{timestamp}GET{path}");
            let signature = STANDARD.decode(header(ACCESS_SIGNATURE)).unwrap();
            UnparsedPublicKey::new(&ED25519, &public)
                .verify(message.as_bytes(), &signature)
                .unwrap_or_else(|_| panic!("the signature over {message} does not verify"));
        }
    }

    #[test]
    fn a_url_that_is_not_a_request_is_refused_before_signing() {
        let err = signed_request(&credentials(), "not a url").unwrap_err();
        assert_eq!(
            err.class(),
            polyoxide_venue::Class::InvalidRequest,
            "{err:?}"
        );
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --lib ws::`

Expected: it does not compile: `error[E0432]: unresolved imports `builder::PredictionsWsBuilder`, `protocol::signed_request`, `protocol::Change`, `protocol::PredictionsMembership`, `protocol::PredictionsProtocol`` in `ws/mod.rs` (both files hold only tests), and `cannot find type `PredictionsProtocol` in this scope` in them.

- [ ] **Step 3: Implement**

Insert at the top of `polyoxide-kalshi/src/predictions/ws/protocol.rs`, directly above the line `#[cfg(test)]`:

```rust
//! Kalshi's order-book socket protocol, as the kit's `Supervisor` drives it, and the
//! membership calls on its handle.

use std::{
    collections::{BTreeSet, HashMap},
    future::Future,
    time::Duration,
};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{
        self,
        client::IntoClientRequest,
        handshake::client::Request as Handshake,
        http::{HeaderName, HeaderValue},
        Message,
    },
    Connection, Correlated, Decoded, Kind, MembershipHandle, Offline, Outage, Protocol, Queue,
    Recovery, WsError,
};
use serde::Deserialize;
use serde_json::json;

use crate::predictions::{
    key::Ticker,
    ws::{
        event::{frames, DisconnectReason, Event},
        PredictionsWsError,
    },
    Credentials,
};

/// Events the consumer may leave unread before the task stops reading.
const EVENT_QUEUE: usize = 1024;
/// The one channel this socket carries.
const CHANNEL: &str = "orderbook_delta";

/// One change to the subscribed markets, made through [`PredictionsMembership`] or
/// `MembershipHandle::apply`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Change {
    /// Subscribe to these markets; one already subscribed is skipped.
    Subscribe(Vec<Ticker>),
    /// Unsubscribe from these markets; one not subscribed is skipped.
    Unsubscribe(Vec<Ticker>),
}

/// Kalshi's order-book socket, which [`PredictionsWsBuilder`](crate::predictions::ws::PredictionsWsBuilder)
/// hands to the kit's `Supervisor`.
///
/// Its membership is the set of tickers subscribed, replayed on every fresh connection as
/// one `subscribe` of `orderbook_delta` with `use_yes_price: true`. A change goes out as an
/// `update_subscription` on the live subscription's `sid`; the first add with no
/// subscription sends a `subscribe`, and emptying the membership sends an `unsubscribe`
/// and keeps the connection. A change is recorded before its frame is sent: one whose
/// connection dies under it is answered `Ok` and replayed, one Kalshi refuses is undone,
/// and one made during an outage is answered at once and replayed. Its divergence rows:
///
/// - **D4, the ping:** an empty protocol Ping on the wall clock.
/// - **D5, liveness:** every frame, the server's Pings and the Pongs to the client's own
///   included.
/// - **D6, a delivered connection:** one that decoded a book frame.
/// - **D8:** `Event::Disconnected`, then `Event::Reconnected`, once each per outage.
/// - **D19:** a connection used after it closed reconnects.
/// - **The handshake** is signed afresh for every attempt, over the URL's path.
/// - **Sequence:** every frame that carries `sid` and `seq`, answers included, is checked
///   against the last of its subscription; a gap reconnects.
///
/// Its queue holds 1024 events.
pub struct PredictionsProtocol {
    url: String,
    credentials: Credentials,
    /// The live subscription's id on this connection, once Kalshi has answered a
    /// `subscribe`.
    sid: Option<u64>,
    /// What this connection's sequenced frames have said.
    sequences: Sequences,
}

impl std::fmt::Debug for PredictionsProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PredictionsProtocol")
            .field("url", &self.url)
            .field("credentials", &self.credentials)
            .field("sid", &self.sid)
            .finish_non_exhaustive()
    }
}

impl PredictionsProtocol {
    pub(crate) fn new(url: String, credentials: Credentials) -> Self {
        Self {
            url,
            credentials,
            sid: None,
            sequences: Sequences::default(),
        }
    }

    /// Note what Kalshi's answer to `command` settled.
    fn accepted(&mut self, command: &Command, answered: &Answered) {
        match (command, answered) {
            (Command::Subscribe { .. }, Answered::Subscribed(sid)) => self.sid = Some(*sid),
            (Command::Unsubscribe { sid }, _) => {
                self.sid = None;
                self.sequences.last.remove(sid);
            }
            _ => {}
        }
    }
}

/// What one connection's sequenced frames have said.
///
/// The first `seq` of a subscription is taken as it comes; after it, each must be the
/// previous plus one. The correlation closure of a request observes every sequenced frame
/// it reads while it waits, and counts in `ahead` each push it keeps for the stream, which
/// `decode` meets later and must not observe again. A gap the closure finds is held in
/// `gap` for the next frame `decode` sees.
#[derive(Debug, Default)]
struct Sequences {
    last: HashMap<u64, u64>,
    ahead: usize,
    gap: Option<PredictionsWsError>,
}

impl Sequences {
    fn advance(&mut self, sid: u64, seq: u64) -> Result<(), PredictionsWsError> {
        match self.last.insert(sid, seq) {
            Some(previous) if seq != previous.saturating_add(1) => {
                Err(PredictionsWsError::SequenceGap {
                    sid,
                    expected: previous.saturating_add(1),
                    received: seq,
                })
            }
            _ => Ok(()),
        }
    }
}

/// The parts of a frame the protocol reads before it reads the rest.
#[derive(Debug, Deserialize)]
struct Peek {
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    sid: Option<u64>,
    #[serde(default)]
    seq: Option<u64>,
}

impl Peek {
    fn sequence(&self) -> Option<(u64, u64)> {
        Some((self.sid?, self.seq?))
    }

    /// Whether the frame answers a command. Kalshi counts an `id` of 0 as none.
    fn answers_a_command(&self) -> bool {
        matches!(
            self.kind.as_deref(),
            Some("subscribed" | "ok" | "unsubscribed" | "error")
        ) && self.id.is_some_and(|id| id != 0)
    }
}

fn parse<'a, T: Deserialize<'a>>(text: &'a str) -> Result<T, PredictionsWsError> {
    serde_json::from_str(text).map_err(|source| decode_error(text, source))
}

fn decode_error(text: &str, source: serde_json::Error) -> PredictionsWsError {
    PredictionsWsError::Json {
        text: text.to_owned(),
        source,
    }
}

/// What a command's answer says.
#[derive(Debug, PartialEq, Eq)]
enum Answered {
    /// `subscribed`, with the new subscription's id.
    Subscribed(u64),
    /// `ok` or `unsubscribed`.
    Done,
}

fn answered(kind: &str, text: &str) -> Result<Answered, PredictionsWsError> {
    match kind {
        "subscribed" => Ok(Answered::Subscribed(
            parse::<frames::Subscribed>(text)?.msg.sid,
        )),
        "error" => {
            let rejected = parse::<frames::Rejected>(text)?;
            Err(PredictionsWsError::Server {
                code: rejected.msg.code,
                message: rejected.msg.msg,
            })
        }
        _ => Ok(Answered::Done),
    }
}

/// The correlation of a request: the frame answering command `id`, and the sequence
/// bookkeeping of every frame read meanwhile.
///
/// A frame that answers another command is dropped by the kit, after it has counted here;
/// a frame that answers none is kept for the stream, and counted in `ahead` if sequenced.
fn correlate(
    sequences: &mut Sequences,
    id: u64,
    frame: &Message,
) -> Correlated<Result<Answered, PredictionsWsError>> {
    let Message::Text(text) = frame else {
        return Correlated::Push;
    };
    // A frame that is not JSON is the stream's to report.
    let Ok(peek) = serde_json::from_str::<Peek>(text.as_str()) else {
        return Correlated::Push;
    };
    let sequenced = peek.sequence();
    if let Some((sid, seq)) = sequenced {
        if let Err(gap) = sequences.advance(sid, seq) {
            if sequences.gap.is_none() {
                sequences.gap = Some(gap);
            }
        }
    }
    if peek.answers_a_command() {
        if peek.id != Some(id) {
            return Correlated::Uncorrelated;
        }
        let kind = peek.kind.as_deref().unwrap_or_default();
        return Correlated::Answer(answered(kind, text.as_str()));
    }
    if sequenced.is_some() {
        sequences.ahead += 1;
    }
    Correlated::Push
}

/// A command to Kalshi, before it has an id.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Subscribe {
        tickers: Vec<String>,
    },
    Update {
        sid: u64,
        action: &'static str,
        tickers: Vec<String>,
    },
    Unsubscribe {
        sid: u64,
    },
}

impl Command {
    /// The frame, for the connection's request `id`.
    fn frame(&self, id: u64) -> String {
        match self {
            Self::Subscribe { tickers } => json!({
                "id": id,
                "cmd": "subscribe",
                "params": {
                    "channels": [CHANNEL],
                    "market_tickers": tickers,
                    "use_yes_price": true,
                },
            }),
            Self::Update {
                sid,
                action,
                tickers,
            } => json!({
                "id": id,
                "cmd": "update_subscription",
                "params": {"sid": sid, "action": action, "market_tickers": tickers},
            }),
            Self::Unsubscribe { sid } => json!({
                "id": id,
                "cmd": "unsubscribe",
                "params": {"sids": [sid]},
            }),
        }
        .to_string()
    }
}

/// Send `command` as the connection's next request and read its answer.
async fn send(
    sequences: &mut Sequences,
    connection: &mut Connection,
    command: &Command,
) -> Result<Answered, PredictionsWsError> {
    match connection
        .request(
            |id| Message::Text(command.frame(id).into()),
            |id, frame| correlate(sequences, id, frame),
        )
        .await
    {
        Ok(answered) => answered,
        Err(err) => Err(err.into()),
    }
}

/// The tickers' text, for a frame.
fn names<'a>(tickers: impl IntoIterator<Item = &'a Ticker>) -> Vec<String> {
    tickers
        .into_iter()
        .map(|ticker| ticker.as_str().to_owned())
        .collect()
}

/// The tickers of `add` that `membership` lacks, once each.
fn added(membership: &BTreeSet<Ticker>, add: Vec<Ticker>) -> Vec<Ticker> {
    add.into_iter()
        .filter(|ticker| !membership.contains(ticker))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The tickers of `remove` that `membership` holds, once each.
fn removed(membership: &BTreeSet<Ticker>, remove: Vec<Ticker>) -> Vec<Ticker> {
    remove
        .into_iter()
        .filter(|ticker| membership.contains(ticker))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The handshake request for one connection attempt, signed afresh: Kalshi's three
/// headers over `timestamp + "GET" + the URL's path`, by the same signer as REST.
///
/// Doc-hidden: the live suite signs its raw socket with it. Not API.
pub fn signed_request(
    credentials: &Credentials,
    url: &str,
) -> Result<Handshake, PredictionsWsError> {
    let mut request = url.into_client_request().map_err(WsError::from)?;
    let signed = credentials.sign_request("GET", request.uri().path())?;
    for (name, value) in signed.pairs() {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(header_error)?;
        let value = HeaderValue::from_str(value).map_err(header_error)?;
        request.headers_mut().insert(name, value);
    }
    Ok(request)
}

/// A header that could not be built is a request that could not be built: the kit's
/// `HttpFormat`, which the socket table classes `InvalidRequest`.
fn header_error(err: impl Into<tungstenite::http::Error>) -> PredictionsWsError {
    PredictionsWsError::from(WsError::from(tungstenite::Error::HttpFormat(err.into())))
}

impl Protocol for PredictionsProtocol {
    type Event = Event;
    type Error = PredictionsWsError;
    type Membership = BTreeSet<Ticker>;
    type Change = Change;
    type Ack = ();

    /// Signed afresh for every attempt: the supervisor calls this for the first and for
    /// each reconnect, and the default `connect` dials its request through the kit.
    fn handshake(&mut self) -> Result<Handshake, PredictionsWsError> {
        signed_request(&self.credentials, &self.url)
    }

    /// One `subscribe` for every ticker; nothing for none. A refusal on a fresh connection
    /// fails the attempt, which recovery decides on.
    async fn replay(
        &mut self,
        connection: &mut Connection,
        membership: &BTreeSet<Ticker>,
    ) -> Result<(), PredictionsWsError> {
        if membership.is_empty() {
            return Ok(());
        }
        let command = Command::Subscribe {
            tickers: names(membership.iter()),
        };
        let answered = send(&mut self.sequences, connection, &command).await?;
        self.accepted(&command, &answered);
        Ok(())
    }

    /// A change on the live connection, recorded before it is sent. Kalshi refusing it, or
    /// answering with a frame that does not read, undoes it; the connection failing under
    /// it leaves it recorded and answers `Ok`, since the next read ends the connection and
    /// the replay applies it.
    async fn apply(
        &mut self,
        connection: &mut Connection,
        membership: &mut BTreeSet<Ticker>,
        change: Change,
    ) -> Result<(), PredictionsWsError> {
        let before = membership.clone();
        let command = match change {
            Change::Subscribe(add) => {
                let new = added(membership, add);
                if new.is_empty() {
                    return Ok(());
                }
                membership.extend(new.iter().cloned());
                match self.sid {
                    Some(sid) => Command::Update {
                        sid,
                        action: "add_markets",
                        tickers: names(&new),
                    },
                    // Nothing is subscribed on this connection yet: the whole
                    // membership goes in one subscribe.
                    None => Command::Subscribe {
                        tickers: names(membership.iter()),
                    },
                }
            }
            Change::Unsubscribe(remove) => {
                let gone = removed(membership, remove);
                if gone.is_empty() {
                    return Ok(());
                }
                for ticker in &gone {
                    membership.remove(ticker);
                }
                match self.sid {
                    // Nothing was ever subscribed on this connection.
                    None => return Ok(()),
                    Some(sid) if membership.is_empty() => Command::Unsubscribe { sid },
                    Some(sid) => Command::Update {
                        sid,
                        action: "delete_markets",
                        tickers: names(&gone),
                    },
                }
            }
        };
        match send(&mut self.sequences, connection, &command).await {
            Ok(answered) => {
                self.accepted(&command, &answered);
                Ok(())
            }
            Err(err @ (PredictionsWsError::Server { .. } | PredictionsWsError::Json { .. })) => {
                *membership = before;
                Err(err)
            }
            Err(err) => {
                tracing::debug!(%err, "the connection failed under a change; it is recorded for the replay");
                Ok(())
            }
        }
    }

    /// A change during an outage is recorded and answered at once; the replay applies it.
    fn record(
        &mut self,
        membership: &mut BTreeSet<Ticker>,
        change: Change,
    ) -> Offline<Change, (), PredictionsWsError> {
        match change {
            Change::Subscribe(add) => membership.extend(add),
            Change::Unsubscribe(remove) => {
                for ticker in &remove {
                    membership.remove(ticker);
                }
            }
        }
        Offline::Recorded(Ok(()))
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Event, PredictionsWsError> {
        // A gap an answer showed is raised by the first frame after it, whatever its kind.
        if let Some(gap) = self.sequences.gap.take() {
            return Decoded::Error(gap);
        }
        // The kit answers the server's Ping and its Close, and a Binary frame is not part
        // of the protocol.
        let Message::Text(text) = frame else {
            return Decoded::Nothing;
        };
        let text = text.as_str();
        let peek = match serde_json::from_str::<Peek>(text) {
            Ok(peek) => peek,
            Err(source) => return Decoded::Error(decode_error(text, source)),
        };
        if let Some((sid, seq)) = peek.sequence() {
            if self.sequences.ahead > 0 {
                // The correlation closure counted this frame while a request waited.
                self.sequences.ahead -= 1;
            } else if let Err(gap) = self.sequences.advance(sid, seq) {
                return Decoded::Error(gap);
            }
        }
        match peek.kind.as_deref() {
            Some("orderbook_snapshot") => match parse::<frames::Snapshot>(text) {
                Ok(frame) => Decoded::Event(Event::Snapshot(frame.msg.into())),
                Err(err) => Decoded::Error(err),
            },
            Some("orderbook_delta") => match parse::<frames::Delta>(text) {
                Ok(frame) => Decoded::Event(Event::Delta(frame.msg.into())),
                Err(err) => Decoded::Error(err),
            },
            // An error that answers no command of ours is Kalshi reporting on a
            // subscription.
            Some("error") if !peek.answers_a_command() => match parse::<frames::Rejected>(text) {
                Ok(frame) => Decoded::Error(PredictionsWsError::Server {
                    code: frame.msg.code,
                    message: frame.msg.msg,
                }),
                Err(err) => Decoded::Error(err),
            },
            // An answer nobody awaits, and every type this socket does not carry.
            _ => Decoded::Nothing,
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<Event, PredictionsWsError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<Event, PredictionsWsError>) -> bool {
        matches!(decoded, Decoded::Event(_) | Decoded::Events(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Ping(Vec::new().into()))
    }

    fn is_retriable(err: &PredictionsWsError) -> bool {
        err.is_retriable()
    }

    /// Kalshi's recovery: a frame that does not decode is skipped; an error frame's code
    /// decides (9 stops the stream, 10, 25 and 27 reconnect, any other is skipped); a gap
    /// reconnects; a connection used after it closed reconnects (D19); and anything else
    /// reconnects exactly when its class is retriable, so a refused handshake's 401 or 403
    /// stops, and a 429 or 5xx retries. No code here matches over `tungstenite::Error`: the
    /// socket table decides the class.
    ///
    /// A skipped error during an attempt reads as a reconnect, the kit's rule for a failed
    /// attempt.
    fn recover(&self, err: &PredictionsWsError) -> Recovery {
        match err {
            PredictionsWsError::Json { .. } => Recovery::Skip,
            PredictionsWsError::Server { code, .. } => match code {
                9 => Recovery::Stop,
                10 | 25 | 27 => Recovery::Reconnect,
                _ => Recovery::Skip,
            },
            PredictionsWsError::SequenceGap { .. } => Recovery::Reconnect,
            PredictionsWsError::Connection(err) if matches!(err.kind(), Kind::AlreadyClosed) => {
                Recovery::Reconnect
            }
            err if err.is_retriable() => Recovery::Reconnect,
            _ => Recovery::Stop,
        }
    }

    /// A refused upgrade's `Retry-After`, as Kalshi's class reads it, which the reconnect
    /// waits out up to the backoff's ceiling and never undercuts (DRIFT R11).
    fn retry_after(&self, err: &PredictionsWsError) -> Option<Duration> {
        err.retry_after()
    }

    fn disconnected(&mut self, outage: Outage<PredictionsWsError>) -> Option<Event> {
        Some(Event::Disconnected {
            reason: DisconnectReason::from(outage),
        })
    }

    fn reconnected(&mut self) -> Event {
        Event::Reconnected
    }

    fn reset(&mut self) {
        self.sid = None;
        self.sequences = Sequences::default();
    }

    fn queue(&self) -> Queue {
        Queue::Bounded(EVENT_QUEUE)
    }
}

/// The membership calls of a supervised Kalshi socket, on its `MembershipHandle`.
///
/// Each waits for the task to take the change and for Kalshi's answer. During an outage
/// the change is recorded and answered at once, and replayed on the next connection. The
/// task takes no change while the consumer's queue is full (1024 unread events), so a call
/// made from the task that reads the stream can wait for ever then: make changes from
/// another task, or keep reading while one is pending.
pub trait PredictionsMembership {
    /// Subscribe to more markets. The first add with nothing subscribed sends a
    /// `subscribe`; later ones go as `update_subscription` on the live subscription. A
    /// market already subscribed is skipped.
    ///
    /// Fails with [`PredictionsWsError::Server`] when Kalshi refuses the change, which
    /// leaves the membership as it was; with a stall when Kalshi left it unanswered for
    /// the staleness window (the connection is then replaced); or once the stream has
    /// stopped.
    fn subscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;

    /// Unsubscribe from markets, failing as [`subscribe`](Self::subscribe) does.
    /// Unsubscribing every market sends an `unsubscribe` and keeps the connection, which
    /// keeps pinging; a reconnect then replays nothing.
    fn unsubscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send;
}

impl PredictionsMembership for MembershipHandle<PredictionsProtocol> {
    fn subscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send {
        self.apply(Change::Subscribe(tickers.into_iter().collect()))
    }

    fn unsubscribe(
        &self,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> impl Future<Output = Result<(), PredictionsWsError>> + Send {
        self.apply(Change::Unsubscribe(tickers.into_iter().collect()))
    }
}

```

Insert at the top of `polyoxide-kalshi/src/predictions/ws/builder.rs`, directly above the line `#[cfg(test)]`:

```rust
//! The builder: Kalshi's socket timings and endpoint, handed to the kit's `Supervisor`
//! with `PredictionsProtocol`.

use std::{collections::BTreeSet, time::Duration};

use polyoxide_ws::{Supervisor, Timing};

use crate::predictions::{
    key::Ticker,
    ws::{
        protocol::PredictionsProtocol, PredictionsWsError, SupervisedPredictionsWs, PRODUCTION_WS,
    },
    Credentials,
};

/// Builder for a supervised order-book socket.
#[derive(Debug, Clone)]
pub struct PredictionsWsBuilder {
    url: String,
    timing: Timing,
}

impl Default for PredictionsWsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PredictionsWsBuilder {
    /// Defaults: the production host ([`PRODUCTION_WS`]; the demo's is
    /// [`DEMO_WS`](crate::predictions::ws::DEMO_WS)), a protocol Ping every 10 s, stale
    /// after 30 s of silence, reconnect backoff from 500 ms doubling to 60 s, a 10 s
    /// connect timeout for each attempt, and no rotation.
    pub fn new() -> Self {
        let mut timing = Timing::default();
        timing.ping_interval = Some(Duration::from_secs(10));
        timing.stale_after = Duration::from_secs(30);
        timing.initial_backoff = Duration::from_millis(500);
        timing.max_backoff = Duration::from_secs(60);
        timing.connect_timeout = Duration::from_secs(10);
        Self {
            url: PRODUCTION_WS.to_owned(),
            timing,
        }
    }

    /// Connect to a different endpoint, such as the demo's
    /// ([`DEMO_WS`](crate::predictions::ws::DEMO_WS)). The URL must be `wss://`, or
    /// `ws://` to a loopback host (`127.0.0.1`, `[::1]` or `localhost`), so a signed
    /// handshake never travels unencrypted to a remote host; anything else, and a URL
    /// that does not parse, is refused with [`PredictionsWsError::InvalidUrl`].
    pub fn url(mut self, url: &str) -> Result<Self, PredictionsWsError> {
        self.url = checked_url(url)?;
        Ok(self)
    }

    polyoxide_ws::timing_setters! {
        /// How often to send a protocol Ping, whatever the traffic (default 10 s, the
        /// cadence of Kalshi's own). The server's Pings count as liveness too, so this
        /// only keeps a quiet connection answering.
        ping_interval;
        /// Silence after which the connection is presumed dead and replaced (default
        /// 30 s). Any frame counts, the server's Pings included. Also bounds each
        /// membership change and each replay: one Kalshi leaves unanswered that long
        /// replaces the connection.
        stale_after;
        /// Reconnect delay schedule: `initial`, doubling to `max` (default 500 ms to
        /// 60 s). There is no cap on attempts. A refused upgrade's `Retry-After` only
        /// lengthens the wait, up to `max`.
        backoff;
        /// How long one connection attempt may take: the TCP connect, the TLS handshake
        /// and the upgrade together (default 10 s). A timed out attempt is retried like
        /// any other lost connection.
        connect_timeout;
    }

    /// Connect, sign the handshake, subscribe to `tickers` and start supervising. With no
    /// tickers the connection opens and subscribes nothing; the first
    /// [`subscribe`](crate::predictions::ws::PredictionsMembership::subscribe) does.
    ///
    /// The first attempt's failure is returned, not retried, so a key Kalshi refuses or a
    /// URL that is wrong is reported at once; later refusals arrive through the stream and
    /// [`PredictionsMembership`](crate::predictions::ws::PredictionsMembership).
    pub async fn connect(
        self,
        credentials: Credentials,
        tickers: impl IntoIterator<Item = Ticker>,
    ) -> Result<SupervisedPredictionsWs, PredictionsWsError> {
        let membership: BTreeSet<Ticker> = tickers.into_iter().collect();
        Supervisor::new(PredictionsProtocol::new(self.url, credentials), self.timing)
            .connect(membership)
            .await
    }
}

/// The URL a socket takes: `wss://`, or `ws://` to a loopback host.
fn checked_url(url: &str) -> Result<String, PredictionsWsError> {
    let invalid = |reason: &'static str| PredictionsWsError::InvalidUrl {
        url: url.to_owned(),
        reason,
    };
    let parsed = url::Url::parse(url).map_err(|_| invalid("it does not parse"))?;
    let allowed = match parsed.scheme() {
        "wss" => true,
        "ws" => match parsed.host() {
            Some(url::Host::Domain(host)) => host == "localhost",
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        },
        _ => false,
    };
    if allowed {
        Ok(url.to_owned())
    } else {
        Err(invalid("it must be wss://, or ws:// to a loopback host"))
    }
}

```

Run: `nix develop --command cargo fmt --all`

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --lib ws::`

Expected: `test result: ok. 38 passed; 0 failed` (8 in `ws::error`, 6 in `ws::event`, 21 in `ws::protocol`, 3 in `ws::builder`). If `the_captured_answers_are_nothing_to_the_stream_and_carry_what_it_reads` fails on `wire["id"].is_u64()`, Kalshi's answer to that command carries no `id` and the correlation cannot match it: stop and report it. If `the_handshake_is_signed_over_the_urls_path` cannot read the header, check that `Credentials::sign_request`'s header names are Story 4's `ACCESS_KEY`, `ACCESS_TIMESTAMP` and `ACCESS_SIGNATURE`.

Then the story gate: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && cargo test -p polyoxide-kalshi --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: exit status 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/ws/protocol.rs polyoxide-kalshi/src/predictions/ws/builder.rs
git commit -m "feat(kalshi): PredictionsProtocol and its builder, a handshake signed on every attempt (Story 6)" -- polyoxide-kalshi/src/predictions/ws
```

### Task 6.5: The scripted answer and the supervision suite

**Files:**
- Create: `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`
- Modify: `polyoxide-kalshi/src/predictions/ws/mod.rs` (the module block), `polyoxide-kalshi/Cargo.toml` (a `[[test]]` stanza, before Story 1's registration anchor)
- Test: `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`

`scripted.rs` is Kalshi's answer for the kit's scripted server: `subscribe` is answered with `subscribed` (sid 1, then 2, and so on) and one snapshot per ticker, `update_subscription` with `ok` (and a snapshot for each added market) and `unsubscribe` with `unsubscribed`, every sequenced frame counting from 1 per `sid`. A `Script` adds what the server sends besides its answers and how it ends the connection, and injects faults: an `ok` that skips a sequence number, a delta ahead of the `ok`, a refused ticker, a subscription-scoped error frame (code 9, 10 or 25), updates left unanswered. It is `#[cfg(test)]` in the crate and `#[path]`-included by the integration suite, so it names nothing of the crate's own (the same shape as Binance's), and its `KalshiServer` wraps the kit's `ScriptedServer` and needs Story 5's `ScriptedServer::handshake_headers()`. The suite proves the story's rules on it: a fresh signature on every attempt, replay, the membership frames, a gap's reconnect (in a push and in an answer), liveness, the error codes (9 ends the stream; 10 and 25 reconnect), refusals and the `Retry-After` floor.

- [ ] **Step 1: Write the failing test**

Current (`polyoxide-kalshi/src/predictions/ws/mod.rs`, two lines of the module block as Task 6.4 left it):

```rust
mod event;
mod protocol;
```

Replace with:

```rust
mod event;
mod protocol;
#[cfg(test)]
mod scripted;
```

Current (`polyoxide-kalshi/Cargo.toml`, the registration anchor, with Task 6.3's stanza above it):

```toml
[[test]]
name = "wire_agreement_predictions_ws"
required-features = ["predictions-ws"]
```

Replace with:

```toml
[[test]]
name = "supervision_predictions"
required-features = ["predictions-ws"]

[[test]]
name = "wire_agreement_predictions_ws"
required-features = ["predictions-ws"]
```

Create `polyoxide-kalshi/src/predictions/ws/scripted.rs` holding only its tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ask(state: &mut State, script: &Script, request: Value) -> Vec<Value> {
        reply(state, script, &request)
    }

    fn subscribe_request(tickers: &[&str]) -> Value {
        json!({"id": 1, "cmd": "subscribe", "params": {
            "channels": ["orderbook_delta"], "market_tickers": tickers, "use_yes_price": true,
        }})
    }

    #[test]
    fn a_subscribe_gets_subscribed_and_a_snapshot_per_market_counting_from_one() {
        let mut state = State::default();
        let frames = ask(
            &mut state,
            &Script::default(),
            subscribe_request(&["A-1", "B-2"]),
        );
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0]["type"], "subscribed");
        assert_eq!(frames[0]["id"], 1);
        assert_eq!(frames[0]["msg"]["sid"], 1);
        assert_eq!(frames[1]["type"], "orderbook_snapshot");
        assert_eq!(
            (frames[1]["sid"].as_u64(), frames[1]["seq"].as_u64()),
            (Some(1), Some(1))
        );
        assert_eq!(frames[1]["msg"]["market_ticker"], "A-1");
        assert_eq!(frames[2]["msg"]["market_ticker"], "B-2");
        assert_eq!(frames[2]["seq"], 2);
    }

    #[test]
    fn an_update_is_answered_with_an_ok_on_the_next_seq_and_a_snapshot_per_added_market() {
        let mut state = State::default();
        ask(&mut state, &Script::default(), subscribe_request(&["A-1"]));
        let add = json!({"id": 2, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "add_markets", "market_tickers": ["B-2"],
        }});
        let frames = ask(&mut state, &Script::default(), add);
        assert_eq!(frames[0]["type"], "ok");
        assert_eq!(frames[0]["id"], 2);
        assert_eq!(frames[0]["seq"], 2);
        assert_eq!(frames[0]["msg"]["market_tickers"], json!(["A-1", "B-2"]));
        assert_eq!(frames[1]["type"], "orderbook_snapshot");
        assert_eq!(frames[1]["seq"], 3);
        let delete = json!({"id": 3, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "delete_markets", "market_tickers": ["A-1"],
        }});
        let frames = ask(&mut state, &Script::default(), delete);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0]["seq"], 4);
        assert_eq!(frames[0]["msg"]["market_tickers"], json!(["B-2"]));
    }

    #[test]
    fn an_unsubscribe_ends_the_subscription_and_an_unknown_sid_is_an_error() {
        let mut state = State::default();
        ask(&mut state, &Script::default(), subscribe_request(&["A-1"]));
        let end = json!({"id": 2, "cmd": "unsubscribe", "params": {"sids": [1]}});
        let frames = ask(&mut state, &Script::default(), end.clone());
        assert_eq!(frames[0]["type"], "unsubscribed");
        assert_eq!(
            (frames[0]["sid"].as_u64(), frames[0]["seq"].as_u64()),
            (Some(1), Some(2))
        );
        let again = ask(&mut state, &Script::default(), end);
        assert_eq!(again[0]["type"], "error");
        assert_eq!(again[0]["msg"]["code"], 7);
    }

    #[test]
    fn a_second_subscription_gets_the_next_sid_and_its_own_count() {
        let mut state = State::default();
        ask(&mut state, &Script::default(), subscribe_request(&["A-1"]));
        let frames = ask(&mut state, &Script::default(), subscribe_request(&["B-2"]));
        assert_eq!(frames[0]["msg"]["sid"], 2);
        assert_eq!(
            (frames[1]["sid"].as_u64(), frames[1]["seq"].as_u64()),
            (Some(2), Some(1))
        );
    }

    #[test]
    fn injected_faults_skip_a_number_refuse_a_ticker_and_report_on_a_subscription() {
        let mut state = State::default();
        let script = Script {
            ok_skips_seq: true,
            delta_before_ok: true,
            refuse_ticker: Some(("BAD-1".to_owned(), 11)),
            error_after_subscribe: Some((25, "Subscription buffer overflow".to_owned())),
            ..Script::default()
        };
        let frames = ask(&mut state, &script, subscribe_request(&["A-1"]));
        assert_eq!(frames[2]["type"], "error");
        assert_eq!(
            (frames[2]["sid"].as_u64(), frames[2]["seq"].as_u64()),
            (Some(1), Some(2))
        );
        assert_eq!(frames[2]["msg"]["code"], 25);
        assert!(frames[2].get("id").is_none());
        let add = json!({"id": 2, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "add_markets", "market_tickers": ["B-2"],
        }});
        let frames = ask(&mut state, &script, add);
        // A delta (3), the number the ok skips (4), then the ok (5).
        assert_eq!(frames[0]["type"], "orderbook_delta");
        assert_eq!(frames[0]["seq"], 3);
        assert_eq!(frames[1]["type"], "ok");
        assert_eq!(frames[1]["seq"], 5);
        let refused = json!({"id": 3, "cmd": "update_subscription", "params": {
            "sid": 1, "action": "add_markets", "market_tickers": ["BAD-1"],
        }});
        let frames = ask(&mut state, &script, refused);
        assert_eq!(frames[0]["type"], "error");
        assert_eq!(frames[0]["id"], 3);
        assert_eq!(frames[0]["msg"]["code"], 11);
    }

    #[test]
    fn a_subscription_error_carries_the_code_it_is_given() {
        // Kalshi's terminal codes: 9 (authentication required), 10 (a channel error) and
        // 25 (the subscription's buffer overflowed).
        for (code, message) in [
            (9, "Authentication required"),
            (10, "Channel error"),
            (25, "Subscription buffer overflow"),
        ] {
            let mut state = State::default();
            let script = Script {
                error_after_subscribe: Some((code, message.to_owned())),
                ..Script::default()
            };
            let frames = ask(&mut state, &script, subscribe_request(&["A-1"]));
            assert_eq!(frames.len(), 3, "{code}");
            assert_eq!(frames[2]["type"], "error", "{code}");
            assert_eq!(frames[2]["msg"]["code"], code, "{code}");
            assert_eq!(frames[2]["msg"]["msg"], message, "{code}");
            assert_eq!(
                (frames[2]["sid"].as_u64(), frames[2]["seq"].as_u64()),
                (Some(1), Some(2)),
                "{code}"
            );
            assert!(frames[2].get("id").is_none(), "{code}");
        }
    }
}
```

Create `polyoxide-kalshi/tests/supervision_predictions.rs`:

```rust
//! The supervised socket against the scripted server, with limits in hundreds of
//! milliseconds. Each test names the bug it exists to catch.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::StreamExt;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        DisconnectReason, Event, PredictionsMembership, PredictionsWsBuilder, PredictionsWsError,
        SupervisedPredictionsWs,
    },
    Credentials,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use polyoxide_ws::test_server::{next_event, next_item, WINDOW};
use ring::signature::{UnparsedPublicKey, ED25519};
use serde_json::{json, Value};

// Kalshi's answer on the shared scripted server, as the unit tests use it.
#[path = "../src/predictions/ws/scripted.rs"]
mod scripted;

use scripted::{delta, KalshiServer, Script};

const PATH: &str = "/trade-api/ws/v2";
const A: &str = "KXAAA-1";
const B: &str = "KXBBB-2";
const C: &str = "KXCCC-3";

fn t(text: &str) -> Ticker {
    Ticker::new(text).expect("a valid ticker")
}

fn vectors() -> Value {
    fixtures!("predictions/signing").json("vectors")
}

fn credentials() -> Credentials {
    let vectors = vectors();
    let body = vectors["ed25519"]["pkcs8_body"]
        .as_str()
        .expect("the vectors hold an Ed25519 key");
    // The fixture stores the key body without its armour lines (spec §6.4).
    let pem = format!("-----BEGIN PRIVATE KEY-----\n{body}\n-----END PRIVATE KEY-----\n");
    Credentials::from_pem("test-key-id", &pem).expect("the vector key parses")
}

fn builder_for(server: &KalshiServer) -> PredictionsWsBuilder {
    PredictionsWsBuilder::new()
        .url(&format!("{}{PATH}", server.url))
        .expect("a loopback URL")
        .ping_interval(Duration::from_millis(50))
        .stale_after(Duration::from_millis(300))
        .backoff(Duration::from_millis(10), Duration::from_millis(20))
}

fn kind(event: &Event) -> &'static str {
    match event {
        Event::Snapshot(_) => "snapshot",
        Event::Delta(_) => "delta",
        Event::Disconnected { .. } => "disconnected",
        Event::Reconnected => "reconnected",
        _ => "other",
    }
}

/// The kinds of the next `count` events.
async fn kinds(ws: &mut SupervisedPredictionsWs, count: usize) -> Vec<&'static str> {
    let mut seen = Vec::new();
    for _ in 0..count {
        seen.push(kind(&next_event(ws).await));
    }
    seen
}

/// Asserts nothing arrives for `quiet`.
async fn assert_quiet(ws: &mut SupervisedPredictionsWs, quiet: Duration) {
    if let Ok(item) = tokio::time::timeout(quiet, ws.next()).await {
        panic!("expected silence, got {item:?}");
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> &'a str {
    headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
        .unwrap_or_else(|| panic!("no {name} header in {headers:?}"))
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// Asserts the handshake's signature verifies over `timestamp + "GET" + PATH` with the
/// vector key's public key.
fn assert_signed(headers: &[(String, String)]) {
    let vectors = vectors();
    let public = hex(vectors["ed25519"]["public_key_hex"]
        .as_str()
        .expect("a public key"));
    let message = format!("{}GET{PATH}", header(headers, "kalshi-access-timestamp"));
    let signature = STANDARD
        .decode(header(headers, "kalshi-access-signature"))
        .expect("base64");
    UnparsedPublicKey::new(&ED25519, public)
        .verify(message.as_bytes(), &signature)
        .unwrap_or_else(|_| panic!("the signature over {message} does not verify"));
}

/// Asserts a request is a `subscribe` of exactly `expected`, in YES pricing.
fn assert_subscribe(frame: &Value, expected: &[&str]) {
    assert_eq!(frame["cmd"], "subscribe");
    assert_eq!(frame["params"]["channels"], json!(["orderbook_delta"]));
    assert_eq!(frame["params"]["use_yes_price"], true);
    let mut tickers: Vec<&str> = frame["params"]["market_tickers"]
        .as_array()
        .expect("a ticker list")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    tickers.sort_unstable();
    assert_eq!(tickers, expected);
}

#[tokio::test]
async fn every_attempt_signs_its_own_handshake() {
    // Bug: signing once, so that a reconnect replays a timestamp Kalshi refuses.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    let handshakes = server.handshake_headers();
    assert_eq!(handshakes.len(), 2);
    assert_ne!(
        header(&handshakes[0], "kalshi-access-timestamp"),
        header(&handshakes[1], "kalshi-access-timestamp")
    );
    for headers in &handshakes {
        assert_eq!(header(headers, "kalshi-access-key"), "test-key-id");
        assert_signed(headers);
    }
    assert_eq!(server.paths(), [PATH, PATH]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_reconnect_replays_every_market_in_one_subscribe() {
    // Bug: resubscribing to nothing, or to the first market only, after a reconnect.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(B), t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 6).await,
        [
            "snapshot",
            "snapshot",
            "disconnected",
            "reconnected",
            "snapshot",
            "snapshot"
        ]
    );
    let firsts = server.first_frames();
    assert_eq!(firsts.len(), 2);
    for first in &firsts {
        assert_subscribe(first, &[A, B]);
    }
    ws.close().await.expect("close");
}

#[tokio::test]
async fn changes_go_out_as_update_subscription_on_the_live_sid() {
    // Bug: a second `subscribe` per change, which opens a second subscription.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));

    membership.subscribe([t(B)]).await.expect("add");
    // The new market's snapshot follows the ok.
    match next_event(&mut ws).await {
        Event::Snapshot(book) => assert_eq!(book.market_ticker, B),
        other => panic!("expected the added market's snapshot, got {other:?}"),
    }
    membership.unsubscribe([t(A)]).await.expect("delete");
    membership
        .unsubscribe([t(B)])
        .await
        .expect("empty the membership");
    membership.subscribe([t(C)]).await.expect("add after empty");

    let requests = server.requests();
    assert_eq!(requests.len(), 5, "{requests:?}");
    assert_subscribe(&requests[0], &[A]);
    assert_eq!(requests[1]["cmd"], "update_subscription");
    assert_eq!(
        requests[1]["params"],
        json!({"sid": 1, "action": "add_markets", "market_tickers": [B]})
    );
    assert_eq!(
        requests[2]["params"],
        json!({"sid": 1, "action": "delete_markets", "market_tickers": [A]})
    );
    assert_eq!(requests[3]["cmd"], "unsubscribe");
    assert_eq!(requests[3]["params"], json!({"sids": [1]}));
    assert_subscribe(&requests[4], &[C]);
    // The kit's ids, counting from 1, one per command.
    let ids: Vec<u64> = requests.iter().filter_map(|r| r["id"].as_u64()).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn the_first_add_on_an_empty_membership_sends_a_subscribe() {
    // Bug: an `update_subscription` with no `sid`, or no frame at all.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), Vec::<Ticker>::new())
        .await
        .expect("connect");
    server
        .wait_for("a handshake", |s| s.handshake_headers().len() == 1)
        .await;
    assert!(server.requests().is_empty());
    let membership = ws.membership();
    membership.subscribe([t(A)]).await.expect("add");
    assert_subscribe(&server.requests()[0], &[A]);
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_refused_change_is_undone_and_not_replayed() {
    // Bug: a market Kalshi refused stays in the membership, so every replay carries it
    // and the subscription is refused for ever.
    let server = KalshiServer::start(vec![
        Script {
            refuse_ticker: Some(("KXBAD-1".to_owned(), 11)),
            cut_after: Some(Duration::from_millis(400)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    let err = membership
        .subscribe([t("KXBAD-1")])
        .await
        .expect_err("Kalshi refuses it");
    assert!(
        matches!(err, PredictionsWsError::Server { code: 11, .. }),
        "{err:?}"
    );
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["disconnected", "reconnected", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_change_made_during_an_outage_is_replayed() {
    // Bug: a change made while the connection is down is lost, or waits for ever.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    // A long backoff keeps the outage open while the change is made.
    let mut ws = builder_for(&server)
        .backoff(Duration::from_millis(200), Duration::from_millis(200))
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert_eq!(kinds(&mut ws, 2).await, ["snapshot", "disconnected"]);
    membership
        .subscribe([t(B)])
        .await
        .expect("recorded during the outage");
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    // Two subscribes and no update: the replay carried the change.
    assert_eq!(server.requests().len(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_change_whose_connection_dies_under_it_is_answered_ok_and_replayed() {
    // Bug: reporting the dead connection to the caller, who then retries a change the
    // replay is about to apply.
    let server = KalshiServer::start(vec![
        Script {
            ignore_updates: true,
            cut_after: Some(Duration::from_millis(200)),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // Kalshi never answers the update; the server cuts the connection at 200 ms.
    membership.subscribe([t(B)]).await.expect("answered Ok");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["disconnected", "reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_sequence_gap_reconnects_and_resnapshots() {
    // Bug: carrying on past a lost delta, so a book built on it is silently wrong.
    let server = KalshiServer::start(vec![
        Script {
            // The snapshot is seq 1; a delta at 3 skips 2.
            pushes: vec![delta(1, 3, A).to_string()],
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 2, 3)),
        other => panic!("expected the gap to end the connection, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_gap_in_an_answer_reconnects() {
    // Bug: checking only the pushes, so an `ok` that skipped a number goes unnoticed and
    // the delta lost behind it is never found.
    let server = KalshiServer::start(vec![
        Script {
            ok_skips_seq: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // The change itself succeeded; the gap behind it ends the connection.
    membership.subscribe([t(B)]).await.expect("the change");
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason:
                DisconnectReason::Error(PredictionsWsError::SequenceGap {
                    sid,
                    expected,
                    received,
                }),
        } => assert_eq!((sid, expected, received), (1, 2, 3)),
        other => panic!("expected the gap in the answer, got {other:?}"),
    }
    assert_eq!(
        kinds(&mut ws, 3).await,
        ["reconnected", "snapshot", "snapshot"]
    );
    assert_subscribe(&server.first_frames()[1], &[A, B]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_delta_between_a_request_and_its_ok_is_not_a_gap() {
    // Bugs: counting the delta a request kept twice, and not counting it before the ok
    // behind it. Either reconnects on every change made while the book is moving.
    let server = KalshiServer::start(vec![Script {
        delta_before_ok: true,
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    let membership = ws.membership();
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    membership.subscribe([t(B)]).await.expect("add");
    assert_eq!(kinds(&mut ws, 2).await, ["delta", "snapshot"]);
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn the_servers_pings_alone_keep_a_quiet_connection_live() {
    // Bug: counting only data as liveness, which drops a healthy connection in every
    // quiet hour. The client sends no pings here, so only the server's keep it live.
    let server = KalshiServer::start(vec![Script {
        ping_every: Some(Duration::from_millis(100)),
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .ping_interval(Duration::MAX)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    // Three staleness windows of nothing but the server's pings.
    assert_quiet(&mut ws, Duration::from_millis(900)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_connection_with_no_traffic_at_all_goes_stale() {
    // The control for the test above: with no pings from either side the same connection
    // is replaced, so that test proves the pings are what kept it.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .ping_interval(Duration::MAX)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Stale { silent },
        } => assert!(silent >= Duration::from_millis(300), "{silent:?}"),
        other => panic!("expected a stall, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn error_code_9_ends_the_stream_with_its_error() {
    // Bug: reconnecting for ever, at the backoff ceiling, on credentials Kalshi refuses.
    let server = KalshiServer::start(vec![Script {
        error_after_subscribe: Some((9, "Authentication required".to_owned())),
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_item(&mut ws).await {
        Err(PredictionsWsError::Server { code, message }) => {
            assert_eq!(code, 9);
            assert_eq!(message, "Authentication required");
        }
        other => panic!("expected the server's error, got {other:?}"),
    }
    let ended = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(ended.is_none());
    assert_eq!(server.connection_count(), 1);
}

/// Kalshi reports `code` on the subscription, which its AsyncAPI calls terminal ("the user
/// must resubscribe"): the stream reports the outage and reconnects, and the replay
/// resubscribes.
async fn assert_error_code_reconnects(code: u32, message: &str) {
    let server = KalshiServer::start(vec![
        Script {
            error_after_subscribe: Some((code, message.to_owned())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Error(PredictionsWsError::Server { code: seen, .. }),
        } => assert_eq!(seen, code),
        other => panic!("expected code {code} to end the connection, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    assert_eq!(server.connection_count(), 2);
    assert_subscribe(&server.first_frames()[1], &[A]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn error_code_10_reconnects() {
    // Bug: yielding the channel error and carrying on with a subscription Kalshi has
    // ended, so the book goes stale with no outage marker.
    assert_error_code_reconnects(10, "Channel error").await;
}

#[tokio::test]
async fn error_code_25_reconnects() {
    // Bug: yielding the overflow and carrying on with a subscription Kalshi has ended.
    assert_error_code_reconnects(25, "Subscription buffer overflow").await;
}

#[tokio::test]
async fn an_error_frame_of_another_code_is_yielded_and_skipped() {
    // Bug: ending the stream, or reconnecting, on an error that is the caller's.
    let server = KalshiServer::start(vec![Script {
        error_after_subscribe: Some((11, "Invalid parameter".to_owned())),
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_item(&mut ws).await {
        Err(PredictionsWsError::Server { code, .. }) => assert_eq!(code, 11),
        other => panic!("expected the server's error, got {other:?}"),
    }
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn the_first_attempts_refusal_is_returned() {
    // Bug: retrying a key Kalshi refuses, so the caller never learns.
    let server = KalshiServer::start(vec![Script {
        refuse_upgrade: Some((401, Vec::new())),
        ..Default::default()
    }])
    .await;
    let err = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect_err("the upgrade is refused");
    assert_eq!(err.class(), Class::Unauthorized);
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn a_401_on_a_reconnect_ends_the_stream() {
    // Bug: reconnecting for ever on a refusal retrying cannot fix.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_upgrade: Some((401, Vec::new())),
            ..Default::default()
        },
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(kinds(&mut ws, 2).await, ["snapshot", "disconnected"]);
    match next_item(&mut ws).await {
        Err(err) => assert_eq!(err.class(), Class::Unauthorized),
        other => panic!("expected the refusal, got {other:?}"),
    }
    let ended = tokio::time::timeout(WINDOW, ws.next())
        .await
        .expect("the stream ends");
    assert!(ended.is_none());
    assert_eq!(server.connection_count(), 2);
}

#[tokio::test]
async fn a_503_on_a_reconnect_is_retried() {
    // Bug: treating a host that is unwell as a refusal.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_upgrade: Some((503, Vec::new())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    assert_eq!(server.connection_count(), 3);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_retry_after_is_waited_out() {
    // Bug: reconnecting on the backoff's own schedule, which sends a request straight
    // back into the 429 and extends the ban Retry-After asked us to sit out.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            refuse_upgrade: Some((429, vec![("Retry-After".to_owned(), "1".to_owned())])),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    // The ceiling is 400 ms, so a one-second Retry-After is waited out up to it.
    let mut ws = builder_for(&server)
        .backoff(Duration::from_millis(10), Duration::from_millis(400))
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 4).await,
        ["snapshot", "disconnected", "reconnected", "snapshot"]
    );
    let accepted = server.accepted_at();
    assert_eq!(accepted.len(), 3);
    let waited = accepted[2] - accepted[1];
    assert!(
        waited >= Duration::from_millis(350),
        "the retry came {waited:?} after the 429"
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn each_outage_yields_one_disconnected_and_one_reconnected() {
    // Bug: a marker missing, doubled, or one of the pair without the other.
    let server = KalshiServer::start(vec![
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script {
            then_cut: true,
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert_eq!(
        kinds(&mut ws, 7).await,
        [
            "snapshot",
            "disconnected",
            "reconnected",
            "snapshot",
            "disconnected",
            "reconnected",
            "snapshot"
        ]
    );
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_server_close_is_reported_with_its_code() {
    // Bug: a close with a code read as an error with none, or not reconnected at all.
    let server = KalshiServer::start(vec![
        Script {
            close_after: true,
            close_code: Some((1011, "internal error".to_owned())),
            ..Default::default()
        },
        Script::default(),
    ])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_event(&mut ws).await {
        Event::Disconnected {
            reason: DisconnectReason::Closed { code, reason },
        } => {
            assert_eq!(code, Some(1011));
            assert_eq!(reason, "internal error");
        }
        other => panic!("expected the close, got {other:?}"),
    }
    assert_eq!(kinds(&mut ws, 2).await, ["reconnected", "snapshot"]);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_frame_that_is_not_json_is_yielded_and_skipped() {
    // Bug: ending the stream, or reconnecting, on one frame Kalshi sent wrongly.
    let server = KalshiServer::start(vec![Script {
        pushes: vec!["not json".to_owned()],
        ..Default::default()
    }])
    .await;
    let mut ws = builder_for(&server)
        .connect(credentials(), [t(A)])
        .await
        .expect("connect");
    assert!(matches!(next_event(&mut ws).await, Event::Snapshot(_)));
    match next_item(&mut ws).await {
        Err(PredictionsWsError::Json { text, .. }) => assert_eq!(text, "not json"),
        other => panic!("expected a decode error, got {other:?}"),
    }
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}

#[tokio::test]
async fn an_empty_membership_connects_sends_nothing_and_keeps_pinging() {
    // Bug: closing a connection that subscribes nothing, or letting it go stale.
    let server = KalshiServer::start(vec![Script::default()]).await;
    let mut ws = builder_for(&server)
        .connect(credentials(), Vec::<Ticker>::new())
        .await
        .expect("connect");
    server
        .wait_for("two client pings", |s| s.ping_count() >= 2)
        .await;
    assert!(server.requests().is_empty());
    assert_quiet(&mut ws, Duration::from_millis(400)).await;
    assert_eq!(server.connection_count(), 1);
    ws.close().await.expect("close");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --test supervision_predictions`

Expected: it does not compile: `error[E0432]: unresolved imports `scripted::delta`, `scripted::KalshiServer`, `scripted::Script`` (the file holds only its tests), and in the lib `cannot find type `State`` for the tests of `ws::scripted`.

- [ ] **Step 3: Implement**

Insert at the top of `polyoxide-kalshi/src/predictions/ws/scripted.rs`, directly above the line `#[cfg(test)]`:

```rust
//! The Kalshi suites' scripts, played on the shared scripted server
//! (`polyoxide_ws::test_server`) with Kalshi's answer.
//!
//! A `Script` says what the server sends besides its answers and how it ends a
//! connection, and injects faults; the answer closure keeps each connection's
//! subscriptions and sequence numbers, as Kalshi does: `sid`s count from 1 per
//! connection, and every sequenced frame of a subscription takes the next number from 1.
//! Its frames use the prices of the AsyncAPI mirror's own examples.
//!
//! One file serves both the unit tests (`#[cfg(test)]`) and the integration suite, which
//! includes it with `#[path]`, so it names nothing of this crate's own. The suite uses
//! part of it.
#![allow(dead_code)]

use std::{collections::HashMap, time::Duration};

use polyoxide_ws::{
    test_server::{Script as Shared, ScriptedServer as Server},
    tungstenite::Message,
};
use serde_json::{json, Value};
use tokio::time::Instant;

const SENDING_TS_MS: u64 = 1_669_149_841_234;
const MARKET_ID: &str = "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1";

/// How the server behaves on one connection.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Frames to send, in order, once the client's first frame has been answered. Their
    /// sequence numbers are the test's own.
    pub pushes: Vec<String>,
    /// While the connection is held open, resend the first push on this cadence.
    pub push_every: Option<Duration>,
    /// Close the connection after the pushes.
    pub close_after: bool,
    /// The close code and reason to send when closing.
    pub close_code: Option<(u16, String)>,
    /// Close the connection this long after the handshake, whatever else is happening.
    pub drop_after: Option<Duration>,
    /// After the pushes, stop reading and writing for good: a dead socket.
    pub go_silent: bool,
    /// Send a protocol ping on this cadence while the connection is held open.
    pub ping_every: Option<Duration>,
    /// Drop the TCP connection without a close frame after the pushes.
    pub then_cut: bool,
    /// Drop the TCP connection without a close frame this long after the handshake.
    pub cut_after: Option<Duration>,
    /// Accept the TCP connection and drop it without a handshake.
    pub reject_handshake: bool,
    /// Answer the WebSocket upgrade with this HTTP status and these headers, such as
    /// `Retry-After`, instead of `101`.
    pub refuse_upgrade: Option<(u16, Vec<(String, String)>)>,
    /// After a `subscribe`'s snapshots, send a subscription-scoped error frame with this
    /// code and message (the suites use 9, authentication required, and 10 and 25, which
    /// Kalshi calls terminal). It carries `sid` and `seq`, and no `id`.
    pub error_after_subscribe: Option<(u32, String)>,
    /// A command naming this ticker is answered with an `error` of this code.
    pub refuse_ticker: Option<(String, u32)>,
    /// Leave every `update_subscription` unanswered.
    pub ignore_updates: bool,
    /// Skip one sequence number before the `ok` answering an `update_subscription`: an
    /// answer with a gap behind it.
    pub ok_skips_seq: bool,
    /// Send a delta ahead of that `ok`.
    pub delta_before_ok: bool,
}

impl Script {
    /// What the shared server sends on the connection: the pushes after the client's first
    /// frame and the answer to it, and how it ends.
    fn shared(&self) -> Shared {
        if self.reject_handshake {
            return Shared::reject();
        }
        if let Some((status, headers)) = &self.refuse_upgrade {
            return headers
                .iter()
                .fold(Shared::refuse_upgrade(*status), |shared, (name, value)| {
                    shared.with_header(name, value)
                });
        }
        let pushes: Vec<&str> = self.pushes.iter().map(String::as_str).collect();
        let mut shared = Shared::frames(&pushes).after_first_frame();
        if let Some(every) = self.push_every {
            shared = shared.push_every(every);
        }
        if self.close_after {
            shared = shared.then_close();
        }
        if let Some((code, reason)) = &self.close_code {
            shared = shared.close_with(*code, reason);
        }
        if let Some(after) = self.drop_after {
            shared = shared.drop_after(after);
        }
        if self.go_silent {
            shared = shared.go_silent();
        }
        if let Some(every) = self.ping_every {
            shared = shared.ping_every(every);
        }
        if self.then_cut {
            shared = shared.then_cut();
        }
        if let Some(after) = self.cut_after {
            shared = shared.cut_after(after);
        }
        shared
    }
}

/// One request the client sent.
#[derive(Debug, Clone)]
pub struct Received {
    /// Which connection, counted from zero.
    pub connection: usize,
    /// When it arrived.
    pub at: Instant,
    /// The frame, parsed.
    pub request: Value,
}

/// A running shared server, answering as Kalshi does.
pub struct KalshiServer {
    /// The base URL to connect to, `ws://127.0.0.1:<port>`; any path is accepted.
    pub url: String,
    inner: Server,
}

impl KalshiServer {
    /// Apply `scripts[n]` to the n-th connection, repeating the last.
    pub async fn start(scripts: Vec<Script>) -> Self {
        let shared = scripts.iter().map(Script::shared).collect();
        let inner = Server::start_answering(shared, move |index| {
            let script = scripts
                .get(index)
                .or_else(|| scripts.last())
                .cloned()
                .unwrap_or_default();
            answer(script)
        })
        .await;
        Self {
            url: inner.url.clone(),
            inner,
        }
    }

    /// Connections accepted so far, counted before the handshake.
    pub fn connection_count(&self) -> usize {
        self.inner.connection_count()
    }

    /// When each connection was accepted, in order.
    pub fn accepted_at(&self) -> Vec<Instant> {
        self.inner.accepted_at()
    }

    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.inner.paths()
    }

    /// The request headers of each handshake that completed, names in lowercase, in
    /// accept order.
    pub fn handshake_headers(&self) -> Vec<Vec<(String, String)>> {
        self.inner.handshake_headers()
    }

    /// The first request of each connection, parsed.
    pub fn first_frames(&self) -> Vec<Value> {
        self.inner
            .first_frames()
            .iter()
            .map(|text| parse(text))
            .collect()
    }

    /// Every request received, in arrival order, across connections.
    pub fn received(&self) -> Vec<Received> {
        self.inner
            .received()
            .into_iter()
            .map(|r| Received {
                connection: r.connection,
                at: r.at,
                request: parse(&r.text),
            })
            .collect()
    }

    /// Every request received, parsed.
    pub fn requests(&self) -> Vec<Value> {
        self.received().into_iter().map(|r| r.request).collect()
    }

    /// Protocol pings received from the client.
    pub fn ping_count(&self) -> usize {
        self.inner.ping_count()
    }

    /// Connections that have ended, however they ended.
    pub fn ended_count(&self) -> usize {
        self.inner.ended_count()
    }

    /// Poll until `predicate` holds, or panic with `label` after five seconds.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        self.inner.wait_for(label, |_| predicate(self)).await;
    }
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text)
        .unwrap_or_else(|err| panic!("a request that is not JSON ({err}): {text}"))
}

/// A snapshot frame of `ticker`'s book, with the prices of the mirror's example: YES bids
/// at 0.08 and 0.22, and NO bids, in YES pricing, at 0.54 and 0.56.
pub fn snapshot(sid: u64, seq: u64, ticker: &str) -> Value {
    json!({
        "type": "orderbook_snapshot",
        "sending_ts_ms": SENDING_TS_MS,
        "sid": sid,
        "seq": seq,
        "msg": {
            "market_ticker": ticker,
            "market_id": MARKET_ID,
            "yes_dollars_fp": [["0.0800", "300.00"], ["0.2200", "333.00"]],
            "no_dollars_fp": [["0.5400", "20.00"], ["0.5600", "146.00"]],
        },
    })
}

/// A delta frame: a YES level of `ticker` losing 54 contracts at 0.96.
pub fn delta(sid: u64, seq: u64, ticker: &str) -> Value {
    json!({
        "type": "orderbook_delta",
        "sending_ts_ms": SENDING_TS_MS,
        "sid": sid,
        "seq": seq,
        "msg": {
            "market_ticker": ticker,
            "market_id": MARKET_ID,
            "price_dollars": "0.9600",
            "delta_fp": "-54.00",
            "side": "yes",
            "ts_ms": 1_669_149_841_000_u64,
        },
    })
}

fn error(id: &Value, code: u32, message: &str) -> Value {
    json!({
        "id": id,
        "type": "error",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"code": code, "msg": message},
    })
}

/// What one connection has subscribed.
#[derive(Default)]
struct State {
    /// The last subscription id issued on this connection.
    issued: u64,
    subscriptions: HashMap<u64, Subscription>,
}

#[derive(Default)]
struct Subscription {
    /// The last sequence number issued.
    seq: u64,
    tickers: Vec<String>,
}

impl State {
    /// The next sequence number of subscription `sid`.
    fn next_seq(&mut self, sid: u64) -> u64 {
        let subscription = self.subscriptions.entry(sid).or_default();
        subscription.seq += 1;
        subscription.seq
    }
}

/// One connection's answerer: Kalshi's answer to `subscribe`, `update_subscription` and
/// `unsubscribe`, given the subscriptions the connection carries, with the script's
/// refusals and injected faults.
fn answer(script: Script) -> impl FnMut(&str) -> Vec<Message> + Send {
    let mut state = State::default();
    move |text| {
        let Ok(request) = serde_json::from_str::<Value>(text) else {
            return Vec::new();
        };
        reply(&mut state, &script, &request)
            .into_iter()
            .map(|frame| Message::Text(frame.to_string().into()))
            .collect()
    }
}

/// The frames Kalshi sends for one request, in order.
fn reply(state: &mut State, script: &Script, request: &Value) -> Vec<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = &request["params"];
    let tickers: Vec<String> = params["market_tickers"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if let Some((refused, code)) = &script.refuse_ticker {
        if tickers.contains(refused) {
            return vec![error(&id, *code, "Invalid parameter")];
        }
    }
    match request["cmd"].as_str() {
        Some("subscribe") => subscribe(state, script, &id, &tickers),
        Some("update_subscription") if script.ignore_updates => Vec::new(),
        Some("update_subscription") => update(state, script, &id, params, &tickers),
        Some("unsubscribe") => unsubscribe(state, &id, params),
        _ => vec![error(&id, 5, "Unknown command")],
    }
}

fn subscribe(state: &mut State, script: &Script, id: &Value, tickers: &[String]) -> Vec<Value> {
    state.issued += 1;
    let sid = state.issued;
    state.subscriptions.insert(
        sid,
        Subscription {
            seq: 0,
            tickers: tickers.to_vec(),
        },
    );
    let mut frames = vec![json!({
        "id": id,
        "type": "subscribed",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"channel": "orderbook_delta", "sid": sid},
    })];
    for ticker in tickers {
        let seq = state.next_seq(sid);
        frames.push(snapshot(sid, seq, ticker));
    }
    if let Some((code, message)) = &script.error_after_subscribe {
        let seq = state.next_seq(sid);
        frames.push(json!({
            "type": "error",
            "sid": sid,
            "seq": seq,
            "sending_ts_ms": SENDING_TS_MS,
            "msg": {"code": code, "msg": message},
        }));
    }
    frames
}

fn update(
    state: &mut State,
    script: &Script,
    id: &Value,
    params: &Value,
    tickers: &[String],
) -> Vec<Value> {
    let Some(sid) = params["sid"]
        .as_u64()
        .filter(|sid| state.subscriptions.contains_key(sid))
    else {
        return vec![error(id, 7, "Unknown subscription ID")];
    };
    let adding = params["action"].as_str() == Some("add_markets");
    {
        let subscription = state.subscriptions.entry(sid).or_default();
        if adding {
            subscription.tickers.extend(tickers.iter().cloned());
        } else {
            subscription.tickers.retain(|held| !tickers.contains(held));
        }
    }
    let mut frames = Vec::new();
    if script.delta_before_ok {
        let ticker = state.subscriptions[&sid]
            .tickers
            .first()
            .cloned()
            .unwrap_or_default();
        let seq = state.next_seq(sid);
        frames.push(delta(sid, seq, &ticker));
    }
    if script.ok_skips_seq {
        // A number the client never sees: the answer that follows has a gap behind it.
        state.next_seq(sid);
    }
    let seq = state.next_seq(sid);
    let held = state.subscriptions[&sid].tickers.clone();
    frames.push(json!({
        "id": id,
        "sid": sid,
        "seq": seq,
        "type": "ok",
        "sending_ts_ms": SENDING_TS_MS,
        "msg": {"market_tickers": held},
    }));
    if adding {
        for ticker in tickers {
            let seq = state.next_seq(sid);
            frames.push(snapshot(sid, seq, ticker));
        }
    }
    frames
}

fn unsubscribe(state: &mut State, id: &Value, params: &Value) -> Vec<Value> {
    let sids: Vec<u64> = params["sids"]
        .as_array()
        .map(|sids| sids.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let mut frames = Vec::new();
    for sid in sids {
        if !state.subscriptions.contains_key(&sid) {
            frames.push(error(id, 7, "Unknown subscription ID"));
            continue;
        }
        let seq = state.next_seq(sid);
        state.subscriptions.remove(&sid);
        frames.push(json!({
            "id": id,
            "sid": sid,
            "seq": seq,
            "type": "unsubscribed",
            "sending_ts_ms": SENDING_TS_MS,
        }));
    }
    frames
}

```

Run: `nix develop --command cargo fmt --all`

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --lib ws::scripted`

Expected: `test result: ok. 5 passed; 0 failed`.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --test supervision_predictions`

Expected: `test result: ok. 28 passed; 0 failed`: the 23 tests of the suite, and the 5 of `scripted.rs`, which run again in this binary as `scripted::tests::*` because the file is `#[path]`-included. A failure that names `handshake_headers` means Story 5 is not in the tree: it is this story's prerequisite.

Then the story gate: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && cargo test -p polyoxide-kalshi --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: exit status 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/src/predictions/ws/scripted.rs polyoxide-kalshi/tests/supervision_predictions.rs
git commit -m "test(kalshi): the supervised order-book socket on the kit's scripted server (Story 6)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/src/predictions/ws polyoxide-kalshi/tests/supervision_predictions.rs
```

### Task 6.6: `live_predictions_ws` and the registry

**Files:**
- Create: `polyoxide-kalshi/tests/live_predictions_ws.rs`
- Modify: `polyoxide-kalshi/Cargo.toml` (a `[[test]]` stanza before Story 1's registration anchor, and the live table directly above `[package.metadata.docs.rs]`)
- Modify (regenerated by `gen_registry.py --write`): `.github/workflows/nightly-behavioral.yml`, `SELF-HEALING.md`, `CLAUDE.md`
- Test: `polyoxide-kalshi/tests/live_predictions_ws.rs`

The live suite runs against the demo host and is auth-gated without `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`. Its credentials come only through the loaders, its failures through `or_fail`, `environmental` and `transient`, it has no `.unwrap()`, `.expect(` or `panic!`, and both tests are `#[ignore]`d. The first connects through the supervised socket, subscribes an open demo market and asks for its snapshot within 30 s, uncrossed. The market is found over unsigned REST against the demo host by Story 2's `common::two_sided_market` (`mod common;`), which pages `status=open&mve_filter=exclude`, since Kalshi's open listing is dominated by multivariate combinations, and confirms both sides of the book on the orderbook route; with none it is an `environmental` skip. The second opens a raw socket through the kit's `connect` with a handshake signed by `signed_request`, subscribes the same market and holds every frame of 30 s against its frame type, both ways, for unmodelled keys: a feed that goes quiet after the snapshot passes.

- [ ] **Step 1: Write the failing test**

Create `polyoxide-kalshi/tests/live_predictions_ws.rs`:

```rust
//! Live tests against the demo host's socket, `#[ignore]`d. Run with:
//! ```sh
//! KALSHI_DEMO_API_KEY_ID=... KALSHI_DEMO_PRIVATE_KEY="$(cat demo.pem)" \
//!     cargo test -p polyoxide-kalshi --features predictions-ws --test live_predictions_ws -- --ignored
//! ```
//!
//! The demo key signs the handshake; keys do not cross environments, so a production
//! key does not work. The market comes from the demo host's own listing over unsigned
//! REST, through `common::two_sided_market`: open, multivariate combinations excluded
//! (`mve_filter=exclude`), with a book on both sides confirmed on the orderbook route. With
//! none, the tests are an environmental skip.
//!
//! `live_frames_carry_no_unmodelled_keys` is the socket's drift detector. It reads raw
//! frames through the kit, decodes each into its frame type, encodes it back, and fails on
//! a key the types do not model or a value that changes. A quiet feed passes: it checks
//! what arrives.
//!
//! Each failure prints the tag the nightly classifier reads, taken from its error's class:
//! a connect timeout, a 5xx or a reset is transient, a refused key is real, and a stream
//! that ends without saying why is transient.

mod common;

use std::{collections::BTreeSet, time::Duration};

use futures_util::StreamExt;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    ws::{
        frames::{Delta, Rejected, Snapshot, Subscribed, Unsubscribed, Updated},
        signed_request, Event, PredictionsWsBuilder, PredictionsWsError, DEMO_WS,
    },
    Credentials, Predictions, DEMO,
};
use polyoxide_test_support::{agreement, load_env, transient, ResultExt};
use polyoxide_ws::{tungstenite::Message, Connection, Mode, DEFAULT_CONNECT_TIMEOUT};
use serde_json::{json, Value};
use tokio::time::Instant;

/// How long the feed gets to say what it will.
const LISTEN: Duration = Duration::from_secs(30);

fn credentials() -> Credentials {
    let creds = load_env(&["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"])
        .unwrap_or_else(|missing| missing.or_auth_gated());
    Credentials::from_pem(
        creds.get("KALSHI_DEMO_API_KEY_ID"),
        creds.get("KALSHI_DEMO_PRIVATE_KEY"),
    )
    .or_fail("the demo key")
}

/// An open market of the demo host, over unsigned REST. `two_sided_market` pages
/// `status=open&mve_filter=exclude` and confirms both sides of the book on the orderbook
/// route, and is an `environmental` skip when none qualifies.
async fn open_demo_ticker() -> Ticker {
    let client = Predictions::builder()
        .base_url(DEMO)
        .build()
        .or_fail("the demo REST client");
    let market = common::two_sided_market(&client).await;
    Ticker::new(&market.ticker).or_fail("the demo market's ticker")
}

/// The key paths the wire sent in `text` that its frame type does not write, or `None`
/// for a frame type this socket does not carry. Panics, untagged, on a value that changes.
fn unmodelled_keys(text: &str) -> Option<Vec<String>> {
    let kind = serde_json::from_str::<Value>(text)
        .ok()?
        .get("type")?
        .as_str()?
        .to_owned();
    let found = match kind.as_str() {
        "subscribed" => agreement::compare::<Subscribed>(&kind, text),
        "ok" => agreement::compare::<Updated>(&kind, text),
        "unsubscribed" => agreement::compare::<Unsubscribed>(&kind, text),
        "error" => agreement::compare::<Rejected>(&kind, text),
        "orderbook_snapshot" => agreement::compare::<Snapshot>(&kind, text),
        "orderbook_delta" => agreement::compare::<Delta>(&kind, text),
        _ => return None,
    };
    Some(
        found
            .unmodelled
            .into_iter()
            .map(|path| format!("{kind}: {path}"))
            .collect(),
    )
}

#[tokio::test]
#[ignore]
async fn live_a_demo_market_delivers_an_uncrossed_snapshot() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let mut feed = PredictionsWsBuilder::new()
        .url(DEMO_WS)
        .or_fail("the demo socket URL")
        .connect(credentials, [ticker])
        .await
        .or_fail("connect to the demo socket");
    let deadline = Instant::now() + LISTEN;
    let mut snapshot = None;
    while snapshot.is_none() {
        let Ok(next) = tokio::time::timeout_at(deadline, feed.next()).await else {
            break;
        };
        let event = next
            .unwrap_or_else(|| transient("the server ended the stream before a snapshot"))
            .or_fail("the supervised demo feed");
        if let Event::Snapshot(book) = event {
            snapshot = Some(book);
        }
    }
    let book = snapshot.unwrap_or_else(|| transient("no orderbook_snapshot arrived within 30 s"));
    // `use_yes_price` puts both sides on one scale; a crossed book would mean it was not
    // honoured. A one-sided book has nothing to cross.
    if let (Some(bid), Some(ask)) = (book.bids.first(), book.asks.first()) {
        assert!(
            bid.price < ask.price,
            "the book is crossed: the best bid {} is not below the best ask {}",
            bid.price,
            ask.price
        );
    }
    feed.close().await.or_fail("close the demo feed");
}

#[tokio::test]
#[ignore]
async fn live_frames_carry_no_unmodelled_keys() {
    let credentials = credentials();
    let ticker = open_demo_ticker().await;
    let request = signed_request(&credentials, DEMO_WS).or_fail("sign the handshake");
    let socket = polyoxide_ws::connect(request, DEFAULT_CONNECT_TIMEOUT)
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("connect a raw socket through the kit");
    let mut connection = Connection::new(socket, Mode::Data);
    let subscribe = json!({"id": 1, "cmd": "subscribe", "params": {
        "channels": ["orderbook_delta"],
        "market_tickers": [ticker.as_str()],
        "use_yes_price": true,
    }});
    connection
        .send(Message::Text(subscribe.to_string().into()))
        .await
        .map_err(PredictionsWsError::from)
        .or_fail("send the subscribe");
    let deadline = Instant::now() + LISTEN;
    let mut frames = 0usize;
    let mut unmodelled = BTreeSet::new();
    while let Ok(next) = tokio::time::timeout_at(deadline, connection.next()).await {
        let message = next
            .unwrap_or_else(|| transient("the server ended the connection"))
            .map_err(PredictionsWsError::from)
            .or_fail("read a frame");
        let Message::Text(text) = message else {
            continue;
        };
        frames += 1;
        if let Some(found) = unmodelled_keys(text.as_str()) {
            unmodelled.extend(found);
        }
    }
    assert!(frames > 0, "no frame arrived within 30 s");
    assert!(
        unmodelled.is_empty(),
        "the wire sent keys no frame type models: {unmodelled:?}"
    );
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py -q'`

Expected: it fails for `live_predictions_ws`: the target exists (cargo discovers `tests/live_predictions_ws.rs`) but has no `[package.metadata.polyoxide.live.live_predictions_ws]` entry, no `required-features` and no job in `nightly-behavioral.yml`.

- [ ] **Step 3: Implement**

Current (`polyoxide-kalshi/Cargo.toml`, the registration anchor, with the stanzas of Tasks 6.3 and 6.5 above it):

```toml
[[test]]
name = "wire_agreement_predictions_ws"
required-features = ["predictions-ws"]
```

Replace with:

```toml
[[test]]
name = "live_predictions_ws"
required-features = ["predictions-ws"]

[[test]]
name = "wire_agreement_predictions_ws"
required-features = ["predictions-ws"]
```

Add its live table, directly before the docs.rs table (whose list Task 6.2 already extended). Current (`polyoxide-kalshi/Cargo.toml`):

```toml
[package.metadata.docs.rs]
```

Replace with:

```toml
[package.metadata.polyoxide.live.live_predictions_ws]
suite = "predictions"
timeout = 15
features = ["predictions-ws"]
secrets = ["KALSHI_DEMO_API_KEY_ID", "KALSHI_DEMO_PRIVATE_KEY"]

[package.metadata.docs.rs]
```

Regenerate the registry regions, which now hold the nightly job:

Run: `nix develop --command python3 scripts/gen_registry.py --write && git status --short .github/workflows SELF-HEALING.md CLAUDE.md`

Expected: ` M .github/workflows/nightly-behavioral.yml`, ` M SELF-HEALING.md` and ` M CLAUDE.md`, each rewritten only inside its generated regions (`git diff` shows nothing outside a `generated:begin` and `generated:end` pair).

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_live_registry.py tests/test_ci_workflow.py -q'`

Expected: all pass: the target is registered, `#[ignore]`d, built with `predictions-ws`, and handed exactly `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`.

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --test live_predictions_ws -- --list`

Expected: the two tests are listed (`live_a_demo_market_delivers_an_uncrossed_snapshot: test` and `live_frames_carry_no_unmodelled_keys: test`) and none runs.

Run (the owner's key in the environment, so the tests can pass; without it each prints `polyoxide-class=auth-gated` and fails as that, which the nightly skips): `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws --test live_predictions_ws -- --ignored`

Expected with the key: `test result: ok. 2 passed; 0 failed`, about 30 s. A failure prints a `polyoxide-class=` line first; `live_frames_carry_no_unmodelled_keys` naming a key means the capture is stale or Kalshi changed: add the key to its `frames` type as in Task 6.3.

Then the story gate: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && cargo test -p polyoxide-kalshi --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: exit status 0 (`live_unwraps.py` finds no counted site in the new file).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-kalshi/tests/live_predictions_ws.rs
git commit -m "test(kalshi): live_predictions_ws, the order-book socket against the demo host (Story 6)" -- polyoxide-kalshi/Cargo.toml polyoxide-kalshi/tests/live_predictions_ws.rs .github/workflows/nightly-behavioral.yml SELF-HEALING.md CLAUDE.md
```

### Task 6.7: CLAUDE.md, the CI, live-suites and WebSocket paragraphs

**Files:**
- Modify: `CLAUDE.md` in three places, each keyed on its text: the CI paragraph (one named text, Story 1's ``(its `@predictions` file exists)``), the live-suites paragraph (the one that opens `The CLI has live integration tests`) and Module Organization's WebSocket paragraph (the one that opens `**WebSocket** support for the CLOB`)

AD-21: a commit that changes what CLAUDE.md states edits it in the same commit. The socket's rules (a signed handshake on every attempt, `use_yes_price`, the per-`sid` sequence check, the recovery rows) and its four live neighbours are not in CLAUDE.md yet. Two paragraphs are this story's own; the generated regions were regenerated in Tasks 6.2 and 6.6. The live-suites paragraph names four Kalshi live targets, `live_predictions` (Story 2), `live_predictions_venue` (Story 3), `live_predictions_account` (Story 4) and `live_predictions_ws`. The third edit is the one exception to a paragraph having one story: the CI paragraph is Story 1's, whose clause for `polyoxide-kalshi`'s allowlist rule ends with exactly ``(its `@predictions` file exists)`` (the only occurrence in CLAUDE.md), and this story adds the `@predictions-ws` file and its `MUST_BUILD` row (Task 6.2), so it replaces that text and nothing else in the paragraph.

- [ ] **Step 1: Write the failing check**

The check is four greps over CLAUDE.md: the live-suites paragraph must name `polyoxide-kalshi`'s live targets, the WebSocket paragraph must name Kalshi's socket, and the CI paragraph must name the `@predictions-ws` file where it named only `@predictions`'s.

```bash
grep -cF "\`polyoxide-kalshi\`'s \`live_predictions\`" CLAUDE.md
grep -cF "Kalshi's order-book socket (\`polyoxide-kalshi/src/predictions/ws/\`" CLAUDE.md
grep -cF '(its `@predictions` and `@predictions-ws` files exist, and `@predictions-ws` must hold `polyoxide-ws`)' CLAUDE.md
grep -cF '(its `@predictions` file exists)' CLAUDE.md
```

- [ ] **Step 2: Run it to see it fail**

Run the four greps above.

Expected: the first three each print `0` and exit with status 1, and the fourth prints `1`: Story 1's text is still there.

- [ ] **Step 3: Implement**

Current (`CLAUDE.md`, in the CI paragraph, the end of Story 1's clause for `polyoxide-kalshi`'s allowlist rule, part of one long line; this text occurs once):

```
(its `@predictions` file exists)
```

Replace with:

```
(its `@predictions` and `@predictions-ws` files exist, and `@predictions-ws` must hold `polyoxide-ws`)
```

Current (`CLAUDE.md`, the end of the live-suites paragraph, the one that opens `The CLI has live integration tests`, part of one long line):

```
and `polyoxide-binance`'s `live_usdm` and `live_usdm_ws`), gated with `#[ignore]` so CI skips them. They hit the real upstream APIs. Run with `-- --ignored` flag.
```

Replace with:

```
`polyoxide-binance`'s `live_usdm` and `live_usdm_ws`, and `polyoxide-kalshi`'s `live_predictions`, `live_predictions_venue`, `live_predictions_account` and `live_predictions_ws`), gated with `#[ignore]` so CI skips them. They hit the real upstream APIs. Run with `-- --ignored` flag. Kalshi's `live_predictions` and `live_predictions_venue` read production unsigned and need no secrets, so every nightly run reads real markets; `live_predictions_account` and `live_predictions_ws` run against the demo host, which takes `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY` (the PEM text) and nothing else, and are auth-gated without them. Production credentials never reach CI.
```

Current (`CLAUDE.md`, the end of Module Organization's WebSocket paragraph, the one that opens `**WebSocket** support for the CLOB`, part of one long line):

```
the sports feed (`src/sports/`, feature `sports`) and Binance's market streams (`polyoxide-binance/src/usdm/ws/`, feature `usdm-ws`) are separate protocols, each in its own module. No socket crate keeps its own `ensure_crypto_provider` copy: perps, Binance, rtds and sports connect through the kit, and clob upgrades each address's TCP connection through the kit's `connect_over`.
```

Replace with:

```
the sports feed (`src/sports/`, feature `sports`), Binance's market streams (`polyoxide-binance/src/usdm/ws/`, feature `usdm-ws`) and Kalshi's order-book socket (`polyoxide-kalshi/src/predictions/ws/`, feature `predictions-ws`) are separate protocols, each in its own module. No socket crate keeps its own `ensure_crypto_provider` copy: perps, Binance, Kalshi, rtds and sports connect through the kit, and clob upgrades each address's TCP connection through the kit's `connect_over`. Kalshi's socket (`PredictionsWsBuilder`, `SupervisedPredictionsWs`, `PredictionsProtocol` on the kit's `Supervisor`) subscribes `orderbook_delta` with `use_yes_price: true`, so a snapshot's asks are the NO bids in YES pricing and one ladder serves both outcomes. Kalshi refuses a handshake without its API-key headers, public data included, so `connect` takes `Credentials` and the handshake is signed afresh for every attempt, the first and each reconnect: `KALSHI-ACCESS-KEY`, `KALSHI-ACCESS-TIMESTAMP` and `KALSHI-ACCESS-SIGNATURE` over `timestamp_ms + "GET" + the URL's path`, by the signer REST uses. The kit's scripted server records each handshake's headers, which is how the suites prove it. The membership is a set of tickers, replayed as one `subscribe` on every connection and changed with `update_subscription` on the live `sid` (the first add sends a `subscribe`, emptying it an `unsubscribe`); a change is recorded before its frame is sent, so one whose connection dies under it is answered `Ok` and replayed, one Kalshi refuses is undone, and one made during an outage is answered at once and replayed. Every frame that carries `sid` and `seq`, the answers included, is checked per `sid`, and a gap is `PredictionsWsError::SequenceGap` and reconnects. The kit's `request` consumes an answer before `decode` sees it, so the request's correlation closure counts the answers and the pushes it reads while it waits, and `decode` counts a push the closure kept only once; a gap an answer showed is raised by the next frame. It pings every 10 s on the wall clock and counts every frame as liveness, the server's Pings included. Recovery follows the error's class (a refused handshake's 401 or 403 stops the stream; a 429 or 5xx retries after at least its `Retry-After`, through `Protocol::retry_after`), except that a frame that does not decode is skipped, a connection used after it closed reconnects, and an error frame's code decides: 9 stops the stream, 10, 25 and 27 reconnect (Kalshi's AsyncAPI calls 10 and 25 terminal, "the user must resubscribe", which the replay does), and any other is yielded and skipped. Its URL must be `wss://`, or `ws://` to a loopback host, so a signed handshake never travels unencrypted to a remote host.
```

- [ ] **Step 4: Run it to see it pass**

Run the four greps of Step 1.

Expected: the first three each print `1` and exit with status 0, and the fourth prints `0` (and exits with status 1): the CI paragraph's named text is replaced.

Run the story's own Verify line, which ends with the touch-set gate:

```bash
nix develop --command bash -c "cargo test -p polyoxide-kalshi --no-default-features --features predictions-ws && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_classify_coverage.py tests/test_live_registry.py tests/test_ci_workflow.py -q' && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh"
```

Expected: every command passes and the touch-set gate prints nothing. Then the story gate: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings && cargo test -p polyoxide-kalshi --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh'`. Expected: exit status 0. Last, search the new code for the landmines this story must not hold: `grep -rnE 'connect_async|allow_burst|\.gzip\(' polyoxide-kalshi/src polyoxide-kalshi/tests scripts/capture_kalshi_predictions_ws.py` prints nothing (the pattern is `\.gzip\(` and not `gzip`, since Story 1's `tests/headers_predictions.rs` asserts the literal `accept-encoding: gzip`).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(claude): Kalshi's order-book socket, its live targets and its allowlist file (Story 6)" -- CLAUDE.md
```

## Story 7: The guide, validated

- **Developer:** junior-dev
- **Files:** `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md`, `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md`, `_bmad-output/implementation-artifacts/deferred-work.md`, `docs/specs/kalshi/OBSERVED.md`
- **Interfaces:** uses the names of Stories 1 to 6 in prose; exposes nothing
- **Verify:** `python3 scripts/gen_registry.py --check && bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh && git diff --check HEAD~1 HEAD`
- **Review:** light (planning documents; no code)

Story 7 runs last and writes documents only: no Rust, no Python, no Cargo change, so none of the cargo story gates has a package to run on. It renames the trading mapping's keys to `kalshi.predictions:…` and its size unit to `SizeUnit::Shares` (spec D21, D12), re-files and extends `deferred-work.md` (spec §9.5), then runs the audit of spec §9.1 over the epic's diff since `git merge-base main HEAD` and records its results, with amendments A8-1 to A8-10 and any gap the run met, in `spine-amendments/epic-8.md` in `epic-6.md`'s format. The audit has four checks: the touch-set gate prints nothing, every changed hunk of the files with generated regions lies inside a region, the only foundation files changed are Story 5's two, and `polyoxide-kalshi` defines none of the duplication inventory's helpers. `docs/specs/kalshi/OBSERVED.md` is edited, by its anchor, only if the audit finds a probe or capture finding that Stories 1, 2, 4 and 6 did not record. No fixture or finding is ever written by hand (spec D19), so such a gap is recorded as a gap, not filled. The merge of the amendments into the spine and the regeneration of `docs/ARCHITECTURE.md` are an owner session after the run, and this story does neither.

### Task 7.1: Rename the trading mapping's keys and size unit

**Files:**
- Modify: `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md:5`, `:17`, `:37`, `:53`

- [ ] **Step 1: Write the failing check**

The mapping must carry no `kalshi.events` key and no `SizeUnit::Contracts`. The check is a grep that must find neither:

```bash
! grep -n 'kalshi\.events\|SizeUnit::Contracts\|Story 8\.1 moves it' _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md
```

- [ ] **Step 2: Run it to see it fail**

Run the command above. Expected: it prints four lines, `5:` (the sentence that says Story 8.1 moves the file), `17:` and `37:` (the `kalshi.events` keys) and `53:` (`SizeUnit::Contracts`), and exits 1.

- [ ] **Step 3: Apply the four edits**

Edit 1, line 5: the mapping stays where it is (spec D21).

Current (`kalshi-trading-mapping.md:5`, the end of the line):

```
Story 8.1 moves it to `docs/specs/kalshi/trading-mapping.md` when it adds Kalshi's mirror entry, and `deferred-work.md` holds that move.
```

Replace with:

```
Epic 8 leaves it here (spec D21 of `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`), since moving it would edit `docs/s3-migration.md`'s citation, and `deferred-work.md` holds its move to `docs/specs/kalshi/trading-mapping.md` with Kalshi's `Trading`.
```

Edit 2, line 17: the keys, and the key space that Epic 8's Story 1 wrote (spec D1, D8).

Current (`kalshi-trading-mapping.md:17`):

```
A Kalshi key names one outcome: `kalshi.events:<TICKER>:yes` or `kalshi.events:<TICKER>:no` (AD-7). The ticker matches `^[A-Z0-9-]+$` (AsyncAPI `#/components/schemas/marketTicker`). Story 8.3 writes the product's `KeySpace`: its `parse_id` checks the ticker and the `:yes` or `:no` suffix and canonicalizes them, and its `id_text` renders them. A key is made only through it, since `MarketKey` has no other constructor.
```

Replace with:

```
A Kalshi key names one outcome: `kalshi.predictions:<TICKER>:yes` or `kalshi.predictions:<TICKER>:no` (AD-7, with the product `predictions` of Epic 8's spec D1). The AsyncAPI gives the ticker the pattern `^[A-Z0-9-]+$` (`#/components/schemas/marketTicker`), but Kalshi's tickers and the AsyncAPI's own examples carry dots (`FED-23DEC-T3.00`). Epic 8's Story 1 wrote the product's `KeySpace`, `PredictionsKeys` (`polyoxide-kalshi/src/predictions/key.rs`, spec D8): its `parse_id` splits the id at its last `:`, uppercases the ticker and checks it against `[A-Z0-9.-]` (1 to 200 characters), and reads `yes` or `no` after it, and its `id_text` writes `<TICKER>:<outcome>`. A key is made only through it, since `MarketKey` has no other constructor.
```

Edit 3, line 37: the NO position's key.

Current (`kalshi-trading-mapping.md:37`, the line's first sentence):

```
**A NO position is a positive position in `kalshi.events:<TICKER>:no`.**
```

Replace with:

```
**A NO position is a positive position in `kalshi.predictions:<TICKER>:no`.**
```

Edit 4, line 53: the size unit (spec D12: each contract pays one dollar on its outcome, as the CLOB's shares do).

Current (`kalshi-trading-mapping.md:53`, the cell's opening):

```
A `FixedPointCount` string of contracts (`SizeUnit::Contracts`).
```

Replace with:

```
A `FixedPointCount` string of contracts (`SizeUnit::Shares`: each contract pays one dollar on its outcome, as the CLOB's shares do, spec D12 of Epic 8).
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 command again. Expected: it prints nothing and exits 0. Then `grep -o 'kalshi\.predictions' _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md | wc -l` prints `3` (two keys on line 17 and one on line 37).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(specs): the Kalshi trading mapping's keys are kalshi.predictions and its size unit is shares (Epic 8, Story 7)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- _bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md
```

### Task 7.2: Re-file the mapping's move and add Epic 8's deferred work

**Files:**
- Modify: `_bmad-output/implementation-artifacts/deferred-work.md:410-411`, and append after its last line

- [ ] **Step 1: Write the failing check**

The old entry says the move happens in Story 8.1, and the file holds 125 entries (`- source_spec:` lines) with none of Epic 8's. The check wants the old wording gone and 131 entries (the six new ones; the re-filed move stays one entry):

```bash
f=_bmad-output/implementation-artifacts/deferred-work.md
test "$(grep -c 'in Story 8.1, in the commit that registers Kalshi' "$f")" -eq 0 && test "$(grep -c '^- source_spec' "$f")" -eq 131
```

- [ ] **Step 2: Run it to see it fail**

Run the check. Expected: it exits 1, because the old wording is still there (the first `grep -c` prints `1`) and the file has 125 entries.

- [ ] **Step 3: Apply the edits**

Edit 1, re-file the move (spec D21). The entry's `source_spec` line stays as it is; its `summary` and `evidence` lines are replaced.

Current (`deferred-work.md:410-411`):

```
  summary: Move the Kalshi trading mapping from `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` to `docs/specs/kalshi/trading-mapping.md` in Story 8.1, in the commit that registers Kalshi (AD-13), and make `kalshi` a venue-gate term then. Update the citations of the old path in `spine-amendments/epic-7.md` ("7.5: no amendment") and `docs/s3-migration.md`.
  evidence: spec D9. `scripts/gen_registry.py:545-549` fails for a `docs/specs/` directory with no `[workspace.metadata.polyoxide.mirrors.<id>]` table, and that table is Story 8.1's registration, so the mapping stays a planning companion until then.
```

Replace with:

```
  summary: Move the Kalshi trading mapping from `_bmad-output/specs/spec-venue-extensibility/kalshi-trading-mapping.md` to `docs/specs/kalshi/trading-mapping.md` in the story that implements Kalshi's `Trading`, in the commit that edits the mapping's last open rows. Update the citations of the old path in `spine-amendments/epic-7.md` ("7.5: no amendment") and `docs/s3-migration.md` in that commit.
  evidence: Epic 8's spec D21 (`docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`) and Epic 7's spec D9. `docs/specs/kalshi/` and its `[workspace.metadata.polyoxide.mirrors.kalshi]` table exist since Epic 8's Story 1, and `kalshi` is a venue-gate term since that story registered the venue, so the move is open. It edits `docs/s3-migration.md`'s citation, which is outside Epic 8's touch set (D20), so the mapping stayed a planning companion. Epic 8's Story 7 renamed its keys to `kalshi.predictions:…` and its size unit to `SizeUnit::Shares`.
```

Edit 2, append six entries directly after the file's last line, the `evidence:` line that ends with ``checks the Open ack's `created` against the clock.`` (the entry about a maker-only user-channel frame). The file already ends with a newline; the new text starts on the next line.

Append:

```
- source_spec: `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`
  summary: Soak Kalshi's unauthenticated read limit over distinct URLs, with each path probed on the host first, record the measured rate in `docs/specs/kalshi/OBSERVED.md`, and then size an unsigned client's bucket from it instead of Basic's provisional 200 tokens per second and capacity of 600.
  evidence: Spec D5 and §1.3. Kalshi publishes no limit for unauthenticated requests, so `PredictionsThrottle` sizes every client without credentials at Basic's Read budget, and `OBSERVED.md` records that sizing as provisional. Every other measured rate in the repository came from a soak over distinct URLs (perps' `examples/info_soak.rs`, data's `v2_soak`), because a repeated URL can be answered by a cache without reaching the origin. A soak was outside the walking skeleton (spec §12).
- source_spec: `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`
  summary: Give `predictions::ws` a local order book kept from `orderbook_snapshot` and `orderbook_delta`, the `get_snapshot` command of `update_subscription`, and the `ticker` and `trade` channels.
  evidence: Spec D13, D14 and §12. Epic 8's socket carries `orderbook_delta` only and yields the snapshots and deltas it decodes; a gap in a subscription's `seq` reconnects and gets fresh snapshots, where a local book could ask for one with `get_snapshot`. The `ticker`, `trade` and user channels, a local book and `get_snapshot` are listed as not in the epic.
- source_spec: `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`
  summary: Make CI check `polyoxide-kalshi`'s features one at a time, `predictions` and `predictions-ws`, by running `cargo hack check --each-feature --no-dev-deps` on it explicitly, or drop `--ignore-private` once no private member is left that is not a venue (amendment A8-10).
  evidence: CLAUDE.md's CI paragraph gives the features job `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`, and `--ignore-private` skips every `publish = false` member, so `polyoxide-kalshi` is not checked feature by feature on a pull request. The header pin builds `predictions` alone, `test_dep_allowlists.py` reads each feature's tree, and the nightly live jobs build each live target with its declared features alone, but none of those compiles the crate's features as `--each-feature` does. The cargo-hack step in `ci.yml` is outside Epic 8's touch set (spec D20).
- source_spec: `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`
  summary: Decide how `Candles` serves a venue that offers fixed widths, either per-venue widths or a refusal of a width the venue lacks, and then implement `Candles` for Kalshi, whose candlesticks come at 1, 60 and 1440 minutes (amendment A8-3).
  evidence: Spec D10. `CandleInterval` names every width from one minute to one week, and AD-6 allows no method that answers "unsupported", so an implementation for a venue that serves three widths cannot be written without one or the other. Epic 8 implements `MarketData`, `Trades` and `EventGroups` for `Predictions` and leaves `Candles` out.
- source_spec: `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`
  summary: Adopt Kalshi's schema corrections dated 2026-10-22 when `nightly-schema.yml` files its drift issues for the `kalshi` and `kalshi-ws` specs: re-diff against the live documents, re-capture the fixtures a correction touches, and run the wire and spec agreement suites.
  evidence: Spec §3.6 and §12. Kalshi's changelog announced the corrections, so the vendored `docs/specs/kalshi/openapi.yaml` (3.34.0) and `asyncapi.yaml` (2.0.0) will drift. Adopting a drift is the issue's `curl`, outside Epic 8, and the mirrors must stay byte for byte as served.
- source_spec: `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`
  summary: Mention Kalshi in the hand-written docs that Epic 8 left alone, in a session that may edit them: `.env.example` (add `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`), `SELF-HEALING.md`'s intro and its secrets sentence (outside the generated regions, which name only Polymarket's secrets), the hand-written lead-ins of `docs/specs/INDEX.md` and `README.md`, CLAUDE.md's Key Patterns default-concurrency list and its Environment Variables section, and the vacuity guards of `.github/scripts/tests/test_classify_coverage.py`, which list no Kalshi type.
  evidence: Spec D20 and §9.1. These files are outside Epic 8's touch set or its pre-approved CLAUDE.md paragraphs, and Story 7's audit forbids hand edits outside generated regions in `README.md`, `docs/specs/INDEX.md` and `SELF-HEALING.md`, so the run leaves them and the docs do not yet say Kalshi is a venue.
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 check. Expected: it exits 0 (no output), with the first `grep -c` at `0` and the entry count at `131`. Then `tail -n 3 _bmad-output/implementation-artifacts/deferred-work.md` ends with the hand-written-docs entry's `evidence:` line, and `git diff --check` over the file prints nothing.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(deferred-work): re-file the Kalshi mapping's move and add Epic 8's deferred work (Epic 8, Story 7)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- _bmad-output/implementation-artifacts/deferred-work.md
```

### Task 7.3: Run the audit and write the amendments file

**Files:**
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md`
- Modify: `docs/specs/kalshi/OBSERVED.md` (by its anchor, only if the audit finds a gap)

- [ ] **Step 1: Write the failing check**

The amendments file must exist and hold the ten amendments and the audit:

```bash
f=_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md
test -f "$f" && test "$(grep -c '^## A8-' "$f")" -ge 10 && grep -q '^## Audit of the epic' "$f"
```

- [ ] **Step 2: Run it to see it fail**

Run the check. Expected: it exits 1, because the file does not exist.

- [ ] **Step 3: Run the audit and read the four results**

Run these from the repository root, in order. Each prints its result; none changes a file. If any result is not the expected one, stop: a path outside the allowlist, a hunk outside a generated region, a foundation file other than Story 5's two, or a copied helper is an edit outside the epic's touch set (spec D20, §9.1), and it goes to the owner. Do not write the file over it.

Check 1, the touch-set gate.

```bash
bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh; echo "exit $?"
```

Expected: no path is printed, and the last line is `exit 0`.

Check 2, the generated regions. This reads each changed hunk of the five files and tests it against the file's `generated:begin` and `generated:end` markers.

```bash
python3 -I - <<'PY'
import re
import subprocess
import sys

base = subprocess.run(
    ["git", "merge-base", "main", "HEAD"], capture_output=True, text=True, check=True
).stdout.strip()
files = [
    "README.md",
    "docs/specs/INDEX.md",
    "SELF-HEALING.md",
    ".github/workflows/nightly-behavioral.yml",
    ".github/workflows/nightly-schema.yml",
]
edge = re.compile(r"^\s*(?:<!-- |# )generated:(begin|end) (\S+?)(?: -->)?\s*$")
hunk = re.compile(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", re.M)
outside = []
for path in files:
    spans, opened = [], None
    with open(path, encoding="utf-8") as handle:
        for number, line in enumerate(handle.read().split("\n"), 1):
            found = edge.match(line)
            if found and found[1] == "begin":
                opened = number
            elif found:
                spans.append((opened, number))
    diff = subprocess.run(
        ["git", "diff", "-U0", base, "--", path], capture_output=True, text=True, check=True
    ).stdout
    for found in hunk.finditer(diff):
        start = int(found[1])
        count = 1 if found[2] is None else int(found[2])
        if count == 0:
            inside = any(begin <= start < end for begin, end in spans)
        else:
            inside = any(begin < start and start + count - 1 < end for begin, end in spans)
        if not inside:
            outside.append(f"{path}: {found[0]}")
print("\n".join(outside) or "every changed hunk lies inside a generated region")
sys.exit(1 if outside else 0)
PY
```

Expected: `every changed hunk lies inside a generated region`, and exit 0.

Check 3, the foundation.

```bash
git diff --name-only "$(git merge-base main HEAD)"..HEAD -- polyoxide-venue polyoxide-core polyoxide-ws polyoxide-test-support
```

Expected: exactly these two lines, Story 5's files.

```
polyoxide-ws/src/test_server/mod.rs
polyoxide-ws/src/test_server/tests.rs
```

Check 4, no copies. The first command greps the crate for a local definition of each shared helper named in `duplication-inventory.md`, and the second does the same for the capture scripts and `scripts/capture_common.py`'s functions.

```bash
names='ensure_crypto_provider|Backoff|Timing|WsError|retrying_can_fix|connect_over|with_timeout|Recovery|ScriptedServer|next_event|next_item|drain|parse_retry_after|retry_delay|UnixMillis|OutputFormat|excerpt|key_paths|assert_values_agree|Ledger|Excuse|Pacer|percentile|parse_stages|parse_routes|MessageVisitor|ThrottleLayer|headers_sent|keys_sent|pairs_sent|query_keys_sent|current_timestamp|Hold|CapacityBucket|WindowQuotaTable|RateLimiter'
grep -rnE "(fn|struct|enum|trait|type|const|macro_rules!)[[:space:]]+(${names})\b" polyoxide-kalshi/ ; test $? -eq 1 && echo "no local copy in polyoxide-kalshi"
grep -nE "^[[:space:]]*(async )?def (get|require_ok|write_json|write_raw|stamp|provenance_table|write_provenance|ws_session)\(" scripts/capture_kalshi_*.py ; test $? -eq 1 && echo "no local copy in the capture scripts"
```

Expected: `no local copy in polyoxide-kalshi` and `no local copy in the capture scripts`, with no hit printed above them. The spec-agreement suite's own YAML reading is not a copy: test-support's `agreement` and `openapi` helpers read JSON only, which A8-9 records, so a helper there that reads the YAML mirror is excused and is not in the list. If a hit is printed, read it: a definition that does the shared helper's job is a copy and stops the story, and a name that means something else is listed in the audit section under "Names that matched and were read" before the file is written.

Check 5, `OBSERVED.md` (spec §9.3). Read `docs/specs/kalshi/OBSERVED.md` against this list and confirm each item has a section or bullet:

- Story 1's: the eight probes of spec §1.3, the AsyncAPI's ticker pattern and Kalshi's three error shapes, the rate limits with what is provisional (Basic sizing, anonymous limits, `min_size` and `size_increment`), and the per-shard Write and balance buckets recorded but not built, with the 429's missing `Retry-After`.
- Story 2's (spec §4.4): the unfiltered open listing's first ten pages were 9,996 multivariate combinations of 10,000 rows, so every walk pages `status=open&mve_filter=exclude`, and any key the wire carries that the schema lacks, excused in `OBSERVED_EXTRA`.
- Story 4's (spec §6.7): what the demo host's `/account/limits` capture showed.
- Story 6's (spec §8.12): each subscription's first `seq`, whether `ok` and `unsubscribed` advance `seq`, whether the snapshot's book is not crossed (which shows `use_yes_price` is honoured), and any bound on tickers per subscription it met.

Confirm also that the file's last line is still the anchor:

```bash
tail -n 1 docs/specs/kalshi/OBSERVED.md
```

Expected: `<!-- capture findings go above this line -->`.

If every item is there, `OBSERVED.md` is not edited. If an item is missing, insert this section directly above the anchor line, with one bullet per missing item, naming it as the list above does. A finding is never written from memory and a fixture is never written by hand, so the section records only that the item is absent:

```
## Findings not recorded by the stories that owed them (Epic 8, Story 7)

- Not recorded by Story <1, 2, 4 or 6>: <the item, as the list above names it>.
```

and the audit section written in Step 4 states `OBSERVED.md` gap: yes, with the same items.

- [ ] **Step 4: Write the amendments file**

Create the file with the content below. It is `epic-6.md`'s format: a heading, the AD-21 sentence, then one `## A8-<n>` section for each amendment with the bullets "Proposed by", "Current rule", "Proposed rule", "Why" and "Follow-up in the code". The audit section is the file's last section. If the dispatch of this story lists gaps that Stories 1 to 6 met, insert one section for each directly after A8-10 and before the audit section, numbered from A8-11 in the order listed, in this same bullet form and with the story that met it named in "Proposed by"; if it lists none, insert none.

````markdown
# Proposed spine amendments: Epic 8

Recorded per AD-21: an epic records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide.

## A8-1: the Kalshi product is `predictions` (amends the examples of AD-4 and AD-7)

- **Proposed by:** the owner, 2026-10-18, during Epic 8's design (spec D1, in `docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`).
- **Current rule:**
  - AD-7 writes a Kalshi key as `kalshi.events:<ticker>:yes|no`, so the product is `events`.
  - AD-4's example of a canonicalized id is "Kalshi `<TICKER>:yes|no`".
- **Proposed rule:**
  - The Kalshi product is `predictions`, and AD-7's example is `kalshi.predictions:<ticker>:yes|no`. AD-4's example id, `<TICKER>:yes|no`, is unchanged.
  - The module is `polyoxide_kalshi::predictions`, its features are `predictions` and `predictions-ws`, and its suites are named `<kind>_predictions`.
  - `margin` stays free for Kalshi's perpetual futures.
- **Why:**
  - **Kalshi split its API.** It now has a Predictions exchange (`/trade-api/v2/…`) and a Margin exchange (`/trade-api/v2/margin/…`, perpetual futures).
  - **`events` collides.** It is also the name of Kalshi's own `/events` group and of every `Event` type, so a module named `events` reads as the wrong thing in both.
- **Follow-up in the code:** Story 1 declares the venue `kalshi` and the product `predictions` in `polyoxide-kalshi`'s `[package.metadata.polyoxide]` and writes `predictions::key`. Story 7 renamed the trading mapping's keys. The merge session edits AD-7's example and the guide's matching text.

## A8-2: the recipe's `RetryPolicy` is optional when core's default fits (amends the guide's step 3 and AD-17)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec D6).
- **Current rule:** step 3 of "Adding a venue" reads "Implement the venue's `Throttle` (compose core primitives), `RetryPolicy` and `Authenticator`". AD-17 says the loop retries only the venue policy's set.
- **Proposed rule:** a venue implements a `Throttle` and an `Authenticator`, and a `RetryPolicy` only when the statuses it retries or the hold it applies differ from core's `DefaultRetryPolicy`, which retries a 429 and holds every request for `RetryConfig::retry_delay(0)` and fails every other status, so it retries no 5xx, 408 or 425.
- **Why:**
  - **Kalshi's set is core's.** It sends no `Retry-After` and promises no cooldown, and its 429 body is `{"error":"too many requests"}`, so the default's floor is the whole policy.
  - **A copy would be a second definition.** A venue policy that restated the default would be a duplicate of it, which the duplication inventory exists to prevent.
- **Follow-up in the code:** Story 1 runs `Predictions` on core's `DefaultRetryPolicy`.

## A8-3: `CandleInterval` promises every width, so a venue that serves some cannot implement `Candles` (amends AD-6)

- **Proposed by:** the owner, 2026-10-18, who chose which traits Epic 8 implements (spec D10).
- **Current rule (AD-6):** optional abilities are separate traits, implemented only where offered, and no method returns `Unsupported`. `CandleInterval` names one minute to one week, each width every candle venue serves.
- **Proposed rule:** this is a proposal for the merge session to decide between. Either `Candles` lets a venue declare the `CandleInterval`s it serves, so a caller can ask before it calls, or a venue refuses a width it lacks as `InvalidRequest`, sent nowhere, as a key of another product is refused.
- **Why:**
  - **Kalshi serves three widths.** Its candlesticks come at 1, 60 and 1440 minutes.
  - **No implementation fits the rule as written.** The trait promises every width, and AD-6 forbids a method that answers "unsupported", so a venue with three widths can implement neither the whole trait nor a part of it.
- **Follow-up in the code:** Epic 8 leaves `Candles` unimplemented for Kalshi, and `deferred-work.md` holds the decision and the implementation.

## A8-4: the scripted test server records each handshake's request headers (amends AD-13's touch list and the guide's step 8)

- **Proposed by:** the owner, 2026-10-18, during Epic 8's design (spec D17).
- **Current rule:** a new venue touches only the files AD-13 lists, and Epic 8's own acceptance criteria add "no edit to `polyoxide-{venue,core,ws,test-support}`".
- **Proposed rule:** the list gains one additive, venue-neutral edit: the kit's scripted server, `polyoxide_ws::test_server`, which is doc-hidden and behind `test-server`, records each accepted handshake's request headers (`ScriptedServer::handshake_headers()`: names lowercase, in accept order, and a refused upgrade records none). It names no venue.
- **Why:**
  - **Kalshi signs its handshake on every attempt.** `PredictionsProtocol::handshake` signs `timestamp_ms + "GET" + <path>` afresh, with the signer REST uses.
  - **The server showed no headers.** It recorded paths and frames, so a suite could not show that a reconnect signs again instead of reusing the first signature, which is what a bug would look like.
- **Follow-up in the code:** Story 5 adds the recorder and its test, and edits CLAUDE.md's scripted-server paragraph (AD-21). Story 6's supervision suite proves a fresh signature on every attempt through it. It is additive, so `docs/s3-removals.md` gains nothing.

## A8-5: a new venue's header pins, and the umbrella's full-pin rule (amends AD-22 and the guide's step 8)

- **Proposed by:** the owner, 2026-10-18, who approved the CI edits (spec D20) and ran the flow profile's matching change (spec D24).
- **Current rule:**
  - CI derives a header pin, `tests/headers_<module>.rs`, for every venue crate that builds core, and the features job's two header-pin steps list each one: the minimal build, and the umbrella's `full`.
  - `test_the_features_job_runs_each_header_pin_under_the_umbrellas_full` asserts that the umbrella's `full` turns each such module on.
  - The recipe's list of what a new venue touches names no CI file.
- **Proposed rule:**
  - A new venue adds its pin to both steps (`cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`, and `cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions`).
  - The "`full` turns each module on" assertion applies only to the venue crates the umbrella depends on, read from the umbrella's `dependencies` in the same metadata. A `publish = false` venue stays outside the umbrella and still runs its pin under `full`.
  - The flow profile's final `features` gate runs the same two commands, each guarded by `[ ! -d polyoxide-kalshi ] ||`, since every worktree shares the profile.
- **Why:** a published umbrella cannot depend on a `publish = false` crate, so the assertion could never hold for Kalshi, and without the two lines Kalshi's headers go unpinned in the one build that unifies every other venue's features.
- **Follow-up in the code:** Story 1 edits `ci.yml` and `test_ci_workflow.py`, with a new test that pins the rule both ways on stand-in metadata.

## A8-6: hand-written registration points the recipe does not name (amends AD-13's "A new venue touches only" and the guide's step 8)

- **Proposed by:** the owner, 2026-10-18 (spec D20 and D23).
- **Current rule:** AD-13 says registration is Cargo metadata, and lists what a new venue touches: its `members` line, its pins, its crate, its specs, its capture scripts, its allowlist files, its amendments file, `_bmad-output/specs/**` and regenerated regions.
- **Proposed rule:** the list also names the places a venue is registered by hand, or each one is derived from metadata:
  - `test_dep_allowlists.py`'s `PER_FEATURE` rule for the crate and its `MUST_BUILD` row for a socket file, since a crate with no `PER_FEATURE` entry is checked with its default features against the socket-only deny-list, which refuses `reqwest` and `polyoxide-core`, so any venue with an HTTP module fails it;
  - CLAUDE.md's hand-written paragraphs that list the venue crates, the header pins, the mock and live suites and the allowlist rules.
- **Why:** Kalshi's `PER_FEATURE` rule is `alloy*`, `hmac` and `keyring` never admitted, `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `predictions-ws`, and `reqwest`, `polyoxide-core`, `governor` and `sha2` with `predictions`, which every file names. The `MUST_BUILD` row requires `polyoxide-ws` in the socket file, so the socket rule cannot pass vacuously.
- **Follow-up in the code:** Story 1 adds the `PER_FEATURE` rule and edits the CLAUDE.md paragraphs it names. Story 6 adds the `MUST_BUILD` row and edits the paragraphs on live suites and sockets.

## A8-7: where "on first use" happens (amends the guide's step 3 and AD-10)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec D5).
- **Current rule:** step 3 says that when a venue serves its limits or per-route costs from an endpoint, they are read on first use, with a drift test of the route table against the costs endpoint.
- **Proposed rule:**
  - The read runs in the client, before its first send, not in the throttle: one confirmation, run once by a `tokio::sync::OnceCell` that every route builder's `send` waits on, `health().ping()` included.
  - It reads the costs endpoint and replaces each compiled cost it lists. With credentials it then reads the limits endpoint and resizes the bucket in place (`CapacityBucket::resize`). It always ends confirmed: a failed read or a zero limit keeps the compiled costs or the provisional sizing, with one warning.
  - Costs are computed when a request is sent, after the confirmation, not when its builder is made.
- **Why:** a `Throttle` is a hook inside core's send loop, so one that sent a request would re-enter the loop. The step said "on first use" without saying where, and the throttle was the wrong place.
- **Follow-up in the code:** Story 1 writes the `before_send` stub and Story 4 replaces it with the confirmation. Story 2's `live_predictions` holds the drift test of the route table against `/account/endpoint_costs`.

## A8-8: the umbrella's generic example cannot be included without every venue's features (amends A6-2)

- **Proposed by:** the owner, 2026-10-18 (spec D22).
- **Current rule (A6-2):** the guide links `polyoxide/examples/venue_neutral.rs`, which reads the best bid and ask of each venue with one `best_bid_ask(&DynMarketData, &MarketKey)` and their errors with one classifier generic over `Classify`, and says "A new venue whose products implement the traits needs no change to it."
- **Proposed rule:** the generic half moves into a file of its own that names no venue, which the umbrella's example includes by `#[path]` and a venue's suite can include with no feature of another venue.
- **Why:** the example imports Polymarket's and Binance's types and has a `main` that uses them. Kalshi's suite includes it by `#[path]`, so its dev-dependency on `polyoxide` (with `default-features = false`) turns on the example's `required-features`, `polymarket-clob`, `polymarket-perps` and `binance-usdm`, and its tokio dev-dependency has `macros` and `rt-multi-thread`. That builds the Polymarket and Binance stacks into Kalshi's test builds. They are dev-only, so no `cargo tree -e normal` and no allowlist changes. The same unification means Kalshi's own "minimal build" header pin (`cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`) is not minimal from Story 3 on: it carries the other venues' features, which the umbrella-full pin exists to unify. It still passes, since core alone declares reqwest's features. The proposed split would also restore the minimal pin, because the generic half would need no dev-dependency on the umbrella's example.
- **Follow-up in the code:** Story 3 carries the dev-dependency and the include. The umbrella's files are not edited by Epic 8, and the merge session decides the split.

## A8-9: test-support's agreement and OpenAPI helpers read JSON only (amends the guide's step 6)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec D19).
- **Current rule:** step 6 says to add `wire_agreement_*` and `spec_agreement_*` tests using the test-support helpers.
- **Proposed rule:** a venue that publishes its spec as YAML reads it through a dev-only YAML parser in its own suite, or `polyoxide-test-support` gains a YAML reader behind a feature. The merge session decides between them.
- **Why:** Kalshi publishes no JSON (`openapi.json` and `asyncapi.json` are 404), and no YAML crate was in the tree, so Epic 8 added `serde-saphyr` as a dev-dependency of `polyoxide-kalshi` alone. A dev-dependency is in no `cargo tree -e normal`, so `ci/dep-allowlists/` is unchanged by it. Nor can test-support's `openapi::check` and `query::documented_parameters` read Kalshi's OpenAPI as served, so Story 2's spec-agreement suite normalises its own copy: it inlines `$ref` parameters, collapses single-arm `allOf` and gives property-less objects empty properties.
- **Follow-up in the code:** Story 2 adds the pin to the root `Cargo.toml` and the dev-dependency, and its `spec_agreement_predictions.rs` reads `docs/specs/kalshi/openapi.yaml` through it.

## A8-10: cargo-hack's `--ignore-private` leaves a `publish = false` venue's features unchecked (amends AD-22)

- **Proposed by:** Epic 8's design, 2026-10-18 (spec §9.2).
- **Current rule (AD-22):** `cargo hack check --each-feature --no-dev-deps` runs on every venue crate. CI's features job runs it as `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`.
- **Proposed rule:** the features job also runs `cargo hack check -p polyoxide-kalshi --each-feature --no-dev-deps`, or `--ignore-private` is dropped once no private member is left that is not a venue.
- **Why:** `--ignore-private` skips every `publish = false` member, so `polyoxide-kalshi`'s features, `predictions` and `predictions-ws`, are not checked one at a time on a pull request. A feature that compiles only beside another would pass unseen. Epic 8 covers the common case with Kalshi's header pin, which builds `predictions` alone, and the nightly live jobs, which build each live target with its declared features alone; neither is `--each-feature`.
- **Follow-up in the code:** none in Epic 8: the cargo-hack step is outside its touch set (spec D20). `deferred-work.md` holds it.

## Audit of the epic's diff (spec §9.1)

Run over the range from `git merge-base main HEAD` to `HEAD`, by Story 7.

- **The touch set.** `bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh` printed no path and exited 0.
- **Generated regions.** Every changed hunk of `README.md`, `docs/specs/INDEX.md`, `SELF-HEALING.md`, `.github/workflows/nightly-behavioral.yml` and `.github/workflows/nightly-schema.yml` lies inside a `generated:begin` and `generated:end` region.
- **The foundation.** The only files changed in `polyoxide-venue`, `polyoxide-core`, `polyoxide-ws` and `polyoxide-test-support` are `polyoxide-ws/src/test_server/mod.rs` and `polyoxide-ws/src/test_server/tests.rs` (Story 5, A8-4). Their `git diff --stat` is below.
- **No copies.** `polyoxide-kalshi` and its capture scripts define none of the helpers named in `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md`: a grep for each name defined locally (`fn <name>`, `struct <name>` and the other item keywords) found none.
- **`OBSERVED.md`.** Every probe and capture finding of spec §3.6, §6.7 and §8.12 is recorded in `docs/specs/kalshi/OBSERVED.md`.
````

After creating the file, append the foundation's `git diff --stat` to the audit section mechanically, so the file holds the command's own output. The audit section is the file's last section and its last bullet is the `OBSERVED.md` one, so the append goes at the end of the file:

```bash
f=_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md
{
  printf '\n```\n'
  git diff --stat "$(git merge-base main HEAD)"..HEAD -- polyoxide-venue polyoxide-core polyoxide-ws polyoxide-test-support
  printf '```\n'
} >> "$f"
```

If Check 5 found `OBSERVED.md` items missing, make two more changes. In the file's last bullet, replace `Every probe and capture finding of spec §3.6, §6.7 and §8.12 is recorded in \`docs/specs/kalshi/OBSERVED.md\`.` with `\`docs/specs/kalshi/OBSERVED.md\` lacks the findings its last section lists (Check 5); the owner re-probes them.` And if Check 4 printed a name that matched and was read as something else, add a bullet to the audit section naming each such file and name, ending "Names that matched and were read: not a copy, because it reads the YAML mirror (A8-9)", or the real reason.

- [ ] **Step 5: Run it to see it pass, and commit**

Run the Step 1 check. Expected: it exits 0, with the `## A8-` count at 10 (more if gaps were appended). Then:

```bash
nix develop --command python3 scripts/gen_registry.py --check
bash _bmad-output/specs/spec-venue-extensibility/epic-8-touch-set.sh
```

Expected: both print nothing and exit 0.

Commit the amendments file, and `OBSERVED.md` only if Check 5 edited it:

```bash
git add -N _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md
git commit -m "docs(spine): Epic 8's amendments A8-1 to A8-10 and the audit of its diff (Epic 8, Story 7)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_017vBjh93Z7zxb4fXNfG1fks" -- _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-8.md
```

(When `OBSERVED.md` was edited, add `docs/specs/kalshi/OBSERVED.md` after the last path.) Last, the story's done-check: `git diff --check HEAD~1 HEAD`, which prints nothing, and the whole **Verify** line. No cargo gate runs: the story changes no package.
