# Data API Leaderboard

Base URL: `https://data-api.polymarket.com`

## Get Leaderboard

`GET /v1/leaderboard`

Returns the trader leaderboard ranked by PnL or volume.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| category | query | string | no | OVERALL | OVERALL, POLITICS, SPORTS, CRYPTO, CULTURE, MENTIONS, WEATHER, ECONOMICS, TECH, FINANCE |
| timePeriod | query | string | no | DAY | DAY, WEEK, MONTH, ALL |
| orderBy | query | string | no | PNL | PNL or VOL |
| limit | query | integer (1-50) | no | 25 | Results per page |
| offset | query | integer (0-1000) | no | 0 | Pagination offset |
| user | query | Address (`0x` + 40 hex) | no | — | Filter by user address |
| userName | query | string | no | — | Filter by username |

**Response:** `TraderLeaderboardEntry[]`

```json
[{
  "rank": "string",
  "proxyWallet": "string",
  "userName": "string",
  "vol": 0,
  "pnl": 0,
  "profileImage": "string",
  "xUsername": "string",
  "verifiedBadge": false
}]
```

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (out-of-range values, invalid category/timePeriod) |
| 401 | Unauthorized |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get top 3 traders by PnL today
curl -s 'https://data-api.polymarket.com/v1/leaderboard?limit=3' | jq '.[0]'
```
