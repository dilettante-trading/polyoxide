# Relay Authentication

Base URL: `https://relayer-v2.polymarket.com`

## Builder API Key Authentication

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_BUILDER_API_KEY` | Builder API key |
| `POLY_BUILDER_TIMESTAMP` | Current UNIX timestamp |
| `POLY_BUILDER_SIGNATURE` | HMAC-SHA256 signature |
| `POLY_BUILDER_PASSPHRASE` | Passphrase (optional) |

Signing uses HMAC-SHA256 with base64url-encoded secret.

## Relayer API Key Authentication

**Headers:**

| Header | Description |
|--------|-------------|
| `RELAYER_API_KEY` | Relayer API key |
| `RELAYER_API_KEY_ADDRESS` | Associated address |
