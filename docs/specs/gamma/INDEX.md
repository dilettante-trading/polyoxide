# Gamma API

Base URL: `https://gamma-api.polymarket.com`

Read-only market metadata API. Provides event/market/tag information, search, comments, sports data, and user profiles. No authentication required.

Machine-readable schema: [openapi.yaml](openapi.yaml) (mirror of `https://docs.polymarket.com/api-spec/gamma-openapi.yaml`).

## Auth

No authentication required for any endpoint.

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [markets.md](markets.md) | /markets, /markets/{id}, /markets/slug/{slug}, /markets/{id}/tags | None |
| [events.md](events.md) | /events, /events/{id}, /events/slug/{slug}, /events/{id}/tags | None |
| [series.md](series.md) | /series, /series/{id} | None |
| [tags.md](tags.md) | /tags, /tags/{id}, /tags/slug/{slug}, related-tags | None |
| [sports.md](sports.md) | /sports, /sports/market-types, /teams | None |
| [comments.md](comments.md) | /comments, /comments/{id}, /comments/user_address/{addr} | None |
| [search.md](search.md) | /public-search | None |
| [user.md](user.md) | /public-profile | None |

## Rate Limits

See [rate-limits.md](rate-limits.md) for all limits.
