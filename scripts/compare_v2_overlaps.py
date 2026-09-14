#!/usr/bin/env python3
"""Compare Data API v2 routes against the undocumented hosts they overlap.

Evidence for docs/specs/data-v2/OBSERVED.md. Two comparisons, both against
wallets chosen live from the weekly v2 PnL board:

1. `/v2/user-pnl` `trade_pnl` vs `user-pnl-api` `/user-pnl` `p`, which the v2
   spec describes as the same series. Joined on timestamp.
2. `/v2/leaderboard?user=` `volume` (documented as shares) vs `lb-api`
   `/volume` `amount` (USDC) for the same wallet and a comparable window.

Usage:
    python3 scripts/compare_v2_overlaps.py [--wallets N]
"""
import json
import sys
import time
import urllib.parse
import urllib.request

DATA = "https://data-api.polymarket.com"
PNL = "https://user-pnl-api.polymarket.com"
LB = "https://lb-api.polymarket.com"


def get(url, **params):
    query = urllib.parse.urlencode(params)
    request = urllib.request.Request(f"{url}?{query}", headers={"user-agent": "polyoxide-overlap-compare"})
    time.sleep(0.5)
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read())


def main():
    count = int(sys.argv[sys.argv.index("--wallets") + 1]) if "--wallets" in sys.argv else 3
    board = get(f"{DATA}/v2/leaderboard", time_period="week", limit=count)["data"]
    wallets = [row["user_id"] for row in board]

    print("## /v2/user-pnl trade_pnl vs user-pnl-api p  (interval=1w, fidelity=1d)\n")
    print("| wallet | points joined | min diff % | max diff % | v2 extra | legacy extra |")
    print("|--------|---------------|-----------|-----------|----------|--------------|")
    for wallet in wallets:
        v2 = get(f"{DATA}/v2/user-pnl", user=wallet, interval="1w", fidelity="1d")["data"]["points"]
        legacy = get(f"{PNL}/user-pnl", user_address=wallet, interval="1w", fidelity="1d")
        v2_by_t = {p["timestamp"]: p["trade_pnl"] for p in v2 if p.get("trade_pnl") is not None}
        legacy_by_t = {p["t"]: p["p"] for p in legacy}
        joined = sorted(set(v2_by_t) & set(legacy_by_t))
        diffs = [
            100.0 * (v2_by_t[t] - legacy_by_t[t]) / abs(legacy_by_t[t])
            for t in joined
            if legacy_by_t[t] != 0
        ]
        lo = f"{min(diffs):+.3f}" if diffs else "n/a"
        hi = f"{max(diffs):+.3f}" if diffs else "n/a"
        print(f"| `{wallet}` | {len(joined)} | {lo} | {hi} | {len(set(v2_by_t) - set(legacy_by_t))} | {len(set(legacy_by_t) - set(v2_by_t))} |")

    print("\n## /v2/leaderboard?user= volume vs lb-api /volume amount  (v2 week vs lb-api 7d)\n")
    lb_rows = get(f"{LB}/volume", window="7d", limit=50)
    lb_by_wallet = {row.get("proxyWallet", "").lower(): row.get("amount") for row in lb_rows}
    print("| wallet | v2 volume | lb-api amount | ratio |")
    print("|--------|-----------|---------------|-------|")
    vol_board = get(f"{DATA}/v2/leaderboard", time_period="week", sort_by="VOLUME", limit=count)["data"]
    for row in vol_board:
        wallet = row["user_id"].lower()
        amount = lb_by_wallet.get(wallet)
        ratio = f"{row['volume'] / amount:.3f}" if amount else "not in lb-api top 50"
        print(f"| `{wallet}` | {row['volume']:.2f} | {amount if amount is not None else '—'} | {ratio} |")


if __name__ == "__main__":
    main()
