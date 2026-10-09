#!/usr/bin/env python3
"""Capture one live response per Data API v2 route as a test fixture.

Inputs are chosen live, never hardcoded: a wallet from the bare trade feed, a
market and event from that wallet's positions, a combo wallet from the combo
winners board, and markets still settling from gamma's UMA status filter. Every
request uses a small `limit`, and requests are spaced out.

Usage:
    python3 scripts/capture_v2_fixtures.py polyoxide-data/tests/fixtures/v2

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import secrets
import sys
from pathlib import Path

import capture_common

HOST = "https://data-api.polymarket.com"
GAMMA = "https://gamma-api.polymarket.com"
PAUSE_SECONDS = 0.5


def get(path, host=HOST, **params):
    return capture_common.get(f"{host}{path}", params=params, pause=PAUSE_SECONDS)


def settling_conditions():
    """One condition per `settlement_time_basis` among markets still settling.

    A resolved row carries no settlement estimate, and neither does a stale
    proposal or a challenged one, so these are probed rather than taken from the
    winners board. Disputes are rare, so a capture can lack `dvm_round_estimate`.
    """
    chosen = {}
    for status in ("proposed", "disputed"):
        markets = capture_common.require_ok(
            get("/markets", host=GAMMA, uma_resolution_status=status, limit=100))
        ids = [m["conditionId"] for m in markets if m.get("conditionId")]
        for i in range(0, len(ids), 20):
            rows = capture_common.require_ok(
                get("/v2/resolutions", condition=",".join(ids[i : i + 20])))
            for row in rows["data"]:
                if row.get("settlement_time_basis"):
                    chosen.setdefault(row["settlement_time_basis"], row["condition_id"])
    if not chosen:
        raise SystemExit("resolutions_pending: no settling market carries an estimate")
    return list(chosen.values())


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    captured = []

    def save(name, path, **params):
        reply = get(path, **params)
        body = capture_common.require_ok(reply, name)
        capture_common.write_json(out / f"{name}.json", body)
        captured.append((name, reply.url))
        return body

    trades = save("trades", "/v2/trades", limit=2)
    wallet = trades["data"][0]["proxy_wallet"]
    token_id = trades["data"][0]["token_id"]

    positions = save("positions", "/v2/positions", user=wallet, limit=2)
    position = positions["data"][0]
    condition = position["condition_id"]
    event_id = position["event_id"]

    save("positions_closed", "/v2/positions", user=wallet, status="CLOSED", limit=2)
    save("activity", "/v2/activity", user=wallet, limit=2)
    save("activity_tips", "/v2/activity", user=wallet, type="TRADE,TIP", limit=2)
    save("approvals", "/v2/approvals", user=wallet)
    save("user_pnl", "/v2/user-pnl", user=wallet, interval="1w", fidelity="1d")
    save("user_stats", "/v2/user-stats", user=wallet)
    # A fresh random address. `0x…0001` is NOT unknown: it answers with zeros.
    unknown_wallet = "0x" + secrets.token_hex(20)
    save("user_stats_unknown", "/v2/user-stats", user=unknown_wallet)
    save("user_volume", "/v2/user-volume", user=wallet)
    save("value", "/v2/value", user=wallet)
    save("holders", "/v2/holders", condition=condition, limit=2)
    save("holders_pnl", "/v2/holders", condition=condition, include_pnl="true", limit=2)
    save("live_volume", "/v2/live-volume", event_id=event_id)
    save("open_interest", "/v2/oi", condition=condition)
    save("open_interest_global", "/v2/oi")
    save("prices_history", "/v2/prices-history", token_id=token_id, interval="1d", limit=2)
    winners = save("biggest_winners", "/v2/biggest-winners", time_period="week", limit=2)
    # A win is only on the board once its market resolved, so this condition is
    # guaranteed to have a resolution row.
    resolved = next(w["condition_id"] for w in winners["data"] if w["kind"] == "market")
    resolutions = save("resolutions", "/v2/resolutions", condition=resolved)
    question_ids = [r["question_id"] for r in resolutions["data"] if r.get("question_id")]
    if question_ids:
        save("resolutions_question", "/v2/resolutions", question_id=question_ids[0])
    save("resolutions_pending", "/v2/resolutions", condition=",".join(settling_conditions()))

    combo_winners = save("biggest_winners_combos", "/v2/biggest-winners", category="combos", time_period="all", limit=2)
    combo_wallet = combo_winners["data"][0]["user_id"]
    save("combo_positions", "/v2/positions/combos", user=combo_wallet, limit=2)
    save("combo_activity", "/v2/activity/combos", user=combo_wallet, limit=2)

    save("builders_leaderboard", "/v2/builders/leaderboard", time_period="week", limit=2)
    save("builder_volume", "/v2/builders/volume", interval="week", limit=2)
    save("leaderboard", "/v2/leaderboard", time_period="week", limit=2)
    save("leaderboard_user", "/v2/leaderboard", user=winners["data"][0]["user_id"], time_period="week")
    save("leaderboard_user_unknown", "/v2/leaderboard", user=unknown_wallet, time_period="week")
    save("status", "/v2/status")

    now = capture_common.stamp()
    lines = [
        "# Data API v2 fixtures",
        "",
        f"Captured {now} by `scripts/capture_v2_fixtures.py` from the live host.",
        "Each file is the complete response body, pretty-printed. Inputs were chosen live",
        "(see the script), so the wallets and markets here are whatever was active then.",
        "",
        *capture_common.provenance_table(
            ("Fixture", "Request"), [(f"`{name}.json`", f"`{url}`") for name, url in captured]),
    ]
    capture_common.write_provenance(out, lines)
    print(f"captured {len(captured)} fixtures into {out}")


if __name__ == "__main__":
    main()
