"""Unit tests for classify_failures.py.

A failing live test prints `polyoxide-class=<tag>` just before it panics, and
that line alone decides its verdict. A log without one is `real`.

Until Story 2.7 the classifier fell back to regexes over the panic text, and
the tests below held its cases. Each case is now a row of a tag table: the tag
the migrated live test prints at that failure site (`None` where it
deliberately prints none, so the failure files), the panic text the regex-era
table matched, and the verdict the regexes gave it (`was`). Every row still
reaches `was`, except a row marked with the AD-14 rule that changed it
(`now`). The same text with no tag is `real`, which is what shows no regex is
left to read it.

A row that names an error the tests can build has a Rust `twin`, a test that
builds that error, checks it renders the row's text, and asserts it fails a
test with the row's tag. So a row's tag is the one the live test really
prints, not one chosen to make the row pass.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import pytest

from classify_failures import Verdict, classify, parse_nextest_json

FIXTURES = Path(__file__).parent / "fixtures"
SCRIPT = Path(__file__).parent.parent / "classify_failures.py"
REPO = Path(__file__).resolve().parents[3]

# The Rust twins, by the file that holds them.
CORE = "polyoxide-test-support/tests/failure_tags.rs"
BINANCE = "polyoxide-binance/tests/failure_tags.rs"
SPORTS = "polyoxide-sports/tests/failure_tags.rs"
RTDS = "polyoxide-rtds/tests/failure_tags.rs"

# What Rust's default panic hook prints after the test-support hook's tag line.
REPORT = (
    "\nthread 'live_x' (4811) panicked at polyoxide-gamma/tests/live_api.rs:42:10:\n"
    "{message}\n"
    "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n"
)


def _tagged(tag: str, message: str) -> str:
    return f"polyoxide-class={tag}\n" + REPORT.format(message=message)


@dataclass(frozen=True)
class Row:
    """One regex-era case, as the tag the live test now prints plus the text
    the old table matched."""

    label: str
    tag: str | None
    text: str
    was: Verdict
    twin: str | None = None
    # Set only where AD-14's tables give a different verdict than the regexes
    # did, with the rule that changed it.
    now: Verdict | None = None
    changed_by: str | None = None

    @property
    def verdict(self) -> Verdict:
        return self.was if self.now is None else self.now

    def log(self) -> str:
        return _tagged(self.tag, self.text) if self.tag else REPORT.format(message=self.text)


def _check(row: Row) -> None:
    """The row's log reaches its verdict, and its text alone is real."""
    assert classify(row.log()) == row.verdict, f"{row.label}: {classify(row.log())}"
    assert classify(REPORT.format(message=row.text)) == Verdict.REAL, (
        f"{row.label}: the text decided without a tag")


def _ids(rows: list[Row]) -> list[str]:
    return [row.label for row in rows]


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


def _check_fixture(name: str, verdict: Verdict) -> None:
    text = _load_fixture_failure_text(name)
    assert classify(text) == verdict
    untagged = re.sub(r"(?m)^polyoxide-class=\S+\n", "", text)
    assert classify(untagged) == Verdict.REAL, "the fixture's text decided without its tag"


def test_classify_real_assertion_failure() -> None:
    """A bare assertion prints no tag, so it files."""
    text = _load_fixture_failure_text("nextest-real-failure.json")
    assert "polyoxide-class=" not in text
    assert classify(text) == Verdict.REAL


def test_classify_transient_429() -> None:
    """Twin: `rate_limit` in polyoxide-test-support's failure_tags.rs."""
    _check_fixture("nextest-transient-429.json", Verdict.TRANSIENT)


def test_classify_transient_503() -> None:
    """Twin: `api_5xx` in polyoxide-test-support's failure_tags.rs."""
    _check_fixture("nextest-transient-503.json", Verdict.TRANSIENT)


def test_classify_transient_connection_refused() -> None:
    """Twin: `network_connect` in polyoxide-test-support's failure_tags.rs."""
    _check_fixture("nextest-transient-connection.json", Verdict.TRANSIENT)


def test_classify_auth_gated() -> None:
    """The credential loaders print `auth-gated`; polyoxide-test-support's
    tests/loaders.rs proves it in a child process."""
    _check_fixture("nextest-auth-gated.json", Verdict.AUTH_GATED)


PRIVATE_KEY_ONLY = Row(
    "private key only", "auth-gated",
    "POLYMARKET_PRIVATE_KEY required; the L2 triple is derived from it",
    Verdict.AUTH_GATED,
)


def test_classify_auth_gated_private_key_variant() -> None:
    """live_ws.rs derives L2 credentials from the private key alone, and its
    loader names only that variable. Its loader prints `auth-gated` either way."""
    _check(PRIVATE_KEY_ONLY)


AUTH_BEATS_503 = Row(
    "auth beats 503", "auth-gated",
    "POLYMARKET_* env vars required for authenticated tests: HTTP 503",
    Verdict.AUTH_GATED,
)


