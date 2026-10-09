//! Every query setter, called with a typed value, sends its key and value in
//! the order it was called: the golden test that holds the setters in place
//! while they move onto `polyoxide_core::query_setters!` (Story 3.8). A
//! changed name or argument type fails to compile; a changed key, value or
//! order fails the test.

use std::{future::Future, pin::Pin};

use polyoxide_data::{
    api::leaderboard::{LeaderboardCategory, LeaderboardOrderBy},
    types as v1,
    v2::types as v2,
    DataApi,
};

fn client(base: &str) -> DataApi {
    DataApi::builder()
        .base_url(base)
        .pnl_base_url(base)
        .rankings_base_url(base)
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
        builder: "v1 GetBuilderLeaderboard",
        path: "/v1/builders/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .builders()
                    .leaderboard()
                    .time_period(v1::TimePeriod::Week)
                    .limit(5u32)
                    .offset(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("timePeriod", "WEEK"), ("limit", "5"), ("offset", "5")],
    },
    Case {
        builder: "v1 GetBuilderVolume",
        path: "/v1/builders/volume",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .builders()
                    .volume()
                    .time_period(v1::TimePeriod::Week)
                    .send()
                    .await;
            })
        },
        sends: &[("timePeriod", "WEEK")],
    },
    Case {
        builder: "v1 ListComboPositions",
        path: "/v1/positions/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .combos()
                    .positions("0xuser")
                    .status([
                        v1::ComboStatus::Open,
                        v1::ComboStatus::Unknown,
                        v1::ComboStatus::ResolvedWin,
                    ])
                    .sort(v1::ComboSort::EntryCostDesc)
                    .market_id(["market_id-1", "market_id-2"])
                    .limit(5u32)
                    .offset(5u32)
                    .updated_after(-7i64)
                    .updated_before(-7i64)
                    .cursor("cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("status", "OPEN,RESOLVED_WIN"),
            ("sort", "entry_cost_desc"),
            ("market_id", "market_id-1,market_id-2"),
            ("limit", "5"),
            ("offset", "5"),
            ("updatedAfter", "-7"),
            ("updatedBefore", "-7"),
            ("cursor", "cursor-v"),
        ],
    },
    Case {
        builder: "v1 ListComboActivity",
        path: "/v1/activity/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .combos()
                    .activity("0xuser")
                    .market_id(["market_id-1", "market_id-2"])
                    .limit(5u32)
                    .offset(5u32)
                    .cursor("cursor-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("market_id", "market_id-1,market_id-2"),
            ("limit", "5"),
            ("offset", "5"),
            ("cursor", "cursor-v"),
        ],
    },
    Case {
        builder: "v1 ListHolders",
        path: "/holders",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .holders()
                    .list(["0xcond"])
                    .limit(5u32)
                    .min_balance(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("market", "0xcond"), ("limit", "5"), ("minBalance", "5")],
    },
    Case {
        builder: "v1 GetLeaderboard",
        path: "/v1/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .leaderboard()
                    .get()
                    .category(LeaderboardCategory::Politics)
                    .time_period(v1::TimePeriod::Week)
                    .order_by(LeaderboardOrderBy::Vol)
                    .limit(5u32)
                    .offset(5u32)
                    .user("user-v")
                    .user_name("user_name-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("category", "POLITICS"),
            ("timePeriod", "WEEK"),
            ("orderBy", "VOL"),
            ("limit", "5"),
            ("offset", "5"),
            ("user", "user-v"),
            ("userName", "user_name-v"),
        ],
    },
    Case {
        builder: "v1 ListMarketPositions",
        path: "/v1/market-positions",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .market_positions()
                    .list("0xcond")
                    .user("user-v")
                    .status(v1::MarketPositionStatus::Closed)
                    .sort_by(v1::MarketPositionSortBy::RealizedPnl)
                    .sort_direction(v1::SortDirection::Asc)
                    .limit(5u32)
                    .offset(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("market", "0xcond"),
            ("user", "user-v"),
            ("status", "CLOSED"),
            ("sortBy", "REALIZED_PNL"),
            ("sortDirection", "ASC"),
            ("limit", "5"),
            ("offset", "5"),
        ],
    },
    Case {
        builder: "v1 ListRevisions",
        path: "/revisions",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .misc()
                    .revisions("0xq")
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("questionID", "0xq"), ("limit", "5")],
    },
    Case {
        builder: "v1 GetOpenInterest",
        path: "/oi",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .open_interest()
                    .get()
                    .market(["market-1", "market-2"])
                    .send()
                    .await;
            })
        },
        sends: &[("market", "market-1,market-2")],
    },
    Case {
        builder: "v1 UserPnlRequest",
        path: "/user-pnl",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .pnl()
                    .history("0xuser")
                    .interval("interval-v")
                    .fidelity(v1::PnlFidelity::EighteenHours)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user_address", "0xuser"),
            ("interval", "interval-v"),
            ("fidelity", "18h"),
        ],
    },
    Case {
        builder: "v1 RankingRequest",
        path: "/volume",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .rankings()
                    .volume()
                    .window(v1::RankingWindow::OneDay)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("window", "1d"), ("limit", "5")],
    },
    Case {
        builder: "v1 ListTrades",
        path: "/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .trades()
                    .list()
                    .user("user-v")
                    .market(["market-1", "market-2"])
                    .event_id(["event_id-1", "event_id-2"])
                    .side(v1::TradeSide::Sell)
                    .taker_only(true)
                    .filter_type(v1::TradeFilterType::Tokens)
                    .filter_amount(1.5f64)
                    .limit(5u32)
                    .offset(5u32)
                    .start(6u64)
                    .end(6u64)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "user-v"),
            ("market", "market-1,market-2"),
            ("eventId", "event_id-1,event_id-2"),
            ("side", "SELL"),
            ("takerOnly", "true"),
            ("filterType", "TOKENS"),
            ("filterAmount", "1.5"),
            ("limit", "5"),
            ("offset", "5"),
            ("start", "6"),
            ("end", "6"),
        ],
    },
    Case {
        builder: "v1 ListPositions",
        path: "/positions",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user("0xuser")
                    .list_positions()
                    .market(["market-1", "market-2"])
                    .event_id(["event_id-1", "event_id-2"])
                    .size_threshold(1.5f64)
                    .redeemable(true)
                    .mergeable(true)
                    .limit(5u32)
                    .offset(5u32)
                    .sort_by(v1::PositionSortBy::CashPnl)
                    .sort_direction(v1::SortDirection::Asc)
                    .title("title-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("market", "market-1,market-2"),
            ("eventId", "event_id-1,event_id-2"),
            ("sizeThreshold", "1.5"),
            ("redeemable", "true"),
            ("mergeable", "true"),
            ("limit", "5"),
            ("offset", "5"),
            ("sortBy", "CASH_PNL"),
            ("sortDirection", "ASC"),
            ("title", "title-v"),
        ],
    },
    Case {
        builder: "v1 GetPositionValue",
        path: "/value",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user("0xuser")
                    .positions_value()
                    .market(["market-1", "market-2"])
                    .send()
                    .await;
            })
        },
        sends: &[("user", "0xuser"), ("market", "market-1,market-2")],
    },
    Case {
        builder: "v1 ListClosedPositions",
        path: "/closed-positions",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user("0xuser")
                    .closed_positions()
                    .market(["market-1", "market-2"])
                    .event_id(["event_id-1", "event_id-2"])
                    .title("title-v")
                    .limit(5u32)
                    .offset(5u32)
                    .sort_by(v1::ClosedPositionSortBy::AvgPrice)
                    .sort_direction(v1::SortDirection::Asc)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("market", "market-1,market-2"),
            ("eventId", "event_id-1,event_id-2"),
            ("title", "title-v"),
            ("limit", "5"),
            ("offset", "5"),
            ("sortBy", "AVG_PRICE"),
            ("sortDirection", "ASC"),
        ],
    },
    Case {
        builder: "v1 ListUserTrades",
        path: "/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user("0xuser")
                    .trades()
                    .market(["market-1", "market-2"])
                    .event_id(["event_id-1", "event_id-2"])
                    .side(v1::TradeSide::Sell)
                    .taker_only(true)
                    .filter_type(v1::TradeFilterType::Tokens)
                    .filter_amount(1.5f64)
                    .limit(5u32)
                    .offset(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("market", "market-1,market-2"),
            ("eventId", "event_id-1,event_id-2"),
            ("side", "SELL"),
            ("takerOnly", "true"),
            ("filterType", "TOKENS"),
            ("filterAmount", "1.5"),
            ("limit", "5"),
            ("offset", "5"),
        ],
    },
    Case {
        builder: "v1 ListActivity",
        path: "/activity",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user("0xuser")
                    .activity()
                    .market(["market-1", "market-2"])
                    .event_id(["event_id-1", "event_id-2"])
                    .activity_type([
                        v1::ActivityType::Trade,
                        v1::ActivityType::Unknown,
                        v1::ActivityType::Redeem,
                    ])
                    .exclude_deposits_withdrawals(true)
                    .side(v1::TradeSide::Sell)
                    .start(-7i64)
                    .end(-7i64)
                    .limit(5u32)
                    .offset(5u32)
                    .sort_by(v1::ActivitySortBy::Cash)
                    .sort_direction(v1::SortDirection::Asc)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("market", "market-1,market-2"),
            ("eventId", "event_id-1,event_id-2"),
            ("type", "TRADE,REDEEM"),
            ("excludeDepositsWithdrawals", "true"),
            ("side", "SELL"),
            ("start", "-7"),
            ("end", "-7"),
            ("limit", "5"),
            ("offset", "5"),
            ("sortBy", "CASH"),
            ("sortDirection", "ASC"),
        ],
    },
    Case {
        builder: "v2 ListBiggestWinners",
        path: "/v2/biggest-winners",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .biggest_winners()
                    .time_period(v2::TimePeriod::Month)
                    .category("category-v")
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("time_period", "month"),
            ("category", "category-v"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 ListBuildersLeaderboard",
        path: "/v2/builders/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .builders_leaderboard()
                    .time_period(v2::TimePeriod::Month)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("time_period", "month"), ("limit", "5")],
    },
    Case {
        builder: "v2 GetBuilderVolume",
        path: "/v2/builders/volume",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .builder_volume()
                    .interval(v2::TimePeriod::Month)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("interval", "month"), ("limit", "5")],
    },
    Case {
        builder: "v2 ListLeaderboard",
        path: "/v2/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .leaderboard()
                    .time_period(v2::TimePeriod::Month)
                    .category("category-v")
                    .board(v2::LeaderboardBoard::Volume)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("time_period", "month"),
            ("category", "category-v"),
            ("sort_by", "VOLUME"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 GetLeaderboardUser",
        path: "/v2/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .leaderboard_user("0xuser")
                    .time_period(v2::TimePeriod::Month)
                    .category("category-v")
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("time_period", "month"),
            ("category", "category-v"),
        ],
    },
    Case {
        builder: "v2 ListTrades",
        path: "/v2/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .trades()
                    .user("user-v")
                    .conditions(["conditions-1", "conditions-2"])
                    .event_ids(["event_ids-1", "event_ids-2"])
                    .side(v2::TradeSide::Buy)
                    .taker_only(true)
                    .filter_type(v2::FilterType::Cash)
                    .filter_amount(1.5f64)
                    .start(-7i64)
                    .end(-7i64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "user-v"),
            ("condition", "conditions-1,conditions-2"),
            ("event_id", "event_ids-1,event_ids-2"),
            ("side", "BUY"),
            ("taker_only", "true"),
            ("filter_type", "CASH"),
            ("filter_amount", "1.5"),
            ("start", "-7"),
            ("end", "-7"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 ListActivity",
        path: "/v2/activity",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .activity("0xuser")
                    .types([v2::ActivityType::Trade, v2::ActivityType::Split])
                    .conditions(["conditions-1", "conditions-2"])
                    .event_ids(["event_ids-1", "event_ids-2"])
                    .side(v2::TradeSide::Buy)
                    .start(-7i64)
                    .end(-7i64)
                    .sort_by(v2::ActivitySortBy::Timestamp)
                    .sort_direction(v1::SortDirection::Desc)
                    .exclude_deposits_withdrawals(true)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("type", "TRADE,SPLIT"),
            ("condition", "conditions-1,conditions-2"),
            ("event_id", "event_ids-1,event_ids-2"),
            ("side", "BUY"),
            ("start", "-7"),
            ("end", "-7"),
            ("sort_by", "TIMESTAMP"),
            ("sort_direction", "DESC"),
            ("exclude_deposits_withdrawals", "true"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 ListComboActivity",
        path: "/v2/activity/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .combo_activity("0xuser")
                    .conditions(["conditions-1", "conditions-2"])
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("condition", "conditions-1,conditions-2"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 ListHolders",
        path: "/v2/holders",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .holders(["0xcond"])
                    .min_balance(1.5f64)
                    .include_pnl(true)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("condition", "0xcond"),
            ("min_balance", "1.5"),
            ("include_pnl", "true"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 GetOpenInterest",
        path: "/v2/oi",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .open_interest()
                    .conditions(["conditions-1", "conditions-2"])
                    .send()
                    .await;
            })
        },
        sends: &[("condition", "conditions-1,conditions-2")],
    },
    Case {
        builder: "v2 ListPricesHistory",
        path: "/v2/prices-history",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .prices_history("123")
                    .start(-7i64)
                    .end(-7i64)
                    .interval(v2::PricesInterval::OneWeek)
                    .bucket_seconds(5u32)
                    .as_of(-7i64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("token_id", "123"),
            ("start", "-7"),
            ("end", "-7"),
            ("interval", "1w"),
            ("bucket_seconds", "5"),
            ("as_of", "-7"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 ListPositions",
        path: "/v2/positions",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .positions(v2::PositionAnchor::User("0xuser".into()))
                    .status(v2::PositionStatus::Redeemable)
                    .event_ids(["event_ids-1", "event_ids-2"])
                    .title("title-v")
                    .filter_type(v2::FilterType::Cash)
                    .filter_amount(1.5f64)
                    .include_archived(true)
                    .sort_by(v2::PositionSortBy::TotalPnl)
                    .sort_direction(v1::SortDirection::Desc)
                    .start(-7i64)
                    .end(-7i64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("status", "REDEEMABLE"),
            ("event_id", "event_ids-1,event_ids-2"),
            ("title", "title-v"),
            ("filter_type", "CASH"),
            ("filter_amount", "1.5"),
            ("include_archived", "true"),
            ("sort_by", "TOTAL_PNL"),
            ("sort_direction", "DESC"),
            ("start", "-7"),
            ("end", "-7"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 ListComboPositions",
        path: "/v2/positions/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .combo_positions("0xuser")
                    .conditions(["conditions-1", "conditions-2"])
                    .statuses([
                        v2::ComboPositionStatus::Open,
                        v2::ComboPositionStatus::ResolvedWin,
                    ])
                    .sort_by(v2::ComboPositionSortBy::EntryCost)
                    .sort_direction(v1::SortDirection::Desc)
                    .updated_after(-7i64)
                    .updated_before(-7i64)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("condition", "conditions-1,conditions-2"),
            ("status", "OPEN,RESOLVED_WIN"),
            ("sort_by", "ENTRY_COST"),
            ("sort_direction", "DESC"),
            ("updated_after", "-7"),
            ("updated_before", "-7"),
            ("limit", "5"),
        ],
    },
    Case {
        builder: "v2 GetUserPnl",
        path: "/v2/user-pnl",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .user_pnl("0xuser")
                    .interval(v2::PnlInterval::OneMonth)
                    .fidelity(v2::PnlFidelity::ThreeHours)
                    .send()
                    .await;
            })
        },
        sends: &[("user", "0xuser"), ("interval", "1m"), ("fidelity", "3h")],
    },
    Case {
        builder: "v2 GetUserVolume",
        path: "/v2/user-volume",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .user_volume("0xuser")
                    .start(-7i64)
                    .end(-7i64)
                    .send()
                    .await;
            })
        },
        sends: &[("user", "0xuser"), ("start", "-7"), ("end", "-7")],
    },
    Case {
        builder: "v2 GetValue",
        path: "/v2/value",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .value("0xuser")
                    .conditions(["conditions-1", "conditions-2"])
                    .send()
                    .await;
            })
        },
        sends: &[
            ("user", "0xuser"),
            ("condition", "conditions-1,conditions-2"),
        ],
    },
    // a list of unknown activity types sends nothing.
    Case {
        builder: "v1 ListActivity",
        path: "/activity",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user("0xuser")
                    .activity()
                    .activity_type([v1::ActivityType::Unknown])
                    .send()
                    .await;
            })
        },
        sends: &[("user", "0xuser")],
    },
    // a list of unknown combo statuses sends nothing.
    Case {
        builder: "v1 ListComboPositions",
        path: "/v1/positions/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .combos()
                    .positions("0xuser")
                    .status([v1::ComboStatus::Unknown])
                    .send()
                    .await;
            })
        },
        sends: &[("user", "0xuser")],
    },
    // an empty csv list sends nothing.
    Case {
        builder: "v2 ListTrades",
        path: "/v2/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .v2()
                    .trades()
                    .conditions(Vec::<String>::new())
                    .send()
                    .await;
            })
        },
        sends: &[],
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
