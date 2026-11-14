# Drift Heal Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The owner chose inline execution (superpowers:executing-plans) on 2026-11-12.

**Goal:** An hourly local poller that starts one loom heal session per open `schema-drift` issue and follows it to a PR that closes the issue, behind a `publish` guard no heal can get past.

**Architecture:** One script, `scripts/heal_drift.py`, with four subcommands: `run` (snapshot GitHub and loom, a pure `decide()`, then `execute`), `publish` and `escalate` (what a heal session calls), `install` (the systemd user timer). A tracked skill, `.claude/skills/heal-drift/SKILL.md`, is the session's runbook; `.claude/settings.json` allows exactly `publish` and `escalate`. The spec is `docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md` and binds where this plan is silent.

**Tech Stack:** Python ≥ 3.11 (stdlib only: `tomllib`, `dataclasses`, `subprocess`), pytest in the `.github/scripts` uv project, `gh`, loom's CLI, git, systemd user units.

**Commands.** Every test run goes through loom's queue: `loom job run -d "<what>" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`. Commit by path only (`git add <paths>`, `git commit -- <paths>`). Each commit message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

## File structure

| File | Responsibility |
|---|---|
| `scripts/heal_drift.py` (create) | The parsers (`Issue`, `Marker`, `Pr`, `Session`), the actions and `decide()`, `execute()` and `snapshot()`, `publish()` and its refusals, `install()`, `main()` |
| `.github/scripts/tests/test_heal_drift.py` (create) | Tests for each of the above, on captured fixtures and temporary git repositories |
| `.github/scripts/tests/fixtures/heal/{issue_96,pr_65,comment,sessions}.json` (create) | Captured `gh` and `loom session list --json` output |
| `.claude/skills/heal-drift/SKILL.md` (create) | The heal session's runbook |
| `.claude/settings.json` (create) | The two allow rules |
| `.claude/rules/heal.md` (create) | The loop's rules, loaded with its files |
| `CLAUDE.md` (modify: the rules table) | The `heal.md` row |
| `SELF-HEALING.md` (modify: the diagram, a "Repair" section, the taxonomy, operations, testing) | The operator's guide |
| `.claude/rules/specs.md` (modify: the "creates no branches" bullet) | Names the heal loop |

---

### Task 1: Fixtures and the parsers

**Files:**
- Create: `.github/scripts/tests/fixtures/heal/issue_96.json`, `pr_65.json`, `comment.json`, `sessions.json`
- Create: `scripts/heal_drift.py`
- Create: `.github/scripts/tests/test_heal_drift.py`

- [ ] **Step 1: Capture the fixtures**

```bash
F=.github/scripts/tests/fixtures/heal
mkdir -p "$F"
gh issue view 96 --json number,state,labels,body,updatedAt > "$F/issue_96.json"
gh pr view 65 --json number,state,body,headRefName,mergeable,statusCheckRollup,comments,author,isCrossRepository > "$F/pr_65.json"
gh pr view 29 --json comments -q '.comments[0]' > "$F/comment.json"
loom session list --json -w polyoxide > "$F/sessions.json"
python3 -c "import json, re; print(re.findall(r'\"diff_sha256\": \"([0-9a-f]{64})\"', json.load(open('$F/issue_96.json'))['body']))"
python3 -c "import json; r = json.load(open('$F/pr_65.json'))['statusCheckRollup']; print(r[1]['name'], sorted({c.get('conclusion') or c.get('state') for c in r}))"
```

