# Kalshi catalog routes (issue #71) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (the owner chose inline execution, 2026-10-22). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `polyoxide-kalshi` the public routes a catalog sync needs (series, the events list, the `/markets` timestamp families and `tickers`, many books at once, the historical archive) and let the venue traits read markets archived past the cutoff.

**Architecture:** Nine routes join `request.rs`'s table, each a `Routed<T>` builder charged on the Read layer like every route today. A new `tickers.rs` holds the byte budget, its refusal and the chunker; refusals ride on `Routed` and surface as `ApiError::Validation` before anything is charged or sent. The `/markets` timestamp families become types (`MarketWindow`, `updated`), the archive's selectors one enum (`HistoricalSelection`). `venue.rs` reads live first and falls back to the archive only when the live answer says the market is not there.

**Tech Stack:** Rust (tokio, serde, rust_decimal, mockito), `polyoxide-core`'s `Routed`/`query_setters!`, `polyoxide-test-support` (`fixtures!`, `query`, `agreement`, `openapi`), Python 3 capture script (`scripts/capture_common.py`).

**Spec:** `docs/superpowers/specs/2026-10-22-kalshi-catalog-routes-design.md` (D1–D17). Where this plan and the spec disagree, the spec binds and the disagreement goes to the owner.

---

## Rules every story keeps

- **Run commands with the dev shell's prefix**, `nix develop --command`, from the worktree root. A story's done-check is its gates (spec §13), run in this order:

  ```bash
  nix develop --command cargo fmt --all -- --check
  nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings
  nix develop --command cargo test -p polyoxide-kalshi --all-features
  nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'
  nix develop --command python3 scripts/gen_registry.py --check
  nix develop --command python3 scripts/venue_gate.py
  nix develop --command python3 scripts/live_unwraps.py
  nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
  bash _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.sh
  ```

  Use `CARGO_INCREMENTAL=0` if the disk fills (memory: local build OOM).
- **The touch-set gate** (Story 1 writes it) prints every changed, staged or untracked path since `git merge-base main HEAD` that `issue-71-touch-set.txt` does not admit, and fails when it prints one. An edit outside the allowlist stops the story and goes to the owner.
- **AD-12.** Add tests; never change an existing test's body, except D16's eight edits, each in a task of its own (Tasks 2.4 and 7.4). Any other existing test a change breaks goes to the owner.
- **Live tests** use test-support's loaders and tags (`or_fail`, `fail`, `environmental`), never `.unwrap()`/`.expect(`; `scripts/live_unwraps.py` holds that.
- **Mirrors stay byte for byte**: `docs/specs/kalshi/openapi.yaml` is never edited.
- **Commits** by pathspec (`git commit -- <paths>`), conventional, naming the story, never `--no-verify`, ending with the session's attribution lines.

## File map

| File | Story | Responsibility |
|---|---|---|
| `scripts/capture_kalshi_predictions.py` | 1 | `--catalog` mode: captures and probes |
| `polyoxide-kalshi/tests/fixtures/predictions/catalog/*`, `…/historical/*` | 1 | captured fixtures, two `PROVENANCE.md` |
| `docs/specs/kalshi/OBSERVED.md` | 1, 8 | `## Issue #71: catalog routes` |
| `_bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.{txt,sh}` | 1 | the touch-set gate |
| `polyoxide-kalshi/src/predictions/tickers.rs` | 2 | budget, refusal, `ticker_chunks` |
| `polyoxide-kalshi/src/predictions/request.rs` | 2 | nine routes, `query_many`, `refuse`, `tickers_joined`, `tickers_repeated` |
| `polyoxide-kalshi/src/predictions/mod.rs` | 2, 4, 6 | `mod tickers` and its re-exports; `series()`, `historical()` |
| `polyoxide-kalshi/src/predictions/api/markets.rs`, `types/markets.rs` | 3 | `tickers`, `window`, `updated`, `orderbooks` |
| `polyoxide-kalshi/src/predictions/api/series.rs`, `types/series.rs` | 4 | `series()` |
| `polyoxide-kalshi/src/predictions/api/events.rs`, `types/events.rs` | 5 | `events().list()` |
| `polyoxide-kalshi/src/predictions/api/historical.rs`, `types/historical.rs` | 6 | `historical()` |
| `polyoxide-kalshi/src/predictions/api/mod.rs`, `types/mod.rs` | 3–6 | re-exports, appended |
| `polyoxide-kalshi/src/predictions/venue.rs` | 7 | the fallback |
| `polyoxide-kalshi/tests/mock_predictions_markets_catalog.rs` | 3 | mock suite |
| `polyoxide-kalshi/tests/mock_predictions_series.rs` | 4 | mock suite |
| `polyoxide-kalshi/tests/mock_predictions_events.rs` | 5 | mock suite |
| `polyoxide-kalshi/tests/mock_predictions_historical.rs` | 6 | mock suite |
| `polyoxide-kalshi/tests/venue_predictions_historical.rs` | 7 | fallback tests |
| `polyoxide-kalshi/tests/{query_setters,spec_agreement,wire_agreement}_predictions.rs` | 3–6 | appended cases and tests |
| `polyoxide-kalshi/tests/{request.rs tests,mock_predictions_routes,spec_agreement_predictions,live_predictions}` | 2 | D16's route-table pins |
| `polyoxide-kalshi/tests/venue_predictions.rs` | 7 | D16's four venue edits |
| `polyoxide-kalshi/tests/live_predictions.rs`, `live_predictions_venue.rs` | 8 | appended live tests |
| `polyoxide-kalshi/src/predictions/README.md`, `CLAUDE.md` | 8 | records |

## Names contract

Each story may call these names once the story that defines them has landed, and no other story's private code.

- Story 2: `predictions::{TICKERS_QUERY_BUDGET, MAX_ORDERBOOK_TICKERS, ticker_chunks, TickerChunks}`; `request::routes::{SERIES_LIST, SERIES, EVENTS, ORDERBOOKS, HISTORICAL_CUTOFF, HISTORICAL_MARKETS, HISTORICAL_MARKET, HISTORICAL_CANDLESTICKS, HISTORICAL_TRADES}`; `Routed::{query_many, refuse, tickers_joined, tickers_repeated}`.
- Story 3: `types::{CreatedStatus, MarketWindow, MarketOrderbook, OrderbooksResponse}`; `api::{ListMarketWindow, ListUpdatedMarkets, GetOrderbooks}`; `MarketsApi::{window, updated, orderbooks}`; `ListMarkets::tickers`; in `spec_agreement_predictions.rs`, `reachable_from` and `unaccounted_from`; in `wire_agreement_predictions.rs`, `hold_exactly`.
- Story 4: `types::{Series, SettlementSource, SeriesList, SeriesResponse}`; `api::{SeriesApi, ListSeries, GetSeries}`; `Predictions::series`.
- Story 5: `types::{EventStatusFilter, EventsPage}`; `api::ListEvents`; `EventsApi::list`.
- Story 6: `types::{HistoricalCutoff, HistoricalSelection, HistoricalBidAsk, HistoricalPrice, HistoricalCandlestick, HistoricalCandlesticksResponse}`; `api::{HistoricalApi, GetHistoricalCutoff, ListHistoricalMarkets, GetHistoricalMarket, GetHistoricalCandlesticks, ListHistoricalTrades}`; `Predictions::historical`.

Fixture names (Story 1 writes them; Stories 3–8 read them through `fixtures!("predictions")`): `catalog/series_list`, `catalog/series`, `catalog/events_page`, `catalog/events_page_nested`, `catalog/orderbooks`, `catalog/markets_updated_page`, `catalog/markets_tickers_page`, `catalog/markets_window_settled_page`, `historical/cutoff`, `historical/markets_page`, `historical/market`, `historical/candlesticks_1`, `historical/candlesticks_60`, `historical/candlesticks_1440`, `historical/trades`.

---

## Story 1: probes, captures and the touch-set gate (spec §4)

No Rust.

### Task 1.1: the touch-set gate

**Files:**
- Create: `_bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.txt`
- Create: `_bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.sh`

- [ ] **Step 1: Write the allowlist**

`issue-71-touch-set.txt`, one extended regex per line:

```
^polyoxide-kalshi/
^docs/specs/kalshi/OBSERVED\.md$
^scripts/capture_kalshi_predictions\.py$
^CLAUDE\.md$
^docs/superpowers/specs/2026-10-22-kalshi-catalog-routes-design\.md$
^docs/plans/2026-10-22-kalshi-catalog-routes\.md$
^_bmad-output/specs/spec-venue-extensibility/issue-71-touch-set\.(txt|sh)$
^\.flow/
```

