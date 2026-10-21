#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["certifi"]
# ///
"""Capture Kalshi's public REST responses as test fixtures for polyoxide-kalshi's
`predictions` module.

Usage: uv run scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions

Reads production (https://external-api.kalshi.com) with no credentials, at the anonymous
default of 10 tokens a request, at most a few hundred requests in all. It picks an active market with
a quote on both sides, a two-sided book with more than one level on some side and a full page of
100 trades by paging `markets?status=open&mve_filter=exclude`:
the filter is needed, since on 2026-10-18 the unfiltered open listing's first ten pages held
9,996 multivariate combinations of 10,000 rows and no quoted single market, while the filtered
listing's first page held 579 quoted rows. It then writes the fixtures and PROVENANCE.md
into OUT_DIR. Nothing is written until every capture has succeeded, so a failed run leaves the
fixtures as they were. Every key is kept; only list lengths are trimmed, so a wire-agreement test
sees each field the host sends. Review the diff before committing.

Epic 9 adds two modes, each with an option first:

    uv run scripts/capture_kalshi_predictions.py --candlesticks polyoxide-kalshi/tests/fixtures/predictions [--ticker T]
    uv run scripts/capture_kalshi_predictions.py --check-candlesticks polyoxide-kalshi/tests/fixtures/predictions

`--candlesticks` reads `GET /markets/candlesticks` at periods 1, 60 and 1440 for one open market, the
series route at 60, the series route at 5 (its 400), and the batch route at 5 and over 10,000 rows
(probes, kept only in OBSERVED.md), and writes OUT_DIR/candlesticks/ and a block of OBSERVED.md above
its Epic 9 anchor. `--check-candlesticks` makes no request: it re-reads the files and exits 1 naming
what is wrong. Neither mode touches the other fixtures.
"""
import itertools
import json
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
from pathlib import Path

import capture_common

BASE = "https://external-api.kalshi.com/trade-api/v2"
PAUSE = 0.25
# Pages of the filtered listing to walk before giving up, and the rows of each.
PAGES = 10
PAGE_SIZE = 1000
# How many quoted candidates to confirm on the orderbook route before giving up.
CANDIDATES = 150
# Trades a candidate's first page must hold: the fixture is a full page of the route's limit.
TRADES_PAGE = 100
# Levels kept on each side of orderbook.json, the best ones.
BOOK_LEVELS = 5
# Markets kept in event.json's nested list, the picked one first.
EVENT_MARKETS = 3
UNKNOWN_TICKER = "NOSUCHMARKET-0"
NO_SIGNATURE_PATH = "/account/limits"
# Kalshi's rate-limit page gives a 429's body as this; provoking one would spend the quota
# the nightly runs share.
RATE_LIMITED_BODY = {"error": "too many requests"}

OBSERVED = Path(__file__).resolve().parent.parent / "docs" / "specs" / "kalshi" / "OBSERVED.md"
EPIC9_ANCHOR = "<!-- epic 9 findings go above this line -->"
CANDLES_BEGIN = "<!-- kalshi-candlesticks-capture:begin -->"
CANDLES_END = "<!-- kalshi-candlesticks-capture:end -->"
# The batch route's period widths in minutes, and the file each is written to.
WIDTHS = (1, 60, 1440)
BATCH_FILES = {"batch_1.json": 1, "batch_60.json": 60, "batch_1440.json": 1440}
CANDLE_FILES = (*BATCH_FILES, "series_60.json", "error_width.json")
# How far back from now each width is read, in seconds, never before the market's open.
SPAN_SECONDS = {1: 2 * 3600, 60: 7 * 86400, 1440: 120 * 86400}
# A width the enum lacks, sent to both routes.
WRONG_WIDTH = 5
# Candidates to try before giving up, tickers in the over-limit probe, and its span.
CANDLE_CANDIDATES = 60
BATCH_TICKERS = 100
OVER_LIMIT_SECONDS = 3 * 3600
BID_ASK_KEYS = {"open_dollars", "low_dollars", "high_dollars", "close_dollars"}


def identity(_method, _url):
    """Asks for an uncompressed body."""
    return {"Accept-Encoding": "identity"}


def fetch(path, **params):
    return capture_common.get(BASE + path, params=params, headers=identity, pause=PAUSE, timeout=60)


