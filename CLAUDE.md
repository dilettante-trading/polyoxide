# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Polyoxide is a multi-venue Rust SDK toolkit, one crate per venue: `polyoxide-polymarket` for Polymarket's APIs (CLOB trading, market data from Gamma, user data, gasless relay transactions, perps market data, and the crypto price and sports streams), `polyoxide-binance` for Binance's USDⓈ-M futures market data and streams and `polyoxide-kalshi` for Kalshi's event-contract market data and its order-book, ticker and trade streams, under a `polyoxide` umbrella with a module per venue. It also provides Python bindings for Polymarket and a standalone CLI for Polymarket and Binance. Hard fork of [polyte](https://github.com/roushou/polyte).

## Where the rules live

This file holds the commands, the CI gates and the rules that cut across crates. Each topic's rules are in `.claude/rules/<topic>.md`, which Claude Code loads when Claude reads or edits a file the topic's `paths:` globs match. When you work on a topic without opening its files (reviewing a diff, writing a plan), read its rules file first.

| File | Covers |
|---|---|
| `ci-gates.md` | what each CI job checks: features, fences, dep allowlists, the venue gate, removals, test bodies |
| `release.md` | the publish order, tombstones, the release loop and its rules, recovery |
| `testing.md` | mock suites, `polyoxide-test-support`, capture scripts |
| `live-tests.md` | live suites, failure tags, the nightly classifier, `live_unwraps.py` |
| `specs.md` | the `docs/specs/` mirrors and the schema-drift check |
| `send-loop.md` | core's send loop, hooks, pings, retry policies, reqwest, gzip, the header pins |
| `rate-limits.md` | window quotas, 429 holds, `Retry-After`, per-signer buckets, the measured tables |
| `errors.md` | the error hierarchy, `Classify`, `Retry-After` parsing, socket error classification |
| `clients.md` | builders, namespaces, query setters and their hand-written exceptions |
| `venue-traits.md` | market keys, records, the market-data traits, class bundles, `Trading` |
| `wire-enums.md` | `wire_enum!`, `open_enum!` and the enums still written by hand |
| `sockets.md` | the `polyoxide-ws` kit, its `Supervisor`, TLS, the scripted test server |
| `umbrella.md` | the `polyoxide` crate's features, re-exports and prelude |
| `cli.md` | the CLI's command tree, stream runner and data commands |
| `keychain.md` | the OS keychain, the stored credential kinds and the CLI's `credentials` command |
| `py.md` | the Python bindings |
| `polymarket-clob.md` | CLOB auth and signing, kill outcomes, its market-data and `Trading` impls |
| `polymarket-clob-ws.md` | the CLOB's bare and supervised sockets |
| `session-keys.md` | Deposit Wallets and session keys |
| `polymarket-gamma.md` | Gamma, its `closed` filter, Protocol V2 |
| `polymarket-data.md` | the Data API, v1 and v2 |
| `polymarket-relay.md` | the relayer |
| `polymarket-perps.md` | perps REST, its market-data impl and its sockets |
| `polymarket-rtds.md` | the RTDS price streams |
| `polymarket-sports.md` | the sports feed |
| `binance.md` | Binance USDⓈ-M: the weight budget, its policy, market-data impl and streams |
| `kalshi.md` | Kalshi: credentials, its market-data impl, the archive and its socket |

- A rule stands until the commit that supersedes it lands, and that commit edits the rule's text, here or in its rules file (AD-21). `.github/scripts/tests/test_claude_rules.py` fails when a rules file names no `paths:` globs (it would load into every session), when a glob matches no file, or when this table and the directory disagree.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) is the agent's guide to the multi-venue restructure: the shape it builds towards, where each kind of change goes, the rules that bite and the release stage. Read it before moving code between crates or adding one. These files describe the code as it is now; the guide describes the target, mostly in the present tense, and says where each kind of change goes. Its "Standing rules this replaces" table lists the rules due to go, and may still list ones already gone. It is copied from the architecture spine's `ARCHITECTURE-GUIDE.md` by `scripts/gen_registry.py --write` and never edited by hand (CI fails when the two differ); its stage line is fed by `[workspace.metadata.polyoxide] stage` in the root `Cargo.toml`.
- [`docs/MUTANTS.md`](docs/MUTANTS.md) lists the mutation-tested rules: for each, the line it holds on, the mutation, and the tests that fail under it. Move one of those lines or tests and its row moves with it, re-proved.

