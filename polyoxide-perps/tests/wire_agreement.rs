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
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a
//!    value that rounds or overflows on decode (a decimal past 28 places, an
//!    integer past `u64`) fails here rather than passing on its key alone.
//!
//! `NEVER_ON_WIRE` lists paths a fixture is known to lack, so the first
//! capture that carries one is checked by hand. An entry in any list that no
//! fixture needs fails the test too.

use std::collections::BTreeSet;

use polyoxide_perps::api::{exchange::*, health::*, market::*, public::*};
use polyoxide_test_support::{
    agreement::{self, Arrays, Ledger},
    fixtures,
};
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

/// `(fixture, path, reason)` a capture is known to lack, so the type behind
/// the path has never been checked against the wire.
const NEVER_ON_WIRE: &[(&str, &str, &str)] = &[(
    "index",
    "/constituents[]",
    "every index answered with [] on 2026-09-30; check IndexConstituent by hand on the first capture that carries one",
)];

fn key_paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    agreement::key_paths(value, prefix, out)
}

/// Asserts every scalar present on both sides is equal, recursing objects by
/// key and arrays by index. Paths on one side only are the key-set checks'
/// business, not this one's.
#[track_caller]
fn assert_values_agree(fixture: &str, path: &str, wire: &Value, emitted: &Value) {
    agreement::assert_values_agree(fixture, path, wire, emitted, Arrays::Zip)
}

fn check<T: DeserializeOwned + Serialize>(
    fixture: &str,
    used: &mut Ledger<(&'static str, &'static str, &'static str)>,
) {
    let text = fixtures!().text(fixture);
    let wire: Value = serde_json::from_str(&text).unwrap();
    let parsed: T = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{fixture}: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();

    assert_values_agree(fixture, "", &wire, &emitted);

    let mut sent = BTreeSet::new();
    key_paths(&wire, "", &mut sent);
    let mut modelled = BTreeSet::new();
    key_paths(&emitted, "", &mut modelled);

    for p in sent.difference(&modelled) {
        if used.excuse(IGNORED, fixture, p).is_none() {
            panic!("{fixture}: server sent {p}, which the type does not model");
        }
    }
    for p in modelled.difference(&sent) {
        if used.excuse(EXPECTED_ABSENT, fixture, p).is_none() {
            panic!("{fixture}: the type emits {p}, which the server did not send");
        }
    }
    used.never_on_wire(NEVER_ON_WIRE, fixture, &sent);
}

#[test]
fn every_fixture_agrees_with_its_type() {
    let mut used = Ledger::new(&[IGNORED, EXPECTED_ABSENT, NEVER_ON_WIRE]);
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

    let stale: Vec<_> = used.stale().into_iter().map(|(f, p, _)| (f, p)).collect();
    assert!(
        stale.is_empty(),
        "allowance entries no fixture needs: {stale:?}"
    );
}
