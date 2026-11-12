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


if __name__ == "__main__":
    sys.exit(main())
