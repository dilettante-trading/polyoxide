use mockito::{Matcher, Server};
use polyoxide_data::types::{
    ActivityType, ComboLegStatus, ComboSort, ComboStatus, PnlFidelity, RankingWindow,
};
use polyoxide_data::{DataApi, DataApiError};

fn test_data(server: &mockito::ServerGuard) -> DataApi {
    DataApi::builder().base_url(server.url()).build().unwrap()
}

#[tokio::test]
async fn list_positions_with_query_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/positions")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc123".into()),
            Matcher::UrlEncoded("sizeThreshold".into(), "5".into()),
            Matcher::UrlEncoded("sortBy".into(), "CURRENT".into()),
            Matcher::UrlEncoded("sortDirection".into(), "ASC".into()),
            Matcher::UrlEncoded("limit".into(), "10".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "proxyWallet": "0xabc123",
                "asset": "token1",
                "conditionId": "cond1",
                "size": 100.5,
                "avgPrice": 0.65,
                "initialValue": 65.0,
                "currentValue": 70.0,
                "cashPnl": 5.0,
                "percentPnl": 7.69,
                "totalBought": 100.5,
                "realizedPnl": 2.0,
                "percentRealizedPnl": 3.08,
                "curPrice": 0.70,
                "redeemable": false,
                "mergeable": true,
                "title": "Will X happen?",
                "slug": "will-x-happen",
                "icon": "https://example.com/icon.png",
                "eventSlug": "x-event",
                "outcome": "Yes",
                "outcomeIndex": 0,
                "oppositeOutcome": "No",
                "oppositeAsset": "token2",
                "endDate": "2025-12-31",
                "negativeRisk": false
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let positions = data
        .user("0xabc123")
        .list_positions()
        .size_threshold(5.0)
        .sort_by(polyoxide_data::types::PositionSortBy::Current)
        .sort_direction(polyoxide_data::types::SortDirection::Asc)
        .limit(10)
        .send()
        .await
        .unwrap();

    assert_eq!(positions.len(), 1);
    let pos = &positions[0];
    assert_eq!(pos.proxy_wallet, "0xabc123");
    assert_eq!(pos.condition_id, "cond1");
    assert!((pos.avg_price - 0.65).abs() < f64::EPSILON);
    assert!((pos.current_value - 70.0).abs() < f64::EPSILON);
    assert!(!pos.redeemable);
    assert!(pos.mergeable);
    assert_eq!(pos.outcome, "Yes");
    assert_eq!(pos.outcome_index, 0);
    assert_eq!(pos.opposite_outcome, "No");
    assert!(!pos.negative_risk);
    mock.assert_async().await;
}

#[tokio::test]
async fn list_positions_redeemable_filter() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/positions")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xaddr".into()),
            Matcher::UrlEncoded("redeemable".into(), "true".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let data = test_data(&server);
    let positions = data
        .user("0xaddr")
        .list_positions()
        .redeemable(true)
        .send()
        .await
        .unwrap();

    assert!(positions.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn user_trades_with_side_filter() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/trades")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xaddr".into()),
            Matcher::UrlEncoded("side".into(), "BUY".into()),
            Matcher::UrlEncoded("limit".into(), "5".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "proxyWallet": "0xaddr",
                "side": "BUY",
                "asset": "token_buy",
                "conditionId": "cond1",
                "size": 50.0,
                "price": 0.72,
                "timestamp": 1700001000,
                "title": "Trade market?",
                "slug": "trade-market",
                "icon": null,
                "eventSlug": null,
                "outcome": "Yes",
                "outcomeIndex": 0,
                "name": "TraderOne",
                "pseudonym": null,
                "bio": null,
                "profileImage": null,
                "profileImageOptimized": null,
                "transactionHash": "0xhash123"
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let trades = data
        .user("0xaddr")
        .trades()
        .side(polyoxide_data::types::TradeSide::Buy)
        .limit(5)
        .send()
        .await
        .unwrap();

    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].side, polyoxide_data::types::TradeSide::Buy);
    assert_eq!(trades[0].proxy_wallet, "0xaddr");
    assert!((trades[0].price - 0.72).abs() < f64::EPSILON);
    assert_eq!(trades[0].transaction_hash.as_deref(), Some("0xhash123"));
    mock.assert_async().await;
}

