# Gamma Comment Wire Agreement, and Four Mirror Syncs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `polyoxide-gamma`'s comment types describe the payload the server actually sends, guarded by a test that fails without the fix, and empty the `schema-drift` issue queue.

**Architecture:** Two PRs. PR 1 (Task 1) is four `curl`s into `docs/specs/**` with no code. PR 2 (Tasks 2–12) rewrites the comment family against captured wire payloads, adds a bidirectional key-set guard that reproduces issue #28 offline, and propagates the change through the CLI and Python bindings.

**Tech Stack:** Rust 1.91, `serde`/`serde_json`, `mockito` for HTTP mocks, `cargo nextest`, PyO3 + maturin for bindings, `pytest` via `uv` for the Python side.

**Spec:** [2026-08-19-gamma-comments-and-mirror-syncs-design.md](../specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md)

---

## Deviations from the spec

Five things were discovered while capturing fixtures, after the spec was approved. Where they conflict with the spec, **this plan wins** and the spec is corrected in Task 11.

1. **`GET /comments/{id}` returns the whole thread, not one comment.** Requesting `3218542` returns six comments — the thread root first, the requested id third. `Vec<Comment>` is confirmed correct, but `get()`'s doc comment must say "thread", and the live test cannot assert `result[0].id == requested`.
2. **`ParentEntityType` uses `#[serde(other)] Unknown`, not `#[non_exhaustive]`.** This is the house pattern (`ApprovalFeature`/`ApprovalStandard` at `polyoxide-data/src/types.rs:361,382`) and it already solves forward-compat, making `#[non_exhaustive]` redundant on the enum. Structs keep `#[non_exhaustive]`.
3. **`clap::ValueEnum` cannot be derived on a foreign type.** The CLI keeps a local enum and maps it to the library type, rather than deriving on `polyoxide_gamma::types::ParentEntityType`. This also keeps `Unknown` out of the CLI's `--help`.
4. **`limit` counts top-level comments, not returned rows.** `limit=2` returns 8 rows, `limit=5` returns 18 — replies accompany their parents. Documentation only; no code change.
5. **Four spec properties never appear in 159 captured comments:** `Reaction.icon`, `Reaction.createdAt`, `CommentProfile.isMod`, `CommentProfile.isCreator`, `CommentProfile.profileImageOptimized`. They are modelled as `Option` per the spec. This is safe: the guard's assertion 1 exempts `null`, so a modelled-but-never-sent optional field does not fail.

## Empirical basis

Every field decision below comes from a survey of **159 real comments** fetched on 2026-08-19 from `parent_entity_id=45915`, plus the `/comments/{id}` and `/comments/user_address/{addr}` shapes. Key frequencies:

| Key | Present | Consequence |
|---|---|---|
| `id`, `body`, `parentEntityType`, `parentEntityID`, `userAddress`, `createdAt`, `profile`, `reportCount`, `reactionCount` | 159/159 | modelled |
| `updatedAt` | 158/159 | must be `Option` |
| `parentCommentID`, `replyAddress` | 95/159 | must be `Option` |
| `reactions` | 86/159 | needs `#[serde(default)]` |
| `profile.positions` | 113/159 | needs `#[serde(default)]` |
| `profile.bio` | 33/159 | must be `Option` |

Reaction keys, all 267 occurrences: `id`, `commentID`, `reactionType`, `userAddress`, `profile`. Position keys, all 277: `tokenId`, `positionSize`.

Note on `Option`: serde's derive treats a missing key as `None` for `Option<T>` fields without needing `#[serde(default)]`. `#[serde(default)]` **is** required for `Vec<T>`.

## File structure

**PR 1 — docs only**

| File | Responsibility |
|---|---|
| `docs/specs/perps/openapi.json` | Mirror of upstream perps REST spec (replaced wholesale) |
| `docs/specs/perps/asyncapi.json` | Mirror of upstream perps WS spec (replaced wholesale) |
| `docs/specs/bridge/openapi.yaml` | Mirror of upstream bridge spec (replaced wholesale) |
| `docs/specs/combos-rfq/asyncapi.json` | Mirror of upstream RFQ WS spec (replaced wholesale) |

**PR 2 — code**

| File | Responsibility |
|---|---|
| `polyoxide-gamma/tests/fixtures/comment_full.json` | Captured payload exercising every modelled key |
| `polyoxide-gamma/tests/fixtures/comment_sparse.json` | Captured payload with `profile` and `reactions` absent |
| `polyoxide-gamma/tests/fixtures/README.md` | Provenance: source URL, fetch date, what was trimmed |
| `polyoxide-gamma/tests/wire_agreement.rs` | The bidirectional guard. Owns `IGNORED`. |
| `polyoxide-gamma/src/types.rs` | `Comment`, `CommentProfile`, `CommentReaction`, `CommentPosition`, `ParentEntityType` |
| `polyoxide-gamma/src/api/comments.rs` | `get()` arity fix; typed `parent_entity_type` |
| `polyoxide-gamma/tests/mock_api.rs` | Mock test pinning the thread-array response |
| `polyoxide-gamma/tests/live_api.rs` | The four failing tests, corrected |
| `polyoxide-cli/src/commands/gamma/comments.rs` | Local clap enum mapping to the library type |
| `polyoxide-py/src/types/gamma.rs` | `py_type!` field lists |
| `polyoxide-py/python/polyoxide/__init__.pyi` | Hand-written stubs |
| `polyoxide-py/tests/test_types.py` | Guards the silent-`None` getter path |
| `docs/specs/gamma/OBSERVED.md` | Where the published spec disagrees with the server |
| `docs/plans/2026-08-19-gamma-type-parity-worklist.md` | Sweep output |

---

## Task 1: PR 1 — sync four mirrors

**Files:**
- Modify: `docs/specs/perps/openapi.json`
- Modify: `docs/specs/perps/asyncapi.json`
- Modify: `docs/specs/bridge/openapi.yaml`
- Modify: `docs/specs/combos-rfq/asyncapi.json`

This task has no tests of its own — the nightly workflow is the test, and it runs in step 4.

- [ ] **Step 1: Fetch all four upstream specs**

```bash
cd "$(git rev-parse --show-toplevel)"
curl -fsSL https://docs.polymarket.com/api-spec/perps-openapi.json  -o docs/specs/perps/openapi.json
curl -fsSL https://docs.polymarket.com/asyncapi-perps.json          -o docs/specs/perps/asyncapi.json
curl -fsSL https://docs.polymarket.com/api-spec/bridge-openapi.yaml -o docs/specs/bridge/openapi.yaml
curl -fsSL https://docs.polymarket.com/asyncapi-rfq.json            -o docs/specs/combos-rfq/asyncapi.json
```

`-f` matters: without it a 404 page is written into the mirror as if it were a spec.

- [ ] **Step 2: Confirm all four changed and are still parseable**

```bash
git diff --stat docs/specs/
python3 -c "
import json
for p in ['docs/specs/perps/openapi.json','docs/specs/perps/asyncapi.json','docs/specs/combos-rfq/asyncapi.json']:
    json.load(open(p)); print('ok', p)
"
head -3 docs/specs/bridge/openapi.yaml
```

Expected: four files in `git diff --stat`; three `ok` lines; the YAML head shows `openapi:` or similar, not HTML.

