# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Polyoxide is a multi-venue Rust SDK toolkit, one crate per venue: `polyoxide-polymarket` for Polymarket's APIs (CLOB trading, market data from Gamma, user data, gasless relay transactions, perps market data, and the crypto price and sports streams) and `polyoxide-binance` for Binance's USDⓈ-M futures market data and streams, under a `polyoxide` umbrella with a module per venue. It also provides Python bindings for Polymarket and a standalone CLI for both venues. Hard fork of [polyte](https://github.com/roushou/polyte).

## Architecture guide

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) is the agent's guide to the multi-venue restructure: the shape it builds towards, where each kind of change goes, the rules that bite, and the release stage the workspace is in. Read it before moving code between crates or adding one. It is copied from the architecture spine's `ARCHITECTURE-GUIDE.md` by `scripts/gen_registry.py --write` and is only ever regenerated from the spine, never edited by hand; CI fails when the two differ. Its stage line is a generated region fed by `[workspace.metadata.polyoxide] stage` in the root `Cargo.toml`.

The two documents describe different things. This file describes the code as it is now; the guide describes the target, mostly in the present tense, and says where each kind of change goes. A rule here stands until the commit that supersedes it lands, and that commit edits the rule here (AD-21). The guide's "Standing rules this replaces" table lists the rules due to go, and may still list ones already gone: "call `note_rate_limited` before `should_retry`" went when Stories 3.4 and 3.5 moved its last call sites, clob's loop and relay's three, onto the send loop, and `impl_api_error_conversions!` went with Story 3.11, when each module's `From<ApiError>` became its one decode.

[`docs/MUTANTS.md`](docs/MUTANTS.md) lists the mutation-tested rules: for each, the line it holds on, the mutation, and the tests that fail under it. Move one of those lines or tests and the ledger moves with it, re-proved.

## Build & Development Commands

**MSRV:** 1.91 (set in workspace `Cargo.toml`).

**Dev shell:** `nix develop` (`flake.nix`) puts the versions CI installs on `PATH`: Rust 1.99.0 with clippy and rustfmt, the MSRV toolchain as `msrv-cargo` (cargo 1.91.0, building into `target/msrv`), cargo-hack 0.6.45, cargo-semver-checks 0.51.0, cargo-nextest, uv and python3, so `nix develop --command <step>` runs a CI step on CI's versions. CI itself does not use the flake. cargo-hack and cargo-semver-checks come from the locked nixpkgs, so check both versions in the shell after a `nix flake update`.

```bash
# Build entire workspace
cargo build --all-features --workspace

# Build a single crate
cargo build -p polyoxide-polymarket

# Run all tests
cargo test --all-features --workspace

# Test a single crate
cargo test -p polyoxide-polymarket --all-features

# Run a single test by name
cargo test -p polyoxide-polymarket --all-features -- test_name

# Lint (must pass with zero warnings)
cargo clippy --all-targets --all-features -- -D warnings

# Docs (must pass with zero warnings — see the note on intra-doc links below)
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace

# Format check
cargo fmt --all -- --check

# Format fix
cargo fmt --all

# Package gate (CI's package job): the metadata crates.io requires and the tombstones' rules,
# then the publish dry run, and each tombstone packaged without building it (none since S3)
python3 scripts/publish_order.py check-manifests
cargo publish --workspace --dry-run --no-verify --locked
for m in tombstones/*/Cargo.toml; do [ -e "$m" ] || continue; cargo package --no-verify --manifest-path "$m" || exit 1; done
```

CI runs nine jobs: **format** (standalone), **lint & test** (clippy, `cargo nextest run`, doctest, then `cargo doc` — sequentially in one job), **package** (`cargo publish --workspace --dry-run --no-verify`, plus `scripts/publish_order.py check-manifests`, which fails when a publishable crate lacks a description or a licence, since cargo only warns about those, or a tombstone breaks its rules (see Publishing Order), then `cargo package --no-verify --manifest-path` for each tombstone, which the workspace dry run cannot see), **msrv** (`cargo check` and `cargo doc` on Rust 1.91, warnings allowed), **features** (`cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`, so a feature that compiles only alongside another fails, then each HTTP module's header pin in its minimal build: `cargo test -p polyoxide-polymarket --no-default-features --features <m> --test headers_<m>` for clob, gamma, data, relay and perps, and `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm` for Binance's `usdm`, one `-p` per command so their features never unify (the pin test derives the HTTP modules from cargo metadata: a venue crate's products whose feature builds `polyoxide-core`, so `rtds` and `sports` pin none), then each of those pins again under the umbrella's `full`, `cargo test -p polyoxide -p <venue crate> --features polyoxide/full --test headers_<m>`, which unifies every module of every venue into the pinned build (the pin test derives that step too, and fails unless `full` turns each module on), then `mock_clob`'s `a_proxy_order_with_no_funder_is_refused_without_gamma` with `--features clob` alone, the one build that compiles clob's refusal of a proxy order with no funder, since `clob` does not imply `gamma`, then `cargo check -p polyoxide-polymarket --no-default-features --features <m>,specta` for each HTTP module of a crate with `specta` (clob, gamma, data, relay and perps; the test derives them from cargo metadata), since a module's specta extras (`specta?/chrono`, `specta?/rust_decimal`) are weak features that neither `--each-feature`, which builds `specta` alone, nor `--all-features`, which turns every extra on at once, builds apart), **removals** (`scripts/api_removals.py check` against the S3 start tag `v0.40.0`; see below), **test bodies** (`scripts/test_body_diff.py --baseline v0.40.0`, Python only; see below), **python bindings** (`uv run pytest tests/` in `polyoxide-py`, gated on **format** passing), and **CI scripts** (`uv run pytest tests/` in `.github/scripts`, whose `test_dependency_fences.py` fails when a member that declares a `venue` depends directly on `governor`, since a venue crate paces its requests with core's throttle primitives, and when a member depends directly, in any dependency kind, on a crate another member owns: `reqwest` is `polyoxide-core`'s, and `tokio-tungstenite` and `rustls` are `polyoxide-ws`'s, and each owner must declare its crate so the check cannot pass vacuously; whose `test_classify_coverage.py` fails when a public error type is missing from its crate's `Classify` assertions (a venue crate gates one per module), when a type with a `polyoxide_ws::WsError` field sits in a file that does not expand `polyoxide_venue::impl_ws_classification!`, and when a file that expands it does not call its `ws_class`, `ws_is_fault` and `ws_retry_after`; and whose `test_dep_allowlists.py` fails when `cargo tree -p <crate> -e normal --target x86_64-unknown-linux-gnu`, with the features the file names (and `--no-default-features` for `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide`, whose files name every feature they build; the socket kit's bare file is its default build), differs from `ci/dep-allowlists/<crate>[@<features>].txt`, naming each crate that joined or left the tree, or when an allowlist admits a crate its deny-list refuses, naming the rule: `polyoxide-ws`'s files are held to the socket-only deny-list (`reqwest`, `alloy*`, `polyoxide-core`, `governor`, `hmac`, `sha2`, `keyring` and `polyoxide-venue`), and `polyoxide-polymarket`'s per feature, after the features a file names turn on the ones they imply, so `reqwest`, `polyoxide-core`, `governor` and `sha2` only with an HTTP module (`clob`, `gamma`, `data`, `relay` or `perps`), which holds the socket modules `rtds` and `sports` to the socket-only tree, `alloy*` and `hmac` only with `clob` or `relay`, and `keyring` only with `keychain` (NFR2; its `@gamma`, `@data`, `@perps`, `@perps-ws`, `@clob`, `@clob-ws`, `@relay`, `@clob,keychain`, `@rtds`, `@rtds,test-server`, `@sports` and `@sports,test-server` files exist, and `@perps` must hold `reqwest`, `@clob` and `@relay` `alloy` and `@clob,keychain` `keyring`, so no rule can pass vacuously), and `polyoxide-binance`'s per feature too, so `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `usdm-ws` (its `@usdm` and `@usdm-ws` files exist, and `@usdm-ws` must hold `polyoxide-ws`; `rustls` is not on the rule, since core's HTTP client builds it for TLS), and the umbrella `polyoxide`'s per feature on its own names, so the HTTP stack only with an HTTP module of either venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps` or `binance-usdm`), `alloy*` and `hmac` only with `polymarket-clob` or `polymarket-relay`, and `keyring` only with `keychain` (its `@polymarket-rtds` and `@polymarket-sports` files are the socket-only tree, and `@polymarket-perps` must hold `reqwest`); after a deliberate change, regenerate the files with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` and say why in the commit; and whose `test_module_readmes.py` fails when a venue crate's `src/<product>/README.md` is not included exactly once under `cfg(all(doctest, feature = "<product>"))`, or such an include names no file, or a module directory has no README and no entry in its `WITHOUT_README` (only `rtds`, whose guide is its `mod.rs` docs), an entry failing once its module has a README; and whose `test_venue_gate.py` runs the venue gate, `scripts/venue_gate.py` (CAP-7), on the real tree, and on v0.39.0's core, which it must catch, so the job checks out every tag). **The foundation crates name no venue.** The gate reads every file under `polyoxide-venue`, `-core`, `-ws` and `-test-support`, case-insensitively, for each venue id as a substring, each product id as an identifier segment (split at `_`, `-`, `.`, `::` and case boundaries, so `clob` in `clob_default` and `perps` in `PERPS_GENERAL`, less the generic words of `ci/venue-gate-skip.txt`, `data` and `events`), each `<venue>.<product>:` prefix and each venue's `identifiers` (Polymarket's `Poly-RateLimit`, and Binance's `fapi`, `fstream` and `X-MBX`, its hosts and weight header), all read from cargo metadata. A hit passes only through a `gate_exceptions` entry, `{ path, term, reason }`, in the metadata of a crate of the hit's venue, and an entry that matches nothing fails, so an exception never outlives what it excuses. A foundation doc or comment says what it means without the venue's words; venue code moves to the venue's crate; and only a protected test name, or a probe that measured one venue's host, takes an exception, in that venue's crate (today the ws suites' `mod perps`, `mod binance` and their kin, excepted by polyoxide-polymarket and polyoxide-binance). Clippy uses `-D warnings` (all warnings are errors). The package job catches a manifest fault on the PR that introduces it rather than halfway through a release, and skips `publish = false` members on its own.

**Removing a public item needs a line in [`docs/s3-removals.md`](docs/s3-removals.md).** From stage S3 the removals job runs `python3 scripts/api_removals.py check --baseline v0.40.0`, against the S3 start tag, the S2 release. Its sibling [`docs/s3-migration.md`](docs/s3-migration.md) holds S3's consumer-impact notes, which the first S3 version-bump commit quotes in CHANGELOG.md. It runs cargo-semver-checks with `--release-type patch` and fails on any reported removal that the listing for the workspace's stage does not hold, printing each one's key to copy there: `check` and `release` read the listing `LISTINGS` names (`docs/s1-removals.md` in S1, `docs/s3-removals.md` in S3), and `check` refuses stage S2. A key is `<crate> <lint id>: <item> (<file>)`, and a deleted crate is `<crate> crate_missing`. cargo-semver-checks cannot see `#[doc(hidden)]` items or type aliases, so the job also imports each path the listing names under "Doc-hidden paths consumers import", in one scratch crate per crate and feature set, so no entry compiles through another's features; a story that removes one of those paths marks it `**Removed**` in the same commit. [`docs/s1-removals.md`](docs/s1-removals.md) is final, and so are S2's [`docs/rename-manifest.md`](docs/rename-manifest.md), whose rows map every path, feature and CLI command that moved or went from `v0.39.0`, and [`docs/rename-map.tsv`](docs/rename-map.tsv), the map `api_removals.py manifest --expand` wrote at the S2 release; no job runs `manifest` any more. The switch commit after the S2 release deleted `tombstones/`, set the stage to S3, rebased the removals and test-bodies jobs on `v0.40.0` (CI's one `S3_BASELINE`), and emptied `ci/test-body-diff.toml`'s S2 entries. cargo-semver-checks 0.51.0 and Rust 1.99.0 are pinned together in `ci.yml`, `release.yml` and `flake.nix`, because the tool reads only the rustdoc JSON some toolchains emit; upgrade both at once. `release.yml`'s `semver` job runs the same tool against the newest release tag behind the commit, before anything publishes, and lets it infer the release type from the two versions. On a 0.x patch bump it fails on any major-level change, not only a removal: a new required field, a changed signature, an enum made non-exhaustive. It also runs the compile test, over the stage's listing's paths (`docs/s3-removals.md`'s in S3; the manifest's in S2). When it fails, raise the 0.x minor in a new version-bump commit on `main` (see Publishing Order); the release that failed was never tagged, so the next green push releases the new version.

**A moved test keeps its body (AD-12).** The test-bodies job runs `python3 scripts/test_body_diff.py --baseline v0.40.0`. It reads every test-context item at the S3 start tag, the S2 release, and at the commit: fns, consts, statics, structs, enums, unions, type aliases, `extern` items, `macro_rules!` and item-level macro invocations, and the methods, consts and types of `impl` and `trait` blocks, in `tests/**`, `[[test]]` and `[[bench]]` targets, `[[example]]`s that set `test = true`, the files a test-context `mod` declares, modules and files whose `cfg` names `test` outside `not(...)` (`#[cfg(test)]`, `cfg(any(test, …))`, `#![cfg(test)]`), and anywhere under a test attribute. Each item is compared with what gates it: the attributes of the `mod x;` that declares its file, the file's inner `cfg`/`cfg_attr`, and the attributes of each inline module around it in its file, so a test switched off by one of those gates shows. A gate on an ancestor file module is not read: `lib.rs`'s `#[cfg(feature = "gamma")] pub mod gamma;` declares only `src/gamma/mod.rs`, so it gates the inline tests of the files under `src/gamma/` unseen. Each `[[test]]` and `[[example]]` stanza is an item too (every field but `name` and `path`, `required-features` mapped through the manifest's `## Features` rows), and so is each non-Markdown file under a `tests/fixtures/`, by its blob. It pairs each item by file (`git diff -M30%`, plus `[files]` in `ci/test-body-diff.toml`) and then by a unique module path, kind and name, and compares attributes (doc comments aside), signature and body after normalising both: comments and `use` statements dropped, a library's `crate::` as its name, the manifest's path rows and then the toml's `[paths]` (for a move into code that is not public, which no manifest row names) applied to the old side, a bare identifier renamed only at the head of a path, `include_str!`/`include_bytes!` targets as blob ids, the `[fixtures]` table on the old side (a baseline `fixtures!()` read as `fixtures!("<subdir>")` and `fixtures!("x")` as `fixtures!("<subdir>/x")` under a scope, for a crate whose fixtures moved under `tests/fixtures/<module>/`, applied before `[literals]`), the `[literals]` table on the old side, the `[gates]` table on the new side (under a path prefix, each feature's `cfg(feature = "<m>")`, or each whole cfg predicate such as `any(feature = "clob", feature = "relay")`, dropped from the gates, where it is a `cfg`'s whole predicate or an operand of its top-level `all(...)`, and each feature from `required-features`: for code that moved behind a module's features, such as a crate root that becomes `src/<m>/mod.rs`, or core's files in `shared`), whitespace collapsed, and rustfmt's re-wrapping under longer paths ignored (a trailing comma before a closing bracket or a generic list's `>`, except in a one-element group that is not a call's, and braces around a lone expression after `=>` or a closure's `|…|`). It fails on a changed item unless a `[[drift]]` entry names a DRIFT row of `divergences.md` and the diff's sha256, on a lost item unless a `[[retired]]` entry names it and an old path whose covering manifest row gives a reason, and on any `[files]`, `[fixtures]`, `[literals]`, `[paths]` or `[gates]` entry that pairs or rewrites nothing. It holds no entries: S2's, which paired the renames against `v0.39.0`, went with the switch to S3. The manifest's rows still normalise the old side; their baseline is `v0.39.0`, so the script warns that they map paths from another tag, which is harmless at `v0.40.0`. git pairs only tracked files, so stage a moved file before running it locally. `--moves` prints each moved item's old `file:line name` and new `file:line`, for `docs/MUTANTS.md`. README and doc-comment doctests, and Python tests, are not compared.