def fetch_ok(path, **params):
    return capture_common.require_ok(fetch(path, **params), f"GET {path}")


def refused(path, status, **params):
    """The JSON body of a reply the host refuses with `status`."""
    reply = fetch(path, **params)
    if reply.status != status or not reply.is_json:
        sys.exit(f"GET {path}: expected a JSON body with HTTP {status}, got HTTP {reply.status}: {reply.body!r}")
    return reply.body


def is_two_sided(market):
    """Active, with a YES bid above 0 and a YES ask below 1."""
    return (
        market["status"] == "active"
        and float(market["yes_bid_dollars"]) > 0
        and float(market["yes_ask_dollars"]) < 1
    )


def listing_rows():
    """The rows of `markets?status=open&mve_filter=exclude&limit=1000`, page by page, at most
    PAGES pages."""
    cursor = ""
    for _ in range(PAGES):
        params = {"status": "open", "mve_filter": "exclude", "limit": PAGE_SIZE}
        if cursor:
            params["cursor"] = cursor
        page = fetch_ok("/markets", **params)
        yield from page["markets"]
        cursor = page.get("cursor") or ""
        if not cursor:
            return


def pick_market():
    """The first listed market that quotes both sides and whose order book, read on the route the
    tests assert on, has levels on both and more than one on some side, so that `depth=1` differs
    from the full book, and whose `/markets/trades?limit=100` page holds a full 100 rows, so that the
    trades fixture has many trades to order. Returns the ticker, the market response, the book and
    the trades response."""
    tried = 0
    for row in listing_rows():
        if not is_two_sided(row):
            continue
        tried += 1
        if tried > CANDIDATES:
            break
        ticker = row["ticker"]
        book = fetch_ok(f"/markets/{ticker}/orderbook")
        sides = book["orderbook_fp"]
        if sides["yes_dollars"] and sides["no_dollars"] and max(len(v) for v in sides.values()) >= 2:
            trades = fetch_ok("/markets/trades", ticker=ticker, limit=TRADES_PAGE)
            if len(trades["trades"]) == TRADES_PAGE:
                return ticker, fetch_ok(f"/markets/{ticker}"), book, trades
    sys.exit(
        f"none of the first {CANDIDATES} quoted open markets has a two-sided book with a deeper side "
        f"and a full page of {TRADES_PAGE} trades; run again"
    )


def best_levels(book, count):
    """Keeps the `count` best levels of each side. The host lists a side worst first, so those
    are the last `count`."""
    return {"orderbook_fp": {side: levels[-count:] for side, levels in book["orderbook_fp"].items()}}


def provenance(ticker, event_ticker):
    today = capture_common.stamp("%Y-%m-%d")
    rows = [
        ("exchange_status.json", "GET /exchange/status", "as fetched"),
        ("endpoint_costs.json", "GET /account/endpoint_costs", "as fetched"),
        ("market.json", f"GET /markets/{ticker}", "as fetched"),
        ("markets_page.json", "GET /markets?status=open&mve_filter=exclude&limit=5", "as fetched"),
        (
            "orderbook.json",
            f"GET /markets/{ticker}/orderbook",
            f"the {BOOK_LEVELS} best levels of each side (the host lists a side worst first, so the last {BOOK_LEVELS})",
        ),
        ("orderbook_depth1.json", f"GET /markets/{ticker}/orderbook?depth=1", "as fetched"),
        ("trades.json", f"GET /markets/trades?ticker={ticker}&limit={TRADES_PAGE}", "as fetched"),
        (
            "event.json",
            f"GET /events/{event_ticker}?with_nested_markets=true",
            f"`event.markets`: the market above first, then up to {EVENT_MARKETS - 1} more",
        ),
        ("error_not_found.json", f"GET /markets/{UNKNOWN_TICKER}", "the 404 body"),
        ("error_validation.json", "GET /markets/trades?limit=5000", "the 400 body"),
        ("error_unauthenticated.json", f"GET {NO_SIGNATURE_PATH}", "the 401 body, sent with no signature"),
        (
            "error_rate_limited.json",
            "none",
            "written from the 429 body on Kalshi's rate-limit page (https://docs.kalshi.com/getting_started/rate_limits), not captured",
        ),
    ]
    table = capture_common.provenance_table(
        ["File", "Request", "Trimmed to"],
        [(f"`{name}`", request if request == "none" else f"`{request}`", note) for name, request, note in rows],
    )
    return [
        "# Provenance",
        "",
        f"Captured {today} from `https://external-api.kalshi.com/trade-api/v2` by",
        "`scripts/capture_kalshi_predictions.py`. No credentials. Every key is kept; only list",
        "lengths are trimmed.",
        "",
        f"The market is `{ticker}`, picked from `markets?status=open&mve_filter=exclude` because it was",
        "active, quoted both sides, had an order book with levels on both and more than one level on some",
        f"side, and had a full page of {TRADES_PAGE} trades. The filter is needed: the",
        "unfiltered open listing serves multivariate combinations first (see `docs/specs/kalshi/OBSERVED.md`).",
        "",
        *table,
    ]


