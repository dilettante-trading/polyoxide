#!/usr/bin/env python3
"""Capture one live response per public Perps route as a test fixture.

Inputs are chosen live: the first instrument from /v1/info/instruments, an
address from the weekly leaderboard, the first base asset whose index has
constituents, the first unlisted instrument id that answers with a settlement
record, and the newest competition. Requests are spaced out.

The three per-competition routes and the widget are captured only while a
competition exists. Without one the three answer 404 and the widget holds only
its timestamp, so the script leaves their fixtures as they are and
PROVENANCE.md says they were written by hand from the spec.

Usage:
    python3 scripts/capture_polymarket_perps.py polyoxide-polymarket/tests/fixtures/perps

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import json
import sys
import time
from pathlib import Path

import capture_common

HOST = "https://api.perpetuals.polymarket.com"
PAUSE_SECONDS = 0.5
DAY_MS = 24 * 60 * 60 * 1000


# Fixtures written by hand from the spec while no competition exists to capture.
COMPETITION_FIXTURES = (
    "competition", "competition_leaderboard", "competition_account", "competition_widget")


def get(path, **params):
    return capture_common.get(f"{HOST}{path}", params=params, pause=PAUSE_SECONDS)


def post(path, body):
    return capture_common.post(f"{HOST}{path}", body, pause=PAUSE_SECONDS)


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    captured = []

    def keep(name, reply, request):
        body = capture_common.require_ok(reply, name)
        capture_common.write_json(out / f"{name}.json", body)
        captured.append((name, request))
        return body

    def save(name, path, **params):
        reply = get(path, **params)
        return keep(name, reply, reply.url)

    now = int(time.time() * 1000)
    day_ago = now - DAY_MS

    save("time", "/v1/info/time")
    save("exchange", "/v1/info/exchange")
    save("assets", "/v1/info/assets")
    instruments = save("instruments", "/v1/info/instruments")
    iid = instruments[0]["instrument_id"]
    save("fees", "/v1/info/fees")
    save("limit_tiers", "/v1/info/limit-tiers")

    save("tickers", "/v1/info/tickers", instrument_id=iid)
    save("statistics", "/v1/info/statistics", instrument_id=iid)
    save("exchange_stats", "/v1/info/exchange-stats", start_timestamp=day_ago, end_timestamp=now)
    save("klines", "/v1/info/klines", instrument_id=iid, interval="1h", start_timestamp=day_ago)
    save("mark_history", "/v1/info/mark-history", instrument_id=iid, interval="1h", start_timestamp=day_ago)
    save("bbo", "/v1/info/bbo", instrument_id=iid)
    save("book", "/v1/info/book", instrument_id=iid, depth=10)
    save("trades", "/v1/info/trades", instrument_id=iid)
    save("funding", "/v1/info/funding", instrument_id=iid)

    # Prefer an index with constituents so IndexConstituent is on the wire.
    chosen = None
    for instrument in instruments:
        reply = get("/v1/info/index", asset=instrument["base_asset"])
        if reply.status == 200 and reply.is_json and reply.body.get("constituents"):
            chosen = instrument["base_asset"]
            break
    save("index", "/v1/info/index", asset=chosen or instruments[0]["base_asset"])

    board = save("leaderboard", "/v1/info/leaderboard", window="week", limit=3)
    address = board["entries"][0]["account"]
    save("leaderboard_account", "/v1/info/leaderboard", window="week", limit=1, address=address)
    portfolio = save("portfolio", "/v1/info/portfolio", address=address)
    fill_iid = portfolio["positions"][0]["instrument_id"] if portfolio["positions"] else iid
    save("position_fills", "/v1/info/position-fills", address=address, instrument_id=fill_iid)
    save("invite", "/v1/info/invite", code="polyoxide-fixture")
    save("registered", "/v1/info/registered", address=address)
    save("builder", "/v1/info/builder", address=address)

    fills = capture_common.require_ok(
        get("/v1/info/position-fills", address=address, instrument_id=fill_iid), "position fills")
    snapshot_body = {
        "address": address,
        "active_instrument_ids": [p["instrument_id"] for p in portfolio["positions"][:20]] or [iid],
        "history_fills": [
            {"instrument_id": f["instrument_id"], "trade_id": str(f["trade_id"]), "timestamp": f["timestamp"]}
            for f in fills["data"][:4]
        ],
    }
    if not snapshot_body["history_fills"]:
        del snapshot_body["history_fills"]
    reply = post("/v1/info/position-snapshots", snapshot_body)
    keep("position_snapshots", reply, f"POST {reply.url} {json.dumps(snapshot_body)}")

    # A settled instrument is left out of the listing, so look for one among
    # the ids the listing skips.
    listed = {i["instrument_id"] for i in instruments}
    for candidate in range(1, max(listed) + 2):
        if candidate in listed:
            continue
        reply = get("/v1/info/instruments", instrument_id=candidate)
        rows = capture_common.require_ok(reply, "instrument")
        if rows and rows[0].get("settlement"):
            keep("instrument_settled", reply, reply.url)
            break

    competitions = save("competitions", "/v1/info/competitions", address=address)
    by_hand = []
    if competitions["competitions"]:
        save("competition_widget", "/v1/info/competition/widget", address=address)
        cid = competitions["competitions"][0]["competition_id"]
        save("competition", "/v1/info/competition", competition_id=cid)
        board = save("competition_leaderboard", "/v1/info/competition/leaderboard",
                     competition_id=cid, limit=3)
        entrant = board["entries"][0]["account"] if board["entries"] else address
        save("competition_account", "/v1/info/competition/account",
             competition_id=cid, address=entrant)
    else:
        by_hand = list(COMPETITION_FIXTURES)

    stamp = capture_common.stamp()
    lines = [
        "# Perps fixtures",
        "",
        f"Captured {stamp} by `scripts/capture_polymarket_perps.py` from the live host.",
        "Each file is the complete response body, pretty-printed. Inputs were chosen live",
        "(see the script), so the instrument and address here are whatever was active then.",
        "",
        *capture_common.provenance_table(
            ("Fixture", "Request"), [(f"`{name}.json`", f"`{url}`") for name, url in captured]),
    ]
    if by_hand:
        lines += [
            "",
            "No competition existed at capture time: every competition id answered 404, and",
            "the widget held only its timestamp,",
            "so these were written by hand from `docs/specs/perps/openapi.json` and kept:",
            "",
            *(f"- `{name}.json`" for name in by_hand),
            "",
            "They are synthetic: `competition.json` and `competition_widget.json` describe a live",
            "competition 3, and `competition_leaderboard.json` and `competition_account.json` an ended,",
            "paid competition 2, each score computed as PnL × √(boosted volume) to six places.",
        ]
    capture_common.write_provenance(out, lines)
    print(f"captured {len(captured)} fixtures to {out}")


if __name__ == "__main__":
    main()
