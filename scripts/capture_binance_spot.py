#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["websockets>=13", "certifi"]
# ///
"""Capture Binance spot REST responses and stream frames as test fixtures for
polyoxide-binance's `spot` module.

Usage: python3 scripts/capture_binance_spot.py rest [OUT_DIR]
       uv run scripts/capture_binance_spot.py ws [OUT_DIR]
       (`rest` needs only the standard library, so `python3` runs it; `ws` imports
       `websockets`, so run it with `uv run`)

OUT_DIR defaults to polyoxide-binance/tests/fixtures/spot. `rest` writes one JSON file per
route under OUT_DIR/rest/, and `ws` one combined-stream envelope per stream under OUT_DIR/ws/;
neither writes anything else, and neither writes PROVENANCE.md. Each prints PROVENANCE.md's
section for what it captured (`## REST (rest/)`, `## Streams (ws/)`) to stdout, and progress
to stderr. Nothing is written until every capture has succeeded, so a failed run leaves the
fixtures as they were. Every key is kept; only list lengths are trimmed, so a wire-agreement
test sees each field the server sends. No credentials. `rest` costs about 180 request weight,
well inside the 6000 per minute. `ws` costs 20 (one `exchangeInfo`) and one WebSocket
connection that sends one request, far under spot's 5 messages a second. Review the diff
before committing.

Run it as a file, never `python3 -I`, which keeps scripts/ off sys.path.
"""
import argparse
import asyncio
import json
import sys
import time
import urllib.parse
from pathlib import Path

import capture_common

BASE = "https://api.binance.com"
DEFAULT_OUT = Path(__file__).resolve().parent.parent / "polyoxide-binance" / "tests" / "fixtures" / "spot"
SYMBOL = "BTCUSDT"

# (file, path, query, how the answer is trimmed), in the order PROVENANCE.md lists them.
REQUESTS = [
    ("time.json", "/api/v3/time", {}, None),
    ("exchange_info.json", "/api/v3/exchangeInfo", {}, "exchange"),
    ("depth.json", "/api/v3/depth", {"symbol": SYMBOL, "limit": 5}, None),
    ("trades.json", "/api/v3/trades", {"symbol": SYMBOL, "limit": 3}, None),
    ("agg_trades.json", "/api/v3/aggTrades", {"symbol": SYMBOL, "limit": 3}, None),
    ("klines.json", "/api/v3/klines", {"symbol": SYMBOL, "interval": "1m", "limit": 2}, None),
    ("klines_1s.json", "/api/v3/klines", {"symbol": SYMBOL, "interval": "1s", "limit": 2}, None),
    ("avg_price.json", "/api/v3/avgPrice", {"symbol": SYMBOL}, None),
    ("ticker_24hr.json", "/api/v3/ticker/24hr", {"symbol": SYMBOL}, None),
    ("tickers_24hr.json", "/api/v3/ticker/24hr", {}, "symbols"),
    ("price.json", "/api/v3/ticker/price", {"symbol": SYMBOL}, None),
    ("prices.json", "/api/v3/ticker/price", {}, "symbols"),
    ("book_ticker.json", "/api/v3/ticker/bookTicker", {"symbol": SYMBOL}, None),
    ("book_tickers.json", "/api/v3/ticker/bookTicker", {}, "symbols"),
]
# The one refused request: an unknown symbol, answered 400 with the venue's error body.
INVALID = ("error_invalid_symbol.json", "/api/v3/exchangeInfo", {"symbol": "NOPEUSDT"})

