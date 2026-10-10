---
title: 'Story 1.2: Publish-order script and resumable releases'
type: 'feature'
created: '2026-10-08'
status: 'done'
route: 'dispatch'
review_loop_iteration: 1
baseline_commit: 'adb0e406f39e2362ce9fbc1a25b1d6733293ca68'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Releases publish a hard-coded crate list:
- `release.yml` omits `polyoxide-cli`;
- `finish_release.sh` cannot resume after a partial release;
- a version is read by regex;
- a re-used version is skipped silently;
- a fork PR on a branch named `main` can trigger a publish with the repository's tokens.

**Approach:** One stdlib script, `scripts/publish_order.py`, derives the order from `cargo metadata`. It lists only the (crate, version) pairs that crates.io lacks, and it decides whether a run releases. Both `release.yml` and `finish_release.sh` publish through one resumable loop over that list. `release.yml` gains AD-25's trigger guard, and it fails loudly on a reused tag.

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **One story.** The three acceptance groups ship together, because all of them edit `release.yml`.
- **Manual dispatch.** `workflow_dispatch` stays and is allowed only on `refs/heads/main`. On dispatch, the "did this commit change the version" test is skipped, and the tag rule still applies.
- **No verification build.** Every publish uses `--no-verify`, as `release.yml` does today, because CI has already built and tested the commit.
- **Spec length.** This spec runs to about 2,100 tokens, over the 1,600 guide. It is kept whole, because its parts share `release.yml`.
- **Repository settings.** The `cargo` and `pypi` environments' branch restriction is a GitHub setting. Neither environment has one today. Claude does not change settings, so the command goes to the user (Design Notes).

## Boundaries & Constraints

