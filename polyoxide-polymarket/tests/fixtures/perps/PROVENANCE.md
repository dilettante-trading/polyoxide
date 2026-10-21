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
| `portfolio.json` | `https://api.perpetuals.polymarket.com/v1/info/portfolio?address=0x65c874d474532F4f5Db9eE02F1290f3034E78d73` |
| `position_fills.json` | `https://api.perpetuals.polymarket.com/v1/info/position-fills?address=0x65c874d474532F4f5Db9eE02F1290f3034E78d73&instrument_id=57` |
| `invite.json` | `https://api.perpetuals.polymarket.com/v1/info/invite?code=polyoxide-fixture` |

## Captured 2026-10-21T16:02:56Z (drift #56)

Two leaderboard captures were replaced, since the 2026-09-30 ones predate the
now-required `roi`, and five fixtures were added for the routes and fields the drift
adopted.

| Fixture | Request |
|---------|---------|
| `leaderboard.json` | `https://api.perpetuals.polymarket.com/v1/info/leaderboard?window=week&limit=3` |
| `leaderboard_account.json` | `https://api.perpetuals.polymarket.com/v1/info/leaderboard?window=week&limit=1&address=0xE0Ebcff994a0f3A644634927aE1B68913ad58A39` |
| `registered.json` | `https://api.perpetuals.polymarket.com/v1/info/registered?address=0xE0Ebcff994a0f3A644634927aE1B68913ad58A39` |
| `position_snapshots.json` | `POST https://api.perpetuals.polymarket.com/v1/info/position-snapshots {"address": "0xE0Ebcff994a0f3A644634927aE1B68913ad58A39", "active_instrument_ids": [2], "history_fills": [{"instrument_id": 2, "trade_id": "2567214057346220", "timestamp": 1792360881197}, {"instrument_id": 2, "trade_id": "322977361449535", "timestamp": 1792316100035}, {"instrument_id": 2, "trade_id": "5353753338467265", "timestamp": 1792189183221}, {"instrument_id": 2, "trade_id": "2520467105731526", "timestamp": 1792189182921}]}` |
| `instrument_settled.json` | `https://api.perpetuals.polymarket.com/v1/info/instruments?instrument_id=51` |
| `competitions.json` | `https://api.perpetuals.polymarket.com/v1/info/competitions?address=0xE0Ebcff994a0f3A644634927aE1B68913ad58A39` |

No competition existed at capture time: every competition id answered 404, and
the widget held only its timestamp,
so these were written by hand from `docs/specs/perps/openapi.json` and kept:

- `competition.json`
- `competition_leaderboard.json`
- `competition_account.json`
- `competition_widget.json`
