# Markets

Base URL: `https://gamma-api.polymarket.com`

## List Markets

`GET /markets`

List markets with optional filtering and pagination.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| id | query | integer[] | no | Filter by market IDs (repeated param) |
| slug | query | string[] | no | Filter by market slugs (repeated param) |
| clob_token_ids | query | string[] | no | Filter by CLOB token IDs (repeated param) |
| condition_ids | query | string[] | no | Filter by condition IDs (repeated param) |
| market_maker_address | query | string[] | no | Filter by market maker addresses (repeated param) |
| liquidity_num_min | query | number | no | Minimum liquidity threshold |
| liquidity_num_max | query | number | no | Maximum liquidity threshold |
| volume_num_min | query | number | no | Minimum trading volume |
| volume_num_max | query | number | no | Maximum trading volume |
| start_date_min | query | string | no | Earliest start date (ISO 8601) |
| start_date_max | query | string | no | Latest start date (ISO 8601) |
| end_date_min | query | string | no | Earliest end date (ISO 8601) |
| end_date_max | query | string | no | Latest end date (ISO 8601) |
| tag_id | query | integer | no | Filter by tag ID |
| related_tags | query | boolean | no | Include related tags in response |
| cyom | query | boolean | no | Filter create-your-own markets |
| uma_resolution_status | query | string | no | Filter by UMA resolution status |
| game_id | query | string | no | Filter by game identifier |
| sports_market_types | query | string[] | no | Filter by sports market types (repeated param) |
| rewards_min_size | query | number | no | Minimum rewards threshold |
| question_ids | query | string[] | no | Filter by question IDs (repeated param) |
| include_tag | query | boolean | no | Include tag data in results |
| closed | query | boolean | no | Filter by closed status |
| archived | query | boolean | no | Filter by archived status |
| active | query | boolean | no | Filter by active status |

**Response:** Array of Market objects

```json
[
  {
    "id": "12345",
    "conditionId": "0xabc...",
    "questionID": "0xdef...",
    "slug": "will-x-happen",
    "question": "Will X happen by end of 2025?",
    "description": "This market resolves...",
    "outcomes": "[\"Yes\",\"No\"]",
    "outcomePrices": "[\"0.55\",\"0.45\"]",
    "volume": "150000",
    "liquidity": "25000",
    "startDateIso": "2025-01-01T00:00:00Z",
    "endDateIso": "2025-12-31T23:59:59Z",
    "active": true,
    "closed": false,
    "marketMakerAddress": "0x1234...",
    "tokens": [
      {"tokenId": "71321...", "outcome": "Yes", "price": "0.55", "winner": false}
    ],
    "tags": [],
    "volumeNum": 150000.0,
    "liquidityNum": 25000.0,
    "volume24hr": 1500.5,
    "volume1wk": 10000.0,
    "volume1mo": 50000.0,
    "volume1yr": 200000.0,
    "negRisk": false,
    "rewardsMinSize": 10.0,
    "rewardsMaxSpread": 0.05,
    "commentCount": 42
  }
]
```

## Get Market by ID

`GET /markets/{id}`

Get a single market by its numeric ID.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | string | yes | Market ID |
| include_tag | query | boolean | no | Include tag data in response |

**Response:** Market object (same schema as list item above)

## Get Market by Slug

`GET /markets/slug/{slug}`

Get a single market by its URL slug.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| slug | path | string | yes | Market slug |
| include_tag | query | boolean | no | Include tag data in response |

**Response:** Market object

## Get Market Tags

`GET /markets/{id}/tags`

Get tags associated with a market.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | string | yes | Market ID |

**Response:** Array of Tag objects

```json
[
  {
    "id": "42",
    "slug": "politics",
    "label": "Politics",
    "forceShow": true,
    "forceHide": false,
    "isCarousel": false,
    "publishedAt": "2024-01-01T00:00:00Z",
    "createdAt": "2024-01-01T00:00:00Z",
    "updatedAt": "2024-06-01T00:00:00Z"
  }
]
```

