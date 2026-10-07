//! Live integration tests against `fapi.binance.com`.
//!
//! These hit the real host and need network access, so they are `#[ignore]`d.
//! No credentials are needed. Run with:
//! ```sh
//! cargo test -p polyoxide-binance --test live_api -- --ignored
//! ```
//!
//! `live_responses_carry_no_unmodelled_keys` is the host's drift detector:
//! Binance publishes no schema, so nothing else notices a new field.

mod common;

use std::time::Duration;

use polyoxide_binance::{
    usdm::types::{
        AggTrade, ContractType, Depth, DepthLimit, ExchangeInfo, FundingInfo, FundingRate,
        Interval, Kline, OpenInterest, PremiumIndex, ServerTime, Symbol, SymbolStatus, Ticker24h,
    },
    BinanceError, Usdm,
};
use serde::{de::DeserializeOwned, Serialize};

fn client() -> Usdm {
    Usdm::new().expect("binance client")
}

/// BTCUSDT, a trading TradFi perpetual and a trading Chinese-character
/// perpetual, chosen from today's `exchangeInfo` so a delisting cannot break
/// the suite.
async fn three_kinds_of_symbol(usdm: &Usdm) -> Vec<Symbol> {
    let info = usdm
        .exchange()
        .exchange_info()
        .send()
        .await
        .expect("exchangeInfo");
    let trading =
        |s: &&polyoxide_binance::usdm::types::SymbolInfo| s.status == SymbolStatus::Trading;
    let tradfi = info
        .symbols
        .iter()
        .filter(trading)
        .find(|s| s.contract_type == ContractType::TradifiPerpetual)
        .expect("no suitable market: no trading TradFi perpetual is listed");
    let chinese = info
        .symbols
        .iter()
        .filter(trading)
        .find(|s| !s.symbol.as_str().is_ascii())
        .expect("no suitable market: no trading Chinese-character symbol is listed");
    vec![
        Symbol::new("BTCUSDT").unwrap(),
        Symbol::new(&*tradfi.symbol).unwrap(),
        Symbol::new(&*chinese.symbol).unwrap(),
    ]
}

#[tokio::test]
#[ignore]
async fn live_ping_time_and_the_weight_header() {
    let usdm = client();
    let latency = usdm.health().ping().await.expect("ping");
    assert!(latency < Duration::from_secs(10), "latency {latency:?}");
    let time = usdm.health().time().send().await.expect("time");
    assert!(time.server_time > 1_790_000_000_000);
    assert!(
        usdm.weight_budget().used() >= 2,
        "X-MBX-USED-WEIGHT-1M was not recorded"
    );
}

#[tokio::test]
#[ignore]
async fn live_every_market_route_answers_for_three_kinds_of_symbol() {
    let usdm = client();
    for symbol in three_kinds_of_symbol(&usdm).await {
        let market = usdm.market();
        let ticker = market.ticker_24h(&symbol).send().await.expect("ticker");
        assert_eq!(ticker.symbol, symbol.as_str());
        let index = market
            .premium_index(&symbol)
            .send()
            .await
            .expect("premiumIndex");
        assert!(
            index.mark_price > 0.into(),
            "{symbol} mark {}",
            index.mark_price
        );
        let candles = market
            .klines(&symbol, Interval::M1)
            .limit(2)
            .send()
            .await
            .expect("klines");
        assert_eq!(candles.len(), 2, "{symbol}");
        let oi = market
            .open_interest(&symbol)
            .send()
            .await
            .expect("openInterest");
        assert_eq!(oi.symbol, symbol.as_str());
        let trades = market
            .agg_trades(&symbol)
            .limit(2)
            .send()
            .await
            .expect("aggTrades");
        assert!(trades.len() <= 2, "{symbol}");
        let book = market
            .depth(&symbol)
            .limit(DepthLimit::Five)
            .send()
            .await
            .expect("depth");
        assert!(book.bids.len() <= 5 && book.asks.len() <= 5, "{symbol}");
        let funding = market
            .funding_rate()
            .symbol(&symbol)
            .limit(2)
            .send()
            .await
            .expect("fundingRate");
        assert!(funding.iter().all(|row| row.symbol == symbol.as_str()));
    }
}

#[tokio::test]
#[ignore]
async fn live_an_unknown_symbol_is_venue_error_1121() {
    let err = client()
        .market()
        .open_interest(&Symbol::new("NOTASYMBOLUSDT").unwrap())
        .send()
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            BinanceError::Venue {
                status: 400,
                code: -1121,
                ..
            }
        ),
        "{err:?}"
    );
}

async fn raw(path: &str) -> String {
    reqwest::Client::builder()
        .gzip(true)
        .build()
        .unwrap()
        .get(format!("https://fapi.binance.com{path}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .error_for_status()
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .text()
        .await
        .unwrap()
}

fn agrees<T: DeserializeOwned + Serialize>(path: &str, text: &str) -> Vec<String> {
    let diff = common::compare::<T>(path, text);
    // A field the type models but this response omits is not drift: it may
    // be sent only sometimes. A key the type does not model is.
    if !diff.invented.is_empty() {
        eprintln!(
            "{path}: modelled but not sent this time: {:?}",
            diff.invented
        );
    }
    diff.unmodelled
        .into_iter()
        .map(|key| format!("{path}: {key}"))
        .collect()
}

#[tokio::test]
#[ignore]
async fn live_responses_carry_no_unmodelled_keys() {
    let mut unmodelled = Vec::new();
    let mut check = |found: Vec<String>| unmodelled.extend(found);

    let path = "/fapi/v1/time";
    check(agrees::<ServerTime>(path, &raw(path).await));
    let path = "/fapi/v1/exchangeInfo";
    check(agrees::<ExchangeInfo>(path, &raw(path).await));
    let path = "/fapi/v1/fundingInfo";
    check(agrees::<Vec<FundingInfo>>(path, &raw(path).await));
    let path = "/fapi/v1/ticker/24hr";
    check(agrees::<Vec<Ticker24h>>(path, &raw(path).await));
    let path = "/fapi/v1/premiumIndex";
    check(agrees::<Vec<PremiumIndex>>(path, &raw(path).await));
    let path = "/fapi/v1/klines?symbol=BTCUSDT&interval=1m&limit=3";
    check(agrees::<Vec<Kline>>(path, &raw(path).await));
    let path = "/fapi/v1/fundingRate?limit=100";
    check(agrees::<Vec<FundingRate>>(path, &raw(path).await));
    let path = "/fapi/v1/openInterest?symbol=BTCUSDT";
    check(agrees::<OpenInterest>(path, &raw(path).await));
    let path = "/fapi/v1/aggTrades?symbol=BTCUSDT&limit=10";
    check(agrees::<Vec<AggTrade>>(path, &raw(path).await));
    let path = "/fapi/v1/depth?symbol=BTCUSDT&limit=5";
    check(agrees::<Depth>(path, &raw(path).await));

    assert!(
        unmodelled.is_empty(),
        "the host sent keys the types do not model; add them and record them in \
         docs/specs/binance/OBSERVED.md: {unmodelled:#?}"
    );
}
