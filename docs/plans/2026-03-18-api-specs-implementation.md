# API Specs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create `docs/specs/` with upstream Polymarket API documentation as a source of truth for Claude.

**Architecture:** Nested Markdown files organized by API (clob, gamma, data, relay), each with an INDEX.md and per-topic files. Content sourced from Polymarket's OpenAPI specs and docs site.

**Tech Stack:** Markdown files, curl for verification commands

---

### Task 1: Scaffold directory structure and top-level INDEX

**Files:**
- Create: `docs/specs/INDEX.md`
- Create: `docs/specs/clob/` (empty dir)
- Create: `docs/specs/gamma/` (empty dir)
- Create: `docs/specs/data/` (empty dir)
- Create: `docs/specs/relay/` (empty dir)

**Step 1: Create directories**

```bash
mkdir -p docs/specs/clob docs/specs/gamma docs/specs/data docs/specs/relay
```

**Step 2: Write top-level INDEX.md**

```markdown
# Polymarket API Specs

Upstream API documentation for Claude. Source of truth for endpoint contracts,
rate limits, auth schemes, and response schemas as documented by Polymarket.

These specs are sourced from https://docs.polymarket.com and the OpenAPI specs at:
- CLOB: https://docs.polymarket.com/api-spec/clob-openapi.yaml
- Gamma: https://docs.polymarket.com/api-spec/gamma-openapi.yaml
- Data: https://docs.polymarket.com/api-spec/data-openapi.yaml
- Relay: https://docs.polymarket.com/api-spec/relayer-openapi.yaml

## APIs

| API | Base URL | Description |
|-----|----------|-------------|
| [CLOB](clob/INDEX.md) | `https://clob.polymarket.com` | Order book trading, market data, rewards, RFQ |
| [Gamma](gamma/INDEX.md) | `https://gamma-api.polymarket.com` | Market/event metadata, search, comments |
| [Data](data/INDEX.md) | `https://data-api.polymarket.com` | User positions, trades, leaderboard |
| [Relay](relay/INDEX.md) | `https://relayer-v2.polymarket.com` | Gasless relay transactions |
```

**Step 3: Commit**

```bash
git add docs/specs/INDEX.md
git commit -m "docs: scaffold docs/specs directory with top-level INDEX"
```

---

### Task 2: CLOB rate-limits.md

**Files:**
- Create: `docs/specs/clob/rate-limits.md`

**Step 1: Write rate-limits.md**

Source: https://docs.polymarket.com/api-reference/rate-limits

```markdown
# CLOB Rate Limits

Base URL: `https://clob.polymarket.com`

Enforcement: sliding time window. Requests are throttled (delayed/queued) rather than immediately rejected when limits are exceeded.

## General

| Limit | Window |
|-------|--------|
| 9,000 requests | 10 seconds |

## Market Data

| Endpoint | Limit | Window |
|----------|-------|--------|
| `GET /book` | 1,500 | 10s |
| `GET /books` | 500 | 10s |
| `GET /price` | 1,500 | 10s |
| `GET /prices` | 500 | 10s |
| `GET /midpoint` | 1,500 | 10s |
| `GET /midpoints` | 500 | 10s |
| `GET /prices-history` | 1,000 | 10s |
| `GET /tick-size` | 200 | 10s |

## Trading (Dual Limits)

| Endpoint | Burst | Burst Window | Sustained | Sustained Window |
|----------|-------|--------------|-----------|------------------|
| `POST /order` | 3,500 | 10s | 36,000 | 10 min |
| `DELETE /order` | 3,000 | 10s | 30,000 | 10 min |
| `POST /orders` | 1,000 | 10s | 15,000 | 10 min |
| `DELETE /orders` | 1,000 | 10s | 15,000 | 10 min |
| `DELETE /cancel-all` | 250 | 10s | 6,000 | 10 min |
| `DELETE /cancel-market-orders` | 1,000 | 10s | 1,500 | 10 min |

## Ledger

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/trades`, `/orders`, `/notifications`, `/order` | 900 | 10s |
| `/data/orders` | 500 | 10s |
| `/data/trades` | 500 | 10s |
| `/notifications` | 125 | 10s |

## Account

| Endpoint | Limit | Window |
|----------|-------|--------|
| `GET /balance-allowance` | 200 | 10s |
| `PUT /balance-allowance` | 50 | 10s |

## Auth

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/auth/*` | 100 | 10s |
```

**Step 2: Commit**

```bash
git add docs/specs/clob/rate-limits.md
git commit -m "docs: add CLOB rate limits spec"
```

---

### Task 3: CLOB auth.md

**Files:**
- Create: `docs/specs/clob/auth.md`

**Step 1: Write auth.md**

Source: https://docs.polymarket.com/api-reference/authentication

```markdown
# CLOB Authentication

