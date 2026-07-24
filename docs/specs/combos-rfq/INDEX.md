# Combos RFQ API

Base URL: `https://combos-rfq-api.polymarket.com`

Request-for-quote flow for combinatorial (multi-leg) markets, from the market
maker's side.

> **Not implemented by polyoxide.** This spec is mirrored so parity audits can
> see the surface. Note the distinction from `polyoxide-data`'s combos support:
> that reads a user's existing combo *positions and activity*
> (`/v1/positions/combos`, `/v1/activity/combos` on the Data API), whereas this
> API is for *quoting and executing* combo trades as a maker.

Machine-readable schema: [openapi.yaml](openapi.yaml) (mirror of
`https://docs.polymarket.com/api-spec/combos-rfq-openapi.yaml`).

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/rfq/combo-markets` | Active markets usable as combo legs, by volume desc. Public — no CLOB auth |
| POST | `/v1/maker/quotes` | Submit a quote in response to an RFQ |
| POST | `/v1/maker/quotes/cancel` | Cancel a submitted quote |
| POST | `/v1/maker/confirmations` | Confirm a matched quote |

Real-time RFQ events are documented separately in
`https://docs.polymarket.com/asyncapi-rfq.json`.
