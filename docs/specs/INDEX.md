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
