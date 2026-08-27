# Data API Holders

Base URL: `https://data-api.polymarket.com`

## Get Holders

`GET /holders`

Returns top token holders for a market.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| market | query | Hash64[] (`0x` + 64 hex) | yes | — | Condition ID(s) |
| limit | query | integer (1-500, clamped) | no | 20 | Results per page |
| minBalance | query | integer (0-999999) | no | 1 | Minimum token balance |

**Response:** `MetaHolder[]`

Verified live on 2026-08-03:

| `limit` | Result |
|---------|--------|
| omitted | 200, 20 rows per token (the default) |
| `0` | 200, body is a bare `null` — **not** `[]` |
| `500` | 200, up to 500 rows per token |
| `501`, `5000` | 200, **clamped** to 500 rows per token |
| `-1`, `abc` | 200, falls back to the default of 20 |

The cap is enforced by silent truncation, not rejection: an over-ceiling request
succeeds and the caller cannot tell from the status code that it received fewer
rows than it asked for.

**This changed upstream.** Until at least 2026-07-25, `limit=501` returned
HTTP 400 `{"error":"max holders limit of 500 exceeded"}`. An earlier revision of
this page documented the maximum as 20 — that was the default being mistaken for
the cap.

> `openapi.yaml` still declares `limit` as `maximum: 20` with the description
> "Capped at 20". That is upstream's published schema and is wrong on both
> counts, but it is deliberately left unedited: `nightly-schema.yml` diffs that
> file byte-for-byte against Polymarket's published spec, so correcting it here
> would manufacture permanent false drift.

## `market` validation

`market` is checked for shape — `0x` followed by exactly 64 hex digits — before
anything is looked up, and **a value that fails the check is reported as if no
value had been sent at all**:

| `market` | Response |
|----------|----------|
| omitted | 400 `{"error":"required query param 'market' not provided"}` |
| `` (empty) | 400, *same message* |
| 62 hex digits | 400, *same message* |
| 65 hex digits, or any non-hex | 400, *same message* |
| Hash64, unknown to the index | 200, body is a bare `null` — **not** `[]` |
| Hash64, known | 200, `MetaHolder[]` |

Verified live on 2026-08-27.

This matters because **`GET /trades` emits both shapes**. Most `conditionId`
values in the trade feed are Hash64, but some are `0x` + 62 hex, zero-padded on
the right — e.g. `0x03474f36a86039e6c40479b1844401d81a0000000000000000000000000000`.
Feeding one of those straight from `/trades` into `/holders` therefore fails,
and the error names the wrong cause: it says the parameter is missing when it
was sent and malformed. Three of these were present in a 2,500-trade sample.

That is what broke the 2026-08-26 nightly (issue #32): `live_holders` took
`trades[0].condition_id` unconditionally, drew one of the 62-hex ids, and the
"not provided" message made an upstream shape mismatch read as an SDK defect.
`polyoxide-data/tests/live_api.rs` now probes for a market `/holders` answers
for instead of assuming the newest trade names one.

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (missing required fields, out-of-range values) |
| 401 | Unauthorized |
| 500 | Internal server error |

A 400 from this endpoint does not distinguish "absent" from "malformed" — see
the table above before concluding the client failed to send the parameter.

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get top holders for a market (replace with a valid condition ID)
curl -s 'https://data-api.polymarket.com/holders?market=0x...&limit=3' | jq .
```
