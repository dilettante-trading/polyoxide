# Provenance

REST fixtures (`rest/`) captured 2026-10-07 from `https://fapi.binance.com` by
`scripts/capture_binance_fixtures.py`. No credentials. Every top-level key is kept; only
list lengths are trimmed.

| File | Request | Trimmed to |
|---|---|---|
| `exchange_info.json` | `GET /fapi/v1/exchangeInfo` | `symbols`: `BTCUSDT`, `XAUUSDT`, `币安人生USDT`, `OMGUSDT`, `BTCUSDT_261225` (BTCUSDT, a TradFi perpetual, a Chinese-character perpetual, the first `SETTLING` contract, the first `CURRENT_QUARTER`); `assets`: the first two |
| `time.json` | `GET /fapi/v1/time` | as fetched |
| `ticker_24hr.json` | `GET /fapi/v1/ticker/24hr` | `BTCUSDT`, `XAUUSDT`, `币安人生USDT` |
| `premium_index.json` | `GET /fapi/v1/premiumIndex` | the same three |
| `funding_info.json` | `GET /fapi/v1/fundingInfo` | the same three, plus a row with `updateTime: null` if none of them has one |
| `klines.json` | `GET /fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=2` | as fetched |
| `funding_rate.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&limit=3` | as fetched |
| `funding_rate_2019.json` | `GET /fapi/v1/fundingRate?symbol=BTCUSDT&startTime=1568102400000&limit=2` | as fetched; `markPrice` is `""` |
| `open_interest.json` | `GET /fapi/v1/openInterest?symbol=BTCUSDT` | as fetched |
| `agg_trades.json` | `GET /fapi/v1/aggTrades?symbol=BTCUSDT&limit=3` | as fetched |
| `depth.json` | `GET /fapi/v1/depth?symbol=BTCUSDT&limit=5` | as fetched |

## Streams (`ws/`)

Captured 2026-10-07 from `wss://fstream.binance.com` by the same script: one combined-stream
envelope (`{"stream", "data"}`) per stream, from `/market/stream` for `!ticker@arr`,
`!markPrice@arr@1s`, `btcusdt@aggTrade`, `btcusdt@kline_1m`, `btcusdt@markPrice@1s` and
`btcusdt@ticker`, and from `/public/stream` for `btcusdt@depth20@100ms` and
`btcusdt@bookTicker`. An array stream is read until a frame carries a COIN-M row, within
30 s, and keeps two of its rows: the first USDⓈ-M row (`st: 1`) with a scheduled funding
time (ticker rows have no `T`, so there the first USDⓈ-M row), then the first COIN-M row
(`st: 2`), or the next row when no frame had one. This run: `!markPrice@arr@1s` kept `BTCUSDT` (`st: 1`) and `BTCUSD_PERP` (`st: 2`); `!ticker@arr` kept `QCOMUSDT` (`st: 1`) and `ETCUSD_PERP` (`st: 2`). Depth sides keep
three levels. Files: `stream_all_markPrice_arr_1s.json`, `stream_all_ticker_arr.json`, `stream_btcusdt_aggTrade.json`, `stream_btcusdt_bookTicker.json`, `stream_btcusdt_depth20_100ms.json`, `stream_btcusdt_kline_1m.json`, `stream_btcusdt_markPrice_1s.json`, `stream_btcusdt_ticker.json`.

## Probes

`docs/specs/binance/probes/` holds the scripts behind the design spec's measured facts:
`probe_rest.py` (weights from `X-MBX-USED-WEIGHT-1M` deltas), `probe_ws.py` and
`probe_ws2.py` (acknowledgements, case, the 1024 cap, the message rate),
`probe_ws_ping.py` (server ping cadence), and `wsprobe.py` (the frame reader they share).
