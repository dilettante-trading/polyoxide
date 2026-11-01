"""Checks on .github/workflows/release.yml's guards.

`branches: [main]` on a `workflow_run` trigger matches the triggering run's head
branch, which for a fork's pull request is the fork's branch name. So a fork PR
from a branch named `main` would start a release with this repository's tokens,
and only the `version` job's `if:` stands in the way. Nothing runs that
expression outside GitHub, so these tests evaluate it here, against the event
each case would send.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest
import yaml

REPO = Path(__file__).resolve().parents[3]
WORKFLOW = yaml.safe_load((REPO / ".github" / "workflows" / "release.yml").read_text())
JOBS = WORKFLOW["jobs"]
THIS_REPO = "dilettante-trading/polyoxide"

TOKEN_RE = re.compile(
    r"\s*(?:'(?P<string>[^']*)'|(?P<op>==|!=|&&|\|\||\(|\))|(?P<path>[A-Za-z_][\w.-]*))"
)


def _tokens(expression: str) -> list[tuple[str, str]]:
    tokens, pos, text = [], 0, expression.strip()
    while pos < len(text):
        match = TOKEN_RE.match(text, pos)
        if match is None:
            raise ValueError(f"cannot parse {text[pos:]!r}")
        pos = match.end()
        kind = match.lastgroup
        tokens.append((kind, match[kind]))
    return tokens


def evaluate(expression: str, context: dict) -> bool:
    """A GitHub expression built from `==`, `!=`, `&&`, `||`, parentheses, string
    literals and context paths, with GitHub's precedence. Anything else fails to
    parse, so a guard written some other way fails these tests rather than
    passing them unread.

    GitHub compares strings ignoring case, and a missing property is null, so both
    are modelled.
    """
    tokens = _tokens(expression)
    pos = 0

    def peek() -> tuple[str, str] | None:
        return tokens[pos] if pos < len(tokens) else None

    def take() -> tuple[str, str]:
        nonlocal pos
        if pos >= len(tokens):
            raise ValueError(f"{expression!r} ends early")
        pos += 1
        return tokens[pos - 1]

    def lookup(path: str):
        value = context
        for key in path.split("."):
            value = value.get(key) if isinstance(value, dict) else None
        return value.lower() if isinstance(value, str) else value

    def operand():
        kind, value = take()
        if (kind, value) == ("op", "("):
            inner = either()
            if take() != ("op", ")"):
                raise ValueError(f"unbalanced parentheses in {expression!r}")
            return inner
        if kind == "string":
            return value.lower()
        if kind == "path":
            return lookup(value)
        raise ValueError(f"unexpected {value!r} in {expression!r}")

    def comparison():
        left = operand()
        if peek() in (("op", "=="), ("op", "!=")):
            op = take()[1]
            right = operand()
            return left == right if op == "==" else left != right
        return left

    def both() -> bool:
        result = bool(comparison())
        while peek() == ("op", "&&"):
            take()
            result = bool(comparison()) and result
        return result

    def either() -> bool:
        result = both()
        while peek() == ("op", "||"):
            take()
            result = both() or result
        return result

    result = either()
    if pos != len(tokens):
        raise ValueError(f"trailing {tokens[pos][1]!r} in {expression!r}")
    return result


def workflow_run(*, conclusion: str = "success", event: str = "push",
                 head_branch: str = "main", head_repository: str = THIS_REPO) -> dict:
    """The context of a Release run started by a completed CI run."""
    return {"github": {
        "event_name": "workflow_run",
        "repository": THIS_REPO,
        "ref": "refs/heads/main",
        "event": {"workflow_run": {
            "conclusion": conclusion,
            "event": event,
            "head_branch": head_branch,
            "head_repository": {"full_name": head_repository},
        }},
    }}


def dispatch(ref: str) -> dict:
    return {"github": {"event_name": "workflow_dispatch", "repository": THIS_REPO,
                       "ref": ref, "event": {}}}


VERSION_IF = JOBS["version"]["if"]


def test_the_evaluator_follows_githubs_precedence() -> None:
    """`&&` binds tighter than `||`, as in GitHub, and parentheses override it."""
    context = {"a": "x"}
    assert evaluate("a == 'x' || a == 'y' && a == 'z'", context)
    assert not evaluate("(a == 'x' || a == 'y') && a == 'z'", context)
    with pytest.raises(ValueError):
        evaluate("!(a == 'x')", context)


def test_a_push_to_main_in_this_repository_releases() -> None:
    assert evaluate(VERSION_IF, workflow_run())


@pytest.mark.parametrize("head_branch", ["main", "MAIN"])
def test_a_fork_pull_request_from_a_branch_named_main_is_skipped(head_branch: str) -> None:
    """The case `branches: [main]` lets through. GitHub's `==` ignores case."""
    context = workflow_run(event="pull_request", head_branch=head_branch,
                           head_repository="someone/polyoxide")
    assert not evaluate(VERSION_IF, context)


