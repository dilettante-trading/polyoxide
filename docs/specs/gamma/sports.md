# Sports

Base URL: `https://gamma-api.polymarket.com`

## Get Sports Metadata

`GET /sports`

Get all sports metadata including configuration and assets.

**Auth:** None

No query parameters.

**Response:** Array of SportMetadata objects

```json
[
  {
    "id": 1,
    "sport": "Basketball",
    "image": "https://example.com/nba.png",
    "resolution": "...",
    "ordering": "...",
    "tags": "...",
    "series": "...",
    "createdAt": "2024-01-01T00:00:00Z"
  }
]
```

## Get Sports Market Types

`GET /sports/market-types`

List valid sports market types.

**Auth:** None

No query parameters.

**Response:** JSON object with market type definitions

## List Teams

`GET /teams`

List sports teams with optional filtering and pagination.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| league | query | string[] | no | Filter by league identifier(s) (repeated param) |
| name | query | string[] | no | Filter by team name(s) (repeated param) |
| abbreviation | query | string[] | no | Filter by team abbreviation(s) (repeated param) |

**Response:** Array of Team objects

```json
[
  {
    "id": 42,
    "name": "Lakers",
    "league": "NBA",
    "record": "50-32",
    "logo": "https://example.com/lakers.png",
    "abbreviation": "LAL",
    "alias": "Los Angeles Lakers",
    "createdAt": "2024-01-01T00:00:00Z",
    "updatedAt": "2024-06-15T12:00:00Z"
  }
]
```

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/sports' | jq .
```