**Clippy and tests passing is not enough.** The lint & test job ends with `cargo doc` under `RUSTDOCFLAGS: -D warnings`, which makes `rustdoc::private_intra_doc_links` an error: a doc comment on a `pub` item may not use ``[`link`]`` syntax to reference a `pub(crate)` item. Doctests do not catch this — they run the code in doc comments and say nothing about whether the prose links resolve. Either make the referenced item `pub` or state the fact inline.

A red doc build costs more than it looks: `release.yml` triggers on `workflow_run: [CI], conclusion == 'success'`, so a failed doc build, or any other red CI job (msrv, features, removals and test bodies included), **silently withholds the release tag**. The version bump lands on `main` and nothing publishes, with no obvious connection between the two symptoms. Fixing the build forward is enough: the next green push to `main` releases any version that is still untagged (see Publishing Order).

```bash
# Run live integration tests (hit real APIs, skipped in CI)
cargo test -p polyoxide-polymarket --test live_clob -- --ignored
```

## Workspace Architecture

Text between `<!-- generated:begin <id> -->` and `<!-- generated:end <id> -->` markers, here and in README.md, docs/specs/INDEX.md, docs/ARCHITECTURE.md, SELF-HEALING.md and the two nightly workflows, is written by `scripts/gen_registry.py` from each crate's `[package.metadata.polyoxide]`, the root manifest's `[workspace.metadata.polyoxide]` (`stage` and `mirrors`) and `cargo metadata`, so never edit it by hand: change the metadata, run `python3 scripts/gen_registry.py --write`, and expect CI's scripts job to fail on any region that differs from what the generator produces.

<!-- generated:begin claude-crate-count -->
Nine crates, in publish order, each with the workspace crates its build needs. Crates that are not published come last:
<!-- generated:end claude-crate-count -->

<!-- generated:begin claude-graph -->
- `polyoxide-venue` — Shared vocabulary: error classes, the status and close-code maps, the socket classification table (`impl_ws_classification!`), the `Retry-After` parser, the wire-enum macros and `UnknownVariant`, positional decimal serde (feature `decimal`), `UnixMillis`, market keys (feature `keys`: `MarketKey`, `RawKey` and the `KeySpace` a venue implements per product), the market-data traits with the records they return and their typed `Extensions` (feature `market`), and the `Trading` trait with its order, fill, position and balance records and its `TradingEvents` stream (feature `trading`); needs: nothing in the workspace; every public error type in the workspace implements its `Classify` trait but `polyoxide-ws`'s `WsError`, which `impl_ws_classification!` classifies in each venue crate that wraps it; by default it depends on nothing, and it has no HTTP, socket or signing dependency
- `polyoxide-core` — Core utilities and shared types; needs: `polyoxide-venue`; the send loop and its hooks, the HTTP client, window quotas, capacity buckets, holds, errors, macros and the keychain; it names no venue, which `scripts/venue_gate.py` holds
- `polyoxide-ws` — The socket kit: the TLS install, `Backoff`, `Timing` and its setters, the deadline arithmetic, `WsError`, connect with a timeout, the bare tier, request/answer correlation, the generic `Supervisor` for a venue `Protocol`, and the scripted test server (feature `test-server`); needs: nothing in the workspace; depends on neither `polyoxide-core` nor `polyoxide-venue`, and builds only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls
- `polyoxide-binance` — Client library for Binance's APIs, one feature per module: `usdm` (USDⓈ-M futures market data, with its streams under `usdm-ws`); re-exported by the umbrella as `polyoxide::binance`; needs: `polyoxide-core` (under `usdm`, on by default), `polyoxide-venue`, `polyoxide-ws` (under `usdm-ws`)
- `polyoxide-polymarket` — Client library for Polymarket's APIs, one feature per module: `clob` (order book, with its sockets under `clob-ws`), `gamma` (market data), `data` (user data, v1 and v2), `relay` (gasless transactions), `perps` (perpetual futures market data, with its sockets under `perps-ws`), `rtds` (crypto price streams) and `sports` (live sports scores), with the code the HTTP modules share in `shared` and, with `clob`, `gamma` and `data`, the unified `Polymarket` client; re-exported by the umbrella as `polyoxide::polymarket`; needs: `polyoxide-core` (under `clob`, `gamma`, `data`, `relay`, `perps`, on by default), `polyoxide-venue`, `polyoxide-ws` (under `clob-ws`, `perps-ws`, `rtds`, `sports`); only `clob` and `relay` build `alloy`, and only the HTTP modules build core, which `test_dep_allowlists.py` holds per feature; `clob` does not turn on `gamma`, and with both the CLOB client looks a proxy wallet up through Gamma; `rtds` and `sports` build no `polyoxide-core`, `reqwest` or `alloy`, so a credential-free feed builds no HTTP or signing stack; `Trading` for the CLOB (`clob::venue`) is built with `clob-ws` and `data` together: its events stream runs on the supervised user socket, which is `clob-ws`'s, and its positions read the Data API; perps' auth and trading pending
- `polyoxide` — The multi-venue umbrella: `polyoxide::polymarket` and `polyoxide::binance` behind features named for each venue and module, `polyoxide::venue` with the market-data traits, `polyoxide::parse_key` and a `prelude`; needs: `polyoxide-binance` (without default features, under `binance-usdm`), `polyoxide-polymarket` (without default features, under `polymarket-clob`, `polymarket-gamma`, `polymarket-data`, `polymarket-relay`, `polymarket-perps`, `polymarket-rtds`, `polymarket-sports`, on by default), `polyoxide-venue`
- `polyoxide-cli` — CLI tool for querying Polymarket APIs and Binance USDⓈ-M market data; needs: `polyoxide-binance` (with `usdm-ws`, without default features), `polyoxide-core` (with `keychain`, under `keychain`), `polyoxide-polymarket` (with `clob-ws`, `gamma`, `data`, `rtds`, `sports`, without default features), `polyoxide-venue` (under `keychain`); its `keychain` also turns on `polyoxide-polymarket`'s `relay` and `keychain`, for the relay's keychain service
- `polyoxide-py` — Python bindings via PyO3 (`publish = false`, wheels on PyPI); needs: `polyoxide-polymarket` (with `clob`, `gamma`, `data`, without default features), `polyoxide-venue`
- `polyoxide-test-support` — Test toolkit: the failure tags the nightly classifier reads, the credential loaders, and the agreement, fixture and soak helpers (`publish = false`); needs: `polyoxide-core` (with `keychain`), `polyoxide-venue`; a path-only dev-dependency of the crates whose tests use it; it names no venue
<!-- generated:end claude-graph -->

<!-- generated:begin claude-cli-deps -->
Note: `polyoxide-cli` does **not** depend on the `polyoxide` umbrella. It depends directly on the component crates — `polyoxide-binance` (with `usdm-ws`, without default features) and `polyoxide-polymarket` (with `clob-ws`, `gamma`, `data`, `rtds`, `sports`, without default features) — plus `polyoxide-core` (with `keychain`) and `polyoxide-venue` only under the optional `keychain` feature.
<!-- generated:end claude-cli-deps -->

The CLI's commands are `polyoxide <venue> <module> <verb…>` (Story 5.8): `polyoxide
polymarket` holds `gamma`, `data`, `clob`, `rtds`, `sports` and, with `keychain`,
`credentials`, and `polyoxide binance` holds `usdm`; `completions` stays at the top. The
library follows the tree, `polyoxide_cli::commands::{polymarket, binance, completions}`, beside
the shared `stream` and `credentials` modules, and the root of `commands` re-exports nothing.
v0.39.0's transport-first groups (`ws`, and `gamma`, `data`, `clob` and `credentials` at the
top) are gone with no aliases. `docs/rename-manifest.md`'s hand-written `## Commands` maps each
old command to its new one, and `src/main.rs`'s `every_new_command_in_the_manifest_parses`
fails unless each new command there parses as a subcommand path and each old one is gone.

**Every socket is its module's `stream` verb, on one runner.** `polymarket clob stream market`
and `… user`, `polymarket rtds stream`, `polymarket sports stream` and `binance usdm stream`
each describe their socket as a `commands::stream::Feed` (what each event and each error
means), and `stream::run` owns the rest: `-n`, counted in printed events and checked before
each wait, so a feed that goes quiet after the last wanted event still ends; `-t`, where a
timeout too large to add means none; "The feed ended"; a `BrokenPipe` on stdout ending the run
quietly; and `excerpt()`, a frame cut to one escaped stderr line. Every stream's `--format` is
the one `stream::OutputFormat`: `pretty`, one human-readable line per event (market's and
user's old `summary`), or `json`, one compact object per line. Events go to stdout, and markers,
filters and skipped frames to stderr. A frame a feed cannot read is skipped with a marker;
any other stream error ends the run with that error (market, user and prices printed `Error:`
and exited 0 before). No stream installs a Ctrl+C handler, and the CLI does not depend on
`ctrlc`. Each command's `run_with(args, events, out, err)` takes any event stream, so
`tests/ws_sports.rs`, `tests/ws_binance.rs` and `tests/stream_polymarket.rs` (market, user and
prices) drive every flag with scripted events, and `stream`'s own tests pin the runner's four
behaviours. The dataset writer's `--format csv|jsonl|parquet` is a different `OutputFormat`,
in `polymarket::clob::prices::types`.

`polymarket sports stream` takes comma-separated `--league` and `--game` filters and
`--changes-only`, and is driven with the sports module's captured frames.

`binance usdm stream` streams Binance USDⓈ-M market data through the supervised tier of
`polyoxide-binance`'s `usdm` module. `--symbol` and `--kind` are comma-separated, and each kind is streamed for each
symbol; `--all-tickers` and `--all-mark-prices` add the two array streams. Outage markers
go to stderr, and `--format json` prints each frame's envelope. `polyoxide-cli/tests/ws_binance.rs`
drives it with the crate's captured frames (`polyoxide_binance::usdm::ws::fixtures`, features
`usdm-ws` and `test-server`).

The CLI installs a `tracing` subscriber that writes to stderr at `warn`, and `RUST_LOG`
overrides the level. That is how the libraries' recoveries become visible: a reconnect being
retried, or a 429 being waited out. Before it, they were discarded, and a feed retrying a dead
host looked like a quiet one.

The CLI's `polymarket clob` group holds the market and user streams and `prices download`,
a bulk, resumable, rate-limited downloader for CLOB historical price data
(`GET /prices-history`) that writes per-market CSV/JSONL/Parquet dataset files
plus a `manifest.jsonl`. Parquet output requires building the CLI with the
`parquet` feature.

The CLI's `polymarket data` command group reads Data API v2, except `data health`, which stays on
v1's `/` because `/v2/status` reports data freshness, not liveness. Listing commands print
the v2 `{data, pagination}` envelope and page with `--cursor`, `--all` (JSONL, flushed per
page) and `--max-pages`; the cursor to resume from goes to stderr when a walk stops early.
`--offset` is refused with a pointer to `--cursor`. `data traded` prints the
`/v2/user-stats` object, or `null` for an unknown wallet. `DataCommand::run_with` takes the
client and the output writers, and `polyoxide-cli/tests/data_v2.rs` uses it to run real
arguments against mock servers serving the `data` module's v2 fixtures
(`polyoxide-polymarket/tests/fixtures/data/v2/`).

**Every list argument splits on commas.** A clap `Vec` field needs `value_delimiter`, not a
value parser that returns a `Vec`: the latter compiles and parses, then panics when the field
is read. Every v1 `data` list flag shipped that way, and no test caught it because the parse
tests never passed those flags. `src/main.rs`'s `every_list_argument_splits_on_commas` walks
the clap tree and fails on any `Vec` argument without `value_delimiter = ','`, positionals and
`--filter`s included; its one exception is the hidden `_ignored` of `data positions closed`,
which swallows what follows the removed command. Each list argument also has a parse test
that passes a comma list.

**`polyoxide polymarket credentials <store|show|delete> <kind>`** (feature `keychain`, Story
5.9) is `commands::credentials::Credentials` over Polymarket's kinds, `clob` and `builder`
(v0.39.0's `relay`), which `commands::polymarket::credentials` lists and nothing more. A kind is
a `polyoxide_core::keychain::StoredCredential` its library defines: `clob::Account` for the
`polyoxide-clob` service and `relay::BuilderAccount` for `polyoxide-relay`, with v0.39.0's field
names, so stored values load as before. The trait gives the service, the `FIELDS`, each a
`Field { name, secret }`, and a `validate`; core's generic `store`, `show` and `delete_all` carry
values as `polyoxide_venue::Secret` (`delete` was taken by the per-key function). The CLI's
shared module generates a flag per field (`api_passphrase` is `--api-passphrase`), redacts, and
calls the keychain. `store` writes only the fields given, and only after every check passes:
core's (a name that is not a field, one given twice, an empty or blank value) and the kind's
(a private key that parses; the relayer API key and its address given together, the address
parsing). A refusal is `KeychainError::Invalid { service, field, reason }`, classed
`InvalidRequest`, whose reason never quotes the value. `show` prints each field as `present`,
`not found` or `error: …`, and the one plain field, the relayer API key's address, as its value
when present; it never prints a secret. The public credential types (`Credentials`,
`BuilderConfig`, `ApiCredentials`) keep their `String` fields; `Secret` on them is S3's.

<!-- generated:begin claude-umbrella-features -->
**polyoxide** (the umbrella) uses feature flags: `polymarket`, `polymarket-clob`, `polymarket-clob-ws` (`polyoxide-polymarket/clob-ws`, `polyoxide-venue/trading`), `polymarket-gamma`, `polymarket-data`, `polymarket-relay`, `polymarket-perps`, `polymarket-perps-ws` (`polyoxide-polymarket/perps-ws`), `polymarket-rtds`, `polymarket-sports`, `binance`, `binance-usdm`, `binance-usdm-ws` (`polyoxide-binance/usdm-ws`), `full` (all), `keychain` (`polyoxide-polymarket?/keychain`). Default = polymarket.
<!-- generated:end claude-umbrella-features -->
The umbrella's crate root holds `venue` (`polyoxide-venue`, always on), `polymarket`
(`polyoxide-polymarket`, under any `polymarket*` feature), `binance` (`polyoxide-binance`, under
any `binance*` feature), `parse_key` (under the features that build a product's keys) and
`prelude`, and nothing else. The features `polymarket` and `binance`
turn on their venue crate's defaults; each `<venue>-<module>` passes its module's feature through,
a `-ws` one turning its module's on, relay's included; `keychain` stays a cross-venue extra, each venue adding its own
`?/keychain`; there is no umbrella `specta`. A module that implements the market-data traits
(`polymarket-clob`, `polymarket-perps`, `binance-usdm`) also turns on `polyoxide-venue/market`, and
one that only names keys (`polymarket-gamma`, `polymarket-data`) `polyoxide-venue/keys`, so
`polyoxide::venue` carries the traits with every module that implements them, which a `const _`
pins. `examples/venue_neutral.rs` reads the best bid and ask of the CLOB, perps and USDⓈ-M with one
`best_bid_ask(&DynMarketData, &MarketKey)` and their errors with one classifier generic over
`Classify` (CAP-5 and CAP-4); `tests/venue_neutral.rs` includes it by `#[path]` and runs both against
mock servers for each module, and `tests/live_venue.rs` runs `best_bid_ask` against the live hosts.
Each passthrough names one type of its module through
the path it opens in a `const _` outside `cfg(test)`, so `cargo hack --each-feature` fails a
re-export whose `cfg` leaves a feature out. The prelude re-exports a selection of the enabled
modules' clients, errors and stream types under the umbrella's feature names, aliasing a stream's
`Event` by its venue (`PerpsEvent`, `SportsEvent`, `UsdmEvent`). One `const _` per prelude block
pins it to its feature, and one more pins the unified client to `polymarket`.

## Key Patterns

**Builder pattern** — All clients use builders: `ClobBuilder::new()`, `Clob::builder(private_key, credentials)`, `Gamma::builder()`, `DataApi::builder()`, `Perps::builder()`, `Usdm::builder()`, `RelayClient::default_builder()`, `Polymarket::builder(account)`. Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry config, concurrency, and the client's default concurrency: relay 2; gamma, data, perps and Binance 4; clob 8) and gets its five transport setters, `base_url`, `timeout_ms`, `pool_size`, `with_retry_config` and `max_concurrent`, from `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()` and adds the throttle and retry policy the venue installs; the config sets no `gzip`. A venue's own knobs stay on its builder: data's `pnl_base_url` and `rankings_base_url`, perps' `with_rate_limiter`, clob's `chain`, `signature_type`, `builder_code`, `gamma` (built only with the `gamma` feature, which `clob` does not imply), `data` (built only with the `data` feature, for the `Trades` the CLOB reads through the Data API; `build` makes one with the builder's timeout and pool size when none is given, and the unified `Polymarket` client hands clob its own), `user_socket` (built only with the `clob-ws` feature: the URL and timing of the supervised user socket each `Trading::events` stream opens) and `with_account`, relay's `url`, `chain_id`, account, auth and wallet setters, and Binance's `weight_budget`. Relay keeps `url(&str) -> Result`, which checks the URL at once, beside `base_url`, and its `build` adds the trailing slash either way, so a path prefix is kept. The unified client, `polyoxide_polymarket::Polymarket` (built with `clob`, `gamma` and `data`; Story 5.7 moved it out of the umbrella, which re-exports it as `polyoxide::polymarket::Polymarket` and from its prelude), builds one CLOB, Gamma and Data client from one `Account`, and the CLOB client reads through that same Gamma and Data client; `Polymarket::new` and `PolymarketBuilder::build` return a `BuildError` whose `Clob`, `Gamma` and `Data` variants wrap the error of the client that could not be built and classify as it does. `PolymarketError` is gone.

**API namespaces** — Clients organize endpoints into namespaces:
- CLOB: `clob.markets()`, `clob.orders()`, `clob.account_api()`, `clob.health()`, `clob.auth()`, `clob.rewards()`, `clob.public_rewards()`, `clob.notifications()`
- Gamma: `gamma.markets()`, `gamma.events()`, `gamma.series()`, `gamma.tags()`, `gamma.comments()`, `gamma.sports()`, `gamma.search()`, `gamma.user()`, `gamma.health()`
- Data: `data.user(addr)`, `data.trades()`, `data.holders()`, `data.leaderboard()`, `data.builders()`, `data.live_volume()`, `data.open_interest()`, `data.market_positions()`, `data.combos()`, `data.misc()`, `data.pnl()`, `data.rankings()`, `data.accounting()`, `data.health()`, `data.v2()`. `data.approvals()` is deprecated: upstream removed `/v1/approvals` and the host now returns `404`; `data.v2().approvals()` serves the same data.
- Perps: `perps.health()`, `perps.exchange()`, `perps.market()`, `perps.public()`; Binance: `usdm.health()`, `usdm.exchange()`, `usdm.market()`.

An accessor that only clones client fields into its namespace comes from `polyoxide_core::namespaces!`, invoked in the client's `impl` with the fields every namespace takes (`http_client;`, Binance's `http;`); `pnl: PnlApi { http_client: pnl_http_client }` maps one field from another, and attributes such as `approvals`' `#[deprecated]` pass through. The namespace structs stay declared by hand, since their paths are public. Eight accessors are written by hand because they do more than clone: data's `user(addr)`, its alias `positions` and the `traded` wrapper, and clob's five account-gated `orders`, `account_api`, `notifications`, `rewards` and `auth`, each projecting its own part of the account and failing with its own message.

