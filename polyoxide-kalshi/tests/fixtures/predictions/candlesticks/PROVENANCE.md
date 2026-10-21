# Provenance

Captured 2026-10-21T09:32:12Z from `https://external-api.kalshi.com/trade-api/v2` by `scripts/capture_kalshi_predictions.py --candlesticks`.
No credentials. Every key is kept and no list is trimmed.

The market is `KXUCLGAME-26OCT21RMARBL-RMA` (series `KXUCLGAME`), picked from `markets?status=open&mve_filter=exclude` because
it quoted both sides and its answers held a traded and an untraded period at every width. Each span starts
at the later of the market's open and the last 2 hours (width 1), 7 days (width 60) or 120 days (width 1440).

| File | Host | Request | Span (UTC) | Captured |
|------|------|---------|------------|----------|
| `batch_1.json` | external-api.kalshi.com | `GET /markets/candlesticks?market_tickers=KXUCLGAME-26OCT21RMARBL-RMA&period_interval=1&start_ts=1792567919&end_ts=1792575119` | 2026-10-21 07:31Z to 2026-10-21 09:31Z | 2026-10-21T09:32:12Z |
| `batch_60.json` | external-api.kalshi.com | `GET /markets/candlesticks?market_tickers=KXUCLGAME-26OCT21RMARBL-RMA&period_interval=60&start_ts=1791970319&end_ts=1792575119` | 2026-10-14 09:31Z to 2026-10-21 09:31Z | 2026-10-21T09:32:12Z |
| `batch_1440.json` | external-api.kalshi.com | `GET /markets/candlesticks?market_tickers=KXUCLGAME-26OCT21RMARBL-RMA&period_interval=1440&start_ts=1791333300&end_ts=1792575119` | 2026-10-07 00:35Z to 2026-10-21 09:31Z | 2026-10-21T09:32:12Z |
| `series_60.json` | external-api.kalshi.com | `GET /series/KXUCLGAME/markets/KXUCLGAME-26OCT21RMARBL-RMA/candlesticks?period_interval=60&start_ts=1791970319&end_ts=1792575119` | 2026-10-14 09:31Z to 2026-10-21 09:31Z | 2026-10-21T09:32:12Z |
| `error_width.json` | external-api.kalshi.com | `GET /series/KXUCLGAME/markets/KXUCLGAME-26OCT21RMARBL-RMA/candlesticks?period_interval=5&start_ts=1791970319&end_ts=1792575119` (the 400 body) | 2026-10-14 09:31Z to 2026-10-21 09:31Z | 2026-10-21T09:32:12Z |
