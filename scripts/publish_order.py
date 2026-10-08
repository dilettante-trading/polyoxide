#!/usr/bin/env python3
"""Publish order and release decisions for the workspace, read from `cargo metadata`.

`release.yml` and `scripts/finish_release.sh` publish through this script, so no
crate list is kept by hand. Four subcommands:

    list     The (crate, version) pairs crates.io does not have yet, in the order
             they must be published: the workspace members, each after every
             member it needs first, then each `tombstones/*/Cargo.toml`. One
             `name version manifest_path` line per pair. With
             `--max-new-names N` it prints nothing and exits 3 when more than N
             of those names have never been published.
    version  The version every publishable member carries. Fails if they disagree.
    tag-sha  The commit origin's `vVERSION` tag points at, or nothing when there is
             no such tag.
    decide   Whether a release.yml run releases the checked-out commit. Prints
             `release` or `skip` on stdout and a log line on stderr, or exits 1
             with `::error::`.
    ci-passed  Exits 0 when a CI run on SHA completed with success, else 1.

Exit codes: 0 on success, 1 on any other error, such as an unreachable
crates.io, 2 for bad arguments (argparse's own), 3 for too many new crate names,
and 4 when the workspace cannot be published as it stands: a cycle, a
dependency on an unpublishable member, or a manifest cargo cannot read. Retrying
cannot fix 3 or 4.

Stdlib only. `list` asks the crates.io API about each pair, with a User-Agent
naming this repository and a second between requests, as crates.io's crawler
policy asks.

Usage:
    python3 scripts/publish_order.py [list] [--max-new-names N]
    python3 scripts/publish_order.py version
    python3 scripts/publish_order.py tag-sha VERSION
    python3 scripts/publish_order.py decide --head-sha SHA [--dispatch]
    python3 scripts/publish_order.py ci-passed SHA
"""
from __future__ import annotations

import argparse
import heapq
import http.client
import json
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from pathlib import Path
from typing import NamedTuple

REPO = Path(__file__).resolve().parents[1]
CRATES_IO = "https://crates.io/api/v1/crates"
USER_AGENT = "polyoxide-release (https://github.com/dilettante-trading/polyoxide)"
REQUEST_INTERVAL = 1.0
COMMAND_TIMEOUT = 120

# A getter takes a URL and request headers and returns the HTTP status.
Getter = Callable[[str, dict], int]
# A runner takes a command and returns the finished process, its output as text.
Runner = Callable[[list], subprocess.CompletedProcess]


class Pair(NamedTuple):
    name: str
    version: str
    manifest: str


class Decision(NamedTuple):
    # `release` or `skip`.
    result: str
    # A line for the run's log, carrying any `::notice::` or `::warning::` prefix.
    log: str


class PublishOrderError(Exception):
    """A condition the release stops on. The message says which."""

    exit_code = 1


class TooManyNewNames(PublishOrderError):
    exit_code = 3


class WorkspaceError(PublishOrderError):
    """The workspace cannot be published as it stands, however often it is retried."""

    exit_code = 4


def run_command(command: list) -> subprocess.CompletedProcess:
    """Run `command` in the repository, capturing its output. Never raises on its exit.

    A stalled `git ls-remote` or `gh` would otherwise hold the job for hours.
    """
    try:
        return subprocess.run(command, cwd=REPO, capture_output=True, text=True,
                              timeout=COMMAND_TIMEOUT)
    except subprocess.TimeoutExpired as err:
        raise PublishOrderError(
            f"`{' '.join(command)}` did not finish within {COMMAND_TIMEOUT}s") from err
    except OSError as err:
        raise PublishOrderError(f"could not run `{command[0]}`: {err}") from err


def _failed(command: list, result: subprocess.CompletedProcess,
            error: type[PublishOrderError] = PublishOrderError) -> PublishOrderError:
    detail = result.stderr.strip() or result.stdout.strip() or "no output"
    return error(f"`{' '.join(command)}` exited {result.returncode}: {detail}")