`data.pnl()` and `data.rankings()` target sibling hosts (`user-pnl-api` and `lb-api`) that have **no published spec** — see `docs/specs/undocumented/INDEX.md`. Polymarket retired both on 2026-10-16: each answers `410 Gone`, so both accessors are deprecated (DRIFT R12) and their live tests hold the `410`. Their base URLs are configurable via `DataApiBuilder::pnl_base_url` / `rankings_base_url`, and all three hosts share one connection pool and concurrency budget via `HttpClient::with_base_url`.

`clob.rewards()` requires an `Account`; `clob.public_rewards()` exposes the
subset that is public upstream (`/rewards/markets/current`,
`/rewards/markets/{condition_id}`, `/rewards/markets/multi`,
`/rebates/current`) without one.

Example: `gamma.markets().list().open(true).send().await?`, `data.leaderboard().get().send().await?`.

**Request builder fluency** — Query parameters are chained with builder methods before `.send().await?`. The setters come from `polyoxide_core::query_setters!`, one entry per setter keeping its doc comment, name, argument type and key: `name: T => "key"`, `name: impl Into<String>`, `name: many T` (repeats the key), `name: csv T` and `name: csv<I, S>` (comma-joined through `polyoxide_core::csv`, omitted when the joined value is empty), `name(arg: T) => "key" = expr`, `name(arg: T) => "key" if cond` and `name(arg: T) => csv "key" = expr`. Each writes `self.request` (core's `Request`), or the field named by a leading `self.<field>;` (data v2's paged builders use `self.inner;`); a field with an inherent `query`, such as Binance's private `Routed<T>`, which replaces a repeated key, keeps it. Seven setters stay hand-written because they are not single query writes: Binance's `GetKlines::limit` and `GetDepth::limit`, which also re-price the route, and gamma's `GetManyMarkets::include_tag` and the `limit` and `offset` of `QueryByInformation` and `QueryAbridged`, which store fields `send` writes. `tests/query_setters_usdm.rs` in Binance, and `tests/query_setters_clob.rs`, `tests/query_setters_gamma.rs` and `tests/query_setters_data.rs` in `polyoxide-polymarket`, call every setter with a typed value and pin the exact ordered pairs it sends.

**Two auth layers, three signing schemes** — managed through the `Account` type in `polyoxide-polymarket/src/clob/account/`. Don't conflate them; they use different EIP-712 domains and are verified by different parties:

| Scheme | Used for | Shape |
|--------|----------|-------|
| **L1** | Creating/deriving API credentials (`/auth/api-key`, `/auth/derive-api-key`) | EIP-712 `ClobAuth`, domain `ClobAuthDomain` v1, **no `verifyingContract`** |
| **L2** | Everything else authenticated — orders, balances, trades | HMAC-SHA256 over `timestamp + method + path [+ body]`, url-safe base64 |
| **Order signing** | The signed order payload itself, posted under L2 | EIP-712 `Order`, domain `Polymarket CTF Exchange` v2, **with** `verifyingContract` |

The two EIP-712 domains are unrelated — order signing needs a verifying contract, L1 auth must not have one. See `docs/specs/clob/auth.md` for both type strings; `polyoxide-polymarket/src/clob/core/eip712.rs` pins them against golden vectors from `py-clob-client`.

A fourth scheme, **order signature type 3**, is a Deposit Wallet signing an ERC-7739
`TypedDataSign` envelope: the exchange domain is the signing domain and the wallet's
`DepositWallet`/`1` domain rides inside the message. Getting that orientation backwards
produces a different digest, so the signature cannot verify for the wallet.
`v1_exchange.envelope_digest` in `polyoxide-polymarket/tests/fixtures/clob/session_keys/order_vectors.json`
pins py-sdk's golden digest, and `polyoxide-polymarket/src/clob/core/eip712.rs` checks it. A session key adds a
6492 envelope on top. The role (owner or session key) is never inferred from
addresses; `SigningTarget::DepositWallet { wallet, role }` carries it. The published
OpenAPI omits almost all of it (the relay mirror documents only `GET /deployed?type=WALLET`):
see `docs/specs/session-keys/`.

**Error hierarchy** — `ApiError` in core, wrapped by crate-specific errors (`ClobError`, `GammaError`, `DataError`, `RelayError`, Binance's `UsdmError`), each module's named `<Module>Error` (data's was `DataApiError` until Story 5.3, and Binance's `UsdmError` was `BinanceError` until Story 5.6), and a module's socket error `<Module>WsError` (clob's `ClobWsError` was `WebSocketError` until Story 5.4). An unsuccessful response is `ApiError::Response(Box<ErrorResponse>)`, whose `ErrorResponse` carries the status, headers, body, the body's `error` or `message` field as `message`, and the unclamped `Retry-After` (a zero is `None`); `Validation` is a refusal made before sending, and only that. Each module's `From<ApiError>` is its one decode (D14), so a `?` on core's error anywhere decodes the same way: gamma's and relay's are the derived `#[from]`, data's turns a v2 envelope into `V2`, perps' a `{status: "err", error, ref}` body into `Venue`, clob's a 400 kill into `FakUnmatched` or `FokUnfilled`, a refused signer cost into `BurstCapacityExceeded` (`polyoxide_polymarket::shared`, the signer layer's refusal) and a `Sign` back into its clob error, and Binance's goes by status first (`418`, `429`, `451`, `403`) and then its `{code, msg}` body. `RequestError` is a marker for any `From<ApiError> + Debug` type. Foreign errors enter a module only through `ApiError`, by `.map_err(ApiError::from)?` or core's own paths, so no module implements `From<reqwest::Error>`, `From<url::ParseError>` or `From<serde_json::Error>` (core's `impl_api_error_conversions!` is gone), and every module's error enum is `#[non_exhaustive]`. Since DRIFT R7, `RelayError::Api` wraps `ApiError`: a relayer failure is classed by its status, a transport, URL or JSON failure is `Api(Network | Url | Serialization)` (relay's own `Reqwest`, `UrlParse` and `SerdeJson` are gone), and a refusal made before sending (validation, missing auth, a header that could not be built) is `Api(Validation)` through `RelayError::validation`, as clob's is.

