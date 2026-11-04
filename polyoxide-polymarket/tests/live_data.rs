//! Live integration tests against the Polymarket Data API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-polymarket --features data --test live_data -- --ignored
//! ```
//!
//! Polymarket retired Data API v1 on 2026-11-02, so every test here reads v2.

use polyoxide_polymarket::data::DataApi;
use polyoxide_test_support::ResultExt;

fn client() -> DataApi {
    DataApi::new().or_fail("data api client")
}

// ── Data API v2 ──────────────────────────────────────────────────
//
// Inputs are chosen live, never hardcoded: a wallet from the bare trade feed,
// that trade's market and token, a resolved market from the winners board.

mod v2 {
    use futures_util::StreamExt;
    use polyoxide_polymarket::data::types::{
        PnlFidelity, PnlInterval, PositionAnchor, PositionStatus, PricesInterval, ResolutionKey,
        TimePeriod, TokenKey, TokenModule,
    };
    use polyoxide_polymarket::data::ErrorCode;
    use polyoxide_polymarket::data::{DataApi, DataError};
    use polyoxide_test_support::{environmental, fail, ResultExt};
    use polyoxide_venue::Classify;

    use super::client;

    /// A recent trade's wallet, condition id and token id.
    async fn recent_trade(data: &DataApi) -> (String, String, String) {
        let page = data.trades().limit(1).send().await.or_fail("v2 trades");
        let trade = page
            .data
            .into_iter()
            .next()
            .expect("the bare feed is never empty"); // live-unwraps: an assertion on the response
        (trade.proxy_wallet, trade.condition_id, trade.token_id)
    }

    /// A wallet absent from every dataset: random, so it cannot collide with
    /// an address that appears on chain (`0x…0001` does, and is "known").
    fn unknown_wallet() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap() // live-unwraps: the clock is after the epoch
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
        let pages: Vec<_> = data.trades().limit(2).pages().take(2).collect().await;

        // A failed page ends the walk, so page 1's error is examined before the
        // count: a transient one is retried rather than filed as a short walk.
        if let Some(Err(e)) = pages.first() {
            fail("page 1", e);
        }
        assert_eq!(pages.len(), 2);
        let first = pages[0].as_ref().unwrap_or_else(|e| fail("page 1", e));
        let second = pages[1].as_ref().unwrap_or_else(|e| fail("page 2", e));
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
        let v2 = &data;

