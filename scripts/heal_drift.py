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
