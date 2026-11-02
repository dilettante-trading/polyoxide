#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["certifi"]
# ///
"""Capture Binance spot REST responses as test fixtures for polyoxide-binance's `spot` module.

Usage: python3 scripts/capture_binance_spot.py rest [OUT_DIR]
       (or `uv run scripts/capture_binance_spot.py rest [OUT_DIR]`)

OUT_DIR defaults to polyoxide-binance/tests/fixtures/spot. Writes one JSON file per route
under OUT_DIR/rest/ and nothing else; it does not write PROVENANCE.md. It prints
PROVENANCE.md's `## REST (rest/)` section to stdout, and progress to stderr. Nothing is
written until every capture has succeeded, so a failed run leaves the fixtures as they
were. Every key is kept; only list lengths are trimmed, so a wire-agreement test sees each
field the server sends. No credentials. Costs about 180 request weight, well inside the
6000 per minute. Review the diff before committing.

Run it as a file, never `python3 -I`, which keeps scripts/ off sys.path.
"""
import argparse
import sys
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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    rest = commands.add_parser("rest", help="capture the REST fixtures under OUT_DIR/rest/")
    rest.add_argument("out", nargs="?", type=Path, default=DEFAULT_OUT,
                      help="the fixtures directory (default: polyoxide-binance/tests/fixtures/spot)")
    args = parser.parse_args(argv)

    log(f"capturing from {BASE}")
    files, rows = capture_rest()
    for name, value in files.items():
        capture_common.write_json(args.out / "rest" / name, value, indent=1)
    log(f"wrote {len(files)} files under {args.out / 'rest'}")
    sys.stdout.reconfigure(encoding="utf-8")
    print(provenance_section(capture_common.stamp("%Y-%m-%d"), rows), end="")


if __name__ == "__main__":
    main()
