---
title: 'Story 1.2: Publish-order script and resumable releases'
type: 'feature'
created: '2026-10-08'
status: 'ready-for-dev'
route: 'dispatch'
review_loop_iteration: 0
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
- [ ] `scripts/publish_order.py` -- new, with these subcommands:
  - **`list`** (default): prints `name version manifest_path` per absent pair, in topological order, workspace crates first, then `tombstones/*/Cargo.toml`, each read with `cargo metadata --manifest-path`. `--max-new-names 5` exits 2 with a message when more publishable names than that are absent.
  - **`version`**: prints the workspace version from `cargo metadata`. It fails if the publishable members disagree.
  - **`decide`**: reads `--current`, `--parent` (or none), `--tag-sha` (or none), `--head-sha` and `--dispatch`. It prints `release`, or `skip` with a notice, or exits 1 with `::error::` (decision table in Design Notes).
  - **Structure:** pure functions (order, absent filter, decision) take injected metadata and an HTTP getter, so tests need no network.
  - **Order:** follow normal and build dependencies, plus versioned dev-dependencies, between publishable members. Ignore path-only dev-deps. A cycle exits with a message naming its crates.
- [ ] `scripts/finish_release.sh` -- becomes the one resumable loop. Up to 3 attempts:
  1. Recompute `publish_order.py list --max-new-names 5`, and stop if it is empty.
  2. Run one `cargo publish --no-verify -p A -p B …` for the workspace pairs (cargo orders and waits).
  3. Run `cargo publish --no-verify --manifest-path …` for each tombstone.
  4. Sleep 30 s after a failure.

  It exits non-zero if pairs remain after the last attempt. It assumes `cargo login` has already run.
- [ ] `.github/workflows/release.yml`:
  - **`version` job condition:**
    - dispatch requires `github.ref == 'refs/heads/main'`;
    - otherwise `conclusion == 'success' && workflow_run.event == 'push' && head_branch == 'main' && head_repository.full_name == github.repository`.
  - **`version` job steps:**
    - `fetch-depth: 2`;
    - `VERSION` from `publish_order.py version`;
    - the parent version from `git show HEAD^:Cargo.toml` through `tomllib` (`[workspace.package].version`);
    - the tag SHA from `git ls-remote origin "refs/tags/v$VERSION^{}"`;
    - `should_release` from `decide`.
  - **`publish` job:** `bash scripts/finish_release.sh`, which replaces the inline loop and the L79 comment.
  - **`release` job:** creating the tag is skipped when the tag already points at this SHA (a resumed run). Add the repository guard to it as well.
- [ ] `polyoxide-cli/Cargo.toml` -- description: `"Command-line client for the Polymarket APIs and streams, and Binance USDⓈ-M market data"`.
- [ ] `.github/scripts/tests/test_changelog.py` -- `workspace_version()` calls `publish_order.workspace_version()`, loaded through `importlib` from `REPO / "scripts"`, which runs `cargo metadata --no-deps --offline`. Drop the version regex and its entry in the parametrized test. Leave the pin regexes. Update the docstring to "no network".
- [ ] `.github/workflows/ci.yml` -- the scripts job gains `dtolnay/rust-toolchain@stable`, so `cargo metadata` is present.
- [ ] `.github/scripts/tests/test_publish_order.py` -- new, built on fixture metadata. Cover:
  - topological order;
  - a path-only dev-dep ignored;
  - a versioned dev-dep cycle failing;
  - `publish: []` excluded;
  - the absent filter with a stub getter that records the User-Agent;
  - tombstones listed last;
  - more than 5 new names exiting 2;
  - every `decide` row;
  - the real workspace: `core` first, `polyoxide` after `clob`, `cli` present and `py` absent.
- [ ] `CLAUDE.md` -- the Publishing Order section: the order is computed by `scripts/publish_order.py` from `cargo metadata`; `release.yml` and `finish_release.sh` publish only the absent pairs, in one resumable loop; `polyoxide-cli` is published. Keep the stated dependency facts.

**Acceptance Criteria:**
- Given today's tree and crates.io, when `publish_order.py list` runs, then it prints exactly `polyoxide-cli 0.38.1 …`.
- Given a fork PR from a branch named `main`, when CI completes, then the `version` job is skipped and nothing publishes.
- Given a version-bump commit whose tag exists at another SHA, when `release.yml` runs, then it fails with `::error::`.
- Given a re-run after a partial publish, when the loop runs, then it publishes only the remaining pairs.
- Given the scripts job, when it runs, then `test_publish_order.py` and `test_changelog.py` pass.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Design Notes

`decide` table, in order (the first match wins):

| dispatch | parent == current | tag | result |
|---|---|---|---|
| no | yes | any | `skip` (not a version-bump commit) |
| any | — | absent | `release` |
| any | — | at HEAD | `release` (a resumed run; publishing skips crates already uploaded) |
| any | — | at another SHA | exit 1: `::error::v$VERSION already tagged at <sha>; bump the version` |

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
- `python3 scripts/publish_order.py list --max-new-names 0; echo $?` -- expected: `2`.
- `bash -n scripts/finish_release.sh && python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))"` -- expected: no output.
- `cargo publish -p polyoxide-cli --dry-run --no-verify` -- expected: success. With `--no-verify` nothing is built.
