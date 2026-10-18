# Provenance

Captured 2026-10-18 from `https://external-api.kalshi.com/trade-api/v2` by
`scripts/capture_kalshi_predictions.py`. No credentials. Every key is kept; only list
lengths are trimmed.

The market is `KXEPLSPREAD-26OCT18BRICRY-BRI5`, picked from `markets?status=open&mve_filter=exclude` because it was
active, quoted both sides, had an order book with levels on both and more than one level on some
side, and had a full page of 100 trades. The filter is needed: the
unfiltered open listing serves multivariate combinations first (see `docs/specs/kalshi/OBSERVED.md`).

| File | Request | Trimmed to |
|------|---------|------------|
| `exchange_status.json` | `GET /exchange/status` | as fetched |
| `endpoint_costs.json` | `GET /account/endpoint_costs` | as fetched |
| `market.json` | `GET /markets/KXEPLSPREAD-26OCT18BRICRY-BRI5` | as fetched |
| `markets_page.json` | `GET /markets?status=open&mve_filter=exclude&limit=5` | as fetched |
| `orderbook.json` | `GET /markets/KXEPLSPREAD-26OCT18BRICRY-BRI5/orderbook` | the 5 best levels of each side (the host lists a side worst first, so the last 5) |
| `orderbook_depth1.json` | `GET /markets/KXEPLSPREAD-26OCT18BRICRY-BRI5/orderbook?depth=1` | as fetched |
| `trades.json` | `GET /markets/trades?ticker=KXEPLSPREAD-26OCT18BRICRY-BRI5&limit=100` | as fetched |
| `event.json` | `GET /events/KXEPLSPREAD-26OCT18BRICRY?with_nested_markets=true` | `event.markets`: the market above first, then up to 2 more |
| `error_not_found.json` | `GET /markets/NOSUCHMARKET-0` | the 404 body |
| `error_validation.json` | `GET /markets/trades?limit=5000` | the 400 body |
| `error_unauthenticated.json` | `GET /account/limits` | the 401 body, sent with no signature |
| `error_rate_limited.json` | none | written from the 429 body on Kalshi's rate-limit page (https://docs.kalshi.com/getting_started/rate_limits), not captured |
