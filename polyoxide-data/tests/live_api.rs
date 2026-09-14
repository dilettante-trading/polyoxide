//! Live integration tests against the Polymarket Data API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-data --test live_api -- --ignored
//! ```

use polyoxide_data::api::holders::MarketHolders;
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

/// Whether `condition_id` has the shape `GET /holders` validates `market`
/// against: `0x` followed by exactly 64 hex digits.
///
/// The check runs before any lookup, and a value that fails it is reported as
/// `required query param 'market' not provided` — the *missing*-parameter
/// message, not a malformed-value one. A caller reading that error has no way
/// to tell it sent a bad id rather than none.
fn is_hash64(condition_id: &str) -> bool {
    condition_id.len() == 66
        && condition_id.starts_with("0x")
        && condition_id[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Pins the shape rule above against values taken from the live trade feed.
///
/// Not `#[ignore]`d: it needs no network, and the reject case is the actual
/// `conditionId` that broke `live_holders` in the 2026-08-26 nightly (issue
/// #32) — `0x` plus 62 hex digits, zero-padded on the right.
#[test]
fn hash64_shape_matches_what_holders_accepts() {
    assert!(is_hash64(
        "0x94f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f90"
    ));

    // Straight from `GET /trades`. 62 hex digits, so 64 characters, not 66.
    assert!(!is_hash64(
        "0x03474f36a86039e6c40479b1844401d81a0000000000000000000000000000"
    ));
    // Missing prefix, over-long, non-hex, and the empty string an absent field
    // would produce — the venue rejects every one of these identically.
    assert!(!is_hash64(
        "94f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f90"
    ));
    assert!(!is_hash64(
        "0x94f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f900"
    ));
    assert!(!is_hash64(
        "0xZZ4f56a80d387a41395ae464e5e3eb2e23d1a6032014b40590074b75aa3447f9"
    ));
    assert!(!is_hash64(""));
}

/// Page size the market probe asks for, and the bound `live_holders` checks.
const HOLDERS_PROBE_LIMIT: u32 = 5;

/// Picks a market `GET /holders` returns rows for, and hands back those rows.
///
/// Taking `trades[0].condition_id` is not enough, and assuming otherwise is
/// what filed issue #32. The trade feed carries ids of two different shapes:
/// most are Hash64, but some are `0x` + 62 hex, and the latter fail the
/// `market` validation described on [`is_hash64`]. There is a second trap
/// behind that one — a well-formed id the holders index has never seen
/// answers with a bare `null`, which the SDK reads as an empty list. Neither
/// is visible from the trade alone, so probe rather than predict, and select
/// on rows actually coming back.
///
/// The rows are returned rather than re-fetched because `/holders` is
/// CDN-cached for 120 seconds **per URL**. A second request with a different
/// `limit` is a different cache entry and can still hold a `null` captured
/// before the market was indexed — which is exactly what failed the
/// 2026-09-13 nightly (issue #37): the `limit=1` probe answered, and the
/// default-limit request made milliseconds later got `null`. A precondition
/// established on one URL says nothing about another.
async fn holders_market(client: &DataApi) -> (String, Vec<MarketHolders>) {
    const MAX_PROBES: usize = 10;

    let trades = client
        .trades()
        .list()
        .limit(100)
        .send()
        .await
        .expect("trades for holders test");
    assert!(
        !trades.is_empty(),
        "need at least one trade for holders test"
    );

    let mut seen: Vec<String> = Vec::new();
    for trade in &trades {
        let condition_id = trade.condition_id.as_str();
        if !is_hash64(condition_id) || seen.iter().any(|s| s == condition_id) {
            continue;
        }
        seen.push(condition_id.to_string());

        // Only well-formed ids reach here, so an error is a real failure —
        // a shape the SDK cannot decode, or an outage — and not a reason to
        // try the next candidate.
        let holders = client
            .holders()
            .list(vec![condition_id])
            .limit(HOLDERS_PROBE_LIMIT)
            .send()
            .await
            .expect("holders should deserialize");
        if !holders.is_empty() {
            return (condition_id.to_string(), holders);
        }
        if seen.len() >= MAX_PROBES {
            break;
        }
    }

    panic!(
        "no qualifying market among the {} most recent trades: probed {} \
         distinct Hash64 condition ids and /holders returned rows for none. \
         Market conditions rather than a defect, so re-run before concluding \
         otherwise",
        trades.len(),
        seen.len()
    );
}

#[tokio::test]
#[ignore]
async fn live_holders() {
    let client = client();

    let (_, holders) = holders_market(&client).await;
    for market in &holders {
        assert!(!market.token.is_empty(), "every entry names its token");
        assert!(
            market.holders.len() <= HOLDERS_PROBE_LIMIT as usize,
            "limit={HOLDERS_PROBE_LIMIT} must bound rows per token, got {} for token {}",
            market.holders.len(),
            market.token
        );
    }
}

/// Pins the actual `limit` contract for `GET /holders`.
///
/// Both the SDK doc comment and the spec mirror were wrong here, in opposite
/// directions: the SDK claimed a default of 100 (it is 20) and the mirror
/// claimed a maximum of 20 (it is 500). Re-run this rather than re-deriving it.
///
/// The cap is enforced by clamping, not by rejection — see the `limit=5000`
/// case below for when that changed.
#[tokio::test]
#[ignore]
async fn live_holders_limit_bounds() {
    let client = client();

    let (condition_id, _) = holders_market(&client).await;

    // Omitting `limit` yields the server default of 20, not 100. Bounded
    // rather than asserted non-empty: this URL has its own cache entry, and
    // can serve a stale miss for a market the probe just saw rows for.
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

    // Above the ceiling the venue clamps rather than rejects. This changed
    // upstream between 2026-07-25 and 2026-08-03: `limit=501` used to return
    // HTTP 400 `{"error":"max holders limit of 500 exceeded"}` and now returns
    // 200 with the response silently truncated to 500 rows per token.
    //
    // Asserted as an upper bound, not an equality: the sampled market may hold
    // fewer than 500 positions, in which case a clamped and an unclamped
    // response are indistinguishable. What must never happen is a token coming
    // back with more rows than the cap.
    let over = client
        .holders()
        .list(vec![condition_id.as_str()])
        .limit(5000)
        .send()
        .await
        .expect("limit above the ceiling is clamped, not rejected");
    for market in &over {
        assert!(
            market.holders.len() <= 500,
            "limit=5000 must clamp to 500, got {} for token {}",
            market.holders.len(),
            market.token
        );
    }

    // `limit=0` is answered with a bare `null` body rather than `[]`, which
    // the SDK reads as an empty list. Unlike the requests above this one
    // cannot be a stale cache entry masking rows: its response is `null` for
    // every market, so rows here would mean the venue changed what 0 means.
    let zero = client
        .holders()
        .list(vec![condition_id.as_str()])
        .limit(0)
        .send()
        .await
        .expect("limit=0 must read as an empty list, not an error");
    assert!(
        zero.is_empty(),
        "limit=0 should return no rows, got {} markets",
        zero.len()
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

// ── Data API v2 ──────────────────────────────────────────────────
//
// Inputs are chosen live, never hardcoded: a wallet from the bare trade feed,
// that trade's market and token, a resolved market from the winners board.

mod v2 {
    use futures_util::StreamExt;
    use polyoxide_data::v2::types::{
        PnlFidelity, PnlInterval, PositionAnchor, PositionStatus, PricesInterval, ResolutionKey,
        TimePeriod,
    };
    use polyoxide_data::v2::ErrorCode;
    use polyoxide_data::{DataApi, DataApiError};

    use super::client;

    /// A recent trade's wallet, condition id and token id.
    async fn recent_trade(data: &DataApi) -> (String, String, String) {
        let page = data.v2().trades().limit(1).send().await.expect("v2 trades");
        let trade = page
            .data
            .into_iter()
            .next()
            .expect("the bare feed is never empty");
        (trade.proxy_wallet, trade.condition_id, trade.token_id)
    }

    /// A wallet absent from every dataset: random, so it cannot collide with
    /// an address that appears on chain (`0x…0001` does, and is "known").
    fn unknown_wallet() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!(
            "0x{:040x}",
            nanos ^ 0x5eed_5eed_5eed_5eed_5eed_5eed_5eed_5eed
        )
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_trades_walk_follows_the_cursor() {
        let data = client();
        let pages: Vec<_> = data.v2().trades().limit(2).pages().take(2).collect().await;

        assert_eq!(pages.len(), 2);
        let first = pages[0].as_ref().expect("page 1");
        let second = pages[1].as_ref().expect("page 2");
        assert!(
            first.pagination.next_cursor.is_some(),
            "the feed has more than 2 rows"
        );
        assert_ne!(
            first.data[0].transaction_hash, second.data[0].transaction_hash,
            "page 2 repeated page 1"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_wallet_routes() {
        let data = client();
        let (wallet, _, _) = recent_trade(&data).await;
        let v2 = data.v2();

        v2.activity(&wallet)
            .limit(2)
            .send()
            .await
            .expect("activity");
        v2.combo_activity(&wallet)
            .limit(2)
            .send()
            .await
            .expect("combo activity");
        v2.positions(wallet.as_str())
            .limit(2)
            .send()
            .await
            .expect("positions");
        v2.positions(wallet.as_str())
            .status(PositionStatus::Closed)
            .limit(2)
            .send()
            .await
            .expect("closed positions");
        v2.combo_positions(&wallet)
            .limit(2)
            .send()
            .await
            .expect("combo positions");
        v2.approvals(&wallet).send().await.expect("approvals");
        let pnl = v2
            .user_pnl(&wallet)
            .interval(PnlInterval::OneWeek)
            .fidelity(PnlFidelity::OneDay)
            .send()
            .await
            .expect("user pnl");
        assert_eq!(pnl.proxy_wallet.to_lowercase(), wallet.to_lowercase());
        let stats = v2.user_stats(&wallet).send().await.expect("user stats");
        assert!(stats.is_some(), "a wallet that just traded is a known user");
        v2.user_volume(&wallet).send().await.expect("user volume");
        v2.value(&wallet).send().await.expect("value");
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_unknown_wallet_is_none_not_an_error() {
        let data = client();
        let wallet = unknown_wallet();

        assert!(data
            .v2()
            .user_stats(&wallet)
            .send()
            .await
            .expect("user stats")
            .is_none());
        assert!(data
            .v2()
            .leaderboard_user(&wallet)
            .send()
            .await
            .expect("leaderboard user")
            .is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_market_routes() {
        let data = client();
        let (_, condition, token_id) = recent_trade(&data).await;
        let v2 = data.v2();

        v2.holders([condition.as_str()])
            .limit(2)
            .send()
            .await
            .expect("holders");
        v2.positions(PositionAnchor::Condition(condition.clone()))
            .limit(2)
            .send()
            .await
            .expect("market positions");
        v2.open_interest()
            .conditions([condition.as_str()])
            .send()
            .await
            .expect("open interest");
        let global = v2
            .open_interest()
            .send()
            .await
            .expect("global open interest");
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].condition_id, "GLOBAL");
        v2.prices_history(&token_id)
            .interval(PricesInterval::OneDay)
            .limit(10)
            .send()
            .await
            .expect("prices history");

        let winners = v2
            .biggest_winners()
            .time_period(TimePeriod::Week)
            .limit(5)
            .send()
            .await
            .expect("winners");
        let resolved = winners
            .data
            .iter()
            .find(|w| w.kind == "market")
            .expect("a market win on the weekly board");
        let rows = v2
            .resolutions(ResolutionKey::Conditions(vec![resolved
                .condition_id
                .clone()]))
            .send()
            .await
            .expect("resolutions");
        assert!(
            !rows.is_empty(),
            "a market on the winners board has resolved"
        );
        v2.live_volume([resolved.event_id])
            .send()
            .await
            .expect("live volume");
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_board_routes() {
        let data = client();
        let v2 = data.v2();

        let board = v2
            .leaderboard()
            .time_period(TimePeriod::Week)
            .limit(2)
            .send()
            .await
            .expect("leaderboard");
        let leader = &board
            .data
            .first()
            .expect("the weekly board is never empty")
            .user_id;
        let standing = v2
            .leaderboard_user(leader)
            .time_period(TimePeriod::Week)
            .send()
            .await
            .expect("leaderboard user")
            .expect("the board's leader has a standing");
        assert!(standing.rank_pnl.is_some() || standing.rank_volume.is_some());
        v2.builders_leaderboard()
            .limit(2)
            .send()
            .await
            .expect("builders leaderboard");
        v2.builder_volume()
            .limit(2)
            .send()
            .await
            .expect("builder volume");
        v2.status().send().await.expect("status");
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_errors_are_structured() {
        let data = client();
        let err = data
            .v2()
            .trades()
            .cursor("garbage")
            .send()
            .await
            .unwrap_err();

        let DataApiError::V2(v2) = &err else {
            panic!("expected a structured v2 error, got {err:?}");
        };
        assert_eq!(v2.status, 400);
        assert_eq!(v2.code, ErrorCode::InvalidRequest);
        assert!(!v2.trace_id.is_empty());
        assert!(!err.is_retriable());
    }
}
