# Epic 5 Context: One crate per venue, multi-venue facades

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Consumers find each venue under one name. `polyoxide-polymarket` holds clob, gamma, data, relay, perps, rtds and sports as feature-gated modules, and `polyoxide-binance` holds usdm. On top sit a multi-venue `polyoxide` umbrella and a `polyoxide <venue> <module> <verb…>` CLI. The Python bindings keep v0.39.0's module names. The foundation crates carry no venue code, and the seven retired crates point to their new home. This is stage S2, the renames stage. Every public rename or move ships in this one release, so prader-rs rewrites its imports once.

**Where things stand.**
- S1 shipped on 2026-10-12 as v0.39.0 (release commit 3da0acb). `polyoxide-venue`, `polyoxide-ws` and `polyoxide-cli` were published for the first time, and the workspace stage is S2.
- Branch `aidanb/restructure-3` is the S2 integration session. It was cut from `main` at b7b3fed, which is v0.39.0 plus the stage update.
- Every story merges into that branch, and nothing reaches `main` until Story 5.12 merges it in one merge.
- Lane B's S3 groundwork (`aidanb/restructure-2`, tag `lane-b-bundle-n`, eabcd1f) is out of scope.

## Stories

- Story 5.1: Open the S2 integration session
- Story 5.2: polyoxide-polymarket and a venue-neutral core
- Story 5.3: Gamma and data become modules
- Story 5.4: clob and relay become modules
- Story 5.5: perps, rtds and sports become modules
- Story 5.6: Binance follows the conventions
- Story 5.7: The multi-venue umbrella
- Story 5.8: The CLI on the venue axis
- Story 5.9: CLI credentials over a shared trait
- Story 5.10: Python on the venue axis (dropped on 2026-10-15)
- Story 5.11: Docs and standing rules for a multi-venue toolkit
- Story 5.12: The S2 release

## Requirements & Constraints

- **No shims.** A move breaks the old path, with no re-export, alias or compatibility module.
  - This covers crates, modules, features and the CLI command tree.
  - Only crate, module and command names change. The `polyoxide` brand stays.
- **The rename manifest is S2's definition of done.**
  - `docs/rename-manifest.md` gives every removed or moved public item one row: old path → new path, or old path → reason.
  - It is checked in and built from a public-API diff. It is the S2 migration guide, and the release notes link to it.
  - Story 5.12 does not merge until every entry has a row.
- **Behaviour does not change (AD-12, NFR7).**
  - **What a move may change.** Only file locations and `use` paths. Test function names and assertions stay.
  - **Body diffs.** Each moving PR shows an empty normalized diff of the moved test bodies, from `scripts/test_body_diff.py`. CI fails on a non-empty diff that has no DRIFT reason.
  - **One DRIFT row.** DRIFT R6 is S2's only DRIFT row: rtds' `test-fixtures` feature becomes `test-server`. It goes in its own commit, named for the row (Story 5.5).
  - **Divergence rows.** Every row in `divergences.md` still binds, and D13's two backoffs are never unified.
  - **Protected suites.** Every moved test is protected. Among them:
    - the supervision suites: perps inline (21); Binance `supervision.rs` (21), `supervision_edges.rs` (7) and its inline tests; rtds (4 + 15 inline); sports (15 + 6 bare + 6 inline);
    - `documented_*_limits` and the signer tests;
    - the limiter, cooldown and `Retry-After` rules;
    - `classify_order_kill`;
    - the EIP-712 and session-key golden vectors;
    - spec and wire agreement, with their allow-lists and stale-excuse checks;
    - the live drift detectors;
    - the Python stub and getter guards;
    - the classifier's tag table and `test_diff_openapi`.
  - **Counts.** Each PR reports per-suite test counts before and after.
  - **Mutants.** Each `docs/MUTANTS.md` row's file:line and failing tests move with the code. Every mutant must still fail its moved test, which 5.12 checks.
  - **Protected names keep their venue words.** Some protected test names contain product ids, such as `test_clob_default_construction` in core's `tests/polymarket_limits.rs`. Those tests leave the foundation; they are never renamed.
