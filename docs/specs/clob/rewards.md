# CLOB Rewards

Base URL: `https://clob.polymarket.com`

## Get User Earnings by Date

`GET /rewards/user`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| date | query | string | yes | YYYY-MM-DD |
| signature_type | query | integer | no | 0, 1, or 2 |
| maker_address | query | string | no | Ethereum address |
| sponsored | query | boolean | no | Sponsored-only earnings |
| next_cursor | query | string | no | Pagination cursor |

**Response:** `PaginatedUserEarnings`

```json
{
  "limit": 100,
  "count": 1,
  "next_cursor": "string",
  "data": [{
    "date": "string — ISO-8601",
    "condition_id": "string",
    "asset_address": "string",
    "maker_address": "string",
    "earnings": 0.0,
    "asset_rate": 0.0
  }]
}
```

## Get Total Earnings by Date

`GET /rewards/user/total`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| date | query | string | yes | YYYY-MM-DD |
| signature_type | query | integer | no | 0, 1, or 2 |
| maker_address | query | string | no | Ethereum address |
| sponsored | query | boolean | no | Aggregate sponsored |

**Response:** Array of earnings objects

```json
[{
  "date": "string",
  "asset_address": "string",
  "maker_address": "string",
  "earnings": 0.0,
  "asset_rate": 0.0
}]
```

## Get Reward Percentages

`GET /rewards/user/percentages`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| signature_type | query | integer | no | 0, 1, or 2 |
| maker_address | query | string | no | Ethereum address |

**Response:** Map of condition_id to percentage

```json
{"0x296ea...": 20, "0xbd31d...": 20}
```

## Get User Earnings and Markets

`GET /rewards/user/markets`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| date | query | string | no | YYYY-MM-DD (default: today) |
| signature_type | query | integer | no | 0, 1, or 2 |
| maker_address | query | string | no | Ethereum address |
| sponsored | query | boolean | no | Sponsored earnings |
| next_cursor | query | string | no | Pagination |
| page_size | query | integer | no | Default 100, max 500 |
| q | query | string | no | Search question/description |
| tag_slug | query | string | no | Filter by tag |
| order_by | query | string | no | Sort field |
| position | query | string | no | ASC or DESC |

**Response:** `PaginatedUserRewardsMarkets`

## Get Active Reward Configurations

`GET /rewards/markets/current`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| sponsored | query | boolean | no | Default false |
| next_cursor | query | string | no | Pagination |

**Response:** `PaginatedCurrentReward`

```json
{
  "limit": 500,
  "count": 1,
  "next_cursor": "string",
  "data": [{
    "condition_id": "string",
    "rewards_max_spread": 0.0,
    "rewards_min_size": 0.0,
    "rewards_config": [],
    "native_daily_rate": 0.0,
    "total_daily_rate": 0.0
  }]
}
```

## Get Market Rewards

`GET /rewards/markets/{condition_id}`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| condition_id | path | string | yes | Condition ID |
| sponsored | query | boolean | no | Default false |
| next_cursor | query | string | no | Pagination |

**Response:** Paginated market reward details

## Get Multiple Markets with Rewards

`GET /rewards/markets/multi`

**Auth:** None

Supports extensive filtering: `q`, `tag_slug`, `event_id`, `order_by`, `position`, volume/spread/price min/max, `next_cursor`, `page_size`.

**Response:** `PaginatedMultiMarketInfo`

## Get Current Rebated Fees

`GET /rebates/current`

**Auth:** None

Returns the current rebated fees for a maker address on a given date.

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| date | query | string | yes | YYYY-MM-DD |
| maker_address | query | string | yes | Ethereum address |

**Response:** Array of `RebatedFees`

```json
[{
  "date": "2026-02-27",
  "condition_id": "string",
  "asset_address": "string",
  "maker_address": "string",
  "rebated_fees_usdc": "0.237519"
}]
```

## Verification

```bash
# Get active reward configs (public)
curl -s 'https://clob.polymarket.com/rewards/markets/current?next_cursor=MA==' | jq '.data[0]'
```