Expected: the first line is a one-element list, `['7a09009f7ccff02c68ebab5b80141459b655586bb68f8bdf293fedc25b428464']` (the file is JSON, so the body's quotes are escaped and a grep of the raw file finds nothing) (if the nightly has refiled #96 with another fingerprint, use the printed one as `SHA_96` below), and the last line prints `Lint & Test` followed by a set holding none of `FAILURE`, `TIMED_OUT` or `ERROR` (the tests make check 1 fail themselves, so the rest must not).

- [ ] **Step 2: Write the failing parser tests**

Create `.github/scripts/tests/test_heal_drift.py`:

```python
"""scripts/heal_drift.py: the drift heal loop's poller, its publish guard and its timer.

docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md is the design. The fixtures under
fixtures/heal/ are captured `gh` and `loom session list --json` output, so a field renamed by
either fails here rather than in the timer.
"""

from __future__ import annotations

import copy
import datetime as dt
import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
FIXTURES = Path(__file__).parent / "fixtures" / "heal"


def _load():
    spec = importlib.util.spec_from_file_location("heal_drift", REPO / "scripts" / "heal_drift.py")
    module = importlib.util.module_from_spec(spec)
    # dataclasses resolve their annotations through sys.modules.
    sys.modules["heal_drift"] = module
    spec.loader.exec_module(module)
    return module


heal = _load()


def fixture(name):
    return json.loads((FIXTURES / name).read_text())


ISSUE_96 = fixture("issue_96.json")
PR_65 = fixture("pr_65.json")
COMMENT = fixture("comment.json")
SESSION = fixture("sessions.json")[0]
SHA_96 = "7a09009f7ccff02c68ebab5b80141459b655586bb68f8bdf293fedc25b428464"
OTHER_SHA = "0" * 64
NOW = dt.datetime(2026, 11, 12, 12, 0, tzinfo=dt.timezone.utc)
BRANCH = "aidanb/heal-kalshi-ws-96-r32"
TITLE_96 = "heal kalshi-ws 96 R32"
TABLE = "| R28 | the reconnect jitter |\n| R31 | perps' new route |\n"


def issue(number=96, *, state="OPEN", labels=(), body=None, updated=None):
    d = copy.deepcopy(ISSUE_96)
    d["number"], d["state"] = number, state
    d["labels"] += [{**d["labels"][0], "name": name} for name in labels]
    if body is not None:
        d["body"] = body
    if updated is not None:
        d["updatedAt"] = updated
    return heal.Issue.from_gh(d)


def comment(body):
    return {**COMMENT, "body": body}


def nudge(kind, n=1):
    return comment(f"<!-- polyoxide-heal-nudge: kind={kind} n={n} -->\nheal: nudged.")


def pr(number=200, *, issue=96, state="OPEN", sha=SHA_96, row="R32",
       published="2026-11-12T09:00:00Z", mergeable="MERGEABLE", failing=False, comments=(),
       branch=BRANCH):
    d = copy.deepcopy(PR_65)
    marker = heal.Marker(issue, "kalshi-ws", sha, row, heal.parse_time(published))
    d.update(number=number, state=state, headRefName=branch, mergeable=mergeable,
             body=f"Adopted.\n\nCloses #{issue}\n\n{marker.render()}\n", comments=list(comments))
    if failing:
        d["statusCheckRollup"][1]["conclusion"] = "FAILURE"
    return heal.Pr.from_gh(d)


def session(issue=96, *, title=TITLE_96, status="Ready", since="2026-11-12T11:30:00Z", id=1,
            branch=BRANCH, worktree="/nowhere"):
    return heal.Session.from_loom({**SESSION, "id": id, "title": title, "issue": issue,
                                   "status": status, "status_since": since, "branch": branch,
                                   "worktree": worktree})


# --- parsing -------------------------------------------------------------------------------


def test_the_fingerprint_is_the_one_the_nightly_wrote_into_96():
    assert issue().fingerprint == SHA_96


@pytest.mark.parametrize("body", [
    "no acknowledgement block",
    f'"diff_sha256": "{SHA_96}"\n"diff_sha256": "{OTHER_SHA}"',
], ids=["none", "two"])
def test_a_body_without_exactly_one_fingerprint_has_none(body):
    assert issue(body=body).fingerprint is None


def test_the_spec_is_the_one_spec_label():
    assert issue().spec == "kalshi-ws"
    assert issue(labels=("spec:kalshi",)).spec is None


def test_a_marker_survives_its_round_trip():
    marker = heal.Marker(96, "kalshi-ws", SHA_96, "R32", NOW)
    assert heal.Marker.parse(f"Adopted.\n\n{marker.render()}\n") == marker


def test_a_body_with_two_markers_has_none():
    text = heal.Marker(96, "kalshi-ws", SHA_96, "R32", NOW).render()
    assert heal.Marker.parse(f"{text}\n{text}") is None


def test_a_pr_without_a_marker_is_not_a_heal_pr():
    # #65 adopted drift by hand, before the loop.
    assert heal.Pr.from_gh(PR_65) is None


def test_a_failed_check_and_the_nudges_are_read_off_the_pr():
    found = pr(failing=True, comments=[nudge("ci"), nudge("ci", 2), comment("looks good")])
    assert found.failing == (("Lint & Test", PR_65["statusCheckRollup"][1]["detailsUrl"]),)
    assert found.nudges == {"ci": 2}
    assert not found.settled


def test_loom_and_github_timestamps_parse():
    assert heal.parse_time(SESSION["status_since"]).tzinfo is not None
    assert heal.parse_time("2026-11-10T13:59:57Z") == dt.datetime(
        2026, 11, 10, 13, 59, 57, tzinfo=dt.timezone.utc)


def test_a_heal_session_is_titled_by_the_poller_and_linked_to_its_issue():
    assert session().heal and session().row == "R32"
    assert not session(issue=94).heal
    assert not session(title="kalshi-ws adoption").heal
    assert session(title="heal kalshi-ws 96 none").row == "none"


def test_the_highest_drift_row_reads_the_real_table():
    assert heal.highest_row((REPO / heal.DIVERGENCES).read_text()) >= 31
    assert heal.highest_row(TABLE) == 31
```

- [ ] **Step 3: Run them to see them fail**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: collection error, `FileNotFoundError` for `scripts/heal_drift.py`.

- [ ] **Step 4: Write the parsers**

Create `scripts/heal_drift.py`:

```python
#!/usr/bin/env python3
"""The drift heal loop: one loom session per open schema-drift issue, ending in a PR.

`run` is the poller a systemd user timer starts hourly. It reads the `schema-drift` issues, the
heal PRs and the loom sessions, and `decide` turns that snapshot into actions: start, message or
kill a heal session, escalate an issue to the owner, close a heal PR the nightly made stale.
`publish` and `escalate` are what a heal session calls; `publish` refuses, in code, everything a
heal must never push. `install` writes and enables the timer.

The design is docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md, the operator's guide
SELF-HEALING.md's "Repair" section, and the rules .claude/rules/heal.md.
"""

from __future__ import annotations

import argparse
import dataclasses
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
WORKSPACE = "polyoxide"
DRIFT_LABEL = "schema-drift"
HUMAN_LABEL = "needs-human"
SENDER = "heal-poller"
# Two heal sessions at once (spec §3.1): about 40 issues in nine weeks, often two or three a night.
CAP = 2
STUCK = dt.timedelta(hours=2)
WINDOW = dt.timedelta(days=14)
# The third nudge of one kind on one PR escalates instead of messaging.
NUDGE_LIMIT = 3
LOG_TAIL = 200
DIVERGENCES = "_bmad-output/specs/spec-venue-extensibility/divergences.md"
# What a heal may never change (spec §4.1 step 4): the release machinery and the loop's own
# guard. An entry ending in `/` guards everything under it.
GUARDED = (
    ".github/workflows/release.yml",
    "scripts/finish_release.sh",
    "scripts/publish_order.py",
    "scripts/heal_drift.py",
    ".claude/settings.json",
    ".claude/skills/heal-drift/",
)
# A check run's `conclusion`, or a commit status's `state`, that counts as failed.
FAILED = frozenset({"FAILURE", "TIMED_OUT", "ERROR"})
ISSUE_FIELDS = "number,state,labels,body,updatedAt"
PR_FIELDS = "number,state,body,headRefName,mergeable,statusCheckRollup,comments"

FINGERPRINT = re.compile(r'"diff_sha256":\s*"([0-9a-f]{64})"')
MARKER = re.compile(
    r"<!-- polyoxide-heal: issue=(?P<issue>\d+) spec=(?P<spec>\S+) "
    r"diff_sha256=(?P<sha>[0-9a-f]{64}) drift_row=(?P<row>R\d+|none) "
    r"published=(?P<published>\S+) -->")
NUDGE = re.compile(r"<!-- polyoxide-heal-nudge: kind=(?P<kind>[a-z]+) n=(?P<n>\d+) -->")
TITLE = re.compile(r"^heal (?P<spec>\S+) (?P<issue>\d+) (?P<row>R\d+|none)$")
DRIFT_ROW = re.compile(r"^\| R(\d+) \|", re.MULTILINE)
SHA = re.compile(r"^[0-9a-f]{64}$")
ROW = re.compile(r"^(?:R\d+|none)$")
JOB_URL = re.compile(r"/actions/runs/\d+/job/(\d+)")


def parse_time(text: str) -> dt.datetime:
    """A GitHub (`2026-11-10T13:59:57Z`) or loom (`2026-11-12T18:56:34.130644589+02:00`)
    timestamp as an aware datetime; fromisoformat takes at most six fractional digits."""
    text = re.sub(r"(\.\d{6})\d+", r"\1", text)
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    return dt.datetime.fromisoformat(text)


def highest_row(divergences: str) -> int:
    """The highest DRIFT row number in divergences.md's table, or 0."""
    return max((int(n) for n in DRIFT_ROW.findall(divergences)), default=0)


@dataclasses.dataclass(frozen=True)
class Issue:
    number: int
    open: bool
    labels: frozenset[str]
    body: str
    updated: dt.datetime

    @classmethod
    def from_gh(cls, d: Mapping) -> Issue:
        return cls(number=d["number"], open=d["state"] == "OPEN",
                   labels=frozenset(label["name"] for label in d["labels"]),
                   body=d["body"] or "", updated=parse_time(d["updatedAt"]))

    @property
    def spec(self) -> str | None:
        """The id of the issue's one `spec:<id>` label."""
        specs = [name.removeprefix("spec:") for name in self.labels if name.startswith("spec:")]
        return specs[0] if len(specs) == 1 else None

    @property
    def fingerprint(self) -> str | None:
        """The `diff_sha256` the nightly writes into the body's acknowledgement block, or None
        unless there is exactly one."""
        found = FINGERPRINT.findall(self.body)
        return found[0] if len(found) == 1 else None


@dataclasses.dataclass(frozen=True)
class Marker:
    """The line `publish` ends a heal PR's body with (spec §3.2)."""
    issue: int
    spec: str
    sha: str
    row: str
    published: dt.datetime

    def render(self) -> str:
        stamp = self.published.astimezone(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        return (f"<!-- polyoxide-heal: issue={self.issue} spec={self.spec} "
                f"diff_sha256={self.sha} drift_row={self.row} published={stamp} -->")

    @classmethod
    def parse(cls, body: str) -> Marker | None:
        found = list(MARKER.finditer(body))
        if len(found) != 1:
            return None
        m = found[0]
        return cls(issue=int(m["issue"]), spec=m["spec"], sha=m["sha"], row=m["row"],
                   published=parse_time(m["published"]))


@dataclasses.dataclass(frozen=True)
class Pr:
    """A heal PR: one whose body carries a marker."""
    number: int
    state: str  # OPEN, CLOSED or MERGED
    branch: str
    mergeable: str  # MERGEABLE, CONFLICTING or UNKNOWN
    marker: Marker
    failing: tuple[tuple[str, str], ...]  # (check name, details URL) of each failed check
    nudges: Mapping[str, int]  # nudge-marker comments, by kind

    @classmethod
    def from_gh(cls, d: Mapping) -> Pr | None:
        marker = Marker.parse(d.get("body") or "")
        if marker is None:
            return None
        failing = tuple(
            (check.get("name") or check.get("context") or "?",
             check.get("detailsUrl") or check.get("targetUrl") or "")
            for check in d.get("statusCheckRollup") or []
            if (check.get("conclusion") or check.get("state") or "") in FAILED)
        nudges: dict[str, int] = {}
        for c in d.get("comments") or []:
            for m in NUDGE.finditer(c.get("body") or ""):
                nudges[m["kind"]] = nudges.get(m["kind"], 0) + 1
        return cls(number=d["number"], state=d["state"], branch=d["headRefName"],
                   mergeable=d.get("mergeable") or "UNKNOWN", marker=marker, failing=failing,
                   nudges=nudges)

    @property
    def settled(self) -> bool:
        """Closed by the poller (rule 4) or already escalated as a rejection (rule 3)."""
        return bool(self.nudges.get("closed") or self.nudges.get("rejected"))


@dataclasses.dataclass(frozen=True)
class Session:
    """A row of `loom session list --json`."""
    id: int
    title: str
    status: str  # loom's: Running, Ready, Loading, Paused, Prompting, Deleting, Recoverable
    since: dt.datetime
    branch: str
    worktree: str
    issue: int

    @classmethod
    def from_loom(cls, d: Mapping) -> Session:
        return cls(id=d["id"], title=d["title"], status=d["status"],
                   since=parse_time(d["status_since"]), branch=d["branch"],
                   worktree=d["worktree"], issue=d["issue"])

    @property
    def heal(self) -> bool:
        """A session the poller started: its title names the issue it is linked to."""
        m = TITLE.match(self.title)
        return m is not None and int(m["issue"]) == self.issue

    @property
    def row(self) -> str | None:
        """The DRIFT row the poller reserved for a heal session, from its title."""
        m = TITLE.match(self.title)
        return m["row"] if m is not None and self.heal else None
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: `11 passed` (the parametrized fingerprint test counts twice).

- [ ] **Step 6: Commit**

```bash
git add scripts/heal_drift.py .github/scripts/tests/test_heal_drift.py .github/scripts/tests/fixtures/heal
git commit -m "feat(heal): the heal loop's parsers for drift issues, heal PRs and loom sessions" -- scripts/heal_drift.py .github/scripts/tests/test_heal_drift.py .github/scripts/tests/fixtures/heal
```

---

### Task 2: The actions and `decide()`

**Files:**
- Modify: `scripts/heal_drift.py` (append)
- Modify: `.github/scripts/tests/test_heal_drift.py` (append)

- [ ] **Step 1: Write the failing lifecycle tests**

Append to `.github/scripts/tests/test_heal_drift.py`:

```python
# --- decide: spec §3.1, one test per rule and per ordering --------------------------------


def decide(issues, prs=(), sessions=(), table=TABLE):
    return heal.decide(issues, prs, sessions, table, NOW)


def test_rule_11_starts_a_heal_session_with_the_next_drift_row():
    assert decide([issue()]) == [heal.Start(96, "kalshi-ws", SHA_96, "R32")]


def test_rule_11_gives_two_issues_consecutive_rows_in_issue_order():
    assert decide([issue(96), issue(94)]) == [
        heal.Start(94, "kalshi-ws", SHA_96, "R32"), heal.Start(96, "kalshi-ws", SHA_96, "R33")]


def test_a_reservation_skips_rows_a_heal_session_or_an_open_pr_holds():
    actions = decide([issue(94), issue(96)],
                     prs=[pr(201, issue=90, row="R34")],
                     sessions=[session(94, title="heal kalshi-ws 94 R33", status="Running")])
    assert actions == [heal.Start(96, "kalshi-ws", SHA_96, "R35")]


def test_rule_11_waits_when_both_slots_are_taken():
    busy = [session(90, title="heal kalshi-ws 90 R32", status="Running", id=2),
            session(92, title="heal kalshi-ws 92 R33", status="Running", id=3)]
    assert decide([issue(90), issue(92), issue(96)], sessions=busy) == [
        heal.Wait(96, "all 2 heal slots are taken")]


def test_rule_11_leaves_an_issue_a_hand_started_session_holds():
    assert decide([issue()], sessions=[session(title="kalshi-ws adoption")]) == [
        heal.Wait(96, "a session the poller did not start is linked to it")]


def test_rule_11_waits_without_a_fingerprint():
    assert decide([issue(body="no block")]) == [
        heal.Wait(96, "its body holds no single diff_sha256")]


def test_rule_1_kills_the_session_once_its_pr_merged_even_under_needs_human():
    s = session()
    assert decide([issue(state="CLOSED", labels=("needs-human",))],
                  prs=[pr(state="MERGED")], sessions=[s]) == [heal.Kill(s)]


def test_rule_2_leaves_a_needs_human_issue_alone():
    assert decide([issue(labels=("needs-human",))], prs=[pr(mergeable="CONFLICTING")],
                  sessions=[session()]) == []


def test_rule_3_escalates_a_pr_closed_unmerged_and_kills_its_session():
    s = session()
    assert decide([issue()], prs=[pr(state="CLOSED")], sessions=[s]) == [
        heal.Escalate(96, "The heal PR #200 was closed unmerged; the poller will not heal this "
                          "issue again until `needs-human` is removed.", reject_pr=200),
        heal.Kill(s)]


def test_a_rejected_pr_does_not_block_a_fresh_heal_once_the_label_is_removed():
    assert decide([issue()], prs=[pr(state="CLOSED", comments=[nudge("rejected")])]) == [
        heal.Start(96, "kalshi-ws", SHA_96, "R32")]


def test_rule_4_closes_the_pr_when_the_nightly_closed_the_issue():
    s = session()
    assert decide([issue(state="CLOSED")], prs=[pr()], sessions=[s]) == [
        heal.ClosePr(200, 96), heal.Kill(s)]


def test_a_pr_the_poller_closed_never_reads_as_a_rejection():
    # The owner reopened the issue after rule 4 closed its PR: heal it afresh.
    assert decide([issue()], prs=[pr(state="CLOSED", comments=[nudge("closed")])]) == [
        heal.Start(96, "kalshi-ws", SHA_96, "R32")]


def test_rule_5_kills_a_session_left_on_a_closed_issue():
    s = session(status="Running")
    assert decide([issue(state="CLOSED")], sessions=[s]) == [heal.Kill(s)]


def test_rule_6_resumes_an_open_pr_that_has_no_session():
    assert decide([issue()], prs=[pr(sha=OTHER_SHA, row="R30")]) == [
        heal.Start(96, "kalshi-ws", OTHER_SHA, "R30", branch=BRANCH, pr=200)]


def test_rule_7_escalates_a_session_stuck_at_a_prompt_before_any_nudge():
    s = session(status="Prompting", since="2026-11-12T09:00:00Z")
    [action] = decide([issue()], prs=[pr(failing=True)], sessions=[s])
    assert isinstance(action, heal.Escalate) and "permission prompt" in action.reason


def test_rule_7_waits_two_hours_before_calling_a_prompt_stuck():
    assert decide([issue()], sessions=[session(status="Prompting",
                                               since="2026-11-12T10:30:00Z")]) == []


def test_rule_7_escalates_a_session_idle_with_nothing_published():
    [action] = decide([issue()], sessions=[session(since="2026-11-12T09:00:00Z")])
    assert isinstance(action, heal.Escalate) and "without publishing" in action.reason


def test_an_idle_session_with_a_green_pr_is_left_alone():
    assert decide([issue()], prs=[pr()], sessions=[session(since="2026-11-12T09:00:00Z")]) == []


def test_rule_8_sends_a_redrift_when_the_nightly_saw_another_drift_after_the_publish():
    s = session()
    [action] = decide([issue(updated="2026-11-12T06:10:00Z")],
                      prs=[pr(sha=OTHER_SHA, published="2026-11-11T09:00:00Z")], sessions=[s])
    assert (action.kind, action.n, action.session, action.pr) == ("redrift", 1, s, 200)
    assert SHA_96 in action.text


def test_rule_8_holds_when_the_pr_adopted_a_newer_upstream_than_the_issue_saw():
    assert decide([issue(updated="2026-11-12T06:10:00Z")],
                  prs=[pr(sha=OTHER_SHA, published="2026-11-12T09:00:00Z")],
                  sessions=[session()]) == []


def test_rule_9_sends_a_conflict():
    [action] = decide([issue()], prs=[pr(mergeable="CONFLICTING")], sessions=[session()])
    assert action.kind == "conflict"


def test_rule_10_sends_the_failing_jobs():
    failing = pr(failing=True)
    [action] = decide([issue()], prs=[failing], sessions=[session()])
    assert (action.kind, action.jobs) == ("ci", failing.failing)


@pytest.mark.parametrize("status", ["Running", "Loading", "Paused"])
def test_a_busy_or_paused_session_is_not_nudged(status):
    assert decide([issue()], prs=[pr(mergeable="CONFLICTING")],
                  sessions=[session(status=status)]) == []


def test_a_second_nudge_is_numbered():
    [action] = decide([issue()], prs=[pr(failing=True, comments=[nudge("ci")])],
                      sessions=[session()])
    assert (action.kind, action.n) == ("ci", 2)


def test_the_third_nudge_of_a_kind_escalates():
    [action] = decide([issue()], prs=[pr(failing=True, comments=[nudge("ci"), nudge("ci", 2)])],
                      sessions=[session()])
    assert isinstance(action, heal.Escalate) and "nudged 2 times for `ci`" in action.reason
```

- [ ] **Step 2: Run them to see them fail**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: the new tests fail with `AttributeError: module 'heal_drift' has no attribute 'decide'` (or `'Start'`); Task 1's 11 pass.

- [ ] **Step 3: Write the actions and `decide()`**

Append to `scripts/heal_drift.py`:

```python
# --- actions and decide (spec §3.1) --------------------------------------------------------


@dataclasses.dataclass(frozen=True)
class Start:
    issue: int
    spec: str
    sha: str
    row: str
    branch: str | None = None  # resume on this branch (rule 6), else a new one (rule 11)
    pr: int | None = None

    @property
    def title(self) -> str:
        return f"heal {self.spec} {self.issue} {self.row}"


@dataclasses.dataclass(frozen=True)
class Send:
    session: Session
    pr: int
    kind: str  # redrift, conflict or ci
    n: int
    text: str
    jobs: tuple[tuple[str, str], ...] = ()  # (name, details URL) whose log tails go with a ci nudge


@dataclasses.dataclass(frozen=True)
class Escalate:
    issue: int
    reason: str
    reject_pr: int | None = None  # leave the kind=rejected marker on this PR (rule 3)


@dataclasses.dataclass(frozen=True)
class Kill:
    session: Session


@dataclasses.dataclass(frozen=True)
class ClosePr:
    pr: int
    issue: int


@dataclasses.dataclass(frozen=True)
class Wait:
    issue: int
    reason: str


Action = Start | Send | Escalate | Kill | ClosePr | Wait
HAND_STARTED = "a session the poller did not start is linked to it"
NO_SLOT = f"all {CAP} heal slots are taken"


def decide(issues: Sequence[Issue], prs: Sequence[Pr], sessions: Sequence[Session],
           divergences: str, now: dt.datetime) -> list[Action]:
    """One poll's actions (spec §3.1): for each issue, in number order, the first rule that
    matches. Pure, so `run --dry-run` and the tests see exactly what `run` would do."""
    heal: dict[int, Session] = {}
    for s in sessions:
        if s.heal:
            heal.setdefault(s.issue, s)
    linked = {s.issue for s in sessions if s.issue}
    by_issue: dict[int, list[Pr]] = {}
    for p in sorted(prs, key=lambda p: p.number):
        by_issue.setdefault(p.marker.issue, []).append(p)
    held = [highest_row(divergences)]
    held += [int(s.row[1:]) for s in heal.values() if s.row and s.row != "none"]
    held += [int(p.marker.row[1:]) for p in prs if p.state == "OPEN" and p.marker.row != "none"]
    next_row = max(held) + 1
    slots = CAP - len(heal)
    actions: list[Action] = []
    for issue in sorted(issues, key=lambda i: i.number):
        session = heal.get(issue.number)
        kill: list[Action] = [Kill(session)] if session else []
        its = by_issue.get(issue.number, [])
        open_pr = next((p for p in reversed(its) if p.state == "OPEN"), None)
        closed = [p for p in its if p.state == "CLOSED" and not p.settled]
        if open_pr is None and any(p.state == "MERGED" for p in its):  # rule 1
            actions += kill
        elif HUMAN_LABEL in issue.labels:  # rule 2
            pass
        elif issue.open and open_pr is None and closed:  # rule 3
            number = closed[-1].number
            actions.append(Escalate(
                issue.number, f"The heal PR #{number} was closed unmerged; the poller will not "
                f"heal this issue again until `{HUMAN_LABEL}` is removed.", reject_pr=number))
            actions += kill
        elif not issue.open and open_pr:  # rule 4
            actions.append(ClosePr(open_pr.number, issue.number))
            actions += kill
        elif not issue.open:  # rule 5
            actions += kill
        elif open_pr and session is None:  # rule 6
            if issue.number in linked:
                actions.append(Wait(issue.number, HAND_STARTED))
            elif slots <= 0:
                actions.append(Wait(issue.number, NO_SLOT))
            else:
                slots -= 1
                m = open_pr.marker
                actions.append(Start(issue.number, m.spec, m.sha, m.row, branch=open_pr.branch,
                                     pr=open_pr.number))
        elif session:
            if stuck := stuck_reason(session, open_pr, now):  # rule 7
                actions.append(Escalate(issue.number, stuck))
            elif open_pr and session.status == "Ready":  # rules 8 to 10
                if nudge := nudge_for(issue, open_pr, session):
                    actions.append(nudge)
        elif issue.number in linked:  # rule 11, from here on
            actions.append(Wait(issue.number, HAND_STARTED))
        elif issue.fingerprint is None:
            actions.append(Wait(issue.number, "its body holds no single diff_sha256"))
        elif issue.spec is None:
            actions.append(Wait(issue.number, "it carries no single spec:<id> label"))
        elif slots <= 0:
            actions.append(Wait(issue.number, NO_SLOT))
        else:
            slots -= 1
            actions.append(Start(issue.number, issue.spec, issue.fingerprint, f"R{next_row}"))
            next_row += 1
    return actions


def stuck_reason(session: Session, open_pr: Pr | None, now: dt.datetime) -> str | None:
    """Rule 7: a heal session at a permission prompt, or idle with nothing published, for over
    STUCK."""
    if now - session.since <= STUCK:
        return None
    since = session.since.astimezone(dt.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    if session.status == "Prompting":
        return (f"The heal session `{session.title}` has waited at a permission prompt "
                f"since {since}.")
    if session.status == "Ready" and open_pr is None:
        return (f"The heal session `{session.title}` has been idle since {since} without "
                "publishing a PR or escalating.")
    return None


def nudge_for(issue: Issue, pr: Pr, session: Session) -> Action | None:
    """Rules 8 to 10 for an idle heal session with an open PR: the first that applies, as a
    message, or as an escalation once its kind has been sent NUDGE_LIMIT - 1 times."""
    sha = issue.fingerprint
    if sha and sha != pr.marker.sha and issue.updated > pr.marker.published:
        kind = "redrift"
        text = (f"Upstream moved: issue #{issue.number}'s fingerprint is now diff_sha256={sha} "
                f"(the nightly of {issue.updated:%Y-%m-%d}), and PR #{pr.number} adopted "
                f"{pr.marker.sha}. Re-adopt from step 2 of heal-drift and publish again.")
    elif pr.mergeable == "CONFLICTING":
        kind = "conflict"
        text = (f"PR #{pr.number} conflicts with main. Merge origin/main (never rebase), "
                "resolve, re-run step 7's gates and publish again.")
    elif pr.failing:
        kind = "ci"
        names = ", ".join(name for name, _ in pr.failing)
        text = (f"PR #{pr.number} has failing checks: {names}. Fix forward, re-run step 7's "
                "gates and publish again. The failed steps' log tails follow.")
    else:
        return None
    n = pr.nudges.get(kind, 0) + 1
    if n >= NUDGE_LIMIT:
        return Escalate(issue.number, f"The heal session `{session.title}` was nudged {n - 1} "
                        f"times for `{kind}` on PR #{pr.number} and has not resolved it.")
    return Send(session, pr.number, kind, n,
                f"{text} (nudge {n} of {NUDGE_LIMIT - 1}; the next escalates to the owner)",
                jobs=pr.failing if kind == "ci" else ())
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: `38 passed`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(heal): decide, the poller's eleven lifecycle rules as a pure function" -- scripts/heal_drift.py .github/scripts/tests/test_heal_drift.py
```

---

### Task 3: `execute`, `snapshot` and `run`

**Files:**
- Modify: `scripts/heal_drift.py` (append)
- Modify: `.github/scripts/tests/test_heal_drift.py` (append)

- [ ] **Step 1: Write the failing tests**

Append to `.github/scripts/tests/test_heal_drift.py`:

```python
# --- execute, snapshot ---------------------------------------------------------------------


class FakeShell(heal.Shell):
    """Answers gh, loom and systemctl from a table and records every call. git runs for real
    (in the temporary repositories below) unless `real_git` is off, when the table answers it
    too. An unanswered command prints nothing."""

    def __init__(self, answers=None, *, real_git=True):
        self.answers = dict(answers or {})
        self.real_git = real_git
        self.calls = []

    def run(self, args, *, cwd=None, input=None):
        args = [str(a) for a in args]
        self.calls.append((args, input))
        if args[0] == "git" and self.real_git:
            return super().run(args, cwd=cwd, input=input)
        for prefix, answer in self.answers.items():
            if tuple(args[: len(prefix)]) == prefix:
                return answer(args) if callable(answer) else answer
        return ""

    def commands(self, *prefix):
        return [(args, input) for args, input in self.calls if tuple(args[: len(prefix)]) == prefix]


def test_a_start_opens_a_heal_session_on_opus_at_high_effort():
    shell = FakeShell()
    heal.execute(heal.Start(96, "kalshi-ws", SHA_96, "R32"), shell)
    [(args, _)] = shell.calls
    assert args[:14] == ["loom", "session", "new", TITLE_96, "-w", "polyoxide", "--issue", "96",
                         "--model", "opus", "--effort", "high", "--permission-mode", "auto"]
    prompt = args[args.index("--prompt") + 1]
    assert "heal-drift" in prompt and SHA_96 in prompt and "--drift-row R32" in prompt
    assert "--branch" not in args


def test_a_resume_checks_out_the_pr_branch():
    shell = FakeShell()
    heal.execute(heal.Start(96, "kalshi-ws", SHA_96, "R32", branch=BRANCH, pr=200), shell)
    [(args, _)] = shell.calls
    assert args[-2:] == ["--branch", BRANCH]
    assert "PR #200" in args[args.index("--prompt") + 1]


def test_a_ci_nudge_carries_each_failing_job_s_log_tail_and_leaves_its_count():
    log = "\n".join(f"line {i}" for i in range(300))
    shell = FakeShell({("gh", "run", "view"): log})
    failing = pr(failing=True)
    heal.execute(heal.Send(session(), 200, "ci", 1, "PR #200 has failing checks.",
                           jobs=failing.failing), shell)
    job = PR_65["statusCheckRollup"][1]["detailsUrl"].rsplit("/", 1)[1]
    assert [args for args, _ in shell.commands("gh", "run", "view")] == [
        ["gh", "run", "view", "--job", job, "--log-failed"]]
    [(send, text)] = shell.commands("loom", "session", "send")
    assert send == ["loom", "session", "send", TITLE_96, "--id", "1", "-w", "polyoxide",
                    "--from", "heal-poller"]
    lines = text.splitlines()
    assert "line 299" in lines and "line 100" in lines and "line 99" not in lines
    [(note, _)] = shell.commands("gh", "pr", "comment")
    assert pr(comments=[comment(note[note.index("--body") + 1])]).nudges == {"ci": 1}


def test_an_escalation_labels_the_issue_and_settles_a_rejected_pr():
    shell = FakeShell()
    heal.execute(heal.Escalate(96, "The heal PR #200 was closed unmerged.", reject_pr=200), shell)
    assert [args[:3] for args, _ in shell.calls] == [
        ["gh", "label", "create"], ["gh", "issue", "comment"], ["gh", "issue", "edit"],
        ["gh", "pr", "comment"]]
    [(_, text)] = shell.commands("gh", "issue", "comment")
    assert "closed unmerged" in text
    [(edit, _)] = shell.commands("gh", "issue", "edit")
    assert edit[-2:] == ["--add-label", "needs-human"]
    [(note, _)] = shell.commands("gh", "pr", "comment")
    assert pr(state="CLOSED", comments=[comment(note[note.index("--body") + 1])]).settled


def test_closing_a_stale_pr_settles_it():
    shell = FakeShell()
    heal.execute(heal.ClosePr(200, 96), shell)
    [(args, _)] = shell.calls
    assert args[:4] == ["gh", "pr", "close", "200"]
    assert pr(state="CLOSED", comments=[comment(args[args.index("--comment") + 1])]).settled


def test_a_kill_names_the_session_by_its_id():
    shell = FakeShell()
    heal.execute(heal.Kill(session(id=7)), shell)
    assert shell.calls == [(["loom", "session", "kill", TITLE_96, "--id", "7", "-y", "-w",
                             "polyoxide"], None)]


def test_a_snapshot_reads_both_issue_lists_the_heal_prs_the_sessions_and_the_table():
    named = {**SESSION, "title": "heal kalshi 80 R29", "issue": 80}
    old = {**ISSUE_96, "number": 80, "state": "CLOSED"}
    marker = heal.Marker(96, "kalshi-ws", SHA_96, "R32", NOW).render()
    healing = {**PR_65, "number": 200, "state": "OPEN", "body": f"Adopted.\n\n{marker}\n"}

    def issues(args):
        return json.dumps([ISSUE_96] if args[args.index("--state") + 1] == "open" else [])

    shell = FakeShell({
        ("loom", "session", "list"): json.dumps([SESSION, named]),
        ("gh", "issue", "list"): issues,
        ("gh", "issue", "view"): json.dumps(old),
        ("gh", "pr", "list"): json.dumps([PR_65, healing]),
        ("git", "show"): TABLE,
    }, real_git=False)
    found, prs, sessions, table = heal.snapshot(shell, NOW)
    assert [i.number for i in found] == [96, 80]
    assert [p.number for p in prs] == [200]
    assert [s.title for s in sessions] == [SESSION["title"], "heal kalshi 80 R29"]
    assert table == TABLE
    [closed] = [args for args, _ in shell.commands("gh", "issue", "list") if "closed" in args]
    assert closed[closed.index("--search") + 1] == "updated:>=2026-10-29"
    assert [args[3] for args, _ in shell.commands("gh", "issue", "view")] == ["80"]
```

- [ ] **Step 2: Run them to see them fail**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: collection error, `AttributeError: module 'heal_drift' has no attribute 'Shell'`.

- [ ] **Step 3: Write `Shell`, the prompts, `execute`, `snapshot`, `run` and `main`**

Append to `scripts/heal_drift.py`:

```python
# --- execute, snapshot, run ----------------------------------------------------------------

START_PROMPT = """\
Invoke the heal-drift skill for schema-drift issue #{issue} (spec `{spec}`).

- The issue's fingerprint at the last nightly is diff_sha256={sha}.
- Your reserved DRIFT row is {row}: use it only if a test-context item moves, and pass \
`--drift-row {row}` to publish if you do, `--drift-row none` if not.
- Publish only with `python3 scripts/heal_drift.py publish`, and escalate only with \
`python3 scripts/heal_drift.py escalate`. Never run `git push` or a `gh` write yourself.
"""

RESUME_PROMPT = """\
Invoke the heal-drift skill to resume schema-drift issue #{issue} (spec `{spec}`). Its heal PR \
#{pr} is open on this branch, and its marker records diff_sha256={sha} and DRIFT row {row}. \
Read the PR, its comments and its checks, then continue from the skill's step 10.

- Publish only with `python3 scripts/heal_drift.py publish`, and escalate only with \
`python3 scripts/heal_drift.py escalate`. Never run `git push` or a `gh` write yourself.
"""


class Shell:
    """Runs a command and returns its stdout; a non-zero exit raises CalledProcessError, with
    stderr."""

    def run(self, args: Sequence[str], *, cwd: Path | None = None,
            input: str | None = None) -> str:
        done = subprocess.run(list(args), cwd=cwd, input=input, capture_output=True, text=True)
        if done.returncode != 0:
            raise subprocess.CalledProcessError(done.returncode, list(args), done.stdout,
                                                done.stderr)
        return done.stdout


def prompt(start: Start) -> str:
    template = RESUME_PROMPT if start.pr else START_PROMPT
    return template.format(issue=start.issue, spec=start.spec, sha=start.sha, row=start.row,
                           pr=start.pr)


def nudge_marker(kind: str, n: int) -> str:
    return f"<!-- polyoxide-heal-nudge: kind={kind} n={n} -->"


def log_tail(shell: Shell, url: str) -> str:
    """The last LOG_TAIL lines of a failed job's failed steps, or why there are none."""
    m = JOB_URL.search(url)
    if m is None:
        return f"(no job log: {url or 'the check has no details URL'})"
    try:
        log = shell.run(["gh", "run", "view", "--job", m[1], "--log-failed"])
    except subprocess.CalledProcessError as err:
        return f"(gh run view failed: {(err.stderr or '').strip()[:500]})"
    return "\n".join(log.splitlines()[-LOG_TAIL:])


def escalate_issue(shell: Shell, issue: int, reason: str) -> None:
    """Comment on the issue and label it needs-human, creating the label if need be."""
    shell.run(["gh", "label", "create", HUMAN_LABEL, "--color", "D93F0B", "--description",
               "The drift heal loop stopped; the owner decides", "--force"])
    shell.run(["gh", "issue", "comment", str(issue), "--body-file", "-"], input=f"heal: {reason}\n")
    shell.run(["gh", "issue", "edit", str(issue), "--add-label", HUMAN_LABEL])


def execute(action: Action, shell: Shell, *, workspace: str = WORKSPACE) -> None:
    """Take one action. Each touches only its issue, its heal PR or its heal session."""
    match action:
        case Start():
            args = ["loom", "session", "new", action.title, "-w", workspace,
                    "--issue", str(action.issue), "--model", "opus", "--effort", "high",
                    "--permission-mode", "auto", "--prompt", prompt(action)]
            if action.branch:
                args += ["--branch", action.branch]
            shell.run(args)
        case Send():
            text = action.text + "".join(
                f"\n\n### {name}\n\n```\n{log_tail(shell, url)}\n```" for name, url in action.jobs)
            shell.run(["loom", "session", "send", action.session.title, "--id",
                       str(action.session.id), "-w", workspace, "--from", SENDER], input=text)
            shell.run(["gh", "pr", "comment", str(action.pr), "--body",
                       f"{nudge_marker(action.kind, action.n)}\nheal: sent the session a "
                       f"`{action.kind}` nudge ({action.n} of {NUDGE_LIMIT - 1}; the next "
                       "escalates)."])
        case Escalate():
            escalate_issue(shell, action.issue, action.reason)
            if action.reject_pr is not None:
                shell.run(["gh", "pr", "comment", str(action.reject_pr), "--body",
                           f"{nudge_marker('rejected', 1)}\nheal: closed unmerged, so "
                           f"#{action.issue} is labelled `{HUMAN_LABEL}`."])
        case Kill():
            shell.run(["loom", "session", "kill", action.session.title, "--id",
                       str(action.session.id), "-y", "-w", workspace])
        case ClosePr():
            shell.run(["gh", "pr", "close", str(action.pr), "--comment",
                       f"{nudge_marker('closed', 1)}\nheal: the nightly closed #{action.issue}, "
                       "so the drift is gone and this heal is no longer needed."])
        case Wait():
            pass


def describe(action: Action) -> str:
    match action:
        case Start(branch=None):
            return f"#{action.issue}: start `{action.title}`"
        case Start():
            return (f"#{action.issue}: start `{action.title}` on {action.branch} "
                    f"for PR #{action.pr}")
        case Send():
            return (f"#{action.session.issue}: send `{action.session.title}` a {action.kind} "
                    f"nudge ({action.n}) for PR #{action.pr}")
        case Escalate():
            return f"#{action.issue}: escalate: {action.reason}"
        case Kill():
            return f"#{action.session.issue}: kill `{action.session.title}`"
        case ClosePr():
            return f"#{action.issue}: close PR #{action.pr}"
        case Wait():
            return f"#{action.issue}: wait: {action.reason}"
    raise AssertionError(action)


def sessions_of(shell: Shell, workspace: str) -> list[Session]:
    return [Session.from_loom(d) for d in
            json.loads(shell.run(["loom", "session", "list", "--json", "-w", workspace]))]


def snapshot(shell: Shell, now: dt.datetime, *, workspace: str = WORKSPACE,
             repo: Path = REPO) -> tuple[list[Issue], list[Pr], list[Session], str]:
    """What one poll reads (spec §3): the open drift issues, those closed in the last WINDOW and
    any a heal session names; the heal PRs updated in the last WINDOW; the sessions; and
    origin/main's divergences.md, for the DRIFT rows."""
    sessions = sessions_of(shell, workspace)
    since = (now - WINDOW).date().isoformat()
    base = ["gh", "issue", "list", "--label", DRIFT_LABEL, "--limit", "100", "--json", ISSUE_FIELDS]
    raw = json.loads(shell.run(base + ["--state", "open"]))
    raw += json.loads(shell.run(base + ["--state", "closed", "--search", f"updated:>={since}"]))
    seen = {d["number"] for d in raw}
    for s in sessions:
        if s.heal and s.issue not in seen:
            raw.append(json.loads(shell.run(["gh", "issue", "view", str(s.issue), "--json",
                                             ISSUE_FIELDS])))
            seen.add(s.issue)
    issues = [Issue.from_gh(d) for d in raw
              if DRIFT_LABEL in {label["name"] for label in d["labels"]}]
    listed = json.loads(shell.run(["gh", "pr", "list", "--state", "all", "--search",
                                   f"updated:>={since}", "--limit", "100", "--json", PR_FIELDS]))
    prs = [p for d in listed if (p := Pr.from_gh(d)) is not None]
    shell.run(["git", "fetch", "--quiet", "origin", "main"], cwd=repo)
    divergences = shell.run(["git", "show", f"origin/main:{DIVERGENCES}"], cwd=repo)
    return issues, prs, sessions, divergences


def cmd_run(shell: Shell, *, dry_run: bool, workspace: str) -> int:
    now = dt.datetime.now(dt.timezone.utc)
    issues, prs, sessions, divergences = snapshot(shell, now, workspace=workspace)
    failures = 0
    for action in decide(issues, prs, sessions, divergences, now):
        print(("[dry run] " if dry_run else "") + describe(action), flush=True)
        if dry_run:
            continue
        try:
            execute(action, shell, workspace=workspace)
        except subprocess.CalledProcessError as err:
            failures += 1
            print(f"  failed ({err.returncode}): {(err.stderr or '').strip()[:500]}",
                  file=sys.stderr, flush=True)
    return 1 if failures else 0


def main(argv: Sequence[str] | None = None, shell: Shell | None = None) -> int:
    parser = argparse.ArgumentParser(description="The drift heal loop (SELF-HEALING.md, Repair).")
    parser.add_argument("--workspace", default=WORKSPACE,
                        help="the loom workspace (default: %(default)s)")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="poll once and act: the timer's command")
    run.add_argument("--dry-run", action="store_true", help="print the actions, take none")
    args = parser.parse_args(argv)
    shell = shell or Shell()
    return cmd_run(shell, dry_run=args.dry_run, workspace=args.workspace)


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: `45 passed`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(heal): run, the poller's snapshot and the actions it takes" -- scripts/heal_drift.py .github/scripts/tests/test_heal_drift.py
```

---

### Task 4: `publish`, `escalate` and their refusals

**Files:**
- Modify: `scripts/heal_drift.py` (append the guard; replace `main`)
- Modify: `.github/scripts/tests/test_heal_drift.py` (append)

- [ ] **Step 1: Write the failing refusal tests**

Append to `.github/scripts/tests/test_heal_drift.py`:

```python
# --- publish: spec §4.1 --------------------------------------------------------------------


def git(cwd, *args):
    return subprocess.run(["git", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null",
                           *args], cwd=cwd, check=True, capture_output=True, text=True).stdout


def write(root, path, text):
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text)


