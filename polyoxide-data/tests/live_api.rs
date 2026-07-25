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

/// Pins the actual `limit` contract for `GET /holders`.
///
/// Both the SDK doc comment and the spec mirror were wrong here, in opposite
/// directions: the SDK claimed a default of 100 (it is 20) and the mirror
/// claimed a maximum of 20 (it is 500). Re-run this rather than re-deriving it.
#[tokio::test]
#[ignore]
async fn live_holders_limit_bounds() {
    let client = client();

    let trades = client
        .trades()
        .list()
        .limit(1)
        .send()
        .await
        .expect("trades for holders test");
    let condition_id = trades
        .first()
        .map(|t| t.condition_id.clone())
        .expect("need at least one trade");

    // Omitting `limit` yields the server default of 20, not 100.
    let defaulted = client
        .holders()
        .list(vec![condition_id.as_str()])
        .send()
        .await
        .expect("holders with default limit");
    if let Some(market) = defaulted.first() {
        assert!(
            market.holders.len() <= 20,
            "server default should be 20, got {}",
            market.holders.len()
        );
    }

    // 100 is accepted — the mirror's claimed 0-20 range was wrong.
    client
        .holders()
        .list(vec![condition_id.as_str()])
        .limit(100)
        .send()
        .await
        .expect("limit=100 must be accepted");

    // 500 is the documented ceiling and is accepted.
    client
        .holders()
        .list(vec![condition_id.as_str()])
        .limit(500)
        .send()
        .await
        .expect("limit=500 must be accepted");

    // 501 is rejected — this is what establishes 500 as the real cap.
    let over = client
        .holders()
        .list(vec![condition_id.as_str()])
        .limit(501)
        .send()
        .await;
    assert!(
        over.is_err(),
        "limit=501 should be rejected with 'max holders limit of 500 exceeded'"
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

// ── Market Positions ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_market_positions() {
    let client = client();

    // Pick a live condition_id from recent trades.
    let trades = client
        .trades()
        .list()
        .limit(1)
        .send()
        .await
        .expect("trades for market_positions test");
    assert!(
        !trades.is_empty(),
        "need at least one trade for market_positions test"
    );
    let condition_id = &trades[0].condition_id;

    // Just verify the endpoint responds and deserializes.
    let _positions = client
        .market_positions()
        .list(condition_id)
        .limit(5)
        .send()
        .await
        .expect("market positions should deserialize");
}

// ── Accounting Snapshot ─────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_accounting_snapshot() {
    let client = client();
    let bytes = client
        .accounting()
        .snapshot(TEST_USER)
        .await
        .expect("accounting snapshot should succeed");
    // ZIP archives start with the local-file-header signature "PK\x03\x04".
    // The Polymarket API may return a minimal archive for empty users, but the
    // header bytes must still be present.
    assert!(
        bytes.len() >= 4,
        "expected at least ZIP header bytes, got {} bytes",
        bytes.len()
    );
    assert_eq!(
        &bytes[0..2],
        b"PK",
        "response should start with ZIP signature"
    );
}

// --- Undocumented sibling hosts -------------------------------------------
//
// user-pnl-api and lb-api have no published OpenAPI spec, so these live tests
// are the only contract check we have. If the shapes drift, these fail here
// rather than silently in a user's deserialize.

#[tokio::test]
#[ignore = "hits live API"]
async fn live_user_pnl_series() {
    let client = client();
    // A high-volume trader, so the series is non-empty.
    let points = client
        .pnl()
        .history("0xcd30f4698c6f5f3829893e68e183a8e5ea18f316")
        .interval("1d")
        .fidelity(polyoxide_data::types::PnlFidelity::OneHour)
        .send()
        .await
        .expect("user-pnl should succeed");

    assert!(!points.is_empty(), "expected a non-empty PnL series");
    assert!(
        points[0].timestamp > 1_000_000_000,
        "timestamp should be Unix seconds, got {}",
        points[0].timestamp
    );
}

#[tokio::test]
#[ignore = "hits live API"]
async fn live_rankings_volume_and_profit() {
    let client = client();

    let volume = client
        .rankings()
        .volume()
        .window(polyoxide_data::types::RankingWindow::All)
        .limit(3)
        .send()
        .await
        .expect("rankings volume should succeed");
    assert!(!volume.is_empty(), "expected ranked entries");
    assert!(
        volume[0]
            .proxy_wallet
            .as_deref()
            .is_some_and(|w| w.starts_with("0x")),
        "expected a 0x proxy wallet, got {:?}",
        volume[0].proxy_wallet
    );

    let profit = client
        .rankings()
        .profit()
        .window(polyoxide_data::types::RankingWindow::SevenDays)
        .limit(3)
        .send()
        .await
        .expect("rankings profit should succeed");
    assert!(!profit.is_empty(), "expected ranked entries");
}
