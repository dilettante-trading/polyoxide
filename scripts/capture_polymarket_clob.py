#!/usr/bin/env python3
"""Capture the CLOB, Data and Gamma responses the clob module's venue tests
serve, for one live outcome token.

The token is chosen live: the first outcome of the first open neg-risk market
on /sampling-markets whose book has at least two levels on each side, whose
market has public trades of that outcome and of another, and whose Gamma row
names its event. Requests are spaced out. Lists are trimmed, as noted in
PROVENANCE.md; every key of every object kept is kept whole.

Usage:
    python3 scripts/capture_polymarket_clob.py polyoxide-polymarket/tests/fixtures/clob/venue

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import sys
import time
from pathlib import Path

import capture_common

CLOB = "https://clob.polymarket.com"
DATA = "https://data-api.polymarket.com"
GAMMA = "https://gamma-api.polymarket.com"
PAUSE_SECONDS = 0.5
BOOK_LEVELS = 10
EVENT_MARKETS = 3
TRADES_KEPT = 30
HISTORY_SECONDS = 3 * 24 * 60 * 60


def get(url, **params):
    return capture_common.get(url, params=params, pause=PAUSE_SECONDS)


def candidate(market):
    """The first outcome token of an open neg-risk market with a book, or None."""
    if not (market.get("neg_risk") and market.get("accepting_orders") and market.get("enable_order_book")):
        return None
    tokens = [t for t in market.get("tokens") or [] if t.get("token_id")]
    return tokens[0]["token_id"] if tokens else None


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    captured = []

    def keep(name, reply, body, note):
        capture_common.write_json(out / f"{name}.json", body)
        captured.append((name, reply.url, note))

    sampling = capture_common.require_ok(get(f"{CLOB}/sampling-markets"), "sampling-markets")
    chosen = None
    for market in sampling["data"]:
        token = candidate(market)
        if token is None:
            continue
        book = get(f"{CLOB}/book", token_id=token)
        if (
            book.status != 200
            or not book.is_json
            or not isinstance(book.body, dict)
            or len(book.body.get("bids", [])) < 2
            or len(book.body.get("asks", [])) < 2
        ):
            continue
        trades = get(f"{DATA}/trades", market=market["condition_id"], takerOnly="true")
        rows = trades.body if trades.status == 200 and isinstance(trades.body, list) else []
        if not any(r.get("asset") == token for r in rows) or not any(r.get("asset") != token for r in rows):
            continue
        gamma = get(f"{GAMMA}/markets", clob_token_ids=token)
        if gamma.status != 200 or not gamma.body or not gamma.body[0].get("events"):
            continue
        chosen = (market, token, book, trades, rows, gamma)
        break
    if chosen is None:
        sys.exit("no open neg-risk market on the first sampling page has a two-sided book, trades of two outcomes and an event")
    market, token, book, trades, rows, gamma = chosen
    condition = market["condition_id"]

    by_token = get(f"{CLOB}/markets-by-token/{token}")
    keep("markets_by_token", by_token, capture_common.require_ok(by_token, "markets-by-token"), "as fetched")
    detail = get(f"{CLOB}/markets/{condition}")
    keep("market", detail, capture_common.require_ok(detail, "market"), "as fetched")

    # The CLOB sends each side worst first, so the levels nearest the touch are last.
    body = dict(book.body)
    body["bids"] = body["bids"][-BOOK_LEVELS:]
    body["asks"] = body["asks"][-BOOK_LEVELS:]
    keep("book", book, body, f"each side to its {BOOK_LEVELS} levels nearest the touch, in wire order")

    now = int(time.time())
    history = get(f"{CLOB}/prices-history", market=token, fidelity=60, startTs=now - HISTORY_SECONDS, endTs=now)
    points = capture_common.require_ok(history, "prices-history")
    if not points.get("history"):
        sys.exit(f"{history.url}: no price history")
    keep("prices_history", history, points, "as fetched")

    kept, ours, theirs = [], 0, 0
    for row in rows:
        if len(kept) >= TRADES_KEPT and ours >= 3 and theirs >= 1:
            break
        kept.append(row)
        ours += row.get("asset") == token
        theirs += row.get("asset") != token
    keep("trades", trades, kept, f"the first {len(kept)} rows, newest first, {ours} of the token and {theirs} of another")

    keep("gamma_markets", gamma, gamma.body, "as fetched")
    event_id = gamma.body[0]["events"][0]["id"]
    event = get(f"{GAMMA}/events/{event_id}")
    event_body = dict(capture_common.require_ok(event, "event"))
    markets = event_body.get("markets") or []
    ours_first = [m for m in markets if token in (m.get("clobTokenIds") or "")]
    others = [m for m in markets if token not in (m.get("clobTokenIds") or "")]
    event_body["markets"] = (ours_first + others)[:EVENT_MARKETS]
    keep("gamma_event", event, event_body, f"`markets` to the token's market and the next {EVENT_MARKETS - 1}")

    stamp = capture_common.stamp()
    lines = [
        "# CLOB venue fixtures",
        "",
        f"Captured {stamp} by `scripts/capture_polymarket_clob.py` from the live hosts, for the",
        f"outcome token `{token}` of the market `{condition}`. The clob module's venue tests",
        "(`tests/venue_clob.rs`) serve them from one mock server. Lists are trimmed as noted.",
        "",
        *capture_common.provenance_table(
            ("Fixture", "Request", "Kept"),
            [(f"`{name}.json`", f"`{url}`", note) for name, url, note in captured]),
    ]
    capture_common.write_provenance(out, lines)
    print(f"captured {len(captured)} fixtures to {out}")


if __name__ == "__main__":
    main()
