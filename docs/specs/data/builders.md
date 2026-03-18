# Data API Builders

Base URL: `https://data-api.polymarket.com`

## Get Builder Leaderboard

`GET /v1/builders/leaderboard`

Returns the builder leaderboard ranked by volume.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| timePeriod | query | string | no | — | DAY, WEEK, MONTH, ALL |
| limit | query | integer (0-50) | no | — | Results per page |
| offset | query | integer (0-1000) | no | 0 | Pagination offset |

**Response:** `LeaderboardEntry[]`

```json
[{
  "rank": 1,
  "builder": "string",
  "volume": 0,
  "activeUsers": 0
}]
```

## Get Builder Volume

`GET /v1/builders/volume`

Returns aggregate builder volume data.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| timePeriod | query | string | no | — | DAY, WEEK, MONTH, ALL |

**Response:** `BuilderVolumeEntry[]`

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (out-of-range values, invalid timePeriod) |
| 401 | Unauthorized |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get top 3 builders
curl -s 'https://data-api.polymarket.com/v1/builders/leaderboard?limit=3' | jq '.[0]'
```
