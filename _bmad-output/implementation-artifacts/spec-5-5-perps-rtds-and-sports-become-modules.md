---
title: 'Story 5.5: perps, rtds and sports become modules'
type: 'refactor'
created: '2026-10-13'
status: 'done'
route: 'dispatch'
baseline_commit: 'def70b785d6c87606d90842b449ccfd447dea5ed'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-4-clob-and-relay-become-modules.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The perps, price and sports feeds are still three crates beside `polyoxide-polymarket`, perps depends on that crate for its policy, and rtds alone calls its fixtures feature `test-fixtures` (DRIFT R6). The per-feature fence does not yet prove that `-F rtds` or `-F sports` builds no HTTP or signing code.

**Approach:** Move `polyoxide-perps`, `polyoxide-rtds` and `polyoxide-sports` into `polyoxide-polymarket` as the modules `src/perps/`, `src/rtds/` and `src/sports/`, as 5.4 moved clob, then delete the old crates. Rename rtds' `test-fixtures` to `test-server` in its own commit first (DRIFT R6). Give the crate one `test-server`, and extend the per-feature fence to the socket-only modules. Every suite moves with an empty body diff.

**Decisions** (each on the recommended option; the user approved the spec as written on 2026-10-13):
- **Module paths.** `polyoxide_{perps,rtds,sports}::X` becomes `polyoxide_polymarket::{perps,rtds,sports}::X`. Internal files are kept, and each `lib.rs` becomes its `mod.rs`. Error names stay (`PerpsError`, `VenueError`, `PerpsWsError`, `RtdsError`, `SportsError`).
- **Features:**
  - `perps` builds the HTTP module: core, `polyoxide-venue/decimal`, rust_decimal, serde, serde_json, thiserror, tracing and url.
  - `perps-ws = ["perps", polyoxide-ws, tokio/time, futures-util]` replaces perps' `ws`.
  - `rtds` and `sports` build only polyoxide-ws, tokio/time, futures-util, serde, serde_json, thiserror and tracing, plus rust_decimal and url for rtds. Never core.
  - `test-server = []`. Each module's `#[cfg(any(test, feature = "test-server"))] pub mod fixtures;` sits inside its module, so it is in effect `all(test-server, <module>)`, the way 5.4 handled clob's gamma gates.
  - docs.rs builds every feature except `test-server`.
- **DRIFT R6 commit.** It is the first commit, and changes only `polyoxide-rtds`: the feature, its `fixtures` gate, the `supervision` stanza and its allowlist file name. The body diff acknowledges R6 with `[[drift]] row = "R6"` entries, one per changed item, each pinned by its sha256. The manifest's `## Features` row maps the stanza.
- **Fixtures:**
  - perps' `tests/fixtures/` moves to `tests/fixtures/perps/`, with `ws/` under it at `perps/ws/`.
  - sports' fixtures move to `tests/fixtures/sports/`.
  - rtds' frames stay inline in `src/rtds/fixtures.rs`.
  - Each `PROVENANCE.md` moves with its files.
- **`fixtures!()` normalisation.** perps' agreement suites call `fixtures!()` with no argument, which no `[literals]` entry can map. `test_body_diff.py` gains a `[fixtures]` table, `"<baseline scope>" = "<subdir>"`, that reads a baseline `fixtures!()` as `fixtures!("<subdir>")` and `fixtures!("x")` as `fixtures!("<subdir>/x")`. Each entry must fire. It has unit tests and its own line in CLAUDE.md.
- **Capture scripts:**
  - `capture_perps_fixtures.py` becomes `capture_polymarket_perps.py`;
  - `capture_perps_ws_fixtures.py` becomes `capture_polymarket_perps_ws.py`;
  - `capture_sports_fixtures.py` becomes `capture_polymarket_sports.py`.
