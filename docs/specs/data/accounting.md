# Data API Accounting

Base URL: `https://data-api.polymarket.com`

## Get Accounting Snapshot

`GET /v1/accounting/snapshot`

Downloads an accounting snapshot for a user as a ZIP archive containing `positions.csv` and `equity.csv`.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |

**Response:** `application/zip` — a ZIP file containing `positions.csv` and `equity.csv`.

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (missing or malformed user address) |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Download a user's accounting snapshot
curl -s 'https://data-api.polymarket.com/v1/accounting/snapshot?user=0x...' -o snapshot.zip && unzip -l snapshot.zip
```
