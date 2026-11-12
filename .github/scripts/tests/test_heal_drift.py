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
