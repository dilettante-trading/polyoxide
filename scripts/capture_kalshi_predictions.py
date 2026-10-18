#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["certifi"]
# ///
"""Capture Kalshi's public REST responses as test fixtures for polyoxide-kalshi's
`predictions` module.

Usage: uv run scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions

Reads production (https://external-api.kalshi.com) with no credentials, at the anonymous
default of 10 tokens a request, at most a few hundred requests in all. It picks an active market with
a quote on both sides, a two-sided book with more than one level on some side and a full page of
100 trades by paging `markets?status=open&mve_filter=exclude`:
the filter is needed, since on 2026-10-18 the unfiltered open listing's first ten pages held
9,996 multivariate combinations of 10,000 rows and no quoted single market, while the filtered
listing's first page held 579 quoted rows. It then writes the fixtures and PROVENANCE.md
into OUT_DIR. Nothing is written until every capture has succeeded, so a failed run leaves the
fixtures as they were. Every key is kept; only list lengths are trimmed, so a wire-agreement test
sees each field the host sends. Review the diff before committing.
"""
import sys
from pathlib import Path

import capture_common

BASE = "https://external-api.kalshi.com/trade-api/v2"
PAUSE = 0.25
# Pages of the filtered listing to walk before giving up, and the rows of each.
PAGES = 10
PAGE_SIZE = 1000
# How many quoted candidates to confirm on the orderbook route before giving up.
CANDIDATES = 150
# Trades a candidate's first page must hold: the fixture is a full page of the route's limit.
TRADES_PAGE = 100
# Levels kept on each side of orderbook.json, the best ones.
BOOK_LEVELS = 5
# Markets kept in event.json's nested list, the picked one first.
EVENT_MARKETS = 3
UNKNOWN_TICKER = "NOSUCHMARKET-0"
NO_SIGNATURE_PATH = "/account/limits"
# Kalshi's rate-limit page gives a 429's body as this; provoking one would spend the quota
# the nightly runs share.
RATE_LIMITED_BODY = {"error": "too many requests"}


def identity(_method, _url):
    """Asks for an uncompressed body."""
    return {"Accept-Encoding": "identity"}


def fetch(path, **params):
    return capture_common.get(BASE + path, params=params, headers=identity, pause=PAUSE, timeout=60)


def fetch_ok(path, **params):
    return capture_common.require_ok(fetch(path, **params), f"GET {path}")


def refused(path, status, **params):
    """The JSON body of a reply the host refuses with `status`."""
    reply = fetch(path, **params)
    if reply.status != status or not reply.is_json:
        sys.exit(f"GET {path}: expected a JSON body with HTTP {status}, got HTTP {reply.status}: {reply.body!r}")
    return reply.body


def is_two_sided(market):
    """Active, with a YES bid above 0 and a YES ask below 1."""
    return (
        market["status"] == "active"
        and float(market["yes_bid_dollars"]) > 0
        and float(market["yes_ask_dollars"]) < 1
    )


def listing_rows():
    """The rows of `markets?status=open&mve_filter=exclude&limit=1000`, page by page, at most
    PAGES pages."""
    cursor = ""
    for _ in range(PAGES):
        params = {"status": "open", "mve_filter": "exclude", "limit": PAGE_SIZE}
        if cursor:
            params["cursor"] = cursor
        page = fetch_ok("/markets", **params)
        yield from page["markets"]
        cursor = page.get("cursor") or ""
        if not cursor:
            return


def pick_market():
    """The first listed market that quotes both sides and whose order book, read on the route the
    tests assert on, has levels on both and more than one on some side, so that `depth=1` differs
    from the full book, and whose `/markets/trades?limit=100` page holds a full 100 rows, so that the
    trades fixture has many trades to order. Returns the ticker, the market response, the book and
    the trades response."""
    tried = 0
    for row in listing_rows():
        if not is_two_sided(row):
            continue
        tried += 1
        if tried > CANDIDATES:
            break
        ticker = row["ticker"]
        book = fetch_ok(f"/markets/{ticker}/orderbook")
        sides = book["orderbook_fp"]
        if sides["yes_dollars"] and sides["no_dollars"] and max(len(v) for v in sides.values()) >= 2:
            trades = fetch_ok("/markets/trades", ticker=ticker, limit=100)
            if len(trades["trades"]) == TRADES_PAGE:
                return ticker, fetch_ok(f"/markets/{ticker}"), book, trades
    sys.exit(
        f"none of the first {CANDIDATES} quoted open markets has a two-sided book with a deeper side "
        f"and a full page of {TRADES_PAGE} trades; run again"
    )


