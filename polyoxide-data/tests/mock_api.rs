use mockito::{Matcher, Server};
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
