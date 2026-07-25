# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Polyoxide is a Rust SDK toolkit for Polymarket APIs. It provides library crates for CLOB trading, market data (Gamma), user data, gasless relay transactions, Python bindings, and a standalone CLI. Hard fork of [polyte](https://github.com/roushou/polyte).

## Build & Development Commands

**MSRV:** 1.91 (set in workspace `Cargo.toml`).

```bash
# Build entire workspace
cargo build --all-features --workspace

# Build a single crate
cargo build -p polyoxide-clob

# Run all tests
cargo test --all-features --workspace

# Test a single crate
cargo test -p polyoxide-clob --all-features

# Run a single test by name
cargo test -p polyoxide-clob --all-features -- test_name

# Lint (must pass with zero warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check

# Format fix
cargo fmt --all
```

CI runs three jobs: **format** (standalone), **lint & test** (clippy, `cargo nextest run`, doctest — sequentially in one job), and **python bindings** (`uv run pytest tests/` in `polyoxide-py`, gated on **format** passing). Clippy uses `-D warnings` (all warnings are errors).

```bash
# Run live integration tests (hit real APIs, skipped in CI)
cargo test -p polyoxide-clob --test live_api -- --ignored
```

## Workspace Architecture

Eight crates with this dependency graph:

```
polyoxide-core          (shared: auth, HTTP client, errors, macros)
├── polyoxide-relay     (gasless transactions via Polygon relayer)
├── polyoxide-gamma     (read-only market data API)
├── polyoxide-data      (read-only user positions/trades API)
├── polyoxide-clob      (order book trading, depends on core; gamma optional, default-on)
│   └── polyoxide        (unified client re-exporting clob/gamma/data, feature-gated)
├── polyoxide-cli       (CLI tool using clap)
└── polyoxide-py        (Python bindings via PyO3 + maturin, publish = false)
```

Note: `polyoxide-cli` does **not** depend on the unified `polyoxide` crate. It depends directly on the component crates — `polyoxide-clob` (with `ws`), `polyoxide-data`, and `polyoxide-gamma` — plus `polyoxide-core` and `polyoxide-relay` only under the optional `keychain` feature.

The CLI's `clob` command group currently exposes `clob prices download` — a bulk,
resumable, rate-limited downloader for CLOB historical price data
(`GET /prices-history`) that writes per-market CSV/JSONL/Parquet dataset files
plus a `manifest.jsonl`. Parquet output requires building the CLI with the
`parquet` feature.

**polyoxide** (the unified crate) uses feature flags: `clob`, `gamma`, `data`, `ws` (WebSocket), `full` (all). Default = clob + gamma + data.

## Key Patterns