#[tokio::test]
async fn user_activity_type_rename() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/activity")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xaddr".into()),
            Matcher::UrlEncoded("type".into(), "TRADE,MERGE".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "proxyWallet": "0xaddr",
                "timestamp": 1700003000,
                "conditionId": "cond_act",
                "type": "TRADE",
                "size": 10.0,
                "usdcSize": 7.50,
                "transactionHash": "0xacthash",
                "price": 0.75,
                "asset": "token_act",
                "side": "BUY",
                "outcomeIndex": 0,
                "title": "Activity market",
                "slug": "activity-market",
                "icon": null,
                "outcome": "Yes",
                "name": null,
                "pseudonym": null,
                "bio": null,
                "profileImage": null,
                "profileImageOptimized": null
            }, {
                "proxyWallet": "0xaddr",
                "timestamp": 1700004000,
                "conditionId": "cond_merge",
                "type": "MERGE",
                "size": 5.0,
                "usdcSize": 3.0,
                "transactionHash": null,
                "price": null,
                "asset": null,
                "side": "",
                "outcomeIndex": null,
                "title": null,
                "slug": null,
                "icon": null,
                "outcome": null,
                "name": null,
                "pseudonym": null,
                "bio": null,
                "profileImage": null,
                "profileImageOptimized": null
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let activities = data
        .user("0xaddr")
        .activity()
        .activity_type([
            polyoxide_data::types::ActivityType::Trade,
            polyoxide_data::types::ActivityType::Merge,
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(activities.len(), 2);
    // Validates #[serde(rename = "type")] -> activity_type
    assert_eq!(
        activities[0].activity_type,
        polyoxide_data::types::ActivityType::Trade
    );
    assert_eq!(
        activities[1].activity_type,
        polyoxide_data::types::ActivityType::Merge
    );
    // Empty string side from API stored as Some("")
    assert_eq!(activities[1].side, Some("".to_string()));
    mock.assert_async().await;
}

#[tokio::test]
async fn activity_type_filter_drops_unknown_variant() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/activity")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xaddr".into()),
            Matcher::UrlEncoded("type".into(), "TRADE".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let data = test_data(&server);
    let activities = data
        .user("0xaddr")
        .activity()
        .activity_type([
            polyoxide_data::types::ActivityType::Trade,
            polyoxide_data::types::ActivityType::Unknown,
        ])
        .send()
        .await
        .unwrap();

    assert!(activities.is_empty());
    // The mock only matches if "type" was exactly "TRADE" (no "UNKNOWN" appended).
    mock.assert_async().await;
}

#[tokio::test]
async fn activity_response_with_future_type_does_not_poison_page() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/activity")
        .match_query(Matcher::UrlEncoded("user".into(), "0xaddr".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "proxyWallet": "0xaddr",
                "timestamp": 1700003000,
                "conditionId": "cond_act",
                "type": "TRADE",
                "size": 10.0,
                "usdcSize": 7.50,
                "transactionHash": "0xacthash",
                "price": 0.75,
                "asset": "token_act",
                "side": "BUY",
                "outcomeIndex": 0,
                "title": "Activity market",
                "slug": "activity-market",
                "icon": null,
                "outcome": "Yes",
                "name": null,
                "pseudonym": null,
                "bio": null,
                "profileImage": null,
                "profileImageOptimized": null
            }, {
                "proxyWallet": "0xaddr",
                "timestamp": 1700004000,
                "conditionId": "cond_future",
                "type": "SOME_FUTURE_TYPE_UPSTREAM_ADDED",
                "size": 1.0,
                "usdcSize": 1.0,
                "transactionHash": null,
                "price": null,
                "asset": null,
                "side": "",
                "outcomeIndex": null,
                "title": null,
                "slug": null,
                "icon": null,
                "outcome": null,
                "name": null,
                "pseudonym": null,
                "bio": null,
                "profileImage": null,
                "profileImageOptimized": null
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let activities = data
        .user("0xaddr")
        .activity()
        .send()
        .await
        .expect("unrecognized activity type should not fail the whole page");

    assert_eq!(activities.len(), 2);
    assert_eq!(
        activities[0].activity_type,
        polyoxide_data::types::ActivityType::Trade
    );
    assert_eq!(
        activities[1].activity_type,
        polyoxide_data::types::ActivityType::Unknown
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn closed_positions_sort_and_pagination() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/closed-positions")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xaddr".into()),
            Matcher::UrlEncoded("sortBy".into(), "REALIZED_PNL".into()),
            Matcher::UrlEncoded("offset".into(), "10".into()),
            Matcher::UrlEncoded("limit".into(), "5".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "proxyWallet": "0xaddr",
                "asset": "token_closed",
                "conditionId": "cond_closed",
                "avgPrice": 0.45,
                "totalBought": 200.0,
                "realizedPnl": -10.0,
                "curPrice": 0.35,
                "timestamp": 1700000000,
                "title": "Closed market?",
                "slug": "closed-market",
                "icon": null,
                "eventSlug": "closed-event",
                "outcome": "No",
                "outcomeIndex": 1,
                "oppositeOutcome": "Yes",
                "oppositeAsset": "token_opp",
                "endDate": "2024-06-30"
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let closed = data
        .user("0xaddr")
        .closed_positions()
        .sort_by(polyoxide_data::types::ClosedPositionSortBy::RealizedPnl)
        .offset(10)
        .limit(5)
        .send()
        .await
        .unwrap();

    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].proxy_wallet, "0xaddr");
    assert!((closed[0].realized_pnl - (-10.0)).abs() < f64::EPSILON);
    assert_eq!(closed[0].timestamp, 1700000000);
    assert_eq!(closed[0].outcome_index, 1);
    assert_eq!(closed[0].end_date.as_deref(), Some("2024-06-30"));
    mock.assert_async().await;
}

#[tokio::test]
async fn trades_list_global() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/trades")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("market".into(), "cond1,cond2".into()),
            Matcher::UrlEncoded("side".into(), "SELL".into()),
            Matcher::UrlEncoded("takerOnly".into(), "false".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "proxyWallet": "0xseller",
                "side": "SELL",
                "asset": "token_sell",
                "conditionId": "cond1",
                "size": 25.0,
                "price": 0.30,
                "timestamp": 1700002000,
                "title": "Global trade",
                "slug": "global-trade",
                "icon": null,
                "eventSlug": null,
                "outcome": "No",
                "outcomeIndex": 1,
                "name": null,
                "pseudonym": null,
                "bio": null,
                "profileImage": null,
                "profileImageOptimized": null,
                "transactionHash": null
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let trades = data
        .trades()
        .list()
        .market(["cond1", "cond2"])
        .side(polyoxide_data::types::TradeSide::Sell)
        .taker_only(false)
        .send()
        .await
        .unwrap();

    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].side, polyoxide_data::types::TradeSide::Sell);
    assert_eq!(trades[0].condition_id, "cond1");
    assert!(trades[0].transaction_hash.is_none());
    mock.assert_async().await;
}

