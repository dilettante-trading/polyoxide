# Gamma type parity sweep (2026-08-19)

Issue #28 fixed three comment types that had been invented during the fork from
polyte: they declared fields the server has never sent and omitted fields it
always sends, and every comment endpoint had been failing at runtime for months
behind a fixture hand-written to match the struct.

This sweep exists to answer one question: **was #28 a one-off, or a class?**

It compares every data type in `polyoxide-gamma` against the vendored spec at
`docs/specs/gamma/openapi.yaml` and — where a live call could settle it —
against payloads captured from `https://gamma-api.polymarket.com`.

The answer is at the bottom. It is **a class**, and one of the instances is
live today on a public endpoint.

---

**IMPORTANT: every claim below is a hypothesis, not a fact.** Each carries a
`file:line`. Open it and confirm the defect before changing anything. If a claim
is wrong, say so and move on — a wrong fix is worse than none. Items that can
only be settled by a live call are marked **LIVE CHECK REQUIRED**, and for those
the verification *is* the first deliverable.

**This document changes no Rust code and files no issues.** It is a worklist.

---

## Evidence base

The vendored spec is the comparison basis but it is **not** the oracle. It is
already known wrong about this API — its `parent_entity_type` enum lists
`market`, which the server rejects, and omits `PerpsAsset`, which the server
accepts (see `docs/specs/gamma/OBSERVED.md`). This sweep found nine more
disagreements; they are collected in the appendix.

Where spec and server disagree, the **server** is right.

### What was compared

30 data types. All 26 in `polyoxide-gamma/src/types.rs`, plus the four that live
in `src/api/`: `UserResponse` and `UserInfo` (`api/user.rs`), `SearchResponse`
and `SearchProfile` (`api/search.rs`). The ~19 remaining `pub struct`s in
`src/api/*.rs` and `src/client.rs` are request builders and client handles
(`ListMarkets`, `GetEvent`, `GammaBuilder`, …) — they carry no wire shape and
are out of scope.

### Live captures

All read-only `GET`s, 31 requests total, on 2026-08-19:

```bash
curl -s 'https://gamma-api.polymarket.com/markets?limit=25&closed=false&order=volumeNum&ascending=false'
curl -s 'https://gamma-api.polymarket.com/markets?limit=20&closed=true&order=id&ascending=false'
curl -s 'https://gamma-api.polymarket.com/markets/2063134?include_tag=true'
curl -s 'https://gamma-api.polymarket.com/markets/keyset?limit=2&closed=false'
curl -s 'https://gamma-api.polymarket.com/events?limit=10&closed=false&order=volume&ascending=false'
curl -s 'https://gamma-api.polymarket.com/events?limit=8&tag_slug=nfl&closed=false'
curl -s 'https://gamma-api.polymarket.com/events?tag_slug=nba&closed=false&limit=6'   # also mlb, epl
curl -s 'https://gamma-api.polymarket.com/events/30829'
curl -s 'https://gamma-api.polymarket.com/events/keyset?limit=2&closed=false'
curl -s 'https://gamma-api.polymarket.com/events/pagination?limit=2&closed=false'
curl -s 'https://gamma-api.polymarket.com/events/creators?limit=2'
curl -s 'https://gamma-api.polymarket.com/series?limit=5&closed=false'
curl -s 'https://gamma-api.polymarket.com/series/1'
curl -s 'https://gamma-api.polymarket.com/series-summary/1'
curl -s 'https://gamma-api.polymarket.com/sports'
curl -s 'https://gamma-api.polymarket.com/teams?limit=5'
curl -s 'https://gamma-api.polymarket.com/tags?limit=5'
curl -s 'https://gamma-api.polymarket.com/tags/1/related-tags'
curl -s 'https://gamma-api.polymarket.com/public-search?q=bitcoin&limit_per_type=2&search_profiles=true&search_tags=true'
curl -s 'https://gamma-api.polymarket.com/public-profile?address=0xc4dced307aad0c8ea95bac8fa0c736062caa2d07'
curl -s 'https://gamma-api.polymarket.com/profiles/user_address/0xc4dced307aad0c8ea95bac8fa0c736062caa2d07'   # + 4 more addresses
curl -s 'https://gamma-api.polymarket.com/comments?limit=3&parent_entity_type=Event&parent_entity_id=30829&get_positions=true'
```

Sample sizes after unnesting: **1,309 `Market` objects**, 36 `Event`, 440
`SportsMetadata`, 5 `Series` (list) + 1 (single), 5 `Team`, 5 `Tag`, 5
`Profile`, 10 `SearchProfile`, 3 `Comment`.

### The one caveat that governs every confidence rating

**Gamma omits null-valued keys from its JSON.** A key absent from a sample is
therefore *not* proof the server never sends it. Confidence below is graded on
that basis:

- **Confirmed** — the wire carries a near-identical key under a different name
  (so the modelled name is provably wrong), *or* a non-`Option` Rust field is
  absent from every sampled payload (so deserialization provably fails), *or*
  the key is present in every sampled payload and unmodelled.
- **High** — absent from every sampled payload **and** absent from the spec.
  Two independent sources.
- **Needs a live sighting** — spec and code disagree and neither was observed.

---

## Already fixed — the pattern being looked for

