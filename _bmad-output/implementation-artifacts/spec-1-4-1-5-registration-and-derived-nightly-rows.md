---
title: 'Stories 1.4 and 1.5: Registration metadata, the generator, and nightly rows derived from it'
type: 'feature'
created: '2026-10-08'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '70c1679714f3c8236e369287ae80a93ebf225548'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The same crate facts are written by hand in about nine places: the README table, `docs/specs/INDEX.md`, CLAUDE.md's graph, publish order, nightly list and schema exclusions, and SELF-HEALING.md's tables. They have already drifted. The exclusions are worded three ways in three files, two of which miss rtds and session-keys. The umbrella features line omits `perps-ws` and `keychain`. The publish order omits `polyoxide-cli`.

**Approach:**
- Each crate declares its facts once, in `[package.metadata.polyoxide]`.
- Mirrors live in `[workspace.metadata.polyoxide.mirrors]`.
- `scripts/gen_registry.py` rewrites only marked regions.
- A test fails CI when the committed regions differ from what the generator produces, or when venue ids collide.

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **One bundle.** The spec runs to about 4,000 tokens, over the 1,600 guide, kept whole by choice. Stories 1.4 and 1.5 ship as one unit, because the user asked on 2026-10-08 to fold stories more coarsely. Both sprint keys move together. One review layer.
- **Generated nightly jobs.** Today's matrix becomes one generated job per `(crate, suite)`: `live-<crate>-<suite>`, keeping `name: Live tests (<crate>, <suite>)`. Each job's `env:` holds exactly the secrets its targets declare, because secrets cannot vary per matrix row.
  - Today's per-row steps move unchanged into a composite action, `.github/actions/live-suite`, so the generated jobs stay short and the steps exist once.
  - The aggregate job's `needs:` becomes the generated list of live job ids. Its close guard becomes "every needed job succeeded", so a job missing from `needs` can never let it close the issue without that job's artifact.
- **Empty secrets.** An unset repository secret arrives as `""` (AD-14). The two loaders that would panic on it with a message the classifier does not treat as auth-gated are changed to treat empty as absent: `polyoxide-clob/tests/live_ws.rs` `l1_account()` and `live_session_keys.rs` `load_fixture()`. Then their existing "… required" panics fire.
- **The secrets check.** Credential loaders arrive in Epic 2, so today the check is a static scan of each live test file:
  - string literals that name environment variables;
  - plus a table for library loaders (`Account::from_env()` reads the four `POLYMARKET_*` constants in `polyoxide-clob/src/account/mod.rs`).

  The scan is one function that Epic 2 swaps for the loader calls.
- **Data's offline test.** `polyoxide-data/tests/live_api.rs`'s deliberately un-ignored offline test, `hash64_shape_matches_what_holders_accepts`, moves to a non-live test file. Its name and assertions are kept (NFR7).
- **The dependency graph.** CLAUDE.md's ASCII tree is a drawing, not the dependency graph: `polyoxide` hangs under clob, and cli and py hang under core. It becomes a generated list in publish order: `crate — readme line; needs: …; <derived notes>`. This is a deliberate correction.
- **`live` metadata.** `live.<target>` is fixed now and filled in now: `suite`, `timeout` (minutes), `features`, `secrets = []` and an optional `note`. That way CLAUDE.md's nightly list and SELF-HEALING's behavioral table can be generated. Story 1.5 derives the workflow jobs from it, and checks it.
- **Where mirror facts live.** Every mirror's details sit in the workspace table, one entry per mirror directory, holding its specs, URLs, base URLs, section, `covers` text and any exclusion reason. Crates only list the mirror directories they cover. A directory that no crate lists is crate-less.
- **The collision check.** It means unique `(venue, product)` pairs, plus ids that match `^[a-z][a-z0-9-]*$`. In S1, seven crates share the venue `polymarket`, so "one venue per crate" is wrong. `products` is a list from S1 on, because S2 merges crates and moves entries without changing the schema.
- **What stays hand-written.** INDEX.md's prose sections (undocumented hosts, session keys, rate limits) and SELF-HEALING.md's prose stay outside regions.

## Boundaries & Constraints

