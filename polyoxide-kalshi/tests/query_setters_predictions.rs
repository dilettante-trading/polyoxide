//! Every query setter, called with a typed value, sends its key and value in the order it was
//! called: the golden test that holds the setters in place. A changed name or argument type
//! fails to compile; a changed key, value or order fails the test.

use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketStatusFilter, MveFilter},
    Predictions,
};
use polyoxide_test_support::query::{assert_cases, Case};

fn client(base: &str) -> Predictions {
    Predictions::builder().base_url(base).build().unwrap()
}

fn ticker() -> Ticker {
    Ticker::new("KXTEST-1").unwrap()
}

const CASES: &[Case] = &[
    Case {
        builder: "ListMarkets",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list()
                    .status(MarketStatusFilter::Settled)
                    .event_ticker("EV-1")
                    .series_ticker("SER-1")
                    .mve_filter(MveFilter::Only)
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "settled"),
            ("event_ticker", "EV-1"),
            ("series_ticker", "SER-1"),
            ("mve_filter", "only"),
            ("limit", "5"),
            ("cursor", "c1"),
        ],
    },
    Case {
        builder: "ListMarkets, a setter called twice",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list()
                    .limit(1)
                    .limit(2)
                    .send()
                    .await;
            })
        },
        sends: &[("limit", "2")],
    },
    Case {
        builder: "GetMarket",
        path: "/trade-api/v2/markets/KXTEST-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).markets().get(&ticker()).send().await;
            })
        },
        sends: &[],
    },
    Case {
        builder: "GetOrderbook",
        path: "/trade-api/v2/markets/KXTEST-1/orderbook",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .orderbook(&ticker())
                    .depth(3)
                    .send()
                    .await;
            })
        },
        sends: &[("depth", "3")],
    },
    Case {
        builder: "ListTrades",
        path: "/trade-api/v2/markets/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .trades()
                    .ticker(&ticker())
                    .limit(5)
                    .cursor("c1")
                    .min_ts(6)
                    .max_ts(7)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("ticker", "KXTEST-1"),
            ("limit", "5"),
            ("cursor", "c1"),
            ("min_ts", "6"),
            ("max_ts", "7"),
        ],
    },
    Case {
        builder: "GetEvent",
        path: "/trade-api/v2/events/EV-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .events()
                    .get("EV-1")
                    .with_nested_markets(true)
                    .send()
                    .await;
            })
        },
        sends: &[("with_nested_markets", "true")],
    },
    Case {
        builder: "GetEndpointCosts",
        path: "/trade-api/v2/account/endpoint_costs",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base).account().endpoint_costs().send().await;
            })
        },
        sends: &[],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(CASES).await;
}