- **Test targets:**
  - perps: `error_classes_perps`, `headers_perps`, `live_perps`, `live_perps_ws`, `mock_perps`, `spec_agreement_perps`, `wire_agreement_perps`, `wire_agreement_perps_ws`;
  - rtds: `failure_tags_rtds`, `live_rtds`, `supervision_rtds`;
  - sports: `bare_sports`, `failure_tags_sports`, `live_sports`, `supervision_sports`.
  - Each has `required-features`.
- **Live suites.** `perps` holds `live_perps` and `live_perps_ws`. `rtds` holds `live_rtds`. `sports` holds `live_sports`, which keeps its 20-minute timeout and note. None has secrets.
- **Examples.** `info_soak` (`test = true`, requiring `perps`) and `twap_stream` (requiring `rtds`) keep their names.
- **READMEs.** perps' and sports' READMEs become gated module READMEs. rtds had none and gets none; its crate docs become `rtds/mod.rs` docs.
- **Dependents:**
  - The umbrella keeps its feature names `rtds`, `perps`, `perps-ws` and `sports`, now passthroughs to polymarket. Its `pub use polyoxide_polymarket` covers them, and its prelude aliases and `PolymarketError::Perps` take the new paths. Story 5.7 reshapes it.
  - The CLI adds `rtds` and `sports` to its polymarket features, and its dev-dependency adds `sports` and `test-server`.
- **Out of scope.** The `game_id` type alignment (deferred-work) is not in 5.5.
- **Scope.** The spec stays whole, at about 3,300 tokens, because the three moves share the fence, manifest and body-diff changes. The user approved it as written on 2026-10-13.

## Boundaries & Constraints

**Always:**
- **No shims.** The three crate directories, their members and their workspace dependencies are deleted.
- **Gating.** `cargo hack check --each-feature` passes. `-F rtds` alone, `-F sports` alone and `-F perps` alone build. `--no-default-features --features perps --test headers_perps` passes.
- **Body diff.** `test_body_diff.py --baseline v0.39.0` reports 0 diffs and 0 lost, every entry fires, and the only `[[drift]]` entries name R6. That covers every supervision suite: perps' inline 109, rtds' 9 integration tests plus its inline tests, and sports' 15, 6 and inline. Never edit an assertion or a test name.
- **The fence (NFR2).** `test_dep_allowlists.py` gains a polymarket rule: `reqwest`, `polyoxide-core`, `governor` and `sha2` only with an HTTP module (`clob`, `gamma`, `data`, `relay` or `perps`).
  - New files: `@rtds`, `@sports`, `@rtds,test-server`, `@sports,test-server` and `@perps-ws`. `@perps` is regenerated.
  - The old rtds and sports files go.
  - `MUST_BUILD` proves the new rule refuses a crate that `@perps` builds. Synthetic tests cover the rule.
- **The manifest.**
  - Rows for the three crate paths, the umbrella's `polyoxide::polyoxide_{perps,rtds,sports}`, and the features `polyoxide-perps/{ws,default}`, `polyoxide-rtds/test-fixtures` (citing DRIFT R6) and `polyoxide-sports/test-server`.
  - The doc-hidden entries move to their new paths and features.
  - `manifest --strict` exits 0.
- **Registration.** products and mirrors gain perps, rtds and sports, and the four live entries are added. Update the readme line and notes, then run `gen_registry.py --write`.
- **AD-21.** Every CLAUDE.md rule naming a moved path, test, script, `ws` feature or `test-fixtures` is rewritten in its own section. `docs/MUTANTS.md` is re-cited and re-proved.
- **CI-script tests** are updated without weakening anti-vacuity.

