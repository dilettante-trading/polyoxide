# Data API Live Volume

Base URL: `https://data-api.polymarket.com`

## Get Live Volume

`GET /live-volume`

Returns live trading volume for an event.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| id | query | integer (>= 1) | yes | — | Event ID |

**Response:** `LiveVolume[]`

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (missing id, non-integer, value < 1) |
| 401 | Unauthorized |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```

## Verification

```bash
# Get live volume for event 1
curl -s 'https://data-api.polymarket.com/live-volume?id=1' | jq .
```
