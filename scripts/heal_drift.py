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