#[tokio::test]
async fn holders_list_with_nested_types() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/holders")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("market".into(), "cond1".into()),
            Matcher::UrlEncoded("limit".into(), "5".into()),
            Matcher::UrlEncoded("minBalance".into(), "100".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "token": "token_abc",
                "holders": [
                    {
                        "proxyWallet": "0xholder1",
                        "bio": "Top trader",
                        "asset": "token_abc",
                        "pseudonym": "whale1",
                        "amount": 50000.0,
                        "displayUsernamePublic": true,
                        "outcomeIndex": 0,
                        "name": "Holder One",
                        "profileImage": "https://example.com/img.png",
                        "profileImageOptimized": "https://example.com/img_opt.png",
                        "verified": true
                    },
                    {
                        "proxyWallet": "0xholder2",
                        "bio": null,
                        "asset": null,
                        "pseudonym": null,
                        "amount": 1000.0,
                        "displayUsernamePublic": null,
                        "outcomeIndex": 1,
                        "name": null,
                        "profileImage": null,
                        "profileImageOptimized": null
                    }
                ]
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let holders = data
        .holders()
        .list(["cond1"])
        .limit(5)
        .min_balance(100)
        .send()
        .await
        .unwrap();

    assert_eq!(holders.len(), 1);
    assert_eq!(holders[0].token, "token_abc");
    assert_eq!(holders[0].holders.len(), 2);

    let h1 = &holders[0].holders[0];
    assert_eq!(h1.proxy_wallet, "0xholder1");
    assert!((h1.amount - 50000.0).abs() < f64::EPSILON);
    assert_eq!(h1.display_username_public, Some(true));
    assert_eq!(h1.verified, Some(true));

    let h2 = &holders[0].holders[1];
    assert_eq!(h2.proxy_wallet, "0xholder2");
    assert!(h2.bio.is_none());
    // verified missing from JSON -> defaults to None via #[serde(default)]
    assert!(h2.verified.is_none());
    mock.assert_async().await;
}

