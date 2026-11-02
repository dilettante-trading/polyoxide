"""Measure Binance REST request weights from X-MBX-USED-WEIGHT-1M deltas.

Usage:
  python3 -I probe_rest.py                    USD-M futures, as before
  python3 -I probe_rest.py --product spot     spot: the four sections below, as Markdown on stdout
  python3 -I probe_rest.py --product spot --section counter

Each call's weight is the header after it minus the header after the previous call,
so the calls run back to back inside one minute window.

The spot sections, each headed as OBSERVED.md's `## Spot` section heads it:
  weights   every route of the design spec's section 3.1, over distinct URLs
  limits    a limit past a route's maximum, depth at any limit, a two-hour aggTrades
            window, an unknown symbol
  window    a ping every 3 s across a minute boundary: is the window the UTC clock minute?
  counter   do api.binance.com and data-api.binance.vision share one counter?
Progress goes to stderr, the Markdown to stdout. The host's clock must be NTP-synced.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from typing import Callable, NamedTuple

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

SPOT_BASE = "https://api.binance.com"
VISION_BASE = "https://data-api.binance.vision"
USED_HEADER = "x-mbx-used-weight-1m"
SYMBOLS = ("BTCUSDT", "ETHUSDT", "BNBUSDT")
COUNTER_SYMBOLS = ("BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT")
FENCE = "`" * 3  # a Markdown code fence, built so this file holds none


def by_symbol(template):
    return [template.format(s=s) for s in SYMBOLS]


def with_limits(template, limits):
    """One URL per limit, each on a different symbol, so no two URLs are alike."""
    return [template.format(s=s, n=n) for s, n in zip(SYMBOLS, limits)]


# (row, the weight the design spec's section 3.1 gives it, the distinct URLs measured).
SPOT_ROWS = [
    ("ping", 1, ["/api/v3/ping", "/api/v3/ping"]),
    ("time", 1, ["/api/v3/time", "/api/v3/time"]),
    ("exchangeInfo, with symbol", 20, [f"/api/v3/exchangeInfo?symbol={s}" for s in SYMBOLS[:2]]),
    ("exchangeInfo, without symbol", 20,
     ["/api/v3/exchangeInfo", "/api/v3/exchangeInfo?showPermissionSets=false"]),
    ("depth, no limit", 5, by_symbol("/api/v3/depth?symbol={s}")),
    ("depth, limit 1 to 100", 5, with_limits("/api/v3/depth?symbol={s}&limit={n}", (1, 7, 100))),
    ("depth, limit 101 to 500", 25, with_limits("/api/v3/depth?symbol={s}&limit={n}", (101, 300, 500))),
    ("depth, limit 501 to 1000", 50, with_limits("/api/v3/depth?symbol={s}&limit={n}", (501, 750, 1000))),
    ("depth, limit above 1000", 250, with_limits("/api/v3/depth?symbol={s}&limit={n}", (1001, 5000, 5001))),
    ("trades, no limit", 25, by_symbol("/api/v3/trades?symbol={s}")),
    ("trades, with limit", 25, with_limits("/api/v3/trades?symbol={s}&limit={n}", (1, 3, 500))),
    ("aggTrades, no limit", 4, by_symbol("/api/v3/aggTrades?symbol={s}")),
    ("aggTrades, with limit", 4, with_limits("/api/v3/aggTrades?symbol={s}&limit={n}", (1, 3, 500))),
    ("klines, no limit", 2, by_symbol("/api/v3/klines?symbol={s}&interval=1m")),
    ("klines, with limit", 2,
     with_limits("/api/v3/klines?symbol={s}&interval=1m&limit={n}", (1, 2, 1000))),
    ("avgPrice", 2, by_symbol("/api/v3/avgPrice?symbol={s}")),
    ("ticker/24hr, one symbol", 2, by_symbol("/api/v3/ticker/24hr?symbol={s}")),
    ("ticker/24hr, all", 80, ["/api/v3/ticker/24hr", "/api/v3/ticker/24hr?type=MINI"]),
    ("ticker/price, one symbol", 2, by_symbol("/api/v3/ticker/price?symbol={s}")),
    ("ticker/price, all", 4, ["/api/v3/ticker/price", "/api/v3/ticker/price?symbolStatus=TRADING"]),
    ("ticker/bookTicker, one symbol", 2, by_symbol("/api/v3/ticker/bookTicker?symbol={s}")),
    ("ticker/bookTicker, all", 4,
     ["/api/v3/ticker/bookTicker", "/api/v3/ticker/bookTicker?symbolStatus=TRADING"]),
]


def call(path, base=BASE):
    req = urllib.request.Request(base + path, headers={"Accept-Encoding": "identity"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            body = r.read()
            return r.status, dict(r.headers), body
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def usdm_main():
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


# ---- spot -------------------------------------------------------------------------


class Probe(NamedTuple):
    path: str
    status: int
    body: bytes
    used: int | None
    weight: int | None


def log(text):
    print(text, file=sys.stderr, flush=True)


def used_of(headers):
    lowered = {k.lower(): v for k, v in headers.items()}
    value = lowered.get(USED_HEADER)
    return int(value) if value is not None else None


def utc_date():
    return datetime.now(timezone.utc).strftime("%Y-%m-%d")


def clock(epoch):
    moment = datetime.fromtimestamp(epoch, timezone.utc)
    return f"{moment:%H:%M:%S}.{moment.microsecond // 100000}"


def wait_for_room(seconds):
    """Sleeps into the next UTC minute when fewer than `seconds` remain in this one, so a
    measurement does not straddle the window's reset."""
    left = 60 - time.time() % 60
    if left < seconds:
        time.sleep(left + 1.0)