Base URL: `https://clob.polymarket.com`

## L1 Authentication (Private Key)

Used for: creating API credentials, deriving existing credentials, locally signing orders.

Signs an EIP-712 message containing: address, timestamp, nonce, and the message "This message attests that I control the given wallet."

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_ADDRESS` | Polygon signer address |
| `POLY_SIGNATURE` | EIP-712 signature |
| `POLY_TIMESTAMP` | Current UNIX timestamp |
| `POLY_NONCE` | Nonce (default: 0) |

## L2 Authentication (API Credentials)

Used for: canceling/retrieving orders, checking balances, posting signed orders.

Generated from L1 auth. Uses HMAC-SHA256 signing with the credential `secret` as key.

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_ADDRESS` | Polygon signer address |
| `POLY_SIGNATURE` | HMAC-SHA256 signature |
| `POLY_TIMESTAMP` | Current UNIX timestamp |
| `POLY_API_KEY` | API key value |
| `POLY_PASSPHRASE` | Passphrase value |

## Builder Authentication

Used for: order attribution to builder accounts.

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_BUILDER_API_KEY` | Builder API key |
| `POLY_BUILDER_PASSPHRASE` | Builder passphrase |
| `POLY_BUILDER_SIGNATURE` | HMAC-SHA256 signature |
| `POLY_BUILDER_TIMESTAMP` | Current UNIX timestamp |

## Signature Types

| Type | Value | Description |
|------|-------|-------------|
| EOA | `0` | Standard Ethereum wallet |
| POLY_PROXY | `1` | Custom proxy (Magic Link users) |
| GNOSIS_SAFE | `2` | Gnosis Safe multisig (most common) |

## Credential Endpoints

- `POST /auth/api-key` — Create new credentials (L1 auth)
- `GET /auth/derive-api-key` — Derive existing credentials (L1 auth)
- `GET /auth/api-keys` — List API keys (L1 auth)
- `DELETE /auth/api-key` — Delete current API key (L2 auth)
- `POST /auth/builder-api-key` — Create builder key (L2 auth)
- `GET /auth/builder-api-key` — List builder keys (L2 auth)
- `DELETE /auth/builder-api-key` — Revoke builder key (Builder auth)
```

**Step 2: Commit**

```bash
git add docs/specs/clob/auth.md
git commit -m "docs: add CLOB authentication spec"
```

---

### Task 4: CLOB markets.md

**Files:**
- Create: `docs/specs/clob/markets.md`

**Step 1: Write markets.md**

Source: CLOB OpenAPI spec — market data endpoints

```markdown
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
```

**Step 2: Commit**

```bash
git add docs/specs/clob/markets.md
git commit -m "docs: add CLOB market data endpoints spec"
```

---

### Task 5: CLOB orders.md

**Files:**
- Create: `docs/specs/clob/orders.md`

**Step 1: Write orders.md**

Source: CLOB OpenAPI spec — trade endpoints

```markdown
# CLOB Orders & Trading

Base URL: `https://clob.polymarket.com`

## Post Single Order

`POST /order`

**Auth:** L2

**Request:** `SendOrder`

```json
{
  "order": {
    "maker": "string — maker address",
    "signer": "string — signer address",
    "taker": "string — taker address (0x0 for any)",
    "tokenId": "string",
    "makerAmount": "string",
    "takerAmount": "string",
    "side": "BUY or SELL",
    "expiration": "string — UNIX timestamp (0 for no expiry)",
    "nonce": "string",
    "feeRateBps": "string",
    "signature": "string",
    "salt": "string",
    "signatureType": 0
  },
  "owner": "string — UUID",
  "orderType": "GTC",
  "deferExec": false
}
```

**Response:** `SendOrderResponse`

```json
{
  "success": true,
  "orderID": "string",
  "status": "live | matched | delayed",
  "makingAmount": "string",
  "takingAmount": "string",
  "transactionHashes": ["string"],
  "tradeIDs": ["string"],
  "errorMsg": "string"
}
```

## Post Multiple Orders

`POST /orders`

**Auth:** L2

**Request:** Array of `SendOrder` (max 15)

**Response:** Array of `SendOrderResponse`

## Get User Orders

`GET /orders`

**Auth:** L2 or Builder

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | query | string | no | Order ID filter |
| market | query | string | no | Condition ID filter |
| asset_id | query | string | no | Token ID filter |
| next_cursor | query | string | no | Pagination cursor |

**Response:** `OrdersResponse`

```json
{
  "limit": 100,
  "next_cursor": "string",
  "count": 1,
  "data": [{
    "id": "string",
    "status": "ORDER_STATUS_LIVE",
    "owner": "string",
    "maker_address": "string",
    "market": "string — condition ID",
    "asset_id": "string — token ID",
    "side": "BUY",
    "original_size": "string",
    "size_matched": "string",
    "price": "string",
    "outcome": "string",
    "expiration": "string",
    "order_type": "GTC",
    "associate_trades": [],
    "created_at": "string"
  }]
}
```

