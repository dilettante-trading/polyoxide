# Perps fixtures

Captured 2026-09-30T09:53:27Z by `scripts/capture_perps_fixtures.py` from the live host.
Each file is the complete response body, pretty-printed. Inputs were chosen live
(see the script), so the instrument and address here are whatever was active then.

| Fixture | Request |
|---------|---------|
| `time.json` | `https://api.perpetuals.polymarket.com/v1/info/time` |
| `exchange.json` | `https://api.perpetuals.polymarket.com/v1/info/exchange` |
| `assets.json` | `https://api.perpetuals.polymarket.com/v1/info/assets` |
| `instruments.json` | `https://api.perpetuals.polymarket.com/v1/info/instruments` |
| `fees.json` | `https://api.perpetuals.polymarket.com/v1/info/fees` |
| `limit_tiers.json` | `https://api.perpetuals.polymarket.com/v1/info/limit-tiers` |
| `tickers.json` | `https://api.perpetuals.polymarket.com/v1/info/tickers?instrument_id=1` |
| `statistics.json` | `https://api.perpetuals.polymarket.com/v1/info/statistics?instrument_id=1` |
| `exchange_stats.json` | `https://api.perpetuals.polymarket.com/v1/info/exchange-stats?start_timestamp=1790675505663&end_timestamp=1790761905663` |
| `klines.json` | `https://api.perpetuals.polymarket.com/v1/info/klines?instrument_id=1&interval=1h&start_timestamp=1790675505663` |
| `mark_history.json` | `https://api.perpetuals.polymarket.com/v1/info/mark-history?instrument_id=1&interval=1h&start_timestamp=1790675505663` |
| `bbo.json` | `https://api.perpetuals.polymarket.com/v1/info/bbo?instrument_id=1` |
| `book.json` | `https://api.perpetuals.polymarket.com/v1/info/book?instrument_id=1&depth=10` |
| `trades.json` | `https://api.perpetuals.polymarket.com/v1/info/trades?instrument_id=1` |
| `funding.json` | `https://api.perpetuals.polymarket.com/v1/info/funding?instrument_id=1` |
| `index.json` | `https://api.perpetuals.polymarket.com/v1/info/index?asset=SP500` |
| `leaderboard.json` | `https://api.perpetuals.polymarket.com/v1/info/leaderboard?window=week&limit=3` |
| `leaderboard_account.json` | `https://api.perpetuals.polymarket.com/v1/info/leaderboard?window=week&limit=1&address=0x65c874d474532F4f5Db9eE02F1290f3034E78d73` |
| `portfolio.json` | `https://api.perpetuals.polymarket.com/v1/info/portfolio?address=0x65c874d474532F4f5Db9eE02F1290f3034E78d73` |
| `position_fills.json` | `https://api.perpetuals.polymarket.com/v1/info/position-fills?address=0x65c874d474532F4f5Db9eE02F1290f3034E78d73&instrument_id=57` |
| `invite.json` | `https://api.perpetuals.polymarket.com/v1/info/invite?code=polyoxide-fixture` |
