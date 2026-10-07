"""Measure Binance USD-M REST request weights from X-MBX-USED-WEIGHT-1M deltas.

Usage: python3 -I probe_rest.py
Each call's weight is the header after it minus the header after the previous call,
so the calls run back to back inside one minute window.
"""
import json
import time
import urllib.request

BASE = "https://fapi.binance.com"
CALLS = [
    ("ping", "/fapi/v1/ping"),
    ("exchangeInfo", "/fapi/v1/exchangeInfo"),
    ("ticker24hr all", "/fapi/v1/ticker/24hr"),
    ("ticker24hr one", "/fapi/v1/ticker/24hr?symbol=BTCUSDT"),
    ("premiumIndex all", "/fapi/v1/premiumIndex"),
    ("premiumIndex one", "/fapi/v1/premiumIndex?symbol=BTCUSDT"),
    ("fundingInfo", "/fapi/v1/fundingInfo"),
    ("klines 1500", "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=1500"),
    ("klines 499", "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=499"),
    ("klines 99", "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=99"),
    ("fundingRate 1000", "/fapi/v1/fundingRate?symbol=BTCUSDT&limit=1000"),
    ("openInterest", "/fapi/v1/openInterest?symbol=BTCUSDT"),
    ("aggTrades 100", "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=100"),
    ("depth 20", "/fapi/v1/depth?symbol=BTCUSDT&limit=20"),
    ("depth 100", "/fapi/v1/depth?symbol=BTCUSDT&limit=100"),
    ("unknown symbol", "/fapi/v1/premiumIndex?symbol=NOTASYMBOLUSDT"),
]


def call(path):
    req = urllib.request.Request(BASE + path, headers={"Accept-Encoding": "identity"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            body = r.read()
            return r.status, dict(r.headers), body
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def main():
    prev = None
    for name, path in CALLS:
        status, headers, body = call(path)
        used = headers.get("X-MBX-USED-WEIGHT-1M") or headers.get("x-mbx-used-weight-1m")
        used = int(used) if used is not None else None
        delta = (used - prev) if (used is not None and prev is not None) else None
        prev = used if used is not None else prev
        extra = {k: v for k, v in headers.items() if k.lower().startswith("x-mbx") and k.lower() != "x-mbx-used-weight-1m"}
        note = ""
        if status != 200:
            note = body[:200].decode("utf-8", "replace")
        print(f"{name:18} status={status} used={used} weight={delta} bytes={len(body)} {extra or ''} {note}")
    # Headers worth knowing once.
    status, headers, _ = call("/fapi/v1/ping")
    keep = {k: v for k, v in headers.items() if k.lower() in ("server", "x-cache", "via", "cache-control", "content-encoding", "retry-after")}
    print("ping headers:", keep)


if __name__ == "__main__":
    main()
