"""The sports capture script's cadence logic (spec D9, D17), offline.

`scripts/capture_polymarket_sports.py` lives outside this uv project, so it is
loaded by path. Nothing here opens a socket or reads a clock: `PingPlan` takes
the time as an argument and the rest are pure functions over lists.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

pytest.importorskip("websockets")  # the script imports it; the dev group has it

REPO = Path(__file__).resolve().parents[3]


def _load(name: str):
    """A module of `scripts/`, which is not on this project's path."""
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


# The script does `import capture_common`, so that module goes first.
_load("capture_common")
cap = _load("capture_polymarket_sports")


def _every_30s(seconds: int = 1800) -> list[float]:
    return [float(t) for t in range(5, seconds, 30)]


@pytest.fixture
def fresh_capture():
    """An empty `CAPTURE`, put back afterwards (the script keeps it global)."""
    saved = cap.CAPTURE
    cap.CAPTURE = cap.Capture()
    yield cap.CAPTURE
    cap.CAPTURE = saved


# --- the ping budget ---------------------------------------------------------


@pytest.mark.parametrize(
    "seconds, cadence, budget",
    [
        (1800, 600.0, 2),  # minutes 10 and 20, not a third at 30 (D17)
        (600, 600.0, 0),  # the ping would fall at the end
        (601, 600.0, 1),
        (3.5, 1.0, 3),
        (2.5, 1.0, 2),
        (1.5, 10.0, 0),  # the cadence is longer than the capture
    ],
)
def test_the_budget_is_what_an_undropped_run_sends(seconds, cadence, budget):
    assert cap.PingPlan(0.0, seconds, cadence).left == budget


def test_a_drop_shifts_the_next_ping_by_the_time_lost():
    plan = cap.PingPlan(0.0, 1800, 600.0)
    end = 1800.0
    assert plan.wait(0.0, end) == 600.0
    plan.lose(90.0)  # a drop that cost 90 s moves the end out by it too
    end += 90.0
    assert plan.next_ping == 690.0
    assert not plan.due(689.0) and plan.due(690.0)
    plan.sent()
    assert plan.next_ping == 1290.0 and plan.left == 1
    assert plan.due(1290.0)
    plan.sent()
    assert plan.left == 0


def test_a_spent_plan_waits_to_the_end_instead_of_spinning():
    plan = cap.PingPlan(0.0, 1800, 600.0)
    plan.sent()
    plan.sent()
    assert plan.left == 0
    # The next ping would be due already, yet nothing is left to send: reading
    # must run to the end, not time out at once again and again.
    assert not plan.due(1890.0)
    assert plan.wait(1291.0, 599.0) == 599.0


def test_an_unspent_plan_waits_for_the_next_ping_and_never_past_the_end():
    plan = cap.PingPlan(0.0, 1800, 600.0)
    assert plan.wait(100.0, 1700.0) == 500.0
    assert plan.wait(100.0, 200.0) == 200.0
    assert plan.wait(700.0, 1100.0) == 0.0  # overdue: send now


# --- a game live throughout --------------------------------------------------


def test_data_in_every_minute_is_live_throughout():
    assert cap.live_throughout(_every_30s(), [(0.0, 1800.0)])


def test_a_minute_with_no_data_is_not_live_throughout():
    gap = [t for t in _every_30s() if not 600 <= t < 660]
    assert not cap.live_throughout(gap, [(0.0, 1800.0)])


def test_no_data_at_all_is_not_live_throughout():
    assert not cap.live_throughout([], [(0.0, 1800.0)])


def test_a_capture_under_a_minute_is_never_live_throughout():
    assert not cap.live_throughout([1.0, 2.0], [(0.0, 59.0)])
    assert not cap.live_throughout([], [(0.0, 59.0)])


def test_minutes_are_counted_inside_each_connected_span():
    segments = [(0.0, 120.0), (200.0, 215.0)]  # the second span holds no whole minute
    assert cap.live_throughout([10.0, 70.0], segments)
    assert not cap.live_throughout([10.0], segments), "the second minute is empty"


def test_a_gap_shorter_than_a_minute_does_not_matter():
    # 10.6 s was the longest gap measured on 2026-11-02.
    assert cap.live_throughout([0.0, 10.6, 58.0, 70.0, 100.0], [(0.0, 120.0)])


# --- the summary and the exit code ------------------------------------------


def _live_capture(capture) -> None:
    capture.segments = [(0.0, 1800.0)]
    capture.frames = [(t, "{}") for t in _every_30s()]
    capture.sent = [600.01, 1200.01]


def test_a_live_capture_names_no_rule_and_stops_for_the_owner(fresh_capture):
    _live_capture(fresh_capture)
    text = "\n".join(cap.cadence_summary(1800, 600.0))
    assert "game live throughout: YES" in text
    assert "D9 sports rule: none" in text
    assert "report to the owner (D17)" in text
    assert "D9 sports rule: A" not in text and "D9 sports rule: C" not in text


def test_a_quiet_capture_with_unanswered_pings_is_rule_c(fresh_capture):
    fresh_capture.segments = [(0.0, 1800.0)]
    fresh_capture.frames = []
    fresh_capture.sent = [600.01, 1200.01]
    text = "\n".join(cap.cadence_summary(1800, 600.0))
    assert "game live throughout: no" in text
    assert "D9 sports rule: C" in text
    assert "none" not in text.split("D9 sports rule:")[1]


def test_a_game_live_throughout_exits_3(fresh_capture):
    _live_capture(fresh_capture)
    assert cap.cadence_exit_code() == 3


def test_a_second_drop_exits_2_even_with_a_game_live(fresh_capture):
    _live_capture(fresh_capture)
    fresh_capture.drops = [(300.0, "first"), (900.0, "second")]
    assert cap.cadence_exit_code() == 2
    assert "stopped: a second drop" in "\n".join(cap.cadence_summary(1800, 600.0))


def test_a_quiet_capture_that_reached_a_rule_exits_0(fresh_capture):
    fresh_capture.segments = [(0.0, 1800.0)]
    fresh_capture.drops = [(300.0, "one drop is survived")]
    assert cap.cadence_exit_code() == 0
