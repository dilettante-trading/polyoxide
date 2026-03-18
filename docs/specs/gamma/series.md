# Series

Base URL: `https://gamma-api.polymarket.com`

## List Series

`GET /series`

List series (tournament/season groupings) with optional filtering and pagination.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| ascending | query | boolean | no | Sort direction |
| closed | query | boolean | no | Filter by closed status |
| slug | query | string[] | no | Filter by slugs (repeated param) |
| categories_ids | query | string[] | no | Filter by category IDs (repeated param) |
| categories_labels | query | string[] | no | Filter by category labels (repeated param) |
| include_chat | query | boolean | no | Include chat data in response |
| recurrence | query | string | no | Filter by recurrence pattern |

**Response:** Array of SeriesData objects

```json
[
  {
    "id": "s1",
    "slug": "nfl-2025",
    "title": "NFL 2025",
    "description": "NFL 2025 season markets",
    "image": "https://example.com/nfl.png",
    "icon": "https://example.com/nfl-icon.png",
    "active": true,
    "closed": false,
    "archived": false,
    "tags": ["sports", "nfl"],
    "volume": 500000.0,
    "liquidity": 100000.0,
    "events": [],
    "competitive": "0"
  }
]
```

## Get Series by ID

`GET /series/{id}`

Get a single series by its ID.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | string | yes | Series ID |
| include_chat | query | boolean | no | Include chat data in response |

**Response:** SeriesData object (same schema as list item above, with nested `events` array populated)

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/series?limit=1' | jq '.[0] | {id}'
```
