---
title: 'Story 5.6: Binance follows the conventions'
type: 'refactor'
created: '2026-10-14'
status: 'in-progress'
route: 'dispatch'
baseline_commit: '1850608eba3dd6b12dc9c70af467b120e5f7337a'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-5-perps-rtds-and-sports-become-modules.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `polyoxide-binance` is laid out unlike `polyoxide-polymarket`. Its error is `BinanceError` at the crate root and `weight` sits beside `usdm`. Its streams are behind `ws`, and its suites and fixtures carry no module name. The CAP-7 gate also cannot see Binance's host words in the foundation crates.

**Approach:** Move `usdm`'s error and weight budget under `src/usdm/`, rename `BinanceError` to `UsdmError`, and replace `ws` with the features `usdm` and `usdm-ws`. Rename the suites and fixtures to the `_usdm` convention, all with empty body diffs, and give the module its README. Declare Binance's gate identifiers.

**Decisions:**
- **Paths:**
  - `polyoxide_binance::{error,weight}` become `polyoxide_binance::usdm::{error,weight}`.
  - `BinanceError` becomes `usdm::UsdmError`.
  - The crate root keeps only `pub mod usdm`, as polymarket's does. `Usdm`, `UsdmBuilder`, `UsdmError` and `WeightBudget` are re-exported from `usdm`, not from the root.
  - The log target `polyoxide_binance::error` becomes `polyoxide_binance::usdm::error`. A filter on `polyoxide_binance` still matches.
  - No `venue` submodule until S3 (6.3).
- **Features:**
  - `default = ["usdm"]`, so today's default build is unchanged.
  - `usdm` builds core, `polyoxide-venue/decimal` and the module's own dependencies, which become optional.
  - `usdm-ws = ["usdm", polyoxide-ws, futures-util, tokio/rt, tokio/macros]` replaces `ws`.
  - `test-server = []`, with fixtures staying `cfg(any(test, feature = "test-server"))` inside `usdm::ws`.
  - docs.rs builds `usdm` and `usdm-ws`.
  - The `const _` block is gated `usdm`, with `usdm-ws` inside.
- **Test targets:**
  - `ban_log_usdm`, `error_classes_usdm`, `failure_tags_usdm`, `headers_usdm`, `live_usdm`, `live_usdm_ws`, `mock_usdm`, `query_setters_usdm`;
  - `supervision_usdm`, `supervision_usdm_edges`, `supervision_usdm_kit`;
  - `wire_agreement_usdm`, `wire_agreement_usdm_ws`.
  - Each declares `required-features`. The live suite key is `usdm`. `weight_probe` keeps its name and requires `usdm`.
- **Fixtures:** `tests/fixtures/{rest,ws,PROVENANCE.md}` move to `tests/fixtures/usdm/`.
- **Capture script:** `capture_binance_fixtures.py` becomes `capture_binance_usdm.py`, keeping its PEP 723 header.
- **README:**
  - The crate README's usage moves to `src/usdm/README.md`, included under `cfg(all(doctest, feature = "usdm"))`.
  - The crate README becomes an overview that links it, as polymarket's does.
- **Gate identifiers:**
  - `identifiers = ["fapi", "fstream", "X-MBX"]`.
  - The four `/fapi/v1/ping` literals in core's protected `tests/health.rs` become a neutral path. A `[literals."polyoxide-core/tests/health.rs"]` entry pairs them, following 5.2's example-host precedent. A literal in a test body is not a test name, so it takes no exception.
- **The funding bucket stays deferred to S3.** Moving it onto a `WindowQuotaTable` would mean replacing governor's `until_ready`, which waits on Quanta and futures_timer. That changes how every Polymarket limiter waits, which AD-12 forbids in S2. SPINE:234 and D3 keep the bucket in Binance, and its six paused-clock tests are protected.

## Boundaries & Constraints

**Always:**
- No shims.
- `test_body_diff.py --baseline v0.39.0`:
  - reports 0 diffs and 0 lost, with every entry firing;
  - has no new `[[drift]]` entries;
  - never sees an assertion or a test name edited.
- Per-binary test counts match the baseline's.
- `cargo hack check --each-feature` passes, and so does `cargo check -p polyoxide-binance --no-default-features --features usdm`.
- Every removed path and feature has a manifest row: `polyoxide-binance/ws` → `usdm-ws`, plus the doc-hidden `fixtures` entry.
  - `manifest --strict` exits 0.
  - The manifest's 5.6 note names the moved log target and that `test-server` no longer turns on the streams.
- AD-21: every CLAUDE.md rule naming a moved path, test, script, feature or `BinanceError` is rewritten in its own section.
- `docs/MUTANTS.md` rows (a), (b) and (k) are re-cited and re-proved.
- The CI-script tests follow the new names without weakening anti-vacuity.

