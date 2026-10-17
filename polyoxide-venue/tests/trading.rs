#![cfg(feature = "trading")]
//! `Trading` on a synthetic paper venue, `acme.binary`: used statically and
//! as `Arc<DynTrading<'static>>`, with kills as `Ok`, undeclared options
//! refused before anything is sent, and an event stream that begins with
//! `Resync`, yields `Ack`, `Fill` and `Resync` in order and outlives the venue
//! handle it came from.
//!
//! The paper venue answers at once, so `now_or_never` drives every call
//! without a runtime.

use std::any::TypeId;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::time::Duration;

use futures_core::Stream;
use futures_util::{FutureExt, StreamExt};
use polyoxide_venue::{
    Balance, Class, ClassifiedError, ClientOrderId, DynTrading, Fill, FillId, KeyError, KeySpace,
    KillReason, Liquidity, MarketKey, Order, OrderId, OrderRequest, OrderStatus, Position,
    ProductId, Side, TimeInForce, TradeId, Trading, TradingEvent, TradingEvents,
    UndeclaredExtension, UnixMillis, VenueId,
};
use rust_decimal::Decimal;

/// Event contracts: one key per outcome, `<TICKER>:yes|no`.
struct Binary;

impl KeySpace for Binary {
    const VENUE: VenueId = VenueId::from_static("acme");
    const PRODUCT: ProductId = ProductId::from_static("binary");
    type Id = String;

    fn parse_id(text: &str) -> Result<String, String> {
        match text.rsplit_once(':') {
            Some((ticker, "yes" | "no")) if !ticker.is_empty() => Ok(text.to_owned()),
            _ => Err(format!("`{text}` has no `:yes` or `:no` outcome")),
        }
    }

    fn id_text(id: &String) -> Cow<'_, str> {
        Cow::Borrowed(id)
    }
}

/// The one option type the paper venue declares.
#[derive(Debug, Clone, PartialEq)]
struct PaperOrderOptions {
    expires_at: Option<UnixMillis>,
}

/// An option it does not.
#[derive(Debug, Clone, PartialEq)]
struct Iceberg {
    visible: Decimal,
}

/// The paper venue's own fill facts.
#[derive(Debug, Clone, PartialEq)]
struct PaperFillExt {
    post_position: Decimal,
}

fn dec(text: &str) -> Decimal {
    text.parse().unwrap()
}

fn yes() -> MarketKey {
    Binary::key(&"RAIN:yes".to_owned())
}

/// The paper venue's state: liquidity to take at any price, what it holds,
/// and the unbounded queue `events()` drains. One queue serves one consumer.
#[derive(Default)]
struct State {
    liquidity: Decimal,
    sent: usize,
    next: u64,
    orders: Vec<Order>,
    position: Decimal,
    queue: VecDeque<Result<TradingEvent, ClassifiedError>>,
    waker: Option<Waker>,
}

#[derive(Clone)]
struct Paper {
    state: Arc<Mutex<State>>,
}

impl Paper {
    fn with_liquidity(liquidity: &str) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                liquidity: dec(liquidity),
                ..State::default()
            })),
        }
    }

    /// Requests that reached the venue.
    fn sent(&self) -> usize {
        self.state.lock().unwrap().sent
    }

    fn push(state: &mut State, event: TradingEvent) {
        state.queue.push_back(Ok(event));
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    /// What a `Trading` impl does when its fills connection reconnects.
    fn reconnected(&self) {
        Self::push(&mut self.state.lock().unwrap(), TradingEvent::Resync);
    }

    fn check_key(key: &MarketKey) -> Result<(), ClassifiedError> {
        Binary::id(key)?;
        Ok(())
    }
}

