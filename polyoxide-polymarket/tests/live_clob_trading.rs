//! Live tests for the CLOB through the venue-neutral `Trading` trait. Ignored
//! by default; run with `cargo test -p polyoxide-polymarket
//! --no-default-features --features clob-ws,data,gamma --test live_clob_trading
//! -- --ignored --nocapture`.
//!
//! Every test reaches the CLOB as `Arc<DynTrading<'static>>`, the way a
//! consumer that picks its venue at run time does. The reads need an account.
//! The round trip places a post-only bid of 5 shares at 0.01 under the best
//! ask, so it rests and moves no capital, and cancels it on every path the
//! test controls. Without credentials each test fails as `auth-gated`.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{FutureExt, StreamExt};
use polyoxide_polymarket::clob::{
    Account, Clob, ClobBuilder, Credentials, OrderBook, SignatureType,
};
use polyoxide_polymarket::gamma::Gamma;
use polyoxide_polymarket::shared::keys::{self, TokenId};
#[cfg(feature = "keychain")]
use polyoxide_test_support::keychain;
use polyoxide_test_support::{
    environmental, fail, fail_classified, load_env, transient, ClassifiedResultExt, ResultExt,
};
use polyoxide_venue::{
    Class, ClientOrderId, DynTrading, MarketKey, Order, OrderId, OrderRequest, OrderStatus, Side,
    TimeInForce, Trading, TradingEvent, TradingEvents, UnixMillis,
};
use rust_decimal::Decimal;

/// How long the events stream may take to yield its first item, which comes
/// once its socket's first connect attempt has ended.
const RESYNC_WITHIN: Duration = Duration::from_secs(30);

/// How long the user channel may take to ack a placement or a cancellation.
const ACK_WITHIN: Duration = Duration::from_secs(30);

/// How many gamma-listed candidates are confirmed against the CLOB book
/// before giving up. Each confirmation is one `/book` request.
const MAX_BOOK_PROBES: usize = 10;

/// How far an order's `created`, from the user channel or from the open
/// orders, may lie from the test's clock: the user channel's `timestamp` is read
/// in either unit by magnitude (D23), and this bound catches a misread unit by
/// orders of magnitude.
const CREATED_WITHIN: Duration = Duration::from_secs(5 * 60);

/// The test's clock.
fn now() -> UnixMillis {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH);
    UnixMillis(elapsed.map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    }))
}

/// Whether `created` is a time within [`CREATED_WITHIN`] of `reference`.
fn within_of(created: Option<UnixMillis>, reference: UnixMillis) -> bool {
    created
        .is_some_and(|time| u128::from(time.0.abs_diff(reference.0)) <= CREATED_WITHIN.as_millis())
}

/// The price the test's bid rests at: a valid multiple on every tick the CLOB
/// lists, and at the bottom of any book whose best ask is above it.
fn resting_price() -> Decimal {
    Decimal::new(1, 2)
}

/// The bid's size in shares, the CLOB's usual minimum order size.
fn bid_size() -> Decimal {
    Decimal::new(5, 0)
}

/// Load the account these tests trade as, from `POLYMARKET_*` and then the OS
/// keychain under the `polyoxide-clob` service when the `keychain` feature is
/// on. When neither source has all four, the loader fails the test as
/// `auth-gated`, which the nightly skips; an empty value counts as absent.
fn load_account() -> Account {
    let creds = load_env(&[
        "POLYMARKET_PRIVATE_KEY",
        "POLYMARKET_API_KEY",
        "POLYMARKET_API_SECRET",
        "POLYMARKET_API_PASSPHRASE",
    ]);
    #[cfg(feature = "keychain")]
    let creds = creds.or_else(|_| {
        keychain(
            "polyoxide-clob",
            &[
                ("POLYMARKET_PRIVATE_KEY", "private_key"),
                ("POLYMARKET_API_KEY", "api_key"),
                ("POLYMARKET_API_SECRET", "api_secret"),
                ("POLYMARKET_API_PASSPHRASE", "api_passphrase"),
            ],
        )
    });
    let creds = creds.unwrap_or_else(|missing| missing.or_auth_gated());
    Account::new(
        creds.get("POLYMARKET_PRIVATE_KEY"),
        Credentials {
            key: creds.get("POLYMARKET_API_KEY").to_owned(),
            secret: creds.get("POLYMARKET_API_SECRET").to_owned(),
            passphrase: creds.get("POLYMARKET_API_PASSPHRASE").to_owned(),
        },
    )
    .or_fail("build the account from POLYMARKET_*")
}

