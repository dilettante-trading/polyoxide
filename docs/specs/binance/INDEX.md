# Binance USDⓈ-M futures and spot

Binance is not a Polymarket host. `polyoxide-binance`'s `usdm` module reads its USDⓈ-M
futures public market data, and its `spot` module reads its spot public market data, for
consumers that trade both venues. The `polyoxide` umbrella re-exports them as
`polyoxide::binance`, under its `binance-usdm`, `binance-usdm-ws` and `binance-spot` features
and `full`.

| Surface | Host | Crate |
|---|---|---|
| USDⓈ-M REST, public market data | `https://fapi.binance.com` | `polyoxide-binance` with the `usdm` feature (`usdm::Usdm`) |
| USDⓈ-M market streams | `wss://fstream.binance.com/{market,public}/stream` | `polyoxide-binance` with the `usdm-ws` feature (`usdm::ws::UsdmWs`, `usdm::ws::UsdmWsBuilder`) |
| Spot REST, public market data | `https://api.binance.com` | `polyoxide-binance` with the `spot` feature (`spot::Spot`) |

Spot has no streams yet.

**This directory is not a mirror.** Binance publishes no OpenAPI or AsyncAPI document for
USDⓈ-M futures (`github.com/binance/binance-api-swagger` holds `spot_api.yaml` only), so
there is nothing to vendor and nothing for `nightly-schema.yml` to diff. Spot's
`spot_api.yaml` was last released on 2024-10-08 (v1.22.0) and lacks fields the wire sends
today (`amendAllowed`, `opoAllowed`, `pegInstructionsAllowed`), so a diff of it would never
fire either. The sources are the prose pages and the wire:

- USDⓈ-M REST pages: `https://developers.binance.com/docs/derivatives/usds-margined-futures/`
- USDⓈ-M stream pages: `https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/`
  (`market` and `public`). The old stream URLs under the REST prefix land on a generic
  page as of 2026-10-07.
- Spot pages: `https://github.com/binance/binance-spot-api-docs`.

Where the pages and the wire disagree, the wire wins and [OBSERVED.md](OBSERVED.md)
records it. The drift detectors are the live suites:
`polyoxide-binance/tests/live_usdm.rs::live_responses_carry_no_unmodelled_keys`,
`live_usdm_ws.rs::live_frames_carry_no_unmodelled_keys` and
`live_spot.rs::live_responses_carry_no_unmodelled_keys` fail on any key the types do not model.
A new enum value or filter type decodes as `Other` and is not seen, and a changed weight
goes unseen until `weight_probe` (USDⓈ-M) or `weight_probe_spot` (spot) is run by hand.

## USDⓈ-M routes covered

| Route | Method | Weight |
|---|---|---|
| `/fapi/v1/ping` | `usdm.health().ping()` | 1 |
| `/fapi/v1/time` | `usdm.health().time()` | 1 |
| `/fapi/v1/exchangeInfo` | `usdm.exchange().exchange_info()` | 1 |
| `/fapi/v1/fundingInfo` | `usdm.exchange().funding_info()` | funding limit, 500 per 5 minutes |
| `/fapi/v1/ticker/24hr` | `usdm.market().ticker_24h(&s)` / `tickers_24h()` | 1 / 40 |
| `/fapi/v1/premiumIndex` | `usdm.market().premium_index(&s)` / `premium_indices()` | 1 / 10 |
| `/fapi/v1/klines` | `usdm.market().klines(&s, interval)` | 1 to 10 by `limit`, 5 without |
| `/fapi/v1/fundingRate` | `usdm.market().funding_rate()` | funding limit, 500 per 5 minutes |
| `/fapi/v1/openInterest` | `usdm.market().open_interest(&s)` | 1 |
| `/fapi/v1/aggTrades` | `usdm.market().agg_trades(&s)` | 20 |
| `/fapi/v1/depth` | `usdm.market().depth(&s)` | 1 without `limit`, 2 to 20 with |

The weight table is `Route::cost` in `polyoxide-binance/src/usdm/weight.rs`, pinned by its
`documented_weights` test and re-measured live by
`cargo run -p polyoxide-binance --example weight_probe`.