@pytest.mark.parametrize("change", [
    {"conclusion": "failure"},
    {"event": "pull_request"},
    {"head_branch": "feature"},
    {"head_repository": "someone/polyoxide"},
], ids=["conclusion", "event", "head_branch", "head_repository"])
def test_each_workflow_run_condition_is_required(change: dict) -> None:
    """Each of the four conditions alone keeps the release from running."""
    assert not evaluate(VERSION_IF, workflow_run(**change))


@pytest.mark.parametrize(("ref", "runs"), [
    ("refs/heads/main", True),
    ("refs/heads/feature", False),
    ("refs/tags/v1.0.0", False),
])
def test_a_manual_run_releases_only_on_main(ref: str, runs: bool) -> None:
    assert evaluate(VERSION_IF, dispatch(ref)) is runs


@pytest.mark.parametrize("job", ["claim", "publish", "publish-python", "release"])
def test_jobs_holding_tokens_check_the_repository(job: str) -> None:
    condition = " ".join(JOBS[job]["if"].split())
    assert f"github.repository == '{THIS_REPO}'" in condition


def test_one_publish_loop_runs_at_a_time() -> None:
    """On the publish job, which runs only to release. At workflow level, a run that
    can never release would join the group, and GitHub cancels the older pending
    run, which could be a real release."""
    assert "concurrency" not in WORKFLOW
    assert JOBS["publish"]["concurrency"] == {"group": "release", "cancel-in-progress": False}


def test_the_registry_token_reaches_cargo_through_the_environment() -> None:
    """A token in `run:` is pasted into a shell command; in `env:` cargo reads it."""
    steps = JOBS["publish"]["steps"]
    publishing = [s for s in steps if "finish_release.sh" in s.get("run", "")]
    assert len(publishing) == 1
    assert publishing[0]["env"]["CARGO_REGISTRY_TOKEN"] == "${{ secrets.CARGO_REGISTRY_TOKEN }}"
    for job in JOBS.values():
        for step in job.get("steps", []):
            assert "secrets." not in step.get("run", ""), step.get("name")


# --- the semver job ----------------------------------------------------------

SEMVER = JOBS["semver"]


def test_the_semver_job_runs_only_when_releasing() -> None:
    assert SEMVER["needs"] == "version"
    assert " ".join(SEMVER["if"].split()) == "needs.version.outputs.should_release == 'true'"


@pytest.mark.parametrize("job", ["publish", "publish-python"])
def test_nothing_publishes_before_the_semver_job_passes(job: str) -> None:
    """A job runs only when every job it needs succeeded, so a patch bump that
    removes an item fails `semver` and neither registry sees the release."""
    assert "semver" in JOBS[job]["needs"]


def test_the_semver_job_checks_against_the_previous_release_tag() -> None:
    checkout = SEMVER["steps"][0]
    assert checkout["uses"].startswith("actions/checkout")
    assert checkout["with"] == {"ref": "${{ needs.version.outputs.sha }}", "fetch-depth": 0}
    [step] = [s for s in SEMVER["steps"] if "run" in s]
    assert step["env"] == {"SHA": "${{ needs.version.outputs.sha }}"}
    assert step["run"].split("\n")[:2] == [
        'BASELINE=$(python3 scripts/publish_order.py previous-tag "$SHA")',
        'python3 scripts/api_removals.py release --baseline "$BASELINE"',
    ]


def test_the_semver_job_only_reads() -> None:
    assert SEMVER["permissions"] == {"contents": "read"}
    assert "secrets." not in yaml.safe_dump(SEMVER)


# --- the claim ---------------------------------------------------------------

CLAIM = JOBS["claim"]
BUILDS = {"build-cli", "build-python", "build-python-sdist"}