## Commands

**MSRV:** 1.91 (set in the workspace `Cargo.toml`).

**Dev shell:** `nix develop` (`flake.nix`) puts Rust 1.99.0 with clippy and rustfmt on `PATH`, with the MSRV toolchain as `msrv-cargo` (cargo 1.91.0, building into `target/msrv`), cargo-hack 0.6.45, cargo-semver-checks 0.51.0, cargo-nextest, uv and python3, so `nix develop --command <step>` runs a CI step on CI's tools. CI itself does not use the flake: its removals job pins Rust 1.99.0 and its msrv job 1.91, and every other job floats on `stable`, so a newer stable's lints can fail only in CI.

```bash
cargo build --all-features --workspace                          # everything
cargo build -p polyoxide-polymarket                             # one crate
cargo test --all-features --workspace                           # all tests
cargo test -p polyoxide-polymarket --all-features -- test_name  # one test by name
cargo clippy --all-targets --all-features -- -D warnings        # must pass with zero warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
cargo fmt --all                                                 # CI checks with `-- --check`
cargo test -p polyoxide-polymarket --test live_clob -- --ignored    # live tests hit real APIs
```

### The nine CI jobs

Run the jobs a change touches before pushing (inside `nix develop`); `-D warnings` makes every warning an error.

| Job | Run locally | Fails on |
|---|---|---|
| format | `cargo fmt --all -- --check` | unformatted code |
| lint & test | `cargo clippy --all-targets --all-features -- -D warnings`, `cargo nextest run --all-features --workspace`, `cargo test --doc --all-features --workspace`, then `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` | any warning, a failing test or doctest (the module READMEs are doctests), a doc warning |
| package | `python3 scripts/publish_order.py check-manifests && cargo publish --workspace --dry-run --no-verify --locked` | a publishable crate without a description or licence (cargo only warns), a manifest fault, a tombstone breaking its rules |
| msrv | `msrv-cargo check --workspace --all-features && msrv-cargo doc --no-deps --workspace --all-features` | anything that does not build on 1.91 (warnings allowed) |
| features | `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`, then the header pins, the clob-without-gamma tests and the specta checks (`ci-gates.md`) | a feature that compiles only alongside another; a request header another feature adds |
| removals | `python3 scripts/api_removals.py check --baseline v0.40.0` | a removed public item that `docs/s3-removals.md` does not list |
| test bodies | `python3 scripts/test_body_diff.py --baseline v0.40.0` | a changed or lost test-context item with no `[[drift]]` or `[[retired]]` entry |
| python bindings | `cd polyoxide-py && uv sync --reinstall-package polyoxide && uv run pytest tests/` | a binding regression (runs once format passes) |
| CI scripts | `cd .github/scripts && uv sync && uv run pytest tests/` | dependency fences, `Classify` coverage, dep allowlists, module READMEs, the venue gate, generated regions, the rules files |

### Rules the gates hold