def best_levels(book, count):
    """Keeps the `count` best levels of each side. The host lists a side worst first, so those
    are the last `count`."""
    return {"orderbook_fp": {side: levels[-count:] for side, levels in book["orderbook_fp"].items()}}


def provenance(ticker, event_ticker):
    today = capture_common.stamp("%Y-%m-%d")
    rows = [
        ("exchange_status.json", "GET /exchange/status", "as fetched"),
        ("endpoint_costs.json", "GET /account/endpoint_costs", "as fetched"),
        ("market.json", f"GET /markets/{ticker}", "as fetched"),
        ("markets_page.json", "GET /markets?status=open&mve_filter=exclude&limit=5", "as fetched"),
        (
            "orderbook.json",
            f"GET /markets/{ticker}/orderbook",
            f"the {BOOK_LEVELS} best levels of each side (the host lists a side worst first, so the last {BOOK_LEVELS})",
        ),
        ("orderbook_depth1.json", f"GET /markets/{ticker}/orderbook?depth=1", "as fetched"),
        ("trades.json", f"GET /markets/trades?ticker={ticker}&limit=100", "as fetched"),
        (
            "event.json",
            f"GET /events/{event_ticker}?with_nested_markets=true",
            f"`event.markets`: the market above first, then up to {EVENT_MARKETS - 1} more",
        ),
        ("error_not_found.json", f"GET /markets/{UNKNOWN_TICKER}", "the 404 body"),
        ("error_validation.json", "GET /markets/trades?limit=5000", "the 400 body"),
        ("error_unauthenticated.json", f"GET {NO_SIGNATURE_PATH}", "the 401 body, sent with no signature"),
        (
            "error_rate_limited.json",
            "none",
            "written from the 429 body on Kalshi's rate-limit page (https://docs.kalshi.com/getting_started/rate_limits), not captured",
        ),
    ]
    table = capture_common.provenance_table(
        ["File", "Request", "Trimmed to"],
        [(f"`{name}`", request if request == "none" else f"`{request}`", note) for name, request, note in rows],
    )
    return [
        "# Provenance",
        "",
        f"Captured {today} from `https://external-api.kalshi.com/trade-api/v2` by",
        "`scripts/capture_kalshi_predictions.py`. No credentials. Every key is kept; only list",
        "lengths are trimmed.",
        "",
        f"The market is `{ticker}`, picked from `markets?status=open&mve_filter=exclude` because it was",
        "active, quoted both sides, had an order book with levels on both and more than one level on some",
        "side, and had a full page of 100 trades. The filter is needed: the",
        "unfiltered open listing serves multivariate combinations first (see `docs/specs/kalshi/OBSERVED.md`).",
        "",
        *table,
    ]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    ticker, market, book, trades = pick_market()
    event_ticker = market["market"]["event_ticker"]

    event = fetch_ok(f"/events/{event_ticker}", with_nested_markets="true")
    nested = sorted(event["event"].get("markets") or [], key=lambda m: m["ticker"] != ticker)
    if not nested or nested[0]["ticker"] != ticker:
        sys.exit(f"/events/{event_ticker}?with_nested_markets=true does not list {ticker}")
    event["event"]["markets"] = nested[:EVENT_MARKETS]

    files = {
        "exchange_status.json": fetch_ok("/exchange/status"),
        "endpoint_costs.json": fetch_ok("/account/endpoint_costs"),
        "market.json": market,
        "markets_page.json": fetch_ok("/markets", status="open", mve_filter="exclude", limit=5),
        "orderbook.json": best_levels(book, BOOK_LEVELS),
        "orderbook_depth1.json": fetch_ok(f"/markets/{ticker}/orderbook", depth=1),
        "trades.json": trades,
        "event.json": event,
        "error_not_found.json": refused(f"/markets/{UNKNOWN_TICKER}", 404),
        "error_validation.json": refused("/markets/trades", 400, limit=5000),
        "error_unauthenticated.json": refused(NO_SIGNATURE_PATH, 401),
        "error_rate_limited.json": RATE_LIMITED_BODY,
    }
    for name, value in files.items():
        capture_common.write_json(out / name, value)
    capture_common.write_provenance(out, provenance(ticker, event_ticker))
    print(f"wrote {len(files)} fixtures and PROVENANCE.md to {out} (market {ticker})")


if __name__ == "__main__":
    main()