**Never:**
- touch Polymarket's modules, the umbrella (5.7), Python, or the CLI beyond its Binance features;
- change `weight.rs`'s funding pacing;
- edit `docs/ARCHITECTURE.md`, `docs/s1-*.md`, CHANGELOG or a historical PROVENANCE;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Minimal build | `--no-default-features --features usdm --test headers_usdm` | passes; no polyoxide-ws in `cargo tree` | N/A |
| Gate | a `fapi` line added to `polyoxide-core` | `venue_gate.py` fails, naming Binance's identifier | N/A |
| Moved gates | baseline `cfg(feature = "ws")` and `required-features = ["test-server"]` | pair with `usdm-ws` and `["usdm-ws", "test-server"]` through `[literals]`/`[gates]`, 0 diffs | an unused entry fails |

</frozen-after-approval>

## Code Map

All paths are relative to `polyoxide-binance/` unless shown.

- **lib.rs:**
  - docs at :1-19, with a doctest at :9-19;
  - the README include at :21-23, bare `cfg(doctest)`;
  - mods at :25-27;
  - root re-exports at :29-31;
  - the `const _` block at :33-44, with the `ws` gates at :40 and :42.
- **usdm/mod.rs:** mods at :3-8, `ws` gated at :7. `crate::error::` at :13 and `crate::weight::` at :18. `BinanceError` at :36, :96 and :125. Add `pub mod error; pub mod weight;` and the re-exports.
- **error.rs → usdm/error.rs:**
  - `BinanceError` at :15, with 11 inline tests;
  - `use crate::weight::MAX_COOLDOWN` at :10;
  - the doc link at :40;
  - the WARN at :113.
  - `ws/error.rs:171` doc-links `crate::BinanceError`.
- **weight.rs → usdm/weight.rs:**
  - 15 inline tests, protected;
  - `crate::usdm::…` at :19-20;
  - doc links to `crate::Usdm` and `crate::UsdmBuilder` at :175, :176 and :194.
- **`crate::error`/`crate::weight` users:**
  - request.rs:10-11, plus the macro paths `$crate::weight::Cost` at :85 and `$crate::BinanceError` at :101;
  - api/{market:7,15; health:9,14; exchange:10};
  - policy.rs:9-10, :25, :140 and :180.
- **BinanceError in tests.** The bodies are paired by the manifest row:
  - ban_log:72;
  - error_classes:14;
  - failure_tags:23-112;
  - live_api:174;
  - mock_api:369-768;
  - README:56.
- **Tests:**
  - **Gates and feature words.** `failure_tags.rs:155` `mod streams` is gated on `ws`. `live_ws.rs:3` and `live_api.rs:6` give `cargo test` usage.
  - **Fixtures.** `fixtures!("rest")` at mock_api:28 and wire_agreement:28,59.
  - **`#[path]` includes.** `scripted.rs` is included by supervision:16, supervision_edges:18 and supervision_kit:23.
  - **Fixture includes.** `ws/fixtures.rs` uses `include_str!("../../../tests/fixtures/ws/…")` at :5-23; these become `usdm/ws/`.
- **Cargo.toml:**
  - features at :14-18;
  - stanzas at :50-75;
  - docs.rs at :82;
  - metadata and live entries at :88-116.