def test_classify_auth_gated_takes_precedence_over_transient() -> None:
    """A credential failure whose text names a 503 is still auth-gated: the tag
    decides, and no text competes with it."""
    _check(AUTH_BEATS_503)


ISSUE_32_TIMEOUT = Row(
    "issue #32 timeout", "transient",
    "last_trade_price should succeed: Api(Network(reqwest::Error "
    '{ kind: Request, url: "https://clob.polymarket.com/last-trade-price'
    '?token_id=3233822019007135143577280177972530224457577521641332595144'
    '3816017994629993401", source: TimedOut }))',
    Verdict.TRANSIENT, twin=f"{CORE}::network_timeout",
)


def test_classify_transient_reqwest_debug_timeout() -> None:
    """Verbatim from the nightly run that filed issue #32: a reqwest timeout,
    which the regexes missed because `.expect()` renders it with `Debug` as
    the bare token `TimedOut`. `or_fail` tags it from its class instead."""
    _check(ISSUE_32_TIMEOUT)


UNREACHABLE_HOST = Row(
    "unreachable host", "transient",
    "markets should succeed: Api(Network(reqwest::Error { kind: Request, "
    'url: "https://gamma-api.polymarket.com/markets", source: '
    "hyper_util::client::legacy::Error(Connect, ConnectError("
    '"tcp connect error", Os { code: 101, kind: NetworkUnreachable, '
    'message: "Network is unreachable" })) }))',
    Verdict.TRANSIENT, twin=f"{CORE}::network_connect",
)


def test_classify_transient_reqwest_debug_connect_error() -> None:
    """The same Debug-vs-Display gap for a failed connect."""
    _check(UNREACHABLE_HOST)


ISSUE_32_VALIDATION = Row(
    "issue #32 validation", "real",
    "holders should deserialize: Api(Validation("
    "\"required query param 'market' not provided\"))",
    Verdict.REAL, twin=f"{CORE}::validation",
)


def test_classify_real_is_not_broadened_by_the_debug_patterns() -> None:
    """Issue #32 also carried a venue validation error reached through the same
    `Api(...)` wrapper. Its class is a venue refusal, so it files."""
    _check(ISSUE_32_VALIDATION)


# Every arm `ApiError::is_retriable()` answers `true` for, in both renderings a
# panic could carry, and Binance's own retriable arms. Each now fails through
# `or_fail`, which tags it from its class.
RETRIABLE_ARMS: list[Row] = [
    Row("Api 5xx / Debug", "transient", 'live_x: Api { status: 503, message: "bad gateway" }',
        Verdict.TRANSIENT, f"{CORE}::api_5xx"),
    Row("Api 5xx / Display", "transient", "live_x: API error: 503 - bad gateway",
        Verdict.TRANSIENT, f"{CORE}::api_5xx"),
    Row("Api 425 / Debug", "transient", 'live_x: Api { status: 425, message: "too early" }',
        Verdict.TRANSIENT, f"{CORE}::api_425"),
    Row("Api 425 / Display", "transient", "live_x: API error: 425 - too early",
        Verdict.TRANSIENT, f"{CORE}::api_425"),
    Row("RateLimit / Debug", "transient", 'live_x: RateLimit("slow down")',
        Verdict.TRANSIENT, f"{CORE}::rate_limit"),
    Row("RateLimit / Display", "transient", "live_x: Rate limit exceeded: slow down",
        Verdict.TRANSIENT, f"{CORE}::rate_limit"),
    Row("Timeout / Debug", "transient", "live_x: Api(Timeout)", Verdict.TRANSIENT,
        f"{CORE}::timeout"),
    Row("Timeout / Display", "transient", "live_x: Request timeout", Verdict.TRANSIENT,
        f"{CORE}::timeout"),
    Row("Network is_timeout / Debug", "transient",
        'live_x: Network(reqwest::Error { kind: Request, '
        'url: "https://clob.polymarket.com/ok", source: TimedOut })',
        Verdict.TRANSIENT, f"{CORE}::network_timeout"),
    Row("Network is_timeout / Display", "transient",
        "live_x: Network error: error sending request for url (https://clob.polymarket.com/ok)",
        Verdict.TRANSIENT, f"{CORE}::network_timeout"),
    Row("Network is_connect / Debug", "transient",
        'live_x: Network(reqwest::Error { kind: Request, '
        'url: "https://clob.polymarket.com/ok", source: '
        "hyper_util::client::legacy::Error(Connect, ConnectError("
        '"tcp connect error", Os { code: 111, kind: ConnectionRefused, '
        'message: "Connection refused" })) })',
        Verdict.TRANSIENT, f"{CORE}::network_connect"),
    Row("Network is_connect / Display", "transient",
        "live_x: Network error: error sending request for url (https://clob.polymarket.com/ok)",
        Verdict.TRANSIENT, f"{CORE}::network_connect"),
    Row("Binance RateLimited / Debug", "transient",
        "live_x: RateLimited { retry_after: Some(1s) }", Verdict.TRANSIENT,
        f"{BINANCE}::rate_limited"),
    Row("Binance RateLimited / Display", "transient",
        "live_x: binance rate limit (429), retry after Some(1s)", Verdict.TRANSIENT,
        f"{BINANCE}::rate_limited"),
    Row("Binance Venue 5xx / Debug", "transient",
        'live_x: Venue { status: 503, code: -1001, msg: "Internal error" }', Verdict.TRANSIENT,
        f"{BINANCE}::venue_5xx"),
    Row("Binance Venue 5xx / Display", "transient",
        "live_x: binance answered 503: -1001 Internal error", Verdict.TRANSIENT,
        f"{BINANCE}::venue_5xx"),
    Row("Binance Venue 408 / Debug", "transient",
        'live_x: Venue { status: 408, code: -1007, msg: "Timeout" }', Verdict.TRANSIENT,
        f"{BINANCE}::venue_408"),
    Row("Binance Venue 408 / Display", "transient",
        "live_x: binance answered 408: -1007 Timeout", Verdict.TRANSIENT,
        f"{BINANCE}::venue_408"),
    # live_api's `raw()` fails a refused fetch with core's reading of its status.
    Row("Binance raw status / 503", "transient",
        "/fapi/v1/time: API error: 503 Service Unavailable", Verdict.TRANSIENT,
        f"{BINANCE}::raw_status_503"),
]