| Type | Modelled but not sent | Sent but not modelled | Status |
|---|---|---|---|
| `Comment` | `user`, `marketId`, `eventId`, `seriesId`, `parentId`, `positions`, `likeCount`, `dislikeCount`, `replyCount` | `parentEntityType`, `parentEntityID`, `userAddress`, `profile`, `reportCount`, `reactionCount` | Fixed in 0.28.0 |
| `CommentReaction` | `userId` | `id`, `commentID`, `icon`, `userAddress`, `createdAt`, `profile` | Fixed in 0.28.0 |
| `CommentPosition` | `outcome`, `shares` | `positionSize` | Fixed in 0.28.0 |

Re-verified live on 2026-08-19. `GET /comments?parent_entity_type=Event&parent_entity_id=30829`
returns exactly `body`, `createdAt`, `id`, `parentEntityID`, `parentEntityType`,
`profile`, `reactionCount`, `reportCount`, `updatedAt`, `userAddress`, with
`profile` carrying `baseAddress`, `displayUsernamePublic`, `name`, `proxyWallet`,
`pseudonym`. The corrected types match. **No further comment-family findings.**

---

## Priority 1 — A public endpoint that cannot succeed

### 1. `Profile.id` is required and the server never sends it — FIXED

Fixed in the change that pulled this finding into PR #29, alongside the
comment fixes. `Profile` was rewritten field-for-field against the endpoint's
own published JSON Schema (`PublicProfile.json`, linked from the response's
`$schema` key) rather than `openapi.yaml` — see the new section in
`docs/specs/gamma/OBSERVED.md`. `id` is gone (it was never real); `taker_tier`,
`taker_tier_name` and `weighted_volume` are now required, matching the
schema's own `required` list and every capture behind
`tests/fixtures/profile_{full,sparse}.json`. `tests/wire_agreement.rs` now
covers `Profile` in both directions, `mock_api.rs`'s hand-written `"id":
"p-1"` fixture is replaced by a captured payload, and `live_api.rs`'s
`live_get_profile_by_address` now distinguishes a 404 (legitimate skip) from a
deserialization error (hard failure) instead of swallowing both.

Discovering `$schema` also bore on finding #10 below (`UserResponse`, served
by `/public-profile` under `PublicProfileResponse.json`) — that half of the
finding has since been fixed the same way; see #10's update. `SearchProfile`,
the other half of #10, remains open — `/public-search` serves no `$schema`.

The rest of this section is the original finding, kept for the record.

#### Original finding: `Profile.id` is required and the server never sends it

`polyoxide-gamma/src/types.rs:388` — `pub id: String`, non-`Option`, no
`#[serde(default)]`.

`GET /profiles/user_address/{address}` returned HTTP 200 for five different
addresses and **none of the five responses contained an `id` key**:

```
0xc4dced307aad0c8ea95bac8fa0c736062caa2d07 -> id absent; keys: createdAt, name, proxyWallet, pseudonym, takerTier, takerTierName, weightedVolume
0x1584cb3b5a84944bd0db072c43d67189397071b6 -> id absent; keys: bio, createdAt, name, profileImage, proxyWallet, pseudonym, takerTier, takerTierName, weightedVolume
0x4ddbeee0f6a5e7090dde6679bf54ef14376fca89 -> id absent; keys: createdAt, name, proxyWallet, pseudonym, takerTier, takerTierName, weightedVolume
0x9d23146d35052098c2b17297f21e894b4fcab55b -> id absent; keys: bio, createdAt, name, profileImage, proxyWallet, pseudonym, takerTier, takerTierName, weightedVolume
0xed4f4efc03c1db87bb5b60a9d806610dd99be25a -> id absent; keys: bio, createdAt, name, profileImage, proxyWallet, pseudonym, takerTier, takerTierName, weightedVolume
```

`Gamma::user().get_by_address(addr)` (`polyoxide-gamma/src/api/user.rs:25`)
therefore returns `Err(missing field 'id')` for every one of them. This is the
#28 failure mode exactly: **fails at runtime, not merely loses data.**

The type's own doc comment (`types.rs:382-383`) states the opposite —
"All fields except `id` are optional" — so the author reasoned about which field
was guaranteed and picked the one that is never present.

**Two tests exist and neither catches it, for two different reasons:**

- `polyoxide-gamma/tests/mock_api.rs:897-905` serves a hand-written fixture
  containing `"id": "p-1"` — a key the server does not send — and asserts
  `profile.id == "p-1"`. This is the #28 fixture pattern verbatim.
- `polyoxide-gamma/tests/live_api.rs:962` calls the real endpoint but wraps it
  in `if let Ok(profile) = gamma.user().get_by_address(&address).send().await`,
  with the comment *"The endpoint returns 404 for non-profile addresses; treat
  that as a valid contract exercise. Only successful deserializations are
  asserted."* The endpoint returns **200**, not 404 — so the `if let Ok` is
  swallowing a deserialization error, not a 404. The assertion inside
  (`assert!(!profile.id.is_empty(), "profile id must not be empty")`) never
  runs.

The live guard swallows the exact failure it was written to detect. Whatever
else is fixed, **that guard should be tightened first** — `Err` must fail the
test unless it is specifically a 404.

**Fixed looks like:** `id` is `Option<String>`, the mock fixture is replaced by
a captured payload, and the live test distinguishes a 404 from a deserialization
error.

**Confidence: confirmed.** 5/5 live responses, HTTP 200.

`Profile` also drops three keys the server sends on every response —
`takerTier`, `takerTierName`, `weightedVolume` — none of which are in the spec.
See the appendix.

