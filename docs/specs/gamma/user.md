# User

Base URL: `https://gamma-api.polymarket.com`

## Get Public Profile

`GET /public-profile`

Get a user's public profile by wallet address.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | User wallet address (signer EOA) |

**Response:** UserResponse object

```json
{
  "proxyWallet": "0xproxy...",
  "address": "0xsigner...",
  "id": "u1",
  "name": "polytrader",
  "createdAt": "2024-01-15T10:00:00Z",
  "profileImage": "https://example.com/avatar.png",
  "displayUsernamePublic": true,
  "bio": "DeFi enthusiast",
  "pseudonym": "poly_anon",
  "xUsername": "polytrader_x",
  "verifiedBadge": true,
  "users": [
    {"id": "uid-1", "creator": true, "mod": false},
    {"id": "uid-2", "creator": false, "mod": true}
  ]
}
```

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/public-profile?address=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604' | jq .
```