def opened(market):
    """The market's `open_time` as Unix seconds."""
    return int(datetime.fromisoformat(market["open_time"].replace("Z", "+00:00")).timestamp())


def fmt_ts(ts):
    return datetime.fromtimestamp(ts, timezone.utc).strftime("%Y-%m-%d %H:%MZ")


def rows_of(body):
    """The candlesticks of a batch answer's first market, or an empty list."""
    markets = body.get("markets") if isinstance(body, dict) else None
    return (markets[0].get("candlesticks") or []) if markets else []


def span(width, now, open_ts):
    """The seconds read at `width`: the last SPAN_SECONDS, but not before the market opened."""
    return max(now - SPAN_SECONDS[width], open_ts), now


def fetch_batch(tickers, width, start, end):
    return fetch("/markets/candlesticks", market_tickers=tickers, start_ts=start, end_ts=end, period_interval=width)


def candidates(wanted):
    """Open markets that quote both sides: the one asked for, else the filtered listing's rows."""
    if wanted:
        yield fetch_ok(f"/markets/{wanted}")["market"]
        return
    for row in listing_rows():
        if is_two_sided(row):
            yield row


def read_widths(market, now):
    """`(bodies, "")` with each width's `(start, end, body)` when the market has a traded and an
    untraded period at every width, else `(None, why)`."""
    bodies = {}
    for width in (60, 1, 1440):
        start, end = span(width, now, opened(market))
        body = capture_common.require_ok(
            fetch_batch(market["ticker"], width, start, end), f"GET /markets/candlesticks at {width}"
        )
        kinds = Counter(kind(row) for row in rows_of(body))
        if not (kinds["traded"] and kinds["untraded"]):
            return None, f"width {width} ({dict(kinds)})"
        bodies[width] = (start, end, body)
    return bodies, ""


def pick_candle_market(wanted, now):
    """The first candidate whose answers hold every kind of period, preferring one that also holds an
    empty period (a period before its first trade); else the first without one."""
    fallback = None
    for tried, market in enumerate(candidates(wanted), start=1):
        if tried > CANDLE_CANDIDATES:
            break
        bodies, why = read_widths(market, now)
        if bodies is None:
            if wanted:
                sys.exit(f"{wanted} lacks a traded or an untraded period at {why}: pass another --ticker")
            continue
        picked = (market, bodies)
        if any(kind(row) == "empty" for _s, _e, body in bodies.values() for row in rows_of(body)):
            return picked
        fallback = fallback or picked
    if fallback:
        return fallback
    sys.exit(
        f"none of the first {CANDLE_CANDIDATES} quoted open markets has a traded and an untraded period at "
        "every width; run again, or pass --ticker"
    )


def over_limit_probe(now):
    """One batch for up to BATCH_TICKERS open markets at width 1 over OVER_LIMIT_SECONDS, which is
    more than the route's 10,000 rows when the markets all have rows; what the host does with it."""
    tickers = [row["ticker"] for row in itertools.islice(listing_rows(), BATCH_TICKERS)]
    reply = fetch_batch(",".join(tickers), 1, now - OVER_LIMIT_SECONDS, now)
    info = {"tickers": len(tickers), "status": reply.status}
    if reply.is_json and isinstance(reply.body, dict) and isinstance(reply.body.get("markets"), list):
        counts = [len(entry.get("candlesticks") or []) for entry in reply.body["markets"]]
        info.update(
            keys=sorted(reply.body),
            markets=len(counts),
            rows=sum(counts),
            fewest=min(counts, default=0),
            most=max(counts, default=0),
        )
    else:
        info["body"] = str(reply.body)[:300]
    return info