If a file shows **no** diff, stop — the issue said it drifted, so either it was already synced or the URL is wrong. Investigate rather than proceeding.

- [ ] **Step 3: Commit**

```bash
git add docs/specs/
git commit -m "docs(specs): sync perps, perps-ws, bridge and combos-rfq mirrors

Adopts the upstream specs verbatim per the instructions in each drift issue.
All four are mirror-only APIs with no polyoxide client crate, so there is no
code change and no release.

Closes #23
Closes #20
Closes #24
Closes #21"
```

- [ ] **Step 4: Verify the workflow closes the issues**

Push the branch, open the PR, merge it, then:

```bash
gh workflow run nightly-schema.yml
sleep 90
gh run list --workflow=nightly-schema.yml --limit 1
gh issue list --label schema-drift --state open
```

Expected: the run concludes `success` and `gh issue list` prints nothing. The workflow closes the issues itself — do **not** close them by hand. If an issue stays open, its diff still exists; read the refreshed issue body rather than re-running.

---

## Task 2: Capture the fixtures

**Files:**
- Create: `polyoxide-gamma/tests/fixtures/comment_full.json`
- Create: `polyoxide-gamma/tests/fixtures/comment_sparse.json`
- Create: `polyoxide-gamma/tests/fixtures/README.md`

- [ ] **Step 1: Create the fixtures directory and the full payload**

```bash
mkdir -p polyoxide-gamma/tests/fixtures
```

Write `polyoxide-gamma/tests/fixtures/comment_full.json`:

```json
{
  "id": "3218542",
  "body": "Marçal é arriscado demais, quase certeza que a candidatura vai ser impugnada, pra mim a questão é so de tempo. Caso os votos sejam anulados somente dps do 1T, há espaço pra crescer nas odds até lá. Porém, pode sair a notícia da impugnação amanhã e tudo cai pra 0 imediatamente",
  "parentEntityType": "Event",
  "parentEntityID": 45915,
  "parentCommentID": "3218360",
  "userAddress": "0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab",
  "replyAddress": "0x3f70913f63616e1c9dc1975c092c3e682f7a9931",
  "createdAt": "2026-08-18T22:56:51.490062Z",
  "updatedAt": "2026-08-18T22:57:01.175744Z",
  "profile": {
    "name": "h0ip0ll0i",
    "displayUsernamePublic": true,
    "bio": "polsci grad / i.r. bachelor",
    "proxyWallet": "0x95bac246a983529e6a57feae41ecc028357d3a5c",
    "baseAddress": "0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab",
    "profileImage": "https://polymarket-upload.s3.us-east-2.amazonaws.com/profile-image-156744-562c2c6c-5dff-4831-bbb4-8b218da9bcf6.png",
    "positions": [
      {
        "tokenId": "30630994248667897740988010928640156931882346081873066002335460180076741328029",
        "positionSize": "290001537"
      },
      {
        "tokenId": "70059117674007036240163073645113489994895807359788046023215512784288966325609",
        "positionSize": "1666666666"
      }
    ]
  },
  "reactions": [
    {
      "id": "3089692",
      "commentID": 3218542,
      "reactionType": "HEART",
      "userAddress": "0x3f70913f63616e1c9dc1975c092c3e682f7a9931",
      "profile": {
        "proxyWallet": "0x512bf947c3798c7041d05eff9a1afd8632719030"
      }
    }
  ],
  "reportCount": 0,
  "reactionCount": 1
}
```

The unicode in `body` is deliberate — it proves UTF-8 survives the round-trip.

- [ ] **Step 2: Create the sparse payload**

Write `polyoxide-gamma/tests/fixtures/comment_sparse.json`:

```json
{
  "id": "2136267",
  "body": "It's over, Tarcisio bros [link removed]",
  "parentEntityType": "Event",
  "parentEntityID": 45915,
  "userAddress": "0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab",
  "createdAt": "2025-12-05T18:44:45.20799Z",
  "updatedAt": "2025-12-05T18:44:56.642122Z",
  "reportCount": 0,
  "reactionCount": 2
}
```

This one carries no `profile` and no `reactions` keys at all. It is what proves the `Option`/`#[serde(default)]` handling is right, and it is the shape `/comments/user_address/{addr}` returns.

- [ ] **Step 3: Record provenance**

Write `polyoxide-gamma/tests/fixtures/README.md`:

```markdown
# Captured Gamma payloads

These are real responses from `gamma-api.polymarket.com`, used by
`tests/wire_agreement.rs` to assert that the comment types agree with what the
server sends. They are the oracle, deliberately in preference to
`docs/specs/gamma/openapi.yaml` — see
`docs/superpowers/specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md`
decision 4, and `docs/specs/gamma/OBSERVED.md` for a case where the published
spec is wrong.

| File | Source | Fetched | Notes |
|---|---|---|---|
| `comment_full.json` | `GET /comments?parent_entity_type=Event&parent_entity_id=45915&limit=64&get_positions=true` | 2026-08-19 | One comment (id `3218542`) selected as the only one of 159 carrying every optional key. Verbatim except `profile.positions` truncated 6→2 and `reactions` truncated 3→1. **No keys added or removed.** |
| `comment_sparse.json` | `GET /comments/user_address/0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab?limit=1` | 2026-08-19 | Verbatim. Carries no `profile` and no `reactions` key. |

## Recapturing

When `wire_agreement.rs` fails because upstream added a field, refetch with the
command above and re-trim. Do not hand-edit a fixture to make a test pass —
that reintroduces the self-referential fixture that caused issue #28.
```

- [ ] **Step 4: Verify the fixtures are valid JSON**

```bash
python3 -c "
import json
for p in ['polyoxide-gamma/tests/fixtures/comment_full.json','polyoxide-gamma/tests/fixtures/comment_sparse.json']:
    d=json.load(open(p)); print(p, '->', len(d), 'keys')
"
```

Expected: `comment_full.json -> 13 keys` and `comment_sparse.json -> 9 keys`.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-gamma/tests/fixtures/
git commit -m "test(gamma): capture live comment payloads as fixtures

Two real responses with provenance, to be used as the oracle for type
agreement in place of the vendored spec."
```

---

## Task 3: The guard test (RED)

**Files:**
- Create: `polyoxide-gamma/tests/wire_agreement.rs`

This task deliberately ends with a failing test. That failure **is** issue #28, reproduced offline with no network.

- [ ] **Step 1: Write the failing test**

Write `polyoxide-gamma/tests/wire_agreement.rs`:

```rust
//! Agreement between the comment types and payloads captured from the live
//! Gamma host. Provenance is in `tests/fixtures/README.md`.
//!
//! Two directions, both of which must fail if a type drifts from the wire:
//!
//! 1. **No invented fields.** Every non-null, non-empty value the type emits
//!    corresponds to a key the server actually sent.
//! 2. **No unmodelled fields.** Every key the server sent is either modelled
//!    or listed in `IGNORED` with a written reason.
//!
//! The oracle is the captured payload, not `docs/specs/gamma/openapi.yaml`.
//! The published spec is known to be wrong about this API — see
//! `docs/specs/gamma/OBSERVED.md`.

use polyoxide_gamma::types::Comment;
use serde_json::Value;

const FULL: &str = include_str!("fixtures/comment_full.json");
const SPARSE: &str = include_str!("fixtures/comment_sparse.json");