/// The account's CLOB client held as the venue-neutral trait object. The test
/// account is a Polymarket proxy wallet, so the client needs the POLY_PROXY
/// signature type to resolve the address its orders and positions belong to.
fn trading() -> Arc<DynTrading<'static>> {
    let clob = ClobBuilder::new()
        .with_account(load_account())
        .signature_type(SignatureType::PolyProxy)
        .build()
        .or_fail("authenticated clob client");
    DynTrading::new_arc(clob)
}

/// Whether the live book lets the test's bid rest unfilled and be accepted:
/// its best ask is above [`resting_price`], its tick divides it, and its
/// minimum order size is no more than [`bid_size`]. A book that omits its tick
/// or minimum size is not held against the token.
fn admits_the_bid(book: &OrderBook) -> bool {
    let best_ask = book.asks.iter().map(|level| level.price).min();
    let tick_fits = match book.tick_size.as_deref() {
        None => true,
        Some(tick) => tick
            .parse::<Decimal>()
            .is_ok_and(|tick| tick <= resting_price()),
    };
    let size_fits = match book.min_order_size.as_deref() {
        None => true,
        Some(least) => least
            .parse::<Decimal>()
            .is_ok_and(|least| least <= bid_size()),
    };
    best_ask.is_some_and(|ask| ask > resting_price()) && tick_fits && size_fits
}

/// Find an open market whose CLOB book admits the test's bid.
///
/// Selecting on the precondition, on the same `/book` route the check reads,
/// is what keeps a cheap first listing from failing the test. Narrowing is
/// free: gamma returns `best_ask` in the listing, and the chosen candidate is
/// then confirmed against the CLOB book, because gamma's figure is a cached
/// view.
///
/// Fails as `environmental` when nothing qualifies. A book request that errors
/// is not "nothing qualifies": when every probe errored, the last error fails
/// the test with its own tag, so an outage is not reported as market
/// conditions.
async fn find_token_the_bid_can_rest_on() -> String {
    let gamma = Gamma::builder().build().or_fail("gamma client");
    let markets = gamma
        .markets()
        .list()
        .closed(false)
        .limit(100)
        .send()
        .await
        .or_fail("gamma list markets");

    let clob = Clob::public();
    let mut probed = 0usize;
    let mut errored = 0usize;
    let mut last_error = None;

    for market in markets.iter() {
        let gamma_ask = market.best_ask.and_then(|ask| Decimal::try_from(ask).ok());
        if gamma_ask.is_none_or(|ask| ask <= resting_price()) {
            continue;
        }
        // clob_token_ids is a JSON-encoded array string: '["id1", "id2"]'
        let token_id = market.clob_token_ids.as_ref().and_then(|ids| {
            serde_json::from_str::<Vec<String>>(ids)
                .ok()
                .and_then(|v| v.into_iter().next())
        });
        let Some(token_id) = token_id else {
            continue;
        };

        probed += 1;
        match clob.markets().order_book(&token_id).send().await {
            Ok(book) => {
                if admits_the_bid(&book) {
                    return token_id;
                }
            }
            Err(err) => {
                errored += 1;
                last_error = Some(err);
            }
        }
        if probed >= MAX_BOOK_PROBES {
            break;
        }
    }

    if let Some(err) = last_error.filter(|_| errored == probed) {
        fail(&format!("every one of {probed} book probe(s) failed"), &err);
    }
    environmental(&format!(
        "no open market whose book admits a post-only bid of {} shares at {} among the \
         markets gamma lists ({probed} book(s) probed); market conditions rather than a \
         defect, so re-run before concluding otherwise",
        bid_size(),
        resting_price()
    ));
}