def d6_problems(rows):
    """What in `rows` would contradict spec D6's reading of the wire: a distribution whose keys are
    not the four, a value that is not a decimal string, an `end_period_ts` that is not an integer."""
    found = []

    def decimal(value, where):
        if not isinstance(value, str):
            found.append(f"{where} is {value!r}, not a string")
            return
        try:
            Decimal(value)
        except InvalidOperation:
            found.append(f"{where} is {value!r}, not a decimal")

    for row in rows:
        if not isinstance(row.get("end_period_ts"), int):
            found.append(f"end_period_ts is {row.get('end_period_ts')!r}")
        for side in ("yes_bid", "yes_ask"):
            dist = row.get(side)
            if not isinstance(dist, dict) or set(dist) != BID_ASK_KEYS:
                found.append(f"{side} is {dist!r}")
                continue
            for key, value in dist.items():
                decimal(value, f"{side}.{key}")
        for key in ("volume_fp", "open_interest_fp"):
            decimal(row.get(key), key)
        for key, value in (row.get("price") or {}).items():
            if value is not None:
                decimal(value, f"price.{key}")
    return sorted(set(found))[:5]


def describe(width, start, end, rows):
    kinds = Counter(kind(row) for row in rows)
    ends = [row["end_period_ts"] for row in rows]
    if ends == sorted(ends):
        order = "ascending"
    elif ends == sorted(ends, reverse=True):
        order = "descending"
    else:
        order = "unordered"
    modulus = 86400 if width == 1440 else width * 60
    offsets = sorted({ts % modulus for ts in ends})
    shown = ", ".join(str(offset) for offset in offsets)
    if width == 1440:
        clock = ", ".join(f"{o // 3600:02d}:{o % 3600 // 60:02d} UTC" for o in offsets)
        shown = f"{shown} ({clock})"
    price_keys = sorted({key for row in rows for key in (row.get("price") or {})})
    return (
        str(width),
        f"{fmt_ts(start)} to {fmt_ts(end)}",
        str(len(rows)),
        f"{kinds['traded']} / {kinds['untraded']} / {kinds['empty']}",
        order,
        shown,
        ", ".join(f"`{key}`" for key in price_keys) or "none",
    )