## Get Single Order

`GET /order/{orderID}`

**Auth:** L2 or Builder

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| orderID | path | string | yes | Order ID |

**Response:** `OpenOrder` (same schema as items in `OrdersResponse.data`)

## Cancel Single Order

`DELETE /order`

**Auth:** L2

**Request:**

```json
{"orderID": "string"}
```

**Response:** `CancelOrdersResponse`

```json
{
  "canceled": ["order_id_1"],
  "not_canceled": {"order_id_2": "reason"}
}
```

## Cancel Multiple Orders

`DELETE /orders`

**Auth:** L2

**Request:** Array of order ID strings (max 3,000)

**Response:** `CancelOrdersResponse`

## Cancel All Orders

`DELETE /cancel-all`

**Auth:** L2

**Response:** `CancelOrdersResponse`

## Cancel Market Orders

`DELETE /cancel-market-orders`

**Auth:** L2

**Request:**

```json
{
  "market": "string — condition ID",
  "asset_id": "string — token ID (optional)"
}
```

**Response:** `CancelOrdersResponse`

## Get Trades

`GET /trades`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| next_cursor | query | string | no | Pagination cursor |

**Response:** Paginated trade list

## Get Order Scoring Status

`GET /order-scoring`

**Auth:** L2

**Response:** Order scoring status

## Get Builder Trades

`GET /builder-trades`

**Auth:** Builder

**Response:** Builder trade list

## Verification

```bash
# Get simplified markets (public, no auth needed)
curl -s 'https://clob.polymarket.com/simplified-markets?next_cursor=MA==' | jq '.data[0]'
```
```

**Step 2: Commit**

```bash
git add docs/specs/clob/orders.md
git commit -m "docs: add CLOB orders and trading spec"
```

---

### Task 6: CLOB account.md

**Files:**
- Create: `docs/specs/clob/account.md`

**Step 1: Write account.md**

```markdown
# CLOB Account

Base URL: `https://clob.polymarket.com`

## Get Balance & Allowance

`GET /balance-allowance`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| asset_type | query | string | yes | COLLATERAL or CONDITIONAL |
| token_id | query | string | no | Required for CONDITIONAL assets |
| signature_type | query | integer | no | 0=EOA, 1=POLY_PROXY, 2=GNOSIS_SAFE |

**Response:**

```json
{
  "balance": "string",
  "allowances": {"address": "amount"}
}
```

## Update Balance & Allowance

`PUT /balance-allowance`

**Auth:** L2

Parameters same as GET. Returns empty object.

## Update and Return Balance

`GET /balance-allowance/update`

**Auth:** L2

Parameters same as GET. Returns `BalanceAllowanceResponse`.

## Send Heartbeat

`POST /v1/heartbeats`

**Auth:** L2

Keeps session alive.

## Get Ban Status

`GET /auth/ban-status/closed-only`

**Auth:** L2

**Response:**

```json
{"closed_only": false}
```

## Verification

```bash
# Server time (no auth, confirms API is up)
curl -s 'https://clob.polymarket.com/time'
```
```

**Step 2: Commit**

```bash
git add docs/specs/clob/account.md
git commit -m "docs: add CLOB account endpoints spec"
```

---

### Task 7: CLOB rewards.md

**Files:**
- Create: `docs/specs/clob/rewards.md`

**Step 1: Write rewards.md**

```markdown
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

## Verification

```bash
# Get active reward configs (public)
curl -s 'https://clob.polymarket.com/rewards/markets/current?next_cursor=MA==' | jq '.data[0]'
```
```

**Step 2: Commit**

```bash
git add docs/specs/clob/rewards.md
git commit -m "docs: add CLOB rewards spec"
```

---

### Task 8: CLOB notifications.md and rfq.md

**Files:**
- Create: `docs/specs/clob/notifications.md`
- Create: `docs/specs/clob/rfq.md`

**Step 1: Write notifications.md**

```markdown
# CLOB Notifications

Base URL: `https://clob.polymarket.com`

## Get Notifications

`GET /notifications`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| signature_type | query | integer | yes | 0=EOA, 1=POLY_PROXY, 2=GNOSIS_SAFE |

**Response:** Array of `Notification`

```json
[{
  "id": "string",
  "owner": "string",
  "type": 0,
  "payload": {},
  "timestamp": "string"
}]
```

## Dismiss Notifications

`DELETE /notifications`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| ids | query | string | yes | Comma-separated notification IDs |

**Response:** `"OK"`
```

**Step 2: Write rfq.md**

Note: RFQ endpoints are not documented in Polymarket's public OpenAPI spec. The following is based on observed API behavior.

```markdown
# CLOB RFQ (Request for Quote)

