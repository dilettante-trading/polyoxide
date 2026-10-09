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
async fn resolve_wallet_probes_both_deposit_wallet_generations_the_safe_and_the_proxy() {
    // Anvil #0: beacon 0xBc0f…, uups 0xdf8b…, safe 0xd93B…, proxy 0x365f… (relay_vectors.json).
    let proxy_address = relay_vectors()["derivations"]["anvil0"]["proxy"]
        .as_str()
        .unwrap()
        .to_string();
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
    let proxy = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), proxy_address),
            Matcher::UrlEncoded("type".into(), "PROXY".into()),
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
    proxy.assert_async().await;
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
        .expect(4)
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
        .expect(4)
        .create_async()
        .await;

    let client = client_unauthed(&server);
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();
    assert_eq!(client.resolve_wallet(owner).await.unwrap(), None);
    none.assert_async().await;
}

#[tokio::test]
async fn resolve_wallet_reports_a_deployed_proxy() {
    let v = relay_vectors();
    let owner: alloy::primitives::Address = v["owner"].as_str().unwrap().parse().unwrap();
    let proxy = v["derivations"]["anvil0"]["proxy"].as_str().unwrap();
    let mut server = Server::new_async().await;
    // Everything but the Proxy says "not deployed".
    let others = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![Matcher::Regex(
            "type=(WALLET|SAFE)".into(),
        )]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":false}"#)
        .expect(3)
        .create_async()
        .await;
    let proxy_mock = server
        .mock("GET", "/deployed")
        .match_query(Matcher::AllOf(vec![
            Matcher::UrlEncoded("address".into(), proxy.into()),
            Matcher::UrlEncoded("type".into(), "PROXY".into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"deployed":true}"#)
        .expect(1)
        .create_async()
        .await;
    let client = client_unauthed(&server);
    let kind = client.resolve_wallet(owner).await.unwrap();
    assert_eq!(
        kind,
        Some(polyoxide_relay::WalletKind::Proxy(proxy.parse().unwrap()))
    );
    others.assert_async().await;
    proxy_mock.assert_async().await;
}

// ── Deposit Wallet execution ───────────────────────────────────

const RELAY_VECTORS: &str = include_str!("fixtures/session_keys/relay_vectors.json");

fn relay_vectors() -> serde_json::Value {
    serde_json::from_str(RELAY_VECTORS).unwrap()
}

fn deposit_wallet_client(server: &mockito::ServerGuard) -> RelayClient {
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let wallet: alloy::primitives::Address =
        relay_vectors()["wallet"].as_str().unwrap().parse().unwrap();
    RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .build()
        .expect("build client")
}

#[tokio::test]
async fn submit_deposit_wallet_batch_posts_the_py_sdk_body_under_builder_hmac() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("POLY_BUILDER_SIGNATURE", Matcher::Any)
        .match_header("content-type", "application/json")
        .match_body(Matcher::Json(v["submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-1","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let b = &v["approval_batch"];
    let call = polyoxide_relay::DepositWalletCall {
        target: b["calls"][0]["target"].as_str().unwrap().parse().unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(b["calls"][0]["data"].as_str().unwrap())
            .unwrap()
            .into(),
    };
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let resp = client
        .submit_deposit_wallet_batch(
            wallet,
            &[call],
            3,
            1_800_000_000,
            b["signature"].as_str().unwrap(),
            Some(String::new()),
        )
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-1");
    mock.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_fetches_the_wallet_nonce_and_signs_the_batch() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
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
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"3"}"#)
        .create_async()
        .await;
    // The deadline is now + 600 s, so the signature cannot be pinned here; the
    // batch_digest/signature vector tests pin the signing. Here: body shape and auth.
    let submit = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(
                r#"{{"type":"WALLET","from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","to":"{}","nonce":"3","depositWalletParams":{{"depositWallet":"{}","calls":[{{"target":"0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB","value":"0"}}]}}}}"#,
                v["config"]["deposit_wallet_factory"].as_str().unwrap(),
                v["wallet"].as_str().unwrap()
            )),
            Matcher::Regex(r#""signature":"0x[0-9a-f]{130}""#.into()),
            Matcher::Regex(r#""deadline":"\d{10}""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-2","state":"STATE_NEW"}"#)
        .create_async()
        .await;
    let legacy_nonce = server
        .mock("GET", "/nonce")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let tx = polyoxide_relay::SafeTransaction {
        to: "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
            .parse()
            .unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(
            v["approval_batch"]["calls"][0]["data"].as_str().unwrap(),
        )
        .unwrap()
        .into(),
        operation: 0,
    };
    let resp = client.execute(vec![tx], Some(String::new())).await.unwrap();
    assert_eq!(resp.transaction_id, "tx-2");
    params.assert_async().await;
    submit.assert_async().await;
    legacy_nonce.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_refuses_delegatecall_before_io() {
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let client = deposit_wallet_client(&server);
    let tx = polyoxide_relay::SafeTransaction {
        to: alloy::primitives::Address::ZERO,
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::Bytes::new(),
        operation: 1,
    };
    let err = client
        .execute(vec![tx], None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("DELEGATECALL"), "{err}");
    params.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_needs_the_wallet_address() {
    let server = Server::new_async().await;
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .build()
        .unwrap();
    let tx = polyoxide_relay::SafeTransaction {
        to: alloy::primitives::Address::ZERO,
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::Bytes::new(),
        operation: 0,
    };
    let err = client
        .execute(vec![tx], None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("deposit_wallet"), "{err}");
}

#[tokio::test]
async fn with_auth_submits_a_signature_in_batch_without_any_key() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::Json(v["submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-3","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let auth = polyoxide_relay::AuthConfig::Builder(BuilderConfig::new(
        "builder-key".into(),
        "c2VjcmV0".into(),
        Some("pp".into()),
    ));
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_auth(auth)
        .build()
        .unwrap();
    assert!(client.address().is_none());

    let b = &v["approval_batch"];
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let owner: alloy::primitives::Address = v["owner"].as_str().unwrap().parse().unwrap();
    let call = polyoxide_relay::DepositWalletCall {
        target: b["calls"][0]["target"].as_str().unwrap().parse().unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(b["calls"][0]["data"].as_str().unwrap())
            .unwrap()
            .into(),
    };
    let typed = client.deposit_wallet_batch_typed_data(
        wallet,
        std::slice::from_ref(&call),
        3,
        1_800_000_000,
    );
    assert_eq!(typed, b["typed_data"]);
    let resp = client
        .submit_deposit_wallet_batch_from(
            owner,
            wallet,
            &[call],
            3,
            1_800_000_000,
            b["signature"].as_str().unwrap(),
            Some(String::new()),
        )
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-3");
    mock.assert_async().await;
}

#[tokio::test]
async fn submit_deposit_wallet_batch_from_a_session_key_posts_the_py_sdk_body() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::Json(v["session_submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-4","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let b = &v["approval_batch"];
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session_signer: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let call = polyoxide_relay::DepositWalletCall {
        target: b["calls"][0]["target"].as_str().unwrap().parse().unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(b["calls"][0]["data"].as_str().unwrap())
            .unwrap()
            .into(),
    };
    let resp = client
        .submit_deposit_wallet_batch_from(
            session_signer,
            wallet,
            &[call],
            3,
            1_800_000_000,
            b["session_signature"].as_str().unwrap(),
            Some(String::new()),
        )
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-4");
    mock.assert_async().await;
}

#[tokio::test]
async fn execute_as_a_session_key_submits_from_the_key_with_the_session_envelope() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
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
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"3"}"#)
        .create_async()
        .await;
    // The envelope opens with the session signer's id (the account address, left
    // padded) and ends in the 32-byte ERC-6492-style magic suffix.
    let submit = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(
                r#"{"type":"WALLET","from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"3"}"#
                    .into(),
            ),
            Matcher::Regex(
                r#""signature":"0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266[0-9a-f]*(6492){16}""#
                    .into(),
            ),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-5","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .deposit_wallet_role(polyoxide_core::DepositWalletRole::SessionKey)
        .build()
        .unwrap();
    let tx = polyoxide_relay::SafeTransaction {
        to: "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
            .parse()
            .unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(
            v["approval_batch"]["calls"][0]["data"].as_str().unwrap(),
        )
        .unwrap()
        .into(),
        operation: 0,
    };
    let resp = client.execute(vec![tx], None).await.unwrap();
    assert_eq!(resp.transaction_id, "tx-5");
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_refuses_an_unsupported_chain_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/submit")
        .expect(0)
        .create_async()
        .await;

    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let client = RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .chain_id(80002)
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .build()
        .unwrap();
    let tx = polyoxide_relay::SafeTransaction {
        to: alloy::primitives::Address::ZERO,
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::Bytes::new(),
        operation: 0,
    };
    let err = client
        .execute(vec![tx], None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("not supported on this chain"), "{err}");
    params.assert_async().await;
    submit.assert_async().await;
}

// ── session signers ────────────────────────────────────────────

#[tokio::test]
async fn authorize_session_signer_typed_data_and_submit_reproduce_the_py_sdk_body() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/authorizations")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("POLY_BUILDER_SIGNATURE", Matcher::Any)
        .match_header("POLY_BUILDER_PASSPHRASE", "pp")
        .match_header("Idempotency-Key", "idem-1")
        .match_body(Matcher::Json(v["authorization_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"operationId":"op-1","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-9"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let (typed, request) = client
        .authorize_session_signer_typed_data_with_valid_until(
            wallet,
            session,
            vec![polyoxide_core::SessionSignerScope::Clob],
            1815534000,
            4,
            1800000600,
        )
        .unwrap();
    assert_eq!(typed, v["authorize_batch"]["typed_data"]);
    assert_eq!(request.valid_until, 1815534000);

    let resp = client
        .submit_session_signer_authorization(
            &request,
            v["authorize_batch"]["signature"].as_str().unwrap(),
            "idem-1",
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status,
        polyoxide_relay::SessionSignerAuthorizationStatus::Submitted
    );
    assert_eq!(resp.transaction_id, "tx-9");
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_typed_data_computes_valid_until_from_the_lifetime() {
    let v = relay_vectors();
    let server = Server::new_async().await;
    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let before = polyoxide_core::current_timestamp();
    let (_, request) = client
        .authorize_session_signer_typed_data(
            wallet,
            session,
            vec![polyoxide_core::SessionSignerScope::All],
            4,
            1800000600,
        )
        .unwrap();
    let after = polyoxide_core::current_timestamp();
    assert!(request.valid_until >= before + polyoxide_relay::SESSION_KEY_LIFETIME_SECS);
    assert!(request.valid_until <= after + polyoxide_relay::SESSION_KEY_LIFETIME_SECS);
}

#[tokio::test]
async fn session_signer_authorization_refuses_relayer_api_key_auth_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/authorizations")
        .expect(0)
        .create_async()
        .await;
    let client = client_with_relayer_api_key_auth(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let (_, request) = client
        .authorize_session_signer_typed_data_with_valid_until(
            wallet,
            session,
            vec![polyoxide_core::SessionSignerScope::Clob],
            1815534000,
            4,
            1800000600,
        )
        .unwrap();
    let err = client
        .submit_session_signer_authorization(&request, "0xsig", "idem")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Builder HMAC"), "{err}");
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_typed_data_rejects_bad_scopes_before_io() {
    let v = relay_vectors();
    let server = Server::new_async().await;
    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let err = client
        .authorize_session_signer_typed_data(
            wallet,
            session,
            vec![
                polyoxide_core::SessionSignerScope::All,
                polyoxide_core::SessionSignerScope::Clob,
            ],
            4,
            1800000600,
        )
        .unwrap_err()
        .to_string();
    assert!(err.contains("ALL"), "{err}");
}

#[tokio::test]
async fn revoke_session_signer_typed_data_and_submit_send_the_venue_body() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/revocations")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("Idempotency-Key", "idem-2")
        .match_body(Matcher::Json(v["revocation_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"operationId":"op-2","status":"FENCED","fenced":true,"transactionId":"tx-10"}"#,
        )
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let (typed, request) = client.revoke_session_signer_typed_data(wallet, session, 5, 1800000600);
    assert_eq!(typed, v["revoke_batch"]["typed_data"]);
    let resp = client
        .submit_session_signer_revocation(
            &request,
            v["revoke_batch"]["signature"].as_str().unwrap(),
            "idem-2",
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status,
        polyoxide_relay::SessionSignerRevocationStatus::Fenced
    );
    assert!(resp.fenced);
    mock.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_with_a_local_key_fetches_the_nonce_and_signs() {
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
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
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"4"}"#)
        .create_async()
        .await;
    let captured: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&captured);
    let submit = server
        .mock("POST", "/v1/session-signers/authorizations")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_header("Idempotency-Key", Matcher::Regex(r"^[0-9a-f-]{36}$".into()))
        .match_body(Matcher::PartialJsonString(format!(
            r#"{{"nonce":"4","scopes":["CLOB"],"sessionSignerAddress":"{}","walletAddress":"{}"}}"#,
            v["session_signer"].as_str().unwrap(),
            v["wallet"].as_str().unwrap()
        )))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body_from_request(move |req| {
            *sink.lock().unwrap() = Some(req.body().unwrap().clone());
            br#"{"operationId":"op-3","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-11"}"#.to_vec()
        })
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let resp = client
        .authorize_session_signer(session, vec![polyoxide_core::SessionSignerScope::Clob])
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-11");
    params.assert_async().await;
    submit.assert_async().await;

    // The signature must be the owner's over exactly the batch the body describes:
    // rebuild it from the posted validUntil, nonce and deadline and recover the signer.
    let body: serde_json::Value =
        serde_json::from_slice(captured.lock().unwrap().as_ref().expect("body captured")).unwrap();
    let field = |k: &str| -> u64 { body[k].as_str().unwrap().parse().unwrap() };
    let (valid_until, nonce, deadline) = (field("validUntil"), field("nonce"), field("deadline"));
    let calls = vec![polyoxide_relay::DepositWalletCall {
        target: wallet,
        value: alloy::primitives::U256::ZERO,
        data: polyoxide_relay::deposit_wallet::authorize_session_signer_calldata(
            session,
            valid_until,
        )
        .into(),
    }];
    let digest =
        polyoxide_relay::deposit_wallet::batch_digest(137, wallet, &calls, nonce, deadline);
    let sig = alloy::primitives::Signature::from_str(body["signature"].as_str().unwrap()).unwrap();
    assert_eq!(
        sig.recover_address_from_prehash(&digest).unwrap(),
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse::<alloy::primitives::Address>()
            .unwrap()
    );
}

#[tokio::test]
async fn submit_session_signer_revocation_accepts_relayer_api_key_auth() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/revocations")
        .match_header("RELAYER_API_KEY", "rk-abc")
        .match_header("RELAYER_API_KEY_ADDRESS", "0xabc123")
        .match_header("Idempotency-Key", "idem-3")
        .match_body(Matcher::Json(v["revocation_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"operationId":"op-4","status":"PENDING","fenced":false,"transactionId":"tx-12"}"#,
        )
        .create_async()
        .await;
    let client = client_with_relayer_api_key_auth(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let (_, request) = client.revoke_session_signer_typed_data(wallet, session, 5, 1800000600);
    let resp = client
        .submit_session_signer_revocation(
            &request,
            v["revoke_batch"]["signature"].as_str().unwrap(),
            "idem-3",
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status,
        polyoxide_relay::SessionSignerRevocationStatus::Pending
    );
    assert_eq!(resp.transaction_id, "tx-12");
    mock.assert_async().await;
}

#[tokio::test]
async fn session_signer_submits_refuse_a_blank_idempotency_key_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let auth = server
        .mock("POST", "/v1/session-signers/authorizations")
        .expect(0)
        .create_async()
        .await;
    let revoke = server
        .mock("POST", "/v1/session-signers/revocations")
        .expect(0)
        .create_async()
        .await;
    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let (_, a) = client
        .authorize_session_signer_typed_data_with_valid_until(
            wallet,
            session,
            vec![polyoxide_core::SessionSignerScope::Clob],
            1815534000,
            4,
            1800000600,
        )
        .unwrap();
    let err = client
        .submit_session_signer_authorization(&a, "0xsig", "   ")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("idempotency key"), "{err}");
    let (_, r) = client.revoke_session_signer_typed_data(wallet, session, 5, 1800000600);
    let err = client
        .submit_session_signer_revocation(&r, "0xsig", "")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("idempotency key"), "{err}");
    auth.assert_async().await;
    revoke.assert_async().await;
}

fn session_key_deposit_wallet_client(server: &mockito::ServerGuard) -> RelayClient {
    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let wallet: alloy::primitives::Address =
        relay_vectors()["wallet"].as_str().unwrap().parse().unwrap();
    RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .deposit_wallet_role(polyoxide_core::DepositWalletRole::SessionKey)
        .build()
        .unwrap()
}

#[tokio::test]
async fn authorize_session_signer_refuses_a_session_key_role_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/v1/session-signers/authorizations")
        .expect(0)
        .create_async()
        .await;
    let client = session_key_deposit_wallet_client(&server);
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let err = client
        .authorize_session_signer(session, vec![polyoxide_core::SessionSignerScope::Clob])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("managed by the wallet owner"), "{err}");
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn authorize_session_signer_refuses_relayer_api_key_auth_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/v1/session-signers/authorizations")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let account =
        BuilderAccount::with_relayer_api_key(TEST_PRIVATE_KEY, "rk-abc".into(), "0xabc123".into())
            .unwrap();
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let client = RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .build()
        .expect("build client");
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let err = client
        .authorize_session_signer(session, vec![polyoxide_core::SessionSignerScope::Clob])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Builder HMAC"), "{err}");
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn revoke_session_signer_accepts_relayer_api_key_auth() {
    // Revocation takes a Builder key or a Relayer API key, unlike authorization;
    // guards against copying the authorize refusal into revoke.
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
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
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"6"}"#)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/v1/session-signers/revocations")
        .match_header("RELAYER_API_KEY", "rk-abc")
        .match_header("RELAYER_API_KEY_ADDRESS", "0xabc123")
        .match_body(Matcher::PartialJsonString(format!(
            r#"{{"walletAddress":"{}","sessionSignerAddress":"{}","nonce":"6"}}"#,
            v["wallet"].as_str().unwrap(),
            v["session_signer"].as_str().unwrap(),
        )))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"operationId":"op-5","status":"PENDING","fenced":false,"transactionId":"tx-14"}"#,
        )
        .expect(1)
        .create_async()
        .await;
    let account =
        BuilderAccount::with_relayer_api_key(TEST_PRIVATE_KEY, "rk-abc".into(), "0xabc123".into())
            .unwrap();
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let client = RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .build()
        .expect("build client");
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let resp = client.revoke_session_signer(session).await.unwrap();
    assert_eq!(resp.transaction_id, "tx-14");
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn revoke_session_signer_refuses_a_session_key_role_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/v1/session-signers/revocations")
        .expect(0)
        .create_async()
        .await;
    let client = session_key_deposit_wallet_client(&server);
    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let err = client
        .revoke_session_signer(session)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("managed by the wallet owner"), "{err}");
    params.assert_async().await;
    submit.assert_async().await;
}

// ── Deposit Wallet redemption ─────────────────────────────────

fn redemption_condition() -> alloy::primitives::B256 {
    "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51"
        .parse()
        .unwrap()
}

fn binary_index_sets() -> [alloy::primitives::U256; 2] {
    [
        alloy::primitives::U256::from(1),
        alloy::primitives::U256::from(2),
    ]
}

#[tokio::test]
async fn redeem_typed_data_and_submit_reproduce_the_py_sdk_batch() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::Json(v["redeem_adapter_submit_body"].clone()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-12","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let (typed, calls) = client.redeem_typed_data(
        wallet,
        redemption_condition(),
        &binary_index_sets(),
        false,
        8,
        1800000600,
    );
    assert_eq!(typed, v["redeem_adapter_batch"]["typed_data"]);
    let resp = client
        .submit_redemption_with_signature(
            wallet,
            &calls,
            8,
            1800000600,
            v["redeem_adapter_batch"]["signature"].as_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-12");
    mock.assert_async().await;
}

#[test]
fn redeem_typed_data_targets_the_neg_risk_adapter_for_a_neg_risk_market() {
    let v = relay_vectors();
    let server = mockito::Server::new();
    let client = deposit_wallet_client(&server);
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let (typed, calls) = client.redeem_typed_data(
        wallet,
        redemption_condition(),
        &binary_index_sets(),
        true,
        9,
        1800000600,
    );
    assert_eq!(typed, v["redeem_neg_risk_batch"]["typed_data"]);
    let neg_risk_adapter: alloy::primitives::Address = "0xadA2005600Dec949baf300f4C6120000bDB6eAab"
        .parse()
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].target, neg_risk_adapter);
    assert_eq!(
        calls[0].target.to_checksum(None),
        v["redeem_neg_risk_batch"]["calls"][0]["target"]
            .as_str()
            .unwrap()
    );
}

#[tokio::test]
async fn submit_deposit_wallet_redemption_redeems_through_the_collateral_adapter() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
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
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"8"}"#)
        .create_async()
        .await;
    // The fixture's call is exactly this redemption (collateral adapter, pUSD
    // collateral, condition, index sets [1, 2]), so its target and calldata are pinned
    // in full. The deadline is now + 600 s, so the signature is pinned only by shape.
    let fixture_call = &v["redeem_adapter_submit_body"]["depositWalletParams"]["calls"][0];
    let submit = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(
                r#"{{"type":"WALLET","from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","to":"{}","nonce":"8","metadata":"","depositWalletParams":{{"depositWallet":"{}","calls":[{{"target":"{}","value":"0","data":"{}"}}]}}}}"#,
                v["config"]["deposit_wallet_factory"].as_str().unwrap(),
                v["wallet"].as_str().unwrap(),
                fixture_call["target"].as_str().unwrap(),
                fixture_call["data"].as_str().unwrap(),
            )),
            Matcher::Regex(r#""signature":"0x[0-9a-f]{130}""#.into()),
            Matcher::Regex(r#""deadline":"\d{10}""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-13","state":"STATE_NEW"}"#)
        .create_async()
        .await;
    let legacy_nonce = server
        .mock("GET", "/nonce")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let resp = client
        .submit_deposit_wallet_redemption(redemption_condition(), false, false)
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-13");
    assert_eq!(
        fixture_call["target"].as_str().unwrap(),
        "0xAdA100Db00Ca00073811820692005400218FcE1f"
    );
    params.assert_async().await;
    submit.assert_async().await;
    legacy_nonce.assert_async().await;
}

#[tokio::test]
async fn submit_deposit_wallet_redemption_redeems_a_neg_risk_market_through_the_neg_risk_adapter() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
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
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"9"}"#)
        .create_async()
        .await;
    // The neg-risk batch's call (neg-risk collateral adapter, pUSD collateral,
    // condition, index sets [1, 2]) pins the target and calldata in full.
    let fixture_call = &v["redeem_neg_risk_batch"]["calls"][0];
    let submit = server
        .mock("POST", "/submit")
        .match_header("POLY_BUILDER_API_KEY", "builder-key")
        .match_body(Matcher::AllOf(vec![
            Matcher::PartialJsonString(format!(
                r#"{{"type":"WALLET","nonce":"9","metadata":"","depositWalletParams":{{"depositWallet":"{}","calls":[{{"target":"{}","value":"0","data":"{}"}}]}}}}"#,
                v["wallet"].as_str().unwrap(),
                fixture_call["target"].as_str().unwrap(),
                fixture_call["data"].as_str().unwrap(),
            )),
            Matcher::Regex(r#""signature":"0x[0-9a-f]{130}""#.into()),
        ]))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-15","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let resp = client
        .submit_deposit_wallet_redemption(redemption_condition(), true, false)
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-15");
    assert_eq!(
        fixture_call["target"].as_str().unwrap(),
        "0xadA2005600Dec949baf300f4C6120000bDB6eAab"
    );
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn estimate_redemption_gas_on_a_deposit_wallet_client_is_refused_before_io() {
    // Refused before the RPC provider is even built, so no mock is needed.
    let server = Server::new_async().await;
    let client = deposit_wallet_client(&server);
    let err = client
        .estimate_redemption_gas([0u8; 32], vec![alloy::primitives::U256::from(1)])
        .await
        .unwrap_err()
        .to_string();
    let gasless = client
        .submit_gasless_redemption([0u8; 32], vec![])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("submit_deposit_wallet_redemption"), "{err}");
    assert_eq!(err, gasless);
}

