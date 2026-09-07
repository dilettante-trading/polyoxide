"""Unit tests for classify_failures.py."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

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


def test_classify_auth_gated_private_key_variant() -> None:
    """live_ws.rs derives L2 credentials from the private key alone, so its
    auth panic names one concrete var instead of the `POLYMARKET_*` glob."""
    text = (
        "thread 'live_user_subscription_accepts_omitted_markets' panicked at "
        "polyoxide-clob/tests/live_ws.rs:111:10:\n"
        "POLYMARKET_PRIVATE_KEY required; the L2 triple is derived from it"
    )
    assert classify(text) == Verdict.AUTH_GATED


def test_classify_auth_gated_takes_precedence_over_transient() -> None:
    """If both patterns match, AUTH_GATED wins (defensive ordering)."""
    text = "POLYMARKET_* env vars required for authenticated tests: HTTP 503"
    assert classify(text) == Verdict.AUTH_GATED


def test_classify_transient_reqwest_debug_timeout() -> None:
    """A panic renders its error with `Debug`, not `Display`.

    Verbatim from the nightly run that filed issue #32. reqwest's timeout
    marker is a unit struct: `Display` writes "operation timed out" (which the
    table already matched), but the derived `Debug` writes the bare token
    `TimedOut`. `.expect()` formats with `{:?}`, so only the second one ever
    reaches this classifier.
    """
    text = (
        "thread 'live_last_trade_price' (7718) panicked at "
        "polyoxide-clob/tests/live_api.rs:620:10:\n"
        "last_trade_price should succeed: Api(Network(reqwest::Error "
        '{ kind: Request, url: "https://clob.polymarket.com/last-trade-price'
        '?token_id=3233822019007135143577280177972530224457577521641332595144'
        '3816017994629993401", source: TimedOut }))'
    )
    assert classify(text) == Verdict.TRANSIENT


def test_classify_transient_reqwest_debug_connect_error() -> None:
    """The same Debug-vs-Display gap for a failed connect.

    hyper-util labels the source "tcp connect error" and reqwest's own
    `Display` for a request-phase failure is "error sending request"; neither
    appeared in the table, so a connect failure whose OS message was not
    literally "Connection refused" (a TLS handshake abort, an unreachable
    host) classified as REAL.
    """
    text = (
        "thread 'live_markets' panicked at polyoxide-gamma/tests/live_api.rs:42:10:\n"
        "markets should succeed: Api(Network(reqwest::Error { kind: Request, "
        'url: "https://gamma-api.polymarket.com/markets", source: '
        "hyper_util::client::legacy::Error(Connect, ConnectError("
        '"tcp connect error", Os { code: 101, kind: NetworkUnreachable, '
        'message: "Network is unreachable" })) }))'
    )
    assert classify(text) == Verdict.TRANSIENT


def test_classify_real_is_not_broadened_by_the_debug_patterns() -> None:
    """The Debug patterns must not swallow genuine contract failures.

    Issue #32 also carried two real ones — a venue validation error reached
    through the same `Api(...)` wrapper. Matching on the wrapper, or on a bare
    `Request`/`Connect` token, would have hidden them.
    """
    text = (
        "thread 'live_holders' (4811) panicked at "
        "polyoxide-data/tests/live_api.rs:209:10:\n"
        "holders should deserialize: Api(Validation("
        "\"required query param 'market' not provided\"))"
    )
    assert classify(text) == Verdict.REAL


# Every arm `ApiError::is_retriable()` answers `true` for, in both of the
# renderings a panic can carry. `.expect()` formats with `{:?}`, and
# `panic!("...: {e}")` with `{}`; the table has to match either, and for the
# `Network` arms the two share no words at all.
#
# Keep this in step with `is_retriable` in polyoxide-core/src/error.rs. A row
# that only proves "some pattern exists" is worth little — each of these is the
# text a real panic produces, so a pattern that stops matching fails here.
RETRIABLE_ARMS: list[tuple[str, str]] = [
    # (label, panic text)
    ("Api 5xx / Debug", 'live_x: Api { status: 503, message: "bad gateway" }'),
    ("Api 5xx / Display", "live_x: API error: 503 - bad gateway"),
    ("Api 425 / Debug", 'live_x: Api { status: 425, message: "too early" }'),
    ("Api 425 / Display", "live_x: API error: 425 - too early"),
    ("RateLimit / Debug", 'live_x: RateLimit("slow down")'),
    ("RateLimit / Display", "live_x: Rate limit exceeded: slow down"),
    ("Timeout / Debug", "live_x: Api(Timeout)"),
    ("Timeout / Display", "live_x: Request timeout"),
    (
        "Network is_timeout / Debug",
        'live_x: Network(reqwest::Error { kind: Request, '
        'url: "https://clob.polymarket.com/ok", source: TimedOut })',
    ),
    (
        "Network is_timeout / Display",
        "live_x: Network error: error sending request for url "
        "(https://clob.polymarket.com/ok)",
    ),
    (
        "Network is_connect / Debug",
        'live_x: Network(reqwest::Error { kind: Request, '
        'url: "https://clob.polymarket.com/ok", source: '
        "hyper_util::client::legacy::Error(Connect, ConnectError("
        '"tcp connect error", Os { code: 111, kind: ConnectionRefused, '
        'message: "Connection refused" })) })',
    ),
    (
        "Network is_connect / Display",
        "live_x: Network error: error sending request for url "
        "(https://clob.polymarket.com/ok)",
    ),
]


@pytest.mark.parametrize("label,text", RETRIABLE_ARMS, ids=[a[0] for a in RETRIABLE_ARMS])
def test_every_retriable_arm_classifies_transient(label: str, text: str) -> None:
    assert classify(text) == Verdict.TRANSIENT, f"{label} fell through to REAL"


# The mirror image: arms `is_retriable()` answers `false` for must stay REAL,
# so widening the table for issue #32 cannot quietly start skipping defects.
NON_RETRIABLE_ARMS: list[tuple[str, str]] = [
    ("Validation / Debug", "live_x: Validation(\"required query param 'market' not provided\")"),
    ("Validation / Display", "live_x: Validation error: bad request"),
    ("Authentication / Debug", 'live_x: Authentication("invalid signature")'),
    ("Api 4xx / Debug", 'live_x: Api { status: 404, message: "not found" }'),
    ("Api 4xx / Display", "live_x: API error: 404 - not found"),
    ("Serialization / Display", "live_x: Serialization error: invalid type at line 1"),
    ("plain assertion", "assertion `left == right` failed\n  left: 3\n right: 4"),
]


@pytest.mark.parametrize(
    "label,text", NON_RETRIABLE_ARMS, ids=[a[0] for a in NON_RETRIABLE_ARMS]
)
def test_non_retriable_arms_stay_real(label: str, text: str) -> None:
    assert classify(text) == Verdict.REAL, f"{label} was wrongly skipped as transient"


def test_classify_environmental_sports_timeout() -> None:
    """live_sports_channel_yields_frames documents that it can legitimately
    time out when no matches are live anywhere; that is a fact about the
    world, not the SDK, so it must not be reported as a real failure."""
    text = (
        "thread 'live_sports_channel_yields_frames' panicked at "
        "polyoxide-clob/tests/live_ws.rs:35:10:\n"
        "sports channel should push a frame within the window; if no matches "
        "are live anywhere this can legitimately time out, so re-run before "
        "concluding a defect: Elapsed(())"
    )
    assert classify(text) == Verdict.ENVIRONMENTAL


def test_classify_environmental_no_qualifying_market() -> None:
    """The order-placing tests need a market whose book satisfies a price
    precondition. When the open listing offers none, the helper refuses to
    post rather than spend money on a crossing order. That is a fact about
    today's markets, not the SDK."""
    text = (
        "thread 'live_fak_unmatched_is_typed_error' panicked at "
        "polyoxide-clob/tests/live_api.rs:47:5:\n"
        "no qualifying market with a best ask above 0.05 in the 100 open "
        "markets gamma lists; market conditions rather than a defect, "
        "so re-run before concluding otherwise"
    )
    assert classify(text) == Verdict.ENVIRONMENTAL


