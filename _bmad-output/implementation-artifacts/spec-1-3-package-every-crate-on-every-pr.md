---
title: 'Story 1.3: Package every crate on every PR'
type: 'feature'
created: '2026-10-08'
status: 'done'
route: 'oneshot'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-1-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Manifest faults surface only at release time, half-way through a publish. Examples are a dependency without a version, a regular path-only dependency, and missing metadata. `polyoxide-cli` has never been packaged in CI at all.

**Approach:** Add a `package` job to `ci.yml` that runs `cargo publish --workspace --dry-run --no-verify` (AD-22). The same job checks with `cargo metadata` that every publishable member has the metadata crates.io requires, because cargo only warns about that.

**Decisions (made by Claude under the user's "don't wait for me" instruction, 2026-10-08):**
- **`--no-verify`.** A verifying package job would be a cold dev-profile build of every crate with default features, several minutes long. Its only extra catch is a file missing from a `.crate`, and no manifest sets `include` or `exclude` today.
- **Review depth.** One review layer, because this is CI plumbing that never publishes (the user asked on 2026-10-08 to scale review to risk).
- **Skipping unpublishable members.** `cargo publish --workspace` skips `publish = false` members on its own, verified with cargo 1.95. So no exclude list is kept, and `polyoxide-test-support` will be skipped too when it lands.

## Boundaries & Constraints

**Always:**
- The job needs no compiler, sccache or nextest. The dry run never invokes `rustc`; it passes with `RUSTC_WRAPPER=/nonexistent`.
- It runs on every PR and push that CI already runs on.

**Never:**
- A real publish, or any registry token in this job.
- Changes to the other jobs.

</frozen-after-approval>

## Code Map

- **`.github/workflows/ci.yml`:**
  - Jobs: `format` (L18-26), `check` "Lint & Test" (L28-48), `python` (L50-64), `scripts` (L66-77).
  - Rust comes from `dtolnay/rust-toolchain@stable`.
  - The workflow-level env sets `RUSTC_WRAPPER: sccache` (L15) and `SCCACHE_GHA_ENABLED`. The dry run does not compile, so the wrapper is never invoked.
- **Measured with cargo 1.95.0:**
  - `cargo publish --workspace --dry-run --no-verify` exits 0 on today's tree in about 37 s, mostly index updates.
  - It packages 11 crates, including `polyoxide-cli`, and skips `polyoxide-py`.
  - Crates already on crates.io only warn "already exists".
- **Faults, reproduced in a throwaway workspace:**
  - A `[workspace.dependencies]` path entry without a version gives exit 101 ("does not specify a version").
  - A regular path-only dependency on a `publish = false` crate gives exit 101.
  - The same dependency with a version also gives exit 101 ("no matching package").
  - A member missing `description` or `license` **exits 0** with only a warning, so a separate check is needed.
- `CLAUDE.md` (the CI paragraph near line 42) says "CI runs four jobs …", and must name the fifth (AD-21).

## Tasks & Acceptance

**Execution:**
- [ ] `.github/workflows/ci.yml` -- a new `package` job after `check`, named "Package (publish dry run)", on `ubuntu-latest`. Its steps:
  1. `actions/checkout@v5`;
  2. `dtolnay/rust-toolchain@stable`;
  3. `cargo publish --workspace --dry-run --no-verify --locked`;
  4. the metadata check: `cargo metadata --no-deps --format-version 1 --locked`, piped to `jq -e`. It fails, naming the crates, when a publishable member (`publish != []`) lacks a `description`, or lacks both `license` and `license_file`. Print the offending names before failing.

  It has no sccache step. The workflow-level `RUSTC_WRAPPER: sccache` is never invoked, because the dry run does not compile.
- [ ] `CLAUDE.md` -- the CI paragraph at L42 adds a fifth job, **package**: `cargo publish --workspace --dry-run --no-verify` plus a metadata check. It catches manifest faults on the PR rather than mid-release, and skips `publish = false` members on its own.

**Acceptance Criteria:**
- Given today's tree, when the `package` job's two commands run locally, then both exit 0, and the dry run lists `polyoxide-cli` and not `polyoxide-py`.
- Given a member with a path-only dependency, or one missing a description or licence, when the job runs, then it fails. Prove this in a throwaway workspace under `target/`, never by committing a fault.

## Implementation Notes

- **`.github/workflows/ci.yml`:** a `package` job, "Package (publish dry run)", with `timeout-minutes: 10` and `RUSTC_WRAPPER: ""`. An empty wrapper was confirmed to disable sccache with cargo 1.95.
  - Its steps: `python3 scripts/publish_order.py check-manifests`, then `cargo publish --workspace --dry-run --no-verify --locked` under `if: !cancelled()`, so a PR sees both kinds of fault.
  - This deviates from the Tasks line's `cargo metadata | jq` check, because of the review. The jq check had no `pipefail`, so a failed `cargo metadata` passed. It also redefined "publishable" without a test. `check-manifests` reuses `is_publishable` and exits 4 when `cargo metadata` fails.
- **`scripts/publish_order.py`:** `missing_metadata()` and the `check-manifests` subcommand. **Tests:** `test_publish_order.py` (8 cases) and a new `test_ci_workflow.py`, which pins dry-run only, no secret, and no `if`/`needs`.
- **`CLAUDE.md`:** "five jobs", with the package job described.
- **Verification:**
  - the dry run on today's tree packages 11 crates, including `polyoxide-cli`, and skips `polyoxide-py`, in 12 s;
  - `check-manifests` exits 0;
  - in the throwaway probes under `target/story13-scratch`, the missing-version and path-only faults fail the dry run with exit 101, both missing-metadata probes are caught by the metadata check, and the clean workspace passes both;
  - 310 scripts tests pass.

## Spec Change Log

## Review Triage Log

- **The metadata check passes when `cargo metadata` fails (no `pipefail`).** Medium. Reproduced with `bash -e -c`. Patched by moving the check into `check-manifests`.
- **A second, untested definition of "publishable" in jq.** Low. jq used `!= []`; the script uses null or `crates-io`. Patched with the same move.
- **The job inherits the sccache wrapper.** Low. Patched with `RUSTC_WRAPPER: ""`, verified.
- **The metadata check only ran if the dry run passed.** Low. Patched: the metadata check runs first, and the dry run runs `if: !cancelled()`.
- **The job name differs from the spec.** Low. Patched to "Package (publish dry run)".
- **No `timeout-minutes`.** Low. Patched with 10.
- **The comment omits what `--no-verify` gives up.** Low. Patched, naming gitignored files and an `include_str!` outside the crate.
- **No test pins the job's rules.** Low. Patched with `test_ci_workflow.py`.
- **`main` is unprotected, so a red Package job does not block a merge.** Medium, real. Deferred: a repository setting, plus a CLAUDE.md edit.
- **CLAUDE.md lacks the local command.** Low. Deferred, because it is a CLAUDE.md edit.
- **Tombstones are not packaged.** Low today, since there are none. Deferred to Epic 5.
- **The new-name cap is not checked on PRs.** Low, rejected. The release refuses with exit 3 before uploading anything, and the check would add crates.io calls to every PR.

## Verification

**Commands:**
- `cargo publish --workspace --dry-run --no-verify --locked` -- expected: exit 0, `polyoxide-cli` packaged.
- The job's metadata-check command, copied from `ci.yml` -- expected: exit 0.
- `cd .github/scripts && uv run python -c "import yaml; yaml.safe_load(open('../workflows/ci.yml'))"` -- expected: no error.
