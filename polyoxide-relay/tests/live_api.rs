//! Live integration tests against the Polymarket Relay API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-relay --test live_api -- --ignored
//! ```

use alloy::primitives::Address;
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient};
use std::time::Duration;

fn client() -> RelayClient {
    RelayClient::builder()
        .expect("default builder URL is valid")
        .build()
        .expect("relay client should build without account")
}

/// Build a relay client using builder HMAC credentials from the environment.
/// Returns `None` if the required env vars are missing.
fn client_with_builder_env() -> Option<RelayClient> {
    let _ = dotenvy::dotenv();

    let private_key = std::env::var("POLYMARKET_PRIVATE_KEY").ok()?;
    let key = std::env::var("BUILDER_API_KEY").ok()?;
    let secret = std::env::var("BUILDER_SECRET").ok()?;
    let passphrase = std::env::var("BUILDER_PASS_PHRASE").ok();

    let config = BuilderConfig::new(key, secret, passphrase);
    let account = BuilderAccount::new(private_key, Some(config)).ok()?;
    RelayClient::builder().ok()?.with_account(account).build().ok()
}

/// Build a relay client using static relayer API key credentials from the environment.
/// Returns `None` if the required env vars are missing.
fn client_with_relayer_api_key_env() -> Option<RelayClient> {
    let _ = dotenvy::dotenv();

    let private_key = std::env::var("POLYMARKET_PRIVATE_KEY").ok()?;
    let key = std::env::var("RELAYER_API_KEY").ok()?;
    let address = std::env::var("RELAYER_API_KEY_ADDRESS").ok()?;

    let account = BuilderAccount::with_relayer_api_key(private_key, key, address).ok()?;
    RelayClient::builder().ok()?.with_account(account).build().ok()
}

// ── Health ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_ping() {
    let client = client();
    let latency = client.ping().await.expect("ping should succeed");
    assert!(
        latency < Duration::from_secs(10),
        "latency too high: {:?}",
        latency
    );
}

// ── Deployed ────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_get_deployed_zero_address() {
    let client = client();
    let deployed = client
        .get_deployed(Address::ZERO)
        .await
        .expect("get_deployed should succeed for zero address");
    // The zero address is almost certainly not a deployed Safe
    assert!(!deployed, "zero address should not be deployed");
}

#[tokio::test]
#[ignore]
async fn live_get_deployed_known_address() {
    // Use the Safe factory address itself as a test -- it exists on-chain
    // but is not a deployed Safe wallet, so result should be false.
    let addr: Address = "0xaacFeEa03eb1561C4e67d661e40682Bd20E3541b"
        .parse()
        .expect("valid address");
    let client = client();
    let deployed = client
        .get_deployed(addr)
        .await
        .expect("get_deployed should deserialize");
    // We just care that it returns a bool without error
    let _ = deployed;
}

// ── Nonce ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_get_nonce() {
    let client = client();
    // Query nonce for the zero address -- should succeed and return 0
    let nonce = client
        .get_nonce(Address::ZERO)
        .await
        .expect("get_nonce should succeed for zero address");
    assert_eq!(nonce, 0, "zero address should have nonce 0");
}

// ── Transactions list ───────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_transactions_with_builder_auth() {
    let Some(client) = client_with_builder_env() else {
        eprintln!("skipping: POLYMARKET_PRIVATE_KEY / BUILDER_* env vars not set");
        return;
    };
    let txs = client
        .list_transactions()
        .await
        .expect("list_transactions should succeed");
    // Just assert deserialization succeeded; count depends on user activity.
    let _ = txs;
}

#[tokio::test]
#[ignore]
async fn live_list_transactions_with_relayer_api_key() {
    let Some(client) = client_with_relayer_api_key_env() else {
        eprintln!("skipping: POLYMARKET_PRIVATE_KEY / RELAYER_API_KEY* env vars not set");
        return;
    };
    let txs = client
        .list_transactions()
        .await
        .expect("list_transactions should succeed");
    let _ = txs;
}

// ── Relayer API keys list ──────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_relayer_api_keys() {
    let Some(client) = client_with_relayer_api_key_env() else {
        eprintln!("skipping: POLYMARKET_PRIVATE_KEY / RELAYER_API_KEY* env vars not set");
        return;
    };
    let keys = client
        .list_relayer_api_keys()
        .await
        .expect("list_relayer_api_keys should succeed");
    // OpenAPI guarantees an empty array is valid, so no count check - just deserialization.
    let _ = keys;
}
