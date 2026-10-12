---
title: 'Story 5.1: Open the S2 integration session'
type: 'chore'
created: '2026-10-12'
status: 'done'
route: 'dispatch'
baseline_commit: 'b7b3fed0cc2d9b99080f2bd2270b92c79771ccdf'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/docs/s1-removals.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** S2 moves and renames most of the public API, and nothing reaches `main` until 5.12. The S1 removal gate fails on any removal against `v0.38.1`, and it sees a deleted crate as one `crate_missing` line. No tool proves that a moved test kept its body. So the integration branch needs CI that reports removals instead of failing on them, a rename manifest that is complete by construction, and a body-diff check, all in place before 5.2 moves anything.

**Approach:** Diff every public path at `v0.39.0` against HEAD, enumerated from rustdoc JSON of both trees. Resolve each removed path through hand rows in `docs/rename-manifest.md`, and write what is unresolved into generated regions. Report all of it without failing. Add `scripts/test_body_diff.py`, which pairs every test-context item at `v0.39.0` with its counterpart at HEAD, normalises both through the manifest's rows, and fails on a diff that has no acknowledgement.

**Decisions:**
- **Manifest granularity** (the user, 2026-10-12; relayed by the restructure-run session and confirmed at this gate). Prefix rules plus per-item rows, checked against an enumerated public-path diff, so not only cargo-semver-checks' `crate_missing` keys. prader-rs migrates from the manifest, and the spine calls it S2's definition of done.
- **A test deleted with its subject** (the user, 2026-10-12). A `[[retired]]` entry in `ci/test-body-diff.toml` names the test and the old path of a manifest row whose reason covers the removed item. That row must exist and must give a reason, not a new path. A changed body still needs a `[[drift]]` entry naming a DRIFT row. Any other lost test fails.
- **One spec** (the user, 2026-10-12), implemented in two parallel parts: the manifest gate, and the body diff.
- **Body-diff coverage widened at review** (the user, 2026-10-12). The body diff also compares:
  - test-local structs, enums, unions, type aliases and the consts and types of an impl;
  - each item's enclosing-module cfgs and its file's inner attributes;
  - each `[[test]]` and `[[example]]` stanza's fields;
  - the blob of every fixture file under `tests/fixtures/`, across the file pairs.

  `ci/test-body-diff.toml` gains a `[paths]` table for moves into non-public code, applied to the baseline side. Every entry in that table must fire.
- **The draft PR** (the user, 2026-10-12). Push `aidanb/restructure-3` and open the draft PR to `main` after 5.1 is committed and reviewed locally, so CI's first run exercises the new jobs.

## Boundaries & Constraints

**Always:**
- **Python.** Stdlib only, apart from pytest in `.github/scripts`.
- **Exit codes,** as `api_removals.py` uses them: 0 passes, 1 fails, 2 cannot decide.
- **The baseline** is `v0.39.0`, set once as `S2_BASELINE` in ci.yml. Every new job checks out with `fetch-depth: 0`.
- **Report-only.** The S2 gate (`manifest`) exits 0 on unresolved entries and stale regions, and prints each one as `::warning::`. `--strict` turns both into exit 1; 5.12 uses it.
- **Public paths** are keyed by `(kind, path)`. The enumeration covers:
  - modules, types, traits, functions, constants, statics and type aliases;
  - exported macros;
  - enum variants, inherent methods and associated constants;
  - public fields;
  - members reached through a type alias, and fields of struct variants.

  It also has these properties:
  - it follows public `use` items, globs and cross-crate re-exports, the last through the sibling crate's JSON;
  - it skips trait, blanket and synthetic impls;
  - it does not descend into ancestor aliases;
  - it includes `#[doc(hidden)]` items, built with `--document-hidden-items` and flagged as hidden;
  - every crate is built with `--all-features`;
  - it refuses a `format_version` other than 57, 60 or 61 with exit 2.
