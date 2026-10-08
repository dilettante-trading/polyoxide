---
title: 'Story 1.1: Re-baseline on current main'
type: 'chore'
created: '2026-10-08'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'aa5d34954639cf72ad399d2ef6bf695a499fb4f3'
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
- [x] Re-run the version check: `cargo metadata` workspace version and the crates.io `max_version` per publishable member, with a repository User-Agent. Record the results in Implementation Notes.
- [x] `_bmad-output/implementation-artifacts/epic-1-context.md` -- change "v0.38.1 expected" to "recorded: `v0.38.1`, commit `12e83164…`", so that Story 1.7 finds it in the context it loads.
- [x] Run `bmad-spec` update on `_bmad-output/specs/spec-venue-extensibility`:
  - the audit line becomes `e3d8c3e`, with gamma and sports re-audited at `v0.38.1` (12e8316);
  - H9 adds gamma's 2 setters;
  - T3 names gamma's `agrees`/`round_trip` pair;
  - a memlog event names the S1 start tag.
- [x] `_bmad-output/implementation-artifacts/sprint-status.yaml` -- replace the 1.1 "still open" lines with the result. Keep the note in the top block.

**Acceptance Criteria:**
- Given the branch, when `cargo metadata` and crates.io are read, then the workspace version equals the latest published version (0.38.1).
- Given Story 1.7, when it configures `--baseline-rev`, then it finds `v0.38.1` recorded as the S1 start tag, with the full commit SHA, in three places: the spec memlog, the sprint-status note and `epic-1-context.md`. It does not need to ask anyone.
- Given `duplication-inventory.md`, when it is read, then it states the gamma and sports re-audit and carries the H9 and T3 differences, written through `bmad-spec`, with its memlog recording the update.

## Implementation Notes

Version check, 2026-10-08:
- `cargo metadata`: every workspace member is 0.38.1. `polyoxide-py` is the only `publish = false` member.
- crates.io, User-Agent `polyoxide-release-check (https://github.com/dilettante-trading/polyoxide)`: `max_version` is 0.38.1 for `polyoxide`, `-core`, `-rtds`, `-sports`, `-perps`, `-binance`, `-relay`, `-gamma`, `-data` and `-clob`, all updated 2026-10-08 09:30–09:31 UTC. `polyoxide-cli` returns 404 ("does not exist").
- Tags: the latest on origin is `v0.38.1`. Tag object `34b54acac477402e9dfdcc50ade1ba79a115c14f` peels to `12e83164e86aebb6270298dd25d2a77d1450f720`, which is also `origin/main`. `v0.37.1` peels to `e3d8c3e`, the inventory's audit commit.

Re-audit, `v0.37.1..v0.38.1`:
- The diff touches only `polyoxide-gamma`, `polyoxide-sports` (`src/update.rs` doc comment and README), their `OBSERVED.md`, `CHANGELOG.md`, `CLAUDE.md`, and the version pins in `Cargo.toml` and `Cargo.lock`. Every other inventory location holds.
- The gamma setter count grew by exactly two (`git grep` count 202 to 204). The audit never recorded how it counted ~197, so the inventory states the delta: "~197 at `e3d8c3e`, +2 in v0.38".
- Before v0.38.0, `check` and `round_trip` sat at `:214` and `:264`. The inventory did not cite them, so it has no stale lines to fix.

The S1 start tag is recorded in the spec memlog (a `decision` entry), the sprint-status 1.1 note, and `epic-1-context.md`, each with the full SHA.

## Spec Change Log

## Review Triage Log

| Finding | Verdict | Evidence | Route |
|---|---|---|---|
| T3 names only `round_trip` and `agrees` as gamma's wrappers of `check` (edge-case) | medium | `wire_agreement.rs` also has six inline round-trips at :330, :339, :348, :357, :415 and :424 (Profile, UserResponse and SearchProfile), all present at v0.37.1, plus `captured_event` (:476) with direct `check` calls. `check(&` occurs 7 times at v0.37.1 and 10 times now. | patch |
| T3 omits the top-level, direction-2-only key check (blind) | medium (same root cause as above) | `sports_events_carry_no_unmodelled_top_level_keys` (:499) loops over `contains_key` and never consults `IGNORED`. A shared helper needs a top-level, one-direction mode to keep its assertion. | patch |
| H9 "~199" cannot be reproduced by the obvious grep (blind) | low | The `git grep` count is 202 → 204. The inventory never stated how ~197 was counted. | patch |
| Sprint status `in-progress` while the spec is `in-review` (edge-case, blind) | low | `sprint-status.yaml` line 59. The legend moves a story to `review` when implementation is complete. | patch |
| Sprint-status note says "ten published crates, matching cargo metadata" (blind) | low | `cargo metadata` lists 11 publishable members. `polyoxide-cli` returns 404, and the note omits it. | patch |
| "records … (recorded: …)" repeats itself in `epic-1-context.md` (blind) | low | Line 68 says "records" and "recorded" in one clause. | patch |
| `null_as_empty<T>` duplicates `deserialize_users` (blind) | low | `types.rs:119` (added in 63ddbb1, v0.38.0) and `api/user.rs:100` have the same body. This existed before this story, and no inventory row covers gamma-internal serde helpers. | defer |
| `game_id` types differ across the gamma–sports join (blind) | low | `ListEvents`/`ListKeysetEvents::game_id` take `i64`, `Event::game_id` and sports `game_id` are `u64`, and `Market::game_id` is `String`. Shipped in v0.38.0; this story did not cause it. | defer |
| The Verification commands are narrower than the criteria (blind) | rejected | The fix would edit this build's spec. Origin's tag was checked separately: `ls-remote` peels `v0.38.1` to 12e8316, and the full `v0.37.1..v0.38.1` file list was read. | — |
| Regenerating `epic-1-context.md` drops the SHA (blind) | low, rejected | Losing it there is unlikely to cost anything: the tag stays in the memlog decision, the sprint note (which survives reruns) and this spec. The fix would need a change to a planning source, which is more than a direct correction. | — |

## Verification

**Commands:**
- `cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print({p["version"] for p in json.load(sys.stdin)["packages"] if p.get("publish") != []})'` -- expected: `{'0.38.1'}`
- `git rev-parse v0.38.1^{commit}` -- expected: `12e83164e86aebb6270298dd25d2a77d1450f720`
- `git diff --stat ce2c8c6 -- '*.rs' Cargo.toml docs .github` -- expected: empty, because nothing outside `_bmad-output` changed

**Manual checks (if no CLI):**
- The inventory's `.memlog.md` has an update event for the re-audit, and `duplication-inventory.md` matches it.
