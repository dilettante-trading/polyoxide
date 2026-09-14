# Data API v2 fixtures

Captured 2026-09-14T10:25:00Z by `scripts/capture_v2_fixtures.py` from the live host.
Each file is the complete response body, pretty-printed. Inputs were chosen live
(see the script), so the wallets and markets here are whatever was active then.

| Fixture | Request |
|---------|---------|
| `trades.json` | `https://data-api.polymarket.com/v2/trades?limit=2` |
| `positions.json` | `https://data-api.polymarket.com/v2/positions?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a&limit=2` |
| `positions_closed.json` | `https://data-api.polymarket.com/v2/positions?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a&status=CLOSED&limit=2` |
| `activity.json` | `https://data-api.polymarket.com/v2/activity?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a&limit=2` |
| `activity_tips.json` | `https://data-api.polymarket.com/v2/activity?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a&type=TRADE%2CTIP&limit=2` |
| `approvals.json` | `https://data-api.polymarket.com/v2/approvals?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a` |
| `user_pnl.json` | `https://data-api.polymarket.com/v2/user-pnl?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a&interval=1w&fidelity=1d` |
| `user_stats.json` | `https://data-api.polymarket.com/v2/user-stats?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a` |
| `user_stats_unknown.json` | `https://data-api.polymarket.com/v2/user-stats?user=0xfa53405fa12212476204b416af94905a8e5c4a07` |
| `user_volume.json` | `https://data-api.polymarket.com/v2/user-volume?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a` |
| `value.json` | `https://data-api.polymarket.com/v2/value?user=0x379a410a36c101e3b8dfb6c90d1176eb0def4b1a` |
| `holders.json` | `https://data-api.polymarket.com/v2/holders?condition=0x94ad92377199e93e51512d57cc5d5c8e360cad52ff1559ebe190e94530464f6b&limit=2` |
| `holders_pnl.json` | `https://data-api.polymarket.com/v2/holders?condition=0x94ad92377199e93e51512d57cc5d5c8e360cad52ff1559ebe190e94530464f6b&include_pnl=true&limit=2` |
| `live_volume.json` | `https://data-api.polymarket.com/v2/live-volume?event_id=1008212` |
| `open_interest.json` | `https://data-api.polymarket.com/v2/oi?condition=0x94ad92377199e93e51512d57cc5d5c8e360cad52ff1559ebe190e94530464f6b` |
| `open_interest_global.json` | `https://data-api.polymarket.com/v2/oi` |
| `prices_history.json` | `https://data-api.polymarket.com/v2/prices-history?token_id=107562952936555819668737826428754978979556920505990557747611348541829551199040&interval=1d&limit=2` |
| `biggest_winners.json` | `https://data-api.polymarket.com/v2/biggest-winners?time_period=week&limit=2` |
| `resolutions.json` | `https://data-api.polymarket.com/v2/resolutions?condition=0x789f0872f66cfffd21a33020e5c90e11f95f947e03be77ac2df7e86b0cb71527` |
| `biggest_winners_combos.json` | `https://data-api.polymarket.com/v2/biggest-winners?category=combos&time_period=all&limit=2` |
| `combo_positions.json` | `https://data-api.polymarket.com/v2/positions/combos?user=0x87746b484db725beb8e79d7ce188261b7c75baf4&limit=2` |
| `combo_activity.json` | `https://data-api.polymarket.com/v2/activity/combos?user=0x87746b484db725beb8e79d7ce188261b7c75baf4&limit=2` |
| `builders_leaderboard.json` | `https://data-api.polymarket.com/v2/builders/leaderboard?time_period=week&limit=2` |
| `builder_volume.json` | `https://data-api.polymarket.com/v2/builders/volume?interval=week&limit=2` |
| `leaderboard.json` | `https://data-api.polymarket.com/v2/leaderboard?time_period=week&limit=2` |
| `leaderboard_user.json` | `https://data-api.polymarket.com/v2/leaderboard?user=0x168a51f1cac0ad3b166797069382495a8aa10d24&time_period=week` |
| `leaderboard_user_unknown.json` | `https://data-api.polymarket.com/v2/leaderboard?user=0xfa53405fa12212476204b416af94905a8e5c4a07&time_period=week` |
| `status.json` | `https://data-api.polymarket.com/v2/status` |