---

## Priority 2 — The `ID`-suffix trap, live on `Market` and `Event`

### 2. `negRiskMarketID` and `negRiskRequestID` are silently `None`

Three fields, all missing an explicit `#[serde(rename)]`:

| Rust field | Derived wire name | Actual wire name | Location |
|---|---|---|---|
| `Market::neg_risk_market_id` | `negRiskMarketId` | `negRiskMarketID` | `polyoxide-gamma/src/types.rs:45` |
| `Market::neg_risk_request_id` | `negRiskRequestId` | `negRiskRequestID` | `polyoxide-gamma/src/types.rs:46` |
| `Event::neg_risk_market_id` | `negRiskMarketId` | `negRiskMarketID` | `polyoxide-gamma/src/types.rs:253` |

Counted across every captured payload:

```
$ grep -ho '"negRiskMarketI[dD]"\|"negRiskRequestI[dD]"' *.json | sort | uniq -c
   1446 "negRiskMarketID"
   1445 "negRiskRequestID"
```

Zero occurrences of either lowercase-`d` form. Sample values from
`GET /markets/2063134`:

```
negRiskMarketID  = '0x55ab76d092f682bf5cbb7e14f13ee12f8410ce7cc1b7906f23b8fb56c11f6500'
negRiskRequestID = '0x8f41e93fb2ebd2b33e72d54940bee621cf7eac4ce795a2403256e4263cba0bf1'
```

Blast radius is wider than the two crates: the field is re-exported to Python at
`polyoxide-py/src/types/gamma.rs:42` and stubbed at
`polyoxide-py/python/polyoxide/__init__.pyi:203`, so Python callers get `None`
with no error at all. `polyoxide-clob`'s own `neg_risk_market_id`
(`polyoxide-clob/src/api/markets.rs:439`) is **correct** — the CLOB API uses
snake_case on the wire, so no rename is needed there. Only the Gamma copies are
wrong.

**Fixed looks like:** all three carry `#[serde(rename = "negRiskMarketID")]` /
`#[serde(rename = "negRiskRequestID")]`, pinned by a `wire_agreement.rs`-style
assertion against a captured market and event.

**Confidence: confirmed.**

### The full `*_id` audit

Every `*_id` / `*_ids` field in the crate, checked individually:

