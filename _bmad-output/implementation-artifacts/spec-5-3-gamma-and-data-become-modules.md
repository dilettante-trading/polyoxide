---
title: 'Story 5.3: Gamma and data become modules'
type: 'refactor'
created: '2026-10-13'
status: 'in-progress'
route: 'dispatch'
baseline_commit: '8a26ce0491b166da01cf298fe4b072233d077dbc'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-2-polyoxide-polymarket-and-a-venue-neutral-core.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A consumer of Polymarket market and user data depends on two crates, `polyoxide-gamma` and `polyoxide-data`, beside `polyoxide-polymarket`. S2's goal is one crate per venue.

**Approach:** Move both crates into `polyoxide-polymarket` as the feature-gated modules `src/gamma/` and `src/data/`, and delete the old crates. Rename `DataApiError` to `DataError`, repoint every dependent, and record each move and rename in the manifest. The moved tests keep their bodies.

**Decisions** (all decided overnight without the user, on the recommended option, 2026-10-13):
- **Module paths.** `polyoxide_gamma::X` → `polyoxide_polymarket::gamma::X` and `polyoxide_data::X` → `polyoxide_polymarket::data::X`. The internal file layout is kept (`client.rs`, `error.rs`, `types.rs`, `api/`, data's `v2/`), so every inline test keeps its module path. Each old `lib.rs` becomes the module's `mod.rs`.
- **Test target names** follow `<kind>_<module>`, and data's v2 suites take `_data_v2`:
  - gamma: `live_gamma`, `mock_gamma`, `wire_agreement_gamma`, `error_classes_gamma`, `headers_gamma`, `query_setters_gamma`;
  - data: `live_data`, `mock_data`, `mock_data_v2`, `spec_agreement_data_v2`, `wire_agreement_data_v2`, `enum_wire_data_v2`, `error_classes_data`, `headers_data`, `query_setters_data`, `holders_shape_data`.
  - Data's `tests/common/mod.rs` keeps its path.
  - Every target declares `required-features`.
- **Live jobs.** Each module has its own suite, so nightly runs `live-polyoxide-polymarket-gamma` and `live-polyoxide-polymarket-data`, each job holding only its own secrets (none today).
- **Fixtures.** They move to `tests/fixtures/gamma/` and `tests/fixtures/data/v2/`. Gamma's `tests/fixtures/README.md` becomes `tests/fixtures/gamma/PROVENANCE.md`.
- **Capture scripts.** `scripts/capture_v2_fixtures.py` becomes `scripts/capture_polymarket_data.py`. Gamma's fixtures were captured by hand, so gamma gets no script.
- **Defaults.** `polyoxide-polymarket`'s defaults stay `[]` until 5.4, which sets `clob`, `gamma` and `data` together. `specta` keeps its name, and each module adds its own specta extras with `specta?/…`.
- **The umbrella in transition.** `polyoxide` re-exports `polyoxide_polymarket` in place of `polyoxide_gamma` and `polyoxide_data`, and its prelude names `DataError`. Story 5.7 reshapes the umbrella.
- **Scope.** The `game_id` type alignment is not in 5.3. The spec stays whole, not split into gamma and data stories, because both share every tooling change.
- **Checkpoints.** The spec runs to about 3,500 tokens. It is kept whole, not split, and approved with no human review; both were decided overnight without the user, under the run's advance approval.
- **The README count check.** It does not exist yet. 5.3 adds it as a scripts-job pytest.

## Boundaries & Constraints

**Always:**
- **No shims.** `polyoxide-gamma/` and `polyoxide-data/` are deleted, along with their workspace members and `[workspace.dependencies]` entries. No alias or re-export keeps an old path alive.
- **Gating.** Every module item and every dependency the modules bring is gated by its feature: `gamma`, `data`, `specta`. `-F gamma` alone and `-F data` alone each build, and `cargo hack check --each-feature` passes.
- **Tests move unchanged.** `scripts/test_body_diff.py --baseline v0.39.0` reports 0 diffs and 0 lost, with every config entry firing. Rewrite HEAD paths until they normalise equal; for example a moved inline test writes `crate::gamma::types::X` where the baseline wrote `crate::types::X`. Never edit an assertion or a test name.
- **The manifest.**
  - `## Paths` rows:
    - `polyoxide_gamma` → `polyoxide_polymarket::gamma`, and `polyoxide_data` → `polyoxide_polymarket::data`;
    - the `DataApiError` rows, at both its root and `error::` paths;
    - the umbrella's `polyoxide_gamma`, `polyoxide_data` and prelude paths.
  - `## Features` rows: `polyoxide-gamma/specta` and `polyoxide-data/specta` → `polyoxide-polymarket/specta`.
  - The doc-hidden entry `polyoxide_data::v2::PageStream` moves to its new path.
  - `api_removals.py manifest --strict` resolves every entry.
- **Registration moves with the code.** `polyoxide-polymarket`'s metadata gains:
  - `products = ["gamma", "data"]`;
  - `mirrors = ["gamma", "data", "data-v2", "undocumented"]`;
  - the two `live.*` entries;
  - a current `readme` line.
  - Run `gen_registry.py --write`.
- **Module READMEs.** `src/gamma/README.md` and `src/data/README.md` are doctests under `cfg(all(doctest, feature = "<m>"))`. The crate README links to both.
- **AD-21.** Every CLAUDE.md rule that names a moved path, test file, script or `DataApiError` is rewritten in its own section.
- **CI-script tests.** Update the names they pin, and never weaken an anti-vacuity assertion.

**Never:**
- touch clob, relay, perps, rtds or sports beyond repointing their gamma and data imports and features;
- add `[[drift]]` entries;
- create `tombstones/` (that is 5.12);
- give `polyoxide-polymarket` default features;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Minimal build | `cargo test -p polyoxide-polymarket --no-default-features --features gamma --test headers_gamma` | passes, asserting the same three headers | N/A |
| Renamed error | a baseline test body naming `DataApiError::V2(..)` | pairs with HEAD's `DataError::V2(..)`, 0 diffs | N/A |
| README count | a `src/<m>/README.md` with no gated include, or the reverse | the new pytest fails, naming the module | N/A |
| Old path | `use polyoxide_data::DataApi;` | does not compile; the manifest row gives the new path | N/A |

</frozen-after-approval>

## Code Map

- **The two crates** (Cargo.toml metadata to move: gamma :44-54, data :66-76):
  - gamma: src = lib, client, error, types and `api/` (10 files). Its tests are error_classes, headers, live_api (46), mock_api (47), query_setters and wire_agreement (21). Its fixtures are flat: 16 json files plus README.md, loaded by `include_str!("fixtures/x.json")` in wire_agreement.rs:54-69. Examples: cf_burst_probe and gamma_batch_ceiling. Dependencies: chrono, tokio, urlencoding and url; `specta` brings chrono.
  - data: src = lib, client, error, types, `api/` (16 files) and `v2/{mod,envelope,error,api/,types/}`. Its tests are `common/mod.rs`, error_classes, headers, holders_shape, live_api (26), mock_api (43), query_setters, v2_enum_wire, v2_mock_api, v2_spec_agreement and v2_wire_agreement. `fixtures!("v2")` is used at v2_spec_agreement.rs:216 and v2_wire_agreement.rs:164, so add a `[literals]` entry scoped to those files mapping `"v2"` → `"data/v2"`. Examples (`[[example]] test = true`): v2_soak/ (main + probes), closed_positions_soak and closed_positions_burst_probe. Dependencies: futures-util and rust_decimal; `specta` brings rust_decimal; the dev-dependencies include test-support `soak`.
  - Classify assertions: gamma lib.rs:64-67, data lib.rs:59-63. They merge into polymarket's `lib.rs` `const _`, each gated.
- **`polyoxide-polymarket`**:
  - Cargo.toml: features at :17-23, `[[test]]` at :49-63, metadata at :72-102.
  - lib.rs: README doctest :14-16, `shared` :18-25, `const _` :30-34.
  - The README says the modules arrive in 5.3.
- **Dependents:**
  - **clob**: `gamma = ["dep:polyoxide-gamma"]` (Cargo :15, :37) becomes `polyoxide-polymarket/gamma`. Code uses at client.rs:28, :922; error.rs:26, :264; tests live_api.rs:19, live_session_keys.rs:36, mock_api.rs:1680, :4480, :4501.
  - **The umbrella `polyoxide`**:
    - Cargo :16-17, :28, :30;
    - lib.rs :88, :90 (`pub use`), :101, :103, :115, :117 (prelude), :151, :156 (`PolymarketError`), and tests :370, :379;
    - README :36-37 and :106, :116, which is a doctest.
  - **cli**: Cargo :30-31. Source has 35 lines across 20 files, `DataApiError` in `commands/data/paging.rs`. Tests: live_api.rs, data_v2.rs:14 and :30 (fixture path), prices_discovery.rs:25.
  - **py**: Cargo :13-14.
    - `error.rs:25-29` holds `DataApiError`.
    - Paths: clients/{data,gamma,data_v2}.rs and types/{data,data_v2,gamma}.rs, including `py_type!` paths, and macros.rs:7.
    - Fixture paths: types/gamma.rs:263 and types/data_v2.rs:410.
    - Python tests: test_comment_types.py:27 and test_data_v2_offline.py:23.
    - `polyoxide-py/scripts/gen_v2_bindings.py:155` emits `polyoxide_data::v2::…`.
  - **Root Cargo.toml**: members :8-9, workspace deps :35 and :38, and the mirror metadata text at :158 and :198.
- **CI:**
  - `ci.yml:113-114`: the header pins become `-p polyoxide-polymarket --no-default-features --features <m> --test headers_<m>`.
  - The nightly regions are generated.
- **`.github/scripts/tests/`:**
  - `test_ci_workflow.py:102-134`, `http_modules`. Count (crate, product) pairs so that polymarket's gamma and data are two modules. Require `tests/headers_<product>.rs` and the exact command line, and keep `>= 6`.
  - `test_classify_coverage.py:229-230`: fold the data row into polymarket's (`GammaError`, `DataError`, `V2Error`, `BurstCapacityExceeded`).
  - `test_live_registry.py:128-131` and :486-488: real-metadata lookups by the old crate and target names.
  - `test_api_removals.py:1414-1434`, the doc-hidden `PageStream` mapping.
  - `test_mutants_ledger.py:46-48`.
- **`scripts/`:**
  - `live_unwraps.baseline.json:11-12`: re-key by running `live_unwraps.py`.
  - `capture_v2_fixtures.py` gets renamed, and its usage path updated.
  - `gen_data_v2_types.py:6` names the old crate.
  - `api_removals.py:11` docstring.
- **`test_body_diff.py`:**
  - A manifest row whose last segment changes renames a bare identifier in files that name the old crate (`_renames` :1206-1224, `apply_rows` :1227-1274). So the `DataApiError` row also maps bare uses.
  - `[gates]` prefixes `polyoxide-polymarket/src/gamma/` = `gamma` and `…/src/data/` = `data`, the documented crate-root case.
- **docs/MUTANTS.md:** rows :88, :89 and :92 and the note at :117 cite `polyoxide-data/tests/mock_api.rs:1509`, `:1532` and `:1556`. Re-cite them at `mock_data.rs`, and re-prove each mutant.
- **CLAUDE.md** hand-written rules:
  - :50 header pins;
  - :126 CLI data_v2 fixtures;
  - :181 error hierarchy;
  - :231 wire enums;
  - :348 headers.rs;
  - :393-415, the Data v2 section;
  - :436-454, gamma OBSERVED;
  - :474-476 testing conventions;
  - plus every `polyoxide-data/examples/…` cite.
  - The rest is generated.
- **Other docs:**
  - `docs/specs/INDEX.md:58`;
  - `docs/specs/gamma/OBSERVED.md` :93, :180, :208, :212, :298, :307, :316, :328, :345, :364;
  - `docs/specs/data-v2/OBSERVED.md` :59, :85, :134, :142, :166, :167, :204;
  - `docs/specs/{data/INDEX.md:9, data/holders.md:73, data/rate-limits.md:40,118, data-v2/INDEX.md:9, undocumented/INDEX.md:13,20,44, combos-rfq/INDEX.md:9, sports/OBSERVED.md:81}`;
  - `polyoxide-clob/README.md:29`, `polyoxide-clob/examples/test_order.rs:33`.
  - Leave `docs/s1-*.md` alone; they are historical.

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps, never push, with messages ending `(Story 5.3)` and the attribution lines:

```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01BjnNeirn7DAPvQNrs5khCr
```

Stage moved files before running the body diff (`git` pairs only tracked files). Build with `CARGO_INCREMENTAL=0`. A rustc killed with signal 15 or exit 254 is earlyoom; re-run it. Run cargo-hack alone. Record the per-binary test counts before you start.

**Execution:**
- [ ] `polyoxide-polymarket/`:
  - move `src/`, `tests/`, `examples/` and the fixtures per Decisions;
  - add features, deps and dev-deps (all gated), `[[test]]` and `[[example]]` stanzas with `required-features`, metadata, the module READMEs, the gated `const _` and the crate README links.
- [ ] Delete `polyoxide-gamma/` and `polyoxide-data/`; update the root `Cargo.toml` and `Cargo.lock`.
- [ ] Rename `DataApiError` → `DataError` everywhere.
- [ ] Repoint clob, the umbrella (with its README doctest), cli and py: paths, features and fixture paths, including the py generator and the Python tests.
- [ ] `ci.yml` header pins; `test_ci_workflow.py`, `test_classify_coverage.py`, `test_live_registry.py`, `test_api_removals.py` and `test_mutants_ledger.py`; `live_unwraps.baseline.json`.
- [ ] `.github/scripts/tests/test_module_readmes.py` (new):
  - for every member with `venue` and `products`, each `src/<product>/README.md` must have exactly one include gated `cfg(all(doctest, feature = "<product>"))`, and each such include a file;
  - fail naming the module;
  - assert at least two modules on the real tree;
  - cover it with synthetic trees.
- [ ] `scripts/capture_polymarket_data.py` (renamed); `gen_data_v2_types.py`; the `api_removals.py` docstring.
- [ ] `ci/test-body-diff.toml`: the `[files]`, `[literals]`, `[paths]` and `[gates]` the move needs, each one firing.
- [ ] `docs/rename-manifest.md` rows, then `api_removals.py manifest --write`.
- [ ] `docs/MUTANTS.md` re-cited and re-proved.
- [ ] CLAUDE.md rules, other docs, then `gen_registry.py --write`.

**Acceptance Criteria:**
- Given the moved tree, when clippy (`-D warnings`), `cargo nextest run --all-features --workspace`, `cargo test --doc --all-features --workspace` and `RUSTDOCFLAGS=-D warnings cargo doc` run, then all pass, and the per-binary counts sum to the baseline's: the same tests, now under `polyoxide-polymarket`.
- Given the same tree, when `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` and the two minimal header pins run, then both pass.
- Given the same tree, when `test_body_diff.py --baseline v0.39.0` runs, then it reports 0 diffs and 0 lost.
- Given the same tree, when `api_removals.py manifest --baseline v0.39.0 --strict` runs, then it exits 0.
- Given the scripts job, when its pytest runs, then it is green, including the new README count test. And `uv run pytest tests/` in `polyoxide-py` passes after `maturin develop` or `uv sync --reinstall-package polyoxide`.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Design Notes

**Gating sketch** (`polyoxide-polymarket/Cargo.toml`):

```toml
gamma = ["dep:polyoxide-core", "dep:serde", "dep:serde_json", "dep:thiserror", "dep:url",
         "dep:chrono", "dep:tokio", "dep:urlencoding", "specta?/chrono"]
data  = ["dep:polyoxide-core", "dep:serde", "dep:serde_json", "dep:thiserror", "dep:url",
         "dep:futures-util", "dep:rust_decimal", "specta?/rust_decimal"]
specta = ["dep:specta"]
```

`lib.rs` declares `#[cfg(feature = "gamma")] pub mod gamma;` and `#[cfg(feature = "data")] pub mod data;`. Write `specta` attributes as `cfg_attr(feature = "specta", …)`, as today; specta's own extras come through the weak features above.

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo nextest run --all-features --workspace` -- expected: all pass, counts as at baseline
- `CARGO_INCREMENTAL=0 cargo test --doc --all-features --workspace` -- expected: pass
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run alone)
- `cargo test -p polyoxide-polymarket --no-default-features --features gamma --test headers_gamma` (and `data`) -- expected: pass
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
- `python3 scripts/gen_registry.py --check` (or `--write` then `git diff --exit-code`) -- expected: no stale region