**Never:**
- touch Binance (5.6), clob, gamma, data or relay beyond the text that names the moved crates;
- edit `docs/ARCHITECTURE.md`, `docs/s1-*.md` or historical PROVENANCE and migration text;
- add `[[drift]]` entries other than R6's;
- create `tombstones/`;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Credential-free feed | `cargo tree -p polyoxide-polymarket --no-default-features --features rtds` | no `polyoxide-core`, reqwest, alloy\*, governor, hmac, sha2 or keyring | N/A |
| HTTP leaks into rtds | a change that makes `@rtds` build polyoxide-core | `test_dep_allowlists.py` fails, naming the crate and the rule | N/A |
| No-argument fixtures | baseline `fixtures!()` in perps' `wire_agreement.rs` | pairs with `fixtures!("perps")`, 0 diffs | an unused `[fixtures]` entry fails |
| R6 | rtds fixture items gated on `test-server` | each diff is acknowledged by its R6 entry; any other change shows | N/A |

</frozen-after-approval>

## Code Map

- **perps** (`polyoxide-perps/`):
  - **lib.rs.** Mods at :23-28 (`ws` gated `feature = "ws"`). Re-exports at :30-31. README include at :19-21. `const _` at :36-42 (`PerpsWsError` at :40).
  - **client.rs:4.** `use polyoxide_polymarket::shared::{self, PolymarketRetryPolicy}` becomes `crate::shared`.
  - **`crate::` paths** to prefix with `perps::`:
    - client.rs:6;
    - api/{mod:11, exchange:7, health:8, market:8, public:8};
    - ws/{channel:8,135; frame:9,294; error:9,151; builder:6,78,108; event:4,140; protocol:16,43,408; client:13,295}.
  - **Doctests** naming the crate: lib.rs:9,11; ws/mod.rs:50,52; ws/client.rs:122,124.
  - **ws/mod.rs.** Doc-hidden `*_for_tests` at :73-105, `SupervisedPerpsWs` at :66, and `#[cfg(test)] mod scripted` at :21.
  - **Tests:** error_classes 3, headers 1, live_api 4, live_ws 2, mock_api 19, spec_agreement 5, wire_agreement 1, ws_wire_agreement 3.
    - `fixtures!()` at spec_agreement:223 and wire_agreement:72; `fixtures!("ws")` at ws_wire_agreement:28.
    - The `../docs/specs/perps/*` paths stay valid.
  - **Fixtures.** 21 files and a PROVENANCE.md, plus `ws/` with 11 and a PROVENANCE.md. Each PROVENANCE names its script at line 3.
  - **Example.** `examples/info_soak.rs`, with 3 tests and `cargo run -p polyoxide-perps` at :9,12.
  - **README.md.** Title at :1, install snippet at :13-16, doctest at :20-31, "the `ws` feature" at :35.
- **rtds** (`polyoxide-rtds/`):
  - **lib.rs.** Mods at :21-28. `fixtures` gated `any(test, feature = "test-fixtures")` at :30-32. `#[cfg(test)] mod test_log` at :39. Re-exports at :42-51. `const _` at :56-59. Docs at :1 and :8.
  - **Cargo.toml.** `test-fixtures` at :17, and the `supervision` stanza at :19-25.
  - **Tests:** failure_tags 1, live_api 3, supervision 9. supervision names `polyoxide_rtds::fixtures::REJECTED_SUBSCRIPTION` in full at :29, :98, :135, :188, :250, :288, :324 and :362.
  - **Example.** `examples/twap_stream.rs`, which uses tokio macros and tracing-subscriber.
- **sports** (`polyoxide-sports/`):
  - **lib.rs.** README include at :25-27. Mods at :29-32. `fixtures` at :34-36. Re-exports at :38-41. `const _` at :46-49.
  - **fixtures.rs.** `include_str!("../tests/fixtures/*.json")` at :8-24. `every_fixture_file_is_listed` reads `"/tests/fixtures"` at :46, so it needs a scoped literal → `"/tests/fixtures/sports"`.
  - **Tests:** bare 6, failure_tags 7, live_api 5, supervision 15.
  - **README.md.** Install snippet at :11-14, doctest at :21-43. The migration section at :48-71 is historical and stays.
