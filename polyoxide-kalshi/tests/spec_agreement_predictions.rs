//! Agreement between the types and builders and `docs/specs/kalshi/openapi.yaml`.
//!
//! Kalshi publishes YAML only, so the mirror is read through a dev-only parser and normalised
//! in memory into the form the shared helpers read (see `mirror`); the file is never changed.
//! Only the schemas the six public routes reach are checked. A key the host sends and the
//! schema lacks is modelled as an optional field, excused in `OBSERVED_EXTRA` and recorded in
//! `docs/specs/kalshi/OBSERVED.md`, as perps' are.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
};

use polyoxide_kalshi::predictions::{
    request::route_templates,
    types::{
        BookSide, EndpointCost, EndpointCosts, Event, EventResponse, Market, MarketResponse,
        MarketStatus, MarketStatusFilter, MarketsPage, MveFilter, OrderbookFp, OrderbookResponse,
        OutcomeSide, PriceRange, Trade, TradesPage,
    },
    Predictions, PredictionsError,
};
use polyoxide_test_support::{fixtures, openapi, query};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/kalshi/openapi.yaml");

/// What the mirror's paths are relative to (its `servers`), and what the crate's routes begin
/// with.
const PREFIX: &str = "/trade-api/v2";

/// `(schema, field, reason)`: fields the mirror requires and the types still read with a
/// `serde(default)`. The normalised copy lists them as optional, and
/// `every_defaulted_field_is_required_in_the_mirror` fails once the mirror stops requiring one.
const DEFAULTED: &[(&str, &str, &str)] = &[
    (
        "OrderbookCountFp",
        "yes_dollars",
        "a side with no levels is an empty list, however the host spells it",
    ),
    (
        "OrderbookCountFp",
        "no_dollars",
        "a side with no levels is an empty list, however the host spells it",
    ),
    (
        "GetEventResponse",
        "markets",
        "the deprecated list, which a caller that reads the nested markets never needs",
    ),
];

/// `(schema, field)` pairs on the wire but not in the spec, each an optional field of its type
/// and recorded in `docs/specs/kalshi/OBSERVED.md`. Empty until the capture sends a key the
/// schema lacks; a developer who meets one adds its row here.
const OBSERVED_EXTRA: &[(&str, &str)] = &[];

fn raw_spec() -> Value {
    let options = serde_saphyr::options! { budget: None };
    serde_saphyr::from_str_with_options(SPEC, options).expect("the mirror is YAML")
}

fn walk(node: &mut Value, visit: &mut impl FnMut(&mut Value)) {
    visit(node);
    match node {
        Value::Object(map) => map.values_mut().for_each(|child| walk(child, visit)),
        Value::Array(items) => items.iter_mut().for_each(|child| walk(child, visit)),
        _ => {}
    }
}

/// Each route's `parameters` as the query parameters it documents, with `$ref`s inlined.
fn inline_query_parameters(spec: &mut Value) {
    let shared = spec["components"]["parameters"].clone();
    let Some(paths) = spec.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };
    for item in paths.values_mut() {
        let Some(parameters) = item
            .get_mut("get")
            .and_then(|get| get.get_mut("parameters"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let inlined: Vec<Value> = parameters
            .iter()
            .map(|parameter| match parameter["$ref"].as_str() {
                Some(r) => shared[openapi::ref_name(r)].clone(),
                None => parameter.clone(),
            })
            .filter(|parameter| parameter["in"] == "query")
            .collect();
        *parameters = inlined;
    }
}

/// `{allOf: [{$ref: X}], ..}` is `{$ref: X, ..}`.
fn flatten_single_all_of(node: &mut Value) {
    let Some(map) = node.as_object_mut() else {
        return;
    };
    let reference = match map.get("allOf").and_then(Value::as_array) {
        Some(arms) if arms.len() == 1 => arms[0].get("$ref").cloned(),
        _ => None,
    };
    if let Some(reference) = reference {
        map.remove("allOf");
        map.insert("$ref".to_owned(), reference);
    }
}

/// An object with no properties is the empty object, which the helper can synthesise.
fn give_bare_objects_properties(node: &mut Value) {
    let Some(map) = node.as_object_mut() else {
        return;
    };
    if map.get("type").and_then(Value::as_str) == Some("object")
        && !map.contains_key("properties")
        && !map.contains_key("allOf")
    {
        map.insert("properties".to_owned(), Value::Object(Map::new()));
    }
}

fn schema_mut<'a>(spec: &'a mut Value, name: &str) -> Option<&'a mut Value> {
    spec.get_mut("components")?
        .get_mut("schemas")?
        .get_mut(name)
}