#[tokio::test]
async fn submit_gasless_redemption_on_a_deposit_wallet_client_is_refused_before_io() {
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/submit")
        .expect(0)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let err = client
        .submit_gasless_redemption([0u8; 32], vec![])
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("submit_deposit_wallet_redemption"),
        "unexpected error: {err}"
    );
    params.assert_async().await;
    submit.assert_async().await;
}

// ── Deposit Wallet metadata ───────────────────────────────────

fn approval_transaction(v: &serde_json::Value) -> polyoxide_relay::SafeTransaction {
    polyoxide_relay::SafeTransaction {
        to: "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB"
            .parse()
            .unwrap(),
        value: alloy::primitives::U256::ZERO,
        data: alloy::primitives::hex::decode(
            v["approval_batch"]["calls"][0]["data"].as_str().unwrap(),
        )
        .unwrap()
        .into(),
        operation: 0,
    }
}

#[tokio::test]
async fn execute_on_a_deposit_wallet_without_metadata_sends_an_empty_string() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"3"}"#)
        .create_async()
        .await;
    // py-sdk's build_deposit_wallet_payload always sends metadata, "" by default.
    let submit = server
        .mock("POST", "/submit")
        .match_body(Matcher::PartialJsonString(
            r#"{"type":"WALLET","metadata":""}"#.into(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"transactionID":"tx-14","state":"STATE_NEW"}"#)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    let resp = client
        .execute(vec![approval_transaction(&v)], None)
        .await
        .unwrap();
    assert_eq!(resp.transaction_id, "tx-14");
    params.assert_async().await;
    submit.assert_async().await;
}

#[tokio::test]
async fn deposit_wallet_metadata_over_500_characters_is_refused_before_io() {
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let params = server
        .mock("GET", "/v1/account/transactions/params")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let submit = server
        .mock("POST", "/submit")
        .expect(0)
        .create_async()
        .await;

    let client = deposit_wallet_client(&server);
    // py-sdk's cap is 500 characters. Both entry points refuse 501 before any I/O:
    // `execute` before it fetches the nonce, the signature-in submit before it posts.
    let err = client
        .execute(vec![approval_transaction(&v)], Some("x".repeat(501)))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("500"), "{err}");

    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let err = client
        .submit_deposit_wallet_batch(wallet, &[], 3, 1_800_000_000, "0x", Some("x".repeat(501)))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("500"), "{err}");
    params.assert_async().await;
    submit.assert_async().await;
}