def commit(work, path, text):
    write(work, path, text)
    git(work, "add", path)
    git(work, "commit", "--quiet", "-m", f"touch {path}")


def guarded_file(path):
    return path + "SKILL.md" if path.endswith("/") else path


@pytest.fixture
def repo(tmp_path, monkeypatch):
    """A heal branch one commit ahead of a bare origin's main, as a heal session leaves it."""
    for key in ("GIT_AUTHOR_NAME", "GIT_COMMITTER_NAME"):
        monkeypatch.setenv(key, "heal test")
    for key in ("GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL"):
        monkeypatch.setenv(key, "heal@example.com")
    origin, work = tmp_path / "origin.git", tmp_path / "work"
    git(tmp_path, "init", "--quiet", "--bare", "--initial-branch=main", str(origin))
    git(tmp_path, "init", "--quiet", "--initial-branch=main", str(work))
    git(work, "remote", "add", "origin", str(origin))
    write(work, "Cargo.toml", '[workspace.package]\nversion = "0.48.0"\n')
    write(work, "docs/specs/kalshi/asyncapi.yaml", "channels: {}\n")
    for path in heal.GUARDED:
        write(work, guarded_file(path), "guarded\n")
    git(work, "add", ".")
    git(work, "commit", "--quiet", "-m", "base")
    git(work, "push", "--quiet", "origin", "main")
    git(work, "switch", "--quiet", "-c", BRANCH)
    commit(work, "docs/specs/kalshi/asyncapi.yaml", "channels: {ebattle_scores: {}}\n")
    return work, origin


