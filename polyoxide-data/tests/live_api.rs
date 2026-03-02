//! Live integration tests against the Polymarket Data API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-data --test live_api -- --ignored
//! ```

use polyoxide_data::DataApi;
use std::time::Duration;

fn client() -> DataApi {
    DataApi::new().expect("data api client")
}

// An address to test user endpoints (doesn't need to be active)
const TEST_USER: &str = "0x0000000000000000000000000000000000000001";

// ── Health ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_health_check() {
    let client = client();
    let health = client.health().check().await.expect("health check");
    assert_eq!(health.data, "OK", "health response should be OK");
}

#[tokio::test]
#[ignore]
async fn live_ping() {
    let client = client();
    let latency = client.health().ping().await.expect("ping");
    assert!(
        latency < Duration::from_secs(10),
        "latency too high: {:?}",
        latency
    );
}

// ── Open Interest ────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_open_interest() {
    let client = client();
    let oi = client
        .open_interest()
        .get()
        .send()
        .await
        .expect("open interest");
    assert!(!oi.is_empty(), "should return at least one market's OI");
}

// ── Trades ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_trades() {
    let client = client();
    let trades = client
        .trades()
        .list()
        .limit(5)
        .send()
        .await
        .expect("list trades");
    assert!(!trades.is_empty(), "should return at least one trade");
}

// ── User endpoints ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_user_traded() {
    let client = client();
    // Verify the endpoint responds and deserializes correctly
    let traded = client
        .user(TEST_USER)
        .traded()
        .await
        .expect("user traded should deserialize");
    assert_eq!(traded.user, TEST_USER, "should echo back the user address");
}

#[tokio::test]
#[ignore]
async fn live_user_positions() {
    let client = client();
    // Just verify the endpoint responds and deserializes — user may have 0 open positions
    let _positions = client
        .user(TEST_USER)
        .list_positions()
        .limit(5)
        .send()
        .await
        .expect("list positions should succeed");
}

// ── Builders ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_builder_leaderboard() {
    let client = client();
    let leaderboard = client
        .builders()
        .leaderboard()
        .limit(5)
        .send()
        .await
        .expect("builder leaderboard");
    assert!(
        !leaderboard.is_empty(),
        "should return at least one builder"
    );
}

// ── User: positions_value ───────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_user_positions_value() {
    let client = client();
    // Just verify the endpoint responds and deserializes — user may have 0 value
    let _value = client
        .user(TEST_USER)
        .positions_value()
        .send()
        .await
        .expect("positions value should deserialize");
}

// ── User: closed_positions ──────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_user_closed_positions() {
    let client = client();
    let _closed = client
        .user(TEST_USER)
        .closed_positions()
        .limit(5)
        .send()
        .await
        .expect("closed positions should deserialize");
}

// ── User: trades ────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_user_trades() {
    let client = client();
    let _trades = client
        .user(TEST_USER)
        .trades()
        .limit(5)
        .send()
        .await
        .expect("user trades should deserialize");
}

// ── User: activity ──────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_user_activity() {
    let client = client();
    let _activity = client
        .user(TEST_USER)
        .activity()
        .limit(5)
        .send()
        .await
        .expect("user activity should deserialize");
}

// ── Holders ─────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_holders() {
    let client = client();

    // Get a valid condition_id from recent trades
    let trades = client
        .trades()
        .list()
        .limit(1)
        .send()
        .await
        .expect("trades for holders test");
    assert!(
        !trades.is_empty(),
        "need at least one trade for holders test"
    );

    let condition_id = &trades[0].condition_id;
    let holders = client
        .holders()
        .list(vec![condition_id.as_str()])
        .limit(5)
        .send()
        .await
        .expect("holders should deserialize");
    assert!(
        !holders.is_empty(),
        "should return at least one market's holders"
    );
}

// ── Live Volume ─────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_live_volume() {
    let client = client();

    // Use event_id 1 — the API should return results or an empty list
    // for any valid numeric event ID without erroring
    let _volume = client
        .live_volume()
        .get(1)
        .await
        .expect("live volume should deserialize");
}

// ── Trader Leaderboard ──────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_trader_leaderboard() {
    let client = client();
    let leaderboard = client
        .leaderboard()
        .get()
        .limit(5)
        .send()
        .await
        .expect("trader leaderboard");
    assert!(!leaderboard.is_empty(), "should return at least one trader");
}

// ── Builders: volume ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_builder_volume() {
    let client = client();
    let volume = client
        .builders()
        .volume()
        .send()
        .await
        .expect("builder volume");
    assert!(
        !volume.is_empty(),
        "should return at least one builder volume entry"
    );
}