## Spot routes covered

| Route | Method | Weight |
|---|---|---|
| `/api/v3/ping` | `spot.health().ping()` | 1 |
| `/api/v3/time` | `spot.health().time()` | 1 |
| `/api/v3/exchangeInfo` | `spot.exchange().exchange_info()`, with `.symbol(&s)` or without | 20 either way |
| `/api/v3/depth` | `spot.market().depth(&s)` | 5 without `limit` and for 1 to 100; 25 for 101 to 500; 50 for 501 to 1000; 250 above 1000 (documented, not measured) |
| `/api/v3/trades` | `spot.market().trades(&s)` | 25, with or without `limit` |
| `/api/v3/aggTrades` | `spot.market().agg_trades(&s)` | 4, with or without `limit` |
| `/api/v3/klines` | `spot.market().klines(&s, interval)` | 2, with or without `limit` |
| `/api/v3/avgPrice` | `spot.market().avg_price(&s)` | 2 |
| `/api/v3/ticker/24hr` | `spot.market().ticker_24h(&s)` / `tickers_24h()` | 2 / 80 |
| `/api/v3/ticker/price` | `spot.market().price(&s)` / `prices()` | 2 / 4 |
| `/api/v3/ticker/bookTicker` | `spot.market().book_ticker(&s)` / `book_tickers()` | 2 / 4 |

The weight table is `Route::cost` in `polyoxide-binance/src/spot/weight.rs`, pinned by its
`documented_weights` test and re-measured live by the `weight_probe_spot` example
(`cargo run -p polyoxide-binance --example weight_probe_spot`, which takes no `--product`
argument; `weight_probe` measures USDⓈ-M only). A `limit` past a route's maximum is truncated
by the venue, not refused, and the cost does not change (`depth` is priced by its band). `api.binance.com` and
`data-api.binance.vision` and the weight window are covered in [OBSERVED.md](OBSERVED.md)'s
`## Spot` section.

## USDⓈ-M streams covered

| Stream | `StreamName` | Path | `Payload` |
|---|---|---|---|
| `!ticker@arr` | `AllTickers` | market | `Tickers(Vec<TickerEvent>)` |
| `!markPrice@arr@1s` | `AllMarkPrices` | market | `MarkPrices(Vec<MarkPriceEvent>)` |
| `<s>@aggTrade` | `AggTrade(symbol)` | market | `AggTrade(AggTradeEvent)` |
| `<s>@kline_<interval>` | `Kline(symbol, interval)` | market | `Kline(KlineEvent)` |
| `<s>@markPrice@1s` | `MarkPrice(symbol)` | market | `MarkPrice(MarkPriceEvent)` |
| `<s>@ticker` | `Ticker(symbol)` | market | `Ticker(TickerEvent)` |
| `<s>@depth<5\|10\|20>`, then `@100ms`, `@500ms` or nothing (250 ms) | `PartialDepth(symbol, levels, speed)` | public | `PartialDepth(PartialDepthEvent)` |
| `<s>@bookTicker` | `BookTicker(symbol)` | public | `BookTicker(BookTickerEvent)` |

## Fixtures and probes

- `polyoxide-binance/tests/fixtures/usdm/rest/`: refreshed by
  `uv run scripts/capture_binance_usdm.py polyoxide-binance/tests/fixtures/usdm`.
- `polyoxide-binance/tests/fixtures/usdm/ws/`: stream envelopes, refreshed by the same script
  and compiled into `polyoxide_binance::usdm::ws::fixtures` under `usdm-ws` and `test-server`.
- `polyoxide-binance/tests/fixtures/spot/rest/`: refreshed by
  `python3 scripts/capture_binance_spot.py rest`, with each file's request and trimming in
  `polyoxide-binance/tests/fixtures/spot/PROVENANCE.md`.
- `probes/`: the stdlib scripts behind most of the design spec's measurements. Its
  `capture.py` and `capture_ws.py` are superseded by `scripts/capture_binance_usdm.py`.
  `probes/probe_rest.py --product {usdm,spot}` measures the REST weights and limits of either
  product (`usdm` is the default).
