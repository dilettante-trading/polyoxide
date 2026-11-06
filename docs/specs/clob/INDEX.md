# CLOB API

Base URL: `https://clob.polymarket.com`
Staging URL: `https://clob-staging.polymarket.com`

Order book trading API for Polymarket. Supports market data queries, order placement/cancellation, balance management, liquidity rewards, and real-time WebSocket streams.

Machine-readable schema: [openapi.yaml](openapi.yaml) (mirror of `https://docs.polymarket.com/api-spec/clob-openapi.yaml`).

Order `signatureType` 3 (`POLY_1271`, a Deposit Wallet signing an ERC-7739
envelope, as owner or session key) is in the schema since #91 (2026-11), with the
Deposit Wallet routes `GET /data/wallet-open-orders` and `DELETE /cancel-wallet-orders`.
Still not in it: `GET /v1/user/session-signers`. See
[../session-keys/README.md](../session-keys/README.md).

## Auth

Three authentication levels. See [auth.md](auth.md) for details.

- **None** — Public market data endpoints
- **L1** — EIP-712 signing for credential creation
- **L2** — HMAC-SHA256 for most authenticated operations
- **Builder** — Separate HMAC keys for builder-attributed orders
- **Read-only key** — `POLY_READONLY_API_KEY` with `POLY_ADDRESS`, no signature; only
  `GET /data/wallet-open-orders` accepts it (`openapi.yaml`'s `polyReadonlyApiKey`, #91)

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [auth.md](auth.md) | /auth/api-key, /auth/derive-api-key, /auth/api-keys, /auth/builder-api-key | L1/L2 |
| [markets.md](markets.md) | /book, /books, /price, /prices, /midpoint, /midpoints, /spread, /spreads, /last-trade-price, /last-trades-prices, /fee-rate, /tick-size, /neg-risk, /prices-history, /batch-prices-history, /clob-markets/{id}, /markets-by-token/{id}, /simplified-markets, /sampling-markets, /sampling-simplified-markets, /markets/live-activity, /markets/live-activity/{id}, /time | None |
| [orders.md](orders.md) | /order, /data/order/{id}, /orders, /data/orders, /cancel-all, /cancel-market-orders, /data/trades, /order-scoring, /orders-scoring, /builder/trades; /data/wallet-open-orders and /cancel-wallet-orders are in openapi.yaml only | L2/Builder |
| [account.md](account.md) | /balance-allowance, /balance-allowance/update, /heartbeats, /v1/heartbeats, /auth/ban-status, /v1/user/rate-limit-tier | L2 |
| [rewards.md](rewards.md) | /rewards/user, /rewards/user/total, /rewards/user/percentages, /rewards/user/markets, /rewards/markets/current, /rewards/markets/{id}, /rewards/markets/multi, /rebates/current | Mixed |
| [rfq.md](rfq.md) | /rfq/request, /rfq/quote, /rfq/quotes, /rfq/requests, /rfq/prices | L2 |
| [notifications.md](notifications.md) | /notifications | L2 |
| [websocket.md](websocket.md) | ws/market, ws/user | Mixed |

## Rate Limits

Two independent layers; a request must satisfy both.

| File | Layer | Keyed on | Counts |
|------|-------|----------|--------|
| [rate-limits.md](rate-limits.md) | Cloudflare IP throttling | client IP | requests |
| [trading-rate-limits.md](trading-rate-limits.md) | per-signer token buckets | signer address | orders (batches cost N) |

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
