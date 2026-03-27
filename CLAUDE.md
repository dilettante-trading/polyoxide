# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Polyoxide is a Rust SDK toolkit for Polymarket APIs. It provides library crates for CLOB trading, market data (Gamma), user data, gasless relay transactions, Python bindings, and a standalone CLI. Hard fork of [polyte](https://github.com/roushou/polyte).

## Build & Development Commands

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

CI runs two jobs: **format** (standalone), then **lint & test** (clippy, `cargo nextest run`, doctest — sequentially in one job). Clippy uses `-D warnings` (all warnings are errors).

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
│       └── polyoxide-cli (CLI tool using clap)
└── polyoxide-py        (Python bindings via PyO3 + maturin, publish = false)
```

**polyoxide** (the unified crate) uses feature flags: `clob`, `gamma`, `data`, `ws` (WebSocket), `full` (all). Default = clob + gamma + data.

## Key Patterns

**Builder pattern** — All clients use builders: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`, `Gamma::builder()`, `DataApi::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`.

**API namespaces** — Clients organize endpoints into namespaces:
- CLOB: `clob.markets()`, `clob.orders()`, `clob.account_api()`, `clob.health()`, `clob.auth()`, `clob.rewards()`, `clob.rfq()`, `clob.notifications()`
- Gamma: `gamma.markets()`, `gamma.events()`, `gamma.series()`, `gamma.tags()`, `gamma.comments()`, `gamma.sports()`, `gamma.search()`, `gamma.user()`, `gamma.health()`
- Data: `data.users()`, `data.trades()`, `data.holders()`, `data.leaderboard()`, `data.builders()`, `data.live_volume()`, `data.open_interest()`, `data.health()`

Example: `gamma.markets().list().open(true).send().await?`, `data.leaderboard().list().send().await?`.

**Request builder fluency** — Query parameters are chained with builder methods before `.send().await?`.

**Two auth layers** — L1 uses EIP-712 signing (via `alloy`) for on-chain orders; L2 uses HMAC-SHA256 for API credentials. Both are managed through the `Account` type in `polyoxide-clob/src/account/`.

**Error hierarchy** — `ApiError` in core, wrapped by crate-specific errors (`ClobError`, `GammaError`, `DataApiError`, `RelayError`). The `impl_api_error_conversions!` macro in core wires up `From` conversions.

**Decimal precision** — Price/size fields use `rust_decimal::Decimal` with `serde(with = "rust_decimal::serde::str")` for string serialization.

## Environment Variables

For authenticated operations (CLOB trading, user data):
```
POLYMARKET_PRIVATE_KEY        # Hex-encoded private key
POLYMARKET_API_KEY            # L2 API key
POLYMARKET_API_SECRET         # L2 API secret (base64)
POLYMARKET_API_PASSPHRASE     # L2 API passphrase
```

Relay operations additionally need `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE`.

## API Specs

Upstream Polymarket API documentation lives in `docs/specs/`. See `docs/specs/INDEX.md` for the full index. These are the source of truth for endpoint contracts, rate limits, and response schemas — sourced from https://docs.polymarket.com and the official OpenAPI specs.

## Testing Conventions

Each crate has live integration tests in `tests/live_api.rs` gated with `#[ignore]` so CI skips them. They hit the real Polymarket APIs. Run with `-- --ignored` flag.

Read-only crates (gamma, data) use `Gamma::builder().build()` / `DataApi::builder().build()` directly. CLOB tests use `Clob::public()` for unauthenticated endpoints.

Mock HTTP tests use `mockito` (workspace dev-dependency). Each crate with mock tests has a `tests/mock_api.rs` file with helper functions like `test_public_clob(server)` that point clients at the mock server URL.

## Module Organization

Each crate follows a consistent layout:
- `lib.rs` — public API re-exports
- `client.rs` — main client struct + builder
- `error.rs` — crate-specific error enum (uses `thiserror`)
- `types.rs` — domain types
- `api/` — namespace modules, one file per API group (markets, orders, etc.)

**WebSocket** support lives in `polyoxide-clob/src/ws/` (not core), feature-gated behind `ws` (not enabled by default in polyoxide-clob; default = `["gamma"]`). Two channels: `WebSocket::connect_market(asset_ids)` (public) and `WebSocket::connect_user(condition_ids, credentials)` (authenticated). Implements `futures_util::Stream`. `WebSocketBuilder` provides auto-ping keep-alive for long-running connections.

## Publishing Order

Crates must be published in dependency order: core → relay → gamma → data → clob → polyoxide. The release workflow in `.github/workflows/release.yml` handles this automatically. `polyoxide-py` is `publish = false` (not on crates.io); its Python wheels are built and published to PyPI via a separate step in the release workflow.
