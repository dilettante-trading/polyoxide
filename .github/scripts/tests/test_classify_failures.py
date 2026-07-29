"""Unit tests for classify_failures.py."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from classify_failures import Verdict, classify, parse_nextest_json

FIXTURES = Path(__file__).parent / "fixtures"
SCRIPT = Path(__file__).parent.parent / "classify_failures.py"


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


def test_cli_classify_writes_outputs(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "classify",
            "--input", str(FIXTURES / "nextest-mixed.json"),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.returncode == 0

    retry = (out_dir / "retry-tests.txt").read_text().splitlines()
    real = (out_dir / "real-failures.txt").read_text().splitlines()
    auth = (out_dir / "auth-gated.txt").read_text().splitlines()
    report = (out_dir / "report.md").read_text()

    # mixed.json has: 1 pass, 1 real, 1 transient (429), 1 auth-gated, 1 transient (DNS)
    assert sorted(retry) == sorted(["live_search_markets", "live_get_order"])
    assert real == ["live_get_market"]
    assert auth == ["live_create_order"]
    assert "live_get_market" in report
    assert "## Real failures" in report


def test_cli_merge_promotes_persistent_transients_to_real(tmp_path: Path) -> None:
    """A test that was transient on first pass and still failing on retry
    becomes a REAL failure in the merged report."""
    # Reuse mixed.json as first-pass: live_search_markets is TRANSIENT.
    # Use a synthetic retry that has live_search_markets STILL failing transient.
    retry_file = tmp_path / "retry.json"
    retry_file.write_text(
        '{"type":"suite","event":"started","test_count":2}\n'
        '{"type":"test","event":"started","name":"live_search_markets"}\n'
        '{"type":"test","name":"live_search_markets","event":"failed","stdout":"thread panic: HTTP 429"}\n'
        '{"type":"test","event":"started","name":"live_get_order"}\n'
        '{"type":"test","name":"live_get_order","event":"ok"}\n'
        '{"type":"suite","event":"failed","passed":1,"failed":1,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.5}\n'
    )

    out_dir = tmp_path / "out"
    out_dir.mkdir()
    result = subprocess.run(
        [
            sys.executable, str(SCRIPT), "merge",
            "--first-pass", str(FIXTURES / "nextest-mixed.json"),
            "--retry", str(retry_file),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert result.returncode == 0

    real = (out_dir / "real-failures.txt").read_text().splitlines()
    # live_get_market was REAL on first pass — stays REAL.
    # live_search_markets was TRANSIENT on first pass, still failing on retry — promoted to REAL.
    # live_get_order was TRANSIENT on first pass, passed on retry — drops to PASS.
    assert sorted(real) == sorted(["live_get_market", "live_search_markets"])
    assert (out_dir / "retry-tests.txt").read_text() == ""
