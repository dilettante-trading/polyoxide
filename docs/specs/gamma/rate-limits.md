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
