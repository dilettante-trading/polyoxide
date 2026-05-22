# Data API Holders

Base URL: `https://data-api.polymarket.com`

## Get Holders

`GET /holders`

Returns top token holders for a market.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| market | query | Hash64[] (`0x` + 64 hex) | yes | — | Condition ID(s) |
| limit | query | integer (0-20) | no | 20 | Results per page |
| minBalance | query | integer (0-999999) | no | 1 | Minimum token balance |

**Response:** `MetaHolder[]`

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (missing required fields, out-of-range values) |
| 401 | Unauthorized |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get top holders for a market (replace with a valid condition ID)
curl -s 'https://data-api.polymarket.com/holders?market=0x...&limit=3' | jq .
```
