# Data API Trades

Base URL: `https://data-api.polymarket.com`

## Get Trades

`GET /trades`

Returns recent trades across the platform.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| limit | query | integer (0-10000) | no | 100 | Results per page |
| offset | query | integer (0-10000) | no | 0 | Pagination offset |
| takerOnly | query | boolean | no | true | Return only taker trades |
| filterType | query | string | no | — | CASH or TOKENS |
| filterAmount | query | number | no | — | Minimum amount for filterType |
| market | query | Hash64[] (`0x` + 64 hex) | no | — | Filter by condition ID(s) |
| eventId | query | integer[] | no | — | Filter by event ID(s) |
| user | query | Address (`0x` + 40 hex) | no | — | Filter by user address |
| side | query | string | no | — | BUY or SELL |

**Response:** `Trade[]`

```json
[{
  "proxyWallet": "string",
  "side": "BUY",
  "asset": "string",
  "conditionId": "string",
  "size": 0,
  "price": 0,
  "timestamp": "string",
  "title": "string",
  "slug": "string",
  "outcome": "string",
  "outcomeIndex": 0,
  "transactionHash": "string"
}]
```

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
# Get the most recent trade
curl -s 'https://data-api.polymarket.com/trades?limit=1' | jq '.[0] | {asset, side, size, price}'
```
