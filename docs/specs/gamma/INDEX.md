# Gamma API

Base URL: `https://gamma-api.polymarket.com`

Read-only market metadata API. Provides event/market/tag information, search, comments, sports data, and user profiles. No authentication required.

## Auth

No authentication required for any endpoint.

## Endpoints

| File | Endpoints | Auth |
|------|-----------|------|
| [status.md](status.md) | /status | None |
| [markets.md](markets.md) | /markets, /markets/{id}, /markets/slug/{slug}, /markets/{id}/tags, /markets/{id}/description, /markets/keyset, POST /markets/information, POST /markets/abridged | None |
| [events.md](events.md) | /events, /events/{id}, /events/slug/{slug}, /events/{id}/tags, /events/pagination, /events/keyset, /events/results, /events/{id}/tweet-count, /events/{id}/comments/count, /events/creators, /events/creators/{id} | None |
| [series.md](series.md) | /series, /series/{id}, /series/{id}/comments/count, /series-summary/{id}, /series-summary/slug/{slug} | None |
| [tags.md](tags.md) | /tags, /tags/{id}, /tags/slug/{slug}, related-tags | None |
| [sports.md](sports.md) | /sports, /sports/market-types, /teams, /teams/{id} | None |
| [comments.md](comments.md) | /comments, /comments/{id}, /comments/user_address/{addr} | None |
| [search.md](search.md) | /public-search | None |
| [user.md](user.md) | /public-profile, /profiles/user_address/{addr} | None |

## Rate Limits

See [rate-limits.md](rate-limits.md) for all limits.
