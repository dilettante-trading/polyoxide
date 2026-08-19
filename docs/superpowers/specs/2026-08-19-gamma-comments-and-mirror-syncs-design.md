# Gamma Comment Wire Agreement, and Four Mirror Syncs — Design

**Status:** Approved (2026-08-19)
**Author:** aidanb
**Branch:** `aidanb/recent-issues`
**Closes:** #28 (`nightly-behavioral`); #20, #21, #23, #24 (`schema-drift`)
**Completes:** the four mirror syncs deferred by [2026-08-15-data-spec-adoption-design.md](2026-08-15-data-spec-adoption-design.md) under "Out of scope"

## Goal

`polyoxide-gamma`'s comment types describe the payload the server actually sends, and a test makes the alternative impossible.

**Invariant:** no public type in `polyoxide-gamma` serializes a field the server does not send, and no field the server does send goes unmodelled without a written reason.

Secondarily: the four mirror-only drift issues reach their terminal state, emptying the `schema-drift` queue.

## Context

### #28 is not upstream drift

The nightly behavioral check failed on 2026-08-18 and again on 2026-08-19, four tests, one error:

```
list comments: Api(Serialization(Error("missing field `userId`", line: 1, column: 656)))
```

The message is misleading. `userId` is one field of `CommentReaction`, but the mismatch is total. Fetched live on 2026-08-19:

```json
{"id":"3217227","body":"Test comment","parentEntityType":"Event","parentEntityID":2890,
 "userAddress":"0x724f2fdf7adee7f16da4a75cec62aaf8f4a804fe",
 "createdAt":"2026-08-17T16:33:43.877563Z","updatedAt":"2026-08-17T16:33:53.117925Z",
 "profile":{"name":"nktchkes","pseudonym":"Inferior-Comptroller","displayUsernamePublic":true,
            "bio":"ok","proxyWallet":"0x1fa0…","baseAddress":"0x724f…"},
 "reactions":[{"id":"3088560","commentID":3217227,"reactionType":"HEART",
               "userAddress":"0x724f…","profile":{"proxyWallet":"0x1fa0…"}}],
 "reportCount":0,"reactionCount":1}
```

`Comment` at `polyoxide-gamma/src/types.rs:511` shares exactly four field names with that. Decisively, **`docs/specs/gamma/openapi.yaml:2937` already describes the real shape correctly**, including `Reaction.userAddress`. The vendored spec and the wire agree; the Rust types match neither and never did. This is a modelling defect inherited from the polyte fork, not a drift event — which is why no `schema-drift` issue was ever filed for gamma.

### Why serde named only one field

`serde_json` reports *missing* required fields only after consuming the whole map, but deserializes nested structs eagerly. `reactions` is parsed mid-stream, so `CommentReaction`'s absent `userId` aborts at column 656 before the outer struct notices that `user`, `likeCount`, `dislikeCount` and `replyCount` are also absent. A one-field error concealed a total mismatch.

### Why it surfaced on 2026-08-18 and not before

An empty `reactions: []` deserializes fine, and an empty comment list never constructs a `Comment` at all. The tests discover their subject with `events().list().limit(5)`, so which event they land on is luck. It stayed green for months because the discovered events had no comments. On 2026-08-17 someone posted "Test comment" with a HEART reaction on event 2890, and the latent defect became observable. **Tests whose input is discovered rather than pinned fail on the calendar, not on the commit.**

### The only guard was self-referential

`test_comment_deserialization` (`types.rs:1247`) feeds the struct a fixture hand-written to match the struct. It passes, and always would have. This is the shape-only pattern: it proves the type deserializes itself, never that it agrees with a counterparty.

### The published gamma spec is wrong about `parent_entity_type`

Independent of #28. `docs/specs/gamma/openapi.yaml` gives the enum as `[Event, Series, market]`. The CLI at `polyoxide-cli/src/commands/gamma/comments.rs:23` maps its `Market` variant to `"Market"`. Probed live on 2026-08-19:

```
parent_entity_type=Market -> 422 {"error":"expected value to be one of \"Event, Series, PerpsAsset\""}
parent_entity_type=market -> 422 {"error":"expected value to be one of \"Event, Series, PerpsAsset\""}
```

