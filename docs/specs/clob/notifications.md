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
  "id": 1,
  "owner": "string",
  "type": 2,
  "payload": {},
  "timestamp": 1675277676
}]
```

`type` is an integer enum: 1=cancel, 2=fill, 3=market registered, 4=resolved, 5=reward, 6=child comment.

## Dismiss Notifications

`DELETE /notifications`

**Auth:** L2

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| ids | query | string | yes | Comma-separated notification IDs |

**Response:** `"OK"`