/// The mirror in the form the shared helpers read.
fn mirror() -> Value {
    let mut spec = raw_spec();
    inline_query_parameters(&mut spec);
    walk(&mut spec, &mut |node| {
        flatten_single_all_of(node);
        give_bare_objects_properties(node);
    });
    // `PriceRange`'s fields are strings with no example, and its type reads decimals.
    if let Some(properties) = schema_mut(&mut spec, "PriceRange")
        .and_then(|schema| schema.get_mut("properties"))
        .and_then(Value::as_object_mut)
    {
        for property in properties.values_mut() {
            property["example"] = Value::from("0.0100");
        }
    }
    for (schema, field, _) in DEFAULTED {
        if let Some(required) = schema_mut(&mut spec, schema)
            .and_then(|schema| schema.get_mut("required"))
            .and_then(Value::as_array_mut)
        {
            required.retain(|name| name != field);
        }
    }
    spec
}

fn mirror_schemas() -> Map<String, Value> {
    openapi::schemas(&mirror())
}

#[track_caller]
fn check_schema<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    openapi::check::<T>(schemas, name, OBSERVED_EXTRA);
}

macro_rules! modelled_schemas {
    ($($schema:literal => $ty:ty),+ $(,)?) => {
        const MODELLED: &[&str] = &[$($schema),+];
        #[test]
        fn every_modelled_schema_agrees_with_the_spec() {
            let schemas = mirror_schemas();
            $( check_schema::<$ty>(&schemas, $schema); )+
        }
    };
}

modelled_schemas! {
    "Market" => Market,
    "PriceRange" => PriceRange,
    "GetMarketResponse" => MarketResponse,
    "GetMarketsResponse" => MarketsPage,
    "OrderbookCountFp" => OrderbookFp,
    "GetMarketOrderbookResponse" => OrderbookResponse,
    "Trade" => Trade,
    "GetTradesResponse" => TradesPage,
    "EventData" => Event,
    "GetEventResponse" => EventResponse,
    "EndpointTokenCost" => EndpointCost,
    "GetAccountEndpointCostsResponse" => EndpointCosts,
}

/// Reachable object schemas deliberately not in the table, each with a reason.
const NOT_MODELLED: &[(&str, &str)] = &[
    (
        "MveSelectedLeg",
        "a combination's legs, kept as raw JSON in Market::mve_selected_legs",
    ),
    (
        "SettlementSource",
        "an event's settlement sources, kept as raw JSON in Event::settlement_sources",
    ),
];

/// The mirror's paths for the six routes this story implements.
const SPEC_ROUTES: &[&str] = &[
    "/markets",
    "/markets/{ticker}",
    "/markets/{ticker}/orderbook",
    "/markets/trades",
    "/events/{event_ticker}",
    "/account/endpoint_costs",
];

/// Every named schema reachable by `$ref` from those routes' responses.
fn reachable_from_the_routes() -> BTreeSet<String> {
    fn follow(schemas: &Map<String, Value>, node: &Value, seen: &mut BTreeSet<String>) {
        match node {
            Value::Object(map) => {
                if let Some(r) = map.get("$ref").and_then(Value::as_str) {
                    let name = openapi::ref_name(r).to_owned();
                    if seen.insert(name.clone()) {
                        follow(schemas, &schemas[&name], seen);
                    }
                }
                for child in map.values() {
                    follow(schemas, child, seen);
                }
            }
            Value::Array(items) => items.iter().for_each(|child| follow(schemas, child, seen)),
            _ => {}
        }
    }
    let spec = mirror();
    let schemas = mirror_schemas();
    let mut seen = BTreeSet::new();
    for path in SPEC_ROUTES {
        let item = &spec["paths"][*path];
        assert!(item.is_object(), "the mirror has no path {path}");
        let get = &item["get"];
        assert!(get.is_object(), "the mirror's {path} has no get");
        let ok = &get["responses"]["200"];
        assert!(
            ok.is_object(),
            "the mirror's GET {path} has no 200 response"
        );
        follow(&schemas, ok, &mut seen);
    }
    seen
}