- **polyoxide-polymarket:**
  - **Cargo.toml.** The comment at :14-15, `perps = ["dep:polyoxide-core"]` at :90, the description, the readme metadata at :362, products and mirrors at :364-365, and docs.rs `all-features`.
  - **Dev-dependencies.** Add tokio `net`, which rtds and sports used.
  - **lib.rs.**
    - Docs at :1-14.
    - README includes, following :26-40.
    - Add `#[cfg(feature = "rtds")] pub mod rtds;` and the same for perps and sports. `shared` stays out of `-F rtds` and `-F sports`.
    - Add the gated `const _` blocks, following :63-90.
  - **README.md.** Link the module READMEs. src/clob/README.md has one mention.
  - **`shared/limits.rs:227`** cites info_soak's old path.
- **Dependents:**
  - **Root Cargo.toml.** Members at :7, :10 and :11; workspace deps at :30, :34 and :35.
  - **The umbrella (`polyoxide/`).** Cargo features at :19-23 and optional deps at :30-32. lib.rs:
    - docs at :4 and :9-11;
    - re-exports at :85-95;
    - prelude at :106-133;
    - `PolymarketError::Perps` at :157-160, with its arms at :178, :192 and :206;
    - the test at :389-399.
    - README at :3, :9 and :39-43.
  - **cli.** Cargo deps at :30-31, dev-dep at :53-54. src uses at ws/prices.rs:12,180 and ws/sports.rs:12. Tests at ws_sports.rs:12 and live_api.rs:11,188.
- **CI:**
  - **Workflows.** `ci.yml:118` header pin becomes `-p polyoxide-polymarket --no-default-features --features perps --test headers_perps`. Also nightly-behavioral's hand-written :42-43.
  - **`.github/scripts/tests/`:**
    - test_classify_coverage :233, :274-277;
    - test_dep_allowlists `PER_FEATURE`/`MUST_BUILD` :76-89, the docstring :21-29, and the synthetic :261, :282-283, :332;
    - test_classify_failures :41-42 (`failure_tags_{sports,rtds}.rs`);
    - test_mutants_ledger :49, :97, :101, :103;
    - test_module_readmes :17, :204-207;
    - test_ci_workflow :208-211.
  - **scripts.** `live_unwraps.baseline.json:8,15` and the three capture scripts' usage text.
  - **`ci/test-body-diff.toml`:**
    - `[literals."polyoxide-perps"] "ws" = "perps-ws"`;
    - the sports `fixtures.rs` literal;
    - `[fixtures] "polyoxide-perps" = "perps"`;
    - `[gates]` dropping the module from the required-features of the moved stanzas: `examples/info_soak.rs` = `perps`, `tests/supervision_rtds.rs` = `rtds`, and `tests/{bare,supervision}_sports.rs` = `sports`. perps' two `ws` stanzas need none, because the manifest's `polyoxide-perps/ws` row maps `["ws"]` to `["perps-ws"]`;
    - R6's `[[drift]]` entries.
- **Docs:**
  - **The manifest.** Rows after :108 and :121. Doc-hidden entries at :139-141 and :145.
  - **MUTANTS.md.** :73-74, row (b) :109, and the (g) note :134-137.
  - **CLAUDE.md** (hand-written):
    - :50, :92-94, :228;
    - the perps section :249-306;
    - :541, :543;
    - sports :551-557 and rtds :559-565.
  - **specs.**
    - perps: INDEX:7, OBSERVED :11, :13, :114, :176 and :178.
    - rtds: OBSERVED :27 and :46, and asyncapi-live-data.json:212.
    - sports: OBSERVED:8, INDEX:7.
  - **SELF-HEALING.md:38.**

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps and never push. Each message ends `(Story 5.5)`, with the attribution lines:
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
`Claude-Session: https://claude.ai/code/session_01BjnNeirn7DAPvQNrs5khCr`

- Record the per-binary test counts first.
- Stage moved files before running the body diff.
- Use `CARGO_INCREMENTAL=0`. A signal 15 or exit 254 is earlyoom: re-run.
- Run cargo-hack alone.

