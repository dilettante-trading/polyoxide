#!/usr/bin/env python3
"""Capture Binance USDⓈ-M REST responses as test fixtures for polyoxide-binance.

Usage: python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures

Writes one JSON file per route under OUT_DIR/rest/ and rewrites OUT_DIR/PROVENANCE.md.
Every top-level key of every response is kept; only list lengths are trimmed, so a
wire-agreement test sees each field the server sends. Stdlib only, no credentials.
Costs about 60 request weight, well inside the 2400 per minute.
"""
import datetime
import json
import os
import sys
import urllib.parse
import urllib.request

BASE = "https://fapi.binance.com"


def get(path, **params):
    query = urllib.parse.urlencode(params)
    url = f"{BASE}{path}" + (f"?{query}" if query else "")
    request = urllib.request.Request(url, headers={"Accept-Encoding": "identity"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.loads(response.read().decode("utf-8"))


def save(out, name, value):
    with open(os.path.join(out, name), "w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=1)
        f.write("\n")


def first(rows, predicate, what):
    for row in rows:
        if predicate(row):
            return row
    sys.exit(f"no {what} listed; pick the fixture rows by hand")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    rest = os.path.join(out, "rest")
    os.makedirs(rest, exist_ok=True)

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
    save(rest, "exchange_info.json", trimmed)

    wanted = {"BTCUSDT", tradfi["symbol"], chinese["symbol"]}
    save(rest, "time.json", get("/fapi/v1/time"))
    save(rest, "ticker_24hr.json", [t for t in get("/fapi/v1/ticker/24hr") if t["symbol"] in wanted])
    save(rest, "premium_index.json", [p for p in get("/fapi/v1/premiumIndex") if p["symbol"] in wanted])
    funding_info = get("/fapi/v1/fundingInfo")
    rows = [f for f in funding_info if f["symbol"] in wanted]
    if not any(f["updateTime"] is None for f in rows):
        rows.append(first(funding_info, lambda f: f["updateTime"] is None, "fundingInfo row with null updateTime"))
    save(rest, "funding_info.json", rows)
    save(rest, "klines.json", get("/fapi/v1/klines", symbol="BTCUSDT", interval="1m", limit=2))
    save(rest, "funding_rate.json", get("/fapi/v1/fundingRate", symbol="BTCUSDT", limit=3))
    # The first funding events, from before markPrice was recorded: it is "" on the wire.
    save(rest, "funding_rate_2019.json",
         get("/fapi/v1/fundingRate", symbol="BTCUSDT", startTime=1568102400000, limit=2))
    save(rest, "open_interest.json", get("/fapi/v1/openInterest", symbol="BTCUSDT"))
    save(rest, "agg_trades.json", get("/fapi/v1/aggTrades", symbol="BTCUSDT", limit=3))
    save(rest, "depth.json", get("/fapi/v1/depth", symbol="BTCUSDT", limit=5))

    today = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d")
    with open(os.path.join(out, "PROVENANCE.md"), "w", encoding="utf-8") as f:
        f.write(f"""# Provenance

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

Captured 2026-10-07 by the stdlib scripts handed over with the design spec
(`docs/specs/binance/probes/capture_ws.py`): one combined-stream envelope
(`{{"stream", "data"}}`) per stream, from `/market/stream` for `!ticker@arr`,
`!markPrice@arr@1s`, `btcusdt@aggTrade`, `btcusdt@kline_1m`, `btcusdt@markPrice@1s` and
`btcusdt@ticker`, and from `/public/stream` for `btcusdt@depth20@100ms` and
`btcusdt@bookTicker`. Arrays are trimmed to two rows and depth sides to three levels.

## Probes

`docs/specs/binance/probes/` holds the scripts behind the design spec's measured facts:
`probe_rest.py` (weights from `X-MBX-USED-WEIGHT-1M` deltas), `probe_ws.py` and
`probe_ws2.py` (acknowledgements, case, the 1024 cap, the message rate),
`probe_ws_ping.py` (server ping cadence), and `wsprobe.py` (the frame reader they share).
""")
    print("captured:", ", ".join(sorted(os.listdir(rest))))


if __name__ == "__main__":
    main()