/// The first item of an events stream, which must be `Resync`.
async fn first_event(events: &mut TradingEvents) -> TradingEvent {
    match tokio::time::timeout(RESYNC_WITHIN, events.next()).await {
        Err(_) => transient("the events stream yielded nothing in its first 30 s"),
        Ok(None) => transient("the events stream ended before its first event"),
        Ok(Some(item)) => item.or_fail("the events stream's first item"),
    }
}

/// The next ack of order `id` whose status satisfies `wanted`, skipping every
/// other event. Fails as `transient` when [`ACK_WITHIN`] passes first, when the
/// stream ends, or when it resyncs (a reconnect between the placement and the
/// ack can lose the placement's ack); the caller's guard still cancels the bid.
async fn next_ack(
    events: &mut TradingEvents,
    id: &OrderId,
    wanted: fn(&OrderStatus) -> bool,
) -> Order {
    let deadline = tokio::time::Instant::now() + ACK_WITHIN;
    loop {
        match tokio::time::timeout_at(deadline, events.next()).await {
            Err(_) => transient("no ack of the order within 30 s"),
            Ok(None) => transient("the events stream ended before the ack"),
            Ok(Some(item)) => match item.or_fail("the events stream yields an item") {
                TradingEvent::Ack(order) if order.id == *id && wanted(&order.status) => {
                    return order;
                }
                TradingEvent::Resync => transient("the events stream resynced before the ack"),
                _ => {}
            },
        }
    }
}

/// Best-effort removal of the test's bid from the venue, for a path on which
/// the test fails with the bid possibly still resting: a `place` that failed
/// with a class under which the order may have been accepted (D18), and a
/// `cancel` that errored.
///
/// It reads the token's open orders and cancels each one that is a buy at the
/// test's price and size, or, when `known` names the order, only that id. When
/// it cannot read the open orders and the id is known, it cancels the id
/// directly. Its own errors are printed and otherwise ignored, so the caller
/// goes on to fail with the original error.
async fn cancel_what_may_rest(
    trading: &Arc<DynTrading<'static>>,
    key: &MarketKey,
    known: Option<&OrderId>,
) {
    let ids: Vec<OrderId> = match trading.open_orders(Some(key)).await {
        Ok(open) => open
            .into_iter()
            .filter(|order| match known {
                Some(id) => order.id == *id,
                None => {
                    matches!(order.side, Side::Buy)
                        && order.price == resting_price()
                        && order.size == bid_size()
                }
            })
            .map(|order| order.id)
            .collect(),
        Err(read_error) => {
            eprintln!("cleanup could not read the open orders: {read_error:?}");
            known.cloned().into_iter().collect()
        }
    };
    for id in ids {
        if let Err(cancel_error) = trading.cancel(&id).await {
            eprintln!("cleanup could not cancel order {id}: {cancel_error:?}");
        }
    }
}

/// The account's open orders, positions and collateral balance read through
/// the trait object. The collateral is pUSD, and a balance is never negative.
#[tokio::test]
#[ignore]
async fn live_trading_reads_open_orders_positions_and_the_collateral_balance() {
    let trading = trading();

    let open = trading
        .open_orders(None)
        .await
        .or_fail("read the open orders");
    for order in &open {
        let _token =
            keys::token_id(&order.key).or_fail("an open order is keyed by a CLOB outcome token");
    }

    let positions = trading.positions().await.or_fail("read the positions");
    for position in &positions {
        let _token =
            keys::token_id(&position.key).or_fail("a position is keyed by a CLOB outcome token");
    }

    let balances = trading.balances().await.or_fail("read the balances");
    assert_eq!(
        balances.len(),
        1,
        "the account holds one pUSD balance record: {balances:?}"
    );
    let collateral = balances.iter().find(|balance| balance.asset == "pUSD");
    assert!(collateral.is_some(), "no pUSD balance among {balances:?}");
    assert!(
        collateral.is_some_and(|balance| balance.total.is_some_and(|total| total >= Decimal::ZERO)),
        "the pUSD balance has no non-negative total: {collateral:?}"
    );
}