**Every error implements `polyoxide_venue::Classify`** — inner and auxiliary types included (`V2Error`, `VenueError`, `BurstCapacityExceeded`, `Refused`, `ParseTickSizeError`, `polyoxide-venue`'s `UnknownVariant`, `InvalidSymbol`, `InvalidStreamName`). `class()` is one of the eight `Class` variants of AD-15, decided by the status before the body; `is_fault()` is false only where the venue answered as designed (the FAK and FOK kills, and a 451 region block wherever a status decides the class); `retry_after()` is the wait the server asked for. The class decides retriability for callers: `Network`, `Unavailable` and `RateLimited` are retriable. `ApiError::Response` is classed by its status (any status outside 400–599 a `Decode`, such as a ping's 2xx whose body says it is not ok), with a `429`'s `Retry-After` as its wait and every status's `Retry-After` as `retry_after()`; `ApiError::Validation` and `Sign` are `InvalidRequest`. No error has an inherent `is_retriable` or `retry_after`, the socket errors included: data's v2 `retryable` flag is surfaced on `V2Error`, not obeyed, and Binance's `418`, `451` and firewall `403` are `Restricted` (D14), not retriable. A failure of clob's Gamma dependency is `ClobError::Gamma`, classed as the Gamma error, and a Data client clob could not build is `ClobError::Data`, classed as the Data error, and a reqwest error that broke off mid-body is `Network` though reqwest labels it a decode error. Each crate's `lib.rs` lists its error types in a `const _` assertion, gated like the types (`polyoxide-venue`'s names the trait `crate::Classify`), so a missing impl fails the build, and `.github/scripts/tests/test_classify_coverage.py` fails when a public error type is missing from that list, or when a type with a `WsError` field sits in a file that does not expand the socket table's macro, or that file does not call the three functions the macro defines. Besides venue's `ClassifiedError`, which must not implement the trait, the sweep exempts `polyoxide-ws`'s `WsError`: venue does not depend on ws (AD-2) and ws does not depend on venue, so neither can implement the trait for it, and in a venue crate both the trait and the type are foreign (the orphan rule), so each venue error that wraps it classifies it through `polyoxide_venue::impl_ws_classification!`, a macro the venue crate expands. `polyoxide-venue` also holds the status and close-code maps and the one `Retry-After` parser, `parse_retry_after`, which every `Retry-After` read goes through with its caller's clamp: core's `max_backoff_ms` for the wait the loop sleeps, none for core's `ErrorResponse::retry_after`, the wait an error surfaces, which perps' `VenueError` and data's `V2Error` copy, and Binance's 3 days for every arm of its decode, a core `Response` it keeps included. A zero is no wait anywhere (DRIFT R4). No crate matches over `tungstenite::Error`: every socket error is on the one socket table, `polyoxide_venue::impl_ws_classification!()` (Story 4.3), perps (Story 4.6), Binance (Story 4.7), rtds (Story 4.8), clob (Story 4.9) and sports (Story 4.10), `PerpsWsError::Connection`, `UsdmWsError::Connect`, `RtdsError::Connection`, `ClobWsError::Connection` and `SportsError::{Connect, Transport}` wrapping `polyoxide_ws::WsError` (Binance and sports map the kit's server close and connect timeout to their own `Closed` and `ConnectTimeout`, and Binance keeps its handshake `403` → `Restricted` row ahead of the table, D14; clob maps a timed-out connect step to its own `ConnectTimeout { url, timeout }`, and a host that resolves to no address is an I/O failure, `Network`), so a 451 refusal is each one's one transport non-fault. Each recovery retries a handshake refused with 408, 425, 429 or 5xx through `polyoxide_ws::retrying_can_fix` (DRIFT R1). The table: it expands, in the invoking module, to private `ws_class`, `ws_is_fault` and `ws_retry_after` over `polyoxide_ws::WsError`'s `Kind`, which a venue's `Classify` impl calls on its `WsError` arm. No lint reports one the venue never calls (rustc does not warn about dead code expanded from another crate's macro), so `test_classify_coverage.py` is the guard: a file that expands the macro must call all three in code, and a type with a `WsError` field must sit in a file that expands it, the file the fence checks. A refused upgrade follows `class_for_handshake_status` with its `Retry-After` read unclamped, a server close `class_for_close_code`, and only a 451 refusal is not a fault; every refusal's `Retry-After`, whatever its status, is the error's `retry_after()`, as HTTP's `ErrorResponse` surfaces it, though only a 429's class carries it (Story 4.10); `polyoxide-venue/tests/ws_classification.rs` covers every row, and that `polyoxide_ws::retrying_can_fix`, given venue's status rule, agrees with the class on every refused status.

**One send loop** — `HttpClient::send` (`polyoxide-core/src/send.rs`) is core's only retry loop, and `Request::send_raw` and `get_bytes` run on it. Core and its loop are venue-neutral: a venue's retry policy, rate limit tables, throttle and signing live in that venue's crate, and Polymarket's in `polyoxide_polymarket::shared` (Story 5.2), each item built by the features of the modules that use it: `PolymarketRetryPolicy` by any of `clob`, `gamma`, `data`, `relay` and `perps`, each `*_limits` table by its own module's, `ClobThrottle`, `signer_cost` and the signer layer by `clob`, and the HMAC `Signer` and the session-signer types by `clob` or `relay`. Clob, gamma, data, relay and perps are `polyoxide-polymarket`'s own modules (Stories 5.3 to 5.5), and so are its socket modules rtds and sports, which build no core. Each attempt takes a concurrency permit, then runs the client's hooks (`polyoxide-core/src/hooks.rs`): `Throttle::acquire` charges it (a `RateLimiter` counts one request, found by method and path; a `Cost` charges any other layer), an optional `Authenticator` signs it, it is sent, `Throttle::observe` sees the response, and `RetryPolicy::decide` returns `Done`, `Retry(wait)` or `Fail` with an optional hold, which is applied whatever the outcome. A retry releases the permit and sleeps the longer of the policy's wait and the loop's own floor, `RetryConfig::retry_delay(attempt)`, and never goes past `max_retries`. A transport error is neither observed nor retried, and a refused cost returns `ApiError::Refused` with nothing sent. `Done` returns the response whatever its status; `Fail`, or a retry with none left, returns `ApiError::Response` (AD-8), after the hold and its WARN, reading the body after the attempt's permit is released; `ErrorResponse`'s `Debug` names the headers without their values and truncates the message and body, since the `Request failed` ERROR line prints it. `Request::send_raw`, `get_bytes` and `health` turn a non-2xx `Done`, which only a custom policy produces, into the same error, so a failed response never reaches a caller as data. `HttpClient` holds one `Arc<DynThrottle<'static>>` and one `Arc<DynRetryPolicy<'static>>` (dynosaur), shared with its `with_base_url` siblings, so a 429 on one host holds them all. Core's `Request<T, E>` is the one request builder: clob's namespaces return `Request<T, ClobError>`, with a method, a JSON body serialised once through `serde_json::Value`, an `Authenticator` (clob's L1, L1-signed and L2 are in the private `polyoxide-polymarket/src/clob/authenticator.rs`) and the `Cost`s a route charges (clob's trading requests carry `shared::signer_cost`). Clob's client installs `shared::ClobThrottle` and `PolymarketRetryPolicy`, and keeps a handle on the throttle's `SignerLimiter` for `tier()` and `rate_limit_status()`. Gamma's `post_json` decodes its own body, so it calls `HttpClient::send` directly (DRIFT R8). Every venue's `ping` calls `HttpClient::health(path, costs)` (`polyoxide-core/src/health.rs`), one `GET` on the send loop that returns a `Pong { round_trip, response }`: `round_trip` is the attempt that answered, timed from its signing by a core-private authenticator, so every ping's latency leaves out the permit, throttle and backoff waits, and a final non-2xx is `ApiError::Response` as the caller's error. Each ping keeps its path (gamma `/status`; data, clob and relay their base URL's path, so relay keeps a prefix; perps `/v1/info/ping`; Binance `/fapi/v1/ping`, costed from `Route::Ping`) and its body check (perps `status == "ok"`, Binance `{}`). A signing failure is `ApiError::Sign`, which `ClobError`'s hand-written `From<ApiError>` takes back out, as it turns a signer-layer `ApiError::Refused` into `BurstCapacityExceeded`. Relay calls `HttpClient::send` through one private helper, which reads no body itself: a failure is the loop's `ApiError::Response` as `RelayError::Api`, and a POST's is also logged at ERROR (its ping goes through `HttpClient::health`), signing with `RelayGetAuth` (the route's own path) or `RelayPostAuth` (the joined path and body, then the caller's extra headers, then `Content-Type`); its auth checks and allow-lists run before the send, so a refused call spends no permit and no token, and the two session-signer posts bound each attempt with `RequestParts::timeout` (300 s). Relay's alloy gas estimation goes to the chain's RPC node and is the one request outside the loop. Binance's route builders send core's `Request` over its `WeightBudget` throttle and `UsdmRetryPolicy`. No crate but core sends through `HttpClient`'s reqwest client, and none but core declares reqwest: the others name its types through `polyoxide_core::reqwest`. `HttpClient::should_retry`, `note_rate_limited` and `acquire_rate_limit` are removed (`docs/s1-removals.md`); `acquire_concurrency` stays public, because tests in core, gamma, data, perps, clob, relay and Binance observe the permit through it. The send loop's own suite, `polyoxide-polymarket/tests/send_loop.rs`, runs on Polymarket's policy, so it moved with it. Core's `Request::send`, which clob's and Binance's requests go through, and relay's `post_json` log a decode failure as one ERROR line through `polyoxide_core::decode_json`.

**Retriability** — `Classify::is_retriable()` (import `polyoxide_venue::Classify`; no crate re-exports it) is true for the `Network`, `Unavailable` and `RateLimited` classes: connection failures and timeouts, `408`, `425 Too Early`, `429` and 5xx. The crates' *own* retry loop is narrower — the send loop retries only what the client's policy decides. Core's `DefaultRetryPolicy` retries `429`; `polyoxide_polymarket::shared::PolymarketRetryPolicy`, which gamma, data, perps, clob and relay install, retries `429` and `425`; no policy retries 5xx or 408 (AD-17). A client built straight on `HttpClientBuilder` gets the default policy, so it does not retry a `425`. Binance installs its own `UsdmRetryPolicy`, which retries a `429` and never a `425`.

**Order kill outcomes are not faults** — Polymarket returns HTTP 400 for both genuine faults and the defined kill outcomes of marketable orders, so `ClobError` splits the latter out as `FakUnmatched` (FAK matched nothing) and `FokUnfilled` (FOK could not fill in full). They are deterministic and never retriable. Classification lives in `classify_order_kill` in `polyoxide-polymarket/src/clob/error.rs` and matches on the venue's message body — the only signal available, since the venue ships no error code. Upstream's error catalogue is [docs.polymarket.com/resources/error-codes](https://docs.polymarket.com/resources/error-codes); it is **not** in `docs/specs/clob/openapi.yaml`, which omits these rows entirely.

**Two rate limit layers, counting different things** — a request must satisfy both, and they are modelled in separate modules:

| Layer | Module | Keyed on | Counts | Applies to |
|-------|--------|----------|--------|------------|
| Cloudflare IP throttling | `polyoxide-core/src/rate_limit.rs` (`WindowQuotaTable`), tables in `polyoxide-polymarket/src/shared/limits.rs` | client IP | **requests** | every host |
| Per-signer token buckets | `polyoxide-polymarket/src/shared/signer_limit.rs` (feature `clob`) | signer address | **orders** | CLOB order/cancel only |

The per-signer layer charges batch endpoints their full size (`POST /orders` costs N, `DELETE /orders` costs N, `cancel-all` costs 1+N), so a batch can cost more than the bucket's burst capacity can *ever* hold — permanently rejected, not throttled. `ClobThrottle` refuses those client-side, before anything is sent, and clob's `From<ApiError>` turns the refusal into `ClobError::BurstCapacityExceeded` (non-retriable) rather than letting the retry loop burn attempts on a 429 it would misread as transient. Tier starts at `Standard` (tightest) and is adopted from the `Poly-RateLimit-Tier` response header, since it derives from 30-day volume the client cannot compute. `cancel-all`/`cancel-market-orders` costs are *not* knowable client-side — `TradingRequest::cost_is_exact` flags that. `shared::ClobThrottle` (`shared::clob_throttle()`, `polyoxide-polymarket/src/shared/throttle.rs`) composes the two layers over one `Hold`: each attempt is charged one request against `clob_limits()` by method and path, then the signer bucket its `shared::signer_cost` names, and every response's `Poly-RateLimit-Tier` is adopted, a 429's included. Clob's client installs it (Story 3.4); every trading route names its bucket with `shared::signer_cost`.

Both tables are pinned by agreement tests (the IP tables' `documented_*_limits` live in `polyoxide-polymarket/tests/polymarket_limits.rs` and use public API only) asserting the **effective quota a request resolves to** (`RateLimiter::effective_quota`, which lists the general bucket first, then the matched row's buckets), not merely that an entry exists. Tests that only check presence and ordering are how `/balance-allowance` went missing and `/closed-positions` sat at 66x its cap, both undetected.

**Published rate limit tables name routes that 404** — upstream lists `Health check (/ok)` under every surface, but only `clob.polymarket.com` serves it. Data's health route is `/`, Gamma's is `/status`. Probe the path on the host before pinning a row.

**The buckets model the published quota; a 429 is what the server actually said.** They disagree — Cloudflare's `error code: 1015` is an IP-scoped block with its own window and arrives as a 429 no matter how many tokens the buckets still hold. So a 429 holds the whole client: the policy's `Decision::hold` goes to `Throttle::hold`, which extends the throttle's `Hold` (`polyoxide-core/src/hold.rs`; for a `RateLimiter`, `begin_cooldown` does the same), and every subsequent `acquire` waits it out regardless of path, before charging and again after any bucket wait of its own (AD-23), so a hold set meanwhile is not sent into. Every layer of one throttle shares one `Hold`, and `Hold::with_ceiling` clamps it. The hold is `RetryConfig::retry_delay(0)`, whatever attempt saw the 429; the attempt's own wait is `retry_delay(attempt)`. A `425` retries without holding anyone else. Two rules make this work, both mutation-tested:

- **`Retry-After` may only extend the wait, never shorten it.** Cloudflare sends one that floors to zero; obeying it verbatim once made the retry delay `Duration::from_millis(0)`, so three retries landed inside 65ms and *extended* the very ban they were waiting on. The floor is the client's own exponential backoff, and `polyoxide_venue::retry_delay`, which `RetryConfig::retry_delay` calls, is the one place the rule lives.
- **Cooldowns extend, never truncate.** Concurrent requests see the same 429 milliseconds apart; taking the newest value would let the smallest delay release everyone early. `Hold::wait` re-checks after waking so a hold extended mid-wait is honoured in full.

The send loop applies a policy's hold **before** it decides whether to retry, and whatever it decides — a request that is out of attempts still has to publish what it learned.

**A bucket's depth and its refill rate are two spends of one budget.** `quota()` deliberately does not call `allow_burst`, leaving capacity at governor's default of one token, and `WindowQuotaTable` offers no burst setting, so no table can ask for one. The obvious spelling — capacity `count`, refilling at `count/period` — reads like a faithful transcription of "150 per 10 seconds" and is wrong: a bucket starting full admits its depth *plus* everything the refill adds, so its first window lets through `count + count`. Every entry in every table over-permitted by exactly 2x until this was measured.

Depth is not spare capacity; it is borrowed against the rate, and `burst + rate × period ≤ count` means any burst of `B` costs `B` requests of sustained allowance permanently. Minimum depth is therefore also maximum throughput — and the safest shape: a request that arrives during a hold waits before any bucket and is paced when it ends. A request already waiting on a bucket when a hold begins takes its token, then waits out the hold (AD-23) and is released with the others when it ends, so that burst is at most the requests in flight then (on the send loop, the concurrency permits). This is inherent to token buckets against a sliding-window server, not an artifact of this implementation: satisfying the bound with `rate = count/period` forces `B ≤ 0`.

**Contrast `shared/signer_limit.rs`, whose buckets must hold their published burst.** Each is a `CapacityBucket` (`polyoxide-core/src/capacity.rs`) whose capacity is the tier's burst, as governor's `allow_burst` was before Story 3.3. Polymarket publishes rate *and* burst for the per-signer layer and says burst is the bucket's capacity, so copying both is a faithful model of a bucket the server also implements as a bucket. A `CapacityBucket` refuses an exact cost above its capacity at once (`Refused`), charges an inexact one at capacity, and can be resized in place keeping its tokens and its `Hold`; a new tier still replaces both signer buckets at full capacity. Cloudflare publishes a *window quota* with no capacity term at all. Same two numbers, opposite meanings — making the two modules "consistent" would reintroduce the bug.

**The published count is reachable as a burst and not as a rate**, which is why `quota()` also reserves a tenth (`RESERVED_FRACTION`) rather than aiming at the published figure. Measured on `/closed-positions` (150/10s) against the live host:

| Sustained rate | Share of published | Result |
|---|---|---|
| 14.9/s | 100% | refused after 15.7s |
| 14.25/s | 95% | refused after 17.3s |
| 13.5/s | 90% | clean over 180s, 2,430 requests |

A one-shot 150 in 0.70s is accepted, so the table is not overstating the cap; Cloudflare's sliding-window estimator simply does not count the way a naive interval count does, and nothing outside the server can observe the difference. Aiming *at* a published quota is therefore a bug even when the arithmetic is right. Reproduce with `polyoxide-polymarket/examples/closed_positions_soak.rs` (`--rate` drives a chosen rate; omit it to exercise the shipped limiter).

Note that a 429 the client retries away is invisible to the caller — the retry loops log a `WARN` and return `Ok` — so a harness that counts `Ok` against `Err` reports a clean run straight through sustained throttling. Detection goes through a `tracing` subscriber instead (`polyoxide_test_support::soak::observe`).

**Decimal precision** — Price/size fields use `rust_decimal::Decimal` with `serde(with = "rust_decimal::serde::str")` for string serialization.

**Market keys** — `polyoxide_venue::MarketKey` (feature `keys`) names one instrument as
`venue.product:id`, and has no public constructor from text or from parts: only a product's
`KeySpace` makes one, after its `parse_id` has checked and canonicalized the id, so `Eq` is
canonical equality. Each venue crate implements `KeySpace` once per product, in one place:
Binance's `usdm::key` (`UsdmKeys`, over `Symbol`, which uppercases ASCII; `usdm::key(&symbol)`,
`key::parse`, `key::symbol`), and Polymarket's `shared::keys`, whose `ClobKeys` (over `TokenId`,
1 to 78 decimal digits with no leading zero) is built with any of `clob`, `gamma` and `data`, since
gamma and data name the clob's outcomes, and whose `PerpsKeys` (over `InstrumentId`, written
without leading zeros) is built with `perps`. A clob key names one outcome token, never a market.
`RawKey::parse` splits text at its first `:` and then the part before it at its first `.`, so an
id may hold either; `RawKey` round-trips through serde as text, `MarketKey` serializes as text
only, and `polyoxide::parse_key` hands a `RawKey` to the constructors of whichever enabled
product it names. Every `KeyError` is an `InvalidRequest`. Each venue crate's
`the_ids_are_the_ones_the_crate_registers` test holds its `VENUE` and product constants to its
`[package.metadata.polyoxide]`, read with `polyoxide_test_support::registration!()`.

**Market-data records** — `polyoxide-venue`'s `market` feature holds the records market data is
read into: `Instrument`, the one record that carries tick size, minimum size and `SizeUnit`
(`Base`, `Contracts` or `Shares`); `Level`, a price and a size, which is a value inside a record
rather than a record; and `Side { Buy, Sell }`. A size is the instrument's native quantity, a
quote-currency amount is a separate `notional`, and a field a venue does not publish is an
`Option`. Records derive `Clone`, `Debug` and `PartialEq`, are `#[non_exhaustive]` with a `new`
that takes their required fields, and carry `Extensions`: at most one value per type, each
`Clone + Debug + PartialEq + Send + Sync + 'static`, compared by value, with no serde, and
`type_ids()` to list them. Inserting a type already held is a `DuplicateExtension`
(`InvalidRequest`), and `Extensions::of(value)` starts one with a single value. A product's own
facts go in its one `<Record>Ext` per record type, never in a new field on the shared record.
`EventOutcome`, an `EventGroup`'s outcome, is a value inside a record like `Level`, and carries
no `Extensions`.

**Market-data traits** — the `market` feature also holds the traits a venue's products implement
so market data reads the same on every venue (AD-5, AD-6, amendment A6-1): `MarketData`
(`instrument(key)`, `quote(key)`, `book(key)`), which every product implements, and the
capability traits `Instruments` (the whole listing, only where one request or a few return it),
`Trades`, `Candles`, `Funding` and `EventGroups`, implemented only where offered; no method
answers "unsupported". Each has `Send + Sync` supertraits, methods returning
`impl Future<Output = Result<_, ClassifiedError>> + Send`, and a dynosaur wrapper
(`DynMarketData` and so on), held as `Arc<Dyn…<'static>>`; an implementation writes `async fn`
and converts its module's error with `?`, so the native clients keep their typed errors. A key of
another product is a `KeyError` (`InvalidRequest`), sent nowhere, and a key the venue does not
list is a `VenueRefusal` by its class (`NotListed` where the venue's answer identifies the
instrument as unlisted, otherwise the venue's own refusal), except where the venue cannot tell
(perps), whose quote and book are then empty; callers match on `class()` and confirm a listing with
`instrument`. The records they return are `Quote`, `Book` (bids highest first, asks lowest first),
`Trade` (its `TradeId` an opaque `Arc<str>`), `Candle` (a venue that samples one price per interval
gives only `close`), `FundingRate` (the rate accruing has a `next_funding` unless none is scheduled,
a settled one none; its `interval`, an `Option<Duration>`, is the length of the funding period
its `rate` is for, and settled rows carry the instrument's current interval, since no venue keeps
a history of it) and `EventGroup` with its `EventOutcome`s; every list a
trait returns is oldest first, one request's worth. `CandleQuery` names a `CandleInterval` (one
minute to one week, each width every candle venue serves) and a span. Binance's `usdm::venue`
implements `MarketData`, `Instruments`, `Trades`, `Candles` and `Funding` on `Usdm`, with
`InstrumentExt`, `QuoteExt`, `BookExt`, `TradeExt`, `CandleExt` and `FundingExt`; its quote is a
5-level `depth` (weight 2) and its book an unlimited one (500 levels, weight 1), its `trades` ask
for 1000 rows (weight 20), its `candles` for 500 (weight 2, where no limit costs 5) and its
`funding_history` for 1000, its `instruments` skips an `exchangeInfo` row that does not convert (a
WARN naming the symbol and the error) and the live suite fails when it skips one, and its funding
methods send `fundingInfo` as well (one extra request on the funding layer, `fundingRate`'s bucket
and not a weight) for the `interval`, `fundingIntervalHours` or 8 h for a symbol it does not list,
and `tests/venue_usdm.rs` holds each method to the native client's values on the captured fixtures.
Polymarket's `perps::venue` implements the same five traits on `Perps` (see Perps public routes).
A live test unwraps a trait's `Result<_, ClassifiedError>` with test-support's
`ClassifiedResultExt::or_fail` (or `fail_classified`), since `ClassifiedError` does not implement
`Classify`; `ResultExt::or_fail` and it may be in scope together.

