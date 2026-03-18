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