/// A post-only bid rests, the events stream acks it after its leading
/// `Resync`, `cancel` succeeds, and the stream acks the cancellation. The bid
/// is cancelled on every path out of the test, including a failed assertion.
#[tokio::test]
#[ignore]
async fn live_trading_places_acks_and_cancels_a_resting_post_only_bid() {
    let trading = trading();
    let token_id = find_token_the_bid_can_rest_on().await;
    let key = keys::clob(&TokenId::new(token_id).or_fail("name the token"));

    // Open the stream before placing: it begins with `Resync`, and from then
    // on its socket delivers the account's order events.
    let mut events = trading.events();
    let first = first_event(&mut events).await;
    assert!(
        matches!(first, TradingEvent::Resync),
        "an events stream begins with Resync, not {first:?}"
    );

    let mut request = OrderRequest::new(key.clone(), Side::Buy, resting_price(), bid_size());
    // The venue refuses a post-only order that would take, so a mispriced bid
    // fails loudly instead of spending money.
    request.post_only = true;
    request.time_in_force = TimeInForce::GoodTilCancelled;
    // Unique per run, so a bid left by an earlier run cannot be mistaken for
    // this one's. The ledger gives the acks this id, so the Open ack checks the
    // order id the client computed against the venue's.
    let client_id = ClientOrderId::new(format!("polyoxide-live-{}", now().0));
    request.client_order_id = Some(client_id.clone());
    let order = match trading.place(request).await {
        Ok(order) => order,
        Err(placement_error) => {
            // A network, availability or decode failure may leave the order
            // on the venue (D18), so look for it before failing.
            if matches!(
                placement_error.class,
                Class::Network | Class::Unavailable { .. } | Class::Decode
            ) {
                cancel_what_may_rest(&trading, &key, None).await;
            }
            fail_classified("place the resting bid", &placement_error)
        }
    };

    // From here the bid rests on the venue. Whatever fails below, it is
    // cancelled, and the original failure is raised afterwards.
    let rested = AssertUnwindSafe(async {
        assert!(
            matches!(order.status, OrderStatus::Open),
            "the bid must rest: {order:?}"
        );
        assert_eq!(
            order.filled,
            Decimal::ZERO,
            "the bid must rest unfilled: {order:?}"
        );
        let resting = trading
            .open_orders(Some(&key))
            .await
            .or_fail("read the open orders of the token");
        let listed = resting.iter().find(|open| open.id == order.id);
        assert!(
            listed.is_some(),
            "order {} is not among the open orders {resting:?}",
            order.id
        );
        let opened = next_ack(&mut events, &order.id, |status| {
            matches!(status, OrderStatus::Open)
        })
        .await;
        assert_eq!(
            opened.client_order_id,
            Some(client_id.clone()),
            "the Open ack lost the client order id: {opened:?}"
        );
        // The ack's `created` is the user channel's `timestamp` (D23).
        assert!(
            within_of(opened.created, now()),
            "the Open ack's created is not within 5 minutes of now: {opened:?}"
        );
        assert!(
            listed
                .is_some_and(|row| within_of(opened.created, row.created.unwrap_or(UnixMillis(0)))),
            "the Open ack's created is not within 5 minutes of the open order's: \
             {opened:?} against {listed:?}"
        );
    })
    .catch_unwind()
    .await;

    let cancelled = trading.cancel(&order.id).await;
    if cancelled.is_err() {
        // The order may still rest: look it up and cancel it again before
        // failing with whatever failed first.
        cancel_what_may_rest(&trading, &key, Some(&order.id)).await;
    }
    if let Err(failure) = rested {
        if let Err(cancel_error) = &cancelled {
            eprintln!(
                "the failed test also failed to cancel order {}: {cancel_error:?}",
                order.id
            );
        }
        std::panic::resume_unwind(failure);
    }
    cancelled.or_fail("cancel the resting bid");

    let closed = next_ack(&mut events, &order.id, |status| {
        matches!(status, OrderStatus::Cancelled)
    })
    .await;
    // The ledger keeps the client id after the cancellation (D21).
    assert_eq!(
        closed.client_order_id,
        Some(client_id),
        "the Cancelled ack lost the client order id: {closed:?}"
    );
}