// ── Every route on the send loop (DRIFT R7) ─────────────────────

type Sent = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), polyoxide_relay::RelayError>> + Send>,
>;

/// One relayer route: the method and path a mock answers, the body it answers
/// with, the client that may call it, and the call.
struct Route {
    method: &'static str,
    path: &'static str,
    body: &'static str,
    relayer_api_key: bool,
    call: fn(RelayClient) -> Sent,
}

/// The twelve routes the client calls on the relayer.
fn every_relay_route() -> Vec<Route> {
    fn owner() -> alloy::primitives::Address {
        "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
            .parse()
            .unwrap()
    }
    fn deposit_wallet_call() -> polyoxide_relay::DepositWalletCall {
        let b = &relay_vectors()["approval_batch"];
        polyoxide_relay::DepositWalletCall {
            target: b["calls"][0]["target"].as_str().unwrap().parse().unwrap(),
            value: alloy::primitives::U256::ZERO,
            data: alloy::primitives::hex::decode(b["calls"][0]["data"].as_str().unwrap())
                .unwrap()
                .into(),
        }
    }
    fn session_wallets() -> (alloy::primitives::Address, alloy::primitives::Address) {
        let v = relay_vectors();
        (
            v["wallet"].as_str().unwrap().parse().unwrap(),
            v["session_signer"].as_str().unwrap().parse().unwrap(),
        )
    }

    vec![
        Route {
            method: "GET",
            path: "/",
            body: "ok",
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.ping().await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/nonce",
            body: r#"{"nonce":"5"}"#,
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.get_nonce(owner()).await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/transaction",
            body: r#"{"transactionID":"tx-1","state":"STATE_NEW"}"#,
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.get_transaction("tx-1").await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/transactions",
            body: "[]",
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.list_transactions().await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/relayer/api/keys",
            body: "[]",
            relayer_api_key: true,
            call: |c| Box::pin(async move { c.list_relayer_api_keys().await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/deployed",
            body: r#"{"deployed":true}"#,
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.get_deployed(owner()).await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/v1/account/transactions/params",
            body: r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"12"}"#,
            relayer_api_key: false,
            call: |c| {
                Box::pin(async move {
                    c.get_execute_params(owner(), polyoxide_relay::WalletType::DepositWallet)
                        .await
                        .map(drop)
                })
            },
        },
        Route {
            method: "GET",
            path: "/v1/account/transactions/tx-77",
            body: r#"{"transaction_id":"tx-77","transaction_hash":null,"state":"STATE_NEW","error_msg":null}"#,
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.get_gasless_transaction("tx-77").await.map(drop) }),
        },
        Route {
            method: "GET",
            path: "/relay-payload",
            body: r#"{"address":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266","nonce":"1"}"#,
            relayer_api_key: false,
            call: |c| Box::pin(async move { c.get_relay_payload(owner()).await.map(drop) }),
        },
        Route {
            method: "POST",
            path: "/submit",
            body: r#"{"transactionID":"tx-1","state":"STATE_NEW"}"#,
            relayer_api_key: false,
            call: |c| {
                Box::pin(async move {
                    let v = relay_vectors();
                    let (wallet, _) = session_wallets();
                    c.submit_deposit_wallet_batch(
                        wallet,
                        &[deposit_wallet_call()],
                        3,
                        1_800_000_000,
                        v["approval_batch"]["signature"].as_str().unwrap(),
                        None,
                    )
                    .await
                    .map(drop)
                })
            },
        },
        Route {
            method: "POST",
            path: "/v1/session-signers/authorizations",
            body: r#"{"operationId":"op-1","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-9"}"#,
            relayer_api_key: false,
            call: |c| {
                Box::pin(async move {
                    let v = relay_vectors();
                    let (wallet, session) = session_wallets();
                    let (_, request) = c.authorize_session_signer_typed_data_with_valid_until(
                        wallet,
                        session,
                        vec![polyoxide_core::SessionSignerScope::Clob],
                        1815534000,
                        4,
                        1800000600,
                    )?;
                    c.submit_session_signer_authorization(
                        &request,
                        v["authorize_batch"]["signature"].as_str().unwrap(),
                        "idem-1",
                    )
                    .await
                    .map(drop)
                })
            },
        },
        Route {
            method: "POST",
            path: "/v1/session-signers/revocations",
            body: r#"{"operationId":"op-2","status":"FENCED","fenced":true,"transactionId":"tx-10"}"#,
            relayer_api_key: false,
            call: |c| {
                Box::pin(async move {
                    let v = relay_vectors();
                    let (wallet, session) = session_wallets();
                    let (_, request) =
                        c.revoke_session_signer_typed_data(wallet, session, 5, 1800000600);
                    c.submit_session_signer_revocation(
                        &request,
                        v["revoke_batch"]["signature"].as_str().unwrap(),
                        "idem-2",
                    )
                    .await
                    .map(drop)
                })
            },
        },
    ]
}

