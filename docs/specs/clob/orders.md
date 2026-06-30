# CLOB Orders & Trading

Base URL: `https://clob.polymarket.com`

> **Note (CLOB V2):** polyoxide signs and submits orders using the CLOB **V2**
> scheme (EIP-712 domain version `"2"`, V2 exchange contracts). The V2 signed
> order replaces `taker`/`nonce`/`feeRateBps` with `timestamp`, `metadata`, and a
> `builder` `bytes32` field. Fees are **not** part of the signed order — they are
> collected on-chain at match time. The example bodies below show the legacy V1
> field layout; the V2 wire body omits the dropped fields and adds the new ones.
> The order `salt` is masked to the JavaScript-safe-integer range (`2^53 - 1`): a
> larger value is corrupted by the server's numeric parse and, because `salt` is
> signed, breaks the EIP-712 signature (`"Invalid order payload"`).

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
  "transactionsHashes": ["string"],
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

`GET /data/orders`

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

`GET /data/order/{orderID}`

**Auth:** L2 or Builder

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| orderID | path | string | yes | Order ID (order hash) |

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

**Request:** Array of order ID strings (max 1,000)

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

## Post-Only Mode

When the venue is in post-only mode, order placement is rejected and only
post-only orders and cancels are accepted. Single-order responses return
`{"error": "post-only mode: only post-only orders and cancels are allowed"}`;
batch endpoints return the same text per order in `errorMsg`.

## Get Trades

`GET /data/trades`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | query | string | no | Trade ID filter |
| maker_address | query | string | yes | Maker address (0x-prefixed, 40 hex chars) |
| market | query | string | no | Condition ID filter |
| asset_id | query | string | no | Token ID filter |
| before | query | string | no | UNIX timestamp filter |
| after | query | string | no | UNIX timestamp filter |
| next_cursor | query | string | no | Pagination cursor |

**Response:** `TradesResponse` (paginated trade list)

## Get Order Scoring Status

`GET /order-scoring`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| order_id | query | string | yes | Order ID (order hash) |

**Response:** `OrderScoringResponse`

```json
{"scoring": true}
```

## Get Orders Scoring (Query)

`GET /orders-scoring`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| order_ids | query | string[] | yes | Repeatable order_ids param |

**Response:** `OrdersScoringResponse` — map of order ID to boolean

```json
{"0xabc...": true, "0xdef...": false}
```

## Get Orders Scoring (Body)

`POST /orders-scoring`

**Auth:** L2

**Request:** Array of order ID strings

**Response:** `OrdersScoringResponse` — map of order ID to boolean

## Get Builder Trades

`GET /builder/trades`

**Auth:** Builder

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | query | string | no | Trade ID filter |
| builder | query | string | no | Builder identifier (auto-set from auth) |
| market | query | string | no | Condition ID filter |
| asset_id | query | string | no | Token ID filter |
| before | query | string | no | UNIX timestamp filter |
| after | query | string | no | UNIX timestamp filter |
| next_cursor | query | string | no | Pagination cursor |

**Response:** `BuilderTradesResponse` (paginated builder trade list)

## Verification

```bash
# Get simplified markets (public, no auth needed)
curl -s 'https://clob.polymarket.com/simplified-markets?next_cursor=MA==' | jq '.data[0]'
```