#[tokio::test]
async fn leaderboard_get_with_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/leaderboard")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("category".into(), "POLITICS".into()),
            Matcher::UrlEncoded("timePeriod".into(), "WEEK".into()),
            Matcher::UrlEncoded("orderBy".into(), "VOL".into()),
            Matcher::UrlEncoded("limit".into(), "10".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "rank": "1",
                "proxyWallet": "0xtop",
                "userName": "top_trader",
                "vol": 5000000.50,
                "pnl": 250000.75,
                "profileImage": "https://example.com/pic.png",
                "xUsername": "top_x",
                "verifiedBadge": true
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let rankings = data
        .leaderboard()
        .get()
        .category(polyoxide_data::api::leaderboard::LeaderboardCategory::Politics)
        .time_period(polyoxide_data::types::TimePeriod::Week)
        .order_by(polyoxide_data::api::leaderboard::LeaderboardOrderBy::Vol)
        .limit(10)
        .send()
        .await
        .unwrap();

    assert_eq!(rankings.len(), 1);
    assert_eq!(rankings[0].rank, "1");
    assert_eq!(rankings[0].proxy_wallet, "0xtop");
    assert_eq!(rankings[0].user_name.as_deref(), Some("top_trader"));
    assert!((rankings[0].vol - 5000000.50).abs() < f64::EPSILON);
    assert_eq!(rankings[0].verified_badge, Some(true));
    mock.assert_async().await;
}

#[tokio::test]
async fn builders_leaderboard() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/builders/leaderboard")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("timePeriod".into(), "MONTH".into()),
            Matcher::UrlEncoded("limit".into(), "5".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "rank": "1",
                "builder": "polymarket-app",
                "volume": 1500000.50,
                "activeUsers": 25000,
                "verified": true,
                "builderLogo": "https://example.com/logo.png"
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let rankings = data
        .builders()
        .leaderboard()
        .time_period(polyoxide_data::types::TimePeriod::Month)
        .limit(5)
        .send()
        .await
        .unwrap();

    assert_eq!(rankings.len(), 1);
    assert_eq!(rankings[0].rank, "1");
    assert_eq!(rankings[0].builder, "polymarket-app");
    assert!((rankings[0].volume - 1500000.50).abs() < f64::EPSILON);
    assert_eq!(rankings[0].active_users, 25000);
    assert!(rankings[0].verified);
    mock.assert_async().await;
}

#[tokio::test]
async fn open_interest_get() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/oi")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "market".into(),
            "cond1".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"market": "cond1", "value": 50000.0}]"#)
        .create_async()
        .await;

    let data = test_data(&server);
    let oi = data
        .open_interest()
        .get()
        .market(["cond1"])
        .send()
        .await
        .unwrap();

    assert_eq!(oi.len(), 1);
    assert_eq!(oi[0].market, "cond1");
    assert!((oi[0].value - 50000.0).abs() < f64::EPSILON);
    mock.assert_async().await;
}

#[tokio::test]
async fn live_volume_get() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/live-volume")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "id".into(),
            "42".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "total": 750000.0,
                "markets": [
                    {"market": "cond_001", "value": 500000.0},
                    {"market": "cond_002", "value": 250000.0}
                ]
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let vols = data.live_volume().get(42).await.unwrap();

    assert_eq!(vols.len(), 1);
    assert!((vols[0].total - 750000.0).abs() < f64::EPSILON);
    assert_eq!(vols[0].markets.len(), 2);
    assert_eq!(vols[0].markets[0].market, "cond_001");
    assert!((vols[0].markets[0].value - 500000.0).abs() < f64::EPSILON);
    mock.assert_async().await;
}

