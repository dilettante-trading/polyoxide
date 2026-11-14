"""scripts/heal_drift.py: the loom watcher's hook, the publish guard, and loom-watch.json.

docs/superpowers/specs/2026-11-12-drift-heal-loop-design.md is the design. The fixtures under
fixtures/heal/ are captured `gh` and `loom session list --json` output, so a field renamed by
either fails here rather than in a heal. The hook's input is loom's wire (docs/specs/watch.md in
loom, "The hook"), built here from the captured issue.
"""

from __future__ import annotations

import copy
import importlib.util
import json
import re
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
SESSION = fixture("sessions.json")[0]
SHA_96 = "7a09009f7ccff02c68ebab5b80141459b655586bb68f8bdf293fedc25b428464"
OTHER_SHA = "0" * 64
THIRD_SHA = "1" * 64
BRANCH = "aidanb/heal-kalshi-ws-96-r32"
TITLE_96 = "heal kalshi-ws 96 R32"
TABLE = "| R28 | the reconnect jitter |\n| R31 | perps' new route |\n"
CONFIG = json.loads((REPO / "loom-watch.json").read_text())
# The variables loom gives every prompt (watch.md, "Prompts"); a hook variable may not reuse one.
PROMPT_VARS = {"issue", "issue_title", "issue_url", "pr", "pr_url", "branch", "base"}


def issue(number=96, *, state="OPEN", labels=(), body=None):
    d = copy.deepcopy(ISSUE_96)
    d["number"], d["state"] = number, state
    d["labels"] += [{**d["labels"][0], "name": name} for name in labels]
    if body is not None:
        d["body"] = body
    return heal.Issue.from_gh(d)


def session(issue=96, *, title=TITLE_96, branch=BRANCH, worktree="/nowhere"):
    return heal.Session.from_loom({**SESSION, "title": title, "issue": issue, "branch": branch,
                                   "worktree": worktree})


def marked(issue=96, *, sha=SHA_96, seen=SHA_96, row="R32"):
    """A heal PR's body as publish writes it."""
    marker = heal.Marker(issue, "kalshi-ws", sha, seen, row).render()
    return f"Adopted.\n\nCloses #{issue}\n\n{marker}\n"


def watched_pr(number=200, *, issue=96, branch=BRANCH, body=None):
    return {"number": number, "issue": issue, "branch": branch,
            "body": marked(issue) if body is None else body}


def watched_session(issue, row, spec="kalshi"):
    return {"title": f"heal {spec} {issue} {row}", "issue": issue,
            "branch": f"aidanb/heal-{spec}-{issue}-{row.lower()}", "status": "Running",
            "vars": {"spec": spec, "row": row}}


def hook_input(point="start", *, body=None, labels=None, pr=None, sessions=(), prs=()):
    """What the watcher writes to the hook's stdin, for issue #96."""
    names = [label["name"] for label in ISSUE_96["labels"]] if labels is None else list(labels)
    data = {
        "point": point, "workspace": "polyoxide", "repo": "dilettante-trading/polyoxide",
        "base": "main", "base_sha": "e" * 40,
        "issue": {"number": 96, "title": "Schema drift: kalshi-ws", "labels": names,
                  "body": ISSUE_96["body"] if body is None else body,
                  "url": "https://github.com/dilettante-trading/polyoxide/issues/96"},
        "pr": pr, "sessions": list(sessions), "prs": list(prs)}
    if point == "nudges":
        data["session"] = watched_session(96, "R32", "kalshi-ws")
        data["head_sha"] = "f" * 40
    return data


def start(table=TABLE, **kwargs):
    return heal.hook_start(hook_input(**kwargs), table)


def nudges(**kwargs):
    return heal.hook_nudges(hook_input("nudges", **kwargs))["nudges"]


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
    marker = heal.Marker(96, "kalshi-ws", SHA_96, OTHER_SHA, "R32")
    assert heal.Marker.parse(f"Adopted.\n\n{marker.render()}\n") == marker


def test_a_body_with_two_markers_has_none():
    text = heal.Marker(96, "kalshi-ws", SHA_96, SHA_96, "R32").render()
    assert heal.Marker.parse(f"{text}\n{text}") is None


def test_a_heal_session_is_titled_by_the_watcher_and_linked_to_its_issue():
    assert session().heal and session().row == "R32"
    assert not session(issue=94).heal
    assert not session(title="kalshi-ws adoption").heal
    assert session(title="heal kalshi-ws 96 none").row == "none"


def test_the_highest_drift_row_reads_the_real_table():
    assert heal.highest_row((REPO / heal.DIVERGENCES).read_text()) >= 31
    assert heal.highest_row(TABLE) == 31