So `polyoxide gamma comments list --parent-entity-type market` is a guaranteed 422 in either casing, and `PerpsAsset` — which the server accepts — is unreachable. gamma has no open drift issue, so the vendored mirror matches upstream's published document and **both disagree with upstream's own server**. The nightly schema check structurally cannot see this: it diffs mirror against published spec, never against the live host. This is the `docs/specs/asyncapi-sports.json` situation in a surface that *is* drift-checked.

### The Python bindings cannot fail loudly

`py_type!` stores the value as `serde_json::Value` and getters resolve by camelCase key lookup (`polyoxide-py/src/convert.rs:23`):

```rust
match value.get(&camel).or_else(|| value.get(field)) {
    Some(v) => value_to_pyobject(py, v),
    None => Ok(py.None()),   // silent
}
```

A stale field list in `py_type!` produces `None`, never an error. The hand-written `.pyi` is the only record that a field was ever supposed to exist, which is why the stub keeps drifting from the macro. Any fix that updates the Rust types without an explicit Python assertion leaves this unguarded.

## Decisions

| # | Question | Decision |
|---|----------|----------|
| 1 | How wide is the audit? | Sweep every type in `polyoxide-gamma/src/types.rs` against the vendored spec. Fix the comment family now; report the rest. |
| 2 | How does the work land? | Two PRs. Docs-only syncs first and immediately; the breaking gamma fix second. |
| 3 | What is the regression guard? | A bidirectional key-set assertion over captured wire payloads, using `serde_json` alone. |
| 4 | What is the guard's oracle? | **The wire, not the vendored spec.** A spec-agreement test would have blessed `parent_entity_type: market`. |
| 5 | How faithful is the new type? | Field-for-field with the wire, `#[non_exhaustive]`, and a typed `ParentEntityType` enum. |
| 6 | Any back-compat shim? | **None.** The removed fields never deserialized against any real response, so no working code depends on them. |
| 7 | Where is the spec/server disagreement recorded? | A new `docs/specs/gamma/OBSERVED.md`. The mirror itself must stay byte-faithful to upstream or drift detection alarms forever. |

### Why the wire and not the spec

Decision 4 reverses the intuitive choice, so it is worth stating plainly. Pinning types to `docs/specs/gamma/openapi.yaml` would hard-code an error we confirmed by probe: the spec's `market` is not accepted and its omission of `PerpsAsset` is wrong. The vendored spec remains the reference for *discovering* mismatches during the sweep — it is how the comment family's defects were found — but it is not the thing CI asserts against. It also happens to be the cheaper option: the wire-based guard needs no new dependencies, whereas a spec-agreement test needs a YAML parser (there is no YAML crate in `Cargo.lock` at all) plus `specta` introspection (`specta` is derive-only here, with no export code anywhere in the workspace).

## PR 1 — `docs(specs)`: sync four mirrors

Four fetches, taken verbatim from the issue bodies:

| Issue | Target | Source |
|---|---|---|
| #23 | `docs/specs/perps/openapi.json` | `https://docs.polymarket.com/api-spec/perps-openapi.json` |
| #20 | `docs/specs/perps/asyncapi.json` | `https://docs.polymarket.com/asyncapi-perps.json` |
| #24 | `docs/specs/bridge/openapi.yaml` | `https://docs.polymarket.com/api-spec/bridge-openapi.yaml` |
| #21 | `docs/specs/combos-rfq/asyncapi.json` | `https://docs.polymarket.com/asyncapi-rfq.json` |

All four are mirror-only: perps, bridge and combos-rfq have no polyoxide client crate, so no code changes and no release. Verification is a `workflow_dispatch` run of `nightly-schema.yml`, which must close all four issues on its own — the same convergence contract S2 established. Nothing in this PR is hand-edited; if a fetch produces a diff the workflow still flags, that is a finding, not something to patch by hand.

## PR 2 — `fix(gamma)!`: model comments against the wire

### Component 1 — the type parity sweep

