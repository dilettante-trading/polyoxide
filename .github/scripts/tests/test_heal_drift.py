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
