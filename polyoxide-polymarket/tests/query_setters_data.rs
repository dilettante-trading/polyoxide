//! Every query setter, called with a typed value, sends its key and value in
//! the order it was called: the golden test that holds the setters in place
//! while they move onto `polyoxide_core::query_setters!` (Story 3.8). A
//! changed name or argument type fails to compile; a changed key, value or
//! order fails the test.

use polyoxide_polymarket::data::{types, DataApi};
use polyoxide_test_support::query::{assert_cases, Case};

fn client(base: &str) -> DataApi {
    DataApi::builder().base_url(base).build().unwrap()
}

const CASES: &[Case] = &[
    Case {
        builder: "ListBiggestWinners",
        path: "/v2/biggest-winners",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .biggest_winners()
                    .time_period(types::TimePeriod::Month)
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
        builder: "ListBuildersLeaderboard",
        path: "/v2/builders/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .builders_leaderboard()
                    .time_period(types::TimePeriod::Month)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("time_period", "month"), ("limit", "5")],
    },
    Case {
        builder: "GetBuilderVolume",
        path: "/v2/builders/volume",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .builder_volume()
                    .interval(types::TimePeriod::Month)
                    .limit(5u32)
                    .send()
                    .await;
            })
        },
        sends: &[("interval", "month"), ("limit", "5")],
    },
    Case {
        builder: "ListLeaderboard",
        path: "/v2/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .leaderboard()
                    .time_period(types::TimePeriod::Month)
                    .category("category-v")
                    .board(types::LeaderboardBoard::Volume)
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
        builder: "GetLeaderboardUser",
        path: "/v2/leaderboard",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .leaderboard_user("0xuser")
                    .time_period(types::TimePeriod::Month)
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
        builder: "ListTrades",
        path: "/v2/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .trades()
                    .user("user-v")
                    .conditions(["conditions-1", "conditions-2"])
                    .event_ids(["event_ids-1", "event_ids-2"])
                    .side(types::TradeSide::Buy)
                    .taker_only(true)
                    .filter_type(types::FilterType::Cash)
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
        builder: "ListActivity",
        path: "/v2/activity",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .activity("0xuser")
                    .types([types::ActivityType::Trade, types::ActivityType::Split])
                    .conditions(["conditions-1", "conditions-2"])
                    .event_ids(["event_ids-1", "event_ids-2"])
                    .side(types::TradeSide::Buy)
                    .start(-7i64)
                    .end(-7i64)
                    .sort_by(types::ActivitySortBy::Timestamp)
                    .sort_direction(types::SortDirection::Desc)
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
        builder: "ListComboActivity",
        path: "/v2/activity/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
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
        builder: "ListHolders",
        path: "/v2/holders",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
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
        builder: "GetOpenInterest",
        path: "/v2/oi",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .open_interest()
                    .conditions(["conditions-1", "conditions-2"])
                    .send()
                    .await;
            })
        },
        sends: &[("condition", "conditions-1,conditions-2")],
    },
    Case {
        builder: "ListPricesHistory",
        path: "/v2/prices-history",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .prices_history("123")
                    .start(-7i64)
                    .end(-7i64)
                    .interval(types::PricesInterval::OneWeek)
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
        builder: "ListPositions",
        path: "/v2/positions",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .positions(types::PositionAnchor::User("0xuser".into()))
                    .status(types::PositionStatus::Redeemable)
                    .event_ids(["event_ids-1", "event_ids-2"])
                    .title("title-v")
                    .filter_type(types::FilterType::Cash)
                    .filter_amount(1.5f64)
                    .include_archived(true)
                    .sort_by(types::PositionSortBy::TotalPnl)
                    .sort_direction(types::SortDirection::Desc)
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
        builder: "ListComboPositions",
        path: "/v2/positions/combos",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .combo_positions("0xuser")
                    .conditions(["conditions-1", "conditions-2"])
                    .statuses([
                        types::ComboPositionStatus::Open,
                        types::ComboPositionStatus::ResolvedWin,
                    ])
                    .sort_by(types::ComboPositionSortBy::EntryCost)
                    .sort_direction(types::SortDirection::Desc)
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
        builder: "GetUserPnl",
        path: "/v2/user-pnl",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
                    .user_pnl("0xuser")
                    .interval(types::PnlInterval::OneMonth)
                    .fidelity(types::PnlFidelity::ThreeHours)
                    .send()
                    .await;
            })
        },
        sends: &[("user", "0xuser"), ("interval", "1m"), ("fidelity", "3h")],
    },
    Case {
        builder: "GetUserVolume",
        path: "/v2/user-volume",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
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
        builder: "GetValue",
        path: "/v2/value",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
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
        builder: "ListTrades",
        path: "/v2/trades",
        fire: |base| {
            Box::pin(async move {
                let _ = client(&base)
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
    assert_cases(CASES).await;
}

#[tokio::test]
async fn a_csv_setter_omits_an_empty_value() {
    // Story 3.8 [RISK]: the comma-joined setters follow `polyoxide_core::csv`,
    // which omits a parameter whose joined value is empty. Given exactly
    // `[""]`, v1's sent `market=` before.
    let pairs = polyoxide_test_support::query::pairs_sent("/v2/trades", |base| async move {
        let _ = client(&base)
            .trades()
            .limit(5u32)
            .conditions([""])
            .event_ids([""])
            .send()
            .await;
    })
    .await;
    assert_eq!(pairs, [("limit".to_owned(), "5".to_owned())]);
}
