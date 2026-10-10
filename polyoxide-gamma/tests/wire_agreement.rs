//! Agreement between gamma's comment, profile, user and sports types and
//! payloads captured from the live Gamma host. Provenance is in
//! `tests/fixtures/README.md`.
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

use polyoxide_gamma::api::search::{SearchProfile, SearchResponse};
use polyoxide_gamma::api::user::UserResponse;
use polyoxide_gamma::types::{
    Comment, Event, HomeAway, ParentEntityType, Profile, ProtocolVersion, SportMetadata, Team,
};
use polyoxide_test_support::agreement::dotted;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const FULL: &str = include_str!("fixtures/comment_full.json");
const SPARSE: &str = include_str!("fixtures/comment_sparse.json");
const PROFILE_FULL: &str = include_str!("fixtures/profile_full.json");
const PROFILE_SPARSE: &str = include_str!("fixtures/profile_sparse.json");
const USER_RESPONSE_FULL: &str = include_str!("fixtures/user_response_full.json");
const USER_RESPONSE_SPARSE: &str = include_str!("fixtures/user_response_sparse.json");
const SEARCH_PROFILE_FULL: &str = include_str!("fixtures/search_profile_full.json");
const SEARCH_PROFILE_SPARSE: &str = include_str!("fixtures/search_profile_sparse.json");
const SEARCH_RESPONSE_PROFILES: &str = include_str!("fixtures/search_response_profiles.json");
const EVENT_GAME_FULL: &str = include_str!("fixtures/event_game_full.json");
const EVENT_GAME_CHILD: &str = include_str!("fixtures/event_game_child.json");
const EVENT_CRICKET: &str = include_str!("fixtures/event_cricket.json");
const EVENT_GAME_ID_SENTINEL: &str = include_str!("fixtures/event_game_id_sentinel.json");
const TEAM_FULL: &str = include_str!("fixtures/team_full.json");
const TEAM_SPARSE: &str = include_str!("fixtures/team_sparse.json");
const SPORT_METADATA: &str = include_str!("fixtures/sport_metadata.json");

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
    (
        "team.$schema",
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
    (
        "searchProfile.profileImage",
        "the sparse capture's subject has not set one; present in 41/228 sampled \
         profiles (see tests/fixtures/README.md)",
    ),
    (
        "searchProfile.bio",
        "the sparse capture's subject has not set one; present in 34/228 sampled \
         profiles (see tests/fixtures/README.md)",
    ),
    (
        "searchProfile.pseudonym",
        "the sparse capture's subject has not set one; present in 223/228 sampled \
         profiles (see tests/fixtures/README.md)",
    ),
    (
        "team.ordering",
        "sent only on a team embedded in an event, where it names the team's \
         side; /teams/{id} omits it",
    ),
    (
        "team.alias",
        "the sparse capture's team has none; on 6/50 sampled /teams rows (see \
         tests/fixtures/README.md)",
    ),
    (
        "team.color",
        "the sparse capture's team has none; on 42/50 sampled /teams rows",
    ),
    (
        "team.providerId",
        "the sparse capture's team has none; on 44/50 sampled /teams rows",
    ),
    (
        "team.updatedAt",
        "the sparse capture's team has none; on 43/50 sampled /teams rows",
    ),
    (
        "event.teams[0].alias",
        "the child and cricket captures' teams have none; on 488 of 888 \
         event-embedded teams sampled (see tests/fixtures/README.md)",
    ),
    (
        "event.teams[1].alias",
        "the child and cricket captures' teams have none; on 488 of 888 \
         event-embedded teams sampled (see tests/fixtures/README.md)",
    ),
];

