//! Live integration tests against the Polymarket CLOB API.
//!
//! These tests hit the real API and require network access.
//! They are gated behind `#[ignore]` so they don't run in CI.
//!
//! Run manually with:
//! ```sh
//! cargo test -p polyoxide-clob --test live_api -- --ignored
//! ```

use polyoxide_clob::{
    Account, Clob, ClobBuilder, CreateOrderParams, OrderKind, OrderSide, SignatureType,
};
use polyoxide_core::QueryBuilder;
use polyoxide_gamma::Gamma;
use std::time::Duration;

fn public_client() -> Clob {
    Clob::public()
}

fn authenticated_client() -> Clob {
    dotenvy::dotenv().ok();
    let account =
        Account::from_env().expect("POLYMARKET_* env vars required for authenticated tests");
    // The test account is a Polymarket proxy wallet, so authenticated read
    // endpoints (balances, notifications, rewards) need the POLY_PROXY signature
    // type to resolve the correct on-chain address.
    ClobBuilder::new()
        .with_account(account)
        .signature_type(SignatureType::PolyProxy)
        .build()
        .expect("authenticated clob client")
}

fn authenticated_address() -> String {
    dotenvy::dotenv().ok();
    let account =
        Account::from_env().expect("POLYMARKET_* env vars required for authenticated tests");
    format!("{:#x}", account.address())
}

/// Find a token_id with an active order book using Gamma.
///
/// The CLOB `/markets` listing returns mostly resolved markets. Gamma's
/// `closed=false` filter reliably returns markets with live order books.
async fn find_active_token_id() -> String {
    let gamma = Gamma::builder().build().expect("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .send()
        .await
        .expect("gamma list markets");

    markets
        .iter()
        .find_map(|m| {
            // clob_token_ids is a JSON-encoded array string: '["id1", "id2"]'
            m.clob_token_ids.as_ref().and_then(|ids| {
                serde_json::from_str::<Vec<String>>(ids)
                    .ok()
                    .and_then(|v| v.into_iter().next())
            })
        })
        .expect("should find at least one active market with a token_id via Gamma")
}

// ── Health ───────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_ping() {
    let client = public_client();
    let latency = client.health().ping().await.expect("ping should succeed");
    assert!(
        latency < Duration::from_secs(10),
        "latency too high: {:?}",
        latency
    );
}

// ── Markets ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_markets() {
    let client = public_client();
    let resp = client.markets().list().send().await.expect("list markets");
    assert!(!resp.data.is_empty(), "should return at least one market");
}

#[tokio::test]
#[ignore]
async fn live_simplified_markets() {
    let client = public_client();
    let resp = client
        .markets()
        .simplified()
        .send()
        .await
        .expect("simplified markets");
    assert!(
        !resp.data.is_empty(),
        "should return at least one simplified market"
    );
}

#[tokio::test]
#[ignore]
async fn live_sampling_markets() {
    let client = public_client();
    let _resp = client
        .markets()
        .sampling()
        .send()
        .await
        .expect("sampling markets should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_sampling_simplified_markets() {
    let client = public_client();
    let _resp = client
        .markets()
        .sampling_simplified()
        .send()
        .await
        .expect("sampling simplified markets should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_fee_rate() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .fee_rate(&token_id)
        .send()
        .await
        .expect("fee_rate should deserialize");

    assert!(
        resp.base_fee <= 10_000,
        "fee rate {} bps seems unreasonably high",
        resp.base_fee
    );
}

#[tokio::test]
#[ignore]
async fn live_midpoint() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .midpoint(&token_id)
        .send()
        .await
        .expect("midpoint should succeed");

    let mid: f64 = resp.mid.parse().expect("mid should be a number");
    assert!(
        (0.0..=1.0).contains(&mid),
        "midpoint {mid} should be between 0 and 1"
    );
}

#[tokio::test]
#[ignore]
async fn live_order_book() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let book = client
        .markets()
        .order_book(&token_id)
        .send()
        .await
        .expect("order book should succeed");

    assert!(
        !book.bids.is_empty() || !book.asks.is_empty(),
        "order book should have at least some levels"
    );
}

#[tokio::test]
#[ignore]
async fn live_price() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .price(&token_id, OrderSide::Buy)
        .send()
        .await
        .expect("price should succeed");

    let price: f64 = resp.price.parse().expect("price should be a number");
    assert!(
        (0.0..=1.0).contains(&price),
        "price {price} should be between 0 and 1"
    );
}

