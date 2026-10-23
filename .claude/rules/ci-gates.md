---
paths:
  - ".github/workflows/ci.yml"
  - ".github/scripts/tests/**"
  - "scripts/{api_removals,test_body_diff,venue_gate,gen_registry,public_api}.py"
  - "ci/**"
  - "**/Cargo.toml"
  - "docs/{s1-removals,s3-removals,s3-migration,rename-manifest,MUTANTS}.md"
  - "docs/rename-map.tsv"
  - "flake.nix"
---

# CI gates in detail

CLAUDE.md lists the nine jobs and the command each runs; this file says what each checks.

## package

`python3 scripts/publish_order.py check-manifests` fails when a publishable crate lacks a
description or a licence (cargo only warns about those), or a tombstone breaks its rules
(`release.md`); then `cargo publish --workspace --dry-run --no-verify --locked`; then
`for m in tombstones/*/Cargo.toml; do [ -e "$m" ] || continue; cargo package --no-verify --manifest-path "$m" || exit 1; done`,
which packages each tombstone without building it (none since S3), since the workspace dry run
cannot see them. The job skips `publish = false` members on its own, and catches a manifest fault
on the PR that introduces it rather than halfway through a release.

## features

1. `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private`: a feature that
   compiles only alongside another fails.
2. Each HTTP module's header pin in its minimal build, one `-p` per command so their features
   never unify: `cargo test -p polyoxide-polymarket --no-default-features --features <m> --test headers_<m>`
   for clob, gamma, data, relay and perps; `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm`;
   `cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions`.
   The pin test derives the HTTP modules from cargo metadata: a venue crate's products whose
   feature builds `polyoxide-core`, so `rtds` and `sports` pin none.
3. The same pins under the umbrella's `full`,
   `cargo test -p polyoxide -p <venue crate> --features polyoxide/full --test headers_<m>`, which
   unifies every module of every venue into the pinned build. The pin test derives this step
   too, and fails unless `full` turns on each module of a crate the umbrella depends on and of
   every published venue crate (only a `publish = false` crate the umbrella does not depend on
   would be exempt; none is).
4. `mock_clob`'s `a_proxy_order_with_no_funder_is_refused_without_gamma` with `--features clob`
   alone, the one build that compiles clob's refusal of a proxy order with no funder (`clob` does
   not imply `gamma`); then `trading_clob` with `--features clob-ws,data`, whose three proxy tests
   refuse without `gamma` and are compiled only there.
5. `cargo check -p polyoxide-polymarket --no-default-features --features <m>,specta` for each HTTP
   module of a crate with `specta` (clob, gamma, data, relay and perps; the test derives them from
   cargo metadata): a module's specta extras (`specta?/chrono`, `specta?/rust_decimal`) are weak
   features that neither `--each-feature` (which builds `specta` alone) nor `--all-features`
   (which turns every extra on at once) builds apart.

## CI scripts (`.github/scripts/tests/`)