#[test]
fn every_reachable_object_schema_is_modelled_or_excused() {
    let schemas = mirror_schemas();
    let excused: BTreeSet<&str> = NOT_MODELLED.iter().map(|(name, _)| *name).collect();
    let modelled: BTreeSet<&str> = MODELLED.iter().copied().collect();
    let unaccounted: Vec<String> = reachable_from_the_routes()
        .into_iter()
        .filter(|name| {
            let schema = &schemas[name];
            schema["type"] == "object"
                || schema.get("allOf").is_some()
                || schema.get("properties").is_some()
        })
        .filter(|name| !modelled.contains(name.as_str()) && !excused.contains(name.as_str()))
        .collect();
    assert!(
        unaccounted.is_empty(),
        "schemas neither modelled nor excused: {unaccounted:?}"
    );
}

#[test]
fn every_defaulted_field_is_required_in_the_mirror() {
    let raw = raw_spec();
    for (schema, field, reason) in DEFAULTED {
        let required = raw["components"]["schemas"][*schema]["required"]
            .as_array()
            .unwrap_or_else(|| panic!("{schema} has no required list"));
        assert!(
            required.iter().any(|name| name == field),
            "{schema}.{field} is no longer required in the mirror; drop the DEFAULTED row ({reason})"
        );
    }
}

// ── Enums ───────────────────────────────────────────────────────────