def publish_shell(work, *, session_issue=96, title=TITLE_96, branch=BRANCH, state="OPEN",
                  open_prs="[]"):
    row = {**SESSION, "title": title, "issue": session_issue, "branch": branch,
           "worktree": str(work)}
    return FakeShell({
        ("loom", "session", "list"): json.dumps([row]),
        ("gh", "issue", "view"): json.dumps({**ISSUE_96, "state": state}),
        ("gh", "pr", "list"): open_prs,
        ("gh", "pr", "create"): "https://github.com/dilettante-trading/polyoxide/pull/200\n",
    })


def publish(shell, work, *, sha=SHA_96, row="R32"):
    return heal.publish(shell, issue=96, title="docs(spec): adopt kalshi-ws drift",
                        body="Adopted the ebattle_scores channel.", sha=sha, row=row, cwd=work,
                        now=NOW)


def heads(origin):
    return git(origin, "for-each-ref", "--format=%(refname)", "refs/heads").split()


def refused(work, origin, shell, **kwargs):
    with pytest.raises(heal.Refused):
        publish(shell, work, **kwargs)
    assert heads(origin) == ["refs/heads/main"]
    assert shell.commands("gh", "pr", "create") == shell.commands("gh", "pr", "edit") == []


def test_publish_pushes_the_heal_branch_and_opens_a_pr_that_closes_the_issue(repo):
    work, origin = repo
    shell = publish_shell(work)
    assert publish(shell, work) == "https://github.com/dilettante-trading/polyoxide/pull/200"
    assert heads(origin) == [f"refs/heads/{BRANCH}", "refs/heads/main"]
    assert git(origin, "rev-parse", BRANCH) == git(work, "rev-parse", "HEAD")
    [(create, body)] = shell.commands("gh", "pr", "create")
    assert create[3:] == ["--base", "main", "--head", BRANCH, "--title",
                          "docs(spec): adopt kalshi-ws drift", "--body-file", "-"]
    assert "\n\nCloses #96\n\n" in body
    assert heal.Marker.parse(body) == heal.Marker(96, "kalshi-ws", SHA_96, "R32", NOW)


