---
paths:
  - "scripts/heal_drift.py"
  - ".claude/skills/heal-drift/**"
  - ".github/scripts/tests/test_heal_drift.py"
---

# The drift heal loop

`scripts/heal_drift.py` closes the loop `nightly-schema.yml` opens. A systemd user timer on the
owner's dev box runs `heal_drift.py run` hourly, which starts one loom session per open
`schema-drift` issue; the session follows `.claude/skills/heal-drift/SKILL.md` and ends in a PR
that closes the issue, which the owner merges. The design is
`docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md`; SELF-HEALING.md's "Repair"
section is the operator's guide.

- **The poller keeps no state.** Every fact it acts on is on GitHub or in loom: the issue's
  `diff_sha256`, the PR's `<!-- polyoxide-heal: … -->` marker, the nudge markers in PR comments,
  and the session's title (`heal <spec> <N> <row>`) and `issue` field. `decide()` is pure: a new
  behaviour is a rule there, with a test, never a state file.
- **Rule order is behaviour.** `decide()` tries the spec's eleven rules in order and the first
  match wins: a merged PR beats `needs-human`, and a stuck session beats a nudge. A new rule gets
  a row in the spec's table and a test that pins where it sits.
- **Only the poller's own sessions are touched.** A session is a heal session when its title,
  which the poller writes, names the issue its `issue` field links; any other session linked to
  the issue blocks a heal and is never messaged or killed.
- **`publish` is the only way out.** A heal session never runs `git push` or a `gh` write;
  `.claude/settings.json` allows `heal_drift.py publish` and `escalate` and nothing broader. Every
  refusal in `publish` comes before its push, and `GUARDED` holds the release machinery and the
  guard itself, so widening what a heal may touch is the owner's change, made outside a heal.
- **The fixtures are captured.** `.github/scripts/tests/fixtures/heal/` holds real `gh` and
  `loom session list --json` output. When either changes shape, re-capture them with the commands
  in `docs/superpowers/plans/2026-11-12-drift-heal-loop.md` (Task 1, step 1); never hand-write a
  new shape.
- **A heal never bumps a version** or edits `CHANGELOG.md`, so a merged heal PR releases nothing
  until the owner cuts a version (`release.md`).