def cargo_metadata(manifest: Path, run: Runner = run_command) -> dict:
    """`cargo metadata` for one manifest, without resolving dependencies.

    `--no-deps` still lists each package's declared dependencies, which is all the
    order needs, and with `--offline` it never touches the network.
    """
    command = ["cargo", "metadata", "--no-deps", "--offline", "--format-version", "1",
               "--manifest-path", str(manifest)]
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result, WorkspaceError)
    return json.loads(result.stdout)


def workspace_metadata(run: Runner = run_command) -> dict:
    return cargo_metadata(REPO / "Cargo.toml", run)


def tombstone_metadata(root: Path = REPO, run: Runner = run_command) -> list[dict]:
    """One metadata document per `tombstones/*/Cargo.toml`, in directory order.

    Each is its own workspace, which cargo allows only because the root
    `Cargo.toml` excludes `tombstones`.
    """
    manifests = sorted(root.glob("tombstones/*/Cargo.toml"))
    return [cargo_metadata(manifest, run) for manifest in manifests]


def members(metadata: dict) -> list[dict]:
    ids = set(metadata["workspace_members"])
    return [p for p in metadata["packages"] if p["id"] in ids]


def is_publishable(package: dict) -> bool:
    """`publish` unset reads back as null, and `publish = false` as `[]`."""
    return package["publish"] is None or "crates-io" in package["publish"]


def publishable(metadata: dict) -> list[dict]:
    """The workspace members that may go to crates.io."""
    return [p for p in members(metadata) if is_publishable(p)]


def missing_metadata(metadata: dict) -> list[str]:
    """Publishable members crates.io would refuse: no description, or no licence.

    cargo only warns about these when packaging, so the CI package job asks here.
    """
    return sorted(
        p["name"] for p in publishable(metadata)
        if not p.get("description") or not (p.get("license") or p.get("license_file")))


def is_path_only(dependency: dict) -> bool:
    """A `path` with no `version`. cargo strips such a dev-dependency when publishing."""
    return dependency.get("path") is not None and dependency["req"] == "*"


def _needs(package: dict, workspace: set[str], publishing: set[str]) -> set[str]:
    """The publishing members `package` needs on crates.io before its own upload.

    Only dependencies with a `path` are members: one without is a registry crate
    that merely shares a member's name.
    """
    needs = set()
    for dep in package["dependencies"]:
        if dep.get("path") is None or dep["name"] not in workspace:
            continue
        if dep["kind"] == "dev" and is_path_only(dep):
            continue
        if dep["name"] in publishing:
            needs.add(dep["name"])
        else:
            # A versioned dev-dependency stays in the published manifest too.
            raise WorkspaceError(
                f"{package['name']} has a {dep['kind'] or 'normal'} dependency on "
                f"{dep['name']}, which is not published to crates.io, so "
                f"{package['name']} cannot be published either."
            )
    return needs


def publish_order(metadata: dict) -> list[dict]:
    """The publishable members, each after every member it needs on crates.io first.

    An upload must resolve its normal and build dependencies on crates.io, and its
    versioned dev-dependencies too, since those stay in the published manifest. A
    path-only dev-dependency is stripped, so it orders nothing. Ties break by name,
    so every run gives the same order.
    """
    workspace = {p["name"] for p in members(metadata)}
    packages = {p["name"]: p for p in publishable(metadata)}
    waiting = {name: _needs(package, workspace, set(packages))
               for name, package in packages.items()}
    ready = [name for name, needs in waiting.items() if not needs]
    heapq.heapify(ready)
    order = []
    while ready:
        name = heapq.heappop(ready)
        order.append(name)
        for other, needs in waiting.items():
            if name in needs:
                needs.discard(name)
                if not needs:
                    heapq.heappush(ready, other)
    if len(order) < len(waiting):
        cycle = " -> ".join(_cycle({n: needs for n, needs in waiting.items() if needs}))
        raise WorkspaceError(
            f"dependency cycle among publishable crates, each needing the next on "
            f"crates.io first: {cycle}. Make one of the dev-dependencies path-only."
        )
    return [packages[name] for name in order]