def test_classify_environmental_no_suitable_market() -> None:
    """The in-test guard fires when gamma's cached best_ask and the CLOB book
    disagree at the threshold. Same category: market state, not a defect."""
    text = (
        "thread 'live_fak_unmatched_is_typed_error' panicked at "
        "polyoxide-clob/tests/live_api.rs:1062:5:\n"
        "no suitable market: best ask 0.042 is too cheap for a safe "
        "non-crossing test; market conditions rather than a defect"
    )
    assert classify(text) == Verdict.ENVIRONMENTAL


def test_classify_bare_market_word_is_real() -> None:
    """The environmental pattern must require the `no qualifying/suitable
    market` phrasing, not merely the word `market` — otherwise most CLOB
    failures would be silently skipped."""
    text = "market_by_token returned the wrong condition_id for the market"
    assert classify(text) == Verdict.REAL


def test_classify_empty_string_is_real() -> None:
    """No information defaults to REAL — better to false-positive than skip."""
    assert classify("") == Verdict.REAL


def test_classify_unrelated_5xx_substring_does_not_match() -> None:
    """The pattern requires `HTTP 5xx` or `status: 5xx`, not arbitrary 5xx."""
    assert classify("computed value 500 differs from expected 600") == Verdict.REAL


def test_parse_mixed_fixture() -> None:
    outcomes = parse_nextest_json(FIXTURES / "nextest-mixed.json")
    by_name = {o.name: o for o in outcomes}
    assert by_name["polyoxide-gamma::live_api$live_list_markets"].verdict == Verdict.PASS
    assert by_name["polyoxide-gamma::live_api$live_get_market"].verdict == Verdict.REAL
    assert by_name["polyoxide-gamma::live_api$live_search_markets"].verdict == Verdict.TRANSIENT
    assert by_name["polyoxide-clob::live_api$live_create_order"].verdict == Verdict.AUTH_GATED
    assert by_name["polyoxide-clob::live_api$live_get_order"].verdict == Verdict.TRANSIENT
    assert by_name["polyoxide-clob::live_ws$live_sports_frames"].verdict == Verdict.ENVIRONMENTAL
    assert len(outcomes) == 6


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
    environmental = (out_dir / "environmental.txt").read_text().splitlines()
    report = (out_dir / "report.md").read_text()

    # mixed.json has: 1 pass, 1 real, 1 transient (429), 1 auth-gated,
    # 1 transient (DNS), 1 environmental (sports timeout)
    assert sorted(retry) == sorted([
        "polyoxide-gamma::live_api$live_search_markets",
        "polyoxide-clob::live_api$live_get_order",
    ])
    assert real == ["polyoxide-gamma::live_api$live_get_market"]
    assert auth == ["polyoxide-clob::live_api$live_create_order"]
    assert environmental == ["polyoxide-clob::live_ws$live_sports_frames"]
    assert "live_get_market" in report
    assert "## Real failures" in report