def section(info, stamp):
    """The OBSERVED.md block for one run."""
    rows = info["rows"]
    every = [row for width in WIDTHS for row in rows[width]]
    table = [describe(width, *info["spans"][width], rows[width]) for width in WIDTHS]
    prices = [row.get("price") or {} for row in every]
    min_max = any("min_dollars" in price or "max_dollars" in price for price in prices)
    nulls = any(value is None for price in prices for value in price.values())
    extra = sorted({key for row in every if kind(row) == "untraded" for key in row["price"] if key != "previous_dollars"})
    empty = next((row for row in every if kind(row) == "empty"), None)
    firsts = []
    for width in WIDTHS:
        traded = sorted((r for r in rows[width] if kind(r) == "traded"), key=lambda r: r["end_period_ts"])
        has = "has" if "previous_dollars" in traded[0]["price"] else "lacks"
        start = info["spans"][width][0]
        where = "the span starts at the market's open" if start == info["open_ts"] else "the span starts after the market opened"
        firsts.append(f"{width}: the earliest traded period {has} `previous_dollars` ({where})")
    problems = d6_problems(every)
    if empty is None:
        empty_text = "No period before the market's first trade was in any span: this market has none to show."
    else:
        empty_text = (
            "A period before the first trade held `price` = "
            f"`{json.dumps(empty['price'])}`, `yes_bid` = `{json.dumps(empty['yes_bid'])}`, "
            f"`yes_ask` = `{json.dumps(empty['yes_ask'])}`, `volume_fp` = `{empty['volume_fp']}`."
        )
    series_same = info["series"]["candlesticks"] == rows[60]
    series_text = (
        "identical to the batch route's rows"
        if series_same
        else f"different from the batch route's rows ({len(info['series']['candlesticks'])} against {len(rows[60])})"
    )
    wrong = info["wrong"]
    over = info["over"]
    if "rows" in over:
        over_text = (
            f"`{over['status']}` with keys {', '.join('`' + k + '`' for k in over['keys'])}: {over['markets']} markets, "
            f"{over['rows']} rows in all ({over['fewest']} to {over['most']} each). "
            + ("That is within the route's 10,000." if over["rows"] <= 10000 else "That is past the route's 10,000: the host did not refuse it.")
        )
    else:
        over_text = f"`{over['status']}` with no market list: {over['body']}"
    lines = [
        "### Candlesticks, production, unsigned",
        "",
        f"Captured {stamp} by `scripts/capture_kalshi_predictions.py --candlesticks` from `{BASE}` on `{info['ticker']}` "
        f"(series `{info['series_ticker']}`, opened {fmt_ts(info['open_ts'])}). Fixtures: "
        "`polyoxide-kalshi/tests/fixtures/predictions/candlesticks/`.",
        "",
        *capture_common.provenance_table(
            (
                "Width (min)",
                "Span (UTC)",
                "Rows",
                "Traded / untraded / empty",
                "Wire order by `end_period_ts`",
                "`end_period_ts` mod the width (s)",
                "`price` keys seen",
            ),
            table,
        ),
        "",
        f"- `min_dollars` or `max_dollars` in any `price`: {'yes' if min_max else 'never'}. A `null` in any `price`: {'yes' if nulls else 'never, the keys are omitted'}.",
        f"- An untraded period's `price` holds a key besides `previous_dollars`: {', '.join('`' + k + '`' for k in extra) if extra else 'no'}.",
        f"- {empty_text}",
        "- First traded period: " + "; ".join(firsts) + ".",
        f"- The series route at 60 answered `{sorted(info['series'])}`, {series_text}.",
        f"- Width {WRONG_WIDTH} on the batch route: `{wrong['status']}`, "
        + (f"{wrong['rows']} rows." if wrong["rows"] is not None else f"body `{wrong['body']}`."),
        f"- Width {WRONG_WIDTH} on the series route: `400`, body `{json.dumps(info['error_width'])}`.",
        f"- One batch for {over['tickers']} open markets at width 1 over {OVER_LIMIT_SECONDS // 3600} hours: {over_text}",
        "- Spec D6's reading of the wire (every `*_dollars` and `*_fp` a decimal string, `yes_bid` and `yes_ask` carrying exactly the four distribution keys, `end_period_ts` an integer): "
        + ("holds in every captured row." if not problems else "**DOES NOT HOLD: " + "; ".join(problems) + ". Report this to the owner before Story 3 runs.**"),
    ]
    return "\n".join(lines)


def record_candle_findings(text):
    """Writes `text` into OBSERVED.md between the capture's markers, replacing an earlier run's, or
    directly above the Epic 9 anchor when there is none."""
    current = OBSERVED.read_text(encoding="utf-8")
    if EPIC9_ANCHOR not in current:
        sys.exit(f"{OBSERVED} has no anchor line {EPIC9_ANCHOR!r}: Task 1.2 writes it")
    block = f"{CANDLES_BEGIN}\n{text.rstrip()}\n{CANDLES_END}"
    if CANDLES_BEGIN in current and CANDLES_END in current:
        head, rest = current.split(CANDLES_BEGIN, 1)
        _, tail = rest.split(CANDLES_END, 1)
        current = head + block + tail
    else:
        current = current.replace(EPIC9_ANCHOR, f"{block}\n\n{EPIC9_ANCHOR}", 1)
    capture_common.write_raw(OBSERVED, current)


def provenance_candles(info, stamp):
    ticker, series = info["ticker"], info["series_ticker"]
    host = "external-api.kalshi.com"
    rows = []
    for name, width in BATCH_FILES.items():
        start, end = info["spans"][width]
        request = f"GET /markets/candlesticks?market_tickers={ticker}&period_interval={width}&start_ts={start}&end_ts={end}"
        rows.append((f"`{name}`", host, f"`{request}`", f"{fmt_ts(start)} to {fmt_ts(end)}", stamp))
    start, end = info["spans"][60]
    path = f"/series/{series}/markets/{ticker}/candlesticks"
    rows.append(("`series_60.json`", host, f"`GET {path}?period_interval=60&start_ts={start}&end_ts={end}`", f"{fmt_ts(start)} to {fmt_ts(end)}", stamp))
    rows.append(("`error_width.json`", host, f"`GET {path}?period_interval={WRONG_WIDTH}&start_ts={start}&end_ts={end}` (the 400 body)", f"{fmt_ts(start)} to {fmt_ts(end)}", stamp))
    return [
        "# Provenance",
        "",
        f"Captured {stamp} from `https://{host}/trade-api/v2` by `scripts/capture_kalshi_predictions.py --candlesticks`.",
        "No credentials. Every key is kept and no list is trimmed.",
        "",
        f"The market is `{ticker}` (series `{series}`), picked from `markets?status=open&mve_filter=exclude` because",
        "it quoted both sides and its answers held a traded and an untraded period at every width. Each span starts",
        "at the later of the market's open and the last 2 hours (width 1), 7 days (width 60) or 120 days (width 1440).",
        "",
        *capture_common.provenance_table(("File", "Host", "Request", "Span (UTC)", "Captured"), rows),
    ]


