# Perps API

Base URL: `https://api.perpetuals.polymarket.com`

Perpetual futures trading: account state, order placement, and market info.

> **Not implemented by polyoxide.** This spec is mirrored so parity audits can
> see the surface. Adding client support means a new crate: the API uses its own
> authentication scheme (see below) rather than the CLOB's L1 EIP-712 / L2 HMAC
> layers, and defines ~270 response schemas.

Machine-readable schema: [openapi.json](openapi.json) (mirror of
`https://docs.polymarket.com/api-spec/perps-openapi.json`).

## Auth

Two API-key headers, unrelated to the CLOB `POLY_*` L2 headers:

| Header | Meaning |
|--------|---------|
| `POLYMARKET-PROXY` | Proxy address |
| `POLYMARKET-SECRET` | Corresponding proxy secret |

Credentials are provisioned through `POST /v1/account/proxy` (EOA-signed) and
retrieved via `GET /v1/account/credentials`.

## Endpoints

43 endpoints across three groups.

| Group | Endpoints |
|-------|-----------|
| `/v1/account/*` | auto-cancel, balances, config, credentials, deposits, equity, fills, funding, internal-transfer(s), invite, limits, open-orders, orders, pnl, portfolio, proxy, referral, rewards, stats, withdraw, withdrawals |
| `/v1/info/*` | assets, bbo, book, exchange, fees, funding, index, instruments, invite, klines, limit-tiers, ping, portfolio, statistics, tickers, time, trades |
| `/v1/trade/*` | auto-cancel, leverage, orders, orders-coid |

Real-time updates are documented separately in
`https://docs.polymarket.com/asyncapi-perps.json`.