def releasing(claimed: str) -> dict:
    """The context of a job after `version` decided to release and `claim` answered."""
    return {"github": {"repository": THIS_REPO},
            "needs": {"version": {"outputs": {"should_release": "true"}},
                      "claim": {"outputs": {"claimed": claimed}}}}


def test_nothing_is_claimed_before_semver_and_every_build_pass() -> None:
    """A version that cannot ship stays untagged, so the next bump's semver
    baseline and `decide` never take it for a release."""
    assert set(CLAIM["needs"]) == {"version", "semver"} | BUILDS
    assert evaluate(CLAIM["if"], releasing(""))


@pytest.mark.parametrize("job", ["publish", "publish-python", "release"])
def test_only_the_run_that_claimed_the_version_publishes(job: str) -> None:
    """`decide` runs at each run's start, unserialised, so two pushes to main while a
    version is untagged both read "release" (0.44.0, 2026-11-01). Only the run whose
    claim pushed the tag, or found it at its own commit, goes on."""
    assert "claim" in JOBS[job]["needs"]
    assert evaluate(JOBS[job]["if"], releasing("true"))
    assert not evaluate(JOBS[job]["if"], releasing("false"))


def test_the_claim_reports_its_answer() -> None:
    assert CLAIM["outputs"] == {"claimed": "${{ steps.claim.outputs.claimed }}"}
    [step] = [s for s in CLAIM["steps"] if "publish_order.py claim" in s.get("run", "")]
    assert step["id"] == "claim"
    assert step["env"] == {"VERSION": "${{ needs.version.outputs.version }}",
                           "SHA": "${{ needs.version.outputs.sha }}"}
    assert CLAIM["permissions"] == {"contents": "write"}


def test_only_the_claim_pushes_a_tag() -> None:
    """A second tag step would race the claim it replaces."""
    for name, job in JOBS.items():
        if name == "claim":
            continue
        for step in job.get("steps", []):
            assert "git tag" not in step.get("run", ""), (name, step.get("name"))
            assert "git push" not in step.get("run", ""), (name, step.get("name"))


def test_the_claim_checks_the_workspace_can_publish_before_tagging() -> None:
    """After the tag, no other commit may finish the version, so the publish loop's
    refusals that no retry fixes (exit 3 and 4) must come first."""
    runs = [s.get("run", "") for s in CLAIM["steps"]]
    preflight = runs.index("python3 scripts/publish_order.py list --max-new-names 5 > /dev/null")
    [claiming] = [i for i, r in enumerate(runs) if "publish_order.py claim" in r]
    assert preflight < claiming


@pytest.mark.parametrize(("answer", "claimed"), [("release", "true"), ("skip", "false")])
def test_the_claim_step_reports_what_claim_printed(
        tmp_path: Path, answer: str, claimed: str) -> None:
    """Run the step's script with `python3` and `git` stubbed: the lock is only as
    good as the line that turns `claim`'s answer into `claimed`."""
    [step] = [s for s in CLAIM["steps"] if s.get("id") == "claim"]
    stubs = tmp_path / "bin"
    stubs.mkdir()
    for name, body in [("python3", f'echo "$@" >> "{tmp_path}/args"\necho {answer}'),
                       ("git", "exit 0")]:
        stub = stubs / name
        stub.write_text(f"#!/bin/sh\n{body}\n")
        stub.chmod(0o755)
    output = tmp_path / "output"
    env = {"PATH": f"{stubs}{os.pathsep}{os.environ['PATH']}", "GITHUB_OUTPUT": str(output),
           "VERSION": "1.0.0", "SHA": "abc"}
    subprocess.run([shutil.which("bash"), "-e", "-c", step["run"]], env=env, check=True)
    assert output.read_text() == f"claimed={claimed}\n"
    assert (tmp_path / "args").read_text() == "scripts/publish_order.py claim 1.0.0 --head-sha abc\n"


RELEASE_STEPS = JOBS["release"]["steps"]


def test_the_release_notes_are_for_the_tag_at_this_commit() -> None:
    """The claim pushes a tag before publishing, so a newer version's tag can exist
    by the time this release writes its notes."""
    [notes] = [s for s in RELEASE_STEPS if s.get("id") == "changelog"]
    assert notes["with"]["args"] == "--current --strip header"


def test_the_github_release_targets_this_commit() -> None:
    [step] = [s for s in RELEASE_STEPS if s.get("uses", "").startswith("softprops/")]
    assert step["with"]["target_commitish"] == "${{ needs.version.outputs.sha }}"
