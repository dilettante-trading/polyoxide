# Data API

Base URL: `https://data-api.polymarket.com`

Read-only API for user positions, trades, activity, leaderboard, and market analytics. No authentication required.

Machine-readable schema: [openapi.yaml](openapi.yaml) (mirror of `https://docs.polymarket.com/api-spec/data-openapi.yaml`).

## Auth

No authentication required for any endpoint.

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [positions.md](positions.md) | /positions, /closed-positions, /value, /traded, /activity, /v1/market-positions | None |
| [combos.md](combos.md) | /v1/positions/combos, /v1/activity/combos | None |
| [trades.md](trades.md) | /trades | None |
| [holders.md](holders.md) | /holders | None |
| [open-interest.md](open-interest.md) | /oi | None |
| [live-volume.md](live-volume.md) | /live-volume | None |
| [leaderboard.md](leaderboard.md) | /v1/leaderboard | None |
| [builders.md](builders.md) | /v1/builders/leaderboard, /v1/builders/volume | None |
| [accounting.md](accounting.md) | /v1/accounting/snapshot | None |
| [misc.md](misc.md) | /other, /revisions | None |

## Rate Limits

See [rate-limits.md](rate-limits.md) for all limits.