/// Walk a captured payload against what the type re-emits, asserting both
/// directions at every level of nesting.
#[track_caller]
fn check(wire: &Value, emitted: &Value, path: &str) {
    dotted::check(wire, emitted, path, IGNORED, EXPECTED_ABSENT)
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

#[test]
fn full_search_profile_agrees_with_captured_payload() {
    let wire: Value = serde_json::from_str(SEARCH_PROFILE_FULL).expect("fixture is valid JSON");
    let typed: SearchProfile = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into SearchProfile: {e}"));
    let emitted = serde_json::to_value(&typed).expect("SearchProfile must serialize");
    check(&wire, &emitted, "searchProfile");
}

#[test]
fn sparse_search_profile_agrees_with_captured_payload() {
    let wire: Value = serde_json::from_str(SEARCH_PROFILE_SPARSE).expect("fixture is valid JSON");
    let typed: SearchProfile = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into SearchProfile: {e}"));
    let emitted = serde_json::to_value(&typed).expect("SearchProfile must serialize");
    check(&wire, &emitted, "searchProfile");
}

#[test]
fn search_response_tolerates_null_profile_entries() {
    // `search_response_profiles.json` is a captured `/public-search` response
    // whose `profiles` array has a server-sent JSON `null` at index 12 — see
    // tests/fixtures/README.md. A `Vec<SearchProfile>` cannot deserialize a
    // `null` element and errors the whole call; this pins that the type
    // tolerates it instead of failing outright.
    let wire: Value =
        serde_json::from_str(SEARCH_RESPONSE_PROFILES).expect("fixture is valid JSON");
    let typed: SearchResponse = serde_json::from_value(wire).unwrap_or_else(|e| {
        panic!(
            "captured payload with a null profile entry must deserialize into SearchResponse: {e}"
        )
    });
    assert_eq!(
        typed.profiles.len(),
        20,
        "the null entry must not silently shrink the array"
    );
    assert!(
        typed.profiles[12].is_none(),
        "index 12 is the server's null entry and must decode to None"
    );
    assert!(
        typed.profiles[0].is_some(),
        "a populated entry must still decode to Some"
    );
}

/// Both directions, for a type re-emitted from a captured payload.
fn agrees<T: DeserializeOwned + Serialize>(fixture: &str, path: &str) {
    let wire: Value = serde_json::from_str(fixture).expect("fixture is valid JSON");
    let typed: T = serde_json::from_value(wire.clone()).unwrap_or_else(|e| {
        panic!(
            "captured payload must deserialize into {}: {e}",
            std::any::type_name::<T>()
        )
    });
    let emitted = serde_json::to_value(&typed).expect("type must serialize");
    check(&wire, &emitted, path);
}

/// The one event in a captured `/events?id=…&include_markets=false` response,
/// as the wire sent it and as `Event` decoded it.
fn captured_event(fixture: &str) -> (Value, Event) {
    let wire: Value = serde_json::from_str(fixture).expect("fixture is valid JSON");
    let [wire] = <[Value; 1]>::try_from(wire.as_array().expect("an /events response").clone())
        .expect("the capture holds exactly one event");
    let event = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("captured payload must deserialize into Event: {e}"));
    (wire, event)
}

const SPORTS_EVENTS: [(&str, &str); 3] = [
    ("event_game_full", EVENT_GAME_FULL),
    ("event_game_child", EVENT_GAME_CHILD),
    ("event_cricket", EVENT_CRICKET),
];

/// Direction 2 at an event's top level: every key the server sent is modelled.
///
/// Direction 1 is not applied at this level. `Event` declares over a hundred
/// fields and a game event carries about half of them, so a field absent from
/// a capture is the normal case here, not a sign of invention.
/// `series` and `tags` are not walked; the sports objects nested in an event
/// get both directions in `event_teams_and_sport_agree_with_captured_payload`.
#[test]
fn sports_events_carry_no_unmodelled_top_level_keys() {
    for (name, fixture) in SPORTS_EVENTS {
        let (wire, event) = captured_event(fixture);
        let emitted = serde_json::to_value(&event).expect("Event must serialize");
        let unmodelled = dotted::unmodelled_top_level(&wire, &emitted);
        assert!(
            unmodelled.is_empty(),
            "{name}: event.{} is sent by the server but not modelled by Event",
            unmodelled.join(", event.")
        );
    }
}

