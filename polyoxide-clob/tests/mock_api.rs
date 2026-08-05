use mockito::{Matcher, Server};
use polyoxide_clob::{
    Account, ClobBuilder, ClobError, Credentials, MultiMarketOrderBy, SignatureType, SortPosition,
    UserRewardMarketOrderBy,
};
use polyoxide_core::RetryConfig;

fn test_public_clob(server: &mockito::ServerGuard) -> polyoxide_clob::Clob {
    ClobBuilder::new().base_url(server.url()).build().unwrap()
}

fn test_authed_clob(server: &mockito::ServerGuard) -> polyoxide_clob::Clob {
    test_authed_clob_with_sig(server, SignatureType::Eoa)
}

/// Authed client with an explicit signature type, for endpoints that thread the
/// configured `signature_type` into their requests.
fn test_authed_clob_with_sig(
    server: &mockito::ServerGuard,
    signature_type: SignatureType,
) -> polyoxide_clob::Clob {
    let creds = Credentials {
        key: "test-key".into(),
        secret: "c2VjcmV0".into(), // base64("secret")
        passphrase: "test-pass".into(),
    };
    // Hardhat account #0
    let account = Account::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        creds,
    )
    .unwrap();
    ClobBuilder::new()
        .base_url(server.url())
        .with_account(account)
        .signature_type(signature_type)
        .build()
        .unwrap()
}

/// Authed client (Hardhat account #0) with a configured builder-attribution code.
fn test_authed_clob_with_builder_code(
    server: &mockito::ServerGuard,
    builder_code: alloy::primitives::B256,
) -> polyoxide_clob::Clob {
    let creds = Credentials {
        key: "test-key".into(),
        secret: "c2VjcmV0".into(), // base64("secret")
        passphrase: "test-pass".into(),
    };
    let account = Account::new(
        "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        creds,
    )
    .unwrap();
    ClobBuilder::new()
        .base_url(server.url())
        .with_account(account)
        .builder_code(builder_code)
        .build()
        .unwrap()
}

#[tokio::test]
async fn server_time_unauthenticated() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("1700000000")
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob.health().server_time().send().await.unwrap();

    assert_eq!(resp.time, 1700000000);
    mock.assert_async().await;
}

#[tokio::test]
async fn health_ping_returns_latency() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_body("OK")
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let latency = clob.health().ping().await.unwrap();

    assert!(
        latency.as_millis() < 5000,
        "Latency should be reasonable for local mock"
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn authenticated_request_sends_l2_headers() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/data/orders")
        .match_header("POLY_API_KEY", "test-key")
        .match_header("POLY_PASSPHRASE", "test-pass")
        .match_header(
            "POLY_ADDRESS",
            Matcher::Regex(r"^0x[0-9a-fA-F]{40}$".into()),
        )
        .match_header("POLY_SIGNATURE", Matcher::Any)
        .match_header("POLY_TIMESTAMP", Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": "LTE="}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob.orders().unwrap().list().send().await.unwrap();

    assert!(resp.data.is_empty());
    assert_eq!(resp.next_cursor.as_deref(), Some("LTE="));
    mock.assert_async().await;
}

#[tokio::test]
async fn authenticated_request_sends_poly_address() {
    let mut server = Server::new_async().await;

    // Hardhat #0 address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
    let mock = server
        .mock("GET", "/data/orders")
        .match_header(
            "POLY_ADDRESS",
            Matcher::Regex(r"(?i)0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": "LTE="}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let _orders = clob.orders().unwrap().list().send().await.unwrap();

    mock.assert_async().await;
}

#[tokio::test]
async fn authenticated_401_returns_authentication_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/data/orders")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "invalid api key"}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let err = clob.orders().unwrap().list().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Authentication(msg)) => {
            assert_eq!(msg, "invalid api key");
        }
        other => panic!("Expected Authentication error, got: {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn cancel_order_sends_delete_with_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/order")
        .match_header("POLY_API_KEY", "test-key")
        .match_body(Matcher::JsonString(r#"{"orderID": "order-123"}"#.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled": ["order-123"], "notCanceled": {}}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .orders()
        .unwrap()
        .cancel("order-123")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.canceled, vec!["order-123"]);
    assert!(resp.not_canceled.is_empty());

    mock.assert_async().await;
}

#[tokio::test]
async fn trades_hits_data_trades_endpoint() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/data/trades")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "maker_address".into(),
            "0x0000000000000000000000000000000000000001".into(),
        )]))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "data": [{
                    "id": "t1",
                    "taker_order_id": "o1",
                    "market": "0xcond",
                    "asset_id": "0xtoken",
                    "side": "BUY",
                    "size": "100",
                    "fee_rate_bps": "0",
                    "price": "0.55",
                    "status": "MATCHED",
                    "match_time": "1700000000",
                    "outcome": "Yes",
                    "owner": "0x0000000000000000000000000000000000000001",
                    "transaction_hash": "0xhash"
                }],
                "next_cursor": "abc123"
            }"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .trades("0x0000000000000000000000000000000000000001")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "t1");
    assert_eq!(resp.next_cursor.as_deref(), Some("abc123"));
    mock.assert_async().await;
}

#[tokio::test]
async fn balance_allowance_returns_flat_response() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/balance-allowance")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "asset_type".into(),
            "COLLATERAL".into(),
        )]))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"balance": "1000.50", "allowances": {"0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E": "999999"}}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .usdc_balance()
        .send()
        .await
        .unwrap();

    assert_eq!(resp.balance, "1000.50");
    assert_eq!(
        resp.allowances
            .get("0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E")
            .unwrap(),
        "999999"
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn balance_allowance_sends_conditional_asset_type() {
    let mut server = Server::new_async().await;

    // balance_allowance(token_id) targets a conditional (outcome) token, so the
    // required asset_type query param must be CONDITIONAL.
    let mock = server
        .mock("GET", "/balance-allowance")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("asset_type".into(), "CONDITIONAL".into()),
            Matcher::UrlEncoded("token_id".into(), "0xtoken".into()),
            Matcher::UrlEncoded("signature_type".into(), "0".into()),
        ]))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"balance": "0", "allowances": {}}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .balance_allowance("0xtoken")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.balance, "0");
    mock.assert_async().await;
}

#[tokio::test]
async fn builder_trades_returns_paginated_response() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/builder/trades")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "data": [{
                    "id": "bt-1",
                    "tradeType": "LIMIT",
                    "takerOrderHash": "0xhash",
                    "builder": "0xbuilder",
                    "market": "0xcond",
                    "assetId": "0xtoken",
                    "side": "BUY",
                    "size": "100",
                    "sizeUsdc": "55.00",
                    "price": "0.55",
                    "status": "MATCHED",
                    "outcome": "Yes",
                    "outcomeIndex": 0,
                    "owner": "0xowner",
                    "maker": "0xmaker",
                    "transactionHash": "0xtx",
                    "matchTime": "1700000000",
                    "fee": "0.01",
                    "feeUsdc": "0.55",
                    "createdAt": "2024-01-01T00:00:00Z",
                    "updatedAt": null
                }],
                "next_cursor": "cursor1"
            }"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .builder_trades("0x0000000000000000000000000000000000000000000000000000000000000001")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "bt-1");
    assert_eq!(resp.data[0].trade_type, "LIMIT");
    assert_eq!(resp.data[0].size_usdc, "55.00");
    assert_eq!(resp.next_cursor.as_deref(), Some("cursor1"));
    mock.assert_async().await;
}

