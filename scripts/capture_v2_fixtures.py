#!/usr/bin/env python3
"""Capture one live response per Data API v2 route as a test fixture.

Inputs are chosen live, never hardcoded: a wallet from the bare trade feed, a
market and event from that wallet's positions, a combo wallet from the combo
winners board. Every request uses a small `limit`, and requests are spaced out.

Usage:
    python3 scripts/capture_v2_fixtures.py polyoxide-data/tests/fixtures/v2

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import json
import secrets
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

HOST = "https://data-api.polymarket.com"
PAUSE_SECONDS = 0.5


def get(path, **params):
    query = urllib.parse.urlencode({k: v for k, v in params.items() if v is not None})
    url = f"{HOST}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(url, headers={"user-agent": "polyoxide-fixture-capture"})
    time.sleep(PAUSE_SECONDS)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return url, response.status, json.loads(response.read())
    except urllib.error.HTTPError as err:
        return url, err.code, json.loads(err.read())


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    captured = []

    def save(name, path, **params):
        url, status, body = get(path, **params)
        if status != 200:
            raise SystemExit(f"{name}: HTTP {status} from {url}: {body}")
        (out / f"{name}.json").write_text(json.dumps(body, indent=2, ensure_ascii=False) + "\n")
        captured.append((name, url))
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

    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines = [
        "# Data API v2 fixtures",
        "",
        f"Captured {now} by `scripts/capture_v2_fixtures.py` from the live host.",
        "Each file is the complete response body, pretty-printed. Inputs were chosen live",
        "(see the script), so the wallets and markets here are whatever was active then.",
        "",
        "| Fixture | Request |",
        "|---------|---------|",
    ]
    lines += [f"| `{name}.json` | `{url}` |" for name, url in captured]
    (out / "PROVENANCE.md").write_text("\n".join(lines) + "\n")
    print(f"captured {len(captured)} fixtures into {out}")


if __name__ == "__main__":
    main()