- **The credential-free fence is checked per feature from S2 (AD-2, NFR2).**
  - **rtds and sports.** `-F rtds` alone and `-F sports` alone each build only three sets of dependencies: `polyoxide-ws`'s, `polyoxide-venue`'s and the module's own. `polyoxide-venue`'s default build has none, and its `decimal` feature adds rust_decimal and serde.
  - **Deny-lists.** A socket-only feature never builds reqwest, alloy\*, polyoxide-core, governor, hmac, sha2 or keyring. No credential-free feature builds alloy\*, keyring or hmac. alloy is built only with `clob` or `relay` (5.4).
  - **Gating in the venue crate.** `polyoxide-core`, `alloy` and every `src/shared/` item are gated by the features that use them.
  - **The tool today.** `test_dep_allowlists.py` passes `--features` without `--no-default-features`, and it applies the socket-only deny-list to every file. Per-feature files on a crate whose defaults are clob, gamma and data need both changed.
- **Transport isolation (AD-18, FR10).**
  - Each venue module's header-pin test runs in three builds: the venue crate alone with minimal features, the workspace with all features, and, new in S2, `polyoxide --features full`.
  - Only core declares reqwest, and only ws declares tokio-tungstenite and rustls. No builder sets `gzip`.
- **Registration (AD-13).**
  - **What moves.** Metadata entries move with their code: venue, products, the README line, mirrors, and `live.<target>` with its declared secrets. The schema stays.
  - **What stays fixed.** Spec ids, the `docs/specs/` layout, the `spec:<id>` labels and the drift acknowledgements.
  - **Generated regions.** They change only through `python3 scripts/gen_registry.py --write`, and the scripts job fails on a stale region.
  - **Two missing keys.** AD-13 names `gate_exceptions` and `identifiers`, but no manifest or script carries either yet. Story 5.2 adds them.
- **Standing rules (AD-21).**
  - A change that makes a CLAUDE.md rule false edits that rule in the same change, inside the rule's own `##` section.
  - The rules S2 supersedes:
    - the `tests/live_api.rs` and `mock_api.rs` names;
    - Module Organization;
    - Binance outside the umbrella;
    - clob's `ws` feature;
    - the `polyoxide.v2` namespace.
  - `docs/ARCHITECTURE.md` is only ever regenerated from the spine.
- **Gates (NFR9, AD-22).** CI runs on every push to the draft PR. A red job on `main` withholds the release. The gates:
  - clippy and rustdoc with `-D warnings`, so a `pub` doc must not link a `pub(crate)` item, and `src/shared/` makes many;
  - fmt and MSRV 1.91;
  - `cargo hack --each-feature`;
  - the publish dry run;
  - the removals job;
  - the scripts job: allowlists, dependency fences, classify coverage and generated regions;
  - the Python tests.
- **Release (NFR1, NFR11, NFR12, AD-25).**
  - **The version bump.** Never on the integration branch. It is a separate commit on `main` after the merge, made after `git fetch` and a crates.io check, and it is the last commit before the tag. It must raise the 0.x minor, because release.yml's semver job fails on a deleted crate otherwise.
  - **Release notes.** They carry a consumer-impact section, link the manifest, and list every moved tracing target (for example `polyoxide_clob` → `polyoxide_polymarket::clob`).
  - **The retry log line stays.** `Retriable status <code> on <path>, retry <n> after <ms>ms`, under `polyoxide_core`, is kept, because the soak harnesses match it.
  - **One new crate name.** `polyoxide-polymarket` is new (the limit is five per release), and the crates.io token must be able to publish it.
  - **No yanks.** No old version is ever yanked.

## Technical Decisions

- **Crate graph (AD-1).**
  - A venue crate depends only on `polyoxide-core`, `polyoxide-ws` and `polyoxide-venue`, and venue crates never depend on each other.
  - The facades:
    - the umbrella depends on `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide-venue`;
    - the CLI depends on the two venue crates and `polyoxide-core`;
    - `polyoxide-py` depends on `polyoxide-polymarket` and `polyoxide-venue`.
  - `polyoxide-test-support` stays a path-only dev-dependency that never names a venue.
  - Anything published behind `test-server` is built only from published crates.
