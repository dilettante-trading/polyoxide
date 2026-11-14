#!/usr/bin/env python3
"""The drift heal loop's repository half: the loom watcher's hook, and the one way out a heal
session has.

loom's issue watcher, which `loom-watch.json` at the repository root configures, starts one loom
session per open `schema-drift` issue, nudges it when its PR goes red or conflicts, escalates it
when it sticks, and kills it once the PR merges (loom's docs/specs/watch.md). `hook` is that
watcher's hook: `start` names the issue's spec and reserves its DRIFT row, and `nudges` tells a
session the nightly saw upstream move after it published. `publish` and `escalate` are what a
heal session calls; `publish` refuses, in code, everything a heal must never push.

The design is docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md, the operator's guide
SELF-HEALING.md's "Repair" section, and the rules .claude/rules/heal.md.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import re
import subprocess
import sys
import tomllib
from collections.abc import Iterable, Mapping, Sequence
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
WORKSPACE = "polyoxide"
DRIFT_LABEL = "schema-drift"
HUMAN_LABEL = "needs-human"
DIVERGENCES = "_bmad-output/specs/spec-venue-extensibility/divergences.md"
# What a heal may never change (spec §4.1 step 4): the workflows, which a PR's CI runs from its
# own branch with the repository's secrets; the release machinery; and the loop itself, whose
# `loom-watch.json` names the command the loom daemon runs as its hook; and what configures the
# Claude sessions loom starts in a checkout of main. An entry ending in `/` guards everything
# under it.
GUARDED = (
    ".github/workflows/",
    ".github/actions/",
    "scripts/finish_release.sh",
    "scripts/publish_order.py",
    "scripts/heal_drift.py",
    ".claude/settings.json",
    ".claude/settings.local.json",
    ".claude/agents/",
    ".claude/hooks/",
    ".claude/skills/heal-drift/",
    ".mcp.json",
    "loom-watch.json",
    ".loom-watch/",
)
ISSUE_FIELDS = "number,state,labels,body"

FINGERPRINT = re.compile(r'"diff_sha256":\s*"([0-9a-f]{64})"')
MARKER = re.compile(
    r"<!-- polyoxide-heal: issue=(?P<issue>\d+) spec=(?P<spec>\S+) "
    r"diff_sha256=(?P<sha>[0-9a-f]{64}) seen=(?P<seen>[0-9a-f]{64}|none) "
    r"drift_row=(?P<row>R\d+|none) -->")
# Anything shaped like a marker, which publish strips from a body before it writes the real one.
ANY_MARKER = re.compile(r"<!-- polyoxide-heal:[^>]*-->")
# `loom-watch.json`'s title template, `heal {spec} {issue} {row}`, as the watcher renders it.
TITLE = re.compile(r"^heal (?P<spec>\S+) (?P<issue>\d+) (?P<row>R\d+|none)$")
DRIFT_ROW = re.compile(r"^\| R(\d+) \|", re.MULTILINE)
SHA = re.compile(r"^[0-9a-f]{64}$")
ROW = re.compile(r"^(?:R\d+|none)$")
# What the watcher lets a title variable hold: [A-Za-z0-9._-]+ with a letter or digit in it.
TITLE_VALUE = re.compile(r"^[A-Za-z0-9._-]*[A-Za-z0-9][A-Za-z0-9._-]*$")


def highest_row(divergences: str) -> int:
    """The highest DRIFT row number in divergences.md's table, or 0."""
    return max((int(n) for n in DRIFT_ROW.findall(divergences)), default=0)


def spec_of(labels: Iterable[str]) -> str | None:
    """The id of an issue's one `spec:<id>` label."""
    specs = [name.removeprefix("spec:") for name in labels if name.startswith("spec:")]
    return specs[0] if len(specs) == 1 else None


def fingerprint_of(body: str) -> str | None:
    """The `diff_sha256` the nightly writes into an issue body's acknowledgement block, or None
    unless there is exactly one."""
    found = FINGERPRINT.findall(body)
    return found[0] if len(found) == 1 else None


