//! Agreement between the v2 types and builders and the served OpenAPI schema.
//!
//! The oracle is `docs/specs/data-v2/openapi.json`, which upstream serves from
//! the API host itself (`/v2/openapi.json`) and the nightly drift check keeps
//! byte-identical. Four things are checked, and each was shown to fail on a
//! deliberate mutation before being trusted:
//!
//! 1. **Optionality.** A property that is required and not nullable must be a
//!    plain field (removing it from the JSON must fail), and every other
//!    property must accept `null`. Mutations caught: a required field made
//!    `Option`; an optional field made non-`Option`.
//! 2. **Field names.** A fully populated value must serialize back to exactly
//!    the spec's property set. Serde ignores unknown keys on the way in, so
//!    without this a struct that forgot or misspelled a field would pass (1).
//!    Mutations caught: a field removed; a field misspelled.
//! 3. **Query parameters, and decoding.** Each route, built with every argument
//!    and setter, must send exactly the spec's parameter names, and its
//!    `send()` must decode that route's captured response from
//!    `tests/fixtures/v2/`, which covers every builder's envelope and return type
//!    offline. Mutations caught: a camelCase key; an extra `offset` setter; a
//!    route handed another route's response.
//! 4. **Envelopes.** Each `{data}` envelope a route answers with must wrap a
//!    schema from (1) and (2), which upstream mostly inlines as a copy rather
//!    than a `$ref`. Mutation caught: a property added to one envelope's copy.
//!
//! An `allOf` schema is checked as the flat object the server sends. Every
//! non-envelope schema must be in the table or excused with a reason.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};

use polyoxide_data::{
    types::SortDirection,
    v2::{types::*, Page, Pagination},
    DataApi, DataApiError,
};
use polyoxide_test_support::{fixtures, openapi, query};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/data-v2/openapi.json");

fn spec() -> Value {
    serde_json::from_str(SPEC).expect("spec parses")
}

fn schemas() -> Map<String, Value> {
    openapi::schemas(&spec())
}

/// Holds one type to one schema. Data v2 declares no wire-only fields.
#[track_caller]
fn check<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    openapi::check::<T>(schemas, name, &[]);
}

macro_rules! agreement {
    ($($schema:literal => $ty:ty),+ $(,)?) => {
        const MODELLED: &[&str] = &[$($schema),+];
        #[test]
        fn every_modelled_schema_agrees_with_the_spec() {
            let schemas = schemas();
            $( check::<$ty>(&schemas, $schema); )+
        }
    };
}

agreement! {
    "Activity" => Activity,
    "ApprovalContract" => ApprovalContract,
    "Approvals" => Approvals,
    "BiggestWinner" => BiggestWinner,
    "BuilderStanding" => BuilderStanding,
    "BuilderVolumePoint" => BuilderVolumePoint,
    "ComboActivity" => ComboActivity,
    "ComboLeg" => ComboLeg,
    "ComboLegEvent" => ComboLegEvent,
    "ComboLegMarket" => ComboLegMarket,
    "ComboPosition" => ComboPosition,
    "ConditionVolume" => ConditionVolume,
    "CursorLag" => CursorLag,
    "Holder" => Holder,
    "IngestionFreshness" => IngestionFreshness,
    "LeaderboardEntry" => LeaderboardEntry,
    "LeaderboardUserEntry" => LeaderboardUserEntry,
    "LiveVolume" => LiveVolume,
    "MetaHolder" => MetaHolder,
    "OpenInterest" => OpenInterest,
    "Pagination" => Pagination,
    "PortfolioValue" => PortfolioValue,
    "Position" => Position,
    "PricePoint" => PricePoint,
    "Resolution" => Resolution,
    "ServiceStatus" => ServiceStatus,
    "ServingFreshness" => ServingFreshness,
    "ServingMechanism" => ServingMechanism,
    "Trade" => Trade,
    "UserPnlPoint" => UserPnlPoint,
    "UserPnlSeries" => UserPnlSeries,
    "UserStats" => UserStats,
    "UserVolume" => UserVolume,
    "ActivityPage" => Page<Activity>,
    "BiggestWinnersPage" => Page<BiggestWinner>,
    "BuildersLeaderboardPage" => Page<BuilderStanding>,
    "ComboActivityPage" => Page<ComboActivity>,
    "ComboPositionsPage" => Page<ComboPosition>,
    "HoldersPage" => Page<MetaHolder>,
    "LeaderboardPage" => Page<LeaderboardEntry>,
    "PositionsPage" => Page<Position>,
    "PricesHistoryPage" => Page<PricePoint>,
    "TradesPage" => Page<Trade>,
}

/// Schemas deliberately not in the table, each with its reason.
const NOT_MODELLED: &[(&str, &str)] = &[
    (
        "ErrorCode",
        "string enum; mapped in v2::error and covered by the mock error tests",
    ),
    (
        "ErrorResponse",
        "parsed inside DataApiError::from_response; covered by the mock error tests",
    ),
    (
        "LeaderboardResponse",
        "oneOf split into leaderboard() and leaderboard_user()",
    ),
];

