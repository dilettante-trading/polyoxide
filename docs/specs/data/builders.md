# Data API Builders

Base URL: `https://data-api.polymarket.com`

## Get Builder Leaderboard

`GET /v1/builders/leaderboard`

Returns the builder leaderboard ranked by volume.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| timePeriod | query | string | no | DAY | DAY, WEEK, MONTH, ALL |
| limit | query | integer (0-50) | no | 25 | Results per page |

> `builderCode` is the builder's onchain attribution code attached to orders (CLOB V2). Legacy builders without a registered code return an empty string.
| offset | query | integer (0-1000) | no | 0 | Pagination offset |

**Response:** `LeaderboardEntry[]`

```json
[{
  "rank": "string",
  "builder": "string",
  "builderCode": "string",
  "volume": 0,
  "activeUsers": 0,
  "verified": false,
  "builderLogo": "string"
}]
```

## Get Builder Volume

`GET /v1/builders/volume`

Returns aggregate builder volume data.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| timePeriod | query | string | no | DAY | DAY, WEEK, MONTH, ALL |

**Response:** `BuilderVolumeEntry[]`

```json
[{
  "dt": "2025-11-15T00:00:00Z",
  "builder": "string",
  "builderCode": "string",
  "builderLogo": "string",
  "verified": false,
  "volume": 0,
  "activeUsers": 0,
  "rank": "string"
}]
```

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
