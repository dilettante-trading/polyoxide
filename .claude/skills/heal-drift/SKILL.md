---
name: heal-drift
description: Heal one schema-drift issue end to end - adopt or acknowledge the drift, keep the SDK's types, fixtures and tests agreeing with it, gate, review, and publish the PR that closes the issue through scripts/heal_drift.py. Use when a prompt asks you to invoke heal-drift for an issue (the drift heal loop's sessions), or to adopt a schema-drift issue by hand.
---

# Heal one schema-drift issue

Your prompt names the issue `N`, its spec id, the fingerprint the nightly saw and the DRIFT row
reserved for you. You heal that one issue, in this worktree, on this branch, and end at a PR
that closes it; the owner reviews and merges it. The design is
`docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md`, the rules `.claude/rules/heal.md`.

## Never

- Never `git push`, and never a `gh` write (`gh pr create`, `gh pr edit`, `gh pr comment`,
  `gh issue comment`, `gh issue edit`, `gh label`, `gh api` with a method). `python3
  scripts/heal_drift.py publish` and `escalate` are the only ways out, and they refuse what a
  heal must not do. A refusal is a fault in your heal: fix it, never work around it.
- Never touch `main` or the main checkout, bump the workspace version, edit `CHANGELOG.md`
  (git-cliff writes it at release), or change `.github/workflows/release.yml`,
  `scripts/finish_release.sh`, `scripts/publish_order.py`, `scripts/heal_drift.py`,
  `.claude/settings.json` or this skill.
- Never run a credentialed live test: no `--ignored` run of a suite whose
  `[package.metadata.polyoxide.live.<target>]` entry lists `secrets`, no order or cancel route,
  no CLOB heartbeat (`live_heartbeat` cancels every resting order on the account).
- Never edit a watched mirror by hand, beyond restoring a regressed line (step 3): it holds
  upstream's bytes, from the issue's `curl`.
- Never edit a protected test item to add coverage: add a new `fn` or `const` beside it (AD-12).
- Builds, tests, clippy and benchmarks run through loom's queue on CI's tools:
  `loom job run -d "<what>" -- direnv exec . <command>`. Keep scratch files outside the worktree
  (`mktemp -d`): `publish` refuses an untracked file.

## 1. Read

- `gh issue view N`: the `spec:<id>` label, the fingerprint (`diff_sha256` in the "Or
  acknowledge" block), the `curl`, and the key-path summary.
- The spec's entry in the root `Cargo.toml`'s `[workspace.metadata.polyoxide.mirrors]`: its
  `url`, `vendored` path and `covers`.
- `.claude/rules/specs.md`; the venue's rules file (CLAUDE.md's table names it);
  `.claude/rules/ci-gates.md`, its "test bodies" and venue-gate sections;
  `.claude/rules/testing.md`; the directory's `OBSERVED.md`; and `docs/MUTANTS.md`'s rows for the
  files you will touch.

## 2. Adopt and re-diff

The issue describes upstream at 06:00 UTC, and upstream often moves again before a heal. Write
every count, commit line and PR sentence from your own diff, never from the issue.

```bash
R=$(git rev-parse --show-toplevel); S=$(mktemp -d)
git show origin/main:<vendored> > "$S/old"
<the issue's curl>        # writes upstream's raw bytes over <vendored>
(cd "$R/.github/scripts" && uv run python diff_openapi.py check --crate <id> \
  --upstream-yaml "$R/<vendored>" --vendored-yaml "$S/old" \
  --upstream-url "<url>" --vendored-label "<vendored>" --output-dir "$S/<id>"); echo "exit $?"
```

- Exit 1 is drift. Read `$S/<id>/summary.md` and `unified-diff.txt`. `$S/<id>/diff-sha256.txt`
  is your fingerprint, the one you publish; with the nightly's real `--upstream-url` and
  `--vendored-label` it equals the issue's unless upstream moved, and if it moved you adopt what
  upstream serves now.
- Exit 0 means upstream matches `main`'s mirror again: restore the mirror
  (`git checkout -- <vendored>`) and escalate (step 6) with that finding; the next nightly
  closes the issue.

## 3. Judge