@pytest.mark.parametrize("row", RETRIABLE_ARMS, ids=_ids(RETRIABLE_ARMS))
def test_every_retriable_arm_classifies_transient(row: Row) -> None:
    _check(row)


# The mirror image: arms `is_retriable()` answers `false` for still file.
NON_RETRIABLE_ARMS: list[Row] = [
    Row("Validation / Debug", "real",
        "live_x: Validation(\"required query param 'market' not provided\")", Verdict.REAL,
        f"{CORE}::validation"),
    Row("Validation / Display", "real", "live_x: Validation error: bad request", Verdict.REAL,
        f"{CORE}::validation"),
    Row("Authentication / Debug", "real", 'live_x: Authentication("invalid signature")',
        Verdict.REAL, f"{CORE}::authentication"),
    Row("Api 4xx / Debug", "real", 'live_x: Api { status: 404, message: "not found" }',
        Verdict.REAL, f"{CORE}::api_4xx"),
    Row("Api 4xx / Display", "real", "live_x: API error: 404 - not found", Verdict.REAL,
        f"{CORE}::api_4xx"),
    Row("Serialization / Display", "real",
        "live_x: Serialization error: invalid type at line 1", Verdict.REAL,
        f"{CORE}::serialization"),
    # An assertion prints no tag.
    Row("plain assertion", None, "assertion `left == right` failed\n  left: 3\n right: 4",
        Verdict.REAL),
    Row("Binance Venue 4xx / Debug", "real",
        'live_x: Venue { status: 400, code: -1121, msg: "Invalid symbol." }', Verdict.REAL,
        f"{BINANCE}::venue_4xx"),
    Row("Binance Venue 4xx / Display", "real",
        "live_x: binance answered 400: -1121 Invalid symbol.", Verdict.REAL,
        f"{BINANCE}::venue_4xx"),
    # A ban and the firewall's refusal are restricted and faults, so they file
    # (amendment A2-1).
    Row("Binance IpBanned / Debug", "real", "live_x: IpBanned { retry_after: None }",
        Verdict.REAL, f"{BINANCE}::ip_banned"),
    Row("Binance IpBanned / Display", "real",
        "live_x: binance has banned this IP (418), retry after None", Verdict.REAL,
        f"{BINANCE}::ip_banned"),
    Row("Binance Forbidden / Display", "real",
        "live_x: binance's firewall refused the request (403): <html>", Verdict.REAL,
        f"{BINANCE}::forbidden"),
]


@pytest.mark.parametrize("row", NON_RETRIABLE_ARMS, ids=_ids(NON_RETRIABLE_ARMS))
def test_non_retriable_arms_stay_real(row: Row) -> None:
    _check(row)


