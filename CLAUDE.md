# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Polyoxide is a Rust SDK toolkit for Polymarket APIs. It provides library crates for CLOB trading, market data (Gamma), user data, gasless relay transactions, Python bindings, and a standalone CLI. Hard fork of [polyte](https://github.com/roushou/polyte).

## Architecture guide

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) is the agent's guide to the multi-venue restructure: the shape it builds towards, where each kind of change goes, the rules that bite, and the release stage the workspace is in. Read it before moving code between crates or adding one. It is copied from the architecture spine's `ARCHITECTURE-GUIDE.md` by `scripts/gen_registry.py --write` and is only ever regenerated from the spine, never edited by hand; CI fails when the two differ. Its stage line is a generated region fed by `[workspace.metadata.polyoxide] stage` in the root `Cargo.toml`.

The two documents describe different things. This file describes the code as it is now; the guide describes the target, mostly in the present tense, and says where each kind of change goes. A rule here stands until the commit that supersedes it lands, and that commit edits the rule here (AD-21). The guide's "Standing rules this replaces" table lists the rules due to go, not ones already gone: "call `note_rate_limited` before `should_retry`" still holds at six call sites today, which `docs/MUTANTS.md` lists.

[`docs/MUTANTS.md`](docs/MUTANTS.md) lists the mutation-tested rules: for each, the line it holds on, the mutation, and the tests that fail under it. Move one of those lines or tests and the ledger moves with it, re-proved.

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

CI runs eight jobs: **format** (standalone), **lint & test** (clippy, `cargo nextest run`, doctest, then `cargo doc` — sequentially in one job), **package** (`cargo publish --workspace --dry-run --no-verify`, plus `scripts/publish_order.py check-manifests`, which fails when a publishable crate lacks a description or a licence, since cargo only warns about those), **msrv** (`cargo check` and `cargo doc` on Rust 1.91, warnings allowed), **features** (`cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`, so a feature that compiles only alongside another fails), **removals** (`scripts/api_removals.py check` against the S1 start tag `v0.38.1`; see below), **python bindings** (`uv run pytest tests/` in `polyoxide-py`, gated on **format** passing), and **CI scripts** (`uv run pytest tests/` in `.github/scripts`). Clippy uses `-D warnings` (all warnings are errors). The package job catches a manifest fault on the PR that introduces it rather than halfway through a release, and skips `publish = false` members on its own.

**Removing a public item needs a line in [`docs/s1-removals.md`](docs/s1-removals.md).** The removals job runs cargo-semver-checks against `v0.38.1` with `--release-type patch` and fails on any reported removal that file does not list, printing each one's key to copy there. cargo-semver-checks cannot see `#[doc(hidden)]` items or type aliases, so the job also imports each such path the file lists, in one scratch crate per crate and feature set, so no entry compiles through another's features. A key is `<crate> <lint id>: <item> (<file>)`, and a deleted crate is `<crate> crate_missing`. cargo-semver-checks 0.51.0 and Rust 1.99.0 are pinned together in `ci.yml` and `release.yml`, because the tool reads only the rustdoc JSON some toolchains emit; upgrade both at once. `release.yml`'s `semver` job runs the same tool against the newest release tag behind the commit, before anything publishes, and lets it infer the release type from the two versions. On a 0.x patch bump it fails on any major-level change, not only a removal: a new required field, a changed signature, an enum made non-exhaustive. It also runs the compile test. When it fails, raise the 0.x minor in a new version-bump commit on `main` (see Publishing Order); the release that failed was never tagged, so the next green push releases the new version.

**Clippy and tests passing is not enough.** The lint & test job ends with `cargo doc` under `RUSTDOCFLAGS: -D warnings`, which makes `rustdoc::private_intra_doc_links` an error: a doc comment on a `pub` item may not use ``[`link`]`` syntax to reference a `pub(crate)` item. Doctests do not catch this — they run the code in doc comments and say nothing about whether the prose links resolve. Either make the referenced item `pub` or state the fact inline.

A red doc build costs more than it looks: `release.yml` triggers on `workflow_run: [CI], conclusion == 'success'`, so a failed doc build, or any other red CI job (msrv, features and removals included), **silently withholds the release tag**. The version bump lands on `main` and nothing publishes, with no obvious connection between the two symptoms. Fixing the build forward is enough: the next green push to `main` releases any version that is still untagged (see Publishing Order).

```bash
# Run live integration tests (hit real APIs, skipped in CI)
cargo test -p polyoxide-clob --test live_api -- --ignored
```

## Workspace Architecture

Text between `<!-- generated:begin <id> -->` and `<!-- generated:end <id> -->` markers, here and in README.md, docs/specs/INDEX.md, docs/ARCHITECTURE.md, SELF-HEALING.md and the two nightly workflows, is written by `scripts/gen_registry.py` from each crate's `[package.metadata.polyoxide]`, the root manifest's `[workspace.metadata.polyoxide]` (`stage` and `mirrors`) and `cargo metadata`, so never edit it by hand: change the metadata, run `python3 scripts/gen_registry.py --write`, and expect CI's scripts job to fail on any region that differs from what the generator produces.

<!-- generated:begin claude-crate-count -->
Fourteen crates, in publish order, each with the workspace crates its build needs. Crates that are not published come last:
<!-- generated:end claude-crate-count -->

