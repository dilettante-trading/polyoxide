# Drift heal loop — design

**Status:** approved by the owner in brainstorming, 2026-11-12; the written spec awaits their review.
**Branch:** `aidanb/self-healing-api`
**Follows:** [2026-08-15-drift-convergence-design.md](2026-08-15-drift-convergence-design.md), whose
nightly detection stands unchanged. Where this spec and an earlier record disagree, this spec binds.

## 1. Goal

`SELF-HEALING.md` describes a loop that detects and reports drift by itself; the fix is still made
by hand. Between 2026-09-11 and 2026-11-10 the nightly filed about 40 `schema-drift` issues, each
adopted by the owner or by a loom session they started. This change closes the loop for schema
drift: an open `schema-drift` issue starts a local agent session that adopts the drift, or
acknowledges it, and opens a pull request that closes the issue. The owner reviews and merges the
PR; nothing else needs a human unless the agent escalates.

**Invariant:** every open `schema-drift` issue without the `needs-human` label has a heal session
working on it or an open heal PR for it, or is waiting, in issue order, for one of the two session
slots (§3.1). Nothing a heal
does can reach `main`, change the workspace version or publish.

### 1.1 The owner's choices (2026-11-12)

| # | Question | Decision |
|---|----------|----------|
| 1 | Which layer heals? | The repair loop (maintenance time), not runtime tolerance in the SDK |
| 2 | Where does the agent run? | Locally, on the dev box, as loom sessions |
| 3 | How far does a heal go? | A PR that closes the issue (`Closes #N`); the owner merges |
| 4 | Which issues trigger it? | `schema-drift` only; `nightly-behavioral` is phase 2 |
| 5 | What does a heal PR include for additive drift? | The mirror, plus whatever existing types, fixtures and tests must change to keep agreeing; a new endpoint or channel is listed as a follow-up, not built |
| 6 | Regression or no green? | A regression gets an acknowledgement PR; a heal that cannot get green, or needs the owner's decision, is escalated with a comment and `needs-human` |
| 7 | Shape | A stateless poller plus one loom session per issue |
| 8 | Who publishes? | The session, but only through `scripts/heal_drift.py publish`, which refuses in code what a heal must never push |
| 9 | Local gates | Only the CI jobs the change touches; the PR's CI runs all ten and is the merge gate |

### 1.2 Why this is not the auto-adoption August rejected

