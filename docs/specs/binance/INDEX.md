# Binance USDⓈ-M futures

Binance is not a Polymarket host. `polyoxide-binance` reads its USDⓈ-M futures public
market data for consumers that trade both venues; it is not part of the `polyoxide`
umbrella crate.

| Surface | Host | Crate |
|---|---|---|
| REST, public market data | `https://fapi.binance.com` | `polyoxide-binance` (`Usdm`) |
| Market streams | `wss://fstream.binance.com/{market,public}/stream` | the WebSocket plan, not yet implemented |

**This directory is not a mirror.** Binance publishes no OpenAPI or AsyncAPI document for
USDⓈ-M futures (`github.com/binance/binance-api-swagger` holds `spot_api.yaml` only), so
there is nothing to vendor and nothing for `nightly-schema.yml` to diff. The sources are
the prose pages and the wire:

- REST pages: `https://developers.binance.com/docs/derivatives/usds-margined-futures/`
- Stream pages: `https://developers.binance.com/en/docs/catalog/core-trading-derivatives-trading-usd-s-m-futures/api/ws-streams/`
  (`market` and `public`). The old stream URLs under the REST prefix land on a generic
  page as of 2026-10-07.

Where the pages and the wire disagree, the wire wins and [OBSERVED.md](OBSERVED.md)
records it. The drift detector is the live suite:
`polyoxide-binance/tests/live_api.rs::live_responses_carry_no_unmodelled_keys` fails on any
key the types do not model.
A new value of an enum decodes as `Other` and is not seen, and a changed weight goes
unseen until `weight_probe` is run by hand.

## Routes covered

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

The weight table is `Route::cost` in `polyoxide-binance/src/weight.rs`, pinned by its
`documented_weights` test and re-measured live by
`cargo run -p polyoxide-binance --example weight_probe`.

## Fixtures and probes

- `polyoxide-binance/tests/fixtures/rest/`: refreshed by
  `python3 -I scripts/capture_binance_fixtures.py polyoxide-binance/tests/fixtures`.
- `polyoxide-binance/tests/fixtures/ws/`: stream envelopes captured 2026-10-07.
- `probes/`: the stdlib scripts behind most of the design spec's measurements. Its
  `capture.py` is superseded by `scripts/capture_binance_fixtures.py`.