#[tokio::test]
async fn market_positions_list_with_filters() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/market-positions")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("market".into(), "cond_mp".into()),
            Matcher::UrlEncoded("user".into(), "0xabc".into()),
            Matcher::UrlEncoded("status".into(), "OPEN".into()),
            Matcher::UrlEncoded("sortBy".into(), "TOTAL_PNL".into()),
            Matcher::UrlEncoded("sortDirection".into(), "DESC".into()),
            Matcher::UrlEncoded("limit".into(), "25".into()),
            Matcher::UrlEncoded("offset".into(), "0".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "token": "token_a",
                "positions": [{
                    "proxyWallet": "0xabc",
                    "name": "Alice",
                    "profileImage": null,
                    "verified": false,
                    "asset": "token_a",
                    "conditionId": "cond_mp",
                    "avgPrice": 0.42,
                    "size": 100.0,
                    "currPrice": 0.51,
                    "currentValue": 51.0,
                    "cashPnl": 9.0,
                    "totalBought": 42.0,
                    "realizedPnl": 0.0,
                    "totalPnl": 9.0,
                    "outcome": "Yes",
                    "outcomeIndex": 0
                }]
            }]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let meta = data
        .market_positions()
        .list("cond_mp")
        .user("0xabc")
        .status(polyoxide_data::types::MarketPositionStatus::Open)
        .sort_by(polyoxide_data::types::MarketPositionSortBy::TotalPnl)
        .sort_direction(polyoxide_data::types::SortDirection::Desc)
        .limit(25)
        .offset(0)
        .send()
        .await
        .unwrap();

    assert_eq!(meta.len(), 1);
    assert_eq!(meta[0].token, "token_a");
    assert_eq!(meta[0].positions.len(), 1);
    let p = &meta[0].positions[0];
    assert_eq!(p.proxy_wallet, "0xabc");
    assert_eq!(p.outcome, "Yes");
    assert!((p.curr_price - 0.51).abs() < f64::EPSILON);
    assert!((p.total_pnl - 9.0).abs() < f64::EPSILON);
    mock.assert_async().await;
}

#[tokio::test]
async fn accounting_snapshot_returns_zip_bytes() {
    let mut server = Server::new_async().await;

    // Local ZIP header bytes ("PK\x03\x04") + a couple of arbitrary bytes, to
    // prove the helper does not touch or parse the body.
    let body: Vec<u8> = vec![0x50, 0x4B, 0x03, 0x04, 0xAA, 0xBB, 0xCC];

    let mock = server
        .mock("GET", "/v1/accounting/snapshot")
        .match_query(Matcher::UrlEncoded("user".into(), "0xabc123".into()))
        .with_status(200)
        .with_header("content-type", "application/zip")
        .with_body(body.clone())
        .create_async()
        .await;

    let data = test_data(&server);
    let bytes = data
        .accounting()
        .snapshot("0xabc123")
        .await
        .expect("snapshot returns bytes");
    assert_eq!(bytes, body);
    mock.assert_async().await;
}

#[tokio::test]
async fn error_404_returns_api_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/positions")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "user".into(),
            "0xbad".into(),
        )]))
        .with_status(404)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "not found"}"#)
        .create_async()
        .await;

    let data = test_data(&server);
    let err = data
        .user("0xbad")
        .list_positions()
        .send()
        .await
        .unwrap_err();

    match err {
        DataApiError::Api(polyoxide_core::ApiError::Api { status, message }) => {
            assert_eq!(status, 404);
            assert_eq!(message, "not found");
        }
        other => panic!("Expected Api error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn ping_returns_latency_on_200() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_body(r#"{"data":"OK"}"#)
        .create_async()
        .await;

    let data = test_data(&server);
    let latency = data.health().ping().await.expect("ping should succeed");
    assert!(
        latency.as_millis() < 5000,
        "latency should be reasonable for local mock"
    );

    mock.assert_async().await;
}

#[tokio::test]
async fn ping_propagates_3xx_as_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/")
        .with_status(301)
        .with_header("location", "/docs")
        .create_async()
        .await;

    let data = test_data(&server);
    let err = data.health().ping().await.unwrap_err();
    assert!(
        matches!(err, DataApiError::Api(_)),
        "expected ApiError for unexpected 3xx, got {err:?}"
    );

    mock.assert_async().await;
}

#[tokio::test]
async fn ping_propagates_5xx_as_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/")
        .with_status(503)
        .with_body("upstream unavailable")
        .create_async()
        .await;

    let data = test_data(&server);
    let err = data.health().ping().await.unwrap_err();
    assert!(
        matches!(err, DataApiError::Api(_)),
        "expected ApiError for 5xx, got {err:?}"
    );

    mock.assert_async().await;
}

