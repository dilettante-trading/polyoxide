# Comments

Base URL: `https://gamma-api.polymarket.com`

## List Comments

`GET /comments`

List comments with optional filtering and pagination.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |
| parent_entity_type | query | string | no | Filter by parent entity type (Event, Series, market) |
| parent_entity_id | query | integer | no | Filter by parent entity ID |
| get_positions | query | boolean | no | Include position data in response |
| holders_only | query | boolean | no | Restrict results to position holders only |

**Response:** Array of Comment objects

```json
[
  {
    "id": "c1",
    "body": "I think this market will resolve yes.",
    "createdAt": "2024-06-01T10:00:00Z",
    "updatedAt": "2024-06-01T10:00:00Z",
    "deletedAt": null,
    "user": {
      "id": "u1",
      "name": "trader1",
      "avatar": null
    },
    "marketId": "mkt-1",
    "eventId": null,
    "seriesId": null,
    "parentId": null,
    "reactions": [],
    "positions": [
      {"tokenId": "t1", "outcome": "Yes", "shares": "100.5"}
    ],
    "likeCount": 5,
    "dislikeCount": 1,
    "replyCount": 3
  }
]
```

## Get Comment by ID

`GET /comments/{id}`

Get comments by comment ID (returns an array).

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | path | integer | yes | Comment ID |
| get_positions | query | boolean | no | Include position data in response |

**Response:** Array of Comment objects

## Get Comments by User Address

`GET /comments/user_address/{user_address}`

Get all comments posted by a specific user address.

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| user_address | path | string | yes | User wallet address |
| limit | query | integer | no | Maximum number of results |
| offset | query | integer | no | Pagination offset |
| order | query | string | no | Order field(s), comma-separated |
| ascending | query | boolean | no | Sort direction |

**Response:** Array of Comment objects

## Verification

```bash
curl -s 'https://gamma-api.polymarket.com/comments?limit=1' | jq '.[0]'
```