Base URL: `https://clob.polymarket.com`

> Note: RFQ endpoints are not fully documented in Polymarket's public API docs. Schema derived from observed API behavior.

## Create RFQ Request

`POST /rfq/request`

**Auth:** L2

## Cancel RFQ Request

`DELETE /rfq/request`

**Auth:** L2

## Create RFQ Quote

`POST /rfq/quote`

**Auth:** L2

## Cancel RFQ Quote

`DELETE /rfq/quote`

**Auth:** L2

## List Quotes

`GET /rfq/quotes`

**Auth:** L2

## List Requests

`GET /rfq/requests`

**Auth:** L2

## Get RFQ Prices

`GET /rfq/prices`

**Auth:** L2
```

**Step 3: Commit**

```bash
git add docs/specs/clob/notifications.md docs/specs/clob/rfq.md
git commit -m "docs: add CLOB notifications and RFQ specs"
```

---

### Task 9: CLOB websocket.md

**Files:**
- Create: `docs/specs/clob/websocket.md`

**Step 1: Write websocket.md**

Source: https://docs.polymarket.com/market-data/websocket/

```markdown
# CLOB WebSocket

## Endpoints

| Channel | URL | Auth |
|---------|-----|------|
| Market | `wss://ws-subscriptions-clob.polymarket.com/ws/market` | None |
| User | `wss://ws-subscriptions-clob.polymarket.com/ws/user` | L2 credentials |
| Sports | `wss://sports-api.polymarket.com/ws` | None |

## Market Channel

### Subscription

```json
{
  "assets_ids": ["token_id_1", "token_id_2"],
  "type": "market",
  "custom_feature_enabled": true
}
```

Setting `custom_feature_enabled: true` enables `best_bid_ask`, `new_market`, and `market_resolved` events.

### Dynamic Subscription

```json
{"operation": "subscribe", "assets_ids": ["token_id"]}
{"operation": "unsubscribe", "assets_ids": ["token_id"]}
```

### Message Types

| Type | Trigger | Key Fields |
|------|---------|------------|
| `book` | On subscribe + after trades | bids, asks, market, asset_id |
| `price_change` | Order placed/cancelled | asset_id, price, size, side, best_bid, best_ask |
| `tick_size_change` | Price > 0.96 or < 0.04 | old_tick_size, new_tick_size |
| `last_trade_price` | Trade executed | price, side, size, fee_rate_bps |
| `best_bid_ask` | Best price changes | best_bid, best_ask, spread |
| `new_market` | Market created | question, market, tokens, outcomes |
| `market_resolved` | Market resolved | winning_asset_id, winning_outcome |

### Keep-Alive

Send `PING` every 10 seconds. Server responds with `PONG`.

## User Channel

### Subscription

```json
{
  "auth": {
    "apiKey": "key",
    "secret": "secret",
    "passphrase": "passphrase"
  },
  "markets": ["condition_id"],
  "type": "user"
}
```

### Dynamic Subscription

```json
{"operation": "subscribe", "markets": ["condition_id"]}
{"operation": "unsubscribe", "markets": ["condition_id"]}
```

### Message Types

**TRADE** — Triggered by order matches and status changes.

| Field | Type | Description |
|-------|------|-------------|
| asset_id | string | Token ID |
| market | string | Condition ID |
| order_id | string | Order ID |
| side | string | BUY or SELL |
| size | string | Trade size |
| price | string | Trade price |
| status | string | MATCHED, MINED, CONFIRMED, RETRYING, FAILED |
| maker_orders | array | Matched order details |

Status flow: MATCHED → MINED → CONFIRMED (or RETRYING → FAILED)

**ORDER** — Triggered by placements, updates, cancellations.

| Field | Type | Description |
|-------|------|-------------|
| id | string | Order ID |
| type | string | PLACEMENT, UPDATE, CANCELLATION |
| side | string | BUY or SELL |
| price | string | Order price |
| original_size | string | Original size |
| size_matched | string | Filled size |

### Keep-Alive

Send `PING` every 10 seconds. Server responds with `PONG`.

## Sports Channel

Server sends `ping` every 5 seconds. Respond with `pong` within 10 seconds or connection closes.

Message type: `sport_result` (game scores and status).
```

**Step 2: Commit**

```bash
git add docs/specs/clob/websocket.md
git commit -m "docs: add CLOB websocket spec"
```

---

### Task 10: CLOB INDEX.md

**Files:**
- Create: `docs/specs/clob/INDEX.md`

**Step 1: Write INDEX.md**

```markdown
# CLOB API

Base URL: `https://clob.polymarket.com`
Staging URL: `https://clob-staging.polymarket.com`

Order book trading API for Polymarket. Supports market data queries, order placement/cancellation, balance management, liquidity rewards, and real-time WebSocket streams.