#[test]
fn event_teams_and_sport_agree_with_captured_payload() {
    for (name, fixture) in SPORTS_EVENTS {
        let (wire, event) = captured_event(fixture);
        let emitted = serde_json::to_value(&event).expect("Event must serialize");
        // Selected because they carry both: without this, a capture missing
        // either key would compare `null` with `null` and prove nothing.
        assert!(wire["teams"].is_array(), "{name} must carry teams");
        assert!(wire["sport"].is_object(), "{name} must carry a sport");
        check(&wire["teams"], &emitted["teams"], "event.teams");
        check(&wire["sport"], &emitted["sport"], "event.sport");
    }
}

/// The key checks above cannot see a value falling through to
/// `HomeAway::Other`, or an id decoded into the wrong field, so pin values.
#[test]
fn game_event_decodes_its_game_id_teams_and_league() {
    let (_, event) = captured_event(EVENT_GAME_FULL);

    assert_eq!(event.game_id, Some(10079774));
    assert_eq!(event.parent_event_id, None);

    let sides: Vec<_> = event
        .teams
        .iter()
        .map(|t| (t.name.as_deref(), t.ordering.clone()))
        .collect();
    assert_eq!(
        sides,
        [
            (Some("Milwaukee Brewers"), Some(HomeAway::Away)),
            (Some("San Diego Padres"), Some(HomeAway::Home)),
        ]
    );
    assert_eq!(event.teams[0].provider_id, Some(32));
    assert_eq!(event.teams[0].color.as_deref(), Some("#224C8F"));

    let sport = event
        .sport
        .as_ref()
        .expect("a game event carries its league");
    assert_eq!(sport.sport, "mlb");
    assert_eq!(sport.name.as_deref(), Some("MLB"));
    assert_eq!(sport.primary_tag_id, Some(100381));

    let metadata = event.event_metadata.as_ref().expect("eventMetadata");
    assert_eq!(
        metadata.get("opticOddsGameId"),
        Some(&json!("37337-25683-2026-10-07-19"))
    );
}

#[test]
fn game_event_decodes_its_volume_comment_count_and_version() {
    let (_, event) = captured_event(EVENT_GAME_FULL);
    assert_eq!(event.volume, Some(1144235.9190239997));
    assert_eq!(event.comment_count, Some(0));
    assert_eq!(event.neg_risk_augmented, Some(false));
    assert_eq!(event.version, Some(ProtocolVersion::V1));
}

/// A game's child events ("More Markets", "Exact Score", …) carry the same
/// `gameId` as the game, so `game_id` alone does not identify one event.
#[test]
fn a_child_event_shares_its_parents_game_id() {
    let (_, child) = captured_event(EVENT_GAME_CHILD);
    assert_eq!(child.game_id, Some(90115236));
    assert_eq!(child.parent_event_id, Some(1015530));
}

/// Cricket events have teams and a league but no `gameId`. The id they do
/// carry is a string inside `eventMetadata`.
#[test]
fn a_cricket_event_carries_its_id_only_in_event_metadata() {
    let (_, cricket) = captured_event(EVENT_CRICKET);
    assert_eq!(cricket.game_id, None);
    assert_eq!(cricket.teams.len(), 2);
    assert_eq!(
        cricket
            .event_metadata
            .as_ref()
            .and_then(|m| m.get("gameId")),
        Some(&json!("1000170151LIVE2026"))
    );
}

/// Gamma sends `gameId: -1` on at least one event that is not a game. It
/// failed prader-sync's whole `/events/keyset` page on 0.38.1, and the sync
/// then retried that cursor for ever, so it reads as no game.
#[test]
fn a_negative_game_id_reads_as_no_game() {
    let (wire, event) = captured_event(EVENT_GAME_ID_SENTINEL);
    assert_eq!(
        wire["gameId"],
        json!(-1),
        "the capture must carry the sentinel"
    );
    assert_eq!(event.game_id, None);
    assert_eq!(event.parent_event_id, None);
    assert!(event.teams.is_empty());
}

#[test]
fn full_team_agrees_with_captured_payload() {
    agrees::<Team>(TEAM_FULL, "team");
}

#[test]
fn sparse_team_agrees_with_captured_payload() {
    agrees::<Team>(TEAM_SPARSE, "team");
}

#[test]
fn sport_metadata_agrees_with_captured_payload() {
    agrees::<SportMetadata>(SPORT_METADATA, "sport");
}