- **README.md:** `ws` at :5 and :71; stale `0.37` at :19-22 and :77; doctests at :26-48 and :53-62.
- **Outside the crate:**
  - **Root Cargo.toml:264.** The binance mirror's `crate_note` (`ws`).
  - **`polyoxide-core/tests/health.rs`.** :143 and :160 in `health_charges_its_costs`; :192 and :207 in `a_refused_health_cost_is_the_callers_error`.
  - **polyoxide-cli/Cargo.toml.** :26 `["ws"]` becomes `["usdm-ws"]`; the dev-dep at :53-54 becomes `["usdm-ws", "test-server"]`.
  - **ci.yml:119.** `--features usdm --test headers_usdm`. nightly-behavioral's hand-written :42-43; the rest is generated.
  - **`.github/scripts/tests/`:**
    - test_classify_coverage:235;
    - test_classify_failures:40;
    - test_module_readmes, its docstring at :18-20 and synthetic at :267;
    - test_ci_workflow's header-pin synthetic at :226-229 and :256-259, which uses a hypothetical crate now. `test_docs_rs_builds_every_polymarket_feature_but_test_server` (:240) is extended to every venue crate;
    - test_mutants_ledger:33-38, :74 and :99-100;
    - test_venue_gate:279-291, which adds the expected identifier terms.
  - **scripts.** `live_unwraps.baseline.json:5-6`; `capture_binance_fixtures.py`'s usage at :9 and its PROVENANCE template at :181-213.
  - **`ci/test-body-diff.toml`:**
    - `[fixtures] "polyoxide-binance" = "usdm"`;
    - `[literals."polyoxide-binance"] "ws" = "usdm-ws"` (5.4's clob precedent);
    - the health.rs literal;
    - `[gates]` dropping `usdm` (the `mod usdm` gate, and the stanzas' new `usdm`) and `usdm-ws` from stanzas that were `["test-server"]`.
  - **Docs:**
    - the manifest, with rows modelled on 5.4's `WebSocketError` → `ClobWsError` and 5.5's feature rows; doc-hidden entry :159;
    - MUTANTS.md:19, :117, :120 and :132;
    - docs/specs/binance/INDEX.md:10, :24-25 and :64-69;
    - CLAUDE.md hand-written lines :50, :98-99, :152, :312-313, :341-342, :356, :380-381, :385, :473, :477, :481 and :521, plus the generated regions.

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps and never push. Each message ends `(Story 5.6)`, with the attribution lines:
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
`Claude-Session: https://claude.ai/code/session_01ECNn4RgL4d8WZCGXgeUi2T`

- Record per-binary counts first.
- Stage moves before the body diff.
- Use `CARGO_INCREMENTAL=0`. A signal 15 or exit 254 is earlyoom: re-run.
- Run cargo-hack alone.

**Execution:**
- [x] `polyoxide-binance/`:
  - move error.rs and weight.rs;
  - rename `BinanceError`;
  - add the features, optional deps, stanzas and live metadata, plus `identifiers`;
  - rename the tests and the fixtures;
  - add the module README and rewrite the crate README;
  - update the docs.
- [x] `polyoxide-core/tests/health.rs`, the CLI's Cargo.toml and the root Cargo.toml note.
- [x] `ci/test-body-diff.toml` entries, then the body diff.
- [x] ci.yml, the nightly comment, the CI-script tests, `live_unwraps.baseline.json` and the capture script.
- [x] `docs/rename-manifest.md` rows, then `api_removals.py manifest --write`.
- [x] MUTANTS.md, re-cited and re-proved.
- [x] CLAUDE.md and the specs INDEX, then `gen_registry.py --write`.

**Acceptance Criteria:**
- Given the tree, when clippy (`-D warnings`), `cargo test --all-features --workspace`, the doctests and `RUSTDOCFLAGS=-D warnings cargo doc` run, then all pass and the per-binary counts sum to the baseline's.
- Given the tree, when cargo-hack `--each-feature` and every header pin run, then both pass.
- Given the tree, when `test_body_diff.py --baseline v0.39.0` runs, then it reports 0 diffs and 0 lost; and when `api_removals.py manifest --strict` runs, then it exits 0.
- Given the scripts job, when its pytest, `venue_gate.py` and `gen_registry.py --check` run, then all pass.

## Implementation Notes

- Commits: da5703c (the crate, core's health tests, the CLI's features, the root note), 9a00091 (body diff, manifest, ci.yml, CI-script tests, live-unwraps baseline, capture script), 50a8ea5 (CLAUDE.md, specs INDEX, regenerated regions), a6e25c6 (MUTANTS.md, re-proved on top of 50a8ea5).
- Deviations, each kept:
  - **`manifest --write` changed nothing.** With the rows in place the run reported 0 unresolved, 0 stale rows and 0 stale regions, so the regions were already current.
  - **The crate README is still a doctest include**, now under `cfg(all(doctest, feature = "usdm"))`. It holds no Rust fence, so it adds no test; the include keeps a future example compiled.
  - **The `readme` registration line** names the module and its features, as polymarket's does, so the generated crate list says `usdm` and `usdm-ws`.
  - **`polyoxide-venue` stays a non-optional dependency**, as in polymarket; `usdm` turns on its `decimal`. The generated "needs" now reads `polyoxide-venue` without `(with decimal)`.
  - **Two anti-vacuity additions.** `test_venue_gate.py` gains a test that Binance's three identifiers in a foundation crate are caught with the real metadata (the matrix's Gate row), and `test_module_readmes.py` asserts module READMEs are found in at least two crates.
  - **MUTANTS.md:19 is left as written.** It is a dated note of what an earlier story moved, and no citation; the ledger's test reads only `file:line` citations.
  - **The minimal build is fenced (matrix row 1).** `test_dep_allowlists.py` holds `polyoxide-binance` per feature, built with `--no-default-features`: `polyoxide-ws`, `tokio-tungstenite` and `tungstenite` only with `usdm-ws`, in new `@usdm` and `@usdm-ws` files, with `@usdm-ws` required to hold `polyoxide-ws` and a synthetic test of the rule. `rustls`, which the request named, is left off the rule: `@usdm` builds it through core's `reqwest` (`rustls-tls`), so the rule would refuse the REST build itself; `tungstenite` takes its place.

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass, counts as at baseline
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run alone)
- `cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm` -- expected: pass
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `python3 scripts/venue_gate.py` and `python3 scripts/gen_registry.py --check` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
- `cargo test -p polyoxide-binance --features usdm-ws --test live_usdm --test live_usdm_ws -- --ignored` -- expected: pass, or environmental (a 451)