        v2.activity(&wallet)
            .limit(2)
            .send()
            .await
            .or_fail("activity");
        v2.combo_activity(&wallet)
            .limit(2)
            .send()
            .await
            .or_fail("combo activity");
        v2.positions(wallet.as_str())
            .limit(2)
            .send()
            .await
            .or_fail("positions");
        v2.positions(wallet.as_str())
            .status(PositionStatus::Closed)
            .limit(2)
            .send()
            .await
            .or_fail("closed positions");
        v2.combo_positions(&wallet)
            .limit(2)
            .send()
            .await
            .or_fail("combo positions");
        v2.approvals(&wallet).send().await.or_fail("approvals");
        let pnl = v2
            .user_pnl(&wallet)
            .interval(PnlInterval::OneWeek)
            .fidelity(PnlFidelity::OneDay)
            .send()
            .await
            .or_fail("user pnl");
        assert_eq!(pnl.proxy_wallet.to_lowercase(), wallet.to_lowercase());
        let stats = v2.user_stats(&wallet).send().await.or_fail("user stats");
        assert!(stats.is_some(), "a wallet that just traded is a known user");
        v2.user_volume(&wallet).send().await.or_fail("user volume");
        v2.value(&wallet).send().await.or_fail("value");
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_unknown_wallet_is_none_not_an_error() {
        let data = client();
        let wallet = unknown_wallet();

        assert!(data
            .user_stats(&wallet)
            .send()
            .await
            .or_fail("user stats")
            .is_none());
        assert!(data
            .leaderboard_user(&wallet)
            .send()
            .await
            .or_fail("leaderboard user")
            .is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_market_routes() {
        let data = client();
        let (_, condition, token_id) = recent_trade(&data).await;
        let v2 = &data;

        v2.holders([condition.as_str()])
            .limit(2)
            .send()
            .await
            .or_fail("holders");
        v2.positions(PositionAnchor::Condition(condition.clone()))
            .limit(2)
            .send()
            .await
            .or_fail("market positions");
        v2.open_interest()
            .conditions([condition.as_str()])
            .send()
            .await
            .or_fail("open interest");
        let global = v2
            .open_interest()
            .send()
            .await
            .or_fail("global open interest");
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].condition_id, "GLOBAL");
        v2.prices_history(&token_id)
            .interval(PricesInterval::OneDay)
            .limit(10)
            .send()
            .await
            .or_fail("prices history");

        let winners = v2
            .biggest_winners()
            .time_period(TimePeriod::Week)
            .limit(5)
            .send()
            .await
            .or_fail("winners");
        let resolved = winners
            .data
            .iter()
            .find(|w| w.kind == "market")
            .expect("a market win on the weekly board"); // live-unwraps: an assertion on the response
        let rows = v2
            .resolutions(ResolutionKey::Conditions(vec![resolved
                .condition_id
                .clone()]))
            .send()
            .await
            .or_fail("resolutions");
        assert!(
            !rows.is_empty(),
            "a market on the winners board has resolved"
        );
        v2.live_volume([resolved.event_id])
            .send()
            .await
            .or_fail("live volume");
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_tokens() {
        let data = client();
        let v2 = &data;

        let winners = v2
            .biggest_winners()
            .time_period(TimePeriod::Week)
            .limit(5)
            .send()
            .await
            .or_fail("winners");
        let Some(resolved) = winners.data.iter().find(|w| w.kind == "market") else {
            environmental("no market win on the weekly board to look tokens up for");
        };
        let family = v2
            .tokens(TokenKey::Conditions(vec![resolved.condition_id.clone()]))
            .send()
            .await
            .or_fail("tokens by condition");
        assert!(
            !family.is_empty(),
            "a market on the winners board has tokens"
        );
        assert!(
            family
                .iter()
                .any(|t| t.resolved && !matches!(t.module, TokenModule::Other(_))),
            "a resolved market has a resolved token of a module this SDK names"
        );

        let first = &family[0];
        let by_id = v2
            .tokens(TokenKey::TokenIds(vec![first.token_id.clone()]))
            .send()
            .await
            .or_fail("tokens by id");
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].token_id, first.token_id);
        assert_eq!(by_id[0].condition_id, first.condition_id);

        // An id upstream does not know is absent, not an error.
        let unknown = v2
            .tokens(TokenKey::TokenIds(vec!["1".into()]))
            .send()
            .await
            .or_fail("unknown token");
        assert!(unknown.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_board_routes() {
        let data = client();
        let v2 = &data;

        let board = v2
            .leaderboard()
            .time_period(TimePeriod::Week)
            .limit(2)
            .send()
            .await
            .or_fail("leaderboard");
        let leader = &board
            .data
            .first()
            .expect("the weekly board is never empty") // live-unwraps: an assertion on the response
            .user_id;
        let standing = v2
            .leaderboard_user(leader)
            .time_period(TimePeriod::Week)
            .send()
            .await
            .or_fail("leaderboard user")
            .expect("the board's leader has a standing"); // live-unwraps: an assertion on the response
        assert!(standing.rank_pnl.is_some() || standing.rank_volume.is_some());
        v2.builders_leaderboard()
            .limit(2)
            .send()
            .await
            .or_fail("builders leaderboard");
        v2.builder_volume()
            .limit(2)
            .send()
            .await
            .or_fail("builder volume");
        v2.status().send().await.or_fail("status");
    }

    #[tokio::test]
    #[ignore]
    async fn live_v2_errors_are_structured() {
        let data = client();
        let err = data.trades().cursor("garbage").send().await.unwrap_err(); // live-unwraps: the venue must refuse a garbage cursor

        // Any other error fails by its class: a 5xx or a dropped connection is
        // retried, and a 400 whose body was not read as v2 files.
        let DataError::Venue(v2) = &err else {
            fail("expected a structured v2 error", &err);
        };
        assert_eq!(v2.status, 400);
        assert_eq!(v2.code, ErrorCode::InvalidRequest);
        assert!(!v2.trace_id.is_empty());
        assert!(!err.is_retriable());
    }
}