#[tokio::test]
async fn builder_trades_sends_required_builder_code_query() {
    const BUILDER_CODE: &str = "0x0000000000000000000000000000000000000000000000000000000000000001";
    let mut server = Server::new_async().await;

    // The mock only matches if builder_code is present in the query string,
    // so a hit proves the required parameter was sent.
    let mock = server
        .mock("GET", "/builder/trades")
        .match_query(Matcher::UrlEncoded(
            "builder_code".into(),
            BUILDER_CODE.into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": null}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .builder_trades(BUILDER_CODE)
        .send()
        .await
        .unwrap();

    assert!(resp.data.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn builder_trades_sends_optional_filters() {
    const BUILDER_CODE: &str = "0x0000000000000000000000000000000000000000000000000000000000000001";
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/builder/trades")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("builder_code".into(), BUILDER_CODE.into()),
            Matcher::UrlEncoded("id".into(), "trade-9".into()),
            Matcher::UrlEncoded("asset_id".into(), "0xtoken".into()),
            Matcher::UrlEncoded("before".into(), "1700000000".into()),
            Matcher::UrlEncoded("next_cursor".into(), "MA==".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": null}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .builder_trades(BUILDER_CODE)
        .id("trade-9")
        .asset_id("0xtoken")
        .before("1700000000")
        .next_cursor("MA==")
        .send()
        .await
        .unwrap();

    assert!(resp.data.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn cancel_many_sends_flat_array_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/orders")
        .match_header("POLY_API_KEY", "test-key")
        .match_body(Matcher::JsonString(
            r#"["order-1", "order-2", "order-3"]"#.into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"canceled": ["order-1", "order-2"], "notCanceled": {"order-3": "not found"}}"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .orders()
        .unwrap()
        .cancel_many(vec![
            "order-1".to_string(),
            "order-2".to_string(),
            "order-3".to_string(),
        ])
        .await
        .unwrap();

    assert_eq!(resp.canceled, vec!["order-1", "order-2"]);
    assert_eq!(resp.not_canceled.len(), 1);
    assert_eq!(resp.not_canceled.get("order-3").unwrap(), "not found");
    mock.assert_async().await;
}

#[tokio::test]
async fn order_book_decimal_serde() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/book")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "token_id".into(),
            "0xtoken".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "market": "0xcond",
                "asset_id": "0xtoken",
                "bids": [{"price": "0.48", "size": "100.5"}],
                "asks": [{"price": "0.52", "size": "200.25"}],
                "timestamp": "1700000000",
                "hash": "abc123",
                "min_order_size": "5",
                "tick_size": "0.001",
                "neg_risk": false,
                "last_trade_price": "0.50"
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let ob = clob.markets().order_book("0xtoken").send().await.unwrap();

    assert_eq!(ob.market, "0xcond");
    assert_eq!(ob.bids.len(), 1);
    assert_eq!(ob.bids[0].price, rust_decimal::Decimal::new(48, 2));
    assert_eq!(ob.bids[0].size, rust_decimal::Decimal::new(1005, 1));
    assert_eq!(ob.asks[0].price, rust_decimal::Decimal::new(52, 2));
    assert_eq!(ob.asks[0].size, rust_decimal::Decimal::new(20025, 2));
    assert_eq!(ob.min_order_size.as_deref(), Some("5"));
    assert_eq!(ob.neg_risk, Some(false));
    mock.assert_async().await;
}

#[tokio::test]
async fn tick_size_string_response() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "token_id".into(),
            "0xtoken".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob.markets().tick_size("0xtoken").send().await.unwrap();

    assert_eq!(resp.minimum_tick_size, "0.01");
    mock.assert_async().await;
}

#[tokio::test]
async fn tick_size_number_response() {
    let mut server = Server::new_async().await;

    // API sometimes returns tick_size as a number instead of a string
    let mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "token_id".into(),
            "0xtoken".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": 0.01}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob.markets().tick_size("0xtoken").send().await.unwrap();

    // Custom deserializer converts number to string
    assert_eq!(resp.minimum_tick_size, "0.01");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_order_deserializes_captured_shape() {
    let mut server = Server::new_async().await;

    // `/data/order/{orderID}` returns the same shape as `/data/orders`; this
    // body mirrors one captured live on 2026-07-24. The previous fixture here
    // was written to match the (wrong) struct rather than the venue, so it
    // asserted camelCase names and a flattened SignedOrder that never arrive.
    let mock = server
        .mock("GET", "/data/order/order-123")
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "order-123",
                "status": "LIVE",
                "owner": "aa17dfae-754d-2498-f336-8bd1db84f525",
                "maker_address": "0xb98ad946c7f753596F26396Bf3F34A2EeBc39E86",
                "market": "0xcond",
                "asset_id": "0xtoken",
                "side": "BUY",
                "original_size": "200.5",
                "size_matched": "100.0",
                "price": "0.55",
                "outcome": "Yes",
                "expiration": "0",
                "order_type": "GTC",
                "associate_trades": ["trade-1"],
                "created_at": 1784930007
            }"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let order = clob
        .orders()
        .unwrap()
        .get("order-123")
        .send()
        .await
        .unwrap();

    assert_eq!(order.id, "order-123");
    assert_eq!(order.asset_id, "0xtoken");
    assert_eq!(order.side, "BUY");
    assert_eq!(order.owner, "aa17dfae-754d-2498-f336-8bd1db84f525");
    assert_eq!(order.original_size, "200.5");
    assert_eq!(order.size_matched, "100.0");
    assert_eq!(order.associate_trades, vec!["trade-1"]);
    assert_eq!(order.order_type, "GTC");
    assert_eq!(order.created_at, 1_784_930_007);
    mock.assert_async().await;
}

#[tokio::test]
async fn prices_history_renamed_fields() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "market".into(),
            "0xtoken".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "history": [
                    {"t": 1700000000, "p": 0.55},
                    {"t": 1700001000, "p": 0.60}
                ]
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .prices_history("0xtoken")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.history.len(), 2);
    // Validates #[serde(rename = "t")] -> timestamp, #[serde(rename = "p")] -> price
    assert_eq!(resp.history[0].timestamp, 1700000000);
    assert!((resp.history[0].price - 0.55).abs() < f64::EPSILON);
    assert_eq!(resp.history[1].timestamp, 1700001000);
    assert!((resp.history[1].price - 0.60).abs() < f64::EPSILON);
    mock.assert_async().await;
}

#[tokio::test]
async fn prices_history_with_query_params() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/prices-history")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("market".into(), "0xtoken".into()),
            Matcher::UrlEncoded("interval".into(), "max".into()),
            Matcher::UrlEncoded("fidelity".into(), "1".into()),
            Matcher::UrlEncoded("startTs".into(), "1700000000".into()),
            Matcher::UrlEncoded("endTs".into(), "1700900000".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"history":[{"t":1700000000,"p":0.5}]}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let query = polyoxide_clob::PricesHistoryQuery {
        interval: Some("max".into()),
        fidelity: Some(1),
        start_ts: Some(1_700_000_000),
        end_ts: Some(1_700_900_000),
    };
    let resp = clob
        .markets()
        .prices_history_with("0xtoken", &query)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.history.len(), 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn calculate_price_post_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/calculate-price")
        .match_body(Matcher::JsonString(
            r#"{"token_id": "0xtoken", "side": "BUY", "amount": "100"}"#.into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"price": "0.52"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .calculate_price("0xtoken", polyoxide_clob::OrderSide::Buy, "100")
        .await
        .unwrap();

    assert_eq!(resp.price, "0.52");
    mock.assert_async().await;
}

#[tokio::test]
async fn cancel_all_sends_delete_no_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/cancel-all")
        .match_header("POLY_API_KEY", "test-key")
        .match_header("POLY_SIGNATURE", Matcher::Any)
        .match_header("POLY_TIMESTAMP", Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled": ["o1", "o2"], "notCanceled": {}}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob.orders().unwrap().cancel_all().await.unwrap();

    assert_eq!(resp.canceled, vec!["o1", "o2"]);
    assert!(resp.not_canceled.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn cancel_market_sends_delete_with_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("DELETE", "/cancel-market-orders")
        .match_header("POLY_API_KEY", "test-key")
        .match_body(Matcher::JsonString(
            r#"{"market": "0xcond", "asset_id": "0xtoken"}"#.into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"canceled": ["o1"], "notCanceled": {}}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .orders()
        .unwrap()
        .cancel_market("0xcond", "0xtoken")
        .await
        .unwrap();

    assert_eq!(resp.canceled, vec!["o1"]);
    mock.assert_async().await;
}

#[tokio::test]
async fn order_scoring_query_param() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/order-scoring")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::AllOf(vec![Matcher::UrlEncoded(
            "order_id".into(),
            "oid-1".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"order_id": "oid-1", "scoring": true}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .orders()
        .unwrap()
        .is_scoring("oid-1")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.order_id, "oid-1");
    assert!(resp.scoring);
    mock.assert_async().await;
}

#[tokio::test]
async fn orders_scoring_query_many() {
    let mut server = Server::new_async().await;

    // query_many adds multiple params with the same key: order_ids=oid-1&order_ids=oid-2
    let mock = server
        .mock("GET", "/orders-scoring")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::AllOf(vec![
            Matcher::Regex("order_ids=oid-1".into()),
            Matcher::Regex("order_ids=oid-2".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"order_id": "oid-1", "scoring": true},
                {"order_id": "oid-2", "scoring": false}
            ]"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .orders()
        .unwrap()
        .are_scoring(vec!["oid-1".to_string(), "oid-2".to_string()])
        .send()
        .await
        .unwrap();

    assert_eq!(resp.len(), 2);
    assert!(resp[0].scoring);
    assert!(!resp[1].scoring);
    mock.assert_async().await;
}

#[tokio::test]
async fn builder_trade_err_msg_rename() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/builder/trades")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "data": [{
                    "id": "bt-err",
                    "tradeType": "LIMIT",
                    "takerOrderHash": "0xhash",
                    "builder": "0xbuilder",
                    "market": "0xcond",
                    "assetId": "0xtoken",
                    "side": "SELL",
                    "size": "50",
                    "sizeUsdc": "25.00",
                    "price": "0.50",
                    "status": "FAILED",
                    "outcome": "No",
                    "outcomeIndex": 1,
                    "owner": "0xowner",
                    "maker": "0xmaker",
                    "transactionHash": "0xtx",
                    "matchTime": "1700000000",
                    "fee": "0",
                    "feeUsdc": "0",
                    "err_msg": "insufficient balance",
                    "createdAt": null,
                    "updatedAt": null
                }],
                "next_cursor": null
            }"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .builder_trades("0x0000000000000000000000000000000000000000000000000000000000000001")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.data.len(), 1);
    // Validates #[serde(rename = "err_msg")] under #[serde(rename_all = "camelCase")]
    assert_eq!(
        resp.data[0].err_msg.as_deref(),
        Some("insufficient balance")
    );
    assert_eq!(resp.data[0].status, "FAILED");
    assert!(resp.next_cursor.is_none());
    mock.assert_async().await;
}

// ── Request retry & error tests ──

fn test_public_clob_fast_retry(server: &mockito::ServerGuard) -> polyoxide_clob::Clob {
    ClobBuilder::new()
        .base_url(server.url())
        .with_retry_config(RetryConfig {
            max_retries: 2,
            initial_backoff_ms: 1, // 1ms backoff for fast tests
            max_backoff_ms: 5,
        })
        .build()
        .unwrap()
}

#[tokio::test]
async fn retry_429_exhausted_returns_rate_limit_error() {
    let mut server = Server::new_async().await;

    // Return 429 on every request (max_retries=2, so 3 total attempts: 0, 1, 2)
    let mock = server
        .mock("GET", "/time")
        .with_status(429)
        .with_body("rate limited")
        .expect(3)
        .create_async()
        .await;

    let clob = test_public_clob_fast_retry(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    // After exhausting retries, the 429 is returned as a RateLimit error
    assert!(
        matches!(err, ClobError::Api(polyoxide_core::ApiError::RateLimit(_))),
        "Expected RateLimit error, got: {:?}",
        err
    );
    // Verify it was retried exactly max_retries times (3 total requests)
    mock.assert_async().await;
}

#[tokio::test]
async fn retry_429_with_retry_after_header() {
    let mut server = Server::new_async().await;

    // 429 with Retry-After header — should still retry and exhaust
    let mock = server
        .mock("GET", "/time")
        .with_status(429)
        .with_header("Retry-After", "0.001")
        .with_body("rate limited")
        .expect(3)
        .create_async()
        .await;

    let clob = test_public_clob_fast_retry(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    assert!(matches!(
        err,
        ClobError::Api(polyoxide_core::ApiError::RateLimit(_))
    ));
    // Retry-After header respected — still 3 total requests
    mock.assert_async().await;
}

#[tokio::test]
async fn retry_425_too_early_is_retried_then_succeeds() {
    // 425 is Polymarket's matching engine restarting. Upstream documents it as
    // "retry with exponential backoff", and it carries no body — nothing was
    // processed — so resending is safe even for a write.
    let mut server = Server::new_async().await;

    // Two 425s, then the real response. mockito serves mocks in creation order and
    // retires each once its `expect` count is met.
    let early = server
        .mock("GET", "/time")
        .with_status(425)
        .expect(2)
        .create_async()
        .await;
    let ok = server
        .mock("GET", "/time")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("1700000000")
        .expect(1)
        .create_async()
        .await;

    let clob = test_public_clob_fast_retry(&server);
    let time = clob
        .health()
        .server_time()
        .send()
        .await
        .expect("425 should be retried through to the eventual success");
    assert_eq!(time.time, 1700000000);

    early.assert_async().await;
    ok.assert_async().await;
}

#[tokio::test]
async fn retry_425_exhausted_returns_too_early_error() {
    let mut server = Server::new_async().await;

    // 425 forever: max_retries=2 on the fast-retry client, so 3 total attempts.
    let mock = server
        .mock("GET", "/time")
        .with_status(425)
        .expect(3)
        .create_async()
        .await;

    let clob = test_public_clob_fast_retry(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match &err {
        ClobError::Api(polyoxide_core::ApiError::Api { status, .. }) => assert_eq!(*status, 425),
        other => panic!("Expected Api error with status 425, got: {other:?}"),
    }
    // Still retriable once surfaced — the caller may back off further and retry.
    assert!(err.is_retriable());
    mock.assert_async().await;
}

#[tokio::test]
async fn server_500_not_retried() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(500)
        .with_body(r#"{"error": "internal server error"}"#)
        .expect(1)
        .create_async()
        .await;

    let clob = test_public_clob_fast_retry(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Api { status, .. }) => {
            assert_eq!(status, 500);
        }
        other => panic!("Expected Api error with status 500, got: {:?}", other),
    }
    // Only called once — 500 is not retried
    mock.assert_async().await;
}

#[tokio::test]
async fn server_502_not_retried() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(502)
        .with_body("Bad Gateway")
        .expect(1)
        .create_async()
        .await;

    let clob = test_public_clob_fast_retry(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Api { status, .. }) => {
            assert_eq!(status, 502);
        }
        other => panic!("Expected Api error with status 502, got: {:?}", other),
    }
    mock.assert_async().await;
}

// ── Error scenario & edge case tests ──

#[tokio::test]
async fn error_400_returns_validation_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "invalid parameters"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Validation(msg)) => {
            assert_eq!(msg, "invalid parameters");
        }
        other => panic!("Expected Validation error, got: {:?}", other),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn error_403_returns_authentication_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(403)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error": "forbidden"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Authentication(msg)) => {
            assert_eq!(msg, "forbidden");
        }
        other => panic!("Expected Authentication error, got: {:?}", other),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn error_message_field_fallback() {
    let mut server = Server::new_async().await;

    // JSON body with "message" instead of "error"
    let mock = server
        .mock("GET", "/time")
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message": "something broke"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Api { status, message }) => {
            assert_eq!(status, 500);
            assert_eq!(message, "something broke");
        }
        other => panic!("Expected Api error, got: {:?}", other),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn error_html_body_uses_raw_text() {
    let mut server = Server::new_async().await;

    // Non-JSON body (e.g. HTML error page from a proxy)
    let mock = server
        .mock("GET", "/time")
        .with_status(503)
        .with_header("content-type", "text/html")
        .with_body("<html><body>Service Unavailable</body></html>")
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    match err {
        ClobError::Api(polyoxide_core::ApiError::Api { status, message }) => {
            assert_eq!(status, 503);
            assert!(message.contains("Service Unavailable"));
        }
        other => panic!("Expected Api error, got: {:?}", other),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn error_empty_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(500)
        .with_body("")
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    // Empty body should still produce an error, not panic
    assert!(matches!(
        err,
        ClobError::Api(polyoxide_core::ApiError::Api { status: 500, .. })
    ));
    mock.assert_async().await;
}

#[tokio::test]
async fn deserialization_failure_on_malformed_json() {
    let mut server = Server::new_async().await;

    // Status 200 but body is invalid JSON for the expected type
    let mock = server
        .mock("GET", "/time")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"not_the_right_field": true}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    assert!(
        matches!(
            err,
            ClobError::Api(polyoxide_core::ApiError::Serialization(_))
        ),
        "Expected Serialization error, got: {:?}",
        err
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn error_408_returns_timeout() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/time")
        .with_status(408)
        .with_body("Request Timeout")
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let err = clob.health().server_time().send().await.unwrap_err();

    assert!(
        matches!(err, ClobError::Api(polyoxide_core::ApiError::Timeout)),
        "Expected Timeout error, got: {:?}",
        err
    );
    mock.assert_async().await;
}

// ── Order creation flow tests ──

#[tokio::test]
async fn create_order_fetches_metadata_and_builds_order() {
    let mut server = Server::new_async().await;

    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .create_async()
        .await;

    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    // V2 order signing no longer fetches /fee-rate; assert it is NOT called.
    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .expect(0)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: "0xtoken".into(),
        price: 0.55,
        size: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    };

    let order = clob.create_order(&params, None).await.unwrap();

    // Verify order fields
    assert_eq!(order.token_id, "0xtoken");
    assert_eq!(order.side, polyoxide_clob::OrderSide::Buy);
    assert!(!order.neg_risk);
    // V2: builder code defaults to zero, timestamp is populated
    assert_eq!(order.builder, alloy::primitives::B256::ZERO);
    assert!(order.timestamp.parse::<u128>().unwrap() > 0);
    // Buy: maker_amount = cost (55 * 10^6), taker_amount = shares (100 * 10^6)
    assert_eq!(order.maker_amount, "55000000");
    assert_eq!(order.taker_amount, "100000000");

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    fee_rate_mock.assert_async().await;
}

