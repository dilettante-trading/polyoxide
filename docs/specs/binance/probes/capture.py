"""Capture trimmed live Binance USD-M payloads as test fixtures.

Usage: python3 -I capture.py <out_dir> <wsprobe_dir>
Writes one pretty JSON file per REST route and per stream envelope.
"""
import json
import os
import sys
import urllib.parse
import urllib.request

OUT = sys.argv[1]
sys.path.insert(0, sys.argv[2])
import wsprobe  # noqa: E402  (the probe's frame reader, reused)

BASE = "https://fapi.binance.com"
SYMBOLS = ["BTCUSDT", "TSLAUSDT", "币安人生USDT"]


def get(path, **params):
    query = urllib.parse.urlencode(params)
    url = f"{BASE}{path}" + (f"?{query}" if query else "")
    with urllib.request.urlopen(url, timeout=30) as r:
        return json.loads(r.read().decode("utf-8"))


def save(name, value):
    with open(os.path.join(OUT, name), "w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=1)
        f.write("\n")


def main():
    os.makedirs(OUT, exist_ok=True)
    info = get("/fapi/v1/exchangeInfo")
    syms = info["symbols"]
    keep = [s for s in syms if s["symbol"] in SYMBOLS]
    settling = next(s for s in syms if s["contractType"] == "PERPETUAL" and s["status"] == "SETTLING")
    quarterly = next(s for s in syms if s["contractType"] == "CURRENT_QUARTER")
    save("exchange_info.json", {
        "timezone": info["timezone"], "serverTime": info["serverTime"],
        "rateLimits": info["rateLimits"], "exchangeFilters": info["exchangeFilters"],
        "assets": info["assets"][:1], "symbols": keep + [settling, quarterly],
    })
    save("ticker_24hr.json", [t for t in get("/fapi/v1/ticker/24hr") if t["symbol"] in SYMBOLS])
    save("premium_index.json", [p for p in get("/fapi/v1/premiumIndex") if p["symbol"] in SYMBOLS])
    funding_info = get("/fapi/v1/fundingInfo")
    save("funding_info.json", [f for f in funding_info if f["symbol"] in SYMBOLS] or funding_info[:2])
    save("klines.json", get("/fapi/v1/klines", symbol="BTCUSDT", interval="1m", limit=2))
    save("funding_rate.json", get("/fapi/v1/fundingRate", symbol="BTCUSDT", limit=3))
    save("open_interest.json", get("/fapi/v1/openInterest", symbol="BTCUSDT"))
    save("agg_trades.json", get("/fapi/v1/aggTrades", symbol="BTCUSDT", limit=3))
    depth = get("/fapi/v1/depth", symbol="BTCUSDT", limit=5)
    save("depth.json", depth)
    print("REST ok:", sorted(os.listdir(OUT)))


if __name__ == "__main__":
    main()