**Execution:**
- [x] `polyoxide-rtds/` and `ci/dep-allowlists/`: the DRIFT R6 commit (`refactor(rtds)!: test-fixtures becomes test-server (DRIFT R6)`).
- [x] `polyoxide-polymarket/`:
  - move src, tests, examples and fixtures per Decisions;
  - add features, deps and gated dev-deps;
  - add the stanzas, metadata, the README includes and the `const _` blocks;
  - update its docs.
- [x] Delete the three crates. Update the root Cargo.toml and Cargo.lock, the umbrella, and the CLI.
- [x] `scripts/test_body_diff.py`: the `[fixtures]` table and its tests. Then the `ci/test-body-diff.toml` entries, including R6's `[[drift]]` entries.
- [x] `test_dep_allowlists.py` and `ci/dep-allowlists/`: the new rule, the new files and synthetic tests.
- [x] ci.yml, the nightly comment, the other CI-script tests, `live_unwraps.baseline.json` and the capture scripts.
- [x] `docs/rename-manifest.md` rows, then `api_removals.py manifest --write`.
- [x] `docs/MUTANTS.md`, re-cited and re-proved.
- [x] CLAUDE.md, the other docs, then `gen_registry.py --write`.

**Acceptance Criteria:**
- Given the moved tree, when clippy (`-D warnings`), `cargo test --all-features --workspace`, the doctests and `RUSTDOCFLAGS=-D warnings cargo doc` run, then all pass and the per-binary counts sum to the baseline's.
- Given the same tree, when cargo-hack `--each-feature`, the five header pins and `cargo check` of `-F rtds`, `-F sports` and `-F perps` alone run, then all pass.
- Given the same tree, when `test_body_diff.py --baseline v0.39.0` runs, then it reports 0 diffs and 0 lost, with only R6's entries acknowledging anything; and when `api_removals.py manifest --strict` runs, then it exits 0.
- Given the scripts job, when its pytest and `gen_registry.py --check` run, then both pass, the rtds and sports allowlists included.

## Implementation Notes

- The first implementer stopped after the DRIFT R6 commit, 8a94f49, with a clean tree. A fresh one resumes from the next task; 8a94f49 stays as it is.
- Deviations the second implementer made, each kept:
  - **No perps `"ws"` literal.** The Code Map's `[literals."polyoxide-perps"] "ws" = "perps-ws"` replaces nothing, so the every-entry-fires rule fails it. No test item sits directly under `mod ws;`, and the stanzas follow the manifest's `polyoxide-perps/ws` row.
  - **rtds' README exemption.** `test_module_readmes.py` requires a README in every module directory, so `WITHOUT_README` names rtds with its reason. It fails as stale once rtds has a README or stops being a module.
  - **Header pins and specta.** `http_modules()` counts only products whose feature builds `polyoxide-core`, so rtds and sports get no header pin. ci.yml gains `perps,specta`, which the specta step's test requires.
  - **docs.rs.** It lists its features rather than `all-features`, so `test-server` stays out.
  - **`@perps` grew.** It gains `arrayvec`, `num-traits` and `rust_decimal`: the old `perps` feature built only `shared`, and the module's types need decimals. `MUST_BUILD` holds `@perps` to `reqwest`.

## Spec Change Log

## Review Triage Log

Review 1 used all three layers on the full diff (def70b7..5c8e004), not the one layer planned. 5.3 and 5.4 both ran three, and this diff changes CI gates: the body diff's `[fixtures]`, the README exemption and the header-pin derivation. Findings are tagged B (blind), E (edge case) and G (verification gap).