#[tokio::test]
async fn combo_positions_with_filters_and_cursor() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/positions/combos")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc123".into()),
            Matcher::UrlEncoded("status".into(), "RESOLVED_WIN,RESOLVED_PARTIAL".into()),
            Matcher::UrlEncoded("sort".into(), "updated_asc".into()),
            Matcher::UrlEncoded("updatedAfter".into(), "1700000000".into()),
            Matcher::UrlEncoded("cursor".into(), "opaque-token".into()),
            Matcher::UrlEncoded("limit".into(), "50".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "combos": [{
                    "combo_condition_id": "0x0391ab0ebea17b65ba87e071b0566e816b0000000000000000000000000000",
                    "combo_position_id": "pos-1",
                    "module_id": 3,
                    "user_address": "0xabc123",
                    "shares_balance": "12.500000",
                    "entry_avg_price_usdc": "0.4",
                    "entry_cost_usdc": "5.000000",
                    "realized_payout_usdc": "0.00",
                    "total_cost_usdc": "5.000000",
                    "gross_entry_cost_usdc": "8999.997488",
                    "entry_fees_usdc": "2.512000",
                    "status": "RESOLVED_PARTIAL",
                    "first_entry_at": "2026-01-02T03:04:05Z",
                    "resolved_at": null,
                    "updated_at": "2026-06-01T00:00:00Z",
                    "legs_total": 2,
                    "legs_resolved": 1,
                    "legs_pending": 1,
                    "legs": [{
                        "leg_index": 0,
                        "leg_position_id": "leg-1",
                        "leg_condition_id": "0xleg",
                        "leg_outcome_index": 1,
                        "leg_outcome_label": "No",
                        "leg_status": "RESOLVED_PARTIAL",
                        "leg_resolved_at": "2026-05-01T00:00:00Z",
                        "leg_current_price": "0.5",
                        "market": {
                            "market_id": "m-1",
                            "slug": "will-x",
                            "title": "Will X happen?",
                            "outcome": "No",
                            "tags": ["politics"],
                            "end_date": "2026-12-31T00:00:00Z",
                            "event": {"event_id": "e-1", "event_slug": "ev", "event_title": "Ev"}
                        }
                    }]
                }],
                "pagination": {"limit": 50, "offset": 0, "has_more": true, "next_cursor": "next-token"}
            }"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let res = data
        .combos()
        .positions("0xabc123")
        .status([ComboStatus::ResolvedWin, ComboStatus::ResolvedPartial])
        .sort(ComboSort::UpdatedAsc)
        .updated_after(1_700_000_000)
        .cursor("opaque-token")
        .limit(50)
        .send()
        .await
        .unwrap();

    assert_eq!(res.combos.len(), 1);
    let combo = &res.combos[0];
    assert_eq!(combo.status, Some(ComboStatus::ResolvedPartial));
    // Monetary fields stay as strings so six-decimal precision survives.
    assert_eq!(combo.gross_entry_cost_usdc.as_deref(), Some("8999.997488"));
    assert_eq!(combo.entry_fees_usdc.as_deref(), Some("2.512000"));
    assert_eq!(combo.updated_at.as_deref(), Some("2026-06-01T00:00:00Z"));
    assert_eq!(combo.legs.len(), 1);
    assert_eq!(
        combo.legs[0].leg_status,
        Some(ComboLegStatus::ResolvedPartial)
    );

    let pagination = res.pagination.unwrap();
    assert!(pagination.has_more);
    assert_eq!(pagination.next_cursor.as_deref(), Some("next-token"));

    mock.assert_async().await;
}