#[tokio::test]
async fn create_order_stamps_configured_builder_code() {
    let mut server = Server::new_async().await;

    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .create_async()
        .await;

    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    // A non-zero builder code configured on the client must reach the built order.
    let code = alloy::primitives::B256::from([0x11u8; 32]);
    let clob = test_authed_clob_with_builder_code(&server, code);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: "0xtoken".into(),
        price: 0.55,
        size: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    };

    let order = clob.create_order(&params, None).await.unwrap();

    assert_eq!(
        order.builder, code,
        "configured builder_code must reach the order"
    );
    // metadata is independent of builder_code and stays zero.
    assert_eq!(order.metadata, alloy::primitives::B256::ZERO);

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
}

#[tokio::test]
async fn place_order_signs_and_submits_v2_body_with_builder_code() {
    // Full offline path: build (create_order) -> sign (sign_order) -> submit (post_order),
    // driven via place_order. Asserts the code-produced (not hand-built) order flows
    // through to a coherent V2 `POST /order` body that carries the configured builder code,
    // the V2-only fields (expiration/timestamp/metadata/builder), a non-empty signature, and
    // none of the dropped V1 fields (taker/nonce/feeRateBps).
    let mut server = Server::new_async().await;

    // Numeric token id: signing parses token_id as a base-10 U256 (it is part of the
    // signed struct), so an "0x..."-style id cannot be signed.
    const TOKEN_ID: &str = "100";

    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN_ID.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .create_async()
        .await;

    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), TOKEN_ID.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    // The builder code configured on the client must appear in the submitted order's
    // `builder` field as a 66-char (0x + 64 hex) string.
    let code = alloy::primitives::B256::from([0x11u8; 32]);
    let code_hex = format!("0x{}", hex::encode(code.as_slice()));
    assert_eq!(code_hex.len(), 66);

    // POST /order body must:
    //   - nest the order under `order` with `builder` == configured code,
    //   - carry a non-empty `signature` (0x-prefixed hex from the EIP-712 signer),
    //   - include the V2-only fields expiration/timestamp/metadata,
    //   - and NOT include the dropped V1 fields taker/nonce/feeRateBps.
    let post_mock = server
        .mock("POST", "/order")
        .match_header("POLY_API_KEY", "test-key")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(r#"{{"order": {{"builder": "{code_hex}"}}}}"#)),
            // `order.signature` present and non-empty (0x followed by >= 1 hex char).
            Matcher::Regex(r#""signature"\s*:\s*"0x[0-9a-fA-F]+""#.into()),
            // V2-only wire fields present inside the nested order object.
            Matcher::Regex(r#""expiration""#.into()),
            Matcher::Regex(r#""timestamp""#.into()),
            Matcher::Regex(r#""metadata""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"success":true,"orderID":"0xabc"}"#)
        .expect(1)
        .create_async()
        .await;

    let clob = test_authed_clob_with_builder_code(&server, code);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: TOKEN_ID.into(),
        price: 0.55,
        size: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    };

    // Exercise the whole path: create_order -> sign_order -> post_order.
    let order = clob.create_order(&params, None).await.unwrap();
    let signed = clob.sign_order(&order).await.unwrap();
    let resp = clob
        .post_order(&signed, polyoxide_clob::OrderKind::Gtc, false)
        .await
        .unwrap();

    assert!(resp.success);
    assert_eq!(resp.order_id.as_deref(), Some("0xabc"));

    // Defense-in-depth: serialize the actually-signed order ourselves and confirm the
    // wire shape directly (the V1 fields are gone, the builder code carried through).
    let body = serde_json::to_value(&signed).unwrap();
    assert_eq!(body["builder"].as_str().unwrap(), code_hex);
    assert!(body["signature"].as_str().unwrap().starts_with("0x"));
    assert!(body["signature"].as_str().unwrap().len() > 2);
    for v2_field in ["expiration", "timestamp", "metadata"] {
        assert!(body.get(v2_field).is_some(), "missing V2 field {v2_field}");
    }
    for v1_field in ["taker", "nonce", "feeRateBps"] {
        assert!(
            body.get(v1_field).is_none(),
            "V1 field {v1_field} must be gone"
        );
    }

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    post_mock.assert_async().await;
}

#[tokio::test]
async fn create_order_rejects_poly1271_without_network_io() {
    // The Poly1271 guard fires before any market-metadata I/O, so no endpoints are mocked.
    // expect(0) on the metadata endpoints proves nothing was fetched.
    let mut server = Server::new_async().await;
    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .expect(0)
        .create_async()
        .await;
    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .expect(0)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: "0xtoken".into(),
        price: 0.55,
        size: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: Some(SignatureType::Poly1271),
    };

    let err = clob.create_order(&params, None).await.unwrap_err();
    assert!(
        err.to_string().contains("Poly1271"),
        "expected a Poly1271 rejection, got: {err}"
    );

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
}

#[tokio::test]
async fn create_order_with_provided_options_skips_metadata_fetch() {
    let mut server = Server::new_async().await;

    // neg_risk and tick_size are provided via options, and V2 no longer fetches /fee-rate,
    // so NO market-metadata endpoints should be called.
    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .expect(0)
        .create_async()
        .await;

    // These should NOT be called
    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .expect(0)
        .create_async()
        .await;
    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .expect(0)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: "0xtoken".into(),
        price: 0.50,
        size: 200.0,
        side: polyoxide_clob::OrderSide::Sell,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: Some(9999999999),
        funder: None,
        signature_type: None,
    };

    let options = polyoxide_clob::PartialCreateOrderOptions {
        neg_risk: Some(true),
        tick_size: Some(polyoxide_clob::TickSize::Hundredth),
    };

    let order = clob.create_order(&params, Some(options)).await.unwrap();

    assert_eq!(order.side, polyoxide_clob::OrderSide::Sell);
    assert!(order.neg_risk);
    // Sell: maker_amount = shares (200 * 10^6), taker_amount = cost (100 * 10^6)
    assert_eq!(order.maker_amount, "200000000");
    assert_eq!(order.taker_amount, "100000000");
    assert_eq!(order.expiration, "9999999999");

    fee_rate_mock.assert_async().await;
    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
}

/// Market orders carry the same floor, and reject before any network I/O.
///
/// The supplied leg is USDC for a buy and shares for a sell, both capped at two
/// decimals, so `0.005` truncates away either way. The mocks assert zero calls:
/// a size the venue cannot express should not cost a round trip to discover.
#[tokio::test]
async fn create_market_order_rejects_amount_below_venue_minimum() {
    let mut server = Server::new_async().await;

    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .expect(0)
        .create_async()
        .await;
    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .expect(0)
        .create_async()
        .await;
    let book_mock = server.mock("GET", "/book").expect(0).create_async().await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::types::MarketOrderArgs {
        token_id: "0xtoken".into(),
        amount: 0.005,
        side: polyoxide_clob::OrderSide::Buy,
        price: Some(0.50),
        fee_rate_bps: None,
        nonce: None,
        funder: None,
        signature_type: None,
        order_type: None,
    };

    let err = clob.create_market_order(&params, None).await.unwrap_err();
    assert!(
        err.to_string().contains("0.01"),
        "error should name the floor, got: {err}"
    );

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    book_mock.assert_async().await;
}

/// An over-precise size must reach the wire truncated to the venue's limit.
///
/// `18.181818` shares is what "$10 at 0.55" comes to, so it is the ordinary
/// output of sizing by budget. The venue caps sizes at 2 decimals and derives
/// the price as `makerAmount / takerAmount`, so the legs that leave here must
/// be `18.18` shares for `9.999` USDC — anything else is signed, submitted, and
/// rejected. This covers the path through `create_order`, not just the helper.
#[tokio::test]
async fn create_order_truncates_over_precise_size_to_tick_limits() {
    let server = Server::new_async().await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: "0xtoken".into(),
        price: 0.55,
        size: 18.181818,
        side: polyoxide_clob::OrderSide::Buy,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    };

    let options = polyoxide_clob::PartialCreateOrderOptions {
        neg_risk: Some(false),
        tick_size: Some(polyoxide_clob::TickSize::Hundredth),
    };

    let order = clob.create_order(&params, Some(options)).await.unwrap();

    // Buy: maker_amount = cost, taker_amount = shares.
    assert_eq!(order.taker_amount, "18180000", "18.181818 → 18.18 shares");
    assert_eq!(order.maker_amount, "9999000", "0.55 * 18.18 = 9.999 USDC");

    // The venue's own check: the legs divide back to exactly 0.55.
    let cost: i64 = order.maker_amount.parse().unwrap();
    let shares: i64 = order.taker_amount.parse().unwrap();
    assert_eq!(cost * 100, 55 * shares, "legs must imply a price of 0.55");
}

