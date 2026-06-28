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

The first row is a shared cap across the listed endpoints as a group; individual endpoints may also have a tighter per-endpoint cap (e.g. `/notifications`).

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/trades`, `/orders`, `/notifications`, `/order` (shared group cap) | 900 | 10s |
| `/data/orders` | 500 | 10s |
| `/data/trades` | 500 | 10s |
| `/notifications` (per-endpoint cap) | 125 | 10s |

## Account

| Endpoint | Limit | Window |
|----------|-------|--------|
| `GET /balance-allowance` | 200 | 10s |
| `PUT /balance-allowance` | 50 | 10s |

## Auth

| Endpoint | Limit | Window |
|----------|-------|--------|
| `/auth/*` | 100 | 10s |