## Auth

Three authentication levels. See [auth.md](auth.md) for details.

- **None** — Public market data endpoints
- **L1** — EIP-712 signing for credential creation
- **L2** — HMAC-SHA256 for most authenticated operations
- **Builder** — Separate HMAC keys for builder-attributed orders

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [markets.md](markets.md) | /book, /books, /price, /prices, /midpoint, /midpoints, /spread, /spreads, /last-trade-price, /last-trades-prices, /fee-rate, /tick-size, /neg-risk, /prices-history, /simplified-markets, /sampling-markets, /time | None |
| [orders.md](orders.md) | /order, /orders, /cancel-all, /cancel-market-orders, /trades, /builder-trades | L2/Builder |
| [account.md](account.md) | /balance-allowance, /v1/heartbeats, /auth/ban-status | L2 |
| [rewards.md](rewards.md) | /rewards/user, /rewards/user/total, /rewards/user/percentages, /rewards/user/markets, /rewards/markets/current, /rewards/markets/{id}, /rewards/markets/multi | Mixed |
| [rfq.md](rfq.md) | /rfq/request, /rfq/quote, /rfq/quotes, /rfq/requests, /rfq/prices | L2 |
| [notifications.md](notifications.md) | /notifications | L2 |
| [websocket.md](websocket.md) | ws/market, ws/user, ws/sports | Mixed |

## Rate Limits

See [rate-limits.md](rate-limits.md) for all limits.

## Error Codes

| Code | Meaning |
|------|---------|
| 400 | Invalid parameters, malformed payload, business logic violations |
| 401 | Missing/invalid API key, bad HMAC, expired timestamp |
| 404 | Nonexistent market/order/token |
| 425 | Matching engine restarting — retry with backoff |
| 429 | Rate limit exceeded |
| 500 | Internal server error — retry with backoff |
| 503 | Exchange paused or cancel-only mode |
```

**Step 2: Commit**

```bash
git add docs/specs/clob/INDEX.md
git commit -m "docs: add CLOB INDEX"
```

---

### Task 11: Gamma rate-limits.md and INDEX.md

**Files:**
- Create: `docs/specs/gamma/rate-limits.md`
- Create: `docs/specs/gamma/INDEX.md`

**Step 1: Write rate-limits.md**

```markdown
# Gamma Rate Limits

Base URL: `https://gamma-api.polymarket.com`

Enforcement: sliding time window.

## General

| Limit | Window |
|-------|--------|
| 4,000 requests | 10 seconds |

## Endpoint-Specific

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/events` | 500 | 10s |
| `/public-search` | 350 | 10s |
| `/markets` | 300 | 10s |
| `/comments` | 200 | 10s |
| `/tags` | 200 | 10s |
```

**Step 2: Write INDEX.md**

```markdown
# Gamma API

Base URL: `https://gamma-api.polymarket.com`

Read-only market metadata API. Provides event/market/tag information, search, comments, sports data, and user profiles. No authentication required.

## Auth

No authentication required for any endpoint.

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [markets.md](markets.md) | /markets, /markets/{id}, /markets/slug/{slug}, /markets/{id}/tags | None |
| [events.md](events.md) | /events, /events/{id}, /events/slug/{slug}, /events/{id}/tags | None |
| [series.md](series.md) | /series, /series/{id} | None |
| [tags.md](tags.md) | /tags, /tags/{id}, /tags/slug/{slug}, related-tags | None |
| [sports.md](sports.md) | /sports, /sports/market-types, /teams | None |
| [comments.md](comments.md) | /comments, /comments/{id}, /comments/user_address/{addr} | None |
| [search.md](search.md) | /public-search | None |
| [user.md](user.md) | /public-profile | None |

## Rate Limits

