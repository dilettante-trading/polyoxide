---
title: 'Story 1.1: Re-baseline on current main'
type: 'chore'
created: '2026-10-08'
status: 'ready-for-dev'
route: 'dispatch'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The restructure branch was cut at v0.37.1, and the duplication inventory was audited there. Releases 0.38.0 and 0.38.1 have shipped since. If S1 started from the old base, it could reuse a shipped version, measure removals against the wrong API, or consolidate gamma code the inventory never saw.

**Approach:** The branch already fast-forwards to local `main` at 2ed438c, which is v0.38.1 plus the planning commit. What remains:
- confirm v0.38.1 is the latest release;
- record `v0.38.1` as the S1 start tag;
- record the gamma and sports re-audit through a `bmad-spec` update.

**Decision (human, 2026-10-08):** the S1 start tag goes in planning records only:
- the spec memlog;
- the sprint-status note;
- `epic-1-context.md`, which Story 1.7's build loads first.

Nothing outside `_bmad-output` records it. Story 1.7 writes `v0.38.1` into `ci.yml` itself.

## Boundaries & Constraints

**Always:**
- Read versions from `cargo metadata` or the crates.io API, never by regex.
- Send crates.io a User-Agent that carries the repository URL and no personal address.
- Change the inventory only through `bmad-spec` update, which re-derives the spec and companions from their memlog.

**Never:**
- Bump a version.
- Create or move a tag.
- Push.
- Edit Rust code.
- Build Story 1.7's gate. This story records the baseline it will read.
- Create anything under `docs/`, or add the tag to `Cargo.toml` or `ci.yml`.

</frozen-after-approval>

## Code Map

- `Cargo.toml` -- `[workspace.package] version = "0.38.1"`.
- Tag `v0.38.1`: annotated, `34b54ac…` on origin, pointing at `12e83164e86aebb6270298dd25d2a77d1450f720`.
- crates.io: all ten published crates report max version 0.38.1. `polyoxide-cli` has never been published; Story 4.11 publishes it.
- `_bmad-output/specs/spec-venue-extensibility/duplication-inventory.md:5` -- "audit of commit `e3d8c3e`".
- Gamma and sports rows: H2, H6–H10, H15, W1–W5, W7–W9, W11, W13–W15, T3, T9, C1–C3.
- Re-audit, `v0.37.1..v0.38.1`. Core, the CLI, the Python bindings, `scripts/` and `.github/` are byte-identical across it.
  - Sports: only `src/update.rs` (a doc comment) and the README changed, so every sports row holds.
  - Gamma H2, H10 and H15 hold: `open_enum!` is still at `types.rs:10`, and the new `HomeAway` uses it.
  - Gamma H9 grows by 2 setters: `ListEvents::game_id` and `ListKeysetEvents::include_markets` (`api/events.rs`).
  - Gamma T3: `tests/wire_agreement.rs:462` adds a generic `agrees<T>` that duplicates the Comment-only `round_trip` at `:311`. Both wrap the one `check` walker at `:261`. Story 2.9 removes the copy.
- `_bmad-output/implementation-artifacts/sprint-status.yaml` -- the note block for 1.1, and the story status.
- `_bmad-output/implementation-artifacts/epic-1-context.md` -- the "1.1 goes first" line under Cross-Story Dependencies says "v0.38.1 expected".

## Tasks & Acceptance

**Execution:**
- [ ] Re-run the version check: `cargo metadata` workspace version and the crates.io `max_version` per publishable member, with a repository User-Agent. Record the results in Implementation Notes.
- [ ] `_bmad-output/implementation-artifacts/epic-1-context.md` -- change "v0.38.1 expected" to "recorded: `v0.38.1`, commit `12e83164…`", so that Story 1.7 finds it in the context it loads.
- [ ] Run `bmad-spec` update on `_bmad-output/specs/spec-venue-extensibility`:
  - the audit line becomes `e3d8c3e`, with gamma and sports re-audited at `v0.38.1` (12e8316);
  - H9 adds gamma's 2 setters;
  - T3 names gamma's `agrees`/`round_trip` pair;
  - a memlog event names the S1 start tag.
- [ ] `_bmad-output/implementation-artifacts/sprint-status.yaml` -- replace the 1.1 "still open" lines with the result. Keep the note in the top block.

**Acceptance Criteria:**
- Given the branch, when `cargo metadata` and crates.io are read, then the workspace version equals the latest published version (0.38.1).
- Given Story 1.7, when it configures `--baseline-rev`, then it finds `v0.38.1` recorded as the S1 start tag, with the full commit SHA, in three places: the spec memlog, the sprint-status note and `epic-1-context.md`. It does not need to ask anyone.
- Given `duplication-inventory.md`, when it is read, then it states the gamma and sports re-audit and carries the H9 and T3 differences, written through `bmad-spec`, with its memlog recording the update.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print({p["version"] for p in json.load(sys.stdin)["packages"] if p.get("publish") != []})'` -- expected: `{'0.38.1'}`
- `git rev-parse v0.38.1^{commit}` -- expected: `12e83164e86aebb6270298dd25d2a77d1450f720`
- `git diff --stat ce2c8c6 -- '*.rs' Cargo.toml docs .github` -- expected: empty, because nothing outside `_bmad-output` changed

**Manual checks (if no CLI):**
- The inventory's `.memlog.md` has an update event for the re-audit, and `duplication-inventory.md` matches it.