**Builder pattern** — All clients use builders: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`, `Gamma::builder()`, `DataApi::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`.

**API namespaces** — Clients organize endpoints into namespaces:
- CLOB: `clob.markets()`, `clob.orders()`, `clob.account_api()`, `clob.health()`, `clob.auth()`, `clob.rewards()`, `clob.public_rewards()`, `clob.notifications()`
- Gamma: `gamma.markets()`, `gamma.events()`, `gamma.series()`, `gamma.tags()`, `gamma.comments()`, `gamma.sports()`, `gamma.search()`, `gamma.user()`, `gamma.health()`
- Data: `data.user(addr)`, `data.trades()`, `data.holders()`, `data.leaderboard()`, `data.builders()`, `data.live_volume()`, `data.open_interest()`, `data.market_positions()`, `data.combos()`, `data.misc()`, `data.pnl()`, `data.rankings()`, `data.accounting()`, `data.health()`

`data.pnl()` and `data.rankings()` target sibling hosts (`user-pnl-api` and `lb-api`) that have **no published spec** — see `docs/specs/undocumented/INDEX.md`. Their base URLs are configurable via `DataApiBuilder::pnl_base_url` / `rankings_base_url`, and all three hosts share one connection pool and concurrency budget via `HttpClient::with_base_url`.

`clob.rewards()` requires an `Account`; `clob.public_rewards()` exposes the
subset that is public upstream (`/rewards/markets/current`,
`/rewards/markets/{condition_id}`, `/rewards/markets/multi`,
`/rebates/current`) without one.

Example: `gamma.markets().list().open(true).send().await?`, `data.leaderboard().get().send().await?`.

**Request builder fluency** — Query parameters are chained with builder methods before `.send().await?`.

**Two auth layers, three signing schemes** — managed through the `Account` type in `polyoxide-clob/src/account/`. Don't conflate them; they use different EIP-712 domains and are verified by different parties:

| Scheme | Used for | Shape |
|--------|----------|-------|
| **L1** | Creating/deriving API credentials (`/auth/api-key`, `/auth/derive-api-key`) | EIP-712 `ClobAuth`, domain `ClobAuthDomain` v1, **no `verifyingContract`** |
| **L2** | Everything else authenticated — orders, balances, trades | HMAC-SHA256 over `timestamp + method + path [+ body]`, url-safe base64 |
| **Order signing** | The signed order payload itself, posted under L2 | EIP-712 `Order`, domain `Polymarket CTF Exchange` v2, **with** `verifyingContract` |

The two EIP-712 domains are unrelated — order signing needs a verifying contract, L1 auth must not have one. See `docs/specs/clob/auth.md` for both type strings; `polyoxide-clob/src/core/eip712.rs` pins them against golden vectors from `py-clob-client`.

**Error hierarchy** — `ApiError` in core, wrapped by crate-specific errors (`ClobError`, `GammaError`, `DataApiError`, `RelayError`). The `impl_api_error_conversions!` macro in core wires up `From` conversions.

**Retriability** — `ApiError::is_retriable()` (and `ClobError::is_retriable()`) is the canonical classifier for callers' retry policies: true for rate limits, timeouts, connection failures, `425 Too Early`, and 5xx. The crates' *own* retry loop is narrower — `HttpClient::should_retry` only ever retries `429`.

**Order kill outcomes are not faults** — Polymarket returns HTTP 400 for both genuine faults and the defined kill outcomes of marketable orders, so `ClobError` splits the latter out as `FakUnmatched` (FAK matched nothing) and `FokUnfilled` (FOK could not fill in full). They are deterministic and never retriable. Classification lives in `classify_order_kill` in `polyoxide-clob/src/error.rs` and matches on the venue's message body — the only signal available, since the venue ships no error code. Upstream's error catalogue is [docs.polymarket.com/resources/error-codes](https://docs.polymarket.com/resources/error-codes); it is **not** in `docs/specs/clob/openapi.yaml`, which omits these rows entirely.

**Decimal precision** — Price/size fields use `rust_decimal::Decimal` with `serde(with = "rust_decimal::serde::str")` for string serialization.

## Environment Variables

For authenticated operations (CLOB trading, user data):
```
POLYMARKET_PRIVATE_KEY        # Hex-encoded private key
POLYMARKET_API_KEY            # L2 API key
POLYMARKET_API_SECRET         # L2 API secret (base64)
POLYMARKET_API_PASSPHRASE     # L2 API passphrase
```

Relay operations need either `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` (HMAC auth) **or** `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS` (static key auth). Relay also reads `RELAYER_URL` and `CHAIN_ID` optionally.

**Keychain alternative** — With the `keychain` feature enabled, credentials can be stored in and loaded from the OS keychain instead of environment variables. Use `Account::from_keychain()` (CLOB), `BuilderAccount::from_keychain()` (Relay), or the CLI `polyoxide credentials store/show/delete` subcommands. The `keychain` feature is optional and not enabled by default.

## API Specs

Upstream Polymarket API documentation lives in `docs/specs/`. See `docs/specs/INDEX.md` for the full index. These are the source of truth for endpoint contracts, rate limits, and response schemas — sourced from https://docs.polymarket.com and the official OpenAPI specs.

**Not yet implemented.** `docs/specs/` also mirrors three upstream APIs that no
polyoxide crate covers: **Perps** (`perps/`, 43 endpoints on
`api.perpetuals.polymarket.com`, with its own `POLYMARKET-PROXY` /
`POLYMARKET-SECRET` header auth rather than the L1/L2 scheme), **Bridge**
(`bridge/`, 5 endpoints), and **Combos RFQ** (`combos-rfq/`, 4 endpoints). They
are mirrored so parity audits can see them; adding client support for any of
them is a separate piece of work.

For the upstream hosted docs, [`docs/specs/polymarket-llms.txt`](docs/specs/polymarket-llms.txt) is a snapshot of Polymarket's own documentation index (`https://docs.polymarket.com/llms.txt`) — a flat list of every doc page (with `.md` URLs) covering CLOB/auth/orders, builder attribution, and the CLOB V2 migration. Use it to locate the authoritative upstream page for a topic when the local `docs/specs/` copies are insufficient.

## Testing Conventions

Each crate has live integration tests in `tests/live_api.rs` gated with `#[ignore]` so CI skips them. They hit the real Polymarket APIs. Run with `-- --ignored` flag.

Read-only crates (gamma, data) use `Gamma::builder().build()` / `DataApi::builder().build()` directly. CLOB tests use `Clob::public()` for unauthenticated endpoints.

Mock HTTP tests use `mockito` (workspace dev-dependency). Each crate with mock tests has a `tests/mock_api.rs` file with helper functions like `test_public_clob(server)` that point clients at the mock server URL.

## Module Organization

Most crates follow a consistent layout:
- `lib.rs` — public API re-exports
- `client.rs` — main client struct + builder
- `error.rs` — crate-specific error enum (uses `thiserror`)
- `types.rs` — domain types
- `api/` — namespace modules, one file per API group (markets, orders, etc.)

**WebSocket** support lives in `polyoxide-clob/src/ws/` (not core), feature-gated behind `ws` (not enabled by default in polyoxide-clob; default = `["gamma"]`). Three channels: `WebSocket::connect_market(asset_ids)` (public), `WebSocket::connect_user(condition_ids, credentials)` (authenticated), and `WebSocket::connect_sports()` (public, served by `sports-api.polymarket.com` and taking no subscription payload). Implements `futures_util::Stream`. `WebSocketBuilder` provides auto-ping keep-alive for long-running connections.

Three market events — `best_bid_ask`, `new_market`, `market_resolved` — are withheld by the server unless the subscription sets `custom_feature_enabled`. Use `WebSocket::connect_market_with(ids, MarketSubscriptionOptions::default().with_custom_features())` to receive them. `MarketMessage` and `Channel` are `#[non_exhaustive]`, since upstream adds event types over time.

The WebSocket contracts are published as AsyncAPI, not OpenAPI — mirrored in `docs/specs/clob/asyncapi-{market,user,sports}.json`. A parity audit that only diffs the OpenAPI files will miss this whole surface.

## Publishing Order

Crates must be published in dependency order: core → relay → gamma → data → clob → polyoxide. The release workflow in `.github/workflows/release.yml` handles this automatically. `polyoxide-py` is `publish = false` (not on crates.io); its Python wheels are built and published to PyPI via a separate step in the release workflow.