/// A client for `route` on `server`, retrying at most `max_retries` times
/// at a 200ms base.
fn retrying_client(server: &mockito::ServerGuard, route: &Route, max_retries: u32) -> RelayClient {
    let account = if route.relayer_api_key {
        BuilderAccount::with_relayer_api_key(TEST_PRIVATE_KEY, "rk-abc".into(), "0xabc123".into())
            .unwrap()
    } else {
        let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
        BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap()
    };
    let wallet: alloy::primitives::Address =
        relay_vectors()["wallet"].as_str().unwrap().parse().unwrap();
    RelayClient::builder()
        .unwrap()
        .url(&server.url())
        .unwrap()
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .with_retry_config(polyoxide_core::RetryConfig {
            max_retries,
            initial_backoff_ms: 200,
            max_backoff_ms: 10_000,
        })
        .build()
        .unwrap()
}

/// A mock for `route` answering `statuses` in turn, then the last of them for
/// good, with `headers` on every response, expecting `hits` requests.
async fn scripted_route(
    server: &mut mockito::ServerGuard,
    route: &Route,
    statuses: &'static [usize],
    headers: &[(&str, &str)],
    hits: usize,
) -> mockito::Mock {
    let served = std::sync::atomic::AtomicUsize::new(0);
    let mut mock = server
        .mock(route.method, route.path)
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |_| {
            let n = served.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            statuses[n.min(statuses.len() - 1)]
        })
        .with_header("content-type", "application/json");
    for (name, value) in headers {
        mock = mock.with_header(*name, value);
    }
    mock.with_body(route.body).expect(hits).create_async().await
}

