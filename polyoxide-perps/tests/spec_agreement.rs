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
};

use polyoxide_perps::{
    api::{exchange::*, health::*, market::*, public::*},
    types::*,
    Perps, PerpsError,
};
use polyoxide_test_support::{fixtures, openapi, query};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{Map, Value};

const SPEC: &str = include_str!("../../docs/specs/perps/openapi.json");

fn spec() -> Value {
    serde_json::from_str(SPEC).expect("spec parses")
}

fn schemas() -> Map<String, Value> {
    openapi::schemas(&spec())
}

fn ref_name(r: &str) -> &str {
    openapi::ref_name(r)
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

/// Holds one type to one schema. The perps schema puts `nullable: true` on
/// the `$ref` target (`exchange_open_interest`, `ui_live_time`), not on the
/// property, which the shared check follows. A required-but-nullable field is
/// treated as omittable: it is left out of the minimal object and set to
/// `null` in the null check, which is why `ExchangeStatistics.open_interest`
/// carries `serde(default)`.
#[track_caller]
fn check<T: DeserializeOwned + Serialize>(schemas: &Map<String, Value>, name: &str) {
    openapi::check::<T>(schemas, name, OBSERVED_EXTRA);
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
    let body = fixtures!().text(fixture);
    query::keys_sent(path, fixture, body, |url| {
        fire(Perps::builder().base_url(url).build().unwrap())
    })
    .await
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
        let documented: BTreeSet<String> =
            query::documented_parameters(&spec, path).unwrap_or_default();
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