| # | Finding | Verdict | Route | Evidence |
|---|---|---|---|---|
| B4/E1 | `on_core` treats a product as off core when core is a non-optional dependency | medium | patch | Binance depends on core non-optionally (`polyoxide-binance/Cargo.toml:21`). Once 5.6 gives `usdm` a feature, `http_modules()` drops Binance and its header pin goes unrequired. |
| B5 | Nothing holds docs.rs' hand-written feature list to the crate's features | low | patch | The next module feature of `polyoxide-polymarket` would be missing from docs.rs, and nothing would fail. |
| B9 | CLAUDE.md's Module Organization still lists `clob`, `gamma`, `data` and `relay` only, and speaks of "the five socket crates" | low | patch | AD-21 asks the story to rewrite these rules, so this completes the CLAUDE.md task rather than deferring an agent-context edit. |
| G3 | Moving the code renames the three modules' `tracing` targets | low | patch | Targets default to the module path, so `RUST_LOG=polyoxide_sports=info` stops matching. The manifest's Story 5.5 note now says so. |
| B6/E3 | The sports drift detector's failure message names the renamed `capture_sports_fixtures.py` | low | patch | `tests/live_sports.rs:264`. A scoped `[literals]` entry keeps the body diff empty, as 5.3's fixture-directory literals did. |
| B10/E4 | The sports README's migration table points at `polyoxide_sports` | low | patch | The table is historical and stays. A one-line note before it gives the current path. |
| B1 | `ListEvents::game_id`'s doc reads "a live score from from the `sports` module" | low | patch | `gamma/api/events.rs:334-335`. |
| B7 | Two perps suites' module docs cite `tests/fixtures/PROVENANCE.md` and `tests/fixtures/ws/PROVENANCE.md` | low | patch | Both files are now under `tests/fixtures/perps/`. |
| B8 | "This crate" in the moved modules' docs now means `polyoxide-polymarket`, and `rtds/decode.rs:7` is false | low | patch | Other modules' scale decisions live in the same crate now. |
| B11 | "; The `rtds` module" in the rtds AsyncAPI notes | low | patch | It is a lowercase continuation after a semicolon. |
| B12 | The crate's categories and keywords omit the sockets it now holds | low | patch | The deleted crates filed under `web-programming::websocket`. |
| G1 | No build checks `polyoxide::polyoxide_polymarket::{rtds,perps,sports}` with only that umbrella feature on | medium | defer | Pre-verified. Story 5.7 rewrites the umbrella's re-exports and should add a per-feature `const _`. |
| G2 | The umbrella's `rtds` and `sports` builds lost their transitive dependency fence | medium | defer | Pre-verified, and caused by this change. The fix needs umbrella allowlists and a rule, and Story 5.7 rewrites the umbrella these would pin, so it is deferred there by name. |
| B2 | `sha2` sits in the HTTP rule, though only the HMAC signer builds it | low | defer | Pre-existing: before this story no per-feature rule held `sha2` for `gamma` or `data` either. Moving it means editing the frozen rule. |
| B3 | Excluding `rtds` and `sports` from `http_modules()` also drops them from the specta step | low | reject | Neither module derives `specta::Type`, and a module list of its own adds complexity for a case no one meets today. |
| E2 | A `[fixtures]` entry with an empty scope or a blank subdirectory passes validation | false | reject | Such an entry rewrites unrelated calls, and the body diff then fails loudly on the diffs. It cannot pass silently. |
| E5 | perps' PROVENANCE files cite the old capture script names | low | reject | They record what ran on the capture date, which the frozen intent keeps. The renamed scripts write the new names on the next refresh. |

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass, counts as at baseline
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run alone)
- `cargo test -p polyoxide-polymarket --no-default-features --features <m> --test headers_<m>` for gamma, data, clob, relay and perps -- expected: pass
- `cargo check -p polyoxide-polymarket --no-default-features --features <m>` for rtds, sports and perps -- expected: pass
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
- `python3 scripts/gen_registry.py --check` -- expected: exit 0
- `cargo test -p polyoxide-polymarket --features perps-ws,rtds,sports --test live_perps --test live_perps_ws --test live_rtds --test live_sports -- --ignored` -- expected: pass, or an environmental tag (sports with no live match)
