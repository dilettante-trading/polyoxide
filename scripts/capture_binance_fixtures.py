#!/usr/bin/env python3
"""Capture Binance USDⓈ-M REST responses and stream frames as test fixtures for
polyoxide-binance.

Usage: python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures

Writes one JSON file per route under OUT_DIR/rest/, one combined-stream envelope per
stream kind under OUT_DIR/ws/, and rewrites OUT_DIR/PROVENANCE.md. Nothing is written
until every capture has succeeded, so a failed run leaves the fixtures as they were.
Every key is kept; only list lengths are trimmed, so a wire-agreement test sees each
field the server sends. Stdlib only, no credentials. Costs 76 request weight, well
inside the 2400 per minute, and two short WebSocket connections. Review the diff before
committing.
"""
import base64
import datetime
import json
import os
import socket
import ssl
import struct
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

BASE = "https://fapi.binance.com"
WS_HOST = "fstream.binance.com"
STREAMS = {
    "market": ["!ticker@arr", "!markPrice@arr@1s", "btcusdt@aggTrade", "btcusdt@kline_1m",
               "btcusdt@markPrice@1s", "btcusdt@ticker"],
    "public": ["btcusdt@depth20@100ms", "btcusdt@bookTicker"],
}


def get(path, **params):
    query = urllib.parse.urlencode(params)
    url = f"{BASE}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(url, headers={"Accept-Encoding": "identity"})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as err:
        body = err.read().decode("utf-8", "replace")[:300]
        sys.exit(f"GET {url}: {err.code} {err.reason}: {body}")


def write(path, value):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=1)
        f.write("\n")


def first(rows, predicate, what):
    for row in rows:
        if predicate(row):
            return row
    sys.exit(f"no {what} listed; pick the fixture rows by hand")


def ws_open(path):
    raw = socket.create_connection((WS_HOST, 443), timeout=15)
    sock = ssl.create_default_context().wrap_socket(raw, server_hostname=WS_HOST)
    key = base64.b64encode(os.urandom(16)).decode()
    sock.sendall((f"GET {path} HTTP/1.1\r\nHost: {WS_HOST}\r\nUpgrade: websocket\r\n"
                  f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
                  f"Sec-WebSocket-Version: 13\r\n\r\n").encode())
    head = b""
    while b"\r\n\r\n" not in head:
        byte = sock.recv(1)
        if not byte:
            sys.exit(f"{path}: the server closed the connection during the handshake")
        head += byte
    status = head.split(b"\r\n")[0]
    if b" 101 " not in status:
        sys.exit(f"{path}: handshake refused: {status.decode()}")
    return sock


def read_exact(sock, n):
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise EOFError("the server closed the connection")
        buf += chunk
    return buf


def read_message(sock):
    """One message as (opcode, payload), joining continuation frames."""
    opcode, payload = None, b""
    while True:
        b1, b2 = read_exact(sock, 2)
        length = b2 & 0x7F
        if length == 126:
            length = struct.unpack(">H", read_exact(sock, 2))[0]
        elif length == 127:
            length = struct.unpack(">Q", read_exact(sock, 8))[0]
        if b2 & 0x80:
            read_exact(sock, 4)
        data = read_exact(sock, length)
        if b1 & 0x0F:
            opcode = b1 & 0x0F
        payload += data
        if b1 & 0x80:
            return opcode, payload


def pick_two(name, rows):
    """The first USDⓈ-M row (`st: 1`) with a scheduled funding time (`T > 0`; ticker
    rows have no `T`, so there the first USDⓈ-M row), then the first COIN-M row
    (`st: 2`), or the next row when the frame has none."""
    first_row = next((r for r in rows if r.get("st") == 1 and r.get("T", 1) > 0), None)
    if first_row is None:
        sys.exit(f"{name}: no USDⓈ-M row with a funding time; pick the fixture rows by hand")
    second = next((r for r in rows if r.get("st") == 2), None)
    if second is None:
        second = next((r for r in rows if r is not first_row), None)
    if second is None:
        sys.exit(f"{name}: a frame of one row; run again")
    return [first_row, second]


