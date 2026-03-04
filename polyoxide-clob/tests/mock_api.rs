use mockito::{Matcher, Server};
use polyoxide_clob::{Account, ClobBuilder, ClobError, Credentials};

fn test_public_clob(server: &mockito::ServerGuard) -> polyoxide_clob::Clob {
    ClobBuilder::new().base_url(server.url()).build().unwrap()
}

fn test_authed_clob(server: &mockito::ServerGuard) -> polyoxide_clob::Clob {
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
        .trades()
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
        .with_body(r#"{"balance": "1000.50", "allowance": "999999"}"#)
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
    assert_eq!(resp.allowance, "999999");
    mock.assert_async().await;
}

#[tokio::test]
async fn builder_trades_returns_paginated_response() {
    let mut server = Server::new_async().await;

    let mock = server
        .mock("GET", "/builder/trades")
        .match_header("POLY_API_KEY", "test-key")
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
        .builder_trades()
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