/// Wire keys deliberately left unmodelled, each with a reason.
///
/// Adding an entry is a written decision that shows up in a reviewed diff.
/// There is no wildcard. Paths are dotted from the root, e.g.
/// `comment.profile.someKey`.
const IGNORED: &[(&str, &str)] = &[];

/// Walk a captured payload against what the type re-emits, asserting both
/// directions at every level of nesting.
fn check(wire: &Value, emitted: &Value, path: &str) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            // Direction 1: nothing invented.
            for (key, value) in e {
                // `Option::None` serializes to null, and `#[serde(default)]`
                // on a Vec serializes to []. Neither claims the server sent
                // anything, so neither can be an invented field.
                if value.is_null() || matches!(value, Value::Array(a) if a.is_empty()) {
                    continue;
                }
                assert!(
                    w.contains_key(key),
                    "{path}.{key} is emitted by the type but absent from the captured \
                     payload — the field is invented"
                );
            }
            // Direction 2: nothing unmodelled.
            for (key, value) in w {
                match e.get(key) {
                    Some(emitted_value) => check(value, emitted_value, &format!("{path}.{key}")),
                    None => {
                        let full = format!("{path}.{key}");
                        assert!(
                            IGNORED.iter().any(|(k, _)| *k == full.as_str()),
                            "{full} is sent by the server but not modelled, and not listed \
                             in IGNORED with a reason"
                        );
                    }
                }
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            for (i, (wi, ei)) in w.iter().zip(e).enumerate() {
                check(wi, ei, &format!("{path}[{i}]"));
            }
        }
        _ => {}
    }
}

fn round_trip(fixture: &str, path: &str) {
    let wire: Value = serde_json::from_str(fixture).expect("fixture is valid JSON");
    let typed: Comment = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into Comment: {e}"));
    let emitted = serde_json::to_value(&typed).expect("Comment must serialize");
    check(&wire, &emitted, path);
}

#[test]
fn full_comment_agrees_with_captured_payload() {
    round_trip(FULL, "comment");
}

#[test]
fn sparse_comment_agrees_with_captured_payload() {
    round_trip(SPARSE, "comment");
}

#[test]
fn id_suffixed_keys_keep_their_wire_casing() {
    let wire: Value = serde_json::from_str(FULL).expect("fixture is valid JSON");
    let typed: Comment = serde_json::from_value(wire).expect("payload deserializes");
    let emitted = serde_json::to_value(&typed).expect("Comment serializes");

    // `rename_all = "camelCase"` would produce `parentEntityId` here, which the
    // server neither sends nor accepts.
    assert!(
        emitted.get("parentEntityID").is_some(),
        "parentEntityID must keep its capitalised suffix"
    );
    assert!(
        emitted.get("parentEntityId").is_none(),
        "rename_all must not be allowed to win over the explicit rename"
    );
    assert!(
        emitted.get("parentCommentID").is_some(),
        "parentCommentID must keep its capitalised suffix"
    );
    assert!(
        emitted["reactions"][0].get("commentID").is_some(),
        "commentID must keep its capitalised suffix"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p polyoxide-gamma --all-features --test wire_agreement
```

Expected: all three tests FAIL. The first two panic with

```
captured payload must deserialize into Comment: missing field `userId`
```

which is verbatim the error from issue #28 — now reproduced with no network and no discovered data. The third fails the same way.

If instead they pass, stop: the types were already fixed by someone else and this plan needs rebasing.

- [ ] **Step 3: Commit the failing test**

```bash
git add polyoxide-gamma/tests/wire_agreement.rs
git commit -m "test(gamma): add wire-agreement guard, currently failing

Reproduces issue #28 offline: the captured payload cannot deserialize into
Comment. Both directions of the assertion fail against the current types."
```

---

## Task 4: `ParentEntityType`

**Files:**
- Modify: `polyoxide-gamma/src/types.rs`

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block at the bottom of `polyoxide-gamma/src/types.rs`:

```rust
    // ── ParentEntityType ────────────────────────────────────────

    #[test]
    fn parent_entity_type_matches_server_accepted_values() {
        // Probed live 2026-08-19: the server rejects anything else with
        // `expected value to be one of "Event, Series, PerpsAsset"`.
        assert_eq!(ParentEntityType::Event.to_string(), "Event");
        assert_eq!(ParentEntityType::Series.to_string(), "Series");
        assert_eq!(ParentEntityType::PerpsAsset.to_string(), "PerpsAsset");
    }

    #[test]
    fn parent_entity_type_deserializes_known_values() {
        let v: ParentEntityType = serde_json::from_str("\"PerpsAsset\"").unwrap();
        assert_eq!(v, ParentEntityType::PerpsAsset);
    }

    #[test]
    fn parent_entity_type_tolerates_unknown_values() {
        // Upstream added PerpsAsset without warning; assume it will add more.
        let v: ParentEntityType = serde_json::from_str("\"SomethingNew\"").unwrap();
        assert_eq!(v, ParentEntityType::Unknown);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p polyoxide-gamma --all-features --lib parent_entity_type
```

Expected: FAIL to compile, `cannot find type ParentEntityType in this scope`.

- [ ] **Step 3: Write the implementation**

First add `use std::fmt;` to the imports at the top of `polyoxide-gamma/src/types.rs` — the file currently imports only `std::collections::HashMap` and `chrono`/`serde`, so `fmt` is not yet in scope.

Then add this immediately above the `Comment` struct:

```rust
/// What a comment is attached to.
///
/// Probed against the live host on 2026-08-19: the server accepts exactly
/// `Event`, `Series` and `PerpsAsset`, and rejects anything else with
/// `expected value to be one of "Event, Series, PerpsAsset"`.
///
/// The vendored spec disagrees — it lists `market`, which the server rejects,
/// and omits `PerpsAsset`. See `docs/specs/gamma/OBSERVED.md`.
///
/// [`ParentEntityType::Unknown`] absorbs values added upstream after this
/// release so that one new entity type cannot fail an entire response. It is
/// never sent as a filter.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParentEntityType {
    /// Comments on an event.
    Event,
    /// Comments on a series.
    Series,
    /// Comments on a perpetual futures asset.
    PerpsAsset,
    /// An entity type this client does not recognize (forward-compat).
    #[serde(other)]
    Unknown,
}

impl fmt::Display for ParentEntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Event => write!(f, "Event"),
            Self::Series => write!(f, "Series"),
            Self::PerpsAsset => write!(f, "PerpsAsset"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p polyoxide-gamma --all-features --lib parent_entity_type
```

Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-gamma/src/types.rs
git commit -m "feat(gamma): add ParentEntityType with the server's accepted values

Event, Series and PerpsAsset, confirmed by probe. The vendored spec's
\`market\` is rejected by the server and is deliberately absent."
```

---

## Task 5: Rewrite the comment types (GREEN)

**Files:**
- Modify: `polyoxide-gamma/src/types.rs:507-560` (the `Comment` … `CommentPosition` block)
- Modify: `polyoxide-gamma/src/types.rs:1246-1273` (`test_comment_deserialization`)

- [ ] **Step 1: Replace the four type definitions**

Delete `Comment`, `CommentUser`, `CommentReaction` and `CommentPosition` as they currently stand, and put this in their place:

```rust
/// Comment on an event, series, or perps asset.
///
/// Modelled against payloads captured from the live host; see
/// `tests/fixtures/README.md`. Three keys carry an explicit
/// `#[serde(rename)]` because the wire capitalises the `ID` suffix and
/// `rename_all = "camelCase"` would emit `parentEntityId` instead.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Comment {
    pub id: String,
    pub body: Option<String>,
    pub parent_entity_type: Option<ParentEntityType>,
    #[serde(rename = "parentEntityID")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub parent_entity_id: Option<i64>,
    /// Set on replies; absent on thread roots.
    #[serde(rename = "parentCommentID")]
    pub parent_comment_id: Option<String>,
    /// Author's base (EOA) address. This is what `/public-profile?address=`
    /// wants — not any id field.
    pub user_address: Option<String>,
    /// Address being replied to. Set on replies only.
    pub reply_address: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub profile: Option<CommentProfile>,
    /// Absent from the payload entirely when a comment has no reactions.
    #[serde(default)]
    pub reactions: Vec<CommentReaction>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub report_count: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub reaction_count: Option<i64>,
}

/// Author profile embedded in a comment.
///
/// Upstream's schema calls this `CommentProfile`; it is not the same shape as
/// [`Profile`], which comes from `/profiles/user_address/{address}`.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CommentProfile {
    pub name: Option<String>,
    pub pseudonym: Option<String>,
    pub display_username_public: Option<bool>,
    pub bio: Option<String>,
    pub is_mod: Option<bool>,
    pub is_creator: Option<bool>,
    pub proxy_wallet: Option<String>,
    pub base_address: Option<String>,
    pub profile_image: Option<String>,
    /// ImageOptimization payload; kept as raw JSON since the upstream shape
    /// is not yet modelled in this crate.
    #[cfg_attr(feature = "specta", specta(skip))]
    pub profile_image_optimized: Option<serde_json::Value>,
    /// Populated only when the request sets `get_positions(true)`.
    #[serde(default)]
    pub positions: Vec<CommentPosition>,
}

/// Reaction to a comment.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CommentReaction {
    pub id: String,
    #[serde(rename = "commentID")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub comment_id: Option<i64>,
    pub reaction_type: Option<String>,
    pub icon: Option<String>,
    pub user_address: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub profile: Option<CommentProfile>,
}

