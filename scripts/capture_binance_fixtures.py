#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["websockets>=13", "certifi"]
# ///
"""Capture Binance USDⓈ-M REST responses and stream frames as test fixtures for
polyoxide-binance.

Usage: uv run scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures

Writes one JSON file per route under OUT_DIR/rest/, one combined-stream envelope per
stream kind under OUT_DIR/ws/, and rewrites OUT_DIR/PROVENANCE.md. Nothing is written
until every capture has succeeded, so a failed run leaves the fixtures as they were.
Every key is kept; only list lengths are trimmed, so a wire-agreement test sees each
field the server sends. No credentials. Costs 76 request weight, well inside the 2400
per minute, and two short WebSocket connections. Review the diff before committing.
"""
import asyncio
import json
import os
import sys
import time

from websockets.exceptions import ConnectionClosed, InvalidHandshake

import capture_common

BASE = "https://fapi.binance.com"
WS_BASE = "wss://fstream.binance.com"
STREAMS = {
    "market": ["!ticker@arr", "!markPrice@arr@1s", "btcusdt@aggTrade", "btcusdt@kline_1m",
               "btcusdt@markPrice@1s", "btcusdt@ticker"],
    "public": ["btcusdt@depth20@100ms", "btcusdt@bookTicker"],
}


def identity(_method, _url):
    """Asks for an uncompressed body."""
    return {"Accept-Encoding": "identity"}


def get(path, **params):
    reply = capture_common.get(f"{BASE}{path}", params=params, headers=identity, timeout=60)
    return capture_common.require_ok(reply, f"GET {path}")


def write(path, value):
    capture_common.write_json(path, value, indent=1)


def first(rows, predicate, what):
    for row in rows:
        if predicate(row):
            return row
    sys.exit(f"no {what} listed; pick the fixture rows by hand")


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


async def read_streams(path, streams):
    """The first envelope of each non-array stream on one connection, and for each
    array stream the latest envelope read and whether it carries a COIN-M row.

    An array stream is read until a frame carries a COIN-M row, within the 30 s: the
    first `!markPrice@arr@1s` frame of a second is often a partial one without them.
    """
    url = f"{WS_BASE}/{path}/stream?streams=" + "/".join(streams)
    envelopes, arrays, wanted = {}, {}, set(streams)
    try:
        async with capture_common.ws_session(url, max_size=2**22) as ws:
            deadline = time.time() + 30
            while (wanted or any(not coin_m for _, coin_m in arrays.values())) and time.time() < deadline:
                try:
                    message = await asyncio.wait_for(ws.recv(), 5)
                except TimeoutError:
                    continue
                if not isinstance(message, str):
                    continue
                envelope = json.loads(message)
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
    except InvalidHandshake as err:
        sys.exit(f"/{path}: handshake refused: {err}")
    except ConnectionClosed as err:
        sys.exit(f"/{path}: the server closed the connection: {err}")
    return envelopes, arrays, wanted


def capture_streams():
    """One envelope per stream, by file name, and the rows each array kept."""
    envelopes, kept = {}, {}
    for path, streams in STREAMS.items():
        found, arrays, wanted = asyncio.run(read_streams(path, streams))
        if wanted:
            sys.exit(f"no frame within 30 s on {sorted(wanted)}; run again")
        envelopes.update(found)
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

    today = capture_common.stamp("%Y-%m-%d")
    capture_common.write_provenance(out, f"""# Provenance

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
