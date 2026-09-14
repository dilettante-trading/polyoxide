//! Agreement between the v2 types and builders and the served OpenAPI schema.
//!
//! The oracle is `docs/specs/data-v2/openapi.json`, which upstream serves from
//! the API host itself (`/v2/openapi.json`) and the nightly drift check keeps
//! byte-identical. Three things are checked, and each was shown to fail on a
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
//! 3. **Query parameters.** Each route, built with every argument and setter,
//!    must send exactly the spec's parameter names. Mutations caught: a
//!    camelCase key; an extra `offset` setter.
//!
//! Every non-envelope schema must be in the table or excused with a reason.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use mockito::{Matcher, Server};
use polyoxide_data::{
    types::SortDirection,
    v2::{types::*, Page, Pagination},
    DataApi,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/data-v2/openapi.json");

fn spec() -> Value {
    serde_json::from_str(SPEC).expect("spec parses")
}

fn schemas() -> Map<String, Value> {
    spec()["components"]["schemas"]
        .as_object()
        .expect("components.schemas")
        .clone()
}

fn is_nullable(prop: &Value) -> bool {
    if let Some(types) = prop["type"].as_array() {
        return types.iter().any(|t| t == "null");
    }
    prop["oneOf"]
        .as_array()
        .is_some_and(|arms| arms.iter().any(|a| a["type"] == "null"))
}

/// A value of the property's type. `full` also fills non-required properties
/// of any object it descends into.
fn synth(schemas: &Map<String, Value>, prop: &Value, full: bool) -> Value {
    if let Some(r) = prop["$ref"].as_str() {
        let name = r.rsplit('/').next().unwrap();
        return synth_object(schemas, name, full);
    }
    if let Some(arms) = prop["oneOf"].as_array() {
        let arm = arms
            .iter()
            .find(|a| a["type"] != "null")
            .expect("non-null arm");
        return synth(schemas, arm, full);
    }
    let ty = match &prop["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(ts) => ts
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap(),
        other => panic!("unsupported type {other}"),
    };
    match ty {
        "string" => Value::from("x"),
        "integer" => Value::from(1),
        "number" => Value::from(1.5),
        "boolean" => Value::from(true),
        "array" => Value::Array(vec![synth(schemas, &prop["items"], full)]),
        other => panic!("unsupported type {other}"),
    }
}

fn synth_object(schemas: &Map<String, Value>, name: &str, full: bool) -> Value {
    let schema = &schemas[name];
    let required: BTreeSet<&str> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut out = Map::new();
    for (key, prop) in schema["properties"].as_object().unwrap() {
        if full || (required.contains(key.as_str()) && !is_nullable(prop)) {
            out.insert(key.clone(), synth(schemas, prop, full));
        }
    }
    Value::Object(out)
}

fn check<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    let schema = &schemas[name];
    let props = schema["properties"].as_object().unwrap();
    let required: BTreeSet<&str> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let minimal = synth_object(schemas, name, false);
    if let Err(e) = serde_json::from_value::<T>(minimal.clone()) {
        panic!("{name}: only required fields present should deserialize: {e}");
    }

    for (key, prop) in props {
        let strictly_required = required.contains(key.as_str()) && !is_nullable(prop);
        if strictly_required {
            let mut without = minimal.clone();
            without.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<T>(without).is_err(),
                "{name}.{key} is required and non-nullable in the spec but the type accepts it missing"
            );
        } else {
            let mut with_null = minimal.clone();
            with_null
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), Value::Null);
            if let Err(e) = serde_json::from_value::<T>(with_null) {
                panic!("{name}.{key} is optional or nullable in the spec but the type rejects null: {e}");
            }
        }
    }

    let full = synth_object(schemas, name, true);
    let parsed: T = serde_json::from_value(full)
        .unwrap_or_else(|e| panic!("{name}: every field present should deserialize: {e}"));
    let emitted = serde_json::to_value(&parsed).unwrap();
    let emitted: BTreeSet<&str> = emitted
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let documented: BTreeSet<&str> = props.keys().map(String::as_str).collect();
    assert_eq!(
        emitted, documented,
        "{name}: emitted keys differ from the spec's properties"
    );
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
        // `data` is byte-identical to a named schema checked above.
        .filter(|n| !n.starts_with("Envelope_"))
        .filter(|n| !modelled.contains(n) && !excused.contains(n))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "schemas neither modelled nor excused: {unaccounted:?}"
    );
}

// ── Query parameters ────────────────────────────────────────────────

type Fire = fn(DataApi) -> Pin<Box<dyn Future<Output = ()> + Send>>;

