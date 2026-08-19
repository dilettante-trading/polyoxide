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
is actually running.

`GET /public-profile` (`polyoxide-gamma/src/api/user.rs`) is the second
confirmation of the same pattern: it serves `PublicProfileResponse.json`, a
sibling schema to `PublicProfile.json` (10 properties there, 12 here, plus the
nested `PublicProfileUser.json` for each entry of `users[]`). Neither schema
has ever described a top-level `address` or `id` — the fork invented both,
exactly as it invented `Profile::id` (#1) and `SearchProfile::address` (part
of the same finding, #10). Fixed by modelling `polyoxide_gamma::api::user::UserResponse`
and `UserInfo` against `PublicProfileResponse.json` / `PublicProfileUser.json`
directly, verified against `tests/fixtures/user_response_{full,sparse}.json`
(39-address live sample) and enforced by `tests/wire_agreement.rs`. Key
findings: the account id lives nested at `users[].id` (required on that
object), not at any top level; `discordUsername` is a documented optional
property never observed on the wire across the sample; and `users[].communityMod`,
while usually present, was absent for 1 of 39 sampled nested entries —
confirming it is genuinely optional rather than always-sent-as-false. Finding
#10's remaining half, `SearchProfile::address`, is now fixed too — see the
`/public-search` section below.

## `GET /public-search` serves no `$schema` — and its `profiles` array can contain `null`

Unlike `/public-profile` and `/profiles/user_address/{address}`,
`/public-search` (`polyoxide-gamma/src/api/search.rs`) serves **no** `$schema`
key at any level (verified 2026-08-19 — a response body is
`{"events": [...], "profiles": [...], "pagination": {...}}` with no schema
link anywhere). There is no published, machine-readable contract for this
endpoint, so `SearchProfile` is modelled from a live sample instead: 228
profile objects across 12 queries (`poly, trader, crypto, whale, a, bot, john,
mod, degen, market, sports, e`) at
`/public-search?q=<q>&search_profiles=true&limit_per_type=20`. Key frequency:
`name`, `displayUsernamePublic`, `proxyWallet` in 228/228; `pseudonym` in
223/228; `profileImage` in 41/228; `bio` in 34/228; `address` in **0/228** —
invented by the fork, exactly like `Profile::id` and `UserResponse::address`/
`id`. Because there is no schema to name a `required` set, every
`SearchProfile` field stays `Option`, unlike `Profile` and `UserResponse`
where a served schema's `required` list justified non-`Option` fields. Fixed
by removing `SearchProfile::address` and adding
`SearchProfile::display_username_public`; verified against
`tests/fixtures/search_profile_{full,sparse}.json` and enforced by
`tests/wire_agreement.rs`. This closes finding #10.

Separately — found while fixing #10, not part of the original sweep —
**`SearchResponse::profiles` can contain a JSON `null` element**, and the old
`Vec<SearchProfile>` could not deserialize one, so the whole call errored
rather than losing data. Reproduce with:

```
GET /public-search?q=sports&search_profiles=true&limit_per_type=20
```

`profiles` returns 20 entries; index 12 is `null`. Stable on 5/5 attempts on
2026-08-19. This is a **hard failure**, not silent data loss —
`gamma.search().public_search("sports").search_profiles(true).limit_per_type(20).send()`
returned `Err(Serialization error: invalid type: null, expected struct
SearchProfile)` before the fix. Fixed by typing the field
`Vec<Option<SearchProfile>>` — `Option<T>`'s own `Deserialize` impl already
decodes a `null` array element as `None`, so no custom deserializer was
needed. Verified against `tests/fixtures/search_response_profiles.json` and
enforced by `tests/wire_agreement.rs`'s
`search_response_tolerates_null_profile_entries`. `events` and `tags` were
probed across the same 12 queries (240 event slots, 68 tag slots) and never
observed to contain `null`, so they are left as `Vec<Event>` / `Vec<Tag>`.
