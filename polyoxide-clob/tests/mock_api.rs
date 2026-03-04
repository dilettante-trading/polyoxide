use mockito::{Matcher, Server};
use polyoxide_clob::{Account, ClobBuilder, Credentials};

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
        .with_body(r#"{"time": "1700000000"}"#)
        .create_async()
        .await;

    let clob = test_public_clob(&server);
    let resp = clob.health().server_time().send().await.unwrap();

    assert_eq!(resp.time, "1700000000");
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
        .with_body("[]")
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let orders = clob.orders().unwrap().list().send().await.unwrap();

    assert!(orders.is_empty());
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
        .with_body("[]")
        .create_async()
        .await;

    let clob = test_authed_clob(&server);
    let _orders = clob.orders().unwrap().list().send().await.unwrap();

    mock.assert_async().await;
}