<!-- generated:begin claude-graph -->
- `polyoxide-venue` — Shared vocabulary: error classes, the status and close-code maps, the `Retry-After` parser; needs: nothing in the workspace; every public error type in the workspace implements its `Classify` trait; it has no HTTP, socket or signing dependency
- `polyoxide-core` — Core utilities and shared types; needs: `polyoxide-venue`; shared auth, HTTP client, errors and macros
- `polyoxide-binance` — Client library for Binance USDⓈ-M futures market data and streams (not part of the unified crate); needs: `polyoxide-core`, `polyoxide-venue`
- `polyoxide-data` — Client library for Polymarket Data API; needs: `polyoxide-core`, `polyoxide-venue`
- `polyoxide-gamma` — Client library for Polymarket Gamma (market data) API; needs: `polyoxide-core`, `polyoxide-venue`
- `polyoxide-perps` — Client library for Polymarket Perps (perpetual futures) public market data; needs: `polyoxide-core`, `polyoxide-venue`; auth and trading pending
- `polyoxide-relay` — Client library for Polymarket Relayer API (gasless transactions); needs: `polyoxide-core`, `polyoxide-venue`
- `polyoxide-clob` — Client library for Polymarket CLOB (order book) API; needs: `polyoxide-core`, `polyoxide-gamma` (under `gamma`, on by default), `polyoxide-venue`; published after `polyoxide-relay`, a versioned dev-dependency
- `polyoxide-rtds` — Client for Polymarket's RTDS crypto price streams; needs: `polyoxide-venue`; no other workspace crate, `reqwest` or `alloy`, so a credential-free price feed builds no HTTP or signing stack
- `polyoxide-sports` — Client for Polymarket's live sports score feed; needs: `polyoxide-venue`; no other workspace crate, like `polyoxide-rtds`
- `polyoxide` — Unified client for Polymarket APIs (CLOB, Gamma, Data, WebSocket, RTDS, Perps, Sports); needs: `polyoxide-clob` (under `clob`, on by default), `polyoxide-data` (under `data`, on by default), `polyoxide-gamma` (under `gamma`, on by default), `polyoxide-perps` (under `perps`), `polyoxide-rtds` (under `rtds`), `polyoxide-sports` (under `sports`), `polyoxide-venue`
- `polyoxide-cli` — CLI tool for querying Polymarket APIs and Binance USDⓈ-M market data; needs: `polyoxide-binance` (with `ws`), `polyoxide-clob` (with `ws`), `polyoxide-core` (with `keychain`, under `keychain`), `polyoxide-data`, `polyoxide-gamma`, `polyoxide-relay` (with `keychain`, under `keychain`), `polyoxide-rtds`, `polyoxide-sports`
- `polyoxide-py` — Python bindings via PyO3 (`publish = false`, wheels on PyPI); needs: `polyoxide-clob` (without default features), `polyoxide-data`, `polyoxide-gamma`
- `polyoxide-test-support` — Live-test toolkit: the failure tags the nightly classifier reads, and the credential loaders (`publish = false`); needs: `polyoxide-core` (with `keychain`), `polyoxide-venue`; a path-only dev-dependency of the crates whose live tests use it; it names no venue
<!-- generated:end claude-graph -->

<!-- generated:begin claude-cli-deps -->
Note: `polyoxide-cli` does **not** depend on the unified `polyoxide` crate. It depends directly on the component crates — `polyoxide-binance` (with `ws`), `polyoxide-clob` (with `ws`), `polyoxide-data`, `polyoxide-gamma`, `polyoxide-rtds` and `polyoxide-sports` — plus `polyoxide-core` (with `keychain`) and `polyoxide-relay` (with `keychain`) only under the optional `keychain` feature.
<!-- generated:end claude-cli-deps -->

The CLI's `ws` group streams `market` and `user` (clob), `prices` (rtds), `sports`
(`polyoxide-sports`) and `binance` (`polyoxide-binance`). `ws sports` takes comma-separated `--league` and `--game` filters and
`--changes-only`. Its `run_with` takes any event stream, so `polyoxide-cli/tests/ws_sports.rs`
drives every flag with captured frames.

`ws binance` streams Binance USDⓈ-M market data through `polyoxide-binance`'s supervised
tier. `--symbol` and `--kind` are comma-separated, and each kind is streamed for each
symbol; `--all-tickers` and `--all-mark-prices` add the two array streams. Outage markers
go to stderr, and `--format json` prints each frame's envelope. Its `run_with` takes any
event stream, and `polyoxide-cli/tests/ws_binance.rs` drives it with the crate's captured
frames (`polyoxide_binance::usdm::ws::fixtures`, feature `test-server`).

The CLI installs a `tracing` subscriber that writes to stderr at `warn`, and `RUST_LOG`
overrides the level. That is how the libraries' recoveries become visible: a reconnect being
retried, or a 429 being waited out. Before it, they were discarded, and a feed retrying a dead
host looked like a quiet one.

The CLI's `clob` command group currently exposes `clob prices download` — a bulk,
resumable, rate-limited downloader for CLOB historical price data
(`GET /prices-history`) that writes per-market CSV/JSONL/Parquet dataset files
plus a `manifest.jsonl`. Parquet output requires building the CLI with the
`parquet` feature.

The CLI's `data` command group reads Data API v2, except `data health`, which stays on
v1's `/` because `/v2/status` reports data freshness, not liveness. Listing commands print
the v2 `{data, pagination}` envelope and page with `--cursor`, `--all` (JSONL, flushed per
page) and `--max-pages`; the cursor to resume from goes to stderr when a walk stops early.
`--offset` is refused with a pointer to `--cursor`. `data traded` prints the
`/v2/user-stats` object, or `null` for an unknown wallet. `DataCommand::run_with` takes the
client and the output writers, and `polyoxide-cli/tests/data_v2.rs` uses it to run real
arguments against mock servers serving `polyoxide-data`'s v2 fixtures.

**A clap `Vec<String>` field needs `value_delimiter`, not a value parser that returns a
`Vec`.** The latter compiles and parses, then panics when the field is read. Every v1
`data` list flag shipped that way, and no test caught it because the parse tests never
passed those flags.

<!-- generated:begin claude-umbrella-features -->
**polyoxide** (the unified crate) uses feature flags: `clob`, `gamma`, `data`, `ws` (`polyoxide-clob/ws`), `rtds`, `perps`, `perps-ws` (`polyoxide-perps/ws`), `sports`, `full` (all but `keychain`), `keychain` (`polyoxide-clob?/keychain`). Default = clob + gamma + data.
<!-- generated:end claude-umbrella-features -->

## Key Patterns