#[test]
fn every_spec_schema_is_modelled_or_excused() {
    let schemas = schemas();
    let excused: BTreeSet<&str> = NOT_MODELLED.iter().map(|(n, _)| *n).collect();
    let modelled: BTreeSet<&str> = MODELLED.iter().copied().collect();
    let unaccounted: Vec<&str> = schemas
        .keys()
        .map(String::as_str)
        // Private `{data}` envelopes: `send()` unwraps them, and each one's
        // `data` is a named schema checked above (see the next test).
        .filter(|n| !n.starts_with("Envelope_"))
        .filter(|n| !modelled.contains(n) && !excused.contains(n))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "schemas neither modelled nor excused: {unaccounted:?}"
    );
}

/// The converse: every table entry names a schema the spec still has, and
/// none is both modelled and excused. When upstream deletes a modelled schema
/// the other tests fail only with serde_json's `no entry found for key`, and a
/// deleted excused schema fails nothing at all.
#[test]
fn every_table_entry_names_a_current_schema() {
    let schemas = schemas();
    let excused: BTreeSet<&str> = NOT_MODELLED.iter().map(|(n, _)| *n).collect();
    let gone: Vec<&str> = MODELLED
        .iter()
        .chain(&excused)
        .copied()
        .filter(|n| !schemas.contains_key(*n))
        .collect();
    assert!(
        gone.is_empty(),
        "table entries the spec no longer has: {gone:?}"
    );
    let both: Vec<&str> = MODELLED
        .iter()
        .copied()
        .filter(|n| excused.contains(n))
        .collect();
    assert!(
        both.is_empty(),
        "schemas both modelled and excused: {both:?}"
    );
}

/// The routes answer with the envelopes, not the named schemas, and upstream
/// writes most envelopes' `data` as an inline copy rather than a `$ref`. Checking
/// the named schema only checks the route while the two stay identical.
#[test]
fn every_envelope_wraps_a_modelled_schema() {
    let schemas = schemas();
    for (name, envelope) in schemas.iter().filter(|(n, _)| n.starts_with("Envelope_")) {
        let data = &envelope["properties"]["data"];
        let row = if data["type"] == "array" {
            &data["items"]
        } else if let Some(arms) = data["oneOf"].as_array() {
            arms.iter()
                .find(|a| a["type"] != "null")
                .expect("non-null arm")
        } else {
            data
        };
        let wrapped = match row["$ref"].as_str() {
            Some(r) => MODELLED.iter().find(|m| r.rsplit('/').next() == Some(**m)),
            None => MODELLED.iter().find(|m| schemas[**m] == *row),
        };
        assert!(
            wrapped.is_some(),
            "{name}: data is not identical to any modelled schema"
        );
    }
}

// ── Query parameters ────────────────────────────────────────────────

type Fire = fn(DataApi) -> Pin<Box<dyn Future<Output = Result<(), DataApiError>> + Send>>;

/// Sends one request through `fire`, requires its response to decode, and
/// returns the query keys it carried.
async fn query_keys_sent(path: &str, fixture: &str, fire: Fire) -> BTreeSet<String> {
    let body = fixtures!("v2").text(fixture);
    query::keys_sent(path, fixture, body, |url| {
        fire(DataApi::builder().base_url(url).build().unwrap())
    })
    .await
}

