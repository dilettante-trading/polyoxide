# Status

Base URL: `https://gamma-api.polymarket.com`

## Gamma API Health Check

`GET /status`

Returns a liveness probe for the Gamma API. Used by clients to measure round-trip latency and verify the service is reachable.

**Auth:** None

No query parameters.

**Response:** `200 OK` with `text/plain` body `"OK"`.

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/status'
```
