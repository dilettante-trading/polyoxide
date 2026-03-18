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