/// Sends one request through `fire` and returns the query keys it carried.
///
/// The mock answers `{}`, which no route deserializes, so `fire` ignores the
/// result: only what went over the wire matters here.
async fn query_keys_sent(path: &str, fire: Fire) -> BTreeSet<String> {
    let mut server = Server::new_async().await;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = Arc::clone(&seen);
    let mock = server
        .mock("GET", path)
        .match_query(Matcher::Any)
        .match_request(move |request| {
            sink.lock()
                .unwrap()
                .push(request.path_and_query().to_owned());
            true
        })
        .with_status(200)
        .with_body("{}")
        .create_async()
        .await;

    fire(DataApi::builder().base_url(server.url()).build().unwrap()).await;
    mock.assert_async().await;

    let seen = seen.lock().unwrap();
    let url = url::Url::parse(&format!("http://mock{}", seen.last().unwrap())).unwrap();
    url.query_pairs().map(|(key, _)| key.into_owned()).collect()
}

/// One entry per builder, each calling every argument and setter it has.
/// Two builders may share a path (`leaderboard` and `leaderboard_user`); their
/// keys are unioned before comparison.
const ROUTES: &[(&str, Fire)] = &[
    ("/v2/user-stats", |data| {
        Box::pin(async move {
            let _ = data.v2().user_stats("0xuser").send().await;
        })
    }),
    ("/v2/trades", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
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
                .await;
        })
    }),
    ("/v2/activity", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
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
                .await;
        })
    }),
    ("/v2/activity/combos", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .combo_activity("0xuser")
                .conditions(["0xcond"])
                .limit(10)
                .cursor("cursor")
                .send()
                .await;
        })
    }),
    ("/v2/approvals", |data| {
        Box::pin(async move {
            let _ = data.v2().approvals("0xuser").send().await;
        })
    }),
    ("/v2/positions", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
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
                .await;
        })
    }),
    ("/v2/positions/combos", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
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
                .await;
        })
    }),
    ("/v2/user-pnl", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .user_pnl("0xuser")
                .interval(PnlInterval::OneWeek)
                .fidelity(PnlFidelity::OneDay)
                .send()
                .await;
        })
    }),
    ("/v2/user-volume", |data| {
        Box::pin(async move {
            let _ = data.v2().user_volume("0xuser").start(1).end(2).send().await;
        })
    }),
    ("/v2/value", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .value("0xuser")
                .conditions(["0xcond"])
                .send()
                .await;
        })
    }),
    ("/v2/holders", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .holders(["0xcond"])
                .min_balance(1.0)
                .include_pnl(true)
                .limit(10)
                .cursor("cursor")
                .send()
                .await;
        })
    }),
    ("/v2/live-volume", |data| {
        Box::pin(async move {
            let _ = data.v2().live_volume([1, 2]).send().await;
        })
    }),
    ("/v2/oi", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .open_interest()
                .conditions(["0xcond"])
                .send()
                .await;
        })
    }),
    ("/v2/prices-history", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .prices_history("123")
                .start(1)
                .end(2)
                .interval(PricesInterval::OneDay)
                .bucket_seconds(60)
                .as_of(3)
                .limit(10)
                .cursor("cursor")
                .send()
                .await;
        })
    }),
    // `/v2/resolutions` takes one selector family per request, so its three
    // entries together cover the documented parameters.
    ("/v2/resolutions", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .resolutions(ResolutionKey::Question("0xq".into()))
                .send()
                .await;
        })
    }),
    ("/v2/resolutions", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .resolutions(ResolutionKey::Conditions(vec!["0xcond".into()]))
                .send()
                .await;
        })
    }),
    ("/v2/resolutions", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .resolutions(ResolutionKey::Events(vec!["1".into()]))
                .send()
                .await;
        })
    }),
    ("/v2/biggest-winners", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .biggest_winners()
                .time_period(TimePeriod::Week)
                .category("sports")
                .limit(10)
                .cursor("cursor")
                .send()
                .await;
        })
    }),
    ("/v2/builders/leaderboard", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .builders_leaderboard()
                .time_period(TimePeriod::Month)
                .limit(10)
                .cursor("cursor")
                .send()
                .await;
        })
    }),
    ("/v2/builders/volume", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .builder_volume()
                .interval(TimePeriod::Week)
                .limit(10)
                .send()
                .await;
        })
    }),
    // `leaderboard` and `leaderboard_user` share the path; together they
    // cover its parameters.
    ("/v2/leaderboard", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .leaderboard()
                .time_period(TimePeriod::All)
                .category("overall")
                .board(LeaderboardBoard::Volume)
                .limit(10)
                .cursor("cursor")
                .send()
                .await;
        })
    }),
    ("/v2/leaderboard", |data| {
        Box::pin(async move {
            let _ = data
                .v2()
                .leaderboard_user("0xuser")
                .time_period(TimePeriod::Day)
                .category("overall")
                .send()
                .await;
        })
    }),
];

fn documented_parameters(path: &str) -> BTreeSet<String> {
    let spec = spec();
    let operation = &spec["paths"][path]["get"];
    assert!(operation.is_object(), "{path} is not a documented route");
    operation["parameters"]
        .as_array()
        .map(|params| {
            params
                .iter()
                .map(|p| p["name"].as_str().unwrap().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn every_route_sends_exactly_the_documented_parameters() {
    let mut sent: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (path, fire) in ROUTES {
        let keys = query_keys_sent(path, *fire).await;
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
