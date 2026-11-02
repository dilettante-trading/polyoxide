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