def _cycle(blocked: dict[str, set[str]]) -> list[str]:
    """One cycle among crates that never became ready, closed by its first crate.

    Every blocked crate still needs at least one other blocked crate, so following
    those needs from any of them must revisit one. The walk may start outside the
    cycle, so only the part from the revisited crate on is returned.
    """
    path = [min(blocked)]
    while True:
        nxt = min(blocked[path[-1]])
        if nxt in path:
            return path[path.index(nxt):] + [nxt]
        path.append(nxt)


def planned(workspace: dict, tombstones: list[dict]) -> list[Pair]:
    """Every publishable pair in publish order: the workspace, then the tombstones."""
    packages = publish_order(workspace) + [p for doc in tombstones for p in publish_order(doc)]
    return [Pair(p["name"], p["version"], p["manifest_path"]) for p in packages]


def http_status(url: str, headers: dict) -> int:
    """GET `url` and return the status. Only the status is read.

    Any failure to get a status at all, a timeout or a reset included, is a
    `PublishOrderError`: it says nothing about whether the pair exists.
    """
    request = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return response.status
    except urllib.error.HTTPError as err:
        err.close()
        return err.code
    except (urllib.error.URLError, OSError, http.client.HTTPException) as err:
        reason = getattr(err, "reason", None) or err
        raise PublishOrderError(f"could not reach crates.io for {url}: {reason}") from err


def absent(pairs: list[Pair], get: Getter,
           sleep: Callable[[float], None] = time.sleep) -> tuple[list[Pair], list[str]]:
    """The pairs crates.io lacks, and the names among them it has never seen.

    One request per pair, and a second for each absent pair, to tell a new name from
    a new version of a known one. Any status but 200 or 404 stops the release, since
    it says nothing about whether the pair exists.
    """
    headers = {"User-Agent": USER_AGENT}
    sent = 0

    def found(url: str) -> bool:
        nonlocal sent
        if sent:
            sleep(REQUEST_INTERVAL)
        sent += 1
        status = get(url, headers)
        if status not in (200, 404):
            raise PublishOrderError(f"crates.io answered {status} for {url}")
        return status == 200

    missing, new_names = [], []
    for pair in pairs:
        if found(f"{CRATES_IO}/{pair.name}/{pair.version}"):
            continue
        missing.append(pair)
        if not found(f"{CRATES_IO}/{pair.name}"):
            new_names.append(pair.name)
    return missing, new_names


def pending(workspace: dict, tombstones: list[dict], get: Getter,
            sleep: Callable[[float], None] = time.sleep,
            max_new_names: int | None = None) -> list[Pair]:
    """The pairs still to publish, in order, refusing too many first-time names."""
    missing, new_names = absent(planned(workspace, tombstones), get, sleep)
    if max_new_names is not None and len(new_names) > max_new_names:
        plural = "" if len(new_names) == 1 else "s"
        raise TooManyNewNames(
            f"this release would register {len(new_names)} new crate name{plural} on "
            f"crates.io ({', '.join(new_names)}), more than --max-new-names "
            f"{max_new_names} allows. AD-25 caps a release at five new names because "
            f"crates.io rate-limits new crate registrations. Split the release into "
            f"releases that each add at most {max_new_names}."
        )
    return missing


def workspace_version(metadata: dict | None = None) -> str:
    """The version every publishable member carries."""
    if metadata is None:
        metadata = workspace_metadata()
    versions = {p["name"]: p["version"] for p in publishable(metadata)}
    distinct = set(versions.values())
    if len(distinct) != 1:
        listed = ", ".join(f"{name} {version}" for name, version in sorted(versions.items()))
        raise WorkspaceError(
            f"expected one version across the publishable members, found "
            f"{len(distinct)}: {listed or 'no publishable members'}"
        )
    return distinct.pop()


