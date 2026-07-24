# Undocumented hosts

Polymarket serves several APIs that appear in **no published OpenAPI or
AsyncAPI document**. They back the web app and were found by probing, then
verified live.

This page is the only contract record we have for them. Everything below was
derived from live responses and from the APIs' own validation errors, which
usefully enumerate accepted values.

> **Stability.** These carry no documented compatibility guarantee. Treat them
> as more likely to change without notice than spec-backed endpoints. The
> `polyoxide-data` live tests (`cargo test -p polyoxide-data --test live_api --
> --ignored`) are the drift detector — if a shape changes, they fail there
> rather than silently in a user's deserialize.

## User PnL

Base URL: `https://user-pnl-api.polymarket.com`
Implemented by: `data.pnl()` (`polyoxide-data`)

### `GET /user-pnl`

Realized-plus-unrealized PnL time series, as used by the web app's PnL chart.

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| user_address | query | Address (`0x` + 40 hex) | yes | Omitting it returns `{"error": "invalid filters: the 'user_address' field is mandatory"}` |
| fidelity | query | string | no | Sampling resolution. Accepted: `1d`, `18h`, `12h`, `3h`, `1h`. Anything else returns an error naming the valid set |
| interval | query | string | no | Trailing window, e.g. `1d`, `all`. Not enumerated by any error message |

**Response:** array of `{ t, p }` — Unix seconds and PnL in USDC. Negative
values are losses.

```json
[{"t":1784836800,"p":-1703608.4},{"t":1784840400,"p":-1703931.5}]
```

## Rankings

Base URL: `https://lb-api.polymarket.com`
Implemented by: `data.rankings()` (`polyoxide-data`)

Distinct from `GET /v1/leaderboard` on the main Data API host, which is
spec-backed and returns a different shape.

### `GET /volume` and `GET /profit`

Ranks traders by traded volume, or by realized profit.

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| window | query | string | no | `all`, `1d`, `7d`, `30d`. Anything else returns `{"error": "invalid request"}` |
| limit | query | integer | no | Maximum entries returned |

**Response:** array of ranking entries. `amount` is volume or profit in USDC
depending on the endpoint. Rows can be sparse — only `proxyWallet` and
`amount` are reliably present.

```json
[{
  "proxyWallet": "0x204f...",
  "pseudonym": "swisstony",
  "amount": 1730945622.28217,
  "name": "swisstony",
  "bio": "",
  "profileImage": "",
  "profileImageOptimized": ""
}]
```

## Known but not implemented

| Host | Evidence | Notes |
|------|----------|-------|
| `ws-live-data.polymarket.com` | Returns `426 Upgrade Required` | A live WebSocket feed. Not described by any of the five AsyncAPI documents; payloads unexamined |