#[tokio::test]
#[ignore]
async fn live_prices_history() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .prices_history(&token_id)
        .query("interval", "max")
        .send()
        .await
        .expect("prices_history should succeed");

    assert!(
        !resp.history.is_empty(),
        "prices history should be non-empty"
    );
}

#[tokio::test]
#[ignore]
async fn live_neg_risk() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let _resp = client
        .markets()
        .neg_risk(&token_id)
        .send()
        .await
        .expect("neg_risk should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_tick_size() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .tick_size(&token_id)
        .send()
        .await
        .expect("tick_size should succeed");

    let tick: f64 = resp
        .minimum_tick_size
        .parse()
        .expect("minimum_tick_size should be a number");
    assert!(tick > 0.0, "tick size {tick} should be positive");
}

#[tokio::test]
#[ignore]
async fn live_get_market() {
    let client = public_client();

    // Get a condition_id from the market list
    let list = client.markets().list().send().await.expect("list markets");
    let condition_id = &list
        .data
        .first()
        .expect("should have at least one market")
        .condition_id;

    let market = client
        .markets()
        .get(condition_id)
        .send()
        .await
        .expect("get market should succeed");

    assert_eq!(
        &market.condition_id, condition_id,
        "returned market should match requested condition_id"
    );
}

#[tokio::test]
#[ignore]
async fn live_get_markets_by_token_ids() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .get_by_token_ids(vec![token_id.clone()])
        .send()
        .await
        .expect("get_by_token_ids should succeed");

    assert!(
        !resp.data.is_empty(),
        "should return at least one market for the given token_id"
    );
}

// ── Path-parameter variants (OpenAPI parity) ────────────────────

#[tokio::test]
#[ignore]
async fn live_fee_rate_path() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .fee_rate_path(&token_id)
        .send()
        .await
        .expect("fee_rate_path should deserialize");

    assert!(
        resp.base_fee <= 10_000,
        "fee rate {} bps seems unreasonably high",
        resp.base_fee
    );
}

#[tokio::test]
#[ignore]
async fn live_tick_size_path() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .tick_size_path(&token_id)
        .send()
        .await
        .expect("tick_size_path should deserialize");

    let tick: f64 = resp
        .minimum_tick_size
        .parse()
        .expect("minimum_tick_size should parse");
    assert!(tick > 0.0, "tick size {tick} should be positive");
}

#[tokio::test]
#[ignore]
async fn live_neg_risk_path() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let _resp = client
        .markets()
        .neg_risk_path(&token_id)
        .send()
        .await
        .expect("neg_risk_path should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_clob_market_details() {
    let client = public_client();
    let list = client.markets().list().send().await.expect("list markets");
    let condition_id = &list
        .data
        .first()
        .expect("should have at least one market")
        .condition_id;

    let _details = client
        .markets()
        .clob_market_details(condition_id)
        .send()
        .await
        .expect("clob_market_details should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_market_by_token() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .market_by_token(&token_id)
        .send()
        .await
        .expect("market_by_token should deserialize");

    assert!(
        !resp.condition_id.is_empty(),
        "condition_id should be non-empty"
    );
    assert!(
        !resp.primary_token_id.is_empty() || !resp.secondary_token_id.is_empty(),
        "should have at least one token id"
    );
}

#[tokio::test]
#[ignore]
async fn live_live_activity_market() {
    let client = public_client();
    let list = client.markets().list().send().await.expect("list markets");
    let condition_id = &list
        .data
        .first()
        .expect("should have at least one market")
        .condition_id;

    let _resp = client
        .markets()
        .live_activity_market(condition_id)
        .send()
        .await
        .expect("live_activity_market should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_live_activity_bulk() {
    let client = public_client();
    let list = client.markets().list().send().await.expect("list markets");
    let ids: Vec<String> = list
        .data
        .iter()
        .take(2)
        .map(|m| m.condition_id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }

    let _resp = client
        .markets()
        .live_activity_bulk(ids)
        .expect("body construction")
        .send()
        .await
        .expect("live_activity_bulk should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_batch_prices_history() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let req = polyoxide_clob::BatchPricesHistoryRequest {
        markets: vec![token_id],
        interval: Some("1d".into()),
        ..Default::default()
    };

    let _resp = client
        .markets()
        .batch_prices_history(&req)
        .expect("body construction")
        .send()
        .await
        .expect("batch_prices_history should deserialize");
}

// ── Health: server time ─────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_server_time() {
    let client = public_client();
    let resp = client
        .health()
        .server_time()
        .send()
        .await
        .expect("server_time should succeed");

    assert!(
        resp.time > 0,
        "server time {} should be positive",
        resp.time
    );
}

// ── Markets: spread ────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_spread() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .spread(&token_id)
        .send()
        .await
        .expect("spread should succeed");

    let spread: f64 = resp.spread.parse().expect("spread should be a number");
    assert!(spread >= 0.0, "spread {spread} should be non-negative");
}

