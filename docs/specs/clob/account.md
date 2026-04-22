# CLOB Account

Base URL: `https://clob.polymarket.com`

## Get Balance & Allowance

`GET /balance-allowance`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| asset_type | query | string | yes | COLLATERAL or CONDITIONAL |
| token_id | query | string | no | Required for CONDITIONAL assets |
| signature_type | query | integer | no | 0=EOA, 1=POLY_PROXY, 2=GNOSIS_SAFE |

**Response:**

```json
{
  "balance": "string",
  "allowances": {"address": "amount"}
}
```

## Update Balance & Allowance

`PUT /balance-allowance`

**Auth:** L2

Parameters same as GET. Returns empty object.

## Update and Return Balance

`GET /balance-allowance/update`

**Auth:** L2

Parameters same as GET. Returns `BalanceAllowanceResponse`.

## Send Heartbeat (Basic)

`POST /heartbeats`

**Auth:** L2

Keeps session alive. If heartbeats are not sent regularly, all open orders for the user will be automatically canceled.

**Response:** `HeartbeatResponse`

```json
{"status": "ok"}
```

## Send Heartbeat (v1)

`POST /v1/heartbeats`

**Auth:** L2

Newer variant that tracks session with a heartbeat ID. The first request sends an empty `heartbeat_id`; the response returns a new ID that must be echoed in subsequent requests. Mismatched IDs are rejected.

**Request:** `HeartbeatRequest`

```json
{"heartbeat_id": ""}
```

**Response:** `HeartbeatV1Response`

```json
{"heartbeat_id": "550e8400-e29b-41d4-a716-446655440000"}
```

## Get Ban Status

`GET /auth/ban-status/closed-only`

**Auth:** L2

**Response:**

```json
{"closed_only": false}
```

## Verification

```bash
# Server time (no auth, confirms API is up)
curl -s 'https://clob.polymarket.com/time'
```