def test_publish_updates_an_open_pr_in_place(repo):
    work, _ = repo
    url = "https://github.com/dilettante-trading/polyoxide/pull/200"
    shell = publish_shell(work, open_prs=json.dumps([{"number": 200, "url": url}]))
    assert publish(shell, work) == url
    assert shell.commands("gh", "pr", "create") == []
    [(edit, body)] = shell.commands("gh", "pr", "edit")
    assert edit[3] == "200" and "Closes #96" in body


@pytest.mark.parametrize("path", heal.GUARDED)
def test_publish_refuses_a_heal_that_changes_a_guarded_path(repo, path):
    work, origin = repo
    commit(work, guarded_file(path), "changed\n")
    refused(work, origin, publish_shell(work))


TREE_REFUSALS = {
    "a bumped version": lambda work: commit(
        work, "Cargo.toml", '[workspace.package]\nversion = "0.49.0"\n'),
    "an untracked file": lambda work: write(work, "scratch.md", "notes\n"),
    "a guarded file moved away": lambda work: (
        git(work, "mv", "scripts/publish_order.py", "scripts/order.py"),
        git(work, "commit", "--quiet", "-m", "move it")),
    "a detached head": lambda work: git(work, "switch", "--quiet", "--detach"),
}


@pytest.mark.parametrize("break_it", TREE_REFUSALS.values(), ids=TREE_REFUSALS.keys())
def test_publish_refuses_a_tree_a_heal_must_never_push(repo, break_it):
    work, origin = repo
    break_it(work)
    refused(work, origin, publish_shell(work))