def tag_sha(version: str, run: Runner = run_command) -> str | None:
    """The commit origin's `v<version>` tag names, or None when there is no tag.

    Release tags are annotated, so the peeled `^{}` line is the one naming the
    commit; the direct line names the tag object. A lightweight tag has only the
    direct line, which then names the commit. A failed lookup raises rather than
    reading as "no tag", which would release a version already shipped.
    """
    tag = f"refs/tags/v{version}"
    peeled = f"{tag}^{{}}"
    command = ["git", "ls-remote", "origin", tag, peeled]
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result)
    refs = {}
    for line in result.stdout.splitlines():
        sha, _, ref = line.partition("\t")
        refs[ref] = sha
    return refs.get(peeled) or refs.get(tag)


def parent_version(sha: str, run: Runner = run_command) -> str | None:
    """The workspace version at `sha`'s first parent, or None when it has no parent."""
    # Imported here so `list` still runs on macOS's system python3, which is 3.9.
    import tomllib

    command = ["git", "rev-list", "--parents", "-n", "1", sha]
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result)
    if len(result.stdout.split()) < 2:
        return None
    command = ["git", "show", f"{sha}^:Cargo.toml"]
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result)
    try:
        return tomllib.loads(result.stdout)["workspace"]["package"]["version"]
    except (tomllib.TOMLDecodeError, KeyError, TypeError) as err:
        raise PublishOrderError(
            f"could not read [workspace.package] version from {sha}^:Cargo.toml: {err!r}"
        ) from err


def release_exists(version: str, run: Runner = run_command) -> bool:
    """Whether GitHub has a release named for `v<version>`.

    `gh` exits 1 both for a missing release and for a failed request, a 401 or a
    dropped connection, so exit 1 reads as "no release" only with gh's own
    "release not found". Anything else raises.
    """
    command = ["gh", "release", "view", f"v{version}", "--json", "tagName"]
    result = run(command)
    if result.returncode == 0:
        return True
    if result.returncode == 1 and "release not found" in result.stderr:
        return False
    raise _failed(command, result)


def ci_passed(sha: str, run: Runner = run_command) -> bool:
    """Whether a run of the CI workflow on `sha` has completed with success."""
    command = ["gh", "run", "list", "--workflow", "CI", "--commit", sha,
               "--json", "conclusion,status"]
    result = run(command)
    if result.returncode != 0:
        raise _failed(command, result)
    try:
        runs = json.loads(result.stdout)
    except json.JSONDecodeError as err:
        raise PublishOrderError(f"`{' '.join(command)}` printed no JSON: {err}") from err
    return any(r.get("status") == "completed" and r.get("conclusion") == "success"
               for r in runs)


def _semver_key(version: str) -> tuple:
    """A sort key for a semver version: a pre-release sorts before its release."""
    core, _, pre = version.partition("+")[0].partition("-")
    try:
        numbers = tuple(int(part) for part in core.split("."))
    except ValueError as err:
        raise PublishOrderError(f"{version!r} is not a semver version") from err
    if not pre:
        return numbers, (1,)
    fields = tuple((0, int(f), "") if f.isdigit() else (1, 0, f) for f in pre.split("."))
    return numbers, (0, fields)


def decide(current: str, parent: str | None, tag: str | None, head_sha: str,
           dispatch: bool, released: bool, ci_ok: bool | None) -> Decision:
    """Whether this run releases `current` from `head_sha`. The first matching row wins.

    `tag` is the commit the version's tag names, `released` whether its GitHub
    release exists, and `ci_ok` whether CI passed on `head_sha`, asked only for a
    manual run, since a `workflow_run` run starts only after CI succeeded.
    """
    if dispatch and not ci_ok:
        raise PublishOrderError(f"CI has not passed on {head_sha}")
    if tag is None:
        return Decision("release", f"::notice::Releasing v{current}.")
    if tag == head_sha:
        if released:
            return Decision("skip", f"::notice::v{current} is already released from this commit.")
        return Decision("release", (
            f"::notice::v{current} is tagged at this commit but has no GitHub release, "
            f"so this run resumes it. Publishing skips what crates.io already has, "
            f"and the tag is not pushed again."
        ))
    if not dispatch and parent == current:
        return Decision("skip", (
            f"v{current} was released from {tag}, and this commit does not change "
            f"the version. Nothing to release."
        ))
    if parent is not None and _semver_key(parent) > _semver_key(current):
        return Decision("skip", (
            f"::warning::The version went backwards, from {parent} to {current}, and "
            f"v{current} is already tagged at {tag}. Not releasing."
        ))
    raise PublishOrderError(f"v{current} already tagged at {tag}; bump the version")


