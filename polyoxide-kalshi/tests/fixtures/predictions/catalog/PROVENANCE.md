# Provenance

Captured 2026-10-22 from `https://external-api.kalshi.com/trade-api/v2` by
`scripts/capture_kalshi_predictions.py --catalog`. No credentials. Every key is kept; only list
lengths are trimmed. The active markets are `KXTEMPMIAH-26OCT2206-T76.99` and `KXTEMPMIAH-26OCT2206-T75.99`, of series `KXTEMPMIAH`.

| File | Request | Trimmed to |
|------|---------|------------|
| `series_list.json` | `GET /series?category=Economics` | `series`: 3 rows, one with `tags: null` first when the page has one |
| `series.json` | `GET /series/KXTEMPMIAH?include_volume=true` | as fetched |
| `events_page.json` | `GET /events?status=open&limit=3` | as fetched |
| `events_page_nested.json` | `GET /events?status=open&limit=2&with_nested_markets=true` | each event's `markets`: the first 2 |
| `orderbooks.json` | `GET /markets/orderbooks?tickers=KXTEMPMIAH-26OCT2206-T76.99&tickers=KXTEMPMIAH-26OCT2206-T75.99&tickers=NOSUCHMARKET-0` | the 5 best levels of each side |
| `markets_updated_page.json` | `GET /markets?min_updated_ts=1792573353&mve_filter=exclude&limit=3` | as fetched |
| `markets_tickers_page.json` | `GET /markets?tickers=KXTEMPMIAH-26OCT2206-T76.99,KXTEMPMIAH-26OCT2206-T75.99` | as fetched |
| `markets_window_settled_page.json` | `GET /markets?status=settled&min_settled_ts=1792054953&mve_filter=exclude&limit=3` | as fetched |