# A WebSocket server restarting, or a proxy dropping the connection, in each
# shape a live test's panic carried it. The suites now fail each through its
# venue's error, a raw socket's close frame and transport error wrapped in it
# first, or call `transient` for a bare stream that ends.
WEBSOCKET_DROPS: list[Row] = [
    Row("reset without close / Display", "transient",
        "disconnected after 12 updates: the sports feed connection failed: "
        "WebSocket protocol error: Connection reset without closing handshake",
        Verdict.TRANSIENT, f"{SPORTS}::reset_without_closing_handshake"),
    Row("reset without close / Debug", "transient",
        "the frame parses: Transport { source: Protocol(ResetWithoutClosingHandshake) }",
        Verdict.TRANSIENT, f"{SPORTS}::reset_without_closing_handshake"),
    Row("TLS EOF without close_notify", "transient",
        "stream error: RTDS connection error: IO error: peer closed connection "
        "without sending TLS close_notify: "
        "https://docs.rs/rustls/latest/rustls/manual/_03_howto/index.html#unexpected-eof",
        Verdict.TRANSIENT, f"{RTDS}::tls_eof"),
    Row("close 1012 / raw socket", "transient",
        'the server closed the socket: code 1012, reason "restarting"', Verdict.TRANSIENT,
        f"{SPORTS}::close_1012"),
    Row("close 1001 / SportsError Display", "transient",
        "disconnected after 3 updates: the sports feed closed the connection "
        "with code 1001: going away", Verdict.TRANSIENT, f"{SPORTS}::close_1001"),
    Row("close 1013 / SportsError Debug", "transient",
        'unexpected: Closed { code: Some(1013), reason: "try again later" }',
        Verdict.TRANSIENT, f"{SPORTS}::close_1013"),
    # clob's live_ws probe calls `transient` for a close frame the socket table
    # classes `Network`, so these name no error to build.
    Row("close Away / tungstenite Debug", "transient",
        "server closed the connection: Some(CloseFrame { code: Away, "
        'reason: Utf8Bytes(b"going away") })', Verdict.TRANSIENT),
    Row("close Error / tungstenite Debug", "transient",
        "server closed the connection: Some(CloseFrame { code: Error, "
        'reason: Utf8Bytes(b"") })', Verdict.TRANSIENT),
    Row("no close code visible", "transient",
        "the server ended the connection after 4 frames, inside 40 s", Verdict.TRANSIENT),
    Row("Binance ConnectTimeout", "transient",
        "connect: no connection within 10s (ConnectTimeout(10s))", Verdict.TRANSIENT,
        f"{BINANCE}::streams::connect_timeout"),
    Row("Binance handshake 503 / Display", "transient",
        "connect: WebSocket transport error: HTTP error: 503 Service Unavailable",
        Verdict.TRANSIENT, f"{BINANCE}::streams::handshake_503"),
    Row("Binance close 1011 / Display", "transient",
        "the server closed the connection (Some(1011): Internal error)", Verdict.TRANSIENT,
        f"{BINANCE}::streams::close_1011"),
    Row("Binance stream ended", "transient", "market: the server ended the connection",
        Verdict.TRANSIENT),
    Row("Binance NoAnswer / Display", "transient", "connect: no answer to request 1 within 10s",
        Verdict.TRANSIENT, f"{BINANCE}::streams::no_answer"),
    # polyoxide-cli's live suite rebuilds the close its stderr reports.
    Row("Binance close 1011 / CLI marker", "transient",
        "# market disconnected: closed by the server (1011 Internal error). "
        "Its streams are stale until it reconnects.", Verdict.TRANSIENT,
        f"{BINANCE}::streams::close_1011"),
]


@pytest.mark.parametrize("row", WEBSOCKET_DROPS, ids=_ids(WEBSOCKET_DROPS))
def test_a_dropped_websocket_is_transient(row: Row) -> None:
    _check(row)


# Close codes and faults that say the client is at fault, or that the feed's
# content is wrong, file. A site that is itself the property under test, such
# as staleness or an unanswered ping, prints no tag.
WEBSOCKET_FAULTS_STAY_REAL: list[Row] = [
    Row("normal close", "transient", 'the server closed the socket: code 1000, reason ""',
        Verdict.REAL, f"{SPORTS}::close_1000", now=Verdict.TRANSIENT,
        changed_by="AD-14's socket table: a server's 1000 mid-test is a drop, so it is retried"),
    Row("policy close", "real", "the sports feed closed the connection with code 1008: policy",
        Verdict.REAL, f"{SPORTS}::close_1008"),
    # clob's probe returns any other close as a rejection, which its assertion files.
    Row("policy close / Debug", None,
        'server closed the connection: Some(CloseFrame { code: Policy, reason: Utf8Bytes(b"") })',
        Verdict.REAL),
    Row("1001 inside a hex id", None,
        "no book for 0xbd31dc8a20211944f6b70f31557f1001557b59905b7738480ca09bd4532f84af",
        Verdict.REAL),
    Row("frame did not parse", "real", "a live frame did not parse: missing field `score`",
        Verdict.REAL, f"{SPORTS}::decode"),
    Row("stale", None,
        "disconnected after 2 updates: nothing received from the sports feed for 45s, "
        "pings included", Verdict.REAL),
    Row("Binance close 1008 Invalid request / Display", "real",
        "the server closed the connection (Some(1008): Invalid request)", Verdict.REAL,
        f"{BINANCE}::streams::close_1008"),
    Row("Binance handshake 404 / Display", "real",
        "connect: WebSocket transport error: HTTP error: 404 Not Found", Verdict.REAL,
        f"{BINANCE}::streams::handshake_404"),
    Row("Binance pong unanswered", None, "pong: NoAnswer { id: 1, timeout: 10s }", Verdict.REAL),
    Row("Binance close 1008 / CLI marker", "real",
        "# market disconnected: closed by the server (1008 Invalid request). "
        "Its streams are stale until it reconnects.", Verdict.REAL,
        f"{BINANCE}::streams::close_1008"),
    Row("Binance kind delivered nothing", None,
        'in 60 s these kinds delivered nothing: ["kline"]; last outage: Some("market: Stale")',
        Verdict.REAL),
    Row("Binance stream sent nothing to check", None,
        'market: in 10 s these streams sent no frame to check: ["btcusdt@kline_1m"]',
        Verdict.REAL),
]