**Builder pattern** — All clients use builders: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`, `Gamma::builder()`, `DataApi::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`.

**API namespaces** — Clients organize endpoints into namespaces:
- CLOB: `clob.markets()`, `clob.orders()`, `clob.account_api()`, `clob.health()`, `clob.auth()`, `clob.rewards()`, `clob.public_rewards()`, `clob.notifications()`
- Gamma: `gamma.markets()`, `gamma.events()`, `gamma.series()`, `gamma.tags()`, `gamma.comments()`, `gamma.sports()`, `gamma.search()`, `gamma.user()`, `gamma.health()`
- Data: `data.user(addr)`, `data.trades()`, `data.holders()`, `data.leaderboard()`, `data.builders()`, `data.live_volume()`, `data.open_interest()`, `data.market_positions()`, `data.combos()`, `data.misc()`, `data.pnl()`, `data.rankings()`, `data.accounting()`, `data.health()`, `data.v2()`. `data.approvals()` is deprecated: upstream removed `/v1/approvals` and the host now returns `404`; `data.v2().approvals()` serves the same data.

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

A fourth scheme, **order signature type 3**, is a Deposit Wallet signing an ERC-7739
`TypedDataSign` envelope: the exchange domain is the signing domain and the wallet's
`DepositWallet`/`1` domain rides inside the message. Getting that orientation backwards
produces a different digest, so the signature cannot verify for the wallet.
`v1_exchange.envelope_digest` in `polyoxide-clob/tests/fixtures/session_keys/order_vectors.json`
pins py-sdk's golden digest, and `polyoxide-clob/src/core/eip712.rs` checks it. A session key adds a
6492 envelope on top. The role (owner or session key) is never inferred from
addresses; `SigningTarget::DepositWallet { wallet, role }` carries it. The published
OpenAPI omits almost all of it (the relay mirror documents only `GET /deployed?type=WALLET`):
see `docs/specs/session-keys/`.

**Error hierarchy** — `ApiError` in core, wrapped by crate-specific errors (`ClobError`, `GammaError`, `DataApiError`, `RelayError`). The `impl_api_error_conversions!` macro in core wires up `From` conversions.

**Every error implements `polyoxide_venue::Classify`** — inner and auxiliary types included (`V2Error`, `VenueError`, `BurstCapacityExceeded`, `ParseTickSizeError`, both `UnknownVariant`s, `InvalidSymbol`, `InvalidStreamName`). `class()` is one of the eight `Class` variants of AD-15, decided by the status before the body; `is_fault()` is false only where the venue answered as designed (the FAK and FOK kills, and a 451 region block wherever a status decides the class); `retry_after()` is the wait the server asked for. The class decides retriability for callers: `Network`, `Unavailable` and `RateLimited` are retriable. Where an inherent `is_retriable` still exists (`ApiError`, `ClobError`, `DataApiError`, `PerpsError`, `VenueError`, `BinanceError`) it may disagree until Epic 3 removes it, and each crate's table test pins both answers: `DataApiError`'s follows the server's `retryable` flag, and `ApiError`'s calls a 408 in `Api` final. Clob's status `0`, which wraps every failure of its Gamma dependency, is a `VenueRefusal`, so neither answer retries it, and a reqwest error that broke off mid-body is `Network` though reqwest labels it a decode error. Each crate's `lib.rs` lists its error types in a `const _` assertion, gated like the types, so a missing impl fails the build, and `.github/scripts/tests/test_classify_coverage.py` fails when a public error type is missing from that list. `polyoxide-venue` also holds the status and close-code maps and the one `Retry-After` parser; the existing call sites keep their own parsers until Story 3.10, and the per-crate match over `tungstenite::Error` stays in each socket crate until Story 4.3.

**Retriability** — `ApiError::is_retriable()` (and `ClobError::is_retriable()`) is true for rate limits, timeouts, connection failures, `425 Too Early`, and 5xx. The crates' *own* retry loop is narrower — `HttpClient::should_retry` only ever retries `429` and `425`.

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

**Not fully implemented.** `docs/specs/` also mirrors three upstream APIs that no
polyoxide crate fully covers: **Perps** (`perps/`, 61 endpoints on
`api.perpetuals.polymarket.com`, with its own `POLYMARKET-PROXY` /
`POLYMARKET-SECRET` header auth rather than the L1/L2 scheme — the 21 public
`/v1/info/*` routes are implemented by `polyoxide-perps`; credentials, the
`/v1/account/*` reads, the signed `/v1/trade/*` routes, funds and BLP are
pending, and `docs/specs/perps/OBSERVED.md` records where the host and the
schema part ways), **Bridge** (`bridge/`, 5 endpoints), and **Combos RFQ**
(`combos-rfq/`, 4 endpoints). They are mirrored so parity audits can see them;
adding client support for the rest is a separate piece of work.

**Perps public routes** are implemented by `polyoxide-perps` (`Perps::new()`,
namespaces `health()`, `exchange()`, `market()`, `public()`). Three test files
hold it in place on the pattern of Data v2: `tests/spec_agreement.rs` (types,
enums and query keys against `docs/specs/perps/openapi.json`, restricted to
schemas reachable from `/v1/info/*`), `tests/wire_agreement.rs` (against
`tests/fixtures/`, refreshed by `scripts/capture_perps_fixtures.py`) and
`tests/live_api.rs`. Wire-only fields are allowed through `OBSERVED_EXTRA` and
recorded in `docs/specs/perps/OBSERVED.md`. Klines and mark points are
positional arrays on the wire and have hand-written serde. The host ignores
`instrument_id` on `/v1/info/tickers` and `/v1/info/statistics` and returns
every instrument, so a caller filters client-side. The four WebSocket fields on
`LimitTier` are a `u32::MAX` sentinel, not a budget, and must not size a
bucket. Every index has empty constituents today. The host is fronted by
CloudFront, so the rate rows in `RateLimiter::perps_default` were measured
with `polyoxide-perps/examples/info_soak.rs` over distinct URLs: klines 30,
trades 10, portfolio 30 and bbo 50 per 10 s, a `/v1/info` catch-all at 10 for
the routes not soaked, and a client-wide general bucket of 30 set by two mixed
validation runs, all recorded in OBSERVED.md's `## Rate limits`.
The `ws` feature adds the six public WebSocket channels: `PerpsWs` (bare) and
`PerpsWsBuilder`/`SupervisedPerpsWs` (keep-alive on the wall clock regardless of traffic,
since the host idle-closes after 60 s without an inbound message; a pong counts as
liveness; staleness; reconnect with resubscribe; `MembershipHandle`). Do not copy the
rtds pump shape for this host: it pings only on a quiet tick. `Channel` is the only way to name a
subscription; payload structs carry the REST field names over the socket's terse keys. Four wire facts the AsyncAPI mirror gets wrong are in
`docs/specs/perps/OBSERVED.md`: `tickers`/`statistics` data are objects and
`::all` fans out per instrument, `ets` is on every frame, `sq` is a
server-wide stamp so only regressions are detectable, and an unknown
instrument subscribes without error. Offline tests drive a scripted server
in `src/ws/test_server.rs`, compiled under `cfg(test)` and additionally exposed
by the `test-server` feature for downstream use; `tests/live_ws.rs` is
the live suite, and `tests/ws_wire_agreement.rs` compares the payload types
value-for-value against `tests/fixtures/ws/` (captured by
`scripts/capture_perps_ws_fixtures.py`, instrument 6 because `trades::1` was
quiet).

**Binance USDⓈ-M futures is not a Polymarket host.** `polyoxide-binance` reads its public
market data on `fapi.binance.com` (`Usdm::new()`, namespaces `health()`, `exchange()`,
`market()`) for consumers that trade both venues, and is deliberately not in the
`polyoxide` umbrella crate or `full`. Binance publishes no OpenAPI or AsyncAPI for
USDⓈ-M, so `docs/specs/binance/` is not a mirror: `OBSERVED.md` records what the host
does, `nightly-schema.yml` has nothing to diff, and
`tests/live_api.rs::live_responses_carry_no_unmodelled_keys` is the REST drift detector and
`tests/live_ws.rs::live_frames_carry_no_unmodelled_keys` the streams'.
Binance limits each IP by request *weight*, which depends on the route and its
parameters, so the crate has its own `WeightBudget` (`src/weight.rs`) instead of core's
`RateLimiter`: the UTC clock minute at 2160 of the published 2400 (the tenth core's
`RESERVED_FRACTION` also reserves), raised by every response's `X-MBX-USED-WEIGHT-1M` but only for the
minute its request was charged in, a separate bucket for the weightless funding routes
(450 per 5 minutes, depth one), and a `429` or `418` held as a client-wide cooldown that
is only ever extended. A `429` with no retry left and no `Retry-After` holds every
request to the next minute: sending into a spent minute is how a `429` becomes a `418`
ban. The weight table, `Route::cost`, is measured, not copied: Binance's page is one off
at the 100 and 500 `klines` edges and does not say that omitting `limit` costs 5 on
`klines` and 1 on `depth`. `cargo run -p polyoxide-binance --example weight_probe`
re-measures it. Response rows carry symbols as `String`; `Symbol` is for what a caller
sends, uppercases ASCII, and accepts `_` for quarterlies (`BTCUSDT_261225`).
`FundingRate::mark_price` is an `Option` because funding events through at least
2022-01-01 send `""`. Core enables reqwest's `gzip` feature
(`exchangeInfo` is 1.15 MB raw, 51 KB gzipped), and `HttpClientBuilder::gzip` is unset by
default, which leaves reqwest's default: every polyoxide client asks for gzip, as each did
before 0.37 for a consumer whose workspace enabled the feature. 0.37.0 forced it off unless
set, which took compression away from those consumers (prader-rs among them); 0.37.1
restored it. Four rate-limit examples pin `.gzip(false)` on their bare clients, so the soak
measurements keep the wire they were made on.

With the `ws` feature `polyoxide-binance` also streams eight USDⓈ-M market streams on
`fstream.binance.com`: `UsdmWs` (one connection on one path) and
`UsdmWsBuilder`/`SupervisedUsdmWs` (one connection per routed path, `/market` or
`/public`, since a stream on the wrong path may deliver nothing; pings on the wall
clock; staleness counting pongs and the server's pings; reconnect with a paced replay;
rotation at 23 h 50 min, under Binance's 24-hour cutoff). Binance acknowledges every
`SUBSCRIBE`, unknown and uppercase names and an explicit `@250ms` depth included (a
250 ms partial depth is the bare `<s>@depth<N>`), and enforces its rules by closing the
connection (on the 1025th stream, or after about 15 requests in a burst), so the client
enforces them first: `StreamName` is the only way to name a stream, and a connection
carries at most 1024 streams, 200 names per request, one request per 200 ms. Every
`Event::Disconnected { path }` is followed by `Event::Reconnected { path }` while the
client runs, even when the path's last stream leaves mid-outage; prader-rs folds outages
on that invariant, and `tests/supervision.rs` and `tests/supervision_edges.rs` pin it.
COIN-M rows (`st: 2`) arrive on this host and count contracts, not USDT, so they do not sum
with USDⓈ-M rows. The offline tests drive the scripted server in `src/usdm/ws/test_server.rs` (feature
`test-server`), which also exposes the captured frames as `usdm::ws::fixtures`.

**Data API v2** (`data-v2/`, 20 endpoints under `/v2` on `data-api.polymarket.com`)
is implemented by `polyoxide-data` as `data.v2()`, alongside the v1 routes, which
upstream says keep working. v2 uses a different contract: a `data` envelope,
cursor-only pagination and snake_case fields. Paged builders return `Page<T>` from
`send()` and a `Stream` from `.pages()`, which clones one request per page so a walk
cannot change its filters: on `trades`/`activity` a changed filter re-anchors the walk
silently, and on `positions` the `title`, `condition` and window filters are not in the
cursor at all. An error with the v2 body becomes `DataApiError::V2` (stable `code`, the
server's `retryable` flag, `trace_id`), recognised by body shape rather than path. Its
spec is the one mirror served by the API host (`/v2/openapi.json`) rather than
`docs.polymarket.com`, so a parity audit that only walks `/api-spec/` misses it.

Three test files hold v2 in place. `tests/v2_spec_agreement.rs` checks every type's
optionality and field names and every builder's query keys against that schema.
`tests/v2_wire_agreement.rs` checks the types against live captures in both directions
(`scripts/capture_v2_fixtures.py` refreshes them). `docs/specs/data-v2/OBSERVED.md`
records where the server and the schema part ways. Notably `/v2/user-pnl` is **not** the
`data.pnl()` series and `/v2/leaderboard` ranks volume in shares, not USDC, so neither
replaces the undocumented host. The v2 rows in `RateLimiter::data_default` were measured with
`polyoxide-data/examples/v2_soak`, which sends raw requests over distinct URLs: several v2
routes are CDN-cached, and a repeated URL is answered by CloudFront without reaching the
origin, so a soak that repeats URLs reports a clean run at any rate. The runs are in
`docs/specs/data-v2/OBSERVED.md`.

**Python bindings** expose v2 as `DataApi().v2()` / `DataApiSync().v2()`. The row classes,
`Page` and the page iterators live on `polyoxide.v2`, because v2 reuses v1 class names.
Three tests hold the bindings up. `every_v2_getter_reads_its_own_key` in
`polyoxide-py/src/types/data_v2.rs` exists because `get_field` returns `None` for a missing
key, which stub consistency cannot see. `test_stub_consistency.py` checks `v2.pyi` members
and signatures against the compiled module. `test_data_v2_offline.py` calls every route
with every argument against a local server. Enum arguments take the exact wire spelling. A
v2 error maps by `code`, and every SDK exception carries `status`, `code`, `retryable`,
`trace_id`, `parameter` and `retry_after`, which are `None` unless the error came from a v2
route.

For the upstream hosted docs, [`docs/specs/polymarket-llms.txt`](docs/specs/polymarket-llms.txt) is a snapshot of Polymarket's own documentation index (`https://docs.polymarket.com/llms.txt`) — a flat list of every doc page (with `.md` URLs) covering CLOB/auth/orders, builder attribution, and the CLOB V2 migration. Use it to locate the authoritative upstream page for a topic when the local `docs/specs/` copies are insufficient.

**A mirror can match upstream and still be wrong.** `docs/specs/gamma/OBSERVED.md`
records places where gamma's published spec disagrees with gamma's own server —
`parent_entity_type` accepts `PerpsAsset` and rejects the documented `market`,
`limit` counts top-level comments rather than rows, and `GET /comments/{id}`
returns a whole thread. The drift check cannot see any of this: it compares the
mirror to the published document, never to the live host. The mirror itself must
stay byte-faithful or `nightly-schema.yml` alarms forever, so the observations
live beside it rather than inside it.

**Polymarket Protocol V2 is not implemented, and is not the CLOB V2 migration.**
Gamma's `Market::version` says which protocol a market trades on. A `v2` market
takes its outcome ids from `position_ids`, signs orders for ExchangeV3 (domain
version `"3"`) and reads balances as `CONDITIONAL-V2`. `polyoxide-clob` signs
domain `"2"` only, which is correct for `v1` markets. Choose ids by `version`,
never by which field is present: 35% of open `v1` markets also carry
`position_ids`, and the CLOB has no book for them. No `v2` market existed on
2026-10-07. Trading support is tracked in #51. Upstream's guide is
`docs.polymarket.com/migrate/polymarket-v2/`; the wire facts are in
`docs/specs/gamma/OBSERVED.md`.

**Deposit Wallets and session keys have no mirror at all.** `docs/specs/session-keys/`
holds the contract (`README.md`) and the SDK behaviours the pages omit (`OBSERVED.md`):
signature type 3, `GET /v1/user/session-signers`, and the relayer's `type: WALLET`
dialect with `/v1/session-signers/*` and `/v1/account/transactions/*`. The sources are
the prose pages plus `github.com/Polymarket/py-sdk` and `github.com/Polymarket/ts-sdk`
(0.11.0), and where they disagree the SDKs win. Per-route auth and timeouts live in
py-sdk's `_require_*` guards and `httpx.Timeout` values, not in its payload builders;
plan 2 got both wrong until a reviewer read `_internal/actions/session_keys.py`.
`scripts/capture_session_key_vectors.py` regenerates the fixtures from py-sdk; every
signing, encoding and wire-body test is pinned to them, never to a self-computed value.
Implemented by `polyoxide-clob` (`SigningTarget`, `Account::{with_signer,l2_only}`,
`*_with_signature`, `list_session_signers`) and `polyoxide-relay` (`WalletType::DepositWallet`,
`deposit_wallet`, `session_signers`, `resolve_wallet`, `with_auth`). The live round trip
is `polyoxide-clob/tests/live_session_keys.rs`, `#[ignore]`d and gated on a Deposit
Wallet fixture account that does not exist until prader-rs #125 is resolved.

## Testing Conventions

Each crate has live integration tests in `tests/live_api.rs` gated with `#[ignore]` so CI skips them. They hit the real upstream APIs. Run with `-- --ignored` flag.

Read-only crates (gamma, data) use `Gamma::builder().build()` / `DataApi::builder().build()` directly. CLOB tests use `Clob::public()` for unauthenticated endpoints.

Mock HTTP tests use `mockito` (workspace dev-dependency). Each crate with mock tests has a `tests/mock_api.rs` file with helper functions like `test_public_clob(server)` that point clients at the mock server URL.

## Nightly API Smoketest

Two GitHub Actions workflows run at `0 6 * * *` UTC and on `workflow_dispatch`. The behavioral one also runs on Saturday and Sunday at 18:30 UTC (`30 18 * * 6,0`). The sports feed carries only what is live, and 06:00 UTC misses North American leagues and weekend soccer, which the drift detector would otherwise never see. That run covers every live job, so its clean result may close the issue.

- `.github/workflows/nightly-behavioral.yml` — runs `--ignored` live tests, one job per crate and suite, each job's `env:` holding only the secrets its tests read. An unset repository secret arrives as `""`, which the live loaders treat as unset:
  <!-- generated:begin claude-nightly -->
  - `live-polyoxide-binance-live` (15 min): `live_api`, `live_ws` with `--features ws`
  - `live-polyoxide-data-live` (15 min): `live_api`
  - `live-polyoxide-gamma-live` (15 min): `live_api`
  - `live-polyoxide-perps-live` (15 min): `live_api`, `live_ws` with `--features ws`
  - `live-polyoxide-relay-live` (15 min): `live_api`; secrets `BUILDER_API_KEY`, `BUILDER_PASS_PHRASE`, `BUILDER_SECRET`, `POLYMARKET_PRIVATE_KEY`, `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS`
  - `live-polyoxide-clob-live` (15 min): `live_api`, `live_ws` with `--features ws`; secrets `POLYMARKET_API_KEY`, `POLYMARKET_API_PASSPHRASE`, `POLYMARKET_API_SECRET`, `POLYMARKET_BUILDER_CODE`, `POLYMARKET_PRIVATE_KEY`
  - `live-polyoxide-clob-session-keys` (40 min): `live_session_keys`; secrets `BUILDER_API_KEY`, `BUILDER_PASS_PHRASE`, `BUILDER_SECRET`, `POLYMARKET_DW_OWNER_PRIVATE_KEY`, `POLYMARKET_DW_SESSION_PRIVATE_KEY`, `POLYMARKET_DW_WALLET`
  - `live-polyoxide-rtds-live` (15 min): `live_api`
  - `live-polyoxide-sports-live` (20 min): `live_api`
  - `live-polyoxide-cli-live` (15 min): `live_api`
  <!-- generated:end claude-nightly -->
  Failures are classified by `.github/scripts/classify_failures.py` from the tag line each failing test prints (below). A failure with no tag is `real`; nothing reads the panic text.
  - **auth-gated** (a credential loader found a secret absent or empty) — silently skipped
  - **environmental** (`environmental(reason)`, where the world can't provide signal right now — the sports feed with no live matches, a quiet RTDS or perps feed, no market satisfying an order test's precondition — or an error whose class is `Restricted` and not a fault, such as Binance refusing a US runner's location with a 451) — logged and skipped
  - **transient** (an error whose class is `Network`, `Unavailable` or `RateLimited`: HTTP 408/425/429/5xx, connection refused, timeouts, DNS, a dropped WebSocket, close codes 1000/1001/1006/1011–1013; or `transient(reason)`, which the socket tests call where a bare stream ends without a close code) — retried up to 2× with `cargo nextest --retries 2`, and filed as real if it still fails, unless the retry is environmental or auth-gated, whose verdict it then takes (a dropped connection, then a quiet feed)
  - **real** (everything else, every untagged failure included) — files or updates a tracking issue with the `nightly-behavioral` label
- `.github/workflows/nightly-schema.yml` — fetches each published upstream spec and compares it against the vendored mirror in `docs/specs/`.
  <!-- generated:begin claude-schema-watch -->
  It watches eight OpenAPI (`clob`, `gamma`, `data`, `data-v2`, `relay`, `perps`, `bridge`, `combos-rfq`) and four AsyncAPI (`clob-ws-market`, `clob-ws-user`, `perps-ws`, `combos-rfq-ws`), each filed under its own `spec:<id>` label.
  <!-- generated:end claude-schema-watch -->
  On drift, files a tracking issue labelled `schema-drift` **and** `spec:<id>`. It creates no branches and opens no PRs — Actions cannot open PRs here (org policy: 12 refusals, 0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out. The workflow holds `contents: read` and `issues: write` only. The issue is found by label intersection, never by title: `gh issue list --search "<title> in:title"` is a tokenized full-text search, so `perps` also matches `perps-ws` — that collision let one spec's job edit and close another's issue for eleven days, and made `combos-rfq-ws` look like it was flapping. A spec we deliberately will not sync is recorded in `docs/specs/.drift-acknowledged.json`, keyed by the SHA-256 of the canonical diff, which makes the check exit 3 and close the issue. Fingerprinting the *disagreement* rather than upstream means the acknowledgement expires the moment either side moves, so it is never permanent blindness. `clob` is acknowledged because upstream's own re-serialization made `example: 'Yes'` parse as boolean `true` on a `type: string` field. The issue body carries a key-path summary (changed JSON pointers with before → after, so drift inside `components.schemas` is named rather than merely counted) plus the canonicalized diff, composed in Python under GitHub's 65536-character cap.
  Deliberately excluded, each for one reason:
  <!-- generated:begin claude-schema-exclusions -->
  - Sports (`docs/specs/sports/`): Upstream's AsyncAPI documents a `slug`-keyed payload and a text ping/pong that the server never sends, so the mirror is modelled on captured wire frames, and diffing it would report drift forever.
  - Undocumented hosts (`docs/specs/undocumented/`): `user-pnl-api` and `lb-api` publish no spec to diff against; their shapes were derived from live responses, and `polyoxide-data`'s live suite is the drift check.
  - RTDS (`docs/specs/rtds/`): Upstream publishes no AsyncAPI for `ws-live-data`; the mirror is modelled on captured wire frames, so there is nothing to diff it against.
  - Deposit Wallets and session keys (`docs/specs/session-keys/`): The surface is almost entirely absent from the published CLOB and relayer OpenAPI (only `/deployed?type=WALLET` appears), so there is no mirror to diff; the SDK-generated fixtures are the drift check.
  - Binance USDⓈ-M (`docs/specs/binance/`): Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites in `polyoxide-binance` are the drift check.
  <!-- generated:end claude-schema-exclusions -->

**Tags decide.** A live test that fails through `polyoxide-test-support` prints `polyoxide-class=<tag>` alone on a stderr line just before it panics, and that line is all `classify_failures.py` reads. Only the tag just above the log's final `panicked at` report decides, since one anywhere else belongs to a panic the test caught or a spawned task raised; a `real` or unknown tag anywhere makes the failure `real`, and so does no tag at all. `.or_fail(ctx)` (`ResultExt`), and `fail(ctx, &err)` for an error in a match arm or a source chain, take the tag from the error's `Class` and `is_fault()` (`Network`, `Unavailable` and `RateLimited` are `transient`; `Restricted` is `environmental` when it is not a fault, such as a 451 region block, and `real` when it is, such as a 418 ban the client earned; every other class is `real`), the credential loaders (`load_env`, `optional_env`, `keychain`) count an empty value as absent and fail through `Missing::or_auth_gated()` as `auth-gated`, and `environmental(reason)` and `transient(reason)` tag what no error value carries. A foreign error is wrapped in the venue's own error first (`ApiError::Network`, `SportsError::Transport`, `UsdmWsError::from`, `WebSocketError::from`), and so is a raw socket's close frame, so it takes its tag from the socket table.

The regexes that once guessed a failure's kind from its panic text are deleted (Story 2.7), and `scripts/live_unwraps.py` keeps it that way. It freezes every regex the classifier's source builds, read from its AST; what is left parses the tag line, the panic report and nextest's retry suffix. It holds a per-file count, at zero, of `.unwrap()`/`.expect(` and `.unwrap_err()`/`.expect_err(` (and `Result::unwrap`-style paths) in live tests and of bare failure sites: every `panic!`, `unreachable!`, `todo!` and `unimplemented!`, and every `assert!`-family macro whose message interpolates or passes a value named for an error or a status. A line that must keep one, such as a parse of test data or an assertion on the response, ends with `// live-unwraps: <reason>`; it fails untagged, so it files as `real`, and moves to a per-file `opted_out` count that may not rise without a hand edit a reviewer sees. Each regex-era case is a row of `.github/scripts/tests/test_classify_failures.py`'s tag table, and each row naming an error the tests can build has a Rust twin in a `tests/failure_tags.rs` (`polyoxide-test-support` for core's `ApiError`, and binance, sports and rtds for theirs) asserting the tag that error prints.

A live target reads the environment only through the loaders, which take their env names as string literals; `gen_registry.py`'s `env_names()` reads those calls for the secrets check, and `direct_env_reads()` refuses any `std::env` or `env::` path, `env!`/`option_env!`, `dotenvy` or a library `from_env()` in a live target, whose names that check could not see. The loaders read a `.env` file into a private map behind the environment, never with `set_var`, and warn naming the file and line of any line that does not parse. nextest suffixes a retried test's name with `#<attempt>`; the classifier strips it, or `merge` would miss every persistent transient.

To enable CLOB/relay's auth-gated tests, set the `POLYMARKET_*`, `BUILDER_*` and `RELAYER_*` repo secrets. The loaders then find them and the tests start contributing real signal; nothing in the classifier changes. Relay's credentialed tests no longer pass silently without their secrets: they fail as `auth-gated`, like clob's.

## Module Organization

Most crates follow a consistent layout:
- `lib.rs` — public API re-exports
- `client.rs` — main client struct + builder
- `error.rs` — crate-specific error enum (uses `thiserror`)
- `types.rs` — domain types
- `api/` — namespace modules, one file per API group (markets, orders, etc.)

**WebSocket** support for the CLOB lives in `polyoxide-clob/src/ws/` (not core), feature-gated behind `ws` (not enabled by default in polyoxide-clob; default = `["gamma"]`). Two channels: `WebSocket::connect_market(asset_ids)` (public) and `WebSocket::connect_user(condition_ids, credentials)` (authenticated). Implements `futures_util::Stream`. `WebSocketBuilder` provides auto-ping keep-alive for long-running connections. The Perps socket (`polyoxide-perps/src/ws/`, feature `ws`), RTDS (`polyoxide-rtds`), the sports feed (`polyoxide-sports`) and Binance's market streams (`polyoxide-binance/src/usdm/ws/`, feature `ws`) are separate protocols in their own crates, each with its own `ensure_crypto_provider` copy.

Three market events — `best_bid_ask`, `new_market`, `market_resolved` — are withheld by the server unless the subscription sets `custom_feature_enabled`. Use `WebSocket::connect_market_with(ids, MarketSubscriptionOptions::default().with_custom_features())` to receive them. `MarketMessage` and `Channel` are `#[non_exhaustive]`, since upstream adds event types over time.

The user channel's market filter is optional: `WebSocket::connect_user_all_markets(creds)` omits it and receives events for every market, and `subscribe_markets` / `unsubscribe_markets` adjust it on a live connection without reconnecting. The market channel changes membership the same way — `subscribe_assets` / `unsubscribe_assets`, or a `MembershipHandle` (from `WebSocketWithPing::membership`, taken **before** `run`) while the ping loop drives the socket. Both frames are documented in the AsyncAPI mirrors (`SubscriptionRequestUpdate`). Verified live 2026-09-09: an added asset gets a fresh `book` in ~155 ms, a duplicate add gets nothing (unsubscribe then subscribe to force a snapshot), and an empty-membership socket stays open under the 10 s `PING` but is reset after ~125 s without it.

The WebSocket contracts are published as AsyncAPI, not OpenAPI — mirrored in `docs/specs/clob/asyncapi-{market,user}.json` and `docs/specs/sports/asyncapi.json`. A parity audit that only diffs the OpenAPI files will miss this whole surface.

**The sports feed is its own crate.** `polyoxide-sports` covers `wss://sports-api.polymarket.com/ws`, which takes no subscription and pushes every live match. Its one workspace dependency is `polyoxide-venue`, and it does not depend on core, `reqwest` or `alloy`, for the same reason as rtds. Upstream's AsyncAPI documents a `slug`-keyed payload and a text `"ping"`/`"pong"`; the server sends neither, so `docs/specs/sports/asyncapi.json` carries `x-observed-*` annotations and is excluded from `nightly-schema.yml`. `MatchUpdate` is modelled on the captured frames in `polyoxide-sports/tests/fixtures/` (refresh with `scripts/capture_sports_fixtures.py`), and the live test `live_frames_round_trip_and_carry_no_unmodelled_keys` is the host's drift detector.

**Pings are the sports feed's liveness signal, the opposite of rtds.** The server sends a protocol PING every 15 s, and data only while a match is live, so a quiet hour has no data at all. `SupervisedSportsWs` resets its 45 s staleness timer on any inbound frame, pings included; counting only data would drop a healthy connection every quiet hour. It is a `Stream` state machine with no background task, because no application message is ever sent to this host (only protocol pongs and a close reply), so the perps task shape is not needed. Pongs therefore go out only while the caller polls. A reconnect refused in a way retrying cannot fix (an HTTP status other than 408, 425, 429 or 5xx, or a malformed URL) is yielded as `SportsError::Connect` and ends the stream, instead of looping at the 60 s backoff ceiling with only a WARN to show for it. `SportsError::retrying_can_fix` decides, and its statuses mirror core's `is_retriable`.

**A sports frame is state, not an event.** The server re-sends unchanged state on a timer (56 of 121 frames in one capture were repeats), and the `ended: true` frame is usually sent once. `Event::Reconnected` tells a caller to reconcile games through gamma's `events?game_id=`; cricket's `metadataGameId` cannot be reconciled. See `docs/specs/sports/OBSERVED.md`. Gamma's half of the join is `Event::game_id`, `teams` (each `Team::ordering` a `HomeAway`), `sport` and `parent_event_id`, none of which `openapi.yaml` lists: they are modelled from the served `Event.json`. One game id can return the game and its child events, so keep the one with no `parent_event_id`. See `docs/specs/gamma/OBSERVED.md`.

**WebSocket TLS needs a nudge.** `reqwest 0.12` (via core) and `alloy`'s `reqwest 0.13` enable `ring` and `aws-lc-rs` on one shared `rustls`, which then installs no default `CryptoProvider`. `ws/client.rs` installs one before connecting; any code that calls `tokio_tungstenite::connect_async` directly must do the same or it will panic. `polyoxide-rtds` has its own copy for this reason. Every socket crate takes `rustls` from one `[workspace.dependencies]` entry that declares `ring` and `std`, so none relies on `reqwest` or `alloy` to turn `std` on; clob once compiled only because they did (DRIFT R5).

**RTDS is a separate crate and a separate protocol.** `polyoxide-rtds` covers `wss://ws-live-data.polymarket.com`, which multiplexes many topics over one connection under an `action`/`subscriptions` envelope — unlike the CLOB channels, which are one channel per connection. Its one workspace dependency is `polyoxide-venue`, which depends on nothing; it does not depend on core, `reqwest` or `alloy`, so a credential-free price feed does not pull in `alloy`: `polyoxide-clob --features ws` builds 352 crates against core's 161, and none of that signing stack is needed to read a price. Two tiers: `Rtds` is a bare `Stream`; `RtdsBuilder`/`SupervisedRtds` adds keep-alive, a staleness watchdog and reconnect-with-resubscribe.

**`full_accuracy_value` does not mean the same thing on every topic.** It is E18 fixed-point on the three Chainlink topics and a **plain decimal** on `crypto_prices` (Binance). Two frames captured a second apart both report BTC at ≈$79,697 with byte-identical payload keys, differing only in that scale — so the two spot payloads are separate types and the scale is never a runtime decision. A test asserting only "the value is a positive Decimal" passes on both and proves nothing; `the_two_spot_topics_do_not_share_a_scale` in `event.rs` is the one that holds this up, and it has been observed failing in both directions.

**Four RTDS behaviours have no counterpart in the docs**, all recorded in `docs/specs/rtds/OBSERVED.md`. A filter with one stray space delivers the backfill and then goes silent forever with no error, which is why `filters` is built by `serde_json` and never accepted as a caller string. One unrecognised topic returns zero frames for *every* topic in the same batch. A Chainlink-spot **snapshot** arrives labelled with the Binance topic, so `correct_mislabelled_spot_snapshot` keys on the symbol format — taking the label at face value files Chainlink prices under Binance, silently. And a backfill is only sent for a **symbol-filtered** subscription; an unfiltered one never receives one, so it cannot re-initialise from a snapshot after a reconnect.

`docs/specs/rtds/` is modelled on captured frames and is deliberately excluded from `nightly-schema.yml` — upstream publishes nothing to diff it against. The documented 5-second `PING` is neither required nor answered, so staleness, not the heartbeat, is the only liveness signal.

## Publishing Order

The order is computed, not written down. `scripts/publish_order.py` reads `cargo metadata` and puts each publishable member (`publish` unset, or naming `crates-io`) after every member it needs on crates.io first: its normal and build dependencies, and any dev-dependency that carries a version, since that one stays in the published manifest. A path-only dev-dependency is stripped at publish time and orders nothing. A cycle, or a normal or build dependency on a `publish = false` member, fails the release before anything is uploaded. Each `tombstones/*/Cargo.toml` comes after every workspace crate; the workspace excludes `tombstones`, or `cargo metadata` could not read them.

<!-- generated:begin claude-publish-order -->
Today's order: `polyoxide-venue` → `polyoxide-core` → `polyoxide-binance` → `polyoxide-data` → `polyoxide-gamma` → `polyoxide-perps` → `polyoxide-relay` → `polyoxide-clob` → `polyoxide-rtds` → `polyoxide-sports` → `polyoxide` → `polyoxide-cli`.
<!-- generated:end claude-publish-order -->

What each crate waits for is in the crate list under Workspace Architecture. `polyoxide-cli` is published like every other member. `polyoxide-py` is `publish = false` (not on crates.io); its Python wheels are built and published to PyPI via a separate step in the release workflow.

**One resumable loop.** `release.yml`'s publish job runs `scripts/finish_release.sh`, which makes up to three attempts. Each asks `publish_order.py list --max-new-names 5` for the (crate, version) pairs crates.io lacks, publishes the workspace pairs in one `cargo publish --no-verify -p A -p B …` (cargo orders them and waits for each to reach the index), then each tombstone by `--manifest-path`. A final recount confirms nothing is left. So a re-run after a partial release publishes only what is left. Exit 3 means more than five new crate names: crates.io rate-limits new registrations, so split the release. Exit 4 means the workspace cannot be published as it stands (a cycle, a dependency on a `publish = false` member, a manifest `cargo metadata` cannot read). Retrying cannot help either, so the loop stops at once on both. A recount that cannot reach crates.io ends in "could not confirm", which is not "still unpublished".

**What releases.** `release.yml` runs for a push to `main` in this repository, or a manual run (`workflow_dispatch`) on `main`. Its `branches: [main]` filter is not the guard, since it also matches a fork's pull request from a branch named `main`; the `version` job checks that the CI run was a `push` to this repository's `main`. Then `publish_order.py decide` reads the `v<version>` tag on origin (the peeled commit, since release tags are annotated), and the first matching rule wins:

1. A manual run on a commit CI has not passed fails, since every publish is `--no-verify`.
2. No tag: release. So the next green push to `main` releases any version still untagged, a fix-forward or an empty commit after a red version-bump commit included.
3. A tag at this commit and a GitHub release: skip. The run is a re-run of a finished release.
4. A tag at this commit and no GitHub release: release, resuming one whose last jobs failed. Publishing skips what crates.io has, and the tag is not pushed twice.
5. A tag at another commit, on a push that leaves the version as its parent had it: skip, with a plain log line. Every ordinary push lands here.
6. A tag at another commit, with the version lower than the parent's: skip with a warning.
7. A tag at another commit otherwise fails the run with `::error::`: the version was already released, so bump it.

A lookup that fails (`git ls-remote`, `gh`) fails the run rather than reading as "not released".

**Recovering a release.** Run Release by hand on `main` (`gh workflow run Release --ref main`). It publishes what is missing and finishes the tag, the GitHub release and the wheels. Running `scripts/finish_release.sh` by hand is a crates.io-only fallback: it refuses a dirty tree, a HEAD that is not on `origin/main`, or one CI has not passed on (`publish_order.py ci-passed`), and it takes `CARGO_REGISTRY_TOKEN` or a prior `cargo login`.

**The second guard is pending.** The `cargo` and `pypi` environments should accept deployments only from `main`, so that a run from any other branch, an edited `release.yml` on a feature branch among them, cannot reach the tokens. Neither has that branch restriction yet. It is a repository setting, not a file in this repository.