impl Trading for Paper {
    async fn place(&self, request: OrderRequest) -> Result<Order, ClassifiedError> {
        Self::check_key(&request.key)?;
        request
            .extensions
            .ensure_declared(&[TypeId::of::<PaperOrderOptions>()])?;
        // The paper venue rests nothing it cannot match and holds no position
        // to reduce, so it honours neither flag: refused, never ignored.
        if request.post_only || request.reduce_only {
            return Err(ClassifiedError::new(
                Class::InvalidRequest,
                "the paper venue takes neither post-only nor reduce-only orders",
            ));
        }

        let mut state = self.state.lock().unwrap();
        state.sent += 1;
        state.next += 1;
        let id = OrderId::from(format!("o-{}", state.next));
        let matchable = request.size.min(state.liquidity);
        let (filled, status) = match request.time_in_force {
            TimeInForce::FillOrKill if matchable < request.size => (
                Decimal::ZERO,
                OrderStatus::Killed {
                    reason: KillReason::Unfillable,
                },
            ),
            TimeInForce::ImmediateOrCancel if matchable < request.size => (
                matchable,
                OrderStatus::Killed {
                    reason: KillReason::Unmatched,
                },
            ),
            _ if matchable == request.size => (matchable, OrderStatus::Filled),
            _ => (matchable, OrderStatus::Open),
        };

        let mut order = Order::new(
            id.clone(),
            request.key.clone(),
            request.side,
            request.price,
            request.size,
            status,
        );
        order.filled = filled;
        order.time_in_force = Some(request.time_in_force);
        order.client_order_id = request.client_order_id.clone();
        order.created = Some(UnixMillis(1_760_000_000_000));
        state.liquidity -= filled;
        if status == OrderStatus::Open {
            state.orders.push(order.clone());
        }
        Self::push(&mut state, TradingEvent::Ack(order.clone()));

        if filled > Decimal::ZERO {
            state.position += match request.side {
                Side::Buy => filled,
                Side::Sell => -filled,
            };
            let mut fill = Fill::new(
                FillId::from(format!("f-{}", state.next)),
                id,
                request.key,
                request.side,
                request.price,
                filled,
                UnixMillis(1_760_000_000_001),
            );
            fill.trade_id = Some(TradeId::new(format!("t-{}", state.next)));
            fill.fee = Some(dec("0.01"));
            fill.liquidity = Some(Liquidity::Taker);
            fill.notional = Some(request.price * filled);
            fill.client_order_id = request.client_order_id;
            let post_position = state.position;
            fill.extensions.insert(PaperFillExt { post_position })?;
            Self::push(&mut state, TradingEvent::Fill(fill));
        }
        Ok(order)
    }

    async fn cancel(&self, id: &OrderId) -> Result<(), ClassifiedError> {
        let mut state = self.state.lock().unwrap();
        state.sent += 1;
        let before = state.orders.len();
        state.orders.retain(|order| order.id != *id);
        if state.orders.len() == before {
            return Err(ClassifiedError::new(
                Class::VenueRefusal { code: None },
                format!("no open order {id}"),
            ));
        }
        Ok(())
    }

    async fn cancel_all(&self, key: Option<&MarketKey>) -> Result<Vec<OrderId>, ClassifiedError> {
        let mut state = self.state.lock().unwrap();
        state.sent += 1;
        let (cancelled, kept) = std::mem::take(&mut state.orders)
            .into_iter()
            .partition::<Vec<_>, _>(|order| key.is_none_or(|key| order.key == *key));
        state.orders = kept;
        Ok(cancelled.into_iter().map(|order| order.id).collect())
    }

    async fn open_orders(&self, key: Option<&MarketKey>) -> Result<Vec<Order>, ClassifiedError> {
        let state = self.state.lock().unwrap();
        Ok(state
            .orders
            .iter()
            .filter(|order| key.is_none_or(|key| order.key == *key))
            .cloned()
            .collect())
    }

    async fn positions(&self) -> Result<Vec<Position>, ClassifiedError> {
        let state = self.state.lock().unwrap();
        Ok(vec![Position::new(yes(), state.position)])
    }

    async fn balances(&self) -> Result<Vec<Balance>, ClassifiedError> {
        let mut balance = Balance::new("USD");
        balance.available = Some(dec("100"));
        Ok(vec![balance])
    }

    fn events(&self) -> TradingEvents {
        // The stream's first item is `Resync`: a fill between this call and
        // the first subscription is covered by a reconciliation.
        Self::push(&mut self.state.lock().unwrap(), TradingEvent::Resync);
        let state = Arc::clone(&self.state);
        TradingEvents::new(futures_util::stream::poll_fn(move |cx| {
            let mut state = state.lock().unwrap();
            match state.queue.pop_front() {
                Some(event) => Poll::Ready(Some(event)),
                None => {
                    state.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }))
    }
}

/// Drives a future the paper venue completes at once, checking it is `Send`.
fn run<F: Future + Send>(future: F) -> F::Output {
    future
        .now_or_never()
        .expect("the paper venue answers at once")
}

/// The bounds a consumer needs to move the stream onto a task of its own.
fn movable<S>(stream: S) -> S
where
    S: Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + Unpin + 'static,
{
    stream
}

fn next(stream: &mut TradingEvents) -> Option<TradingEvent> {
    stream
        .next()
        .now_or_never()
        .flatten()
        .map(|event| event.unwrap())
}

/// Venue-neutral strategy code: buys and reports where the order stands.
async fn buy<T: Trading + ?Sized>(
    venue: &T,
    size: &str,
    time_in_force: TimeInForce,
) -> Result<Order, ClassifiedError> {
    let mut request = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec(size));
    request.time_in_force = time_in_force;
    venue.place(request).await
}

#[test]
fn a_fill_or_kill_that_cannot_fill_is_ok_and_killed_unfillable() {
    let venue = Paper::with_liquidity("3");
    let order = run(buy(&venue, "5", TimeInForce::FillOrKill)).unwrap();
    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    );
    assert_eq!(order.filled, Decimal::ZERO);

