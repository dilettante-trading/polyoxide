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
    "active": true,
    "closed": false,
    "liquidity": 500000.0,
    "volume24hr": 25000.0,
    "negRisk": true,
    "negRiskMarketID": "0xabc...",
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
| id | path | integer | yes | Event ID |
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

## Get Event Tags

`GET /events/{id}/tags`

Get tags associated with an event.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Event ID |

**Response:** Array of Tag objects

## List Events (Paginated)

`GET /events/pagination`

List events and return pagination metadata alongside the result set.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| include_chat | query | boolean | no | Include chat relations |
| include_template | query | boolean | no | Include template relations |
| recurrence | query | string | no | Filter by recurrence pattern |

**Response:** EventsPagination object — `{ "data": [Event, ...], "pagination": {...} }`

## List Events (Keyset)

`GET /events/keyset`

Cursor-based pagination for efficient navigation of large result sets. Does not accept `offset` — returns 422 if provided.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Max results, 1–500 (default 20) |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction (default true) |
| after_cursor | query | string | no | Opaque cursor token from previous response |
| id | query | integer[] | no | Filter by event IDs |
| slug | query | string[] | no | Filter by event slugs |
| closed | query | boolean | no | Filter by closed status |
| live | query | boolean | no | Filter by live status |
| featured | query | boolean | no | Filter by featured status |
| cyom | query | boolean | no | Filter create-your-own events |
| title_search | query | string | no | Full-text search on event title |
| liquidity_min | query | number | no | Minimum liquidity |
| liquidity_max | query | number | no | Maximum liquidity |
| volume_min | query | number | no | Minimum volume |
| volume_max | query | number | no | Maximum volume |
| start_date_min | query | string | no | Earliest start date |
| start_date_max | query | string | no | Latest start date |
| end_date_min | query | string | no | Earliest end date |
| end_date_max | query | string | no | Latest end date |
| start_time_min | query | string | no | Earliest start time |
| start_time_max | query | string | no | Latest start time |
| tag_id | query | integer[] | no | Filter by tag IDs |
| tag_slug | query | string | no | Filter by tag slug |
| exclude_tag_id | query | integer[] | no | Tag IDs to exclude (no overlap with tag_id) |
| related_tags | query | boolean | no | Include related tags |
| tag_match | query | string | no | Tag matching strategy |
| series_id | query | integer[] | no | Filter by series IDs |
| game_id | query | integer[] | no | Filter by game IDs |
| event_date | query | string | no | Specific event date |
| event_week | query | integer | no | Specific event week |
| featured_order | query | boolean | no | Order by featured status |
| recurrence | query | string | no | Filter by recurrence pattern |
| created_by | query | string[] | no | Filter by creator addresses |
| parent_event_id | query | integer | no | Filter by parent event |
| include_children | query | boolean | no | Include child events |
| partner_slug | query | string | no | Attach external partners for matching events |
| include_chat | query | boolean | no | Include chats and series chats |
| include_template | query | boolean | no | Include templates |
| include_best_lines | query | boolean | no | Include best lines |
| locale | query | string | no | Locale preference |

**Response:** KeysetEventsResponse object — `{ "data": [Event, ...], "next_cursor": "..." }`. The `next_cursor` field is omitted on the final page.

## List Sport Event Results

`GET /events/results`

List sport event results with outcomes.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |

**Response:** Array of Event objects.

## Get Event Tweet Count

`GET /events/{id}/tweet-count`

Retrieve the tweet count associated with an event.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Event ID |

**Response:** EventTweetCount object — `{ "tweetCount": <int> }`.

## Get Event Comment Count

`GET /events/{id}/comments/count`

Get comment count for the specified event.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Event ID |

**Response:** Count object — `{ "count": <int> }`.

## List Event Creators

`GET /events/creators`

List event creators with optional filtering.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| creator_name | query | string | no | Filter by creator name |
| creator_handle | query | string | no | Filter by creator handle |

**Response:** Array of EventCreator objects.

## Get Event Creator by ID

`GET /events/creators/{id}`

Retrieve a specific event creator by identifier.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Creator ID |

**Response:** EventCreator object.

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/events?limit=1' | jq '.[0] | {id, title, slug}'
```
