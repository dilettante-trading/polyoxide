//! Every query setter, called with a typed value, sends its key and value in the order it was
//! called: the golden test that holds the setters in place. A changed name or argument type
//! fails to compile; a changed key, value or order fails the test.

use polyoxide_kalshi::predictions::{
    key::Ticker,
    types::{MarketStatusFilter, MveFilter, PeriodInterval},
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

/// The candlestick builders: the required arguments first, in the order the route documents
/// them, then `include_latest_before_start`. Each width sends its number of minutes.
const CANDLE_CASES: &[Case] = &[
    Case {
        builder: "GetCandlesticks",
        path: "/trade-api/v2/markets/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .candlesticks(&ticker(), PeriodInterval::Hour1, 100, 200)
                    .include_latest_before_start(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market_tickers", "KXTEST-1"),
            ("start_ts", "100"),
            ("end_ts", "200"),
            ("period_interval", "60"),
            ("include_latest_before_start", "true"),
        ],
    },
    Case {
        builder: "GetCandlesticks, one minute",
        path: "/trade-api/v2/markets/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .candlesticks(&ticker(), PeriodInterval::Minute1, 1, 2)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market_tickers", "KXTEST-1"),
            ("start_ts", "1"),
            ("end_ts", "2"),
            ("period_interval", "1"),
        ],
    },
    Case {
        builder: "GetCandlesticks, one day, a setter called twice",
        path: "/trade-api/v2/markets/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .candlesticks(&ticker(), PeriodInterval::Day1, 3, 4)
                    .include_latest_before_start(true)
                    .include_latest_before_start(false)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market_tickers", "KXTEST-1"),
            ("start_ts", "3"),
            ("end_ts", "4"),
            ("period_interval", "1440"),
            ("include_latest_before_start", "false"),
        ],
    },
    Case {
        builder: "GetSeriesCandlesticks",
        path: "/trade-api/v2/series/KXSER-1/markets/KXTEST-1/candlesticks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .series_candlesticks("KXSER-1", &ticker(), PeriodInterval::Hour1, 100, 200)
                    .include_latest_before_start(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("start_ts", "100"),
            ("end_ts", "200"),
            ("period_interval", "60"),
            ("include_latest_before_start", "true"),
        ],
    },
];

#[tokio::test]
async fn every_candle_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(CANDLE_CASES).await;
}

// ── The markets catalog builders (issue #71) ────────────────────

use polyoxide_kalshi::predictions::types::{CreatedStatus, MarketWindow};

fn second() -> Ticker {
    Ticker::new("KXTEST-2").unwrap()
}

/// A window's state and bounds come first, in that order, then the setters in call order;
/// `updated` sends its bound and `mve_filter=exclude` first; `orderbooks` one pair per ticker.
const MARKETS_CATALOG_CASES: &[Case] = &[
    Case {
        builder: "ListMarkets, tickers",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .list()
                    .tickers(&[ticker(), second()])
                    .limit(5)
                    .send()
                    .await;
            })
        },
        sends: &[("tickers", "KXTEST-1,KXTEST-2"), ("limit", "5")],
    },
    Case {
        builder: "ListMarketWindow, created",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .window(MarketWindow::Created {
                        status: Some(CreatedStatus::Unopened),
                        min_ts: Some(1),
                        max_ts: Some(2),
                    })
                    .event_ticker("EV-1")
                    .series_ticker("SER-1")
                    .mve_filter(MveFilter::Exclude)
                    .tickers(&[ticker()])
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "unopened"),
            ("min_created_ts", "1"),
            ("max_created_ts", "2"),
            ("event_ticker", "EV-1"),
            ("series_ticker", "SER-1"),
            ("mve_filter", "exclude"),
            ("tickers", "KXTEST-1"),
            ("limit", "5"),
            ("cursor", "c1"),
        ],
    },
    Case {
        builder: "ListMarketWindow, closed",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .window(MarketWindow::Closed {
                        closed_only: true,
                        min_ts: Some(3),
                        max_ts: Some(4),
                    })
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "closed"),
            ("min_close_ts", "3"),
            ("max_close_ts", "4"),
        ],
    },
    Case {
        builder: "ListMarketWindow, settled",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .window(MarketWindow::Settled {
                        settled_only: true,
                        min_ts: Some(5),
                        max_ts: Some(6),
                    })
                    .send()
                    .await;
            })
        },
        sends: &[
            ("status", "settled"),
            ("min_settled_ts", "5"),
            ("max_settled_ts", "6"),
        ],
    },
    Case {
        builder: "ListUpdatedMarkets",
        path: "/trade-api/v2/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .updated(7)
                    .max_ts(8)
                    .series_ticker("SER-1")
                    .limit(5)
                    .cursor("c1")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("min_updated_ts", "7"),
            ("mve_filter", "exclude"),
            ("max_updated_ts", "8"),
            ("series_ticker", "SER-1"),
            ("limit", "5"),
            ("cursor", "c1"),
        ],
    },
    Case {
        builder: "GetOrderbooks",
        path: "/trade-api/v2/markets/orderbooks",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .markets()
                    .orderbooks(&[ticker(), second()])
                    .send()
                    .await;
            })
        },
        sends: &[("tickers", "KXTEST-1"), ("tickers", "KXTEST-2")],
    },
];

#[tokio::test]
async fn every_markets_catalog_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(MARKETS_CATALOG_CASES).await;
}

// ── The series builders (issue #71) ─────────────────────────────

const SERIES_CASES: &[Case] = &[
    Case {
        builder: "ListSeries",
        path: "/trade-api/v2/series",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .series()
                    .list()
                    .category("Economics")
                    .tags("Inflation")
                    .include_product_metadata(true)
                    .include_volume(true)
                    .min_updated_ts(9)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("category", "Economics"),
            ("tags", "Inflation"),
            ("include_product_metadata", "true"),
            ("include_volume", "true"),
            ("min_updated_ts", "9"),
        ],
    },
    Case {
        builder: "GetSeries",
        path: "/trade-api/v2/series/KXSER-1",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .series()
                    .get("KXSER-1")
                    .include_volume(true)
                    .send()
                    .await;
            })
        },
        sends: &[("include_volume", "true")],
    },
];

#[tokio::test]
async fn every_series_setter_sends_its_key_and_value_in_call_order() {
    assert_cases(SERIES_CASES).await;
}
