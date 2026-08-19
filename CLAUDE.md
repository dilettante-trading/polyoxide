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

# Docs (must pass with zero warnings — see the note on intra-doc links below)
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace

# Format check
cargo fmt --all -- --check

# Format fix
cargo fmt --all
```

CI runs four jobs: **format** (standalone), **lint & test** (clippy, `cargo nextest run`, doctest, then `cargo doc` — sequentially in one job), **python bindings** (`uv run pytest tests/` in `polyoxide-py`, gated on **format** passing), and **CI scripts** (`uv run pytest tests/` in `.github/scripts`). Clippy uses `-D warnings` (all warnings are errors).

**Clippy and tests passing is not enough.** The lint & test job ends with `cargo doc` under `RUSTDOCFLAGS: -D warnings`, which makes `rustdoc::private_intra_doc_links` an error: a doc comment on a `pub` item may not use ``[`link`]`` syntax to reference a `pub(crate)` item. Doctests do not catch this — they run the code in doc comments and say nothing about whether the prose links resolve. Either make the referenced item `pub` or state the fact inline.

A red doc build costs more than it looks: `release.yml` triggers on `workflow_run: [CI], conclusion == 'success'`, so a failed doc build **silently withholds the release tag**. The version bump lands on `main` and nothing publishes, with no obvious connection between the two symptoms.

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
- Data: `data.user(addr)`, `data.trades()`, `data.holders()`, `data.leaderboard()`, `data.builders()`, `data.live_volume()`, `data.open_interest()`, `data.market_positions()`, `data.combos()`, `data.misc()`, `data.pnl()`, `data.rankings()`, `data.accounting()`, `data.approvals()`, `data.health()`

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

**Two rate limit layers, counting different things** — a request must satisfy both, and they are modelled in separate modules:

| Layer | Module | Keyed on | Counts | Applies to |
|-------|--------|----------|--------|------------|
| Cloudflare IP throttling | `polyoxide-core/src/rate_limit.rs` | client IP | **requests** | every host |
| Per-signer token buckets | `polyoxide-core/src/signer_limit.rs` | signer address | **orders** | CLOB order/cancel only |

The per-signer layer charges batch endpoints their full size (`POST /orders` costs N, `DELETE /orders` costs N, `cancel-all` costs 1+N), so a batch can cost more than the bucket's burst capacity can *ever* hold — permanently rejected, not throttled. `SignerLimiter::acquire` refuses those client-side as `ClobError::BurstCapacityExceeded` (non-retriable) rather than letting the retry loop burn attempts on a 429 it would misread as transient. Tier starts at `Standard` (tightest) and is adopted from the `Poly-RateLimit-Tier` response header, since it derives from 30-day volume the client cannot compute. `cancel-all`/`cancel-market-orders` costs are *not* knowable client-side — `TradingRequest::cost_is_exact` flags that.

Both tables are pinned by `documented_*_limits` agreement tests asserting the **effective quota a request resolves to**, not merely that an entry exists. Tests that only check presence and ordering are how `/balance-allowance` went missing and `/closed-positions` sat at 66x its cap, both undetected.

**Published rate limit tables name routes that 404** — upstream lists `Health check (/ok)` under every surface, but only `clob.polymarket.com` serves it. Data's health route is `/`, Gamma's is `/status`. Probe the path on the host before pinning a row.

**The buckets model the published quota; a 429 is what the server actually said.** They disagree — Cloudflare's `error code: 1015` is an IP-scoped block with its own window and arrives as a 429 no matter how many tokens the buckets still hold. So a 429 feeds back into the limiter as a *client-wide cooldown* (`HttpClient::note_rate_limited` → `RateLimiter::begin_cooldown`), which every subsequent `acquire` waits out regardless of path. Two rules make this work, both mutation-tested:

- **`Retry-After` may only extend the wait, never shorten it.** Cloudflare sends one that floors to zero; obeying it verbatim made `should_retry` return `Duration::from_millis(0)`, so three retries landed inside 65ms and *extended* the very ban they were waiting on. The floor is the client's own exponential backoff.
- **Cooldowns extend, never truncate.** Concurrent requests see the same 429 milliseconds apart; taking the newest value would let the smallest delay release everyone early. `await_cooldown` re-checks after waking so a cooldown extended mid-wait is honoured in full.

Every retry loop must call `note_rate_limited` **before** `should_retry` and unconditionally — a request that is out of attempts still has to publish what it learned.

**A bucket's depth and its refill rate are two spends of one budget.** `quota()` deliberately does not call `allow_burst`, leaving capacity at governor's default of one token. The obvious spelling — capacity `count`, refilling at `count/period` — reads like a faithful transcription of "150 per 10 seconds" and is wrong: a bucket starting full admits its depth *plus* everything the refill adds, so its first window lets through `count + count`. Every entry in every table over-permitted by exactly 2x until this was measured.

Depth is not spare capacity; it is borrowed against the rate, and `burst + rate × period ≤ count` means any burst of `B` costs `B` requests of sustained allowance permanently. Minimum depth is therefore also maximum throughput — and the safest shape, since the client never concentrates requests into an instant, including on release from a cooldown when every parked request resumes at once. This is inherent to token buckets against a sliding-window server, not an artifact of this implementation: satisfying the bound with `rate = count/period` forces `B ≤ 0`.

**Contrast `signer_limit.rs`, which must keep its `allow_burst`.** Polymarket publishes rate *and* burst for the per-signer layer and says burst is the bucket's capacity, so copying both is a faithful model of a bucket the server also implements as a bucket. Cloudflare publishes a *window quota* with no capacity term at all. Same two numbers, opposite meanings — making the two modules "consistent" would reintroduce the bug.

**The published count is reachable as a burst and not as a rate**, which is why `quota()` also reserves a tenth (`RESERVED_FRACTION`) rather than aiming at the published figure. Measured on `/closed-positions` (150/10s) against the live host:

| Sustained rate | Share of published | Result |
|---|---|---|
| 14.9/s | 100% | refused after 15.7s |
| 14.25/s | 95% | refused after 17.3s |
| 13.5/s | 90% | clean over 180s, 2,430 requests |

A one-shot 150 in 0.70s is accepted, so the table is not overstating the cap; Cloudflare's sliding-window estimator simply does not count the way a naive interval count does, and nothing outside the server can observe the difference. Aiming *at* a published quota is therefore a bug even when the arithmetic is right. Reproduce with `polyoxide-data/examples/closed_positions_soak.rs` (`--rate` drives a chosen rate; omit it to exercise the shipped limiter).

Note that a 429 the client retries away is invisible to the caller — the retry loops log a `WARN` and return `Ok` — so a harness that counts `Ok` against `Err` reports a clean run straight through sustained throttling. Detection goes through a `tracing` subscriber instead.

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
polyoxide crate covers: **Perps** (`perps/`, 49 endpoints on
`api.perpetuals.polymarket.com`, with its own `POLYMARKET-PROXY` /
`POLYMARKET-SECRET` header auth rather than the L1/L2 scheme), **Bridge**
(`bridge/`, 5 endpoints), and **Combos RFQ** (`combos-rfq/`, 4 endpoints). They
are mirrored so parity audits can see them; adding client support for any of
them is a separate piece of work.

For the upstream hosted docs, [`docs/specs/polymarket-llms.txt`](docs/specs/polymarket-llms.txt) is a snapshot of Polymarket's own documentation index (`https://docs.polymarket.com/llms.txt`) — a flat list of every doc page (with `.md` URLs) covering CLOB/auth/orders, builder attribution, and the CLOB V2 migration. Use it to locate the authoritative upstream page for a topic when the local `docs/specs/` copies are insufficient.