/// Position held by a comment author, shown alongside their comment.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CommentPosition {
    pub token_id: Option<String>,
    /// Raw integer string in token base units, e.g. `"290001537"`.
    pub position_size: Option<String>,
}
```

`CommentUser` is deleted outright — no such schema exists upstream and the server never sent it.

The `#[serde(rename)]` pattern is not new to this file: `Market::question_id` at `polyoxide-gamma/src/types.rs:13` already carries `#[serde(rename = "questionID")]` for exactly this reason. Follow it.

- [ ] **Step 2: Delete the self-referential unit test**

Remove `test_comment_deserialization` from the `mod tests` block entirely (it starts at `fn test_comment_deserialization()` and ends at the closing brace before `// ── UserResponse ──`). It asserts a fixture invented to match the old struct; keeping it would preserve the exact pattern that caused #28. `tests/wire_agreement.rs` replaces it.

- [ ] **Step 3: Run the guard to verify it now passes**

```bash
cargo test -p polyoxide-gamma --all-features --test wire_agreement
```

Expected: 3 passed. If `sparse_comment_agrees_with_captured_payload` fails with an invented-field message naming `reactions`, the `matches!(value, Value::Array(a) if a.is_empty())` exemption in Task 3 was dropped — restore it.

- [ ] **Step 4: Run the whole gamma suite**

```bash
cargo test -p polyoxide-gamma --all-features
```

Expected: PASS, except compilation errors in `tests/live_api.rs` referring to `comment.user` — those are fixed in Task 8. If the crate will not compile, proceed to Task 8 and return here.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-gamma/src/types.rs
git commit -m "fix(gamma)!: model comments against the wire, not the fork's invention

Comment, CommentReaction and CommentPosition described a payload the server
has never sent. CommentUser had no upstream counterpart at all and is removed.
The replacement is field-for-field with captured responses and with
docs/specs/gamma/openapi.yaml, which already described the real shape.

Fixes #28"
```

---

## Task 6: `get()` returns a thread

**Files:**
- Modify: `polyoxide-gamma/src/api/comments.rs:20-25`
- Modify: `polyoxide-gamma/tests/mock_api.rs`

- [ ] **Step 1: Write the failing test**

Append to `polyoxide-gamma/tests/mock_api.rs`:

```rust
// ── /comments/{id} ─────────────────────────────────────────────

#[tokio::test]
async fn get_comment_by_id_returns_the_whole_thread() {
    let mut server = Server::new_async().await;

    // Probed live 2026-08-19: requesting one comment id returns the entire
    // thread — root first, the requested id somewhere inside it.
    let mock = server
        .mock("GET", "/comments/3218542")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"id": "3218360", "body": "root", "parentEntityType": "Event",
                 "parentEntityID": 45915, "reportCount": 0, "reactionCount": 7},
                {"id": "3218542", "body": "reply", "parentEntityType": "Event",
                 "parentEntityID": 45915, "parentCommentID": "3218360",
                 "reportCount": 0, "reactionCount": 1}
            ]"#,
        )
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let thread = gamma.comments().get("3218542").send().await.unwrap();

    assert_eq!(thread.len(), 2);
    assert!(
        thread.iter().any(|c| c.id == "3218542"),
        "the requested comment must be somewhere in the thread"
    );
    assert_eq!(thread[0].id, "3218360", "the root comes first, not the request");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p polyoxide-gamma --all-features --test mock_api get_comment_by_id_returns_the_whole_thread
```

Expected: FAIL to compile — `thread.len()` does not exist on `Comment`, because `get()` currently returns a single `Comment`.

- [ ] **Step 3: Change the signature**

In `polyoxide-gamma/src/api/comments.rs`, replace the `get` method:

```rust
    /// Get the comment thread containing `id` (`GET /comments/{id}`).
    ///
    /// Despite the name, upstream returns the **whole thread** — the root
    /// comment and every reply — not just the comment identified by `id`.
    /// Confirmed by probe on 2026-08-19: requesting `3218542` returned six
    /// comments with the requested one third in the list. Callers wanting the
    /// single comment must search the result:
    ///
    /// ```no_run
    /// # async fn f(gamma: &polyoxide_gamma::Gamma) -> Result<(), polyoxide_gamma::GammaError> {
    /// let thread = gamma.comments().get("3218542").send().await?;
    /// let this = thread.iter().find(|c| c.id == "3218542");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self, id: impl Into<String>) -> Request<Vec<Comment>, GammaError> {
        Request::new(
            self.http_client.clone(),
            format!("/comments/{}", urlencoding::encode(&id.into())),
        )
    }
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p polyoxide-gamma --all-features --test mock_api get_comment_by_id_returns_the_whole_thread
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add polyoxide-gamma/src/api/comments.rs polyoxide-gamma/tests/mock_api.rs
git commit -m "fix(gamma)!: GET /comments/{id} returns a thread, not one comment