# Any heal branch: loom names a session's branch after its title, `heal <spec> <issue> <row>`,
# lower-cased with dashes for spaces, after the workspace's branch prefix.
HEAL_BRANCH = re.compile(r"(?:^|/)heal-[a-z0-9._-]+-\d+-r(\d+)$")


def heal_branch(branch: str, issue: int) -> tuple[str, str] | None:
    """The spec and DRIFT row a heal branch for `issue` encodes, as the title held them (`R<n>`
    or `none`), or None for any other branch."""
    m = re.search(rf"(?:^|/)heal-(?P<spec>[a-z0-9._-]+)-{issue}-(?P<row>r\d+|none)$", branch)
    if m is None:
        return None
    return m["spec"], "none" if m["row"] == "none" else "R" + m["row"][1:]


def branch_rows(refs: Iterable[str]) -> list[int]:
    """The DRIFT rows every heal branch git knows encodes, local or on origin, whatever became of
    its session and PR: a PR the owner closed unmerged leaves its branch behind, and a new heal
    given that row would get that branch again, with the commits the owner rejected."""
    return [int(m[1]) for ref in refs if (m := HEAL_BRANCH.search(ref))]


def refs_of(repo: Path) -> list[str]:
    """The repository's local and remote-tracking branches. The watcher's checkout is a worktree
    of the repository, so it sees every branch a heal session made or pushed."""
    done = subprocess.run(["git", "for-each-ref", "--format=%(refname)", "refs/heads",
                           "refs/remotes"], cwd=repo, capture_output=True, text=True, check=True)
    return done.stdout.splitlines()


@dataclasses.dataclass(frozen=True)
class Issue:
    number: int
    open: bool
    labels: frozenset[str]
    body: str

    @classmethod
    def from_gh(cls, d: Mapping) -> Issue:
        return cls(number=d["number"], open=d["state"] == "OPEN",
                   labels=frozenset(label["name"] for label in d["labels"]),
                   body=d["body"] or "")

    @property
    def spec(self) -> str | None:
        return spec_of(self.labels)

    @property
    def fingerprint(self) -> str | None:
        return fingerprint_of(self.body)


@dataclasses.dataclass(frozen=True)
class Marker:
    """The line `publish` ends a heal PR's body with (spec §3.2): the fingerprint the heal
    adopted, and the issue's fingerprint when it published, which the `nudges` hook compares with
    the issue's fingerprint now. Content, not time: a comment on the issue moves nothing here."""
    issue: int
    spec: str
    sha: str
    seen: str  # the issue's fingerprint at publish time, or `none`
    row: str

    def render(self) -> str:
        return (f"<!-- polyoxide-heal: issue={self.issue} spec={self.spec} "
                f"diff_sha256={self.sha} seen={self.seen} drift_row={self.row} -->")

    @classmethod
    def parse(cls, body: str) -> Marker | None:
        found = list(MARKER.finditer(body))
        if len(found) != 1:
            return None
        m = found[0]
        return cls(issue=int(m["issue"]), spec=m["spec"], sha=m["sha"], seen=m["seen"],
                   row=m["row"])


@dataclasses.dataclass(frozen=True)
class Session:
    """A row of `loom session list --json`."""
    id: int
    title: str
    branch: str
    worktree: str
    issue: int

    @classmethod
    def from_loom(cls, d: Mapping) -> Session:
        return cls(id=d["id"], title=d["title"], branch=d["branch"], worktree=d["worktree"],
                   issue=d["issue"])

    @property
    def heal(self) -> bool:
        """A session the watcher started: its title names the issue it is linked to."""
        m = TITLE.match(self.title)
        return m is not None and int(m["issue"]) == self.issue

    @property
    def row(self) -> str | None:
        """The DRIFT row the hook reserved for a heal session, from its title."""
        m = TITLE.match(self.title)
        return m["row"] if m is not None and self.heal else None


# --- the watcher's hook (loom's docs/specs/watch.md, "The hook") ---------------------------


