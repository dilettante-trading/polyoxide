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
