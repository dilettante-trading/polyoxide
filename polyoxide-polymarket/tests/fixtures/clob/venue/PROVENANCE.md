# CLOB venue fixtures

Captured 2026-10-16T18:05:58Z by `scripts/capture_polymarket_clob.py` from the live hosts, for the
outcome token `54533043819946592547517511176940999955633860128497669742211153063842200957669` of the market `0x0f49db97f71c68b1e42a6d16e3de93d85dbf7d4148e3f018eb79e88554be9f75`. The clob module's venue tests
(`tests/venue_clob.rs`) serve them from one mock server. Lists are trimmed as noted.

| Fixture | Request | Kept |
|---------|---------|------|
| `markets_by_token.json` | `https://clob.polymarket.com/markets-by-token/54533043819946592547517511176940999955633860128497669742211153063842200957669` | as fetched |
| `market.json` | `https://clob.polymarket.com/markets/0x0f49db97f71c68b1e42a6d16e3de93d85dbf7d4148e3f018eb79e88554be9f75` | as fetched |
| `book.json` | `https://clob.polymarket.com/book?token_id=54533043819946592547517511176940999955633860128497669742211153063842200957669` | each side to its 10 levels nearest the touch, in wire order |
| `prices_history.json` | `https://clob.polymarket.com/prices-history?market=54533043819946592547517511176940999955633860128497669742211153063842200957669&fidelity=60&startTs=1791914757&endTs=1792173957` | as fetched |
| `trades.json` | `https://data-api.polymarket.com/trades?market=0x0f49db97f71c68b1e42a6d16e3de93d85dbf7d4148e3f018eb79e88554be9f75&takerOnly=true` | the first 30 rows, newest first, 26 of the token and 4 of another |
| `gamma_markets.json` | `https://gamma-api.polymarket.com/markets?clob_token_ids=54533043819946592547517511176940999955633860128497669742211153063842200957669` | as fetched |
| `gamma_event.json` | `https://gamma-api.polymarket.com/events/30829` | `markets` to the token's market and the next 2 |