@pytest.mark.parametrize("row", WEBSOCKET_FAULTS_STAY_REAL, ids=_ids(WEBSOCKET_FAULTS_STAY_REAL))
def test_a_websocket_fault_stays_real(row: Row) -> None:
    _check(row)


SPORTS_TIMEOUT = Row(
    "sports timeout", "environmental",
    "sports channel should push a frame within the window; if no matches "
    "are live anywhere this can legitimately time out, so re-run before "
    "concluding a defect: Elapsed(())",
    Verdict.ENVIRONMENTAL,
)


def test_classify_environmental_sports_timeout() -> None:
    """A feed with no match live anywhere is a fact about the world, so the
    test calls `environmental`."""
    _check(SPORTS_TIMEOUT)


NO_QUALIFYING_MARKET = Row(
    "no qualifying market", "environmental",
    "no qualifying market with a best ask above 0.05 in the 100 open "
    "markets gamma lists; market conditions rather than a defect, "
    "so re-run before concluding otherwise",
    Verdict.ENVIRONMENTAL,
)


def test_classify_environmental_no_qualifying_market() -> None:
    """The order-placing tests need a market whose book satisfies a price
    precondition, and call `environmental` when none does."""
    _check(NO_QUALIFYING_MARKET)


NO_SUITABLE_MARKET = Row(
    "no suitable market", "environmental",
    "no suitable market: best ask 0.042 is too cheap for a safe "
    "non-crossing test; market conditions rather than a defect",
    Verdict.ENVIRONMENTAL,
)


def test_classify_environmental_no_suitable_market() -> None:
    """The in-test guard is a precondition check that calls `environmental`."""
    _check(NO_SUITABLE_MARKET)


# Binance refuses a caller in a place it does not serve with HTTP 451, on REST
# and on the stream handshake. A 451 is restricted and not a fault, so it is
# environmental however it arrives.
BINANCE_REGION_BLOCKS: list[Row] = [
    Row("RegionBlocked / Display", "environmental",
        "live_x: binance does not serve this location (451): "
        "Service unavailable from a restricted location", Verdict.ENVIRONMENTAL,
        f"{BINANCE}::region_blocked"),
    Row("RegionBlocked / Debug", "environmental",
        'live_x: RegionBlocked { msg: "Service unavailable from a restricted location" }',
        Verdict.ENVIRONMENTAL, f"{BINANCE}::region_blocked"),
    Row("raw status", "environmental",
        "/fapi/v1/time: API error: 451 Unavailable For Legal Reasons", Verdict.ENVIRONMENTAL,
        f"{BINANCE}::raw_status_451"),
    Row("handshake / Display", "environmental",
        "live_x: WebSocket transport error: HTTP error: 451 Unavailable For Legal Reasons",
        Verdict.ENVIRONMENTAL, f"{BINANCE}::streams::handshake_451"),
    Row("handshake / Debug", "environmental",
        "live_x: Connect(Http(Response { status: 451, version: HTTP/1.1, headers: {} }))",
        Verdict.ENVIRONMENTAL, f"{BINANCE}::streams::handshake_451"),
]


@pytest.mark.parametrize("row", BINANCE_REGION_BLOCKS, ids=_ids(BINANCE_REGION_BLOCKS))
def test_a_binance_region_block_is_environmental(row: Row) -> None:
    _check(row)


OTHER_BINANCE_REFUSALS: list[Row] = [
    Row("firewall", "real", "live_x: binance's firewall refused the request (403): <html>",
        Verdict.REAL, f"{BINANCE}::forbidden"),
    Row("ban", "real", "live_x: IpBanned { retry_after: None }", Verdict.REAL,
        f"{BINANCE}::ip_banned"),
    Row("raw 403", "real", "/fapi/v1/time: API error: 403 Forbidden", Verdict.REAL,
        f"{BINANCE}::raw_status_403"),
]


def test_other_binance_refusals_are_not_environmental() -> None:
    """A firewall refusal or a ban is about how this client behaved, not where
    it runs, so each is restricted and a fault, and files (amendment A2-1)."""
    for row in OTHER_BINANCE_REFUSALS:
        _check(row)


MARKET_WORD = Row(
    "market word", None, "market_by_token returned the wrong condition_id for the market",
    Verdict.REAL,
)


