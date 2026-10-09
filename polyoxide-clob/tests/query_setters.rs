//! Every query setter, called with a typed value, sends its key and value in
//! the order it was called: the golden test that holds the setters in place
//! while they move onto `polyoxide_core::query_setters!` (Story 3.8). A
//! changed name or argument type fails to compile; a changed key, value or
//! order fails the test.

use std::{future::Future, pin::Pin};

use polyoxide_clob::{
    Account, Clob, ClobBuilder, Credentials, MultiMarketOrderBy, SortPosition,
    UserRewardMarketOrderBy,
};

fn public(base: &str) -> Clob {
    ClobBuilder::new().base_url(base).build().unwrap()
}

/// Hardhat account #0, for the builders behind L2 auth.
fn authed(base: &str) -> Clob {
    let creds = Credentials {
        key: "test-key".into(),
        secret: "c2VjcmV0".into(),
        passphrase: "test-pass".into(),
    };
    let account = Account::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        creds,
    )
    .unwrap();
    ClobBuilder::new()
        .base_url(base)
        .with_account(account)
        .build()
        .unwrap()
}

type Fire = fn(String) -> Pin<Box<dyn Future<Output = ()> + Send>>;

/// A builder, the path it sends to, a call of every setter it has, and the
/// pairs that call sends.
struct Case {
    builder: &'static str,
    path: &'static str,
    fire: Fire,
    sends: &'static [(&'static str, &'static str)],
}