**Always:**
- Use only Python's standard library (`json`, `subprocess`, `tomllib`, `urllib`), as every existing script does.
- Treat `"publish": []` in `cargo metadata` as `publish = false`.
- A dependency with `path` set and `req == "*"` is path-only.
- Send crates.io a User-Agent naming the repository URL, and wait at least 1 s between requests (crates.io's crawler policy).
- Compare tag SHAs with the peeled `refs/tags/vX^{}` ref, because tags are annotated.

**Never:**
- Bump the version.
- Create a tag or publish locally.
- Push.
- Change GitHub settings.
- Add `cargo semver-checks` to `release.yml`; that is Story 1.7.
- Generate the CLAUDE.md publish-order region; that is Story 1.4.

</frozen-after-approval>

## Code Map

- `.github/workflows/release.yml`:
  - **Trigger (L3-8):** `workflow_run` on CI with only `branches: [main]`, plus `workflow_dispatch`.
  - **`version` job (L19-59):**
    - L21 gates on `conclusion == 'success'`.
    - L28-35 picks the SHA.
    - L37 checks out at depth 1.
    - L44 reads `VERSION` by regex.
    - L53 runs `gh release view` and skips if a release exists.
  - **`publish` job (L61-110):** `environment: cargo`.
    - L77 runs `cargo login`.
    - L83-110 loop over a hard-coded `CRATES=(…)` with no `cli`, calling `cargo search` by regex at L86, and retrying 6 times.
  - **Other jobs:** `publish-python` (L236-256, `environment: pypi`). `release` (L258-294) creates an annotated tag at L271-274, then runs git-cliff and the GitHub release action.
- `scripts/finish_release.sh`: `set -e`, then sequential `cargo publish -p` calls in a fixed order, including `cli`, with sleeps. It cannot resume.
- `.github/scripts/`:
  - uv project, Python 3.11 or later (`tomllib`), dependency `pyyaml`, `pytest` under `testpaths = ["tests"]`.
  - Tests reach the repository through `REPO = Path(__file__).resolve().parents[3]`.
  - The CI scripts job is `ci.yml` L66-75. It runs `uv run pytest tests/ -v` and has no Rust toolchain step.
- `.github/scripts/tests/test_changelog.py`: `WORKSPACE_VERSION_RE` (L54), `workspace_version()` (L78-90), and `test_parsers_still_match_something` (L186-200), which is parametrized over the regexes. Its docstring promises "no git, no network".
- `polyoxide-cli/Cargo.toml:9`: the stale description `"CLI tool for querying Polymarket Gamma API"`. Its dry-run package already succeeds.
- `cargo metadata` today:
  - 12 members, all at 0.38.1. Only `polyoxide-py` is `publish: []`.
  - The workspace-internal dev-deps (`clob→relay`, `cli→binance`, `cli→sports`) are all versioned.
  - On crates.io only `polyoxide-cli` is absent, so one name is new.
- `CLAUDE.md:440-442` (Publishing Order) and `release.yml:79` (a comment) state the hand-written order. `tombstones/` does not exist.

## Tasks & Acceptance

**Execution:**
- [x] `scripts/publish_order.py` -- new. Stdlib only. Pure functions take injected metadata, an HTTP getter and a command runner, so tests need no network.
  - **`list`** (default): prints `name version manifest_path` for each absent pair, in topological order.
    - Workspace crates come first, then `tombstones/*/Cargo.toml`, each read with `cargo metadata --manifest-path`.
    - `--max-new-names 5` exits **3** when more publishable names than that are absent. Code 3 is distinct from argparse's 2. The message cites AD-25's cap and that crates.io rate-limits new crate registrations, and says to split the release.
  - **`version`**: prints the workspace version from `cargo metadata`. It fails if the publishable members disagree.
  - **`tag-sha VERSION`**: runs `git ls-remote origin refs/tags/vX refs/tags/vX^{}`.
    - It prints the peeled SHA, or the direct one when there is no peeled line (a lightweight tag), or nothing when the tag is absent.
    - A failed `ls-remote` exits 1 with `::error::`, and never reads as "absent".
  - **`decide --head-sha SHA [--dispatch]`**: gathers its inputs, then applies the Design Notes table. It prints `release` or `skip`, or exits 1 with `::error::`. The inputs:
    - the current version, from `version`;
    - the parent version, from `git show SHA^:Cargo.toml` through `tomllib`, or none when there is no parent;
    - the tag SHA, from `tag-sha`;
    - whether a GitHub release exists, from `gh release view vX` with a 0 or 1 exit; any other exit is an error;
    - on dispatch, whether CI passed on SHA: `gh run list --workflow CI --commit SHA --json conclusion,status` has a completed `success`.
  - **Order:**
    - Follow normal and build dependencies, plus versioned dev-dependencies, but only edges whose dependency has `path` set, between publishable members.
    - Ignore path-only dev-deps.
    - A normal or build dependency on an unpublishable member exits 1, naming both crates.
    - A cycle exits 1, naming only the crates in the cycle.
    - Publishable means `publish` is null or contains `"crates-io"`.
  - **`http_status`** maps `HTTPError` to its status. It turns `URLError`, `OSError` and `http.client.HTTPException` into the script's error, so a read timeout or a reset gives `::error::` rather than a traceback.
- [x] `Cargo.toml` -- add `exclude = ["tombstones"]` to `[workspace]`. A tombstone cannot be read by `cargo metadata` without it (verified with cargo 1.95).
- [x] `scripts/finish_release.sh` -- the one resumable loop, up to 3 attempts:
  1. Recompute `list --max-new-names 5`, and stop when it is empty. Exit 3 is final; other list failures use up an attempt.
  2. Run one `cargo publish --no-verify -p A -p B …` for the workspace pairs.
  3. Run `cargo publish --no-verify --manifest-path …` for each tombstone, with an array expansion that is safe on bash 3.2.
  4. Sleep 30 s after a failure.

  The final recount gets the same 3 tries. If it cannot reach crates.io, it exits 1 with "could not confirm", which is distinct from "still unpublished". On a hand run (`GITHUB_ACTIONS` unset), it first refuses a dirty tree, or a HEAD that `git merge-base --is-ancestor HEAD origin/main` rejects after `git fetch origin main`. It accepts `CARGO_REGISTRY_TOKEN` or a prior `cargo login`.
- [x] `.github/workflows/release.yml`:
  - **Workflow level:** `concurrency: {group: release, cancel-in-progress: false}`.
  - **`version` job condition:** dispatch requires `github.ref == 'refs/heads/main'`. Otherwise all of these: `workflow_run` with `conclusion == 'success'`, `event == 'push'`, `head_branch == 'main'` and `head_repository.full_name == github.repository`.
  - **`version` job steps:** `fetch-depth: 2`, the Rust toolchain, `VERSION` from `version`, and `should_release` from `decide` (with `GH_TOKEN`).
  - **`publish` job:** set `CARGO_REGISTRY_TOKEN` in the step's `env:`, drop `cargo login`, and run `bash scripts/finish_release.sh`.
  - **`release` job:**
    - add the repository guard;
    - its tag step re-asks `tag-sha` and skips tagging when the tag is at this SHA, because re-running one job reuses stale outputs.
- [x] `polyoxide-cli/Cargo.toml` -- description `"Command-line client for the Polymarket APIs and streams, and Binance USDⓈ-M market data"`. Keywords `["polymarket", "prediction-markets", "cli", "trading", "binance"]`.
- [x] `.github/scripts/tests/test_changelog.py` -- `workspace_version()` uses `publish_order.workspace_version()`. Drop the version regex and its parametrized entry. Keep the pin regexes. Change the docstring to "no network".
- [x] `.github/workflows/ci.yml` -- the scripts job gains `dtolnay/rust-toolchain@stable`.
- [x] `.github/scripts/tests/test_publish_order.py` -- cover:
  - **Order:** topological order; a path-only dev-dep ignored; the cycle error, with a blocked crate that sorts before the cycle and is not named; a normal dependency on an unpublishable member refused; `publish: []` and `["other"]` excluded.
  - **Absent filter:** with a stub getter that records the User-Agent; more than 5 new names exits 3.
  - **Tombstones:** listed last; `tombstone_metadata` on a real temporary tree (a workspace with `exclude`).
  - **`list` against the real tree:** monkeypatch `tombstone_metadata` so a future tombstone cannot break it.
  - **`tag-sha` and `decide` lookups, with a stub runner:** peeled preferred; lightweight fallback; an `ls-remote` failure raises; no parent gives none; `gh` exit codes.
  - **`decide`:** every row of the table.
  - **`http_status`:** against a local `ThreadingHTTPServer` returning 200, 404, 429, and a dropped connection; check the User-Agent the server received.
  - **The real workspace:** `core` first, `polyoxide` after `clob`, `cli` present and `py` absent.
- [x] `.github/scripts/tests/test_finish_release.py` -- new. Copy the script into a temporary repository with stub `python3`, `cargo`, `sleep` and `git` on PATH. Assert:
  - an empty list publishes nothing and exits 0;
  - exit 3 stops at once;
  - a partial attempt followed by success publishes only the remainder;
  - pairs left after 3 attempts give exit 1;
  - one `-p` call covers the workspace, and tombstones go through `--manifest-path`;
  - a transient recount is retried;
  - a hand run refuses a dirty tree.
- [x] `.github/scripts/tests/test_release_workflow.py` -- new. Load `release.yml` with pyyaml and assert:
  - the `version` job's `if:` carries the dispatch ref check and the four `workflow_run` conditions;
  - the concurrency group;
  - the publish step's token comes from `env:`.
- [x] `CLAUDE.md`:
  - **Publishing Order:** the computed order, the one resumable loop, and the "What releases" rules from the Design Notes table. The way to recover is to dispatch Release on `main`. `finish_release.sh` by hand is a crates.io-only fallback that refuses a dirty tree or a HEAD that is not on `main`. Name the environment branch restriction as the second guard, pending a repository setting.
  - **The red-doc-build paragraph near line 46:** the next green push to `main` releases any version that is still untagged.

**Acceptance Criteria:**
- Given today's tree and crates.io, when `list` runs, then it prints exactly `polyoxide-cli 0.38.1 …`.
- Given a fork PR from a branch named `main`, when CI completes, then the `version` job is skipped.
- Given a version-bump commit whose tag exists at another SHA, when `release.yml` runs, then it fails with `::error::`, including when the tag is lightweight.
- Given a failed `ls-remote` or `gh`, when `decide` runs, then it fails rather than releasing.
- Given a re-run after a partial publish, when the loop runs, then it publishes only the remaining pairs.
- Given a green push after a red version-bump commit, including an empty commit, when it runs, then that version releases.
- Given a re-run on a commit already tagged and released, when it runs, then it skips.

## Implementation Notes

- **Deviation from the Tasks line, by loop-1 review:** `concurrency` sits on the `publish` job, not at workflow level. GitHub keeps one pending run per group, so a workflow-level group let a fork-PR or failed-CI run cancel a queued real release.
- **Additions beyond the spec, from the loop-1 review:**
  - exit 4 for deterministic `list` errors, which `finish_release.sh` does not retry;
  - a 120 s timeout on every subprocess, and `timeout-minutes: 15` on the `version` job;
  - a `ci-passed SHA` subcommand, which a hand run of `finish_release.sh` requires;
  - a versioned dev-dependency on an unpublishable member is refused.
- **From the implementer:**
  - `gh release view` exit 1 counts as "no release" only when stderr says "release not found", because a 401 also exits 1;
  - the `version` job gets `contents: read, actions: read` and `GH_REPO`.
- **Verification:**
  - 297 scripts tests pass;
  - `list` prints only `polyoxide-cli 0.38.1`;
  - the cap exits 3;
  - `tag-sha 0.38.1` prints `12e83164…`;
  - the CLI dry-run publish succeeds;
  - the implementer ran 26 mutation checks on attempt 2.

## Spec Change Log

- **Loop 1 (2026-10-08).**
  - **Trigger:** the review of attempt 1 found that the `decide` table and the loop design were wrong:
    - a re-run on a finished release released it again;
    - a fix-forward or empty-commit push after a red bump commit withheld that version, and CLAUDE.md's wedged-CI recovery is exactly such an empty commit;
    - a dispatch published `--no-verify` without CI having passed;
    - exit 2 collided with argparse;
    - a transient final recount failed the job.

    It also found these code defects: the tag lookup failed open on an `ls-remote` error or a lightweight tag, there was no concurrency group, tombstones were unreadable without a workspace `exclude`, and `http_status` let some network errors escape.
  - **Amended:** Tasks, the acceptance criteria and the Design Notes table. The git and `gh` lookups moved into the script so they can be tested, and `test_finish_release.py` and `test_release_workflow.py` were added.
  - **Known-bad states avoided:** a silent unreleased version; a duplicate release; a manual release of untested code; a reused version publishing before the tag push fails.
  - **KEEP:** attempt 1's code is at `/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-restructure-run-18dc8edd29815581/7fb3a17f-04f2-421b-a3a7-69fa742baec9/scratchpad/story-1-2-attempt1-code.diff`. Start from it, and keep:
    - the `publish_order.py` structure, its order algorithm and cycle naming, the `/crates/{name}` check for new names, the 1 s pacing and the stderr annotations;
    - the 36 existing tests;
    - the `test_changelog.py` change;
    - the `ci.yml` toolchain step and the toolchain step in the `version` job;
    - the `release.yml` guard expression and the tag re-check in the `release` job;
    - the `#!/usr/bin/env bash` shebang;
    - the CLI description;
    - most of the CLAUDE.md Publishing Order prose.

## Review Triage Log

| Finding | Verdict | Evidence | Route |
|---|---|---|---|
| A re-run on a finished release redoes every job (blind, edge) | medium | Tag at HEAD → `release` in the old table. The removed `gh release view` skip covered it. | bad_spec |
| A non-bump push skips an unreleased version silently (blind) | high | `parent == current` → `skip` even with no tag. The empty-commit recovery for a wedged CI run stops releasing. | bad_spec |
| A dispatch never checks that CI passed (blind) | medium | Dispatch goes straight to `decide`, and the publish runs `--no-verify`. | bad_spec |
| Exit 2 collides with argparse and "can't open file" (blind) | low | Both exit 2, so `finish_release.sh` treats them as final. | bad_spec |
| A transient final recount fails a complete publish (edge) | medium | `finish_release.sh:54` makes one `list` call under `set -e`. | bad_spec |
| A version revert reads as a bump and fails red (edge) | low | `parent != current` with the tag at another SHA → error. | bad_spec (table row) |
| The tag lookup fails open on an `ls-remote` error (blind, edge) | medium | The default `run` shell has no `pipefail`, and `cut` succeeds. | patch, folded into the spec |
| A lightweight tag is invisible (blind, edge, implementer) | medium | Only `refs/tags/vX^{}` is queried. | patch, folded |
| `http_status` lets read timeouts and resets escape (gap, blind, edge) | low | urllib wraps only `h.request` errors. | patch, folded |
| Tombstones are unreadable without a workspace `exclude` (blind, edge) | medium | Reproduced with cargo 1.95. `tombstone_metadata` is never exercised. | patch, folded |
| No concurrency group (blind) | low | A dispatch during a `workflow_run` release can run two loops. | patch, folded |
| `finish_release.sh` has no tests (gap, blind) | medium | `git grep` finds no test that runs it. | patch, folded |
| The git lookups feeding `decide` are untested inline bash (gap) | medium | `release.yml:69-84`; no test runs them. | patch, folded (moved into the script) |
| `http_status` itself is untested (gap) | medium | Every test injects a stub getter. | patch, folded |
| The cycle test cannot catch a broken trim (blind) | low | The cycle members sort first, so slicing from 0 still passes. | patch, folded |
| The `list` test breaks on the first tombstone (blind, edge) | low | The stub knows only workspace names. | patch, folded |
| `publish = ["other"]` is treated as crates.io (blind, edge) | low | `publishable()` accepts any non-empty list. | patch, folded |
| A registry dependency named like a member makes an edge (edge) | low | The edge test ignores `path`. | patch, folded |
| A normal dependency on an unpublishable member is dropped silently (edge) | low | The upload would fail mid-release. | patch, folded |
| A bash 3.2 empty array under `set -u` (edge) | low | Possible on a hand run on macOS. | patch, folded |
| The hand-run advice has no guard and contradicts itself (blind) | medium | CLAUDE.md gives two recovery routes, and the script publishes any tree. | patch, folded |
| The fork-guard half is untested and the environments are untracked (blind) | medium | No test pins the `if:`. The environment step lives only in the Design Notes. | patch, folded; environments → deferred-work |
| The new-name refusal gives the wrong reason (blind) | low | The message calls 6 "a mistake" and advises a hand publish. | patch, folded |
| The registry token is pasted into a shell command (blind) | low | Pre-existing `cargo login`. Trivial while the step is being rewritten. | patch, folded |
| The CLI keywords still say `gamma` (blind) | low | Its first crates.io listing. | patch, folded |
| The parent version is read differently from the current one (blind, edge) | low, rejected | Lockstep versions are a project rule (NFR1). The fix needs a worktree checkout, which is more than a direct correction. | — |
| Two test files load the module separately (blind) | low, rejected | No test raises across files; the conftest move is more than a direct correction. | — |
| Sprint status disagrees with the spec (blind) | low | Orchestrator bookkeeping, synced at step 5. | patch |
| Loop-1 review: workflow-level concurrency lets a never-releasing run cancel a pending real release (edge) | medium | GitHub keeps one pending run per group and cancels the older; fork-PR and failed-CI runs join the group before the `version` job skips. | patch: job-level on `publish` (deviates from the Tasks line; see Implementation Notes) |
| Loop-1: versioned dev-dep on an unpublishable member neither ordered nor refused (edge) | low | the `elif dep['kind'] != 'dev'` branch skips it; cargo keeps versioned dev-deps. | patch |
| Loop-1: no subprocess timeouts (edge) | low | `subprocess.run` without `timeout`; the job has no `timeout-minutes`. | patch |
| Loop-1: deterministic `list` errors retried as transient (edge) | low | a cycle or metadata error exits 1, so the loop retries it and then reports "could not confirm". | patch: exit 4 |
| Loop-1: a hand run publishes `--no-verify` without CI having passed (edge, claim) | medium | the hand-run guard checks only a clean tree and ancestry. | patch |
| Loop-1: `polyoxide-cli` is new and the token's publish-new scope is unknown (edge) | medium | `cli` publishes last, so a token without the scope fails the release after the other uploads. | defer (an operational check, in deferred-work.md) |
| Loop-1: a downgrade with no tag releases (edge) | low, rejected | a never-released lower version on `main` is what `main` says; unlikely, and the fix needs a scan of every tag. | — |
| Loop-1: a mixed-tree resume across commits (edge) | low, rejected | accepted in the Design Notes; the prior workflow behaved the same way. | — |
| Loop-1: dispatch cannot target an older commit (edge) | low, rejected | `finish_release.sh` run by hand on that commit covers the recovery. | — |
| Loop-1: `gh release view` "release not found" wording (edge, claim) | low, rejected | it fails closed and loudly if `gh` changes the wording. | — |

## Design Notes

`decide` table (the first match wins):

| # | Condition | Result |
|---|---|---|
| 1 | dispatch, and CI has not passed on HEAD | exit 1: `::error::CI has not passed on <sha>` |
| 2 | no `vX` tag | `release` |
| 3 | tag at HEAD and the GitHub release exists | `skip` (already released) |
| 4 | tag at HEAD | `release` (resume after a failed release job) |
| 5 | tag at another SHA, not dispatch, parent == current | `skip`, as a plain log line, not a `::notice::` |
| 6 | tag at another SHA, parent newer than current | `skip` with `::warning::` (the version went backwards) |
| 7 | tag at another SHA | exit 1: `::error::vX already tagged at <sha>; bump the version` |

Under rule 2, a newer commit can finish a release that failed mid-publish. The old `gh release view` workflow behaved the same way.

User step (repository settings, not done here):

```
gh api -X PUT repos/dilettante-trading/polyoxide/environments/cargo \
  -F 'deployment_branch_policy[protected_branches]=false' \
  -F 'deployment_branch_policy[custom_branch_policies]=true'
gh api -X POST repos/dilettante-trading/polyoxide/environments/cargo/deployment-branch-policies \
  -f name=main -f type=branch
```

Then repeat both commands for `pypi`.

## Verification

**Commands:**
- `cd .github/scripts && uv run pytest tests/ -v` -- expected: all pass.
- `python3 scripts/publish_order.py list` -- expected: one line, `polyoxide-cli 0.38.1 …`.
- `python3 scripts/publish_order.py list --max-new-names 0; echo $?` -- expected: `3`.
- `python3 scripts/publish_order.py tag-sha 0.38.1` -- expected: `12e83164e86aebb6270298dd25d2a77d1450f720`.
- `bash -n scripts/finish_release.sh` -- expected: no output.
- `cargo metadata --no-deps --format-version 1 >/dev/null` -- expected: success with `exclude` added.
- `cargo publish -p polyoxide-cli --dry-run --no-verify --allow-dirty` -- expected: success.