    let dyn_venue: Arc<DynTrading<'static>> = DynTrading::new_arc(Paper::with_liquidity("3"));
    let order = run(buy(&*dyn_venue, "5", TimeInForce::FillOrKill)).unwrap();
    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unfillable
        }
    );
}

#[test]
fn an_immediate_or_cancel_remainder_is_killed_unmatched_and_says_what_filled() {
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(Paper::with_liquidity("3"));
    let order = run(buy(&*venue, "5", TimeInForce::ImmediateOrCancel)).unwrap();
    assert_eq!(
        order.status,
        OrderStatus::Killed {
            reason: KillReason::Unmatched
        }
    );
    assert_eq!(order.filled, dec("3"));
    assert_eq!(order.time_in_force, Some(TimeInForce::ImmediateOrCancel));
}

#[test]
fn an_undeclared_option_is_refused_as_invalid_request_and_nothing_is_sent() {
    let venue = Paper::with_liquidity("3");
    let mut request = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec("1"));
    request
        .extensions
        .insert(Iceberg {
            visible: dec("0.5"),
        })
        .unwrap();
    assert_eq!(
        request.extensions.get::<Iceberg>().unwrap().visible,
        dec("0.5")
    );

    let dyn_venue: Arc<DynTrading<'static>> = DynTrading::new_arc(venue.clone());
    let err = run(dyn_venue.place(request.clone())).unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    let undeclared = err.source.downcast_ref::<UndeclaredExtension>().unwrap();
    assert!(undeclared.type_name.ends_with("Iceberg"), "{undeclared}");

    assert_eq!(
        run(venue.place(request)).unwrap_err().class,
        Class::InvalidRequest
    );
    assert_eq!(venue.sent(), 0, "nothing reached the venue");

    let mut events = venue.events();
    assert_eq!(next(&mut events), Some(TradingEvent::Resync));
    assert!(next(&mut events).is_none(), "and nothing was acknowledged");
}

#[test]
fn a_declared_option_is_accepted() {
    let venue = Paper::with_liquidity("0");
    let mut request = OrderRequest::new(yes(), Side::Sell, dec("0.60"), dec("2"));
    request.client_order_id = Some(ClientOrderId::from("mine-1"));
    request
        .extensions
        .insert(PaperOrderOptions {
            expires_at: Some(UnixMillis(1_760_003_600_000)),
        })
        .unwrap();
    assert_eq!(
        request
            .extensions
            .get::<PaperOrderOptions>()
            .unwrap()
            .expires_at,
        Some(UnixMillis(1_760_003_600_000))
    );

    let order = run(venue.place(request)).unwrap();
    assert_eq!(order.status, OrderStatus::Open);
    assert_eq!(order.client_order_id, Some(ClientOrderId::from("mine-1")));
    assert_eq!(venue.sent(), 1);
}

#[test]
fn a_flag_the_venue_cannot_honour_is_refused_before_sending() {
    let venue = Paper::with_liquidity("3");
    let mut post_only = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec("1"));
    post_only.post_only = true;
    let mut reduce_only = OrderRequest::new(yes(), Side::Sell, dec("0.56"), dec("1"));
    reduce_only.reduce_only = true;

    for request in [post_only, reduce_only] {
        let err = run(venue.place(request)).unwrap_err();
        assert_eq!(err.class, Class::InvalidRequest);
    }
    assert_eq!(venue.sent(), 0, "nothing reached the venue");
}

#[test]
fn a_key_of_another_product_is_refused_before_sending() {
    struct Swap;
    impl KeySpace for Swap {
        const VENUE: VenueId = VenueId::from_static("acme");
        const PRODUCT: ProductId = ProductId::from_static("swap");
        type Id = String;

        fn parse_id(text: &str) -> Result<String, String> {
            Ok(text.to_owned())
        }

        fn id_text(id: &String) -> Cow<'_, str> {
            Cow::Borrowed(id)
        }
    }

    let venue = Paper::with_liquidity("3");
    let request = OrderRequest::new(
        Swap::key(&"BTCUSD".to_owned()),
        Side::Buy,
        dec("1"),
        dec("1"),
    );
    let err = run(venue.place(request)).unwrap_err();
    assert_eq!(err.class, Class::InvalidRequest);
    assert!(err.source.downcast_ref::<KeyError>().is_some());
    assert_eq!(venue.sent(), 0);
}

