"""Each error class raises its own exception, against a local server: no network.

The exception follows the error's class one to one (Story 3.12), so these
assert the exact type, through gamma and the Data API's v1 routes. A v2 error
body still maps by its code; `test_data_v2_offline.py` holds that.
"""

from __future__ import annotations

import json
import socket

import pytest

import polyoxide
from test_data_v2_offline import FakeDataApi


@pytest.fixture
def server():
    fake = FakeDataApi()
    yield fake
    fake.httpd.shutdown()
    fake.httpd.server_close()


def gamma_market(server: FakeDataApi) -> None:
    polyoxide.GammaSync(base_url=server.url).markets().get("1")


def data_ping(server: FakeDataApi) -> None:
    polyoxide.DataApiSync(base_url=server.url).health().ping()


# (status, body, the call, the exception it raises). A 429 is retried three
# times with backoff first, so its row takes about four seconds.
CASES = [
    (503, {"error": "down"}, gamma_market, polyoxide.UnavailableError),
    (500, {"error": "broke"}, data_ping, polyoxide.UnavailableError),
    (408, {"error": "slow"}, gamma_market, polyoxide.UnavailableError),
    (429, {"error": "slow down"}, data_ping, polyoxide.RateLimitError),
    (401, {"error": "who are you"}, gamma_market, polyoxide.AuthenticationError),
    (403, {"error": "forbidden"}, data_ping, polyoxide.AuthenticationError),
    (400, {"error": "bad limit"}, gamma_market, polyoxide.ApiError),
    (404, {"error": "not found"}, data_ping, polyoxide.ApiError),
    (451, {"error": "not here"}, gamma_market, polyoxide.RestrictedError),
    (418, {"error": "banned"}, data_ping, polyoxide.RestrictedError),
    (200, "not json", gamma_market, polyoxide.DecodeError),
]


@pytest.mark.parametrize(
    ("status", "body", "call", "error"),
    CASES,
    ids=[f"{status}-{call.__name__}" for status, _, call, _ in CASES],
)
def test_each_status_raises_its_class_s_exception(server, status, body, call, error) -> None:
    path = "/markets/1" if call is gamma_market else "/"
    server.reply(path, body if isinstance(body, str) else json.dumps(body), status=status)

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        call(server)

    e = raised.value
    assert type(e) is error, f"{status} raised {type(e).__name__}: {e}"
    # Not a v2 error, so the six attributes are all None.
    assert (e.status, e.code, e.retryable, e.trace_id, e.parameter, e.retry_after) == (None,) * 6


def test_a_message_that_names_another_class_does_not_change_the_type(server) -> None:
    # The type comes from the class, never from the text: a 404 whose body
    # talks of rate limits and timeouts is still a venue refusal.
    server.reply("/markets/1", json.dumps({"error": "Rate limit exceeded: 429 timeout Network"}), status=404)

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.GammaSync(base_url=server.url).markets().get("1")

    assert type(raised.value) is polyoxide.ApiError


def test_no_response_is_a_network_error() -> None:
    # Bind a port, then free it, so nothing is listening there.
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]

    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.GammaSync(base_url=f"http://127.0.0.1:{port}").markets().get("1")

    assert type(raised.value) is polyoxide.NetworkError


def test_a_base_url_that_does_not_parse_is_a_validation_error() -> None:
    with pytest.raises(polyoxide.PolyoxideError) as raised:
        polyoxide.GammaSync(base_url="not a url")

    assert type(raised.value) is polyoxide.ValidationError


def test_the_classes_are_one_exception_each() -> None:
    errors = [
        polyoxide.NetworkError,
        polyoxide.UnavailableError,
        polyoxide.RateLimitError,
        polyoxide.AuthenticationError,
        polyoxide.ValidationError,
        polyoxide.ApiError,
        polyoxide.RestrictedError,
        polyoxide.DecodeError,
    ]
    assert len(set(errors)) == 8
    for error in errors:
        assert error.__bases__ == (polyoxide.PolyoxideError,), error
    # Only a v2 `request_timeout` raises it, and the host is unavailable then.
    assert polyoxide.TimeoutError.__bases__ == (polyoxide.UnavailableError,)
