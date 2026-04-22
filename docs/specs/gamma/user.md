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

## Get Profile by User Address

`GET /profiles/user_address/{user_address}`

Retrieve public profile data for a user by their wallet address. Distinct from `/public-profile` — returns a richer schema with UTM tracking, wallet activation state, and certification-request fields.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| user_address | path | string | yes | User's wallet address (0x + 40 hex chars) |

**Response:** Profile object

```json
{
  "id": "p1",
  "name": "polytrader",
  "user": 123,
  "referral": "abc",
  "createdBy": 1,
  "updatedBy": 1,
  "createdAt": "2024-01-15T10:00:00Z",
  "updatedAt": "2024-06-15T12:00:00Z",
  "utmSource": "twitter",
  "utmMedium": "social",
  "utmCampaign": "summer",
  "utmContent": "banner",
  "utmTerm": "polymarket",
  "walletActivated": true,
  "pseudonym": "poly_anon",
  "displayUsernamePublic": true,
  "profileImage": "https://example.com/avatar.png",
  "bio": "DeFi enthusiast",
  "proxyWallet": "0xproxy...",
  "profileImageOptimized": null,
  "isCloseOnly": false,
  "isCertReq": false,
  "certReqDate": null
}
```

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/public-profile?address=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604' | jq .
```
