//! Agreement between the comment types and payloads captured from the live
//! Gamma host. Provenance is in `tests/fixtures/README.md`.
//!
//! Two directions, both of which must fail if a type drifts from the wire:
//!
//! 1. **No invented fields.** Every key the type emits corresponds to a key
//!    the server actually sent, unless it is declared in `EXPECTED_ABSENT`
//!    with a reason.
//! 2. **No unmodelled fields.** Every key the server sent is either modelled
//!    or listed in `IGNORED` with a written reason.
//!
//! The oracle is the captured payload, not `docs/specs/gamma/openapi.yaml`.
//! The published spec is known to be wrong about this API — see
//! `docs/specs/gamma/OBSERVED.md`.
//!
//! # Why direction 1 needs `EXPECTED_ABSENT` rather than a `null` exemption
//!
//! An earlier version of this guard exempted `null` values and empty arrays
//! from direction 1, on the theory that a legitimately-optional field the
//! server omitted also serializes to `null`. That reasoning is correct as far
//! as it goes, but the exemption cannot distinguish "optional and absent this
//! time" from "does not exist at all" — every field in the comment family is
//! `Option<T>` or `#[serde(default)] Vec<T>`, so a wholly invented field also
//! serializes to `null` or `[]` and passed unnoticed. Verified 2026-08-19:
//! adding `totally_invented_field: Option<String>` to `Comment` left all three
//! tests passing, and re-adding `positions: Vec<CommentPosition>` at the
//! `Comment` level — the exact field issue #28 removed — would also have
//! passed.
//!
//! `EXPECTED_ABSENT` closes that gap without reintroducing the blanket
//! exemption: every key the type emits must be either present on the wire or
//! named here with a reason. A field can be legitimately absent from a
//! capture — the server omitted it for this subject, or it is modelled from
//! the vendored spec and rarely sent — but "absent from every capture" and
//! "does not exist" look identical from here, so each one must be declared
//! rather than silently exempted. An invented field has no entry and no
//! excuse, so it fails.
//!
//! This guard caught issue #28 only because that type's invented fields
//! included *required* ones (`user`, `like_count`), which failed
//! deserialization before these assertions ever ran. `EXPECTED_ABSENT` is
//! what makes an invented `Option<T>` field fail too.

use polyoxide_gamma::api::user::UserResponse;
use polyoxide_gamma::types::{Comment, ParentEntityType, Profile};
use serde_json::Value;

const FULL: &str = include_str!("fixtures/comment_full.json");
const SPARSE: &str = include_str!("fixtures/comment_sparse.json");
const PROFILE_FULL: &str = include_str!("fixtures/profile_full.json");
const PROFILE_SPARSE: &str = include_str!("fixtures/profile_sparse.json");
const USER_RESPONSE_FULL: &str = include_str!("fixtures/user_response_full.json");
const USER_RESPONSE_SPARSE: &str = include_str!("fixtures/user_response_sparse.json");

/// Wire keys deliberately left unmodelled, each with a reason.
///
/// Adding an entry is a written decision that shows up in a reviewed diff.
/// There is no wildcard. Paths are dotted from the root, e.g.
/// `comment.profile.someKey`.
const IGNORED: &[(&str, &str)] = &[
    (
        "profile.$schema",
        "response metadata (a link to the published JSON Schema for this \
         endpoint), not data — see docs/specs/gamma/OBSERVED.md",
    ),
    (
        "user.$schema",
        "response metadata (a link to the published JSON Schema for this \
         endpoint), not data — see docs/specs/gamma/OBSERVED.md",
    ),
];

/// Keys the type emits that the captured payloads do not contain.
///
/// A field can be legitimately absent from a capture — the server omits it
/// for this subject, or it is modelled from the vendored spec and rarely
/// sent. But "absent from every capture" and "does not exist" look identical
/// from here, so each one must be declared with a reason rather than
/// silently exempted. This is what stops an invented `Option<T>` field from
/// passing unnoticed. There is no wildcard; paths follow the same dotted
/// convention as `IGNORED`.
const EXPECTED_ABSENT: &[(&str, &str)] = &[
    (
        "comment.profile.isMod",
        "spec-sourced field; not observed in the 166-comment live sample",
    ),
    (
        "comment.profile.isCreator",
        "spec-sourced field; not observed in the 166-comment live sample",
    ),
    (
        "comment.profile.profileImageOptimized",
        "spec-sourced field; not observed in the 166-comment live sample",
    ),
    (
        "comment.profile.pseudonym",
        "this capture's author has none set; 164 of 166 sampled comments carry it \
         (see tests/fixtures/README.md)",
    ),
    (
        "comment.reactions[0].createdAt",
        "spec-sourced field; not observed in the 166-comment live sample",
    ),
    (
        "comment.reactions[0].icon",
        "spec-sourced field; not observed in the 166-comment live sample",
    ),
    (
        "comment.reactions[0].profile.name",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.pseudonym",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.displayUsernamePublic",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.bio",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.isMod",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.isCreator",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.baseAddress",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.profileImage",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.profileImageOptimized",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.reactions[0].profile.positions",
        "the reactor's embedded profile in this capture carries only proxyWallet",
    ),
    (
        "comment.parentCommentID",
        "the sparse capture is a thread root, not a reply",
    ),
    (
        "comment.replyAddress",
        "the sparse capture is a thread root, not a reply",
    ),
    (
        "comment.profile",
        "the sparse capture's author has no profile in this response",
    ),
    ("comment.reactions", "the sparse capture has no reactions"),
    (
        "profile.profileImage",
        "the sparse capture's subject has not set one; absent in 34/65 sampled \
         profiles (see tests/fixtures/README.md)",
    ),
    (
        "profile.bio",
        "the sparse capture's subject has not set one; absent in 49/65 sampled \
         profiles (see tests/fixtures/README.md)",
    ),
    (
        "user.discordUsername",
        "not observed in a 39-address sample of /public-profile — see \
         tests/fixtures/README.md",
    ),
    (
        "user.profileImage",
        "the sparse capture's subject has not set one; present in 12/39 sampled \
         /public-profile responses (see tests/fixtures/README.md)",
    ),
    (
        "user.bio",
        "the sparse capture's subject has not set one; present in 5/39 sampled \
         /public-profile responses (see tests/fixtures/README.md)",
    ),
    (
        "user.xUsername",
        "the sparse capture's subject has not set one; present in 5/39 sampled \
         /public-profile responses (see tests/fixtures/README.md)",
    ),
];

