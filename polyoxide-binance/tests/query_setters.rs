//! Every query setter, called with a typed value, sends its key and value in
//! the order it was called: the golden test that holds the setters in place
//! while they move onto `polyoxide_core::query_setters!` (Story 3.8). A
//! changed name or argument type fails to compile; a changed key, value or
//! order fails the test.

use polyoxide_binance::{
    usdm::types::{DepthLimit, Interval, Symbol},
    Usdm,
};
use polyoxide_test_support::query::{assert_cases, Case};

fn client(base: &str) -> Usdm {
    Usdm::builder().base_url(base).build().unwrap()
}

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

const CASES: &[Case] = &[
    Case {
        builder: "GetKlines",
        path: "/fapi/v1/klines",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .klines(&btc(), Interval::M1)
                    .start_time(6u64)
                    .end_time(6u64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("symbol", "BTCUSDT"),
            ("interval", "1m"),
            ("startTime", "6"),
            ("endTime", "6"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "GetFundingRate",
        path: "/fapi/v1/fundingRate",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .funding_rate()
                    .symbol(&btc())
                    .start_time(6u64)
                    .end_time(6u64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("symbol", "BTCUSDT"),
            ("startTime", "6"),
            ("endTime", "6"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "GetAggTrades",
        path: "/fapi/v1/aggTrades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .agg_trades(&btc())
                    .from_id(6u64)
                    .start_time(6u64)
                    .end_time(6u64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("symbol", "BTCUSDT"),
            ("fromId", "6"),
            ("startTime", "6"),
            ("endTime", "6"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "GetDepth",
        path: "/fapi/v1/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(DepthLimit::Ten)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "10")],
    },
    // a repeated parameter replaces the first.
    Case {
        builder: "GetKlines",
        path: "/fapi/v1/klines",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .klines(&btc(), Interval::M1)
                    .limit(5u32)
                    .limit(9u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("interval", "1m"), ("limit", "9")],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value() {
    assert_cases(CASES).await;
}
