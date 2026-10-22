# Provenance

Captured 2026-10-22 from `https://external-api.kalshi.com/trade-api/v2` by
`scripts/capture_kalshi_predictions.py --catalog`. No credentials. Every key is kept; only list
lengths are trimmed. The archived market is `KXHIGHNY-26AUG13-T92`: the newest KXHIGHNY market in the archive with volume that `GET /markets/{ticker}` answers 404.

| File | Request | Trimmed to |
|------|---------|------------|
| `cutoff.json` | `GET /historical/cutoff` | as fetched |
| `markets_page.json` | `GET /historical/markets?series_ticker=KXHIGHNY&limit=3` | as fetched |
| `market.json` | `GET /historical/markets/KXHIGHNY-26AUG13-T92` | as fetched |
| `candlesticks_1.json` | `GET /historical/markets/KXHIGHNY-26AUG13-T92/candlesticks?start_ts=<open>&end_ts=<close + 1 min>&period_interval=1` | the first 120 rows |
| `candlesticks_60.json` | `GET /historical/markets/KXHIGHNY-26AUG13-T92/candlesticks?start_ts=<open>&end_ts=<close + 60 min>&period_interval=60` | as fetched |
| `candlesticks_1440.json` | `GET /historical/markets/KXHIGHNY-26AUG13-T92/candlesticks?start_ts=<open>&end_ts=<close + 1440 min>&period_interval=1440` | as fetched |
| `trades.json` | `GET /historical/trades?ticker=KXHIGHNY-26AUG13-T92&limit=20` | as fetched |