/// Runs `check` on every route at once: relay's table paces a client at one
/// request every 2.9s, so one route after another would take minutes. A
/// failure names its route.
async fn on_every_route(
    check: fn(Route) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
) {
    let tasks: Vec<_> = every_relay_route()
        .into_iter()
        .map(|route| tokio::spawn(check(route)))
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn each_relay_route_retries_a_429() {
    // DRIFT R7: relay's own loops are gone; every route runs on core's send
    // loop with Polymarket's policy, so a 429 with a retry left is retried.
    on_every_route(|route| {
        Box::pin(async move {
            let name = format!("{} {}", route.method, route.path);
            let mut server = Server::new_async().await;
            let mock = scripted_route(&mut server, &route, &[429, 200], &[], 2).await;
            let client = retrying_client(&server, &route, 1);

            let result = (route.call)(client).await;
            assert!(result.is_ok(), "{name}: {:?}", result.err());
            mock.assert_async().await;
        })
    })
    .await;
}

#[tokio::test]
async fn each_relay_route_s_429_holds_the_next_request() {
    // The 429 asks for 4s, past relay's own pacing of 2.9s, so only its hold
    // can keep the next request back that long.
    on_every_route(|route| {
        Box::pin(async move {
            use polyoxide_venue::{Class, Classify};

            let name = format!("{} {}", route.method, route.path);
            let mut server = Server::new_async().await;
            let mock =
                scripted_route(&mut server, &route, &[429, 200], &[("retry-after", "4")], 2).await;
            let client = retrying_client(&server, &route, 0);

            // No retry left: the 429 is the caller's, classed by its status.
            let err = (route.call)(client.clone())
                .await
                .expect_err(&format!("{name}: a 429 and no retry"));
            assert!(
                matches!(
                    err,
                    polyoxide_relay::RelayError::Api(polyoxide_core::ApiError::RateLimit(_))
                ),
                "{name}: {err:?}"
            );
            assert_eq!(
                err.class(),
                Class::RateLimited { retry_after: None },
                "{name}"
            );

            // Its hold stops the next request on the client.
            let start = std::time::Instant::now();
            let result = (route.call)(client).await;
            assert!(result.is_ok(), "{name}: {:?}", result.err());
            assert!(
                start.elapsed() >= std::time::Duration::from_millis(3_500),
                "{name}: the next request went after {:?}, inside the 429's 4s hold",
                start.elapsed()
            );
            mock.assert_async().await;
        })
    })
    .await;
}

#[tokio::test]
async fn a_relay_503_is_not_retried_and_a_425_is() {
    use polyoxide_venue::{Class, Classify};

    let route = &every_relay_route()[1];
    let mut server = Server::new_async().await;
    let mock = scripted_route(&mut server, route, &[503], &[], 1).await;
    let err = (route.call)(retrying_client(&server, route, 3))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            polyoxide_relay::RelayError::Api(polyoxide_core::ApiError::Api { status: 503, .. })
        ),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::Unavailable { code: None });
    mock.assert_async().await;

    let mut server = Server::new_async().await;
    let mock = scripted_route(&mut server, route, &[425, 200], &[], 2).await;
    (route.call)(retrying_client(&server, route, 3))
        .await
        .expect("a 425 is the matching engine restarting, retried to the success");
    mock.assert_async().await;
}

