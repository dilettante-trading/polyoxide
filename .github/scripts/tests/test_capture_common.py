"""`scripts/capture_common.py`, the HTTP, file and WebSocket helpers every
fixture capture script shares.

Everything runs against servers on 127.0.0.1, so nothing here needs the
network. The `ws_session` tests need the `websockets` package, which this
project does not depend on, and are skipped without it:
`uv run --with websockets pytest tests/test_capture_common.py` runs them.
"""

from __future__ import annotations

import asyncio
import importlib.util
import json
import re
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

import pytest

REPO = Path(__file__).resolve().parents[3]


def _load_capture_common():
    """`scripts/capture_common.py`, which lives outside this uv project."""
    if "capture_common" in sys.modules:
        return sys.modules["capture_common"]
    spec = importlib.util.spec_from_file_location(
        "capture_common", REPO / "scripts" / "capture_common.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


capture_common = _load_capture_common()


# --- a local HTTP server --------------------------------------------------------


class _Handler(BaseHTTPRequestHandler):
    """`/echo` answers with the path, query and headers it received; `/html` is a
    CDN-style 503 page; `/text` a 200 that is not JSON; `/missing` a JSON 404."""

    def do_GET(self) -> None:  # noqa: N802 (the stdlib's name)
        route = urlsplit(self.path).path
        if route == "/echo":
            body = json.dumps({
                "path": self.path,
                "query": parse_qs(urlsplit(self.path).query),
                "headers": {k.lower(): v for k, v in self.headers.items()},
            }).encode()
            self._send(200, "application/json", body)
        elif route == "/html":
            self._send(503, "text/html", b"<html><body>503 Service Unavailable</body></html>")
        elif route == "/text":
            self._send(200, "text/plain", b"plainly not json")
        else:
            self._send(404, "application/json", b'{"error": "not found"}')

    def _send(self, status: int, content_type: str, body: bytes) -> None:
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args) -> None:
        pass