- **A red CI job withholds the release.** `release.yml` triggers on `workflow_run: [CI]` with `conclusion == 'success'`, so any red job (the doc build, msrv, features, removals and test bodies included) silently withholds the release tag: the version bump lands on `main` and nothing publishes, with no obvious connection between the two. Fixing forward is enough: the next green push to `main` releases any version still untagged.
- **Clippy and tests passing is not enough.** The doc build makes `rustdoc::private_intra_doc_links` an error: a doc comment on a `pub` item may not use ``[`link`]`` syntax to reference a `pub(crate)` item. Doctests do not catch it; they run the code in doc comments and never resolve the prose links. Make the referenced item `pub` or state the fact inline. A bare `<word>` in a doc comment fails too, as an unclosed HTML tag (clap's `///` help on a `pub` field is a doc comment), so put placeholders in backticks.
- **Removing a public item needs a line in [`docs/s3-removals.md`](docs/s3-removals.md).** The removals job compares against the S3 start tag, `v0.40.0` (the S2 release; CI's one `S3_BASELINE`). A removal, or any other major-level change, raises the 0.x minor in a version-bump commit of its own on `main`.
- **A moved test keeps its body (AD-12).** The test-bodies job fails on a changed test-context item unless a `[[drift]]` entry in `ci/test-body-diff.toml` names a DRIFT row and the diff's sha256, and on a lost one unless a `[[retired]]` entry names it. git pairs only tracked files, so stage a moved file before running the script locally.
- **Generated regions are never edited by hand.** Text between `<!-- generated:begin <id> -->` and `<!-- generated:end <id> -->` markers, here and in README.md, docs/specs/INDEX.md, docs/ARCHITECTURE.md, SELF-HEALING.md and the two nightly workflows, is written by `scripts/gen_registry.py` from each crate's `[package.metadata.polyoxide]`, the root manifest's `[workspace.metadata.polyoxide]` (`stage` and `mirrors`) and `cargo metadata`. Change the metadata, run `python3 scripts/gen_registry.py --write`, and expect CI's scripts job to fail on any region that differs from what the generator produces.
- **The foundation crates name no venue.** `scripts/venue_gate.py` (CAP-7) reads every file under `polyoxide-venue`, `-core`, `-ws` and `-test-support` for each venue and product id and each venue's identifiers (Polymarket's `Poly-RateLimit`; Binance's `fapi`, `fstream` and `X-MBX`). A foundation doc or comment says what it means without the venue's words, and venue code moves to the venue's crate; only a protected test name, or a probe that measured one venue's host, takes a `gate_exceptions` entry, in that venue's crate.
- **Dependency fences.** Only `polyoxide-core` declares `reqwest` (other members name its types through `polyoxide_core::reqwest`); only `polyoxide-ws` declares `tokio-tungstenite` and `rustls`; no venue crate depends directly on `governor`. Each crate's normal dependency tree is pinned per feature set in `ci/dep-allowlists/`: after a deliberate change, regenerate the files with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` and say why in the commit.
- **cargo-semver-checks 0.51.0 and Rust 1.99.0 are pinned together** in `ci.yml`, `release.yml` and `flake.nix`, because the tool reads only the rustdoc JSON some toolchains emit: upgrade both at once, in all three. The flake takes cargo-hack and cargo-semver-checks from nixpkgs, so check both versions in the shell after a `nix flake update`.

## Workspace

<!-- generated:begin claude-crate-count -->
Ten crates, in publish order, each with the workspace crates its build needs. Crates that are not published come last:
<!-- generated:end claude-crate-count -->
<!-- generated:begin claude-graph -->
- `polyoxide-venue` — Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey`, the `KeySpace` a venue implements per product and the `ProductClass` it declares), the market-data traits and their class bundles, with the records they return and their typed `Extensions` (feature `market`), and the `Trading` trait with its order, fill, position and balance records and its `TradingEvents` stream (feature `trading`); needs: nothing in the workspace; every public error type in the workspace implements its `Classify` trait but `polyoxide-ws`'s `WsError`, which `impl_ws_classification!` classifies in each venue crate that wraps it; by default it depends on nothing, and it has no HTTP, socket or signing dependency
- `polyoxide-core` — Core utilities and shared types; needs: `polyoxide-venue`; the send loop and its hooks, the HTTP client, window quotas, capacity buckets, holds, errors, macros and the keychain; it names no venue, which `scripts/venue_gate.py` holds
- `polyoxide-ws` — The socket kit: the TLS install, `Backoff`, `Timing` and its setters, the deadline arithmetic, `WsError`, connect with a timeout, the bare tier, request/answer correlation, the generic `Supervisor` for a venue `Protocol`, and the scripted test server (feature `test-server`); needs: nothing in the workspace; depends on neither `polyoxide-core` nor `polyoxide-venue`, and builds only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls
- `polyoxide-binance` — Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`); re-exported by the umbrella as `polyoxide::binance`; needs: `polyoxide-core` (under `usdm`, on by default), `polyoxide-venue`, `polyoxide-ws` (under `usdm-ws`)
- `polyoxide-kalshi` — Client library for Kalshi's APIs, one feature per module: `predictions` (event-contract market data, with its streams under `predictions-ws`); re-exported by the umbrella as `polyoxide::kalshi`; needs: `polyoxide-core` (under `predictions`, on by default), `polyoxide-venue`, `polyoxide-ws` (under `predictions-ws`)
- `polyoxide-polymarket` — Client library for Polymarket's APIs, one feature per module: `clob` (order book, with its sockets under `clob-ws`), `gamma` (market data), `data` (user data, v1 and v2), `relay` (gasless transactions), `perps` (perpetual futures market data, with its sockets under `perps-ws`), `rtds` (crypto price streams) and `sports` (live sports scores), with the code the HTTP modules share in `shared` and, with `clob`, `gamma` and `data`, the unified `Polymarket` client; re-exported by the umbrella as `polyoxide::polymarket`; needs: `polyoxide-core` (under `clob`, `gamma`, `data`, `relay`, `perps`, on by default), `polyoxide-venue`, `polyoxide-ws` (under `clob-ws`, `perps-ws`, `rtds`, `sports`); only `clob` and `relay` build `alloy`, and only the HTTP modules build core, which `test_dep_allowlists.py` holds per feature; `clob` does not turn on `gamma`, and with both the CLOB client looks a proxy wallet up through Gamma; `rtds` and `sports` build no `polyoxide-core`, `reqwest` or `alloy`, so a credential-free feed builds no HTTP or signing stack; `Trading` for the CLOB (`clob::venue`) is built with `clob-ws` and `data` together: its events stream runs on the supervised user socket, which is `clob-ws`'s, and its positions read the Data API; perps' auth and trading pending
- `polyoxide` — The multi-venue umbrella: `polyoxide::polymarket`, `polyoxide::binance` and `polyoxide::kalshi` behind features named for each venue and module, `polyoxide::venue` with the market-data traits, `polyoxide::parse_key` and a `prelude`; needs: `polyoxide-binance` (without default features, under `binance-usdm`), `polyoxide-kalshi` (without default features, under `kalshi-predictions`), `polyoxide-polymarket` (without default features, under `polymarket-clob`, `polymarket-gamma`, `polymarket-data`, `polymarket-relay`, `polymarket-perps`, `polymarket-rtds`, `polymarket-sports`, on by default), `polyoxide-venue`
- `polyoxide-cli` — CLI tool for querying Polymarket APIs and Binance USDⓈ-M market data; needs: `polyoxide-binance` (with `usdm-ws`, without default features), `polyoxide-core` (with `keychain`, under `keychain`), `polyoxide-polymarket` (with `clob-ws`, `gamma`, `data`, `rtds`, `sports`, without default features), `polyoxide-venue` (under `keychain`); its `keychain` also turns on `polyoxide-polymarket`'s `relay` and `keychain`, for the relay's keychain service
- `polyoxide-py` — Python bindings via PyO3 (`publish = false`, wheels on PyPI); needs: `polyoxide-polymarket` (with `clob`, `gamma`, `data`, without default features), `polyoxide-venue`
- `polyoxide-test-support` — Test toolkit: the failure tags the nightly classifier reads, the credential loaders, and the agreement, fixture and soak helpers (`publish = false`); needs: `polyoxide-core` (with `keychain`), `polyoxide-venue`; a path-only dev-dependency of the crates whose tests use it; it names no venue
<!-- generated:end claude-graph -->
<!-- generated:begin claude-cli-deps -->
Note: `polyoxide-cli` does **not** depend on the `polyoxide` umbrella. It depends directly on the component crates — `polyoxide-binance` (with `usdm-ws`, without default features) and `polyoxide-polymarket` (with `clob-ws`, `gamma`, `data`, `rtds`, `sports`, without default features) — plus `polyoxide-core` (with `keychain`) and `polyoxide-venue` only under the optional `keychain` feature.
<!-- generated:end claude-cli-deps -->

<!-- generated:begin claude-umbrella-features -->
**polyoxide** (the umbrella) uses feature flags: `polymarket`, `polymarket-clob`, `polymarket-clob-ws` (`polyoxide-polymarket/clob-ws`, `polyoxide-venue/trading`), `polymarket-gamma`, `polymarket-data`, `polymarket-relay`, `polymarket-perps`, `polymarket-perps-ws` (`polyoxide-polymarket/perps-ws`), `polymarket-rtds`, `polymarket-sports`, `binance`, `binance-usdm`, `binance-usdm-ws` (`polyoxide-binance/usdm-ws`), `kalshi`, `kalshi-predictions`, `kalshi-predictions-ws` (`polyoxide-kalshi/predictions-ws`), `full` (all), `keychain` (`polyoxide-polymarket?/keychain`). Default = polymarket.
<!-- generated:end claude-umbrella-features -->

**Module layout.** Each module of a venue crate lives in `src/<module>/` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay`, `perps`, `rtds` and `sports`; `polyoxide-binance`'s `usdm`; `polyoxide-kalshi`'s `predictions`), with `mod.rs` in place of `lib.rs`. Most HTTP modules have `client.rs` (the client and its builder; `usdm`'s and `predictions`' are in their `mod.rs`), `error.rs` (a `thiserror` enum), `types.rs` (Kalshi's is `types/`), `api/` (one file per API group) and their sockets in `ws/`; `rtds` and `sports` are socket modules. A module's `README.md` sits beside `mod.rs` and is compiled as a doctest under the module's feature (all but `rtds`, whose guide is its `mod.rs` docs). Its suites are `tests/<kind>_<module>.rs`.

## Patterns every crate follows

- **Builders, namespaces and setters.** Every HTTP client is built on a `polyoxide_core::ClientConfig` (`Gamma::builder()`, `Clob::builder(private_key, credentials)`, `Usdm::builder()`, …), with its transport setters from `client_config_setters!`. Endpoints sit in namespaces (`gamma.markets()`, `data.v2()`) whose accessors come from `namespaces!`, and query parameters are setters from `query_setters!`, chained before `.send().await?`: `gamma.markets().list().open(true).send().await?`. `clients.md` lists the hand-written exceptions, and a new setter gets a row in its module's `tests/query_setters_*.rs`.
- **Decimals.** Price and size fields use `rust_decimal::Decimal` with `serde(with = "rust_decimal::serde::str")`.
- **Wire enums** come from `polyoxide-venue`'s `wire_enum!` (a closed set the client sends) and `open_enum!` (a set the venue extends, which keeps an unknown spelling in `Other(String)`).
- **Errors.** Each HTTP module's `<Module>Error` wraps core's `ApiError`, is `#[non_exhaustive]`, and decodes in its one `From<ApiError>`; foreign errors enter only through `ApiError`, so no module implements `From<reqwest::Error>`, `From<serde_json::Error>` or `From<url::ParseError>`. Every public error implements `polyoxide_venue::Classify` and is listed in its crate's `lib.rs` `const _` assertion. Its class decides retriability for callers (`Network`, `Unavailable` and `RateLimited` are retriable); no error has an inherent `is_retriable` or `retry_after`.
- **One send loop.** `HttpClient::send` in `polyoxide-core` is the only retry loop: never add another. It retries only what the client's policy decides, and no policy retries 5xx or 408 (AD-17). Core names no venue: a venue's retry policy, rate-limit tables, throttle and signing live in its own crate (Polymarket's in `polyoxide_polymarket::shared`).
- **Rate limits.** Never give a window quota depth (`allow_burst`), or aim one at the published count (`RESERVED_FRACTION` reserves a tenth). A `Retry-After` may only extend a wait, a cooldown only extends, and a 429 holds the whole client. The per-signer CLOB buckets are the opposite model and hold their published burst.
- **Sockets** connect through the `polyoxide-ws` kit, never `tokio_tungstenite::connect_async` directly: the kit installs the rustls `CryptoProvider` a direct connect panics without. A host that requires pings gets them on the wall clock; the rtds pump, which pings only on a quiet tick, is not the shape to copy.
- **Market keys and the venue traits.** A `MarketKey` (`venue.product:id`) is made only by its product's `KeySpace`; the market-data traits and `Trading` live in `polyoxide-venue` and return `ClassifiedError`.

## Environment

- CLOB trading and user data: `POLYMARKET_PRIVATE_KEY` (hex) and the L2 credentials `POLYMARKET_API_KEY`, `POLYMARKET_API_SECRET` (base64) and `POLYMARKET_API_PASSPHRASE`.
- Relay: `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` (HMAC auth) **or** `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS` (static key auth); optionally `RELAYER_URL` and `CHAIN_ID`.
- Kalshi: no client reads the environment; `PredictionsBuilder::credentials` takes an API key id and a PEM private key. Its signed live suites read `KALSHI_DEMO_API_KEY_ID` and `KALSHI_DEMO_PRIVATE_KEY`, against the demo host only.
- **Keychain** (feature `keychain`, off by default): `Account::from_keychain()` (CLOB, the `polyoxide-clob` service), `BuilderAccount::from_keychain()` (relay, `polyoxide-relay`), or the CLI's `polyoxide polymarket credentials <store|show|delete> <clob|builder>`. Each account's fields are listed once, in its `polyoxide_core::keychain::StoredCredential` impl, which its `delete_from_keychain` and the CLI both use.

## API specs

`docs/specs/` mirrors the upstream API documentation, one directory per host, indexed by `docs/specs/INDEX.md`: the source of truth for endpoint contracts, rate limits and response schemas. Never edit a mirror that `nightly-schema.yml` watches, which must stay byte-faithful; record what a host actually does in its `OBSERVED.md`.

**Not fully implemented.** `docs/specs/` also mirrors three upstream APIs that no crate fully covers, so parity audits can see them: **Perps** (`perps/`, 74 endpoints on `api.perpetuals.polymarket.com`, with its own header auth; `polyoxide-polymarket`'s `perps` module implements the 29 public `/v1/info/*` routes), **Bridge** (`bridge/`, 5 endpoints) and **Combos RFQ** (`combos-rfq/`, 4 endpoints). **Data API v2** (`data-v2/`, 21 endpoints under `/v2` on `data-api.polymarket.com`) is implemented as `data.v2()`, beside the v1 routes.

## Testing and the nightly

- Each HTTP module has a `mockito` suite, `tests/mock_<module>.rs`; socket suites run on `polyoxide-ws`'s scripted server; the shared test helpers live once, in `polyoxide-test-support`.
- Live suites, `tests/live_*.rs`, hit the real APIs, are `#[ignore]`d, and run with `-- --ignored`. A live test fails through test-support's tags (`.or_fail`, `fail`, the credential loaders, `environmental`, `transient`), not through a bare `unwrap`, `expect` or `panic!`: the nightly classifies a failure by its tag line alone. `scripts/live_unwraps.py` holds such sites at zero per file; a line that must keep one ends with `// live-unwraps: <reason>` and counts as opted out.

Two workflows run at `0 6 * * *` UTC and on `workflow_dispatch`:

- `.github/workflows/nightly-behavioral.yml` runs the live suites, one job per crate and suite, and runs again on Saturday and Sunday at 18:30 UTC for the sports feed. A `real` failure files or updates the `nightly-behavioral` issue:
  <!-- generated:begin claude-nightly -->
  - `live-polyoxide-binance-usdm` (15 min): `live_usdm` with `--features usdm`, `live_usdm_ws` with `--features usdm-ws`
  - `live-polyoxide-kalshi-predictions` (15 min): `live_predictions` with `--features predictions`, `live_predictions_account` with `--features predictions`, `live_predictions_venue` with `--features predictions`, `live_predictions_ws` with `--features predictions-ws`; secrets `KALSHI_DEMO_API_KEY_ID`, `KALSHI_DEMO_PRIVATE_KEY`
  - `live-polyoxide-polymarket-clob` (15 min): `live_clob` with `--features clob,gamma`, `live_clob_trading` with `--features clob-ws,data,gamma`, `live_clob_ws` with `--features clob-ws`; secrets `POLYMARKET_API_KEY`, `POLYMARKET_API_PASSPHRASE`, `POLYMARKET_API_SECRET`, `POLYMARKET_BUILDER_CODE`, `POLYMARKET_PRIVATE_KEY`
  - `live-polyoxide-polymarket-clob-session-keys` (40 min): `live_clob_session_keys` with `--features clob,relay,gamma`; secrets `BUILDER_API_KEY`, `BUILDER_PASS_PHRASE`, `BUILDER_SECRET`, `POLYMARKET_DW_OWNER_PRIVATE_KEY`, `POLYMARKET_DW_SESSION_PRIVATE_KEY`, `POLYMARKET_DW_WALLET`
  - `live-polyoxide-polymarket-gamma` (15 min): `live_gamma` with `--features gamma`
  - `live-polyoxide-polymarket-data` (15 min): `live_data` with `--features data`
  - `live-polyoxide-polymarket-relay` (15 min): `live_relay` with `--features relay`; secrets `BUILDER_API_KEY`, `BUILDER_PASS_PHRASE`, `BUILDER_SECRET`, `POLYMARKET_PRIVATE_KEY`, `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS`
  - `live-polyoxide-polymarket-perps` (15 min): `live_perps` with `--features perps`, `live_perps_ws` with `--features perps-ws`
  - `live-polyoxide-polymarket-rtds` (15 min): `live_rtds` with `--features rtds`
  - `live-polyoxide-polymarket-sports` (20 min): `live_sports` with `--features sports`
  - `live-polyoxide-venue` (15 min): `live_venue` with `--features polymarket-clob,polymarket-perps,binance-usdm,kalshi-predictions`
  - `live-polyoxide-cli-live` (15 min): `live_api`
  <!-- generated:end claude-nightly -->
- `.github/workflows/nightly-schema.yml` compares each published upstream spec with its mirror in `docs/specs/` and files a `schema-drift` and `spec:<id>` issue on drift.
  <!-- generated:begin claude-schema-watch -->
  It watches nine OpenAPI (`clob`, `gamma`, `data`, `data-v2`, `relay`, `perps`, `bridge`, `combos-rfq`, `kalshi`) and five AsyncAPI (`clob-ws-market`, `clob-ws-user`, `perps-ws`, `combos-rfq-ws`, `kalshi-ws`), each filed under its own `spec:<id>` label.
  <!-- generated:end claude-schema-watch -->
  Deliberately excluded, each for one reason:
  <!-- generated:begin claude-schema-exclusions -->
  - Sports (`docs/specs/sports/`): Upstream's AsyncAPI documents a `slug`-keyed payload and a text ping/pong that the server never sends, so the mirror is modelled on captured wire frames, and diffing it would report drift forever.
  - Undocumented hosts (`docs/specs/undocumented/`): `user-pnl-api` and `lb-api` publish no spec to diff against; their shapes were derived from live responses, and `polyoxide-polymarket`'s `live_data` suite is the drift check.
  - RTDS (`docs/specs/rtds/`): Upstream publishes no AsyncAPI for `ws-live-data`; the mirror is modelled on captured wire frames, so there is nothing to diff it against.
  - Deposit Wallets and session keys (`docs/specs/session-keys/`): The surface is almost entirely absent from the published CLOB and relayer OpenAPI (only `/deployed?type=WALLET` appears), so there is no mirror to diff; the SDK-generated fixtures are the drift check.
  - Binance USDⓈ-M (`docs/specs/binance/`): Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites in `polyoxide-binance` are the drift check.
  <!-- generated:end claude-schema-exclusions -->

## Releasing

`scripts/publish_order.py` computes the publishing order from `cargo metadata`.
<!-- generated:begin claude-publish-order -->
Today's order: `polyoxide-venue` → `polyoxide-core` → `polyoxide-ws` → `polyoxide-binance` → `polyoxide-kalshi` → `polyoxide-polymarket` → `polyoxide` → `polyoxide-cli`.
<!-- generated:end claude-publish-order -->
`polyoxide-py` is `publish = false`; its wheels go to PyPI from the release workflow.

- A version bump is a commit of its own on `main`. `release.yml` publishes it once CI is green there, and the next green push releases any version still untagged.
- Anything that publishes (`scripts/finish_release.sh`, `cargo publish` without `--dry-run`, `gh workflow run Release`) cannot be taken back: no old version is ever yanked, and every publish is `--no-verify`.
- Recover a partial release by running Release by hand on `main`: `gh workflow run Release --ref main`.