#[tokio::test]
async fn a_refused_relay_call_sends_nothing() {
    // A refusal made before sending is `Api(Validation)`, and the server never
    // sees the request.
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/transactions")
        .match_query(Matcher::Any)
        .expect(0)
        .create_async()
        .await;
    let err = client_unauthed(&server)
        .list_transactions()
        .await
        .expect_err("no auth is configured");
    assert!(
        matches!(
            err,
            polyoxide_relay::RelayError::Api(polyoxide_core::ApiError::Validation(_))
        ),
        "{err:?}"
    );
    mock.assert_async().await;
}

// ── What each attempt is signed over ────────────────────────────

/// One served request: its `POLY_BUILDER_TIMESTAMP`, its
/// `POLY_BUILDER_SIGNATURE` and its body.
type Attempt = (u64, String, String);

/// The signature and body of each request a mock served, in order.
#[derive(Clone, Default)]
struct Attempts(std::sync::Arc<std::sync::Mutex<Vec<Attempt>>>);

impl Attempts {
    fn record(&self, request: &mockito::Request) {
        let header = |name: &str| request.header(name)[0].to_str().unwrap().to_owned();
        let timestamp = header("poly_builder_timestamp").parse().unwrap();
        let signature = header("poly_builder_signature");
        let body = String::from_utf8_lossy(request.body().unwrap()).into_owned();
        self.0.lock().unwrap().push((timestamp, signature, body));
    }