def test_classify_bare_market_word_is_real() -> None:
    """An assertion that mentions a market is not market conditions."""
    _check(MARKET_WORD)


def test_classify_empty_string_is_real() -> None:
    """No information defaults to REAL — better to false-positive than skip."""
    assert classify("") == Verdict.REAL


FIVE_HUNDRED_IN_PROSE = Row(
    "500 in prose", None, "computed value 500 differs from expected 600", Verdict.REAL,
)


def test_classify_unrelated_5xx_substring_does_not_match() -> None:
    _check(FIVE_HUNDRED_IN_PROSE)


# The fixtures' twins, by fixture, for the rows the fixture tests check.
FIXTURE_TWINS = {
    "nextest-transient-429.json": f"{CORE}::rate_limit",
    "nextest-transient-503.json": f"{CORE}::api_5xx",
    "nextest-transient-connection.json": f"{CORE}::network_connect",
}
SINGLE_ROWS: list[Row] = [
    PRIVATE_KEY_ONLY, AUTH_BEATS_503, ISSUE_32_TIMEOUT, UNREACHABLE_HOST, ISSUE_32_VALIDATION,
    SPORTS_TIMEOUT, NO_QUALIFYING_MARKET, NO_SUITABLE_MARKET, MARKET_WORD, FIVE_HUNDRED_IN_PROSE,
]
ALL_ROWS = (RETRIABLE_ARMS + NON_RETRIABLE_ARMS + WEBSOCKET_DROPS + WEBSOCKET_FAULTS_STAY_REAL
            + BINANCE_REGION_BLOCKS + OTHER_BINANCE_REFUSALS + SINGLE_ROWS)
TWINS = sorted({row.twin for row in ALL_ROWS if row.twin} | set(FIXTURE_TWINS.values()))


@pytest.mark.parametrize("twin", TWINS)
def test_every_twin_exists(twin: str) -> None:
    """A row's twin names a Rust test that is really there."""
    path, name = twin.split("::", 1)
    source = (REPO / path).read_text(encoding="utf-8")
    assert re.search(rf"^\s*(?:async )?fn {re.escape(name.split('::')[-1])}\(\)", source, re.M), twin
    for module in name.split("::")[:-1]:
        assert re.search(rf"^\s*mod {re.escape(module)} \{{", source, re.M), twin


def test_only_marked_rows_changed_verdict() -> None:
    """Every row reaches the regex era's verdict but those AD-14 changed, and
    each of those says which rule changed it."""
    changed = [row for row in ALL_ROWS if row.verdict != row.was]
    assert [row.label for row in changed] == ["normal close"]
    assert all(row.changed_by for row in changed)
    assert all(row.changed_by is None for row in ALL_ROWS if row.now is None)


def test_a_row_without_a_tag_names_no_twin() -> None:
    """A twin asserts a tag, so a row whose site prints none has nothing to twin."""
    assert not [row.label for row in ALL_ROWS if row.tag is None and row.twin]


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
        '{"type":"test","name":"polyoxide-gamma::live_api$live_search_markets","event":"failed","stdout":"polyoxide-class=transient\\n\\nthread \'live_search_markets\' panicked at a.rs:1:1:\\nHTTP 429"}\n'
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