/// One entry per builder: its path, the captured response it must decode (from
/// `tests/fixtures/v2/`), and a call using every argument and setter it has.
/// Two builders may share a path (`leaderboard` and `leaderboard_user`); their
/// keys are unioned before comparison.
const ROUTES: &[(&str, &str, Fire)] = &[
    ("/v2/user-stats", "user_stats", |data| {
        Box::pin(async move { data.v2().user_stats("0xuser").send().await.map(|_| ()) })
    }),
    ("/v2/trades", "trades", |data| {
        Box::pin(async move {
            data.v2()
                .trades()
                .user("0xuser")
                .conditions(["0xcond"])
                .event_ids([1])
                .side(TradeSide::Buy)
                .taker_only(false)
                .filter_type(FilterType::Cash)
                .filter_amount(1.0)
                .start(1)
                .end(2)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/activity", "activity", |data| {
        Box::pin(async move {
            data.v2()
                .activity("0xuser")
                .types([ActivityType::Trade, ActivityType::Tip])
                .conditions(["0xcond"])
                .event_ids([1])
                .side(TradeSide::Sell)
                .start(1)
                .end(2)
                .sort_by(ActivitySortBy::Timestamp)
                .sort_direction(SortDirection::Asc)
                .exclude_deposits_withdrawals(false)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/activity/combos", "combo_activity", |data| {
        Box::pin(async move {
            data.v2()
                .combo_activity("0xuser")
                .conditions(["0xcond"])
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/approvals", "approvals", |data| {
        Box::pin(async move { data.v2().approvals("0xuser").send().await.map(|_| ()) })
    }),
    ("/v2/positions", "positions", |data| {
        Box::pin(async move {
            data.v2()
                .positions(PositionAnchor::UserInConditions {
                    user: "0xuser".into(),
                    conditions: vec!["0xcond".into()],
                })
                .status(PositionStatus::Closed)
                .event_ids([1])
                .title("bitcoin")
                .filter_type(FilterType::Tokens)
                .filter_amount(1.0)
                .include_archived(true)
                .sort_by(PositionSortBy::RealizedPnl)
                .sort_direction(SortDirection::Asc)
                .start(1)
                .end(2)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/positions/combos", "combo_positions", |data| {
        Box::pin(async move {
            data.v2()
                .combo_positions("0xuser")
                .conditions(["0xcond"])
                .statuses([ComboPositionStatus::Open, ComboPositionStatus::Partial])
                .sort_by(ComboPositionSortBy::Updated)
                .sort_direction(SortDirection::Asc)
                .updated_after(1)
                .updated_before(2)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/user-pnl", "user_pnl", |data| {
        Box::pin(async move {
            data.v2()
                .user_pnl("0xuser")
                .interval(PnlInterval::OneWeek)
                .fidelity(PnlFidelity::OneDay)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/user-volume", "user_volume", |data| {
        Box::pin(async move {
            data.v2()
                .user_volume("0xuser")
                .start(1)
                .end(2)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/value", "value", |data| {
        Box::pin(async move {
            data.v2()
                .value("0xuser")
                .conditions(["0xcond"])
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/holders", "holders_pnl", |data| {
        Box::pin(async move {
            data.v2()
                .holders(["0xcond"])
                .min_balance(1.0)
                .include_pnl(true)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/live-volume", "live_volume", |data| {
        Box::pin(async move { data.v2().live_volume([1, 2]).send().await.map(|_| ()) })
    }),
    ("/v2/oi", "open_interest", |data| {
        Box::pin(async move {
            data.v2()
                .open_interest()
                .conditions(["0xcond"])
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/prices-history", "prices_history", |data| {
        Box::pin(async move {
            data.v2()
                .prices_history("123")
                .start(1)
                .end(2)
                .interval(PricesInterval::OneDay)
                .bucket_seconds(60)
                .as_of(3)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    // `/v2/resolutions` takes one selector family per request, so its three
    // entries together cover the documented parameters.
    ("/v2/resolutions", "resolutions", |data| {
        Box::pin(async move {
            data.v2()
                .resolutions(ResolutionKey::Question("0xq".into()))
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/resolutions", "resolutions", |data| {
        Box::pin(async move {
            data.v2()
                .resolutions(ResolutionKey::Conditions(vec!["0xcond".into()]))
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/resolutions", "resolutions", |data| {
        Box::pin(async move {
            data.v2()
                .resolutions(ResolutionKey::Events(vec!["1".into()]))
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/biggest-winners", "biggest_winners", |data| {
        Box::pin(async move {
            data.v2()
                .biggest_winners()
                .time_period(TimePeriod::Week)
                .category("sports")
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/builders/leaderboard", "builders_leaderboard", |data| {
        Box::pin(async move {
            data.v2()
                .builders_leaderboard()
                .time_period(TimePeriod::Month)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/builders/volume", "builder_volume", |data| {
        Box::pin(async move {
            data.v2()
                .builder_volume()
                .interval(TimePeriod::Week)
                .limit(10)
                .send()
                .await
                .map(|_| ())
        })
    }),
    // `leaderboard` and `leaderboard_user` share the path; together they
    // cover its parameters.
    ("/v2/leaderboard", "leaderboard", |data| {
        Box::pin(async move {
            data.v2()
                .leaderboard()
                .time_period(TimePeriod::All)
                .category("overall")
                .board(LeaderboardBoard::Volume)
                .limit(10)
                .cursor("cursor")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/leaderboard", "leaderboard_user", |data| {
        Box::pin(async move {
            data.v2()
                .leaderboard_user("0xuser")
                .time_period(TimePeriod::Day)
                .category("overall")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v2/status", "status", |data| {
        Box::pin(async move { data.v2().status().send().await.map(|_| ()) })
    }),
];

#[track_caller]
fn documented_parameters(path: &str) -> BTreeSet<String> {
    let Some(names) = query::documented_parameters(&spec(), path) else {
        panic!("{path} is not a documented route")
    };
    names
}

#[tokio::test]
async fn every_route_sends_exactly_the_documented_parameters() {
    let mut sent: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (path, fixture, fire) in ROUTES {
        let keys = query_keys_sent(path, fixture, *fire).await;
        sent.entry(path).or_default().extend(keys);
    }
    for (path, keys) in sent {
        assert_eq!(
            keys,
            documented_parameters(path),
            "{path}: query keys sent differ from the documented parameters"
        );
    }
}

#[test]
fn every_documented_route_has_a_builder() {
    let spec = spec();
    let documented: BTreeSet<&str> = spec["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let covered: BTreeSet<&str> = ROUTES.iter().map(|(path, _, _)| *path).collect();
    assert_eq!(
        covered, documented,
        "routes without a builder, or builders for undocumented routes"
    );
}