    fn all(&self) -> Vec<Attempt> {
        self.0.lock().unwrap().clone()
    }
}

/// A mock for `route` answering 429 with `Retry-After: 1.1` once, then 200,
/// recording every attempt. The wait puts the attempts in different seconds,
/// so a fresh signature has a different timestamp from the first.
async fn throttled_once(
    server: &mut mockito::ServerGuard,
    route: &Route,
    attempts: &Attempts,
) -> mockito::Mock {
    let attempts = attempts.clone();
    let served = std::sync::atomic::AtomicUsize::new(0);
    server
        .mock(route.method, route.path)
        .match_query(Matcher::Any)
        .with_status_code_from_request(move |request| {
            attempts.record(request);
            match served.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => 429,
                _ => 200,
            }
        })
        .with_header("retry-after", "1.1")
        .with_header("content-type", "application/json")
        .with_body(route.body)
        .expect(2)
        .create_async()
        .await
}

/// The Builder HMAC the relayer recomputes for one attempt: the secret
/// `c2VjcmV0` over the attempt's own timestamp, the method, the path and the
/// body.
fn builder_signature(timestamp: u64, method: &str, path: &str, body: Option<&str>) -> String {
    use polyoxide_core::{Base64Format, Signer};

    Signer::new("c2VjcmV0")
        .sign(
            &Signer::create_message(timestamp, method, path, body),
            Base64Format::UrlSafe,
        )
        .unwrap()
}

/// The route in [`every_relay_route`] at `method` and `path`.
fn relay_route(method: &str, path: &str) -> Route {
    every_relay_route()
        .into_iter()
        .find(|route| route.method == method && route.path == path)
        .unwrap()
}

#[tokio::test]
async fn a_retried_relay_post_is_signed_over_its_path_and_body() {
    let route = relay_route("POST", "/submit");
    let mut server = Server::new_async().await;
    let attempts = Attempts::default();
    let mock = throttled_once(&mut server, &route, &attempts).await;

    (route.call)(retrying_client(&server, &route, 1))
        .await
        .unwrap();
    mock.assert_async().await;

    let attempts = attempts.all();
    assert_eq!(attempts.len(), 2);
    for (timestamp, signature, body) in &attempts {
        assert!(!body.is_empty(), "a submit carries its batch");
        assert_eq!(
            *signature,
            builder_signature(*timestamp, "POST", "/submit", Some(body)),
            "the signature covers this attempt's own timestamp, the path and the body"
        );
    }
    assert_eq!(attempts[0].2, attempts[1].2, "the body is resent as is");
    assert_ne!(
        attempts[0].0, attempts[1].0,
        "the retry was signed afresh, a second later"
    );
}