**The trading trait** — `polyoxide-venue`'s `trading` feature (which turns on `market`; Story 7.3)
holds `Trading`, the one interface a venue with signed order routes implements so a consumer
trades the same way on each (AD-3, AD-5, amendment A7-1), and its `DynTrading` wrapper, held as
`Arc<DynTrading<'static>>`. Its methods return `Result<_, ClassifiedError>`: `place(OrderRequest)`
gives the `Order` as the venue accepted it, `cancel(&OrderId)`, `cancel_all(Option<&MarketKey>)`
the ids cancelled, `open_orders(Option<&MarketKey>)`, `positions()` and `balances()`, and
`events()` is not async and returns a `TradingEvents`. The records are `#[non_exhaustive]` with a
`new` that takes their required fields and a `pub extensions: Extensions`: `OrderRequest` (a
`TimeInForce` defaulting to `GoodTilCancelled`, an optional `ClientOrderId`, `post_only` and
`reduce_only`), `Order` (an `OrderStatus` of `Open`, `Filled`, `Cancelled` or
`Killed { reason: KillReason }`), `Fill` (its `trade_id` is `market`'s `TradeId`, its `liquidity`
a `Liquidity`), `Position` and `Balance`; `OrderId`, `FillId` and `ClientOrderId` are opaque
`Arc<str>` ids. A request is refused, never ignored, as `InvalidRequest` with nothing sent when it
carries an extension its venue does not declare, a key of another product, or a `post_only`,
`reduce_only` or time in force the venue cannot honour: `Extensions::ensure_declared(&[TypeId])`
(feature `market`) refuses the first undeclared value in `TypeId` order with an
`UndeclaredExtension` (`InvalidRequest`, in the `Classify` assertion). An immediate-or-cancel order
that matched nothing and a fill-or-kill order that could not fill are `Ok` with `Killed`, not
errors. `TradingEvents` is a newtype over a pinned boxed stream, `Send + Unpin + 'static`, because
dynosaur 0.3.1 drops `'static` from a returned `impl Stream` and the stream would borrow the
`Arc` (E0597, A7-1). The queue behind it is unbounded, so a slow consumer loses no ack or fill, and
the venue replays nothing: the stream's first item is `TradingEvent::Resync` (A7-2, once the fills
connection's first attempt has ended, whether it went live or began in an outage), and it yields
`Resync` after every reconnect of that connection and when the venue voids a fill it reported (a
trade that failed after it matched), which means "reconcile through `open_orders()` and the
trade history: the stream cannot vouch for what it reported or missed". An order may be reported
before the `place` that created it returns. `polyoxide-venue/tests/trading.rs` proves the trait on
a made-up venue, statically and through `Arc<DynTrading<'static>>`, including a stream that
outlives every `Arc` on another thread.

**The CLOB through the traits** — `clob::venue` (Story 6.5) implements `MarketData` and `Candles`
on `Clob` with `clob` alone, `Trades` with `data` as well and `EventGroups` with `gamma` as well
(`all(clob, data)` and `all(clob, gamma)`), keyed by `shared::keys::clob`: a key names one outcome
token (AD-7). `instrument` resolves the token's market with `/markets-by-token/{token}`, then reads
`/markets/{condition}` (a token the CLOB does not know is its `404`, a `VenueRefusal`); `quote`
and `book` read `/book` (a listed token with no book is also its `404`), whose sides the CLOB sends worst first, so both are sorted best first;
`candles` read `/prices-history`, one price per interval as `close`, the `CandleInterval` sent as
`fidelity` minutes, from `start` rounded up to the second and no further than 14 days on, since a
longer window is a `400` (`docs/specs/clob/OBSERVED.md`); `trades` read the Data API's taker-only `/trades` for the token's market and
keep the token's rows, oldest first, with no trade id (the Data API gives none); and `event_group`
reads the event id from Gamma's `/markets?clob_token_ids=` (a private decode, since Gamma's
`Market` does not model its `events`), asked again with `closed=true` when that is empty, since
Gamma leaves closed markets out unless asked (`docs/specs/gamma/OBSERVED.md`), then `/events/{id}`,
keeping only `v1` markets' outcomes.
The CLOB does not implement `Instruments` (amendment A6-1). A number the CLOB or Data API sends as
a float is read through its shortest decimal spelling. `tests/venue_clob.rs` holds each method to
the native clients' values on `tests/fixtures/clob/venue/`, captured for one live neg-risk outcome
by `scripts/capture_polymarket_clob.py`.

**The CLOB through `Trading`** — `clob::venue` (Story 7.4) implements `polyoxide_venue::Trading` on `Clob` with `clob-ws` and `data` together (amendment A7-3): `events()` runs on the supervised user socket, which is `clob-ws`'s, and `positions` reads the Data API. `clob-ws` turns on `polyoxide-venue/trading`, and so does the umbrella's `polymarket-clob-ws`, whose `const _` pins `DynTrading` and `Clob: Trading`; the gate is in the crate's `notes`, so the generated crate list shows it. Every method needs an `Account` and fails with the CLOB's own "Account required" error (`InvalidRequest`) without one. The address that trades is the client's signature type's (`ClobBuilder::signature_type`, by default the account target's): `place` signs as it, and `positions` and `events()` resolve their maker address from it with no funder, as `balances` reads with it, so a client built for a proxy wallet places, lists positions and follows fills as that wallet (found through Gamma; without the `gamma` feature, the native refusal of a proxy with no funder). `place` supports every account the native order routes sign for, a Deposit Wallet's (signature type 3) included: the order's id is its EIP-712 hash under its exchange's domain, computed before it is signed for every signature type, and a Deposit Wallet's order is signed as its ERC-7739 envelope. It refuses before it sends anything, as `InvalidRequest`: a key of another product, an extension other than `ClobOrderOptions` (`neg_risk`, `tick_size` and `expiration`; no fee terms, since a V2 order signs no fee rate, and not `Eq`, since `TickSize` is not), reduce-only, `post_only` or an expiration with anything but good-til-cancelled, a client order id an order still holds, a price off the tick size (the options', else `GET /tick-size`) or outside 0 to 1, and a size over two decimals or under 0.01. It then builds the order with the native `create_order`, signs it and posts it. A FAK or FOK kill, matched on the native `ClobError` variants before any conversion (AD-20), is `Ok` with `Killed { reason }`; a 200 whose `success` is false is a `VenueRefusal`; `live` and `delayed` are `Open`; `matched` is read back from `GET /data/order/{id}` for its `size_matched` (`Filled`, a short IOC `Killed { Unmatched }` with its `filled`, a short FOK `Killed { Unfillable }`, a short resting order `Open`), and a failed read-back leaves it `Open` with a WARN, since an `Err` would invite a duplicate. `cancel` is `Ok` only for an id among `canceled`, and `cancel_all(Some(key))` resolves the token's condition through `/markets-by-token` and sends both it and the token, which the route requires. `open_orders` pages `/data/orders` to the `LTE=` sentinel and refuses a repeated cursor; `positions` is Data v1 `/positions` for the maker address with `sizeThreshold=0`, in pages of 500 up to Data's offset of 10,000, past which it is a `VenueRefusal`; `balances` is one pUSD balance with six decimals and each allowance in `BalanceExt`, an unlimited approval being `Decimal::MAX`. The CLOB stores no client order id, so a `Clob` keeps an in-process ledger of the ones it placed orders with, by the order's id, shared by its clones and its events streams and bounded at 10,000 entries: `place` drops an entry for an order that ended at once or was refused and keeps it after a `Network` or `Unavailable` failure, `cancel`, `cancel_all` and an ack of a `Cancelled` or `Filled` order drop it, and it is lost when the process ends. `events()` opens its own socket when first polled (`ClobBuilder::user_socket` sets its URL and timing) and yields a leading `Resync`, a `Resync` after every reconnect and for a `FAILED` trade (the venue voided a fill the stream reported, and the trait has no event that takes one back), one `Fill` per order of the account's in a `MATCHED` trade (the taker's when the trade's `owner` is the account's API key, then each maker entry at its maker address, a maker on the trade's own token taking the opposite side), and an ack per order message; `MINED`, `CONFIRMED` and `RETRYING` yield nothing, an order message does not carry the time in force, and text that does not parse is an `Err` item that skips its message. `tests/trading_clob.rs` drives all of it on mock servers and the kit's scripted server.

**Wire enums** — Story 3.9 declared the wire enums of gamma (`polyoxide-polymarket/src/gamma/types.rs`), data v2 (`src/data/v2/types/common.rs` there), perps (`src/perps/types.rs` in `polyoxide-polymarket`), Binance (`usdm/types.rs`) and relay (`src/relay/types.rs` and `session_signers.rs` in `polyoxide-polymarket`) with `polyoxide-venue`'s macros: `wire_enum!` for a closed set the client sends (serde renames, `ALL`, `as_str(self) -> &'static str`, `Display`, and a `FromStr` whose error is `polyoxide_venue::UnknownVariant`, `"v" is not a valid X`, classed `InvalidRequest`), and `open_enum!` for a set the venue extends (`#[non_exhaustive]`, an `Other(String)` that keeps an unknown spelling verbatim, `ALL`, `as_str`, `from_wire`, an infallible `FromStr`, and string serde). Both take `$vis enum Name { Variant => "wire", .. }` with attributes on the enum and each variant; neither adds `#[non_exhaustive]` to a closed set, so data's pass it themselves. The expansions name `::serde`, so the calling crate depends on serde, and `wire_enum!`, which derives `::serde::Serialize` and `::serde::Deserialize`, also needs serde's `derive` feature; `open_enum!` implements both by hand and does not. They emit no specta code, since a `cfg(feature = "specta")` in an exported macro is evaluated in every calling crate: data's closed enums pass `#[cfg_attr(feature = "specta", derive(specta::Type))]`, and gamma and data give their open enums a `String` shape with one `polyoxide_venue::specta_as_string!(..)`. Perps and Binance keep separate `Interval` enums (D16). Other one-spelling-per-variant enums are still written by hand: clob's `OrderSide` (`src/clob/types.rs`) and `SortPosition`, `MultiMarketOrderBy` and `UserRewardMarketOrderBy` (`src/clob/api/rewards.rs`), relay's `WalletType`, Binance's `StreamPath` (`usdm/ws/mod.rs`), gamma's `ParentEntityType`, and data's v1 enums in `data/types.rs`, among them the `#[serde(other)] Unknown` ones (`TradeSide`, `ActivityType`, `ApprovalFeature`, `ApprovalStandard`, `ComboStatus`, `ComboLegStatus`).

## Environment Variables

For authenticated operations (CLOB trading, user data):
```
POLYMARKET_PRIVATE_KEY        # Hex-encoded private key
POLYMARKET_API_KEY            # L2 API key
POLYMARKET_API_SECRET         # L2 API secret (base64)
POLYMARKET_API_PASSPHRASE     # L2 API passphrase
```

Relay operations need either `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` (HMAC auth) **or** `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS` (static key auth). Relay also reads `RELAYER_URL` and `CHAIN_ID` optionally.

**Keychain alternative** — With the `keychain` feature enabled, credentials can be stored in and loaded from the OS keychain instead of environment variables. Use `Account::from_keychain()` (CLOB, the `polyoxide-clob` service), `BuilderAccount::from_keychain()` (Relay, `polyoxide-relay`), or the CLI's `polyoxide polymarket credentials <store|show|delete> <clob|builder>`. Each account's fields are listed once, in its `polyoxide_core::keychain::StoredCredential` impl, which its `delete_from_keychain` and the CLI both use. The `keychain` feature is optional and not enabled by default.

## API Specs

Upstream API documentation lives in `docs/specs/`, one directory per host. See `docs/specs/INDEX.md` for the full index. These are the source of truth for endpoint contracts, rate limits, and response schemas. Polymarket's are mirrored from https://docs.polymarket.com and the official OpenAPI specs; Binance publishes no spec for USDⓈ-M, so `docs/specs/binance/` holds what its hosts were observed to do, not a mirror.

**Not fully implemented.** `docs/specs/` also mirrors three upstream APIs that no
polyoxide crate fully covers: **Perps** (`perps/`, 61 endpoints on
`api.perpetuals.polymarket.com`, with its own `POLYMARKET-PROXY` /
`POLYMARKET-SECRET` header auth rather than the L1/L2 scheme — the 21 public
`/v1/info/*` routes are implemented by `polyoxide-polymarket`'s `perps` module; credentials, the
`/v1/account/*` reads, the signed `/v1/trade/*` routes, funds and BLP are
pending, and `docs/specs/perps/OBSERVED.md` records where the host and the
schema part ways), **Bridge** (`bridge/`, 5 endpoints), and **Combos RFQ**
(`combos-rfq/`, 4 endpoints). They are mirrored so parity audits can see them;
adding client support for the rest is a separate piece of work.

**Perps public routes** are implemented by `polyoxide-polymarket`'s `perps` module, feature
`perps` (`perps::Perps::new()`, namespaces `health()`, `exchange()`, `market()`, `public()`).
`perps::venue` implements `polyoxide-venue`'s `MarketData`, `Instruments`, `Trades`, `Candles` and
`Funding` on `Perps` (Story 6.4), keyed by `shared::keys::perps`: an instrument is its
`/v1/info/instruments` row plus its type and category's `/v1/info/fees` entry (two requests),
with sizes in `SizeUnit::Base` (one contract is one unit of the base asset, which
`docs/specs/perps/OBSERVED.md` shows from the fixtures) and no `tick_size`, since a non-integer price keeps five significant figures, and its fee
tiers, risk tiers, price bounds and decimals in `InstrumentExt`; a quote is the `bbo` row, a side
of zero quantity none and an instrument with no row an empty quote; the funding rate is the
instrument's row of `tickers`, which the host sends for every instrument, and its `interval` is
the instrument's `funding_interval` (`1h`, read as a whole number of minutes, hours or days; an
unreadable one is a `Decode`), one more `/v1/info/instruments` request, which a `funding_history`
with no rows skips; `trades` and
`funding_history` reverse the host's newest-first pages, and `funding_history` asks for the hourly
settlements from `start`, at most 100 (`end_timestamp` is `start` + 100 h less a minute), since the
host answers a start alone with the newest 100 rows since it. `tests/venue_perps.rs` holds each
method to the native client's values on the captured fixtures.
Three test files of `polyoxide-polymarket` hold it in place on the pattern of Data v2:
`tests/spec_agreement_perps.rs` (types, enums and query keys against
`docs/specs/perps/openapi.json`, restricted to schemas reachable from `/v1/info/*`),
`tests/wire_agreement_perps.rs` (against `tests/fixtures/perps/`, refreshed by
`scripts/capture_polymarket_perps.py`) and `tests/live_perps.rs`. Wire-only fields are allowed through `OBSERVED_EXTRA` and
recorded in `docs/specs/perps/OBSERVED.md`. Klines, mark points and book levels are
positional arrays on the wire, decoded through tuples of exact arity over
`polyoxide_venue::positional::DecimalStr`; Binance's klines and levels use the same
module's `element` and `drain` in visitors that tolerate extra elements. The host ignores
`instrument_id` on `/v1/info/tickers` and `/v1/info/statistics` and returns
every instrument, so a caller filters client-side. The four WebSocket fields on
`LimitTier` are a `u32::MAX` sentinel, not a budget, and must not size a
bucket. Every index has empty constituents today. The host is fronted by
CloudFront, so the rate rows in `shared::perps_limits` were measured
with `polyoxide-polymarket/examples/info_soak.rs` over distinct URLs: klines 30,
trades 10, portfolio 30 and bbo 50 per 10 s, a `/v1/info` catch-all at 10 for
the routes not soaked, and a client-wide general bucket of 30 set by two mixed
validation runs, all recorded in OBSERVED.md's `## Rate limits`.
The `perps-ws` feature adds the six public WebSocket channels, `perps::ws`: `PerpsWs` (bare) and
`PerpsWsBuilder`, whose `connect` returns `SupervisedPerpsWs`, an alias for the kit's
`polyoxide_ws::Supervised<PerpsProtocol>` (Story 4.6). The kit's `Supervisor` keeps the JSON
`post/ping` on the wall clock regardless of traffic, and also while the 1024-event queue is
full, without awaiting the pong; staleness counts any text frame, a pong included, and a pong
that is not `ok` reconnects; reconnect with resubscribe; a 10 s connect timeout (DRIFT R2).
The host's own keep-alive is a protocol Ping every 10 s: its pings and the client's automatic
pongs count as traffic, so a client that reads was never closed in 180 s, and only with the
pongs withheld did it close, with `1011 pong_timeout` after about 70 s (bundle K's probe,
2026-10-10; the documented 60 s idle close was not seen). Whether it also counts `post/ping`
was not measured. The pongs go out only while the connection is read, so a supervised stream
whose queue stays full that long may be closed with a 1011 and reconnect. Membership changes go
through `membership()`, a `MembershipHandle<PerpsProtocol>`, whose `PerpsMembership` trait gives
`subscribe` and `unsubscribe` (or `apply` with a `MembershipChange`). `PerpsProtocol` keeps
perps' rows: no `Disconnected` (D8), a `message_rate_limited` subscribe retried within the
change and `SequenceRegressed` per connection (D10), `AlreadyClosed` stops the stream (D19),
and an empty membership stays wanted, so a reconnect replays nothing. Supervision logs under
`polyoxide_ws`. Do not copy the rtds pump shape for this host: it pings only on a quiet tick. `Channel` is the only way to name a
subscription; payload structs carry the REST field names over the socket's terse keys. Four wire facts the AsyncAPI mirror gets wrong are in
`docs/specs/perps/OBSERVED.md`: `tickers`/`statistics` data are objects and
`::all` fans out per instrument, `ets` is on every frame, `sq` is a
server-wide stamp so only regressions are detectable, and an unknown
instrument subscribes without error. Offline tests drive the kit's scripted
server (`polyoxide_ws::test_server`) with perps' answer, through the `cfg(test)`
adapter `src/perps/ws/scripted.rs`; perps exposes nothing under `test-server` (Story 4.6).
`tests/live_perps_ws.rs` is the live suite, and `tests/wire_agreement_perps_ws.rs` compares the
payload types value-for-value against `tests/fixtures/perps/ws/` (captured by
`scripts/capture_polymarket_perps_ws.py`, instrument 6 because `trades::1` was
quiet).