#[tokio::test]
async fn combo_positions_status_drops_unknown_variant() {
    let mut server = Server::new_async().await;

    // Only OPEN should reach the wire; Unknown has no upstream value to filter on.
    let mock = server
        .mock("GET", "/v1/positions/combos")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc".into()),
            Matcher::UrlEncoded("status".into(), "OPEN".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"combos": [], "pagination": null}"#)
        .create_async()
        .await;

    let data = test_data(&server);
    let res = data
        .combos()
        .positions("0xabc")
        .status([ComboStatus::Open, ComboStatus::Unknown])
        .send()
        .await
        .unwrap();

    assert!(res.combos.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn combo_positions_unknown_status_does_not_poison_page() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/positions/combos")
        .match_query(Matcher::UrlEncoded("user".into(), "0xabc".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"combos": [
                {"combo_condition_id": "0xaaa", "status": "SOME_FUTURE_STATUS", "legs": []},
                {"combo_condition_id": "0xbbb", "status": "OPEN", "legs": []}
            ]}"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let res = data.combos().positions("0xabc").send().await.unwrap();

    assert_eq!(res.combos.len(), 2, "unknown status must not drop the page");
    assert_eq!(res.combos[0].status, Some(ComboStatus::Unknown));
    assert_eq!(res.combos[1].status, Some(ComboStatus::Open));

    mock.assert_async().await;
}

#[tokio::test]
async fn combo_activity_list() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/v1/activity/combos")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc".into()),
            Matcher::UrlEncoded("market_id".into(), "0xcombo1,0xcombo2".into()),
            Matcher::UrlEncoded("limit".into(), "10".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "activity": [{
                    "id": "act-1",
                    "type": "redeem",
                    "event_kind": "PositionRedeemed",
                    "side": "Redeem",
                    "module_kind": "Combinatorial",
                    "user_address": "0xabc",
                    "combo_condition_id": "0xcombo1",
                    "amount_usdc": null,
                    "payout_usdc": 42.5,
                    "timestamp": 1700000000,
                    "tx_hash": "0xdead",
                    "legs": []
                }],
                "pagination": {"limit": 10, "offset": 0, "has_more": false, "next_cursor": null}
            }"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let res = data
        .combos()
        .activity("0xabc")
        .market_id(["0xcombo1", "0xcombo2"])
        .limit(10)
        .send()
        .await
        .unwrap();

    assert_eq!(res.activity.len(), 1);
    let row = &res.activity[0];
    assert_eq!(row.activity_type.as_deref(), Some("redeem"));
    assert_eq!(row.payout_usdc, Some(42.5));
    assert_eq!(row.amount_usdc, None);
    assert!(!res.pagination.unwrap().has_more);

    mock.assert_async().await;
}

#[tokio::test]
async fn misc_other_size_and_revisions() {
    let mut server = Server::new_async().await;

    let other = server
        .mock("GET", "/other")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("id".into(), "123".into()),
            Matcher::UrlEncoded("user".into(), "0xabc".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"id": 123, "user": "0xabc", "size": 7.5}]"#)
        .create_async()
        .await;

    let revisions = server
        .mock("GET", "/revisions")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("questionID".into(), "0xq".into()),
            Matcher::UrlEncoded("limit".into(), "5".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{"questionID": "0xq", "revisions": [{"revision": "clarified", "timestamp": 1700000000}]}]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);

    let sizes = data.misc().other_size(123, "0xabc").send().await.unwrap();
    assert_eq!(sizes.len(), 1);
    assert_eq!(sizes[0].size, Some(7.5));

    let revs = data.misc().revisions("0xq").limit(5).send().await.unwrap();
    assert_eq!(revs.len(), 1);
    assert_eq!(revs[0].question_id.as_deref(), Some("0xq"));
    assert_eq!(revs[0].revisions[0].revision.as_deref(), Some("clarified"));

    other.assert_async().await;
    revisions.assert_async().await;
}

#[tokio::test]
async fn trades_list_with_time_window() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/trades")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc".into()),
            Matcher::UrlEncoded("start".into(), "1".into()),
            Matcher::UrlEncoded("end".into(), "1700000000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let data = test_data(&server);
    let trades = data
        .trades()
        .list()
        .user("0xabc")
        .start(1)
        .end(1_700_000_000)
        .send()
        .await
        .unwrap();

    assert!(trades.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn activity_type_covers_deposit_withdrawal_yield() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/activity")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user".into(), "0xabc".into()),
            Matcher::UrlEncoded("type".into(), "DEPOSIT,WITHDRAWAL,YIELD".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"proxyWallet": "0xabc", "timestamp": 1, "conditionId": "c", "type": "DEPOSIT",
                 "size": 1.0, "usdcSize": 1.0, "transactionHash": "0x1"},
                {"proxyWallet": "0xabc", "timestamp": 2, "conditionId": "c", "type": "WITHDRAWAL",
                 "size": 1.0, "usdcSize": 1.0, "transactionHash": "0x2"},
                {"proxyWallet": "0xabc", "timestamp": 3, "conditionId": "c", "type": "YIELD",
                 "size": 1.0, "usdcSize": 1.0, "transactionHash": "0x3"}
            ]"#,
        )
        .create_async()
        .await;

    let data = test_data(&server);
    let rows = data
        .user("0xabc")
        .activity()
        .activity_type([
            ActivityType::Deposit,
            ActivityType::Withdrawal,
            ActivityType::Yield,
        ])
        .send()
        .await
        .unwrap();

    // These three used to fall through to `Unknown` via #[serde(other)].
    assert_eq!(rows[0].activity_type, ActivityType::Deposit);
    assert_eq!(rows[1].activity_type, ActivityType::Withdrawal);
    assert_eq!(rows[2].activity_type, ActivityType::Yield);

    mock.assert_async().await;
}

