# Relay API

Base URL: `https://relayer-v2.polymarket.com`

Gasless transaction relay for Polymarket. Submits transactions to Polygon via Safe, Proxy or Deposit Wallets without requiring users to hold MATIC for gas.

Machine-readable schema: [openapi.yaml](openapi.yaml) (mirror of `https://docs.polymarket.com/api-spec/relayer-openapi.yaml`).

Not in the schema: `type: WALLET` (Deposit Wallet batches) on `/submit` and
`/deployed`, `GET /v1/account/transactions/params`, `GET /v1/account/transactions/{id}`,
`POST /v1/session-signers/authorizations` and `POST /v1/session-signers/revocations`.
See [../session-keys/README.md](../session-keys/README.md).

## Auth

See [auth.md](auth.md) for details.

- **None** — Nonce lookup, deployment check, transaction status
- **Builder API Key** — Transaction submission, recent transactions
- **Relayer API Key** — Transaction submission, API key management

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [transactions.md](transactions.md) | /submit, /transaction, /transactions, /nonce, /relay-payload, /deployed, /relayer/api/keys | Mixed |
| [contracts.md](contracts.md) | Contract addresses (reference, not endpoints) | N/A |

## Rate Limits

See [rate-limits.md](rate-limits.md) — 25 req/min global.