See [rate-limits.md](rate-limits.md) for all limits.
```

**Step 3: Commit**

```bash
git add docs/specs/gamma/rate-limits.md docs/specs/gamma/INDEX.md
git commit -m "docs: add Gamma rate limits and INDEX"
```

---

### Task 12: Gamma endpoint files (markets, events, series, tags, sports, comments, search, user)

**Files:**
- Create: `docs/specs/gamma/markets.md`
- Create: `docs/specs/gamma/events.md`
- Create: `docs/specs/gamma/series.md`
- Create: `docs/specs/gamma/tags.md`
- Create: `docs/specs/gamma/sports.md`
- Create: `docs/specs/gamma/comments.md`
- Create: `docs/specs/gamma/search.md`
- Create: `docs/specs/gamma/user.md`

**Step 1: Write all Gamma endpoint files**

Content for each file follows the standard template. Source from Gamma OpenAPI spec. Key details:

**markets.md:** `GET /markets` (list, with limit/offset/filtering), `GET /markets/{id}`, `GET /markets/slug/{slug}`, `GET /markets/{id}/tags`. Pagination via limit/offset. Filtering: closed, active, tag_id, liquidity_num_min/max, volume_num_min/max, etc.

**events.md:** `GET /events` (list, extensive filtering), `GET /events/{id}`, `GET /events/slug/{slug}`, `GET /events/slug/{slug}/related`, `GET /events/{id}/tags`. Filtering: tag, date range, liquidity, volume.

**series.md:** `GET /series` (list, limit/offset/closed filter), `GET /series/{id}` (optional include_chat).

**tags.md:** `GET /tags` (list), `GET /tags/{id}`, `GET /tags/slug/{slug}`, `GET /tags/{id}/related-tags`, `GET /tags/slug/{slug}/related-tags`, `GET /tags/{id}/related-tags/tags`, `GET /tags/slug/{slug}/related-tags/tags`.

**sports.md:** `GET /sports`, `GET /sports/market-types`, `GET /teams` (with limit/offset, league/name/abbreviation filtering).

**comments.md:** `GET /comments` (list, limit/offset/order/ascending, entity filtering), `GET /comments/{id}`, `GET /comments/user_address/{address}`.

**search.md:** `GET /public-search` with `q` param, multiple filter options.

**user.md:** `GET /public-profile` with `address` query param.

Each file ends with a Verification section containing a representative curl:

```bash
# markets.md
curl -s 'https://gamma-api.polymarket.com/markets?limit=1' | jq '.[0] | {id, question, slug}'

# events.md
curl -s 'https://gamma-api.polymarket.com/events?limit=1' | jq '.[0] | {id, title, slug}'

# tags.md
curl -s 'https://gamma-api.polymarket.com/tags?limit=5' | jq '.[0]'

# search.md
curl -s 'https://gamma-api.polymarket.com/public-search?q=bitcoin&limit=3' | jq .
```

**Step 2: Commit**

```bash
git add docs/specs/gamma/markets.md docs/specs/gamma/events.md docs/specs/gamma/series.md docs/specs/gamma/tags.md docs/specs/gamma/sports.md docs/specs/gamma/comments.md docs/specs/gamma/search.md docs/specs/gamma/user.md
git commit -m "docs: add Gamma endpoint specs"
```

---

### Task 13: Data API rate-limits.md and INDEX.md

**Files:**
- Create: `docs/specs/data/rate-limits.md`
- Create: `docs/specs/data/INDEX.md`

**Step 1: Write rate-limits.md**

```markdown
# Data API Rate Limits

Base URL: `https://data-api.polymarket.com`

Enforcement: sliding time window.

## General

| Limit | Window |
|-------|--------|
| 1,000 requests | 10 seconds |

## Endpoint-Specific

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/trades` | 200 | 10s |
| `/positions` | 150 | 10s |
| `/closed-positions` | 150 | 10s |
| `/` (health) | 100 | 10s |
```

**Step 2: Write INDEX.md**

```markdown
# Data API

Base URL: `https://data-api.polymarket.com`

Read-only API for user positions, trades, activity, leaderboard, and market analytics. No authentication required.

## Auth

No authentication required for any endpoint.

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [positions.md](positions.md) | /positions, /closed-positions, /value, /traded, /activity, /v1/market-positions | None |
| [trades.md](trades.md) | /trades | None |
| [holders.md](holders.md) | /holders | None |
| [open-interest.md](open-interest.md) | /oi | None |
| [live-volume.md](live-volume.md) | /live-volume | None |
| [leaderboard.md](leaderboard.md) | /v1/leaderboard | None |
| [builders.md](builders.md) | /v1/builders/leaderboard, /v1/builders/volume | None |

## Rate Limits