#[tokio::test]
async fn create_order_without_account_errors() {
    let server = Server::new_async().await;

    let clob = test_public_clob(&server);
    let params = polyoxide_clob::CreateOrderParams {
        token_id: "0xtoken".into(),
        price: 0.50,
        size: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        order_type: polyoxide_clob::OrderKind::Gtc,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    };

    let err = clob.create_order(&params, None).await.unwrap_err();
    assert!(
        matches!(err, ClobError::Api(polyoxide_core::ApiError::Validation(_))),
        "Expected Validation error for missing account, got: {:?}",
        err
    );
}

#[tokio::test]
async fn create_market_order_with_explicit_price() {
    let mut server = Server::new_async().await;

    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": true}"#)
        .create_async()
        .await;

    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    // V2 order signing no longer fetches /fee-rate; assert it is NOT called.
    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .expect(0)
        .create_async()
        .await;

    // Order book should NOT be called when price is provided
    let book_mock = server.mock("GET", "/book").expect(0).create_async().await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::types::MarketOrderArgs {
        token_id: "0xtoken".into(),
        amount: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        price: Some(0.50),
        fee_rate_bps: None,
        nonce: None,
        funder: None,
        signature_type: None,
        order_type: None,
    };

    let order = clob.create_market_order(&params, None).await.unwrap();

    assert_eq!(order.token_id, "0xtoken");
    assert!(order.neg_risk);
    assert_eq!(order.expiration, "0");
    assert_eq!(order.maker_amount, "100000000");
    assert_eq!(order.taker_amount, "200000000");

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    fee_rate_mock.assert_async().await;
    book_mock.assert_async().await;
}

