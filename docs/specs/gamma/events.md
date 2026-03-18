# Events

Base URL: `https://gamma-api.polymarket.com`

## List Events

`GET /events`

List events with optional filtering and pagination.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| id | query | integer[] | no | Filter by event IDs (repeated param) |
| tag_id | query | integer | no | Filter by tag ID |
| exclude_tag_id | query | integer[] | no | Exclude events with these tag IDs (repeated param) |
| slug | query | string[] | no | Filter by event slugs (repeated param) |
| tag_slug | query | string | no | Filter by tag slug |
| related_tags | query | boolean | no | Include related tags in response |
| active | query | boolean | no | Filter by active status |
| archived | query | boolean | no | Filter by archived status |
| featured | query | boolean | no | Filter by featured status |
| cyom | query | boolean | no | Filter create-your-own-market events |
| include_chat | query | boolean | no | Include chat data in response |
| include_template | query | boolean | no | Include template data |
| recurrence | query | string | no | Filter by recurrence pattern |
| closed | query | boolean | no | Filter by closed status |
| liquidity_min | query | number | no | Minimum liquidity threshold |
| liquidity_max | query | number | no | Maximum liquidity threshold |
| volume_min | query | number | no | Minimum trading volume |
| volume_max | query | number | no | Maximum trading volume |
| start_date_min | query | string | no | Earliest start date (ISO 8601) |
| start_date_max | query | string | no | Latest start date (ISO 8601) |
| end_date_min | query | string | no | Earliest end date (ISO 8601) |
| end_date_max | query | string | no | Latest end date (ISO 8601) |

**Response:** Array of Event objects

```json
[
  {
    "id": "evt-123",
    "title": "2025 US Presidential Election",
    "slug": "2025-us-presidential-election",
    "description": "Markets related to the 2025 election.",
    "startDate": "2024-01-01",
    "endDate": "2025-01-20",
    "startDateIso": "2024-01-01T00:00:00Z",
    "endDateIso": "2025-01-20T00:00:00Z",
    "active": true,
    "closed": false,
    "liquidity": 500000.0,
    "volume24hr": 25000.0,
    "negRisk": true,
    "negRiskMarketId": "0xabc...",
    "markets": [],
    "tags": [],
    "series": [],
    "cyom": false
  }
]
```

## Get Event by ID

`GET /events/{id}`

Get a single event by its ID.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | string | yes | Event ID |
| include_chat | query | boolean | no | Include chat data in response |
| include_template | query | boolean | no | Include template data in response |

**Response:** Event object (same schema as list item above, with nested `markets` array populated)

## Get Event by Slug

`GET /events/slug/{slug}`

Get a single event by its URL slug.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| slug | path | string | yes | Event slug |
| include_chat | query | boolean | no | Include chat data in response |
| include_template | query | boolean | no | Include template data in response |

**Response:** Event object

## Get Related Events by Slug

`GET /events/slug/{slug}/related`

Get events related to the event identified by slug.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| slug | path | string | yes | Event slug |

**Response:** Array of Event objects

## Get Event Tags

`GET /events/{id}/tags`

Get tags associated with an event.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | string | yes | Event ID |

**Response:** Array of Tag objects

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/events?limit=1' | jq '.[0] | {id, title, slug}'
```