def capture_candlesticks(out, wanted):
    now = int(time.time())
    market, bodies = pick_candle_market(wanted, now)
    ticker = market["ticker"]
    series_ticker = fetch_ok(f"/events/{market['event_ticker']}")["event"]["series_ticker"]
    start, end, _body = bodies[60]
    path = f"/series/{series_ticker}/markets/{ticker}/candlesticks"
    series = fetch_ok(path, start_ts=start, end_ts=end, period_interval=60)
    error_width = refused(path, 400, start_ts=start, end_ts=end, period_interval=WRONG_WIDTH)
    wrong = fetch_batch(ticker, WRONG_WIDTH, start, end)
    wrong_rows = rows_of(wrong.body) if wrong.is_json else None
    over = over_limit_probe(now)
    stamp = capture_common.stamp()
    info = {
        "ticker": ticker,
        "series_ticker": series_ticker,
        "open_ts": opened(market),
        "spans": {width: (s, e) for width, (s, e, _b) in bodies.items()},
        "rows": {width: rows_of(body) for width, (_s, _e, body) in bodies.items()},
        "series": series,
        "error_width": error_width,
        "wrong": {"status": wrong.status, "rows": None if wrong_rows is None else len(wrong_rows), "body": str(wrong.body)[:200]},
        "over": over,
    }
    folder = out / "candlesticks"
    files = {name: bodies[width][2] for name, width in BATCH_FILES.items()}
    files["series_60.json"] = series
    files["error_width.json"] = error_width
    for name, value in files.items():
        capture_common.write_json(folder / name, value)
    capture_common.write_provenance(folder, provenance_candles(info, stamp))
    record_candle_findings(section(info, stamp))
    print(f"wrote {len(files)} fixtures and PROVENANCE.md to {folder} (market {ticker})")
    print(f"recorded the findings in {OBSERVED}")


def kind(row):
    """`traded` when the period has a close, `untraded` when it has only `previous_dollars`, else
    `empty`."""
    price = row.get("price") or {}
    if price.get("close_dollars") is not None:
        return "traded"
    if price.get("previous_dollars") is not None:
        return "untraded"
    return "empty"


