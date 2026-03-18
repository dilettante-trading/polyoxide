# API Specs Directory Design

**Date:** 2026-03-18
**Status:** Approved

## Goal

Create a `docs/specs/` directory containing upstream Polymarket API documentation as a source of truth for Claude. Replaces the need to fetch from Polymarket's docs at runtime.

## Scope

Specs cover only what Polymarket documents: endpoint contracts, rate limits, auth schemes, and response schemas. No implementation decisions or internal notes.

## Structure

Approach B: nested by API, split by concern. Each API gets its own directory with an INDEX.md and per-topic files.

```
docs/specs/
├── INDEX.md
├── clob/
│   ├── INDEX.md
│   ├── rate-limits.md
│   ├── auth.md
│   ├── markets.md
│   ├── orders.md
│   ├── account.md
│   ├── rewards.md
│   ├── rfq.md
│   ├── notifications.md
│   └── websocket.md
├── gamma/
│   ├── INDEX.md
│   ├── rate-limits.md
│   ├── markets.md
│   ├── events.md
│   ├── series.md
│   ├── tags.md
│   ├── sports.md
│   ├── comments.md
│   ├── search.md
│   └── user.md
├── data/
│   ├── INDEX.md
│   ├── rate-limits.md
│   ├── positions.md
│   ├── trades.md
│   ├── holders.md
│   ├── open-interest.md
│   ├── live-volume.md
│   ├── leaderboard.md
│   └── builders.md
└── relay/
    ├── INDEX.md
    ├── rate-limits.md
    ├── auth.md
    ├── transactions.md
    └── contracts.md
```

## File Templates

### Endpoint files

Each endpoint file follows a consistent structure:

- Heading with endpoint group name
- Base URL
- Per-endpoint sections with: method, path, description, auth level, parameters table, response schema
- Verification section with representative curl commands for spot-checking

### INDEX files

**Top-level:** Lists all APIs with base URLs, descriptions, and links to sub-indexes.

**Per-API:** Overview of the API, auth summary, table of endpoint files with their endpoints and auth levels, link to rate-limits.md.

### Rate limit files

Per-API rate limit documentation including:
- Enforcement mechanism (sliding window, throttling behavior)
- General/default limit
- Endpoint-specific limits table with burst and sustained windows where applicable

## Sourcing

- Rate limits, endpoint contracts, auth schemes: from `https://docs.polymarket.com`
- Response schemas: from Polymarket API reference, supplemented by observed curl responses where docs are incomplete (noted as such)

## Integration

A one-liner added to CLAUDE.md pointing to `docs/specs/INDEX.md` as the source of truth for upstream API documentation.