Diff every type in `polyoxide-gamma/src/types.rs` against `docs/specs/gamma/openapi.yaml`, recording per type: fields modelled but absent from the schema, schema properties unmodelled, and nullability mismatches. Output is `docs/plans/2026-08-19-gamma-type-parity-worklist.md`, ranked by blast radius, following the convention of `docs/plans/2026-07-25-prader-audit-upstream-worklist.md`: every claim is a hypothesis carrying a `file:line`, to be confirmed against the source or a live call before anything is changed.

The comment family is fixed in this PR. Every other finding becomes a tracked issue rather than a change here, so PR 2 stays reviewable. The sweep's purpose is to answer whether #28 is a one-off or a class; that answer belongs in the report either way.

Three members are already confirmed:

| Type | Modelled but not sent | Sent but not modelled |
|---|---|---|
| `Comment` | `user`, `marketId`, `eventId`, `seriesId`, `parentId`, `positions`, `likeCount`, `dislikeCount`, `replyCount` | `parentEntityType`, `parentEntityID`, `userAddress`, `profile`, `reportCount`, `reactionCount` |
| `CommentReaction` | `userId` | `id`, `commentID`, `icon`, `userAddress`, `createdAt`, `profile` |
| `CommentPosition` | `outcome`, `shares` | `positionSize` |

`CommentUser` is deleted. No such schema exists; the real object is `CommentProfile`.

### Component 2 — the types

```rust
#[non_exhaustive]
pub struct Comment {
    pub id: String,
    pub body: Option<String>,
    pub parent_entity_type: Option<ParentEntityType>,
    pub parent_entity_id: Option<i64>,
    pub parent_comment_id: Option<String>,
    pub user_address: Option<String>,
    pub reply_address: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub profile: Option<CommentProfile>,
    #[serde(default)]
    pub reactions: Vec<CommentReaction>,
    pub report_count: Option<i64>,
    pub reaction_count: Option<i64>,
}

pub enum ParentEntityType {
    Event,
    Series,
    PerpsAsset,
    /// An entity type this client does not recognize (forward-compat).
    #[serde(other)]
    Unknown,
}
```

As built, `ParentEntityType` uses `#[serde(other)] Unknown` rather than
`#[non_exhaustive]`, following the house pattern already established in
`polyoxide-data` (`TradeSide::Unknown`, `ActivityType::Unknown`). Both defend
against upstream adding a variant, but `#[non_exhaustive]` only blocks
exhaustive matching in downstream crates — it does nothing for
deserialization, which is where an unrecognized value actually arrives. A
fallback variant handles the real failure mode: one new entity type does not
fail the whole response.

`CommentProfile` carries `name`, `pseudonym`, `display_username_public`, `bio`, `is_mod`, `is_creator`, `proxy_wallet`, `base_address`, `profile_image`, `profile_image_optimized`, `positions`. The last two follow the existing precedent at `types.rs:409`:

```rust
/// ImageOptimization payload; kept as raw JSON since the upstream shape
/// is not yet modelled in this crate.
#[cfg_attr(feature = "specta", specta(skip))]
pub profile_image_optimized: Option<serde_json::Value>,
```

`CommentReaction` and `CommentPosition` are corrected to the table above and both gain `#[non_exhaustive]`.

Two naming notes. Rust type names stay client-side — `CommentReaction`, not the spec's `Reaction` — because field names are the wire contract and type names are not. And `parent_entity_id` is `i64`, not `String`: the wire sends `2890` unquoted.

#### The `ID` suffix defeats `rename_all`

Four wire keys end in a capitalised `ID`: `parentEntityID`, `parentCommentID`, `commentID` and (on `ImageOptimization`) `relID`. `#[serde(rename_all = "camelCase")]` turns `parent_entity_id` into `parentEntityId`, which is **not** what the server sends. Every one of these needs an explicit rename:

```rust
#[serde(rename = "parentEntityID")]
pub parent_entity_id: Option<i64>,
```

The same trap exists one layer down. `py_type!`'s `snake_to_camel` produces `parentEntityId` too, so these fields must use the macro's explicit-key form, which exists for exactly this reason (`convert.rs:34` cites `questionID` as the precedent):

```rust
parent_entity_id => "parentEntityID",
```

This is worth stating because it is invisible to review: `rename_all` looks like it covers the whole struct, and a wrong key here produces `None` in Python and a missing-field error in Rust that names a field the reader can see in the payload. Assertion 2 of Component 5 catches it, which is the point — but the implementer should not need the test to find it.

