//! Agreement between the v2 types and the served OpenAPI schema.
//!
//! The oracle is `docs/specs/data-v2/openapi.json`, which upstream serves from
//! the API host itself (`/v2/openapi.json`) and the nightly drift check keeps
//! byte-identical. Two things are checked, and each was shown to fail on a
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
//!
//! Every non-envelope schema must be in the table or excused with a reason.

use std::collections::BTreeSet;

use polyoxide_data::v2::{types::*, Page, Pagination};
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