## Get Market Description

`GET /markets/{id}/description` (marked internal in upstream OpenAPI)

Retrieve the description text for a specific market.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Market ID |

**Response:** MarketDescription object — `{ "description": <string, nullable> }`.

## List Markets (Keyset)

`GET /markets/keyset`

Cursor-based pagination for efficient navigation of large result sets. Does not accept `offset` — returns 422 if provided.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Max results, 1–100 (default 20) |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction (default true) |
| after_cursor | query | string | no | Opaque cursor token from previous response |
| id | query | integer[] | no | Filter by market IDs |
| slug | query | string[] | no | Filter by market slugs |
| closed | query | boolean | no | Filter by closed status (default false) |
| decimalized | query | boolean | no | Decimalization filter |
| clob_token_ids | query | string[] | no | Filter by CLOB token IDs |
| condition_ids | query | string[] | no | Filter by condition IDs |
| question_ids | query | string[] | no | Filter by question IDs |
| market_maker_address | query | string[] | no | Filter by market maker addresses |
| liquidity_num_min | query | number | no | Minimum liquidity |
| liquidity_num_max | query | number | no | Maximum liquidity |
| volume_num_min | query | number | no | Minimum volume |
| volume_num_max | query | number | no | Maximum volume |
| start_date_min | query | string | no | Earliest start date |
| start_date_max | query | string | no | Latest start date |
| end_date_min | query | string | no | Earliest end date |
| end_date_max | query | string | no | Latest end date |
| tag_id | query | integer[] | no | Filter by tag IDs |
| related_tags | query | boolean | no | Include related tags |
| tag_match | query | string | no | Tag matching strategy |
| cyom | query | boolean | no | Filter create-your-own markets |
| rfq_enabled | query | boolean | no | Filter by RFQ-enabled markets |
| uma_resolution_status | query | string | no | Filter by UMA resolution status |
| game_id | query | string | no | Filter by game identifier |
| sports_market_types | query | string[] | no | Filter by sports market types |
| include_tag | query | boolean | no | Include tag data on each market |
| locale | query | string | no | Locale preference |

**Response:** KeysetMarketsResponse object — `{ "data": [Market, ...], "next_cursor": "..." }`. The `next_cursor` field is omitted on the final page.

## Query Markets by Information

`POST /markets/information` (marked internal in upstream OpenAPI)

Query markets using detailed information filters passed in the request body. Filters too complex for URL query-string encoding.

**Auth:** None

**Request Body:** MarketsInformationBody JSON

```json
{
  "id": [12345],
  "slug": ["will-x-happen"],
  "closed": false,
  "clobTokenIds": ["71321..."],
  "conditionIds": ["0xabc..."],
  "marketMakerAddress": ["0x1234..."],
  "liquidityNumMin": 1000.0,
  "liquidityNumMax": 100000.0,
  "volumeNumMin": 500.0,
  "volumeNumMax": 1000000.0,
  "startDateMin": "2025-01-01T00:00:00Z",
  "startDateMax": "2025-12-31T23:59:59Z",
  "endDateMin": "2025-06-01T00:00:00Z",
  "endDateMax": "2026-01-01T00:00:00Z",
  "relatedTags": true,
  "tagId": 42,
  "cyom": false,
  "umaResolutionStatus": "resolved",
  "gameId": "game-1",
  "sportsMarketTypes": ["spread", "moneyline"],
  "rewardsMinSize": 10.0,
  "questionIds": ["0xdef..."],
  "includeTags": true
}
```

**Response:** Array of Market objects (same schema as `GET /markets`).

## Query Abridged Markets

`POST /markets/abridged` (marked internal in upstream OpenAPI)

Identical request body to `/markets/information`, but returns abridged (lighter-weight) market records. Suitable when only a subset of market fields is needed.

**Auth:** None

**Request Body:** Same as `/markets/information` (MarketsInformationBody).

**Response:** Array of abridged Market objects.

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/markets?limit=1' | jq '.[0] | {id, question, slug}'
```
