# Perps API

Base URL: `https://api.perpetuals.polymarket.com`

Perpetual futures trading: account state, order placement, and market info.

> **Partially implemented.** `polyoxide-perps` covers the 21 public
> `/v1/info/*` routes and, under its `ws` feature, the six public WebSocket
> channels (`bbo`, `book`, `trades`, `klines`, `tickers`, `statistics`; see
> [asyncapi.json](asyncapi.json)). Credentials (`POST /v1/account/proxy`), the
> header-authenticated `/v1/account/*` reads, the signed `/v1/trade/*` routes,
> the private WebSocket channels, funds and BLP are not yet implemented; the
> API's own `POLYMARKET-PROXY` / `POLYMARKET-SECRET` auth (below) is separate
> from the CLOB's L1/L2 layers.

Machine-readable schema: [openapi.json](openapi.json) (mirror of
`https://docs.polymarket.com/api-spec/perps-openapi.json`).

Observed behaviour the schema does not describe: [OBSERVED.md](OBSERVED.md).

## Auth

Two API-key headers, unrelated to the CLOB `POLY_*` L2 headers:

| Header | Meaning |
|--------|---------|
| `POLYMARKET-PROXY` | Proxy address |
| `POLYMARKET-SECRET` | Corresponding proxy secret |

Credentials are provisioned through `POST /v1/account/proxy` (EOA-signed) and
retrieved via `GET /v1/account/credentials`.

## Endpoints

61 endpoints across four groups.

| Group | Endpoints |
|-------|-----------|
| `/v1/account/*` | auto-cancel, backstops, balances, config, credentials, deposits, equity, fills, funding, internal-transfer(s), invite, limits, notifications(/read), open-orders, orders, pnl, portfolio, proxy, referral, rewards, stats, withdraw, withdrawals |
| `/v1/blp/*` | enroll, enrollment, liquidations |
| `/v1/info/*` | assets, bbo, book, exchange, exchange-stats, fees, funding, index, instruments, invite, klines, leaderboard, limit-tiers, mark-history, ping, portfolio, position-fills, statistics, tickers, time, trades |
| `/v1/trade/*` | auto-cancel, leverage(/batch), margin, orders (place, modify via `PATCH`, cancel), orders-coid (modify via `PATCH`, cancel), orders/all |

Real-time updates are documented separately in
`https://docs.polymarket.com/asyncapi-perps.json`.