def test_cli_classify_writes_retry_filterset(tmp_path: Path) -> None:
    """nextest's libtest-json names are `crate::binary$test`, which the
    `test(=...)` filterset predicate cannot match. The classifier must emit a
    ready-made filterset that pins both the binary and the bare test name."""
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "classify",
            "--input", str(FIXTURES / "nextest-mixed.json"),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    filterset = (out_dir / "retry-filter.txt").read_text().strip()
    assert filterset == (
        "(binary_id(=polyoxide-gamma::live_api) & test(=live_search_markets))"
        " | (binary_id(=polyoxide-clob::live_api) & test(=live_get_order))"
    )


def test_cli_classify_writes_empty_filterset_when_nothing_transient(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "classify",
            "--input", str(FIXTURES / "nextest-real-failure.json"),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert (out_dir / "retry-filter.txt").read_text() == ""


def test_cli_merge_promotes_persistent_transients_to_real(tmp_path: Path) -> None:
    """A test that was transient on first pass and still failing on retry
    becomes a REAL failure in the merged report."""
    # Reuse mixed.json as first-pass: live_search_markets is TRANSIENT.
    # Use a synthetic retry that has live_search_markets STILL failing transient.
    retry_file = tmp_path / "retry.json"
    retry_file.write_text(
        '{"type":"suite","event":"started","test_count":2}\n'
        '{"type":"test","event":"started","name":"polyoxide-gamma::live_api$live_search_markets"}\n'
        '{"type":"test","name":"polyoxide-gamma::live_api$live_search_markets","event":"failed","stdout":"thread panic: HTTP 429"}\n'
        '{"type":"test","event":"started","name":"polyoxide-clob::live_api$live_get_order"}\n'
        '{"type":"test","name":"polyoxide-clob::live_api$live_get_order","event":"ok"}\n'
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
    # live_sports_frames was ENVIRONMENTAL on first pass — logged, never REAL.
    assert sorted(real) == sorted([
        "polyoxide-gamma::live_api$live_get_market",
        "polyoxide-gamma::live_api$live_search_markets",
    ])
    assert (out_dir / "retry-tests.txt").read_text() == ""
    environmental = (out_dir / "environmental.txt").read_text().splitlines()
    assert environmental == ["polyoxide-clob::live_ws$live_sports_frames"]