WS_BASE = "wss://stream.binance.com:9443"
# How long `ws` waits for the first frame of every stream, in seconds.
WS_WAIT = 30
# (file, wire name, how the frame is trimmed), in the order PROVENANCE.md lists them. The
# last name takes the non-ASCII symbol `exchangeInfo` lists, through `stream_list`.
STREAMS = [
    ("stream_btcusdt_aggTrade.json", "btcusdt@aggTrade", None),
    ("stream_btcusdt_trade.json", "btcusdt@trade", None),
    ("stream_btcusdt_kline_1m.json", "btcusdt@kline_1m", None),
    ("stream_btcusdt_miniTicker.json", "btcusdt@miniTicker", None),
    ("stream_all_miniTicker_arr.json", "!miniTicker@arr", "rows"),
    ("stream_btcusdt_ticker.json", "btcusdt@ticker", None),
    ("stream_btcusdt_bookTicker.json", "btcusdt@bookTicker", None),
    ("stream_btcusdt_depth5.json", "btcusdt@depth5", None),
    ("stream_btcusdt_depth20_100ms.json", "btcusdt@depth20@100ms", "levels"),
    ("stream_btcusdt_avgPrice.json", "btcusdt@avgPrice", None),
    ("stream_non_ascii_miniTicker.json", "{non_ascii}@miniTicker", None),
]
# What PROVENANCE.md's "Trimmed to" column says of each way a frame is trimmed.
TRIMMED = {
    None: "as fetched",
    "rows": "`data`: the first two rows",
    "levels": "`bids` and `asks`: the first three levels each",
}


def log(text):
    print(text, file=sys.stderr, flush=True)


def identity(_method, _url):
    """Asks for an uncompressed body."""
    return {"Accept-Encoding": "identity"}


def reply(path, **params):
    return capture_common.get(f"{BASE}{path}", params=params, headers=identity, pause=0.2, timeout=60)


def get(path, **params):
    return capture_common.require_ok(reply(path, **params), f"GET {path}")


def first(rows, predicate, what):
    for row in rows:
        if predicate(row):
            return row
    sys.exit(f"no {what} listed; pick the fixture rows by hand")


def pick_symbols(info):
    """The `exchangeInfo` rows to keep, each with why: BTCUSDT, the first symbol whose
    status is not TRADING, the first with a non-ASCII character if one is listed, then, in
    listing order, each symbol holding a filter type none of the rows kept so far holds,
    until every filter type listed anywhere is held."""
    symbols = info["symbols"]
    picked = []

    def add(row, why):
        if not any(row is kept for kept, _ in picked):
            picked.append((row, why))

    add(first(symbols, lambda s: s["symbol"] == SYMBOL, SYMBOL), SYMBOL)
    halted = first(symbols, lambda s: s["status"] != "TRADING", "symbol whose status is not TRADING")
    add(halted, f"the first not `TRADING`, status `{halted['status']}`")
    odd = next((s for s in symbols if not s["symbol"].isascii()), None)
    if odd is not None:
        add(odd, "the first with a non-ASCII character")
    held = {f["filterType"] for row, _ in picked for f in row["filters"]}
    for row in symbols:
        types = {f["filterType"] for f in row["filters"]}
        new = types - held
        if new:
            add(row, "the first with " + ", ".join(f"`{t}`" for t in sorted(new)))
            held |= types
    return picked


def wanted_symbols(non_ascii):
    """The symbols a list response keeps: BTCUSDT, ETHUSDT and the non-ASCII symbol of
    `exchangeInfo`, if one is listed."""
    return {SYMBOL, "ETHUSDT"} | ({non_ascii} if non_ascii else set())


def keep_symbols(rows, wanted, what):
    """The rows of a list response whose symbol is wanted, in the response's order.
    BTCUSDT and ETHUSDT must be there."""
    if not isinstance(rows, list):
        sys.exit(f"{what}: expected a list, got {type(rows).__name__}")
    kept = [row for row in rows if row["symbol"] in wanted]
    held = {row["symbol"] for row in kept}
    for needed in (SYMBOL, "ETHUSDT"):
        if needed not in held:
            sys.exit(f"{what}: {needed} is missing from the response")
    return kept


def provenance_section(today, rows):
    """PROVENANCE.md's `## REST (rest/)` section, from table rows of cells."""
    lines = [
        "## REST (rest/)",
        "",
        f"Captured {today} from `{BASE}` by `scripts/capture_binance_spot.py rest`. No credentials. "
        "Every key is kept; only list lengths are trimmed.",
        "",
    ]
    lines += capture_common.provenance_table(["File", "Request", "Trimmed to"], rows, short_rule=True)
    return "\n".join(lines) + "\n"


