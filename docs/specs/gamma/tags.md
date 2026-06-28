# Tags

Base URL: `https://gamma-api.polymarket.com`

## List Tags

`GET /tags`

List tags with optional filtering and pagination.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| include_template | query | boolean | no | Include template data in response |
| is_carousel | query | boolean | no | Filter by carousel status |

**Response:** Array of Tag objects

```json
[
  {
    "id": "42",
    "slug": "politics",
    "label": "Politics",
    "forceShow": true,
    "forceHide": false,
    "isCarousel": true,
    "publishedAt": "2024-01-01T00:00:00Z",
    "createdBy": 1,
    "updatedBy": 2,
    "createdAt": "2024-01-01T00:00:00Z",
    "updatedAt": "2024-06-01T00:00:00Z"
  }
]
```

## Get Tag by ID

`GET /tags/{id}`

Get a single tag by its ID.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Tag ID |

**Response:** Tag object

## Get Tag by Slug

`GET /tags/slug/{slug}`

Get a single tag by its URL slug.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| slug | path | string | yes | Tag slug |

**Response:** Tag object

## Get Related Tags by ID

`GET /tags/{id}/related-tags`

Get tag relationships for the specified tag by ID.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Tag ID |
| omit_empty | query | boolean | no | Omit tags with no events |
| status | query | string | no | Filter by tag status (`active`, `closed`, `all`) |

**Response:** Array of RelatedTag objects (relationship rows, not the tags themselves)

## Get Related Tags by Slug

`GET /tags/slug/{slug}/related-tags`

Get tag relationships for the specified tag by slug.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| slug | path | string | yes | Tag slug |
| omit_empty | query | boolean | no | Omit tags with no events |
| status | query | string | no | Filter by tag status (`active`, `closed`, `all`) |

**Response:** Array of RelatedTag objects (relationship rows, not the tags themselves)

## Get Tags Related to a Tag by ID

`GET /tags/{id}/related-tags/tags`

Get the tags that are related to the specified tag by ID. Returns the related Tag objects themselves (not events).

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Tag ID |
| omit_empty | query | boolean | no | Omit tags with no events |
| status | query | string | no | Filter by tag status (`active`, `closed`, `all`) |

**Response:** Array of Tag objects

## Get Tags Related to a Tag by Slug

`GET /tags/slug/{slug}/related-tags/tags`

Get the tags that are related to the specified tag by slug. Returns the related Tag objects themselves (not events).

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| slug | path | string | yes | Tag slug |
| omit_empty | query | boolean | no | Omit tags with no events |
| status | query | string | no | Filter by tag status (`active`, `closed`, `all`) |

**Response:** Array of Tag objects

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/tags?limit=5' | jq '.[0]'
```
