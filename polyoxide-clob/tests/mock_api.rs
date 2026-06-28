use mockito::{Matcher, Server};
use polyoxide_clob::{Account, ClobBuilder, ClobError, Credentials, SignatureType};
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
async fn get_order_flatten_rename() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/data/order/order-123")
        .match_header("POLY_API_KEY", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
                "id": "order-123",
                "market": "0xcond",
                "assetId": "0xtoken",
                "salt": "999",
                "maker": "0x0000000000000000000000000000000000000001",
                "signer": "0x0000000000000000000000000000000000000002",
                "taker": "0x0000000000000000000000000000000000000000",
                "tokenId": "0xtoken",
                "makerAmount": "1000",
                "takerAmount": "500",
                "expiration": "0",
                "nonce": "0",
                "feeRateBps": "100",
                "side": "BUY",
                "signatureType": 0,
                "signature": "0xsig",
                "status": "LIVE",
                "owner": "0xowner",
                "makerAddress": "0xmaker",
                "originalSize": "200.5",
                "sizeMatched": "100.0",
                "price": "0.55",
                "associateTrades": ["trade-1"],
                "outcome": "Yes",
                "orderType": "GTC",
                "createdAt": "2024-01-01T00:00:00Z",
                "updatedAt": "2024-01-02T00:00:00Z"
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
    // Flattened SignedOrder fields
    assert_eq!(order.order.signature, "0xsig");
    assert_eq!(order.order.order.maker_amount, "1000");
    // camelCase rename fields
    assert_eq!(order.owner.as_deref(), Some("0xowner"));
    assert_eq!(order.maker_address.as_deref(), Some("0xmaker"));
    assert_eq!(order.original_size.as_deref(), Some("200.5"));
    assert_eq!(order.size_matched.as_deref(), Some("100.0"));
    assert_eq!(order.associate_trades, vec!["trade-1"]);
    assert_eq!(order.order_type.as_deref(), Some("GTC"));
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

    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"base_fee": 100}"#)
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
    assert_eq!(order.fee_rate_bps, "100");
    assert!(!order.neg_risk);
    // Buy: maker_amount = cost (55 * 10^6), taker_amount = shares (100 * 10^6)
    assert_eq!(order.maker_amount, "55000000");
    assert_eq!(order.taker_amount, "100000000");

    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
    fee_rate_mock.assert_async().await;
}

#[tokio::test]
async fn create_order_with_provided_options_skips_metadata_fetch() {
    let mut server = Server::new_async().await;

    // Only fee_rate should be fetched — neg_risk and tick_size provided via options
    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"base_fee": 50}"#)
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
    assert_eq!(order.fee_rate_bps, "50");
    // Sell: maker_amount = shares (200 * 10^6), taker_amount = cost (100 * 10^6)
    assert_eq!(order.maker_amount, "200000000");
    assert_eq!(order.taker_amount, "100000000");
    assert_eq!(order.expiration, "9999999999");

    fee_rate_mock.assert_async().await;
    neg_risk_mock.assert_async().await;
    tick_size_mock.assert_async().await;
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

    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"base_fee": 100}"#)
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

    let fee_rate_mock = server
        .mock("GET", "/fee-rate")
        .match_query(Matcher::UrlEncoded("token_id".into(), "0xtoken".into()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"base_fee": 100}"#)
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
    assert_eq!(order.fee_rate_bps, "100");
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
