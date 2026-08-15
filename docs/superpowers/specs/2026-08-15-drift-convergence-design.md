# Drift Findings Converge — Design

**Status:** Approved (2026-08-15)
**Author:** aidanb
**Branch:** `aidanb/issues`
**Follows:** [2026-08-14-schema-drift-detection-design.md](2026-08-14-schema-drift-detection-design.md) (S1, shipped in PR #19)
**Supersedes:** the remaining PR/branch machinery from [2026-05-08-nightly-api-smoketest-design.md](2026-05-08-nightly-api-smoketest-design.md) Component 2.

## Goal

Every drift finding reaches one of three terminal states — **adopted**, **acknowledged**, or **recovered** — and nothing accumulates.

**Invariant:** an open `schema-drift` issue exists if and only if that spec has an unacknowledged finding. The workflow changes no repository state at all.

## Context — this is S2 of three

S1 made findings *true*: exact issue identity by `spec:<id>` label, key-path summaries naming what changed, and suppression of the int/float canonicalization artifact. It shipped in PR #19 and its rollout produced the evidence this spec is built on.

S3 remains: absorbing the backlog into `polyoxide-data` (the `/v1/approvals` endpoint and the `Position` fee-basis fields).

### What the S1 rollout showed

Run 31811673456 filed six issues, one per drifting spec, each correctly labelled. Of those six:

- **Five want adopting.** `perps`, `perps-ws`, `bridge`, `combos-rfq-ws` are mirror-only updates; `data` additionally needs S3 code work.
- **One must never be adopted.** `clob`'s only finding is upstream's own regression: a re-serialization dropped the quotes on `example: 'Yes'`, so YAML now reads bare `Yes` as boolean `true` on a `type: string` field. Syncing those bytes would import an invalid example into the mirror.

That single counterexample disqualifies any design that adopts drift automatically.

The same run logged **12** refusals of `gh pr create` and produced **zero** PRs. The block is not fixable here: the repo reports `can_approve_pull_request_reviews: false`, the org permissions endpoint returns 403 requiring `admin:org`, and the only repository secret is `CARGO_REGISTRY_TOKEN`. A PR-based delivery mechanism cannot work in this repository.

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | How does a finding reach review? | It doesn't need to. The issue carries the instruction; adopting is one `curl`. |
| 2 | Keep the `nightly-schema-drift/*` branches? | **No.** Delete them and stop creating them. |
| 3 | How is "we will not sync this" recorded? | A committed `docs/specs/.drift-acknowledged.json` keyed by the SHA-256 of the canonical diff. |
| 4 | Where does the acknowledgement check live? | Python, under pytest. Bash reads only an exit code. |
| 5 | Severity classification? | Dropped. YAGNI — key-path summaries already make findings readable at a glance. |

### Why no branches

The branch existed to hold the result of `--apply-on-drift`, which is a single line:

```python
shutil.copyfile(args.upstream_yaml, args.vendored_yaml)
```

Adopting a drift by hand is the equivalent one-liner:

```bash
curl -fsSL https://docs.polymarket.com/api-spec/clob-openapi.yaml -o docs/specs/clob/openapi.yaml
```

For that convenience the workflow held `contents: write` and `pull-requests: write`, force-pushed six branches nightly, and required a cleanup subsystem to undo its own accumulation. The cleanup existed only to reverse a problem nothing else needed to create.

**Accepted trade-off:** the branch was a frozen snapshot of the upstream bytes at detection time. Without it, adopting weeks later fetches whatever upstream serves that day rather than what the issue described. This is acceptable, and arguably correct: the nightly refiles daily, so the issue is never stale for long, and adopting current-upstream is more useful than adopting a stale snapshot. The issue body's canonical diff records what was reviewed.

## Architecture

Two files change. No new modules.

```
.github/scripts/diff_openapi.py          # fingerprint, acknowledgement check, paste blocks
.github/workflows/nightly-schema.yml     # delete branch/PR machinery, drop permissions
docs/specs/.drift-acknowledged.json      # new, human-authored
```

## Component 1 — `diff_fingerprint`

```python
def diff_fingerprint(diff_text: str) -> str:
    """SHA-256 of a canonical unified diff, as a hex digest."""
```

One definition of the hash, used by both `check` (to compare) and `render-issue` (to display). Computing it in bash with `sha256sum` would create a second definition of the same concept — the failure class that produced both original bugs.

**Fingerprint the disagreement, not upstream.** Hashing the diff rather than the upstream document makes the acknowledgement expire in both directions: it stops matching if upstream changes *or* if we adopt part of the drift. Hashing upstream alone would stay silent after a partial adoption, exactly when a fresh look is warranted.

## Component 2 — the acknowledgement file

`docs/specs/.drift-acknowledged.json`:

```json
{
  "clob": {
    "diff_sha256": "…",
    "reason": "Upstream re-serialization dropped the quotes on `example: 'Yes'`, so YAML reads bare `Yes` as boolean true on a `type: string` field. Adopting would import the regression.",
    "acknowledged": "2026-08-15"
  }
}
```

Only `diff_sha256` is read. `reason` and `acknowledged` are documentation for whoever reads `docs/specs/` later and asks why the mirror disagrees with upstream on purpose.

Human-authored and committed. The workflow never writes it — the act of committing is the decision record, reviewable like any other change.

## Component 3 — exit-code contract

`check` gains `--acknowledged-file <path>`. After writing `unified-diff.txt`, it fingerprints the diff and compares against this spec's entry.

| Code | Meaning | Workflow response |
|---|---|---|
| 0 | no drift | close issue — "recovered" |
| 1 | drift, unacknowledged | file or update issue |
| 2 | parse error | fail the job loudly |
| **3** | **drift, acknowledged** | close issue — "acknowledged" |

**On exit 3, `--apply-on-drift` must not run.** We have decided not to sync, so the vendored bytes must be left alone. This is the one failure mode that would silently corrupt a mirror, and it is pinned by a test.

Codes 0 and 3 converge to the same end state — no open issue — and differ only in the closing comment, so they share one workflow step rather than two.

### The step output becomes three-valued

The `Detect drift` step currently writes a boolean, `drift=true|false`, and every downstream step gates on it. Three exit codes cannot be expressed in a boolean, so it is replaced by:

```
state=clean | drift | acknowledged
```

Gating changes accordingly:

| Step | Old condition | New condition |
|---|---|---|
| Open or update drift issue | `drift == 'true'` | `state == 'drift'` |
| Converge (close issue) | `drift == 'false'` | `state != 'drift'` |
| Upload drift artifacts | `drift == 'true'` | `state != 'clean'` |

Artifacts upload for acknowledged specs too: when a hash unexpectedly fails to match, the diff that produced the mismatch is the thing you need, and it is otherwise discarded with the runner.

Leaving `drift` in place alongside `state` would create two sources of truth for the same fact — the pattern that caused both of S1's bugs. It is removed, not supplemented.

## Component 4 — paste-ready terminal states

`render-issue` gains `--adopt-url` and `--vendored-path`, and embeds two blocks in the issue body:

````markdown
### Adopt

```bash
curl -fsSL <upstream url> -o <vendored path>
```

### Or acknowledge

Add to `docs/specs/.drift-acknowledged.json`:

```json
"clob": {
  "diff_sha256": "<computed>",
  "reason": "…",
  "acknowledged": "<date>"
}
```
````

Both terminal states become copy-paste. The hash is pre-computed so declining a finding never requires reproducing the canonicalization by hand.

## Component 5 — workflow changes

- Delete the `gh pr create` / `gh pr edit` block and the `PR_BODY` assembly.
- Delete the branch checkout, commit, and force-push from the `Open or update drift PR + issue` step; rename it `Open or update drift issue`.
- Replace the boolean `drift` step output with the three-valued `state` described above, and re-gate every dependent step.
- Merge `Close stale drift PR + issue (recovery)` into a single converge step handling exit codes 0 and 3, parameterised by the closing comment.
- Stop passing `--apply-on-drift`; pass `--acknowledged-file docs/specs/.drift-acknowledged.json`.
- Pass `--adopt-url` and `--vendored-path` to `render-issue` from the existing matrix entries.
- Reduce permissions from `contents: write` + `pull-requests: write` + `issues: write` to **`contents: read` + `issues: write`**.
- Remove the `Configure git for bot commits` step, which exists only to author branch commits.

## Data flow

```
fetch upstream
  → check --acknowledged-file    canonicalize, detect, write summary.md +
                                 unified-diff.txt, fingerprint, exit 0/1/2/3
  → render-issue                 compose issue-body.md with both paste blocks
  → workflow                     exit 1 → file/update issue by spec: label
                                 exit 0/3 → close issue with the matching comment
```

## Failure handling

| Condition | Behavior |
|---|---|
| Acknowledgement file absent | Treated as no acknowledgements. Fails toward speaking up. |
| File present but malformed JSON | Same — warn and treat as unacknowledged. Never silence on a parse failure. |
| Spec absent from the file | Not acknowledged. |
| Entry present, hash stale | Not acknowledged; the finding refiles. This is the expiry working. |
| Spec parse failure | Exit 2, job fails loudly. Unchanged. |
| Upstream fetch failure | Warn and skip. Unchanged. |
| Two open issues share a `spec:` label | Lowest number wins, `::warning::` emitted. Unchanged from S1. |

The bias is deliberate and one-directional: every ambiguous state resolves toward reporting drift. Silence is only ever produced by an exact hash match.

## Testing

| Test | Pins |
|---|---|
| `test_diff_fingerprint_is_stable` | Same diff text yields the same digest across calls. |
| `test_diff_fingerprint_changes_with_diff` | A one-character change changes the digest. |
| `test_cli_check_acknowledged_exits_three` | Matching hash → exit 3. |
| `test_cli_check_stale_acknowledgement_exits_one` | Hash that no longer matches → exit 1, finding refiles. |
| `test_cli_check_missing_acknowledged_file_exits_one` | Absent file → exit 1, never silence. |
| `test_cli_check_malformed_acknowledged_file_exits_one` | Unparseable JSON → exit 1, never silence. |
| `test_cli_check_acknowledged_does_not_apply_upstream` | **Vendored bytes unchanged on exit 3.** The one silent-corruption path. |
| `test_render_issue_includes_adopt_and_acknowledge_blocks` | Both blocks present, correct URL, correct pre-computed hash. |

## Migration

1. Delete the six `nightly-schema-drift/*` branches from origin.
2. Acknowledge `clob` by committing its entry, using the hash from issue #25.
3. Leave the other five issues open; they are genuine adoption work, four mirror-only and one (`data`) feeding S3.
4. Dispatch the workflow and confirm: `clob`'s issue closes as acknowledged, the other five remain open, no branches are created, and no PR attempt is logged.

## Documentation

Update `CLAUDE.md`'s `nightly-schema.yml` bullet: it currently describes an auto-PR and a deterministic drift branch, neither of which will exist. Replace with the issue-only lifecycle, the acknowledgement file, and the reduced permissions.

## Out of scope

S3: `polyoxide-data` support for `GET /v1/approvals` and the `Position` fee-basis fields (`grossInitialValue`, `entryFeesUsdc`), plus the per-outcome `REDEEM` row semantics on `/activity`.