- [ ] **Step 2: Write the gate** (Epic 9's with `epic-9` replaced by `issue-71`)

```bash
#!/usr/bin/env bash
cd "$(git rev-parse --show-toplevel)" || exit 2
{ git diff --no-renames --name-only "$(git merge-base main HEAD)"; git ls-files --others --exclude-standard; } | sort -u | grep -v -E -f "$(dirname "$0")/issue-71-touch-set.txt"; test $? -eq 1
```

- [ ] **Step 3: Run it**

Run: `chmod +x _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.sh && bash _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.sh; echo "exit $?"`
Expected: no path printed, `exit 0` (the spec and plan are admitted).

### Task 1.2: the `--catalog` capture mode

**Files:**
- Modify: `scripts/capture_kalshi_predictions.py` (docstring, imports, a new section before `def epic9_command`, and `main`)

- [ ] **Step 1: Document the mode.** Append to the module docstring, before its closing `"""`:

```
Issue #71 adds a third mode:

    uv run scripts/capture_kalshi_predictions.py --catalog polyoxide-kalshi/tests/fixtures/predictions

`--catalog` reads the catalog routes (series, the events list, the `/markets` timestamp families and
`tickers`, `/markets/orderbooks`) and the archive (`/historical/*`), writes OUT_DIR/catalog/ and
OUT_DIR/historical/ with a PROVENANCE.md in each, and prints the probes of spec §4 as Markdown lines
for OBSERVED.md. It touches no other fixture, and writes nothing until every capture has succeeded.
```

- [ ] **Step 2: Import `urllib.parse`.** After `import time` add `import urllib.parse`.

- [ ] **Step 3: Add the mode's section** immediately before `def epic9_command(args):`

```python
# ── Issue #71: the catalog routes ───────────────────────────────

CATALOG_ROWS = 3
"""Rows kept in each listing fixture."""
NESTED_MARKETS = 2
"""Markets kept under each event of the nested events page."""
SERIES_CATEGORY = "Economics"
ARCHIVE_SERIES = "KXHIGHNY"
"""A daily series with years of settled markets, so its archive always holds some."""
ARCHIVE_SCAN = 200
MINUTE_ROWS = 120
"""Rows kept in the one-minute archived candlesticks."""
DAY = 86_400


def fetch_pairs(path, pairs):
    """GET `path` with the query `pairs` in order, a key repeated as often as it appears."""
    query = urllib.parse.urlencode(pairs)
    return capture_common.get(f"{BASE}{path}?{query}", headers=identity, pause=PAUSE, timeout=60)


def refusal_text(body):
    """What a refusal says: `error.details`, `error.message`, a bare `error`, or `msg`."""
    if not isinstance(body, dict):
        return repr(body)
    error = body.get("error")
    if isinstance(error, dict):
        return error.get("details") or error.get("message") or repr(error)
    return error or body.get("msg") or repr(body)


def answer(path, pairs, rows=None):
    """`HTTP <status>`, with the refusal's text, or the number of rows under `rows` on a 200."""
    reply = fetch_pairs(path, pairs)
    if reply.status != 200:
        return f"HTTP {reply.status} ({refusal_text(reply.body)})"
    if rows is None:
        return "HTTP 200"
    return f"HTTP 200, {len(reply.body.get(rows) or [])} {rows}"


def epoch(text):
    return int(datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp())


def trimmed(body, key, count):
    """`body` with the list under `key` cut to its first `count` rows."""
    return {**body, key: (body.get(key) or [])[:count]}


def traded_open_markets(count):
    """The first `count` markets of the filtered open listing that traded in the last day."""
    picked = []
    for market in listing_rows():
        if float(market.get("volume_24h_fp") or 0) > 0:
            picked.append(market)
            if len(picked) == count:
                return picked
    sys.exit(f"the filtered open listing has fewer than {count} markets that traded in the last day")


def archived_market():
    """The newest archived market of ARCHIVE_SERIES with volume that the live market route
    answers 404, so the venue fallback has something to read."""
    page = fetch_ok("/historical/markets", series_ticker=ARCHIVE_SERIES, limit=ARCHIVE_SCAN)
    for market in page["markets"]:
        if float(market.get("volume_fp") or 0) <= 0:
            continue
        if fetch(f"/markets/{market['ticker']}").status == 404:
            return market
    sys.exit(f"none of the newest {ARCHIVE_SCAN} archived {ARCHIVE_SERIES} markets with volume is gone from the live route")


def series_rows(series):
    """Up to CATALOG_ROWS series, one with `tags: null` among them when the page has one."""
    with_null = [row for row in series if row.get("tags") is None][:1]
    rest = [row for row in series if row not in with_null]
    return with_null + rest[: CATALOG_ROWS - len(with_null)]


def archived_candles(market, width):
    """The archived market's candlesticks of `width` minutes over its life and one width more."""
    start = epoch(market["open_time"])
    end = epoch(market["close_time"]) + width * 60
    body = fetch_ok(
        f"/historical/markets/{market['ticker']}/candlesticks",
        start_ts=start,
        end_ts=end,
        period_interval=width,
    )
    return trimmed(body, "candlesticks", MINUTE_ROWS) if width == 1 else body


def catalog_probes(active, event, series, archived, now):
    """Spec §4's probes, as Markdown lines for OBSERVED.md."""
    a, b = active[0]["ticker"], active[1]["ticker"]
    week = now - 7 * DAY
    lines = [f"Probed {capture_common.stamp()} on production, unsigned, by `--catalog`."]
    families = {
        "created (`status=open`, `min_created_ts`)": [("status", "open"), ("min_created_ts", week)],
        "closed (`status=closed`, `min_close_ts`)": [("status", "closed"), ("min_close_ts", week)],
        "settled (`status=settled`, `min_settled_ts`)": [("status", "settled"), ("min_settled_ts", week)],
        "updated (`min_updated_ts`)": [("min_updated_ts", now - DAY)],
    }
    extras = {
        "tickers": [("tickers", f"{a},{b}")],
        "event_ticker": [("event_ticker", event)],
        "series_ticker": [("series_ticker", series)],
        "mve_filter=only": [("mve_filter", "only")],
        "mve_filter=exclude": [("mve_filter", "exclude")],
    }
    for family, pairs in families.items():
        for name, extra in extras.items():
            got = answer("/markets", pairs + extra + [("limit", 1)], "markets")
            lines.append(f"- `GET /markets`, {family} with `{name}`: {got}")
    books = capture_common.require_ok(
        fetch_pairs("/markets/orderbooks", [("tickers", b), ("tickers", a), ("tickers", b)]),
        "GET /markets/orderbooks",
    )
    order = [row["ticker"] for row in books["orderbooks"]]
    lines.append(f"- `GET /markets/orderbooks?tickers={b}&tickers={a}&tickers={b}`: books of {order}")
    lines.append(
        f"- `GET /historical/trades?ticker={UNKNOWN_TICKER}`: "
        + answer("/historical/trades", [("ticker", UNKNOWN_TICKER), ("limit", 5)], "trades")
    )
    start, end = epoch(archived["open_time"]), epoch(archived["close_time"])
    lines.append(
        f"- `GET /historical/markets/{UNKNOWN_TICKER}/candlesticks`: "
        + answer(f"/historical/markets/{UNKNOWN_TICKER}/candlesticks",
                 [("start_ts", start), ("end_ts", end), ("period_interval", 60)])
    )
    over = [("start_ts", end - 10_001 * 60), ("end_ts", end), ("period_interval", 1)]
    lines.append(
        f"- `GET /historical/markets/{archived['ticker']}/candlesticks` over 10,001 one-minute periods: "
        + answer(f"/historical/markets/{archived['ticker']}/candlesticks", over, "candlesticks")
    )
    lines.append("- `GET /events?limit=201`: " + answer("/events", [("limit", 201)], "events"))
    lines.append("- `GET /series/NOSUCHSERIES-0`: " + answer("/series/NOSUCHSERIES-0", []))
    return lines


def catalog_provenance(rows, what):
    table = capture_common.provenance_table(
        ["File", "Request", "Trimmed to"],
        [(f"`{name}`", f"`{request}`", note) for name, request, note in rows],
    )
    return [
        "# Provenance",
        "",
        f"Captured {capture_common.stamp('%Y-%m-%d')} from `https://external-api.kalshi.com/trade-api/v2` by",
        "`scripts/capture_kalshi_predictions.py --catalog`. No credentials. Every key is kept; only list",
        f"lengths are trimmed. {what}",
        "",
        *table,
    ]


def capture_catalog(out):
    now = int(time.time())
    active = traded_open_markets(2)
    a, b = active[0]["ticker"], active[1]["ticker"]
    event_ticker = active[0]["event_ticker"]
    series_ticker = fetch_ok(f"/events/{event_ticker}")["event"]["series_ticker"]
    archived = archived_market()
    t = archived["ticker"]

    nested = fetch_ok("/events", status="open", limit=2, with_nested_markets="true")
    for event in nested["events"]:
        event["markets"] = (event.get("markets") or [])[:NESTED_MARKETS]
    books = capture_common.require_ok(
        fetch_pairs("/markets/orderbooks", [("tickers", a), ("tickers", b), ("tickers", UNKNOWN_TICKER)]),
        "GET /markets/orderbooks",
    )
    for row in books["orderbooks"]:
        row["orderbook_fp"] = best_levels(row, BOOK_LEVELS)["orderbook_fp"]
    catalog = {
        "series_list.json": {
            "series": series_rows(fetch_ok("/series", category=SERIES_CATEGORY)["series"])
        },
        "series.json": fetch_ok(f"/series/{series_ticker}", include_volume="true"),
        "events_page.json": fetch_ok("/events", status="open", limit=CATALOG_ROWS),
        "events_page_nested.json": nested,
        "orderbooks.json": books,
        "markets_updated_page.json": fetch_ok(
            "/markets", min_updated_ts=now - DAY, mve_filter="exclude", limit=CATALOG_ROWS
        ),
        "markets_tickers_page.json": fetch_ok("/markets", tickers=f"{a},{b}"),
        "markets_window_settled_page.json": fetch_ok(
            "/markets", status="settled", min_settled_ts=now - 7 * DAY, mve_filter="exclude", limit=CATALOG_ROWS
        ),
    }
    historical = {
        "cutoff.json": fetch_ok("/historical/cutoff"),
        "markets_page.json": fetch_ok("/historical/markets", series_ticker=ARCHIVE_SERIES, limit=CATALOG_ROWS),
        "market.json": fetch_ok(f"/historical/markets/{t}"),
        **{f"candlesticks_{w}.json": archived_candles(archived, w) for w in WIDTHS},
        "trades.json": fetch_ok("/historical/trades", ticker=t, limit=20),
    }
    probes = catalog_probes(active, event_ticker, series_ticker, archived, now)

    catalog_rows = [
        ("series_list.json", f"GET /series?category={SERIES_CATEGORY}",
         f"`series`: {CATALOG_ROWS} rows, one with `tags: null` first when the page has one"),
        ("series.json", f"GET /series/{series_ticker}?include_volume=true", "as fetched"),
        ("events_page.json", f"GET /events?status=open&limit={CATALOG_ROWS}", "as fetched"),
        ("events_page_nested.json", "GET /events?status=open&limit=2&with_nested_markets=true",
         f"each event's `markets`: the first {NESTED_MARKETS}"),
        ("orderbooks.json", f"GET /markets/orderbooks?tickers={a}&tickers={b}&tickers={UNKNOWN_TICKER}",
         f"the {BOOK_LEVELS} best levels of each side"),
        ("markets_updated_page.json",
         f"GET /markets?min_updated_ts={now - DAY}&mve_filter=exclude&limit={CATALOG_ROWS}", "as fetched"),
        ("markets_tickers_page.json", f"GET /markets?tickers={a},{b}", "as fetched"),
        ("markets_window_settled_page.json",
         f"GET /markets?status=settled&min_settled_ts={now - 7 * DAY}&mve_filter=exclude&limit={CATALOG_ROWS}",
         "as fetched"),
    ]
    historical_rows = [
        ("cutoff.json", "GET /historical/cutoff", "as fetched"),
        ("markets_page.json", f"GET /historical/markets?series_ticker={ARCHIVE_SERIES}&limit={CATALOG_ROWS}", "as fetched"),
        ("market.json", f"GET /historical/markets/{t}", "as fetched"),
        *[
            (f"candlesticks_{w}.json",
             f"GET /historical/markets/{t}/candlesticks?start_ts=<open>&end_ts=<close + {w} min>&period_interval={w}",
             f"the first {MINUTE_ROWS} rows" if w == 1 else "as fetched")
            for w in WIDTHS
        ],
        ("trades.json", f"GET /historical/trades?ticker={t}&limit=20", "as fetched"),
    ]
    for name, value in catalog.items():
        capture_common.write_json(out / "catalog" / name, value)
    for name, value in historical.items():
        capture_common.write_json(out / "historical" / name, value)
    capture_common.write_provenance(
        out / "catalog",
        catalog_provenance(catalog_rows, f"The active markets are `{a}` and `{b}`, of series `{series_ticker}`."),
    )
    capture_common.write_provenance(
        out / "historical",
        catalog_provenance(
            historical_rows,
            f"The archived market is `{t}`: the newest {ARCHIVE_SERIES} market in the archive with volume "
            "that `GET /markets/{ticker}` answers 404.",
        ),
    )
    print("\n".join(probes))
    print(f"\nwrote {len(catalog)} catalog and {len(historical)} historical fixtures to {out}")
```

- [ ] **Step 4: Dispatch the mode.** In `main()`, replace

```python
    args = sys.argv[1:]
    if args and args[0].startswith("--"):
```

with

```python
    args = sys.argv[1:]
    if args and args[0] == "--catalog":
        if len(args) != 2:
            sys.exit(__doc__)
        capture_catalog(Path(args[1]))
        return
    if args and args[0].startswith("--"):
```

- [ ] **Step 5: Check it parses**

Run: `nix develop --command python3 -m py_compile scripts/capture_kalshi_predictions.py && echo ok`
Expected: `ok`

### Task 1.3: capture and record

**Files:**
- Create: `polyoxide-kalshi/tests/fixtures/predictions/catalog/*.json`, `catalog/PROVENANCE.md`
- Create: `polyoxide-kalshi/tests/fixtures/predictions/historical/*.json`, `historical/PROVENANCE.md`
- Modify: `docs/specs/kalshi/OBSERVED.md` (append before the file's end)

- [ ] **Step 1: Run the capture**

Run: `nix develop --command uv run scripts/capture_kalshi_predictions.py --catalog polyoxide-kalshi/tests/fixtures/predictions | tee /tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-gh-71-kalshi-the-market-data-routes-a-catalog-18e0cc41d1362c43/8cc716e5-3fc1-4610-9e93-7c99c294820a/scratchpad/catalog-probes.md`
Expected: the probe lines, then `wrote 8 catalog and 8 historical fixtures to …`. Check `git status --short polyoxide-kalshi/tests/fixtures` lists only `catalog/` and `historical/` as new.

- [ ] **Step 2: Append the OBSERVED.md section.** After the last line of `docs/specs/kalshi/OBSERVED.md` (`<!-- epic 9 findings go above this line -->`), append:

```markdown

## Issue #71: catalog routes

The design session's probes (production, unsigned, 2026-10-22), from the spec
(`docs/superpowers/specs/2026-10-22-kalshi-catalog-routes-design.md` §2):

- **`min_updated_ts` tracks metadata, not trading.** `GET /markets?min_updated_ts=<now − 10 min>&mve_filter=exclude` answered no markets; `<now − 1 day>` answered a full page of 1000 with a cursor. The mirror says so ("Tracks non-trading changes only"). The incremental tier sees listings and rule changes, not price moves; a price poller uses `tickers`.
- **A request target over about 16 KiB is `414 URI Too Long`.** `GET /markets?tickers=` with 30-character tickers answered `200` at 520 tickers (16,154 bytes of path and query) and `414` at 530 (16,464 bytes). The mirror states no maximum. The client refuses a set past 8,000 bytes (`TICKERS_QUERY_BUDGET`).
- **`GET /series` has no paging.** Unfiltered it answered every series, 15,336, in one 19 MB body with no `cursor`; `category=Economics` answered 758 KB. 2,617 series sent `tags: null`, 7 `settlement_sources: null` and 1 `additional_prohibitions: null`; `fee_multiplier` is sent as an integer or a float.
- **`GET /markets/orderbooks` serves unsigned.** Repeated `tickers=` keys answered `{orderbooks: [{ticker, orderbook_fp}]}`, one per ticker, an unknown ticker's sides empty. 101 tickers answered `400` with a validation `msg` (the `max` tag).
- **`GET /markets` refuses an illegal timestamp combination with `400`, naming it**: "settled ts filters are mutually exclusive with close ts filters", "… with open or closed status", "min_updated_ts and max_updated_ts are incompatible with filters other than series_ticker and mve_filter=exclude". `min_updated_ts` without `mve_filter` answered `200`.
- **`GET /events` combines its filters and refuses none.** Over a day, `min_updated_ts` alone passed 6,000 events (30 pages, not exhausted), with `status=open` 3,139, with `min_close_ts=<now>` 5,504. Every answer carries `milestones`, asked for or not.
- **`GET /historical/markets` takes one selector.** Any two of `tickers`, `event_ticker`, `series_ticker` and `mve_filter` answered `400`: "tickers, event_ticker, series_ticker, and mve_filter filters are mutually exclusive".
- **The live routes drop an archived market, and overlap the archive near the cutoff.** For `KXHIGHNY-26MAY30-B72.5` and `KXHIGHNY-26MAR08-B66.5`: `GET /markets/{t}` `404`; `GET /markets/trades?ticker=` `200` with no trades; `GET /markets/candlesticks?market_tickers=` `200` with `{"markets": []}`, the market absent; the order book both sides empty; `GET /events/{e}?with_nested_markets=true` `200` with no nested markets. A listed market with no candle in the span is present with `"candlesticks": []`. `KXPERLIGA1TOTAL-26AUG22JUAASO-9`, settled hours before the cutoff, answered on every live route and on the historical ones.
- **The historical candlestick shape is not the live one**: `yes_bid`, `yes_ask` and `price` without `_dollars`, `volume` and `open_interest` without `_fp`, and an absent price sent as `null` where the live route leaves it out. An unknown ticker is `404`.
- **The cutoff** answered `market_settled_ts` and `trades_created_ts` 2026-08-23T00:00:00Z, `orders_updated_ts` 2026-10-08T00:00:00Z. `GET /historical/markets/{t}` for a live market is `404`.
- **`GET /account/endpoint_costs` lists none of the nine new routes**, so each costs the default, 10.

### The capture's probes

<!-- the --catalog probe lines go here -->

<!-- issue 71 findings go above this line -->
```

Then replace the line `<!-- the --catalog probe lines go here -->` with the lines Step 1 printed before `wrote …` (the file in the scratchpad, without its last two lines).

- [ ] **Step 3: Read the probes against the spec.** For each line:
  - A window family whose `tickers`, `event_ticker` or `series_ticker` answered other than `HTTP 200`: that setter comes off `ListMarketWindow` (D5). Stop and tell the owner before Story 3.
  - A family × `mve_filter` refusal: same rule for `ListMarketWindow::mve_filter`.
  - `GET /historical/trades?ticker=NOSUCHMARKET-0`: note whether it is `404` or `200, 0 trades`; D13's trades row holds either way.
  - The over-10,000 historical candle probe: if it is refused (`HTTP 400`), D13's clamp covers it; if it answers `200`, note the row count.

- [ ] **Step 4: Commit**

```bash
git add scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions/catalog polyoxide-kalshi/tests/fixtures/predictions/historical docs/specs/kalshi/OBSERVED.md _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.txt _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.sh
git commit -m "test(kalshi): capture the catalog and historical routes (#71, Story 1)" -- scripts/capture_kalshi_predictions.py polyoxide-kalshi/tests/fixtures/predictions/catalog polyoxide-kalshi/tests/fixtures/predictions/historical docs/specs/kalshi/OBSERVED.md _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.txt _bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.sh
```

---

## Story 2: plumbing (spec §5; D2, D3, D4, D16)

### Task 2.1: the budget, the refusal and the chunker

**Files:**
- Create: `polyoxide-kalshi/src/predictions/tickers.rs`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs` (module list and re-exports)

- [ ] **Step 1: Write the module with its failing tests**

`polyoxide-kalshi/src/predictions/tickers.rs`:

```rust
//! The `tickers` budget: how many tickers one request may name, and the chunker that splits a set
//! into requests that fit.
//!
//! Kalshi answers a request target over about 16 KiB with `414 URI Too Long`
//! (`docs/specs/kalshi/OBSERVED.md`, issue #71), and a [`Ticker`] may be 200 characters long, so a
//! count alone cannot keep a set under it. A set is measured as if each ticker were sent as its own
//! `tickers=` pair, which the comma-joined form never exceeds, and refused past
//! [`TICKERS_QUERY_BUDGET`].

use crate::predictions::key::Ticker;

/// The most bytes of query one request's tickers may take: half the request target Kalshi was
/// measured to accept, so the path and the other parameters keep the rest. About 200 tickers of
/// today's length fit.
pub const TICKERS_QUERY_BUDGET: usize = 8_000;

/// The most tickers `GET /markets/orderbooks` takes in one request; Kalshi refuses 101 with a
/// `400`.
pub const MAX_ORDERBOOK_TICKERS: usize = 100;

/// The bytes one ticker costs besides its own: `tickers=` and a separator.
const PER_TICKER: usize = "tickers=".len() + 1;

/// What `ticker` costs the budget.
fn cost(ticker: &str) -> usize {
    ticker.len() + PER_TICKER
}

/// Why one request may not send `tickers`, or `None` when it may: an empty set, which would ask
/// for every market, a set of more than `max_count`, or one past [`TICKERS_QUERY_BUDGET`].
pub(crate) fn refusal(tickers: &[&str], max_count: usize) -> Option<String> {
    let size: usize = tickers.iter().map(|ticker| cost(ticker)).sum();
    if tickers.is_empty() {
        Some("the set is empty, and sending none would ask for every market".to_owned())
    } else if tickers.len() > max_count {
        Some(format!(
            "{} tickers are more than the {max_count} the route takes; split them with ticker_chunks",
            tickers.len()
        ))
    } else if size > TICKERS_QUERY_BUDGET {
        Some(format!(
            "{} tickers take {size} bytes of query, past the budget of {TICKERS_QUERY_BUDGET}; \
             split them with ticker_chunks",
            tickers.len()
        ))
    } else {
        None
    }
}

/// Splits `tickers` into consecutive slices, in order, each of at most `max_count` tickers (a
/// `max_count` of 0 is taken as 1) and within [`TICKERS_QUERY_BUDGET`], so each slice can be sent in
/// one request. Pass [`MAX_ORDERBOOK_TICKERS`] for `markets().orderbooks`, and `usize::MAX` for a
/// listing. An empty set yields nothing.
///
/// ```
/// use polyoxide_kalshi::predictions::{key::Ticker, ticker_chunks, MAX_ORDERBOOK_TICKERS};
///
/// let tickers: Vec<Ticker> = (0..250)
///     .map(|i| Ticker::new(&format!("KXT-{i}")).unwrap())
///     .collect();
/// let sizes: Vec<usize> = ticker_chunks(&tickers, MAX_ORDERBOOK_TICKERS)
///     .map(<[Ticker]>::len)
///     .collect();
/// assert_eq!(sizes, [100, 100, 50]);
/// ```
pub fn ticker_chunks(tickers: &[Ticker], max_count: usize) -> TickerChunks<'_> {
    TickerChunks {
        rest: tickers,
        max_count: max_count.max(1),
    }
}

/// The slices [`ticker_chunks`] yields.
#[derive(Debug, Clone)]
pub struct TickerChunks<'a> {
    rest: &'a [Ticker],
    max_count: usize,
}

impl<'a> Iterator for TickerChunks<'a> {
    type Item = &'a [Ticker];

    fn next(&mut self) -> Option<&'a [Ticker]> {
        if self.rest.is_empty() {
            return None;
        }
        let mut size = 0;
        let mut taken = 0;
        for ticker in self.rest {
            let next = size + cost(ticker.as_str());
            // A first ticker always fits: a ticker is at most 200 characters.
            if taken == self.max_count || (taken > 0 && next > TICKERS_QUERY_BUDGET) {
                break;
            }
            size = next;
            taken += 1;
        }
        let (chunk, rest) = self.rest.split_at(taken);
        self.rest = rest;
        Some(chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `count` distinct tickers of `len` characters each.
    fn tickers(count: usize, len: usize) -> Vec<Ticker> {
        (0..count)
            .map(|i| {
                let digits = i.to_string();
                Ticker::new(&format!("{}{digits}", "K".repeat(len - digits.len()))).unwrap()
            })
            .collect()
    }

    fn texts(tickers: &[Ticker]) -> Vec<&str> {
        tickers.iter().map(Ticker::as_str).collect()
    }

    fn sizes(chunks: TickerChunks<'_>) -> Vec<usize> {
        chunks.map(<[Ticker]>::len).collect()
    }

    #[test]
    fn a_ticker_costs_its_length_and_nine_bytes() {
        assert_eq!(PER_TICKER, 9);
        assert_eq!(cost("KXT-1"), 14);
    }

    #[test]
    fn an_empty_set_is_refused_because_it_would_ask_for_every_market() {
        let reason = refusal(&[], usize::MAX).unwrap();
        assert!(reason.contains("every market"), "{reason}");
    }

    #[test]
    fn a_set_over_the_count_is_refused_and_one_at_it_is_not() {
        let set = tickers(101, 10);
        let set = texts(&set);
        assert!(refusal(&set[..100], MAX_ORDERBOOK_TICKERS).is_none());
        let reason = refusal(&set, MAX_ORDERBOOK_TICKERS).unwrap();
        assert!(reason.contains("101 tickers") && reason.contains("100"), "{reason}");
    }

    #[test]
    fn a_set_at_the_budget_is_sent_and_one_a_byte_over_is_refused() {
        // 200 tickers of 31 characters cost 200 × 40 = 8,000 bytes, the budget.
        let at = tickers(200, 31);
        assert_eq!(
            texts(&at).iter().map(|ticker| cost(ticker)).sum::<usize>(),
            TICKERS_QUERY_BUDGET
        );
        assert!(refusal(&texts(&at), usize::MAX).is_none());

        let mut over = tickers(199, 31);
        over.push(Ticker::new(&"K".repeat(32)).unwrap());
        let reason = refusal(&texts(&over), usize::MAX).unwrap();
        assert!(reason.contains("8001 bytes"), "{reason}");
    }

    #[test]
    fn chunks_keep_order_and_fill_up_to_the_count_or_the_budget() {
        let set = tickers(450, 31);
        // 200 tickers of 31 characters fill the budget.
        assert_eq!(sizes(ticker_chunks(&set, usize::MAX)), [200, 200, 50]);
        assert_eq!(
            sizes(ticker_chunks(&set, MAX_ORDERBOOK_TICKERS)),
            [100, 100, 100, 100, 50]
        );
        let joined: Vec<&Ticker> = ticker_chunks(&set, 7).flatten().collect();
        assert_eq!(joined, set.iter().collect::<Vec<_>>());
        for chunk in ticker_chunks(&set, usize::MAX) {
            assert!(refusal(&texts(chunk), usize::MAX).is_none());
        }
    }

    #[test]
    fn an_empty_set_yields_nothing_and_a_count_of_zero_is_one() {
        assert_eq!(ticker_chunks(&[], 5).count(), 0);
        assert_eq!(sizes(ticker_chunks(&tickers(3, 10), 0)), [1, 1, 1]);
    }

    #[test]
    fn the_longest_tickers_fit_38_to_a_chunk() {
        // 209 bytes each: 38 take 7,942 bytes, and a 39th would pass 8,000.
        let set = tickers(50, 200);
        assert_eq!(sizes(ticker_chunks(&set, usize::MAX)), [38, 12]);
    }
}
```

- [ ] **Step 2: Wire the module.** In `polyoxide-kalshi/src/predictions/mod.rs`, after `pub mod request;` add `mod tickers;`, and after `pub use key::key;` add:

```rust
pub use tickers::{ticker_chunks, TickerChunks, MAX_ORDERBOOK_TICKERS, TICKERS_QUERY_BUDGET};
```

- [ ] **Step 3: Run the tests**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib tickers::`
Expected: 7 passed. Then `nix develop --command cargo test -p polyoxide-kalshi --all-features --doc ticker_chunks`: 1 passed.

### Task 2.2: `Routed` gains `query_many`, `refuse` and the two ticker forms

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/request.rs` (the `Routed` struct, `new`, `query`, a new `impl` block, `send`, and `routed_tests`)

- [ ] **Step 1: Write the failing tests.** Append to `mod routed_tests` (before its closing `}`):

```rust
    #[test]
    fn query_many_appends_a_pair_per_value_after_the_others() {
        let kalshi = Predictions::new().unwrap();
        let request = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .query("a", 1)
            .query_many("tickers", ["X", "Y"])
            .query_many("tickers", ["Z"]);
        assert_eq!(
            request.query,
            [("a", "1"), ("tickers", "X"), ("tickers", "Y"), ("tickers", "Z")]
                .map(|(k, v)| (k, v.to_owned()))
        );
    }

    #[tokio::test]
    async fn a_refused_key_sends_nothing_and_a_later_value_for_it_clears_the_refusal() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", Matcher::Any)
            .match_query(Matcher::Any)
            .expect(0)
            .create_async()
            .await;
        let kalshi = Predictions::builder()
            .base_url(server.url())
            .build()
            .unwrap();
        let err = Routed::<Value>::new(&kalshi.inner, routes::EXCHANGE_STATUS, &[])
            .refuse("tickers", "no".to_owned())
            .send()
            .await
            .unwrap_err();
        assert!(
            matches!(&err, PredictionsError::Api(ApiError::Validation(reason)) if reason == "no"),
            "{err:?}"
        );
        assert_eq!(err.class(), Class::InvalidRequest);
        mock.assert_async().await;

        let pairs = pairs_sent(STATUS, |url| async move {
            let kalshi = Predictions::builder().base_url(url).build().unwrap();
            let _ = Routed::<Value>::new(&kalshi.inner, routes::EXCHANGE_STATUS, &[])
                .refuse("tickers", "no".to_owned())
                .query("tickers", "A")
                .send()
                .await;
        })
        .await;
        assert_eq!(pairs, [("tickers".to_owned(), "A".to_owned())]);
    }

    #[test]
    fn a_ticker_set_is_joined_or_repeated_or_refused() {
        let kalshi = Predictions::new().unwrap();
        let joined = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .tickers_joined("tickers", &["A", "B"]);
        assert_eq!(joined.query, [("tickers", "A,B".to_owned())]);
        assert!(joined.refusals.is_empty());

        let repeated = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .tickers_repeated("tickers", &["A", "B"], 100);
        assert_eq!(
            repeated.query,
            [("tickers", "A".to_owned()), ("tickers", "B".to_owned())]
        );

        let empty =
            Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[]).tickers_joined("tickers", &[]);
        assert!(empty.query.is_empty());
        assert_eq!(empty.refusals.len(), 1);
        assert!(empty.refusals[0].1.starts_with("tickers: "), "{:?}", empty.refusals);

        let over = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .tickers_repeated("tickers", &["A", "B", "C"], 2);
        assert!(over.query.is_empty());
        assert_eq!(over.refusals.len(), 1);

        // A refused set followed by a good one sends the good one.
        let replaced = Routed::<Value>::new(&kalshi.inner, routes::MARKETS, &[])
            .tickers_joined("tickers", &[])
            .tickers_joined("tickers", &["C"]);
        assert!(replaced.refusals.is_empty());
        assert_eq!(replaced.query, [("tickers", "C".to_owned())]);
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib routed_tests`
Expected: compile errors: no method `query_many`, `refuse`, `tickers_joined`, `tickers_repeated`; no field `refusals`.

- [ ] **Step 3: Implement.** In `request.rs`:

Add `use crate::predictions::tickers;` after `use crate::predictions::{error::PredictionsError, Inner};`.

In `struct Routed<T>`, after the `refused` field, add:

```rust
    /// Why a query key's value is refused, by key, recorded by [`Routed::refuse`].
    refusals: Vec<(&'static str, String)>,
```

In `Routed::new`'s struct literal, after `refused,` add `refusals: Vec::new(),`.

Replace `query` with:

```rust
    /// Sets a query parameter, replacing an earlier value for the same key, and clears a refusal
    /// of the key.
    pub(crate) fn query(mut self, key: &'static str, value: impl ToString) -> Self {
        self.refusals.retain(|(k, _)| *k != key);
        let value = value.to_string();
        match self.query.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.query.push((key, value)),
        }
        self
    }

    /// Appends one `key` pair per value, after every pair already set, replacing none, and clears a
    /// refusal of the key.
    pub(crate) fn query_many<V: ToString>(
        mut self,
        key: &'static str,
        values: impl IntoIterator<Item = V>,
    ) -> Self {
        self.refusals.retain(|(k, _)| *k != key);
        self.query
            .extend(values.into_iter().map(|value| (key, value.to_string())));
        self
    }

    /// Records why `key`'s value is not sent, replacing an earlier refusal of the key.
    /// [`Routed::send`] returns the first refusal as `ApiError::Validation` before it charges or
    /// sends anything.
    pub(crate) fn refuse(mut self, key: &'static str, reason: String) -> Self {
        self.refusals.retain(|(k, _)| *k != key);
        self.refusals.push((key, reason));
        self
    }

    /// Sends `tickers` comma-joined as `key`, or records why the set is refused: empty, or past
    /// the query budget.
    pub(crate) fn tickers_joined(self, key: &'static str, tickers: &[&str]) -> Self {
        match tickers::refusal(tickers, usize::MAX) {
            Some(reason) => self.refuse(key, format!("{key}: {reason}")),
            None => self.query(key, tickers.join(",")),
        }
    }

    /// Sends each of `tickers` as its own `key` pair, or records why the set is refused: empty,
    /// more than `max_count`, or past the query budget.
    pub(crate) fn tickers_repeated(
        self,
        key: &'static str,
        tickers: &[&str],
        max_count: usize,
    ) -> Self {
        match tickers::refusal(tickers, max_count) {
            Some(reason) => self.refuse(key, format!("{key}: {reason}")),
            None => self.query_many(key, tickers.iter().copied()),
        }
    }
```

In `send`, after the block

```rust
        if let Some(reason) = self.refused {
            return Err(ApiError::Validation(reason).into());
        }
```

add

```rust
        if let Some((_, reason)) = self.refusals.into_iter().next() {
            return Err(ApiError::Validation(reason).into());
        }
```

and in `send`'s doc comment replace "A route parameter that `new` refused is returned first," with "A route parameter that `new` refused is returned first, then the first query key [`Routed::refuse`] recorded,".

- [ ] **Step 4: Run the tests**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib routed_tests`
Expected: all pass, the three new ones included.

### Task 2.3: the nine routes

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/request.rs` (`mod routes`)

- [ ] **Step 1: Add the routes.** In `mod routes`, replace the doc comment `/// The ten routes of this module, by name.` (above `pub(crate) mod routes`) with `/// The nineteen routes of this module, by name.`, and after `SERIES_CANDLESTICKS` add:

```rust
    /// Every series, in one body.
    pub(crate) const SERIES_LIST: Route = Route::get("/trade-api/v2/series");
    /// One series.
    pub(crate) const SERIES: Route = Route::get("/trade-api/v2/series/:series_ticker");
    /// The events, a page at a time.
    pub(crate) const EVENTS: Route = Route::get("/trade-api/v2/events");
    /// Up to 100 markets' books in one read.
    pub(crate) const ORDERBOOKS: Route = Route::get("/trade-api/v2/markets/orderbooks");
    /// Where the archive begins.
    pub(crate) const HISTORICAL_CUTOFF: Route = Route::get("/trade-api/v2/historical/cutoff");
    /// The archived markets, a page at a time.
    pub(crate) const HISTORICAL_MARKETS: Route = Route::get("/trade-api/v2/historical/markets");
    /// One archived market.
    pub(crate) const HISTORICAL_MARKET: Route =
        Route::get("/trade-api/v2/historical/markets/:ticker");
    /// One archived market's candlesticks.
    pub(crate) const HISTORICAL_CANDLESTICKS: Route =
        Route::get("/trade-api/v2/historical/markets/:ticker/candlesticks");
    /// The archived trades, a page at a time.
    pub(crate) const HISTORICAL_TRADES: Route = Route::get("/trade-api/v2/historical/trades");
```

and append to `ALL`, after `SERIES_CANDLESTICKS,`:

```rust
        SERIES_LIST,
        SERIES,
        EVENTS,
        ORDERBOOKS,
        HISTORICAL_CUTOFF,
        HISTORICAL_MARKETS,
        HISTORICAL_MARKET,
        HISTORICAL_CANDLESTICKS,
        HISTORICAL_TRADES,
```

- [ ] **Step 2: Run the route tests to see the pins fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib request::tests`
Expected: `the_route_table_is_the_epics_ten_routes` FAILS (the table holds nineteen); `every_route_is_written_from_the_api_root_and_none_is_listed_twice` and `our_routes_find_themselves_and_not_each_other` PASS.

### Task 2.4: D16's four route-table pins (owner's exception)

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/request.rs` (`the_route_table_is_the_epics_ten_routes`)
- Modify: `polyoxide-kalshi/tests/mock_predictions_routes.rs` (`the_route_table_holds_the_ten_routes_in_endpoint_costs_spelling`)
- Modify: `polyoxide-kalshi/tests/spec_agreement_predictions.rs` (`every_route_the_crate_holds_is_a_documented_operation`)
- Modify: `polyoxide-kalshi/tests/live_predictions.rs` (`live_the_route_table_agrees_with_the_endpoint_costs`)

- [ ] **Step 1: `request.rs`.** Rename `the_route_table_is_the_epics_ten_routes` to `the_route_table_is_the_modules_nineteen_routes`, and in its expected array, after the `SERIES_CANDLESTICKS` tuple, add:

```rust
                ("GET", "/trade-api/v2/series"),
                ("GET", "/trade-api/v2/series/:series_ticker"),
                ("GET", "/trade-api/v2/events"),
                ("GET", "/trade-api/v2/markets/orderbooks"),
                ("GET", "/trade-api/v2/historical/cutoff"),
                ("GET", "/trade-api/v2/historical/markets"),
                ("GET", "/trade-api/v2/historical/markets/:ticker"),
                ("GET", "/trade-api/v2/historical/markets/:ticker/candlesticks"),
                ("GET", "/trade-api/v2/historical/trades"),
```

- [ ] **Step 2: `mock_predictions_routes.rs`.** Rename the test to `the_route_table_holds_the_nineteen_routes_in_endpoint_costs_spelling` and replace its `expected` array with the sorted nineteen:

```rust
    let expected = [
        ("GET", "/trade-api/v2/account/endpoint_costs"),
        ("GET", "/trade-api/v2/account/limits"),
        ("GET", "/trade-api/v2/events"),
        ("GET", "/trade-api/v2/events/:event_ticker"),
        ("GET", "/trade-api/v2/exchange/status"),
        ("GET", "/trade-api/v2/historical/cutoff"),
        ("GET", "/trade-api/v2/historical/markets"),
        ("GET", "/trade-api/v2/historical/markets/:ticker"),
        ("GET", "/trade-api/v2/historical/markets/:ticker/candlesticks"),
        ("GET", "/trade-api/v2/historical/trades"),
        ("GET", "/trade-api/v2/markets"),
        ("GET", "/trade-api/v2/markets/:ticker"),
        ("GET", "/trade-api/v2/markets/:ticker/orderbook"),
        ("GET", "/trade-api/v2/markets/candlesticks"),
        ("GET", "/trade-api/v2/markets/orderbooks"),
        ("GET", "/trade-api/v2/markets/trades"),
        ("GET", "/trade-api/v2/series"),
        ("GET", "/trade-api/v2/series/:series_ticker"),
        (
            "GET",
            "/trade-api/v2/series/:series_ticker/markets/:ticker/candlesticks",
        ),
    ];
```

- [ ] **Step 3: `spec_agreement_predictions.rs`.** In `every_route_the_crate_holds_is_a_documented_operation`, replace `assert_eq!(count, 10, "the route table holds ten routes");` with `assert_eq!(count, 19, "the route table holds nineteen routes");`.

- [ ] **Step 4: `live_predictions.rs`.** In `live_the_route_table_agrees_with_the_endpoint_costs`, replace `assert_eq!(checked, 10, "the route table holds ten routes");` with `assert_eq!(checked, 19, "the route table holds nineteen routes");`.

- [ ] **Step 5: Run the pins**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features route_table`
Expected: `the_route_table_is_the_modules_nineteen_routes`, `the_route_table_holds_the_nineteen_routes_in_endpoint_costs_spelling` PASS. Then `nix develop --command cargo test -p polyoxide-kalshi --all-features --test spec_agreement_predictions every_route_the_crate_holds`: PASS (the mirror documents all nineteen).

### Task 2.5: Story 2's gates and commit

- [ ] **Step 1: Run the story gates** (Rules section). Expected: all pass; the touch-set gate prints nothing.

- [ ] **Step 2: Commit**

```bash
git add polyoxide-kalshi/src/predictions/tickers.rs
git commit -m "feat(kalshi): the tickers budget, ticker_chunks and nine catalog routes (#71, Story 2)" -- polyoxide-kalshi/src/predictions/tickers.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/src/predictions/request.rs polyoxide-kalshi/tests/mock_predictions_routes.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/live_predictions.rs
```

---

## Story 3: `markets()` (spec §6; D5–D8, D12)

### Task 3.1: `CreatedStatus`, `MarketWindow` and the orderbooks types

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/types/markets.rs` (before `#[cfg(test)]`, and its `tests` module)
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs` (the `markets` re-export)

- [ ] **Step 1: Write the failing test.** Append to `types/markets.rs`'s `mod tests`:

```rust
    #[test]
    fn a_window_sends_its_status_then_its_bounds_and_leaves_out_what_is_unset() {
        let owned = |pairs: &[(&'static str, &str)]| -> Vec<(&'static str, String)> {
            pairs.iter().map(|(k, v)| (*k, (*v).to_owned())).collect()
        };
        let created = MarketWindow::Created {
            status: Some(CreatedStatus::Unopened),
            min_ts: Some(1),
            max_ts: Some(2),
        };
        assert_eq!(
            created.pairs(),
            owned(&[("status", "unopened"), ("min_created_ts", "1"), ("max_created_ts", "2")])
        );
        let closed = MarketWindow::Closed {
            closed_only: true,
            min_ts: None,
            max_ts: Some(4),
        };
        assert_eq!(closed.pairs(), owned(&[("status", "closed"), ("max_close_ts", "4")]));
        let settled = MarketWindow::Settled {
            settled_only: false,
            min_ts: Some(5),
            max_ts: None,
        };
        assert_eq!(settled.pairs(), owned(&[("min_settled_ts", "5")]));
        let open = MarketWindow::Created {
            status: Some(CreatedStatus::Open),
            min_ts: None,
            max_ts: None,
        };
        assert_eq!(open.pairs(), owned(&[("status", "open")]));
    }

    #[test]
    fn a_created_status_is_one_of_the_two_the_created_family_allows() {
        assert_eq!(CreatedStatus::ALL, [CreatedStatus::Unopened, CreatedStatus::Open]);
        assert_eq!(CreatedStatus::Open.to_string(), "open");
        assert!("closed".parse::<CreatedStatus>().is_err());
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib types::markets`
Expected: compile errors: `MarketWindow`, `CreatedStatus` not found.

- [ ] **Step 3: Implement.** In `types/markets.rs`, immediately before `#[cfg(test)]`, add:

```rust
polyoxide_venue::wire_enum! {
    /// The states a created-time window of `GET /markets` may name
    /// ([`MarketWindow::Created`]); Kalshi refuses any other beside `min_created_ts`.
    #[non_exhaustive]
    pub enum CreatedStatus {
        /// Not yet open.
        Unopened => "unopened",
        /// Open for trading.
        Open => "open",
    }
}

/// One family of `GET /markets`' timestamp filters, with the states the host allows beside it.
///
/// Kalshi refuses two families at once, and a state the family does not allow, with a `400`
/// (`docs/specs/kalshi/OBSERVED.md`), so a window names one family and its states and nothing
/// else; the fourth family, `min_updated_ts`, is `markets().updated`. Times are Unix seconds, and
/// a bound left `None` is not sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarketWindow {
    /// Markets created in the window (`min_created_ts`, `max_created_ts`), optionally only the
    /// unopened or only the open ones.
    Created {
        /// Only markets in this state.
        status: Option<CreatedStatus>,
        /// The earliest creation time.
        min_ts: Option<i64>,
        /// The latest creation time.
        max_ts: Option<i64>,
    },
    /// Markets that close in the window (`min_close_ts`, `max_close_ts`), optionally only the
    /// closed ones (`status=closed`).
    Closed {
        /// Send `status=closed`.
        closed_only: bool,
        /// The earliest close time.
        min_ts: Option<i64>,
        /// The latest close time.
        max_ts: Option<i64>,
    },
    /// Markets that settled in the window (`min_settled_ts`, `max_settled_ts`), optionally only
    /// the settled ones (`status=settled`).
    Settled {
        /// Send `status=settled`.
        settled_only: bool,
        /// The earliest settlement time.
        min_ts: Option<i64>,
        /// The latest settlement time.
        max_ts: Option<i64>,
    },
}

impl MarketWindow {
    /// The query pairs the window sends, in order: its state, then its lower and upper bound.
    pub(crate) fn pairs(&self) -> Vec<(&'static str, String)> {
        let (status, (min_key, min_ts), (max_key, max_ts)) = match *self {
            Self::Created {
                status,
                min_ts,
                max_ts,
            } => (
                status.map(CreatedStatus::as_str),
                ("min_created_ts", min_ts),
                ("max_created_ts", max_ts),
            ),
            Self::Closed {
                closed_only,
                min_ts,
                max_ts,
            } => (
                closed_only.then_some("closed"),
                ("min_close_ts", min_ts),
                ("max_close_ts", max_ts),
            ),
            Self::Settled {
                settled_only,
                min_ts,
                max_ts,
            } => (
                settled_only.then_some("settled"),
                ("min_settled_ts", min_ts),
                ("max_settled_ts", max_ts),
            ),
        };
        let mut pairs = Vec::new();
        if let Some(status) = status {
            pairs.push(("status", status.to_owned()));
        }
        if let Some(min_ts) = min_ts {
            pairs.push((min_key, min_ts.to_string()));
        }
        if let Some(max_ts) = max_ts {
            pairs.push((max_key, max_ts.to_string()));
        }
        pairs
    }
}

/// One market's book in `GET /markets/orderbooks` (`MarketOrderbookFp`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketOrderbook {
    /// The market's ticker.
    pub ticker: String,
    /// Its book; an unlisted ticker's sides are empty.
    pub orderbook_fp: OrderbookFp,
}

/// `GET /markets/orderbooks` (`GetMarketOrderbooksResponse`): one book per ticker asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderbooksResponse {
    /// The books.
    pub orderbooks: Vec<MarketOrderbook>,
}
```

In `types/mod.rs`, replace the `pub use markets::{ … };` block with:

```rust
pub use markets::{
    BatchCandlesticksResponse, BidAskDistribution, BookSide, CreatedStatus, Market,
    MarketCandlestick, MarketCandlesticks, MarketOrderbook, MarketResponse, MarketStatus,
    MarketStatusFilter, MarketWindow, MarketsPage, MveFilter, OrderbookFp, OrderbookResponse,
    OrderbooksResponse, OutcomeSide, PeriodInterval, PriceDistribution, PriceLevel, PriceRange,
    SeriesCandlesticksResponse, Trade, TradesPage,
};
```

- [ ] **Step 4: Run the tests**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib types::markets`
Expected: all pass, the two new ones included.

### Task 3.2: the setter cases (failing)

**Files:**
- Modify: `polyoxide-kalshi/tests/query_setters_predictions.rs` (append)

- [ ] **Step 1: Append the cases**

```rust
// ── The markets catalog builders (issue #71) ────────────────────

use polyoxide_kalshi::predictions::types::{CreatedStatus, MarketWindow};

fn second() -> Ticker {
    Ticker::new("KXTEST-2").unwrap()
}

/// A window's state and bounds come first, in that order, then the setters in call order;
/// `updated` sends its bound and `mve_filter=exclude` first; `orderbooks` one pair per ticker.
const MARKETS_CATALOG_CASES: &[Case] = &[
    Case {
        builder: "ListMarkets, tickers",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list()
                    .tickers(&[ticker(), second()])
                    .limit(5)
                    .send()
                    .await;
            })
        },
        sends: &[("tickers", "KXTEST-1,KXTEST-2"), ("limit", "5")],
    },
    Case {
        builder: "ListMarketWindow, created",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .window(MarketWindow::Created {
                        status: Some(CreatedStatus::Unopened),
                        min_ts: Some(1),
                        max_ts: Some(2),
                    })
                    .event_ticker("EV-1")
                    .series_ticker("SER-1")
                    .mve_filter(MveFilter::Exclude)
                    .tickers(&[ticker()])
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "unopened"),
            ("min_created_ts", "1"),
            ("max_created_ts", "2"),
            ("event_ticker", "EV-1"),
            ("series_ticker", "SER-1"),
            ("mve_filter", "exclude"),
            ("tickers", "KXTEST-1"),
            ("limit", "5"),
            ("cursor", "c1"),
        ],
    },
    Case {
        builder: "ListMarketWindow, closed",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .window(MarketWindow::Closed {
                        closed_only: true,
                        min_ts: Some(3),
                        max_ts: Some(4),
                    })
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "closed"),
            ("min_close_ts", "3"),
            ("max_close_ts", "4"),
        ],
    },
    Case {
        builder: "ListMarketWindow, settled",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .window(MarketWindow::Settled {
                        settled_only: true,
                        min_ts: Some(5),
                        max_ts: Some(6),
                    })
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "settled"),
            ("min_settled_ts", "5"),
            ("max_settled_ts", "6"),
        ],
    },
    Case {
        builder: "ListUpdatedMarkets",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .updated(7)
                    .max_ts(8)
                    .series_ticker("SER-1")
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("min_updated_ts", "7"),
            ("mve_filter", "exclude"),
            ("max_updated_ts", "8"),
            ("series_ticker", "SER-1"),
            ("limit", "5"),
            ("cursor", "c1"),
        ],
    },
    Case {
        builder: "GetOrderbooks",
        path: "/trade-api/v2/markets/orderbooks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .orderbooks(&[ticker(), second()])
                    .send()
                    .await;
            })
        },
        sends: &[("tickers", "KXTEST-1"), ("tickers", "KXTEST-2")],
    },
];

#[tokio::test]
async fn every_markets_catalog_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(MARKETS_CATALOG_CASES).await;
}
```

- [ ] **Step 2: Run to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: compile errors: no method `tickers`, `window`, `updated`, `orderbooks`.

### Task 3.3: the builders

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/api/markets.rs`
- Modify: `polyoxide-kalshi/src/predictions/api/mod.rs`

- [ ] **Step 1: Imports.** Replace the `use crate::predictions::{ … };` block of `api/markets.rs` with:

```rust
use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        BatchCandlesticksResponse, MarketResponse, MarketStatusFilter, MarketWindow, MarketsPage,
        MveFilter, OrderbookResponse, OrderbooksResponse, PeriodInterval,
        SeriesCandlesticksResponse, TradesPage,
    },
    Inner, PredictionsError, MAX_ORDERBOOK_TICKERS,
};
```

- [ ] **Step 2: The constructors.** In `impl MarketsApi`, after `list`, add:

```rust
    /// `GET /trade-api/v2/markets` in one timestamp family: the markets created, closed or settled
    /// in a window, with only the states that family allows. Kalshi refuses a state or a second
    /// family beside it with a `400`, so the builder has no `status` setter.
    pub fn window(&self, window: MarketWindow) -> ListMarketWindow {
        let mut request = Routed::new(&self.inner, routes::MARKETS, &[]);
        for (key, value) in window.pairs() {
            request = request.query(key, value);
        }
        ListMarketWindow { request }
    }

    /// `GET /trade-api/v2/markets` updated after `min_ts`, in Unix seconds, multivariate
    /// combinations excluded.
    ///
    /// Kalshi tracks metadata here, not trading: a market whose price moved and whose terms did not
    /// is not listed, so a price poller reads `list().tickers(..)` instead. The host allows only
    /// `series_ticker` and `mve_filter=exclude` beside this filter, so the builder offers nothing
    /// else.
    pub fn updated(&self, min_ts: i64) -> ListUpdatedMarkets {
        ListUpdatedMarkets {
            request: Routed::new(&self.inner, routes::MARKETS, &[])
                .query("min_updated_ts", min_ts)
                .query("mve_filter", MveFilter::Exclude),
        }
    }
```

and after `orderbook`, add:

```rust
    /// `GET /trade-api/v2/markets/orderbooks`: up to [`MAX_ORDERBOOK_TICKERS`] markets' books in one
    /// read, one per ticker, each side worst first, an unlisted ticker's sides empty.
    ///
    /// An empty set, more than 100 tickers, or a set past
    /// [`TICKERS_QUERY_BUDGET`](crate::predictions::TICKERS_QUERY_BUDGET) is refused when sent,
    /// with nothing sent; [`ticker_chunks`](crate::predictions::ticker_chunks) splits a larger set.
    pub fn orderbooks(&self, tickers: &[Ticker]) -> GetOrderbooks {
        let texts: Vec<&str> = tickers.iter().map(Ticker::as_str).collect();
        GetOrderbooks {
            request: Routed::new(&self.inner, routes::ORDERBOOKS, &[]).tickers_repeated(
                "tickers",
                &texts,
                MAX_ORDERBOOK_TICKERS,
            ),
        }
    }
```

  Then add to `orderbooks`' doc comment one sentence saying what Story 1's probe found (`OBSERVED.md`, "The capture's probes"): whether the answer keeps the asked order, and what a repeated ticker returns (D8).

- [ ] **Step 3: `ListMarkets::tickers`.** In `impl ListMarkets`, after the `query_setters!` block, add:

```rust
    /// Only these markets, comma-joined: a price poller's read of a named set.
    ///
    /// An empty set, or one past
    /// [`TICKERS_QUERY_BUDGET`](crate::predictions::TICKERS_QUERY_BUDGET), is refused when sent,
    /// with nothing sent; [`ticker_chunks`](crate::predictions::ticker_chunks) splits a larger set.
    /// Setting it again replaces the set and any refusal.
    pub fn tickers(mut self, tickers: &[Ticker]) -> Self {
        let texts: Vec<&str> = tickers.iter().map(Ticker::as_str).collect();
        self.request = self.request.tickers_joined("tickers", &texts);
        self
    }
```

- [ ] **Step 4: The new builders.** After `impl ListMarkets { … }`, add:

```rust
/// Request builder for `GET /trade-api/v2/markets` in one timestamp family
/// ([`MarketsApi::window`]).
#[must_use = "a request does nothing until it is sent"]
pub struct ListMarketWindow {
    pub(crate) request: Routed<MarketsPage>,
}

impl ListMarketWindow {
    polyoxide_core::query_setters! {
        /// Only the markets of this event.
        event_ticker: impl Into<String> => "event_ticker",
        /// Only the markets of this series.
        series_ticker: impl Into<String> => "series_ticker",
        /// Only the multivariate combinations, or everything but them.
        mve_filter: MveFilter => "mve_filter",
        /// Markets per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
    }

    /// Only these markets, comma-joined, refused as [`ListMarkets::tickers`] refuses.
    pub fn tickers(mut self, tickers: &[Ticker]) -> Self {
        let texts: Vec<&str> = tickers.iter().map(Ticker::as_str).collect();
        self.request = self.request.tickers_joined("tickers", &texts);
        self
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketsPage, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets` updated after a time
/// ([`MarketsApi::updated`]).
#[must_use = "a request does nothing until it is sent"]
pub struct ListUpdatedMarkets {
    pub(crate) request: Routed<MarketsPage>,
}

impl ListUpdatedMarkets {
    polyoxide_core::query_setters! {
        /// Only markets updated at or before this Unix time, in seconds.
        max_ts: i64 => "max_updated_ts",
        /// Only the markets of this series.
        series_ticker: impl Into<String> => "series_ticker",
        /// Markets per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketsPage, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/markets/orderbooks` ([`MarketsApi::orderbooks`]).
#[must_use = "a request does nothing until it is sent"]
pub struct GetOrderbooks {
    pub(crate) request: Routed<OrderbooksResponse>,
}

impl GetOrderbooks {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<OrderbooksResponse, PredictionsError> {
        self.request.send().await
    }
}
```

- [ ] **Step 5: Re-export.** In `api/mod.rs`, replace `pub use markets::{GetMarket, GetOrderbook, ListMarkets, ListTrades, MarketsApi};` with:

```rust
pub use markets::{
    GetMarket, GetOrderbook, GetOrderbooks, ListMarketWindow, ListMarkets, ListTrades,
    ListUpdatedMarkets, MarketsApi,
};
```

- [ ] **Step 6: Run the setter cases**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: all pass, `every_markets_catalog_setter_sends_its_key_and_value_in_call_order` included.

### Task 3.4: the mock suite

**Files:**
- Create: `polyoxide-kalshi/tests/mock_predictions_markets_catalog.rs`

- [ ] **Step 1: Write the suite**

```rust
//! Mock-server tests for the markets catalog routes (issue #71): each builder's request, the
//! decoding of its captured body, and the ticker sets refused with nothing sent. The setters'
//! exact pairs are `query_setters_predictions.rs`'s.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketWindow, MarketsPage},
    Predictions, PredictionsError, MAX_ORDERBOOK_TICKERS,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

const MARKETS: &str = "/trade-api/v2/markets";

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

fn ticker(text: &str) -> Ticker {
    Ticker::new(text).unwrap()
}

/// A route answering `body` once, matched on these decoded query pairs.
async fn route(server: &mut ServerGuard, path: &str, pairs: &[(&str, &str)], body: &str) -> Mock {
    let query = Matcher::AllOf(
        pairs
            .iter()
            .map(|(key, value)| Matcher::UrlEncoded((*key).to_owned(), (*value).to_owned()))
            .collect(),
    );
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

/// A server that fails the test if it is asked for anything.
async fn silent() -> (ServerGuard, Mock) {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    (server, nothing)
}

fn page_tickers(page: &MarketsPage) -> Vec<&str> {
    page.markets.iter().map(|market| market.ticker.as_str()).collect()
}

fn wire_tickers(body: &Value) -> Vec<&str> {
    body["markets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|market| market["ticker"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn a_settled_window_sends_its_state_and_bounds_and_decodes_the_captured_page() {
    let name = "catalog/markets_window_settled_page";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        MARKETS,
        &[
            ("status", "settled"),
            ("min_settled_ts", "100"),
            ("max_settled_ts", "200"),
            ("series_ticker", "SER-1"),
            ("limit", "3"),
        ],
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .markets()
        .window(MarketWindow::Settled {
            settled_only: true,
            min_ts: Some(100),
            max_ts: Some(200),
        })
        .series_ticker("SER-1")
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    assert_eq!(page_tickers(&page), wire_tickers(&sent));
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    for (market, row) in page.markets.iter().zip(sent["markets"].as_array().unwrap()) {
        assert_eq!(market.settlement_ts.as_deref(), row["settlement_ts"].as_str());
    }
}

#[tokio::test]
async fn updated_excludes_multivariate_combinations_and_decodes_the_captured_page() {
    let name = "catalog/markets_updated_page";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        MARKETS,
        &[
            ("min_updated_ts", "100"),
            ("mve_filter", "exclude"),
            ("max_updated_ts", "200"),
        ],
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .markets()
        .updated(100)
        .max_ts(200)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    assert_eq!(page_tickers(&page), wire_tickers(&sent));
    for (market, row) in page.markets.iter().zip(sent["markets"].as_array().unwrap()) {
        assert_eq!(market.updated_time, row["updated_time"].as_str().unwrap());
    }
}

#[tokio::test]
async fn tickers_are_sent_comma_joined_and_the_captured_page_decodes() {
    let name = "catalog/markets_tickers_page";
    let sent = wire(name);
    let asked: Vec<Ticker> = wire_tickers(&sent).into_iter().map(ticker).collect();
    let joined = asked
        .iter()
        .map(Ticker::as_str)
        .collect::<Vec<_>>()
        .join(",");
    let mut server = Server::new_async().await;
    let mock = route(&mut server, MARKETS, &[("tickers", joined.as_str())], &fixture(name)).await;
    let page = client(&server)
        .markets()
        .list()
        .tickers(&asked)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(page_tickers(&page), wire_tickers(&sent));
}

#[tokio::test]
async fn orderbooks_sends_one_pair_per_ticker_and_decodes_the_captured_books() {
    let name = "catalog/orderbooks";
    let sent = wire(name);
    let rows = sent["orderbooks"].as_array().unwrap();
    let asked: Vec<Ticker> = rows
        .iter()
        .map(|row| ticker(row["ticker"].as_str().unwrap()))
        .collect();
    let pairs: Vec<(&str, &str)> = asked.iter().map(|t| ("tickers", t.as_str())).collect();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/markets/orderbooks",
        &pairs,
        &fixture(name),
    )
    .await;
    let books = client(&server)
        .markets()
        .orderbooks(&asked)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    assert_eq!(books.orderbooks.len(), rows.len());
    let side = |row: &Value, key: &str| row["orderbook_fp"][key].as_array().map_or(0, Vec::len);
    for (book, row) in books.orderbooks.iter().zip(rows) {
        assert_eq!(book.ticker, row["ticker"].as_str().unwrap());
        assert_eq!(book.orderbook_fp.yes_dollars.len(), side(row, "yes_dollars"));
        assert_eq!(book.orderbook_fp.no_dollars.len(), side(row, "no_dollars"));
    }
}

fn assert_refused(err: &PredictionsError, what: &str) {
    assert!(
        matches!(err, PredictionsError::Api(ApiError::Validation(reason)) if reason.starts_with("tickers: ")),
        "{what}: {err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest, "{what}");
}

#[tokio::test]
async fn an_empty_or_oversized_ticker_set_is_refused_with_nothing_sent() {
    let (server, nothing) = silent().await;
    let kalshi = client(&server);
    // 31 characters each: 250 cost 10,000 bytes, past the 8,000-byte budget.
    let long: Vec<Ticker> = (0..250)
        .map(|i| ticker(&format!("KXLONGTICKER-26OCT22-T{i:09}")))
        .collect();
    let many: Vec<Ticker> = (0..=MAX_ORDERBOOK_TICKERS)
        .map(|i| ticker(&format!("KXT-{i}")))
        .collect();

    let err = kalshi.markets().list().tickers(&[]).send().await.unwrap_err();
    assert_refused(&err, "an empty listing set");
    let err = kalshi.markets().list().tickers(&long).send().await.unwrap_err();
    assert_refused(&err, "a listing set past the budget");
    let err = kalshi
        .markets()
        .window(MarketWindow::Closed {
            closed_only: true,
            min_ts: None,
            max_ts: None,
        })
        .tickers(&[])
        .send()
        .await
        .unwrap_err();
    assert_refused(&err, "an empty window set");
    let err = kalshi.markets().orderbooks(&[]).send().await.unwrap_err();
    assert_refused(&err, "an empty orderbooks set");
    let err = kalshi.markets().orderbooks(&many).send().await.unwrap_err();
    assert_refused(&err, "101 orderbooks");
    nothing.assert_async().await;
}
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test mock_predictions_markets_catalog`
Expected: 5 passed. If a decode fails on a field the capture holds and the type lacks, that is Task 3.6's finding: model it (an optional field), excuse it in `spec_agreement_predictions.rs`'s `OBSERVED_EXTRA`, and record it in `OBSERVED.md`.

### Task 3.5: spec agreement

**Files:**
- Modify: `polyoxide-kalshi/tests/spec_agreement_predictions.rs` (append)

- [ ] **Step 1: Append the section**

```rust
// ── Catalog routes (issue #71) ──────────────────────────────────────

use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{CreatedStatus, MarketOrderbook, MarketWindow, OrderbooksResponse},
};

/// Every named schema reachable by `$ref` from `paths`' `200` responses.
fn reachable_from(paths: &[&str]) -> BTreeSet<String> {
    fn follow(schemas: &Map<String, Value>, node: &Value, seen: &mut BTreeSet<String>) {
        match node {
            Value::Object(map) => {
                if let Some(r) = map.get("$ref").and_then(Value::as_str) {
                    let name = openapi::ref_name(r).to_owned();
                    if seen.insert(name.clone()) {
                        follow(schemas, &schemas[&name], seen);
                    }
                }
                for child in map.values() {
                    follow(schemas, child, seen);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| follow(schemas, child, seen)),
            _ => {}
        }
    }
    let spec = mirror();
    let schemas = mirror_schemas();
    let mut seen = BTreeSet::new();
    for path in paths {
        let ok = &spec["paths"][*path]["get"]["responses"]["200"];
        assert!(ok.is_object(), "the mirror's GET {path} has no 200 response");
        follow(&schemas, ok, &mut seen);
    }
    seen
}

/// The object schemas reachable from `paths` that neither `checked` nor `excused` names.
fn unaccounted_from(paths: &[&str], checked: &[&str], excused: &[&str]) -> Vec<String> {
    let schemas = mirror_schemas();
    reachable_from(paths)
        .into_iter()
        .filter(|name| {
            let schema = &schemas[name];
            schema["type"] == "object"
                || schema.get("allOf").is_some()
                || schema.get("properties").is_some()
        })
        .filter(|name| !checked.contains(&name.as_str()) && !excused.contains(&name.as_str()))
        .collect()
}

#[test]
fn every_markets_catalog_schema_agrees_with_the_spec() {
    let schemas = mirror_schemas();
    check_schema::<MarketOrderbook>(&schemas, "MarketOrderbookFp");
    check_schema::<OrderbooksResponse>(&schemas, "GetMarketOrderbooksResponse");
    let unaccounted = unaccounted_from(
        &["/markets/orderbooks"],
        &["GetMarketOrderbooksResponse", "MarketOrderbookFp", "OrderbookCountFp"],
        &[],
    );
    assert!(
        unaccounted.is_empty(),
        "schemas neither checked nor excused: {unaccounted:?}"
    );
}

#[tokio::test]
async fn the_markets_catalog_builders_send_exactly_the_documented_query_keys() {
    let spec = mirror();
    let mut listing = keys_of("/markets", "catalog/markets_tickers_page", |p| {
        Box::pin(async move {
            p.markets()
                .list()
                .status(MarketStatusFilter::Open)
                .event_ticker("EV-1")
                .series_ticker("SER-1")
                .mve_filter(MveFilter::Exclude)
                .tickers(&[Ticker::new("KXTEST-1").unwrap()])
                .limit(5)
                .cursor("c1")
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    listing.extend(
        keys_of("/markets", "catalog/markets_window_settled_page", |p| {
            Box::pin(async move {
                p.markets()
                    .window(MarketWindow::Created {
                        status: Some(CreatedStatus::Open),
                        min_ts: Some(1),
                        max_ts: Some(2),
                    })
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    listing.extend(
        keys_of("/markets", "catalog/markets_window_settled_page", |p| {
            Box::pin(async move {
                p.markets()
                    .window(MarketWindow::Closed {
                        closed_only: true,
                        min_ts: Some(1),
                        max_ts: Some(2),
                    })
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    listing.extend(
        keys_of("/markets", "catalog/markets_window_settled_page", |p| {
            Box::pin(async move {
                p.markets()
                    .window(MarketWindow::Settled {
                        settled_only: true,
                        min_ts: Some(1),
                        max_ts: Some(2),
                    })
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    listing.extend(
        keys_of("/markets", "catalog/markets_updated_page", |p| {
            Box::pin(async move {
                p.markets()
                    .updated(1)
                    .max_ts(2)
                    .series_ticker("SER-1")
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    assert_eq!(
        Some(listing),
        query::documented_parameters(&spec, "/markets"),
        "the listing's builders together send every documented key, and only those"
    );

    let books = keys_of("/markets/orderbooks", "catalog/orderbooks", |p| {
        Box::pin(async move {
            p.markets()
                .orderbooks(&[Ticker::new("KXTEST-1").unwrap()])
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    assert_eq!(
        Some(books),
        query::documented_parameters(&spec, "/markets/orderbooks")
    );
}
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test spec_agreement_predictions`
Expected: all pass, the two new tests included.

### Task 3.6: wire agreement

**Files:**
- Modify: `polyoxide-kalshi/tests/wire_agreement_predictions.rs` (append)

- [ ] **Step 1: Append the section**

```rust
// ── Catalog routes (issue #71) ──────────────────────────────────

use polyoxide_kalshi::predictions::types::OrderbooksResponse;

/// Holds `T` to the capture `name` with nothing excused: every key the host sent is modelled, and
/// every key the type emits was sent.
fn hold_exactly<T: DeserializeOwned + Serialize>(name: &str) {
    let text = fixtures!("predictions").text(name);
    let diff = agreement::compare::<T>(name, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{name}: the host sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{name}: the type emits {:?}, which the host did not send",
        diff.invented
    );
}

#[test]
fn every_markets_catalog_fixture_agrees_with_its_type() {
    for name in [
        "catalog/markets_updated_page",
        "catalog/markets_tickers_page",
        "catalog/markets_window_settled_page",
    ] {
        hold_exactly::<MarketsPage>(name);
    }
    hold_exactly::<OrderbooksResponse>("catalog/orderbooks");
}

#[test]
fn the_markets_catalog_fixtures_cover_the_cases_the_routes_exist_for() {
    let books: OrderbooksResponse = read("catalog/orderbooks");
    assert_eq!(books.orderbooks.len(), 3, "two active tickers and an unknown one");
    assert!(
        books
            .orderbooks
            .iter()
            .any(|book| book.orderbook_fp.yes_dollars.is_empty() && book.orderbook_fp.no_dollars.is_empty()),
        "the unknown ticker's book is empty"
    );
    let settled: MarketsPage = read("catalog/markets_window_settled_page");
    assert!(!settled.markets.is_empty(), "a week held settled markets");
    assert!(
        settled.markets.iter().all(|market| market.settlement_ts.is_some()),
        "a settled market carries its settlement time"
    );
    let named: MarketsPage = read("catalog/markets_tickers_page");
    assert_eq!(named.markets.len(), 2, "the two tickers asked for");
    let updated: MarketsPage = read("catalog/markets_updated_page");
    assert!(!updated.markets.is_empty(), "a day held updated markets");
}
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test wire_agreement_predictions`
Expected: all pass. A key in a capture that `Market` does not model fails `every_markets_catalog_fixture_agrees_with_its_type` naming it: model it as an optional field of `Market`, add its `("Market", "<field>")` row to `OBSERVED_EXTRA` only if the mirror lacks it, and record it in `OBSERVED.md`'s issue #71 section. A field the mirror documents and the capture sends as `null` where the type skips `None`: drop that field's `skip_serializing_if`.

### Task 3.7: Story 3's gates and commit

- [ ] **Step 1: Run the story gates.** Expected: all pass.

- [ ] **Step 2: Commit**

```bash
git add polyoxide-kalshi/tests/mock_predictions_markets_catalog.rs
git commit -m "feat(kalshi): markets tickers, timestamp windows, updated and orderbooks (#71, Story 3)" -- polyoxide-kalshi/src/predictions/types/markets.rs polyoxide-kalshi/src/predictions/types/mod.rs polyoxide-kalshi/src/predictions/api/markets.rs polyoxide-kalshi/src/predictions/api/mod.rs polyoxide-kalshi/tests/query_setters_predictions.rs polyoxide-kalshi/tests/mock_predictions_markets_catalog.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/wire_agreement_predictions.rs
```

(Add `docs/specs/kalshi/OBSERVED.md` to the pathspec if Task 3.6 recorded a field.)

---

## Story 4: `series()` (spec §7; D9, D12)

### Task 4.1: the types

**Files:**
- Create: `polyoxide-kalshi/src/predictions/types/series.rs`
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs`

- [ ] **Step 1: Write the types with their failing test**

`types/series.rs`:

```rust
//! Series: the templates Kalshi's recurring events follow, with their categories and tags.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

/// A source a series' markets settle against (`SettlementSource`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementSource {
    /// The source's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where it publishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// A series (`Series`): the template of a recurring event, such as a daily temperature.
///
/// A list Kalshi may send as `null` is `None` here and written back as `null`, so the value
/// round-trips: on 2026-10-22, 2,617 of 15,336 series sent `tags: null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    /// The series' ticker, the prefix of its events' tickers.
    pub ticker: String,
    /// How often its events recur, in words: `daily`, `weekly`, `one-off` and so on.
    pub frequency: String,
    /// Its title.
    pub title: String,
    /// Its primary category.
    pub category: String,
    /// Every discovery category it is listed under; `series().list().category(..)` matches any.
    pub categories: Vec<String>,
    /// The subjects it relates to.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// The official sources its markets settle against.
    #[serde(default)]
    pub settlement_sources: Option<Vec<SettlementSource>>,
    /// The original filing of the contract.
    pub contract_url: String,
    /// The current terms of the contract.
    pub contract_terms_url: String,
    /// Internal metadata, as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_metadata: Option<Value>,
    /// The fee structure: `quadratic`, `quadratic_with_maker_fees`,
    /// `quadratic_with_combo_maker_fees` or `flat`.
    pub fee_type: String,
    /// The multiplier applied to the fee, as sent (an integer or a float).
    pub fee_multiplier: Number,
    /// Further trading prohibitions.
    #[serde(default)]
    pub additional_prohibitions: Option<Vec<String>>,
    /// Contracts traded across its events; sent only when asked for with `include_volume`.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "rust_decimal::serde::str_option"
    )]
    pub volume_fp: Option<Decimal>,
    /// When its metadata last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated_ts: Option<String>,
    /// The exchange shard its events trade on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange_index: Option<u32>,
}

/// `GET /series` (`GetSeriesListResponse`): every series the filters admit, in one body with no
/// cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesList {
    /// The series.
    pub series: Vec<Series>,
}

/// `GET /series/{series_ticker}` (`GetSeriesResponse`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesResponse {
    /// The series.
    pub series: Series,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_list_round_trips_as_null_and_a_number_keeps_its_spelling() {
        let text = r#"{"ticker":"KXT","frequency":"daily","title":"T","category":"C","categories":[],"tags":null,"settlement_sources":null,"contract_url":"u","contract_terms_url":"v","fee_type":"flat","fee_multiplier":1,"additional_prohibitions":null}"#;
        let series: Series = serde_json::from_str(text).unwrap();
        assert!(series.tags.is_none() && series.settlement_sources.is_none());
        let back = serde_json::to_value(&series).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(text).unwrap());
    }
}
```

In `types/mod.rs`, add `mod series;` after `mod markets;`, and `pub use series::{Series, SeriesList, SeriesResponse, SettlementSource};` after the `markets` re-export.

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib types::series`
Expected: 1 passed.

### Task 4.2: the setter cases (failing)

**Files:**
- Modify: `polyoxide-kalshi/tests/query_setters_predictions.rs` (append)

- [ ] **Step 1: Append**

```rust
// ── The series builders (issue #71) ─────────────────────────────

const SERIES_CASES: &[Case] = &[
    Case {
        builder: "ListSeries",
        path: "/trade-api/v2/series",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .series()
                    .list()
                    .category("Economics")
                    .tags("Inflation")
                    .include_product_metadata(true)
                    .include_volume(true)
                    .min_updated_ts(9)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("category", "Economics"),
            ("tags", "Inflation"),
            ("include_product_metadata", "true"),
            ("include_volume", "true"),
            ("min_updated_ts", "9"),
        ],
    },
    Case {
        builder: "GetSeries",
        path: "/trade-api/v2/series/KXSER-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .series()
                    .get("KXSER-1")
                    .include_volume(true)
                    .send()
                    .await;
            })
        },
        sends: &[("include_volume", "true")],
    },
];

#[tokio::test]
async fn every_series_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(SERIES_CASES).await;
}
```

- [ ] **Step 2: Run to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: compile error: no method `series` on `Predictions`.

### Task 4.3: the namespace

**Files:**
- Create: `polyoxide-kalshi/src/predictions/api/series.rs`
- Modify: `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/mod.rs`

- [ ] **Step 1: Write `api/series.rs`**

```rust
//! Series: the templates Kalshi's recurring events follow, with their categories and tags.

use std::sync::Arc;

use crate::predictions::{
    request::{routes, Routed},
    types::{SeriesList, SeriesResponse},
    Inner, PredictionsError,
};

/// Series namespace.
#[derive(Debug, Clone)]
pub struct SeriesApi {
    pub(crate) inner: Arc<Inner>,
}

impl SeriesApi {
    /// `GET /trade-api/v2/series`: every series the filters admit, in one body with no cursor.
    ///
    /// Unfiltered it is every series Kalshi lists, 15,336 of them in 19 MB on 2026-10-22, so a
    /// caller filters by `category` or `min_updated_ts` where it can.
    pub fn list(&self) -> ListSeries {
        ListSeries {
            request: Routed::new(&self.inner, routes::SERIES_LIST, &[]),
        }
    }

    /// `GET /trade-api/v2/series/{series_ticker}`: one series.
    pub fn get(&self, series_ticker: &str) -> GetSeries {
        GetSeries {
            request: Routed::new(&self.inner, routes::SERIES, &[series_ticker]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/series`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListSeries {
    pub(crate) request: Routed<SeriesList>,
}

impl ListSeries {
    polyoxide_core::query_setters! {
        /// Only series listed under this discovery category, matched exactly and case-sensitively
        /// against any of a series' `categories`.
        category: impl Into<String> => "category",
        /// Only series with these tags.
        tags: impl Into<String> => "tags",
        /// Include each series' `product_metadata`.
        include_product_metadata: bool => "include_product_metadata",
        /// Include each series' traded volume, `volume_fp`.
        include_volume: bool => "include_volume",
        /// Only series whose metadata changed after this Unix time, in seconds.
        min_updated_ts: i64 => "min_updated_ts",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<SeriesList, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/series/{series_ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetSeries {
    pub(crate) request: Routed<SeriesResponse>,
}

impl GetSeries {
    polyoxide_core::query_setters! {
        /// Include the series' traded volume, `volume_fp`.
        include_volume: bool => "include_volume",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<SeriesResponse, PredictionsError> {
        self.request.send().await
    }
}
```

- [ ] **Step 2: Wire it.** In `api/mod.rs`, add `pub mod series;` after `pub mod markets;` and `pub use series::{GetSeries, ListSeries, SeriesApi};` after the `markets` re-export. In `predictions/mod.rs`'s `polyoxide_core::namespaces! { inner; … }`, after the `events` entry, add:

```rust
        /// Series: the templates events follow, with their categories and tags.
        series: api::SeriesApi,
```

- [ ] **Step 3: Run the setter cases**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: all pass.

### Task 4.4: the mock suite

**Files:**
- Create: `polyoxide-kalshi/tests/mock_predictions_series.rs`

- [ ] **Step 1: Write the suite**

```rust
//! Mock-server tests for the series routes (issue #71): each request and the decoding of its
//! captured body.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_kalshi::predictions::Predictions;
use polyoxide_test_support::fixtures;
use serde_json::Value;

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str) -> Mock {
    server
        .mock("GET", path)
        .match_query(Matcher::Exact(query.to_owned()))
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn the_series_list_sends_its_filters_and_decodes_the_captured_body() {
    let name = "catalog/series_list";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/series",
        "category=Economics&min_updated_ts=9",
        &fixture(name),
    )
    .await;
    let list = client(&server)
        .series()
        .list()
        .category("Economics")
        .min_updated_ts(9)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    let rows = sent["series"].as_array().unwrap();
    assert_eq!(list.series.len(), rows.len());
    for (series, row) in list.series.iter().zip(rows) {
        assert_eq!(series.ticker, row["ticker"].as_str().unwrap());
        assert_eq!(series.category, row["category"].as_str().unwrap());
        assert_eq!(series.tags.is_none(), row["tags"].is_null());
        assert_eq!(series.fee_multiplier.to_string(), row["fee_multiplier"].to_string());
    }
}

#[tokio::test]
async fn a_series_is_read_by_ticker_with_its_volume() {
    let name = "catalog/series";
    let sent = wire(name);
    let ticker = sent["series"]["ticker"].as_str().unwrap();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/series/{ticker}"),
        "include_volume=true",
        &fixture(name),
    )
    .await;
    let read = client(&server)
        .series()
        .get(ticker)
        .include_volume(true)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.series.ticker, ticker);
    assert_eq!(
        read.series.volume_fp.map(|volume| volume.to_string()).as_deref(),
        sent["series"]["volume_fp"].as_str()
    );
}
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test mock_predictions_series`
Expected: 2 passed.

### Task 4.5: spec and wire agreement

**Files:**
- Modify: `polyoxide-kalshi/tests/spec_agreement_predictions.rs` (append)
- Modify: `polyoxide-kalshi/tests/wire_agreement_predictions.rs` (append)

- [ ] **Step 1: Append to `spec_agreement_predictions.rs`**

```rust
use polyoxide_kalshi::predictions::types::{Series, SeriesList, SeriesResponse, SettlementSource};

#[test]
fn every_series_schema_agrees_with_the_spec() {
    let schemas = mirror_schemas();
    check_schema::<SettlementSource>(&schemas, "SettlementSource");
    check_schema::<Series>(&schemas, "Series");
    check_schema::<SeriesList>(&schemas, "GetSeriesListResponse");
    check_schema::<SeriesResponse>(&schemas, "GetSeriesResponse");
    let unaccounted = unaccounted_from(
        &["/series", "/series/{series_ticker}"],
        &["Series", "SettlementSource", "GetSeriesListResponse", "GetSeriesResponse"],
        &[],
    );
    assert!(
        unaccounted.is_empty(),
        "schemas neither checked nor excused: {unaccounted:?}"
    );
}

#[tokio::test]
async fn the_series_builders_send_exactly_the_documented_query_keys() {
    let spec = mirror();
    let list = keys_of("/series", "catalog/series_list", |p| {
        Box::pin(async move {
            p.series()
                .list()
                .category("Economics")
                .tags("Inflation")
                .include_product_metadata(true)
                .include_volume(true)
                .min_updated_ts(9)
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    assert_eq!(Some(list), query::documented_parameters(&spec, "/series"));
    let one = keys_of("/series/KXSER-1", "catalog/series", |p| {
        Box::pin(async move {
            p.series()
                .get("KXSER-1")
                .include_volume(true)
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    assert_eq!(
        Some(one),
        query::documented_parameters(&spec, "/series/{series_ticker}")
    );
}
```

- [ ] **Step 2: Append to `wire_agreement_predictions.rs`**

```rust
use polyoxide_kalshi::predictions::types::{SeriesList, SeriesResponse};

#[test]
fn every_series_fixture_agrees_with_its_type() {
    hold_exactly::<SeriesList>("catalog/series_list");
    hold_exactly::<SeriesResponse>("catalog/series");
}

#[test]
fn the_series_fixtures_cover_the_cases_the_types_exist_for() {
    let list: SeriesList = read("catalog/series_list");
    assert!(!list.series.is_empty());
    let one: SeriesResponse = read("catalog/series");
    assert!(one.series.volume_fp.is_some(), "series.json was asked for its volume");
}
```

- [ ] **Step 3: Run both**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test spec_agreement_predictions --test wire_agreement_predictions`
Expected: all pass. A key the capture holds and `Series` lacks: model it, as Task 3.6 says.

### Task 4.6: Story 4's gates and commit

- [ ] **Step 1: Run the story gates.** Expected: all pass.

- [ ] **Step 2: Commit**

```bash
git add polyoxide-kalshi/src/predictions/types/series.rs polyoxide-kalshi/src/predictions/api/series.rs polyoxide-kalshi/tests/mock_predictions_series.rs
git commit -m "feat(kalshi): the series namespace (#71, Story 4)" -- polyoxide-kalshi/src/predictions/types/series.rs polyoxide-kalshi/src/predictions/types/mod.rs polyoxide-kalshi/src/predictions/api/series.rs polyoxide-kalshi/src/predictions/api/mod.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/tests/query_setters_predictions.rs polyoxide-kalshi/tests/mock_predictions_series.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/wire_agreement_predictions.rs
```

---

## Story 5: `events().list()` (spec §8; D10, D12)

### Task 5.1: the types

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/types/events.rs` (append; imports)
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs`

- [ ] **Step 1: Write the failing test.** Append to `types/events.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_reads_milestones_null_or_missing_as_empty() {
        for body in [
            r#"{"events":[],"cursor":"","milestones":null}"#,
            r#"{"events":[],"cursor":""}"#,
        ] {
            let page: EventsPage = serde_json::from_str(body).unwrap();
            assert!(page.milestones.is_empty(), "{body}");
        }
    }

    #[test]
    fn the_event_states_are_the_four_the_route_documents() {
        let spelled: Vec<&str> = EventStatusFilter::ALL.iter().map(|s| s.as_str()).collect();
        assert_eq!(spelled, ["unopened", "open", "closed", "settled"]);
        assert!("paused".parse::<EventStatusFilter>().is_err());
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib types::events`
Expected: compile errors: `EventsPage`, `EventStatusFilter` not found.

- [ ] **Step 3: Implement.** In `types/events.rs`, before the new `#[cfg(test)]`, add:

```rust
polyoxide_venue::wire_enum! {
    /// The states `GET /events` filters by.
    #[non_exhaustive]
    pub enum EventStatusFilter {
        /// Not yet open.
        Unopened => "unopened",
        /// Open for trading.
        Open => "open",
        /// Closed.
        Closed => "closed",
        /// Settled.
        Settled => "settled",
    }
}

/// `GET /events` (`GetEventsResponse`): one page of events, multivariate events excluded. An
/// empty `cursor` ends a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventsPage {
    /// The page's events.
    pub events: Vec<Event>,
    /// The events' milestones, as sent; Kalshi sends the list, empty, when none were asked for.
    #[serde(default, deserialize_with = "null_as_default")]
    pub milestones: Vec<Value>,
    /// The cursor of the next page, empty on the last.
    pub cursor: String,
}
```

In `types/mod.rs`, replace `pub use events::{Event, EventResponse};` with `pub use events::{Event, EventResponse, EventStatusFilter, EventsPage};`.

- [ ] **Step 4: Run the tests**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib types::events`
Expected: 2 passed.

### Task 5.2: the setter cases (failing)

**Files:**
- Modify: `polyoxide-kalshi/tests/query_setters_predictions.rs` (append)

- [ ] **Step 1: Append**

```rust
// ── The events listing (issue #71) ──────────────────────────────

use polyoxide_kalshi::predictions::types::EventStatusFilter;

const EVENTS_CASES: &[Case] = &[
    Case {
        builder: "ListEvents",
        path: "/trade-api/v2/events",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .list()
                    .limit(5)
                    .cursor("c1")
                    .with_nested_markets(true)
                    .status(EventStatusFilter::Open)
                    .series_ticker("SER-1")
                    .tickers(["EV-1", "EV-2"])
                    .min_close_ts(6)
                    .min_updated_ts(7)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("limit", "5"),
            ("cursor", "c1"),
            ("with_nested_markets", "true"),
            ("status", "open"),
            ("series_ticker", "SER-1"),
            ("tickers", "EV-1,EV-2"),
            ("min_close_ts", "6"),
            ("min_updated_ts", "7"),
        ],
    },
];

#[tokio::test]
async fn every_events_listing_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(EVENTS_CASES).await;
}
```

- [ ] **Step 2: Run to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: compile error: no method `list` on `EventsApi`.

### Task 5.3: the builder

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/api/events.rs`
- Modify: `polyoxide-kalshi/src/predictions/api/mod.rs`

- [ ] **Step 1: Implement.** Replace `api/events.rs`'s `use` block with:

```rust
use crate::predictions::{
    request::{routes, Routed},
    types::{EventResponse, EventStatusFilter, EventsPage},
    Inner, PredictionsError,
};
```

In `impl EventsApi`, before `get`, add:

```rust
    /// `GET /trade-api/v2/events`: one page of events, multivariate events excluded.
    ///
    /// Kalshi combines these filters and refuses none of them, unlike `GET /markets`
    /// (`docs/specs/kalshi/OBSERVED.md`). Nested markets leave out those settled before the
    /// historical cutoff, which `historical().markets` reads.
    pub fn list(&self) -> ListEvents {
        ListEvents {
            request: Routed::new(&self.inner, routes::EVENTS, &[]),
        }
    }
```

After `impl GetEvent { … }`, add:

```rust
/// Request builder for `GET /trade-api/v2/events`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListEvents {
    pub(crate) request: Routed<EventsPage>,
}

impl ListEvents {
    polyoxide_core::query_setters! {
        /// Events per page, up to 200, the host's default.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
        /// Nest each event's markets in `event.markets`.
        with_nested_markets: bool => "with_nested_markets",
        /// Only events in this state.
        status: EventStatusFilter => "status",
        /// Only the events of this series.
        series_ticker: impl Into<String> => "series_ticker",
        /// Only events with a market that closes after this Unix time, in seconds.
        min_close_ts: i64 => "min_close_ts",
        /// Only events whose metadata changed after this Unix time, in seconds.
        min_updated_ts: i64 => "min_updated_ts",
    }

    /// Only these events, comma-joined. An empty set, or one past
    /// [`TICKERS_QUERY_BUDGET`](crate::predictions::TICKERS_QUERY_BUDGET), is refused when sent,
    /// with nothing sent. Setting it again replaces the set and any refusal.
    pub fn tickers<S: Into<String>>(mut self, tickers: impl IntoIterator<Item = S>) -> Self {
        let tickers: Vec<String> = tickers.into_iter().map(Into::into).collect();
        let texts: Vec<&str> = tickers.iter().map(String::as_str).collect();
        self.request = self.request.tickers_joined("tickers", &texts);
        self
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<EventsPage, PredictionsError> {
        self.request.send().await
    }
}
```

In `api/mod.rs`, replace `pub use events::{EventsApi, GetEvent};` with `pub use events::{EventsApi, GetEvent, ListEvents};`.

- [ ] **Step 2: Run the setter cases**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: all pass.

### Task 5.4: the mock suite

**Files:**
- Create: `polyoxide-kalshi/tests/mock_predictions_events.rs`

- [ ] **Step 1: Write the suite**

```rust
//! Mock-server tests for the events listing (issue #71): its request, the decoding of its captured
//! pages, and an empty event set refused with nothing sent.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{types::EventStatusFilter, Predictions, PredictionsError};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

const EVENTS: &str = "/trade-api/v2/events";

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

async fn route(server: &mut ServerGuard, query: &str, body: &str) -> Mock {
    server
        .mock("GET", EVENTS)
        .match_query(Matcher::Exact(query.to_owned()))
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

#[tokio::test]
async fn the_listing_sends_its_filters_and_decodes_the_captured_page() {
    let name = "catalog/events_page";
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "limit=3&status=open&min_updated_ts=9", &fixture(name)).await;
    let page = client(&server)
        .events()
        .list()
        .limit(3)
        .status(EventStatusFilter::Open)
        .min_updated_ts(9)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    let rows = sent["events"].as_array().unwrap();
    assert_eq!(page.events.len(), rows.len());
    assert_eq!(page.cursor, sent["cursor"].as_str().unwrap());
    for (event, row) in page.events.iter().zip(rows) {
        assert_eq!(event.event_ticker, row["event_ticker"].as_str().unwrap());
        assert!(event.markets.is_empty(), "markets are nested only when asked for");
    }
}

#[tokio::test]
async fn a_nested_page_carries_each_events_markets() {
    let name = "catalog/events_page_nested";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "status=open&with_nested_markets=true",
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .events()
        .list()
        .status(EventStatusFilter::Open)
        .with_nested_markets(true)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;

    let sent = wire(name);
    for (event, row) in page.events.iter().zip(sent["events"].as_array().unwrap()) {
        let nested = row["markets"].as_array().map_or(0, Vec::len);
        assert_eq!(event.markets.len(), nested, "{}", event.event_ticker);
    }
}

#[tokio::test]
async fn an_empty_event_set_is_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let err = client(&server)
        .events()
        .list()
        .tickers(Vec::<String>::new())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PredictionsError::Api(ApiError::Validation(reason)) if reason.starts_with("tickers: ")),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest);
    nothing.assert_async().await;
}
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test mock_predictions_events`
Expected: 3 passed.

### Task 5.5: spec and wire agreement

**Files:**
- Modify: `polyoxide-kalshi/tests/spec_agreement_predictions.rs` (append)
- Modify: `polyoxide-kalshi/tests/wire_agreement_predictions.rs` (append)

- [ ] **Step 1: Append to `spec_agreement_predictions.rs`**

```rust
use polyoxide_kalshi::predictions::types::{EventStatusFilter, EventsPage};

#[test]
fn every_events_listing_schema_agrees_with_the_spec() {
    let spec = mirror();
    let schemas = mirror_schemas();
    check_schema::<EventsPage>(&schemas, "GetEventsResponse");
    let unaccounted = unaccounted_from(
        &["/events"],
        &["GetEventsResponse", "EventData", "Market", "PriceRange", "SettlementSource"],
        &["Milestone", "MveSelectedLeg"],
    );
    assert!(
        unaccounted.is_empty(),
        "schemas neither checked nor excused: {unaccounted:?}"
    );
    let parameters = spec["paths"]["/events"]["get"]["parameters"]
        .as_array()
        .expect("/events documents query parameters");
    let status = parameters
        .iter()
        .find(|parameter| parameter["name"] == "status")
        .expect("/events documents status");
    assert_eq!(
        wire(EventStatusFilter::ALL),
        documented(&status["schema"]["enum"], "/events status")
    );
}

#[tokio::test]
async fn the_events_listing_sends_every_documented_key_but_with_milestones() {
    let spec = mirror();
    let sent = keys_of("/events", "catalog/events_page", |p| {
        Box::pin(async move {
            p.events()
                .list()
                .limit(5)
                .cursor("c1")
                .with_nested_markets(true)
                .status(EventStatusFilter::Open)
                .series_ticker("SER-1")
                .tickers(["EV-1"])
                .min_close_ts(6)
                .min_updated_ts(7)
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    let mut documented = query::documented_parameters(&spec, "/events").unwrap_or_default();
    // Milestones are kept as raw JSON, so the listing never asks for them.
    assert!(documented.remove("with_milestones"));
    assert_eq!(sent, documented);
}
```

(`wire` and `documented` are the file's enum helpers.)

- [ ] **Step 2: Append to `wire_agreement_predictions.rs`**

```rust
use polyoxide_kalshi::predictions::types::EventsPage;

#[test]
fn every_events_listing_fixture_agrees_with_its_type() {
    hold_exactly::<EventsPage>("catalog/events_page");
    hold_exactly::<EventsPage>("catalog/events_page_nested");
}

#[test]
fn the_events_listing_fixtures_cover_the_cases_the_type_exists_for() {
    let flat: EventsPage = read("catalog/events_page");
    assert!(!flat.events.is_empty());
    assert!(!flat.cursor.is_empty(), "a page with more behind it");
    let nested: EventsPage = read("catalog/events_page_nested");
    assert!(
        nested.events.iter().any(|event| !event.markets.is_empty()),
        "a nested page nests markets"
    );
}
```

- [ ] **Step 3: Run both**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test spec_agreement_predictions --test wire_agreement_predictions`
Expected: all pass.

### Task 5.6: Story 5's gates and commit

- [ ] **Step 1: Run the story gates.** Expected: all pass.

- [ ] **Step 2: Commit**

```bash
git add polyoxide-kalshi/tests/mock_predictions_events.rs
git commit -m "feat(kalshi): the events listing (#71, Story 5)" -- polyoxide-kalshi/src/predictions/types/events.rs polyoxide-kalshi/src/predictions/types/mod.rs polyoxide-kalshi/src/predictions/api/events.rs polyoxide-kalshi/src/predictions/api/mod.rs polyoxide-kalshi/tests/query_setters_predictions.rs polyoxide-kalshi/tests/mock_predictions_events.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/wire_agreement_predictions.rs
```

---

## Story 6: `historical()` (spec §9; D11, D12)

### Task 6.1: the types

**Files:**
- Create: `polyoxide-kalshi/src/predictions/types/historical.rs`
- Modify: `polyoxide-kalshi/src/predictions/types/mod.rs`

- [ ] **Step 1: Write the types with their failing test**

`types/historical.rs`:

```rust
//! The archive: markets that settled before Kalshi's historical cutoff, their candlesticks and
//! their trades, which the live routes no longer serve.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::predictions::key::Ticker;

/// `GET /historical/cutoff` (`GetHistoricalCutoffResponse`): where the archive begins, each time
/// RFC 3339 text as sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalCutoff {
    /// A market that settled before this is served only by the historical routes.
    pub market_settled_ts: String,
    /// A fill before this is served only by `GET /historical/fills`.
    pub trades_created_ts: String,
    /// An order cancelled or executed before this is served only by `GET /historical/orders`.
    pub orders_updated_ts: String,
    /// The backfill horizon of settled positions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub market_positions_last_updated_ts: Option<String>,
}

/// Which archived markets `historical().markets` lists.
///
/// Kalshi takes one of these at a time and refuses two with a `400`
/// (`docs/specs/kalshi/OBSERVED.md`), so the choice is one value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HistoricalSelection {
    /// Every archived market, multivariate combinations included.
    All,
    /// Every archived market but the multivariate combinations (`mve_filter=exclude`).
    ExcludeMultivariate,
    /// These markets, comma-joined. An empty set, or one past
    /// [`TICKERS_QUERY_BUDGET`](crate::predictions::TICKERS_QUERY_BUDGET), is refused when sent,
    /// with nothing sent.
    Tickers(Vec<Ticker>),
    /// The markets of one event.
    Event(String),
    /// The markets of one series.
    Series(String),
}

/// Offers on one side of an archived market over a candlestick's period
/// (`BidAskDistributionHistorical`), in dollars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalBidAsk {
    /// The price at the start of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open: Decimal,
    /// The lowest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub low: Decimal,
    /// The highest price in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub high: Decimal,
    /// The price at the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub close: Decimal,
}

/// Trade prices of an archived market over a candlestick's period (`PriceDistributionHistorical`),
/// in dollars. Kalshi sends a price it has no value for as `null`, where the live route leaves it
/// out, so a `None` is written back as `null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalPrice {
    /// The first YES trade of the period.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub open: Option<Decimal>,
    /// The lowest YES trade of the period.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub low: Option<Decimal>,
    /// The highest YES trade of the period.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub high: Option<Decimal>,
    /// The last YES trade of the period.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub close: Option<Decimal>,
    /// The mean YES trade price of the period.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub mean: Option<Decimal>,
    /// The last YES trade before the period.
    #[serde(default, with = "rust_decimal::serde::str_option")]
    pub previous: Option<Decimal>,
}

/// One archived candlestick (`MarketCandlestickHistorical`), named by when its period ends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalCandlestick {
    /// The end of the period, inclusive, in Unix seconds.
    pub end_period_ts: i64,
    /// Offers to buy YES.
    pub yes_bid: HistoricalBidAsk,
    /// Offers to sell YES.
    pub yes_ask: HistoricalBidAsk,
    /// Trades.
    pub price: HistoricalPrice,
    /// Contracts traded in the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub volume: Decimal,
    /// Contracts bought, without netting, by the end of the period.
    #[serde(with = "rust_decimal::serde::str")]
    pub open_interest: Decimal,
}

/// `GET /historical/markets/{ticker}/candlesticks` (`GetMarketCandlesticksHistoricalResponse`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoricalCandlesticksResponse {
    /// The market's ticker.
    pub ticker: String,
    /// Its candlesticks.
    pub candlesticks: Vec<HistoricalCandlestick>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_price_is_null_on_the_wire_and_round_trips_as_null() {
        let text = r#"{"open":null,"low":null,"high":null,"close":null,"mean":null,"previous":"0.4200"}"#;
        let price: HistoricalPrice = serde_json::from_str(text).unwrap();
        assert!(price.close.is_none());
        assert_eq!(price.previous.map(|p| p.to_string()), Some("0.4200".to_owned()));
        assert_eq!(serde_json::to_string(&price).unwrap(), text);
    }
}
```

In `types/mod.rs`, add `mod historical;` after `mod exchange;`, and:

```rust
pub use historical::{
    HistoricalBidAsk, HistoricalCandlestick, HistoricalCandlesticksResponse, HistoricalCutoff,
    HistoricalPrice, HistoricalSelection,
};
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib types::historical`
Expected: 1 passed.

### Task 6.2: the setter cases (failing)

**Files:**
- Modify: `polyoxide-kalshi/tests/query_setters_predictions.rs` (append)

- [ ] **Step 1: Append**

```rust
// ── The archive (issue #71) ─────────────────────────────────────

use polyoxide_kalshi::predictions::types::HistoricalSelection;

/// Each selection sends its one key; the candlestick builder its three arguments in the order the
/// route documents them; the trades builder its setters in call order.
const HISTORICAL_CASES: &[Case] = &[
    Case {
        builder: "GetHistoricalCutoff",
        path: "/trade-api/v2/historical/cutoff",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).historical().cutoff().send().await;
            })
        },
        sends: &[],
    },
    Case {
        builder: "ListHistoricalMarkets, all",
        path: "/trade-api/v2/historical/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .markets(HistoricalSelection::All)
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[("limit", "5"), ("cursor", "c1")],
    },
    Case {
        builder: "ListHistoricalMarkets, multivariate excluded",
        path: "/trade-api/v2/historical/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .markets(HistoricalSelection::ExcludeMultivariate)
                    .send()
                    .await;
            })
        },
        sends: &[("mve_filter", "exclude")],
    },
    Case {
        builder: "ListHistoricalMarkets, tickers",
        path: "/trade-api/v2/historical/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .markets(HistoricalSelection::Tickers(vec![ticker(), second()]))
                    .send()
                    .await;
            })
        },
        sends: &[("tickers", "KXTEST-1,KXTEST-2")],
    },
    Case {
        builder: "ListHistoricalMarkets, event",
        path: "/trade-api/v2/historical/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .markets(HistoricalSelection::Event("EV-1".to_owned()))
                    .send()
                    .await;
            })
        },
        sends: &[("event_ticker", "EV-1")],
    },
    Case {
        builder: "ListHistoricalMarkets, series",
        path: "/trade-api/v2/historical/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .markets(HistoricalSelection::Series("SER-1".to_owned()))
                    .send()
                    .await;
            })
        },
        sends: &[("series_ticker", "SER-1")],
    },
    Case {
        builder: "GetHistoricalMarket",
        path: "/trade-api/v2/historical/markets/KXTEST-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).historical().market(&ticker()).send().await;
            })
        },
        sends: &[],
    },
    Case {
        builder: "GetHistoricalCandlesticks",
        path: "/trade-api/v2/historical/markets/KXTEST-1/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .candlesticks(&ticker(), PeriodInterval::Hour1, 100, 200)
                    .send()
                    .await;
            })
        },
        sends: &[("start_ts", "100"), ("end_ts", "200"), ("period_interval", "60")],
    },
    Case {
        builder: "ListHistoricalTrades",
        path: "/trade-api/v2/historical/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .historical()
                    .trades()
                    .ticker(&ticker())
                    .min_ts(1)
                    .max_ts(2)
                    .limit(5)
                    .cursor("c1")
                    .is_block_trade(false)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("ticker", "KXTEST-1"),
            ("min_ts", "1"),
            ("max_ts", "2"),
            ("limit", "5"),
            ("cursor", "c1"),
            ("is_block_trade", "false"),
        ],
    },
];

#[tokio::test]
async fn every_historical_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(HISTORICAL_CASES).await;
}
```

- [ ] **Step 2: Run to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: compile error: no method `historical` on `Predictions`.

### Task 6.3: the namespace

**Files:**
- Create: `polyoxide-kalshi/src/predictions/api/historical.rs`
- Modify: `polyoxide-kalshi/src/predictions/api/mod.rs`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/api/markets.rs` (one doc sentence)

- [ ] **Step 1: Write `api/historical.rs`**

```rust
//! The archive: markets that settled before Kalshi's historical cutoff, their candlesticks and
//! their trades, which the live routes answer as if they did not exist.

use std::sync::Arc;

use crate::predictions::{
    key::Ticker,
    request::{routes, Routed},
    types::{
        HistoricalCandlesticksResponse, HistoricalCutoff, HistoricalSelection, MarketResponse,
        MarketsPage, MveFilter, PeriodInterval, TradesPage,
    },
    Inner, PredictionsError,
};

/// Historical namespace.
#[derive(Debug, Clone)]
pub struct HistoricalApi {
    pub(crate) inner: Arc<Inner>,
}

impl HistoricalApi {
    /// `GET /trade-api/v2/historical/cutoff`: where the archive begins. A market that settled
    /// before `market_settled_ts` is served only by this namespace's routes. The venue traits read
    /// archived markets without it, from what the live routes answer.
    pub fn cutoff(&self) -> GetHistoricalCutoff {
        GetHistoricalCutoff {
            request: Routed::new(&self.inner, routes::HISTORICAL_CUTOFF, &[]),
        }
    }

    /// `GET /trade-api/v2/historical/markets`: one page of archived markets, chosen by one
    /// selection, since Kalshi refuses two.
    pub fn markets(&self, selection: HistoricalSelection) -> ListHistoricalMarkets {
        let request = Routed::new(&self.inner, routes::HISTORICAL_MARKETS, &[]);
        let request = match &selection {
            HistoricalSelection::All => request,
            HistoricalSelection::ExcludeMultivariate => {
                request.query("mve_filter", MveFilter::Exclude)
            }
            HistoricalSelection::Tickers(tickers) => {
                let texts: Vec<&str> = tickers.iter().map(Ticker::as_str).collect();
                request.tickers_joined("tickers", &texts)
            }
            HistoricalSelection::Event(event_ticker) => request.query("event_ticker", event_ticker),
            HistoricalSelection::Series(series_ticker) => {
                request.query("series_ticker", series_ticker)
            }
        };
        ListHistoricalMarkets { request }
    }

    /// `GET /trade-api/v2/historical/markets/{ticker}`: one archived market. A market not in the
    /// archive is a `404`, a live one included.
    pub fn market(&self, ticker: &Ticker) -> GetHistoricalMarket {
        GetHistoricalMarket {
            request: Routed::new(&self.inner, routes::HISTORICAL_MARKET, &[ticker.as_str()]),
        }
    }

    /// `GET /trade-api/v2/historical/markets/{ticker}/candlesticks`: an archived market's
    /// candlesticks of one width that end between `start_ts` and `end_ts`, both inclusive and in
    /// Unix seconds. An unknown ticker is a `404`.
    pub fn candlesticks(
        &self,
        ticker: &Ticker,
        period: PeriodInterval,
        start_ts: i64,
        end_ts: i64,
    ) -> GetHistoricalCandlesticks {
        GetHistoricalCandlesticks {
            request: Routed::new(
                &self.inner,
                routes::HISTORICAL_CANDLESTICKS,
                &[ticker.as_str()],
            )
            .query("start_ts", start_ts)
            .query("end_ts", end_ts)
            .query("period_interval", period),
        }
    }

    /// `GET /trade-api/v2/historical/trades`: archived trades across markets, newest first.
    pub fn trades(&self) -> ListHistoricalTrades {
        ListHistoricalTrades {
            request: Routed::new(&self.inner, routes::HISTORICAL_TRADES, &[]),
        }
    }
}

/// Request builder for `GET /trade-api/v2/historical/cutoff`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetHistoricalCutoff {
    pub(crate) request: Routed<HistoricalCutoff>,
}

impl GetHistoricalCutoff {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<HistoricalCutoff, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/markets`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListHistoricalMarkets {
    pub(crate) request: Routed<MarketsPage>,
}

impl ListHistoricalMarkets {
    polyoxide_core::query_setters! {
        /// Markets per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketsPage, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/markets/{ticker}`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetHistoricalMarket {
    pub(crate) request: Routed<MarketResponse>,
}

impl GetHistoricalMarket {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<MarketResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/markets/{ticker}/candlesticks`.
#[must_use = "a request does nothing until it is sent"]
pub struct GetHistoricalCandlesticks {
    pub(crate) request: Routed<HistoricalCandlesticksResponse>,
}

impl GetHistoricalCandlesticks {
    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<HistoricalCandlesticksResponse, PredictionsError> {
        self.request.send().await
    }
}

/// Request builder for `GET /trade-api/v2/historical/trades`.
#[must_use = "a request does nothing until it is sent"]
pub struct ListHistoricalTrades {
    pub(crate) request: Routed<TradesPage>,
}

impl ListHistoricalTrades {
    polyoxide_core::query_setters! {
        /// Only the trades of this market.
        ticker: &Ticker => "ticker",
        /// Only trades at or after this Unix time, in seconds.
        min_ts: i64 => "min_ts",
        /// Only trades at or before this Unix time, in seconds.
        max_ts: i64 => "max_ts",
        /// Trades per page, up to 1000; the host defaults to 100.
        limit: u32 => "limit",
        /// The cursor of the page to read.
        cursor: impl Into<String> => "cursor",
        /// Only block trades, or only the others; every trade when unset.
        is_block_trade: bool => "is_block_trade",
    }

    /// Waits for the Read budget, sends, and decodes the response.
    pub async fn send(self) -> Result<TradesPage, PredictionsError> {
        self.request.send().await
    }
}
```

- [ ] **Step 2: Wire it.** In `api/mod.rs`, add `pub mod historical;` after `pub mod health;` and:

```rust
pub use historical::{
    GetHistoricalCandlesticks, GetHistoricalCutoff, GetHistoricalMarket, HistoricalApi,
    ListHistoricalMarkets, ListHistoricalTrades,
};
```

In `predictions/mod.rs`'s `namespaces!`, after the `series` entry, add:

```rust
        /// The archive: markets settled before the historical cutoff, with their candlesticks
        /// and trades.
        historical: api::HistoricalApi,
```

In `api/markets.rs`, in `candlesticks`' doc comment, replace

```rust
    /// A market that settled before Kalshi's historical cutoff has candlesticks only on
    /// `GET /historical/markets/{ticker}/candlesticks`, which this client does not call.
```

with

```rust
    /// A market that settled before Kalshi's historical cutoff has candlesticks only on
    /// `GET /historical/markets/{ticker}/candlesticks`, which `historical().candlesticks` reads.
```

- [ ] **Step 3: Run the setter cases**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions`
Expected: all pass.

### Task 6.4: the mock suite

**Files:**
- Create: `polyoxide-kalshi/tests/mock_predictions_historical.rs`

- [ ] **Step 1: Write the suite**

```rust
//! Mock-server tests for the archive (issue #71): each request and the decoding of its captured
//! body, and an empty ticker selection refused with nothing sent.

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{HistoricalSelection, PeriodInterval},
    Predictions, PredictionsError,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{Class, Classify};
use serde_json::Value;

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn wire(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .build()
        .unwrap()
}

/// A route answering `body` once, matched on its whole query string, or on its having none.
async fn route(server: &mut ServerGuard, path: &str, query: &str, body: &str) -> Mock {
    let query = if query.is_empty() {
        Matcher::Missing
    } else {
        Matcher::Exact(query.to_owned())
    };
    server
        .mock("GET", path)
        .match_query(query)
        .with_status(200)
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

/// The archived market the capture holds.
fn archived() -> Ticker {
    Ticker::new(wire("historical/market")["market"]["ticker"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn the_cutoff_decodes_the_captured_body() {
    let name = "historical/cutoff";
    let mut server = Server::new_async().await;
    let mock = route(&mut server, "/trade-api/v2/historical/cutoff", "", &fixture(name)).await;
    let cutoff = client(&server).historical().cutoff().send().await.unwrap();
    mock.assert_async().await;
    let sent = wire(name);
    assert_eq!(cutoff.market_settled_ts, sent["market_settled_ts"].as_str().unwrap());
    assert_eq!(cutoff.trades_created_ts, sent["trades_created_ts"].as_str().unwrap());
    assert_eq!(cutoff.orders_updated_ts, sent["orders_updated_ts"].as_str().unwrap());
}

#[tokio::test]
async fn a_series_selection_lists_the_captured_page() {
    let name = "historical/markets_page";
    let sent = wire(name);
    let series = sent["markets"][0]["ticker"]
        .as_str()
        .unwrap()
        .split('-')
        .next()
        .unwrap()
        .to_owned();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/historical/markets",
        &format!("series_ticker={series}&limit=3"),
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .historical()
        .markets(HistoricalSelection::Series(series))
        .limit(3)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    let tickers: Vec<&str> = page.markets.iter().map(|m| m.ticker.as_str()).collect();
    let rows: Vec<&str> = sent["markets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["ticker"].as_str().unwrap())
        .collect();
    assert_eq!(tickers, rows);
}

#[tokio::test]
async fn an_archived_market_is_read_by_ticker() {
    let ticker = archived();
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        &format!("/trade-api/v2/historical/markets/{ticker}"),
        "",
        &fixture("historical/market"),
    )
    .await;
    let read = client(&server)
        .historical()
        .market(&ticker)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    assert_eq!(read.market.ticker, ticker.as_str());
    assert!(read.market.settlement_ts.is_some(), "an archived market has settled");
}

#[tokio::test]
async fn archived_candlesticks_decode_in_their_own_shape() {
    let ticker = archived();
    for (minutes, period) in [
        (1, PeriodInterval::Minute1),
        (60, PeriodInterval::Hour1),
        (1440, PeriodInterval::Day1),
    ] {
        let name = format!("historical/candlesticks_{minutes}");
        let mut server = Server::new_async().await;
        let mock = route(
            &mut server,
            &format!("/trade-api/v2/historical/markets/{ticker}/candlesticks"),
            &format!("start_ts=100&end_ts=200&period_interval={minutes}"),
            &fixture(&name),
        )
        .await;
        let read = client(&server)
            .historical()
            .candlesticks(&ticker, period, 100, 200)
            .send()
            .await
            .unwrap();
        mock.assert_async().await;
        let sent = wire(&name);
        let rows = sent["candlesticks"].as_array().unwrap();
        assert_eq!(read.ticker, ticker.as_str(), "{name}");
        assert_eq!(read.candlesticks.len(), rows.len(), "{name}");
        for (candle, row) in read.candlesticks.iter().zip(rows) {
            assert_eq!(candle.end_period_ts, row["end_period_ts"].as_i64().unwrap());
            assert_eq!(candle.volume.to_string(), row["volume"].as_str().unwrap());
            assert_eq!(
                candle.price.close.map(|p| p.to_string()).as_deref(),
                row["price"]["close"].as_str()
            );
        }
    }
}

#[tokio::test]
async fn archived_trades_decode_the_captured_page() {
    let ticker = archived();
    let name = "historical/trades";
    let mut server = Server::new_async().await;
    let mock = route(
        &mut server,
        "/trade-api/v2/historical/trades",
        &format!("ticker={ticker}&limit=20"),
        &fixture(name),
    )
    .await;
    let page = client(&server)
        .historical()
        .trades()
        .ticker(&ticker)
        .limit(20)
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
    let sent = wire(name);
    let rows = sent["trades"].as_array().unwrap();
    assert_eq!(page.trades.len(), rows.len());
    for (trade, row) in page.trades.iter().zip(rows) {
        assert_eq!(trade.trade_id, row["trade_id"].as_str().unwrap());
        assert_eq!(trade.ticker, ticker.as_str());
    }
}

#[tokio::test]
async fn an_empty_ticker_selection_is_refused_with_nothing_sent() {
    let mut server = Server::new_async().await;
    let nothing = server
        .mock("GET", Matcher::Any)
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let err = client(&server)
        .historical()
        .markets(HistoricalSelection::Tickers(Vec::new()))
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(&err, PredictionsError::Api(ApiError::Validation(reason)) if reason.starts_with("tickers: ")),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::InvalidRequest);
    nothing.assert_async().await;
}
```

- [ ] **Step 2: Run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test mock_predictions_historical`
Expected: 6 passed.

### Task 6.5: spec and wire agreement

**Files:**
- Modify: `polyoxide-kalshi/tests/spec_agreement_predictions.rs` (append)
- Modify: `polyoxide-kalshi/tests/wire_agreement_predictions.rs` (append)

- [ ] **Step 1: Append to `spec_agreement_predictions.rs`**

```rust
use polyoxide_kalshi::predictions::types::{
    HistoricalBidAsk, HistoricalCandlestick, HistoricalCandlesticksResponse, HistoricalCutoff,
    HistoricalPrice, HistoricalSelection,
};

#[test]
fn every_historical_schema_agrees_with_the_spec() {
    let schemas = mirror_schemas();
    check_schema::<HistoricalCutoff>(&schemas, "GetHistoricalCutoffResponse");
    check_schema::<HistoricalBidAsk>(&schemas, "BidAskDistributionHistorical");
    check_schema::<HistoricalPrice>(&schemas, "PriceDistributionHistorical");
    check_schema::<HistoricalCandlestick>(&schemas, "MarketCandlestickHistorical");
    check_schema::<HistoricalCandlesticksResponse>(
        &schemas,
        "GetMarketCandlesticksHistoricalResponse",
    );
    let unaccounted = unaccounted_from(
        &[
            "/historical/cutoff",
            "/historical/markets",
            "/historical/markets/{ticker}",
            "/historical/markets/{ticker}/candlesticks",
            "/historical/trades",
        ],
        &[
            "GetHistoricalCutoffResponse",
            "GetMarketsResponse",
            "GetMarketResponse",
            "Market",
            "PriceRange",
            "GetMarketCandlesticksHistoricalResponse",
            "MarketCandlestickHistorical",
            "BidAskDistributionHistorical",
            "PriceDistributionHistorical",
            "GetTradesResponse",
            "Trade",
        ],
        &["MveSelectedLeg"],
    );
    assert!(
        unaccounted.is_empty(),
        "schemas neither checked nor excused: {unaccounted:?}"
    );
}

#[tokio::test]
async fn the_historical_builders_send_exactly_the_documented_query_keys() {
    let spec = mirror();
    let mut markets = keys_of("/historical/markets", "historical/markets_page", |p| {
        Box::pin(async move {
            p.historical()
                .markets(HistoricalSelection::ExcludeMultivariate)
                .limit(5)
                .cursor("c1")
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    markets.extend(
        keys_of("/historical/markets", "historical/markets_page", |p| {
            Box::pin(async move {
                p.historical()
                    .markets(HistoricalSelection::Tickers(vec![
                        Ticker::new("KXTEST-1").unwrap()
                    ]))
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    markets.extend(
        keys_of("/historical/markets", "historical/markets_page", |p| {
            Box::pin(async move {
                p.historical()
                    .markets(HistoricalSelection::Event("EV-1".to_owned()))
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    markets.extend(
        keys_of("/historical/markets", "historical/markets_page", |p| {
            Box::pin(async move {
                p.historical()
                    .markets(HistoricalSelection::Series("SER-1".to_owned()))
                    .send()
                    .await
                    .map(|_| ())
            })
        })
        .await,
    );
    assert_eq!(
        Some(markets),
        query::documented_parameters(&spec, "/historical/markets")
    );

    let candles = keys_of(
        "/historical/markets/KXTEST-1/candlesticks",
        "historical/candlesticks_60",
        |p| {
            Box::pin(async move {
                p.historical()
                    .candlesticks(
                        &Ticker::new("KXTEST-1").unwrap(),
                        PeriodInterval::Hour1,
                        1,
                        2,
                    )
                    .send()
                    .await
                    .map(|_| ())
            })
        },
    )
    .await;
    assert_eq!(
        Some(candles),
        query::documented_parameters(&spec, "/historical/markets/{ticker}/candlesticks")
    );

    let trades = keys_of("/historical/trades", "historical/trades", |p| {
        Box::pin(async move {
            p.historical()
                .trades()
                .ticker(&Ticker::new("KXTEST-1").unwrap())
                .min_ts(1)
                .max_ts(2)
                .limit(5)
                .cursor("c1")
                .is_block_trade(true)
                .send()
                .await
                .map(|_| ())
        })
    })
    .await;
    assert_eq!(
        Some(trades),
        query::documented_parameters(&spec, "/historical/trades")
    );

    for path in ["/historical/cutoff", "/historical/markets/{ticker}"] {
        assert_eq!(
            query::documented_parameters(&spec, path),
            Some(BTreeSet::new()),
            "{path} documents a query, so its builder needs a setter"
        );
    }
}
```

- [ ] **Step 2: Append to `wire_agreement_predictions.rs`**

```rust
use polyoxide_kalshi::predictions::types::{HistoricalCandlesticksResponse, HistoricalCutoff};

#[test]
fn every_historical_fixture_agrees_with_its_type() {
    hold_exactly::<HistoricalCutoff>("historical/cutoff");
    hold_exactly::<MarketsPage>("historical/markets_page");
    hold_exactly::<MarketResponse>("historical/market");
    for width in [1, 60, 1440] {
        hold_exactly::<HistoricalCandlesticksResponse>(&format!("historical/candlesticks_{width}"));
    }
    hold_exactly::<TradesPage>("historical/trades");
}

#[test]
fn the_historical_fixtures_cover_the_cases_the_fallback_is_built_on() {
    let hourly: HistoricalCandlesticksResponse = read("historical/candlesticks_60");
    assert!(
        hourly.candlesticks.iter().any(|row| row.price.close.is_some()),
        "an hourly period with a trade"
    );
    assert!(
        hourly
            .candlesticks
            .iter()
            .any(|row| row.price.close.is_none() && row.price.previous.is_some()),
        "an hourly period with no trade that carries previous"
    );
    let market: MarketResponse = read("historical/market");
    assert_eq!(hourly.ticker, market.market.ticker);
    let trades: TradesPage = read("historical/trades");
    assert!(!trades.trades.is_empty(), "the archived market traded");
    assert!(trades.trades.iter().all(|trade| trade.ticker == market.market.ticker));
}
```

- [ ] **Step 3: Run both**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test spec_agreement_predictions --test wire_agreement_predictions`
Expected: all pass. If `the_historical_fixtures_cover_the_cases_the_fallback_is_built_on` fails because the archived market's hourly candles hold no untraded period with `previous`, re-run Task 1.3's capture with a different archived market (raise `ARCHIVE_SCAN` or pick the next row) and tell the owner; do not weaken the assertion.

### Task 6.6: Story 6's gates and commit

- [ ] **Step 1: Run the story gates.** Expected: all pass.

- [ ] **Step 2: Commit**

```bash
git add polyoxide-kalshi/src/predictions/types/historical.rs polyoxide-kalshi/src/predictions/api/historical.rs polyoxide-kalshi/tests/mock_predictions_historical.rs
git commit -m "feat(kalshi): the historical namespace (#71, Story 6)" -- polyoxide-kalshi/src/predictions/types/historical.rs polyoxide-kalshi/src/predictions/types/mod.rs polyoxide-kalshi/src/predictions/api/historical.rs polyoxide-kalshi/src/predictions/api/mod.rs polyoxide-kalshi/src/predictions/api/markets.rs polyoxide-kalshi/src/predictions/mod.rs polyoxide-kalshi/tests/query_setters_predictions.rs polyoxide-kalshi/tests/mock_predictions_historical.rs polyoxide-kalshi/tests/spec_agreement_predictions.rs polyoxide-kalshi/tests/wire_agreement_predictions.rs
```

---

## Story 7: the trait fallback (spec §10; D13, D16)

### Task 7.1: the fallback tests (failing)

**Files:**
- Create: `polyoxide-kalshi/tests/venue_predictions_historical.rs`

- [ ] **Step 1: Write the suite**

```rust
//! Kalshi's venue traits for a market archived past the historical cutoff (issue #71, D13): each
//! reads live first and the archive only when the live answer says the market is not there, on
//! mock servers serving the captured archive.

use std::sync::Arc;

use mockito::{Matcher, Mock, Server, ServerGuard};
use polyoxide_core::RetryConfig;
use polyoxide_kalshi::predictions::{
    key::{self, Outcome, Ticker},
    types::HistoricalCandlesticksResponse,
    venue::InstrumentExt,
    Predictions,
};
use polyoxide_test_support::fixtures;
use polyoxide_venue::{
    CandleInterval, CandleQuery, Candles, Class, ClassifiedError, DynCandles, DynEventGroups,
    DynMarketData, DynTrades, EventGroups, EventOutcome, MarketData, MarketKey, NotListed,
    TradeId, Trades, UnixMillis,
};
use serde_json::{json, Value};

const NOT_FOUND: &str = r#"{"error":{"code":"not_found","message":"not found"}}"#;
const UNAVAILABLE: &str = r#"{"error":{"code":"service_unavailable","message":"try later"}}"#;
const CANDLES: &str = "/trade-api/v2/markets/candlesticks";
const TRADES: &str = "/trade-api/v2/markets/trades";
const ARCHIVED_TRADES: &str = "/trade-api/v2/historical/trades";
const ARCHIVED_MARKETS: &str = "/trade-api/v2/historical/markets";

fn fixture(name: &str) -> String {
    fixtures!("predictions").text(name)
}

fn fixture_json(name: &str) -> Value {
    fixtures!("predictions").json(name)
}

fn client(server: &ServerGuard) -> Predictions {
    Predictions::builder()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 0,
            ..RetryConfig::default()
        })
        .build()
        .unwrap()
}

/// A server with the account's `endpoint_costs` mounted and never asserted, which a client reads
/// on its first send.
async fn start() -> ServerGuard {
    let mut server = Server::new_async().await;
    let _costs = server
        .mock("GET", "/trade-api/v2/account/endpoint_costs")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fixture("endpoint_costs"))
        .create_async()
        .await;
    server
}

/// `path` answering `status` with `body` `hits` times, whatever its query.
async fn answer(server: &mut ServerGuard, path: &str, status: usize, body: &str, hits: usize) -> Mock {
    server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(hits)
        .create_async()
        .await
}

/// `path` answering 200 with `body` once, for exactly `query`.
async fn exact(server: &mut ServerGuard, path: &str, query: String, body: &str) -> Mock {
    server
        .mock("GET", path)
        .match_query(Matcher::Exact(query))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(1)
        .create_async()
        .await
}

/// The archived market the capture holds: its ticker and its row.
fn archived() -> (Ticker, Value) {
    let body = fixture_json("historical/market");
    let ticker = Ticker::new(body["market"]["ticker"].as_str().unwrap()).unwrap();
    (ticker, body["market"].clone())
}

/// The live market the capture holds.
fn live() -> Ticker {
    Ticker::new(fixture_json("market")["market"]["ticker"].as_str().unwrap()).unwrap()
}

fn market_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/markets/{ticker}")
}

fn archived_market_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/historical/markets/{ticker}")
}

fn archived_candles_path(ticker: &Ticker) -> String {
    format!("/trade-api/v2/historical/markets/{ticker}/candlesticks")
}

fn ms(seconds: i64) -> u64 {
    u64::try_from(seconds).unwrap() * 1000
}

fn assert_not_listed(err: &ClassifiedError, key: &MarketKey) {
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, *key);
}

// ── instrument ──────────────────────────────────────────────────

#[tokio::test]
async fn an_archived_market_is_read_from_the_archive_when_the_live_route_answers_404() {
    let (ticker, row) = archived();
    let mut server = start().await;
    let live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let archive = answer(
        &mut server,
        &archived_market_path(&ticker),
        200,
        &fixture("historical/market"),
        1,
    )
    .await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let instrument = venue
        .instrument(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;
    assert_eq!(instrument.tradable, Some(false), "an archived market does not trade");
    let ext = instrument.extensions.get::<InstrumentExt>().unwrap();
    assert_eq!(ext.event_ticker.as_deref(), row["event_ticker"].as_str());
}

#[tokio::test]
async fn a_ticker_neither_route_lists_is_not_listed() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let archive = answer(&mut server, &archived_market_path(&ticker), 404, NOT_FOUND, 1).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let key = key::key(&ticker, Outcome::No);
    let err = venue.instrument(&key).await.unwrap_err();
    archive.assert_async().await;
    assert_not_listed(&err, &key);
}

#[tokio::test]
async fn an_archive_that_fails_otherwise_is_that_failure() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let _archive = answer(&mut server, &archived_market_path(&ticker), 503, UNAVAILABLE, 1).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    let err = venue
        .instrument(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap_err();
    assert_eq!(err.class, Class::Unavailable);
    assert!(err.source.downcast_ref::<NotListed>().is_none(), "{err}");
}

#[tokio::test]
async fn a_live_market_never_asks_the_archive() {
    let ticker = live();
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 200, &fixture("market"), 1).await;
    let archive = answer(&mut server, &archived_market_path(&ticker), 404, NOT_FOUND, 0).await;
    let venue: Arc<DynMarketData<'static>> = DynMarketData::new_arc(client(&server));
    venue
        .instrument(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    archive.assert_async().await;
}

// ── candles ─────────────────────────────────────────────────────

/// The captured hourly archive, and the span of open times that covers every row: the first row
/// opens an hour before it ends, and the last row opens an hour before it ends.
fn hourly() -> (String, HistoricalCandlesticksResponse, i64, i64) {
    let body = fixture("historical/candlesticks_60");
    let rows: HistoricalCandlesticksResponse = serde_json::from_str(&body).unwrap();
    let first = rows.candlesticks.iter().map(|row| row.end_period_ts).min().unwrap();
    let last = rows.candlesticks.iter().map(|row| row.end_period_ts).max().unwrap();
    (body, rows, first - 3_600, last - 3_600)
}

#[tokio::test]
async fn a_market_the_live_answer_leaves_out_is_read_from_the_archive_with_the_same_span() {
    let (ticker, _) = archived();
    let (body, rows, first_open, last_open) = hourly();
    // Kalshi selects a candle by its end and the trait by its open, so both requests ask for the
    // span moved on by one width.
    let (start_ts, end_ts) = (first_open + 3_600, last_open + 3_600);
    let mut server = start().await;
    let live = exact(
        &mut server,
        CANDLES,
        format!("market_tickers={ticker}&start_ts={start_ts}&end_ts={end_ts}&period_interval=60"),
        r#"{"markets":[]}"#,
    )
    .await;
    let archive = exact(
        &mut server,
        &archived_candles_path(&ticker),
        format!("start_ts={start_ts}&end_ts={end_ts}&period_interval=60"),
        &body,
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(first_open)))
        .end(UnixMillis(ms(last_open)));
    let candles = venue
        .candles(&key::key(&ticker, Outcome::Yes), &query)
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;

    // A row with a trade or a previous price is a candle opening one hour before it ends.
    let mut kept: Vec<_> = rows
        .candlesticks
        .iter()
        .filter(|row| row.price.close.is_some() || row.price.previous.is_some())
        .collect();
    kept.sort_by_key(|row| row.end_period_ts);
    assert!(!kept.is_empty(), "the capture holds a candle");
    let opens: Vec<u64> = candles.iter().map(|candle| candle.open_time.0).collect();
    let expected: Vec<u64> = kept.iter().map(|row| ms(row.end_period_ts - 3_600)).collect();
    assert_eq!(opens, expected);
    for (candle, row) in candles.iter().zip(&kept) {
        assert_eq!(candle.close, row.price.close.or(row.price.previous).unwrap());
        assert_eq!(candle.volume, Some(row.volume));
        assert_eq!(candle.open.is_some(), row.price.close.is_some());
    }
}

#[tokio::test]
async fn an_archived_span_past_5_000_periods_is_clamped_to_its_first_5_000() {
    let (ticker, _) = archived();
    // 9,001 one-minute periods: within the live route's 10,000, past the archive's 5,000.
    let first_open: i64 = 1_799_971_200;
    let last_open = first_open + 9_000 * 60;
    let start_ts = first_open + 60;
    let mut server = start().await;
    let live = exact(
        &mut server,
        CANDLES,
        format!(
            "market_tickers={ticker}&start_ts={start_ts}&end_ts={}&period_interval=1",
            last_open + 60
        ),
        r#"{"markets":[]}"#,
    )
    .await;
    let archive = exact(
        &mut server,
        &archived_candles_path(&ticker),
        format!(
            "start_ts={start_ts}&end_ts={}&period_interval=1",
            start_ts + 4_999 * 60
        ),
        &json!({"ticker": ticker.as_str(), "candlesticks": []}).to_string(),
    )
    .await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let query = CandleQuery::new(CandleInterval::Minute1, UnixMillis(ms(first_open)))
        .end(UnixMillis(ms(last_open)));
    let candles = venue
        .candles(&key::key(&ticker, Outcome::Yes), &query)
        .await
        .unwrap();
    live.assert_async().await;
    archive.assert_async().await;
    assert!(candles.is_empty());
}

#[tokio::test]
async fn an_answer_that_lists_only_another_market_asks_the_archive_and_its_404_is_not_listed() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let mut server = start().await;
    let other = json!({"markets": [{"market_ticker": "KXOTHER-1", "candlesticks": []}]}).to_string();
    let _live = answer(&mut server, CANDLES, 200, &other, 1).await;
    let archive = answer(&mut server, &archived_candles_path(&ticker), 404, NOT_FOUND, 1).await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let key = key::key(&ticker, Outcome::Yes);
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)).end(UnixMillis(ms(3_600)));
    let err = venue.candles(&key, &query).await.unwrap_err();
    archive.assert_async().await;
    assert_not_listed(&err, &key);
}

#[tokio::test]
async fn a_listed_market_with_no_candle_in_the_span_never_asks_the_archive() {
    let ticker = live();
    let mut server = start().await;
    let empty = json!({"markets": [{"market_ticker": ticker.as_str(), "candlesticks": []}]}).to_string();
    let _live = answer(&mut server, CANDLES, 200, &empty, 1).await;
    let archive = answer(&mut server, &archived_candles_path(&ticker), 404, NOT_FOUND, 0).await;
    let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(0)).end(UnixMillis(ms(3_600)));
    let candles = venue
        .candles(&key::key(&ticker, Outcome::Yes), &query)
        .await
        .unwrap();
    assert!(candles.is_empty());
    archive.assert_async().await;
}

// ── trades ──────────────────────────────────────────────────────

const NO_TRADES: &str = r#"{"cursor":"","trades":[]}"#;

#[tokio::test]
async fn trades_the_live_route_does_not_have_are_read_from_the_archive_oldest_first() {
    let (ticker, _) = archived();
    let mut server = start().await;
    let live = exact(
        &mut server,
        TRADES,
        format!("ticker={ticker}&limit=1000"),
        NO_TRADES,
    )
    .await;
    let archive = exact(
        &mut server,
        ARCHIVED_TRADES,
        format!("ticker={ticker}&limit=1000"),
        &fixture("historical/trades"),
    )
    .await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let trades = venue.trades(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    live.assert_async().await;
    archive.assert_async().await;

    let newest_first: Vec<TradeId> = fixture_json("historical/trades")["trades"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| TradeId::new(row["trade_id"].as_str().unwrap()))
        .collect();
    let ids: Vec<TradeId> = trades.iter().filter_map(|trade| trade.id.clone()).collect();
    assert_eq!(ids, newest_first.into_iter().rev().collect::<Vec<_>>());
}

#[tokio::test]
async fn a_market_with_live_trades_never_asks_the_archive() {
    let ticker = live();
    let mut server = start().await;
    let _live = answer(&mut server, TRADES, 200, &fixture("trades"), 1).await;
    let archive = answer(&mut server, ARCHIVED_TRADES, 200, NO_TRADES, 0).await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let trades = venue.trades(&key::key(&ticker, Outcome::Yes)).await.unwrap();
    assert!(!trades.is_empty());
    archive.assert_async().await;
}

#[tokio::test]
async fn an_archive_with_no_trades_is_no_trades_and_its_404_is_not_listed() {
    let ticker = Ticker::new("KXNOSUCH-1").unwrap();
    let key = key::key(&ticker, Outcome::Yes);

    let mut server = start().await;
    let _live = answer(&mut server, TRADES, 200, NO_TRADES, 1).await;
    let _archive = answer(&mut server, ARCHIVED_TRADES, 200, NO_TRADES, 1).await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    assert!(venue.trades(&key).await.unwrap().is_empty());

    let mut server = start().await;
    let _live = answer(&mut server, TRADES, 200, NO_TRADES, 1).await;
    let _archive = answer(&mut server, ARCHIVED_TRADES, 404, NOT_FOUND, 1).await;
    let venue: Arc<DynTrades<'static>> = DynTrades::new_arc(client(&server));
    let err = venue.trades(&key).await.unwrap_err();
    assert_not_listed(&err, &key);
}

// ── event groups ────────────────────────────────────────────────

/// The captured event, renamed `event_ticker`, nesting `markets`.
fn event_body(event_ticker: &str, markets: Vec<Value>) -> String {
    let mut body = fixture_json("event");
    body["event"]["event_ticker"] = json!(event_ticker);
    body["event"]["markets"] = json!(markets);
    body.to_string()
}

/// The archived row, renamed `ticker`: a sibling market of the same event.
fn sibling(row: &Value, ticker: &str) -> Value {
    let mut sibling = row.clone();
    sibling["ticker"] = json!(ticker);
    sibling
}

/// The tickers of a group's outcomes, one per market, in order.
fn group_tickers(outcomes: &[EventOutcome]) -> Vec<String> {
    outcomes
        .chunks(2)
        .map(|pair| pair[0].market.clone())
        .collect()
}

#[tokio::test]
async fn an_archived_markets_group_is_read_from_the_archive() {
    let (ticker, row) = archived();
    let event = row["event_ticker"].as_str().unwrap().to_owned();
    let other = format!("{ticker}-B");
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let _archive = answer(&mut server, &archived_market_path(&ticker), 200, &fixture("historical/market"), 1).await;
    let _event = answer(
        &mut server,
        &format!("/trade-api/v2/events/{event}"),
        200,
        &event_body(&event, vec![]),
        1,
    )
    .await;
    let siblings = exact(
        &mut server,
        ARCHIVED_MARKETS,
        format!("event_ticker={event}&limit=1000"),
        &json!({"cursor": "", "markets": [row.clone(), sibling(&row, &other)]}).to_string(),
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue
        .event_group(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    siblings.assert_async().await;
    assert_eq!(group.id, event);
    assert_eq!(group_tickers(&group.outcomes), [ticker.as_str().to_owned(), other]);
}

#[tokio::test]
async fn a_partly_archived_event_lists_each_market_once_live_ones_first() {
    let (ticker, row) = archived();
    let event = row["event_ticker"].as_str().unwrap().to_owned();
    let other = format!("{ticker}-B");
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 404, NOT_FOUND, 1).await;
    let _archive = answer(&mut server, &archived_market_path(&ticker), 200, &fixture("historical/market"), 1).await;
    let _event = answer(
        &mut server,
        &format!("/trade-api/v2/events/{event}"),
        200,
        &event_body(&event, vec![sibling(&row, &other)]),
        1,
    )
    .await;
    let _siblings = answer(
        &mut server,
        ARCHIVED_MARKETS,
        200,
        &json!({"cursor": "", "markets": [row.clone(), sibling(&row, &other)]}).to_string(),
        1,
    )
    .await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue
        .event_group(&key::key(&ticker, Outcome::No))
        .await
        .unwrap();
    assert_eq!(group_tickers(&group.outcomes), [other, ticker.as_str().to_owned()]);
}

#[tokio::test]
async fn an_event_that_nests_the_market_never_asks_the_archive() {
    let ticker = live();
    let market = fixture_json("market");
    let event = market["market"]["event_ticker"].as_str().unwrap().to_owned();
    let mut server = start().await;
    let _live = answer(&mut server, &market_path(&ticker), 200, &fixture("market"), 1).await;
    let _event = answer(
        &mut server,
        &format!("/trade-api/v2/events/{event}"),
        200,
        &event_body(&event, vec![market["market"].clone()]),
        1,
    )
    .await;
    let archive = answer(&mut server, ARCHIVED_MARKETS, 200, r#"{"cursor":"","markets":[]}"#, 0).await;
    let venue: Arc<DynEventGroups<'static>> = DynEventGroups::new_arc(client(&server));
    let group = venue
        .event_group(&key::key(&ticker, Outcome::Yes))
        .await
        .unwrap();
    assert_eq!(group_tickers(&group.outcomes), [ticker.as_str().to_owned()]);
    archive.assert_async().await;
}
```

- [ ] **Step 2: Run to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test venue_predictions_historical`
Expected: it compiles. Every test that expects the archive to be asked FAILS, since today nothing asks it (`instrument` is `NotListed`, `candles` and `trades` are empty, `event_group` lacks the archived markets), and so does `an_archive_that_fails_otherwise_is_that_failure` (today a live `404` is `NotListed` at once). The four "never asks the archive" tests PASS.

### Task 7.2: the fallback

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/venue.rs`

- [ ] **Step 1: Imports.** Replace

```rust
use super::types::{
    self, BidAskDistribution, MarketCandlestick, MarketStatus, OrderbookFp, OutcomeSide,
    PeriodInterval, PriceRange,
};
```

with

```rust
use super::types::{
    self, BidAskDistribution, HistoricalCandlestick, HistoricalSelection, MarketCandlestick,
    MarketStatus, OrderbookFp, OutcomeSide, PeriodInterval, PriceDistribution, PriceRange,
};
```

- [ ] **Step 2: `NotListed` from a 404.** After `fn is_not_found`, add:

```rust
/// `err` as the traits return it: `NotListed` for `key` when Kalshi answered `404`.
fn unless_not_found(key: &MarketKey, err: PredictionsError) -> ClassifiedError {
    if is_not_found(&err) {
        NotListed::new(key.clone()).into()
    } else {
        err.into()
    }
}
```

- [ ] **Step 3: `listed` reads the archive.** Replace `impl Predictions { async fn listed … }` with:

```rust
impl Predictions {
    /// The outcome a key names and its market's row, read live and, when the live route answers
    /// `404`, from the archive; `NotListed` when both answer `404` (D11, issue #71's D13).
    async fn listed(&self, key: &MarketKey) -> Result<(OutcomeId, types::Market), ClassifiedError> {
        let id = key::outcome(key)?;
        match self.markets().get(&id.ticker).send().await {
            Ok(response) => return Ok((id, response.market)),
            Err(err) if is_not_found(&err) => {}
            Err(err) => return Err(err.into()),
        }
        let response = self
            .historical()
            .market(&id.ticker)
            .send()
            .await
            .map_err(|err| unless_not_found(key, err))?;
        Ok((id, response.market))
    }
}
```

- [ ] **Step 4: `trades` reads the archive.** Replace `Trades::trades`' doc comment and body with:

```rust
    /// The last 1000 trades of the market, oldest first. Kalshi serves them newest first, with a
    /// cursor this method does not follow. When the live route has none, they are read from the
    /// archive, which holds a market settled before the historical cutoff; its `404` is
    /// `NotListed`.
    async fn trades(&self, key: &MarketKey) -> Result<Vec<Trade>, ClassifiedError> {
        let id = key::outcome(key)?;
        let mut rows = self
            .markets()
            .trades()
            .ticker(&id.ticker)
            .limit(1000)
            .send()
            .await?
            .trades;
        if rows.is_empty() {
            rows = self
                .historical()
                .trades()
                .ticker(&id.ticker)
                .limit(1000)
                .send()
                .await
                .map_err(|err| unless_not_found(key, err))?
                .trades;
        }
        rows.iter()
            .rev()
            .map(|row| trade(key, id.outcome, row))
            .collect()
    }
```

- [ ] **Step 5: `candles` reads the archive.** After `fn ohlc`, add:

```rust
/// An archived candlestick in the live route's shape, so one mapping reads both.
fn live_shape(row: &HistoricalCandlestick) -> MarketCandlestick {
    let offers = |side: &types::HistoricalBidAsk| BidAskDistribution {
        open_dollars: side.open,
        low_dollars: side.low,
        high_dollars: side.high,
        close_dollars: side.close,
    };
    MarketCandlestick {
        end_period_ts: row.end_period_ts,
        yes_bid: offers(&row.yes_bid),
        yes_ask: offers(&row.yes_ask),
        price: PriceDistribution {
            open_dollars: row.price.open,
            low_dollars: row.price.low,
            high_dollars: row.price.high,
            close_dollars: row.price.close,
            mean_dollars: row.price.mean,
            previous_dollars: row.price.previous,
            min_dollars: None,
            max_dollars: None,
        },
        volume_fp: row.volume,
        open_interest_fp: row.open_interest,
    }
}
```

After `const MAX_PERIODS_PER_REQUEST`, add:

```rust
/// The most candlesticks the archive answers in one request: it refuses more with a `400` ("max
/// candlesticks: 5000"), half the live route's cap (`docs/specs/kalshi/OBSERVED.md`, issue #71).
const MAX_ARCHIVED_PERIODS_PER_REQUEST: i64 = 5_000;
```

In `Candles::candles`, replace

```rust
        let Some(market) = response
            .markets
            .into_iter()
            .find(|market| market.market_ticker == id.ticker.as_str())
        else {
            return Ok(Vec::new());
        };
        let mut candles = Vec::new();
        for row in &market.candlesticks {
```

with

```rust
        let rows = match response
            .markets
            .into_iter()
            .find(|market| market.market_ticker == id.ticker.as_str())
        {
            Some(market) => market.candlesticks,
            // Kalshi lists a live market with no candle in the span with an empty list, and
            // leaves out a market it has archived, whose candles the archive serves.
            None => self
                .historical()
                .candlesticks(
                    &id.ticker,
                    period,
                    start_ts,
                    end_ts.min(
                        start_ts.saturating_add(
                            MAX_ARCHIVED_PERIODS_PER_REQUEST.saturating_sub(1) * width,
                        ),
                    ),
                )
                .send()
                .await
                .map_err(|err| unless_not_found(key, err))?
                .candlesticks
                .iter()
                .map(live_shape)
                .collect(),
        };
        let mut candles = Vec::new();
        for row in &rows {
```

and in its doc comment, after "and the rest is not read." add " A market the answer leaves out is read from the archive, which answers at most 5,000 candlesticks, so its span is clamped to its first 5,000 periods; its `404` is `NotListed`."

- [ ] **Step 6: `event_group` reads the archive.** Replace `EventGroups::event_group`'s doc comment and its first lines, from `async fn event_group` through `let mut group = EventGroup::new(event.event_ticker.clone());`, and the loop header `for nested in &event.markets {`, so that the method reads:

```rust
    /// The event of the key's market, with both outcomes of each of its markets.
    ///
    /// The event is read with its markets nested, and Kalshi nests only the live ones, so when the
    /// key's market is not among them (it settled before the historical cutoff) the event's
    /// archived markets are read too, one page of 1000: the live markets first, then each archived
    /// one not already listed.
    async fn event_group(&self, key: &MarketKey) -> Result<EventGroup, ClassifiedError> {
        let (id, market) = self.listed(key).await?;
        let response = self
            .events()
            .get(&market.event_ticker)
            .with_nested_markets(true)
            .send()
            .await?;
        let event = response.event;
        let nests = |ticker: &str| event.markets.iter().any(|nested| nested.ticker == ticker);
        let archived = if nests(id.ticker.as_str()) {
            Vec::new()
        } else {
            self.historical()
                .markets(HistoricalSelection::Event(event.event_ticker.clone()))
                .limit(1000)
                .send()
                .await?
                .markets
        };
        let mut group = EventGroup::new(event.event_ticker.clone());
        group.title = Some(event.title.clone());
        let markets = event
            .markets
            .iter()
            .chain(archived.iter().filter(|row| !nests(row.ticker.as_str())));
        for nested in markets {
```

(The loop's body and the rest of the method stay as they are.)

- [ ] **Step 7: Run the new suite**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test venue_predictions_historical`
Expected: all pass.

### Task 7.3: the module docs

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/venue.rs` (module doc comment)

- [ ] **Step 1: Replace the archive sentence.** In the `//!` block, replace

```rust
//! interest in YES terms whatever the key's outcome, and the mean trade price,
//! turned over for a `no` key like the other prices. A market that settled before
//! Kalshi's historical cutoff has candlesticks only on its historical route, which
//! this client does not call: the live route's answer for it, an empty list or a
//! refusal, is returned as the venue gave it.
```

with

```rust
//! interest in YES terms whatever the key's outcome, and the mean trade price,
//! turned over for a `no` key like the other prices.
//!
//! A market that settled before Kalshi's historical cutoff is gone from the live
//! routes, which answer it as an unknown ticker, and is read from the archive
//! (`historical()`) instead. `instrument` reads `GET /historical/markets/{ticker}`
//! when the live market route answers `404`; `candles` reads the archived
//! candlesticks with the same span when the batch answer leaves the market out (it
//! lists a live market with no candle in the span with an empty list), clamped
//! to its first 5,000 periods, the most the archive answers; `trades`
//! reads `GET /historical/trades` when the live route has none; and `event_group`
//! reads the event's archived markets when the live event does not nest the key's
//! market. A `404` from the archive is `NotListed`. No method reads the cutoff: the
//! live answer decides, so a market the live routes still serve near the cutoff is
//! read live. `quote` and `book` read only the live book, which is empty for an
//! archived market.
```

- [ ] **Step 2: Replace the request table** (the `//! | Method | Route | Requests |` table) with:

```rust
//! | Method | Route | Requests |
//! | --- | --- | --- |
//! | `instrument` | `GET /trade-api/v2/markets/{ticker}`, and for an archived market `GET /trade-api/v2/historical/markets/{ticker}` | 1, or 2 |
//! | `quote` | `GET /trade-api/v2/markets/{ticker}/orderbook`, `depth=1` | 1 |
//! | `book` | `GET /trade-api/v2/markets/{ticker}/orderbook`, every level | 1 |
//! | `trades` | `GET /trade-api/v2/markets/trades` for the ticker, 1000 rows, and when it has none `GET /trade-api/v2/historical/trades` | 1, or 2 |
//! | `candles` | `GET /trade-api/v2/markets/candlesticks` for the ticker, and when it leaves the market out `GET /trade-api/v2/historical/markets/{ticker}/candlesticks` | 1, or 2 |
//! | `event_group` | `instrument`'s reads, then `GET /trade-api/v2/events/{event}` with `with_nested_markets=true`, and for an archived market `GET /trade-api/v2/historical/markets?event_ticker=` | 2 to 4 |
```

- [ ] **Step 3: Check the docs build**

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-kalshi --all-features'`
Expected: no warning.

### Task 7.4: D16's four venue edits (owner's exception)

**Files:**
- Modify: `polyoxide-kalshi/tests/venue_predictions.rs`

- [ ] **Step 1: Run the suite to see the four fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test venue_predictions`
Expected: FAIL in exactly `an_unknown_ticker_is_not_listed_but_its_quote_and_book_are_empty`, `only_a_404_is_not_listed`, `an_unlisted_key_has_no_event_group_and_asks_for_no_event` and `a_market_the_answer_does_not_list_has_no_candles` (each now asks the archive, which the mock server does not serve). Any other failure goes to the owner.

- [ ] **Step 2: `an_unknown_ticker_is_not_listed_but_its_quote_and_book_are_empty`.** After the `market` mock's `.await;`, add:

```rust
    // The live route's 404 sends the read to the archive, which does not list it either.
    let archive = server
        .mock("GET", format!("/trade-api/v2/historical/markets/{ticker}").as_str())
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .expect(2)
        .create_async()
        .await;
```

and after `market.assert_async().await;` add `archive.assert_async().await;`.

- [ ] **Step 3: `only_a_404_is_not_listed`.** After the `_plain` mock's `.await;`, add:

```rust
    let _archive = server
        .mock("GET", format!("/trade-api/v2/historical/markets/{ticker}").as_str())
        .with_status(404)
        .with_body("not found")
        .create_async()
        .await;
```

- [ ] **Step 4: `an_unlisted_key_has_no_event_group_and_asks_for_no_event`.** After the `_market` mock's `.await;`, add:

```rust
    let _archive = server
        .mock("GET", format!("/trade-api/v2/historical/markets/{ticker}").as_str())
        .with_status(404)
        .with_body(fixture("error_not_found"))
        .create_async()
        .await;
```

- [ ] **Step 5: `a_market_the_answer_does_not_list_has_no_candles`.** Replace the whole test with:

```rust
#[tokio::test]
async fn a_market_the_answer_does_not_list_is_asked_of_the_archive() {
    let ticker = test_ticker();
    let query = || {
        CandleQuery::new(CandleInterval::Hour1, UnixMillis(ms(T0))).end(UnixMillis(ms(T0 + 3_600)))
    };
    let other = json!({"markets": [{"market_ticker": "KXOTHER-1", "candlesticks": [
        row(T0 + 3_600, traded())
    ]}]})
    .to_string();
    // Neither an empty answer nor one with only another market lists this one, so each asks the
    // archive for the same span, whose 404 is `NotListed`.
    for body in [json!({"markets": []}).to_string(), other] {
        let mut server = start().await;
        let live = route(
            &mut server,
            CANDLES,
            candles_query(&ticker, T0 + 3_600, T0 + 7_200, 60),
            &body,
            1,
        )
        .await;
        let archive = server
            .mock(
                "GET",
                format!("/trade-api/v2/historical/markets/{ticker}/candlesticks").as_str(),
            )
            .match_query(Matcher::Exact(format!(
                "start_ts={}&end_ts={}&period_interval=60",
                T0 + 3_600,
                T0 + 7_200
            )))
            .with_status(404)
            .with_body(fixture("error_not_found"))
            .expect(1)
            .create_async()
            .await;
        let venue: Arc<DynCandles<'static>> = DynCandles::new_arc(client(&server));
        let key = key::key(&ticker, Outcome::Yes);
        let err = venue.candles(&key, &query()).await.unwrap_err();
        live.assert_async().await;
        archive.assert_async().await;
        assert_eq!(err.class, Class::VenueRefusal { code: None });
        assert_eq!(err.source.downcast_ref::<NotListed>().unwrap().key, key);
    }
}
```

- [ ] **Step 6: Run the suite**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test venue_predictions`
Expected: all pass.

### Task 7.5: Story 7's gates and commit

- [ ] **Step 1: Run the story gates.** Expected: all pass. Also run the umbrella's venue-neutral suite, which reads Kalshi through the traits: `nix develop --command cargo test -p polyoxide --all-features --test venue_neutral`. Expected: pass.

- [ ] **Step 2: Commit**

```bash
git add polyoxide-kalshi/tests/venue_predictions_historical.rs
git commit -m "feat(kalshi): the venue traits read archived markets from the archive (#71, Story 7)" -- polyoxide-kalshi/src/predictions/venue.rs polyoxide-kalshi/tests/venue_predictions_historical.rs polyoxide-kalshi/tests/venue_predictions.rs
```

---

## Story 8: live suites and records (spec §11; D14, D15)

### Task 8.1: the live catalog tests

**Files:**
- Modify: `polyoxide-kalshi/tests/live_predictions.rs` (append; its header comment)

- [ ] **Step 1: Append**

```rust
// ── Catalog routes (issue #71) ──────────────────────────────────

use polyoxide_kalshi::predictions::types::{
    EventStatusFilter, EventsPage, HistoricalCandlesticksResponse, HistoricalCutoff,
    HistoricalSelection, MarketStatusFilter, MveFilter, OrderbooksResponse, SeriesList,
    SeriesResponse,
};

/// A daily series with years of settled markets, so its archive always holds some.
const ARCHIVE_SERIES: &str = "KXHIGHNY";

fn now_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

/// An RFC 3339 time the host sent, in Unix seconds; one that does not parse fails the test as the
/// decode failure it is.
fn seconds(field: &str, text: &str) -> i64 {
    match chrono::DateTime::parse_from_rfc3339(text) {
        Ok(time) => time.timestamp(),
        Err(_) => fail(
            field,
            &ApiError::Validation(format!("{field} {text:?} is not RFC 3339")),
        ),
    }
}

/// Three open single markets' tickers, from the filtered listing.
async fn three_open_tickers(client: &Predictions) -> Vec<Ticker> {
    let page = client
        .markets()
        .list()
        .status(MarketStatusFilter::Open)
        .mve_filter(MveFilter::Exclude)
        .limit(3)
        .send()
        .await
        .or_fail("the open listing");
    if page.markets.len() < 3 {
        environmental("the open listing held fewer than three markets");
    }
    page.markets
        .iter()
        .map(|market| Ticker::new(&market.ticker).or_fail("name a listed market"))
        .collect()
}

#[tokio::test]
#[ignore]
async fn live_updated_lists_only_markets_updated_since_its_bound() {
    let bound = now_seconds() - 86_400;
    let page = client()
        .markets()
        .updated(bound)
        .limit(100)
        .send()
        .await
        .or_fail("markets updated in the last day");
    if page.markets.is_empty() {
        environmental("no market's metadata changed in the last day");
    }
    for market in &page.markets {
        assert!(
            seconds("updated_time", &market.updated_time) >= bound,
            "{} was updated at {}, before the bound {bound}",
            market.ticker,
            market.updated_time
        );
    }
}

#[tokio::test]
#[ignore]
async fn live_tickers_answer_exactly_the_markets_asked_for() {
    let client = client();
    let asked = three_open_tickers(&client).await;
    let mut wanted: Vec<&str> = asked.iter().map(Ticker::as_str).collect();
    wanted.sort_unstable();

    let page = client
        .markets()
        .list()
        .tickers(&asked)
        .send()
        .await
        .or_fail("the named markets");
    let mut listed: Vec<&str> = page.markets.iter().map(|m| m.ticker.as_str()).collect();
    listed.sort_unstable();
    assert_eq!(listed, wanted, "the listing answers the set it was asked for");

    let books = client
        .markets()
        .orderbooks(&asked)
        .send()
        .await
        .or_fail("the named markets' books");
    let mut booked: Vec<&str> = books.orderbooks.iter().map(|b| b.ticker.as_str()).collect();
    booked.sort_unstable();
    assert_eq!(booked, wanted, "one book per ticker asked for");
}

#[tokio::test]
#[ignore]
async fn live_series_and_the_events_listing_read() {
    let client = client();
    let list = client
        .series()
        .list()
        .category("Economics")
        .send()
        .await
        .or_fail("the Economics series");
    let Some(first) = list.series.first() else {
        environmental("Kalshi lists no Economics series");
    };
    let one = client
        .series()
        .get(&first.ticker)
        .include_volume(true)
        .send()
        .await
        .or_fail("one series");
    assert_eq!(one.series.ticker, first.ticker);
    assert!(one.series.volume_fp.is_some(), "the volume was asked for");

    let page = client
        .events()
        .list()
        .status(EventStatusFilter::Open)
        .with_nested_markets(true)
        .limit(5)
        .send()
        .await
        .or_fail("the open events");
    if page.events.is_empty() {
        environmental("Kalshi lists no open event");
    }
}

#[tokio::test]
#[ignore]
async fn live_the_cutoff_parses_and_an_archived_market_reads_back() {
    let client = client();
    let cutoff = client.historical().cutoff().send().await.or_fail("the cutoff");
    for (name, time) in [
        ("market_settled_ts", &cutoff.market_settled_ts),
        ("trades_created_ts", &cutoff.trades_created_ts),
        ("orders_updated_ts", &cutoff.orders_updated_ts),
    ] {
        assert!(seconds(name, time) > 0, "{name} {time} is before 1970");
    }
    let page = client
        .historical()
        .markets(HistoricalSelection::Series(ARCHIVE_SERIES.to_owned()))
        .limit(5)
        .send()
        .await
        .or_fail("archived markets");
    let Some(first) = page.markets.first() else {
        environmental(&format!("the archive holds no {ARCHIVE_SERIES} market"));
    };
    let ticker = Ticker::new(&first.ticker).or_fail("name an archived market");
    let read = client
        .historical()
        .market(&ticker)
        .send()
        .await
        .or_fail("an archived market");
    assert_eq!(read.market.ticker, first.ticker);
}

#[tokio::test]
#[ignore]
async fn live_catalog_responses_carry_no_unmodelled_keys() {
    let client = client();
    let asked = three_open_tickers(&client).await;
    let archived = client
        .historical()
        .markets(HistoricalSelection::Series(ARCHIVE_SERIES.to_owned()))
        .limit(1)
        .send()
        .await
        .or_fail("an archived market")
        .markets;
    let Some(archived) = archived.first() else {
        environmental(&format!("the archive holds no {ARCHIVE_SERIES} market"));
    };
    let open = seconds("open_time", &archived.open_time);
    let close = seconds("close_time", &archived.close_time);

    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(ApiError::Network)
        .or_fail("a reqwest client");
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/trade-api/v2/series?category=Economics";
    check(compare_live::<SeriesList>(path, &raw(&http, path).await));
    let path = format!("/trade-api/v2/series/{ARCHIVE_SERIES}?include_volume=true");
    check(compare_live::<SeriesResponse>(&path, &raw(&http, &path).await));
    let path = "/trade-api/v2/events?status=open&limit=5";
    check(compare_live::<EventsPage>(path, &raw(&http, path).await));
    let path = "/trade-api/v2/events?status=open&limit=2&with_nested_markets=true";
    check(compare_live::<EventsPage>(path, &raw(&http, path).await));
    let pairs: Vec<String> = asked.iter().map(|t| format!("tickers={t}")).collect();
    let path = format!("/trade-api/v2/markets/orderbooks?{}", pairs.join("&"));
    check(compare_live::<OrderbooksResponse>(&path, &raw(&http, &path).await));
    let path = format!(
        "/trade-api/v2/markets?min_updated_ts={}&mve_filter=exclude&limit=5",
        now_seconds() - 86_400
    );
    check(compare_live::<MarketsPage>(&path, &raw(&http, &path).await));
    let path = "/trade-api/v2/historical/cutoff";
    check(compare_live::<HistoricalCutoff>(path, &raw(&http, path).await));
    let path = format!("/trade-api/v2/historical/markets?series_ticker={ARCHIVE_SERIES}&limit=5");
    check(compare_live::<MarketsPage>(&path, &raw(&http, &path).await));
    let path = format!("/trade-api/v2/historical/markets/{}", archived.ticker);
    check(compare_live::<MarketResponse>(&path, &raw(&http, &path).await));
    let path = format!(
        "/trade-api/v2/historical/markets/{}/candlesticks?start_ts={open}&end_ts={}&period_interval=60",
        archived.ticker,
        close + 3_600
    );
    check(compare_live::<HistoricalCandlesticksResponse>(&path, &raw(&http, &path).await));
    let path = format!("/trade-api/v2/historical/trades?ticker={}&limit=5", archived.ticker);
    check(compare_live::<TradesPage>(&path, &raw(&http, &path).await));

    assert!(
        unmodelled.is_empty(),
        "the host sent what the types do not model; model it as an optional field, excuse it \
         in OBSERVED_EXTRA and record it in docs/specs/kalshi/OBSERVED.md: {unmodelled:#?}"
    );
}
```

In the file's `//!` header, after the paragraph naming `live_responses_carry_no_unmodelled_keys`, add:

```rust
//!
//! `live_catalog_responses_carry_no_unmodelled_keys` is the same detector for the catalog and
//! historical routes (issue #71), and `live_updated_lists_only_markets_updated_since_its_bound` and
//! `live_tickers_answer_exactly_the_markets_asked_for` hold the two filters the design session did
//! not see at work on the host.
```

- [ ] **Step 2: Build it and check the live-unwraps gate**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test live_predictions --no-run && nix develop --command python3 scripts/live_unwraps.py`
Expected: builds; `live_unwraps.py` passes (no `.unwrap()`/`.expect(` added).

- [ ] **Step 3: Run the new live tests**

Run: `nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions -- --ignored live_updated live_tickers live_series live_the_cutoff live_catalog`
Expected: 5 passed. A failure tagged `environmental` is the host's state, not the code's; any other failure goes back to the story that owns the route.

### Task 8.2: the live fallback test

**Files:**
- Modify: `polyoxide-kalshi/tests/live_predictions_venue.rs` (append)

- [ ] **Step 1: Append**

```rust
// ── An archived market (issue #71) ─────────────────────────────

use polyoxide_core::ApiError;
use polyoxide_kalshi::predictions::{types::HistoricalSelection, PredictionsError};
use polyoxide_test_support::fail;

/// An RFC 3339 time the host sent, in milliseconds since the epoch; one that does not parse fails
/// the test as the decode failure it is.
fn millis(field: &str, text: &str) -> u64 {
    match chrono::DateTime::parse_from_rfc3339(text) {
        Ok(time) => time.timestamp_millis().unsigned_abs(),
        Err(_) => fail(
            field,
            &ApiError::Validation(format!("{field} {text:?} is not RFC 3339")),
        ),
    }
}

/// The newest archived market of a long-running daily series with volume that the live market
/// route answers `404`, chosen on the route the traits read first.
async fn archived_market(client: &Predictions) -> polyoxide_kalshi::predictions::types::Market {
    let page = client
        .historical()
        .markets(HistoricalSelection::Series("KXHIGHNY".to_owned()))
        .limit(200)
        .send()
        .await
        .or_fail("archived KXHIGHNY markets");
    for market in page.markets {
        if market.volume_fp <= Decimal::ZERO {
            continue;
        }
        let ticker = Ticker::new(&market.ticker).or_fail("name an archived market");
        match client.markets().get(&ticker).send().await {
            Err(PredictionsError::Venue(venue)) if venue.status == 404 => return market,
            _ => continue,
        }
    }
    environmental("no archived KXHIGHNY market with volume is gone from the live route");
}

#[tokio::test]
#[ignore]
async fn live_an_archived_market_reads_through_the_traits() {
    let client = client();
    let market = archived_market(&client).await;
    let ticker = Ticker::new(&market.ticker).or_fail("name the archived market");
    let yes = key::key(&ticker, Outcome::Yes);

    let instrument = client.instrument(&yes).await.or_fail("the archived instrument");
    assert_eq!(instrument.tradable, Some(false), "{yes} trades after settling");

    let open = millis("open_time", &market.open_time);
    let close = millis("close_time", &market.close_time);
    let query = CandleQuery::new(CandleInterval::Hour1, UnixMillis(open)).end(UnixMillis(close));
    let candles = client.candles(&yes, &query).await.or_fail("the archived candles");
    assert!(!candles.is_empty(), "{yes} traded, so the archive holds a candle");

    let trades = client.trades(&yes).await.or_fail("the archived trades");
    assert!(!trades.is_empty(), "{yes} traded, so the archive holds a trade");
}
```

- [ ] **Step 2: Build it, check the gate, run it**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --test live_predictions_venue --no-run && nix develop --command python3 scripts/live_unwraps.py && nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test live_predictions_venue -- --ignored live_an_archived_market`
Expected: builds, the gate passes, 1 passed.

### Task 8.3: the records

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/README.md`
- Modify: `CLAUDE.md`
- Modify: `docs/specs/kalshi/OBSERVED.md`

- [ ] **Step 1: The module README.** After the README's third paragraph (the one beginning "Every request runs on the send loop"), add:

```markdown
For a catalog kept in sync, the module offers each tier its own read. `markets().list()` walks every market, and `markets().updated(since)` lists only those whose metadata changed, which does not include a price move. `markets().list().tickers(..)` reads a named set's quotes in one request, and `markets().orderbooks(..)` up to 100 books; `ticker_chunks` splits a larger set into requests that fit Kalshi's limit on a request's length, past which a set is refused before it is sent. `markets().window(..)` reads the markets created, closed or settled in a span, as one type, since Kalshi refuses two such filters at once. `series()` and `events().list()` read the metadata above markets. Markets that settled before Kalshi's historical cutoff are served only by `historical()`, and the market-data traits read them there on their own when the live routes no longer list them.
```

- [ ] **Step 2: CLAUDE.md.** Three edits:

  - In the API namespaces bullet for Perps/Binance/Kalshi, replace `Kalshi: `kalshi.health()`, `kalshi.exchange()`, `kalshi.markets()`, `kalshi.events()`, `kalshi.account()`.` with `Kalshi: `kalshi.health()`, `kalshi.exchange()`, `kalshi.markets()`, `kalshi.events()`, `kalshi.series()`, `kalshi.historical()`, `kalshi.account()`.`
  - In "Request builder fluency", replace `Seven setters stay hand-written because they are not single query writes: Binance's `GetKlines::limit` and `GetDepth::limit`, which also re-price the route, and gamma's `GetManyMarkets::include_tag` and the `limit` and `offset` of `QueryByInformation` and `QueryAbridged`, which store fields `send` writes.` with `Ten setters stay hand-written because they are not single query writes: Binance's `GetKlines::limit` and `GetDepth::limit`, which also re-price the route, gamma's `GetManyMarkets::include_tag` and the `limit` and `offset` of `QueryByInformation` and `QueryAbridged`, which store fields `send` writes, and Kalshi's `tickers` of `ListMarkets`, `ListMarketWindow` and `ListEvents`, which refuse an empty set or one past `TICKERS_QUERY_BUDGET` (8,000 bytes, half the request target Kalshi answers `414` past) before anything is sent.`
  - In the Kalshi `predictions::venue` paragraph under "Market-data traits", replace `A market settled before Kalshi's historical cutoff is served only by a `/historical` route this client does not implement, so the live route's answer for it passes through as the venue gave it.` with `A market settled before Kalshi's historical cutoff is gone from the live routes and is read from `historical()` instead (issue #71): `instrument` when the live market route answers `404`, `candles` when the batch answer leaves the market out, `trades` when the live route has none, and `event_group` when the live event does not nest the key's market; the archive's `404` is `NotListed`, and no method reads the cutoff.`

- [ ] **Step 3: OBSERVED.md.** Under `## Issue #71: catalog routes`, before `<!-- issue 71 findings go above this line -->`, add a `### The live suites` paragraph recording what Tasks 8.1 and 8.2 saw on the host (whether `updated` answered markets inside its bound, whether `tickers` answered exactly the set, and the archived market the fallback read), dated, with the host.

- [ ] **Step 4: Check the README doctest and the registry**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --doc && nix develop --command python3 scripts/gen_registry.py --check`
Expected: pass.

### Task 8.4: Story 8's gates and commit

- [ ] **Step 1: Run the story gates.** Expected: all pass.

- [ ] **Step 2: Commit**

```bash
git commit -m "test(kalshi): live catalog and archive suites, and the records (#71, Story 8)" -- polyoxide-kalshi/tests/live_predictions.rs polyoxide-kalshi/tests/live_predictions_venue.rs polyoxide-kalshi/src/predictions/README.md CLAUDE.md docs/specs/kalshi/OBSERVED.md
```

---

## After the stories: the final gates

- [ ] **Step 1: Run the profile's final gates** (spec §13), each under `nix develop --command`:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command bash -c 'cargo clippy --all-targets --all-features -- -D warnings && cargo nextest run --all-features --workspace && cargo test --doc --all-features --workspace && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace'
nix develop --command bash -c 'python3 scripts/publish_order.py check-manifests && cargo publish --workspace --dry-run --no-verify --locked'
nix develop --command bash -c 'msrv-cargo check --workspace --all-features && msrv-cargo doc --no-deps --workspace --all-features'
nix develop --command bash -c 'cargo hack check --workspace --each-feature --no-dev-deps --ignore-private && cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions && cargo test -p polyoxide -p polyoxide-kalshi --features polyoxide/full --test headers_predictions'
nix develop --command python3 scripts/api_removals.py check --baseline v0.40.0
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command bash -c 'cd .github/scripts && uv sync && uv run pytest tests/ -v'
```

Expected: all pass. `api_removals.py` reports no removal (everything is additive). Delete `target/semver-checks` afterwards (memory: local build OOM).

- [ ] **Step 2: Hand off.** The release (`0.43.0`) is the owner's, after the branch lands (spec §12).
