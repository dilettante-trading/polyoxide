//! Agreement between the v2 types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/v2/PROVENANCE.md`.
//!
//! `v2_spec_agreement.rs` checks the types against the served schema; this
//! checks the schema's claims against the wire. Each fixture is deserialized
//! into its type and serialized back, and the two key-path sets are compared,
//! recursing into nested objects and arrays:
//!
//! 1. **Nothing unmodelled.** Every path the server sent is emitted by the type,
//!    unless it is listed in [`IGNORED`] with a reason.
//! 2. **Nothing invented.** Every path the type emits was sent by the server,
//!    unless it is listed in [`EXPECTED_ABSENT`] with a reason. Every `Option`
//!    serializes as `null`, so a field modelled from the spec but absent from
//!    the wire shows up here instead of hiding.
//!
//! An entry in either list that no fixture needs fails the test too, so the
//! lists cannot go stale.

use std::collections::BTreeSet;

use polyoxide_data::v2::{types::*, Page};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

/// `{ "data": T }`. The crate's own envelope is private.
#[derive(Deserialize, Serialize)]
struct Data<T> {
    data: T,
}

/// `(fixture, path)` pairs the types deliberately do not model.
const IGNORED: &[(&str, &str, &str)] = &[];

/// `(fixture, path)` pairs a type emits that this capture did not contain.
const EXPECTED_ABSENT: &[(&str, &str, &str)] = &[
    (
        "resolutions",
        "/data[]/market_type",
        "documented for condition-keyed rows, but this one omits it (OBSERVED.md)",
    ),
    (
        "resolutions",
        "/data[]/price",
        "UMA lifecycle rows populate the price fields; this row has no UMA lifecycle",
    ),
    (
        "resolutions",
        "/data[]/proposed_price",
        "present on question-keyed rows only",
    ),
    (
        "resolutions",
        "/data[]/question_id",
        "absent on condition-keyed rows",
    ),
    (
        "resolutions",
        "/data[]/reporter",
        "documented without a condition, but omitted on this reported row (OBSERVED.md)",
    ),
    (
        "resolutions",
        "/data[]/reproposed_price",
        "present on question-keyed rows only, like proposed_price",
    ),
    (
        "activity",
        "/data[]/is_combo",
        "sent only on combo trade rows; this wallet's rows are not combos",
    ),
    (
        "activity_tips",
        "/data[]/is_combo",
        "sent only on combo trade rows; this wallet's rows are not combos",
    ),
    (
        "holders",
        "/data[]/holders[]/avg_price",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
    (
        "holders",
        "/data[]/holders[]/current_price",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
    (
        "holders",
        "/data[]/holders[]/current_value",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
    (
        "holders",
        "/data[]/holders[]/entry_cost_usdc",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
    (
        "holders",
        "/data[]/holders[]/realized_pnl",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
    (
        "holders",
        "/data[]/holders[]/total_pnl",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
    (
        "holders",
        "/data[]/holders[]/unrealized_pnl",
        "position economics are sent only with include_pnl=true (see holders_pnl)",
    ),
];

fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let path = format!("{prefix}/{key}");
                out.insert(path.clone());
                key_paths(child, &path, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                key_paths(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

fn load(fixture: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/v2/{fixture}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap()
}

/// Returns the unexcused differences for one fixture, and marks the excuses used.
fn diff<T: DeserializeOwned + Serialize>(
    fixture: &str,
    used: &mut BTreeSet<(String, String)>,
) -> Vec<String> {
    let wire = load(fixture);
    let parsed: T = serde_json::from_value(wire.clone())
        .unwrap_or_else(|e| panic!("{fixture}: does not deserialize: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();

    let mut sent = BTreeSet::new();
    key_paths(&wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(&emitted, "", &mut modelled);

    let mut problems = Vec::new();
    for path in sent.difference(&modelled) {
        if IGNORED.iter().any(|(f, p, _)| *f == fixture && p == path) {
            used.insert((fixture.to_owned(), path.clone()));
        } else {
            problems.push(format!(
                "{fixture}: server sent {path}, which no type models"
            ));
        }
    }
    for path in modelled.difference(&sent) {
        if EXPECTED_ABSENT
            .iter()
            .any(|(f, p, _)| *f == fixture && p == path)
        {
            used.insert((fixture.to_owned(), path.clone()));
        } else {
            problems.push(format!(
                "{fixture}: type emits {path}, which the server did not send"
            ));
        }
    }
    problems
}

#[test]
fn every_fixture_agrees_with_its_type_in_both_directions() {
    let mut used = BTreeSet::new();
    let mut problems = Vec::new();
    let mut check = |p: Vec<String>| problems.extend(p);

    check(diff::<Page<Trade>>("trades", &mut used));
    check(diff::<Page<Position>>("positions", &mut used));
    check(diff::<Page<Position>>("positions_closed", &mut used));
    check(diff::<Page<Activity>>("activity", &mut used));
    check(diff::<Page<Activity>>("activity_tips", &mut used));
    check(diff::<Data<Approvals>>("approvals", &mut used));
    check(diff::<Data<UserPnlSeries>>("user_pnl", &mut used));
    check(diff::<Data<Option<UserStats>>>("user_stats", &mut used));
    check(diff::<Data<Option<UserStats>>>(
        "user_stats_unknown",
        &mut used,
    ));
    check(diff::<Data<UserVolume>>("user_volume", &mut used));
    check(diff::<Data<PortfolioValue>>("value", &mut used));
    check(diff::<Page<MetaHolder>>("holders", &mut used));
    check(diff::<Page<MetaHolder>>("holders_pnl", &mut used));
    check(diff::<Data<LiveVolume>>("live_volume", &mut used));
    check(diff::<Data<Vec<OpenInterest>>>("open_interest", &mut used));
    check(diff::<Data<Vec<OpenInterest>>>(
        "open_interest_global",
        &mut used,
    ));
    check(diff::<Page<PricePoint>>("prices_history", &mut used));
    check(diff::<Data<Vec<Resolution>>>("resolutions", &mut used));
    check(diff::<Page<BiggestWinner>>("biggest_winners", &mut used));
    check(diff::<Page<BiggestWinner>>(
        "biggest_winners_combos",
        &mut used,
    ));
    check(diff::<Page<ComboPosition>>("combo_positions", &mut used));
    check(diff::<Page<ComboActivity>>("combo_activity", &mut used));
    check(diff::<Page<BuilderStanding>>(
        "builders_leaderboard",
        &mut used,
    ));
    check(diff::<Data<Vec<BuilderVolumePoint>>>(
        "builder_volume",
        &mut used,
    ));
    check(diff::<Page<LeaderboardEntry>>("leaderboard", &mut used));
    check(diff::<Data<Option<LeaderboardUserEntry>>>(
        "leaderboard_user",
        &mut used,
    ));
    check(diff::<Data<ServiceStatus>>("status", &mut used));

    for (fixture, path, _) in IGNORED.iter().chain(EXPECTED_ABSENT) {
        if !used.contains(&((*fixture).to_owned(), (*path).to_owned())) {
            problems.push(format!(
                "{fixture}: stale excuse for {path}; no difference needs it"
            ));
        }
    }

    assert!(problems.is_empty(), "\n{}\n", problems.join("\n"));
}