def capture_rest():
    """Every fixture by file name, and PROVENANCE.md's table rows."""
    files, rows, non_ascii = {}, [], None
    for name, path, query, trim in REQUESTS:
        value = get(path, **query)
        request = "GET " + path + ("?" + urllib.parse.urlencode(query) if query else "")
        if trim == "exchange":
            picked = pick_symbols(value)
            non_ascii = next((s["symbol"] for s in value["symbols"] if not s["symbol"].isascii()), None)
            value = {**value, "symbols": [row for row, _ in picked]}
            trimmed = "`symbols`: " + ", ".join(f"`{row['symbol']}` ({why})" for row, why in picked)
        elif trim == "symbols":
            value = keep_symbols(value, wanted_symbols(non_ascii), name)
            trimmed = ", ".join(f"`{row['symbol']}`" for row in value)
        else:
            trimmed = "as fetched"
        files[name] = value
        rows.append([f"`{name}`", f"`{request}`", trimmed])
        log(f"  {request}")
    name, path, query = INVALID
    answer = reply(path, **query)
    if answer.status != 400 or not answer.is_json or not isinstance(answer.body, dict):
        sys.exit(f"GET {path}?symbol={query['symbol']}: expected a 400 with a JSON object, "
                 f"got HTTP {answer.status} from {answer.url}")
    files[name] = answer.body
    rows.append([
        f"`{name}`",
        f"`GET {path}?symbol={query['symbol']}`",
        f"the body of the `{answer.status}` reply (`code` {answer.body.get('code')}), as fetched",
    ])
    log(f"  GET {path}?symbol={query['symbol']} -> {answer.status}")
    return files, rows


def wire_symbol(symbol):
    """A symbol as a stream name spells it: ASCII letters lowercased, every other character
    as it is (`str.lower` would also rewrite some non-ASCII ones)."""
    return "".join(c.lower() if c.isascii() else c for c in symbol)


def trading_non_ascii(info):
    """The first `TRADING` symbol of an `exchangeInfo` answer with a non-ASCII character."""
    row = first(
        info["symbols"],
        lambda s: s["status"] == "TRADING" and not s["symbol"].isascii(),
        "TRADING symbol with a non-ASCII character",
    )
    return row["symbol"]


def stream_list(non_ascii):
    """`STREAMS` with the non-ASCII stream's name filled in from the symbol."""
    return [(file, name.format(non_ascii=wire_symbol(non_ascii)), how) for file, name, how in STREAMS]


def trim(how, data, name):
    """`data` trimmed the way `how` says; a frame too short to trim ends the run."""
    if how == "rows":
        if not isinstance(data, list) or len(data) < 2:
            sys.exit(f"{name}: expected an array of at least two rows to keep two of")
        return data[:2]
    if how == "levels":
        return {**data, "bids": data["bids"][:3], "asks": data["asks"][:3]}
    return data


async def read_streams(names):
    """Subscribes to `names` on one connection and reads until the first envelope of each has
    arrived or `WS_WAIT` seconds have passed. Returns the envelopes by stream name, the names
    still missing, every stream name seen, and the text of the answer to the `SUBSCRIBE`."""
    from websockets.exceptions import ConnectionClosed, InvalidHandshake

    url = f"{WS_BASE}/stream"
    found, wanted, seen, answer = {}, set(names), set(), None
    try:
        async with capture_common.ws_session(url, max_size=2**22, ping_interval=None) as ws:
            await ws.send(json.dumps({"method": "SUBSCRIBE", "params": names, "id": 1}))
            deadline = time.monotonic() + WS_WAIT
            while wanted:
                left = deadline - time.monotonic()
                if left <= 0:
                    break
                try:
                    message = await asyncio.wait_for(ws.recv(), left)
                except TimeoutError:
                    break
                if not isinstance(message, str):
                    continue
                frame = json.loads(message)
                if "stream" not in frame:
                    if "error" in frame or "code" in frame:
                        sys.exit(f"the SUBSCRIBE was refused: {message}")
                    if frame.get("id") == 1:
                        answer = message
                    continue
                name = frame["stream"]
                seen.add(name)
                if name in wanted:
                    wanted.discard(name)
                    found[name] = frame
    except InvalidHandshake as err:
        sys.exit(f"{url}: handshake refused: {err}")
    except ConnectionClosed as err:
        sys.exit(f"{url}: the server closed the connection: {err}")
    return found, wanted, seen, answer


