# Bridge API

Base URL: `https://bridge.polymarket.com`

Cross-chain deposits and withdrawals: quoting, address creation, and status.

> **Not implemented by polyoxide.** This spec is mirrored so parity audits can
> see the surface.

Machine-readable schema: [openapi.yaml](openapi.yaml) (mirror of
`https://docs.polymarket.com/api-spec/bridge-openapi.yaml`).

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/quote` | Get a quote for a bridge transfer |
| POST | `/deposit` | Create bridge deposit addresses |
| POST | `/withdraw` | Create withdrawal addresses |
| GET | `/status/{address}` | Transaction status for an address |
| GET | `/supported-assets` | Assets available for bridging |
