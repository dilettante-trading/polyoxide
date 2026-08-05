# Polymarket API Specs

Upstream API documentation for Claude. Source of truth for endpoint contracts,
rate limits, auth schemes, and response schemas as documented by Polymarket.

These specs are sourced from https://docs.polymarket.com and the OpenAPI specs at:
- CLOB: https://docs.polymarket.com/api-spec/clob-openapi.yaml
- Gamma: https://docs.polymarket.com/api-spec/gamma-openapi.yaml
- Data: https://docs.polymarket.com/api-spec/data-openapi.yaml
- Relay: https://docs.polymarket.com/api-spec/relayer-openapi.yaml
- Perps: https://docs.polymarket.com/api-spec/perps-openapi.json
- Bridge: https://docs.polymarket.com/api-spec/bridge-openapi.yaml
- Combos RFQ: https://docs.polymarket.com/api-spec/combos-rfq-openapi.yaml

## APIs

Covered by a polyoxide crate:

| API | Base URL | Description | Crate |
|-----|----------|-------------|-------|
| [CLOB](clob/INDEX.md) | `https://clob.polymarket.com` | Order book trading, market data, rewards, RFQ | `polyoxide-clob` |
| [Gamma](gamma/INDEX.md) | `https://gamma-api.polymarket.com` | Market/event metadata, search, comments | `polyoxide-gamma` |
| [Data](data/INDEX.md) | `https://data-api.polymarket.com` | User positions, trades, combos, leaderboard | `polyoxide-data` |
| [Relay](relay/INDEX.md) | `https://relayer-v2.polymarket.com` | Gasless relay transactions | `polyoxide-relay` |

Mirrored for reference, **not implemented** by any crate:

| API | Base URL | Description |
|-----|----------|-------------|
| [Perps](perps/INDEX.md) | `https://api.perpetuals.polymarket.com` | Perpetual futures: accounts, orders, market info |
| [Bridge](bridge/INDEX.md) | `https://bridge.polymarket.com` | Cross-chain deposits and withdrawals |
| [Combos RFQ](combos-rfq/INDEX.md) | `https://combos-rfq-api.polymarket.com` | Maker quoting for combinatorial markets |

## Hosts with no upstream spec

Some Polymarket APIs are published in **no** OpenAPI or AsyncAPI document.
[undocumented/INDEX.md](undocumented/INDEX.md) records what we know about them
— `user-pnl-api` and `lb-api` are implemented by `polyoxide-data`; the shapes
there were derived from live responses rather than a vendor contract.

## Rate limits are not in the OpenAPI either

Neither limiter appears in any machine-readable spec; both are prose pages.
CLOB is governed by **two independent layers** that count different things —
diffing the OpenAPI shows neither.

| Spec | Layer | Keyed on | Counts |
|------|-------|----------|--------|
| [clob/rate-limits.md](clob/rate-limits.md) | Cloudflare IP throttling | client IP | requests |
| [clob/trading-rate-limits.md](clob/trading-rate-limits.md) | per-signer token buckets | signer address | orders (batches cost N) |
| [gamma/rate-limits.md](gamma/rate-limits.md) | Cloudflare IP throttling | client IP | requests |
| [data/rate-limits.md](data/rate-limits.md) | Cloudflare IP throttling | client IP | requests |

Upstream's published tables also carry rows that name routes the host does not
serve — every surface lists `Health check (/ok)`, but only `clob.polymarket.com`
answers there. Probe the path on the host before pinning a row.

## WebSocket specs

Real-time contracts are published as **AsyncAPI**, separately from the OpenAPI
files above. A parity audit that only diffs OpenAPI misses this surface
entirely.

| Spec | Covers |
|------|--------|
| [clob/asyncapi-market.json](clob/asyncapi-market.json) | Market channel (11 messages) |
| [clob/asyncapi-user.json](clob/asyncapi-user.json) | User channel (6 messages) |
| [clob/asyncapi-sports.json](clob/asyncapi-sports.json) | Sports channel (3 messages) |
| [perps/asyncapi.json](perps/asyncapi.json) | Perps WebSocket (23 channels) — not implemented |
| [combos-rfq/asyncapi.json](combos-rfq/asyncapi.json) | RFQ quoter gateway — not implemented |