SESSION_REFUSALS = {
    "a session linked to another issue": dict(session_issue=94, title="heal kalshi-ws 94 R32"),
    "a closed issue": dict(state="CLOSED"),
    "another session's branch": dict(branch="aidanb/other"),
    "a row the title did not reserve": dict(title="heal kalshi-ws 96 R31"),
}


@pytest.mark.parametrize("kwargs", SESSION_REFUSALS.values(), ids=SESSION_REFUSALS.keys())
def test_publish_refuses_a_session_or_issue_that_does_not_match(repo, kwargs):
    work, origin = repo
    refused(work, origin, publish_shell(work, **kwargs))


@pytest.mark.parametrize("sha,row", [("not-hex", "R32"), (SHA_96, "R-32")], ids=["sha", "row"])
def test_publish_refuses_a_malformed_marker(repo, sha, row):
    work, origin = repo
    refused(work, origin, publish_shell(work), sha=sha, row=row)


def test_publish_refuses_main_itself(repo):
    work, origin = repo
    git(work, "switch", "--quiet", "main")
    commit(work, "docs/specs/kalshi/asyncapi.yaml", "channels: {x: {}}\n")
    refused(work, origin, publish_shell(work, branch="main"))


def test_publish_refuses_outside_a_session_worktree(repo, tmp_path):
    work, origin = repo
    shell = publish_shell(work)
    shell.answers[("loom", "session", "list")] = json.dumps([{
        **SESSION, "title": TITLE_96, "issue": 96, "branch": BRANCH,
        "worktree": str(tmp_path / "elsewhere")}])
    refused(work, origin, shell)


def test_main_reports_a_refusal_and_exits_2(tmp_path, capsys):
    body = tmp_path / "body.md"
    body.write_text("The drift is gone.\n")
    shell = FakeShell({("loom", "session", "list"): "[]"})
    assert heal.main(["escalate", "--issue", "96", "--body-file", str(body)], shell=shell) == 2
    assert "refused" in capsys.readouterr().err
    assert shell.commands("gh") == []
```

- [ ] **Step 2: Run them to see them fail**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: the new tests fail with `AttributeError: module 'heal_drift' has no attribute 'publish'` (or `'Refused'`), and `test_main_reports_a_refusal_and_exits_2` with argparse's `invalid choice: 'escalate'` (exit 2 via `SystemExit`); the earlier 45 pass.

- [ ] **Step 3: Write the guard**

Insert above the `# --- execute, snapshot, run` banner in `scripts/heal_drift.py`:

```python
# --- publish and escalate (spec §4) --------------------------------------------------------


class Refused(Exception):
    """A publish, escalate or install the guard will not make; nothing was pushed or written."""


def toplevel(shell: Shell, cwd: Path) -> Path:
    return Path(shell.run(["git", "rev-parse", "--show-toplevel"], cwd=cwd).strip())


def own_session(shell: Shell, issue: int, top: Path, workspace: str) -> tuple[Session, Issue]:
    """Step 1 of publish and escalate: the caller is the session whose worktree this is, linked
    to `issue`, an open drift issue. samefile, since /tb/Source and /home/aidanb/Source name one
    tree."""
    mine = [s for s in sessions_of(shell, workspace)
            if os.path.exists(s.worktree) and os.path.samefile(s.worktree, top)]
    if len(mine) != 1:
        raise Refused(f"{len(mine)} loom sessions have {top} as their worktree; run this from a "
                      "heal session's worktree")
    session = mine[0]
    if session.issue != issue:
        raise Refused(f"this session is linked to #{session.issue}, not #{issue}")
    found = Issue.from_gh(json.loads(shell.run(["gh", "issue", "view", str(issue), "--json",
                                                ISSUE_FIELDS])))
    if not found.open or DRIFT_LABEL not in found.labels:
        raise Refused(f"#{issue} is not an open {DRIFT_LABEL} issue")
    return session, found


def guarded(paths: Sequence[str]) -> list[str]:
    """The paths among `paths` a heal may not change."""
    return [p for p in paths
            if any(p == g or (g.endswith("/") and p.startswith(g)) for g in GUARDED)]


def workspace_version(shell: Shell, rev: str, top: Path) -> str:
    manifest = tomllib.loads(shell.run(["git", "show", f"{rev}:Cargo.toml"], cwd=top))
    return manifest["workspace"]["package"]["version"]


def publish(shell: Shell, *, issue: int, title: str, body: str, sha: str, row: str, cwd: Path,
            now: dt.datetime, workspace: str = WORKSPACE) -> str:
    """Push the heal session's branch and open or update its PR (spec §4.1); return the PR's
    URL. Every refusal comes before the push, so a refused publish leaves nothing behind."""
    top = toplevel(shell, cwd)
    session, found = own_session(shell, issue, top, workspace)
    if found.spec is None:
        raise Refused(f"#{issue} carries no single spec:<id> label")
    if not SHA.match(sha):
        raise Refused(f"--diff-sha256 {sha!r} is not 64 lowercase hex digits")
    if not ROW.match(row) or row not in ("none", session.row):
        raise Refused(f"--drift-row {row!r} is neither `none` nor the row this session's title "
                      f"reserved ({session.row})")
    if shell.run(["git", "status", "--porcelain"], cwd=top).strip():
        raise Refused("the worktree has uncommitted or untracked files; commit by path, and keep "
                      "scratch files outside it")
    try:
        branch = shell.run(["git", "symbolic-ref", "--quiet", "--short", "HEAD"], cwd=top).strip()
    except subprocess.CalledProcessError:
        raise Refused("HEAD is detached; publish from the heal session's branch") from None
    if branch == "main" or branch != session.branch:
        raise Refused(f"the branch is {branch!r}; a heal publishes only its session's own branch "
                      f"({session.branch!r}), never main")
    shell.run(["git", "fetch", "--quiet", "origin", "main"], cwd=top)
    here = workspace_version(shell, "HEAD", top)
    there = workspace_version(shell, "origin/main", top)
    if here != there:
        raise Refused(f"the workspace version is {here} here and {there} on origin/main; a heal "
                      "never bumps it")
    base = shell.run(["git", "merge-base", "HEAD", "origin/main"], cwd=top).strip()
    changed = shell.run(["git", "diff", "--no-renames", "--name-only", base, "HEAD"],
                        cwd=top).splitlines()
    if hit := guarded(changed):
        raise Refused(f"a heal may not change {', '.join(hit)}")
    shell.run(["git", "push", "--quiet", "origin", f"HEAD:refs/heads/{branch}"], cwd=top)
    text = (f"{body.rstrip()}\n\nCloses #{issue}\n\n"
            f"{Marker(issue, found.spec, sha, row, now).render()}\n")
    existing = json.loads(shell.run(["gh", "pr", "list", "--head", branch, "--state", "open",
                                     "--json", "number,url"], cwd=top))
    if existing:
        shell.run(["gh", "pr", "edit", str(existing[0]["number"]), "--title", title,
                   "--body-file", "-"], cwd=top, input=text)
        return existing[0]["url"]
    return shell.run(["gh", "pr", "create", "--base", "main", "--head", branch, "--title", title,
                      "--body-file", "-"], cwd=top, input=text).strip()
```

