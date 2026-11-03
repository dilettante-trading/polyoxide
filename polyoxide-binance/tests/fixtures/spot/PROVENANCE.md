# Provenance

## REST (rest/)

Captured 2026-11-02 from `https://api.binance.com` by `scripts/capture_binance_spot.py rest`. No credentials. Every key is kept; only list lengths are trimmed.

| File | Request | Trimmed to |
|---|---|---|
| `time.json` | `GET /api/v3/time` | as fetched |
| `exchange_info.json` | `GET /api/v3/exchangeInfo` | `symbols`: `BTCUSDT` (BTCUSDT), `NEOBTC` (the first not `TRADING`, status `BREAK`), `币安人生USDT` (the first with a non-ASCII character), `UNIDOWNUSDT` (the first with `MAX_POSITION`) |
| `depth.json` | `GET /api/v3/depth?symbol=BTCUSDT&limit=5` | as fetched |
| `trades.json` | `GET /api/v3/trades?symbol=BTCUSDT&limit=3` | as fetched |
| `agg_trades.json` | `GET /api/v3/aggTrades?symbol=BTCUSDT&limit=3` | as fetched |
| `klines.json` | `GET /api/v3/klines?symbol=BTCUSDT&interval=1m&limit=2` | as fetched |
| `klines_1s.json` | `GET /api/v3/klines?symbol=BTCUSDT&interval=1s&limit=2` | as fetched |
| `avg_price.json` | `GET /api/v3/avgPrice?symbol=BTCUSDT` | as fetched |
| `ticker_24hr.json` | `GET /api/v3/ticker/24hr?symbol=BTCUSDT` | as fetched |
| `tickers_24hr.json` | `GET /api/v3/ticker/24hr` | `BTCUSDT`, `ETHUSDT`, `币安人生USDT` |
| `price.json` | `GET /api/v3/ticker/price?symbol=BTCUSDT` | as fetched |
| `prices.json` | `GET /api/v3/ticker/price` | `BTCUSDT`, `ETHUSDT`, `币安人生USDT` |
| `book_ticker.json` | `GET /api/v3/ticker/bookTicker?symbol=BTCUSDT` | as fetched |
| `book_tickers.json` | `GET /api/v3/ticker/bookTicker` | `BTCUSDT`, `ETHUSDT`, `币安人生USDT` |
| `error_invalid_symbol.json` | `GET /api/v3/exchangeInfo?symbol=NOPEUSDT` | the body of the `400` reply (`code` -1121), as fetched |

## Streams (ws/)

Captured 2026-11-02 from `wss://stream.binance.com:9443/stream` by `scripts/capture_binance_spot.py ws`. No credentials. One connection and one `SUBSCRIBE` (`id` 1) of the 11 names below, answered `{"result":null,"id":1}`; the first combined-stream envelope (`{"stream", "data"}`) of each, within 30 s, every key kept.

| File | Stream | Trimmed to |
|---|---|---|
| `stream_btcusdt_aggTrade.json` | `btcusdt@aggTrade` | as fetched |
| `stream_btcusdt_trade.json` | `btcusdt@trade` | as fetched |
| `stream_btcusdt_kline_1m.json` | `btcusdt@kline_1m` | as fetched |
| `stream_btcusdt_miniTicker.json` | `btcusdt@miniTicker` | as fetched |
| `stream_all_miniTicker_arr.json` | `!miniTicker@arr` | `data`: the first two rows |
| `stream_btcusdt_ticker.json` | `btcusdt@ticker` | as fetched |
| `stream_btcusdt_bookTicker.json` | `btcusdt@bookTicker` | as fetched |
| `stream_btcusdt_depth5.json` | `btcusdt@depth5` | as fetched |
| `stream_btcusdt_depth20_100ms.json` | `btcusdt@depth20@100ms` | `bids` and `asks`: the first three levels each |
| `stream_btcusdt_avgPrice.json` | `btcusdt@avgPrice` | as fetched |
| `stream_non_ascii_miniTicker.json` | `币安人生usdt@miniTicker` | as fetched |

The non-ASCII stream is for `币安人生USDT`, the first `TRADING` symbol in `GET /api/v3/exchangeInfo` with a non-ASCII character, its ASCII letters lowercased in the stream name (`币安人生usdt@miniTicker`).
