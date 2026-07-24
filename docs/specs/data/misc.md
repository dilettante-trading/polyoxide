# Data API Misc

Base URL: `https://data-api.polymarket.com`

Endpoints that don't belong to a larger group.

## Get "Other" Size

`GET /other`

Returns the "Other" outcome size for an augmented neg-risk event and user.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| id | query | integer (≥ 1) | yes | — | Gamma event ID of the augmented neg-risk event |
| user | query | Address (`0x` + 40 hex) | yes | — | User wallet address |

**Response:** `OtherSize[]`

```json
[{"id": 0, "user": "string", "size": 0}]
```

## Get Question Revisions

`GET /revisions`

Returns moderated revisions recorded for a question.

**Auth:** None

| Name | In | Type | Required | Default | Description |
|------|-----|------|----------|---------|-------------|
| questionID | query | Hash64 (`0x` + 64 hex) | yes | — | Question ID |
| limit | query | integer (0-500) | no | 100 | Maximum revisions returned |

**Response:** `RevisionPayload[]`

```json
[{
  "questionID": "string",
  "revisions": [{"revision": "string", "timestamp": 0}]
}]
```

## Errors

| Code | Description |
|------|-------------|
| 400 | Invalid parameters (missing required fields, out-of-range values) |
| 404 | No record for the given question |
| 500 | Internal server error |

**ErrorResponse:**

```json
{"error": "string"}
```
