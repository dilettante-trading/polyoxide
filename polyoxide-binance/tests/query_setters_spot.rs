//! Every spot query setter, called with a typed value, sends its key and value
//! in the order it was called. A changed name or argument type fails to
//! compile; a changed key, value or order fails the test.

use polyoxide_binance::spot::{
    types::{Interval, Symbol},
    Spot,
};
use polyoxide_test_support::query::{assert_cases, Case};

fn client(base: &str) -> Spot {
    Spot::builder().base_url(base).build().unwrap()
}

fn btc() -> Symbol {
    Symbol::new("BTCUSDT").unwrap()
}

const CASES: &[Case] = &[
    Case {
        builder: "GetTrades",
        path: "/api/v3/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .trades(&btc())
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "5")],
    },
    Case {
        builder: "GetAggTrades",
        path: "/api/v3/aggTrades",
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
        builder: "GetKlines",
        path: "/api/v3/klines",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .klines(&btc(), Interval::Minute1)
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
    // `GetDepth::limit` is hand-written; it sends the limit as given, past the
    // venue's maximum included.
    Case {
        builder: "GetDepth",
        path: "/api/v3/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(7u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "7")],
    },
    Case {
        builder: "GetDepth",
        path: "/api/v3/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(5001u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "5001")],
    },
    // a repeated parameter replaces the first.
    Case {
        builder: "GetKlines",
        path: "/api/v3/klines",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .klines(&btc(), Interval::Second1)
                    .limit(5u32)
                    .limit(9u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("interval", "1s"), ("limit", "9")],
    },
    Case {
        builder: "GetDepth",
        path: "/api/v3/depth",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market()
                    .depth(&btc())
                    .limit(5u32)
                    .limit(9u32)
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT"), ("limit", "9")],
    },
    // Story 4's `GetExchangeInfo::symbol`, a `query_setters!` setter, pinned
    // here beside the market setters.
    Case {
        builder: "GetExchangeInfo",
        path: "/api/v3/exchangeInfo",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .exchange()
                    .exchange_info()
                    .symbol(&btc())
                    .send()
                    .await;
            })
        },
        sends: &[("symbol", "BTCUSDT")],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value() {
    assert_cases(CASES).await;
}