@pytest.mark.parametrize("branch,row", [
    (BRANCH, "R32"), ("aidanb/heal-kalshi-ws-96-none", "none"), ("aidanb/heal-kalshi-ws-96", None),
    ("aidanb/heal-kalshi-196-r32", None), ("aidanb/gamma-59", None),
], ids=["row", "none", "no row", "another issue", "not a heal"])
def test_a_branch_gives_back_the_row_its_title_reserved(branch, row):
    assert heal.branch_row(branch, 96) == row


# --- the hook's start (watch.md, "start") ---------------------------------------------------


def test_start_reserves_the_row_after_main_s_table():
    assert start() == {"start": True,
                       "vars": {"spec": "kalshi-ws", "row": "R32", "fingerprint": SHA_96}}


HOLDERS = {
    # #94's session holds R35 by its title.
    "a session's title": dict(sessions=[watched_session(94, "R35")]),
    # #95's PR holds R35 by its marker (its branch says r33).
    "a pr's marker": dict(prs=[watched_pr(201, issue=95, branch="aidanb/heal-kalshi-95-r33",
                                          body=marked(95, row="R35"))]),
    # #97's acknowledgement PR holds R35 by its branch alone, since its marker says none.
    "a pr's branch": dict(prs=[watched_pr(202, issue=97, branch="aidanb/heal-kalshi-97-r35",
                                          body=marked(97, row="none"))]),
}


@pytest.mark.parametrize("held", HOLDERS.values(), ids=HOLDERS.keys())
def test_start_skips_a_row_a_heal_session_or_an_open_heal_pr_holds(held):
    assert start(**held)["vars"]["row"] == "R36"


@pytest.mark.parametrize("body", [
    "no acknowledgement block",
    f'"diff_sha256": "{SHA_96}"\n"diff_sha256": "{OTHER_SHA}"',
], ids=["none", "two"])
def test_start_refuses_an_issue_without_exactly_one_fingerprint(body):
    found = start(body=body)
    assert found["start"] is False and "diff_sha256" in found["reason"]


@pytest.mark.parametrize("labels", [
    ["schema-drift"], ["schema-drift", "spec:kalshi", "spec:kalshi-ws"], ["spec:kalshi ws"],
], ids=["none", "two", "not a title value"])
def test_start_refuses_an_issue_without_one_spec_label_a_title_can_hold(labels):
    found = start(labels=labels)
    assert found["start"] is False and "spec:" in found["reason"]


def test_a_resume_keeps_the_row_its_pr_s_branch_encodes():
    # An acknowledgement PR's marker says none; the branch still carries R30, and the watcher
    # resumes only when the title it renders makes that branch again.
    pr = watched_pr(branch="aidanb/heal-kalshi-ws-96-r30", body=marked(row="none"))
    assert start(table="| R40 | later |\n", pr=pr, prs=[pr])["vars"] == {
        "spec": "kalshi-ws", "row": "R30", "fingerprint": SHA_96}


def test_a_resume_refuses_a_branch_that_encodes_no_row():
    found = start(pr=watched_pr(branch="aidanb/kalshi-ws-by-hand"))
    assert found["start"] is False and "encodes no DRIFT row" in found["reason"]


def test_start_answers_within_the_watcher_s_bounds():
    found = start()
    assert set(found) == {"start", "vars"} and len(found["vars"]) <= 16
    for name, value in found["vars"].items():
        assert re.fullmatch(r"[a-z][a-z0-9_]*", name) and name not in PROMPT_VARS
        assert len(value.encode()) <= 64
    title_vars = set(re.findall(r"\{([a-z][a-z0-9_]*)\}", CONFIG["title"])) - {"issue"}
    for name in title_vars:
        assert heal.TITLE_VALUE.match(found["vars"][name])
    # The title the watcher renders is one publish recognises as this issue's heal session.
    title = CONFIG["title"].replace("{issue}", "96").format(**found["vars"])
    assert session(title=title).heal and session(title=title).row == "R32"


# --- the hook's nudges (watch.md, "nudges") -------------------------------------------------


def test_nudges_sends_a_redrift_when_the_nightly_wrote_a_fingerprint_the_pr_never_saw():
    [nudge] = nudges(body=f'"diff_sha256": "{THIRD_SHA}"',
                     pr=watched_pr(body=marked(sha=SHA_96, seen=OTHER_SHA)))
    assert nudge["kind"] == "redrift" and nudge["key"] == THIRD_SHA
    assert THIRD_SHA in nudge["text"] and "PR #200" in nudge["text"]


@pytest.mark.parametrize("sha,seen", [(SHA_96, OTHER_SHA), (THIRD_SHA, SHA_96)],
                         ids=["adopted it", "adopted a newer upstream than the issue shows"])
def test_nudges_holds_while_the_issue_shows_what_the_pr_adopted_or_saw(sha, seen):
    assert nudges(pr=watched_pr(body=marked(sha=sha, seen=seen))) == []


