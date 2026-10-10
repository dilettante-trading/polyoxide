"""What the fixture capture scripts share: one HTTP GET, the fixture and
PROVENANCE.md writers, and a WebSocket session.

Each `scripts/capture_*.py` imports this module from its own directory, so run
them as files: `python3 scripts/capture_x.py …`, or `uv run scripts/capture_x.py …`
for one that declares its dependencies inline. Never `python3 -I`, which leaves
the script's own directory off `sys.path`, so this import fails.

Signing stays in each script. `get` and `ws_session` take a `headers(method, url)`
function and call it for every request or connection, so a signature computed
from the time or the URL is never reused.

Only the standard library is imported at load time. `ws_session` imports
`websockets` when it is called, so a script that only reads HTTP does not need
it. Certificates are checked against `certifi`'s roots when that package is
installed, as it is wherever a script declares it, and against the system's
otherwise: a Python that `uv` installed may not find the system's roots.
"""

from __future__ import annotations

import json
import ssl
import time
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable, Iterable, Mapping, Sequence
from contextlib import asynccontextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

USER_AGENT = "polyoxide-fixture-capture"

# How much of a refused body a failure message quotes.
EXCERPT_CHARS = 500

Headers = Callable[[str, str], Mapping[str, str]]


@dataclass(frozen=True)
class Reply:
    """One response: the URL requested, its status, body and headers. `body` is
    the parsed JSON, or the text when the body is not JSON (`is_json` says which)."""

    url: str
    status: int
    body: Any
    is_json: bool
    headers: Mapping[str, str]


def _tls() -> ssl.SSLContext:
    """A client TLS context over `certifi`'s roots when it is installed, else
    over the system's."""
    try:
        import certifi
    except ImportError:
        return ssl.create_default_context()
    return ssl.create_default_context(cafile=certifi.where())


def _decode(raw: bytes) -> tuple[Any, bool]:
    try:
        return json.loads(raw), True
    except ValueError:
        return raw.decode("utf-8", "replace"), False


def get(
    url: str,
    *,
    params: Mapping[str, Any] | None = None,
    headers: Headers | None = None,
    pause: float = 0.0,
    ua: str = USER_AGENT,
    timeout: float = 30,
) -> Reply:
    """GET `url` with `params` (a `None` value is left out), after sleeping
    `pause` seconds so a run spaces its requests out.

    `headers("GET", full_url)` is called for this request, and what it returns
    is sent alongside the `user-agent`. An error status is returned, not raised,
    and so is a body that is not JSON, such as a CDN's HTML error page.
    """
    query = urllib.parse.urlencode({k: v for k, v in (params or {}).items() if v is not None})
    if query:
        url = f"{url}?{query}"
    if pause:
        time.sleep(pause)
    sent = {"user-agent": ua}
    if headers is not None:
        sent.update(headers("GET", url))
    request = urllib.request.Request(url, headers=sent)
    try:
        with urllib.request.urlopen(request, timeout=timeout, context=_tls()) as response:
            body, is_json = _decode(response.read())
            return Reply(url, response.status, body, is_json, dict(response.headers))
    except urllib.error.HTTPError as err:
        body, is_json = _decode(err.read())
        return Reply(url, err.code, body, is_json, dict(err.headers or {}))


def _excerpt(body: Any) -> str:
    text = body if isinstance(body, str) else json.dumps(body, ensure_ascii=False)
    return text if len(text) <= EXCERPT_CHARS else text[:EXCERPT_CHARS] + "…"


def require_ok(reply: Reply, what: str | None = None) -> Any:
    """The body of a 200 JSON reply; anything else ends the run, naming `what`."""
    prefix = f"{what}: " if what else ""
    if reply.status != 200:
        raise SystemExit(f"{prefix}HTTP {reply.status} from {reply.url}: {_excerpt(reply.body)}")
    if not reply.is_json:
        raise SystemExit(f"{prefix}HTTP 200 from {reply.url}, but the body is not JSON: "
                         f"{_excerpt(reply.body)}")
    return reply.body


def write_raw(path: str | Path, data: str | bytes) -> Path:
    """Writes `data` to `path` exactly: bytes as given, text as UTF-8, with no
    newline translation and nothing appended. Creates the parent directories."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data if isinstance(data, bytes) else data.encode("utf-8"))
    return path


def write_json(path: str | Path, value: Any, *, indent: int = 2, ensure_ascii: bool = False) -> Path:
    """Writes `value` as JSON with `indent` and a trailing newline."""
    return write_raw(path, json.dumps(value, indent=indent, ensure_ascii=ensure_ascii) + "\n")


def stamp(fmt: str = "%Y-%m-%dT%H:%M:%SZ") -> str:
    """The current UTC time in `fmt`."""
    return datetime.now(timezone.utc).strftime(fmt)


def provenance_table(
    headers: Sequence[str], rows: Iterable[Sequence[str]], *, short_rule: bool = False
) -> list[str]:
    """A Markdown table as lines. The rule under the header is as wide as each
    heading, or `---` per column with `short_rule`."""
    rule = ["---" if short_rule else "-" * (len(h) + 2) for h in headers]
    lines = ["| " + " | ".join(headers) + " |", "|" + "|".join(rule) + "|"]
    lines += ["| " + " | ".join(row) + " |" for row in rows]
    return lines


def write_provenance(out_dir: str | Path, text: str | Iterable[str]) -> Path:
    """Writes `out_dir/PROVENANCE.md`: a string as it is, or lines joined with
    newlines and ending in one."""
    if not isinstance(text, str):
        text = "\n".join(text) + "\n"
    return write_raw(Path(out_dir) / "PROVENANCE.md", text)


@asynccontextmanager
async def ws_session(
    url: str,
    *,
    headers: Headers | None = None,
    max_size: int | None = 2**20,
    ping_interval: float | None = 20,
    create_connection: type | None = None,
):
    """An open WebSocket connection to `url`, closed on exit.

    A `wss://` URL's certificate is checked as `get` checks one.
    `headers("GET", url)` is sent with the handshake, and `create_connection`
    replaces the connection class, for a script that watches protocol frames.
    """
    from websockets.asyncio.client import connect

    options: dict[str, Any] = {"max_size": max_size, "ping_interval": ping_interval}
    if url.startswith("wss://"):
        options["ssl"] = _tls()
    if headers is not None:
        options["additional_headers"] = dict(headers("GET", url))
    if create_connection is not None:
        options["create_connection"] = create_connection
    async with connect(url, **options) as ws:
        yield ws