// ── Markets: last trade price ──────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_last_trade_price() {
    let token_id = find_active_token_id().await;
    let client = public_client();

    let resp = client
        .markets()
        .last_trade_price(&token_id)
        .send()
        .await
        .expect("last_trade_price should succeed");

    let price_str = resp
        .price
        .or(resp.last_trade_price)
        .expect("response should have price or last_trade_price");
    let price: f64 = price_str.parse().expect("price should be a number");
    assert!(
        (0.0..=1.0).contains(&price),
        "last trade price {price} should be between 0 and 1"
    );
}

// ── Authenticated: Account ──────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_usdc_balance() {
    let client = authenticated_client();
    let resp = client
        .account_api()
        .expect("account_api")
        .usdc_balance()
        .send()
        .await
        .expect("usdc_balance should deserialize");

    let balance: f64 = resp.balance.parse().expect("balance should be a number");
    assert!(balance >= 0.0, "balance {balance} should be non-negative");
}

#[tokio::test]
#[ignore]
async fn live_balance_allowance() {
    let token_id = find_active_token_id().await;
    let client = authenticated_client();
    let _resp = client
        .account_api()
        .expect("account_api")
        .balance_allowance(&token_id)
        .send()
        .await
        .expect("balance_allowance should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_update_balance_allowance() {
    let client = authenticated_client();
    let _resp = client
        .account_api()
        .expect("account_api")
        .update_balance_allowance("COLLATERAL", None, None)
        .await
        .expect("update_balance_allowance should succeed");
}

#[tokio::test]
#[ignore]
async fn live_list_trades() {
    let client = authenticated_client();
    let maker = authenticated_address();
    let _trades = client
        .account_api()
        .expect("account_api")
        .trades(maker)
        .send()
        .await
        .expect("trades should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_list_trades_with_filter() {
    let client = authenticated_client();
    let maker = authenticated_address();
    let _trades = client
        .account_api()
        .expect("account_api")
        .trades(maker)
        .after("0")
        .send()
        .await
        .expect("trades with after filter should deserialize");
}

/// ── Authenticated: Account — builder trades ────────────────────

#[tokio::test]
#[ignore]
async fn live_builder_trades() {
    dotenvy::dotenv().ok();
    // `builder_code` is required by the endpoint and is account-specific, so it
    // lives in the environment rather than being hard-coded. Skip when absent
    // so a non-builder account doesn't see a spurious failure.
    let Ok(builder_code) = std::env::var("POLYMARKET_BUILDER_CODE") else {
        eprintln!("skipping live_builder_trades: POLYMARKET_BUILDER_CODE not set");
        return;
    };

    let client = authenticated_client();
    let _trades = client
        .account_api()
        .expect("account_api")
        .builder_trades(builder_code)
        .send()
        .await
        .expect("builder_trades should deserialize");
}

// ── Authenticated: Account — heartbeat ──────────────────────────

#[tokio::test]
#[ignore]
async fn live_heartbeat() {
    let client = authenticated_client();
    let _resp = client
        .account_api()
        .expect("account_api")
        .heartbeat()
        .await
        .expect("heartbeat should succeed");
}

// ── Authenticated: Notifications ────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_list_notifications() {
    let client = authenticated_client();
    let _notifications = client
        .notifications()
        .expect("notifications")
        .list()
        .send()
        .await
        .expect("list notifications should deserialize");
}

// ── Authenticated: Auth — L1 (EIP-712) ─────────────────────────

/// The only end-to-end check that L1 signing is accepted by the server.
///
/// L1 has exactly one signing path, so this covers `create_api_key`,
/// `derive_api_key`, and `create_readonly_key` alike. Unit tests pin the
/// EIP-712 struct against py-clob-client, but only the server can confirm it
/// accepts the header set; before the ClobAuth fix this returned
/// "Invalid L1 Request headers".
///
/// Derive is deliberate over create: it is read-only and returns the existing
/// deterministic credential rather than provisioning a new one.
#[tokio::test]
#[ignore]
async fn live_l1_derive_api_key() {
    let client = authenticated_client();
    let resp = client
        .auth()
        .expect("auth")
        .derive_api_key(0)
        .send()
        .await
        .expect("L1 derive_api_key should be accepted by the server");

    assert!(!resp.api_key.is_empty(), "apiKey should deserialize");
    assert!(!resp.secret.is_empty(), "secret should deserialize");
    assert!(!resp.passphrase.is_empty(), "passphrase should deserialize");
}

// ── Authenticated: Auth — ban status ────────────────────────────

#[tokio::test]
#[ignore]
async fn live_closed_only_status() {
    let client = authenticated_client();
    let _resp = client
        .auth()
        .expect("auth")
        .closed_only_status()
        .send()
        .await
        .expect("closed_only_status should deserialize");
}

// ── Authenticated: Rewards ─────────────────────────────────────

#[tokio::test]
#[ignore]
async fn live_reward_earnings() {
    let client = authenticated_client();
    let _resp = client
        .rewards()
        .expect("rewards")
        .earnings("2024-01-01")
        .send()
        .await
        .expect("earnings should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_reward_total_earnings() {
    let client = authenticated_client();
    let _resp = client
        .rewards()
        .expect("rewards")
        .total_earnings("2024-01-01")
        .send()
        .await
        .expect("total_earnings should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_reward_percentages() {
    let client = authenticated_client();
    let _resp = client
        .rewards()
        .expect("rewards")
        .percentages()
        .send()
        .await
        .expect("percentages should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_reward_market_earnings() {
    let client = authenticated_client();
    let _resp = client
        .rewards()
        .expect("rewards")
        .market_earnings()
        .send()
        .await
        .expect("market_earnings should deserialize");
}

#[tokio::test]
#[ignore]
async fn live_reward_current_markets() {
    let client = authenticated_client();
    let _resp = client
        .rewards()
        .expect("rewards")
        .current_markets()
        .send()
        .await
        .expect("current_markets should deserialize");
}

// ── Authenticated: Orders ───────────────────────────────────────

/// Checks that `GET /data/orders` deserializes through the typed path.
///
/// **Weak on a quiet account.** With no resting orders the venue returns
/// `{"data":[],...}`, which parses under any naming convention — that is
/// exactly how the camelCase `OpenOrder` bug survived to 0.21.0. Run it while
/// holding at least one open order for it to mean anything, e.g. straight
/// after `live_v2_place_and_cancel` posts one.
///
/// The load-bearing guard is the unit test
/// `open_order_deserializes_captured_response` in `api/orders.rs`, pinned to a
/// captured body. This test exists to catch venue-side drift a frozen fixture
/// cannot.
#[tokio::test]
#[ignore]
async fn live_list_open_orders() {
    let client = authenticated_client();
    let resp = client
        .orders()
        .expect("orders")
        .list()
        .send()
        .await
        .expect("list open orders should deserialize");

    if resp.data.is_empty() {
        eprintln!(
            "note: account holds no resting orders, so this run proves little \
             — see the doc comment"
        );
    }
    for o in &resp.data {
        assert!(!o.id.is_empty());
        assert!(o.created_at > 1_500_000_000, "created_at is Unix seconds");
    }
}

// ── Authenticated: Orders — V2 re-validation (places a real-money order) ──

/// End-to-end re-validation that the CLOB **V2** order shape is accepted by the
/// live exchange: build → sign → post a small GTC buy at the best ask, then
/// cancel it. Places a real (small) order, so it is gated behind `#[ignore]`
/// and a funded proxy account; never runs in CI.
#[tokio::test]
#[ignore] // live; run with `-- --ignored`, needs funded proxy account
async fn live_v2_place_and_cancel() {
    let clob = authenticated_client();
    let token_id = find_active_token_id().await;
    let book = clob.markets().order_book(&token_id).send().await.unwrap();
    let best_ask: f64 = book
        .asks
        .iter()
        .map(|l| l.price)
        .min()
        .expect("asks")
        .to_string()
        .parse()
        .unwrap();

    // Rest, do not fill. This previously bid the best ask, which crosses the
    // spread and executes immediately — leaving a real position bought at the
    // worst available price and making the cancel below a no-op.
    //
    // 0.01 is the minimum tick on most markets and a valid multiple on
    // finer-ticked ones, so a bid there sits at the bottom of the book.
    const RESTING_PRICE: f64 = 0.01;
    assert!(
        RESTING_PRICE < best_ask,
        "market too thin to rest under the ask (best ask {best_ask}); pick another"
    );

    let params = CreateOrderParams {
        token_id,
        price: RESTING_PRICE,
        size: 5.0,
        side: OrderSide::Buy,
        order_type: OrderKind::Gtc,
        // Belt and braces: the venue rejects a post-only order outright rather
        // than letting it take, so a mispriced bid fails loudly instead of
        // silently spending money.
        post_only: true,
        expiration: None,
        funder: None,
        signature_type: Some(SignatureType::PolyProxy),
    };
    let order = clob.create_order(&params, None).await.unwrap();
    let signed = clob.sign_order(&order).await.unwrap();
    let resp = clob
        .post_order(&signed, OrderKind::Gtc, true)
        .await
        .unwrap();
    assert!(resp.success, "V2 order rejected: {:?}", resp.error_msg);
    let id = resp.order_id.expect("accepted order must return an id");

    // Prove it rested. Without this the test passes just as happily when the
    // order fills, which is what hid the original behaviour.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let open = clob
        .orders()
        .expect("orders")
        .list()
        .send()
        .await
        .expect("list open orders");
    let resting = open.data.iter().find(|o| o.id == id);
    assert!(
        resting.is_some(),
        "order {id} is not resting — it filled or was rejected"
    );
    assert_eq!(
        resting.unwrap().size_matched,
        "0",
        "order must rest unfilled, not execute"
    );

    // Always clean up, and verify the cleanup.
    let cancelled = clob
        .orders()
        .expect("orders")
        .cancel(id.clone())
        .send()
        .await
        .expect("cancel");
    assert!(
        cancelled.canceled.contains(&id),
        "cancel did not report {id}: {cancelled:?}"
    );
}

// ── Authenticated: Orders — FAK kill outcome ──

/// Live re-validation that an unmatched FAK surfaces as [`ClobError::FakUnmatched`]
/// rather than a generic validation error.
///
/// Posts a deliberately non-crossing marketable BUY — 1c against a book whose best
/// ask is far above it — so the matching engine finds no counterparty and kills the
/// order. **No capital moves**: an unmatched FAK fills nothing, which makes this the
/// cheapest of the order-placing live tests. It still needs a funded proxy account,
/// because the venue checks balance/allowance before it reaches the matching engine.
///
/// Size is 200 shares (= $2.00 notional at 1c) rather than the bare 100. Polymarket
/// enforces a $1 minimum notional on *marketable* orders — resting limit orders are
/// exempt — and under it you get `invalid amount for a marketable BUY order ($X),
/// min size: 1` without ever reaching the matching engine, i.e. the wrong path.
///
/// Not covered here: a *partially* filled FAK. That requires an order that genuinely
/// crosses a book with less depth than the order size, which does spend money. The
/// expectation is a 200 with `status: matched` and partial amounts, since the error
/// is specifically the zero-match branch — but that remains unverified.
#[tokio::test]
#[ignore] // live; run with `-- --ignored`, needs funded proxy account
async fn live_fak_unmatched_is_typed_error() {
    use polyoxide_clob::ClobError;

    let clob = authenticated_client();
    let token_id = find_active_token_id().await;

    // Precondition: the book must sit well clear of 1c, or the order would cross
    // and we would be testing the fill path (and spending money).
    let book = clob.markets().order_book(&token_id).send().await.unwrap();
    let best_ask = book.asks.iter().map(|l| l.price).min().expect("asks");
    assert!(
        best_ask > rust_decimal::Decimal::new(5, 2),
        "book too cheap for a safe non-crossing test: best ask {best_ask}"
    );

    let params = CreateOrderParams {
        token_id,
        price: 0.01,
        size: 200.0,
        side: OrderSide::Buy,
        order_type: OrderKind::Fak,
        post_only: false,
        expiration: None,
        funder: None,
        signature_type: Some(SignatureType::PolyProxy),
    };

    let err = clob
        .place_order(&params, None)
        .await
        .expect_err("a non-crossing FAK must be killed, not filled");

    match &err {
        ClobError::FakUnmatched { message } => {
            assert!(
                message.to_ascii_lowercase().contains("fak order"),
                "unexpected venue prose: {message}"
            );
        }
        // Surface the actual error: a min-notional rejection means the order never
        // reached the matching engine and this run proved nothing.
        other => panic!("expected ClobError::FakUnmatched, got: {other:?}"),
    }

    assert!(
        !err.is_retriable(),
        "an unmatched FAK must not be retriable"
    );
}