@pytest.mark.parametrize("pr", [
    watched_pr(body=PR_65["body"]), watched_pr(body=marked(95, sha=OTHER_SHA, seen=OTHER_SHA)),
], ids=["no marker", "another issue's marker"])
def test_nudges_sends_nothing_for_a_pr_without_this_issue_s_marker(pr):
    assert nudges(pr=pr) == []


# --- the hook as the watcher runs it --------------------------------------------------------


def run_hook(point, data):
    done = subprocess.run([sys.executable, str(REPO / "scripts" / "heal_drift.py"), "hook", point],
                          input=json.dumps(data), cwd=REPO, capture_output=True, text=True)
    return done.returncode, done.stdout, done.stderr


def test_the_hook_reads_stdin_and_writes_one_json_object_to_stdout():
    code, out, _ = run_hook("start", hook_input())
    assert code == 0
    answer = json.loads(out)  # one object and nothing after it, or this raises
    real = heal.highest_row((REPO / heal.DIVERGENCES).read_text())
    assert answer == {"start": True,
                      "vars": {"spec": "kalshi-ws", "row": f"R{real + 1}", "fingerprint": SHA_96}}
    code, out, _ = run_hook("nudges", hook_input("nudges", pr=watched_pr()))
    assert (code, json.loads(out)) == (0, {"nudges": []})


def test_a_hook_fault_exits_non_zero_with_nothing_on_stdout():
    code, out, err = run_hook("start", {"point": "start"})
    assert code != 0 and out == "" and "KeyError" in err


# --- loom-watch.json and its prompts ---------------------------------------------------------


def test_the_watcher_config_names_this_loop():
    assert CONFIG["version"] == 1
    assert (CONFIG["label"], CONFIG["escalate_label"]) == (heal.DRIFT_LABEL, heal.HUMAN_LABEL)
    assert CONFIG["title"] == "heal {spec} {issue} {row}"
    # The nightly rewrites the issue body each night; the hook's redrift nudge reads it instead.
    assert CONFIG["issue_changed"] is False
    assert CONFIG["hook"]["command"] == ["python3", "scripts/heal_drift.py", "hook"]
    for key in ("loom-watch.json", CONFIG["session"]["prompt"], CONFIG["session"]["resume_prompt"]):
        assert heal.guarded([key]) == [key]


@pytest.mark.parametrize("key", ["prompt", "resume_prompt"])
def test_a_prompt_names_only_variables_the_watcher_and_the_hook_give(key):
    text = (REPO / CONFIG["session"][key]).read_text()
    names = set(re.findall(r"\{([a-z][a-z0-9_]*)\}", text))
    assert names <= PROMPT_VARS | set(start()["vars"])
    assert {"issue", "spec", "row", "fingerprint"} <= names
    assert "heal_drift.py publish" in text and len(text.encode()) <= 64 * 1024


# --- publish: spec §4.1 ------------------------------------------------------------------


class FakeShell(heal.Shell):
    """Answers gh and loom from a table and records every call. git runs for real
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
                  labels=(), open_prs="[]"):
    row = {**SESSION, "title": title, "issue": session_issue, "branch": branch,
           "worktree": str(work)}
    return FakeShell({
        ("loom", "session", "list"): json.dumps([row]),
        ("gh", "issue", "view"): json.dumps({
            **ISSUE_96, "state": state,
            "labels": ISSUE_96["labels"] + [{**ISSUE_96["labels"][0], "name": n} for n in labels]}),
        ("gh", "pr", "list"): open_prs,
        ("gh", "pr", "create"): "https://github.com/dilettante-trading/polyoxide/pull/200\n",
    })


def publish(shell, work, *, sha=SHA_96, row="R32", body="Adopted the ebattle_scores channel."):
    return heal.publish(shell, issue=96, title="docs(spec): adopt kalshi-ws drift", body=body,
                        sha=sha, row=row, cwd=work)


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
    assert heal.Marker.parse(body) == heal.Marker(96, "kalshi-ws", SHA_96, SHA_96, "R32")


def test_publish_strips_a_marker_the_body_already_holds(repo):
    work, _ = repo
    shell = publish_shell(work)
    stale = heal.Marker(96, "kalshi-ws", OTHER_SHA, OTHER_SHA, "R30").render()
    publish(shell, work, body=f"Adopted.\n\n{stale}\n")
    [(_, body)] = shell.commands("gh", "pr", "create")
    assert heal.Marker.parse(body) == heal.Marker(96, "kalshi-ws", SHA_96, SHA_96, "R32")


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
    "an uncommitted change": lambda work: write(work, "docs/specs/kalshi/asyncapi.yaml", "x: 1\n"),
    "a new workflow": lambda work: commit(work, ".github/workflows/exfiltrate.yml", "on: push\n"),
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
    "an issue the owner has": dict(labels=("needs-human",)),
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