def test_cli_merge_keeps_an_environmental_or_auth_gated_retry(tmp_path: Path) -> None:
    """A transient first pass whose retry is environmental (a dropped connection,
    then a quiet feed) or auth-gated takes the retry's verdict, not REAL."""
    retry_file = tmp_path / "retry.json"
    retry_file.write_text(
        '{"type":"suite","event":"started","test_count":2}\n'
        '{"type":"test","event":"started","name":"polyoxide-gamma::live_api$live_search_markets"}\n'
        '{"type":"test","name":"polyoxide-gamma::live_api$live_search_markets","event":"failed","stdout":"polyoxide-class=environmental\\n\\nthread \'live_search_markets\' panicked at a.rs:1:1:\\nquiet"}\n'
        '{"type":"test","event":"started","name":"polyoxide-clob::live_api$live_get_order"}\n'
        '{"type":"test","name":"polyoxide-clob::live_api$live_get_order","event":"failed","stdout":"polyoxide-class=auth-gated\\n\\nthread \'live_get_order\' panicked at a.rs:1:1:\\nunset"}\n'
        '{"type":"suite","event":"failed","passed":0,"failed":2,"ignored":0,"measured":0,"filtered_out":0,"exec_time":0.5}\n'
    )

    out_dir = tmp_path / "out"
    out_dir.mkdir()
    subprocess.run(
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

    real = (out_dir / "real-failures.txt").read_text().splitlines()
    assert real == ["polyoxide-gamma::live_api$live_get_market"]
    environmental = (out_dir / "environmental.txt").read_text().splitlines()
    assert sorted(environmental) == sorted([
        "polyoxide-clob::live_ws$live_sports_frames",
        "polyoxide-gamma::live_api$live_search_markets",
    ])
    auth = (out_dir / "auth-gated.txt").read_text().splitlines()
    assert "polyoxide-clob::live_api$live_get_order" in auth


# --- tag lines (AD-14) --------------------------------------------------------
#
# A test that fails through polyoxide-test-support prints `polyoxide-class=<tag>`
# alone on a line just before it panics. The tag decides, and a log without one
# is real.


@pytest.mark.parametrize("tag,message", [
    # Each message is one the regex era classified otherwise.
    ("transient", 'markets: V2(V2Error { code: "dependency_unavailable" })'),
    ("real", "markets: the HTTP 503 page did not parse"),
    ("environmental", "markets: Api(Api { status: 503 })"),
    ("auth-gated", "credentials not configured: KEY absent or empty in the environment"),
    ("real", "POLYMARKET_* env vars required for authenticated tests"),
    ("transient", "no qualifying market with a best ask above 0.05"),
])
def test_a_tag_decides_whatever_the_text_says(tag: str, message: str) -> None:
    assert classify(REPORT.format(message=message)) == Verdict.REAL
    assert classify(_tagged(tag, message)) == Verdict(tag)


def test_the_tag_just_before_the_final_report_decides() -> None:
    # The hook's own output: the tag, then a blank line, then the report.
    assert classify(_tagged("environmental", "nothing live")) == Verdict.ENVIRONMENTAL
    # A caught transient, then a final environmental failure.
    text = _tagged("transient", "first connect") + _tagged("environmental", "nothing live")
    assert classify(text) == Verdict.ENVIRONMENTAL


@pytest.mark.parametrize("stale", ["transient", "environmental", "auth-gated"])
def test_a_stale_tag_before_an_untagged_final_panic_is_ignored(stale: str) -> None:
    """A caught tagged panic, or a spawned task's, then the test failing through
    a bare `assert!` or `unwrap()`: the tag is not about the final failure."""
    text = _tagged(stale, "first connect") + REPORT.format(message="assertion failed: frames > 0")
    assert classify(text) == Verdict.REAL
    # Nor does the final panic's text decide.
    text = _tagged(stale, "first connect") + REPORT.format(message="markets: Api { status: 503 }")
    assert classify(text) == Verdict.REAL


def test_a_tag_parted_from_the_report_by_other_output_is_ignored() -> None:
    text = "polyoxide-class=transient\nsome other hook ran\n" + REPORT.format(message="x")
    assert classify(text) == Verdict.REAL


@pytest.mark.parametrize("later", ["environmental", "transient", "auth-gated"])
def test_a_real_tag_is_never_outranked_by_a_later_one(later: str) -> None:
    """A fault the test caught, or a spawned task hit, still files an issue."""
    text = _tagged("real", "a frame did not parse") + _tagged(later, "nothing live")
    assert classify(text) == Verdict.REAL


@pytest.mark.parametrize("text", [
    "panicked: polyoxide-class=transient",
    "  polyoxide-class=transient",
    "polyoxide-class=transient because the feed dropped",
    "the test printed polyoxide-class=environmental in its message\n",
])
def test_a_tag_that_is_not_alone_on_its_line_is_ignored(text: str) -> None:
    assert classify(text) == Verdict.REAL
    assert classify(text + REPORT.format(message="HTTP 503")) == Verdict.REAL


@pytest.mark.parametrize("text", [
    "polyoxide-class=transient\n\nthread 'x' panicked at a.rs:1:1:\nboom\n",
    "polyoxide-class=transient\nthread 'x' (7) panicked at a.rs:1:1:\nboom",
    "polyoxide-class=transient   \n\nthread 'x' panicked at a.rs:1:1:\n",
    "polyoxide-class=transient\r\n\r\nthread 'x' (7) panicked at a.rs:1:1:\r\nboom\r\n",
    "earlier output\npolyoxide-class=transient\n\nthread 'x' panicked at a.rs:1:1:\n",
])
def test_a_tag_alone_on_its_line_is_read(text: str) -> None:
    assert classify(text) == Verdict.TRANSIENT


def test_a_tag_with_no_panic_report_after_it_is_ignored() -> None:
    assert classify("polyoxide-class=transient\n") == Verdict.REAL


@pytest.mark.parametrize("tag", ["flaky", "pass", "Transient", "transient-ish"])
def test_an_unknown_tag_is_real(tag: str) -> None:
    # Even when the text names a retriable status or missing credentials.
    assert classify(_tagged(tag, "Api { status: 503 }")) == Verdict.REAL
    assert classify(_tagged(tag, "POLYMARKET_* env vars required")) == Verdict.REAL


def test_an_unknown_last_tag_is_real_even_after_a_known_one() -> None:
    assert classify(_tagged("transient", "x") + _tagged("bogus", "y")) == Verdict.REAL


def test_parse_tagged_fixture() -> None:
    outcomes = parse_nextest_json(FIXTURES / "nextest-tagged.json")
    verdicts = {o.name: o.verdict for o in outcomes}
    assert verdicts == {
        "polyoxide-gamma::live_api$live_list_markets": Verdict.PASS,
        "polyoxide-gamma::live_api$live_get_market": Verdict.TRANSIENT,
        "polyoxide-gamma::live_api$live_search_markets": Verdict.REAL,
        "polyoxide-clob::live_api$live_create_order": Verdict.AUTH_GATED,
        "polyoxide-clob::live_api$live_fak_unmatched_is_typed_error": Verdict.ENVIRONMENTAL,
        # Untagged: real, whatever its text says.
        "polyoxide-clob::live_api$live_get_order": Verdict.REAL,
        # A caught real failure, then a final environmental one, in a log the
        # regexes call transient: the fault still files.
        "polyoxide-sports::live_api$live_api_channel_yields_frames": Verdict.REAL,
    }


def test_every_tagged_fixture_row_is_decided_by_its_tag() -> None:
    """Guards the test above: each tagged row that is not real would be real
    without its tag, so it passes only if the tag was read."""
    import classify_failures

    tagged = [o for o in parse_nextest_json(FIXTURES / "nextest-tagged.json")
              if classify_failures.TAG_LINE.search(o.output)]
    assert {o.verdict for o in tagged} >= {Verdict.TRANSIENT, Verdict.AUTH_GATED,
                                           Verdict.ENVIRONMENTAL}
    for outcome in tagged:
        untagged = classify_failures.TAG_LINE.sub("", outcome.output)
        assert classify(untagged) == Verdict.REAL, outcome.name


def test_cli_classify_reads_tags_end_to_end(tmp_path: Path) -> None:
    out_dir = tmp_path / "out"
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "classify",
            "--input", str(FIXTURES / "nextest-tagged.json"),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    assert (out_dir / "retry-tests.txt").read_text().splitlines() == [
        "polyoxide-gamma::live_api$live_get_market",
    ]
    assert (out_dir / "real-failures.txt").read_text().splitlines() == [
        "polyoxide-gamma::live_api$live_search_markets",
        "polyoxide-clob::live_api$live_get_order",
        "polyoxide-sports::live_api$live_api_channel_yields_frames",
    ]
    assert (out_dir / "auth-gated.txt").read_text().splitlines() == [
        "polyoxide-clob::live_api$live_create_order",
    ]
    assert (out_dir / "environmental.txt").read_text().splitlines() == [
        "polyoxide-clob::live_api$live_fak_unmatched_is_typed_error",
    ]


def test_a_tag_opening_stderr_starts_a_line_after_a_stdout_without_one(tmp_path: Path) -> None:
    """Joined bare, `stdout`'s last line and the tag would share a line, and the
    tag would be read as quoted text."""
    fixture = tmp_path / "split.json"
    event = {
        "type": "test", "event": "failed", "name": "polyoxide-gamma::live_api$live_x",
        "stdout": "fetched 3 markets",
        "stderr": "polyoxide-class=environmental\n" + REPORT.format(message="nothing live"),
    }
    fixture.write_text(json.dumps(event) + "\n")
    [outcome] = parse_nextest_json(fixture)
    assert outcome.verdict == Verdict.ENVIRONMENTAL
    assert outcome.output.startswith("fetched 3 markets\npolyoxide-class=environmental\n")


# --- retried names -----------------------------------------------------------


def test_parse_strips_the_attempt_suffix_nextest_adds_to_a_retried_test() -> None:
    names = [o.name for o in parse_nextest_json(FIXTURES / "nextest-retry-suffix.json")]
    assert names == [
        "polyoxide-gamma::live_api$live_search_markets",
        "polyoxide-clob::live_api$live_get_order",
    ]


def test_cli_merge_promotes_a_suffixed_persistent_transient_to_real(tmp_path: Path) -> None:
    """The retry pass runs with `--retries 2`, so nextest names a test that
    failed every attempt `...$live_search_markets#3`. Looked up bare, it was
    missing from the retry results and merged as a PASS, so a persistent
    transient was never filed."""
    raw = (FIXTURES / "nextest-retry-suffix.json").read_text()
    assert "$live_search_markets#3" in raw and "$live_get_order#2" in raw

    out_dir = tmp_path / "out"
    subprocess.run(
        [
            sys.executable, str(SCRIPT), "merge",
            "--first-pass", str(FIXTURES / "nextest-mixed.json"),
            "--retry", str(FIXTURES / "nextest-retry-suffix.json"),
            "--output-dir", str(out_dir),
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    real = (out_dir / "real-failures.txt").read_text().splitlines()
    # live_get_market was REAL on first pass. live_search_markets failed all
    # three attempts. live_get_order passed on its second attempt.
    assert sorted(real) == sorted([
        "polyoxide-gamma::live_api$live_get_market",
        "polyoxide-gamma::live_api$live_search_markets",
    ])
    assert "live_search_markets" in (out_dir / "report.md").read_text()