Decide, finding by finding, whether upstream changed its contract or broke its own document. A
regression is an example that contradicts its own type (clob's `example: 'Yes'`, which
upstream's re-serialization made boolean `true` on a `type: string` field), a field removed that
the live host still sends, or a schema that contradicts what `OBSERVED.md` records the host
doing. Probe (step 4) before you call anything a regression.

- **Adopt** what upstream changed: steps 4 to 10.
- **Acknowledge** a regression: restore only the regressed lines in the mirror (or the whole
  mirror, if nothing else changed), re-run step 2's check with `--output-dir "$S/residual"`, and
  add an entry for the residual fingerprint to `docs/specs/.drift-acknowledged.json`:
  `"<id>": {"diff_sha256": "<residual fingerprint>", "reason": "<what upstream broke, and how
  you know>", "acknowledged": "<today>"}`. The check exits 3 on it, and the nightly closes the
  issue. Publish step 2's fingerprint, not the residual one. Then steps 7 to 10.

## 4. Probe

You may `curl` a public GET route of the host, and run a live suite whose
`[package.metadata.polyoxide.live.<target>]` entry lists no `secrets`
(`loom job run -d "<suite>" -- direnv exec . cargo test -p <crate> --features <features> --test
<suite> -- --ignored`). Nothing else touches a venue.

- A newly required response field becomes non-`Option` only after a probe of every arm shows it
  (for positions: OPEN, CLOSED, REDEEMABLE and market-anchored); otherwise it goes in as
  `Option`. A newly required socket key goes in as `Option`, with an `ABSENT_ON_THE_WIRE` row
  until a capture shows it.
- A `nullable: true` array response means a bare `null` 200, which breaks a `Vec<T>` decode.
- A description-only change can record real behaviour (a bare `?cursor=` that began to 400):
  probe it.
- A small drift can be the edge of a new surface: read the served schema and the page
  `docs/specs/polymarket-llms.txt` lists for it before modelling.
- Probe a repeated query key in both orders before trusting its precedence, and cap any scan
  (`/markets/keyset` has over 40k open markets).
- Record what the host does that its spec does not say in the directory's `OBSERVED.md`, dated.

## 5. Keep agreement

Change what the drift makes disagree, and nothing else.

- **Types:** a field upstream added to a type the crate models (`Option` unless step 4 proved it
  always present); a variant a closed `wire_enum!` must now accept (an `open_enum!` already keeps
  an unknown spelling); a param that gains a value becomes an additive typed setter (gamma's
  `closed_filter`), never a changed signature.
- **Fixtures:** run the module's `scripts/capture_*.py` into scratch, key-path-diff it against the
  committed set, re-capture only the changed files from their PROVENANCE URLs, and add a dated
  note to the PROVENANCE.
- **Spec-agreement tests** read the mirror; one that now fails is the drift reaching a modelled
  type. Fix the type, or add a row with its reason to the test's documented allow-list.
- **Test bodies (AD-12):** a protected item that must change (a re-captured fixture, a mock body
  missing a newly required field, the `SPEC` blob a spec-agreement test reads by `include_str!`)
  takes your reserved DRIFT row. In `_bmad-output/specs/spec-venue-extensibility/divergences.md`,
  append `, <row> <today>` inside the parenthesis of the `## DRIFT: decided …` heading and add
  the row to the table after the highest one; add the row to the set
  `.github/scripts/tests/test_test_body_diff.py` asserts `config.drift`'s rows within, with a
  clause in the comment above it; stage the changed files, run `cargo fmt --all`, then
  `python3 scripts/test_body_diff.py --baseline v0.40.0`, and turn each `::error::` line into a
  `[[drift]]` entry in `ci/test-body-diff.toml` (`file`, `item`, `row`, `sha256`). An item
  already under a DRIFT row that moves again has its sha re-pinned and its row switched to yours,
  whose text says it holds both changes.
- **MUTANTS:** a change that moves a line or test `docs/MUTANTS.md` cites moves its row and
  `test_mutants_ledger.py`'s `SNIPPETS`, re-proved, in a `docs(mutants)` commit of its own.
  Append new mock tests at the end of their suite, so cited lines stay put.
- **Counts:** a mirror's endpoint or channel count comes from `covers = "…"` in the root
  `Cargo.toml`'s mirrors metadata: edit it and run `python3 scripts/gen_registry.py --write`.
  Never edit a generated region.
- **Surface a test forces** (perps' `every_info_route_has_a_builder_entry`, for a new `/v1/info`
  route) follows its module's existing pattern exactly; numeric strings go in perps' suite's
  `NUMERIC_STRINGS` table. If the pattern does not settle the shape, escalate.
- **New surface nothing forces** (an endpoint, channel or route) is not built: list it under
  "Follow-ups" in the PR body, with its path and one line on what it serves.

## 6. Escalate instead

Escalate, with your analysis, when the heal would remove a public item (a `docs/s3-removals.md`
line, and so a version decision), needs a public API shape no existing pattern settles, or cannot
get its gates green after three attempts, and when step 2 found the drift gone.

```bash
B=$(mktemp)   # what drifted, what you found, what the owner must decide
python3 scripts/heal_drift.py escalate --issue N --body-file "$B"
```

Leave your work committed on the branch, where the owner can read it, and end your turn. The
issue is the owner's from then on, and the poller leaves it alone.

## 7. Gate

Through `loom job run -d "<what>" -- direnv exec . <command>`, run the CI jobs your change touches
(CLAUDE.md's table holds each job's commands):

- always: `cargo fmt --all -- --check`;
- for each crate you changed: `cargo clippy -p <crate> --all-targets --all-features -- -D
  warnings`, `cargo nextest run -p <crate> --all-features` and `cargo test --doc -p <crate>
  --all-features`;
- the CI scripts job, `sh -c 'cd .github/scripts && uv run pytest tests/'`, when a mirror,
  `Cargo.toml` metadata, a generated region or anything under `docs/specs/` moved;
- test bodies, `python3 scripts/test_body_diff.py --baseline v0.40.0` after staging moved files,
  when a test-context item moved;
- the features job's header pins (`.claude/rules/ci-gates.md`) when a request changed.

The PR's CI runs all ten jobs; it is the merge gate.

## 8. Review

Dispatch one reviewer subagent (`flow:code-quality-reviewer`) with the issue number, the spec id
and `git diff origin/main...HEAD`, not your conclusions, and ask it to check the adoption against
the mirror diff and the rules files of step 1. Fix what it finds that you can confirm, re-run
step 7's gates, and do not review again.

## 9. Commit and publish

- Commit by path (`git add <paths>`, `git commit -- <paths>`), never `-a`, `-A` or `.`, with
  subjects in the precedent's style: `docs(spec): adopt <id> drift: <what>` for a heal that
  changed no code, `feat(<venue>): …` or `fix(<venue>): …` when code changed. No `Closes` in a
  subject; `publish` adds it to the PR body.
- Write the PR body to `$S/pr.md`: what was adopted; what was acknowledged, and why; the DRIFT
  row, if you used it; "Follow-ups" for new surface; and any change cargo-semver-checks would call
  major (a field added to a struct that is not `#[non_exhaustive]`), which bears on the owner's
  choice of version.
- Publish:

```bash
python3 scripts/heal_drift.py publish --issue N --title "<the main commit's subject>" \
  --body-file "$S/pr.md" --diff-sha256 "$(cat "$S/<id>/diff-sha256.txt")" --drift-row <row|none>
```

It prints the PR's URL, or refuses with the reason and pushes nothing.

## 10. Watch

Run `gh pr checks <pr> --watch --fail-fast` in the background. On a red check, read its log
(`gh run view --job <id> --log-failed`), fix forward with a new commit (never amend or force),
re-run step 7's gates and publish again; then end your turn. From then on the poller's messages
(`From heal-poller:`) bring you back:

- **redrift:** upstream moved again; redo step 2 onward and publish.
- **conflict:** `git fetch origin main && git merge origin/main` (never rebase), resolve,
  regenerate what the conflict touched (`gen_registry.py --write`, the `[[drift]]` entries),
  re-gate and publish.
- **ci:** the message carries the failing jobs' log tails; fix forward, re-gate and publish.

The third nudge of one kind escalates the issue to the owner.

## Resuming

A prompt that resumes a heal names its open PR: read it (`gh pr view <pr> --comments`), its
checks and the branch's log, then continue from step 10.
