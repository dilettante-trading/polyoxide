# Search

Base URL: `https://gamma-api.polymarket.com`

## Public Search

`GET /public-search`

Search across markets, events, and user profiles.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| q | query | string | yes | Search query string |
| search_profiles | query | boolean | no | Include profile results in search |
| limit_per_type | query | integer | no | Maximum results per type (events, profiles, tags) |
| page | query | integer | no | Page number for pagination |
| cache | query | boolean | no | Enable/disable caching |
| events_status | query | string | no | Filter by event status |
| events_tag | query | integer[] | no | Filter by event tag IDs (repeated param) |
| keep_closed_markets | query | boolean | no | Include closed markets in results |
| sort | query | string | no | Sort order (e.g. "volume") |
| search_tags | query | boolean | no | Include tag search results |
| recurrence | query | string | no | Filter by recurrence pattern |
| exclude_tag_id | query | integer[] | no | Exclude events with these tag IDs (repeated param) |
| optimized | query | boolean | no | Enable optimized search |

**Response:** SearchResponse object

```json
{
  "profiles": [
    {
      "address": "0xabc...",
      "name": "trader1",
      "profileImage": "https://example.com/avatar.png",
      "pseudonym": "anon_trader",
      "bio": "DeFi enthusiast",
      "proxyWallet": "0xproxy..."
    }
  ],
  "events": [
    {
      "id": "evt-1",
      "title": "Bitcoin Price Markets",
      "slug": "bitcoin-price-markets",
      "markets": []
    }
  ],
  "tags": [
    {
      "id": "10",
      "slug": "crypto",
      "label": "Crypto"
    }
  ]
}
```

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/public-search?q=bitcoin&limit=3' | jq .
```