#[tokio::test]
async fn a_relay_get_is_signed_over_its_path() {
    let route = relay_route("GET", "/transactions");
    let mut server = Server::new_async().await;
    let attempts = Attempts::default();
    let mock = throttled_once(&mut server, &route, &attempts).await;

    (route.call)(retrying_client(&server, &route, 1))
        .await
        .unwrap();
    mock.assert_async().await;

    let attempts = attempts.all();
    assert_eq!(attempts.len(), 2);
    for (timestamp, signature, body) in &attempts {
        assert!(body.is_empty(), "a GET carries no body: {body:?}");
        assert_eq!(
            *signature,
            builder_signature(*timestamp, "GET", "/transactions", None),
            "the signature covers this attempt's own timestamp, GET and the path"
        );
    }
    assert_ne!(
        attempts[0].0, attempts[1].0,
        "the retry was signed afresh, a second later"
    );
}

#[tokio::test]
async fn a_relayer_refusal_keeps_its_reason() {
    // A non-2xx is classed by its status and keeps the relayer's reason: the
    // `error` field of a JSON body, or the body itself.
    let owner: alloy::primitives::Address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
        .parse()
        .unwrap();

    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/nonce")
        .match_query(Matcher::Any)
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"insufficient funds"}"#)
        .expect(1)
        .create_async()
        .await;
    let err = client_unauthed(&server)
        .get_nonce(owner)
        .await
        .expect_err("the relayer refused it");
    match &err {
        polyoxide_relay::RelayError::Api(polyoxide_core::ApiError::Validation(message)) => {
            assert_eq!(message, "insufficient funds")
        }
        other => panic!("expected Api(Validation), got {other:?}"),
    }
    mock.assert_async().await;

    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", "/nonce")
        .match_query(Matcher::Any)
        .with_status(500)
        .with_body("upstream exploded")
        .expect(1)
        .create_async()
        .await;
    let err = client_unauthed(&server)
        .get_nonce(owner)
        .await
        .expect_err("the relayer failed");
    match &err {
        polyoxide_relay::RelayError::Api(polyoxide_core::ApiError::Api { status, message }) => {
            assert_eq!(*status, 500);
            assert!(message.contains("upstream exploded"), "{message:?}");
        }
        other => panic!("expected Api(Api {{ status: 500 }}), got {other:?}"),
    }
    mock.assert_async().await;
}

#[tokio::test]
async fn a_session_signer_post_outlasts_the_client_timeout() {
    // The two session-signer posts wait five minutes, as py-sdk's do, because
    // the relayer broadcasts the batch before it answers. A client that gives
    // up after 100ms, and a relayer that answers the authorization after 400ms.
    let v = relay_vectors();
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/v1/session-signers/authorizations")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body_from_request(|_| {
            std::thread::sleep(std::time::Duration::from_millis(400));
            br#"{"operationId":"op-1","status":"SUBMITTED","transactionHash":null,"transactionId":"tx-9"}"#
                .to_vec()
        })
        .expect(1)
        .create_async()
        .await;

    let config = BuilderConfig::new("builder-key".into(), "c2VjcmV0".into(), Some("pp".into()));
    let account = BuilderAccount::new(TEST_PRIVATE_KEY, Some(config)).unwrap();
    let wallet: alloy::primitives::Address = v["wallet"].as_str().unwrap().parse().unwrap();
    let client = RelayClient::builder()
        .expect("builder")
        .url(&server.url())
        .expect("valid mock URL")
        .timeout_ms(100)
        .with_account(account)
        .wallet_type(polyoxide_relay::WalletType::DepositWallet)
        .deposit_wallet(wallet)
        .build()
        .expect("build client");

    let session: alloy::primitives::Address =
        v["session_signer"].as_str().unwrap().parse().unwrap();
    let (_, request) = client
        .authorize_session_signer_typed_data_with_valid_until(
            wallet,
            session,
            vec![polyoxide_core::SessionSignerScope::Clob],
            1815534000,
            4,
            1800000600,
        )
        .unwrap();
    let resp = client
        .submit_session_signer_authorization(
            &request,
            v["authorize_batch"]["signature"].as_str().unwrap(),
            "idem-1",
        )
        .await
        .expect("the post waits past the client's 100ms timeout");
    assert_eq!(resp.transaction_id, "tx-9");
    mock.assert_async().await;
}

#[tokio::test]
async fn a_retried_ping_reports_the_answering_attempt() {
    // Story 3.7: the ping runs on `HttpClient::health`, so its latency is the
    // round trip of the attempt that answered, as every venue's is. It used
    // to time the whole call: the permit, relay's pacing and the retry's
    // backoff. The base URL's path prefix is kept.
    let mut server = Server::new_async().await;
    let served = std::sync::atomic::AtomicUsize::new(0);
    let mock = server
        .mock("GET", "/prefix/")
        .with_status_code_from_request(move |_| {
            match served.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => 429,
                _ => 200,
            }
        })
        .with_body("ok")
        .expect(2)
        .create_async()
        .await;
    let client = RelayClient::builder()
        .unwrap()
        .url(&format!("{}/prefix/", server.url()))
        .unwrap()
        .with_retry_config(polyoxide_core::RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 400,
            max_backoff_ms: 10_000,
        })
        .build()
        .unwrap();

    let start = std::time::Instant::now();
    let latency = client.ping().await.expect("retried to the 200");
    let elapsed = start.elapsed();
    mock.assert_async().await;
    assert!(
        elapsed >= std::time::Duration::from_millis(300),
        "the call took {elapsed:?}, inside the retry's 300ms floor"
    );
    assert!(
        latency < std::time::Duration::from_millis(300),
        "the latency is the answering attempt's, without the backoff: {latency:?}"
    );
}