- `test_dependency_fences.py` fails when a member that declares a `venue` depends directly on
  `governor` (a venue crate paces its requests with core's throttle primitives), and when a member
  depends directly, in any dependency kind, on a crate another member owns: `reqwest` is
  `polyoxide-core`'s; `tokio-tungstenite` and `rustls` are `polyoxide-ws`'s. Each owner must
  declare its crate, so the check cannot pass vacuously.
- `test_classify_coverage.py`: `errors.md`.
- `test_dep_allowlists.py` fails when
  `cargo tree -p <crate> -e normal --target x86_64-unknown-linux-gnu`, with the features the file
  names (and `--no-default-features` for `polyoxide-polymarket`, `polyoxide-binance`,
  `polyoxide-kalshi` and `polyoxide`, whose files name every feature they build; the socket kit's
  bare file is its default build), differs from `ci/dep-allowlists/<crate>[@<features>].txt`,
  naming each crate that joined or left the tree; or when an allowlist admits a crate its
  deny-list refuses, naming the rule. After a deliberate change, regenerate the files with
  `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` and say why in the
  commit. The deny-lists (NFR2), applied after a file's features turn on the ones they imply:
  - `polyoxide-ws`, the socket-only deny-list: `reqwest`, `alloy*`, `polyoxide-core`, `governor`,
    `hmac`, `sha2`, `keyring` and `polyoxide-venue`.
  - `polyoxide-polymarket`: `reqwest`, `polyoxide-core`, `governor` and `sha2` only with an HTTP
    module (`clob`, `gamma`, `data`, `relay` or `perps`), which holds the socket modules `rtds`
    and `sports` to the socket-only tree; `alloy*` and `hmac` only with `clob` or `relay`;
    `keyring` only with `keychain`. Its files are `@gamma`, `@data`, `@perps`, `@perps-ws`,
    `@clob`, `@clob-ws`, `@relay`, `@clob,keychain`, `@rtds`, `@rtds,test-server`, `@sports` and
    `@sports,test-server`; `@perps` must hold `reqwest`, `@clob` and `@relay` `alloy`, and
    `@clob,keychain` `keyring`, so no rule can pass vacuously.
  - `polyoxide-binance`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with
    `usdm-ws` (files `@usdm` and `@usdm-ws`; `@usdm-ws` must hold `polyoxide-ws`). `rustls` is not
    on the rule, since core's HTTP client builds it for TLS.
  - `polyoxide-kalshi`: `reqwest`, `polyoxide-core`, `governor` and `sha2` only with
    `predictions`, which every file names; `alloy*`, `hmac` and `keyring` never; `polyoxide-ws`,
    `tokio-tungstenite` and `tungstenite` only with `predictions-ws` (files `@predictions` and
    `@predictions-ws`; the latter must hold `polyoxide-ws`).
  - `polyoxide`, per feature on its own names: the HTTP stack only with an HTTP module of any
    venue (`polymarket-clob`, `-gamma`, `-data`, `-relay`, `-perps`, `binance-usdm` or
    `kalshi-predictions`); `alloy*` and `hmac` only with `polymarket-clob` or `polymarket-relay`;
    `keyring` only with `keychain`. `@polymarket-rtds` and `@polymarket-sports` are the
    socket-only tree, `@polymarket-perps` and `@kalshi-predictions` must hold `reqwest`, and
    `@kalshi-predictions-ws` holds the socket kit.
- `test_module_readmes.py` fails when a venue crate's `src/<product>/README.md` is not included
  exactly once under `cfg(all(doctest, feature = "<product>"))`, when such an include names no
  file, or when a module directory has no README and no entry in its `WITHOUT_README` (only
  `rtds`, whose guide is its `mod.rs` docs); an entry fails once its module has a README.
- `test_venue_gate.py` runs `scripts/venue_gate.py` (CAP-7) on the real tree, and on v0.39.0's
  core, which it must catch, so the job checks out every tag.
- `test_claude_rules.py` holds `.claude/rules/` to its contract (CLAUDE.md, "Where the rules
  live").
- `test_gen_registry.py`, `test_spec_docs.py` (the endpoint counts CLAUDE.md and the spec indexes
  state) and the rest guard the scripts and the generated or counted text.

## The venue gate

`scripts/venue_gate.py` reads every file under `polyoxide-venue`, `-core`, `-ws` and
`-test-support`, case-insensitively, for:

- each venue id, as a substring;
- each product id, as an identifier segment (split at `_`, `-`, `.`, `::` and case boundaries, so
  `clob` in `clob_default` and `perps` in `PERPS_GENERAL`), less the generic words of
  `ci/venue-gate-skip.txt` (`data` and `events`);
- each `<venue>.<product>:` prefix;
- each venue's `identifiers` (Polymarket's `Poly-RateLimit`; Binance's `fapi`, `fstream` and
  `X-MBX`, its hosts and weight header);

all read from cargo metadata. A hit passes only through a `gate_exceptions` entry,
`{ path, term, reason }`, in the metadata of a crate of the hit's venue, and an entry that matches
nothing fails, so an exception never outlives what it excuses. Only a protected test name, or a
probe that measured one venue's host, takes an exception, in that venue's crate (today the ws
suites' `mod perps`, `mod binance` and their kin, excepted by `polyoxide-polymarket` and
`polyoxide-binance`).

## removals

`python3 scripts/api_removals.py check --baseline v0.40.0` runs cargo-semver-checks with
`--release-type patch` and fails on any reported removal that the listing for the workspace's
stage does not hold, printing each one's key to copy there.

- `check` and `release` read the listing `LISTINGS` names (`docs/s1-removals.md` in S1,
  `docs/s3-removals.md` in S3); `check` refuses stage S2. A key is
  `<crate> <lint id>: <item> (<file>)`; a deleted crate is `<crate> crate_missing`.
- cargo-semver-checks cannot see `#[doc(hidden)]` items or type aliases, so the job also imports
  each path the listing names under "Doc-hidden paths consumers import", in one scratch crate per
  crate and feature set, so no entry compiles through another's features. A story that removes
  one of those paths marks it `**Removed**` in the same commit.
- `docs/s3-migration.md` holds S3's consumer-impact notes, which the first S3 version-bump commit
  quotes in CHANGELOG.md.
- Final and no longer written: `docs/s1-removals.md`; S2's `docs/rename-manifest.md` (rows mapping
  every path, feature and CLI command that moved or went from `v0.39.0`) and `docs/rename-map.tsv`
  (the map `api_removals.py manifest --expand` wrote at the S2 release). No job runs `manifest`.
- cargo-semver-checks 0.51.0 and Rust 1.99.0 are pinned together in `ci.yml`, `release.yml` and
  `flake.nix`, because the tool reads only the rustdoc JSON some toolchains emit (1.93 to 1.99):
  upgrade both at once, in all three. `flake.nix` takes cargo-hack and cargo-semver-checks from
  nixpkgs, so check their versions in the shell after a `nix flake update`.