/// Walk a captured payload against what the type re-emits, asserting both
/// directions at every level of nesting.
fn check(wire: &Value, emitted: &Value, path: &str) {
    match (wire, emitted) {
        (Value::Object(w), Value::Object(e)) => {
            // Direction 1: nothing invented. A key present on both sides
            // recurses so nested mismatches are caught too; a key the type
            // emits but the wire lacks must be declared in EXPECTED_ABSENT.
            for (key, value) in e {
                let full = format!("{path}.{key}");
                match w.get(key) {
                    Some(wire_value) => check(wire_value, value, &full),
                    None => assert!(
                        EXPECTED_ABSENT.iter().any(|(k, _)| *k == full.as_str()),
                        "{full} is emitted by the type but absent from the captured \
                         payload, and not listed in EXPECTED_ABSENT with a reason — \
                         the field may be invented"
                    ),
                }
            }
            // Direction 2: nothing unmodelled. Keys present on both sides
            // were already recursed into above.
            for key in w.keys() {
                if e.contains_key(key) {
                    continue;
                }
                let full = format!("{path}.{key}");
                assert!(
                    IGNORED.iter().any(|(k, _)| *k == full.as_str()),
                    "{full} is sent by the server but not modelled, and not listed \
                     in IGNORED with a reason"
                );
            }
        }
        (Value::Array(w), Value::Array(e)) => {
            assert_eq!(
                w.len(),
                e.len(),
                "{path} has {} elements on the wire but the type re-emits {} — a \
                 truncated collection would otherwise pass unnoticed by comparing \
                 only the shorter length",
                w.len(),
                e.len()
            );
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
fn full_profile_agrees_with_captured_payload() {
    let wire: Value = serde_json::from_str(PROFILE_FULL).expect("fixture is valid JSON");
    let typed: Profile = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into Profile: {e}"));
    let emitted = serde_json::to_value(&typed).expect("Profile must serialize");
    check(&wire, &emitted, "profile");
}

#[test]
fn sparse_profile_agrees_with_captured_payload() {
    let wire: Value = serde_json::from_str(PROFILE_SPARSE).expect("fixture is valid JSON");
    let typed: Profile = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into Profile: {e}"));
    let emitted = serde_json::to_value(&typed).expect("Profile must serialize");
    check(&wire, &emitted, "profile");
}

#[test]
fn full_user_response_agrees_with_captured_payload() {
    let wire: Value = serde_json::from_str(USER_RESPONSE_FULL).expect("fixture is valid JSON");
    let typed: UserResponse = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into UserResponse: {e}"));
    let emitted = serde_json::to_value(&typed).expect("UserResponse must serialize");
    check(&wire, &emitted, "user");
}

#[test]
fn sparse_user_response_agrees_with_captured_payload() {
    let wire: Value = serde_json::from_str(USER_RESPONSE_SPARSE).expect("fixture is valid JSON");
    let typed: UserResponse = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into UserResponse: {e}"));
    let emitted = serde_json::to_value(&typed).expect("UserResponse must serialize");
    check(&wire, &emitted, "user");
}

#[test]
fn user_response_tolerates_null_users() {
    // The published schema (`PublicProfileResponse.json`) types `users` as
    // `["array","null"]` — an explicit JSON `null` is legal, distinct from the
    // key being absent (already covered by `#[serde(default)]`). Not observed
    // in the wild across a 39-address sample, but the schema allows it, so it
    // must not error.
    let json = r#"{"takerTier": 0, "takerTierName": "Tier 0", "weightedVolume": 0, "users": null}"#;
    let user: UserResponse = serde_json::from_str(json)
        .expect("an explicit null for `users` must deserialize, not error");
    assert!(user.users.is_empty());
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

    // A key-set guard alone cannot catch a value falling through to
    // `ParentEntityType::Unknown`: if `"Event"` ever stopped deserializing to
    // `ParentEntityType::Event` (a renamed variant, upstream switching to
    // lowercase `event`, ...), `#[serde(other)]` would silently absorb it into
    // `Unknown`, which re-serializes to the string `"Unknown"` — the key sets
    // would still match and every test above would still pass, while
    // `polyoxide gamma comments list` printed `"parentEntityType": "Unknown"`
    // on every row. Pin the decoded value, not just its presence.
    assert_eq!(typed.parent_entity_type, Some(ParentEntityType::Event));
}
