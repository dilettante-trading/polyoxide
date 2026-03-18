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
