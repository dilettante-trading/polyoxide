# Perps WebSocket fixtures

Captured 2026-09-30T15:10:03Z by `scripts/capture_perps_ws_fixtures.py` from `wss://ws.perpetuals.polymarket.com/v1/ws`:
one `sub` for ['bbo::6', 'book::6', 'book::6::50', 'trades::6', 'klines::6::1m', 'tickers::6', 'tickers::all', 'statistics::6', 'statistics::all'], a ping, a malformed `sub`, 20 s of frames, then an `unsub`.
Instrument 6. Each file is the first frame seen for its label, pretty-printed
(for `klines`, the first frame with a non-empty `data` when one arrived).

`tickers::all` and `statistics::all` fanned out as 174 per-instrument labels; no frame carried an `::all` label.

| Fixture | Label or request |
|---------|------------------|
| `response_subscribe.json` | `response_subscribe` |
| `response_ping.json` | `response_ping` |
| `response_refused.json` | `response_refused` |
| `tickers.json` | `tickers` |
| `book.json` | `book` |
| `book_50.json` | `book_50` |
| `bbo.json` | `bbo` |
| `klines.json` | `klines` |
| `statistics.json` | `statistics` |
| `trades.json` | `trades` |
| `response_unsubscribe.json` | `response_unsubscribe` |
