//! Mock HTTP tests for the relay client. These do not hit the real API.

use mockito::Server;
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient};

/// Well-known test private key (anvil/hardhat default #0). Do not use with real funds.
const TEST_PRIVATE_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

fn client_with_builder_auth(server: &mockito::ServerGuard) -> RelayClient {
    // Use a base64-encoded dummy secret ("secret" -> "c2VjcmV0") so the v2
    // HMAC header generation succeeds.
    let config = BuilderConfig::new(
        "builder-key".into(),
        "c2VjcmV0".into(),
        Some("pp".into()),
    );
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .build()
        .expect("build client")
}

fn client_with_relayer_api_key_auth(server: &mockito::ServerGuard) -> RelayClient {
    let account = BuilderAccount::with_relayer_api_key(
        TEST_PRIVATE_KEY,
        "rk-abc".into(),
        "0xabc123".into(),
    )
    .unwrap();
    RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .build()
        .expect("build client")
}

fn client_unauthed(server: &mockito::ServerGuard) -> RelayClient {
    RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .build()
        .expect("build client")
}

// ── list_transactions ──────────────────────────────────────────

#[tokio::test]
async fn list_transactions_with_builder_auth_sends_hmac_headers() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/transactions")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("POLY_BUILDER_SIGNATURE", mockito::Matcher::Any)
        .match_header("POLY_BUILDER_TIMESTAMP", mockito::Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "transactionID": "tx-1",
                "state": "STATE_CONFIRMED",
                "type": "SAFE"
            }]"#,
        )
        .create_async()
        .await;

    let client = client_with_builder_auth(&server);
    let txs = client.list_transactions().await.unwrap();
    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].transaction_id, "tx-1");
    assert_eq!(txs[0].state, "STATE_CONFIRMED");
    assert_eq!(txs[0].kind.as_deref(), Some("SAFE"));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_transactions_with_relayer_api_key_sends_static_headers() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/transactions")
        .match_header("RELAYER_API_KEY", "rk-abc")
        .match_header("RELAYER_API_KEY_ADDRESS", "0xabc123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let client = client_with_relayer_api_key_auth(&server);
    let txs = client.list_transactions().await.unwrap();
    assert!(txs.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn list_transactions_errors_when_no_auth_configured() {
    let server = Server::new_async().await;
    // No mock set up: request must fail before any HTTP call.
    let client = client_unauthed(&server);
    let err = client.list_transactions().await.expect_err("should error");
    let msg = format!("{err}");
    assert!(
        msg.contains("Account missing"),
        "expected missing-account error, got: {msg}"
    );
}

// ── list_relayer_api_keys ──────────────────────────────────────

#[tokio::test]
async fn list_relayer_api_keys_sends_relayer_api_key_headers() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/relayer/api/keys")
        .match_header("RELAYER_API_KEY", "rk-abc")
        .match_header("RELAYER_API_KEY_ADDRESS", "0xabc123")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"[{
                "apiKey": "01967c03-b8c8-7000-8f68-8b8eaec6fd3d",
                "address": "0xabc123",
                "createdAt": "2026-02-24T18:20:11.237485Z",
                "updatedAt": "2026-02-24T18:20:11.237485Z"
            }]"#,
        )
        .create_async()
        .await;

    let client = client_with_relayer_api_key_auth(&server);
    let keys = client.list_relayer_api_keys().await.unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].api_key, "01967c03-b8c8-7000-8f68-8b8eaec6fd3d");
    assert_eq!(keys[0].address, "0xabc123");
    assert_eq!(keys[0].created_at, "2026-02-24T18:20:11.237485Z");
    assert_eq!(keys[0].updated_at, "2026-02-24T18:20:11.237485Z");
    mock.assert_async().await;
}

#[tokio::test]
async fn list_relayer_api_keys_empty_response_ok() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/relayer/api/keys")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("[]")
        .create_async()
        .await;

    let client = client_with_relayer_api_key_auth(&server);
    let keys = client.list_relayer_api_keys().await.unwrap();
    assert!(keys.is_empty());
    mock.assert_async().await;
}

#[tokio::test]
async fn list_relayer_api_keys_rejects_builder_hmac_auth() {
    let server = Server::new_async().await;
    // No mock set up: request must be rejected client-side before hitting HTTP.
    let client = client_with_builder_auth(&server);
    let err = client
        .list_relayer_api_keys()
        .await
        .expect_err("builder auth should not be allowed");
    let msg = format!("{err}");
    assert!(
        msg.contains("Relayer API Key auth"),
        "expected API-key-required error, got: {msg}"
    );
}

#[tokio::test]
async fn list_relayer_api_keys_errors_when_no_auth_configured() {
    let server = Server::new_async().await;
    let client = client_unauthed(&server);
    let err = client
        .list_relayer_api_keys()
        .await
        .expect_err("should error");
    let msg = format!("{err}");
    assert!(
        msg.contains("Account missing"),
        "expected missing-account error, got: {msg}"
    );
}
