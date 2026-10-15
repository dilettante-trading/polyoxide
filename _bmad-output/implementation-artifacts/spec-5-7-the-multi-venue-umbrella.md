---
title: 'Story 5.7: The multi-venue umbrella'
type: 'refactor'
created: '2026-10-14'
status: 'done'
route: 'dispatch'
baseline_commit: '0a3253de73fc9048d965f71e9f6f69147e0e0422'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-6-binance-follows-the-conventions.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The `polyoxide` umbrella is still Polymarket's alone. It re-exports `polyoxide_polymarket` under its own name, leaves Binance and relay out, and keys its features on Polymarket's module names. It also holds the unified `Polymarket` client and a `PolymarketError` that no other crate uses.

**Approach:**
- **Venues.** Rebuild the umbrella as `polyoxide::{venue, polymarket, binance, prelude}` behind venue-prefixed features.
- **The unified client.** Move `Polymarket` into `polyoxide-polymarket` with a `BuildError`, and remove `PolymarketError`.
- **Gates.** Pin the venue header tests under `polyoxide --features full`, and close 5.5's two umbrella deferrals: G1, paths resolving per feature, and G2, the rtds and sports dependency fences.

**Decisions:**
- **Paths:**
  - `pub use polyoxide_venue as venue;`, always on.
  - `pub use polyoxide_polymarket as polymarket;`, gated on any `polymarket*` feature.
  - `pub use polyoxide_binance as binance;`, gated on any `binance*` feature.
  - The crate root keeps nothing else. `polyoxide::{Polymarket, PolymarketBuilder}` become `polyoxide::polymarket::{…}`.
  - `parse_key` is S3's (6.2).
- **Polymarket features:**
  - `default = ["polymarket"]`.
  - `polymarket = ["polymarket-clob", "polymarket-gamma", "polymarket-data"]`, polymarket's defaults, named so the umbrella's `cfg`s can see them.
  - One `polymarket-<f> = ["dep:polyoxide-polymarket", "polyoxide-polymarket/<f>"]` for each of `clob`, `clob-ws`, `gamma`, `data`, `relay`, `perps`, `perps-ws`, `rtds` and `sports`. The `-ws` ones also turn on their module's passthrough.
  - Relay joins the umbrella.