The convergence design rejected adopting drift automatically because one finding (clob's
`example: 'Yes'`, which upstream's re-serialization made a boolean on a `type: string` field) must
never be adopted, and because Actions in this repository cannot open PRs. Here an agent judges
each finding before adopting it, a regression becomes an acknowledgement rather than an adoption,
the owner still reviews every PR before it merges, and the PR is opened from the owner's `gh`
identity on the dev box, which can open PRs (#63, #64 and #65 adopted drift that way). The nightly
workflow keeps `contents: read` and `issues: write` and still pushes nothing.

## 2. Components

| Unit | What it is | Writes |
|------|-----------|--------|
| `scripts/heal_drift.py` | One script, four subcommands: `run` (the poller), `publish`, `escalate` and `install` | `run`: starts, messages and kills heal sessions, comments nudges, escalates and closes stale heal PRs. `publish` and `escalate`: called by a session (§4) |
| `polyoxide-heal.service` and `polyoxide-heal.timer` | systemd user units, written by `heal_drift.py install`. The timer is `OnCalendar=hourly` with `Persistent=true`, so a run missed while the box was off happens at the next boot | Nothing themselves. The service runs `direnv exec <main checkout> python3 scripts/heal_drift.py run` from the main checkout, so the poller is always `main`'s version and runs on CI's tools; its log is `journalctl --user -u polyoxide-heal` |
| `.claude/skills/heal-drift/SKILL.md` | The adoption runbook (§5). Its content comes from the owner's private `polyoxide-drift-adoption` memory, which moves into the repository here | — |
| A heal session | `loom session new "heal <spec-id> <N>" --issue N --model opus --effort high --permission-mode auto --prompt "<prompt>"`, where the prompt names the skill, the issue, its `diff_sha256` and the DRIFT row reserved for it | Its own worktree and branch, and GitHub only through `publish` and `escalate` |
| `.claude/settings.json` | New, tracked. It allows exactly `Bash(python3 scripts/heal_drift.py publish:*)` and `Bash(python3 scripts/heal_drift.py escalate:*)` | — |

A heal session is one whose title the poller wrote, `heal <spec-id> <N> <R<k>|none>`, and whose
`issue` field in `loom session list --json` (with `-w polyoxide`) is that `N`; the title also
carries the DRIFT row reserved for it (§3.3). Only heal sessions are messaged or killed. Any other
session linked to the issue (one the owner started by hand) is left alone, and keeps the poller
from starting a heal for that issue. A heal PR is found by its marker (§3.2), never by title.

## 3. The poller (`heal_drift.py run`)

The poller keeps no state of its own. Each run reads:

- open and recently closed `schema-drift` issues (`gh issue list --label schema-drift --json …`);
- open and recently closed PRs whose body carries a heal marker (`gh pr list --json …`), with
  `mergeable`, `statusCheckRollup`, `headRefName`, `state` and their comments;
- the sessions (`loom session list --json -w polyoxide`).

A pure function, `decide(issues, prs, sessions, now) -> list[Action]`, turns that snapshot into
actions, and `run` executes them; `run --dry-run` prints the actions instead. Because every fact
the poller needs lives on GitHub or in loom, two runs, or a run from another checkout, never act
twice.

### 3.1 The lifecycle

The rules are tried in this order for each issue, and the first that matches wins:

| # | What the poller sees | What it does |
|---|----------------------|--------------|
| 1 | A heal PR for the issue was merged | Kills the session if one is left; `Closes #N` closed the issue |
| 2 | The issue carries `needs-human` | Nothing, whether or not a session or PR exists; the owner inspects them |
| 3 | The issue is open and a heal PR for it was closed unmerged, with no `kind=rejected` marker | Adds `needs-human` to the issue with a comment ("the heal PR was closed unmerged"), leaves a `kind=rejected` marker comment on the PR, kills the session. The owner's "no" stands until they remove the label; the marker keeps that PR from firing this rule again afterwards |
| 4 | The issue is closed (the nightly found the drift gone) and a heal PR is open | Closes the PR with a comment linking the issue, kills the session |
| 5 | The issue is closed and no heal PR is open | Kills the session if one is left |
| 6 | A heal PR is open and no session is linked to the issue | Starts a heal session on the PR's branch (`loom session new … --branch <headRefName>`) with a resume prompt, if under the cap; later runs then apply rules 7 to 10 to it |
| 7 | A heal session has been `Prompting` (waiting at a permission dialog) for over 2 hours, or `Ready` (idle) with no heal PR for over 2 hours | Escalates (§4.2): the session is stuck at a prompt, or ended its turn without publishing or escalating |
| 8 | A heal PR is open, the session is `Ready`, the issue's `diff_sha256` differs from the PR marker's, and the issue was updated after the marker's `published` time | Messages the session: upstream moved to `<sha>`, re-adopt and re-publish. The time test keeps a session that adopted a newer upstream than the issue's snapshot from being sent back to the older one |
| 9 | A heal PR is open, the session is `Ready`, and `mergeable` is `CONFLICTING` | Messages the session: merge `origin/main`, re-gate, re-publish |
| 10 | A heal PR is open, the session is `Ready`, and a check concluded `FAILURE` or `TIMED_OUT` | Messages the session the failing jobs and their log tails (`gh run view --job <id> --log-failed`, last 200 lines each) |
| 11 | No session is linked to the issue and no heal PR is open | If fewer than 2 heal sessions exist, reserves a DRIFT row (§3.3) and starts a heal session; otherwise waits for a slot |

Each message in rules 8 to 10 is also left on the PR as a comment carrying a nudge marker,
`<!-- polyoxide-heal-nudge: kind=<redrift|conflict|ci> n=<k> -->`, so the count lives on GitHub.
The third nudge of one kind on one PR escalates instead of messaging. The statuses are loom's
(`session/instance.go`): `Ready` is idle, waiting for input; `Prompting` is waiting at a permission
dialog; a `Running`, `Loading` or `Paused` heal session is left alone, a paused one because the
owner paused it.

The issues the poller reads are the open `schema-drift` issues, those closed in the last 14 days,
and any issue a heal session names; the PRs are those carrying a heal marker and updated in the
last 14 days. The owner authorises the poller, by this spec, to kill a heal session in rules 1, 3,
4 and 5; it kills no other session.

The cap of two concurrent heal sessions counts sessions whose `issue` is a `schema-drift` issue.
At the history's rate (about 40 issues in nine weeks, often two or three filed the same night) two
sessions drain a night's issues within the day.

### 3.2 The PR marker

`publish` writes this line at the end of every heal PR's body, and the poller reads it back:

```
<!-- polyoxide-heal: issue=96 spec=kalshi-ws diff_sha256=7a09…8464 drift_row=R32 published=2026-11-12T09:00:00Z -->
```

`diff_sha256` is the fingerprint of the drift the PR adopts, and `published` the UTC time of the
`publish` that wrote it. The poller reads the issue's current
fingerprint from the one `"diff_sha256": "<64 hex>"` the nightly writes into the issue body's
acknowledgement block; an issue body without exactly one is an error that the run logs and skips,
never a reason to act. `drift_row` is `none` for a heal that moves no test-context item.

### 3.3 DRIFT row reservation

DRIFT rows are numbered in sequence in
`_bmad-output/specs/spec-venue-extensibility/divergences.md`, and two branches that each take the
next number collide. The poller reserves one per new session: one past the highest row on
`origin/main` and every row named by an open heal PR's marker or by a heal session's title. Gaps
in the sequence (R29 and R30 on 2026-11-12) are never filled, since another branch may hold
them. The reservation is passed in the session's prompt and recorded in its PR's marker; a heal
that needs no row leaves the number unused, and a gap in the sequence is harmless.

## 4. Publishing and escalating

### 4.1 `heal_drift.py publish --issue N --title <t> --body-file <f>`

A session calls it when its heal is committed. In this order it:

1. finds the session whose `worktree` is the current directory (by `os.path.samefile`, since
   `/tb/Source` and `/home/aidanb/Source` name the same tree) and requires that its `issue` is `N`
   and that `N` is an open `schema-drift` issue;
2. requires a clean tree (`git status --porcelain` prints nothing);
3. runs `git fetch origin main` and requires that the workspace version (`[workspace.package]
   version` in the root `Cargo.toml`) is the same at `HEAD` and at `origin/main`, and that the
   `--drift-row` given is `none` or the row the session's title reserved;
4. requires that the diff from the merge base with `origin/main` touches none of
   `.github/workflows/release.yml`, `scripts/finish_release.sh`, `scripts/publish_order.py`,
   `scripts/heal_drift.py`, `.claude/settings.json` or `.claude/skills/heal-drift/`: a heal may
   not change the release machinery or its own guard;
5. pushes with `git push origin HEAD:refs/heads/<branch>`, where `<branch>` is the current branch
   and is not `main`; there is no code path that builds any other target, and no force push;
6. opens the PR against `main` (`gh pr create`) or, when one is open for the branch, replaces its
   body (`gh pr edit`). The script appends `Closes #N` and the marker itself, so a body cannot
   omit them, and prints the PR's URL.

Any refusal exits non-zero with the reason and pushes nothing.

### 4.2 `heal_drift.py escalate --issue N --body-file <f>`

It applies step 1 of `publish`, posts the body as a comment on the issue and adds `needs-human`,
creating the label first if it does not exist. The poller's own escalations (rules 3 and 7 and the
third nudge) call the same comment-and-label function directly, without step 1, since rule 3 runs
after the session is gone.

## 5. A heal session (`.claude/skills/heal-drift/SKILL.md`)

1. **Read.** It reads the issue (`gh issue view N`): spec id, fingerprint, the adopting `curl`.
   Then `.claude/rules/specs.md`, the venue's rules file, `ci-gates.md` (test bodies, MUTANTS) and
   `testing.md`.
2. **Adopt and re-diff.** It runs the issue's `curl`, then `diff_openapi.py check` with the
   nightly's real `--upstream-url` and `--vendored-label`, against `git show origin/main:<mirror>`,
   so its fingerprint is comparable with the issue's. A different fingerprint means upstream moved
   since 06:00 UTC; the session adopts what upstream serves now and records the new fingerprint.
   Counts, commit text and the PR body come from this diff's `summary.md`, never from the issue.
3. **Judge.** Is the finding an upstream regression: an example that contradicts its own type, a
   field removed that the live host still sends, a schema that contradicts the host's
   `OBSERVED.md`? If so, it restores the mirror, adds a `.drift-acknowledged.json` entry for the
   fingerprint with its reason, and goes to step 7 as an acknowledgement PR. A finding can be
   partly adopted and partly acknowledged; the residual diff's fingerprint is what gets
   acknowledged.
4. **Probe.** It may `curl` public GET routes, and run a live suite whose
   `[package.metadata.polyoxide.live]` entry lists no secrets. It never runs a credentialed
   `--ignored` test, an order or cancel route, or a CLOB heartbeat. A newly required field becomes
   non-`Option` only after a probe of every arm shows it; a host that disagrees with its spec is
   recorded in the directory's `OBSERVED.md`.
5. **Keep agreement.** It changes what the drift makes disagree: fields on modelled types,
   variants, fixtures re-captured from their PROVENANCE URLs with a dated note, spec-agreement
   tests (new `fn`s beside protected ones, never edits to them), the reserved DRIFT row with its
   `[[drift]]` entries and `test_test_body_diff.py` row set, MUTANTS rows with a re-proof in their
   own `docs(mutants)` commit, `covers = "…"` counts with `gen_registry.py --write`. A new
   endpoint or channel is not built: the PR body lists it under "Follow-ups".
6. **Escalate instead** (§4.2), with its analysis, when the heal would remove a public item (a
   line in `docs/s3-removals.md`, and so a version decision), needs a new public API shape, or
   cannot get its gates green after three attempts.
7. **Gate.** Through `loom job`, it runs the CI jobs the change touches, from CLAUDE.md's table:
   format always; clippy and the tests of each crate it changed; the CI scripts job when a mirror,
   `Cargo.toml` metadata or a generated region moved; test bodies when a test-context item moved.
8. **Review.** One independent reviewer subagent reads the issue and the diff against
   `origin/main`, without the session's conclusions; the session fixes what the review finds and
   it confirms, and re-runs step 7's gates.
9. **Commit and publish.** It commits by path, with subjects in the precedent's style (`docs(spec):
   adopt kalshi-ws drift …`, `feat(kalshi): …`) and no `Closes` in them. It does not edit
   `CHANGELOG.md`, which git-cliff writes at release. The PR body states what was adopted,
   acknowledged and left as a follow-up, the DRIFT row if one is used, and any change
   cargo-semver-checks would call major (a field added to an exhaustive struct), which bears on
   the owner's choice of version. Then it runs `heal_drift.py publish`.
10. **Watch.** It runs `gh pr checks <n> --watch` in the background, fixes a red check, re-gates,
    re-publishes, and then ends its turn. The poller's messages (§3.1) wake it from then on.

It never runs `git push`, `gh pr create`, `gh issue edit` or any other GitHub write directly,
never edits `main` or the main checkout, and never bumps a version.

## 6. Testing

`.github/scripts/tests/test_heal_drift.py`, in the CI scripts job:

- **`decide()`** has one case per row of §3.1, plus the rule order (a merged PR on a closed issue
  meets rule 1, not 5; a stuck session with a red PR meets rule 7, not 10; a PR the poller closed
  under rule 4 never meets rule 3), the cap, and the third nudge escalating. Its inputs are real captured
  output (`gh issue view 96 --json …`, `gh pr view --json …` of a heal-shaped PR,
  `loom session list --json`) under `.github/scripts/tests/fixtures/heal/`, so a renamed field in
  `gh` or loom fails a test rather than the timer.
- **`publish`'s refusals** each run against a temporary git repository with a bare `origin`: a
  bumped version, each guarded path touched, a dirty tree, a session for another issue, a closed
  issue. Each refusal pushes nothing, and the one accepted case pushes only to the current branch.
- **Parsing:** the fingerprint from #96's real body; a body with no fingerprint and one with two;
  the PR marker's round trip; the nudge marker's count; DRIFT reservation from `divergences.md`
  and open markers.
- **`install`** writes the two units into a temporary directory, and their text is compared with
  the expected text.

The skill has no unit test; its acceptance test is the first live run (§8).

## 7. Documentation

- `SELF-HEALING.md` gains a "Repair" section: the third branch of the loop diagram, the lifecycle
  table in brief, `needs-human` in the failure taxonomy, and operations (install, `--dry-run`, the
  journal, pausing). Its "Adopting a schema drift is a maintainer's commit" bullet becomes "a heal
  PR, or a maintainer's commit".
- `.claude/rules/heal.md`, new, with `paths:` naming `scripts/heal_drift.py`,
  `.claude/skills/heal-drift/**` and `.github/scripts/tests/test_heal_drift.py`, and its row in
  CLAUDE.md's rules table (`test_claude_rules.py` holds the two in agreement).
- `.claude/rules/specs.md`'s "adopting a drift is one `curl`" line names the heal loop.
- After the branch lands, the owner's `polyoxide-drift-adoption` memory is cut to a pointer at
  the skill.

## 8. Rollout

1. This branch adds the script, the skill, the settings, the tests and the docs; the owner lands
   it.
2. From the main checkout, `heal_drift.py run --dry-run` prints what it would do with the issues
   open then (on 2026-11-12, #94 `kalshi` and #96 `kalshi-ws`).
3. `heal_drift.py install` enables the timer. Those issues' heal PRs are the acceptance test: the
   owner reviews them as they would any adoption.
4. `systemctl --user stop polyoxide-heal.timer` pauses the loop; running sessions finish their
   turn and are left as they are.

## 9. Out of scope

- `nightly-behavioral` failures (phase 2, its own spec once this loop has run for a while).
- Building new endpoints and channels; the heal PR lists them.
- Runtime drift tolerance in the SDK.
- Running anywhere but the dev box: nothing heals while it is off, and `Persistent=true` catches
  up at boot.