**Binance USDⓈ-M futures is not a Polymarket host.** `polyoxide-binance`'s `usdm` module,
feature `usdm` (on by default), reads its public market data on `fapi.binance.com`
(`usdm::Usdm::new()`, namespaces `health()`, `exchange()`, `market()`) for consumers that
trade both venues. The `polyoxide` umbrella re-exports the crate as `polyoxide::binance`,
under its `binance`, `binance-usdm` and `binance-usdm-ws` features and `full`, none of which
the umbrella's default, `polymarket`, turns on. Binance publishes no OpenAPI or AsyncAPI for
USDⓈ-M, so `docs/specs/binance/` is not a mirror: `OBSERVED.md` records what the host
does, `nightly-schema.yml` has nothing to diff, and
`tests/live_usdm.rs::live_responses_carry_no_unmodelled_keys` is the REST drift detector and
`tests/live_usdm_ws.rs::live_frames_carry_no_unmodelled_keys` the streams'.
Binance limits each IP by request *weight*, which depends on the route and its
parameters, so the crate has its own `usdm::WeightBudget` (`src/usdm/weight.rs`) instead of core's
`RateLimiter`: the UTC clock minute at 2160 of the published 2400 (the tenth core's
`RESERVED_FRACTION` also reserves), raised by every response's `X-MBX-USED-WEIGHT-1M` but only for the
minute its request was charged in, a separate bucket for the weightless funding routes
(450 per 5 minutes, depth one, paced by core's `WindowQuotaTable::paced_interval` on the
budget's own clock), and a `429` or `418` held as a client-wide cooldown that is only ever
extended (core's `Hold`, with a 3-day ceiling). The budget is the client's `Throttle` on
core's send loop: each route builder (`GetKlines`, `GetTime` and the rest, over a private
`Routed<T>`) hands core's `Request` its weight as a `Cost` on `usdm::weight::WEIGHT_LAYER` or
`FUNDING_LAYER`, and every `Usdm` built with one budget shares its hold. The private
`UsdmRetryPolicy` (`src/usdm/policy.rs`) retries a `429` with a retry left, holding the
longer of its backoff and its `Retry-After`; a `429` with no retry left and no `Retry-After`
holds every request to the next minute, since sending into a spent minute is how a `429`
becomes a `418` ban; a `418` fails and holds its `Retry-After`, or two minutes; a `425` is
not retried. A `418` logs its body at WARN under `polyoxide_binance::usdm::error`
(`polyoxide_binance::error` until Story 5.6), and the send loop
warns of every hold under `polyoxide_core`. The weight table, `Route::cost`, is measured, not copied: Binance's page is one off
at the 100 and 500 `klines` edges and does not say that omitting `limit` costs 5 on
`klines` and 1 on `depth`. `cargo run -p polyoxide-binance --example weight_probe`
re-measures it. Response rows carry symbols as `String`; `Symbol` is for what a caller
sends, uppercases ASCII, and accepts `_` for quarterlies (`BTCUSDT_261225`).
`FundingRate::mark_price` is an `Option` because funding events through at least
2022-01-01 send `""`. Core is the only member that declares reqwest (AD-18): it enables
`json`, `rustls-tls` and `gzip` with no default features, re-exports the crate as
`polyoxide_core::reqwest`, through which every other member's src, tests and examples name
its types, and `test_dependency_fences.py` fails on any other member's direct dependency, so
no crate can change reqwest's features for the rest. The same fence holds `tokio-tungstenite`
and `rustls` to `polyoxide-ws` (Story 4.10). Each of those six HTTP modules has a
`tests/headers_<module>.rs` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay` and
`perps`, and `polyoxide-binance`'s `usdm`)
that sends one unauthenticated `GET` through `polyoxide_test_support::query::headers_sent`
and asserts the literal set `accept: */*`, `accept-encoding: gzip` and `host`, nothing else; CI
runs it in the crate's minimal build, with only the module's own feature (the **features**
job), in the workspace build, and beside the umbrella with `polyoxide/full` (the **features**
job again), so a feature another member, or another venue's module, switches on fails one of
the three. Core enables reqwest's `gzip` feature
(`exchangeInfo` is 1.15 MB raw, 51 KB gzipped), and `HttpClientBuilder::gzip` is unset by
default, which leaves reqwest's default: every polyoxide client asks for gzip, as each did
before 0.37 for a consumer whose workspace enabled the feature. 0.37.0 forced it off unless
set, which took compression away from those consumers (prader-rs among them); 0.37.1
restored it. No client builder sets it, Binance's included, and `ClientConfig` has no `gzip`
knob. Four rate-limit examples pin `.gzip(false)` on their bare clients, so the soak
measurements keep the wire they were made on.

With the `usdm-ws` feature the `usdm` module also streams eight USDⓈ-M market streams on
`fstream.binance.com`: `UsdmWs` (one connection on one path, on the kit's `Connection`) and
`UsdmWsBuilder`/`SupervisedUsdmWs` (one kit `Supervisor` per routed path, `/market` or
`/public`, since a stream on the wrong path may deliver nothing, behind a router that
sends each change to its path, opens a path for its first stream and forwards both paths
into one queue of 1024; the private `UsdmProtocol` in `src/usdm/ws/protocol.rs` keeps
Binance's rows: pings on the wall clock, also while the queue is full, so a slow consumer
is never stale; staleness counting every frame, pongs and the server's pings included;
reconnect with a paced replay, bounded as a whole by `stale_after`; rotation at 23 h 50
min, under Binance's 24-hour cutoff; a change recorded before it is sent, so one whose
connection dies under it, or that the server leaves unanswered for `stale_after`, is
answered `Ok` and replayed). Binance acknowledges every
`SUBSCRIBE`, unknown and uppercase names and an explicit `@250ms` depth included (a
250 ms partial depth is the bare `<s>@depth<N>`), and enforces its rules by closing the
connection (on the 1025th stream, or after about 15 requests in a burst), so the client
enforces them first: `StreamName` is the only way to name a stream, and a connection
carries at most 1024 streams, 200 names per request, one request per 200 ms. A path that
stops on an error ends the whole stream after yielding it. Once the router has that stop,
`close()` and every later membership call answer `UsdmWsError::Stopped { cause }`, the
cause that error's `StopCause` (its class, fault, wait and text), so a stream stopped by a
451 refusal is `Restricted` and not a fault (Story 4.10); a change in flight when the path
stops, or a `close()` made right after the stopping error, can still get
`Stopped { cause: None }` or `Ok`. Every `Event::Disconnected { path }` is followed by
`Event::Reconnected { path }` while the client runs, even when the path's last stream
leaves mid-outage; prader-rs folds outages on that invariant, and `tests/supervision_usdm.rs`,
`tests/supervision_usdm_edges.rs` and `tests/supervision_usdm_kit.rs` pin it.
COIN-M rows (`st: 2`) arrive on this host and count contracts, not USDT, so they do not sum
with USDⓈ-M rows. The offline tests drive the kit's scripted server with Binance's answer
(`src/usdm/ws/scripted.rs`, built for the unit tests and included by the integration
suites with `#[path]`); the `test-server` feature, with `usdm-ws`, exposes the captured
frames as `usdm::ws::fixtures`, which the CLI's tests import, and nothing else (Binance's own
server is gone, Story 4.7). Since Story 5.6 it turns no module on, so a build that wants the
frames names both features.

**Data API v2** (`data-v2/`, 20 endpoints under `/v2` on `data-api.polymarket.com`)
is implemented by `polyoxide-polymarket`'s `data` module as `data.v2()`, alongside the v1 routes, which
upstream says keep working. v2 uses a different contract: a `data` envelope,
cursor-only pagination and snake_case fields. Paged builders return `Page<T>` from
`send()` and a `Stream` from `.pages()`, which clones one request per page so a walk
cannot change its filters: on `trades`/`activity` a changed filter re-anchors the walk
silently, and on `positions` the `title`, `condition` and window filters are not in the
cursor at all. An error with the v2 body becomes `DataError::V2` (stable `code`, the
server's `retryable` flag, `trace_id`), recognised by body shape rather than path. Its
spec is the one mirror served by the API host (`/v2/openapi.json`) rather than
`docs.polymarket.com`, so a parity audit that only walks `/api-spec/` misses it.

Three test files of `polyoxide-polymarket` hold v2 in place. `tests/spec_agreement_data_v2.rs`
checks every type's optionality and field names and every builder's query keys against that
schema. `tests/wire_agreement_data_v2.rs` checks the types against live captures in
`tests/fixtures/data/v2/`, in both directions (`scripts/capture_polymarket_data.py` refreshes
them). `docs/specs/data-v2/OBSERVED.md`
records where the server and the schema part ways. Notably `/v2/user-pnl` is **not** the
`data.pnl()` series and `/v2/leaderboard` ranks volume in shares, not USDC, so neither
replaces the undocumented host. The v2 rows in `shared::data_limits` were measured with
`polyoxide-polymarket/examples/v2_soak`, which sends raw requests over distinct URLs: several v2
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
v2 error maps by `code`; every other error maps by its class, one class to one exception
(`exception_for` in `polyoxide-py/src/error.rs`): `Network` `NetworkError`, `Unavailable`
`UnavailableError`, `RateLimited` `RateLimitError`, `Unauthorized` `AuthenticationError`,
`InvalidRequest` `ValidationError`, `VenueRefusal` `ApiError`, `Restricted`
`RestrictedError` and `Decode` `DecodeError`, never by its message (C4).
`TimeoutError`, which only v2's `request_timeout` raises, subclasses `UnavailableError`, and
`tests/test_errors_offline.py` asserts each class's exact type. Every SDK exception carries
`status`, `code`, `retryable`, `trace_id`, `parameter` and `retry_after`, which are `None`
unless the error came from a v2 route.

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
version `"3"`) and reads balances as `CONDITIONAL-V2`. The `clob` module signs
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
`scripts/capture_polymarket_session_keys.py` regenerates the fixtures in
`polyoxide-polymarket/tests/fixtures/{clob,relay}/session_keys/` from py-sdk; every
signing, encoding and wire-body test is pinned to them, never to a self-computed value.
Implemented by `polyoxide-polymarket`'s `clob` module (`SigningTarget`,
`Account::{with_signer,l2_only}`, `*_with_signature`, `list_session_signers`) and its `relay`
module (`WalletType::DepositWallet`, `deposit_wallet`, `session_signers`, `resolve_wallet`,
`with_auth`). The live round trip is `polyoxide-polymarket/tests/live_clob_session_keys.rs`
(features `clob`, `relay` and `gamma`), `#[ignore]`d and gated on a Deposit Wallet fixture
account that does not exist until prader-rs #125 is resolved.

## Testing Conventions

The CLI has live integration tests in `tests/live_api.rs`, the umbrella in `tests/live_venue.rs` (the venue-neutral example against each venue), and each module of a venue crate in `tests/live_<module>[_<suite>].rs` (`polyoxide-polymarket`'s `live_clob`, `live_clob_ws`, `live_clob_session_keys`, `live_gamma`, `live_data`, `live_relay`, `live_perps`, `live_perps_ws`, `live_rtds` and `live_sports`, and `polyoxide-binance`'s `live_usdm` and `live_usdm_ws`), gated with `#[ignore]` so CI skips them. They hit the real upstream APIs. Run with `-- --ignored` flag.

The read-only modules (gamma, data) use `Gamma::builder().build()` / `DataApi::builder().build()` directly. CLOB tests use `Clob::public()` for unauthenticated endpoints.

Mock HTTP tests use `mockito` (workspace dev-dependency). Each HTTP module of a venue crate has a `tests/mock_<module>.rs` file: `polyoxide-polymarket`'s `mock_clob`, `mock_gamma`, `mock_data`, `mock_relay` and `mock_perps`, data's v2 in `tests/mock_data_v2.rs` beside `mock_data.rs`, and `polyoxide-binance`'s `tests/mock_usdm.rs`, which stands alone. The socket modules `rtds` and `sports` have none: their offline suites (`supervision_rtds`, `supervision_sports`, `bare_sports`) run on the kit's scripted server (polymarket's `tests/common/mod.rs` is shared by `live_data.rs` and `holders_shape_data.rs`, not by the mock suites). Each has helper functions like `test_public_clob(server)` that point clients at the mock server URL.

**One scripted WebSocket server lives in `polyoxide-ws`**, `polyoxide_ws::test_server` behind its `test-server` feature (doc-hidden, built only from ws' own dependencies, never from `polyoxide-test-support`). `ScriptedServer::start(Vec<Script>)` plays one script per connection, the last repeating, and `start_answering` takes a closure called per connection that returns that connection's answerer, owning its protocol state. The actions are reject, stall, refuse-upgrade with headers, send, `after_first_frame` (record the client's first frame before sending), `push_every`, `ping_every` (big-endian sequence payloads), `close_after` with an optional code, `drop_after`, `go_silent`, hold (the default) and a cut with no close frame, after sending or on a schedule (`cut_after`); the recorders cover frames per connection, first frames, paths, closes, close replies, pongs, pings received, accept times and end counts, and `wait_for` polls for 5 s. The helpers are `fast()` (a `Timing` of 50 ms pings, 300 ms staleness and 10 to 20 ms backoff), `next_item`, `next_event` and `drain`. perps', Binance's and rtds' suites run on it, and their own servers are gone (Stories 4.6 to 4.8); sports' suites run on it too, and its own server is gone as well (Story 4.8); clob's membership tests run on it, and its two loopback servers are gone (Story 4.9); each venue switched its suites first, under its old code, and then moved its loop or adopted the blocks (amendment A4-1); each venue keeps only its protocol's answer and its fixtures.

**The agreement, fixture and soak helpers exist once, in `polyoxide-test-support`.** `fixtures!("sub")` reads `tests/fixtures/sub/` of the crate that expands it (a macro, because `env!("CARGO_MANIFEST_DIR")` inside test-support names test-support). `agreement` holds `key_paths`, `assert_values_agree` (`Arrays::Zip` for perps REST and binance, whose kline drops an element on purpose; `Arrays::SameLength` for perps ws), binance's `compare`/`compare_values`, the allow-list `Ledger` (`excuse`, perps' `never_on_wire`, `stale`) over `(fixture, path, reason)` or `(path, reason)` tables, and gamma's dotted walker as `agreement::dotted`. `openapi` is the schema synthesiser and `check` (perps' `$ref`-nullable, enum, example and `OBSERVED_EXTRA` handling plus data's two-`allOf`-arms refusal), `query` (feature) reads a builder's query keys off a mock server (`keys_sent`), and its ordered key/value pairs (`pairs_sent`, which does not need the body to decode; `Case` and `assert_cases` run a table of builder calls over it for each `tests/query_setters*.rs`), and the full header set one request carries (`headers_sent`, for each HTTP module's `tests/headers*.rs`), `minute` keeps a test clear of a UTC minute boundary, `registration!()` reads the ids the calling crate declares in its `[package.metadata.polyoxide]`, and `soak` (feature) is the harnesses' `Pacer`, `percentile`, `parse_stages`, `parse_routes`, the `observe` throttle layer (the `WARN` under `polyoxide_core` that `tests/observe.rs` proves against core's client) and the two verdict rulebooks, `soak::verdict::tolerant` (data's) and `soak::verdict::strict` (perps'), which are different policies and stay apart. Each suite keeps a thin local function with its old name that states its own allow-lists and venue facts, so test names and bodies did not move; binance's live and agreement targets alias the module as `use polyoxide_test_support::agreement as common;`. The capture scripts share `scripts/capture_common.py`: `get` (a `headers(method, url)` function called per request, so signing stays in each script; a non-JSON error body is returned, not raised), `require_ok`, `write_json`, `write_raw`, `stamp`, `provenance_table`, `write_provenance` and the async `ws_session`, which imports `websockets` lazily. Run a capture script as a file (`python3 scripts/capture_x.py` or `uv run scripts/capture_x.py`), never `python3 -I`, which keeps `scripts/` off `sys.path`; Binance's, `scripts/capture_binance_usdm.py`, is a PEP 723 `uv run` script.

## Nightly API Smoketest

Two GitHub Actions workflows run at `0 6 * * *` UTC and on `workflow_dispatch`. The behavioral one also runs on Saturday and Sunday at 18:30 UTC (`30 18 * * 6,0`). The sports feed carries only what is live, and 06:00 UTC misses North American leagues and weekend soccer, which the drift detector would otherwise never see. That run covers every live job, so its clean result may close the issue.

- `.github/workflows/nightly-behavioral.yml` — runs `--ignored` live tests, one job per crate and suite, built with `--no-default-features` and its targets' declared features alone, each job's `env:` holding only the secrets its tests read. An unset repository secret arrives as `""`, which the live loaders treat as unset:
  <!-- generated:begin claude-nightly -->
  - `live-polyoxide-binance-usdm` (15 min): `live_usdm` with `--features usdm`, `live_usdm_ws` with `--features usdm-ws`
  - `live-polyoxide-polymarket-clob` (15 min): `live_clob` with `--features clob,gamma`, `live_clob_ws` with `--features clob-ws`; secrets `POLYMARKET_API_KEY`, `POLYMARKET_API_PASSPHRASE`, `POLYMARKET_API_SECRET`, `POLYMARKET_BUILDER_CODE`, `POLYMARKET_PRIVATE_KEY`
  - `live-polyoxide-polymarket-clob-session-keys` (40 min): `live_clob_session_keys` with `--features clob,relay,gamma`; secrets `BUILDER_API_KEY`, `BUILDER_PASS_PHRASE`, `BUILDER_SECRET`, `POLYMARKET_DW_OWNER_PRIVATE_KEY`, `POLYMARKET_DW_SESSION_PRIVATE_KEY`, `POLYMARKET_DW_WALLET`
  - `live-polyoxide-polymarket-gamma` (15 min): `live_gamma` with `--features gamma`
  - `live-polyoxide-polymarket-data` (15 min): `live_data` with `--features data`
  - `live-polyoxide-polymarket-relay` (15 min): `live_relay` with `--features relay`; secrets `BUILDER_API_KEY`, `BUILDER_PASS_PHRASE`, `BUILDER_SECRET`, `POLYMARKET_PRIVATE_KEY`, `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS`
  - `live-polyoxide-polymarket-perps` (15 min): `live_perps` with `--features perps`, `live_perps_ws` with `--features perps-ws`
  - `live-polyoxide-polymarket-rtds` (15 min): `live_rtds` with `--features rtds`
  - `live-polyoxide-polymarket-sports` (20 min): `live_sports` with `--features sports`
  - `live-polyoxide-venue` (15 min): `live_venue` with `--features polymarket-clob,polymarket-perps,binance-usdm`
  - `live-polyoxide-cli-live` (15 min): `live_api`
  <!-- generated:end claude-nightly -->
  Failures are classified by `.github/scripts/classify_failures.py` from the tag line each failing test prints (below). A failure with no tag is `real`; nothing reads the panic text.
  - **auth-gated** (a credential loader found a secret absent or empty) — silently skipped
  - **environmental** (`environmental(reason)`, where the world can't provide signal right now — the sports feed with no live matches, a quiet RTDS or perps feed, no market satisfying an order test's precondition — or an error whose class is `Restricted` and not a fault, such as Binance refusing a US runner's location with a 451) — logged and skipped
  - **transient** (an error whose class is `Network`, `Unavailable` or `RateLimited`: HTTP 408/425/429/5xx, connection refused, timeouts, DNS, a dropped WebSocket, close codes 1000/1001/1006/1011–1013; or `transient(reason)`, which the socket tests call where a bare stream ends without a close code) — retried up to 2× with `cargo nextest --retries 2`, and filed as real if it still fails, unless the retry is environmental or auth-gated, whose verdict it then takes (a dropped connection, then a quiet feed)
  - **real** (everything else, every untagged failure included) — files or updates a tracking issue with the `nightly-behavioral` label. Only a run on `main` touches that issue: the step that files, comments on and closes it is guarded by `github.ref == 'refs/heads/main'`, so a dispatch on another branch, such as an integration branch before it merges, classifies and uploads its reports and leaves main's issue alone; there a step guarded by `github.ref != 'refs/heads/main'` writes the combined report to the run's summary and fails the run on any real failure, since the live jobs pass whatever their tests did
- `.github/workflows/nightly-schema.yml` — fetches each published upstream spec and compares it against the vendored mirror in `docs/specs/`.
  <!-- generated:begin claude-schema-watch -->
  It watches eight OpenAPI (`clob`, `gamma`, `data`, `data-v2`, `relay`, `perps`, `bridge`, `combos-rfq`) and four AsyncAPI (`clob-ws-market`, `clob-ws-user`, `perps-ws`, `combos-rfq-ws`), each filed under its own `spec:<id>` label.
  <!-- generated:end claude-schema-watch -->
  On drift, files a tracking issue labelled `schema-drift` **and** `spec:<id>`. It creates no branches and opens no PRs — Actions cannot open PRs here (org policy: 12 refusals, 0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out. The workflow holds `contents: read` and `issues: write` only. The issue is found by label intersection, never by title: `gh issue list --search "<title> in:title"` is a tokenized full-text search, so `perps` also matches `perps-ws` — that collision let one spec's job edit and close another's issue for eleven days, and made `combos-rfq-ws` look like it was flapping. A spec we deliberately will not sync is recorded in `docs/specs/.drift-acknowledged.json`, keyed by the SHA-256 of the canonical diff, which makes the check exit 3 and close the issue. Fingerprinting the *disagreement* rather than upstream means the acknowledgement expires the moment either side moves, so it is never permanent blindness. `clob` is acknowledged because upstream's own re-serialization made `example: 'Yes'` parse as boolean `true` on a `type: string` field. The issue body carries a key-path summary (changed JSON pointers with before → after, so drift inside `components.schemas` is named rather than merely counted) plus the canonicalized diff, composed in Python under GitHub's 65536-character cap.
  Deliberately excluded, each for one reason:
  <!-- generated:begin claude-schema-exclusions -->
  - Sports (`docs/specs/sports/`): Upstream's AsyncAPI documents a `slug`-keyed payload and a text ping/pong that the server never sends, so the mirror is modelled on captured wire frames, and diffing it would report drift forever.
  - Undocumented hosts (`docs/specs/undocumented/`): `user-pnl-api` and `lb-api` publish no spec to diff against; their shapes were derived from live responses, and `polyoxide-polymarket`'s `live_data` suite is the drift check.
  - RTDS (`docs/specs/rtds/`): Upstream publishes no AsyncAPI for `ws-live-data`; the mirror is modelled on captured wire frames, so there is nothing to diff it against.
  - Deposit Wallets and session keys (`docs/specs/session-keys/`): The surface is almost entirely absent from the published CLOB and relayer OpenAPI (only `/deployed?type=WALLET` appears), so there is no mirror to diff; the SDK-generated fixtures are the drift check.
  - Binance USDⓈ-M (`docs/specs/binance/`): Not a Polymarket host, and Binance publishes no OpenAPI or AsyncAPI for USDⓈ-M futures; the live suites in `polyoxide-binance` are the drift check.
  <!-- generated:end claude-schema-exclusions -->

**Tags decide.** A live test that fails through `polyoxide-test-support` prints `polyoxide-class=<tag>` alone on a stderr line just before it panics, and that line is all `classify_failures.py` reads. Only the tag just above the log's final `panicked at` report decides, since one anywhere else belongs to a panic the test caught or a spawned task raised; a `real` or unknown tag anywhere makes the failure `real`, and so does no tag at all. `.or_fail(ctx)` (`ResultExt`, and `ClassifiedResultExt` for a venue trait's `ClassifiedError`), and `fail(ctx, &err)` (`fail_classified` for a `ClassifiedError`) for an error in a match arm or a source chain, take the tag from the error's `Class` and `is_fault()` (`Network`, `Unavailable` and `RateLimited` are `transient`; `Restricted` is `environmental` when it is not a fault, such as a 451 region block, and `real` when it is, such as a 418 ban the client earned; every other class is `real`), the credential loaders (`load_env`, `optional_env`, `keychain`) count an empty value as absent and fail through `Missing::or_auth_gated()` as `auth-gated`, and `environmental(reason)` and `transient(reason)` tag what no error value carries. A foreign error is wrapped in the venue's own error first (`ApiError::Network`, `UsdmWsError::from(WsError::from(..))`, `RtdsError::from(WsError::from(..))`, `ClobWsError::from(WsError::from(..))`, `SportsError::Transport { source: WsError::from(..) }`), and so is a raw socket's close frame, so it takes its tag from the socket table.

The regexes that once guessed a failure's kind from its panic text are deleted (Story 2.7), and `scripts/live_unwraps.py` keeps it that way. It freezes every regex the classifier's source builds, read from its AST; what is left parses the tag line, the panic report and nextest's retry suffix. It holds a per-file count, at zero, of `.unwrap()`/`.expect(` and `.unwrap_err()`/`.expect_err(` (and `Result::unwrap`-style paths) in live tests and of bare failure sites: every `panic!`, `unreachable!`, `todo!` and `unimplemented!`, and every `assert!`-family macro whose message interpolates or passes a value named for an error or a status. A line that must keep one, such as a parse of test data or an assertion on the response, ends with `// live-unwraps: <reason>`; it fails untagged, so it files as `real`, and moves to a per-file `opted_out` count that may not rise without a hand edit a reviewer sees. Each regex-era case is a row of `.github/scripts/tests/test_classify_failures.py`'s tag table, and each row naming an error the tests can build has a Rust twin in a failure-tags suite (`polyoxide-test-support`'s `tests/failure_tags.rs` for core's `ApiError`, `polyoxide-binance`'s `tests/failure_tags_usdm.rs` for its own, and `polyoxide-polymarket`'s `tests/failure_tags_sports.rs` and `tests/failure_tags_rtds.rs` for sports' and rtds') asserting the tag that error prints.

A live target reads the environment only through the loaders, which take their env names as string literals; `gen_registry.py`'s `env_names()` reads those calls for the secrets check, and `direct_env_reads()` refuses any `std::env` or `env::` path, `env!`/`option_env!`, `dotenvy` or a library `from_env()` in a live target, whose names that check could not see. The loaders read a `.env` file into a private map behind the environment, never with `set_var`, and warn naming the file and line of any line that does not parse. nextest suffixes a retried test's name with `#<attempt>`; the classifier strips it, or `merge` would miss every persistent transient.

To enable CLOB/relay's auth-gated tests, set the `POLYMARKET_*`, `BUILDER_*` and `RELAYER_*` repo secrets. The loaders then find them and the tests start contributing real signal; nothing in the classifier changes. Relay's credentialed tests no longer pass silently without their secrets: they fail as `auth-gated`, like clob's.

## Module Organization

Most crates follow a consistent layout, and each module of a venue crate keeps it under
`src/<module>/` (`polyoxide-polymarket`'s `clob`, `gamma`, `data`, `relay`, `perps`, `rtds` and
`sports`, and `polyoxide-binance`'s `usdm`, whose `mod.rs` holds its client), with `mod.rs` in
place of `lib.rs`, its README beside it (but `rtds`, whose guide is its `mod.rs` docs, as
`test_module_readmes.py`'s `WITHOUT_README` records) and its suites named `<kind>_<module>`:
- `lib.rs` — public API re-exports
- `client.rs` — main client struct + builder
- `error.rs` — crate-specific error enum (uses `thiserror`)
- `types.rs` — domain types
- `api/` — namespace modules, one file per API group (markets, orders, etc.)

**WebSocket** support for the CLOB lives in `polyoxide-polymarket/src/clob/ws/` (not core), feature-gated behind `clob-ws` (not enabled by default; `polyoxide-polymarket`'s default = `["clob", "gamma", "data"]`). Its error is `ClobWsError` (`WebSocketError` until Story 5.4), whose never-constructed `ConnectionClosed` variant stays until S3, because a protected test builds it (`Authentication` is built by the supervised user socket, on the server's 1008 close). Two channels: `WebSocket::connect_market(asset_ids)` (public) and `WebSocket::connect_user(condition_ids, credentials)` (authenticated). Implements `futures_util::Stream`. `WebSocketBuilder` provides auto-ping keep-alive for long-running connections. The Perps socket (`polyoxide-polymarket/src/perps/ws/`, feature `perps-ws`), RTDS (`src/rtds/`, feature `rtds`), the sports feed (`src/sports/`, feature `sports`) and Binance's market streams (`polyoxide-binance/src/usdm/ws/`, feature `usdm-ws`) are separate protocols, each in its own module. No socket crate keeps its own `ensure_crypto_provider` copy: perps, Binance, rtds and sports connect through the kit, and clob upgrades each address's TCP connection through the kit's `connect_over`.

**The supervised CLOB market socket** (Story 7.1) is `clob::ws::MarketWsBuilder` and its
`SupervisedMarketWs`, in `polyoxide-polymarket/src/clob/ws/supervised/`, beside the unchanged bare
tier (`WebSocket`, `WebSocketBuilder`, `WebSocketWithPing`). `ClobMarketProtocol` describes the
market channel to the kit's `Supervisor`. The stream yields `MarketEvent`s, which are the channel's
messages, one `Disconnected` and one `Reconnected` per outage, and a `Disconnected` carries a
`DisconnectReason`, the kit's `Outage` as `Closed`, `Error`, `Stale` or `Rotation`. It reconnects
after a server close, a stall or a lost connection, and replays the whole membership, changes and
the builder's `MarketSubscriptionOptions` included, as one subscribe frame on every connection, an
empty one too, which the venue keeps open. It sends the text `PING` every 10 s on the wall clock
(D4), the first one interval after the connection opens, counts every inbound frame as liveness and
is stale after 30 s (D5), backs off 500 ms to 60 s with the kit's `Retry-After` floor, never
rotates, and queues 1024 events. `MarketWsBuilder::connect(asset_ids)` refuses more than 500 ids
before connecting and returns the first attempt's failure instead of retrying it. A frame that
carries several books yields every one, where the bare tier's `MarketMessage::from_json` keeps the
first and a protected test pins that; `PONG`, `{}`, empty text and any text without an `event_type`
are skipped, and a Binary frame is decoded as text (D12). A frame that does not decode is yielded as
`ClobWsError::Json` and the connection stays; `ClobWsError::Authentication` stops the stream; a
connection used after it closed reconnects (D19), though its class is `InvalidRequest`; and any
other failure reconnects exactly when its class is retriable, with no match over
`tungstenite::Error`. The connection is dialled by clob's per-address `dial` through
`Protocol::connect` (D20), so every address of a blackholed route is tried within `connect_timeout`
per step under supervision too. The URL is `wss://`, or `ws://` to a loopback host, which
`MarketWsBuilder::url` enforces, where the bare `WebSocketBuilder` keeps its `wss://`-only rule. A
change through `MarketMembership`, on the kit's `MembershipHandle<ClobMarketProtocol>`, is recorded
before its frame is sent and sends the ids that change, once each; one whose connection dies under
it is answered `Ok` and replayed, one made during an outage is answered at once and replayed, and a
subscribe past 500 ids is refused as `InvalidMessage` and changes nothing. `clob::ws::scripted`,
built for the unit tests, holds the `PING`/`PONG` answerer the supervised sockets' tests share.

**The supervised CLOB user socket** (Story 7.2) is `UserWsBuilder` and `SupervisedUserWs` in `polyoxide-polymarket`'s `clob::ws::supervised`, beside the market socket and on the same kit `Supervisor`, with `ClobUserProtocol`, `UserEvent`, `UserChange` and `UserMembership`, all at `clob::ws`. Its membership is `Option<Vec<String>>` (AD-11): `None` follows every market and the subscribe frame omits `markets`; `Some(markets)` carries them; `Some(vec![])` sends `"markets": []`, which follows none. Nothing ever turns a `Some` into `None`, so unsubscribing the last market and then reconnecting replays an empty filter and does not widen the subscription to every market (a test pins it). A subscribe on `None` changes nothing and sends nothing, an unsubscribe on `None` is refused as `InvalidMessage`, and on a `Some` each change sends one update frame for the markets it actually changed, at most 500 per connection. A change is recorded before its frame is sent and one made during an outage is answered at once, so the replay carries both. The protocol holds the account's `ApiCredentials` and rebuilds the credential-bearing `UserSubscription` on every connection, so a reconnect replays them; the builder holds none, and `connect` and `start` take them (`start` begins in an outage when the first attempt fails in a way recovery would retry). The queue is `Queue::Unbounded` by the owner's choice (AD-24), so the kit never waits on the consumer and a consumer that stops reading loses no fill, at the cost of memory that grows with its backlog. A server close with code 1008 is how the host refuses credentials (about 1.2 s after the subscribe, with no error frame; probed live 2026-10-17): it decodes as `ClobWsError::Authentication(<the close reason>)`, classed `Unauthorized`, and recovery stops on it, so the stream yields the error and ends instead of reconnecting for ever at the backoff ceiling; it is that variant's first use. Every `UserEvent::Disconnected` is followed by one `UserEvent::Reconnected`, unless the stream ends on an error recovery stops on (a reconnect refused with a status retrying cannot fix, such as 401, 403 or 404) and yields it first, and what the socket carried in between is lost, so a consumer reconciles there. A frame that does not decode is yielded as an error and skipped, a JSON array yields one event per element, and an array with an element that does not decode is skipped whole. The URL must be `wss://`, or `ws://` to a loopback host, so credentials never travel unencrypted to a remote host. Its suites drive the kit's scripted server through `src/clob/ws/scripted.rs`'s `PING`/`PONG` answerer.

**The socket kit is `polyoxide-ws`; perps and Binance run on its `Supervisor` (Stories 4.6 and 4.7), and rtds, sports and clob on its blocks (Stories 4.8 and 4.9).** It holds one copy of each block the five socket clients carried when each was a crate of its own (clob, perps, Binance, rtds and sports; all but Binance are now `polyoxide-polymarket`'s modules): `ensure_crypto_provider`, `Backoff`, `Timing` with `timing_setters!` and `next_wait`, `WsError` read through its `#[non_exhaustive]` `Kind`, `connect`, `connect_over` (the upgrade of a TCP connection a venue dialled itself), `with_timeout` and `retrying_can_fix` (the status rule injected), the bare tier `Connection` (`Mode::Data` or `Mode::All`, the server's close captured and its RFC 6455 reply flushed, DRIFT R3) with request/answer correlation, and the scripted test server. It also holds the generic supervision loop, `Supervisor<P: Protocol>` (Story 4.5, `src/supervisor/`): a venue describes its socket as a `Protocol` (a handshake built per attempt, `connect`, the dial of each attempt, which by default dials the handshake's request and which a venue that tries each address overrides, `configure` of each fresh connection, such as Binance's 200 ms request pacing, `replay` and `apply` of its membership over the connection's request correlation, `decode` of every frame into `Decoded` events, its D5 liveness and D6 delivered predicates, its D4 ping, `wanted`, `recover` to `Recovery::{Reconnect, Skip, Stop}` (by default `Reconnect` exactly when its required `is_retriable` says so), `retry_after`, the wait a failure asked for, which perps and Binance answer from `Classify` (by default `None`), `record` for a change made during an outage (held by default), its D8 outage markers, `disconnected` owning its `Outage` so a marker can carry the error, and its `Queue`), and gets a `Supervised<P>` stream with a `MembershipHandle<P>`. The task pings on the wall clock even while a bounded queue is full (it then reads nothing, and the wait is not silence), drops no decoded event, yields at most one `Disconnected` and exactly one `Reconnected` per outage, closes politely and ends when the membership stops being wanted on a healthy connection, rotates at `max_connection_age`, waits out a refusal's `Retry-After` before the next attempt, never less than the backoff's step and never past its ceiling (`Backoff::take_at_least`, DRIFT R11, which rtds and sports apply at their own reconnect waits), bounds `replay` and `apply` by `stale_after` and replaces a connection whose request outlived it, and ends promptly when the consumer drops the stream; `src/supervisor/tests.rs` pins each rule. It depends on neither core nor venue, re-exports `tungstenite`, and `polyoxide_venue::impl_ws_classification!` classifies its `WsError`. Each venue keeps its own copies until its story moves it: 4.6 moved perps and 4.7 Binance onto the `Supervisor` (4.5) over the kit, Binance with one supervisor per routed path behind a router; 4.8 moved rtds and sports onto its blocks, each keeping its supervision shape (D7): rtds took TLS, `Backoff`, connect with a timeout, `retrying_can_fix`, the bare tier's close reply and the test server, and sports TLS, `Backoff`, `with_timeout` around its own connect, the `Socket` alias and the test server, then in 4.10 the kit's `connect`, the bare tier (`Mode::All`, so pings still count as liveness, D5) and `retrying_can_fix`, behind its own `Capacity` and `AlreadyClosed` (D19) rows in a private recovery function; 4.9 moved clob onto TLS, `WsError`, the bare tier (`Mode::Data`, so a server close is answered, DRIFT R3) and the test server, its per-address connect its own (D20), each step bounded by `with_timeout` and the upgrade through `connect_over`; clob's bare tier keeps its own `select!`, its 10 s text `PING`, its outbound queue and Binary decoded as text (D12); its supervised sockets (Epic 7) run on the `Supervisor` instead, as described above. A per-venue divergence row (D4–D19) stays at the venue; only identical or parametric copies merged.

Three market events — `best_bid_ask`, `new_market`, `market_resolved` — are withheld by the server unless the subscription sets `custom_feature_enabled`. Use `WebSocket::connect_market_with(ids, MarketSubscriptionOptions::default().with_custom_features())` to receive them. `MarketMessage` and `Channel` are `#[non_exhaustive]`, since upstream adds event types over time.

The user channel's market filter is optional: `WebSocket::connect_user_all_markets(creds)` omits it and receives events for every market, and `subscribe_markets` / `unsubscribe_markets` adjust it on a live connection without reconnecting. The market channel changes membership the same way — `subscribe_assets` / `unsubscribe_assets`, or a `MembershipHandle` (from `WebSocketWithPing::membership`, taken **before** `run`) while the ping loop drives the socket. Both frames are documented in the AsyncAPI mirrors (`SubscriptionRequestUpdate`). Verified live 2026-09-09: an added asset gets a fresh `book` in ~155 ms, a duplicate add gets nothing (unsubscribe then subscribe to force a snapshot), and an empty-membership socket stays open under the 10 s `PING` but is reset after ~125 s without it.

The WebSocket contracts are published as AsyncAPI, not OpenAPI — mirrored in `docs/specs/clob/asyncapi-{market,user}.json` and `docs/specs/sports/asyncapi.json`. A parity audit that only diffs the OpenAPI files will miss this whole surface.

**The sports feed is its own module.** `polyoxide-polymarket`'s `sports` module, feature `sports`, covers `wss://sports-api.polymarket.com/ws`, which takes no subscription and pushes every live match. Its feature builds `polyoxide-venue` and the socket kit, `polyoxide-ws`, and not core, `reqwest` or `alloy`, for the same reason as rtds; `test_dep_allowlists.py` holds `@sports` to that. Upstream's AsyncAPI documents a `slug`-keyed payload and a text `"ping"`/`"pong"`; the server sends neither, so `docs/specs/sports/asyncapi.json` carries `x-observed-*` annotations and is excluded from `nightly-schema.yml`. `MatchUpdate` is modelled on the captured frames in `polyoxide-polymarket/tests/fixtures/sports/` (refresh with `scripts/capture_polymarket_sports.py`), which `sports::fixtures` holds for the tests here and in the CLI under `test-server`, and the live test `live_frames_round_trip_and_carry_no_unmodelled_keys` in `tests/live_sports.rs` is the host's drift detector.

**Pings are the sports feed's liveness signal, the opposite of rtds.** The server sends a protocol PING every 15 s, and data only while a match is live, so a quiet hour has no data at all. `SupervisedSportsWs` resets its 45 s staleness timer on any inbound frame, pings included; counting only data would drop a healthy connection every quiet hour. It is a `Stream` state machine with no background task, because no application message is ever sent to this host (only protocol pongs and a close reply), so the perps task shape is not needed. Pongs therefore go out only while the caller polls. A reconnect refused in a way retrying cannot fix (an HTTP status other than 408, 425, 429 or 5xx, or a malformed URL) is yielded as `SportsError::Connect` and ends the stream, instead of looping at the 60 s backoff ceiling with only a WARN to show for it. A private recovery function in `src/sports/supervised.rs` decides (D7 keeps sports off the `Supervisor`): a `Capacity` failure and a connection used after it closed (D19) are retried, and anything else follows `polyoxide_ws::retrying_can_fix` with polyoxide-venue's status rule (`class_for_handshake_status`, which `Classify::is_retriable` reads).

**A sports frame is state, not an event.** The server re-sends unchanged state on a timer (56 of 121 frames in one capture were repeats), and the `ended: true` frame is usually sent once. `Event::Reconnected` tells a caller to reconcile games through gamma's `events?game_id=`; cricket's `metadataGameId` cannot be reconciled. See `docs/specs/sports/OBSERVED.md`. Gamma's half of the join is `Event::game_id`, `teams` (each `Team::ordering` a `HomeAway`), `sport` and `parent_event_id`, none of which `openapi.yaml` lists: they are modelled from the served `Event.json`. One game id can return the game and its child events, so keep the one with no `parent_event_id`. See `docs/specs/gamma/OBSERVED.md`.

**WebSocket TLS needs a nudge.** `reqwest 0.12` (via core) and `alloy`'s `reqwest 0.13` enable `ring` and `aws-lc-rs` on one shared `rustls`, which then installs no default `CryptoProvider`. The kit installs one before connecting (`polyoxide_ws::ensure_crypto_provider`, which its `connect` and `connect_over` call); any code that calls `tokio_tungstenite::connect_async` directly must do the same or it will panic, so a crate that connects through the kit needs no copy. `polyoxide-ws` takes `rustls` from the one `[workspace.dependencies]` entry, which declares `ring` and `std`, so it relies on neither `reqwest` nor `alloy` to turn `std` on; clob once compiled only because they did (DRIFT R5). Binance and `polyoxide-polymarket` (for clob, perps, rtds and sports) declare neither `rustls` nor tokio-tungstenite: the kit owns their transport (Stories 4.6 to 4.10), and Binance's, clob's and sports' live suites open their raw sockets through the kit's `connect` (DRIFT R9).

**RTDS is a separate module and a separate protocol.** `polyoxide-polymarket`'s `rtds` module, feature `rtds`, covers `wss://ws-live-data.polymarket.com`, which multiplexes many topics over one connection under an `action`/`subscriptions` envelope — unlike the CLOB channels, which are one channel per connection. Its feature builds `polyoxide-venue`, which depends on nothing by default (its `decimal` feature, which perps, Binance and, through `market`, clob enable, adds rust_decimal and serde for `positional`), and the socket kit, `polyoxide-ws`, which builds no HTTP or signing stack; it builds no core, `reqwest` or `alloy`, which `test_dep_allowlists.py` holds `@rtds` to, so a credential-free price feed does not pull in `alloy`: `polyoxide-polymarket --no-default-features --features clob-ws` builds 254 crates against core's 126 (unique names in `cargo tree -e normal`), and none of that signing stack is needed to read a price. Two tiers: `Rtds` is a bare `Stream`; `RtdsBuilder`/`SupervisedRtds` adds keep-alive, a staleness watchdog and reconnect-with-resubscribe. Each connect goes through the kit's `connect`, bounded by a connect timeout, 10 s by default (`RtdsBuilder::connect_timeout`, DRIFT R2), and the bare tier reads through the kit's `Connection` (`Mode::Data`), which answers the server's close with the RFC 6455 close reply (DRIFT R3) and logs it under `polyoxide_ws`.

**`full_accuracy_value` does not mean the same thing on every topic.** It is E18 fixed-point on the three Chainlink topics and a **plain decimal** on `crypto_prices` (Binance). Two frames captured a second apart both report BTC at ≈$79,697 with byte-identical payload keys, differing only in that scale — so the two spot payloads are separate types and the scale is never a runtime decision. A test asserting only "the value is a positive Decimal" passes on both and proves nothing; `the_two_spot_topics_do_not_share_a_scale` in `src/rtds/event.rs` is the one that holds this up, and it has been observed failing in both directions.

**Four RTDS behaviours have no counterpart in the docs**, all recorded in `docs/specs/rtds/OBSERVED.md`. A filter with one stray space delivers the backfill and then goes silent forever with no error, which is why `filters` is built by `serde_json` and never accepted as a caller string. One unrecognised topic returns zero frames for *every* topic in the same batch. A Chainlink-spot **snapshot** arrives labelled with the Binance topic, so `correct_mislabelled_spot_snapshot` keys on the symbol format — taking the label at face value files Chainlink prices under Binance, silently. And a backfill is only sent for a **symbol-filtered** subscription; an unfiltered one never receives one, so it cannot re-initialise from a snapshot after a reconnect.

`docs/specs/rtds/` is modelled on captured frames and is deliberately excluded from `nightly-schema.yml` — upstream publishes nothing to diff it against. The documented 5-second `PING` is neither required nor answered, so staleness, not the heartbeat, is the only liveness signal.

## Publishing Order

The order is computed, not written down. `scripts/publish_order.py` reads `cargo metadata` and puts each publishable member (`publish` unset, or naming `crates-io`) after every member it needs on crates.io first: its normal and build dependencies, and any dev-dependency that carries a version, since that one stays in the published manifest. A path-only dev-dependency is stripped at publish time and orders nothing. A cycle, or a normal or build dependency on a `publish = false` member, fails the release before anything is uploaded. Each `tombstones/*/Cargo.toml` comes after every workspace crate; the workspace excludes `tombstones`, or `cargo metadata` could not read them.

**The tombstones.** S2 retired seven crates (`clob`, `data`, `gamma`, `perps`, `relay`, `rtds` and `sports`), and the S2 release, 0.40.0, published each once more from `tombstones/<dir>/` as `polyoxide-<dir>`: a crate with no dependencies whose crate doc, README and one `#[cfg(not(docsrs))] compile_error!` name `polyoxide-polymarket`, the feature and the rename manifest, and which declares its crate's 0.39 features, empty, so a dependency that names one still reaches the error. The switch commit to S3 deleted `tombstones/`. The machinery stays for a stage that retires a crate: the root manifest still excludes `tombstones`, `publish_order.py check-manifests` holds the directory to the crates `TOMBSTONES` lists for the workspace's stage (none for S3, so an absent directory passes), the release publishes any tombstone after every workspace crate, and CI's package job packages each one it finds. No old version is ever yanked.

<!-- generated:begin claude-publish-order -->
Today's order: `polyoxide-venue` → `polyoxide-core` → `polyoxide-ws` → `polyoxide-binance` → `polyoxide-polymarket` → `polyoxide` → `polyoxide-cli`.
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

**The second guard is a repository setting.** The `cargo` and `pypi` environments accept deployments only from `main` (a custom branch policy naming `main` on each), so a run from any other branch, an edited `release.yml` on a feature branch among them, cannot reach the tokens. It is not a file in this repository: `gh api repos/dilettante-trading/polyoxide/environments/<name>/deployment-branch-policies` shows it.
