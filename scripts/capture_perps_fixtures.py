#!/usr/bin/env python3
"""Capture one live response per public Perps route as a test fixture.

Inputs are chosen live: the first instrument from /v1/info/instruments, an
address from the weekly leaderboard, and the first base asset whose index has
constituents. Requests are spaced out.

Usage:
    python3 scripts/capture_perps_fixtures.py polyoxide-perps/tests/fixtures

Writes `<name>.json` per capture plus `PROVENANCE.md` listing each URL and the
capture time. Re-run to refresh; review the diff before committing.
"""
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

HOST = "https://api.perpetuals.polymarket.com"
PAUSE_SECONDS = 0.5
DAY_MS = 24 * 60 * 60 * 1000


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
        url, status, body = get("/v1/info/index", asset=instrument["base_asset"])
        if status == 200 and body.get("constituents"):
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

    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines = [
        "# Perps fixtures",
        "",
        f"Captured {stamp} by `scripts/capture_perps_fixtures.py` from the live host.",
        "Each file is the complete response body, pretty-printed. Inputs were chosen live",
        "(see the script), so the instrument and address here are whatever was active then.",
        "",
        "| Fixture | Request |",
        "|---------|---------|",
    ]
    lines += [f"| `{name}.json` | `{url}` |" for name, url in captured]
    (out / "PROVENANCE.md").write_text("\n".join(lines) + "\n")
    print(f"captured {len(captured)} fixtures to {out}")


if __name__ == "__main__":
    main()