def gather(head_sha: str, dispatch: bool, run: Runner = run_command) -> dict:
    """`decide`'s arguments, read from the checkout, origin and GitHub.

    The checkout must be at `head_sha` with its parent fetched, as release.yml's
    `fetch-depth: 2` leaves it.
    """
    current = workspace_version(workspace_metadata(run))
    return {
        "current": current,
        "parent": parent_version(head_sha, run),
        "tag": tag_sha(current, run),
        "head_sha": head_sha,
        "dispatch": dispatch,
        "released": release_exists(current, run),
        "ci_ok": ci_passed(head_sha, run) if dispatch else None,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Publish order and release decisions, from cargo metadata.")
    commands = parser.add_subparsers(dest="command", required=True)
    listing = commands.add_parser(
        "list", help="the pairs crates.io lacks, in publish order (the default)")
    listing.add_argument("--max-new-names", type=int, metavar="N",
                         help="exit 3 if more than N names have never been published")
    commands.add_parser("version", help="the workspace version")
    tagging = commands.add_parser("tag-sha", help="the commit origin's vVERSION tag names")
    tagging.add_argument("version", metavar="VERSION", help="the version, without the v")
    deciding = commands.add_parser(
        "decide", help="whether a release.yml run releases the checked-out commit")
    deciding.add_argument("--head-sha", required=True, help="the checked-out commit")
    deciding.add_argument("--dispatch", action="store_true",
                          help="a manual run: requires CI to have passed on the commit")
    checking = commands.add_parser("ci-passed", help="exit 0 if CI passed on SHA, else 1")
    checking.add_argument("sha", metavar="SHA")
    commands.add_parser(
        "check-manifests",
        help="exit 1 if a publishable member lacks a description or a licence")
    return parser


def main(argv: list[str] | None = None, get: Getter = http_status,
         sleep: Callable[[float], None] = time.sleep, run: Runner = run_command) -> int:
    argv = sys.argv[1:] if argv is None else list(argv)
    if not argv or (argv[0].startswith("-") and argv[0] not in ("-h", "--help")):
        argv = ["list", *argv]
    args = _parser().parse_args(argv)
    try:
        if args.command == "list":
            pairs = pending(workspace_metadata(run), tombstone_metadata(REPO, run), get,
                            sleep, args.max_new_names)
            for pair in pairs:
                print(*pair)
        elif args.command == "version":
            print(workspace_version(workspace_metadata(run)))
        elif args.command == "tag-sha":
            sha = tag_sha(args.version, run)
            if sha:
                print(sha)
        elif args.command == "check-manifests":
            missing = missing_metadata(workspace_metadata(run))
            if missing:
                print("::error::Publishable crates missing a description or a licence, "
                      f"which crates.io refuses: {', '.join(missing)}", file=sys.stderr)
                return 1
        elif args.command == "ci-passed":
            if not ci_passed(args.sha, run):
                print(f"::error::CI has not passed on {args.sha}", file=sys.stderr)
                return 1
        else:
            decision = decide(**gather(args.head_sha, args.dispatch, run))
            print(decision.log, file=sys.stderr)
            print(decision.result)
    except PublishOrderError as err:
        print(f"::error::{err}", file=sys.stderr)
        return err.exit_code
    return 0


if __name__ == "__main__":
    sys.exit(main())
