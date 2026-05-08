"""Unit tests for classify_failures.py."""

from __future__ import annotations

import json
from pathlib import Path

from classify_failures import TestOutcome, Verdict, classify, parse_nextest_json

FIXTURES = Path(__file__).parent / "fixtures"


def _load_fixture_failure_text(name: str) -> str:
    """Read the first failed-event's stdout from an NDJSON fixture."""
    path = FIXTURES / name
    with path.open() as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            event = json.loads(line)
            if event.get("type") == "test" and event.get("event") == "failed":
                return event.get("stdout", "") + event.get("stderr", "")
    raise AssertionError(f"no failed-event found in {name}")


def test_classify_real_assertion_failure() -> None:
    text = _load_fixture_failure_text("nextest-real-failure.json")
    assert classify(text) == Verdict.REAL


def test_classify_transient_429() -> None:
    text = _load_fixture_failure_text("nextest-transient-429.json")
    assert classify(text) == Verdict.TRANSIENT


def test_classify_transient_503() -> None:
    text = _load_fixture_failure_text("nextest-transient-503.json")
    assert classify(text) == Verdict.TRANSIENT


def test_classify_transient_connection_refused() -> None:
    text = _load_fixture_failure_text("nextest-transient-connection.json")
    assert classify(text) == Verdict.TRANSIENT


def test_classify_auth_gated() -> None:
    text = _load_fixture_failure_text("nextest-auth-gated.json")
    assert classify(text) == Verdict.AUTH_GATED


def test_classify_auth_gated_takes_precedence_over_transient() -> None:
    """If both patterns match, AUTH_GATED wins (defensive ordering)."""
    text = "POLYMARKET_* env vars required for authenticated tests: HTTP 503"
    assert classify(text) == Verdict.AUTH_GATED


def test_classify_empty_string_is_real() -> None:
    """No information defaults to REAL — better to false-positive than skip."""
    assert classify("") == Verdict.REAL


def test_classify_unrelated_5xx_substring_does_not_match() -> None:
    """The pattern requires `HTTP 5xx` or `status: 5xx`, not arbitrary 5xx."""
    assert classify("computed value 500 differs from expected 600") == Verdict.REAL


def test_parse_mixed_fixture() -> None:
    outcomes = parse_nextest_json(FIXTURES / "nextest-mixed.json")
    by_name = {o.name: o for o in outcomes}
    assert by_name["live_list_markets"].verdict == Verdict.PASS
    assert by_name["live_get_market"].verdict == Verdict.REAL
    assert by_name["live_search_markets"].verdict == Verdict.TRANSIENT
    assert by_name["live_create_order"].verdict == Verdict.AUTH_GATED
    assert by_name["live_get_order"].verdict == Verdict.TRANSIENT
    assert len(outcomes) == 5