def guard(status, path):
    if status in (418, 429):
        sys.exit(f"{path}: HTTP {status}; the host is limiting this address. "
                 "Stop and wait out its Retry-After.")


def measure(paths, base=SPOT_BASE):
    """One Probe per path, in order. The weight is the rise in the used-weight header over
    the call before it (a `time` call gives the first one its baseline); it is None when the
    header is missing or fell, which is a minute rollover."""
    wait_for_room(8 + 2 * len(paths))
    status, headers, _ = call("/api/v3/time", base)
    guard(status, "/api/v3/time")
    prev = used_of(headers)
    probes = []
    for path in paths:
        status, headers, body = call(path, base)
        guard(status, path)
        used = used_of(headers)
        weight = used - prev if used is not None and prev is not None and used >= prev else None
        probes.append(Probe(path, status, body, used, weight))
        if used is not None:
            prev = used
    return probes


def weight_verdict(weights, expected):
    """`agrees` when every distinct URL cost the spec's weight, else `DIFFERS`."""
    if weights and all(w == expected for w in weights):
        return "agrees"
    return "DIFFERS"


def window_verdict(samples):
    """`samples` is [(epoch seconds, used weight)] in time order. Returns a code and a
    sentence: the window is the UTC clock minute when the first fall in the reading happens
    between two samples taken in different UTC minutes."""
    for (t0, u0), (t1, u1) in zip(samples, samples[1:]):
        if u0 is None or u1 is None or u1 >= u0:
            continue
        fell = f"the reading fell from {u0} at {clock(t0)} to {u1} at {clock(t1)}"
        if int(t0 // 60) != int(t1 // 60):
            return "UTC_MINUTE", f"The window is the UTC clock minute: {fell}, across a minute boundary."
        return "NOT_UTC_MINUTE", f"The window is NOT the UTC clock minute: {fell}, inside one minute."
    return "NO_DROP", (f"The reading never fell in {len(samples)} samples: the run did not cross "
                       "a boundary, or the window slides. Run the section again.")


def counter_verdict(calls):
    """`calls` is [(host, weight, reading)] in order, hosts alternating. Shared when every
    reading after the first is the other host's last reading plus its own weight; separate
    when every reading after the second is the same host's last reading plus its own weight
    and none fits the shared rule."""
    if len(calls) < 4 or any(used is None for _, _, used in calls):
        return "INCONCLUSIVE", "Fewer than four readings, or a reading without the header."
    shared = [calls[i][2] == calls[i - 1][2] + calls[i][1] for i in range(1, len(calls))]
    own = [calls[i][2] == calls[i - 2][2] + calls[i][1] for i in range(2, len(calls))]
    if all(shared):
        return "SHARED", ("One counter: each host's reading is the other host's last reading "
                          "plus its own call's weight.")
    if all(own) and not any(shared):
        return "NOT_SHARED", ("Two counters: each host's reading is its own last reading "
                              "plus its own call's weight, whatever the other host was last asked.")
    return "INCONCLUSIVE", ("The readings fit neither rule (other traffic from this address, or "
                            "a minute rollover). Run the section again.")


def json_of(body):
    try:
        return json.loads(body)
    except ValueError:
        return None


def rate_limits_sentence(body):
    info = json_of(body)
    try:
        parts = [f"{r['rateLimitType']} {r['limit']} per {r['intervalNum']} {r['interval'].lower()}"
                 for r in info["rateLimits"]]
    except (KeyError, TypeError):
        return "`exchangeInfo`'s `rateLimits` could not be read."
    return "`exchangeInfo`'s `rateLimits`: " + "; ".join(parts) + "."


def weights_section():
    log("weights: every spot route, over distinct URLs")
    lines = [
        "### REST weights",
        "",
        f"Measured {utc_date()} on `api.binance.com`: the rise in `X-MBX-USED-WEIGHT-1M` across "
        "back-to-back requests inside one UTC minute, after a `time` call for the baseline. Each "
        "row is measured over distinct URLs (symbols and limits vary within a row), so a cached "
        "answer cannot stand in for a repeat. `Result` compares every URL with the design "
        "spec's section 3.1. `docs/specs/binance/probes/probe_rest.py --product spot --section "
        "weights` re-runs it.",
        "",
        "| Row | Requests | Status | Measured | Spec §3.1 | Result |",
        "|---|---|---|---|---|---|",
    ]
    limits = None
    for label, expected, paths in SPOT_ROWS:
        probes = measure(paths)
        weights = [p.weight for p in probes]
        verdict = weight_verdict(weights, expected)
        if any(p.status != 200 for p in probes):
            verdict = "REFUSED"
        if label == "exchangeInfo, without symbol":
            limits = rate_limits_sentence(probes[0].body)
        log(f"  {label}: {weights} (spec {expected}) {verdict}")
        requests = "; ".join(f"`{p.path}`" for p in probes)
        statuses = ", ".join(str(p.status) for p in probes)
        measured = ", ".join("?" if w is None else str(w) for w in weights)
        lines.append(f"| {label} | {requests} | {statuses} | {measured} | {expected} | {verdict} |")
    if limits:
        lines += ["", limits]
    return lines


def rows(body):
    return len(body)


def levels(body):
    return len(body["bids"]), len(body["asks"])


def code(body):
    return body["code"]


def limits_section():
    log("limits: past a route's maximum, depth at any limit, a two-hour window, an unknown symbol")
    start = int(time.time() * 1000) - 3 * 3600 * 1000
    end = start + 2 * 3600 * 1000
    # (row, path, the status the spec expects, how to read the answer, what it reads, how to show it)
    checks: list[tuple[str, str, int, Callable, object, Callable]] = [
        ("klines, limit 1001", "/api/v3/klines?symbol=BTCUSDT&interval=1m&limit=1001", 200,
         rows, 1000, lambda g: f"{g} rows"),
        ("trades, limit 1001", "/api/v3/trades?symbol=BTCUSDT&limit=1001", 200,
         rows, 1000, lambda g: f"{g} rows"),
        ("aggTrades, limit 1001", "/api/v3/aggTrades?symbol=BTCUSDT&limit=1001", 200,
         rows, 1000, lambda g: f"{g} rows"),
        ("depth, limit 5001", "/api/v3/depth?symbol=BTCUSDT&limit=5001", 200,
         levels, (5000, 5000), lambda g: f"{g[0]} bids, {g[1]} asks"),
        ("depth, limit 7", "/api/v3/depth?symbol=ETHUSDT&limit=7", 200,
         levels, (7, 7), lambda g: f"{g[0]} bids, {g[1]} asks"),
        ("aggTrades, a two-hour window",
         f"/api/v3/aggTrades?symbol=BTCUSDT&startTime={start}&endTime={end}", 200,
         rows, 500, lambda g: f"{g} rows"),
        ("depth, unknown symbol", "/api/v3/depth?symbol=NOPEUSDT", 400,
         code, -1121, lambda g: f"code {g}"),
    ]
    lines = [
        "### Limits past a route's maximum",
        "",
        f"Measured {utc_date()} on `api.binance.com`. `Weight` is the rise in `X-MBX-USED-WEIGHT-1M` "
        "over a `time` call before it; a refused request still shows what it cost. "
        "`docs/specs/binance/probes/probe_rest.py --product spot --section limits` re-runs it.",
        "",
        "| Request | Status | Returned | Weight | Spec §3.1 says | Result |",
        "|---|---|---|---|---|---|",
    ]
    for label, path, want_status, read, want, show in checks:
        (probe,) = measure([path])
        try:
            got = read(json_of(probe.body))
        except (KeyError, TypeError):
            got = None
        agrees = probe.status == want_status and got == want
        returned = "unreadable" if got is None else show(got)
        weight = "?" if probe.weight is None else str(probe.weight)
        log(f"  {label}: status {probe.status}, {returned}, weight {weight}")
        lines.append(f"| `{path}` | {probe.status} | {returned} | {weight} | "
                     f"{want_status}, {show(want)} | {'agrees' if agrees else 'DIFFERS'} |")
    return lines


def window_section():
    log("window: a ping every 3 s across a minute boundary (about 70 s)")
    time.sleep((48 - time.time() % 60) % 60)
    samples = []
    for _ in range(22):
        began = time.time()
        status, headers, _ = call("/api/v3/ping", SPOT_BASE)
        guard(status, "/api/v3/ping")
        samples.append((began, used_of(headers)))
        time.sleep(max(0.0, 3 - (time.time() - began)))
    _, sentence = window_verdict(samples)
    log(f"  {sentence}")
    lines = [
        "### The weight window",
        "",
        f"Measured {utc_date()} on `api.binance.com`: a `ping` every 3 s from 12 s before a UTC "
        f"minute boundary, {len(samples)} readings of `X-MBX-USED-WEIGHT-1M`. "
        "A sliding 60-second window would not fall to the weight of the last few calls at the "
        "boundary. `docs/specs/binance/probes/probe_rest.py --product spot --section window` "
        "re-runs it.",
        "",
        FENCE + "text",
    ]
    lines += [f"{clock(t)} used={u}" for t, u in samples]
    lines += [FENCE, "", sentence]
    return lines


def counter_section():
    log("counter: alternating api.binance.com and data-api.binance.vision")
    wait_for_room(30)
    sequence = []
    for symbol in COUNTER_SYMBOLS:
        sequence.append(("api.binance.com", SPOT_BASE, 20, f"/api/v3/exchangeInfo?symbol={symbol}"))
        sequence.append(("data-api.binance.vision", VISION_BASE, 2, f"/api/v3/ticker/price?symbol={symbol}"))
    calls, shown = [], []
    for index, (host, base, weight, path) in enumerate(sequence):
        status, headers, _ = call(path, base)
        guard(status, path)
        used = used_of(headers)
        calls.append((host, weight, used))
        shared = "-" if index < 1 or calls[index - 1][2] is None else str(calls[index - 1][2] + weight)
        own = "-" if index < 2 or calls[index - 2][2] is None else str(calls[index - 2][2] + weight)
        shown.append(f"| {index + 1} | `{host}` | `{path}` | {weight} | {used} | {shared} | {own} |")
        log(f"  {host} {path}: status {status}, reading {used}")
    _, sentence = counter_verdict(calls)
    lines = [
        "### One counter across hosts",
        "",
        f"Measured {utc_date()}: alternating calls on `api.binance.com` (`exchangeInfo?symbol=`, "
        "weight 20) and `data-api.binance.vision` (`ticker/price?symbol=`, weight 2), each "
        "read against the other host's last value, one symbol per pair so no URL repeats. If "
        "the counter is shared, a reading is the other host's last reading plus its own call's "
        "weight (`If shared`); if not, it is the same host's last reading plus its own "
        "(`If separate`). `docs/specs/binance/probes/probe_rest.py --product spot --section counter` "
        "re-runs it.",
        "",
        "| # | Host | Request | Weight | Reading | If shared | If separate |",
        "|---|---|---|---|---|---|---|",
    ]
    lines += shown
    lines += ["", sentence]
    return lines


SPOT_SECTIONS = {
    "weights": weights_section,
    "limits": limits_section,
    "window": window_section,
    "counter": counter_section,
}


def spot_main(section):
    sys.stdout.reconfigure(encoding="utf-8")
    names = [section] if section else list(SPOT_SECTIONS)
    out = []
    for name in names:
        out += SPOT_SECTIONS[name]() + [""]
    print("\n".join(out))
    flagged = sum(1 for line in out if line.endswith(("| DIFFERS |", "| REFUSED |")))
    if flagged:
        log(f"{flagged} row(s) DIFFER from the design spec or were refused: say so in OBSERVED.md "
            "and tell the owner before the spot client is written.")


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--product", choices=("usdm", "spot"), default="usdm",
                        help="the host to measure (default: usdm)")
    parser.add_argument("--section", choices=tuple(SPOT_SECTIONS),
                        help="with --product spot, run one section (default: all four)")
    args = parser.parse_args(argv)
    if args.section and args.product != "spot":
        parser.error("--section needs --product spot")
    return args


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.product == "spot":
        spot_main(args.section)
    else:
        usdm_main()


if __name__ == "__main__":
    main()