| Type | Rust field | Wire name | Explicit rename? | Verdict |
|---|---|---|---|---|
| `Market` | `condition_id` | `conditionId` | no | correct |
| `Market` | `question_id` | `questionID` | **yes** | correct |
| `Market` | `neg_risk_market_id` | `negRiskMarketId` | no | **defect (#2)** |
| `Market` | `neg_risk_request_id` | `negRiskRequestId` | no | **defect (#2)** |
| `Market` | `clob_token_ids` | `clobTokenIds` | no | correct |
| `Market` | `game_id` | `gameId` | no | correct (see #12 for the type) |
| `MarketToken` | `token_id` | `tokenId` | no | moot — type is unreachable (#9) |
| `Event` | `neg_risk_market_id` | `negRiskMarketId` | no | **defect (#2)** |
| `RelatedTag` | `tag_id` | `tagID` | **yes** | correct |
| `RelatedTag` | `related_tag_id` | `relatedTagID` | **yes** | correct |
| `Comment` | `parent_entity_id` | `parentEntityID` | **yes** | correct |
| `Comment` | `parent_comment_id` | `parentCommentID` | **yes** | correct |
| `CommentReaction` | `comment_id` | `commentID` | **yes** | correct |
| `CommentPosition` | `token_id` | `tokenId` | no | correct |
| `MarketsInformationBody` | `clob_token_ids`, `condition_ids`, `tag_id`, `game_id`, `question_ids` | `clobTokenIds`, `conditionIds`, `tagId`, `gameId`, `questionIds` | no | correct — request body, matches spec |

`GET /tags/1/related-tags` re-confirmed live: `{"id":"34736","tagID":1,"relatedTagID":450,"rank":1}`.

---

## Priority 3 — Silent data loss on the two most-called types

Everything in this section deserializes fine and quietly discards a value. No
runtime failure.

### 3. `Event` drops `volume` and `commentCount`

`polyoxide-gamma/src/types.rs:202`. Both are in the spec **and** on the wire —
`commentCount` in 36/36 sampled events, `volume` in 35/36. From
`GET /events/30829`:

```
volume       = 1262296449.908129
commentCount = 793
```

`Event` models `volume_24hr`, `volume_1wk`, `volume_1mo`, `volume_1yr` but not
the lifetime total — and `volume` is the field `?order=volume` sorts by, so a
caller sorting by it cannot read it back.

**Confidence: confirmed.**

### 4. `Event` drops the entire sports payload

`GET /events?tag_slug=epl&closed=false` returns, on each of the 4 sampled events
that are an actual fixture (4/36 overall — the rest are futures markets), three
keys that `Event` does not model and the spec does not list:

| Wire key | Shape | Note |
|---|---|---|
| `sport` | object | Field-for-field a `SportsMetadata` — the crate already has `SportMetadata` at `types.rs:479` |
| `teams` | array of objects | Field-for-field `Team` — already at `types.rs:495`, `alias` included |
| `gameId` | **integer** (e.g. `90114069`) | Note: `Market::game_id` is `Option<String>` (`types.rs:175`) |

Captured `teams[0]`:

```json
{"id": 100005, "name": "Arsenal FC", "league": "epl", "record": "0-0-0",
 "logo": "https://polymarket-upload.s3.us-east-2.amazonaws.com/Arsenal FC-c2e42b7ef6.png",
 "abbreviation": "ars", "alias": "Arsenal", "createdAt": "2024-09-11T21:12:09.487208Z", ...}
```

The types to hold this already exist, so the fix is three fields, not three
types. Mind `gameId`'s integer-vs-string split between `Event` and `Market`
before typing it — **LIVE CHECK REQUIRED** for `Market::game_id`, which was not
observed on any sampled market.

**Confidence: confirmed** for the three keys, **needs a live sighting** for
`Market::game_id`'s type.

### 5. `Market` drops `startDate` and `endDate`

`polyoxide-gamma/src/types.rs:26-27` models only `endDateIso` / `startDateIso`.
The server sends **both** pairs, and they are not the same value:

```
startDate    = '2026-04-27T21:55:14.576Z'   # timestamp
startDateIso = '2026-04-27'                 # date only
endDate      = '2026-06-01T00:00:00Z'
endDateIso   = '2026-06-01'
```

So the precise timestamps are unreachable through `Market`. Both keys are present
in 1,309/1,309 sampled markets, non-null in all of them, and in the spec.

**Confidence: confirmed.**

### 6. `Market` drops fourteen more wire keys

`polyoxide-gamma/src/types.rs:11`. Counts are non-null occurrences across the
1,309 sampled markets.

| Wire key | In spec? | Non-null | Sample value / shape |
|---|---|---|---|
| `feesEnabled` | yes | 1309 | `false` |
| `cyom` | no — spec has it on `Event` only | 1309 | bool; `Event` models it at `types.rs:264`, `Market` does not |
| `approved` | no | 1309 | `true` |
| `comboStatus` | no | 1309 | `"disabled"` |
| `holdingRewardsEnabled` | no | 1309 | `false` |
| `pagerDutyNotificationEnabled` | no | 1309 | `false` |
| `requiresTranslation` | no | 1309 | `false` |
| `version` | no | 1309 | `"v1"` |
| `feeSchedule` | yes | 1255 | `{"exponent":1,"rate":0.05,"takerOnly":true,"rebateRate":0.25}`; spec has a `FeeSchedule` schema |
| `feeType` | no | 1255 | string |
| `clobRewards` | no | 111 | array; `{"id","conditionId","assetAddress","rewardsAmount","rewardsDailyRate","startDate","endDate"}` — this is the *real* rewards field (contrast #9) |
| `positionIds` | no | 87 | array |
| `events` | yes | 45 | array of parent `Event` objects. Sent on all 45 top-level `/markets` results and on none of the 1,264 markets nested inside an event — where the parent is already the enclosing object. Saves a round trip |
| `marketMetadata` | no | 25 | object |

**Confidence: confirmed** (each observed non-null on the wire).

### 7. `SeriesData` drops 17 of the 27 keys `/series/{id}` returns

`polyoxide-gamma/src/types.rs:339` models 14 fields. `GET /series/1` returns:

```
$schema, active, archived, closed, commentCount, commentsEnabled, competitive,
createdAt, createdBy, description, events, featured, id, layout, new,
publishedAt, recurrence, requiresTranslation, restricted, seriesType, slug,
startDate, ticker, title, updatedAt, updatedBy, volume24hr
```

Unmodelled: `ticker`, `seriesType`, `recurrence`, `layout`, `new`, `featured`,
`restricted`, `publishedAt`, `createdBy`, `updatedBy`, `createdAt`, `updatedAt`,
`commentsEnabled`, `volume24hr`, `startDate`, `commentCount`,
`requiresTranslation`.

All but `requiresTranslation` are in the spec. Note the crate's *other* series
type, `SeriesInfo` (`types.rs:304`), models most of them — so the two series
types have diverged for no stated reason. `SeriesInfo` in turn misses `volume`
and `liquidity`, which the nested `event.series[]` payload does carry.

**Confidence: confirmed.**

### 8. Search and profile types

| Type | Finding | Location |
|---|---|---|
| `SearchResponse` | drops `pagination`; server sends `{"hasMore":true,"totalResults":117849}` and the crate already has a `Pagination` type at `types.rs:681` | `api/search.rs:124` |
| `SearchResponse.tags` | typed `Vec<Tag>`, but the server sends `{"id","label","slug","event_count"}` — the spec's `SearchTag`. Deserializes (all three required `Tag` fields are present) but silently discards `event_count` | `api/search.rs:133` |
| `SearchProfile` | drops `displayUsernamePublic`, sent non-null in 10/10 sampled profiles | `api/search.rs:140` |
| `UserResponse` | drops `takerTier`, `takerTierName`, `weightedVolume` — sent on every `/public-profile` response, absent from the spec | `api/user.rs:39` |
| `UserInfo` | drops `communityMod`; live `users[0]` is `{"id","creator","mod","communityMod"}` | `api/user.rs:71` |
| `SportMetadata` | drops `name` and `primaryTagId`; both non-null in 440/440 rows from `GET /sports` | `types.rs:479` |
| `Team` | drops `color` and `providerId` (5/5 from `GET /teams`), and `ordering` on the 8 teams nested in sports events. `alias` is modelled but absent from all 5 `/teams` rows while present on all 8 nested ones — so it is null-omitted, not dead | `types.rs:495` |
| `Tag` | drops `requiresTranslation`; non-null in 5/5 from `GET /tags` and 69/69 nested in events | `types.rs:423` |

**Confidence: confirmed** for all eight.

---

## Priority 4 — Fields modelled that the server never sends

Dead API surface: always `None` or always empty, no error. Cheap to remove,
and each one is a small lie in the public API.

### 9. `Market::tokens` and the whole `MarketToken` type are unreachable

`polyoxide-gamma/src/types.rs:18` and `types.rs:191`.

`tokens` appears in **0 of 1,309** sampled markets and in **0** of the spec's 140
`Market` properties. Gamma expresses outcomes as `outcomes`, `outcomePrices` and
`clobTokenIds` — three parallel JSON-encoded strings — which `Market` already
models at `types.rs:64-65,109`.

The origin is visible in the source. `polyoxide_clob::api::markets::Market`
(`polyoxide-clob/src/api/markets.rs:422-439`) declares, in this order:
`condition_id`, `question_id`, `tokens`, `rewards`, `minimum_order_size`,
`minimum_tick_size`, `description`, `category`, `end_date_iso`, `question`, …,
`neg_risk`, `neg_risk_market_id`. Gamma's `Market` opens with the same run of
fields, camelCased. Somebody took the CLOB market shape and pointed it at the
Gamma endpoint. The give-away is the type drift that came with it:
`minimum_order_size` is `Option<f64>` in CLOB and `Option<String>` in Gamma.

The same origin explains five more dead `Market` fields — none of them in the
spec, none observed in 1,309 markets:

| Rust field | Location | Gamma's actual field |
|---|---|---|
| `rewards` | `types.rs:20` | `clobRewards` (unmodelled — #6) |
| `minimum_order_size` | `types.rs:21` | `orderMinSize` (modelled, `types.rs:98`) |
| `minimum_tick_size` | `types.rs:22` | `orderPriceMinTickSize` (modelled, `types.rs:97`) |
| `min_incentive_size` | `types.rs:29` | `rewardsMinSize` (modelled, `types.rs:155`) |
| `max_incentive_spread` | `types.rs:30` | `rewardsMaxSpread` (modelled, `types.rs:156`) |

`Market::comment_count` (`types.rs:49`) is a sixth: not in the spec, not on the
wire for markets, though `Event` does carry `commentCount` (#3).

`MarketToken` is re-exported to Python as `MarketToken`
(`polyoxide-py/src/types/gamma.rs:24-26`) alongside the CLOB one as
`ClobMarketToken`, so Python users are offered two classes where only one can
ever hold data.

Note `test_market_with_tokens` (`types.rs:915`) is the #28 fixture pattern
again: a hand-written JSON blob with a `tokens` array, asserting the struct
parses it.

**Confidence: high** — spec and 1,309 live objects agree.

### 10. `UserResponse::address`, `UserResponse::id`, `SearchProfile::address` — PARTIALLY FIXED

Fixed for `UserResponse` and `UserInfo` in the change that pulled this half of
the finding into PR #29. Both were rewritten field-for-field against
`/public-profile`'s own published JSON Schema (`PublicProfileResponse.json`
and, for each `users[]` entry, `PublicProfileUser.json` — both linked from the
response's `$schema` key) rather than `openapi.yaml`, which does not describe
this endpoint at all — see the extended section in
`docs/specs/gamma/OBSERVED.md`. `address` and `id` are gone from
`UserResponse` (neither was ever real); `taker_tier`, `taker_tier_name` and
`weighted_volume` are now required, matching the schema and every capture
behind `tests/fixtures/user_response_{full,sparse}.json` (39-address sample).
`UserInfo::id` is now a required `String` (the schema's only required property
on that object) instead of `Option<String>`, and `UserInfo` gains
`community_mod`, observed absent from 1 of 39 sampled nested entries and so
kept `#[serde(default)]` rather than required. `tests/wire_agreement.rs` now
covers `UserResponse` in both directions, including a hand-written case for
the schema's explicit `null` on `users`; the `types.rs` unit tests that
asserted `"address"`/`"id"` from a hand-written fixture are updated to the
real shape.

**`SearchProfile::address` (`api/search.rs:142`) is still open.** It was
explicitly out of scope for this change, and unlike `/public-profile`,
`/public-search` serves **no** `$schema` key (verified 2026-08-19 — the
response body is `{"events": [...], "profiles": [...], "pagination": {...}}`
with no schema link at any level). Fixing it will need capture-based work
against the live host rather than a served schema, the way `Profile` and
`Comment` were originally approached before `$schema` was discovered.

The rest of this section is the original finding, kept for the record.

#### Original finding

| Field | Location | Doc comment claims |
|---|---|---|
| `UserResponse::address` | `api/user.rs:44` | "The user's EOA address (Signer)" |
| `UserResponse::id` | `api/user.rs:46` | "Account ID" |
| `SearchProfile::address` | `api/search.rs:142` | "User address" |

None appears in the spec (`PublicProfileResponse` has 10 properties, `Profile`
has 23; neither includes `address`, and only `Profile` has `id`). None appears
on the wire. `GET /public-profile?address=…` returns:

```json
{"$schema":"…","createdAt":"2024-06-13T20:58:43.307795Z",
 "proxyWallet":"0xc4dced…","displayUsernamePublic":true,
 "pseudonym":"Handsome-Sweat","name":"the-goal-is-more-bitcoin",
 "users":[{"id":"550876","creator":false,"mod":false,"communityMod":false}],
 "verifiedBadge":false,"takerTier":0,"takerTierName":"Tier 0","weightedVolume":0}
```

`SearchProfile` across 10 search results: the union of keys is `bio`,
`displayUsernamePublic`, `name`, `profileImage`, `proxyWallet`, `pseudonym`.
`address` appears in **0 of 10**; `displayUsernamePublic` in **10 of 10** and is
unmodelled (#8).

This matters more than it looks. `address` is the field a caller reaches for
first when resolving a search hit or a profile, and it is always `None`; the
value they want is under `proxy_wallet` / `proxy`. `test_user_response_full_profile`
(`types.rs:1376`) asserts `"address": "0xsigner"` and `"id": "u1"` from a
hand-written fixture — the #28 pattern, third instance.

**Confidence: high** (spec + live agree). Note `Profile::proxy_wallet`
(`types.rs:409`) and `UserResponse::proxy` (`api/user.rs:42`, explicitly renamed
to `proxyWallet`) are correct.

### 11. `Event::start_date_iso` and `Event::end_date_iso`

`polyoxide-gamma/src/types.rs:215-216`. Absent from all 36 sampled events and
from the spec's 90 `Event` properties. `Event` correctly models the `startDate` /
`endDate` the server does send (`types.rs:210,212`).

These two are real on `Market` (#5) and were evidently copied across.

**Confidence: high.**

### 12. `Event::parent_event` names a field that does not exist, and the real one is an integer

`polyoxide-gamma/src/types.rs:248` — `parent_event: Option<String>` → wire
`parentEvent`. The spec lists `parentEvent`. The server sends **`parentEventId`,
as an integer**:

```
sp_epl.json  epl-ars-cov-2026-08-21-halftime-result      parentEventId = 814080 (int)
sp_epl.json  epl-ars-cov-2026-08-21-second-half-result   parentEventId = 814080 (int)
sp_epl.json  epl-ars-cov-2026-08-21-exact-score          parentEventId = 814080 (int)
```

**This one has a trap in the fix.** Renaming to `parentEventId` while leaving
the type as `String` converts a silent `None` into a hard deserialization
failure on every sub-event. It needs `Option<i64>` and the rename together.

Related: `Event::sub_events: Vec<String>` (`types.rs:257`) was never observed
either; `parentEventId` appears to be how the parent/child link is actually
expressed. **LIVE CHECK REQUIRED** before assuming `subEvents` is dead — it is
in the spec, and 36 events is a small sample for a rare field.

**Confidence: confirmed** for `parentEventId`'s name and integer type;
**needs a live sighting** for whether `subEvents` is ever populated.

### 13. Three `Market` names where spec and code disagree and neither was observed

**LIVE CHECK REQUIRED** — do not guess on any of these.

| Rust field | Rust wire name | Spec name | Location |
|---|---|---|---|
| `denomination_token` | `denomationToken` (explicit rename) | `denominationToken` | `types.rs:57-58` |
| `team_aid` | `teamAid` | `teamAID` | `types.rs:112` |
| `team_bid` | `teamBid` | `teamBID` | `types.rs:113` |

`denomationToken` is spelled as a deliberate typo with the comment
`// API field is "denomationToken" (typo in Polymarket API)` and a test asserting
it (`types.rs:956`, `test_market_denomination_token_rename`) — but that test is
a hand-written fixture, so it is evidence of the author's belief, not of the
wire. The spec spells it correctly. One of the two is wrong and nothing in the
repo can settle it.

`teamAid` / `teamBid` are `teamAID` / `teamBID` in the spec, which would make
them two more instances of the `ID`-suffix trap — but neither casing was
observed across 1,309 markets, including 26 with a non-null `sportsMarketType`.
The sports markets sampled carry team data on the **event** (`teams`, #4), not
the market.

To settle: find a market that actually populates them. `sportsMarketType` is a
good filter, but the sampled sports markets did not carry team fields — a
different sport or an older/closed game market may be needed.

`schedule_deployment_timestamp` (`types.rs:182`, and `Event` at `types.rs:296`)
is a fourth of the same kind: spec says `scheduledDeploymentTimestamp`, code
says `scheduleDeploymentTimestamp`, neither observed. The server does send
`deployingTimestamp` and `pendingDeployment`, which are modelled correctly.

---

## Priority 5 — Nullability

### 14. Spec says `nullable`, Rust says required — none observed to bite

Checked every non-`Option`, non-`Vec`, non-`default` field against every sampled
payload. Apart from `Profile::id` (#1), **no field was ever absent or null**.
These are therefore theoretical, ordered by how exposed they look:

| Field | Location | Spec | Observed |
|---|---|---|---|
| `Market::question` | `types.rs:28` | nullable | present in 1,309/1,309 |
| `Tag::slug`, `Tag::label` | `types.rs:425-426` | nullable | present in all sampled tags |
| `SeriesInfo::slug`, `::title` | `types.rs:306-307` | nullable | present in 5/5 |
| `SeriesData::slug`, `::title` | `types.rs:341-342` | nullable | present in 6/6 |
| `SeriesData::active`, `::closed`, `::archived` | `types.rs:346-348` | nullable | present in 6/6 |

Serde fails a whole `Vec<T>` on one bad element, so a single null row costs the
caller the entire response — the reasoning already written into `RelatedTag`'s
doc comment (`types.rs:453-455`). The same argument applies to `Tag`, which is
returned in arrays from `/tags`, `/events[].tags` and `/public-search`.

Whether to loosen these is a judgement call, not a defect. **Do not loosen them
speculatively** — each one is a breaking change for consumers, and #1 is the
only one with evidence behind it.

The reverse direction (Rust `Option`, spec `required`) produced no findings: the
crate is uniformly more permissive, which is lossless.

### 15. `SeriesData::tags` is typed `Vec<String>`; the spec says `array<Tag>`

`polyoxide-gamma/src/types.rs:350`. Never populated in the sampled responses
(`/series` and `/series/1` both omit it), so the `#[serde(default)]` hides the
question.

If the server ever sends what the spec describes — objects, as
`/events[].tags` and `/markets/{id}?include_tag=true` both do — this is a **hard
deserialization failure**, not data loss.

**LIVE CHECK REQUIRED:** find a series that populates `tags`. If none can be
found, this stays a latent hazard rather than a confirmed defect. Everywhere
else in the crate a `tags` array is `Vec<Tag>` (`Market::tags` `types.rs:43`,
`Event::tags` `types.rs:261`), and `GET /markets/2063134?include_tag=true`
confirms the object shape live:

```json
{"id":"104289","label":"Ethiopia","slug":"ethiopia",
 "createdAt":"2026-03-23T22:42:45.650425Z","updatedAt":"2026-04-17T20:32:29.088718Z",
 "requiresTranslation":false}
```

**Confidence: needs a live sighting.**

---

## Types with no counterpart, and types that are clean

**No spec counterpart, correctly so** — internal helpers, not wire shapes:
`Cursor` (`types.rs:650`), `PaginatedResponse<T>` (`types.rs:658`).
`MarketToken` (`types.rs:191`) also has no counterpart, but for the reason in #9.

**Clean — Rust, spec and wire all agree**, no findings:

| Type | Location | Verified against |
|---|---|---|
| `SeriesSummary` | `types.rs:366` | `GET /series-summary/1`; the mixed casing (`eventDates` camel, `earliest_open_*` snake) documented at `types.rs:360-363` is correct |
| `RelatedTag` | `types.rs:458` | `GET /tags/1/related-tags` |
| `EventCreator` | `types.rs:667` | `GET /events/creators?limit=2` |
| `Pagination` | `types.rs:681` | nested in `/events/pagination` and `/public-search` |
| `EventsPagination` | `types.rs:691` | `GET /events/pagination` |
| `KeysetEventsResponse` | `types.rs:703` | `GET /events/keyset` |
| `KeysetMarketsResponse` | `types.rs:808` | `GET /markets/keyset` |
| `MarketsInformationBody` | `types.rs:728` | 23/23 properties match the spec exactly (request body) |
| `CountResponse` | `types.rs:641` | matches spec `Count` |
| `MarketDescription` | `types.rs:716` | matches spec |
| `Comment`, `CommentProfile`, `CommentReaction`, `CommentPosition`, `ParentEntityType` | `types.rs:522-635` | re-verified live, see above |

All three keyset/pagination responses carry an extra `$schema` key
(`"https://gamma-api.polymarket.com/schemas/…"`). It is metadata, absent from
the spec, and correctly ignored. Not a finding.

---

## Appendix — nine more places the spec disagrees with the server

For `docs/specs/gamma/OBSERVED.md`, alongside the `parent_entity_type` case.
**Do not edit `docs/specs/gamma/openapi.yaml`** — it is a byte-faithful upstream
mirror and any edit makes the nightly drift check alarm forever.

1. **`Market` omits `negRisk`, `negRiskMarketID`, `negRiskRequestID`.** The
   spec carries `negRiskOther` (line ~2242) but none of the other three. Live
   counts over 1,309 markets: `negRisk` 1309, `negRiskOther` 1309,
   `negRiskMarketID` 1246, `negRiskRequestID` 1289. (The spec *does* carry
   `negRiskMarketID` on `Event`, line ~2477 — so it is inconsistent with
   itself.)
2. **`SportsMetadata` omits `id`, `createdAt`, `name`, `primaryTagId`.** The
   spec lists six properties; `GET /sports` returns ten. `SportMetadata` is
   right about `id` and `createdAt` and the spec is wrong.
3. **`PublicProfileResponse` and `Profile` omit `takerTier`, `takerTierName`,
   `weightedVolume`.** Sent on every response from both `/public-profile` and
   `/profiles/user_address/{addr}`.
4. **`PublicProfileUser` omits `communityMod`.**
5. **`Tag` omits `requiresTranslation`.**
6. **`Team` omits `color`, `providerId` and `ordering`.**
7. **`Event` omits `negRiskAugmented`, `countryName`, `cumulativeMarkets`,
   `electionType`, `eventMetadata`, `gameId`, `sport`, `teams`, `parentEventId`,
   `requiresTranslation`, `version`** — and lists `parentEvent`, which the server
   does not send (#12).
8. **`Market` omits `approved`, `clobRewards`, `comboStatus`, `cyom`, `feeType`,
   `holdingRewardsEnabled`, `marketMetadata`, `pagerDutyNotificationEnabled`,
   `positionIds`, `requiresTranslation`, `version`.**
9. **`Series` omits `requiresTranslation`**, and lists `pythTokenID`,
   `cgAssetName`, `subtitle`, `isTemplate`, `templateVariables`, `score`,
   `collections`, `categories`, `chats` — none observed on `/series` or
   `/series/1`. (Weak: null-omission could explain these.)

`version` and `requiresTranslation` appear on `Market`, `Event`, `Series` and
`Tag` and are in the spec for none of them. That looks like one upstream schema
generation lagging one upstream deploy, rather than four separate omissions.

---

## Verdict: #28 is a class, not a one-off

Three independent lines of evidence.

**1. The same runtime failure is live on another endpoint right now.**
`Profile::id` is required and never sent (#1), so `get_by_address` fails for
every address tried. That is not a near-miss — it is #28's exact shape on a
different endpoint: a required field the server does not send, on a public
method, undetected.

**2. The same *cause* is visible in the source.** #28's types were "invented
during a fork". So were these. `Market` opens with a camelCased transcription of
`polyoxide_clob::api::markets::Market` — `tokens`, `rewards`,
`minimum_order_size`, `minimum_tick_size` in that order, with
`minimum_order_size`'s type drifting from `f64` to `String` on the way across
(#9). `Event::start_date_iso` / `end_date_iso` are copied from `Market`, where
they are real (#11). `UserResponse::address` / `id` and `SearchProfile::address`
were invented outright (#10), exactly as `CommentUser` was. This is one fork
artefact with several surface expressions, not several unrelated bugs.

**3. The same *detection failure* is present, in four more tests.** #28
survived "because the only test used a fixture hand-written to match the
struct". So do these:

- `mock_api.rs:897` — fixture contains `"id": "p-1"`, which no server sends.
- `types.rs:915` `test_market_with_tokens` — fixture contains a `tokens` array.
- `types.rs:956` `test_market_denomination_token_rename` — fixture contains the
  disputed typo, with a comment asserting the API has it.
- `types.rs:1376` `test_user_response_full_profile` — fixture contains
  `"address"` and `"id"`.

And the one test that *does* call the live server — `live_api.rs:962` — wraps it
in `if let Ok(…)` on a stated assumption ("returns 404 for non-profile
addresses") that is false: the endpoint returns 200 and the guard is swallowing
a deserialization error.

So the failure mode is intact and general. **Nothing here is fixed by fixing
individual fields.** The load-bearing change is the one #28 already made:
`polyoxide-gamma/tests/wire_agreement.rs` asserts in both directions — nothing
invented, nothing unmodelled, no wildcard in `IGNORED` — against a captured
payload rather than the spec. It currently covers only `Comment`. Extending it
to `Market`, `Event`, `Profile`, `UserResponse` and `SearchResponse` would have
caught every Priority 1-3 finding in this document mechanically, and will catch
the next one.

**Suggested order:** tighten the `live_api.rs:962` guard first (it is three
lines and it is actively hiding #1) → fix #1 → fix #2 → extend
`wire_agreement.rs` to `Market`, `Event` and `Profile`, watch it fail on #3-#8,
then fix those under it.

## Conventions

- MSRV 1.91. `cargo clippy --all-targets --all-features -- -D warnings` clean,
  `cargo fmt --all -- --check` passing, and
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace`
  clean — a red doc build silently withholds the release tag.
- #1, #2, #12 and #15 are breaking (`Profile::id` becomes `Option`, three fields
  change wire name, `parent_event` changes name and type). **Group breaking work
  into one release**; the main consumer pins all six crates in lockstep.
- Any field added or renamed on `Market`/`Event` needs the matching
  `polyoxide-py` binding (`polyoxide-py/src/types/gamma.rs`) and `.pyi` stub
  updated in the same change — the `.pyi` is hand-maintained and drifts.
- Add captured payloads under `polyoxide-gamma/tests/fixtures/` with provenance
  in that directory's `README.md`, as the comment fixtures do. **Prove each test
  fails without the fix.**
- Update `CHANGELOG.md`; mark breaking entries `[**breaking**]`.

---

## Follow-up 16 — the wire-agreement guard has a hole (found 2026-08-19)

`polyoxide-gamma/tests/wire_agreement.rs` does **not** catch an invented
`Option<T>` field. Direction 1 must exempt `null` values, because a genuinely
optional field the server omitted this time also serializes to `null`; that
exemption cannot tell "absent this time" from "does not exist at all".

**Verified empirically:** adding `totally_invented_field: Option<String>` to
`Comment` left all three tests passing.

This is why the guard caught #28 — the old type's invented fields included
required ones (`user: CommentUser`, `like_count: u32`) that failed
deserialization before the assertions ran. Had the fork made them all `Option`,
the guard would have gone green on a type that was still entirely fictional.

Note this bears directly on findings #10 and #11 in this document
(`UserResponse::address`, `UserResponse::id`, `SearchProfile::address`,
`Event::start_date_iso`, `Event::end_date_iso`) — all invented, all `Option`.
Extending the current guard to those types would **not** flag them.

**Possible fix:** for every key the type emits as `null` across all fixtures,
require that the key appear either in the vendored spec's property list for
that schema, or in `IGNORED` with a reason. That uses the spec only to answer
"does this name exist anywhere", which is a question it can answer reliably —
distinct from using it as the authority on values and enums, which
`docs/specs/gamma/OBSERVED.md` shows it cannot.

**Confidence: confirmed by experiment.** Not fixed in 0.28.0.
