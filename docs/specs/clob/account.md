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

Sends one legacy order heartbeat for the authenticated API key. The first accepted send enables timeout cancellation for all open orders owned by that API key, across markets and processes. Keep sending accepted heartbeats every 5 seconds. No scheduler or disarm operation is provided: stopping sends or closing a client leaves the deadline active. It shares the key's registration with `POST /v1/heartbeats` and resets the expected v1 heartbeat ID to an empty string.

After 10 seconds without an accepted send, a five-second scan removes the registration and queues a best-effort cancel-all. Cancellation timing and success are not guaranteed, and later accepted sends register again. A lost response may follow an accepted send. WebSocket keep-alives do not refresh the deadline. Stop placing orders, and cancel and verify the orders to remove, before shutdown.

**Response:** `HeartbeatResponse`

```json
{"status": "ok"}
```

## Send Heartbeat (v1)

`POST /v1/heartbeats`

**Auth:** L2

Sends one order heartbeat with a rotating ID, on the same per-key registration and deadline as `POST /heartbeats`. Start with an empty `heartbeat_id` and keep each new ID a successful response returns; serialize sends per key. Rejected requests do not refresh the deadline, and a lost response may follow an accepted send, so a failure does not prove the registration is inactive.

**Request:** `HeartbeatRequest`

```json
{"heartbeat_id": ""}
```

**Response:** `HeartbeatV1Response`

```json
{"heartbeat_id": "550e8400-e29b-41d4-a716-446655440000"}
```

**Errors:** `400` either way the send is refused:

- ID mismatch (`HeartbeatErrorResponse`): `error_msg` and the `heartbeat_id` the venue expects, which is empty after a legacy send. Retry with that ID.

  ```json
  {"error_msg": "Invalid Heartbeat ID", "heartbeat_id": "550e8400-e29b-41d4-a716-446655440000"}
  ```

- A body that could not be decoded (`ErrorResponse`): `{"error": "Invalid payload"}`.

## Get Ban Status

`GET /auth/ban-status/closed-only`

**Auth:** L2

**Response:**

```json
{"closed_only": false}
```

## Get Rate Limit Tier

`GET /v1/user/rate-limit-tier`

**Auth:** L2

Returns the [trading rate limit tier](trading-rate-limits.md) currently assigned to the
signer address of the API credentials used. The response reflects the most recent tier
refresh, so a change in volume can take up to three hours to appear. `503` with
`{"error": "rate limit tiers unavailable"}` when the tiers could not be loaded.

**Response:** `RateLimitTierResponse` (`tier`, `period` and `tier_details` are required)

```json
{
  "tier": "gold",
  "usdc_volume": "612345.67",
  "next_tier_threshold": 2500000,
  "period": 30,
  "tier_details": {
    "name": "gold",
    "volume_threshold": 500000,
    "cancel_rate_per_sec": 800,
    "order_rate_per_sec": 400,
    "order_burst": 600,
    "cancel_burst": 1200,
    "allow_negative": true
  }
}
```

`usdc_volume` is omitted below the lowest paid tier, `next_tier_threshold` at the top tier
and for manually assigned tiers.

## Verification

```bash
# Server time (no auth, confirms API is up)
curl -s 'https://clob.polymarket.com/time'
```
