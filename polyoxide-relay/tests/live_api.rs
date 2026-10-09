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
use polyoxide_test_support::{load_env, optional_env, ResultExt};
use std::time::Duration;

fn client() -> RelayClient {
    RelayClient::builder()
        .or_fail("default builder URL is valid")
        .build()
        .or_fail("relay client should build without account")
}

/// Build a relay client using builder HMAC credentials from the environment.
/// Fails the test as `auth-gated` when any required variable is absent or
/// empty, which the nightly skips silently.
fn client_with_builder_env() -> RelayClient {
    let creds = load_env(&[
        "POLYMARKET_PRIVATE_KEY",
        "BUILDER_API_KEY",
        "BUILDER_SECRET",
    ])
    .unwrap_or_else(|missing| missing.or_auth_gated());
    let passphrase = optional_env("BUILDER_PASS_PHRASE");

    let config = BuilderConfig::new(
        creds.get("BUILDER_API_KEY").to_owned(),
        creds.get("BUILDER_SECRET").to_owned(),
        passphrase,
    );
    let account = BuilderAccount::new(creds.get("POLYMARKET_PRIVATE_KEY"), Some(config))
        .or_fail("builder account from the environment");
    RelayClient::builder()
        .or_fail("default builder URL is valid")
        .with_account(account)
        .build()
        .or_fail("relay client should build with builder credentials")
}

/// Build a relay client using static relayer API key credentials from the environment.
/// Fails the test as `auth-gated` when any required variable is absent or
/// empty, which the nightly skips silently.
fn client_with_relayer_api_key_env() -> RelayClient {
    let creds = load_env(&[
        "POLYMARKET_PRIVATE_KEY",
        "RELAYER_API_KEY",
        "RELAYER_API_KEY_ADDRESS",
    ])
    .unwrap_or_else(|missing| missing.or_auth_gated());

    let account = BuilderAccount::with_relayer_api_key(
        creds.get("POLYMARKET_PRIVATE_KEY"),
        creds.get("RELAYER_API_KEY").to_owned(),
        creds.get("RELAYER_API_KEY_ADDRESS").to_owned(),
    )
    .or_fail("relayer API key account from the environment");
    RelayClient::builder()
        .or_fail("default builder URL is valid")
        .with_account(account)
        .build()
        .or_fail("relay client should build with a relayer API key")
}

// ── Health ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_ping() {
    let client = client();
    let latency = client.ping().await.or_fail("ping should succeed");
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
        .or_fail("get_deployed should succeed for zero address");
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
        .expect("valid address"); // live-unwraps: parses a constant
    let client = client();
    let deployed = client
        .get_deployed(addr)
        .await
        .or_fail("get_deployed should deserialize");
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
        .or_fail("get_nonce should succeed for zero address");
    assert_eq!(nonce, 0, "zero address should have nonce 0");
}

// ── Transactions list ───────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_transactions_with_builder_auth() {
    let client = client_with_builder_env();
    let txs = client
        .list_transactions()
        .await
        .or_fail("list_transactions should succeed");
    // Just assert deserialization succeeded; count depends on user activity.
    let _ = txs;
}

#[tokio::test]
#[ignore]
async fn live_list_transactions_with_relayer_api_key() {
    let client = client_with_relayer_api_key_env();
    let txs = client
        .list_transactions()
        .await
        .or_fail("list_transactions should succeed");
    let _ = txs;
}

// ── Relayer API keys list ──────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_relayer_api_keys() {
    let client = client_with_relayer_api_key_env();
    let keys = client
        .list_relayer_api_keys()
        .await
        .or_fail("list_relayer_api_keys should succeed");
    // OpenAPI guarantees an empty array is valid, so no count check - just deserialization.
    let _ = keys;
}