def held_rows(data: Mapping) -> list[int]:
    """The DRIFT rows the watcher's sessions and open PRs hold: each session's title row, and
    each open PR's marker row and the row its branch encodes (an acknowledgement PR's marker says
    `none`, while its branch still carries the row its session reserved)."""
    rows = [(s.get("vars") or {}).get("row", "") for s in data.get("sessions") or []]
    for pr in data.get("prs") or []:
        if marker := Marker.parse(pr.get("body") or ""):
            rows.append(marker.row)
        rows.append((heal_branch(pr.get("branch") or "", pr["issue"]) or ("", ""))[1])
    return [int(r[1:]) for r in rows if re.fullmatch(r"R\d+", r)]


def hook_start(data: Mapping, divergences: str, refs: Iterable[str] = ()) -> dict:
    """The `start` point: the title's `spec` and `row` and the prompt's `fingerprint`, or a
    refusal. A new heal takes the issue's spec label and reserves the row past every one main's
    table, a heal session, an open heal PR or any heal branch holds. A resume (`pr` set) takes
    both from its PR's branch, since the watcher starts a resume only when the title it renders
    makes that branch again: the issue's label may have changed since, and an acknowledgement
    PR's marker says `none` for the row its branch carries."""
    issue = data["issue"]
    number = issue["number"]
    sha = fingerprint_of(issue.get("body") or "")
    if sha is None:
        return {"start": False, "reason": f"#{number}'s body holds no single diff_sha256"}
    if pr := data.get("pr"):
        found = heal_branch(pr["branch"], number)
        if found is None:
            return {"start": False, "reason": f"PR #{pr['number']}'s branch {pr['branch']} is "
                    f"not a heal branch for #{number}"}
        spec, row = found
    else:
        spec = spec_of(issue.get("labels") or [])
        if spec is None or not TITLE_VALUE.match(spec):
            return {"start": False, "reason": f"#{number} carries no single spec:<id> label"}
        row = f"R{max([highest_row(divergences), *held_rows(data), *branch_rows(refs)]) + 1}"
    return {"start": True, "vars": {"spec": spec, "row": row, "fingerprint": sha}}


def hook_nudges(data: Mapping) -> dict:
    """The `nudges` point: `redrift`, keyed by the issue's fingerprint, when the nightly wrote
    one that is neither the fingerprint the PR adopted nor the one the issue showed when it
    published, so upstream moved since. The watcher counts it, spaces it and escalates on the
    third; loom's own `conflict` and `ci` nudges follow it."""
    issue, pr = data["issue"], data["pr"]
    sha = fingerprint_of(issue.get("body") or "")
    marker = Marker.parse(pr.get("body") or "")
    if sha is None or marker is None or marker.issue != issue["number"]:
        return {"nudges": []}
    if sha in (marker.sha, marker.seen):
        return {"nudges": []}
    text = (f"Upstream moved: issue #{issue['number']}'s fingerprint is now diff_sha256={sha}, "
            f"and PR #{pr['number']} adopted {marker.sha}. Re-adopt from step 2 of heal-drift "
            "and publish again.")
    return {"nudges": [{"kind": "redrift", "key": sha, "text": text}]}


def hook(point: str, data: Mapping, repo: Path = REPO) -> dict:
    """Answer the watcher at `point`. It runs the hook in its own checkout of origin/main, which
    it refreshes before the hook runs, so `repo`'s divergences.md is main's; a failed read of it
    or of the refs fails the call, and the issue waits rather than take a row in use."""
    if point == "start":
        if data.get("pr"):
            return hook_start(data, "")
        return hook_start(data, (repo / DIVERGENCES).read_text(), refs_of(repo))
    return hook_nudges(data)


# --- publish and escalate (spec §4) --------------------------------------------------------