**Always:**
- Use only Python's standard library: `tomllib` for manifests (it keeps `[features]` order) and `cargo metadata` for dependencies.
- Import `publish_order.publish_order()` for the order; never reimplement it.
- Region markers are line-based, written with the host file's comment leader: `<!-- generated:begin <id> -->` and `<!-- generated:end <id> -->` in Markdown, `# generated:begin <id>` in YAML and TOML. Text outside markers is never touched, and indentation is preserved.
- Inline regions (CLAUDE.md's nightly list and schema exclusions) are first restructured onto their own lines.
- Keep every count `test_spec_docs.py` asserts: "Perps WebSocket (27 channels)", and the endpoint counts.

**Never:**
- Edit SELF-HEALING.md's stale prose about auto-PRs.
- Touch Rust code beyond:
  - the two test loaders above;
  - the data test move;
  - clob's `[[test]] name = "live_ws"` stanza with `required-features = ["ws"]`.
- Change a step's behaviour, an artifact name (`behavioral-<crate>-<suite>`), the artifact layout, the classifier CLI, `--run-ignored only`, `--no-fail-fast`, the libtest-json env, or the issue label logic.
- Change `spec:<id>` labels or `matrix.include` row keys in nightly-schema.

</frozen-after-approval>

## Code Map

- **`README.md:10-23`:** the crate table, `| [name](./dir) | readme line |`, sorted with `polyoxide` first.
  - Corrections: the umbrella line must name RTDS, Perps and Sports, and the cli line must name Binance.
- **`docs/specs/INDEX.md`:**
  - `:7-14`: upstream OpenAPI URLs.
  - `:20-28`: the covered table (API, Base URL, Description, Crate).
  - `:32-35`: not implemented (bridge, combos-rfq).
  - `:41-43`: other venues (binance).
  - `:90-97`: the WebSocket AsyncAPI table, with a "Covers" column.
  - `:45-82` stays prose.
  - Corrections: rtds is missing from the tables, and the AsyncAPI rows name no crate.
- **`CLAUDE.md`:**
  - `:55`: "Twelve crates…" (the count).
  - `:57-71`: the ASCII tree.
  - `:73`: CLI dependencies.
  - `:112`: umbrella features (missing `perps-ws` and `keychain`).
  - `:398`: the nightly list, inline.
  - `:403`: the watch list and "Deliberately excluded…", inline.
  - Publishing Order: rewritten by Story 1.2. Its order sentence becomes a region.
- **`SELF-HEALING.md` (repository root):**
  - `:38-48`: the behavioral table (Crate, Test binaries), with free-text notes such as "20-minute budget for its 180 s wire-agreement window".
  - `:85-95`: the schema watch table. Today it groups clob, gamma, data and relay into one row; correct this to one row per spec id.
  - `:112-123`: the exclusions bullets, which miss rtds and session-keys.
- **Today's sources:**
  - `.github/workflows/nightly-behavioral.yml:49-59`: 10 live rows, with timeouts of 15, 40 (session-keys) and 20 (sports).
  - `.github/workflows/nightly-schema.yml:51-63`: the watch list. `:32-50` holds 5 exclusions: sports, undocumented, rtds, session-keys, binance.
- **Mirror directories → specs → crates:**

  | Directory | Specs | Crate |
  |---|---|---|
  | `clob` | clob (drift-acknowledged), clob-ws-market, clob-ws-user | clob |
  | `gamma` | gamma | gamma |
  | `data` | data | data |
  | `data-v2` | data-v2 | data |
  | `relay` | relay | relay |
  | `perps` | perps, perps-ws | perps |
  | `bridge` | bridge | none |
  | `combos-rfq` | combos-rfq, combos-rfq-ws | none |
  | `sports` | excluded (the AsyncAPI does not match the wire) | sports |
  | `rtds` | excluded (nothing published) | rtds |
  | `undocumented` | excluded (no spec) | data |
  | `session-keys` | excluded (no mirror) | clob and relay |
  | `binance` | excluded (no spec) | binance |

- **Venue and product ids:**
  - venue `polymarket`, with products clob, gamma, data, relay, perps, rtds, sports (one per crate);
  - venue `binance`, with product `usdm`;
  - no venue: core, polyoxide, cli, py.
- **`.github/workflows/nightly-behavioral.yml`:**
  - Triggers: two crons and dispatch. Permissions: `contents: read`, `issues: write`. Concurrency group `nightly-behavioral`. Workflow env includes the load-bearing `NEXTEST_EXPERIMENTAL_LIBTEST_JSON: 1`.
  - The `test` job (33-127) has 10 matrix rows (50-59) of crate, suite, timeout and flags. The flags are `--features <union>` first, then the sorted `--test <target>` list, and reproduce every row exactly.
  - Its steps (60-127): checkout, toolchain, nextest, uv, `uv sync`; the first pass to `artifacts/<crate>/first-pass.json` with an empty-file guard; `classify_failures.py classify`; a retry with `-E "$(cat …/retry-filter.txt)"` or a stub; `merge`; upload `behavioral-<crate>-<suite>`.
  - `aggregate` (129-193) has `needs: [test]` and `if: always()`. It reads `merged/real-failures.txt` and `report.md` per artifact. It closes the issue only on `needs.test.result == success` (169, 187).
  - No test secrets are wired today.
- **`.github/workflows/nightly-schema.yml`:**
  - The `check` job's `matrix.include` (51-63) holds 12 flow-mapping rows `{id, url, vendored}`.
  - The exclusions are a comment block (32-50).
  - `test_spec_docs.py:126-136` parses `jobs.check.strategy.matrix.include`.
- **Live targets (13 across 10 crates; `cargo metadata` exposes `required-features`):**
  - Only perps and binance `live_ws` declare `["ws"]`. Clob's `live_ws.rs:12` is `#![cfg(feature = "ws")]` with no stanza, so without the feature it compiles to an empty binary that runs 0 tests.
  - Every test is `#[ignore]`d except `polyoxide-data/tests/live_api.rs:203`.
- **Env names read today:**
  - clob `live_api`: `Account::from_env()` → `POLYMARKET_PRIVATE_KEY`, `_API_KEY`, `_API_SECRET`, `_API_PASSPHRASE`; plus `POLYMARKET_BUILDER_CODE` (an optional soft-skip, :779);
  - clob `live_ws`: `POLYMARKET_PRIVATE_KEY` (:40);
  - clob `live_session_keys`: `POLYMARKET_DW_OWNER_PRIVATE_KEY`, `POLYMARKET_DW_WALLET`, `POLYMARKET_DW_SESSION_PRIVATE_KEY`, `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE` (optional) (:56-86);
  - relay `live_api`: `POLYMARKET_PRIVATE_KEY`, `BUILDER_API_KEY`, `BUILDER_SECRET`, `BUILDER_PASS_PHRASE`, `RELAYER_API_KEY`, `RELAYER_API_KEY_ADDRESS` (both of its loaders soft-skip);
  - every other live file reads none.
- **`classify_failures.py`:** `AUTH_GATED_RE` (:25) matches "POLYMARKET_* env vars required" and "POLYMARKET_PRIVATE_KEY required". `retry_filterset` (:206-220) parses `crate::binary$test`.
- **`scripts/publish_order.py`** (Story 1.2) provides `publish_order()` and the metadata loaders.
- **`.github/scripts/tests/test_spec_docs.py`** already pins the endpoint and channel counts. Follow its style for the real-tree test.

## Tasks & Acceptance

**Execution:**
- [x] The root `Cargo.toml` -- `[workspace.metadata.polyoxide.mirrors.<dir>]` for every directory in the Code Map:
  - `name`, `section` (`covered` | `not-implemented` | `other-venue` | `excluded`), `base_urls`, `description`;
  - an optional `crate_note`;
  - optional `exclude` (a reason sentence);
  - `specs = [{id, kind, url, vendored, covers?}]`.
- [x] Each member's `Cargo.toml` -- `[package.metadata.polyoxide]`:
  - `readme` (the README line, corrected);
  - `venue` and `products`, where they apply;
  - `mirrors` (directory ids);
  - `notes` (an optional list of free-text graph annotations that cannot be derived);
  - `[package.metadata.polyoxide.live.<target>]` for each of today's 10 nightly rows: `suite`, `timeout`, `features`, `secrets = []`, `note?`.
- [x] `scripts/gen_registry.py` -- new.
  - **Modes:** `--write` (rewrite the regions) and `--check` (exit 1 with a unified diff when they differ).
  - **Validation:** `(venue, product)` uniqueness and the id format; a crate listing an unknown mirror directory; every mirror directory existing under `docs/specs/`.
  - **Renderers**, one per region id:
    - `readme-crates`;
    - `index-upstream`, `index-covered`, `index-not-implemented`, `index-other-venues`, `index-asyncapi`;
    - `claude-crate-count`, `claude-graph`, `claude-umbrella-features` (from the umbrella's `[features]`, in order), `claude-cli-deps`, `claude-publish-order`, `claude-nightly`, `claude-schema-watch`, `claude-schema-exclusions`;
    - `selfheal-behavioral`, `selfheal-watch`, `selfheal-exclusions`.
- [x] `README.md`, `docs/specs/INDEX.md`, `CLAUDE.md`, `SELF-HEALING.md` -- insert the markers, restructure the inline lists onto their own lines, and run `--write`. CLAUDE.md gains one sentence saying these regions come from `scripts/gen_registry.py` and Cargo metadata, and must not be edited by hand (AD-21).
- [x] `.github/actions/live-suite/action.yml` -- new composite action. Inputs are `crate`, `suite` and `flags`. It holds today's per-row steps verbatim from checkout onward (the job does the checkout first), with `shell: bash` on each `run`.
- [x] `.github/workflows/nightly-behavioral.yml`:
  - **Generated region `nightly-live-jobs`:** one job per `(crate, suite)`, with job id `live-<crate>-<suite>`, `name: Live tests (<crate>, <suite>)`, `timeout-minutes` (the targets' maximum), `env:` (`NAME: ${{ secrets.NAME }}` for the union of declared secrets, sorted), and steps of checkout plus `uses: ./.github/actions/live-suite`.
  - **Generated region `nightly-aggregate-needs`:** the aggregate job's `needs:`. Its close guard uses `!contains(needs.*.result, …)` for failure, cancelled and skipped.
  - The 15-line explanatory comment stays outside the regions.
- [x] `.github/workflows/nightly-schema.yml` -- generated regions `schema-watch` (the `include:` rows `{id, url, vendored}`) and `schema-exclusions` (the comment block, one reason per excluded mirror or spec). The `spec:<id>` labels are unchanged.
- [x] Rust test edits:
  - clob `live_ws.rs` `l1_account()` and `live_session_keys.rs` `load_fixture()` treat `""` as absent;
  - clob `Cargo.toml` gains `[[test]] name = "live_ws"` with `required-features = ["ws"]`;
  - data's `hash64_shape_matches_what_holders_accepts` moves to a new non-live test file, keeping its name and body.
- [x] `.github/scripts/tests/test_live_registry.py` -- new. Cover:
  - the `live_*` targets in `cargo metadata` equal the `live` metadata entries, which equal the generated jobs;
  - every `#[test]` or `#[tokio::test]` in a `tests/live_*.rs` is `#[ignore]`d;
  - each target's `features` are within its `required-features`, and a target that needs a feature declares it;
  - the scanned env names of each live file equal its target's `secrets`, with a mutation check: renaming one literal must fail;
  - the aggregate's `needs` equals the generated job ids.
- [x] `.github/scripts/tests/test_gen_registry.py` -- new. Cover:
  - the real tree passes `--check`;
  - a hand edit inside a region fails it;
  - text outside markers is kept byte for byte;
  - indentation is kept;
  - duplicate `(venue, product)` fails, and so does a bad id;
  - an unknown mirror fails;
  - each renderer, on fixture metadata.

**Acceptance Criteria:**
- Given today's tree, when `gen_registry.py --check` runs, then it exits 0. The diff from the hand-written originals is only the listed deliberate corrections.
- Given a hand edit inside any region, when CI's scripts job runs, then it fails.
- Given two crates declaring the same `(venue, product)`, when the check runs, then it fails, naming both.
- Given the generated nightly-behavioral, when its jobs are listed, then they match today's 10 rows: crate, suite, flags and timeout, including clob's 40-minute session-keys job. Each job's `env:` carries only its declared secrets.
- Given an unset repository secret (an empty string), when a clob live test loads credentials, then it panics with an auth-gated message.
- Given a new `tests/live_x.rs` without metadata, or an un-ignored live test, or a live target missing `required-features`, or an env name not declared in `secrets`, when CI's scripts job runs, then it fails.

## Implementation Notes

- **Metadata:** 13 mirror directories, plus `[package.metadata.polyoxide]` on all 12 members, with 13 live targets in 10 jobs. Secrets are the exact env names each live file and its `mod` files read.
- **`scripts/gen_registry.py`:** stdlib only, with 21 renderers, `--write` and `--check`.
  - Validation covers: unique `(venue, product)`; id format; unknown or undeclared mirror directories; unique spec ids; a watched spec without a URL; secret names; a note's minutes against its timeout.
  - `env_names()` and `target_source()` are the secrets scan that Epic 2 replaces.
- **Additions beyond the spec:** an optional spec `note` (Data v2's "served by the API host") and a derived link page (rtds has no INDEX.md).
- **Ordering:** generated lists follow publish order, so SELF-HEALING's table and CLAUDE.md's nightly list are reordered. README stays sorted, and the 12 schema rows keep their exact order.
- **Workflows:**
  - `.github/actions/live-suite` holds today's steps; the only changes are `matrix.*` → `inputs.*` and `shell: bash`.
  - nightly-behavioral has 10 generated `live-<crate>-<suite>` jobs. The aggregate's `needs:` is generated, and the issue closes only when every needed job succeeded.
  - nightly-schema's rows are byte-identical, and its exclusions comment is generated.
- **Rust:**
  - the clob `live_ws`, `live_session_keys` and `live_api` builder-code reads treat `""` as absent;
  - clob gains `[[test]] live_ws` with `required-features = ["ws"]`;
  - data's offline test moved to `tests/holders_shape.rs`, with its name and body kept and `is_hash64` shared through `tests/common/mod.rs`.
- **Prose outside regions edited because the change made it wrong:** CLAUDE.md's dependency-facts sentence, the "matrix" wording, and SELF-HEALING's two "adding a spec or suite" bullets. SELF-HEALING's auto-PR prose is untouched.
- **Verification:**
  - `--check` exits 0;
  - 466 scripts tests pass;
  - clippy `-D warnings` on clob and data is clean, and so is fmt;
  - the moved data test passes;
  - with every secret `""`, the clob loaders panic with auth-gated wording (HEAD's `live_ws.rs` panicked "build account from private key");
  - mutations in the real tree each fail the suite: a removed stanza, an un-ignored test, an unregistered `live_x.rs`, a renamed env literal or loader constant, and a hand edit inside a region.

## Spec Change Log

## Review Triage Log

One layer (edge-case hunter), with 24 findings.

**Patched:**
- **Shared `mod` files are not scanned** (env names, `#[ignore]`). Medium. Data and binance live targets pull from `tests/common/mod.rs`.
- **Other test macros escape the ignore check.** Low. The fix is a direct regex correction.
- **`POLYMARKET_BUILDER_CODE=""` does not soft-skip** (`live_api.rs:779`). Low. The same empty-as-absent correction as the other two loaders.
- **SELF-HEALING's data-v2 row lost its "served by the API host" hint.** Low. Patched by rendering the spec's note.

**Deferred:** a partly configured credential set fails as real. Medium. `Account::from_env()` accepts empty L2 values; that is library code and AD-14's loaders in Epic 2 (deferred-work.md).

**Rejected, low:** unlikely in this workspace, and each fix adds a guard for a state nobody has shown can occur.
- **Possible crashes on unusual inputs:** an empty CLI direct-dependency list; an empty watch list; a renamed umbrella or CLI crate (`StopIteration`); a missing `docs/specs` or target file; a job-id collision from hyphenated names.
- **Unchecked or misleading metadata:**
  - a note's minutes against the suite maximum;
  - `..` in `vendored`;
  - an optional dependency enabled without `?`;
  - a member declared twice;
  - a CLI optional dependency without default features;
  - a `publish = false` crate with a versioned dev-dependency;
  - a newline or `|` in a readme string;
  - a hidden directory under `docs/specs`.
- **Scanner and test-fixture edge cases:** an attribute and its fn on one line (rustfmt forbids it); an env name inside a comment (fails loudly by demanding a declaration, never silently); a test fixture picking an empty region (none renders empty today).

**Rejected, false:**
- `full` enabling a feature only transitively: `polyoxide/Cargo.toml` lists every feature except `keychain` directly, and the region says "all but `keychain`".
- The CLAUDE.md "dependency facts" paragraph deleted: what each crate needs is now generated, and what waits for it can be read from the same list.

## Design Notes

Deliberate corrections, all of which must appear in the diff:
- the README umbrella and cli lines;
- INDEX.md's rtds rows and the AsyncAPI crate column;
- the graph as a derived list;
- the umbrella features with `perps-ws` and `keychain`;
- the publish order from `publish_order()`, with cli;
- the exclusions completed (rtds, session-keys) with one reason each;
- SELF-HEALING's watch table at one row per id.

## Verification

**Commands:**
- `python3 scripts/gen_registry.py --check` -- expected: exit 0.
- `cd .github/scripts && uv run pytest tests/ -v` -- expected: all pass, including `test_spec_docs.py`.
- `cargo metadata --no-deps --format-version 1 >/dev/null` -- expected: success.
- `cargo test -p polyoxide-data --test <new offline file>` -- expected: the moved test passes.
- `cargo check -p polyoxide-clob --tests --features ws -j 4` -- expected: success. Keep cargo target dirs out of `/tmp`.
- A pyyaml load of both nightly workflows and `.github/actions/live-suite/action.yml` -- expected: success.