`sessions_of` and `Shell` are defined further down the file; Python resolves them at call time, so the order is fine.

- [ ] **Step 4: Replace `main` with the version that has `publish` and `escalate`**

Replace the whole `def main(...)` in `scripts/heal_drift.py` with:

```python
def main(argv: Sequence[str] | None = None, shell: Shell | None = None) -> int:
    parser = argparse.ArgumentParser(description="The drift heal loop (SELF-HEALING.md, Repair).")
    parser.add_argument("--workspace", default=WORKSPACE,
                        help="the loom workspace (default: %(default)s)")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="poll once and act: the timer's command")
    run.add_argument("--dry-run", action="store_true", help="print the actions, take none")
    pub = sub.add_parser("publish", help="push this heal session's branch and open or update its PR")
    pub.add_argument("--issue", type=int, required=True)
    pub.add_argument("--title", required=True, help="the PR's title")
    pub.add_argument("--body-file", required=True, help="the PR's body, from outside the worktree")
    pub.add_argument("--diff-sha256", required=True, help="the fingerprint the heal adopted")
    pub.add_argument("--drift-row", required=True, help="the reserved DRIFT row, or none")
    esc = sub.add_parser("escalate", help="comment on this heal session's issue, label it needs-human")
    esc.add_argument("--issue", type=int, required=True)
    esc.add_argument("--body-file", required=True, help="the comment, from outside the worktree")
    args = parser.parse_args(argv)
    shell = shell or Shell()
    try:
        if args.command == "run":
            return cmd_run(shell, dry_run=args.dry_run, workspace=args.workspace)
        if args.command == "publish":
            now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
            print(publish(shell, issue=args.issue, title=args.title,
                          body=Path(args.body_file).read_text(), sha=args.diff_sha256,
                          row=args.drift_row, cwd=Path.cwd(), now=now, workspace=args.workspace))
            return 0
        own_session(shell, args.issue, toplevel(shell, Path.cwd()), args.workspace)
        escalate_issue(shell, args.issue, Path(args.body_file).read_text().strip())
        print(f"escalated #{args.issue}")
        return 0
    except Refused as refusal:
        print(f"heal_drift: refused: {refusal}", file=sys.stderr)
        return 2
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: `66 passed` (45 + 2 accepted + 6 guarded + 4 tree + 4 session + 2 marker + main itself + outside a worktree + main's exit).

- [ ] **Step 6: Prove one refusal is load-bearing**

Temporarily change `"--no-renames", ` to nothing in `publish`'s `git diff` call, run `loom job run -d "heal mutation" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q -k moved'`, expect `1 failed` (git reports only the new name of a rename; `-k` matches the id "a guarded file moved away", spaces and all), then restore the flag and re-run to `1 passed`.

- [ ] **Step 7: Commit**

```bash
git commit -m "feat(heal): publish and escalate, behind refusals no heal can get past" -- scripts/heal_drift.py .github/scripts/tests/test_heal_drift.py
```

---

### Task 5: `install` and the timer

**Files:**
- Modify: `scripts/heal_drift.py` (insert the installer; replace `main`)
- Modify: `.github/scripts/tests/test_heal_drift.py` (append)

- [ ] **Step 1: Write the failing tests**

Append to `.github/scripts/tests/test_heal_drift.py`:

```python
# --- install -------------------------------------------------------------------------------


def test_install_writes_the_hourly_timer_and_its_service(tmp_path):
    checkout = Path("/home/u/Source/polyoxide")
    written = heal.install(checkout, tmp_path, "/usr/bin:/bin", "/usr/bin/direnv")
    assert [p.name for p in written] == ["polyoxide-heal.service", "polyoxide-heal.timer"]
    assert (tmp_path / "polyoxide-heal.service").read_text() == (
        "[Unit]\n"
        "Description=polyoxide drift heal poller (scripts/heal_drift.py run)\n"
        "\n"
        "[Service]\n"
        "Type=oneshot\n"
        "WorkingDirectory=/home/u/Source/polyoxide\n"
        'Environment="PATH=/usr/bin:/bin"\n'
        "ExecStart=/usr/bin/direnv exec /home/u/Source/polyoxide python3 scripts/heal_drift.py run\n")
    assert (tmp_path / "polyoxide-heal.timer").read_text() == (
        "[Unit]\n"
        "Description=Run the polyoxide drift heal poller hourly\n"
        "\n"
        "[Timer]\n"
        "OnCalendar=hourly\n"
        "Persistent=true\n"
        "RandomizedDelaySec=120\n"
        "\n"
        "[Install]\n"
        "WantedBy=timers.target\n")


def test_install_refuses_a_loom_worktree(tmp_path):
    with pytest.raises(heal.Refused):
        heal.install(Path("/home/u/polyoxide/.loom/worktrees/aidanb/x"), tmp_path, "/usr/bin",
                     "/usr/bin/direnv")
    assert list(tmp_path.iterdir()) == []
```

- [ ] **Step 2: Run them to see them fail**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: 2 failed with `AttributeError: module 'heal_drift' has no attribute 'install'`; 66 passed.

- [ ] **Step 3: Write the installer**

Insert above the `# --- execute, snapshot, run` banner in `scripts/heal_drift.py`:

```python
# --- install -------------------------------------------------------------------------------

SERVICE = """\
[Unit]
Description=polyoxide drift heal poller (scripts/heal_drift.py run)

[Service]
Type=oneshot
WorkingDirectory={checkout}
Environment="PATH={path}"
ExecStart={direnv} exec {checkout} python3 scripts/heal_drift.py run
"""

TIMER = """\
[Unit]
Description=Run the polyoxide drift heal poller hourly

[Timer]
OnCalendar=hourly
Persistent=true
RandomizedDelaySec=120

[Install]
WantedBy=timers.target
"""


def install(checkout: Path, unit_dir: Path, path: str, direnv: str) -> list[Path]:
    """Write the service and its timer into `unit_dir`. The service runs `checkout`'s copy of
    this script through `direnv exec`, so the poller is main's version on CI's tools."""
    if "/.loom/worktrees/" in checkout.resolve().as_posix():
        raise Refused(f"{checkout} is a loom worktree, which loom deletes with its session; "
                      "install from the main checkout")
    unit_dir.mkdir(parents=True, exist_ok=True)
    written = []
    for name, text in (("polyoxide-heal.service",
                        SERVICE.format(checkout=checkout, path=path, direnv=direnv)),
                       ("polyoxide-heal.timer", TIMER)):
        (unit_dir / name).write_text(text)
        written.append(unit_dir / name)
    return written


def cmd_install(shell: Shell, *, unit_dir: Path, enable: bool) -> int:
    direnv = shutil.which("direnv")
    if direnv is None:
        raise Refused("direnv is not on PATH, and the service runs the poller through "
                      "`direnv exec`")
    for written in install(REPO, unit_dir, os.environ.get("PATH", ""), direnv):
        print(f"wrote {written}")
    if enable:
        shell.run(["systemctl", "--user", "daemon-reload"])
        shell.run(["systemctl", "--user", "enable", "--now", "polyoxide-heal.timer"])
        print("enabled polyoxide-heal.timer; `journalctl --user -u polyoxide-heal` is its log")
    return 0
```

- [ ] **Step 4: Replace `main` with the final version**

Replace the whole `def main(...)` in `scripts/heal_drift.py` with:

```python
def main(argv: Sequence[str] | None = None, shell: Shell | None = None) -> int:
    parser = argparse.ArgumentParser(description="The drift heal loop (SELF-HEALING.md, Repair).")
    parser.add_argument("--workspace", default=WORKSPACE,
                        help="the loom workspace (default: %(default)s)")
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="poll once and act: the timer's command")
    run.add_argument("--dry-run", action="store_true", help="print the actions, take none")
    pub = sub.add_parser("publish", help="push this heal session's branch and open or update its PR")
    pub.add_argument("--issue", type=int, required=True)
    pub.add_argument("--title", required=True, help="the PR's title")
    pub.add_argument("--body-file", required=True, help="the PR's body, from outside the worktree")
    pub.add_argument("--diff-sha256", required=True, help="the fingerprint the heal adopted")
    pub.add_argument("--drift-row", required=True, help="the reserved DRIFT row, or none")
    esc = sub.add_parser("escalate", help="comment on this heal session's issue, label it needs-human")
    esc.add_argument("--issue", type=int, required=True)
    esc.add_argument("--body-file", required=True, help="the comment, from outside the worktree")
    ins = sub.add_parser("install", help="write and enable the systemd user timer")
    ins.add_argument("--unit-dir", default="~/.config/systemd/user")
    ins.add_argument("--no-enable", dest="enable", action="store_false",
                     help="write the units without enabling the timer")
    args = parser.parse_args(argv)
    shell = shell or Shell()
    try:
        if args.command == "run":
            return cmd_run(shell, dry_run=args.dry_run, workspace=args.workspace)
        if args.command == "publish":
            now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
            print(publish(shell, issue=args.issue, title=args.title,
                          body=Path(args.body_file).read_text(), sha=args.diff_sha256,
                          row=args.drift_row, cwd=Path.cwd(), now=now, workspace=args.workspace))
            return 0
        if args.command == "escalate":
            own_session(shell, args.issue, toplevel(shell, Path.cwd()), args.workspace)
            escalate_issue(shell, args.issue, Path(args.body_file).read_text().strip())
            print(f"escalated #{args.issue}")
            return 0
        return cmd_install(shell, unit_dir=Path(args.unit_dir).expanduser(), enable=args.enable)
    except Refused as refusal:
        print(f"heal_drift: refused: {refusal}", file=sys.stderr)
        return 2
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `loom job run -d "heal_drift tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_heal_drift.py -q'`
Expected: `68 passed`.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat(heal): install, the hourly systemd user timer" -- scripts/heal_drift.py .github/scripts/tests/test_heal_drift.py
```

---

### Task 6: The skill and the allow rules

**Files:**
- Create: `.claude/skills/heal-drift/SKILL.md`
- Create: `.claude/settings.json`

- [ ] **Step 1: Write the skill**

Create `.claude/skills/heal-drift/SKILL.md`:

````markdown
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
````

- [ ] **Step 2: Write the allow rules**

Create `.claude/settings.json`:

```json
{
  "permissions": {
    "allow": [
      "Bash(python3 scripts/heal_drift.py publish:*)",
      "Bash(python3 scripts/heal_drift.py escalate:*)"
    ]
  }
}
```

- [ ] **Step 3: Check both parse**

Run (it parses two files and compiles nothing, so it runs directly):

```bash
uv run --directory .github/scripts python -c 'import json, re, yaml; json.load(open("../../.claude/settings.json")); t = open("../../.claude/skills/heal-drift/SKILL.md").read(); print(yaml.safe_load(re.match(r"---\n(.*?)\n---\n", t, re.S)[1])["name"])'
```

Expected: `heal-drift`.

- [ ] **Step 4: Commit**

```bash
git add .claude/skills/heal-drift/SKILL.md .claude/settings.json
git commit -m "feat(heal): the heal-drift skill and the two allow rules a heal session publishes through" -- .claude/skills/heal-drift/SKILL.md .claude/settings.json
```

---

### Task 7: The rules file and the docs

**Files:**
- Create: `.claude/rules/heal.md`
- Modify: `CLAUDE.md` (the rules table, after the `specs.md` row)
- Modify: `SELF-HEALING.md` (the diagram; a "Repair" section before "Failure taxonomy"; the taxonomy table; the "Adopting a schema drift" bullet; "Testing the machinery itself")
- Modify: `.claude/rules/specs.md` (the "creates no branches" bullet)

- [ ] **Step 1: Write the rules file**

Create `.claude/rules/heal.md`:

```markdown
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
```

- [ ] **Step 2: Add the CLAUDE.md row**

In `CLAUDE.md`, after the line ``| `specs.md` | the `docs/specs/` mirrors and the schema-drift check |``, insert:

```markdown
| `heal.md` | the drift heal loop: its stateless poller, the `heal-drift` skill, `publish`'s refusals, the timer |
```

- [ ] **Step 3: Update SELF-HEALING.md**

(a) Replace the diagram's last three lines,

```
              clean night ──► issue auto-closed  drift gone or ──► issue
                              "Recovered"        acknowledged      auto-closed