- `release.yml`'s `semver` job runs the same tool before anything publishes (`release.md`).

## test bodies (AD-12)

`python3 scripts/test_body_diff.py --baseline v0.40.0` (CI's one `S3_BASELINE`).

- **What it reads**, at the baseline and at the commit: every test-context item (fns, consts,
  statics, structs, enums, unions, type aliases, `extern` items, `macro_rules!` and item-level
  macro invocations, and the methods, consts and types of `impl` and `trait` blocks) in
  `tests/**`, `[[test]]` and `[[bench]]` targets, `[[example]]`s that set `test = true`, the files
  a test-context `mod` declares, modules and files whose `cfg` names `test` outside `not(...)`
  (`#[cfg(test)]`, `cfg(any(test, …))`, `#![cfg(test)]`), and anywhere under a test attribute.
  Also each `[[test]]` and `[[example]]` stanza (every field but `name` and `path`;
  `required-features` mapped through the manifest's `## Features` rows) and each non-Markdown
  file under a `tests/fixtures/`, by its blob.
- **Gates:** each item is compared with what gates it: the attributes of the `mod x;` that
  declares its file, the file's inner `cfg`/`cfg_attr`, and the attributes of each inline module
  around it, so a test switched off by one of those shows. A gate on an ancestor file module is not
  read: `lib.rs`'s `#[cfg(feature = "gamma")] pub mod gamma;` declares only `src/gamma/mod.rs`, so
  it gates the inline tests of the files under `src/gamma/` unseen.
- **Pairing:** by file (`git diff -M30%`, plus `[files]` in `ci/test-body-diff.toml`), then by a
  unique module path, kind and name.
- **Normalising** both sides before comparing attributes (doc comments aside), signature and body:
  comments and `use` statements dropped; a library's `crate::` as its name; the manifest's path
  rows, then the toml's `[paths]` (for a move into code that is not public, which no manifest row
  names), applied to the old side; a bare identifier renamed only at the head of a path;
  `include_str!`/`include_bytes!` targets as blob ids; the `[fixtures]` table on the old side (a
  baseline `fixtures!()` read as `fixtures!("<subdir>")` and `fixtures!("x")` as
  `fixtures!("<subdir>/x")` under a scope, for a crate whose fixtures moved under
  `tests/fixtures/<module>/`, applied before `[literals]`); the `[literals]` table on the old side;
  the `[gates]` table on the new side (under a path prefix, each feature's
  `cfg(feature = "<m>")`, or each whole predicate such as
  `any(feature = "clob", feature = "relay")`, dropped where it is a `cfg`'s whole predicate or an
  operand of its top-level `all(...)`, and each feature from `required-features`: for code that
  moved behind a module's features, such as a crate root that becomes `src/<m>/mod.rs`, or core's
  files in `shared`); whitespace collapsed; rustfmt's re-wrapping under longer paths ignored (a
  trailing comma before a closing bracket or a generic list's `>`, except in a one-element group
  that is not a call's, and braces around a lone expression after `=>` or a closure's `|…|`).
- **Fails** on a changed item unless a `[[drift]]` entry names a DRIFT row of
  `_bmad-output/specs/spec-venue-extensibility/divergences.md` and the diff's sha256; on a lost
  item unless a `[[retired]]` entry names it and an old path whose covering manifest row gives a
  reason; and on any `[files]`, `[fixtures]`, `[literals]`, `[paths]` or `[gates]` entry that
  pairs or rewrites nothing.
- **The 25 `[[drift]]` entries:** `mock_gamma`'s `get_many_fans_out_closed_true_and_false`
  (DRIFT R13); `spec_agreement_data_v2`'s `NOT_MODELLED`, `ROUTES`, `SPEC` and `agreement!#1`,
  which adopting a spec change moves (R14); `polyoxide-ws`'s scripted test server's
  `struct Recorder`, `fn handshake` and `fn serve`, changed to record each handshake's request
  headers (R15); `enum_wire_data_v2`'s
  `enums_that_also_appear_in_responses_spell_what_the_server_sends`, whose `ActivityType` set
  gained `MIGRATION` (R16); and sixteen perps items that adopting its drift (#56) moved, among them
  `spec_agreement_perps`'s and `wire_agreement_perps`'s tables, four `mock_perps` tests,
  `perps::types`' enum test and the `leaderboard.json` and `leaderboard_account.json` fixtures
  (R17), seven of which adopting its builder-fee drift (#76) moved again and now cite R18.
- The manifest's rows still normalise the old side; their baseline is `v0.39.0`, so the script
  warns that they map paths from another tag, which is harmless at `v0.40.0`.
- git pairs only tracked files, so **stage a moved file before running it locally**. `--moves`
  prints each moved item's old `file:line name` and new `file:line`, for `docs/MUTANTS.md`.
  README and doc-comment doctests, and Python tests, are not compared.