- **Binance features** (the user's choice, 2026-10-15). They follow Polymarket's `<venue>` / `<venue>-<feature>` rule:
  - `binance = ["binance-usdm"]`, Binance's defaults;
  - `binance-usdm = ["dep:polyoxide-binance", "polyoxide-binance/usdm"]`;
  - `binance-usdm-ws = ["binance-usdm", "polyoxide-binance/usdm-ws"]`.
- **`keychain`** (the user's choice, 2026-10-15). It keeps its name as a cross-venue extra, `polyoxide-polymarket?/keychain`, so a later venue adds its own `?/keychain`. It gets no rename and no manifest row.
- **`full`.** Every passthrough of both venues, plus `keychain`. `specta` stays polymarket's.
- **Unified client:**
  - `polyoxide_polymarket::{Polymarket, PolymarketBuilder, BuildError}` live in a new `src/client.rs`, re-exported at the root and gated `all(clob, gamma, data)`.
  - `BuildError` is `#[non_exhaustive] { Clob(ClobError), Gamma(GammaError), Data(DataError) }`, each `#[from]`, and implements `Classify` by delegation.
  - `Polymarket::new` and `PolymarketBuilder::build` return it, and the builder's setters are unchanged.
  - `PolymarketError`, with its `Perps` and never-built `Config` variants, is removed.
- **Prelude:**
  - **Items that stay.** Today's items keep their names, on the new features, and lose `PolymarketError`.
  - **New items.** `BuildError`, and Binance's `Usdm`, `UsdmBuilder`, `UsdmError` and `WeightBudget`.
  - **Streams.** Under `binance-usdm-ws`, `UsdmWs`, `UsdmWsBuilder`, `UsdmWsError`, `StreamName` and `Event as UsdmEvent`, aliased as perps' and sports' are.
- **The umbrella's test.** `every_variant_classifies_as_the_error_it_wraps` retires (`[[retired]]`), with a reason row for `polyoxide::PolymarketError`. `BuildError` gets its own test in polyoxide-polymarket.
- **G1.** One `const _` per passthrough names one type of its module through `crate::polymarket::…` or `crate::binance::…`, outside `cfg(test)`, so cargo-hack compiles each path per feature.
- **G2:**
  - `PER_FEATURE["polyoxide"]` holds the socket-only deny-list, admitted by the HTTP features.
  - Files `polyoxide@polymarket-rtds`, `@polymarket-sports` and `@polymarket-perps`. The last carries a `MUST_BUILD` of reqwest.
- **Header pins under `full`.** A CI step runs every derived pin with `cargo test -p polyoxide -p <venue crate> --features polyoxide/full --test headers_<m>`. `test_ci_workflow.py` derives and asserts that step as it does the minimal one.
- **docs.rs.** The umbrella's docs.rs builds `full`.

## Boundaries & Constraints

**Always:**
- No shims.
- `test_body_diff.py --baseline v0.39.0` reports 0 diffs and 0 lost, its one `[[retired]]` entry aside, and no new `[[drift]]`.
- The manifest:
  - **Re-pointed rows.** The 5.3–5.5 `polyoxide::polyoxide_*` rows and their rename rows point at `polyoxide::polymarket::…`.
  - **New rows.** `polyoxide::{Polymarket, PolymarketBuilder}`; reason rows for `polyoxide::PolymarketError` and `polyoxide::prelude::PolymarketError`; and feature rows `polyoxide/{clob,gamma,data,ws,rtds,perps,perps-ws,sports}` → `polyoxide/polymarket-*`.
  - **The 5.7 note.** It names `BuildError` as the new return type.
  - `manifest --strict` exits 0.
- The root README and the umbrella README stay doctests that compile under `--all-features`.
- AD-21: CLAUDE.md's rules on umbrella features, `Polymarket::builder`, and Binance not being in the umbrella are rewritten in their own sections. The generated regions are regenerated.
- The CI-script tests follow the changes without weakening anti-vacuity: classify coverage (`BuildError` in polymarket; the umbrella row goes), allowlists and the header-pin derivation.

**Never:**
- change the venue crates' features or modules;
- touch the CLI or Python;
- add a `specta` umbrella feature;
- edit `docs/ARCHITECTURE.md`, `docs/s1-*.md` or CHANGELOG;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| One passthrough | `cargo hack check -p polyoxide --each-feature` | each `polymarket-*` and Binance feature compiles its own `const _` path | a cfg gate missing that feature fails to compile |
| Socket-only feed | `cargo tree -p polyoxide --no-default-features --features polymarket-rtds` | no core, reqwest, alloy\*, governor, hmac, sha2 or keyring | `test_dep_allowlists.py` fails, naming the crate |
| Full build | `polyoxide/full` with each venue's `headers_<m>` | every pin passes on the unified feature set | a header a feature adds fails its pin |
| Build failure | `Polymarket::builder(acct).clob_base_url("not a url").build()` | `Err(BuildError::Clob(_))`, class `InvalidRequest` | N/A |

</frozen-after-approval>

## Code Map

- **The umbrella (`polyoxide/`):**
  - **Cargo.toml.** Features at :14-24; deps at :26-31, which add `polyoxide-binance` as optional; metadata `readme` at :41; description and keywords at :9-10.
  - **src/lib.rs:**
    - docs and the doctest at :1-73;
    - the README includes at :75-83, kept;
    - `pub use polyoxide_polymarket` at :88-96;
    - the private uses at :98-103;
    - the prelude at :106-138;
    - `PolymarketError` and its `Classify` at :140-212;
    - the `const _` at :217-220, which goes;
    - `Polymarket` and its builder at :222-344, which move;
    - the test at :346-409, which retires.
  - **README.md.** The feature table at :31-44; `polyoxide::polyoxide_polymarket::…` at :106, :116 and :126; the prelude and unified example at :48-98.
- **`polyoxide-polymarket/`:**
  - **src/lib.rs.** Root at :53-75, so add `mod client;` and its gated re-exports. The `const _` blocks are at :80-128; add `BuildError` under `all(clob, gamma, data)`.
  - **Cargo.toml.** The metadata `readme` at :504.
  - **Module READMEs naming the umbrella.** gamma :14-19, data :23-28, sports :73-75 and relay :20.
- **polyoxide-binance:** `Cargo.toml:141` metadata `readme` ("not part of the unified crate"); `README.md:3-4`.
- **Root:**
  - **README.md.** `cargo add` lines at :28-49, with `ws` at :45 and relay "left out" at :47. The compiled rust fence at :71 uses the prelude and `Polymarket::builder`; `:109` is `ignore`.
  - **The binance mirror** at :259-265 is unchanged.
- **CI and scripts:**
  - **ci.yml.** The header pins at :112-119; add the `full` step after them.
  - **`.github/scripts/tests/`:**
    - test_ci_workflow:154-174, where the real-tree check unpacks exactly one `--test headers` step, so identify the steps by name;
    - test_classify_coverage:230-237;
    - test_dep_allowlists `PER_FEATURE` :84-104, `MUST_BUILD` :106-112 and `test_the_allowlists_exist` :215-224. Create the files before `--write`.
  - **scripts/gen_registry.py.** The `claude_umbrella_features` region at :1004-1020 currently says `full` is "all but `keychain`". Adjust the rendering only if the new shape reads wrongly.
- **`ci/test-body-diff.toml`.** The `[[retired]]` format is at :80-86: `file = "polyoxide/src/lib.rs"`, `item = "tests::every_variant_classifies_as_the_error_it_wraps"`, `row = "polyoxide::PolymarketError"`. The reason row must not start with a backtick.
- **docs:**
  - **rename-manifest.md.** Re-point :90-94, :106-108 and :123-125. Their prose says "until Story 5.7", at :83, :101 and :113-115. Add `## Features` rows after :160.
  - **docs/specs/binance/INDEX.md:3-5.**
  - **CLAUDE.md, hand-written.** :133 `Polymarket::builder(account)`; :306-310, "deliberately not in the `polyoxide` umbrella crate or `full`"; the regions at :73-87 and :127-129 are generated.

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps and never push. Each message ends `(Story 5.7)`, with the attribution lines:
`Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
`Claude-Session: https://claude.ai/code/session_01ECNn4RgL4d8WZCGXgeUi2T`

- Record per-binary counts first.
- Use `CARGO_INCREMENTAL=0`. A signal 15 or exit 254 is earlyoom: re-run.
- Run cargo-hack alone.

**Execution:**
- [x] `polyoxide-polymarket/src/client.rs` and `lib.rs`: `Polymarket`, `PolymarketBuilder` and `BuildError` with its `Classify` and test, plus the `const _` entry.
- [x] `polyoxide/`:
  - the features and deps;
  - the paths, prelude and G1 `const _` blocks;
  - remove `PolymarketError`, the client and the test;
  - docs, README, docs.rs and metadata.
- [x] `ci/test-body-diff.toml`: the `[[retired]]` entry. `docs/rename-manifest.md`: rows and the note, then `api_removals.py manifest --write`.
- [x] ci.yml's `full` pin step, `test_ci_workflow.py`, `test_classify_coverage.py`, and `test_dep_allowlists.py` with its three umbrella files.
- [x] READMEs and docs naming the umbrella (root, polymarket modules, binance, binance INDEX), CLAUDE.md, then `gen_registry.py --write`.

**Acceptance Criteria:**
- Given the tree, when clippy (`-D warnings`), `cargo test --all-features --workspace`, the doctests and `RUSTDOCFLAGS=-D warnings cargo doc` run, then all pass and the per-binary counts match the baseline's, the one retired test and the new `BuildError` test aside.
- Given the tree, when cargo-hack `--each-feature`, the minimal header pins and the `full` header pins run, then all pass.
- Given the tree, when `test_body_diff.py` and `api_removals.py manifest --strict` run, then they report 0 diffs (1 retired) and exit 0.
- Given the scripts job, when its pytest, `venue_gate.py` and `gen_registry.py --check` run, then all pass.

## Implementation Notes

- Commits: cac3b63 (the client, `BuildError` and its test in polyoxide-polymarket; the umbrella's features, deps, paths, prelude and G1 consts; the umbrella and root READMEs its doctests read), 59319a9 (the `[[retired]]` entry and the manifest's rows and 5.7 note), 7c58f8e (ci.yml's `full` pin step, `test_ci_workflow.py`, `test_classify_coverage.py`, `test_dep_allowlists.py` and the three umbrella files), 1ee45bd (module, Binance and polymarket READMEs, the specs INDEX, CLAUDE.md, `gen_registry.py` and the regenerated regions, and the two 5.7 deferrals marked done).
- Counts: every binary matches the baseline but two: the umbrella's lib tests 1 → 0 (the retired test) and polyoxide-polymarket's lib tests 977 → 978 (`client::tests::a_build_failure_names_its_client_and_classifies_as_its_error`). The umbrella's doctests stay 6 passed and 4 ignored: the README gained no compiled fence, so Binance's usage there is prose and `cargo add` lines.
- Deviations, each kept:
  - **The umbrella's fence mirrors Polymarket's three rules** rather than one rule admitting the whole socket-only deny-list with any HTTP feature: the HTTP stack (reqwest, polyoxide-core, governor, sha2) with an HTTP module of either venue, including `binance-usdm`; alloy\* and hmac only with `polymarket-clob` or `polymarket-relay`; keyring only with `keychain`. It refuses everything the literal rule does and also holds NFR2's "no credential-free feature builds alloy\*, keyring or hmac" for `polymarket-gamma`, `-data`, `-perps` and `binance-usdm`. Proven: adding `polyoxide-polymarket/data` to `polymarket-rtds` fails the tree check, and once regenerated, the deny-list.
  - **G1's consts are `const _: fn(crate::polymarket::clob::Clob) = drop;`**, one per passthrough: a fn pointer names the type without a value, so no `declare_interior_mutable_const` concern. Proven: dropping `polymarket-relay` from the re-export's `cfg(any(...))` fails `--features polymarket-relay` with E0433.
  - **`gen_registry.py` changed its rendering.** With `full` listing every passthrough, the old rule rendered it "all but `polymarket` and `binance`", which reads wrongly, so a feature whose every item `full` turns on counts as covered and `full` renders "all"; the synthetic test gains such a feature (`va`). "the unified crate" became "the umbrella" in the two regions that named it, since "unified" now names the Polymarket client.
  - **`polyoxide-binance` stays a default-features dependency of the umbrella.** Its only default is `usdm`, which every Binance feature of the umbrella turns on, and the workspace entry is shared with the CLI, so it was left alone.
  - **The full-pin test also requires** that the umbrella's `full` turn on `<crate>/<product>` for each derived module and that the step follow the minimal one; both proven by mutation (a dropped pin line, and `binance-usdm` dropped from `full`).
  - **CLAUDE.md gains a paragraph** after the generated feature region on the umbrella's paths, G1 and the prelude's aliases, besides rewriting the Builder-pattern, Binance and header-pin rules and the CI paragraph's features-job and allowlist clauses.
  - **The prelude's Binance streams** are `Event as UsdmEvent, StreamName, UsdmWs, UsdmWsBuilder, UsdmWsError`; relay has no prelude items, as the spec lists none.
- Review fixes: 1bdeeb8 (review 1's ten patches; see the Review Triage Log). After them, every Verification command passed: clippy, the workspace tests (98 binaries), rustdoc, cargo-hack `--each-feature`, all 12 header pins, `test_body_diff.py` (15 acknowledged, 1 retired, 0 stale), `manifest --strict`, `venue_gate.py`, `gen_registry.py --check` and the scripts' pytest (1301).

## Spec Change Log

## Review Triage Log

Review 1 used all three layers on the full diff (0a3253d..1ee45bd), not the one layer the S2 plan names for 5.3–5.10. Like 5.6, this diff adds gates: the G1 consts, the umbrella's dependency fence, and the `full` header-pin step with its derivation test. A defect in any of them would leave the gate silently vacuous. Findings are tagged B (blind), E (edge case) and G (verification gap); G-O is the gap layer's "other" finding.

| # | Finding | Verdict | Route | Evidence |
|---|---|---|---|---|
| G1 | The prelude's `cfg` blocks are never compiled per feature, so narrowing one silently empties it for default and single-feature consumers | medium | patch | Pre-verified. The G1 consts name only `crate::polymarket::…` and `crate::binance::…`, and every job that compiles a prelude user runs `--all-features`. |
| G2 | Nothing pins that the default, `polymarket`, builds the unified client and the Data API | medium | patch | Pre-verified. Dropping `polymarket-data` from `polymarket` compiles under cargo-hack's `default` and `polymarket` runs, and `gen_registry --check` stays green. |
| B1/E6 | The umbrella's signing and `keyring` rules have no file that builds what they admit, though the docstring says no rule can pass that way | medium | patch | `MUST_BUILD` gains only `@polymarket-perps` → reqwest, and the three umbrella files build neither clob, relay nor keychain. This came from the three-rule deviation; the spec's one rule was covered by reqwest alone. |
| B6/E8 | The umbrella keeps `polyoxide-binance`'s default features, so a `binance-<module>` feature cannot build its module alone | low | patch | Root `Cargo.toml:26` has no `default-features = false`, unlike polymarket's at :29. Both CLI uses name `usdm-ws`, so setting it on the workspace entry is a one-line correction. |
| B11/E7 | `full` renders "all", but with `full` alone `cfg(feature = "polymarket")` and `cfg(feature = "binance")` are false. The subset heuristic behind it also misreads an alias of an alias | low | patch | `full` lists passthroughs, not the two venue features. Adding `polymarket` and `binance` to `full` makes "all" literal and lets the `_turns_on` heuristic and its `va` fixture go. |
| B8/E2/E3 | The crate docs, the umbrella README and CLAUDE.md say the prelude re-exports every enabled module's clients and errors. Relay has none, Binance's `SupervisedUsdmWs` is missing, and `RtdsError` and `PerpsWsError` were never there | low | patch | The prelude has always been a selection; the spec lists its new items. Rewording the three sentences completes this story's AD-21 rewrite, as 5.6's B5 did. |
| B10 | `BuildError`'s test cannot catch a `retry_after` that stops delegating, or a Gamma arm hard-coded to `InvalidRequest` | low | patch | Every row asserts `retry_after() == None`, and the one Gamma row is `InvalidRequest`. `ErrorResponse::new` is public, so a 429-with-`Retry-After` row and a non-`InvalidRequest` Gamma row are plain test rows. |
| B13 | The six `headers_*.rs` files say CI runs them in two builds; there are now three | low | patch | `polyoxide-polymarket/tests/headers_gamma.rs:4-6` and its siblings. A comment edit; the body diff drops comments. |
| B16 | The re-pointed manifest rows credit Stories 5.3–5.5 for paths and features that exist only since 5.7 | low | patch | E.g. "`polyoxide::polyoxide_gamma` → `polyoxide::polymarket::gamma` Story 5.3, feature `polymarket-gamma`". A text correction. |
| B18 | `pub use polyoxide_venue as venue;` has no doc comment, unlike its two siblings | low | patch | `polyoxide/src/lib.rs`: the always-on path renders bare in rustdoc. A one-line doc comment. |
| E1 | The unified builder never hands its Gamma to `ClobBuilder::gamma`, so CLOB's proxy lookup ignores `gamma_base_url` and builds a second Gamma pool | medium | defer | `clob/client.rs:919-927` builds a default Gamma when none is set. Pre-existing: the umbrella's builder did the same at v0.39.0, and the code moved verbatim. |
| B3 | The umbrella rule copies Polymarket's `sha2` placement, which a 5.5 deferral says belongs with `hmac` | low | defer | Pre-existing under 5.5's B2 entry. Fixing only the umbrella's copy would split the two tables, so the entry now covers both. |
| B15 | The root README's first line and CLAUDE.md's overview still say "for Polymarket APIs" | low | defer | Pre-existing: Binance has been a workspace crate since 0.37. Part of the fix is in an agent-context file. |
| B2 | No umbrella allowlist file holds the Binance passthroughs | low | reject | The real-table test fails if `binance-usdm` comes to admit the signing stack, and Binance's own `@usdm` and `@usdm-ws` files pin its tree. A new file and rule are more than a direct correction. |
| E5 | The umbrella's `binance-usdm` could come to build the socket stack with no failing test | low | reject | Unlikely, and the fix is a new rule plus a new file. Binance's `@usdm` file already refuses the socket stack in the crate's own REST build. |
| B4/G-O | The `full` header-pin step repeats coverage the `--all-features` workspace nextest run already gives | low | reject | True: that run's feature set is a superset. But the step is a frozen intent decision, and the cost is CI minutes. Removing it is an edit to the frozen intent. |
| B5/E4 | `umbrella_enables` counts a weak `x?/f` item as turning its module on | low | reject | Only a `full` made of weak items alone would fool it, which is unlikely, and the fix adds a branch. |
| B7 | `polymarket` and `binance` are hand-copied from each venue crate's defaults, and nothing checks them | low | reject | Venue defaults rarely change, and the fix is a new CI-script test. G2's const covers the case that matters to consumers (the default building the unified client). |
| B9 | `BuildError` is a venue-less name, re-exported into the prelude unaliased | low | reject | The frozen intent names `BuildError`. |
| B12 | `full` now builds `keychain`, and that change is recorded only in prose | false | reject | The 5.7 note in `docs/rename-manifest.md` says `full` "now covers every module of both venues and `keychain`", which is where 5.12's consumer notes read. Whether `full` should include it is frozen. |
| B14 | No Binance example compiles through the umbrella, and the README doctests are not gated by feature | low | reject | Pre-existing: a `--features rtds` doctest run failed the same way at v0.39.0. CI runs doctests `--all-features`, and a Binance example is new content. |
| B17 | The clob, perps, rtds and usdm READMEs do not mention the umbrella | low | reject | Pre-existing: only the four READMEs that named the umbrella before were re-pointed, and the others never did. |

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run alone)
- each ci.yml header-pin line, minimal and `full` -- expected: pass
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 1 retired, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `python3 scripts/venue_gate.py` and `python3 scripts/gen_registry.py --check` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
