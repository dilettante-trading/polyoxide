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

## `limit` on `GET /events/keyset` is clamped, not validated

**Spec** (`openapi.yaml`, `listEventsKeyset`): `maximum: 100` (lowered from
500 upstream in September 2026).

**Server**, probed 2026-09-23: `limit=100`, `limit=101` and `limit=500` all
return `200` with exactly 100 events. A `maximum` in the schema reads like a
`422` above it; the server clamps instead. A caller that asks for 500 and
treats a short page as the end of the data stops early. Paginate on
`next_cursor`, never on page length.

## `GET /comments/{id}` returns a thread

Upstream's summary is "Get comments by comment id". It returns the root comment
and every reply, with the requested id anywhere in the list. Requesting
`3218542` on 2026-08-19 returned six comments, the requested one third.

## Market-maker fields and filters: dropped from the spec, still served

Upstream's published `openapi.yaml` removed all of the following; the nightly
first flagged it on 2026-09-04 (issue #34):

- `Market`: `marketMakerAddress`, `ammType`, `fpmmLive`, `liquidityAmm`,
  `volumeAmm`, `volume24hrAmm`, `volume1wkAmm`, `volume1moAmm`, `volume1yrAmm`
- `Event`: `liquidityAmm`
- the `market_maker_address` query parameter on `GET /markets` and
  `GET /markets/keyset`
- `marketMakerAddress` in the `POST /markets/information` body

**The server kept every one of them.** Verified 2026-09-14:

| What | Server |
|------|--------|
| `Market.json` (served `$schema`) | still lists all nine `Market` properties, and `marketMakerAddress` is in `required` alongside `id`, `conditionId`, `feeType` |
| `Event.json` (served `$schema`) | still lists `liquidityAmm` |
| `marketMakerAddress` on the wire | present on 100/100 open and 100/100 closed markets sampled, and on all 1,221 markets nested in 100 events; `""` for all but one (id `560317`, an old AMM market) |
| `ammType`, `fpmmLive`, `volume*Amm` on the wire | absent from all 200 sampled markets |
| `liquidityAmm` on the wire | absent from open markets; `0` on 92/100 closed markets |
| `GET /markets?market_maker_address=<560317's address>` | 1 market, id `560317` |
| same, with an address no market has | `[]` |
| `GET /markets/keyset` with that unknown address | `{"markets":[]}` |
| `POST /markets/information` `{"marketMakerAddress":[<560317's address>]}` | 1 market, id `560317`; with an unknown address, `[]` |

So the filter is applied, not ignored. The distinction matters because an
**unknown** body field *is* ignored: `POST /markets/information`
`{"bogusField":["x"]}` returns an unfiltered page of 20. If the server ever drops
the filter, callers will get unfiltered results with no error.

**What polyoxide does.** Following the server, and the served schema that ranks
above this mirror: `Market::market_maker_address` stays a required `String`, the
AMM fields stay `Option`, and `ListMarkets::market_maker_address`,
`ListKeysetMarkets::market_maker_address` and
`MarketsInformationBody::market_maker_address` stay. The removal is a docs change
until the server says otherwise, and the nightly checks for that:
`live_market_maker_address_filter_is_still_applied` in
`polyoxide-gamma/tests/live_api.rs` fails if any of the three routes stops
applying the filter. Deserialization of every live-market test fails if
`marketMakerAddress` disappears from the wire.

## `include_markets` on `GET /events`: undocumented, applied

`openapi.yaml` lists `include_chat` and `include_template` on `GET /events`
but no `include_markets`. The server applies it. Probed 2026-09-23 with
`limit=3&closed=false&order=id&ascending=false`:

| Query | Each event |
|-------|------------|
| no `include_markets` | has `markets` (23 entries) |
| `include_markets=true` | has `markets` (23 entries) |
| `include_markets=false` | no `markets` key at all |

`GET /events/keyset?limit=3&closed=false` behaves the same way: `markets` is
present by default and missing with `include_markets=false`.

So omitting the parameter is the same as `true`, and `false` removes the key
rather than sending `[]`. `ListEvents::include_markets` sends it.
`Event::markets` is `#[serde(default)]`, so the missing key parses as an empty
`Vec`. That empty `Vec` looks the same as an event with no markets, and only
the request says which one it is. `ListKeysetEvents` has no
`include_markets` builder yet.

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
| `GET /markets/{id}` | `Market.json` |
| `GET /markets/keyset` | `MarketsKeysetListResponse.json` (items `$ref` `Market.json`) |
| `GET /events/keyset` | `EventsKeysetListResponse.json` (items `$ref` `Event.json`) |

The first two were found on 2026-08-19; the last three were seen on 2026-09-14
and may have been serving it earlier. `/markets`, `/events`, `/series`, `/tags`,
`/comments` and `/public-search` do **not** send a `$schema` key, so it is not a
universal feature of the API. `Market.json` still describes what `/markets`
returns even though that route does not link it: every key across 200 sampled
`/markets` rows, and across 1,221 markets nested in `/events`, is a
`Market.json` property (2026-09-14).

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
