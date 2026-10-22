# Kalshi catalog routes (issue #71) — design

Status: approved by the owner, 2026-10-22. This spec is the design record for the plan
`docs/plans/2026-10-22-kalshi-catalog-routes.md`. It builds on the Epic 8 and Epic 9 specs
(`docs/superpowers/specs/2026-10-18-epic-8-kalshi-walking-skeleton-design.md`,
`docs/superpowers/specs/2026-10-19-epic-9-kalshi-published-design.md`), whose decisions stand
unless this spec replaces one. Read it with the guide (`docs/ARCHITECTURE.md`) and CLAUDE.md.
Where this spec and an earlier record disagree, this spec binds.

## 1. Scope, stage and sources

**1.1 Goal.** Give `polyoxide-kalshi` the public market-data routes a catalog sync needs
(issue #71, for prader-rs's `prader-sync`): an incremental tier, a price poller over a named set, a
settled feed, the archive past the historical cutoff, series and event metadata, and many books in
one read. The venue-neutral traits also read archived markets, so a caller does not have to know
where the cutoff is.

**1.2 Stage.** The workspace stays in S3. The change adds API to `polyoxide-kalshi` and removes
none, so `docs/s3-removals.md` gains nothing. `polyoxide-kalshi` is absent from the removals
baseline (`v0.40.0`), which `api_removals.py` skips with a warning. The release is `0.43.0`, made
after the branch lands as its own commit on `main` (D17, §12). prader-rs takes it in a lockstep
bump.

**1.3 The owner's choices (2026-10-22).**

- Scope: all six parts of the issue in one plan, the trait fallback included.
- The `/markets` timestamp families: two new builders, `window(MarketWindow)` and
  `updated(min_ts)`, so the spec's compatibility table is a type (D5, D6).
- The trait fallback: live first, for `instrument`, `candles`, `trades` and `event_group`, with no
  cutoff read (D13).
- The `tickers` cap: a byte budget, with a chunker the caller can use (D3, D4).
- Facades: none. The CLI, the Python bindings and the umbrella's prelude do not change; the new
  API is reachable as `polyoxide::kalshi::predictions::…`.

**1.4 Sources.** The vendored mirror `docs/specs/kalshi/openapi.yaml` (3.34.0), `OBSERVED.md`,
the design session's probes (§2) and Story 1's. Where a probe and the mirror disagree, the probe
wins and `OBSERVED.md` records it; the mirror stays byte for byte.

**1.5 The plan's stories.**

| Story | What | Needs |
|---|---|---|
| 1 | Probes, captures, `OBSERVED.md`, the touch-set gate | — |
| 2 | Plumbing: the route table, the repeated-key query, the ticker budget and `ticker_chunks` | — |
| 3 | `markets()`: `tickers`, `window`, `updated`, `orderbooks` | 1, 2 |
| 4 | `series()` | 1, 2 |
| 5 | `events().list()` | 1, 2 |
| 6 | `historical()` | 1, 2 |
| 7 | The trait fallback | 6 |
| 8 | Live suites and docs | 3, 4, 5, 6, 7 |

Story 2 may run in parallel with Story 1. Stories 3 to 6 each add their own API file, types file
and mock suite; they share only append-only edits (the re-exports in `api/mod.rs`,
`types/mod.rs` and `predictions/mod.rs`, and new rows or tests appended to
`query_setters_predictions.rs`, `spec_agreement_predictions.rs` and
`wire_agreement_predictions.rs`). No story quotes code another story writes, except that Story 7
calls Story 6's public names and every story calls Story 2's (the plan's names contract).

## 2. The design session's probes (production, unsigned, 2026-10-22)

Story 1 copies these into `OBSERVED.md` under `## Issue #71: catalog routes`, with its own.

- **P1. `min_updated_ts` tracks metadata, not trading.** `GET /markets?min_updated_ts=<now − 10
  min>&mve_filter=exclude` answered no markets; `<now − 1 day>` answered a full page of 1000 with
  a cursor. The mirror says so ("Tracks non-trading changes only"). The incremental tier sees
  listings and rule changes, not price moves; a price poller uses `tickers`.
- **P2. A request target over about 16 KiB is `414 URI Too Long`.** `GET /markets?tickers=` with
  30-character tickers answered `200` at 520 tickers (16,154 bytes of path and query) and `414`
  at 530 (16,464 bytes). The mirror states no maximum.
- **P3. `GET /series` has no paging.** Unfiltered it answered every series, 15,336 of them, in one
  19 MB body with no `cursor`; `category=Economics` answered 758 KB. `GET /series/{ticker}`
  answered one.
- **P4. `GET /markets/orderbooks` serves unsigned.** Two repeated `tickers=` keys answered
  `{orderbooks: [{ticker, orderbook_fp}]}`, one per ticker, an unknown ticker's sides empty. 101
  tickers answered `400` with a validation `msg` (the `max` tag).
- **P5. `GET /markets` refuses an illegal timestamp combination with `400`, naming it.**
  `min_settled_ts` with `min_close_ts`: "settled ts filters are mutually exclusive with close ts
  filters"; `min_settled_ts` with `status=open`: "… with open or closed status"; `min_updated_ts`
  with `status=open`: "min_updated_ts and max_updated_ts are incompatible with filters other than
  series_ticker and mve_filter=exclude". `min_updated_ts` without `mve_filter` answered `200`.
- **P6. `GET /events` combines its filters and refuses none.** Over a day, `min_updated_ts` alone
  passed 6,000 events (30 pages, not exhausted), with `status=open` 3,139, with
  `min_close_ts=<now>` 5,504. Every answer carries `milestones`, asked for or not.
- **P7. `GET /historical/markets` takes one selector.** Any two of `tickers`, `event_ticker`,
  `series_ticker` and `mve_filter` answered `400`: "tickers, event_ticker, series_ticker, and
  mve_filter filters are mutually exclusive". `tickers` alone (comma-joined) answered both.
- **P8. The live routes drop an archived market, and overlap the archive near the cutoff.** For
  `KXHIGHNY-26MAY30-B72.5` and `KXHIGHNY-26MAR08-B66.5` (settled months before the cutoff):
  `GET /markets/{t}` `404`; `GET /markets/trades?ticker=` `200` with no trades;
  `GET /markets/candlesticks?market_tickers=` `200` with `{"markets": []}`, the market absent;
  `GET /markets/{t}/orderbook` both sides empty; `GET /events/{e}?with_nested_markets=true` `200`
  with no nested markets. A listed market with no candle in the span is present with
  `"candlesticks": []`, so absence tells the two apart. `KXPERLIGA1TOTAL-26AUG22JUAASO-9`,
  settled at 22:54 UTC on 2026-08-22, hours before the cutoff, answered on every live route and on
  the historical ones.
- **P9. The historical candlestick shape is not the live one.** `GET
  /historical/markets/{t}/candlesticks` answers `{ticker, candlesticks}`, each row `end_period_ts`,
  `yes_bid` and `yes_ask` as `{open, low, high, close}`, `price` as
  `{open, low, high, close, mean, previous}`, `volume` and `open_interest`: no `_dollars` or `_fp`
  suffixes, and an absent price sent as `null` (the live route omits it). An unknown ticker is
  `404`.
- **P10. The cutoff.** `GET /historical/cutoff` answered `market_settled_ts` and
  `trades_created_ts` 2026-08-23T00:00:00Z, `orders_updated_ts` 2026-10-08T00:00:00Z and
  `market_positions_last_updated_ts` 2026-08-23T00:00:00Z. `GET /historical/markets/{t}` for a
  live market is `404`.
- **P11. `GET /account/endpoint_costs` lists none of the nine new routes**, so each costs the
  default, 10.

## 3. Decisions (binding)

- **D1. Scope.** §1.3. Everything is a public read; a client without credentials sends every new
  route unsigned, and a keyed client signs them, as it does every route today.

- **D2. The route table.** `request.rs`'s `routes` gains nine routes, appended to `ALL` in this
  order, all `GET`:

  | Name | Template |
  |---|---|
  | `SERIES_LIST` | `/trade-api/v2/series` |
  | `SERIES` | `/trade-api/v2/series/:series_ticker` |
  | `EVENTS` | `/trade-api/v2/events` |
  | `ORDERBOOKS` | `/trade-api/v2/markets/orderbooks` |
  | `HISTORICAL_CUTOFF` | `/trade-api/v2/historical/cutoff` |
  | `HISTORICAL_MARKETS` | `/trade-api/v2/historical/markets` |
  | `HISTORICAL_MARKET` | `/trade-api/v2/historical/markets/:ticker` |
  | `HISTORICAL_CANDLESTICKS` | `/trade-api/v2/historical/markets/:ticker/candlesticks` |
  | `HISTORICAL_TRADES` | `/trade-api/v2/historical/trades` |

  Each is charged on the Read layer at `DEFAULT_COST` until `endpoint_costs` lists it (P11), as
  every route is. The four tests that pin the route table move to nineteen routes (D16).
  `Routed` gains `query_many(key, values)`, which appends one pair per value and replaces nothing,
  for `orderbooks`, and `refuse(key, reason)`, which records a refusal for one query key,
  replacing an earlier refusal for that key; `query` and `query_many` for that key clear it.
  `send` returns a refused path parameter first, as today, then the first key refusal recorded,
  as `ApiError::Validation`, before it charges or sends anything.

- **D3. The `tickers` budget.** A set of tickers sent in one request is refused, as
  `ApiError::Validation` (`InvalidRequest`), with nothing sent, when it is empty or when its size
  exceeds `TICKERS_QUERY_BUDGET`, 8,000 bytes, half of P2's limit. Its size is the sum over the
  set of the ticker's length plus 9 (`tickers=` and a separator), the repeated form's length,
  which the comma-joined form never exceeds, so one measure serves both forms. That admits about
  200 tickers of today's length. An empty set is refused because sending no `tickers` would widen
  the request to every market, and `query_setters!`' `csv` drops an empty value. The rule applies
  to `ListMarkets::tickers`, `ListMarketWindow::tickers`, `markets().orderbooks(..)`,
  `ListEvents::tickers` and `HistoricalSelection::Tickers`; `orderbooks` also refuses more than
  `MAX_ORDERBOOK_TICKERS`, 100 (P4). These setters are hand-written, since a refusal is not a
  query write; setting `tickers` again replaces both its value and its refusal. Both constants are
  public and in `predictions`.

- **D4. `ticker_chunks`.** `predictions::ticker_chunks(tickers: &[Ticker], max_count: usize)`
  returns `TickerChunks<'_>`, an `Iterator<Item = &[Ticker]>` over consecutive, non-empty slices
  of `tickers` in order, each holding at most `max_count` tickers (a `max_count` of 0 is taken as
  1) and fitting D3's budget. Every ticker fits alone, since a `Ticker` is at most 200 characters.
  An empty input yields nothing. `markets().orderbooks` callers pass `MAX_ORDERBOOK_TICKERS`; list
  callers pass `usize::MAX`. It lives in `src/predictions/tickers.rs` with D3's constants and the
  measure D3's refusals call.

- **D5. `markets().window(MarketWindow)`.** Returns `ListMarketWindow` (`GET /markets`), whose
  setters are `event_ticker`, `series_ticker`, `mve_filter`, `tickers` (D3), `limit` and
  `cursor`, and which has no `status`. `MarketWindow` is `#[non_exhaustive]`:

  ```rust
  pub enum MarketWindow {
      Created { status: Option<CreatedStatus>, min_ts: Option<i64>, max_ts: Option<i64> },
      Closed { closed_only: bool, min_ts: Option<i64>, max_ts: Option<i64> },
      Settled { settled_only: bool, min_ts: Option<i64>, max_ts: Option<i64> },
  }
  ```

  `CreatedStatus` is a `wire_enum!` of `Unopened => "unopened"` and `Open => "open"`. `Created`
  sends `status` when set and `min_created_ts`/`max_created_ts`; `Closed` sends `status=closed`
  when `closed_only` and `min_close_ts`/`max_close_ts`; `Settled` sends `status=settled` when
  `settled_only` and `min_settled_ts`/`max_settled_ts`; each bound only when `Some`. Times are Unix
  seconds. Story 1 probes each family with `tickers`, `event_ticker`, `series_ticker` and each
  `mve_filter`; a combination the host refuses is not offered (the builder loses that setter), and
  the finding goes to the owner before Story 3.

- **D6. `markets().updated(min_ts: i64)`.** Returns `ListUpdatedMarkets` (`GET /markets`), which
  sends `min_updated_ts` and `mve_filter=exclude` always, and whose setters are `max_ts` (sent as
  `max_updated_ts`), `series_ticker`, `limit` and `cursor`. Its docs say P1: it sees metadata
  changes, not price moves.

- **D7. `ListMarkets::tickers(&[Ticker])`.** Sends the comma-joined set (D3). The existing setters
  are unchanged.

- **D8. `markets().orderbooks(&[Ticker])`.** Returns `GetOrderbooks`, which sends each ticker as a
  `tickers=` pair (`query_many`) and decodes `OrderbooksResponse { orderbooks:
  Vec<MarketOrderbook> }`, where `MarketOrderbook { ticker: String, orderbook_fp: OrderbookFp }`
  reuses today's `OrderbookFp`. It refuses as D3 says. Story 1 records whether the answer keeps
  the asked order and what a repeated ticker returns; the docs say what it found.

- **D9. `series()`.** A new namespace, `SeriesApi`, from `Predictions::series()`:
  - `list()` returns `ListSeries` (`GET /series`) with `category`, `tags`,
    `include_product_metadata`, `include_volume` and `min_updated_ts`, and decodes
    `SeriesList { series: Vec<Series> }`. It has no cursor (P3); its docs say an unfiltered list
    is one body of about 19 MB.
  - `get(series_ticker: &str)` returns `GetSeries` (`GET /series/{series_ticker}`) with
    `include_volume`, and decodes `SeriesResponse { series: Series }`.
  - `Series` and `SettlementSource { name, url }` follow the mirror's `Series` schema and the
    captures, as Epic 8's types do: a field the schema requires and the wire always carries is
    required, a nullable list is an `Option<Vec<_>>` read with `serde(default)` and written back
    as `null`, as `Event::settlement_sources` is, so the `null` the host sends round-trips (2,617
    of 15,336 series sent `tags: null` on 2026-10-22), and anything else is an `Option`.
    `fee_type` is a `String` and `fee_multiplier` a `serde_json::Number`, as `Event`'s overrides
    are; `volume_fp` is an `Option<Decimal>`; `product_metadata` an `Option<Value>`.

- **D10. `events().list()`.** Returns `ListEvents` (`GET /events`) with `limit` (the host's
  maximum is 200), `cursor`, `with_nested_markets`, `status` (`EventStatusFilter`, a `wire_enum!`
  of `unopened`, `open`, `closed` and `settled`), `series_ticker`, `tickers` (event tickers,
  `impl IntoIterator<Item = impl Into<String>>`, comma-joined, under D3's rule), `min_close_ts` and
  `min_updated_ts`, and decodes `EventsPage { events: Vec<Event>, cursor: String, milestones:
  Vec<Value> }`. No combination is refused, since the host refuses none (P6). `with_milestones` is
  not offered. `Event` is today's type.

- **D11. `historical()`.** A new namespace, `HistoricalApi`, from `Predictions::historical()`:
  - `cutoff()` returns `GetHistoricalCutoff` and decodes `HistoricalCutoff { market_settled_ts,
    trades_created_ts, orders_updated_ts: String, market_positions_last_updated_ts:
    Option<String> }`; times stay RFC 3339 text, as the crate's decoded types keep them.
  - `markets(selection: HistoricalSelection)` returns `ListHistoricalMarkets` with `limit` and
    `cursor`, and decodes `MarketsPage`. `HistoricalSelection` (P7) is `#[non_exhaustive]`:
    `All`, `ExcludeMultivariate` (sends `mve_filter=exclude`), `Tickers(Vec<Ticker>)` (D3),
    `Event(String)` and `Series(String)`.
  - `market(&Ticker)` returns `GetHistoricalMarket` and decodes `MarketResponse`; a market not in
    the archive is `404`.
  - `candlesticks(&Ticker, PeriodInterval, start_ts, end_ts)` returns
    `GetHistoricalCandlesticks` and decodes `HistoricalCandlesticksResponse { ticker,
    candlesticks: Vec<HistoricalCandlestick> }`, whose rows are P9's shape:
    `HistoricalBidAsk { open, low, high, close: Decimal }` and
    `HistoricalPrice { open, low, high, close, mean, previous: Option<Decimal> }`, with `volume` and
    `open_interest` as `Decimal`.
  - `trades()` returns `ListHistoricalTrades` with `ticker`, `min_ts`, `max_ts`, `limit`,
    `cursor` and `is_block_trade`, and decodes `TradesPage`.
  `markets().candlesticks`' docs point at `historical().candlesticks` in place of "which this
  client does not call".

- **D12. Setters and tests.** Every single-key setter goes through `query_setters!`; D3's are the
  only hand-written ones. `tests/query_setters_predictions.rs` gains a case for every new setter,
  D3's included, pinning the exact ordered pairs it sends, and a case per `MarketWindow` variant
  and `HistoricalSelection` arm. `spec_agreement_predictions.rs` gains each new type's optionality
  and field names and each builder's query keys against the mirror;
  `wire_agreement_predictions.rs` gains each new type against Story 1's fixtures, both directions.
  Each of Stories 3 to 6 adds its mock suite: `mock_predictions_markets_catalog.rs`,
  `mock_predictions_series.rs`, `mock_predictions_events.rs` and `mock_predictions_historical.rs`,
  each decoding its fixtures through the native client and pinning its D3 refusals with a mock
  that expects no request (`match_query(Matcher::Any)`, then asserted).

- **D13. The trait fallback: live first.** The traits never read the cutoff. A live answer that
  says the market is not there sends the read to the archive:

  | Method | Falls back when | Then | And |
  |---|---|---|---|
  | `instrument` | `GET /markets/{t}` is not found (today's `is_not_found`) | `historical().market(t)` | a second not-found is `NotListed`; any other error is returned |
  | `candles` | the batch answer has no entry whose `market_ticker` is `t` | `historical().candlesticks` with the same `period_interval`, `start_ts` and `end_ts` (the same shift and clamp) | a not-found is `NotListed`; rows map by Epic 9's D6, `previous` null or absent alike |
  | `trades` | the live page has no trades | `historical().trades().ticker(t).limit(1000)`, reversed to oldest first | a not-found is `NotListed`; an empty page is an empty list |
  | `event_group` | the event's nested markets lack `t` | `historical().markets(HistoricalSelection::Event(e)).limit(1000)`, one page | the outcomes are the nested markets in Kalshi's order, then each archived market not already among them, in Kalshi's order |

  `event_group` finds the market through `instrument`'s lookup, so an archived market's event is
  read too. `quote` and `book` are unchanged: an archived market's book is empty (P8). `CandleExt`
  from a historical row carries `yes_bid` and `yes_ask` as `Ohlc`, `open_interest` and `mean`, as
  from a live one. A live market costs what it costs today, and an archived one one request more.
  Two behaviours change: `candles` of an unknown ticker is `NotListed` (it was an empty list), and
  `trades` of one is `NotListed` if the archive answers it with a not-found (Story 1 records
  which). `venue.rs`'s module docs and request table say all of it. Story 7's tests run on mock
  servers with an archived ticker and an unknown one for each row, and for `event_group` an event
  whose markets are partly archived.

- **D14. Live suites.** All appended, never edits (D16):
  - `live_predictions.rs`: each new route reads and decodes; `updated(now − 1 day)` answers only
    markets whose `updated_time` is at or after the bound (`environmental` when it answers none);
    `list().tickers(..)` for three tickers taken from a listing answers exactly those three;
    `orderbooks` for the same three answers one book each; the cutoff's four times parse as
    RFC 3339; `historical().market` reads a ticker taken from `historical().markets`;
    `live_catalog_responses_carry_no_unmodelled_keys` holds every new type to the wire, as
    `live_responses_carry_no_unmodelled_keys` holds today's; `series().list()` is filtered by
    `category` to keep the body small.
  - `live_predictions_venue.rs`: an archived market with volume, taken from
    `historical().markets`, reads through `instrument`, `candles` and `trades`.
  These run in the existing nightly job `live-polyoxide-kalshi-predictions`; its 15-minute budget
  holds them.

- **D15. Records.** `OBSERVED.md` gains `## Issue #71: catalog routes` (§2 and Story 1's
  findings). The `predictions` README (a doctest) gains a short catalog-sync section: the tiers,
  `ticker_chunks`, and the fallback. CLAUDE.md's Kalshi lines follow the code: the namespaces line
  (`kalshi.series()`, `kalshi.historical()`), the hand-written setters sentence (D3's),
  the sentence that says a market settled before the cutoff "is served only by a `/historical`
  route this client does not implement", and the request counts.

- **D16. Test edits the owner allows (AD-12).** Add tests; never change an existing test's body,
  except these, each in a task of its own:
  - Story 2 moves the four route-table pins to the nineteen routes of D2, renaming "ten" to
    "nineteen" in their names, messages and counts, with the `routes` doc comment:
    `request.rs`'s `the_route_table_is_the_epics_ten_routes`, `mock_predictions_routes.rs`'s
    `the_route_table_holds_the_ten_routes_in_endpoint_costs_spelling`,
    `spec_agreement_predictions.rs`'s `every_route_the_crate_holds_is_a_documented_operation` and
    `live_predictions.rs`'s `live_the_route_table_agrees_with_the_endpoint_costs`.
  - Story 7 adds the archive's answer to the venue tests D13 changes, in `venue_predictions.rs`:
    `an_unknown_ticker_is_not_listed_but_its_quote_and_book_are_empty`,
    `only_a_404_is_not_listed` and `an_unlisted_key_has_no_event_group_and_asks_for_no_event`
    each gain a `404` mock of `GET /historical/markets/{t}`, and
    `a_market_the_answer_does_not_list_has_no_candles` becomes D13's rule (an absent market asks
    the archive, whose `404` is `NotListed`), renamed to say so.
  Any other existing test a story would change goes back to the owner.

- **D17. The release.** `0.43.0`, after the branch lands, by the owner (§12).

## 4. Story 1: probes and captures

No Rust. Production, unsigned, through `scripts/capture_kalshi_predictions.py`, extended.

- Capture a fixture per new route, with a mode of its own (`--catalog OUT_DIR`) that leaves the
  other fixtures alone, each with a row in its folder's `PROVENANCE.md`. Under
  `polyoxide-kalshi/tests/fixtures/predictions/catalog/`: `series_list.json` (one `category`),
  `series.json`, `events_page.json`, `events_page_nested.json`, `orderbooks.json` (two active
  tickers and one unknown), `markets_updated_page.json`, `markets_tickers_page.json` and
  `markets_window_settled_page.json`. Under `…/predictions/historical/`: `cutoff.json`,
  `markets_page.json`, `market.json`, `candlesticks_1.json`, `candlesticks_60.json` and
  `candlesticks_1440.json` for one archived market with volume that the live market route
  answers `404`, and `trades.json`.
- Probe, and record in `OBSERVED.md`:
  - each `MarketWindow` family with `tickers`, `event_ticker`, `series_ticker` and each
    `mve_filter` (D5);
  - `orderbooks`' answer order and a repeated ticker (D8);
  - `GET /historical/trades` and `GET /historical/markets/{t}/candlesticks` for an unknown ticker
    (`404` or empty), and the historical candle route over 10,000 periods (D13);
  - `GET /events?limit=201`, and `GET /series/{t}` for an unknown series.
- Write `_bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.txt` and
  `issue-71-touch-set.sh` (Epic 9's with `epic-9` replaced by `issue-71`), admitting exactly §13's
  paths.
- A finding that contradicts D5, D8, D11 or D13 goes to the owner before the story that needs it.

## 5. Story 2: plumbing (D2, D3, D4)

`request.rs` (the nine routes, `query_many`, `refuse`), `src/predictions/tickers.rs`
(`TICKERS_QUERY_BUDGET`, `MAX_ORDERBOOK_TICKERS`, the measure, `ticker_chunks`, `TickerChunks`),
re-exported from `predictions`. Unit tests: the measure; the chunker's bounds (empty input, a
`max_count` smaller than the set, a set exactly at the budget and one a byte over, `max_count` 0);
`query_many`'s order; `refuse` sending nothing, and a later `query` for its key clearing it. The
four route-table pins (D16).

## 6. Story 3: `markets()` (D5 to D8)

`api/markets.rs` and `types/markets.rs`: `ListMarkets::tickers`, `window` and
`ListMarketWindow`, `updated` and `ListUpdatedMarkets`, `orderbooks` and `GetOrderbooks`,
`MarketWindow`, `CreatedStatus`, `OrderbooksResponse`, `MarketOrderbook`. Mock suite
`mock_predictions_markets_catalog.rs`; D12's rows.

## 7. Story 4: `series()` (D9)

`api/series.rs`, `types/series.rs`, `Predictions::series()`. Mock suite
`mock_predictions_series.rs`; D12's rows.

## 8. Story 5: `events().list()` (D10)

`api/events.rs` (append), `types/events.rs` (append `EventsPage`, `EventStatusFilter`). Mock suite
`mock_predictions_events.rs`; D12's rows.

## 9. Story 6: `historical()` (D11)

`api/historical.rs`, `types/historical.rs`, `Predictions::historical()`, the pointer in
`markets().candlesticks`' docs. Mock suite `mock_predictions_historical.rs`; D12's rows.

## 10. Story 7: the trait fallback (D13)

`venue.rs`: the four fallbacks, the historical row's candle mapping beside the live one, the
module docs and request table. New tests in `venue_predictions_historical.rs`; D16's four edits.

## 11. Story 8: live suites and docs (D14, D15)

The live tests of D14, `OBSERVED.md`'s closing anchor, the README section, CLAUDE.md's lines.

## 12. After the run: the release (D17)

An owner session, not a story. Before picking the version, check origin's `main` and tags
(`git fetch`, `git ls-remote --tags`): if `v0.43.0` exists or `main` has moved past `0.42.0`,
stop and ask. The branch lands on `main` as a main-first merge, then the release commit
`chore(release): v0.43.0` with its CHANGELOG entry; the owner pushes.

## 13. Gates every story keeps

- The flow profile's story gates under `nix develop --command`: fmt, clippy `-D warnings`, tests,
  rustdoc `-D warnings`, `gen_registry.py --check`, `venue_gate.py`, `live_unwraps.py`,
  `test_body_diff.py --baseline v0.40.0`.
- The touch-set gate, against `issue-71-touch-set.txt`, admitting: `polyoxide-kalshi/`;
  `docs/specs/kalshi/OBSERVED.md`; `scripts/capture_kalshi_predictions.py`; `CLAUDE.md`; this
  spec, the plan, `_bmad-output/specs/spec-venue-extensibility/issue-71-touch-set.*`; and
  `.flow/`. The mirror (`openapi.yaml`), the umbrella, the foundation crates and CI are outside
  it.
- The final gates add `api_removals.py check --baseline v0.40.0`, the package job's three steps,
  and the features job's steps (Kalshi's `predictions` header pin, alone and under
  `polyoxide/full`).
- Live tests follow test-support's loaders and tags, with no `.unwrap()`; the mirror stays byte
  for byte.
- Commits by pathspec, conventional, naming the story, never `--no-verify`.

## 14. Not in this change

- `/portfolio/*`, `/historical/fills`, `/historical/orders`, `/historical/positions` and anything
  signed-only.
- `/events/multivariate`, `/series/fee_changes`, `/events/{e}/metadata`, `with_milestones` and a
  typed `Milestone`.
- A `pages()` stream over the cursor routes; callers loop on `cursor`.
- `Instruments` for Kalshi, the CLI, the Python bindings, the umbrella's prelude.

## 15. Owner actions

- None before the run: every route is unsigned on production.
- After the run, the release (§12).