def capture_ws():
    """Every fixture by file name, PROVENANCE.md's table rows, the non-ASCII symbol the
    non-ASCII stream is for, and the answer to the `SUBSCRIBE`."""
    symbol = trading_non_ascii(get("/api/v3/exchangeInfo"))
    log(f"  GET /api/v3/exchangeInfo -> {symbol}")
    streams = stream_list(symbol)
    names = [name for _, name, _ in streams]
    envelopes, missing, seen, answer = asyncio.run(read_streams(names))
    if missing:
        sys.exit(f"no frame within {WS_WAIT} s on {sorted(missing)}; frames came on "
                 f"{sorted(seen)}; nothing written, run again")
    files, rows = {}, []
    for file, name, how in streams:
        envelope = envelopes[name]
        envelope["data"] = trim(how, envelope["data"], name)
        files[file] = envelope
        rows.append([f"`{file}`", f"`{name}`", TRIMMED[how]])
        log(f"  {name}")
    return files, rows, symbol, answer or "(no answer read)"


def streams_section(today, rows, symbol, answer):
    """PROVENANCE.md's `## Streams (ws/)` section, from table rows of cells."""
    lines = [
        "## Streams (ws/)",
        "",
        f"Captured {today} from `{WS_BASE}/stream` by `scripts/capture_binance_spot.py ws`. "
        "No credentials. One connection and one `SUBSCRIBE` (`id` 1) of the "
        f"{len(rows)} names below, answered `{answer}`; the first combined-stream envelope "
        f"(`{{\"stream\", \"data\"}}`) of each, within {WS_WAIT} s, every key kept.",
        "",
    ]
    lines += capture_common.provenance_table(["File", "Stream", "Trimmed to"], rows, short_rule=True)
    lines += [
        "",
        f"The non-ASCII stream is for `{symbol}`, the first `TRADING` symbol in "
        "`GET /api/v3/exchangeInfo` with a non-ASCII character, its ASCII letters lowercased "
        f"in the stream name (`{wire_symbol(symbol)}@miniTicker`).",
    ]
    return "\n".join(lines) + "\n"


def run_rest(out):
    log(f"capturing from {BASE}")
    files, rows = capture_rest()
    for name, value in files.items():
        capture_common.write_json(out / "rest" / name, value, indent=1)
    log(f"wrote {len(files)} files under {out / 'rest'}")
    print(provenance_section(capture_common.stamp("%Y-%m-%d"), rows), end="")


def run_ws(out):
    log(f"capturing from {WS_BASE}/stream")
    files, rows, symbol, answer = capture_ws()
    for name, value in files.items():
        capture_common.write_json(out / "ws" / name, value, indent=1)
    log(f"wrote {len(files)} files under {out / 'ws'}")
    print(streams_section(capture_common.stamp("%Y-%m-%d"), rows, symbol, answer), end="")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    rest = commands.add_parser("rest", help="capture the REST fixtures under OUT_DIR/rest/")
    ws = commands.add_parser("ws", help="capture the stream fixtures under OUT_DIR/ws/")
    for command in (rest, ws):
        command.add_argument("out", nargs="?", type=Path, default=DEFAULT_OUT,
                             help="the fixtures directory (default: polyoxide-binance/tests/fixtures/spot)")
    args = parser.parse_args(argv)

    sys.stdout.reconfigure(encoding="utf-8")
    if args.command == "rest":
        run_rest(args.out)
    else:
        run_ws(args.out)


if __name__ == "__main__":
    main()