#[test]
fn events_begin_with_resync_then_yield_ack_fill_and_resync_in_order_through_dyn() {
    let paper = Paper::with_liquidity("3");
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());
    let mut events = movable(venue.events());
    // The stream needs nothing of the handle it came from.
    drop(venue);

    assert_eq!(
        next(&mut events),
        Some(TradingEvent::Resync),
        "the first item is Resync"
    );
    assert!(next(&mut events).is_none(), "nothing has happened yet");
    let order = run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    assert_eq!(order.status, OrderStatus::Filled);
    paper.reconnected();
    let later = run(buy(&paper, "1", TimeInForce::GoodTilCancelled)).unwrap();

    match next(&mut events) {
        Some(TradingEvent::Ack(acked)) => assert_eq!(acked, order),
        other => panic!("expected the order's Ack, got {other:?}"),
    }
    match next(&mut events) {
        Some(TradingEvent::Fill(fill)) => {
            assert_eq!(fill.order_id, order.id);
            assert_eq!(fill.size, dec("2"));
            assert_eq!(fill.notional, Some(dec("1.12")));
            assert_eq!(fill.liquidity, Some(Liquidity::Taker));
            assert_eq!(
                fill.extensions.get::<PaperFillExt>(),
                Some(&PaperFillExt {
                    post_position: dec("2")
                })
            );
        }
        other => panic!("expected the order's Fill, got {other:?}"),
    }
    assert_eq!(next(&mut events), Some(TradingEvent::Resync));
    match next(&mut events) {
        Some(TradingEvent::Ack(acked)) => assert_eq!(acked.id, later.id),
        other => panic!("expected the later Ack after Resync, got {other:?}"),
    }
    assert!(matches!(next(&mut events), Some(TradingEvent::Fill(_))));
    assert!(next(&mut events).is_none());
}

#[test]
fn the_stream_outlives_a_dropped_dyn_clone_and_moves_to_another_thread() {
    let paper = Paper::with_liquidity("3");
    let held: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());

    // Taken from a temporary clone that is dropped at once, then the original
    // dropped too: nothing the stream could borrow from is left.
    let events = Arc::clone(&held).events();
    drop(held);

    let order = run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    paper.reconnected();

    // `std::thread::spawn` takes only a `Send + 'static` closure, so this
    // compiles only because the stream is both.
    let seen = std::thread::spawn(move || {
        let mut events = events;
        std::iter::from_fn(|| next(&mut events)).collect::<Vec<_>>()
    })
    .join()
    .unwrap();

    assert_eq!(seen.len(), 4, "{seen:?}");
    assert_eq!(seen[0], TradingEvent::Resync);
    assert!(matches!(&seen[1], TradingEvent::Ack(acked) if acked.id == order.id));
    assert!(matches!(&seen[2], TradingEvent::Fill(fill) if fill.order_id == order.id));
    assert_eq!(seen[3], TradingEvent::Resync);
}