See [rate-limits.md](rate-limits.md) for all limits.
```

**Step 3: Commit**

```bash
git add docs/specs/data/rate-limits.md docs/specs/data/INDEX.md
git commit -m "docs: add Data API rate limits and INDEX"
```

---

### Task 14: Data API endpoint files

**Files:**
- Create: `docs/specs/data/positions.md`
- Create: `docs/specs/data/trades.md`
- Create: `docs/specs/data/holders.md`
- Create: `docs/specs/data/open-interest.md`
- Create: `docs/specs/data/live-volume.md`
- Create: `docs/specs/data/leaderboard.md`
- Create: `docs/specs/data/builders.md`

**Step 1: Write all Data endpoint files**

Source from Data OpenAPI spec. Key details:

**positions.md:** `GET /positions` (user required, market/eventId filtering, limit 0-500, sortBy CURRENT|INITIAL|TOKENS|CASHPNL|PERCENTPNL|TITLE|RESOLVING|PRICE|AVGPRICE), `GET /closed-positions` (user required, limit 0-50, sortBy REALIZEDPNL|TITLE|PRICE|AVGPRICE|TIMESTAMP), `GET /value` (user required), `GET /traded` (user required), `GET /activity` (user required, type TRADE|SPLIT|MERGE|REDEEM|REWARD|CONVERSION|MAKER_REBATE), `GET /v1/market-positions` (market required).

Position response schema:
```json
{
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
}
```

**trades.md:** `GET /trades` (limit 0-10000, offset 0-10000, takerOnly default true, filterType CASH|TOKENS, market/eventId/user/side filtering).

**holders.md:** `GET /holders` (market required, limit 0-20, minBalance default 1).

**open-interest.md:** `GET /oi` (market param).

**live-volume.md:** `GET /live-volume` (id required, integer ≥ 1).

**leaderboard.md:** `GET /v1/leaderboard` (category OVERALL|POLITICS|SPORTS|CRYPTO|CULTURE|MENTIONS|WEATHER|ECONOMICS|TECH|FINANCE, timePeriod DAY|WEEK|MONTH|ALL, orderBy PNL|VOL, limit 1-50, offset 0-1000).

**builders.md:** `GET /v1/builders/leaderboard` (timePeriod, limit, offset), `GET /v1/builders/volume` (timePeriod).

Each file ends with verification curls:

```bash
# positions.md — requires a known user address
curl -s 'https://data-api.polymarket.com/positions?user=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604&limit=1' | jq '.[0] | {asset, title, size, curPrice}'

# trades.md
curl -s 'https://data-api.polymarket.com/trades?limit=1' | jq '.[0] | {asset, side, size, price}'

# leaderboard.md
curl -s 'https://data-api.polymarket.com/v1/leaderboard?limit=3' | jq '.[0]'
```

**Step 2: Commit**

```bash
git add docs/specs/data/positions.md docs/specs/data/trades.md docs/specs/data/holders.md docs/specs/data/open-interest.md docs/specs/data/live-volume.md docs/specs/data/leaderboard.md docs/specs/data/builders.md
git commit -m "docs: add Data API endpoint specs"
```

---

### Task 15: Relay rate-limits.md, auth.md, contracts.md, transactions.md, INDEX.md

**Files:**
- Create: `docs/specs/relay/rate-limits.md`
- Create: `docs/specs/relay/auth.md`
- Create: `docs/specs/relay/contracts.md`
- Create: `docs/specs/relay/transactions.md`
- Create: `docs/specs/relay/INDEX.md`

**Step 1: Write rate-limits.md**

```markdown
# Relay Rate Limits

Base URL: `https://relayer-v2.polymarket.com`

## General

| Limit | Window |
|-------|--------|
| 25 requests | 1 minute |

No endpoint-specific limits. Single global limiter.
```

**Step 2: Write auth.md**

```markdown
# Relay Authentication

Base URL: `https://relayer-v2.polymarket.com`

## Builder API Key Authentication

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_BUILDER_API_KEY` | Builder API key |
| `POLY_BUILDER_TIMESTAMP` | Current UNIX timestamp |
| `POLY_BUILDER_SIGNATURE` | HMAC-SHA256 signature |
| `POLY_BUILDER_PASSPHRASE` | Passphrase (optional) |

Signing uses HMAC-SHA256 with base64url-encoded secret.

## Relayer API Key Authentication

**Headers:**

| Header | Description |
|--------|-------------|
| `RELAYER_API_KEY` | Relayer API key |
| `RELAYER_API_KEY_ADDRESS` | Associated address |
```

**Step 3: Write contracts.md**

```markdown
# Relay Contract Addresses

## Polygon Mainnet (Chain ID: 137)

### Core Trading

| Contract | Address |
|----------|---------|
| CTF Exchange | `0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E` |
| Neg Risk CTF Exchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` |
| Neg Risk Adapter | `0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296` |
| Conditional Tokens (CTF) | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` |
| USDC.e (Bridged USDC) | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` |

### Wallet Factories

| Contract | Address |
|----------|---------|
| Gnosis Safe Factory | `0xaacFeEa03eb1561C4e67d661e40682Bd20E3541b` |
| Safe MultiSend | `0xA238CBeb142c10Ef7Ad8442C6D1f9E89e07e7761` |
| Polymarket Proxy Factory | `0xaB45c5A4B0c941a2F231C04C3f49182e1A254052` |

### Resolution

| Contract | Address |
|----------|---------|
| UMA Adapter | `0x6A9D222616C90FcA5754cd1333cFD9b7fb6a4F74` |
| UMA Optimistic Oracle | `0xCB1822859cEF82Cd2Eb4E6276C7916e692995130` |

### Liquidity

| Contract | Address |
|----------|---------|
| Uniswap v3 USDC.e/USDC Pool | `0xd36ec33c8bed5a9f7b6630855f1533455b98a418` |
```

**Step 4: Write transactions.md**

```markdown
# Relay Transactions