#[tokio::test]
async fn create_market_order_fetches_orderbook_for_price() {
    let mut server = Server::new_async().await;

    let neg_risk_mock = server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .create_async()
        .await;

    let tick_size_mock = server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    // V2 order signing no longer fetches /fee-rate; assert it is NOT called.
    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .expect(0)
        .create_async()
        .await;

    // Order book IS fetched when price is None
    let book_mock = server
        .mock("GET", "/book")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "market": "0xcond",
                "asset_id": "0xtoken",
                "bids": [{"price": "0.48", "size": "500"}],
                "asks": [{"price": "0.52", "size": "500"}],
                "timestamp": "1700000000",
                "hash": "abc123"
            }"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let params = polyoxide_clob::types::MarketOrderArgs {
        token_id: "0xtoken".into(),
        amount: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        price: None, // will be fetched from orderbook asks
        fee_rate_bps: None,
        nonce: None,
        funder: None,
        signature_type: None,
        order_type: None,
    };

    let order = clob.create_market_order(&params, None).await.unwrap();

    assert_eq!(order.token_id, "0xtoken");
    assert_eq!(order.expiration, "0");
    assert!(!order.neg_risk);
    assert_eq!(order.side, polyoxide_clob::OrderSide::Buy);
    // Buy market order: price calculated from asks = 0.52
    // maker_amount = USDC (100 * 10^6), taker_amount = shares (100/0.52 truncated * 10^6)
    let maker_val: u64 = order.maker_amount.parse().unwrap();
    let taker_val: u64 = order.taker_amount.parse().unwrap();
    assert_eq!(maker_val, 100_000_000); // 100 USDC
    assert!(taker_val > 192_000_000, "taker should be ~192.3 shares"); // 100/0.52 ≈ 192.3

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    fee_rate_mock.assert_async().await;
    book_mock.assert_async().await;
}

// ── New path-parameter variants (OpenAPI parity) ─────────────────

#[tokio::test]
async fn fee_rate_path_variant() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/fee-rate/0xtoken")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"base_fee": 30}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .fee_rate_path("0xtoken")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.base_fee, 30);
    mock.assert_async().await;
}

#[tokio::test]
async fn tick_size_path_variant() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/tick-size/0xtoken")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .tick_size_path("0xtoken")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.minimum_tick_size, "0.01");
    mock.assert_async().await;
}

#[tokio::test]
async fn neg_risk_path_variant() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/neg-risk/0xtoken")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .neg_risk_path("0xtoken")
        .send()
        .await
        .unwrap();
    assert!(!resp.neg_risk);
    mock.assert_async().await;
}

#[tokio::test]
async fn clob_market_details_returns_abbreviated_shape() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/clob-markets/0xcondition")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "gst": null,
                "r": {},
                "t": [
                    {"t": "713210456", "o": "Yes"},
                    {"t": "521143195", "o": "No"}
                ],
                "mos": 5.0,
                "mts": 0.01,
                "mbf": 0,
                "tbf": 0,
                "rfqe": false,
                "itode": false,
                "ibce": true,
                "fd": {"r": null, "e": null, "to": null},
                "oas": 0
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let details = clob
        .markets()
        .clob_market_details("0xcondition")
        .send()
        .await
        .unwrap();

    assert_eq!(details.t.len(), 2);
    assert_eq!(details.t[0].t, "713210456");
    assert_eq!(details.t[0].o, "Yes");
    assert_eq!(details.rfqe, Some(false));
    assert!(details.ibce);
    mock.assert_async().await;
}

#[tokio::test]
async fn market_by_token_returns_both_token_ids() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets-by-token/0xtoken")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "condition_id": "0xcondition",
                "primary_token_id": "0xprimary",
                "secondary_token_id": "0xsecondary"
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .market_by_token("0xtoken")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.condition_id, "0xcondition");
    assert_eq!(resp.primary_token_id, "0xprimary");
    assert_eq!(resp.secondary_token_id, "0xsecondary");
    mock.assert_async().await;
}

#[tokio::test]
async fn live_activity_market_returns_single_market() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/markets/live-activity/0xcondition")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "condition_id": "0xcondition",
                "id": 42,
                "question": "Will X happen?",
                "market_slug": "will-x-happen",
                "event_slug": "x-event",
                "series_slug": null,
                "icon": "",
                "image": "",
                "tags": ["crypto"]
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .live_activity_market("0xcondition")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.id, Some(42));
    assert_eq!(resp.question.as_deref(), Some("Will X happen?"));
    assert_eq!(resp.tags, vec!["crypto"]);
    mock.assert_async().await;
}

#[tokio::test]
async fn live_activity_bulk_sends_array_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/markets/live-activity")
        .match_body(Matcher::JsonString(r#"["0xcond-1", "0xcond-2"]"#.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[
                {"condition_id": "0xcond-1", "id": 1, "question": "Q1", "tags": []},
                {"condition_id": "0xcond-2", "id": 2, "question": "Q2", "tags": []}
            ]"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob
        .markets()
        .live_activity_bulk(vec!["0xcond-1".into(), "0xcond-2".into()])
        .unwrap()
        .send()
        .await
        .unwrap();

    assert_eq!(resp.len(), 2);
    assert_eq!(resp[0].condition_id.as_deref(), Some("0xcond-1"));
    assert_eq!(resp[1].id, Some(2));
    mock.assert_async().await;
}

#[tokio::test]
async fn batch_prices_history_sends_request_body() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/batch-prices-history")
        .match_body(Matcher::JsonString(
            r#"{"markets": ["0xa", "0xb"], "interval": "1d"}"#.into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "history": {
                    "0xa": [{"t": 1700000000, "p": 0.55}],
                    "0xb": [{"t": 1700000000, "p": 0.30}]
                }
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let req = polyoxide_clob::BatchPricesHistoryRequest {
        markets: vec!["0xa".into(), "0xb".into()],
        interval: Some("1d".into()),
        ..Default::default()
    };
    let resp = clob
        .markets()
        .batch_prices_history(&req)
        .unwrap()
        .send()
        .await
        .unwrap();

    assert_eq!(resp.history.len(), 2);
    assert_eq!(resp.history.get("0xa").unwrap()[0].t, 1_700_000_000);
    mock.assert_async().await;
}

#[tokio::test]
async fn update_balance_allowance_gets_balance_allowance_update_with_query() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/balance-allowance/update")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("asset_type".into(), "COLLATERAL".into()),
            Matcher::UrlEncoded("token_id".into(), "0xtoken".into()),
            Matcher::UrlEncoded("signature_type".into(), "1".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .update_balance_allowance("COLLATERAL", Some("0xtoken".into()), Some(1))
        .await
        .unwrap();
    assert!(resp.is_object());
    mock.assert_async().await;
}

