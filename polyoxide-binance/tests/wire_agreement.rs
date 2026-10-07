//! Agreement between the types and payloads captured from the live host.
//! Provenance is in `tests/fixtures/PROVENANCE.md`.
//!
//! Each fixture is decoded into its type and encoded back, and:
//!
//! 1. **Nothing unmodelled.** Every key path the server sent is emitted by the
//!    type.
//! 2. **Nothing invented.** Every key path the type emits was sent by the
//!    server. An `Option` encodes as `null`, so a field modelled but absent from
//!    the wire shows up here instead of hiding.
//! 3. **Nothing altered.** Every scalar present on both sides is equal, so a
//!    price with more digits than an `f64` keeps survives exactly or fails.
//!
//! `kline`'s twelfth element, which Binance documents as "ignore", is the one
//! thing dropped on purpose; positional arrays are compared by index only as
//! far as the shorter side goes.

mod common;

use polyoxide_binance::usdm::types::{
    AggTrade, Depth, ExchangeInfo, FundingInfo, FundingRate, Kline, OpenInterest, PremiumIndex,
    ServerTime, Ticker24h,
};
use serde::{de::DeserializeOwned, Serialize};

fn check<T: DeserializeOwned + Serialize>(fixture: &str) {
    let path = format!(
        "{}/tests/fixtures/rest/{fixture}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let diff = common::compare::<T>(fixture, &text);
    assert!(
        diff.unmodelled.is_empty(),
        "{fixture}: the server sent {:?}, which the type does not model",
        diff.unmodelled
    );
    assert!(
        diff.invented.is_empty(),
        "{fixture}: the type emits {:?}, which the server did not send",
        diff.invented
    );
}

#[test]
fn every_rest_fixture_agrees_with_its_type() {
    check::<ExchangeInfo>("exchange_info");
    check::<ServerTime>("time");
    check::<Vec<FundingInfo>>("funding_info");
    check::<Vec<Ticker24h>>("ticker_24hr");
    check::<Vec<PremiumIndex>>("premium_index");
    check::<Vec<Kline>>("klines");
    check::<Vec<FundingRate>>("funding_rate");
    check::<Vec<FundingRate>>("funding_rate_2019");
    check::<OpenInterest>("open_interest");
    check::<Vec<AggTrade>>("agg_trades");
    check::<Depth>("depth");
}

#[test]
fn the_fixtures_cover_the_cases_the_types_exist_for() {
    let read = |name: &str| {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/rest/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    };

    let info: ExchangeInfo = serde_json::from_str(&read("exchange_info")).unwrap();
    use polyoxide_binance::usdm::types::{ContractType, Filter, SymbolStatus, UnderlyingType};
    let has = |f: &dyn Fn(&polyoxide_binance::usdm::types::SymbolInfo) -> bool| {
        info.symbols.iter().any(f)
    };
    assert!(
        has(&|s| s.contract_type == ContractType::TradifiPerpetual),
        "a TradFi perpetual"
    );
    assert!(
        has(&|s| s.contract_type == ContractType::CurrentQuarter),
        "a quarterly"
    );
    assert!(
        has(&|s| s.status == SymbolStatus::Settling),
        "a delisted contract"
    );
    assert!(
        has(&|s| !s.symbol.as_str().is_ascii()),
        "a Chinese-character symbol"
    );
    assert!(
        has(&|s| s.symbol.as_str().contains('_')),
        "a symbol with an underscore"
    );
    // An unknown value would land in `Other` and decode fine; failing here
    // is how a value Binance adds gets modelled instead of passing silently.
    let unnamed: Vec<_> = info
        .symbols
        .iter()
        .filter(|s| {
            matches!(s.contract_type, ContractType::Other(_))
                || matches!(s.status, SymbolStatus::Other(_))
                || matches!(s.underlying_type, UnderlyingType::Other(_))
        })
        .map(|s| (&s.symbol, &s.contract_type, &s.status, &s.underlying_type))
        .collect();
    assert!(
        unnamed.is_empty(),
        "values the enums do not name, model them: {unnamed:?}"
    );
    // The same for filters: a misspelled parse arm would otherwise turn a
    // PRICE_FILTER into `Other`, and every other test would still pass.
    let unnamed: Vec<&Filter> = info
        .symbols
        .iter()
        .flat_map(|s| &s.filters)
        .filter(|f| matches!(f, Filter::Other { .. }))
        .collect();
    assert!(
        unnamed.is_empty(),
        "filter types Filter does not name, model them: {unnamed:?}"
    );

    let funding: Vec<FundingInfo> = serde_json::from_str(&read("funding_info")).unwrap();
    assert!(
        funding.iter().any(|f| f.update_time.is_none()),
        "a null updateTime"
    );

    let old: Vec<FundingRate> = serde_json::from_str(&read("funding_rate_2019")).unwrap();
    assert!(old.iter().all(|f| f.mark_price.is_none()), "markPrice \"\"");
}