- **The venue-crate layout.**
  - **Directories.** Each module is `src/<module>/{api,ws,types,error,venue}`, with either `api.rs` or an `api/` directory. Venue-private code shared by modules goes in `src/shared/`, feature-gated.
  - **Features.**
    - Each module has a `<module>` feature. A module with both transports also has `<module>-ws`: `clob-ws` and `perps-ws` here, `usdm-ws` for Binance.
    - There is one `test-server`, additive per module through `cfg(all(feature = "test-server", feature = "<module>"))`.
    - `specta`, `keychain` and `parquet` keep their names.
    - Defaults are `clob`, `gamma` and `data`. `clob` stops implying `gamma`, so clob's gamma helpers are gated `all(clob, gamma)`.
    - docs.rs lists every module and `-ws` feature, and not `test-server`.
  - **Errors.**
    - **Naming.** One `#[non_exhaustive]` enum per module per transport tier, named `<Module>Error` or `<Module>WsError`, each implementing `Classify`.
    - **The renames:**
      - `DataApiError` → `DataError` (5.3);
      - `WebSocketError` → `ClobWsError` (5.4);
      - `BinanceError` → `UsdmError` (5.6);
      - `PolymarketError` is removed, and the `Polymarket` builder returns a `BuildError` (5.7).
    - **Names that stay.** The other enums already fit the convention.
    - **Socket errors.** Every enum that wraps `WsError` still expands `impl_ws_classification!` and calls all three of its functions.
  - **Test targets.** `live_<module>[_<suite>]`, `mock_<module>`, `supervision_<module>[_edges]`, `wire_agreement_<module>[_ws]` and `spec_agreement_<module>`. Each declares `required-features`, and every live test is `#[ignore]`.
  - **Drift detectors.**
    - Fixtures live at `tests/fixtures/<module>/`, with a `PROVENANCE.md`.
    - `OBSERVED.md` stays at `docs/specs/<host>/`.
    - Capture scripts are `scripts/capture_<venue>_<module>.py`, built on `capture_common.py`.
  - **Module docs.** Each module's `src/<module>/README.md` is a doctest under `cfg(all(doctest, feature = "<module>"))`, linked from the crate README. CI is meant to check that the README count equals the include count; that check does not exist yet.
- **Polymarket code leaves core (5.2, AD-16, AD-17).**
  - **What moves into `src/shared/`.**
    - core's `polymarket` module: the limit tables, `ClobThrottle`, `PolymarketRetryPolicy` with its 425 rule, and `signer_cost`;
    - the rest of core's Polymarket code: the HMAC `Signer`, the session-signer types, the signer layer and its refusal type, and the `Poly-RateLimit-*` handling.
  - **What stays in core.** The venue-neutral primitives: the send loop and hooks, `WindowQuotaTable`, `CapacityBucket`, `Hold` and the keychain.
  - **One retry policy.** It still serves every Polymarket HTTP module.
  - **The tables.** They keep `documented_*_limits` tests that assert the effective quota.
- **The CAP-7 gate (5.2).**
  - **What it greps.** `polyoxide-{venue,core,ws,test-support}`, case-insensitively, for:
    - venue ids, as substrings;
    - product ids, as identifier segments split at `_`, `-`, `.`, `::` and case boundaries, minus a checked-in skip list of generic words (`data`, `events`);
    - `venue.product:` prefixes;
    - each venue's extra `identifiers` (for Polymarket, `Poly-RateLimit`).
  - **Exceptions.** They live only in each venue's own `gate_exceptions`, and foundation docs name no venue.
  - **What it must catch today.** `clob_default`, `PERPS_GENERAL` and the `Poly-RateLimit-*` headers.
  - **Residue outside core.**
    - `polyoxide-ws` names Binance in about 13 files, mostly in docs, tests and examples.
    - `polyoxide-venue` names Polymarket in its Cargo keywords, and Binance in three doc comments, D14's 403 among them.
    - test-support was kept venue-free in S1 for this gate.
- **The umbrella (5.7).**
  - It exposes `polyoxide::{venue, polymarket, binance, prelude}`.
  - Its features are `polymarket` (the default), `polymarket-<module>` passthroughs, `binance`, and `full`. `full` covers every module of every venue, including `-ws` and `keychain`.
  - The prelude re-exports the enabled venues, including traits that call sites need, such as `PerpsMembership`.
  - The unified client moves to `polyoxide_polymarket::Polymarket`.
  - The root README stays a doctest.
  - `parse_key` belongs to S3's keys.
