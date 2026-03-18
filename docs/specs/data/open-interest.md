# Data API Open Interest

Base URL: `https://data-api.polymarket.com`

## Get Open Interest

`GET /oi`

Returns open interest data for markets.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| market | query | Hash64[] (`0x` + 64 hex) | no | — | Filter by condition ID(s) |

**Response:** `OpenInterest[]`

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (out-of-range values) |
| 401 | Unauthorized |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get open interest across all markets
curl -s 'https://data-api.polymarket.com/oi' | jq '.[0]'
```
