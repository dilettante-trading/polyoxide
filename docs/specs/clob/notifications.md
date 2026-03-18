# CLOB Notifications

Base URL: `https://clob.polymarket.com`

## Get Notifications

`GET /notifications`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| signature_type | query | integer | yes | 0=EOA, 1=POLY_PROXY, 2=GNOSIS_SAFE |

**Response:** Array of `Notification`

```json
[{
  "id": "string",
  "owner": "string",
  "type": 0,
  "payload": {},
  "timestamp": "string"
}]
```

## Dismiss Notifications

`DELETE /notifications`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| ids | query | string | yes | Comma-separated notification IDs |

**Response:** `"OK"`