fn wire<T: Serialize>(all: &[T]) -> BTreeSet<String> {
    all.iter()
        .map(|variant| {
            serde_json::to_value(variant)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn documented(values: &Value, what: &str) -> BTreeSet<String> {
    values
        .as_array()
        .unwrap_or_else(|| panic!("{what} has no enum"))
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn every_enum_matches_the_spec() {
    let spec = mirror();
    let schemas = mirror_schemas();
    assert_eq!(
        wire(MarketStatus::ALL),
        documented(
            &schemas["Market"]["properties"]["status"]["enum"],
            "Market.status"
        )
    );
    assert_eq!(
        wire(OutcomeSide::ALL),
        documented(&schemas["OutcomeSide"]["enum"], "OutcomeSide")
    );
    assert_eq!(
        wire(OutcomeSide::ALL),
        documented(
            &schemas["Trade"]["properties"]["taker_outcome_side"]["enum"],
            "Trade.taker_outcome_side"
        )
    );
    assert_eq!(
        wire(OutcomeSide::ALL),
        documented(
            &schemas["Trade"]["properties"]["taker_side"]["enum"],
            "Trade.taker_side"
        )
    );
    assert_eq!(
        wire(BookSide::ALL),
        documented(&schemas["BookSide"]["enum"], "BookSide")
    );

    let filter = documented(
        &spec["components"]["parameters"]["MarketStatusQuery"]["schema"]["enum"],
        "MarketStatusQuery",
    );
    assert_eq!(wire(MarketStatusFilter::ALL), filter);

    let mve = documented(
        &spec["components"]["parameters"]["MveFilterQuery"]["schema"]["enum"],
        "MveFilterQuery",
    );
    assert_eq!(wire(MveFilter::ALL), mve);
}

// ── Query parameters ────────────────────────────────────────────────

type Dispatch =
    fn(Predictions) -> Pin<Box<dyn Future<Output = Result<(), PredictionsError>> + Send>>;

/// Sends one request through `fire`, requires its response to decode, and returns the query
/// keys it carried.
async fn keys_of(mock_path: &str, fixture: &str, fire: Dispatch) -> BTreeSet<String> {
    let body = fixtures!("predictions").text(fixture);
    query::keys_sent(&format!("{PREFIX}{mock_path}"), fixture, body, |url| {
        fire(Predictions::builder().base_url(url).build().unwrap())
    })
    .await
}

/// One entry per builder with a query: the mirror's path, the path it is sent to, the fixture
/// it must decode, and a call using every setter it has. The endpoint-costs route documents no
/// query and is checked in `routes_that_document_no_query_send_none`.
const ROUTES: &[(&str, &str, &str, Dispatch)] = &[
    ("/markets", "/markets", "markets_page", |p| {
        Box::pin(async move {
            p.markets()
                .list()
                .status(MarketStatusFilter::Open)
                .event_ticker("EV-1")
                .series_ticker("SER-1")
                .mve_filter(MveFilter::Exclude)
                .limit(5)
                .cursor("c1")
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/markets/{ticker}", "/markets/KXTEST-1", "market", |p| {
        Box::pin(async move {
            let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
            p.markets().get(&ticker).send().await.map(|_| ())
        })
    }),
    (
        "/markets/{ticker}/orderbook",
        "/markets/KXTEST-1/orderbook",
        "orderbook",
        |p| {
            Box::pin(async move {
                let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
                p.markets()
                    .orderbook(&ticker)
                    .depth(1)
                    .send()
                    .await
                    .map(|_| ())
            })
        },
    ),
    ("/markets/trades", "/markets/trades", "trades", |p| {
        Box::pin(async move {
            let ticker = polyoxide_kalshi::predictions::key::Ticker::new("KXTEST-1").unwrap();
            p.markets()
                .trades()
                .ticker(&ticker)
                .limit(5)
                .cursor("c1")
                .min_ts(6)
                .max_ts(7)
                .send()
                .await
                .map(|_| ())
        })
    }),
    ("/events/{event_ticker}", "/events/EV-1", "event", |p| {
        Box::pin(async move {
            p.events()
                .get("EV-1")
                .with_nested_markets(true)
                .send()
                .await
                .map(|_| ())
        })
    }),
];

/// The parameters a mirror path documents that no setter sends, each with a reason.
const NOT_SENT: &[(&str, &str, &str)] = &[
    (
        "/markets",
        "min_created_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "max_created_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "min_updated_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "max_updated_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "max_close_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "min_close_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "min_settled_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "max_settled_ts",
        "a filter the skeleton does not model",
    ),
    (
        "/markets",
        "tickers",
        "a filter the skeleton does not model",
    ),
    (
        "/markets/trades",
        "is_block_trade",
        "a filter the skeleton does not model",
    ),
    (
        "/events/{event_ticker}",
        "exclude_markets",
        "the skeleton asks for the markets nested or not at all",
    ),
];

#[tokio::test]
async fn every_builder_sends_only_documented_query_keys_and_every_other_one_is_excused() {
    let spec = mirror();
    let mut by_path: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    for (spec_path, mock_path, fixture, fire) in ROUTES {
        by_path
            .entry(spec_path)
            .or_default()
            .extend(keys_of(mock_path, fixture, *fire).await);
    }
    for (spec_path, sent) in by_path {
        let documented: BTreeSet<String> =
            query::documented_parameters(&spec, spec_path).unwrap_or_default();
        let excused: BTreeSet<String> = NOT_SENT
            .iter()
            .filter(|(path, _, _)| *path == spec_path)
            .map(|(_, parameter, _)| (*parameter).to_owned())
            .collect();
        let unknown: Vec<&String> = sent.difference(&documented).collect();
        assert!(
            unknown.is_empty(),
            "{spec_path}: query keys sent that the mirror does not document: {unknown:?}"
        );
        let unsent: BTreeSet<String> = documented.difference(&sent).cloned().collect();
        assert_eq!(
            unsent, excused,
            "{spec_path}: documented parameters no setter sends differ from NOT_SENT"
        );
    }
}

#[test]
fn routes_that_document_no_query_send_none() {
    let spec = mirror();
    for path in [
        "/account/endpoint_costs",
        "/exchange/status",
        "/account/limits",
    ] {
        assert_eq!(
            query::documented_parameters(&spec, path),
            Some(BTreeSet::new()),
            "{path} documents a query, so its builder needs a setter or an entry in ROUTES"
        );
    }
}

// ── The route table ─────────────────────────────────────────────────

/// `/trade-api/v2/markets/:ticker` as the mirror writes it, without the server prefix:
/// `/markets/{ticker}`.
fn mirror_path(template: &str) -> String {
    let relative = template
        .strip_prefix(PREFIX)
        .unwrap_or_else(|| panic!("{template} does not begin with {PREFIX}"));
    relative
        .split('/')
        .map(|segment| match segment.strip_prefix(':') {
            Some(name) => format!("{{{name}}}"),
            None => segment.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn every_route_the_crate_holds_is_a_documented_operation() {
    let spec = mirror();
    let mut count = 0;
    for (method, template) in route_templates() {
        count += 1;
        let path = mirror_path(template);
        assert!(
            spec["paths"][&path][method.to_ascii_lowercase()].is_object(),
            "{method} {template} is not an operation of the mirror at {path}"
        );
    }
    assert_eq!(count, 10, "the route table holds ten routes");
}