- **An entry** is a baseline path, or a baseline `<crate>/<feature>`, that HEAD does not have.
- **A row** is an old path, then `→`, then either a new path in backticks or a reason. Rows are applied by the longest prefix at a `::` boundary. A path row must map each entry it covers to a path HEAD has. A row that covers no baseline path is stale.
- **Test bodies.** The tool compares fns, consts, statics and `macro_rules!` in `tests/**/*.rs`, in `examples/` that set `test = true`, and inside `#[cfg(test)]` modules, all keyed by `(file, module path, kind, name)`.
  - **What it compares:** attributes other than doc comments, the signature and the body.
  - **What it does not:** README and doc-comment doctests.
- **Every rule CLAUDE.md states about this CI** is edited in the same commit that makes it false (AD-21).

**Never:**
- touch generated `<!-- generated:… -->` regions by hand;
- change `release.yml`'s commands;
- weaken an existing anti-vacuity assertion;
- run cargo-semver-checks or cargo-hack alongside anything that edits manifests;
- push, or open the PR, without the user's go-ahead;
- extend `test_body_diff.py` to Python tests (that is Story 5.10's, logged in deferred-work).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| No moves yet | HEAD equals `v0.39.0` | `manifest`: 0 entries, regions match; body diff: every item paired, 0 diffs, exit 0 | N/A |
| Crate moved under a rule | `polyoxide_clob` → `polyoxide_polymarket::clob`, and a path is gone from HEAD | the entry resolves; if its mapped path is missing, it is listed in the generated `unresolved` region with "points nowhere" | warning; exit 1 under `--strict` |
| Item renamed | a `DataApiError` row overrides its crate rule | longest prefix wins, and members such as `DataApiError::V2` follow the row | N/A |
| Removed outright | row `` `x::Y` → removed: never constructed (Story 5.4) `` | entry covered | an empty reason is exit 2 |
| Body changed | normalised texts differ, no acknowledgement | unified diff printed, exit 1 | N/A |
| Body changed, acknowledged | `[[drift]]` with the DRIFT row id and the diff's sha256 | passes, and prints that it was acknowledged | a row id not in `divergences.md`, or a stale sha256, is exit 1 |
| Test lost | old item with no counterpart | named; exit 1 unless a `[[retired]]` cites a manifest reason row covering its subject | a `[[retired]]` whose row is missing or maps to a path is exit 1 |
| Wrong rustdoc format | `format_version` not 57, 60 or 61 | refused | exit 2 |

</frozen-after-approval>

## Code Map

- `scripts/api_removals.py`
  - **Reuse:** `stream_command`, `parse_report`, `code_spans`, `_front_matter`, `_sections`, `_expand`, `compile_hidden`, `baseline_members`, `verify_baseline` and the error and exit handling in `main`.
  - **Leave alone:** `check`, which reads `docs/s1-removals.md` and returns in S3, and the `release` command line.
- `scripts/publish_order.py` -- `cargo_metadata`, `members`, `REPO`.
- `scripts/gen_registry.py:589` -- `lex` and `view` (and `CODE`, `STRING`), the lexer the body diff must reuse. Match braces on `view(src, CODE)` and slice from `view(src, CODE+STRING)`. This was checked: it finds all 2601 tests, and every `.rs` file balances.
  - `[workspace.metadata.polyoxide] stage` in the root `Cargo.toml` reads `"S2"`; `gen_registry.py:449` reads it.
- `<scratchpad>/rustdoc-probe/walker.py`
  - A prototype walker, verified on all 12 crates. Run on HEAD, it finds 11.7k paths, 5,197 of them the umbrella's, about 4,650 of those through whole-crate re-exports.
  - Pitfalls already handled: glob cycles, ancestor aliases, `paths` entries that point through private modules (use `index` for listing, and `paths` only for foreign ids), keying on `(kind, path)`, and `#[macro_export]` macros at the crate root.
  - Absolute path: `/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-restructure-3-18ddc6b32f684e0e/3cc99012-d044-4b36-b477-f37d9f3f0223/scratchpad/rustdoc-probe/`.
- **Building the JSON.**
  - Command: `RUSTC_BOOTSTRAP=1 cargo rustdoc -p <c> --all-features --lib --locked -- -Z unstable-options --output-format json --document-hidden-items`.
  - Local Rust 1.95 emits format 57, and CI's 1.99.0 probably emits 61; the fields the walker reads are the same in both. A cold build takes about 66 s.
  - The baseline tree comes from `git archive v0.39.0`, extracted under `target/api-removals/baseline/`. cargo-semver-checks will not build JSON for crates the workspace no longer has.
- `.github/workflows/ci.yml:116-152` -- the removals job: env `S1_BASELINE`, a cache keyed on it, and one run line.
- `.github/scripts/tests/test_ci_workflow.py:146-200` -- pins the removals job's env, command, cache key and toolchain pairing, and the `msrv`/`features`/`removals` lists.
- `.github/scripts/tests/test_api_removals.py:776-790` -- tests on the real tree that tie the listing to the job's baseline.
- `docs/s1-removals.md` -- final for S1. Its live entries under "Doc-hidden paths consumers import" seed the manifest's own section of that name.
- `_bmad-output/specs/spec-venue-extensibility/divergences.md:38-55` -- the DRIFT rows R1–R11, the only valid `[[drift]]` row ids.
- `docs/MUTANTS.md`, `test_mutants_ledger.py` -- cite tests as `file:line name`. The body diff's `--moves` prints old `file:line name` → new `file:line` for the ledger.
- `CLAUDE.md:50-56` -- the prose describing the CI jobs and the removals rule. It is outside any generated region.

## Tasks & Acceptance

**Parts.** Two implementers work in parallel in the same worktree, and each edits only its own files.
- **Part B** owns `scripts/test_body_diff.py`, `ci/test-body-diff.toml` and `.github/scripts/tests/test_test_body_diff.py`.
- **Part A** owns everything else, including the `test-bodies` job in ci.yml, its pins, CLAUDE.md and deferred-work.

The interface between them is `api_removals.read_manifest(root: Path) -> Manifest`, with `Manifest.baseline: str`, `Manifest.rows: tuple[Row, ...]` and `Row(old: str, new: str | None, reason: str | None)`. It covers `## Paths` rows only. Part A provides it first.

Part B's normaliser takes rows as an argument, so its unit tests never need A's code. Its CLI reads rows through `read_manifest` once that exists.

Neither part commits; the lead does.

**Execution:**
- [x] `scripts/public_api.py` (new)
  - **Builder:** builds rustdoc JSON for each publishable member with a library target, at a revision (`git archive`) or at HEAD. It reuses JSON when the source tree's hash and the toolchain's version match.
  - **Walker:** returns `{(kind, path): hidden}` for each crate, following re-exports through sibling JSON.
  - **Features:** each crate's `<crate>/<feature>` set, read from its manifest.
  - This goes in its own module because `api_removals.py` already runs to 765 lines.
- [x] `scripts/api_removals.py`
  - **New subcommand:** `manifest --baseline REV [--write] [--strict] [--expand FILE] [--no-semver-checks]`.
  - **What it does:**
    - computes the entries;
    - parses the rows;
    - regenerates three regions of `docs/rename-manifest.md`, each between `<!-- manifest:begin <id> -->` and its end marker:
      - `unresolved`: each entry with its kind, its reason and whether it is hidden;
      - `stale`: each row that covers no baseline path;
      - `changed-in-place`: cargo-semver-checks' non-removal lint keys, report-only;
    - prints each region's diff against the checked-in file;
    - with `--expand`, writes every entry as one TSV line: old, new or reason, and kind;
    - runs `compile_hidden` over the manifest's hidden section.
  - **`release`:** reads hidden paths from the manifest while the stage is S2, and from `docs/s1-removals.md` otherwise.
- [x] `docs/rename-manifest.md` (new)
  - Front matter: `baseline: v0.39.0`.
  - **Prose:** how to read a row, how to add one, and what each region means. It also says that only the hub regenerates the regions.
  - **Sections:**
    - `## Paths`, empty;
    - `## Features`, empty;
    - `## Doc-hidden paths consumers import`, holding s1's live entries;
    - the three generated regions.
- [x] `scripts/test_body_diff.py` (new): `[--baseline REV] [--moves]`.
  - **Pairing files:** files at the two revisions are paired by `git diff -M30% --name-status`, plus `[files]` overrides.
  - **Pairing items:** items are paired within a pair of files, and otherwise by a unique `(module path, kind, name)` match anywhere at HEAD. A duplicate key is exit 2.
  - **Normalising**, in order:
    1. remove comments, and every whole `use …;` statement;
    2. rewrite `crate::` to the file's library name;
    3. apply the manifest's path rows to the old side, and identifier renames where a row's last segment changes;
    4. replace `include_str!` and `include_bytes!` targets with the blob's sha;
    5. apply `[literals]`;
    6. collapse whitespace.
  - **Output:** per-file counts before and after; each diff, unless a matching `[[drift]]` covers it, and each lost item unless a valid `[[retired]]` covers it. Items new at HEAD are listed as information only.
- [x] `ci/test-body-diff.toml` (new): empty `[files]`, `[literals]`, `[[drift]]` and `[[retired]]` tables, with a header comment describing each field.
- [x] `.github/workflows/ci.yml`
  - **Removals job:**
    - `S2_BASELINE: v0.39.0`, and `manifest --baseline "$S2_BASELINE" --expand target/rename-map.tsv`;
    - keep the semver-checks cache, keyed on `S2_BASELINE`, and add a cache of `target/api-removals/baseline`;
    - upload the regenerated manifest and the map as artifacts, even on failure.
  - **New job `test-bodies`:** Python only, `fetch-depth: 0`, running `python3 scripts/test_body_diff.py --baseline "$S2_BASELINE"`.
- [x] `.github/scripts/tests/test_ci_workflow.py`, `test_api_removals.py`, plus new `test_public_api.py` and `test_test_body_diff.py`
  - Re-pin the removals job to S2, and pin `test-bodies` the same way the other gates are pinned.
  - Unit-test every row of the I/O matrix on synthetic rustdoc JSON and synthetic git trees. A dedicated test shows that the walker includes the hidden paths and follows the umbrella's cross-crate re-exports.
  - Real-tree tests: the manifest parses, and its baseline equals the job's.
- [x] `CLAUDE.md`, `docs/s1-removals.md`
  - CLAUDE.md: nine jobs; the removals bullet and the "Removing a public item" rule are reworded for S2 (a manifest row, report-only, `--strict` at 5.12); and the new job.
  - s1-removals.md: one line saying it is final and that S2 uses `docs/rename-manifest.md`.
- [x] `deferred-work.md`: Python body diff for 5.10; a full map, checked in or as an artifact, decided at 5.12; extend the body diff to doctests.

**Acceptance Criteria:**
- Given this branch before any move, when `manifest --baseline v0.39.0` runs, then it reports 0 entries and its regions equal the checked-in file; and when `test_body_diff.py --baseline v0.39.0` runs, then it pairs all 2601 tests and every helper with 0 diffs and exits 0.
- Given a scratch tree that renames one `pub` item and one test-body token (reverted afterwards), when both tools run, then each reports exactly one finding; and when a row and a `[[drift]]` entry are added, then both findings clear.
- Given the committed workflows, when the scripts job's pytest runs, then it is green and pins the final commands and baseline of the removals and `test-bodies` jobs.

## Spec Change Log

## Review Triage Log

Review 1, 2026-10-12. Three layers, blind (B), edge-case (E) and verification-gap (V), produced 53 findings. Two subagents verified them (`scratchpad/verified-{gate,body}.md`), and they are logged under the verifiers' ids. Where a finding came from several layers, its row lists each layer.

| Id | Finding (layers) | Verdict | Evidence | Route |
|----|------------------|---------|----------|-------|
| G01 | `release`/`check` read s1-removals outside S2 (B, E) | false | Reachable only after the commit that sets `stage = "S3"`, which is the switch planned for 5.12's follow-up. It fails loudly: `compile_hidden` names the crates that are gone. | reject |
| G02 | A row whose old path is still at HEAD is not stale (B) | low | Synthetic run: 0 entries, 0 stale. The body diff fails loudly on such a row. | patch |
| G03 | A resolved entry is not checked for signature, impl or feature meaning (B) | medium | The walker records no signatures and skips trait impls; semver-checks sees only `crate_missing`. | defer (5.12) |
| G04 | The CLI tree and Python names are not enumerated (B) | medium | `public_api` lists only Rust paths and features, but the spine's S2 row includes both. | defer (5.8, 5.10) |
| G05 | `--expand` cannot tell resolved from points-nowhere, and tabs are unescaped (B, E, V) | low | `api_removals.py:1104` writes the mapped path either way. | patch |
| G06 | The seeding test passes once any row covers a hidden path (B, V gap 2) | medium | Demonstrated: with a crate row added and both clob `DynSigner` entries deleted, the test still passes. | patch |
| G07 | A wrapped row is silently truncated (B) | low | `_sections` kept only the first line, and the reason was cut. | patch |
| G08 | "Points nowhere" is reported for a path that exists under another kind (B) | low | `resolve_entries` keys on kind (:961). | patch |
| G09 | The JSON stamp and the cache key omit the rustdoc flags (B, E, V) | medium | A change to `rustdoc_command` would reuse cached baseline JSON while HEAD's is rebuilt. | patch |
| G10 | The shared build dir may copy the other tree's JSON (E) | false | cargo re-documents on every run, since JSON output writes no `index.html` and the unit stays dirty. Shown four times. | reject |
| G11 | An interrupted re-extract leaves the old `tree.stamp` (E) | low | Demonstrated: a partial tree of rev Y was reused for X. | patch |
| G12 | Tuple-variant fields are not enumerated, and their lint is excluded from changed-in-place (E) | medium | 141 tuple variants; `V(A,B)` → `V(A)` shows nowhere. | patch |
| G13 | A sibling path missing from the index gives an item with no members (E) | false | 0 misses over 12,335 paths at both revisions; not shown to be reachable. | reject |
| G14 | A missing variant id gives a TypeError and exit 1 (E) | false | 0 missing ids in the real JSON, and the failure would be loud. | reject |
| G15 | Un-hiding a listed path blames the rustdoc format (E) | low | The `check_hidden_flags` message at :1040. | patch |
| G16 | A semver-checks error loses the artifacts (E) | false | Exit 2 by contract, and loud; no S2 path to that error was shown. | reject |
| G17 | A public → doc-hidden mapping resolves silently (E) | low | `resolved` ignores the hidden flag (:961). | patch |
| G18 | The region own-line rule is not enforced (E) | false | Stray text stays outside the region and is kept verbatim; nothing is misread. | reject |
| G19 | CI disk or time could run out (E) | maybe-false | The S1 gate took 9m08s in CI and a cold local run takes 6.5 min; about 1.4 GB more disk, not measured. The first draft-PR run settles it. | defer |
| G20 | changed-in-place includes `*_now_doc_hidden` (E) | false | Deliberate: it is the only trace of a path that became hidden. Documented in CLAUDE.md. | reject |
| G21 | AC2's "one finding" fails for items under umbrella re-exports (E) | false | Each entry is a real import path, and AC2's check used a single-path item. The fix would edit the spec. | reject |
| G22 | The revision side of `snapshot(REV)` is untested (V gap 1) | medium | Two mutations survive all tests: `tree = root`, and reading metadata from root. | patch |
| G23 | `--moves` does not map the library lines MUTANTS cites (B) | false | `test_mutants_ledger.py` pins each line's text, so a moved line fails loudly. | reject |
| T01 | Test-local types and impl consts/types are not compared (B) | medium | Changing derives, clap `#[arg]` or variants gave status 0; about 47 items. | patch (scope widened by the user) |
| T02 | Enclosing-module cfgs, file inner attributes and `[[test]]` stanzas are not compared (B, E) | medium | Each mutation gave status 0; `#![cfg(feature="ws")]` exists in clob's `live_ws.rs`. | patch (scope widened by the user) |
| T03 | Fixtures read at runtime are not compared by content (B) | medium | Only `include_*!` targets are hashed; a `fixtures!` call compares by its argument. | patch (scope widened by the user) |
| T04 | Dropping `use` hides a swapped import (B) | low | Real, but AD-12 lets a move change `use` paths, and comparing imports would give false positives with no way out. | reject |
| T05 | Stale `[literals]`/`[files]` entries never fail, contrary to the docs (B, E, V) | medium | An unused literal and a self-pair gave status 0. | patch |
| T06 | A move into private code cannot be acknowledged (B) | high | 5.2 moves `polyoxide_core::polymarket` into `src/shared/`. A reason row is not rewritten, and `[[drift]]` needs an R-row. | patch (`[paths]`, the user) |
| T07 | `[[drift]]` accepts R-rows closed in S1 (B) | low | It accepts R1–R11; a citation is visible in review, so misuse is unlikely. | reject |
| T08 | Free items under complementary cfgs are a duplicate key, exit 2 (B) | low | Loud failure, and none exist today. | reject |
| T09 | Identifier renames rewrite unrelated calls such as `Vec::new` (E, V) | medium | Demonstrated `Vec::new()` → `Vec::create()` on the baseline side. | patch |
| T10 | The trailing-comma relayout makes `(x,)` equal `(x)` (E) | low | Demonstrated; a hole in the gate with a direct fix. | patch |
| T11 | Ordinals of `const _` and invocations shift (E) | low | Latent; no module has two today. | reject |
| T12 | A non-table `literals` gives a traceback and exit 1 (E) | low | Demonstrated. | patch |
| T13 | A scope key without a trailing slash matches sibling dirs (E) | low | `polyoxide-clob` matched `polyoxide-clob-ws/`. | patch |
| T14 | Escaped literal keys never match (E) | low | Such keys are unlikely, and the fix adds unescaping. | reject |
| T15 | Duplicate `[[drift]]`/`[[retired]]` entries collapse silently (E) | low | Demonstrated status 0 with a stale duplicate. | patch |
| T16 | `+ use<'a>` is read as a `use` statement (E) | low | Inside a body it silently swallows tokens; direct fix. | patch |
| T17 | Const-generic braces in a signature (E) | low | Loud exit 2, and none exist today. | reject |
| T18 | A `[[test]]` path outside `tests/` is not a test context (E) | low | Latent; direct fix. | patch |
| T19 | The toml lacks empty `[[drift]]`/`[[retired]]` tables (E) | false | `drift = []` would forbid later `[[drift]]` tables, and the header documents why. | reject |

## Design Notes

Row grammar, matched by `code_spans` like s1's:

```
- `polyoxide_clob` → `polyoxide_polymarket::clob` Story 5.4, feature `clob`.
- `polyoxide::polyoxide_clob` → `polyoxide::polymarket::clob` Story 5.7.
- `polyoxide_data::DataApiError` → `polyoxide_polymarket::data::DataError` Story 5.3.
- `polyoxide_clob::ws::WebSocketError::ConnectionClosed` → removed: never constructed (Story 5.4).
- `polyoxide-clob/ws` → `polyoxide-polymarket/clob-ws` Story 5.4.   (## Features)
```

cargo-semver-checks stays in the S2 job, report-only. The walker cannot see signatures, and 5.12's consumer-impact notes need the changes made in place, such as `Polymarket::builder` returning `BuildError`.

## Verification

**Commands:**
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
- `CARGO_INCREMENTAL=0 env -u RUSTFLAGS python3 scripts/api_removals.py manifest --baseline v0.39.0 --no-semver-checks` -- expected: 0 entries, regions unchanged, exit 0
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost, exit 0
- `cargo fmt --all -- --check` -- expected: clean (no Rust changes)
