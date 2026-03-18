# CLOB Market Data

Base URL: `https://clob.polymarket.com`

## Get Server Time

`GET /time`

Returns current server UNIX timestamp.

**Auth:** None

**Response:**

```json
1234567890
```

## Get Order Book

`GET /book`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | yes | Token ID |

**Response:** `OrderBookSummary`

```json
{
  "market": "string — condition ID",
  "asset_id": "string — token ID",
  "timestamp": "string",
  "hash": "string",
  "bids": [{"price": "string", "size": "string"}],
  "asks": [{"price": "string", "size": "string"}],
  "min_order_size": "string",
  "tick_size": "string",
  "neg_risk": true,
  "last_trade_price": "string"
}
```

## Get Order Books (Query)

`GET /books`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_ids | query | string | yes | Comma-separated token IDs |

**Response:** Array of `OrderBookSummary`

## Get Order Books (Body)

`POST /books`

**Auth:** None

**Request:** Array of `BookRequest` objects (`{"token_id": "string"}`)

**Response:** Array of `OrderBookSummary`

## Get Market Price

`GET /price`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | yes | Token ID |
| side | query | string | yes | BUY or SELL |

**Response:**

```json
{"price": 0.55}
```

## Get Market Prices (Query)

`GET /prices`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_ids | query | string | yes | Comma-separated token IDs |
| sides | query | string | yes | Comma-separated BUY/SELL |

**Response:** Object mapping token_id to `{side: price}` map

## Get Market Prices (Body)

`POST /prices`

**Auth:** None

**Request:** Array of `BookRequest` with `token_id` and `side`

**Response:** Object mapping token_id to `{side: price}` map

## Get Midpoint

`GET /midpoint`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | yes | Token ID |

**Response:**

```json
{"mid_price": "0.55"}
```

## Get Midpoints (Query)

`GET /midpoints`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_ids | query | string | yes | Comma-separated token IDs |

**Response:** Object mapping token_id to price string

## Get Midpoints (Body)

`POST /midpoints`

**Auth:** None

**Request:** Array of `BookRequest`

**Response:** Object mapping token_id to price string

## Get Spread

`GET /spread`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | yes | Token ID |

**Response:**

```json
{"spread": "0.02"}
```

## Get Spreads

`POST /spreads`

**Auth:** None

**Request:** Array of `BookRequest`

**Response:** Object mapping token_id to spread string

## Get Last Trade Price

`GET /last-trade-price`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | yes | Token ID |

**Response:**

```json
{"price": "0.55", "side": "BUY"}
```

## Get Last Trade Prices (Query)

`GET /last-trades-prices`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_ids | query | string | yes | Comma-separated (max 500) |

**Response:** Array of `{"token_id": "string", "price": "string", "side": "string"}`

## Get Last Trade Prices (Body)

`POST /last-trades-prices`

**Auth:** None

**Request:** Array of `BookRequest` (max 500)

**Response:** Array of `{"token_id": "string", "price": "string", "side": "string"}`

## Get Fee Rate (Query)

`GET /fee-rate`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | no | Token ID |

**Response:** `{"base_fee": "string"}`

## Get Fee Rate (Path)

`GET /fee-rate/{token_id}`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | path | string | yes | Token ID |

**Response:** `{"base_fee": "string"}`

## Get Tick Size (Query)

`GET /tick-size`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | no | Token ID |

**Response:** `{"minimum_tick_size": "string"}`

## Get Tick Size (Path)

`GET /tick-size/{token_id}`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | path | string | yes | Token ID |

**Response:** `{"minimum_tick_size": "string"}`

## Get Neg Risk (Query)

`GET /neg-risk`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | query | string | no | Token ID |

**Response:** `{"neg_risk": true}`

## Get Neg Risk (Path)

`GET /neg-risk/{token_id}`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| token_id | path | string | yes | Token ID |

**Response:** `{"neg_risk": true}`

## Get Price History

`GET /prices-history`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| market | query | string | yes | Asset ID |
| startTs | query | integer | no | UNIX timestamp |
| endTs | query | integer | no | UNIX timestamp |
| interval | query | string | no | max, all, 1m, 1w, 1d, 6h, 1h |
| fidelity | query | integer | no | Minutes (default 1) |

**Response:** `PricesHistoryResponse`

## Get Simplified Markets

`GET /simplified-markets`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| next_cursor | query | string | no | Pagination cursor |

**Response:** `PaginatedSimplifiedMarkets`

## Get Sampling Markets

`GET /sampling-markets`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| next_cursor | query | string | no | Pagination cursor |

**Response:** `PaginatedMarkets`

## Get Sampling Simplified Markets

`GET /sampling-simplified-markets`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| next_cursor | query | string | no | Pagination cursor |

**Response:** `PaginatedSimplifiedMarkets`

## Get Live Activity Markets

`POST /markets/live-activity`

**Auth:** None

**Request:** Array of condition ID strings

**Response:** Array of `LiveActivityMarket`

## Get Single Live Activity Market

`GET /markets/live-activity/{condition_id}`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| condition_id | path | string | yes | Condition ID |

**Response:** `LiveActivityMarket`

## Verification

```bash
# Get order book for a token
curl -s 'https://clob.polymarket.com/book?token_id=71321045679252212594626385532706912750332728571942532289631379312455583992563' | jq .

# Get server time
curl -s 'https://clob.polymarket.com/time'

# Get midpoint
curl -s 'https://clob.polymarket.com/midpoint?token_id=71321045679252212594626385532706912750332728571942532289631379312455583992563' | jq .
```