#[tokio::test]
async fn update_balance_allowance_omits_optional_query_params() {
    let mut server = Server::new_async().await;

    // Without token_id/signature_type, only asset_type should appear in the query string.
    let mock = server
        .mock("GET", "/balance-allowance/update")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::UrlEncoded(
            "asset_type".into(),
            "CONDITIONAL".into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let _resp = clob
        .account_api()
        .unwrap()
        .update_balance_allowance("CONDITIONAL", None, None)
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn update_balance_allowance_tolerates_empty_body() {
    let mut server = Server::new_async().await;

    // The live endpoint returns 200 with an empty body; that must be treated as
    // success rather than an "EOF while parsing a value" deserialization error.
    let mock = server
        .mock("GET", "/balance-allowance/update")
        .match_header("POLY_API_KEY", "test-key")
        .match_query(Matcher::UrlEncoded(
            "asset_type".into(),
            "COLLATERAL".into(),
        ))
        .with_status(200)
        .with_body("")
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob
        .account_api()
        .unwrap()
        .update_balance_allowance("COLLATERAL", None, None)
        .await
        .unwrap();
    assert!(resp.is_null());
    mock.assert_async().await;
}

#[tokio::test]
async fn notifications_list_sends_configured_signature_type() {
    let mut server = Server::new_async().await;
    // A proxy-configured client must thread its signature_type into the
    // (otherwise required-param-less) /notifications request.
    let clob = test_authed_clob_with_sig(&server, SignatureType::PolyProxy);

    let mock = server
        .mock("GET", "/notifications")
        .match_query(Matcher::UrlEncoded("signature_type".into(), "1".into()))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let notifications = clob.notifications().unwrap().list().send().await.unwrap();
    assert!(notifications.is_empty());
    mock.assert_async().await;
}

/// Deserialize a *non-empty* `/notifications` body.
///
/// The test above serves `[]`, which is right for what it checks (that
/// `signature_type` is threaded through) but means it never exercises the row
/// type at all — an empty array deserializes cleanly no matter how
/// `Notification` is declared. That blind spot is why `id` sat wrongly typed as
/// a `String` while every test passed. The body below mirrors a real response.
#[tokio::test]
async fn notifications_list_deserializes_a_real_row() {
    let mut server = Server::new_async().await;
    let clob = test_authed_clob_with_sig(&server, SignatureType::PolyProxy);

    let mock = server
        .mock("GET", "/notifications")
        .match_query(Matcher::UrlEncoded("signature_type".into(), "1".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "id": 1390056400,
                "type": 2,
                "owner": "aa17dfae-754d-2498-f336-8bd1d0e6a1c3",
                "payload": {"orderId": "0xabc", "outcome": "Yes"},
                "timestamp": "2026-08-03T14:54:12.384486Z"
            }]"#,
        )
        .create_async()
        .await;

    let notifications = clob.notifications().unwrap().list().send().await.unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].id, 1_390_056_400);
    assert_eq!(notifications[0].notification_type, 2);
    assert_eq!(
        notifications[0].timestamp.as_deref(),
        Some("2026-08-03T14:54:12.384486Z")
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn reward_earnings_sends_date_and_signature_type() {
    let mut server = Server::new_async().await;
    let clob = test_authed_clob(&server); // default signature_type = EOA (0)

    let mock = server
        .mock("GET", "/rewards/user")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("date".into(), "2024-01-01".into()),
            Matcher::UrlEncoded("signature_type".into(), "0".into()),
        ]))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": "LTE=", "limit": 100, "count": 0}"#)
        .create_async()
        .await;

    let _resp = clob
        .rewards()
        .unwrap()
        .earnings("2024-01-01")
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn reward_percentages_sends_signature_type() {
    let mut server = Server::new_async().await;
    // Proxy-configured client: percentages must carry the threaded signature_type.
    let clob = test_authed_clob_with_sig(&server, SignatureType::PolyProxy);

    let mock = server
        .mock("GET", "/rewards/user/percentages")
        .match_query(Matcher::UrlEncoded("signature_type".into(), "1".into()))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"maker": "0.5", "taker": "0.3"}"#)
        .create_async()
        .await;

    let _resp = clob.rewards().unwrap().percentages().send().await.unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn reward_market_earnings_sends_signature_type() {
    let mut server = Server::new_async().await;
    let clob = test_authed_clob(&server); // default signature_type = EOA (0)

    let mock = server
        .mock("GET", "/rewards/user/markets")
        .match_query(Matcher::UrlEncoded("signature_type".into(), "0".into()))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": "LTE=", "limit": 100, "count": 0}"#)
        .create_async()
        .await;

    let _resp = clob
        .rewards()
        .unwrap()
        .market_earnings()
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn reward_total_earnings_sends_date_and_signature_type() {
    let mut server = Server::new_async().await;
    let clob = test_authed_clob(&server); // default signature_type = EOA (0)

    let mock = server
        .mock("GET", "/rewards/user/total")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("date".into(), "2024-01-01".into()),
            Matcher::UrlEncoded("signature_type".into(), "0".into()),
        ]))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"asset_address": "0xabc", "total": "0"}]"#)
        .create_async()
        .await;

    let _resp = clob
        .rewards()
        .unwrap()
        .total_earnings("2024-01-01")
        .send()
        .await
        .unwrap();
    mock.assert_async().await;
}

#[tokio::test]
async fn usdc_balance_threads_configured_signature_type() {
    let mut server = Server::new_async().await;
    // Proxy-configured client: usdc_balance must send the configured signature
    // type (previously hardcoded to 1), proving AccountApi threads it.
    let clob = test_authed_clob_with_sig(&server, SignatureType::PolyProxy);

    let mock = server
        .mock("GET", "/balance-allowance")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("asset_type".into(), "COLLATERAL".into()),
            Matcher::UrlEncoded("signature_type".into(), "1".into()),
        ]))
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"balance": "0", "allowances": {}}"#)
        .create_async()
        .await;

    let resp = clob
        .account_api()
        .unwrap()
        .usdc_balance()
        .send()
        .await
        .unwrap();
    assert_eq!(resp.balance, "0");
    mock.assert_async().await;
}

#[tokio::test]
async fn heartbeat_posts_to_heartbeats_with_l2_auth() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/heartbeats")
        .match_header("POLY_API_KEY", "test-key")
        .match_header("POLY_PASSPHRASE", "test-pass")
        .match_header(
            "POLY_ADDRESS",
            Matcher::Regex(r"^0x[0-9a-fA-F]{40}$".into()),
        )
        .match_header("POLY_SIGNATURE", Matcher::Any)
        .match_header("POLY_TIMESTAMP", Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"status":"ok"}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let resp = clob.account_api().unwrap().heartbeat().await.unwrap();

    assert_eq!(resp.status, "ok");
    mock.assert_async().await;
}

#[tokio::test]
async fn heartbeat_propagates_http_error() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("POST", "/heartbeats")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"unauthorized"}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let err = clob.account_api().unwrap().heartbeat().await.unwrap_err();
    assert!(
        matches!(err, ClobError::Api(_)),
        "Expected Api error, got: {:?}",
        err
    );
    mock.assert_async().await;
}

#[tokio::test]
async fn create_market_order_insufficient_liquidity() {
    let mut server = Server::new_async().await;

    // Order book with very little liquidity
    let _book_mock = server
        .mock("GET", "/book")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "market": "0xcond",
                "asset_id": "0xtoken",
                "bids": [],
                "asks": [{"price": "0.50", "size": "1"}],
                "timestamp": "1700000000",
                "hash": "abc123"
            }"#,
        )
        .create_async()
        .await;

    let clob = test_authed_clob(&server);

    let options = polyoxide_clob::PartialCreateOrderOptions {
        neg_risk: Some(false),
        tick_size: Some(polyoxide_clob::TickSize::Hundredth),
    };

    let params = polyoxide_clob::types::MarketOrderArgs {
        token_id: "0xtoken".into(),
        amount: 1000.0, // way more than available
        side: polyoxide_clob::OrderSide::Buy,
        price: None,
        fee_rate_bps: None,
        nonce: None,
        funder: None,
        signature_type: None,
        order_type: None,
    };

    let err = clob
        .create_market_order(&params, Some(options))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ClobError::Api(polyoxide_core::ApiError::Validation(_))),
        "Expected Validation error for insufficient liquidity, got: {:?}",
        err
    );
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

    let clob = test_public_clob(&server);
    let err = clob.health().ping().await.unwrap_err();
    assert!(
        matches!(err, ClobError::Api(_)),
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

    let clob = test_public_clob(&server);
    let err = clob.health().ping().await.unwrap_err();
    assert!(
        matches!(err, ClobError::Api(_)),
        "expected ApiError for 5xx, got {err:?}"
    );

    mock.assert_async().await;
}

