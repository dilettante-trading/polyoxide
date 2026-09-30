//! Agreement between the types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/PROVENANCE.md`.
//!
//! Each fixture is deserialized into its type and serialized back, and the two
//! key-path sets are compared:
//!
//! 1. **Nothing unmodelled.** Every path the server sent is emitted by the type,
//!    unless listed in `IGNORED` with a reason.
//! 2. **Nothing invented.** Every path the type emits was sent by the server,
//!    unless listed in `EXPECTED_ABSENT` with a reason. Every `Option`
//!    serializes as `null`, so a field modelled from the spec but absent from
//!    the wire shows up here instead of hiding.
//!
//! An entry in either list that no fixture needs fails the test too.

use std::collections::BTreeSet;

use polyoxide_perps::api::{exchange::*, health::*, market::*, public::*};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

/// `(fixture, path, reason)` the types deliberately do not model.
const IGNORED: &[(&str, &str, &str)] = &[];

/// `(fixture, path, reason)` a type emits that this capture did not contain.
const EXPECTED_ABSENT: &[(&str, &str, &str)] = &[
    (
        "leaderboard",
        "/account",
        "sent only when the request names an address",
    ),
    (
        "position_fills",
        "/cursor",
        "sent only when another page exists",
    ),
];

fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let p = format!("{prefix}/{k}");
                out.insert(p.clone());
                key_paths(v, &p, out);
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

fn check<T: DeserializeOwned + Serialize>(
    fixture: &str,
    used: &mut BTreeSet<(&'static str, &'static str)>,
) {
    let path = format!(
        "{}/tests/fixtures/{fixture}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let wire: Value = serde_json::from_str(&text).unwrap();
    let parsed: T = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{fixture}: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();

    let mut sent = BTreeSet::new();
    key_paths(&wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(&emitted, "", &mut modelled);

    for p in sent.difference(&modelled) {
        match IGNORED.iter().find(|(f, ip, _)| *f == fixture && *ip == p) {
            Some((f, ip, _)) => {
                used.insert((f, ip));
            }
            None => panic!("{fixture}: server sent {p}, which the type does not model"),
        }
    }
    for p in modelled.difference(&sent) {
        match EXPECTED_ABSENT
            .iter()
            .find(|(f, ap, _)| *f == fixture && *ap == p)
        {
            Some((f, ap, _)) => {
                used.insert((f, ap));
            }
            None => panic!("{fixture}: the type emits {p}, which the server did not send"),
        }
    }
}

#[test]
fn every_fixture_agrees_with_its_type() {
    let mut used = BTreeSet::new();
    check::<Time>("time", &mut used);
    check::<Exchange>("exchange", &mut used);
    check::<Vec<Asset>>("assets", &mut used);
    check::<Vec<Instrument>>("instruments", &mut used);
    check::<FeesInfo>("fees", &mut used);
    check::<Vec<LimitTier>>("limit_tiers", &mut used);
    check::<Vec<Ticker>>("tickers", &mut used);
    check::<Vec<Statistic>>("statistics", &mut used);
    check::<ExchangeStatistics>("exchange_stats", &mut used);
    check::<Klines>("klines", &mut used);
    check::<MarkHistory>("mark_history", &mut used);
    check::<Vec<Bbo>>("bbo", &mut used);
    check::<Book>("book", &mut used);
    check::<Index>("index", &mut used);
    check::<Trades>("trades", &mut used);
    check::<FundingHistory>("funding", &mut used);
    check::<PublicPortfolio>("portfolio", &mut used);
    check::<PositionFills>("position_fills", &mut used);
    check::<Leaderboard>("leaderboard", &mut used);
    check::<Leaderboard>("leaderboard_account", &mut used);
    check::<InviteCheck>("invite", &mut used);

    let listed: BTreeSet<(&str, &str)> = IGNORED
        .iter()
        .chain(EXPECTED_ABSENT.iter())
        .map(|(f, p, _)| (*f, *p))
        .collect();
    let stale: Vec<_> = listed.difference(&used).collect();
    assert!(
        stale.is_empty(),
        "allowance entries no fixture needs: {stale:?}"
    );
}