def capture_streams():
    """One envelope per stream, by file name, and the rows each array kept.

    An array stream is read until a frame carries a COIN-M row, within the 30 s: the
    first `!markPrice@arr@1s` frame of a second is often a partial one without them.
    """
    envelopes, kept = {}, {}
    for path, streams in STREAMS.items():
        sock = ws_open(f"/{path}/stream?streams=" + "/".join(streams))
        sock.settimeout(5)
        wanted, arrays = set(streams), {}
        deadline = time.time() + 30
        while (wanted or any(not coin_m for _, coin_m in arrays.values())) and time.time() < deadline:
            try:
                opcode, data = read_message(sock)
            except (socket.timeout, TimeoutError):
                continue
            if opcode != 1:
                continue
            envelope = json.loads(data.decode("utf-8"))
            name = envelope.get("stream")
            if isinstance(envelope.get("data"), list):
                if name in wanted or (name in arrays and not arrays[name][1]):
                    wanted.discard(name)
                    has_coin_m = any(r.get("st") == 2 for r in envelope["data"])
                    arrays[name] = (envelope, has_coin_m)
                continue
            if name not in wanted:
                continue
            wanted.discard(name)
            if "@depth" in name:
                envelope["data"]["b"] = envelope["data"]["b"][:3]
                envelope["data"]["a"] = envelope["data"]["a"][:3]
            envelopes[name] = envelope
        sock.close()
        if wanted:
            sys.exit(f"no frame within 30 s on {sorted(wanted)}; run again")
        for name, (envelope, _) in arrays.items():
            envelope["data"] = pick_two(name, envelope["data"])
            kept[name] = [(r["s"], r["st"]) for r in envelope["data"]]
            envelopes[name] = envelope
    files = {"stream_" + n.replace("!", "all_").replace("@", "_") + ".json": e
             for n, e in envelopes.items()}
    return files, kept


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    rest = {}

    info = get("/fapi/v1/exchangeInfo")
    symbols = info["symbols"]
    trading = [s for s in symbols if s["status"] == "TRADING"]
    chinese = first(trading, lambda s: not s["symbol"].isascii(), "Chinese-character symbol")
    tradfi = first(trading, lambda s: s["contractType"] == "TRADIFI_PERPETUAL", "TradFi perpetual")
    settling = first(symbols, lambda s: s["status"] == "SETTLING", "SETTLING contract")
    quarterly = first(trading, lambda s: s["contractType"] == "CURRENT_QUARTER", "quarterly")
    picked = [first(trading, lambda s: s["symbol"] == "BTCUSDT", "BTCUSDT"), tradfi, chinese, settling, quarterly]
    names = [s["symbol"] for s in picked]
    trimmed = dict(info)
    trimmed["assets"] = info["assets"][:2]
    trimmed["symbols"] = picked
    rest["exchange_info.json"] = trimmed

    wanted = {"BTCUSDT", tradfi["symbol"], chinese["symbol"]}
    rest["time.json"] = get("/fapi/v1/time")
    rest["ticker_24hr.json"] = [t for t in get("/fapi/v1/ticker/24hr") if t["symbol"] in wanted]
    rest["premium_index.json"] = [p for p in get("/fapi/v1/premiumIndex") if p["symbol"] in wanted]
    funding_info = get("/fapi/v1/fundingInfo")
    rows = [f for f in funding_info if f["symbol"] in wanted]
    if not any(f["updateTime"] is None for f in rows):
        rows.append(first(funding_info, lambda f: f["updateTime"] is None, "fundingInfo row with null updateTime"))
    rest["funding_info.json"] = rows
    rest["klines.json"] = get("/fapi/v1/klines", symbol="BTCUSDT", interval="1m", limit=2)
    rest["funding_rate.json"] = get("/fapi/v1/fundingRate", symbol="BTCUSDT", limit=3)
    # The first funding events, from before markPrice was recorded: it is "" on the wire.
    rest["funding_rate_2019.json"] = get(
        "/fapi/v1/fundingRate", symbol="BTCUSDT", startTime=1568102400000, limit=2)
    rest["open_interest.json"] = get("/fapi/v1/openInterest", symbol="BTCUSDT")
    rest["agg_trades.json"] = get("/fapi/v1/aggTrades", symbol="BTCUSDT", limit=3)
    rest["depth.json"] = get("/fapi/v1/depth", symbol="BTCUSDT", limit=5)
    stream_files, kept = capture_streams()

    for name, value in rest.items():
        write(os.path.join(out, "rest", name), value)
    for name, value in stream_files.items():
        write(os.path.join(out, "ws", name), value)
    kept_text = "; ".join(
        f"`{name}` kept " + " and ".join(f"`{s}` (`st: {st}`)" for s, st in rows)
        for name, rows in sorted(kept.items()))

    today = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    with open(os.path.join(out, "PROVENANCE.md"), "w", encoding="utf-8") as f:
        f.write(f"""# Provenance

REST fixtures (`rest/`) captured {today} from `https://fapi.binance.com` by
`scripts/capture_binance_fixtures.py`. No credentials. Every top-level key is kept; only
list lengths are trimmed.

| File | Request | Trimmed to |
|---|---|---|
| `exchange_info.json` | `GET /fapi/v1/exchangeInfo` | `symbols`: {", ".join(f"`{n}`" for n in names)} (BTCUSDT, a TradFi perpetual, a Chinese-character perpetual, the first `SETTLING` contract, the first `CURRENT_QUARTER`); `assets`: the first two |
| `time.json` | `GET /fapi/v1/time` | as fetched |
| `ticker_24hr.json` | `GET /fapi/v1/ticker/24hr` | {", ".join(f"`{n}`" for n in sorted(wanted))} |
| `premium_index.json` | `GET /fapi/v1/premiumIndex` | the same three |
| `funding_info.json` | `GET /fapi/v1/fundingInfo` | the same three, plus a row with `updateTime: null` if none of them has one |
| `klines.json` | `GET /fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=2` | as fetched |
| `funding_rate.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&limit=3` | as fetched |
| `funding_rate_2019.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&startTime=1568102400000&limit=2` | as fetched; `markPrice` is `""` |
| `open_interest.json` | `GET /fapi/v1/openInterest?symbol=BTCUSDT` | as fetched |
| `agg_trades.json` | `GET /fapi/v1/aggTrades?symbol=BTCUSDT&limit=3` | as fetched |
| `depth.json` | `GET /fapi/v1/depth?symbol=BTCUSDT&limit=5` | as fetched |

## Streams (`ws/`)

Captured {today} from `wss://fstream.binance.com` by the same script: one combined-stream
envelope (`{{"stream", "data"}}`) per stream, from `/market/stream` for `!ticker@arr`,
`!markPrice@arr@1s`, `btcusdt@aggTrade`, `btcusdt@kline_1m`, `btcusdt@markPrice@1s` and
`btcusdt@ticker`, and from `/public/stream` for `btcusdt@depth20@100ms` and
`btcusdt@bookTicker`. An array stream is read until a frame carries a COIN-M row, within
30 s, and keeps two of its rows: the first USDⓈ-M row (`st: 1`) with a scheduled funding
time (ticker rows have no `T`, so there the first USDⓈ-M row), then the first COIN-M row
(`st: 2`), or the next row when no frame had one. This run: {kept_text}. Depth sides keep
three levels. Files: {", ".join(f"`{f}`" for f in sorted(stream_files))}.

## Probes

`docs/specs/binance/probes/` holds the scripts behind the design spec's measured facts:
`probe_rest.py` (weights from `X-MBX-USED-WEIGHT-1M` deltas), `probe_ws.py` and
`probe_ws2.py` (acknowledgements, case, the 1024 cap, the message rate),
`probe_ws_ping.py` (server ping cadence), and `wsprobe.py` (the frame reader they share).
""")
    print("captured:", ", ".join(sorted(rest)), "and", ", ".join(sorted(stream_files)))


if __name__ == "__main__":
    main()
