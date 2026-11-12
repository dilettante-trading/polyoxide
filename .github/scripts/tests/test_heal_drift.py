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
