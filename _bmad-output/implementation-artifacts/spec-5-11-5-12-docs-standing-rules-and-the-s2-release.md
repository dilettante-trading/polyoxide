---
title: 'Stories 5.11 and 5.12: docs for a multi-venue toolkit, and the S2 release'
type: 'chore'
created: '2026-10-15'
status: 'ready-for-dev'
route: 'dispatch'
baseline_commit: '85c660d727b8df262f308113f7fbb4f1d613677e'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/docs/s1-migration.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** S2's code is finished on the hub, but nothing releases it yet.
- **The docs.** Four documents still describe a single venue.
- **The old crates.** The seven retired crates have no tombstones, and nothing checks a tombstone's manifest.
- **The gates.** The removals job only reports. The 20 mutants were last proved before the moves.
- **The notes and pins.** No consumer-impact notes exist. Most README pins name `0.39` (polyoxide-ws's names `0.38`), which never resolves to the S2 layout.
- **CI and nightly.** Stories 5.3–5.9 have never run in CI, and nightly-behavioral has never run on the hub.
- **After the release.** The removal gate S3 needs is not built.

**Approach:** Finish everything that can land on the hub, `aidanb/restructure-3`. Then hand off the steps that only GitHub and `main` can take:
- on the hub, through review: the docs, the tombstones and their check, the strict gate, `docs/s2-migration.md`, the README pins, the S3 gate keyed by stage, and the mutants;
- the handoff, in order: CI green on draft PR #55, a clean nightly dispatch on the hub, one merge to `main`, the 0.40.0 bump commit, the release, then the S3 switch commit.

**Decisions:**
- **Version.** The release is 0.40.0, which raises the 0.x minor (AD-25). `origin/main` is still at 0.39.0, at b7b3fed, and the bump commit checks that again.
- **Tombstones.** Seven crates live under `tombstones/<crate>/` for `polyoxide-{clob,gamma,data,relay,perps,rtds,sports}`. Each has three files:
  - `Cargo.toml`: no dependency tables of any kind, and the workspace's metadata (edition 2021, `MIT OR Apache-2.0`, the repository, `rust-version` 1.91). Its version equals the workspace's version: 0.39.0 on the hub, and the bump commit raises it with the workspace.
  - `src/lib.rs`: a crate doc that names `polyoxide-polymarket`, the feature (`clob-ws` and `perps-ws` stand for the old `ws`), the module path, the manifest's URL and `= "0.39"` for staying put, plus one `#[cfg(not(docsrs))] compile_error!` that says the same in one line.
  - `README.md`: the same pointer, in prose.
- **The tombstone check.** CI checks only the manifests: `publish_order.py check-manifests` also reads `tombstone_metadata()`. It fails on any of these:
  - a dependency table;
  - a version unequal to the workspace's;
  - a missing description or licence;
  - a name that is not `polyoxide-<dir>`;
  - a set that is not exactly the seven.

  The package job also dry-runs each tombstone with `--manifest-path`, which packages it without compiling it (DW L22). A test pins that `gen_registry.py` skips `tombstones/`.
- **The removals job.** It gains `--strict`, once the `changed-in-place` region has been regenerated with cargo-semver-checks. That runs locally if the pinned toolchain works there; otherwise the region comes from CI's `rename-manifest` artifact after the push. The job's pin test and CLAUDE.md's removals paragraph change with it.
- **Release notes, on S1's model.**
  - `docs/s2-migration.md` carries the consumer-impact section and links the manifest.
  - It lists the seven moved tracing targets:
    - `polyoxide_core::signer_limit`, `polyoxide_clob::ws::client`, `polyoxide_relay::client`, `polyoxide_perps::ws::{client,protocol}`, `polyoxide_rtds::{event,supervisor}` and `polyoxide_sports::{client,supervised}`, each to `polyoxide_polymarket::…`;
    - `polyoxide_binance::error` to `polyoxide_binance::usdm::error`.
  - It restates the behaviour notes in deferred-work's L311, L320 and L326 with 5.7's names. L326's claim is corrected: the crate is new, and the trap is a `features = [..]` list copied without `default-features = false`.
  - The bump commit's CHANGELOG section opens with a paragraph that links the migration guide, as 0.39.0's did. `release.yml` does not change.
- **The S3 gate, prepared now.** It is keyed by stage:
  - `docs/s3-removals.md` has baseline v0.40.0, no removals, and the doc-hidden paths consumers import at their S2 paths.
  - `api_removals.py`'s `check` and `release` read the listing for the workspace's stage.
  - The switch commit after the release changes only these: it deletes `tombstones/`, sets `stage = "S3"`, makes the removals job `check` against v0.40.0, updates its pin test and CLAUDE.md's paragraph, and runs `gen_registry --write`.
- **Nightly.** The issue step of `nightly-behavioral.yml` runs only on `refs/heads/main`, so a dispatch on the hub cannot comment on main's issue or close it. No nightly issue is open, so there is nothing to close or rename.
- **Mutants.** All 20 are proved by hand on the final tree of the hub and recorded in `docs/MUTANTS.md` as "proved again on top of `<sha>`".
- **The spine amendments stay unmerged** (the user's choice, 2026-10-15).
  - A2-1 to A5-1 are not merged in this bundle, so `docs/ARCHITECTURE.md` ships with five stale guide lines: :58, :224 and :241 (Python, A5-1), :73 ("cooldown") and :102 (`allow_burst`).
  - deferred-work gets one entry: merge every amendment through `bmad-architecture`, correct A4-2 (L268), and fix the two Epic 3 lines in the spine.
- **The rename map is checked in, frozen** (the user's choice, 2026-10-15). `docs/rename-map.tsv` is written once by `manifest --expand` from the final tree and is final like `s1-removals.md`. The migration guide links it, and DW L275 is closed.
- **The handoff follows the runbook** (the user's choice, 2026-10-15).
  - The user pushes, and merges #55 as a merge commit.
  - Once CI is green, this session dispatches nightly-behavioral on the hub with `gh workflow run`.
  - A loom session on `main` makes the bump commit and, after the release, the switch commit.
- **One spec** (the user's choice, 2026-10-15), at about 4,000 tokens.
- **Not in this bundle (deferred-work gets a status line for each):**
  - L153: the hand-written enums.
  - L281: a tool that diffs the signatures behind manifest rows. The notes come from the stories' specs instead.
  - L295: `current_timestamp` stays where it is; S3 can add a clock in venue without breaking anything.

## Boundaries & Constraints

**Always:**
- AD-21: each rule a change makes false is edited in its own section.
- `test_body_diff.py --baseline v0.39.0` reports 0 diffs and 0 lost tests.
- `manifest --strict` exits 0.
- `gen_registry.py --check` passes, and generated regions change only through `--write`.
- Commit locally, and every message ends `(Stories 5.11 and 5.12)` plus the session trailers.

**Never:**
- edit `docs/ARCHITECTURE.md` or the CHANGELOG by hand;
- bump the version on the hub;
- push, tag, publish, merge or yank;
- change a Python name or any library behaviour;
- weaken a CI-script assertion;
- touch Lane B.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Old crate, new version | `polyoxide-gamma = "0.40"` | the build fails with the `compile_error!` naming `polyoxide-polymarket`, `gamma` and the manifest | compile error |
| Old crate, pinned | `polyoxide-gamma = "0.39"` | builds as before, since nothing is yanked | N/A |
| docs.rs | a tombstone built with `--cfg docsrs` | renders the pointer | N/A |
| Bad tombstone | a `[dependencies]` entry, or version 0.38.0 | `check-manifests` names the crate and the fault | exit 1 |
| Hub dispatch | nightly on `aidanb/restructure-3` with a real failure | report artifact only; no issue filed, commented on or closed | N/A |

</frozen-after-approval>

## Code Map

- **Docs (5.11).** Line numbers are from the audit at 85c660d. The four rules S2 supersedes are already rewritten (CLAUDE.md:365-370, :605, :538, :592-603).
  - `CLAUDE.md`:
    - :7, the single-venue description;
    - :302, "Polymarket API documentation", though `binance/` holds observations;
    - :542, which says mock suites exist for each module: there are none for rtds or sports;
    - :657, "second guard pending": the `cargo` and `pypi` environments are restricted to `main` now (DW L10);
    - :50, which omits the features job's specta step;
    - the Build commands block, which gains `publish_order.py check-manifests` and the dry run (L19);
    - the `[gates]` wording (L308);
    - :54's "every enclosing module's attributes" overclaim (L317), as wording only.
  - `README.md`: :3 (multi-venue); :119-128 (needs `polymarket-clob-ws`); :148 (`polyoxide::polymarket::relay`).
  - `docs/specs/INDEX.md`: :1-4 (title and source); :73, which should read `polyoxide-polymarket/tests/fixtures/{clob,relay}/session_keys/`.
  - `SELF-HEALING.md`: :3-4 (two venues), plus the auto-PR text that predates the restructure (:20-29, :108-117, :146, :152, :159-181). `nightly-schema.yml` only files issues, under `contents: read`.
  - Module READMEs:
    - `src/sports/README.md`: :54, :57;
    - `src/clob/README.md`: :5 (the org URL), :38 ("any HTTP module"), :428 (`../../../LICENSE-*`);
    - `src/relay/README.md`: :28, :320;
    - `polyoxide-core/README.md`: :20 (the keychain row).
- **Pins → `"0.40"`.**
  - `polyoxide-polymarket/README.md`: :74, :81.
  - `src/{clob,data,gamma,perps,relay,sports}/README.md`: data and gamma each have two pins.
  - `polyoxide-binance/README.md`: :23.
  - `src/usdm/README.md`: :21, :79.
  - `polyoxide-core/README.md`: :76.
  - `polyoxide-venue/README.md`: :18.
  - `polyoxide-ws/README.md`: :48 (`"0.38"`).
- **Release tooling:**
  - `scripts/publish_order.py`: `tombstone_metadata` :138, `missing_metadata` :163, `planned` :249 and `check-manifests` :550. `finish_release.sh`:41-57 already publishes the tombstones. Tests go in `.github/scripts/tests/test_publish_order.py`, and the generator-skip test beside `gen_registry`'s tests.
  - `.github/workflows/ci.yml`: the package job at :59-74, and the removals job at :190 (`--strict`), pinned by `test_ci_workflow.py`:363-368.
  - `scripts/api_removals.py`: `LISTING` :102, `check` :860, `release` :891 (S2 uses the manifest), `workspace_stage` :1122. Model `docs/s3-removals.md` on `docs/s1-removals.md`.
  - `.github/workflows/nightly-behavioral.yml`: the issue step at :240-270 gets the ref guard. Any workflow test pins it.
  - `docs/MUTANTS.md`, with its ledger `test_mutants_ledger.py`: 18 rows and 20 mutants. None of its cited lines is stale. Re-prove each by hand, as earlier stories did: run the named tests clean; apply the mutation; run `cargo test -p <crate> [--test <target>] -- <names>` and see them fail; restore; then `git diff --exit-code`. Plan for 45–90 minutes.
  - The `deferred-work.md` lines named above.

## Tasks & Acceptance

**Execution:**
- [ ] The docs above, then `gen_registry.py --write` -- 5.11.
- [ ] `tombstones/*` plus the `check-manifests` extension, its pytest and the generator-skip test, and the package-job dry run -- the release's pointers.
- [ ] Regenerate `changed-in-place`, then `--strict` with its pin test and CLAUDE.md -- the strict gate.
- [ ] `docs/s3-removals.md`, `api_removals.py` keyed by stage, and pytest -- the S3 gate.
- [ ] The nightly ref guard, with its test -- a safe hub dispatch.
- [ ] `docs/s2-migration.md`, the pins, the deferred-work status lines and the unmerged-amendments entry -- the notes.
- [ ] `docs/rename-map.tsv` from `manifest --expand` on the final tree -- the frozen map.
- [ ] Re-prove the 20 mutants last, then update `docs/MUTANTS.md` -- the AC.

**Acceptance Criteria:**
- Given the tree, when clippy, the workspace tests, rustdoc and cargo-hack run, then they pass with the per-binary test counts unchanged.
- Given the tombstones, when `cargo package --no-verify --manifest-path` runs on each, then it succeeds. `publish_order.py list` plans them after every workspace crate.
- Given the stage set to S3 in a scratch copy, when `api_removals.py check` and `release` run, then they read `docs/s3-removals.md`.
- Given each of the four docs, when it is searched for single-venue wording, then none is found.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Design Notes

**The runbook after review:**
1. You push. CI goes green on #55; on a failure, fix it on the hub.
2. Dispatch nightly on the hub. It must be clean, or only `auth-gated` or `environmental`.
3. Merge #55 with a merge commit.
4. On `main`, the bump commit:
   - `git fetch`, and check that crates.io has no 0.40.0;
   - the workspace and the seven tombstones to 0.40.0;
   - the git-cliff CHANGELOG with its intro paragraph;
   - push. The release then publishes 14 pairs, with one new name.
5. Once the GitHub release exists, the switch commit (see Decisions).

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` and `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: clean, all pass
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run it alone)
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict` -- expected: exit 0
- `python3 scripts/publish_order.py check-manifests && python3 scripts/venue_gate.py && python3 scripts/gen_registry.py --check` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