#[test]
fn events_from_the_static_venue_have_the_same_bounds() {
    let paper = Paper::with_liquidity("3");
    let mut events = movable(paper.events());
    paper.reconnected();
    assert_eq!(next(&mut events), Some(TradingEvent::Resync), "the first");
    assert_eq!(
        next(&mut events),
        Some(TradingEvent::Resync),
        "the reconnect"
    );
    assert!(next(&mut events).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_consumer_waiting_on_another_task_is_woken_by_the_next_event() {
    let paper = Paper::with_liquidity("3");
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());
    let mut events = venue.events();
    drop(venue);

    let consumer = tokio::spawn(async move {
        let mut seen = Vec::new();
        while seen.len() < 3 {
            seen.push(events.next().await.expect("the stream is open").unwrap());
        }
        seen
    });

    run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    let seen = tokio::time::timeout(Duration::from_secs(5), consumer)
        .await
        .expect("the waiting consumer was woken")
        .unwrap();

    assert_eq!(seen[0], TradingEvent::Resync);
    assert!(matches!(&seen[1], TradingEvent::Ack(_)));
    assert!(matches!(&seen[2], TradingEvent::Fill(_)));
}

#[test]
fn reconciliation_reads_back_statically_and_through_dyn() {
    let paper = Paper::with_liquidity("1");
    let venue: Arc<DynTrading<'static>> = DynTrading::new_arc(paper.clone());

    let resting = run(buy(&*venue, "4", TimeInForce::GoodTilCancelled)).unwrap();
    assert_eq!(resting.status, OrderStatus::Open);
    assert_eq!(resting.filled, dec("1"));

    let open = run(venue.open_orders(Some(&yes()))).unwrap();
    assert_eq!(open, vec![resting.clone()]);
    assert_eq!(run(paper.open_orders(None)).unwrap(), open);

    let positions = run(venue.positions()).unwrap();
    assert_eq!(positions[0].key, yes());
    assert_eq!(positions[0].size, dec("1"));

    let balances = run(venue.balances()).unwrap();
    assert_eq!(balances[0].asset, "USD");
    assert_eq!(balances[0].available, Some(dec("100")));

    run(venue.cancel(&resting.id)).unwrap();
    let err = run(venue.cancel(&resting.id)).unwrap_err();
    assert_eq!(err.class, Class::VenueRefusal { code: None });
    assert!(run(venue.open_orders(None)).unwrap().is_empty());

    let first = run(buy(&paper, "2", TimeInForce::GoodTilCancelled)).unwrap();
    let cancelled = run(venue.cancel_all(Some(&yes()))).unwrap();
    assert_eq!(cancelled, vec![first.id]);
    assert!(run(paper.cancel_all(None)).unwrap().is_empty());
}

#[test]
fn the_constructors_fill_the_documented_defaults() {
    assert_eq!(TimeInForce::default(), TimeInForce::GoodTilCancelled);

    let request = OrderRequest::new(yes(), Side::Buy, dec("0.56"), dec("2"));
    assert_eq!(request.key, yes());
    assert_eq!(request.side, Side::Buy);
    assert_eq!(request.price, dec("0.56"));
    assert_eq!(request.size, dec("2"));
    assert_eq!(request.time_in_force, TimeInForce::GoodTilCancelled);
    assert_eq!(request.client_order_id, None);
    assert!(!request.post_only);
    assert!(!request.reduce_only);
    assert!(request.extensions.is_empty());

    let order = Order::new(
        OrderId::from("o"),
        yes(),
        Side::Sell,
        dec("0.6"),
        dec("2"),
        OrderStatus::Open,
    );
    assert_eq!(order.filled, Decimal::ZERO);
    assert_eq!(order.time_in_force, None);
    assert_eq!(order.client_order_id, None);
    assert_eq!(order.created, None);
    assert!(order.extensions.is_empty());

    let fill = Fill::new(
        FillId::from("f"),
        OrderId::from("o"),
        yes(),
        Side::Buy,
        dec("0.56"),
        dec("1"),
        UnixMillis(5),
    );
    assert_eq!(fill.trade_id, None);
    assert_eq!(fill.notional, None);
    assert_eq!(fill.fee, None);
    assert_eq!(fill.liquidity, None);
    assert_eq!(fill.client_order_id, None);
    assert_eq!(fill.time, UnixMillis(5));
    assert!(fill.extensions.is_empty());

    let position = Position::new(yes(), dec("-1"));
    assert_eq!(position.size, dec("-1"));
    assert_eq!(position.average_price, None);
    assert!(position.extensions.is_empty());

    let balance = Balance::new("USD");
    assert_eq!(balance.asset, "USD");
    assert_eq!(balance.total, None);
    assert_eq!(balance.available, None);
    assert!(balance.extensions.is_empty());
}

#[test]
fn the_ids_are_their_text() {
    let order = OrderId::from("ee587a1c-0001");
    assert_eq!(order.as_str(), "ee587a1c-0001");
    assert_eq!(order.to_string(), "ee587a1c-0001");
    assert_eq!(order, OrderId::new(String::from("ee587a1c-0001")));
    assert_ne!(order, OrderId::from("ee587a1c-0002"));
    assert_eq!(FillId::new("f-1").to_string(), "f-1");
    assert_eq!(
        ClientOrderId::from(String::from("mine-1")).as_str(),
        "mine-1"
    );
}

#[test]
fn the_events_stream_debug_prints_without_its_contents() {
    let events = TradingEvents::new(futures_util::stream::empty());
    assert_eq!(format!("{events:?}"), "TradingEvents(..)");
}

#[test]
fn the_events_stream_reports_the_size_hint_of_the_stream_it_wraps() {
    let items = || {
        futures_util::stream::iter(vec![
            Ok(TradingEvent::Resync),
            Ok(TradingEvent::Resync),
            Ok(TradingEvent::Resync),
        ])
    };
    let wrapped = TradingEvents::new(items());
    assert_eq!(items().size_hint(), (3, Some(3)));
    assert_eq!(wrapped.size_hint(), items().size_hint());
}