```

with

```
                                 │      hourly heal poller (dev box) ──► one loom
                                 │      session per issue ──► PR that closes it
                                 │                  │
              clean night ──► issue auto-closed  drift gone, adopted ──► issue
                              "Recovered"        or acknowledged      auto-closed
```

(b) Insert before `## Failure taxonomy — what goes where`:

```markdown
## Repair — `scripts/heal_drift.py`

The nightly reports a drift; the heal loop adopts it. A systemd user timer on the owner's dev
box (`polyoxide-heal.timer`, hourly, catching up at boot) runs `scripts/heal_drift.py run` from
the main checkout. For each open `schema-drift` issue it starts one loom session (opus at high
effort, two at a time), which follows `.claude/skills/heal-drift/SKILL.md`: adopt the mirror, or
acknowledge an upstream regression; keep the SDK's types, fixtures and tests agreeing; gate,
review, and publish a PR whose `Closes #N` closes the issue. The owner reviews and merges. A new
endpoint or channel is listed in the PR as a follow-up, not built.

- **Nothing leaves the box but through `publish`.** A heal session never pushes or writes to
  GitHub itself. `heal_drift.py publish` refuses a dirty tree, a branch that is not the session's
  own, a version bump, and any change to the release machinery or to the loop's own guard; then
  it pushes the branch and opens the PR. Merging stays the owner's, so a heal can never land on
  `main` or publish a crate.
- **It follows each PR to a close.** Later polls message the session when upstream moves again,
  the PR conflicts with `main`, or a check fails. The third nudge of one kind, a session stuck at
  a permission prompt for two hours, or one idle with nothing published, labels the issue
  `needs-human` with a comment. Merging the PR kills the session; closing it unmerged labels the
  issue `needs-human`, and the poller leaves a `needs-human` issue alone until the label is
  removed.
- **It keeps no state.** The poller reads GitHub and `loom session list --json` and writes no
  file, so a re-run or a dry run never acts twice.

Operations:

- **Install:** from the main checkout, `direnv exec . python3 scripts/heal_drift.py install`
  writes the two units to `~/.config/systemd/user/` and enables the timer (`--no-enable` only
  writes them). It refuses a loom worktree, which loom deletes with its session.
- **See what it would do:** `direnv exec . python3 scripts/heal_drift.py run --dry-run`.
- **Its log:** `journalctl --user -u polyoxide-heal`.
- **Pause:** `systemctl --user stop polyoxide-heal.timer` (and `disable` to keep it off across
  boots). Running heal sessions finish their turn and stay as they are.
- **Hand an issue back to the loop:** remove its `needs-human` label.
```

(c) In the failure taxonomy table, after the `| Schema drift | …` row, insert:

```markdown
| A heal that stopped | Comment on the drift issue | `needs-human` |
```

(d) Replace the operations bullet that begins `- **Adopting a schema drift** is a maintainer's commit:` (five lines, ending `and because one \`curl\` is all an adoption needs.`) with:

```markdown
- **Adopting a schema drift** is a heal PR (Repair, above) or a maintainer's commit: run the
  `curl` the issue spells out, review the diff, and commit the mirror. The nightly itself opens
  no PR, because the organization forbids Actions from opening pull requests (the
  repository-level setting answers `409 Conflict` while the org policy stands); the heal loop
  opens its PRs from the dev box, as the owner.
```

(e) In `## Testing the machinery itself`, after the sentence ending `via the \`CI Scripts\` job in \`ci.yml\`.`, add: `` `test_heal_drift.py` holds the heal loop's rules, its `publish` refusals and its timer to the design, on captured `gh` and loom output. ``

- [ ] **Step 4: Update specs.md**

In `.claude/rules/specs.md`, replace

```markdown
  0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out.
```

with

```markdown
  0 PRs in run 31811673456), and adopting a drift is one `curl`, which the issue body spells out.
  The drift heal loop (`heal.md`) adopts it from the owner's dev box instead, in a PR that closes
  the issue.
```

- [ ] **Step 5: Run the rules and generated-region tests**

Run: `loom job run -d "rules and registry tests" -- sh -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_gen_registry.py -q'`
Expected: all pass (`heal.md`'s three globs each match a file, CLAUDE.md's table lists it, and no generated region moved).

- [ ] **Step 6: Commit**

```bash
git add .claude/rules/heal.md
git commit -m "docs(heal): the heal loop's rules file, its Repair section in SELF-HEALING.md, and the specs rule naming it" -- .claude/rules/heal.md CLAUDE.md SELF-HEALING.md .claude/rules/specs.md
```

---

### Task 8: The CI scripts job and a dry run

**Files:** none changed.

- [ ] **Step 1: Run the CI scripts job as CI does**

Run: `loom job run -d "CI scripts job" -- sh -c 'cd .github/scripts && uv sync && uv run pytest tests/ -q && cd ../.. && python3 scripts/runtime_gate.py'`
Expected: every test passes, then the runtime gate exits 0 with no output.

- [ ] **Step 2: Dry-run the poller against today's state**

Run: `python3 scripts/heal_drift.py run --dry-run` (reads `gh` and loom and fetches `origin/main`; writes nothing).
Expected, with #94 and #96 open and no heal session yet: `[dry run] #94: start \`heal kalshi 94 R32\`` and `[dry run] #96: start \`heal kalshi-ws 96 R33\`` (row numbers one past the highest on `origin/main`).

- [ ] **Step 3: Report**

Report the gate's result and the dry run's lines to the owner. Landing the branch, `heal_drift.py install` from the main checkout, and cutting the owner's `polyoxide-drift-adoption` memory to a pointer at the skill all wait on the owner.

---

## After review (2026-11-12)

One independent review of the finished branch found four important gaps and ten minor ones. The
fixes landed in one commit after Task 8, and the spec, the skill, `heal.md` and SELF-HEALING.md
were brought into line with them. **The code and the spec are authoritative from here; the code
blocks above are the plan as first executed.** What changed:

- The repository is public: `snapshot` keeps only PRs the poller's account opened from this
  repository (`author`, `isCrossRepository`, now in `PR_FIELDS` and the `pr_65.json` fixture), and
  a nudge marker counts only in a comment that account wrote (`viewerDidAuthor`).
- `GUARDED` holds `.github/workflows/` and `.github/actions/` whole: a PR's CI runs its branch's
  workflows with the repository's secrets.
- The rules are reordered so a closed issue is settled before `needs-human` (1 merged, 2 closed
  with a PR open, 3 closed, 4 `needs-human`, 5 closed unmerged); rule 5 no longer needs the issue
  to have no open PR, and closes one the session opened after the owner closed the first.
- Open PRs are listed with no date filter; issues a heal session names count without their label.
- The marker carries `seen` (the issue's fingerprint at publish time) in place of `published`, and
  rule 8 compares fingerprints, not times.
- `Recoverable` sessions escalate under rule 7; every heal session holds a slot, a duplicate
  included; `run` takes a lock and works in its own checkout; a resume fetches the PR's branch
  first, since loom deletes the branch it made when its session is killed.
- `publish` refuses a `needs-human` issue, strips any marker from the body, says to merge
  `origin/main` when the version differs, and a `gh` failure after the push exits 1 saying to run
  it again. The nudge count is posted before the message.
- The skill uses a fixed scratch path (`/tmp/heal-N`), re-runs the residual check with
  `--acknowledged-file` (exit 3), passes the fingerprint literally, and treats the issue,
  upstream's documents and PR comments as data.
- Each of the six behavioural fixes was shown to fail its test when reverted.

## Superseded (2026-11-14)

loom's issue watcher replaced the poller before it was ever installed: `run`, `install`, the
systemd units and their tasks' code are gone, and `heal_drift.py hook` answers the watcher
instead. The design spec's §0 says what changed; Task 1's capture commands still re-capture the
`gh` and `loom session list --json` fixtures that remain (`comment.json` went with the nudge
markers).