**A mirror can match upstream and still be wrong.** `docs/specs/gamma/OBSERVED.md`
records places where gamma's published spec disagrees with gamma's own server —
`parent_entity_type` accepts `PerpsAsset` and rejects the documented `market`,
`limit` counts top-level comments rather than rows, and `GET /comments/{id}`
returns a whole thread. The drift check cannot see any of this: it compares the
mirror to the published document, never to the live host. The mirror itself must
stay byte-faithful or `nightly-schema.yml` alarms forever, so the observations
live beside it rather than inside it.

## Testing Conventions

Each crate has live integration tests in `tests/live_api.rs` gated with `#[ignore]` so CI skips them. They hit the real Polymarket APIs. Run with `-- --ignored` flag.

Read-only crates (gamma, data) use `Gamma::builder().build()` / `DataApi::builder().build()` directly. CLOB tests use `Clob::public()` for unauthenticated endpoints.

Mock HTTP tests use `mockito` (workspace dev-dependency). Each crate with mock tests has a `tests/mock_api.rs` file with helper functions like `test_public_clob(server)` that point clients at the mock server URL.

## Nightly API Smoketest

Two GitHub Actions workflows run at `0 6 * * *` UTC and on `workflow_dispatch`:

- `.github/workflows/nightly-behavioral.yml` — runs `--ignored` live tests across the five crates with live suites (gamma, data, clob incl. `live_ws` under `--features ws`, relay, cli). Failures are classified by `.github/scripts/classify_failures.py` into:
  - **auth-gated** (matches the `POLYMARKET_* env vars required` / `POLYMARKET_PRIVATE_KEY required` panics) — silently skipped
  - **environmental** (test says the world can't provide signal right now, e.g. the sports channel with no live matches — matches `legitimately time out`) — logged and skipped
  - **transient** (HTTP 429/5xx, connection refused, timeouts, DNS) — retried up to 2× with `cargo nextest --retries 2`
  - **real** (everything else) — files or updates a tracking issue with the `nightly-behavioral` label
- `.github/workflows/nightly-schema.yml` — fetches each published upstream spec (seven OpenAPI: clob, gamma, data, relay, perps, bridge, combos-rfq; four AsyncAPI: clob market/user, perps WS, combos-rfq WS) and compares against the vendored mirror in `docs/specs/`. On drift, files a tracking issue labelled `schema-drift` **and** `spec:<id>`. It creates no branches and opens no PRs — Actions cannot open PRs here (org policy: 12 refusals, 0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out. The workflow holds `contents: read` and `issues: write` only. The issue is found by label intersection, never by title: `gh issue list --search "<title> in:title"` is a tokenized full-text search, so `perps` also matches `perps-ws` — that collision let one spec's job edit and close another's issue for eleven days, and made `combos-rfq-ws` look like it was flapping. A spec we deliberately will not sync is recorded in `docs/specs/.drift-acknowledged.json`, keyed by the SHA-256 of the canonical diff, which makes the check exit 3 and close the issue. Fingerprinting the *disagreement* rather than upstream means the acknowledgement expires the moment either side moves, so it is never permanent blindness. `clob` is acknowledged because upstream's own re-serialization made `example: 'Yes'` parse as boolean `true` on a `type: string` field. The issue body carries a key-path summary (changed JSON pointers with before → after, so drift inside `components.schemas` is named rather than merely counted) plus the canonicalized diff, composed in Python under GitHub's 65536-character cap. Deliberately excluded: the sports AsyncAPI mirror (modelled on captured wire frames, so it never matches upstream's published doc) and the undocumented `user-pnl-api`/`lb-api` hosts (nothing to diff).

To enable CLOB/relay's auth-gated tests (currently ~25 + 8 tests), set the `POLYMARKET_*` and `BUILDER_*` repo secrets and remove the auth patterns from `AUTH_GATED_RE` in `.github/scripts/classify_failures.py`. Auth tests will then start contributing real signal.

## Module Organization

Most crates follow a consistent layout:
- `lib.rs` — public API re-exports
- `client.rs` — main client struct + builder
- `error.rs` — crate-specific error enum (uses `thiserror`)
- `types.rs` — domain types
- `api/` — namespace modules, one file per API group (markets, orders, etc.)

**WebSocket** support lives in `polyoxide-clob/src/ws/` (not core), feature-gated behind `ws` (not enabled by default in polyoxide-clob; default = `["gamma"]`). Three channels: `WebSocket::connect_market(asset_ids)` (public), `WebSocket::connect_user(condition_ids, credentials)` (authenticated), and `WebSocket::connect_sports()` (public, served by `sports-api.polymarket.com` and taking no subscription payload). Implements `futures_util::Stream`. `WebSocketBuilder` provides auto-ping keep-alive for long-running connections.

Three market events — `best_bid_ask`, `new_market`, `market_resolved` — are withheld by the server unless the subscription sets `custom_feature_enabled`. Use `WebSocket::connect_market_with(ids, MarketSubscriptionOptions::default().with_custom_features())` to receive them. `MarketMessage` and `Channel` are `#[non_exhaustive]`, since upstream adds event types over time.

The user channel's market filter is optional: `WebSocket::connect_user_all_markets(creds)` omits it and receives events for every market, and `subscribe_markets` / `unsubscribe_markets` adjust it on a live connection without reconnecting.

The WebSocket contracts are published as AsyncAPI, not OpenAPI — mirrored in `docs/specs/clob/asyncapi-{market,user,sports}.json`. A parity audit that only diffs the OpenAPI files will miss this whole surface.

**The sports mirror does not match the wire.** Upstream's own page documents a `slug`-keyed payload and a text `"ping"`/`"pong"` keep-alive; the server sends neither. `SportsUpdateMessage` is modelled on 229 captured frames instead — see `x-observed-payload` in `asyncapi-sports.json`. Diffing polyoxide against that mirror will report a false positive.

**WebSocket TLS needs a nudge.** `reqwest 0.12` (via core) and `alloy`'s `reqwest 0.13` enable `ring` and `aws-lc-rs` on one shared `rustls`, which then installs no default `CryptoProvider`. `ws/client.rs` installs one before connecting; any code that calls `tokio_tungstenite::connect_async` directly must do the same or it will panic.

## Publishing Order

Crates must be published in dependency order: core → relay → gamma → data → clob → polyoxide. The release workflow in `.github/workflows/release.yml` handles this automatically. `polyoxide-py` is `publish = false` (not on crates.io); its Python wheels are built and published to PyPI via a separate step in the release workflow.