Base URL: `https://relayer-v2.polymarket.com`

## Submit Transaction

`POST /submit`

**Auth:** Builder API Key or Relayer API Key

**Request:**

```json
{
  "from": "string — signer address",
  "to": "string — target contract",
  "proxyWallet": "string — user's proxy wallet",
  "data": "string — encoded calldata",
  "nonce": "string",
  "signature": "string",
  "signatureParams": {
    "gasPrice": "string",
    "operation": 0,
    "safeTxnGas": "string",
    "baseGas": "string",
    "gasToken": "string",
    "refundReceiver": "string"
  },
  "type": "SAFE | PROXY"
}
```

**Response:**

```json
{
  "transactionID": "string",
  "transactionHash": "string",
  "state": "STATE_NEW"
}
```

## Get Transaction

`GET /transaction`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | query | string | yes | Transaction ID |

**Response:** Array of `RelayerTransaction`

```json
[{
  "transactionID": "string",
  "transactionHash": "string",
  "from": "string",
  "to": "string",
  "proxyAddress": "string",
  "data": "string",
  "nonce": "string",
  "state": "STATE_NEW | STATE_EXECUTED | STATE_MINED | STATE_CONFIRMED | STATE_INVALID | STATE_FAILED",
  "type": "SAFE | PROXY",
  "createdAt": "string",
  "updatedAt": "string"
}]
```

## Get Recent Transactions

`GET /transactions`

**Auth:** Builder API Key or Relayer API Key

**Response:** Array of `RelayerTransaction`

## Get Nonce

`GET /nonce`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | Wallet address |
| type | query | string | yes | PROXY or SAFE |

**Response:**

```json
{"nonce": "string"}
```

## Get Relay Payload

`GET /relay-payload`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | Wallet address |
| type | query | string | yes | PROXY or SAFE |

**Response:**

```json
{"address": "string", "nonce": "string"}
```

## Check Safe Deployment

`GET /deployed`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | Safe address |

**Response:**

```json
{"deployed": true}
```

## Get Relayer API Keys

`GET /relayer/api/keys`

**Auth:** Relayer API Key

**Response:** Array of `RelayerApiKey`

```json
[{"apiKey": "string", "address": "string", "createdAt": "string", "updatedAt": "string"}]
```

## Verification

```bash
# Check if an address has a deployed Safe
curl -s 'https://relayer-v2.polymarket.com/deployed?address=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604' | jq .

# Get nonce
curl -s 'https://relayer-v2.polymarket.com/nonce?address=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604&type=SAFE' | jq .
```
```

**Step 5: Write INDEX.md**

```markdown
# Relay API

Base URL: `https://relayer-v2.polymarket.com`

Gasless transaction relay for Polymarket. Submits transactions to Polygon via Safe or Proxy wallets without requiring users to hold MATIC for gas.

## Auth

See [auth.md](auth.md) for details.

- **None** — Nonce lookup, deployment check, transaction status
- **Builder API Key** — Transaction submission, recent transactions
- **Relayer API Key** — Transaction submission, API key management

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [transactions.md](transactions.md) | /submit, /transaction, /transactions, /nonce, /relay-payload, /deployed | Mixed |
| [contracts.md](contracts.md) | Contract addresses (reference, not endpoints) | N/A |

## Rate Limits

See [rate-limits.md](rate-limits.md) — 25 req/min global.
```

**Step 6: Commit**

```bash
git add docs/specs/relay/
git commit -m "docs: add Relay API specs"
```

---

### Task 16: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Add API specs reference**

Add after the "## Environment Variables" section:

```markdown
## API Specs

Upstream Polymarket API documentation lives in `docs/specs/`. See `docs/specs/INDEX.md` for the full index. These are the source of truth for endpoint contracts, rate limits, and response schemas — sourced from https://docs.polymarket.com and the official OpenAPI specs.
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: reference docs/specs in CLAUDE.md"
```

---

### Task 17: Final review and cleanup

**Step 1: Verify all files exist**

```bash
find docs/specs -name '*.md' | sort
```

Expected: a top-level INDEX plus per-API spec files for CLOB, Gamma, Data, and Relay. Tasks batch multiple files per commit, so verify the actual count rather than relying on a fixed total.

**Step 2: Verify all links resolve**

Spot-check that INDEX.md links point to files that exist.

**Step 3: Run a representative curl from each API to confirm verification commands work**

```bash
curl -s 'https://clob.polymarket.com/time'
curl -s 'https://gamma-api.polymarket.com/markets?limit=1' | jq '.[0].id'
curl -s 'https://data-api.polymarket.com/v1/leaderboard?limit=1' | jq '.[0].rank'
```

**Step 4: Commit any fixes**

```bash
git add docs/specs/
git commit -m "docs: finalize API specs directory"
```