The endpoint returns the root comment and every reply. Typing it as a single
Comment made the call fail outright on a sequence."
```

---

## Task 7: Type the `parent_entity_type` filter

**Files:**
- Modify: `polyoxide-gamma/src/api/comments.rs:69-73`
- Modify: `polyoxide-cli/src/commands/gamma/comments.rs:7-26`

- [ ] **Step 1: Write the failing test**

Append to `polyoxide-gamma/tests/mock_api.rs`:

```rust
#[tokio::test]
async fn list_comments_sends_typed_parent_entity_type() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/comments")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("parent_entity_type".into(), "PerpsAsset".into()),
            Matcher::UrlEncoded("parent_entity_id".into(), "42".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let gamma = test_gamma(&server);
    let comments = gamma
        .comments()
        .list()
        .parent_entity_type(polyoxide_gamma::types::ParentEntityType::PerpsAsset)
        .parent_entity_id(42)
        .send()
        .await
        .unwrap();

    assert!(comments.is_empty());
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p polyoxide-gamma --all-features --test mock_api list_comments_sends_typed_parent_entity_type
```

Expected: FAIL to compile — `parent_entity_type` takes `impl Into<String>`, and `ParentEntityType` does not implement `Into<String>`.

- [ ] **Step 3: Change the builder method**

In `polyoxide-gamma/src/api/comments.rs`, replace `parent_entity_type`. Add `use crate::types::ParentEntityType;` to the file's imports.

```rust
    /// Filter by parent entity type.
    ///
    /// [`ParentEntityType::Unknown`] is not a filter the server understands,
    /// so passing it sends no parameter at all — mirroring
    /// `ListActivity::activity_type` in `polyoxide-data`.
    pub fn parent_entity_type(mut self, entity_type: ParentEntityType) -> Self {
        if entity_type != ParentEntityType::Unknown {
            self.request = self.request.query("parent_entity_type", entity_type);
        }
        self
    }
```

`Request::query` takes `impl ToString`, which the `Display` impl from Task 4 satisfies.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p polyoxide-gamma --all-features --test mock_api list_comments_sends_typed_parent_entity_type
```

Expected: PASS.

- [ ] **Step 5: Fix the CLI**

`clap::ValueEnum` cannot be derived on a type from another crate, so the CLI keeps a local enum and maps it over. This also keeps `Unknown` out of `--help`. In `polyoxide-cli/src/commands/gamma/comments.rs`, replace the local enum and its impl:

```rust
/// Parent entity type for comments.
///
/// Mirrors [`polyoxide_gamma::types::ParentEntityType`] minus its `Unknown`
/// forward-compat variant, which is not a filter a user can meaningfully ask
/// for. `market` is deliberately absent: the server rejects it.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ParentEntityType {
    /// Event comments
    Event,
    /// Series comments
    Series,
    /// Perpetual futures asset comments
    PerpsAsset,
}

impl From<ParentEntityType> for polyoxide_gamma::types::ParentEntityType {
    fn from(value: ParentEntityType) -> Self {
        match value {
            ParentEntityType::Event => Self::Event,
            ParentEntityType::Series => Self::Series,
            ParentEntityType::PerpsAsset => Self::PerpsAsset,
        }
    }
}
```

In the same file's `run`, replace `request.parent_entity_type(pet.as_str())` with:

```rust
                if let Some(pet) = parent_entity_type {
                    request = request.parent_entity_type(pet.into());
                }
```

- [ ] **Step 6: Fix the CLI's tests**

In the same file's `mod tests`, the three `parent_entity_type_*` tests call `pet.as_str()`, which no longer exists. Replace `parent_entity_type_market` entirely and adjust the other two:

```rust
    #[test]
    fn parent_entity_type_event() {
        let cmd = parse(&["test", "list", "--parent-entity-type", "event"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_type, ..
            } => {
                let pet = parent_entity_type.unwrap();
                assert!(matches!(pet, ParentEntityType::Event));
                assert_eq!(
                    polyoxide_gamma::types::ParentEntityType::from(pet).to_string(),
                    "Event"
                );
            }
        }
    }

    #[test]
    fn parent_entity_type_series() {
        let cmd = parse(&["test", "list", "--parent-entity-type", "series"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_type, ..
            } => {
                let pet = parent_entity_type.unwrap();
                assert!(matches!(pet, ParentEntityType::Series));
                assert_eq!(
                    polyoxide_gamma::types::ParentEntityType::from(pet).to_string(),
                    "Series"
                );
            }
        }
    }

    #[test]
    fn parent_entity_type_perps_asset() {
        let cmd = parse(&["test", "list", "--parent-entity-type", "perps-asset"]);
        match cmd {
            CommentsCommand::List {
                parent_entity_type, ..
            } => {
                let pet = parent_entity_type.unwrap();
                assert!(matches!(pet, ParentEntityType::PerpsAsset));
                assert_eq!(
                    polyoxide_gamma::types::ParentEntityType::from(pet).to_string(),
                    "PerpsAsset"
                );
            }
        }
    }

    #[test]
    fn market_is_no_longer_accepted() {
        // Probed live 2026-08-19: the server rejects both `market` and
        // `Market` with a 422. It must not be reachable from the CLI.
        assert_parse_err(&["test", "list", "--parent-entity-type", "market"]);
    }
```

The existing `invalid_parent_entity_type_errors` test stays as it is.

- [ ] **Step 7: Run both crates' tests**

```bash
cargo test -p polyoxide-gamma --all-features --test mock_api
cargo test -p polyoxide-cli --all-features
```

Expected: both PASS.

- [ ] **Step 8: Commit**

```bash
git add polyoxide-gamma/src/api/comments.rs polyoxide-gamma/tests/mock_api.rs polyoxide-cli/src/commands/gamma/comments.rs
git commit -m "fix(cli)!: drop the rejected \`market\` entity type, add PerpsAsset

The server accepts Event, Series and PerpsAsset. \`--parent-entity-type market\`
was a guaranteed 422 in either casing, and PerpsAsset was unreachable. The
filter is now typed in the library rather than passed as a free string."
```

---

## Task 8: Fix the live tests

**Files:**
- Modify: `polyoxide-gamma/tests/live_api.rs:700-725` (`live_list_comments`)
- Modify: `polyoxide-gamma/tests/live_api.rs:730-765` (`live_get_comment_by_id`)
- Modify: `polyoxide-gamma/tests/live_api.rs:855-890` (`live_get_user`)
- Modify: `polyoxide-gamma/tests/live_api.rs:891-935` (`live_get_profile_by_address`)

All four call `.parent_entity_type("Event")` with a string and three read `comment.user.id`. Both are now compile errors.

- [ ] **Step 1: Add the import**

`polyoxide-gamma/tests/live_api.rs` currently imports only `use polyoxide_gamma::Gamma;`. Add a second line beneath it:

```rust
use polyoxide_gamma::types::ParentEntityType;
```

- [ ] **Step 2: Update every call site**

Replace all four occurrences of:

```rust
        .parent_entity_type("Event")
```

with:

```rust
        .parent_entity_type(ParentEntityType::Event)
```

- [ ] **Step 3: Fix `live_list_comments`**

Replace its final two lines (`// Some events may have no comments…` and `let _ = comments;`) with:

```rust
    // An empty result is not signal: the discovered event may simply have no
    // comments, which is exactly the luck that let issue #28 hide for months.
    // Say so out loud rather than passing silently.
    if comments.is_empty() {
        eprintln!(
            "no comments on event {event_id}; this test could not legitimately \
             exercise deserialization"
        );
        return;
    }
    assert!(
        comments.iter().all(|c| !c.id.is_empty()),
        "every comment must carry an id"
    );
```

The `legitimately` wording matters: `.github/scripts/classify_failures.py` matches `legitimately time out` to classify environmental results. Confirm the phrasing there before relying on it — see Task 12, step 2.

- [ ] **Step 4: Fix `live_get_comment_by_id`**

Replace the `if let Some(comment) = comments.first() { … }` block with:

```rust
    let Some(comment) = comments.first() else {
        eprintln!("no comments on event {event_id}; nothing to fetch by id");
        return;
    };
    let thread = gamma
        .comments()
        .get(&comment.id)
        .send()
        .await
        .expect("get comment thread by id");
    // Upstream returns the whole thread, with the requested id somewhere
    // inside it — not necessarily first.
    assert!(
        thread.iter().any(|c| c.id == comment.id),
        "the requested comment must appear in the returned thread"
    );
```

- [ ] **Step 5: Fix `live_get_user`**

Replace its `if let Some(comment) = comments.first() { … }` block with:

```rust
    let Some(comment) = comments.first() else {
        eprintln!("no comments on event {event_id}; no address to resolve");
        return;
    };
    // `/public-profile` wants an address. The old code passed `comment.user.id`,
    // which was an id-shaped field that never existed on the wire.
    let Some(address) = comment.user_address.as_deref() else {
        eprintln!("comment {} carries no userAddress", comment.id);
        return;
    };
    let user = gamma
        .user()
        .get(address)
        .send()
        .await
        .expect("get user profile");
    let _ = user;
```

- [ ] **Step 6: Fix `live_get_profile_by_address`**

Replace `gamma.user().get(&comment.user.id)` with:

```rust
    let Some(user_address) = comment.user_address.as_deref() else {
        return;
    };
    let user = gamma
        .user()
        .get(user_address)
        .send()
        .await
        .expect("resolve user to proxy wallet");
```

- [ ] **Step 7: Verify the crate compiles and the live tests pass**

```bash
cargo test -p polyoxide-gamma --all-features --no-run
cargo test -p polyoxide-gamma --all-features --test live_api -- --ignored
```

Expected: compiles clean; the four comment tests PASS against the real host. This is the check that closes #28 — a green nightly is not enough evidence on its own, because the nightly's subject is discovered.

- [ ] **Step 8: Commit**

```bash
git add polyoxide-gamma/tests/live_api.rs
git commit -m "test(gamma): fix the four live comment tests

They read comment.user.id — a field the wire never carried — and passed it to
/public-profile, which wants an address. They now use userAddress, tolerate the
thread shape of GET /comments/{id}, and report an empty result out loud instead
of passing silently."
```

---

## Task 9: Python bindings

**Files:**
- Modify: `polyoxide-py/src/types/gamma.rs:126-171`
- Modify: `polyoxide-py/python/polyoxide/__init__.pyi:369-437`
- Create: `polyoxide-py/tests/test_comment_types.py`

`py_type!` getters resolve by camelCase key lookup and return `None` for anything missing (`polyoxide-py/src/convert.rs:23`), so a stale field list fails silently. The pytest in step 4 is the only thing that catches it.

- [ ] **Step 1: Update the `py_type!` invocations**

In `polyoxide-py/src/types/gamma.rs`, replace the `PyComment`, `PyCommentUser`, `PyCommentReaction` and `PyCommentPosition` blocks with:

```rust
py_type!(
    PyComment,
    "Comment",
    polyoxide_gamma::types::Comment,
    id,
    body,
    parent_entity_type,
    parent_entity_id => "parentEntityID",
    parent_comment_id => "parentCommentID",
    user_address,
    reply_address,
    created_at,
    updated_at,
    profile,
    reactions,
    report_count,
    reaction_count,
);

py_type!(
    PyCommentProfile,
    "CommentProfile",
    polyoxide_gamma::types::CommentProfile,
    name,
    pseudonym,
    display_username_public,
    bio,
    is_mod,
    is_creator,
    proxy_wallet,
    base_address,
    profile_image,
    profile_image_optimized,
    positions,
);

py_type!(
    PyCommentReaction,
    "CommentReaction",
    polyoxide_gamma::types::CommentReaction,
    id,
    comment_id => "commentID",
    reaction_type,
    icon,
    user_address,
    created_at,
    profile,
);

py_type!(
    PyCommentPosition,
    "CommentPosition",
    polyoxide_gamma::types::CommentPosition,
    token_id,
    position_size,
);
```

`PyCommentUser` is deleted. The `=> "key"` form is required for the three `ID`-suffixed keys because `snake_to_camel` would produce `parentEntityId`.

- [ ] **Step 2: Deregister `PyCommentUser`**

```bash
grep -rn "PyCommentUser" polyoxide-py/src/
```

Remove every hit — there will be a module registration alongside the definition. Add `PyCommentProfile` to the same registration list, following the pattern of the entry you removed.

- [ ] **Step 3: Update the stubs**

In `polyoxide-py/python/polyoxide/__init__.pyi`, replace the `Comment`, `CommentUser`, `CommentReaction` and `CommentPosition` classes with:

```python
class Comment:
    """A comment on an event, series, or perps asset."""
    @property
    def id(self) -> Any: ...
    @property
    def body(self) -> Any: ...
    @property
    def parent_entity_type(self) -> Any: ...
    @property
    def parent_entity_id(self) -> Any: ...
    @property
    def parent_comment_id(self) -> Any: ...
    @property
    def user_address(self) -> Any: ...
    @property
    def reply_address(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def updated_at(self) -> Any: ...
    @property
    def profile(self) -> Any: ...
    @property
    def reactions(self) -> Any: ...
    @property
    def report_count(self) -> Any: ...
    @property
    def reaction_count(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CommentProfile:
    """The author profile embedded in a comment."""
    @property
    def name(self) -> Any: ...
    @property
    def pseudonym(self) -> Any: ...
    @property
    def display_username_public(self) -> Any: ...
    @property
    def bio(self) -> Any: ...
    @property
    def is_mod(self) -> Any: ...
    @property
    def is_creator(self) -> Any: ...
    @property
    def proxy_wallet(self) -> Any: ...
    @property
    def base_address(self) -> Any: ...
    @property
    def profile_image(self) -> Any: ...
    @property
    def profile_image_optimized(self) -> Any: ...
    @property
    def positions(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CommentReaction:
    """A reaction on a comment."""
    @property
    def id(self) -> Any: ...
    @property
    def comment_id(self) -> Any: ...
    @property
    def reaction_type(self) -> Any: ...
    @property
    def icon(self) -> Any: ...
    @property
    def user_address(self) -> Any: ...
    @property
    def created_at(self) -> Any: ...
    @property
    def profile(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class CommentPosition:
    """A position held by a comment author."""
    @property
    def token_id(self) -> Any: ...
    @property
    def position_size(self) -> Any: ...
    def to_dict(self) -> dict[str, Any]: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
```

Also remove `CommentUser` from any `__all__` list in the stub or in `polyoxide-py/python/polyoxide/__init__.py`, and add `CommentProfile`:

```bash
grep -rn "CommentUser" polyoxide-py/python/
```

- [ ] **Step 4: Write the test that guards the silent-`None` path**

Create `polyoxide-py/tests/test_comment_types.py`:

```python
"""Guards the py_type! field lists for the comment family.

py_type! getters resolve by camelCase key lookup and return None for any key
that is not there (polyoxide-py/src/convert.rs:23), so a stale field list is
invisible without an explicit assertion. Every getter below has a value in the
captured payload; a None means the macro's key does not match the wire.
"""

import json
import pathlib

import polyoxide

FIXTURE = (
    pathlib.Path(__file__).parents[2]
    / "polyoxide-gamma"
    / "tests"
    / "fixtures"
    / "comment_full.json"
)


def test_fixture_is_present():
    assert FIXTURE.is_file(), f"missing shared fixture: {FIXTURE}"


def test_every_comment_getter_resolves():
    wire = json.loads(FIXTURE.read_text())
    expected = {
        "id": "3218542",
        "parent_entity_type": "Event",
        "parent_entity_id": 45915,
        "parent_comment_id": "3218360",
        "user_address": "0x8ef197aa37f6d0ec4c5802b40c34ec358e4da6ab",
        "reply_address": "0x3f70913f63616e1c9dc1975c092c3e682f7a9931",
        "report_count": 0,
        "reaction_count": 1,
    }
    # Sanity: the fixture really does carry these, under wire spelling.
    assert wire["parentEntityID"] == expected["parent_entity_id"]
    assert wire["parentCommentID"] == expected["parent_comment_id"]

    for attr in expected:
        assert hasattr(polyoxide.Comment, attr), (
            f"Comment.{attr} is missing from the py_type! field list"
        )
    for attr in ("name", "pseudonym", "proxy_wallet", "base_address", "positions"):
        assert hasattr(polyoxide.CommentProfile, attr), (
            f"CommentProfile.{attr} is missing from the py_type! field list"
        )
    for attr in ("id", "comment_id", "reaction_type", "user_address", "profile"):
        assert hasattr(polyoxide.CommentReaction, attr), (
            f"CommentReaction.{attr} is missing from the py_type! field list"
        )
    for attr in ("token_id", "position_size"):
        assert hasattr(polyoxide.CommentPosition, attr), (
            f"CommentPosition.{attr} is missing from the py_type! field list"
        )


def test_comment_user_is_gone():
    # It had no upstream counterpart and was removed in 0.28.0.
    assert not hasattr(polyoxide, "CommentUser")
```

- [ ] **Step 5: Build and run**

```bash
cd polyoxide-py
uv run maturin develop
uv run pytest tests/test_comment_types.py -v
```

Expected: 3 passed. A failure naming a specific attribute means that entry is missing from the `py_type!` list in step 1.

- [ ] **Step 6: Commit**

```bash
cd "$(git rev-parse --show-toplevel)"
git add polyoxide-py/
git commit -m "fix(py)!: align the comment bindings with the corrected types

Adds CommentProfile, removes CommentUser, and uses py_type!'s explicit-key form
for the three ID-suffixed wire keys that snake_to_camel would mangle. Adds the
first test of the getters, which otherwise return None silently."
```

---

## Task 10: The type parity sweep

**Files:**
- Create: `docs/plans/2026-08-19-gamma-type-parity-worklist.md`

- [ ] **Step 1: Enumerate every type and its schema counterpart**

```bash
cd "$(git rev-parse --show-toplevel)"
grep -n "^pub struct \|^pub enum " polyoxide-gamma/src/types.rs
grep -nE "^    [A-Z][A-Za-z]+:$" docs/specs/gamma/openapi.yaml
```

- [ ] **Step 2: For each type, diff its serde field names against the schema properties**

For every `pub struct` in `types.rs`, list its fields under `rename_all = "camelCase"` (applying any explicit `#[serde(rename)]`) and compare against the matching schema's `properties` keys. Record three categories per type: modelled-but-absent, sent-but-unmodelled, nullability mismatches.

Watch for the `ID` trap throughout — `relID` at `docs/specs/gamma/openapi.yaml:1765` is a fourth instance beyond the three fixed in this plan.

- [ ] **Step 3: Write the worklist**

Create `docs/plans/2026-08-19-gamma-type-parity-worklist.md`, following the format of `docs/plans/2026-07-25-prader-audit-upstream-worklist.md`. It must open with the same warning that every claim is a hypothesis carrying a `file:line`, to be confirmed before anything is changed, and mark items settleable only by a live call as **LIVE CHECK REQUIRED**.

Include the three already-confirmed entries as resolved, so a reader sees the pattern the sweep is looking for:

```markdown
| Type | Modelled but not sent | Sent but not modelled | Status |
|---|---|---|---|
| `Comment` | `user`, `marketId`, `eventId`, `seriesId`, `parentId`, `positions`, `likeCount`, `dislikeCount`, `replyCount` | `parentEntityType`, `parentEntityID`, `userAddress`, `profile`, `reportCount`, `reactionCount` | Fixed in 0.28.0 |
| `CommentReaction` | `userId` | `id`, `commentID`, `icon`, `userAddress`, `createdAt`, `profile` | Fixed in 0.28.0 |
| `CommentPosition` | `outcome`, `shares` | `positionSize` | Fixed in 0.28.0 |
```

- [ ] **Step 4: File an issue per finding**

For each unresolved finding, one issue:

```bash
gh issue create \
  --title "gamma type parity: <TypeName> disagrees with the published schema" \
  --label "bug" \
  --body "Found by the 2026-08-19 gamma type parity sweep.

See docs/plans/2026-08-19-gamma-type-parity-worklist.md.

**Modelled but not sent:** …
**Sent but not modelled:** …

Unverified — confirm against \`docs/specs/gamma/openapi.yaml\` and a live call before changing anything."
```

If the sweep finds nothing beyond the comment family, say so explicitly in the worklist. That is a real result and it answers the question the sweep was run to answer.

- [ ] **Step 5: Commit**

```bash
git add docs/plans/2026-08-19-gamma-type-parity-worklist.md
git commit -m "docs: record the gamma type parity sweep"
```

---

## Task 11: Documentation and version

**Files:**
- Create: `docs/specs/gamma/OBSERVED.md`
- Modify: `CLAUDE.md`
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml`
- Modify: `docs/superpowers/specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md`

- [ ] **Step 1: Record where the published spec is wrong**

Create `docs/specs/gamma/OBSERVED.md`:

```markdown
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
```

- [ ] **Step 2: Confirm the classifier phrasing used in Task 8**

```bash
grep -n "legitimately\|ENVIRONMENTAL_RE\|environmental" .github/scripts/classify_failures.py
```

If the pattern is not `legitimately time out`, change the `eprintln!` wording in Task 8 step 3 to whatever the classifier actually matches, or leave it as a plain note if empty results are not classified at all. Do not guess.

- [ ] **Step 3: Point CLAUDE.md at the new file**

In `CLAUDE.md`, in the **API Specs** section immediately after the paragraph about `polymarket-llms.txt`, add:

```markdown
**A mirror can match upstream and still be wrong.** `docs/specs/gamma/OBSERVED.md`
records places where gamma's published spec disagrees with gamma's own server —
`parent_entity_type` accepts `PerpsAsset` and rejects the documented `market`,
`limit` counts top-level comments rather than rows, and `GET /comments/{id}`
returns a whole thread. The drift check cannot see any of this: it compares the
mirror to the published document, never to the live host. The mirror itself must
stay byte-faithful or `nightly-schema.yml` alarms forever, so the observations
live beside it rather than inside it.
```

- [ ] **Step 4: Bump the version**

In the workspace `Cargo.toml`, change `version = "0.27.0"` to `version = "0.28.0"`.

- [ ] **Step 5: Write the changelog entry**

Add to the top of `CHANGELOG.md`, matching the existing entry format:

```markdown
## 0.28.0

### Breaking

- **`polyoxide-gamma`: the comment types have been rewritten against the wire.**
  `Comment`, `CommentReaction` and `CommentPosition` described a payload
  Polymarket has never sent; `CommentUser` had no upstream counterpart at all
  and is removed. Removed fields: `Comment::user`, `market_id`, `event_id`,
  `series_id`, `parent_id`, `positions`, `like_count`, `dislike_count`,
  `reply_count`; `CommentReaction::user_id`; `CommentPosition::outcome` and
  `shares`. Added: `Comment::parent_entity_type`, `parent_entity_id`,
  `parent_comment_id`, `user_address`, `reply_address`, `profile`,
  `report_count`, `reaction_count`; the new `CommentProfile`; and
  `CommentPosition::position_size`. Any code touching these types was already
  failing at runtime with a deserialization error.
- **`Comments::get` returns `Vec<Comment>`.** `GET /comments/{id}` returns the
  whole thread, not one comment.
- **`ListComments::parent_entity_type` takes `ParentEntityType`** rather than a
  string, and the CLI's `--parent-entity-type market` is replaced by
  `perps-asset`. The server rejects `market` in either casing.
- **`polyoxide-py`: `CommentUser` is removed and `CommentProfile` added**, with
  the comment getters renamed to match.

### Fixed

- `polyoxide-gamma` comment endpoints no longer fail with
  `missing field 'userId'` on any response containing a reaction (#28).

### Added

- `polyoxide-gamma/tests/wire_agreement.rs` asserts both directions of
  agreement between the comment types and captured live payloads.
- `docs/specs/gamma/OBSERVED.md` records where gamma's published spec
  disagrees with gamma's server.
```

- [ ] **Step 6: Reconcile the spec with what was built**

Apply the five deviations from this plan's header to
`docs/superpowers/specs/2026-08-19-gamma-comments-and-mirror-syncs-design.md`,
so the two documents do not contradict each other. Specifically: the
`ParentEntityType` preview loses `#[non_exhaustive]` and gains
`#[serde(other)] Unknown`; Component 3's claim that "the clap value-enum
derives from the library type" becomes a local enum with a `From` impl; and
Component 3 gains the thread-not-comment finding.

The spec's Testing table also names tests this plan did not build under those
names — the recursive checker in Task 3 covers `reaction_agrees_with_wire`
inside the full-fixture walk rather than as a separate test, and several names
changed. Replace that table with the tests as actually written:
`full_comment_agrees_with_captured_payload`,
`sparse_comment_agrees_with_captured_payload`,
`id_suffixed_keys_keep_their_wire_casing`,
`get_comment_by_id_returns_the_whole_thread`,
`list_comments_sends_typed_parent_entity_type`,
`market_is_no_longer_accepted`, `parent_entity_type_perps_asset`,
`test_every_comment_getter_resolves`.

- [ ] **Step 7: Commit**

```bash
git add docs/specs/gamma/OBSERVED.md CLAUDE.md CHANGELOG.md Cargo.toml docs/superpowers/specs/
git commit -m "docs: record gamma's spec-vs-server gaps and release 0.28.0"
```

---

## Task 12: Full verification

- [ ] **Step 1: Run every CI gate**

```bash
cd "$(git rev-parse --show-toplevel)"
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace
```

All four must pass. The `cargo doc` gate is not optional: a red doc build makes
`release.yml` withhold the release tag silently, because it triggers on
`workflow_run: [CI], conclusion == 'success'`. The new doc comments in Tasks 5
and 6 reference `Profile` and `ParentEntityType` — both are `pub`, so the
intra-doc links resolve, but confirm rather than assume.

If a build is killed with signal 15 or exit 254, that is `earlyoom` on this
machine, not a real failure. Re-run with `cargo build -j 4`.

- [ ] **Step 2: Run the Python suite**

```bash
cd polyoxide-py && uv run pytest tests/ && cd ..
```

- [ ] **Step 3: Run the live gamma tests**

```bash
cargo test -p polyoxide-gamma --all-features --test live_api -- --ignored
```

Expected: no failures. Specifically confirm the four comment tests ran rather
than early-returning — if `live_list_comments` printed its "no comments" notice,
the run proved nothing about deserialization. Re-run against a busier event by
temporarily raising the `limit` in the discovery call if so.

- [ ] **Step 4: Confirm the guard actually guards**

Prove the test fails without the fix, rather than trusting that it would:

`git stash` will NOT work here — the fix is committed, so a stash of a clean
file is a no-op that silently "passes". Restore the pre-fix file by SHA
instead:

```bash
git checkout 17ab027 -- polyoxide-gamma/src/types.rs   # commit before Task 5
cargo test -p polyoxide-gamma --all-features --test wire_agreement 2>&1 | tail -20
git checkout HEAD -- polyoxide-gamma/src/types.rs      # restore the fix
cargo test -p polyoxide-gamma --all-features --test wire_agreement 2>&1 | tail -3
git status --short                                     # must be clean
```

Expected: 3 failed with the old types, 3 passed with the new ones. A guard that
passes either way is not a guard — this is the check that distinguishes this
work from the shape-only test it replaces.

**Already executed during Task 5** and confirmed: 3 failed / 3 passed. Re-run
only if `types.rs` or the fixtures changed after that point.

- [ ] **Step 5: Open the PR**

```bash
git push -u origin aidanb/recent-issues
gh pr create --title "fix(gamma)!: model comments against the wire" --body "$(cat <<'BODY'
## Summary

`polyoxide-gamma`'s comment types described a payload Polymarket has never
sent. `docs/specs/gamma/openapi.yaml` already had the shape right — the types
matched neither it nor the wire, since the polyte fork. Nightly #28 surfaced it
on 2026-08-18 only because a comment with a reaction appeared under a
discovered event; an empty `reactions` array had been hiding it for months.

Also fixes two defects found while scoping: `GET /comments/{id}` returns a
whole thread rather than one comment, and the CLI's `--parent-entity-type
market` was a guaranteed 422 in either casing while the server's `PerpsAsset`
was unreachable.

## Guard

`polyoxide-gamma/tests/wire_agreement.rs` asserts both directions against
captured live payloads — nothing emitted that the server does not send, nothing
sent that is not modelled or explicitly ignored with a reason. Verified to fail
against the old types, not merely to pass against the new ones.

The oracle is the captured wire, deliberately not the vendored spec: the spec
is wrong about this API, which is what `docs/specs/gamma/OBSERVED.md` records.

Fixes #28

🤖 Generated with [Claude Code](https://claude.com/claude-code)

https://claude.ai/code/session_01Rddha6vQrwbME1sGuwrcZX
BODY
)"
```

- [ ] **Step 6: Confirm the release**

After CI goes green and the PR merges, confirm `release.yml` ran and published
0.28.0. If CI passed but no release appeared, check for a wedged run — see the
`Wedged CI run` note; the fix is an empty commit, not a force-cancel.
