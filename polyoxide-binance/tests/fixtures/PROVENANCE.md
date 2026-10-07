# Provenance

Captured 2026-10-07 from `https://fapi.binance.com` and `wss://fstream.binance.com` by the
stdlib scripts handed over with the spec (`docs/specs/binance/probes/capture.py` and
`capture_ws.py`), during the
prader-rs Binance perps work. No credentials.

## REST (`rest/`)

| File | Request | Trimmed to |
|---|---|---|
| `exchange_info.json` | `GET /fapi/v1/exchangeInfo` | `BTCUSDT`, `TSLAUSDT` (TradFi perpetual), `币安人生USDT`, the first `SETTLING` perpetual (`OMGUSDT`) and the first `CURRENT_QUARTER` (`BTCUSDT_261225`); `assets` to one entry |
| `ticker_24hr.json` | `GET /fapi/v1/ticker/24hr` | the three symbols |
| `premium_index.json` | `GET /fapi/v1/premiumIndex` | the three symbols |
| `funding_info.json` | `GET /fapi/v1/fundingInfo` | the three symbols, or its first two rows when none of them is listed |
| `klines.json` | `GET /fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=2` | as fetched |
| `funding_rate.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&limit=3` | as fetched |
| `open_interest.json` | `GET /fapi/v1/openInterest?symbol=BTCUSDT` | as fetched |
| `agg_trades.json` | `GET /fapi/v1/aggTrades?symbol=BTCUSDT&limit=3` | as fetched |
| `depth.json` | `GET /fapi/v1/depth?symbol=BTCUSDT&limit=5` | as fetched |

## Streams (`ws/`)

One combined-stream envelope (`{"stream", "data"}`) per stream: `/market/stream` for
`!ticker@arr`, `!markPrice@arr@1s`, `btcusdt@aggTrade`, `btcusdt@kline_1m`,
`btcusdt@markPrice@1s`, `btcusdt@ticker`; `/public/stream` for `btcusdt@depth20@100ms` and
`btcusdt@bookTicker`. Arrays are trimmed to two rows and depth sides to three levels.

## Probes

`docs/specs/binance/probes/` holds the scripts behind the spec's measured facts: `probe_rest.py` (weights from
`X-MBX-USED-WEIGHT-1M` deltas), `probe_ws.py` and `probe_ws2.py` (acknowledgements, case,
the 1024 cap, the message rate), `probe_ws_ping.py` (server ping cadence), and `wsprobe.py`
(the frame reader they share). They are evidence, not product code; the plan's
`scripts/capture_binance_fixtures.py` replaces the capture pair.