def check_candlesticks(root):
    """Re-reads root/candlesticks/ and exits 1 naming each thing that is wrong."""
    folder = root / "candlesticks"
    problems = []

    def load(name):
        try:
            return json.loads((folder / name).read_text(encoding="utf-8"))
        except (OSError, ValueError) as err:
            problems.append(f"{name}: {err}")
            return None

    tickers = set()
    for name, width in BATCH_FILES.items():
        body = load(name)
        if body is None:
            continue
        markets = body.get("markets") if isinstance(body, dict) else None
        if not isinstance(markets, list) or len(markets) != 1:
            problems.append(f"{name}: expected exactly one entry in `markets`")
            continue
        tickers.add(markets[0].get("market_ticker"))
        rows = markets[0].get("candlesticks")
        if not isinstance(rows, list) or not all(
            isinstance(row.get("end_period_ts"), int) and isinstance(row.get("price"), dict) for row in rows
        ):
            problems.append(f"{name}: `candlesticks` is not a list of rows with an integer `end_period_ts` and a `price` object")
            continue
        kinds = Counter(kind(row) for row in rows)
        if not kinds["traded"]:
            problems.append(f"{name}: no traded period")
        if not kinds["untraded"]:
            problems.append(f"{name}: no untraded period carrying `previous_dollars`")
        modulus = 86400 if width == 1440 else width * 60
        offsets = {row["end_period_ts"] % modulus for row in rows}
        if len(offsets) > 1:
            problems.append(f"{name}: periods end at different offsets of the width: {sorted(offsets)}")
    if len(tickers) > 1:
        problems.append(f"the batch files name different markets: {sorted(map(str, tickers))}")

    series = load("series_60.json")
    if series is not None:
        if not isinstance(series, dict) or set(series) != {"candlesticks", "ticker"}:
            problems.append("series_60.json: expected exactly the keys `candlesticks` and `ticker`")
        elif tickers and series["ticker"] not in tickers:
            problems.append(f"series_60.json: ticker {series['ticker']!r} is not the batch files' {sorted(map(str, tickers))}")
    refusal = load("error_width.json")
    if refusal is not None and not (isinstance(refusal, dict) and refusal):
        problems.append("error_width.json: expected a non-empty JSON object")

    try:
        provenance = (folder / "PROVENANCE.md").read_text(encoding="utf-8")
    except OSError as err:
        problems.append(f"PROVENANCE.md: {err}")
    else:
        absent = [name for name in CANDLE_FILES if name not in provenance]
        if absent:
            problems.append("PROVENANCE.md does not mention " + ", ".join(absent))
    try:
        observed = OBSERVED.read_text(encoding="utf-8")
    except OSError as err:
        problems.append(f"{OBSERVED}: {err}")
    else:
        if CANDLES_BEGIN not in observed or CANDLES_END not in observed:
            problems.append(f"{OBSERVED} holds no block between {CANDLES_BEGIN} and {CANDLES_END}")

    for problem in problems:
        print(f"  {problem}")
    if problems:
        raise SystemExit(f"{len(problems)} problem(s) in {folder}")
    print(f"ok: {len(CANDLE_FILES)} candlestick fixtures in {folder}")


def epic9_command(args):
    """Runs one of Epic 9's two modes; `args` is `sys.argv[1:]`, whose first item starts with `--`."""
    if len(args) == 2 and args[0] == "--check-candlesticks":
        check_candlesticks(Path(args[1]))
        return
    if args[0] == "--candlesticks":
        rest = args[1:]
        wanted = None
        if len(rest) == 3 and rest[1] == "--ticker":
            wanted = rest[2]
            rest = rest[:1]
        if len(rest) == 1:
            capture_candlesticks(Path(rest[0]), wanted)
            return
    sys.exit(__doc__)


def main():
    args = sys.argv[1:]
    if args and args[0].startswith("--"):
        epic9_command(args)
        return
    if len(args) != 1:
        sys.exit(__doc__)
    out = Path(args[0])
    ticker, market, book, trades = pick_market()
    event_ticker = market["market"]["event_ticker"]

    event = fetch_ok(f"/events/{event_ticker}", with_nested_markets="true")
    nested = sorted(event["event"].get("markets") or [], key=lambda m: m["ticker"] != ticker)
    if not nested or nested[0]["ticker"] != ticker:
        sys.exit(f"/events/{event_ticker}?with_nested_markets=true does not list {ticker}")
    event["event"]["markets"] = nested[:EVENT_MARKETS]

    files = {
        "exchange_status.json": fetch_ok("/exchange/status"),
        "endpoint_costs.json": fetch_ok("/account/endpoint_costs"),
        "market.json": market,
        "markets_page.json": fetch_ok("/markets", status="open", mve_filter="exclude", limit=5),
        "orderbook.json": best_levels(book, BOOK_LEVELS),
        "orderbook_depth1.json": fetch_ok(f"/markets/{ticker}/orderbook", depth=1),
        "trades.json": trades,
        "event.json": event,
        "error_not_found.json": refused(f"/markets/{UNKNOWN_TICKER}", 404),
        "error_validation.json": refused("/markets/trades", 400, limit=5000),
        "error_unauthenticated.json": refused(NO_SIGNATURE_PATH, 401),
        "error_rate_limited.json": RATE_LIMITED_BODY,
    }
    for name, value in files.items():
        capture_common.write_json(out / name, value)
    capture_common.write_provenance(out, provenance(ticker, event_ticker))
    print(f"wrote {len(files)} fixtures and PROVENANCE.md to {out} (market {ticker})")


if __name__ == "__main__":
    main()