### Component 3 — the two API bugs

`Comments::get()` at `api/comments.rs:20` is typed `Request<Comment, GammaError>`, but `GET /comments/{id}` returns an array. Confirmed live and stated in the spec at `openapi.yaml:1485`. It becomes `Request<Vec<Comment>, GammaError>`. Faithful over ergonomic: callers write `.into_iter().next()`, and the type does not lie about arity. As built, the response is not merely an array but the whole thread — the root comment first, with the requested id appearing anywhere in the list, not necessarily first — confirmed live on 2026-08-19 (`get_comment_by_id_returns_the_whole_thread` in `polyoxide-gamma/tests/mock_api.rs`).

`ListComments::parent_entity_type` takes `ParentEntityType` instead of `impl Into<String>`, so the CLI's invalid `market` value stops being expressible. As built, the CLI keeps its own local `ParentEntityType` enum rather than deriving the clap value-enum from the library type: `clap::ValueEnum` can only be derived on a type defined in the crate deriving it, and `polyoxide_gamma::types::ParentEntityType` is foreign to `polyoxide-cli`. The CLI enum instead carries a `From<cli::ParentEntityType> for polyoxide_gamma::types::ParentEntityType` impl (`polyoxide-cli/src/commands/gamma/comments.rs`) and omits `Unknown`, which is not a filter a user can meaningfully ask for.

### Component 4 — bindings

`polyoxide-py/src/types/gamma.rs`: `py_type!` field lists updated for `PyComment`, `PyCommentReaction`, `PyCommentPosition`; `PyCommentUser` deleted; `PyCommentProfile` added. The enum needs no Python machinery — it serializes to a string, and the getter returns a `str`.

`polyoxide-py/python/polyoxide/__init__.pyi:369-437`: stubs rewritten to match.

### Component 5 — the guard

`polyoxide-gamma/tests/wire_agreement.rs`, with captured payloads under `polyoxide-gamma/tests/fixtures/`. Each fixture carries a provenance header — source URL, fetch date, and the entity id it was captured from — so a stale fixture can be recaptured rather than guessed at.

```rust
let wire: Value = serde_json::from_str(CAPTURED)?;
let typed: Comment = serde_json::from_value(wire.clone())?;
let emitted = serde_json::to_value(&typed)?;
```

Two assertions, both of which fail on today's code:

1. **No invented fields.** Every key `emitted` contains with a non-null value must exist in `wire`. Null-valued keys are exempt, since `Option::None` serializes to `null` for a field the server legitimately omits. Today `likeCount` fails this.
2. **No unmodelled fields.** Every key in `wire` is either modelled or listed in an `IGNORED` constant with a written reason. Today `userAddress` fails this.

The `IGNORED` list is the design's escape hatch and is deliberately awkward: skipping a field is a written act in a reviewed diff, not an omission.

### Component 6 — the live tests

All four failing tests in `polyoxide-gamma/tests/live_api.rs` read `comment.user.id` and pass it to `/public-profile?address=` — an id where an address belongs, which would have been wrong even had it compiled against real data. They are rewritten to pass `comment.user_address`, which is what that endpoint wants.

`live_list_comments` keeps its discovery-based subject but gains an explicit assertion that at least one comment was returned for the chosen event, escalating "nothing to check" from a silent pass to a visible skip. This is what distinguishes an environmental result from a green one, and `classify_failures.py` already understands the difference.

## Failure handling

The `IGNORED` list in Component 5 is the only place a wire field may be dropped, and it requires a reason string. There is no wildcard.

Recapture is manual and expected: when upstream adds a field, assertion 2 fails in CI with the field named. The fix is to model it or to ignore it explicitly — both are diffs someone reads.

Assertion 1 is satisfied by removing an invented field, or by declaring it in `EXPECTED_ABSENT` (Component 5) with a written reason — an explicit, reviewed exemption, not a weakening of the assertion itself. An earlier draft of this document claimed assertion 1 could *only* be satisfied by removal, reasoning that a field the server never sends has no non-null value to emit. That reasoning was only as sound as the `null`/empty-array exemption it depended on, and that exemption is exactly what let an invented `Option<T>` field pass unnoticed — a 2026-08-19 review disproved the claim by adding one and watching all three `wire_agreement.rs` tests stay green. `EXPECTED_ABSENT` replaces the blanket exemption: every key the type emits must be present on the wire or named here with a reason, so a field with no legitimate excuse still fails.