#[tokio::test]
async fn rewards_multi_markets_with_filters() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/rewards/markets/multi")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("q".into(), "election".into()),
            Matcher::UrlEncoded("tag_slug".into(), "politics".into()),
            Matcher::UrlEncoded("order_by".into(), "volume_24hr".into()),
            Matcher::UrlEncoded("position".into(), "DESC".into()),
            Matcher::UrlEncoded("min_volume_24hr".into(), "1000".into()),
            Matcher::UrlEncoded("page_size".into(), "50".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "limit": 50,
                "count": 1,
                "next_cursor": "LTE=",
                "data": [{"condition_id": "0xabc", "rewards_max_spread": 3.5}]
            }"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let page = clob
        .public_rewards()
        .multi_markets()
        .query_text("election")
        .tag_slug("politics")
        .order_by(MultiMarketOrderBy::Volume24hr)
        .position(SortPosition::Desc)
        .min_volume_24hr(1000.0)
        .page_size(50)
        .send()
        .await
        .unwrap();

    assert_eq!(page.data.len(), 1);
    assert_eq!(page.next_cursor.as_deref(), Some("LTE="));
    assert_eq!(page.data[0].data["condition_id"], "0xabc");

    mock.assert_async().await;
}

#[tokio::test]
async fn current_rebates_returns_typed_rows() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/rebates/current")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("date".into(), "2026-02-27".into()),
            Matcher::UrlEncoded("maker_address".into(), "0xFeA4".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "date": "2026-02-27",
                "condition_id": "0xbd31",
                "asset_address": "0xC011",
                "maker_address": "0xFeA4",
                "rebated_fees_usdc": "0.237519"
            }]"#,
        )
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let rebates = clob
        .public_rewards()
        .current_rebates("2026-02-27", "0xFeA4")
        .send()
        .await
        .unwrap();

    assert_eq!(rebates.len(), 1);
    assert_eq!(rebates[0].rebated_fees_usdc, "0.237519");
    assert_eq!(rebates[0].condition_id, "0xbd31");

    mock.assert_async().await;
}