@pytest.fixture(scope="module")
def base() -> str:
    server = ThreadingHTTPServer(("127.0.0.1", 0), _Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    yield f"http://127.0.0.1:{server.server_address[1]}"
    server.shutdown()
    server.server_close()


# --- get --------------------------------------------------------------------------


def test_params_are_encoded_and_a_none_is_left_out(base: str) -> None:
    reply = capture_common.get(f"{base}/echo", params={"limit": 2, "user": "0xa b", "cursor": None})
    assert reply.status == 200 and reply.is_json
    assert reply.url == f"{base}/echo?limit=2&user=0xa+b"
    assert reply.body["query"] == {"limit": ["2"], "user": ["0xa b"]}


def test_no_params_leaves_the_url_bare(base: str) -> None:
    reply = capture_common.get(f"{base}/echo", params={"cursor": None})
    assert reply.url == f"{base}/echo"


def test_the_default_user_agent_reaches_the_server(base: str) -> None:
    reply = capture_common.get(f"{base}/echo")
    assert reply.body["headers"]["user-agent"] == "polyoxide-fixture-capture"
    assert capture_common.USER_AGENT == "polyoxide-fixture-capture"


def test_headers_are_asked_for_on_every_request_and_sent(base: str) -> None:
    calls = []

    def sign(method: str, url: str) -> dict[str, str]:
        calls.append((method, url))
        return {"x-signature": f"sig-{len(calls)}"}

    first = capture_common.get(f"{base}/echo", params={"n": 1}, headers=sign)
    second = capture_common.get(f"{base}/echo", params={"n": 2}, headers=sign)
    assert calls == [("GET", f"{base}/echo?n=1"), ("GET", f"{base}/echo?n=2")]
    assert first.body["headers"]["x-signature"] == "sig-1"
    assert second.body["headers"]["x-signature"] == "sig-2"
    assert first.body["headers"]["user-agent"] == "polyoxide-fixture-capture"


def test_pause_sleeps_before_the_request(base: str, monkeypatch: pytest.MonkeyPatch) -> None:
    slept = []
    monkeypatch.setattr(capture_common.time, "sleep", slept.append)
    capture_common.get(f"{base}/echo", pause=0.5)
    capture_common.get(f"{base}/echo")
    assert slept == [0.5]


def test_a_non_json_error_body_is_returned_not_raised(base: str) -> None:
    reply = capture_common.get(f"{base}/html")
    assert reply.status == 503
    assert not reply.is_json
    assert "503 Service Unavailable" in reply.body
    assert reply.headers["content-type"] == "text/html"


def test_a_json_error_body_is_parsed(base: str) -> None:
    reply = capture_common.get(f"{base}/missing")
    assert (reply.status, reply.is_json, reply.body) == (404, True, {"error": "not found"})


# --- require_ok -------------------------------------------------------------------


def test_require_ok_returns_a_200_json_body(base: str) -> None:
    body = capture_common.require_ok(capture_common.get(f"{base}/echo"))
    assert body["path"] == "/echo"


def test_require_ok_ends_the_run_on_an_error_status(base: str) -> None:
    with pytest.raises(SystemExit) as exit_:
        capture_common.require_ok(capture_common.get(f"{base}/missing"), "trades")
    assert str(exit_.value) == f'trades: HTTP 404 from {base}/missing: {{"error": "not found"}}'


def test_require_ok_names_nothing_without_what(base: str) -> None:
    with pytest.raises(SystemExit) as exit_:
        capture_common.require_ok(capture_common.get(f"{base}/html"))
    assert str(exit_.value).startswith(f"HTTP 503 from {base}/html: <html>")


def test_require_ok_ends_the_run_on_a_200_that_is_not_json(base: str) -> None:
    with pytest.raises(SystemExit) as exit_:
        capture_common.require_ok(capture_common.get(f"{base}/text"), "status")
    assert str(exit_.value) == (
        f"status: HTTP 200 from {base}/text, but the body is not JSON: plainly not json")


def test_require_ok_truncates_a_long_body() -> None:
    reply = capture_common.Reply("http://x", 500, "y" * 2000, False, {})
    with pytest.raises(SystemExit) as exit_:
        capture_common.require_ok(reply)
    assert str(exit_.value) == "HTTP 500 from http://x: " + "y" * 500 + "…"


# --- writers ------------------------------------------------------------------------


def test_write_json_indents_keeps_unicode_and_ends_in_a_newline(tmp_path: Path) -> None:
    path = capture_common.write_json(tmp_path / "a" / "b.json", {"k": ["USDⓈ-M", 1]})
    assert path.read_bytes() == '{\n  "k": [\n    "USDⓈ-M",\n    1\n  ]\n}\n'.encode()


def test_write_json_takes_an_indent_and_can_escape(tmp_path: Path) -> None:
    capture_common.write_json(tmp_path / "c.json", {"k": "USDⓈ-M"}, indent=1, ensure_ascii=True)
    assert (tmp_path / "c.json").read_bytes() == b'{\n "k": "USD\\u24c8-M"\n}\n'


def test_write_raw_writes_exactly_what_it_is_given(tmp_path: Path) -> None:
    capture_common.write_raw(tmp_path / "d" / "frame.json", '{"a":"Ⓢ"}')
    capture_common.write_raw(tmp_path / "bytes.bin", b"\x00\r\n")
    assert (tmp_path / "d" / "frame.json").read_bytes() == '{"a":"Ⓢ"}'.encode()
    assert (tmp_path / "bytes.bin").read_bytes() == b"\x00\r\n"


def test_provenance_table_rules_match_each_heading() -> None:
    assert capture_common.provenance_table(("Fixture", "Request"), [("`a.json`", "`url`")]) == [
        "| Fixture | Request |",
        "|---------|---------|",
        "| `a.json` | `url` |",
    ]
    assert capture_common.provenance_table(("Fixture", "Label or request"), []) == [
        "| Fixture | Label or request |",
        "|---------|------------------|",
    ]


def test_provenance_table_short_rule() -> None:
    assert capture_common.provenance_table(
        ("League", "Frames"), [("nfl", "3"), ("mlb", "1")], short_rule=True) == [
        "| League | Frames |",
        "|---|---|",
        "| nfl | 3 |",
        "| mlb | 1 |",
    ]


def test_write_provenance_joins_lines_with_a_final_newline(tmp_path: Path) -> None:
    path = capture_common.write_provenance(tmp_path / "out", ["# Title", "", "body"])
    assert path == tmp_path / "out" / "PROVENANCE.md"
    assert path.read_bytes() == b"# Title\n\nbody\n"


def test_write_provenance_writes_a_string_as_it_is(tmp_path: Path) -> None:
    capture_common.write_provenance(tmp_path, "# Title — no newline")
    assert (tmp_path / "PROVENANCE.md").read_bytes() == "# Title — no newline".encode()


def test_stamp_is_utc_in_the_format_asked_for() -> None:
    assert re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ", capture_common.stamp())
    assert re.fullmatch(r"\d{4}-\d\d-\d\d", capture_common.stamp("%Y-%m-%d"))


# --- ws_session ---------------------------------------------------------------------


def _serve(handler):
    """A `websockets` server on 127.0.0.1 running `handler`, as an async context."""
    from websockets.asyncio.server import serve

    return serve(handler, "127.0.0.1", 0)


def _url(server) -> str:
    return f"ws://127.0.0.1:{server.sockets[0].getsockname()[1]}"


def test_ws_session_sends_the_headers_it_is_given() -> None:
    pytest.importorskip("websockets")
    calls = []

    async def echo_header(connection) -> None:
        await connection.send(connection.request.headers.get("x-signature", "none"))

    def sign(method: str, url: str) -> dict[str, str]:
        calls.append((method, url))
        return {"x-signature": "signed"}

    async def run() -> tuple[str, str]:
        async with _serve(echo_header) as server:
            url = _url(server)
            async with capture_common.ws_session(url, headers=sign) as ws:
                return url, await ws.recv()

    url, received = asyncio.run(run())
    assert received == "signed"
    assert calls == [("GET", url)]


def test_ws_session_uses_the_connection_class_it_is_given() -> None:
    pytest.importorskip("websockets")
    from websockets.asyncio.client import ClientConnection

    class Marked(ClientConnection):
        pass

    async def greet(connection) -> None:
        await connection.send("hello")

    async def run():
        async with _serve(greet) as server:
            async with capture_common.ws_session(_url(server), create_connection=Marked) as ws:
                return type(ws), await ws.recv()

    kind, received = asyncio.run(run())
    assert kind is Marked
    assert received == "hello"


def test_ws_session_refuses_a_message_over_max_size() -> None:
    pytest.importorskip("websockets")
    from websockets.exceptions import ConnectionClosedError

    async def flood(connection) -> None:
        await connection.send("x" * 2048)
        await connection.wait_closed()

    async def run():
        async with _serve(flood) as server:
            async with capture_common.ws_session(_url(server), max_size=1024) as ws:
                with pytest.raises(ConnectionClosedError) as closed:
                    await ws.recv()
                return closed.value

    closed = asyncio.run(run())
    assert closed.sent is not None and closed.sent.code == 1009