const CASES: &[Case] = &[
    Case {
        builder: "ListClobTrades",
        path: "/data/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .account_api()
                    .unwrap()
                    .trades("0xmaker")
                    .id("id-v")
                    .market("market-v")
                    .asset_id("asset_id-v")
                    .before("before-v")
                    .after("after-v")
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("maker_address", "0xmaker"),
            ("id", "id-v"),
            ("market", "market-v"),
            ("asset_id", "asset_id-v"),
            ("before", "before-v"),
            ("after", "after-v"),
            ("next_cursor", "next_cursor-v"),
        ],
    },
    Case {
        builder: "ListBuilderTrades",
        path: "/builder/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .account_api()
                    .unwrap()
                    .builder_trades("0xcode")
                    .after("after-v")
                    .maker_address("maker_address-v")
                    .market("market-v")
                    .id("id-v")
                    .asset_id("asset_id-v")
                    .before("before-v")
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("builder_code", "0xcode"),
            ("after", "after-v"),
            ("maker_address", "maker_address-v"),
            ("market", "market-v"),
            ("id", "id-v"),
            ("asset_id", "asset_id-v"),
            ("before", "before-v"),
            ("next_cursor", "next_cursor-v"),
        ],
    },
    Case {
        builder: "ListClobMarkets",
        path: "/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = public(&base)
                    .markets()
                    .list()
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[("next_cursor", "next_cursor-v")],
    },
    Case {
        builder: "ListOrders",
        path: "/data/orders",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .orders()
                    .unwrap()
                    .list()
                    .id("id-v")
                    .market("market-v")
                    .asset_id("asset_id-v")
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("id", "id-v"),
            ("market", "market-v"),
            ("asset_id", "asset_id-v"),
            ("next_cursor", "next_cursor-v"),
        ],
    },
    Case {
        builder: "UserEarningsRequest",
        path: "/rewards/user",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .rewards()
                    .unwrap()
                    .earnings("2026-01-02")
                    .maker_address("maker_address-v")
                    .sponsored(true)
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("date", "2026-01-02"),
            ("signature_type", "0"),
            ("maker_address", "maker_address-v"),
            ("sponsored", "true"),
            ("next_cursor", "next_cursor-v"),
        ],
    },
    Case {
        builder: "UserTotalEarningsRequest",
        path: "/rewards/user/total",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .rewards()
                    .unwrap()
                    .total_earnings("2026-01-02")
                    .maker_address("maker_address-v")
                    .sponsored(true)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("date", "2026-01-02"),
            ("signature_type", "0"),
            ("maker_address", "maker_address-v"),
            ("sponsored", "true"),
        ],
    },
    Case {
        builder: "UserPercentagesRequest",
        path: "/rewards/user/percentages",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .rewards()
                    .unwrap()
                    .percentages()
                    .maker_address("maker_address-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("signature_type", "0"),
            ("maker_address", "maker_address-v"),
        ],
    },
    Case {
        builder: "ListRewardMarkets",
        path: "/rewards/markets/current",
        fire: |base| {
            Box::pin(async move {
                let _ = public(&base)
                    .public_rewards()
                    .current_markets()
                    .sponsored(true)
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[("sponsored", "true"), ("next_cursor", "next_cursor-v")],
    },
    Case {
        builder: "RewardMarketRequest",
        path: "/rewards/markets/0xcond",
        fire: |base| {
            Box::pin(async move {
                let _ = public(&base)
                    .public_rewards()
                    .market("0xcond")
                    .sponsored(true)
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[("sponsored", "true"), ("next_cursor", "next_cursor-v")],
    },
    Case {
        builder: "ListMultiRewardMarkets",
        path: "/rewards/markets/multi",
        fire: |base| {
            Box::pin(async move {
                let _ = public(&base)
                    .public_rewards()
                    .multi_markets()
                    .query_text("query_text-v")
                    .tag_slug("tag_slug-v")
                    .event_id("event_id-v")
                    .event_title("event_title-v")
                    .order_by(MultiMarketOrderBy::Volume24hr)
                    .position(SortPosition::Desc)
                    .min_volume_24hr(1.5f64)
                    .max_volume_24hr(1.5f64)
                    .min_spread(1.5f64)
                    .max_spread(1.5f64)
                    .min_price(1.5f64)
                    .max_price(1.5f64)
                    .page_size(5u32)
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("q", "query_text-v"),
            ("tag_slug", "tag_slug-v"),
            ("event_id", "event_id-v"),
            ("event_title", "event_title-v"),
            ("order_by", "volume_24hr"),
            ("position", "DESC"),
            ("min_volume_24hr", "1.5"),
            ("max_volume_24hr", "1.5"),
            ("min_spread", "1.5"),
            ("max_spread", "1.5"),
            ("min_price", "1.5"),
            ("max_price", "1.5"),
            ("page_size", "5"),
            ("next_cursor", "next_cursor-v"),
        ],
    },
    Case {
        builder: "ListUserRewardMarkets",
        path: "/rewards/user/markets",
        fire: |base| {
            Box::pin(async move {
                let _ = authed(&base)
                    .rewards()
                    .unwrap()
                    .market_earnings()
                    .date("date-v")
                    .maker_address("maker_address-v")
                    .sponsored(true)
                    .query_text("query_text-v")
                    .tag_slug("tag_slug-v")
                    .favorite_markets(true)
                    .no_competition(true)
                    .only_mergeable(true)
                    .only_open_orders(true)
                    .only_open_positions(true)
                    .order_by(UserRewardMarketOrderBy::Earnings)
                    .position(SortPosition::Desc)
                    .page_size(5u32)
                    .next_cursor("next_cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("signature_type", "0"),
            ("date", "date-v"),
            ("maker_address", "maker_address-v"),
            ("sponsored", "true"),
            ("q", "query_text-v"),
            ("tag_slug", "tag_slug-v"),
            ("favorite_markets", "true"),
            ("no_competition", "true"),
            ("only_mergeable", "true"),
            ("only_open_orders", "true"),
            ("only_open_positions", "true"),
            ("order_by", "earnings"),
            ("position", "DESC"),
            ("page_size", "5"),
            ("next_cursor", "next_cursor-v"),
        ],
    },
];

#[tokio::test]
async fn every_setter_sends_its_key_and_value() {
    for case in CASES {
        let pairs = polyoxide_test_support::query::pairs_sent(case.path, case.fire).await;
        let sent: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(sent, case.sends, "{} on {}", case.builder, case.path);
    }
}