#[tokio::test]
async fn reward_markets_current_forwards_cursor_and_sponsored() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/rewards/markets/current")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("sponsored".into(), "true".into()),
            Matcher::UrlEncoded("next_cursor".into(), "MTAw".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"limit": 500, "count": 0, "next_cursor": "LTE=", "data": []}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let page = clob
        .public_rewards()
        .current_markets()
        .sponsored(true)
        .next_cursor("MTAw")
        .send()
        .await
        .unwrap();

    assert!(page.data.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn simplified_markets_paginates_with_cursor() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/simplified-markets")
        .match_query(Matcher::UrlEncoded("next_cursor".into(), "MTAw".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": "LTE="}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let page = clob
        .markets()
        .simplified()
        .next_cursor("MTAw")
        .send()
        .await
        .unwrap();

    assert_eq!(page.next_cursor.as_deref(), Some("LTE="));
    mock.assert_async().await;
}

#[tokio::test]
async fn orders_list_forwards_filters() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/data/orders")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("market".into(), "0xcond".into()),
            Matcher::UrlEncoded("asset_id".into(), "token-1".into()),
            Matcher::UrlEncoded("next_cursor".into(), "MTAw".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [], "next_cursor": "LTE="}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let page = clob
        .orders()
        .unwrap()
        .list()
        .market("0xcond")
        .asset_id("token-1")
        .next_cursor("MTAw")
        .send()
        .await
        .unwrap();

    assert!(page.data.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn user_reward_markets_forwards_full_filter_set() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/rewards/user/markets")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("date".into(), "2026-02-27".into()),
            Matcher::UrlEncoded("maker_address".into(), "0xFeA4".into()),
            Matcher::UrlEncoded("only_open_orders".into(), "true".into()),
            Matcher::UrlEncoded("order_by".into(), "earning_percentage".into()),
            Matcher::UrlEncoded("position".into(), "ASC".into()),
            Matcher::UrlEncoded("page_size".into(), "250".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"limit": 250, "count": 0, "next_cursor": "LTE=", "data": []}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let page = clob
        .rewards()
        .unwrap()
        .market_earnings()
        .date("2026-02-27")
        .maker_address("0xFeA4")
        .only_open_orders(true)
        .order_by(UserRewardMarketOrderBy::EarningPercentage)
        .position(SortPosition::Asc)
        .page_size(250)
        .send()
        .await
        .unwrap();

    assert!(page.data.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn l1_headers_send_checksummed_address_and_nonce() {
    let mut server = Server::new_async().await;

    // Case-sensitive on purpose. The pre-existing POLY_ADDRESS assertions use
    // `(?i)` / `[0-9a-fA-F]`, so they pass against either casing and could not
    // catch this drift. py-clob-client sends eth_account's checksummed form.
    let mock = server
        .mock("POST", "/auth/api-key")
        .match_header("POLY_ADDRESS", "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
        .match_header("POLY_NONCE", "7")
        .match_header("POLY_SIGNATURE", Matcher::Any)
        .match_header("POLY_TIMESTAMP", Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"apiKey":"k","secret":"cw==","passphrase":"p"}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    clob.auth().unwrap().create_api_key(7).send().await.unwrap();

    mock.assert_async().await;
}

#[tokio::test]
async fn l1_address_header_is_not_lowercase() {
    let mut server = Server::new_async().await;

    // Negative control: the lowercase form must no longer be sent. Without
    // this, a regression to `{:?}` would still satisfy a checksummed-only
    // assertion if the matcher were ever loosened.
    let mock = server
        .mock("GET", "/auth/derive-api-key")
        .match_header("POLY_ADDRESS", "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"apiKey":"k","secret":"cw==","passphrase":"p"}"#)
        .expect(0)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    // Reaches the server but must not match the lowercase mock.
    let _ = clob.auth().unwrap().derive_api_key(0).send().await;

    mock.assert_async().await;
}

#[tokio::test]
async fn orders_list_send_raw_returns_unparsed_body() {
    let mut server = Server::new_async().await;

    // A body the typed OpenOrder cannot represent. send_raw must still hand it
    // back, so a struct/venue mismatch is recoverable rather than a hard block.
    let body = r#"{"data":[{"id":"0xabc","unexpected_field":123}],"next_cursor":"LTE="}"#;
    let mock = server
        .mock("GET", "/data/orders")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .expect(2) // one typed attempt, one raw
        .create_async()
        .await;

    let clob = test_authed_clob(&server);

    // The typed path fails on this body...
    let typed = clob.orders().unwrap().list().send().await;
    assert!(typed.is_err(), "typed parse should fail on this shape");

    // ...but the raw path still yields it verbatim.
    let raw = clob
        .orders()
        .unwrap()
        .list()
        .send_raw()
        .await
        .expect("send_raw should succeed")
        .text()
        .await
        .expect("body");
    assert_eq!(raw, body);

    mock.assert_async().await;
}

// ── FAK/FOK kill outcomes ──
//
// Polymarket reports a marketable order that the matching engine killed as HTTP
// 400 with prose in `error` — the same status it uses for malformed payloads,
// bans, and tick-size violations. See
// https://docs.polymarket.com/resources/error-codes ("Order Processing Errors").
// These are *defined* outcomes of FAK/FOK, not faults, and they are
// deterministic: re-sending cannot change the answer. They must therefore
// surface as their own error variants, must not be retried, and must not be
// confused with the neighbouring 400s.

/// Venue prose for a FAK order that found nothing to match against, verbatim.
const FAK_UNMATCHED_MSG: &str = "no orders found to match with FAK order. \
FAK orders are partially filled or killed if no match is found.";

/// Venue prose for a FOK order that could not be filled in full, verbatim.
const FOK_UNFILLED_MSG: &str =
    "order couldn't be fully filled. FOK orders are fully filled or killed.";

/// Mock the `create_order` prerequisites (`/neg-risk`, `/tick-size`) plus a
/// `POST /order` that fails with `status` and `{"error": body_msg}`.
///
/// Returns the `POST /order` mock so callers can assert the attempt count.
async fn mock_order_rejection(
    server: &mut mockito::ServerGuard,
    token_id: &str,
    status: usize,
    body_msg: &str,
) -> mockito::Mock {
    server
        .mock("GET", "/neg-risk")
        .match_query(Matcher::UrlEncoded("token_id".into(), token_id.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"neg_risk": false}"#)
        .create_async()
        .await;

    server
        .mock("GET", "/tick-size")
        .match_query(Matcher::UrlEncoded("token_id".into(), token_id.into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"minimum_tick_size": "0.01"}"#)
        .create_async()
        .await;

    server
        .mock("POST", "/order")
        .with_status(status)
        .with_header("content-type", "application/json")
        .with_body(serde_json::json!({ "error": body_msg }).to_string())
        .expect(1)
        .create_async()
        .await
}

/// A non-crossing marketable BUY: 100 shares at 1c, the repro from the report.
fn deep_otm_params(
    token_id: &str,
    order_type: polyoxide_clob::OrderKind,
) -> polyoxide_clob::CreateOrderParams {
    polyoxide_clob::CreateOrderParams {
        token_id: token_id.into(),
        price: 0.01,
        size: 100.0,
        side: polyoxide_clob::OrderSide::Buy,
        order_type,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: None,
    }
}

#[tokio::test]
async fn fak_unmatched_maps_to_typed_error_not_generic_validation() {
    let mut server = Server::new_async().await;
    const TOKEN_ID: &str = "100";

    let post_mock = mock_order_rejection(&mut server, TOKEN_ID, 400, FAK_UNMATCHED_MSG).await;

    let clob = test_authed_clob(&server);
    let err = clob
        .place_order(
            &deep_otm_params(TOKEN_ID, polyoxide_clob::OrderKind::Fak),
            None,
        )
        .await
        .unwrap_err();

    match &err {
        ClobError::FakUnmatched { message } => {
            // The venue's prose is preserved for logs, but callers never need to read it.
            assert_eq!(message, FAK_UNMATCHED_MSG);
        }
        other => panic!("Expected ClobError::FakUnmatched, got: {other:?}"),
    }

    // Deterministic: re-sending cannot change the outcome.
    assert!(
        !err.is_retriable(),
        "an unmatched FAK must not be retriable"
    );

    // Exactly one placement attempt reached the venue.
    post_mock.assert_async().await;
}

#[tokio::test]
async fn fok_unfilled_maps_to_typed_error_not_generic_validation() {
    let mut server = Server::new_async().await;
    const TOKEN_ID: &str = "100";

    let post_mock = mock_order_rejection(&mut server, TOKEN_ID, 400, FOK_UNFILLED_MSG).await;

    let clob = test_authed_clob(&server);
    let err = clob
        .place_order(
            &deep_otm_params(TOKEN_ID, polyoxide_clob::OrderKind::Fok),
            None,
        )
        .await
        .unwrap_err();

    match &err {
        ClobError::FokUnfilled { message } => assert_eq!(message, FOK_UNFILLED_MSG),
        other => panic!("Expected ClobError::FokUnfilled, got: {other:?}"),
    }

    assert!(!err.is_retriable(), "an unfilled FOK must not be retriable");
    post_mock.assert_async().await;
}

#[tokio::test]
async fn unrelated_400_still_maps_to_validation_error() {
    // Negative control. The classifier keys off the message body, so it must not
    // swallow the neighbouring 400s that genuinely are faults. Without this, a
    // sloppy match (e.g. on "order") would silently reclassify real errors.
    let mut server = Server::new_async().await;
    const TOKEN_ID: &str = "100";

    let post_mock = mock_order_rejection(
        &mut server,
        TOKEN_ID,
        400,
        "order 0xabc is invalid. Price (100) breaks minimum tick size rule: 0.1",
    )
    .await;

    let clob = test_authed_clob(&server);
    let err = clob
        .place_order(
            &deep_otm_params(TOKEN_ID, polyoxide_clob::OrderKind::Fak),
            None,
        )
        .await
        .unwrap_err();

    match &err {
        ClobError::Api(polyoxide_core::ApiError::Validation(msg)) => {
            assert!(
                msg.contains("minimum tick size"),
                "unexpected message: {msg}"
            );
        }
        other => panic!("Expected a generic Validation error, got: {other:?}"),
    }
    assert!(!err.is_retriable());
    post_mock.assert_async().await;
}

#[tokio::test]
async fn fak_prose_on_non_400_status_is_not_reclassified() {
    // The kill outcome is documented only for 400. A 500 carrying similar prose is
    // an engine fault ("order timed out" territory) and stays retriable, so the
    // classifier must be gated on the status, not on the message alone.
    let mut server = Server::new_async().await;
    const TOKEN_ID: &str = "100";

    let post_mock = mock_order_rejection(&mut server, TOKEN_ID, 500, FAK_UNMATCHED_MSG).await;

    let clob = test_authed_clob(&server);
    let err = clob
        .place_order(
            &deep_otm_params(TOKEN_ID, polyoxide_clob::OrderKind::Fak),
            None,
        )
        .await
        .unwrap_err();

    match &err {
        ClobError::Api(polyoxide_core::ApiError::Api { status, .. }) => assert_eq!(*status, 500),
        other => panic!("Expected a generic Api error with status 500, got: {other:?}"),
    }
    assert!(err.is_retriable(), "a 5xx is a transient fault");
    post_mock.assert_async().await;
}

// ── Per-signer trading rate limits ───────────────────────────────
//
// A second limiter, independent of the IP-based one: it counts *orders*, not
// requests, so a batch can cost more than the signer's bucket can ever hold.
// See docs/specs/clob/trading-rate-limits.md.

#[tokio::test]
async fn over_capacity_batch_cancel_is_rejected_without_sending_a_request() {
    // 2,000 IDs costs 2,000 cancel tokens; Standard holds 120 and even Elite
    // only 1,800. Waiting can never satisfy it, so the client must refuse
    // locally rather than let the venue answer 429 — which the retry loop
    // would misread as transient and burn three attempts on.
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/orders")
        .with_status(200)
        .with_body(r#"{"canceled": [], "notCanceled": {}}"#)
        .expect(0) // must never be reached
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let ids: Vec<String> = (0..2_000).map(|i| format!("order-{i}")).collect();

    let err = clob
        .orders()
        .unwrap()
        .cancel_many(ids)
        .await
        .expect_err("an over-capacity batch must be refused");

    match &err {
        polyoxide_clob::ClobError::BurstCapacityExceeded(e) => {
            assert_eq!(e.cost, 2_000);
            assert_eq!(e.capacity, 120, "Standard tier cancel burst");
        }
        other => panic!("expected BurstCapacityExceeded, got {other:?}"),
    }
    assert!(!err.is_retriable(), "splitting is the only remedy");

    // The decisive assertion: nothing went over the wire.
    mock.assert_async().await;
}

#[tokio::test]
async fn a_batch_within_capacity_still_reaches_the_venue() {
    // The guard must not be over-eager: 100 fits Standard's 120 cancel burst.
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("DELETE", "/orders")
        .with_status(200)
        .with_body(r#"{"canceled": [], "notCanceled": {}}"#)
        .expect(1)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let ids: Vec<String> = (0..100).map(|i| format!("order-{i}")).collect();
    clob.orders().unwrap().cancel_many(ids).await.unwrap();

    mock.assert_async().await;
}

#[tokio::test]
async fn the_tier_is_adopted_from_the_response_header() {
    // Tier derives from 30-day volume the client cannot compute, so the header
    // is the only source. Before any trading request it must read Standard.
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("DELETE", "/order")
        .with_status(200)
        .with_header("poly-ratelimit-tier", "gold")
        .with_header("poly-ratelimit-remaining", "1183")
        .with_body(r#"{"canceled": ["order-1"], "notCanceled": {}}"#)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    assert_eq!(clob.tier(), polyoxide_core::Tier::Standard);
    assert_eq!(clob.rate_limit_status().tier, None);

    clob.orders()
        .unwrap()
        .cancel("order-1")
        .send()
        .await
        .unwrap();

    assert_eq!(clob.tier(), polyoxide_core::Tier::Gold);
    assert_eq!(clob.rate_limit_status().remaining, Some(1_183));
}

#[tokio::test]
async fn adopting_a_higher_tier_admits_a_previously_impossible_batch() {
    // Proves discovery actually resizes the buckets rather than just recording
    // a label: 500 IDs is impossible at Standard (120) and fine at Gold (1,200).
    let mut server = mockito::Server::new_async().await;
    let _cancel_one = server
        .mock("DELETE", "/order")
        .with_status(200)
        .with_header("poly-ratelimit-tier", "gold")
        .with_body(r#"{"canceled": ["order-1"], "notCanceled": {}}"#)
        .create_async()
        .await;
    let batch = server
        .mock("DELETE", "/orders")
        .with_status(200)
        .with_body(r#"{"canceled": [], "notCanceled": {}}"#)
        .expect(1)
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let ids: Vec<String> = (0..500).map(|i| format!("order-{i}")).collect();

    assert!(
        clob.orders()
            .unwrap()
            .cancel_many(ids.clone())
            .await
            .is_err(),
        "500 exceeds Standard's 120 cancel burst"
    );

    // Learn the tier from an unrelated trading response, then retry.
    clob.orders()
        .unwrap()
        .cancel("order-1")
        .send()
        .await
        .unwrap();
    clob.orders().unwrap().cancel_many(ids).await.unwrap();

    batch.assert_async().await;
}