#[tokio::test]
async fn user_pnl_series_parses() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/user-pnl")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("user_address".into(), "0xabc".into()),
            Matcher::UrlEncoded("interval".into(), "1d".into()),
            Matcher::UrlEncoded("fidelity".into(), "1h".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"t":1784836800,"p":-1703608.4},{"t":1784840400,"p":12.5}]"#)
        .create_async()
        .await;

    let data = DataApi::builder()
        .pnl_base_url(server.url())
        .build()
        .unwrap();

    let points = data
        .pnl()
        .history("0xabc")
        .interval("1d")
        .fidelity(PnlFidelity::OneHour)
        .send()
        .await
        .unwrap();

    assert_eq!(points.len(), 2);
    assert_eq!(points[0].timestamp, 1_784_836_800);
    assert!((points[0].pnl - -1_703_608.4).abs() < 1e-6);

    mock.assert_async().await;
}

#[tokio::test]
async fn rankings_volume_and_profit_parse() {
    let mut server = Server::new_async().await;

    let volume = server
        .mock("GET", "/volume")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("window".into(), "all".into()),
            Matcher::UrlEncoded("limit".into(), "2".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{"proxyWallet":"0x204f","pseudonym":"swisstony","amount":1730945622.28217,
                 "name":"swisstony","bio":"","profileImage":"","profileImageOptimized":""}]"#,
        )
        .create_async()
        .await;

    let profit = server
        .mock("GET", "/profit")
        .match_query(Matcher::UrlEncoded("window".into(), "7d".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"proxyWallet":"0x56","amount":22053933.75}]"#)
        .create_async()
        .await;

    let data = DataApi::builder()
        .rankings_base_url(server.url())
        .build()
        .unwrap();

    let vol = data
        .rankings()
        .volume()
        .window(RankingWindow::All)
        .limit(2)
        .send()
        .await
        .unwrap();
    assert_eq!(vol[0].proxy_wallet.as_deref(), Some("0x204f"));
    assert_eq!(vol[0].name.as_deref(), Some("swisstony"));

    let prof = data
        .rankings()
        .profit()
        .window(RankingWindow::SevenDays)
        .send()
        .await
        .unwrap();
    // Sparse rows must not fail: only two of the seven fields are present.
    assert_eq!(prof[0].proxy_wallet.as_deref(), Some("0x56"));
    assert_eq!(prof[0].name, None);

    volume.assert_async().await;
    profit.assert_async().await;
}

#[tokio::test]
async fn each_namespace_targets_its_own_host() {
    // Three servers standing in for the three hosts. Each mock asserts it was
    // hit exactly once, so a namespace routed to the wrong base URL fails here
    // rather than silently querying the wrong service.
    let mut main = Server::new_async().await;
    let mut pnl = Server::new_async().await;
    let mut rankings = Server::new_async().await;

    let main_mock = main
        .mock("GET", "/trades")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .expect(1)
        .create_async()
        .await;
    let pnl_mock = pnl
        .mock("GET", "/user-pnl")
        // Explicit: mockito's default query matcher is not `Any`, and this is
        // the only one of the three that sends a query string.
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .expect(1)
        .create_async()
        .await;
    let rankings_mock = rankings
        .mock("GET", "/volume")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .expect(1)
        .create_async()
        .await;

    let data = DataApi::builder()
        .base_url(main.url())
        .pnl_base_url(pnl.url())
        .rankings_base_url(rankings.url())
        .build()
        .unwrap();

    data.trades().list().send().await.unwrap();
    data.pnl().history("0xabc").send().await.unwrap();
    data.rankings().volume().send().await.unwrap();

    main_mock.assert_async().await;
    pnl_mock.assert_async().await;
    rankings_mock.assert_async().await;
}

#[tokio::test]
async fn overriding_one_host_leaves_the_others_alone() {
    // Only the PnL host is overridden; the main host must still be reachable
    // at its own base URL rather than following the override.
    let mut main = Server::new_async().await;
    let main_mock = main
        .mock("GET", "/trades")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .expect(1)
        .create_async()
        .await;

    let data = DataApi::builder()
        .base_url(main.url())
        .pnl_base_url("http://127.0.0.1:1")
        .build()
        .unwrap();

    data.trades().list().send().await.unwrap();
    main_mock.assert_async().await;
}
