"""Data API v2 bindings against a local server: no network.

Each route is called with every argument it accepts, and the request that
reaches the server is compared with the query the route should send. Values
are distinct per argument, so a kwarg wired to the wrong setter fails. The
responses are the captured payloads in `polyoxide-data/tests/fixtures/v2/`.
"""

from __future__ import annotations

import asyncio
import json
import pathlib
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qsl, urlsplit

import pytest

import polyoxide
from polyoxide import v2

FIXTURES = pathlib.Path(__file__).resolve().parents[2] / "polyoxide-data" / "tests" / "fixtures" / "v2"


def fixture(name: str) -> dict:
    return json.loads((FIXTURES / f"{name}.json").read_text())


class FakeDataApi:
    """Answers each path from a queue of (status, headers, body) and records every request.

    The last response queued for a path repeats.
    """

    def __init__(self) -> None:
        self.requests: list[tuple[str, dict[str, str]]] = []
        self.responses: dict[str, list[tuple[int, dict[str, str], str]]] = {}
        server = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802 - http.server's naming
                url = urlsplit(self.path)
                server.requests.append((url.path, dict(parse_qsl(url.query, keep_blank_values=True))))
                queue = server.responses.get(url.path) or [(404, {}, "404 page not found")]
                status, headers, body = queue.pop(0) if len(queue) > 1 else queue[0]
                payload = body.encode()
                self.send_response(status)
                for name, value in {"content-type": "application/json", **headers}.items():
                    self.send_header(name, value)
                self.send_header("content-length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def log_message(self, *args: object) -> None:
                pass

        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.url = f"http://127.0.0.1:{self.httpd.server_address[1]}"
        # A short poll keeps shutdown() from waiting out the default 0.5s per test.
        threading.Thread(target=self.httpd.serve_forever, args=(0.01,), daemon=True).start()

    def reply(self, path: str, *bodies: dict | str, status: int = 200, headers: dict[str, str] | None = None) -> None:
        self.responses[path] = [
            (status, headers or {}, body if isinstance(body, str) else json.dumps(body)) for body in bodies
        ]


@pytest.fixture
def server():
    fake = FakeDataApi()
    yield fake
    fake.httpd.shutdown()
    fake.httpd.server_close()


# (method, positional args, kwargs, path, query the route must send, fixture, shape, row class)
ROUTES = [
    ("approvals", ["0xuser"], {}, "/v2/approvals", {"user": "0xuser"}, "approvals", "row", "Approvals"),
    (
        "positions",
        [],
        {
            "user": "0xuser",
            "conditions": ["0xc1", "0xc2"],
            "status": "CLOSED",
            "event_ids": ["e1", "e2"],
            "title": "a title",
            "filter_type": "TOKENS",
            "filter_amount": 2.5,
            "include_archived": True,
            "sort_by": "REALIZED_PNL",
            "sort_direction": "ASC",
            "start": 11,
            "end": 12,
            "limit": 13,
            "cursor": "cur",
        },
        "/v2/positions",
        {
            "user": "0xuser",
            "condition": "0xc1,0xc2",
            "status": "CLOSED",
            "event_id": "e1,e2",
            "title": "a title",
            "filter_type": "TOKENS",
            "filter_amount": "2.5",
            "include_archived": "true",
            "sort_by": "REALIZED_PNL",
            "sort_direction": "ASC",
            "start": "11",
            "end": "12",
            "limit": "13",
            "cursor": "cur",
        },
        "positions",
        "page",
        "Position",
    ),
    (
        "combo_positions",
        ["0xuser"],
        {
            "conditions": ["0xc1"],
            "statuses": ["OPEN", "PARTIAL"],
            "sort_by": "UPDATED",
            "sort_direction": "DESC",
            "updated_after": 21,
            "updated_before": 22,
            "limit": 23,
            "cursor": "cur",
        },
        "/v2/positions/combos",
        {
            "user": "0xuser",
            "condition": "0xc1",
            "status": "OPEN,PARTIAL",
            "sort_by": "UPDATED",
            "sort_direction": "DESC",
            "updated_after": "21",
            "updated_before": "22",
            "limit": "23",
            "cursor": "cur",
        },
        "combo_positions",
        "page",
        "ComboPosition",
    ),
    (
        "user_pnl",
        ["0xuser"],
        {"interval": "1w", "fidelity": "12h"},
        "/v2/user-pnl",
        {"user": "0xuser", "interval": "1w", "fidelity": "12h"},
        "user_pnl",
        "row",
        "UserPnlSeries",
    ),
    ("user_stats", ["0xuser"], {}, "/v2/user-stats", {"user": "0xuser"}, "user_stats", "row", "UserStats"),
    (
        "user_volume",
        ["0xuser"],
        {"start": 31, "end": 32},
        "/v2/user-volume",
        {"user": "0xuser", "start": "31", "end": "32"},
        "user_volume",
        "row",
        "UserVolume",
    ),
    (
        "value",
        ["0xuser"],
        {"conditions": ["0xc1", "0xc2"]},
        "/v2/value",
        {"user": "0xuser", "condition": "0xc1,0xc2"},
        "value",
        "row",
        "PortfolioValue",
    ),
    (
        "activity",
        ["0xuser"],
        {
            "types": ["TRADE", "TIP"],
            "conditions": ["0xc1"],
            "event_ids": ["e1"],
            "side": "SELL",
            "start": 41,
            "end": 42,
            "sort_by": "TIMESTAMP",
            "sort_direction": "ASC",
            "exclude_deposits_withdrawals": False,
            "limit": 43,
            "cursor": "cur",
        },
        "/v2/activity",
        {
            "user": "0xuser",
            "type": "TRADE,TIP",
            "condition": "0xc1",
            "event_id": "e1",
            "side": "SELL",
            "start": "41",
            "end": "42",
            "sort_by": "TIMESTAMP",
            "sort_direction": "ASC",
            "exclude_deposits_withdrawals": "false",
            "limit": "43",
            "cursor": "cur",
        },
        "activity",
        "page",
        "Activity",
    ),
    (
        "combo_activity",
        ["0xuser"],
        {"conditions": ["0xc1"], "limit": 51, "cursor": "cur"},
        "/v2/activity/combos",
        {"user": "0xuser", "condition": "0xc1", "limit": "51", "cursor": "cur"},
        "combo_activity",
        "page",
        "ComboActivity",
    ),
    (
        "trades",
        [],
        {
            "user": "0xuser",
            "conditions": ["0xc1"],
            "event_ids": ["e1"],
            "side": "BUY",
            "taker_only": True,
            "filter_type": "CASH",
            "filter_amount": 6.5,
            "start": 61,
            "end": 62,
            "limit": 63,
            "cursor": "cur",
        },
        "/v2/trades",
        {
            "user": "0xuser",
            "condition": "0xc1",
            "event_id": "e1",
            "side": "BUY",
            "taker_only": "true",
            "filter_type": "CASH",
            "filter_amount": "6.5",
            "start": "61",
            "end": "62",
            "limit": "63",
            "cursor": "cur",
        },
        "trades",
        "page",
        "Trade",
    ),
    (
        "holders",
        [["0xc1", "0xc2"]],
        {"min_balance": 7.5, "include_pnl": True, "limit": 71, "cursor": "cur"},
        "/v2/holders",
        {"condition": "0xc1,0xc2", "min_balance": "7.5", "include_pnl": "true", "limit": "71", "cursor": "cur"},
        "holders_pnl",
        "page",
        "MetaHolder",
    ),
    ("live_volume", [["e1", "e2"]], {}, "/v2/live-volume", {"event_id": "e1,e2"}, "live_volume", "row", "LiveVolume"),
    (
        "open_interest",
        [],
        {"conditions": ["0xc1", "0xc2"]},
        "/v2/oi",
        {"condition": "0xc1,0xc2"},
        "open_interest",
        "list",
        "OpenInterest",
    ),
    (
        "prices_history",
        ["tok"],
        {"start": 81, "end": 82, "interval": "6h", "bucket_seconds": 83, "as_of": 84, "limit": 85, "cursor": "cur"},
        "/v2/prices-history",
        {
            "token_id": "tok",
            "start": "81",
            "end": "82",
            "interval": "6h",
            "bucket_seconds": "83",
            "as_of": "84",
            "limit": "85",
            "cursor": "cur",
        },
        "prices_history",
        "page",
        "PricePoint",
    ),
    (
        "resolutions",
        [],
        {"question_id": "0xq"},
        "/v2/resolutions",
        {"question_id": "0xq"},
        "resolutions",
        "list",
        "Resolution",
    ),
    (
        "resolutions",
        [],
        {"conditions": ["0xc1", "0xc2"]},
        "/v2/resolutions",
        {"condition": "0xc1,0xc2"},
        "resolutions",
        "list",
        "Resolution",
    ),
    (
        "resolutions",
        [],
        {"event_ids": ["e1"]},
        "/v2/resolutions",
        {"event_id": "e1"},
        "resolutions",
        "list",
        "Resolution",
    ),
    (
        "biggest_winners",
        [],
        {"time_period": "month", "category": "sports", "limit": 91, "cursor": "cur"},
        "/v2/biggest-winners",
        {"time_period": "month", "category": "sports", "limit": "91", "cursor": "cur"},
        "biggest_winners",
        "page",
        "BiggestWinner",
    ),
    (
        "builders_leaderboard",
        [],
        {"time_period": "day", "limit": 101, "cursor": "cur"},
        "/v2/builders/leaderboard",
        {"time_period": "day", "limit": "101", "cursor": "cur"},
        "builders_leaderboard",
        "page",
        "BuilderStanding",
    ),
    (
        "builder_volume",
        [],
        {"interval": "all", "limit": 111},
        "/v2/builders/volume",
        {"interval": "all", "limit": "111"},
        "builder_volume",
        "list",
        "BuilderVolumePoint",
    ),
    (
        "leaderboard",
        [],
        {"time_period": "week", "category": "crypto", "board": "VOLUME", "limit": 121, "cursor": "cur"},
        "/v2/leaderboard",
        {"time_period": "week", "category": "crypto", "sort_by": "VOLUME", "limit": "121", "cursor": "cur"},
        "leaderboard",
        "page",
        "LeaderboardEntry",
    ),
    (
        "leaderboard_user",
        ["0xuser"],
        {"time_period": "all", "category": "politics"},
        "/v2/leaderboard",
        {"user": "0xuser", "time_period": "all", "category": "politics"},
        "leaderboard_user",
        "row",
        "LeaderboardUserEntry",
    ),
    ("status", [], {}, "/v2/status", {}, "status", "row", "ServiceStatus"),
]


def test_every_route_is_covered() -> None:
    covered = {route[0] for route in ROUTES}
    names = {name for name in dir(v2.DataV2Sync) if not name.startswith(("_", "iter_"))}
    assert covered == names


def assert_shape(result: object, shape: str, row: str) -> None:
    row_class = getattr(v2, row)
    if shape == "page":
        assert isinstance(result, v2.Page)
        assert len(result) == len(result.data) > 0
        assert all(isinstance(r, row_class) for r in result.data)
        assert isinstance(result.pagination, v2.Pagination)
    elif shape == "list":
        assert isinstance(result, list) and result
        assert all(isinstance(r, row_class) for r in result)
    else:
        assert isinstance(result, row_class)


@pytest.mark.parametrize(
    ("method", "args", "kwargs", "path", "query", "body", "shape", "row"),
    ROUTES,
    ids=[f"{r[0]}-{'-'.join(r[2]) or 'bare'}" for r in ROUTES],
)
def test_route_sends_every_argument(server, method, args, kwargs, path, query, body, shape, row) -> None:
    server.reply(path, fixture(body))
    result = getattr(polyoxide.DataApiSync(base_url=server.url).v2(), method)(*args, **kwargs)

    assert server.requests == [(path, query)]
    assert_shape(result, shape, row)


@pytest.mark.parametrize(("method", "args", "kwargs", "path", "query", "body", "shape", "row"), ROUTES[:3], ids=[r[0] for r in ROUTES[:3]])
def test_async_route_sends_every_argument(server, method, args, kwargs, path, query, body, shape, row) -> None:
    server.reply(path, fixture(body))

    async def call():
        return await getattr(polyoxide.DataApi(base_url=server.url).v2(), method)(*args, **kwargs)

    result = asyncio.run(call())
    assert server.requests == [(path, query)]
    assert_shape(result, shape, row)


def test_getters_read_the_payload(server) -> None:
    body = fixture("activity_tips")
    server.reply("/v2/activity", body)
    page = polyoxide.DataApiSync(base_url=server.url).v2().activity("0xuser")

    first = body["data"][0]
    assert page.data[0].activity_type == first["type"]
    assert page.data[0].to_dict().items() >= first.items()
    assert page.pagination.next_cursor == body["pagination"]["next_cursor"]
    assert page.pagination.has_more is body["pagination"]["has_more"]


def test_an_unknown_wallet_is_none(server) -> None:
    server.reply("/v2/user-stats", fixture("user_stats_unknown"))
    server.reply("/v2/leaderboard", fixture("leaderboard_user_unknown"))
    client = polyoxide.DataApiSync(base_url=server.url).v2()

    assert client.user_stats("0xunknown") is None
    assert client.leaderboard_user("0xunknown") is None


def page_of(body: str, cursor: str | None) -> dict:
    page = fixture(body)
    page["pagination"] = {**page["pagination"], "has_more": cursor is not None, "next_cursor": cursor}
    return page


def test_iter_walks_every_page_with_the_same_filters(server) -> None:
    server.reply("/v2/trades", page_of("trades", "c2"), page_of("trades", "c3"), page_of("trades", None))
    walk = polyoxide.DataApiSync(base_url=server.url).v2().iter_trades(user="0xuser", limit=2)

    pages = list(walk)

    assert [p.pagination.next_cursor for p in pages] == ["c2", "c3", None]
    assert server.requests == [
        ("/v2/trades", {"user": "0xuser", "limit": "2"}),
        ("/v2/trades", {"user": "0xuser", "limit": "2", "cursor": "c2"}),
        ("/v2/trades", {"user": "0xuser", "limit": "2", "cursor": "c3"}),
    ]
    assert list(walk) == [], "an exhausted walk stays exhausted"


def test_async_iter_walks_every_page(server) -> None:
    server.reply("/v2/holders", page_of("holders", "c2"), page_of("holders", None))

    async def walk():
        return [page async for page in polyoxide.DataApi(base_url=server.url).v2().iter_holders(["0xc1"])]

    pages = asyncio.run(walk())

    assert [p.pagination.next_cursor for p in pages] == ["c2", None]
    assert all(isinstance(row, v2.MetaHolder) for p in pages for row in p.data)
    assert [q.get("cursor") for _, q in server.requests] == [None, "c2"]


@pytest.mark.parametrize(
    ("call", "message"),
    [
        (lambda c: c.positions(), "positions needs user, conditions, or both"),
        (lambda c: c.positions(conditions=["0xc1", "0xc2"]), "exactly one condition, got 2"),
        (lambda c: c.resolutions(), "exactly one of question_id, conditions or event_ids"),
        (lambda c: c.resolutions(question_id="0xq", event_ids=["e1"]), "exactly one of"),
        (lambda c: c.trades(filter_type="cash"), 'invalid filter_type "cash", expected one of: CASH, TOKENS'),
        (lambda c: c.leaderboard(time_period="WEEK"), "expected one of: day, week, month, all"),
        (lambda c: c.combo_positions("0xuser", statuses=["OPEN", "NOPE"]), 'invalid statuses "NOPE"'),
        (lambda c: c.activity("0xuser", sort_direction="asc"), 'invalid sort_direction "asc", expected one of: ASC, DESC'),
        (lambda c: c.iter_trades(filter_type="cash"), 'invalid filter_type "cash"'),
    ],
)
def test_a_bad_argument_raises_before_any_request(server, call, message) -> None:
    with pytest.raises(ValueError, match=message):
        call(polyoxide.DataApiSync(base_url=server.url).v2())
    assert server.requests == []


def test_async_bad_argument_raises_at_the_call_not_the_await(server) -> None:
    with pytest.raises(ValueError):
        polyoxide.DataApi(base_url=server.url).v2().trades(filter_type="cash")
    assert server.requests == []


def test_positions_anchors_send_their_own_keys(server) -> None:
    server.reply("/v2/positions", fixture("positions"))
    client = polyoxide.DataApiSync(base_url=server.url).v2()

    client.positions(user="0xuser")
    client.positions(conditions=["0xc1"])
    client.positions(user="0xuser", conditions=["0xc1", "0xc2"])

    assert [q for _, q in server.requests] == [
        {"user": "0xuser"},
        {"condition": "0xc1"},
        {"user": "0xuser", "condition": "0xc1,0xc2"},
    ]


def test_response_enums_pass_unknown_values_through(server) -> None:
    server.reply("/v2/activity", fixture("activity"))
    polyoxide.DataApiSync(base_url=server.url).v2().activity("0xuser", types=["FUTURE_TYPE"], side="FUTURE_SIDE")
    assert server.requests[0][1] == {"user": "0xuser", "type": "FUTURE_TYPE", "side": "FUTURE_SIDE"}


def test_a_walk_that_repeats_its_cursor_stops_with_the_base_error(server) -> None:
    server.reply("/v2/trades", page_of("trades", "same"))
    walk = polyoxide.DataApiSync(base_url=server.url).v2().iter_trades(cursor="same")

    with pytest.raises(polyoxide.PolyoxideError) as err:
        next(walk)

    assert type(err.value) is polyoxide.PolyoxideError
    assert "server returned the cursor it was sent" in str(err.value)
    assert err.value.code is None


def v2_error(code: str, **extra: object) -> dict:
    return {"error": f"{code} happened", "code": code, "retryable": code != "invalid_request", "trace_id": "t-1", **extra}


@pytest.mark.parametrize(
    ("status", "body", "headers", "error"),
    [
        (400, v2_error("invalid_request", parameter="user"), {}, polyoxide.ValidationError),
        (503, v2_error("request_timeout"), {"retry-after": "2"}, polyoxide.TimeoutError),
        (503, v2_error("dependency_unavailable"), {}, polyoxide.ApiError),
        (500, v2_error("brand_new_code"), {}, polyoxide.ApiError),
    ],
    ids=["invalid_request", "request_timeout", "dependency_unavailable", "unknown_code"],
)
def test_a_v2_error_body_maps_by_code_and_keeps_its_fields(server, status, body, headers, error) -> None:
    server.reply("/v2/user-pnl", body, status=status, headers=headers)

    with pytest.raises(error) as raised:
        polyoxide.DataApiSync(base_url=server.url).v2().user_pnl("0xuser")

    e = raised.value
    assert type(e) is error
    assert e.status == status
    assert e.code == ("unknown" if body["code"] == "brand_new_code" else body["code"])
    assert e.retryable is body["retryable"]
    assert e.trace_id == "t-1"
    assert e.parameter == body.get("parameter")
    assert e.retry_after == (2.0 if headers else None)
    assert "trace_id t-1" in str(e)


def test_rate_limited_is_a_rate_limit_error(server) -> None:
    # The client retries 429 three times with backoff first, so this takes ~4s.
    server.reply("/v2/status", v2_error("rate_limited"), status=429, headers={"retry-after": "0"})

    with pytest.raises(polyoxide.RateLimitError) as raised:
        polyoxide.DataApiSync(base_url=server.url).v2().status()

    assert raised.value.code == "rate_limited"
    assert raised.value.retryable is True
    assert len(server.requests) == 4


def test_async_error_carries_the_same_fields(server) -> None:
    server.reply("/v2/trades", v2_error("invalid_request", parameter="side"), status=400)

    async def call():
        await polyoxide.DataApi(base_url=server.url).v2().trades()

    with pytest.raises(polyoxide.ValidationError) as raised:
        asyncio.run(call())
    assert raised.value.parameter == "side"


def test_an_error_without_a_v2_body_has_none_fields(server) -> None:
    server.reply("/v2/status", "upstream exploded", status=500)

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.DataApiSync(base_url=server.url).v2().status()

    e = raised.value
    assert (e.status, e.code, e.retryable, e.trace_id, e.parameter, e.retry_after) == (None,) * 6


def test_v1_errors_also_carry_the_attributes(server) -> None:
    server.reply("/", "nope", status=500)

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.DataApiSync(base_url=server.url).health().ping()

    assert raised.value.code is None
