//! Agreement between the types and builders and `docs/specs/perps/openapi.json`.
//!
//! Only schemas reachable from the `/v1/info/*` routes are checked; the other
//! 300-odd belong to routes later slices implement. Wire-only fields are
//! allowed through `OBSERVED_EXTRA`, each recorded in
//! `docs/specs/perps/OBSERVED.md`.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use mockito::{Matcher, Server};
use polyoxide_perps::{
    api::{exchange::*, health::*, market::*, public::*},
    types::*,
    Perps, PerpsError,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/perps/openapi.json");

fn spec() -> Value {
    serde_json::from_str(SPEC).expect("spec parses")
}

fn schemas() -> Map<String, Value> {
    spec()["components"]["schemas"]
        .as_object()
        .expect("components.schemas")
        .clone()
}

fn ref_name(r: &str) -> &str {
    r.rsplit('/').next().unwrap()
}

/// Whether a property admits `null`. The perps schema puts `nullable: true`
/// on the `$ref` target (`exchange_open_interest`, `ui_live_time`), not on
/// the property, so the reference is followed.
fn is_nullable(schemas: &Map<String, Value>, prop: &Value) -> bool {
    if let Some(r) = prop["$ref"].as_str() {
        return is_nullable(schemas, &schemas[ref_name(r)]);
    }
    if let Some(types) = prop["type"].as_array() {
        return types.iter().any(|t| t == "null");
    }
    prop["nullable"] == true
        || prop["oneOf"]
            .as_array()
            .is_some_and(|arms| arms.iter().any(|a| a["type"] == "null"))
}

/// A value of the property's type. `full` also fills non-required properties
/// of any object it descends into. A positional row (`kline` and `mark_point`
/// have no `items`; `level` has primitive `items` and `maxItems: 2`) is taken
/// from its `example`, and so is a string, since the decimal fields are
/// `type: string` whose `example` is a decimal spelled as a string, and a
/// `Decimal` field would reject a placeholder.
fn synth(schemas: &Map<String, Value>, prop: &Value, full: bool) -> Value {
    if let Some(r) = prop["$ref"].as_str() {
        let target = &schemas[ref_name(r)];
        if target["type"] == "object"
            || target.get("allOf").is_some()
            || target.get("properties").is_some()
        {
            return synth_object(schemas, ref_name(r), full);
        }
        return synth(schemas, target, full);
    }
    if let Some(arms) = prop["oneOf"].as_array() {
        let arm = arms
            .iter()
            .find(|a| a["type"] != "null")
            .expect("non-null arm");
        return synth(schemas, arm, full);
    }
    if let Some(e) = prop["enum"].as_array() {
        return e[0].clone();
    }
    let ty = match &prop["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(ts) => ts
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap(),
        other => panic!("unsupported type {other} in {prop}"),
    };
    match ty {
        "string" => prop
            .get("example")
            .filter(|e| e.is_string())
            .cloned()
            .unwrap_or_else(|| Value::from("x")),
        "integer" => Value::from(1),
        "number" => Value::from(1.5),
        "boolean" => Value::from(true),
        "array" => {
            let items = prop.get("items");
            let positional = items.is_none_or(|i| i.get("$ref").is_none() && i["type"] != "object");
            match prop.get("example") {
                Some(example) if positional => example.clone(),
                _ => {
                    let items =
                        items.unwrap_or_else(|| panic!("array without items or example: {prop}"));
                    Value::Array(vec![synth(schemas, items, full)])
                }
            }
        }
        "object" => synth_object_inline(schemas, prop, full),
        other => panic!("unsupported type {other}"),
    }
}

/// An object schema's properties and required names, flattening `allOf`.
fn fields(schemas: &Map<String, Value>, schema: &Value) -> (Map<String, Value>, BTreeSet<String>) {
    if let Some(r) = schema["$ref"].as_str() {
        return fields(schemas, &schemas[ref_name(r)]);
    }
    let own = schema["properties"].as_object();
    let arms = schema["allOf"].as_array();
    assert!(
        own.is_some() || arms.is_some(),
        "neither properties nor allOf: {schema}"
    );
    let mut props = own.cloned().unwrap_or_default();
    let mut required: BTreeSet<String> = schema["required"]
        .as_array()
        .map(|r| {
            r.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    for arm in arms.into_iter().flatten() {
        let (arm_props, arm_required) = fields(schemas, arm);
        for (key, prop) in arm_props {
            props.insert(key, prop);
        }
        required.extend(arm_required);
    }
    (props, required)
}

fn synth_object_inline(schemas: &Map<String, Value>, schema: &Value, full: bool) -> Value {
    let (props, required) = fields(schemas, schema);
    let mut out = Map::new();
    for (key, prop) in &props {
        if full || (required.contains(key) && !is_nullable(schemas, prop)) {
            out.insert(key.clone(), synth(schemas, prop, full));
        }
    }
    Value::Object(out)
}

fn synth_object(schemas: &Map<String, Value>, name: &str, full: bool) -> Value {
    synth_object_inline(schemas, &schemas[name], full)
}

/// `(schema, field)` pairs on the wire but not in the spec. Each must be an
/// `Option` on the type and have an entry in OBSERVED.md.
const OBSERVED_EXTRA: &[(&str, &str)] = &[
    ("Instrument", "display_symbol"),
    ("Instrument", "close_only"),
    ("Instrument", "logo"),
    ("TradeData", "settlement"),
    ("AccountTradeData", "builder_fee"),
    ("AccountTradeData", "settlement"),
    ("AccountTradeData", "total_fee"),
    ("LimitTier", "connects_per_minute_limit"),
    ("LimitTier", "max_connections"),
    ("LimitTier", "ws_messages_burst_limit"),
    ("LimitTier", "ws_messages_per_minute_limit"),
];

/// Holds one type to one schema. A required-but-nullable field is treated as
/// omittable: it is left out of the minimal object and set to `null` in the
/// null check, which is why `ExchangeStatistics.open_interest` carries
/// `serde(default)`.
fn check<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    let (props, required) = fields(schemas, &schemas[name]);

    for (_, field) in OBSERVED_EXTRA.iter().filter(|(schema, _)| *schema == name) {
        assert!(
            !props.contains_key(*field),
            "{name}.{field} is now documented; drop the OBSERVED_EXTRA row and the OBSERVED.md entry"
        );
    }

    let minimal = synth_object(schemas, name, false);
    if let Err(e) = serde_json::from_value::<T>(minimal.clone()) {
        panic!("{name}: only required fields present should deserialize: {e}");
    }

    for (key, prop) in &props {
        if required.contains(key) && !is_nullable(schemas, prop) {
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
                panic!(
                    "{name}.{key} is optional or nullable in the spec but the type rejects null: {e}"
                );
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
    let mut documented: BTreeSet<&str> = props.keys().map(String::as_str).collect();
    documented.extend(
        OBSERVED_EXTRA
            .iter()
            .filter(|(schema, _)| *schema == name)
            .map(|(_, field)| *field),
    );
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
    "Time" => Time,
    "Exchange" => Exchange,
    "Asset" => Asset,
    "Instrument" => Instrument,
    "RiskTier" => RiskTier,
    "FeesInfo" => FeesInfo,
    "FeeScheduleEntry" => FeeScheduleEntry,
    "FeeTier" => FeeTier,
    "LimitTier" => LimitTier,
    "Ticker" => Ticker,
    "Statistic" => Statistic,
    "ExchangeStatistics" => ExchangeStatistics,
    "KlinesResponse" => Klines,
    "MarkHistoryResponse" => MarkHistory,
    "BBO" => Bbo,
    "Book" => Book,
    "Index" => Index,
    "IndexConstituent" => IndexConstituent,
    "Trades" => Trades,
    "TradeData" => Trade,
    "FundingHistory" => FundingHistory,
    "FundingRate" => FundingRate,
    "PublicPortfolio" => PublicPortfolio,
    "PublicPortfolioPosition" => PublicPortfolioPosition,
    "AccountTrades" => PositionFills,
    "AccountTradeData" => PositionFill,
    "Leaderboard" => Leaderboard,
    "LeaderboardEntry" => LeaderboardEntry,
    "LeaderboardAccount" => LeaderboardAccount,
    "InviteCheckResponse" => InviteCheck,
}

/// Reachable object schemas deliberately not in the table, each with a reason.
const NOT_MODELLED: &[(&str, &str)] = &[(
    "TickerData",
    "the allOf base of Ticker; no route serves it bare",
)];

/// Every named schema reachable by `$ref` from the `/v1/info/*` responses and
/// parameters.
fn reachable_from_info_routes() -> BTreeSet<String> {
    fn walk(schemas: &Map<String, Value>, node: &Value, seen: &mut BTreeSet<String>) {
        match node {
            Value::Object(map) => {
                if let Some(r) = map.get("$ref").and_then(Value::as_str) {
                    let name = ref_name(r).to_owned();
                    if seen.insert(name.clone()) {
                        walk(schemas, &schemas[&name], seen);
                    }
                }
                for v in map.values() {
                    walk(schemas, v, seen);
                }
            }
            Value::Array(items) => items.iter().for_each(|v| walk(schemas, v, seen)),
            _ => {}
        }
    }
    let spec = spec();
    let schemas = schemas();
    let mut seen = BTreeSet::new();
    for (path, ops) in spec["paths"].as_object().unwrap() {
        if path.starts_with("/v1/info/") {
            walk(&schemas, &ops["get"]["responses"]["200"], &mut seen);
            walk(&schemas, &ops["get"]["parameters"], &mut seen);
        }
    }
    seen
}

#[test]
fn every_reachable_object_schema_is_modelled_or_excused() {
    let schemas = schemas();
    let excused: BTreeSet<&str> = NOT_MODELLED.iter().map(|(n, _)| *n).collect();
    let modelled: BTreeSet<&str> = MODELLED.iter().copied().collect();
    let unaccounted: Vec<String> = reachable_from_info_routes()
        .into_iter()
        .filter(|n| {
            let s = &schemas[n];
            s["type"] == "object" || s.get("allOf").is_some() || s.get("properties").is_some()
        })
        .filter(|n| !modelled.contains(n.as_str()) && !excused.contains(n.as_str()))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "schemas neither modelled nor excused: {unaccounted:?}"
    );
}

#[test]
fn every_enum_matches_the_spec() {
    let schemas = schemas();
    fn wire<T: Serialize>(all: &[T]) -> BTreeSet<String> {
        all.iter()
            .map(|v| {
                serde_json::to_value(v)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }
    fn documented(schemas: &Map<String, Value>, name: &str) -> BTreeSet<String> {
        schemas[name]["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("{name} has no enum"))
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    }
    assert_eq!(wire(Interval::ALL), documented(&schemas, "interval"));
    assert_eq!(wire(Side::ALL), documented(&schemas, "side"));
    assert_eq!(
        wire(InstrumentType::ALL),
        documented(&schemas, "instrument_type")
    );
    assert_eq!(
        wire(InstrumentCategory::ALL),
        documented(&schemas, "category")
    );
    assert_eq!(wire(LeaderboardWindow::ALL), documented(&schemas, "window"));
    assert_eq!(wire(LeaderboardSort::ALL), documented(&schemas, "sort_by"));
    assert_eq!(wire(SortOrder::ALL), documented(&schemas, "sort"));

    let depths: BTreeSet<u64> = schemas["depth"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(
        BookDepth::ALL
            .iter()
            .map(|d| u64::from(d.levels()))
            .collect::<BTreeSet<_>>(),
        depths
    );
}

// ── Query parameters ────────────────────────────────────────────────

type Fire = fn(Perps) -> Pin<Box<dyn Future<Output = Result<(), PerpsError>> + Send>>;

/// Sends one request through `fire`, requires its response to decode, and
/// returns the query keys it carried.
async fn query_keys_sent(path: &str, fixture: &str, fire: Fire) -> BTreeSet<String> {
    let body_path = format!(
        "{}/tests/fixtures/{fixture}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let body = std::fs::read_to_string(&body_path).unwrap_or_else(|e| panic!("{body_path}: {e}"));
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
        .with_body(body)
        .create_async()
        .await;

    let decoded = fire(Perps::builder().base_url(server.url()).build().unwrap()).await;
    mock.assert_async().await;
    if let Err(e) = decoded {
        panic!("{path}: the builder did not decode `{fixture}.json`: {e}");
    }

    let seen = seen.lock().unwrap();
    let url = url::Url::parse(&format!("http://mock{}", seen.last().unwrap())).unwrap();
    url.query_pairs().map(|(key, _)| key.into_owned()).collect()
}

/// One entry per builder: its path, the fixture it must decode, and a call
/// using every argument and setter it has.
const ROUTES: &[(&str, &str, Fire)] = &[
    ("/v1/info/time", "time", |p| {
        Box::pin(async move { p.health().time().send().await.map(|_| ()) })
    }),
    ("/v1/info/exchange", "exchange", |p| {
        Box::pin(async move { p.exchange().exchange().send().await.map(|_| ()) })
    }),
    ("/v1/info/assets", "assets", |p| {
        Box::pin(async move { p.exchange().assets().send().await.map(|_| ()) })
    }),
    ("/v1/info/instruments", "instruments", |p| {
        Box::pin(async move {
            p.exchange()
                .instruments()
                .instrument_id(InstrumentId(1))
                .instrument_type(InstrumentType::Perpetual)
                .category(InstrumentCategory::Index)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/fees", "fees", |p| {
        Box::pin(async move { p.exchange().fees().send().await.map(|_| ()) })
    }),
    ("/v1/info/limit-tiers", "limit_tiers", |p| {
        Box::pin(async move { p.exchange().limit_tiers().send().await.map(|_| ()) })
    }),
    ("/v1/info/tickers", "tickers", |p| {
        Box::pin(async move {
            p.market()
                .tickers()
                .instrument_id(InstrumentId(1))
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/statistics", "statistics", |p| {
        Box::pin(async move {
            p.market()
                .statistics()
                .instrument_id(InstrumentId(1))
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/exchange-stats", "exchange_stats", |p| {
        Box::pin(async move { p.market().exchange_stats(1, 2).send().await.map(|_| ()) })
    }),
    ("/v1/info/klines", "klines", |p| {
        Box::pin(async move {
            p.market()
                .klines(InstrumentId(1), Interval::H1, 1)
                .end(2)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/mark-history", "mark_history", |p| {
        Box::pin(async move {
            p.market()
                .mark_history(InstrumentId(1), Interval::H1, 1)
                .end(2)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/bbo", "bbo", |p| {
        Box::pin(async move {
            p.market()
                .bbo()
                .instrument_id(InstrumentId(1))
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/book", "book", |p| {
        Box::pin(async move {
            p.market()
                .book(InstrumentId(1))
                .depth(BookDepth::Ten)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/index", "index", |p| {
        Box::pin(async move { p.market().index("BTC").send().await.map(|_| ()) })
    }),
    ("/v1/info/trades", "trades", |p| {
        Box::pin(async move {
            p.market()
                .trades(InstrumentId(1))
                .start(1)
                .end(2)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/funding", "funding", |p| {
        Box::pin(async move {
            p.market()
                .funding(InstrumentId(1))
                .start(1)
                .end(2)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/portfolio", "portfolio", |p| {
        Box::pin(async move { p.public().portfolio("0xabc").send().await.map(|_| ()) })
    }),
    ("/v1/info/position-fills", "position_fills", |p| {
        Box::pin(async move {
            p.public()
                .position_fills("0xabc", InstrumentId(1))
                .cursor("c")
                .sort(SortOrder::Asc)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/leaderboard", "leaderboard_account", |p| {
        Box::pin(async move {
            p.public()
                .leaderboard()
                .window(LeaderboardWindow::Week)
                .sort_by(LeaderboardSort::Pnl)
                .limit(1)
                .offset(0)
                .address("0xabc")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/v1/info/invite", "invite", |p| {
        Box::pin(async move {
            p.public()
                .invite("code")
                .address("0xabc")
                .send()
                .await
                .map(|_| ())
        })
    }),
];

#[tokio::test]
async fn every_builder_sends_exactly_the_documented_query_keys() {
    let spec = spec();
    let mut by_path: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (path, fixture, fire) in ROUTES {
        by_path
            .entry(path)
            .or_default()
            .extend(query_keys_sent(path, fixture, *fire).await);
    }
    for (path, sent) in by_path {
        let documented: BTreeSet<String> = spec["paths"][path]["get"]["parameters"]
            .as_array()
            .map(|ps| {
                ps.iter()
                    .map(|p| p["name"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(
            sent, documented,
            "{path}: query keys sent differ from the spec's parameters"
        );
    }
}

#[test]
fn every_info_route_has_a_builder_entry() {
    let spec = spec();
    let routes: BTreeSet<&str> = ROUTES.iter().map(|(p, _, _)| *p).collect();
    let missing: Vec<&str> = spec["paths"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|p| p.starts_with("/v1/info/") && *p != "/v1/info/ping")
        .filter(|p| !routes.contains(p))
        .collect();
    assert!(
        missing.is_empty(),
        "info routes with no builder entry: {missing:?}"
    );
}