- **Tombstones (5.12, AD-16, AD-25).**
  - **Which crates.** The seven retired ones: clob, gamma, data, relay, perps, rtds and sports.
  - **What each is.** One crate under `tombstones/<crate>/`, outside the workspace (the root `Cargo.toml` already excludes `tombstones`). It is published at the S2 version and has no dependencies and no re-exports. It holds only a pointer and a `compile_error!` gated `#[cfg(not(docsrs))]`, naming `polyoxide-polymarket`, the feature and the manifest.
  - **Already built in S1.** `publish_order.py` and `finish_release.sh` already publish `tombstones/*` after every workspace crate, by `--manifest-path` and with `--no-verify`, and the generator never sees them.
  - **What CI checks.** Only their manifests: no dependencies, and the S2 version.
  - **After the release.** The next commit on `main` deletes `tombstones/`.
- **The removal gate in S2 (5.1, AD-22).**
  - **Today.** CI's removals job runs `api_removals.py check --baseline v0.38.1` against `docs/s1-removals.md`, which is final and whose front matter says `baseline: v0.38.1`.
    - `check` fails if the listing's baseline differs from `--baseline`.
    - `test_ci_workflow.py` pins `S1_BASELINE`, the exact command and the cache key.
  - **What 5.1 changes.**
    - The gate runs against v0.39.0 and reports only. It writes its findings into `docs/rename-manifest.md` and fails nothing.
    - That pinned test changes in the same change.
    - From S3 on, the gate fails on any removal, with the S2 tag as its baseline.
  - **Blind spots the manifest must cover by hand.**
    - A retired crate is one `<crate> crate_missing` key, and a crate absent at the baseline (`polyoxide-polymarket`) is `--exclude`d. So for seven crates the per-item old → new rows cannot come from cargo-semver-checks.
    - cargo-semver-checks also misses doc-hidden items and type aliases. Those are the fixture modules, perps' `*_for_tests`, the three `DynSigner` aliases, `PageStream` and `SupervisedPerpsWs`.
    - It may miss the umbrella's cross-crate re-exports (see Deferred work).
  - **The compile test.** Both `check` and `release` (release.yml's semver job) compile `s1-removals.md`'s live doc-hidden and alias paths. Every one sits in a crate S2 retires or moves, so S2 needs its own listing for them. Otherwise the first crate move breaks the removals job, and the release fails at 5.12.
  - **Running it locally.** CI pins cargo-semver-checks 0.51.0 with Rust 1.99.0, since the tool reads only the rustdoc JSON that Rust 1.93–1.99 emit. Never run it, or cargo-hack, while another process edits manifests: cargo-hack rewrites every `Cargo.toml`.
- **The body-diff tool (5.1).** Test bodies name crates and error types in many qualified paths outside `use` lines: over 100 lines name `polyoxide_clob::` in clob's tests, and `DataApiError` appears 29 times in data's tests. The normalizer must map old paths and renamed identifiers to their new names before it compares, or every move will report a diff.

## UX & Interaction Patterns

- **CLI shape (5.8).**
  - Commands are `polyoxide <venue> <module> <verb…>`, and nested verbs are allowed: `clob prices download` becomes `polymarket clob prices download`.
  - Every socket streams through `stream`, with one shared runner (count, deadline, broken pipe, feed ended, `excerpt()`) and one `OutputFormat`. market, user and prices drop their untestable ctrl-c + `select!` loops.
  - No copy of the runner, `OutputFormat` or the stream tests (C1–C3) remains outside the CLI's shared module.
  - Every list flag uses `value_delimiter` and has a parse test that passes it. A value parser that returns a `Vec` panics when the field is read.
  - `ws_sports`, `ws_binance` and `data_v2` keep their assertions on the new paths.
- **CLI credentials (5.9, NFR6).**
  - **The command.** `polyoxide <venue> credentials <store|show|delete> <kind>`, generic over a `StoredCredential` trait in core's keychain. The trait covers the service, the field names, which fields are secret, and validation.
  - **Who owns what.** Secret fields are held in `polyoxide_venue::Secret<T>`. Shared code owns keyring access, redaction and the plumbing. Each venue owns its kinds and service names; Polymarket's kinds are `clob` and `builder`.
  - **What must hold.**
    - `show` never prints a secret field in clear.
    - The existing service names, `polyoxide-clob` and `polyoxide-relay`, keep working.
    - Parse tests cover every kind and verb.
- **Python (5.10): dropped.**
  - The user dropped it on 2026-10-15, and spine amendment A5-1 records it. No Python consumer has asked for per-venue modules.
  - The bindings keep v0.39.0's module names: `polyoxide`, `polyoxide.v2` and the `Clob*` classes. They already build on `polyoxide-polymarket`.
  - Their guards keep running in CI unchanged: `test_stub_consistency.py`, `every_v2_getter_reads_its_own_key` and the offline suites.

## Cross-Story Dependencies

- **Order within Epic 5.**
  - 5.1 comes first: the draft PR from `aidanb/restructure-3` to `main`, the report-only gate and `test_body_diff.py`.
    - `ci.yml` already runs on every pull request to `main`. release.yml runs only on a push to `main`, so the session cannot release.
  - 5.2 creates `polyoxide-polymarket` and turns the gate on. 5.3, 5.4 and 5.5 move crates into it.
  - 5.4's `all(clob, gamma)` gating needs gamma's module from 5.3.
  - 5.6 is independent of the Polymarket moves.
  - 5.7 and 5.8 follow the venue moves. 5.9 builds on 5.8's command tree. 5.11 follows everything, and 5.12 is last.
  - Every move updates the CI-script tests that name crates or error types: classify coverage, dependency fences, the allowlists and `live_unwraps.baseline.json`'s per-file keys. Update the names, and never weaken their anti-vacuity assertions.
- **Nightly (5.11, 5.12).**
  - Renamed live targets change the derived nightly job names, so issues whose test identity changed are closed or renamed.
  - `nightly-behavioral`'s issue step does not check the ref. A dispatch on the integration ref comments on, or closes, `main`'s one `nightly-behavioral` issue.
- **Deferred work that lands in Epic 5:**
  - **5.1 or 5.7.** cargo-semver-checks 0.51.0 may not report removed cross-crate re-exports, which is most of the umbrella's API; this is unverified. Settle it by removing one `pub use` in a scratch tree. If the removal goes unreported, list the umbrella's paths by hand.
  - **5.2.** S1 kept `RateLimiter`'s name; any rename goes in the manifest.
  - **5.3 or 5.5 (a candidate, not a commitment).** The gamma and sports `game_id` join uses `i64` setters, `u64` fields and a `String` on `Market`. S2 is the natural point to align them, as a breaking change with a manifest row.
  - **5.4.** clob's `WebSocketError::{ConnectionClosed, Authentication}` are never constructed, and they may be pruned at the `ClobWsError` rename.
  - **5.6.** Make `WindowQuotaTable`'s clock injectable, then move Binance's funding bucket onto it. Its six protected `weight.rs` tests run on tokio's paused clock and must survive.
  - **5.9.** Put `Secret` on the credential fields themselves; S1 put it only on the holders.
  - **5.12.** Extend the package job to dry-run each tombstone with `--manifest-path`, since the workspace dry run cannot see them.
- **Prerequisites and later epics.**
  - Epics 1–4 are on `main` and released.
  - Lane B (S3) stays off `main` until 5.12 ships, and its `polyoxide-venue` work must pass the CAP-7 gate. Its venue-crate halves wait for S2's crates: 6.2's constructors, 6.3's Binance impl, 6.4, 6.5 and `parse_key`.
  - Epic 7's supervised clob sockets build on `clob-ws` and `ClobWsError`.
- **Unmerged amendments.**
  - A2-1, A3-1 to A3-5 and A4-1 to A4-4 are accepted but not merged into the spine, and no story is assigned to merge them.
  - A4-2 claims that an unused classification function fails `-D warnings`. That claim is false. The guard is `test_classify_coverage.py`, and the amendment must be corrected when it is merged.
  - 5.11 regenerates the guide from whatever the spine then says.