## Testing

As built, the tests are named and split differently than sketched above —
across `polyoxide-gamma/tests/wire_agreement.rs`, `polyoxide-gamma/tests/mock_api.rs`,
`polyoxide-cli/src/commands/gamma/comments.rs`, and `polyoxide-py/tests/test_comment_types.py`:

| Test | Pins |
|---|---|
| `full_comment_agrees_with_captured_payload` | Both assertions (Component 5) against `tests/fixtures/comment_full.json`, a comment with a reaction. Fails on today's code: invents `likeCount` and leaves `userAddress` unmodelled. |
| `sparse_comment_agrees_with_captured_payload` | Both assertions against `tests/fixtures/comment_sparse.json`, a comment with most optional fields absent — exercises the null/empty exemptions in the "no invented fields" direction. |
| `id_suffixed_keys_keep_their_wire_casing` | `parentEntityID`, `parentCommentID` and `commentID` round-trip with their capitalised suffix and `parentEntityId` is absent; `rename_all` alone would emit the latter and fail. |
| `get_comment_by_id_returns_the_whole_thread` | Mock test pinning `GET /comments/{id}` to a two-element JSON array (root then reply), asserting the root comes first and the requested id appears somewhere in the thread; fails against the old single-object signature. |
| `list_comments_sends_typed_parent_entity_type` | Mock test asserting the query string carries `parent_entity_type=PerpsAsset`, not a string a caller could misspell as `market`. |
| `market_is_no_longer_accepted` | The CLI's `--parent-entity-type market` fails to parse. |
| `parent_entity_type_perps_asset` | The CLI's `--parent-entity-type perps-asset` parses and converts to `polyoxide_gamma::types::ParentEntityType::PerpsAsset`. |
| `test_every_comment_getter_resolves` (pytest) | Asserts `hasattr` on the *class* for each field name in the `py_type!` list — catches a field dropped from the list entirely, but not a stale rename, since PyO3 registers the descriptor regardless of which JSON key it resolves. It is not a defence against the silent-`None` path; a 2026-08-19 review showed it cannot fail on one. |
| `comment_getters_resolve_against_the_shared_fixture` (Rust, `polyoxide-py/src/types/gamma.rs`) | Builds a real `PyComment` from the shared fixture and reads every getter, added 2026-08-19 as the instance-level defence the pytest version could not provide. |

`test_comment_deserialization`, the old self-referential fixture test, is deleted; the fixture-driven tests above replace it.

Live: the four `--ignored` gamma tests must pass against the real host before merge, not only in the next nightly.

## Version

Breaking changes to public types in `polyoxide-gamma` take the workspace from **0.27.0 to 0.28.0**. Pre-1.0, and no working code can depend on types that never deserialized a real response.

Publishing order is unchanged; gamma is already in the release workflow's sequence.

## Documentation

`CHANGELOG.md` records the break, naming the removed fields so a reader upgrading sees why their code stops compiling.

`docs/specs/gamma/OBSERVED.md` is new: the `parent_entity_type` probe, its date, and the reasoning for why the mirror is not edited to match. CLAUDE.md gains one line under the API Specs section pointing at it, alongside the existing note about the sports AsyncAPI mirror — the two are instances of the same phenomenon.

`docs/plans/2026-08-19-gamma-type-parity-worklist.md` is the sweep's output.

## Out of scope

Non-comment findings from the sweep are reported and tracked, not fixed here.

The same audit across `polyoxide-data`, `polyoxide-clob` and `polyoxide-relay`. If the gamma sweep shows this is a class rather than a one-off, that becomes its own piece of work with its own breaking releases.

Rewriting `py_type!` so field lists cannot go stale. The pytest assertion in Component 4 covers the comment types; making the macro structurally safe for every type is a separate change.

The CLI gaining `comments get` / `comments by-user` subcommands. `Comments::get` and `Comments::by_user` remain library-only.
