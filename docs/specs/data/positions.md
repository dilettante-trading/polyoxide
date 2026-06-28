# Data API Positions

Base URL: `https://data-api.polymarket.com`

## Get Positions

`GET /positions`

Returns open positions for a user.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |
| market | query | Hash64[] (`0x` + 64 hex) | no | — | Filter by condition ID(s) |
| eventId | query | integer[] | no | — | Filter by event ID(s) |
| sizeThreshold | query | number | no | 1 | Minimum position size |
| redeemable | query | boolean | no | — | Filter redeemable positions |
| mergeable | query | boolean | no | — | Filter mergeable positions |
| limit | query | integer (0-500) | no | 100 | Results per page |
| offset | query | integer (0-10000) | no | 0 | Pagination offset |
| sortBy | query | string | no | TOKENS | CURRENT, INITIAL, TOKENS, CASHPNL, PERCENTPNL, TITLE, RESOLVING, PRICE, AVGPRICE |
| sortDirection | query | string | no | DESC | ASC or DESC |
| title | query | string (max 100) | no | — | Filter by market title substring |

**Response:** `Position[]`

```json
[{
  "proxyWallet": "string",
  "asset": "string",
  "conditionId": "string",
  "size": 0,
  "avgPrice": 0,
  "initialValue": 0,
  "currentValue": 0,
  "cashPnl": 0,
  "percentPnl": 0,
  "realizedPnl": 0,
  "curPrice": 0,
  "redeemable": false,
  "mergeable": false,
  "title": "string",
  "slug": "string",
  "outcome": "string",
  "outcomeIndex": 0,
  "negativeRisk": false
}]
```

## Get Closed Positions

`GET /closed-positions`

Returns closed (resolved or sold) positions for a user.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |
| market | query | Hash64[] (`0x` + 64 hex) | no | — | Filter by condition ID(s) |
| title | query | string (max 100) | no | — | Filter by market title substring |
| eventId | query | integer[] | no | — | Filter by event ID(s) |
| limit | query | integer (0-50) | no | 10 | Results per page |
| offset | query | integer (0-100000) | no | 0 | Pagination offset |
| sortBy | query | string | no | REALIZEDPNL | REALIZEDPNL, TITLE, PRICE, AVGPRICE, TIMESTAMP |
| sortDirection | query | string | no | DESC | ASC or DESC |

**Response:** `ClosedPosition[]`

```json
[{
  "proxyWallet": "string",
  "asset": "string",
  "conditionId": "string",
  "avgPrice": 0,
  "totalBought": 0,
  "realizedPnl": 0,
  "curPrice": 0,
  "timestamp": 0,
  "title": "string",
  "slug": "string",
  "icon": "string",
  "eventSlug": "string",
  "outcome": "string",
  "outcomeIndex": 0,
  "oppositeOutcome": "string",
  "oppositeAsset": "string",
  "endDate": "string"
}]
```

Note: `ClosedPosition` shares `avgPrice`, `totalBought`, `realizedPnl`, and `curPrice` with `Position` and adds `timestamp` (int64). Relative to `Position` it lacks `size`, `initialValue`, `currentValue`, `cashPnl`, `percentPnl`, `percentRealizedPnl`, `redeemable`, `mergeable`, and `negativeRisk`.

## Get Portfolio Value

`GET /value`

Returns portfolio value breakdown for a user.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |
| market | query | Hash64[] (`0x` + 64 hex) | no | — | Filter by condition ID(s) |

**Response:** `Value[]`

## Get Traded Status

`GET /traded`

Returns whether a user has ever traded.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |

**Response:** `Traded`

## Get Activity

`GET /activity`

Returns user activity (trades, splits, merges, redemptions, rewards, conversions, deposits, withdrawals, yield, maker rebates, taker rebates, referral rewards).

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |
| market | query | Hash64[] (`0x` + 64 hex) | no | — | Filter by condition ID(s) |
| eventId | query | integer[] | no | — | Filter by event ID(s) |
| type | query | string[] | no | — | TRADE, SPLIT, MERGE, REDEEM, REWARD, CONVERSION, DEPOSIT, WITHDRAWAL, YIELD, MAKER_REBATE, TAKER_REBATE, REFERRAL_REWARD |
| start | query | integer | no | — | Start timestamp (Unix) |
| end | query | integer | no | — | End timestamp (Unix) |
| limit | query | integer (0-500) | no | 100 | Results per page |
| offset | query | integer (0-10000) | no | 0 | Pagination offset |
| sortBy | query | string | no | TIMESTAMP | TIMESTAMP, TOKENS, CASH |
| sortDirection | query | string | no | DESC | ASC or DESC |
| side | query | string | no | — | BUY or SELL |

**Response:** `Activity[]`

## Get Market Positions

`GET /v1/market-positions`

Returns all positions for a specific market (across all users).

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| market | query | Hash64 (`0x` + 64 hex) | yes | — | Condition ID |
| user | query | Address (`0x` + 40 hex) | no | — | Filter by user address |
| status | query | string | no | — | OPEN, CLOSED, ALL |
| sortBy | query | string | no | TOTAL_PNL | TOKENS, CASH_PNL, REALIZED_PNL, TOTAL_PNL |
| sortDirection | query | string | no | DESC | ASC or DESC |
| limit | query | integer (0-500) | no | 50 | Results per page |
| offset | query | integer (0-10000) | no | 0 | Pagination offset |

**Response:** `MetaMarketPositionV1[]`

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
# Get positions for a known user
curl -s 'https://data-api.polymarket.com/positions?user=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604&limit=1' | jq '.[0] | {asset, title, size, curPrice}'
```