class Refused(Exception):
    """A publish or escalate the guard will not make; nothing was pushed or written."""


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
            workspace: str = WORKSPACE) -> str:
    """Push the heal session's branch and open or update its PR (spec §4.1); return the PR's
    URL. Every refusal comes before the push, so a refused publish leaves nothing behind."""
    top = toplevel(shell, cwd)
    session, found = own_session(shell, issue, top, workspace)
    if HUMAN_LABEL in found.labels:
        raise Refused(f"#{issue} carries `{HUMAN_LABEL}`: the owner has it, and a heal publishes "
                      "nothing more")
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
        raise Refused(f"the workspace version is {here} here and {there} on origin/main: merge "
                      "origin/main, and never edit the version, which a heal never changes")
    base = shell.run(["git", "merge-base", "HEAD", "origin/main"], cwd=top).strip()
    changed = shell.run(["git", "diff", "--no-renames", "--name-only", base, "HEAD"],
                        cwd=top).splitlines()
    if hit := guarded(changed):
        raise Refused(f"a heal may not change {', '.join(hit)}")
    shell.run(["git", "push", "--quiet", "origin", f"HEAD:refs/heads/{branch}"], cwd=top)
    marker = Marker(issue, found.spec, sha, found.fingerprint or "none", row)
    text = f"{ANY_MARKER.sub('', body).rstrip()}\n\nCloses #{issue}\n\n{marker.render()}\n"
    existing = json.loads(shell.run(["gh", "pr", "list", "--head", branch, "--state", "open",
                                     "--json", "number,url"], cwd=top))
    if existing:
        shell.run(["gh", "pr", "edit", str(existing[0]["number"]), "--title", title,
                   "--body-file", "-"], cwd=top, input=text)
        return existing[0]["url"]
    return shell.run(["gh", "pr", "create", "--base", "main", "--head", branch, "--title", title,
                      "--body-file", "-"], cwd=top, input=text).strip()


# --- the shell and main --------------------------------------------------------------------


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


def escalate_issue(shell: Shell, issue: int, reason: str) -> None:
    """Comment on the issue and label it needs-human, creating the label if need be."""
    shell.run(["gh", "label", "create", HUMAN_LABEL, "--color", "D93F0B", "--description",
               "The drift heal loop stopped; the owner decides", "--force"])
    shell.run(["gh", "issue", "comment", str(issue), "--body-file", "-"], input=f"heal: {reason}\n")
    shell.run(["gh", "issue", "edit", str(issue), "--add-label", HUMAN_LABEL])


def sessions_of(shell: Shell, workspace: str) -> list[Session]:
    return [Session.from_loom(d) for d in
            json.loads(shell.run(["loom", "session", "list", "--json", "-w", workspace]))]


def main(argv: Sequence[str] | None = None, shell: Shell | None = None) -> int:
    parser = argparse.ArgumentParser(description="The drift heal loop (SELF-HEALING.md, Repair).")
    parser.add_argument("--workspace", default=WORKSPACE,
                        help="the loom workspace (default: %(default)s)")
    sub = parser.add_subparsers(dest="command", required=True)
    hk = sub.add_parser("hook", help="answer loom's issue watcher: JSON on stdin and stdout")
    hk.add_argument("point", choices=["start", "nudges"])
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
    if args.command == "hook":
        # stdout carries one JSON object and nothing else; a fault is a traceback on stderr and
        # a non-zero exit, which the watcher reports as the hook's failure.
        print(json.dumps(hook(args.point, json.load(sys.stdin))))
        return 0
    shell = shell or Shell()
    try:
        if args.command == "publish":
            print(publish(shell, issue=args.issue, title=args.title,
                          body=Path(args.body_file).read_text(), sha=args.diff_sha256,
                          row=args.drift_row, cwd=Path.cwd(), workspace=args.workspace))
            return 0
        own_session(shell, args.issue, toplevel(shell, Path.cwd()), args.workspace)
        escalate_issue(shell, args.issue, Path(args.body_file).read_text().strip())
        print(f"escalated #{args.issue}")
        return 0
    except Refused as refusal:
        print(f"heal_drift: refused: {refusal}", file=sys.stderr)
        return 2
    except subprocess.CalledProcessError as err:
        print(f"heal_drift: `{' '.join(err.cmd[:4])} ...` failed ({err.returncode}): "
              f"{(err.stderr or '').strip()[:500]}\nRun the same command again: publish pushes "
              "the same branch again and opens or updates its PR.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
