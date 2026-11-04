# CLOB Authentication

Base URL: `https://clob.polymarket.com`

> **Note (CLOB V2):** polyoxide now targets the CLOB **V2** order scheme — orders
> are signed with EIP-712 domain version `"2"` against the V2 exchange contracts, and
> orders for a Protocol V2 position id with version `"3"` against ExchangeV3
> (see "Order-signing domain, per protocol" below).
> The signed order carries a `builder` `bytes32` field (builder-program
> attribution); fees are no longer part of the signed order — they are collected
> on-chain at match time. See [orders.md](./orders.md) for the signed-order shape.

## L1 Authentication (Private Key)

Used for: creating API credentials, deriving existing credentials, locally signing orders.

Signs an EIP-712 message containing: address, timestamp, nonce, and the message "This message attests that I control the given wallet."

### EIP-712 shape

Both type strings must match byte-for-byte or the server rejects the signature.
Reference: `py_clob_client/signing/{model,eip712}.py`.

```
domain: EIP712Domain(string name,string version,uint256 chainId)
struct: ClobAuth(address address,string timestamp,uint256 nonce,string message)
```

Three details are easy to get wrong, and each silently produces a valid-looking
signature that fails verification:

- The **domain carries no `verifyingContract`.** It is built from name,
  version, and chainId only. Passing a zero address adds a fourth field to the
  type string and changes the separator. (The *order* domain does use all four
  — they are different domains.)
- **`timestamp` is a `string`,** not a uint, so the digest covers its decimal
  text.
- **`message` is a constant.** The timestamp and nonce are their own fields;
  they are not interpolated into the message text.

Domain values: `name = "ClobAuthDomain"`, `version = "1"`.

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_ADDRESS` | Polygon signer address |
| `POLY_SIGNATURE` | EIP-712 signature |
| `POLY_TIMESTAMP` | Current UNIX timestamp |
| `POLY_NONCE` | Nonce (default: 0) |

## L2 Authentication (API Credentials)

Used for: canceling/retrieving orders, checking balances, posting signed orders.

Generated from L1 auth. Uses HMAC-SHA256 signing with the credential `secret` as key.

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_ADDRESS` | Polygon signer address |
| `POLY_SIGNATURE` | HMAC-SHA256 signature |
| `POLY_TIMESTAMP` | Current UNIX timestamp |
| `POLY_API_KEY` | API key value |
| `POLY_PASSPHRASE` | Passphrase value |

## Builder Authentication

Used for: order attribution to builder accounts.

**Headers:**

| Header | Description |
|--------|-------------|
| `POLY_BUILDER_API_KEY` | Builder API key |
| `POLY_BUILDER_PASSPHRASE` | Builder passphrase |
| `POLY_BUILDER_SIGNATURE` | HMAC-SHA256 signature |
| `POLY_BUILDER_TIMESTAMP` | Current UNIX timestamp |

## Signature Types

| Type | Value | Description |
|------|-------|-------------|
| EOA | `0` | Standard Ethereum wallet |
| POLY_PROXY | `1` | Custom proxy (Magic Link users) |
| GNOSIS_SAFE | `2` | Gnosis Safe multisig (most common) |

## Credential Endpoints

- `POST /auth/api-key` — Create new credentials (L1 auth)
- `GET /auth/derive-api-key` — Derive existing credentials (L1 auth)
- `GET /auth/api-keys` — List API keys (L1 auth)
- `DELETE /auth/api-key` — Delete current API key (L2 auth)
- `POST /auth/builder-api-key` — Create builder key (L2 auth)
- `GET /auth/builder-api-key` — List builder keys (L2 auth)
- `DELETE /auth/builder-api-key` — Revoke builder key (L2 auth)

## Order-signing domain, per protocol

The order domain is `EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)`
with `name = "Polymarket CTF Exchange"`. Its `version` and `verifyingContract` follow the order's
token id (py-sdk 0.12.0, `_internal/protocol.py`, `orders.py`, `context.py`); nothing else changes,
and a signature type 3 envelope uses the same domain as its outer (app) domain.

| Token id | `version` | `verifyingContract` |
|---|---|---|
| Protocol V2 position id (py-sdk's `is_v2_position_id` and `decode_v2_outcome_position_id` both accept it) | `"3"` | ExchangeV3 `0xe3333700cA9d93003F00f0F71f8515005F6c00Aa` (Polygon mainnet; none known on Amoy), whatever `neg_risk` is |
| any other (a CTF token id) | `"2"` | the standard exchange, or the neg-risk exchange when `neg_risk` is true |
