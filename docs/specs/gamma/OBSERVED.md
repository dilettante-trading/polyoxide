# Gamma: where the published spec disagrees with the server

`openapi.yaml` in this directory is a byte-faithful mirror of upstream's
published document and **must stay that way** — `nightly-schema.yml` diffs it
against upstream and will alarm forever on any local edit. This file records
places where that document and upstream's own server disagree, which the drift
check structurally cannot see: it compares mirror to document, never to the
live host.

This is the same phenomenon as `docs/specs/clob/asyncapi-sports.json`, which
carries `x-observed-payload` inline. That mirror can be annotated because it is
excluded from drift checking; gamma's cannot.

## `parent_entity_type` on `GET /comments`

**Spec** (`openapi.yaml`, `listComments`): `enum: [Event, Series, market]`

**Server**, probed 2026-08-19:

```
$ curl -s "https://gamma-api.polymarket.com/comments?parent_entity_type=Market&parent_entity_id=559651&limit=1"
{"type":"validation error","error":"expected value to be one of \"Event, Series, PerpsAsset\""}

$ curl -s "https://gamma-api.polymarket.com/comments?parent_entity_type=market&parent_entity_id=559651&limit=1"
{"type":"validation error","error":"expected value to be one of \"Event, Series, PerpsAsset\""}
```

So `market` is rejected in either casing and `PerpsAsset` is undocumented.
`polyoxide_gamma::types::ParentEntityType` follows the server.

## `limit` on `GET /comments`

`limit` bounds top-level comments, not returned rows — replies accompany their
parents. Measured 2026-08-19 on `parent_entity_id=45915`: `limit=2` returned 8
rows, `limit=5` returned 18, `limit=64` returned 160. Callers sizing a buffer
from `limit` will under-allocate.

## `GET /comments/{id}` returns a thread

Upstream's summary is "Get comments by comment id". It returns the root comment
and every reply, with the requested id anywhere in the list. Requesting
`3218542` on 2026-08-19 returned six comments, the requested one third.

## More instances

The 2026-08-19 type parity sweep found nine further places where the spec and
the server disagree. They are catalogued in the appendix of
`docs/plans/2026-08-19-gamma-type-parity-worklist.md` rather than duplicated
here.

## Some Gamma endpoints publish their own live JSON Schema — a better oracle than `openapi.yaml`

Some Gamma responses carry a `"$schema"` key pointing at
`https://gamma-api.polymarket.com/schemas/<Name>.json` — an authoritative,
machine-readable schema for that exact endpoint, served live by the same host.
Two are known to do this:

| Endpoint | `$schema` |
|---|---|
| `GET /profiles/user_address/{address}` | `PublicProfile.json` |
| `GET /public-profile?address=` | `PublicProfileResponse.json` |

`/markets`, `/events` and `/comments` do **not** send a `$schema` key — this is
not a universal feature of the API, only of these two (so far).

Where a served schema disagrees with `openapi.yaml`, the served schema is
right. For `/profiles/user_address/{address}` the disagreement isn't a missing
field or two: `openapi.yaml`'s `Profile` schema (23 properties, `id` required)
describes a completely different object from what this endpoint actually
returns (`PublicProfile.json`: 10 properties including `$schema` itself,
`takerTier`/`takerTierName`/`weightedVolume` required, no `id` at all). See
`docs/plans/2026-08-19-gamma-type-parity-worklist.md` finding #1 — fixed by
modelling `polyoxide_gamma::types::Profile` against `PublicProfile.json`
directly, verified against `tests/fixtures/profile_{full,sparse}.json` and
enforced by `tests/wire_agreement.rs`.

**Practical upshot for future parity work:** before trusting `openapi.yaml`
for an endpoint, check whether a live response from it carries `$schema`. If
it does, fetch that URL and treat it as the oracle instead — it is closer to
the server than the vendored mirror can ever be, since the mirror is a
point-in-time copy and the served schema is generated from whatever the server
is actually running. This bears directly on `UserResponse`
(`polyoxide-gamma/src/api/user.rs`, `/public-profile`), which serves
`PublicProfileResponse.json` and remains unfixed — see finding #10 in the
worklist.
