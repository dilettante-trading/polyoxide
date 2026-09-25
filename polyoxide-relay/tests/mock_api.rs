//! Mock HTTP tests for the relay client. These do not hit the real API.

use mockito::{Matcher, Server};
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient};

/// Well-known test private key (anvil/hardhat default #0). Do not use with real funds.
const TEST_PRIVATE_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

fn client_with_builder_auth(server: &mockito::ServerGuard) -> RelayClient {
    // Use a base64-encoded dummy secret ("secret" -> "c2VjcmV0") so the v2
    // HMAC header generation succeeds.
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
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
    let account =
        BuilderAccount::with_relayer_api_key(TEST_PRIVATE_KEY, "rk-abc".into(), "0xabc123".into())
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

// ── v1 account routes ──────────────────────────────────────────

#[tokio::test]
async fn get_execute_params_asks_for_the_wallet_type_nonce() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "address".into(),
                "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266".into(),
            ),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"12"}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();
    let nonce = client
        .get_execute_params(owner, polyoxide_relay::WalletType::DepositWallet)
        .await
        .unwrap();
    assert_eq!(nonce, 12);
    mock.assert_async().await;
}

#[tokio::test]
async fn get_deployed_typed_passes_the_type() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "address".into(),
                "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".into(),
            ),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let wallet: alloy::primitives::Address = "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50"
        .parse()
        .unwrap();
    assert!(client
        .get_deployed_typed(wallet, polyoxide_relay::WalletType::DepositWallet)
        .await
        .unwrap());
    mock.assert_async().await;
}

#[tokio::test]
async fn get_gasless_transaction_reads_the_v1_record() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/account/transactions/tx-77")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transaction_id":"tx-77","transaction_hash":"0xabc","state":"STATE_CONFIRMED","error_msg":null}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let tx = client.get_gasless_transaction("tx-77").await.unwrap();
    assert_eq!(tx.state, polyoxide_relay::TransactionState::Confirmed);
    assert_eq!(tx.transaction_hash.as_deref(), Some("0xabc"));
    mock.assert_async().await;
}

#[tokio::test]
async fn get_gasless_transaction_percent_encodes_the_id() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/v1/account/transactions/a%2Fb%3Fc")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transaction_id":"a/b?c","transaction_hash":null,"state":"STATE_NEW","error_msg":null}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let tx = client.get_gasless_transaction("a/b?c").await.unwrap();
    assert_eq!(tx.transaction_id, "a/b?c");
    mock.assert_async().await;
}

#[tokio::test]
async fn resolve_wallet_probes_both_deposit_wallet_generations_and_the_safe() {
    // Anvil #0: beacon 0xBc0f…, uups 0xdf8b…, safe 0xd93B… (relay_vectors.json).
    let mut server = Server::new_async().await;
    let beacon = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "address".into(),
                "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50".into(),
            ),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .create_async()
        .await;
    let uups = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "address".into(),
                "0xdf8b9E8f9AB23f261F6e1B171B7454ae6E46Ba76".into(),
            ),
            Matcher::UrlEncoded("type".into(), "WALLET".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .create_async()
        .await;
    let safe = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded(
                "address".into(),
                "0xd93B25cb943D14d0d34FBaF01Fc93a0f8b5F6E47".into(),
            ),
            Matcher::UrlEncoded("type".into(), "SAFE".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();
    let kind = client.resolve_wallet(owner).await.unwrap();
    assert_eq!(
        kind,
        Some(polyoxide_relay::WalletKind::DepositWallet(
            "0xBc0fF067b7740Eff76C1ca93c875Ba6B890d6B50"
                .parse()
                .unwrap()
        ))
    );
    beacon.assert_async().await;
    uups.assert_async().await;
    safe.assert_async().await;
}

#[tokio::test]
async fn resolve_wallet_refuses_two_deployed_wallets() {
    let mut server = Server::new_async().await;
    let all_deployed = server
        .mock("GET", "/deployed")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .expect(3)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();
    let err = client.resolve_wallet(owner).await.unwrap_err().to_string();
    assert!(err.contains("more than one"), "{err}");
    all_deployed.assert_async().await;
}

#[tokio::test]
async fn resolve_wallet_reports_none_when_nothing_is_deployed() {
    let mut server = Server::new_async().await;
    let none = server
        .mock("GET", "/deployed")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .expect(3)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();
    assert_eq!(client.resolve_wallet(owner).await.unwrap(), None);
    none.assert_async().await;
}
