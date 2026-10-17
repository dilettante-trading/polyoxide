//! The trading trait, its records, and the stream it reports on.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;
use rust_decimal::Decimal;

use crate::{
    ClassifiedError, ClientOrderId, Extensions, FillId, MarketKey, OrderId, Side, TradeId,
    UnixMillis,
};

/// Places, cancels and reconciles orders on one venue, and reports what
/// happens to them.
///
/// Every method returns [`ClassifiedError`], which an implementation builds
/// from its module error with `?`. Held as `Arc<DynTrading<'static>>` by code
/// that picks its venue at run time.
///
/// # Requests
///
/// An [`OrderRequest`] names an outcome or instrument key, a [`Side`], a price
/// and a size in the instrument's native quantity. The venue's own options
/// ride in its `extensions`, and the venue reads only the option types it
/// declares: [`place`](Self::place) refuses a request carrying any other, as
/// [`Class::InvalidRequest`](crate::Class::InvalidRequest), and sends nothing
/// ([`Extensions::ensure_declared`]). So does a key of another venue or
/// product, and so does a request whose `post_only`, `reduce_only` or time in
/// force the venue cannot honour: it is refused, never ignored.
///
/// # Kills
///
/// An immediate-or-cancel order that matched nothing, or a fill-or-kill order
/// that could not fill in full, is not an error: `place` returns `Ok` with
/// [`OrderStatus::Killed`], and its reason. `filled` says how much an
/// immediate-or-cancel order matched before its remainder was cancelled.
///
/// # Events
///
/// [`events`](Self::events) is fed from an unbounded queue, so a slow consumer
/// loses no acknowledgement or fill. The venue replays nothing: what happened
/// while the connection that carries fills was down is not sent again.
///
/// The stream's first item is [`TradingEvent::Resync`], yielded once the
/// connection's first attempt to connect has ended, whether it went live or
/// began in an outage, so a fill between `events()` returning and the first
/// subscription is covered by a reconciliation. `Resync` comes again after
/// every reconnect of the connection that carries fills, and when the venue
/// voids a fill it reported. The consumer reconciles its inventory each
/// time through [`open_orders`](Self::open_orders) and the venue's trade history.
#[dynosaur::dynosaur(pub DynTrading = dyn(box) Trading)]
pub trait Trading: Send + Sync {
    /// Places `order` and returns it as the venue accepted it: open, filled,
    /// or killed.
    fn place(
        &self,
        order: OrderRequest,
    ) -> impl Future<Output = Result<Order, ClassifiedError>> + Send;

    /// Cancels the order `id`.
    ///
    /// Cancelling an order that has already ended, or that the venue does not
    /// know, is an error classed
    /// [`Class::VenueRefusal`](crate::Class::VenueRefusal). The order's final
    /// state arrives through [`events`](Self::events) and
    /// [`open_orders`](Self::open_orders).
    fn cancel(&self, id: &OrderId) -> impl Future<Output = Result<(), ClassifiedError>> + Send;

    /// Cancels every open order, or every open order on `key`, and returns the
    /// ids the venue cancelled. An order the venue did not cancel is left out
    /// of them and stays open, so [`open_orders`](Self::open_orders) shows it.
    fn cancel_all(
        &self,
        key: Option<&MarketKey>,
    ) -> impl Future<Output = Result<Vec<OrderId>, ClassifiedError>> + Send;

    /// Every open order, or every open order on `key`.
    fn open_orders(
        &self,
        key: Option<&MarketKey>,
    ) -> impl Future<Output = Result<Vec<Order>, ClassifiedError>> + Send;

    /// Every position the account holds.
    fn positions(&self) -> impl Future<Output = Result<Vec<Position>, ClassifiedError>> + Send;

    /// Every balance the account holds.
    fn balances(&self) -> impl Future<Output = Result<Vec<Balance>, ClassifiedError>> + Send;

    /// The account's acknowledgements and fills from now on, beginning with a
    /// [`TradingEvent::Resync`] and carrying one after every reconnect.
    ///
    /// Each call returns a stream of its own, which sees every event from that
    /// call on. An order may be reported (acked or filled) before the
    /// [`place`](Self::place) that created it returns. See [`TradingEvents`]
    /// for when the stream ends.
    fn events(&self) -> TradingEvents;
}

/// The stream [`Trading::events`] returns: the account's acknowledgements and
/// fills, from an unbounded queue.
///
/// It is `Send`, `Unpin` and `'static`, so a consumer can move it onto a task
/// of its own, whether it reached the venue statically or through
/// `Arc<DynTrading<'static>>`. (A trait method returning `impl Stream + 'static`
/// would lose the `'static` through the dyn wrapper, so the stream is this
/// named type.)
///
/// Its first item is [`TradingEvent::Resync`]. The venue replays nothing: after
/// every reconnect of the connection that carries fills, and when the venue
/// voids a fill it reported (a trade that failed after it matched, which the
/// stream has no event to take back), the stream yields `Resync` before any
/// later fill. What happened while the connection was down will not arrive, and
/// the consumer reconciles through [`Trading::open_orders`] and the venue's
/// trade history.
///
/// An `Err` item the venue recovered from, such as a lost connection it
/// re-established, is followed by more items. After one it could not recover
/// from, the stream ends. `None` means the feed has stopped for good.
pub struct TradingEvents(
    Pin<Box<dyn Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static>>,
);

impl TradingEvents {
    /// Wraps the stream a [`Trading`] implementation feeds from its queue.
    pub fn new(
        stream: impl Stream<Item = Result<TradingEvent, ClassifiedError>> + Send + 'static,
    ) -> Self {
        Self(Box::pin(stream))
    }
}

impl Stream for TradingEvents {
    type Item = Result<TradingEvent, ClassifiedError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

/// How long an order rests.
///
/// An order that expires at a set time is a venue option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TimeInForce {
    /// Rests until it fills or is cancelled.
    #[default]
    GoodTilCancelled,
    /// Matches what it can at once; the remainder is cancelled.
    ImmediateOrCancel,
    /// Fills in full at once, or not at all. A venue that cancels one partway,
    /// such as for self-trade prevention, ends it
    /// [`Cancelled`](OrderStatus::Cancelled), possibly partly filled, not
    /// [`Killed`](OrderStatus::Killed).
    FillOrKill,
}

/// An order to place.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderRequest {
    /// The instrument, or the outcome of an event, to trade.
    pub key: MarketKey,
    /// Buy or sell.
    pub side: Side,
    /// The limit price.
    pub price: Decimal,
    /// The size, in the instrument's native quantity.
    pub size: Decimal,
    /// How long it rests.
    pub time_in_force: TimeInForce,
    /// The caller's id for it. The venue echoes it, or the implementation
    /// keeps it for the orders it placed, as its documentation says; a venue
    /// that can do neither refuses a request that carries one.
    pub client_order_id: Option<ClientOrderId>,
    /// Rest only: refuse to take liquidity.
    pub post_only: bool,
    /// Only reduce a position, never open or grow one.
    pub reduce_only: bool,
    /// The venue's own options; one it does not declare is refused.
    pub extensions: Extensions,
}

impl OrderRequest {
    /// A good-til-cancelled order to `side` `size` of `key` at `price`, with
    /// no client id, neither flag and no extensions.
    pub fn new(key: MarketKey, side: Side, price: Decimal, size: Decimal) -> Self {
        Self {
            key,
            side,
            price,
            size,
            time_in_force: TimeInForce::default(),
            client_order_id: None,
            post_only: false,
            reduce_only: false,
            extensions: Extensions::new(),
        }
    }
}

/// Why an order was killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KillReason {
    /// An immediate-or-cancel order's remainder, cancelled because nothing
    /// more matched at once. `filled` says how much did.
    Unmatched,
    /// A fill-or-kill order that could not fill in full.
    Unfillable,
}

/// Where an order stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OrderStatus {
    /// Resting, possibly partly filled.
    Open,
    /// Filled in full.
    Filled,
    /// Cancelled, by the caller or the venue, possibly partly filled.
    Cancelled,
    /// Killed by its time in force: a defined outcome, not an error.
    Killed {
        /// Why.
        reason: KillReason,
    },
}

/// An order as the venue reports it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Order {
    /// The venue's id for it.
    pub id: OrderId,
    /// The instrument or outcome it trades.
    pub key: MarketKey,
    /// Buy or sell.
    pub side: Side,
    /// The limit price.
    pub price: Decimal,
    /// The size ordered, in the instrument's native quantity.
    pub size: Decimal,
    /// How much has filled.
    pub filled: Decimal,
    /// Where it stands.
    pub status: OrderStatus,
    /// How long it rests.
    pub time_in_force: Option<TimeInForce>,
    /// The caller's id for it.
    pub client_order_id: Option<ClientOrderId>,
    /// When the venue accepted it.
    pub created: Option<UnixMillis>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Order {
    /// The order `id` to `side` `size` of `key` at `price`, with nothing
    /// filled yet.
    pub fn new(
        id: OrderId,
        key: MarketKey,
        side: Side,
        price: Decimal,
        size: Decimal,
        status: OrderStatus,
    ) -> Self {
        Self {
            id,
            key,
            side,
            price,
            size,
            filled: Decimal::ZERO,
            status,
            time_in_force: None,
            client_order_id: None,
            created: None,
            extensions: Extensions::new(),
        }
    }
}

/// Whether a fill took or made liquidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Liquidity {
    /// The order rested and was matched.
    Maker,
    /// The order matched a resting one.
    Taker,
}

/// One order's part in a trade.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Fill {
    /// The venue's id for the fill.
    pub id: FillId,
    /// The order filled.
    pub order_id: OrderId,
    /// The trade it was part of.
    pub trade_id: Option<TradeId>,
    /// The instrument or outcome traded.
    pub key: MarketKey,
    /// The order's side.
    pub side: Side,
    /// The price.
    pub price: Decimal,
    /// The size, in the instrument's native quantity.
    pub size: Decimal,
    /// The quote-currency amount, where the venue reports one.
    pub notional: Option<Decimal>,
    /// The fee charged, in the quote currency unless the venue's own facts say
    /// otherwise.
    pub fee: Option<Decimal>,
    /// Whether the order made or took liquidity.
    pub liquidity: Option<Liquidity>,
    /// The caller's id for the order.
    pub client_order_id: Option<ClientOrderId>,
    /// When it filled.
    pub time: UnixMillis,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Fill {
    /// The fill `id` of order `order_id`: `size` of `key` at `price`, at
    /// `time`.
    pub fn new(
        id: FillId,
        order_id: OrderId,
        key: MarketKey,
        side: Side,
        price: Decimal,
        size: Decimal,
        time: UnixMillis,
    ) -> Self {
        Self {
            id,
            order_id,
            trade_id: None,
            key,
            side,
            price,
            size,
            notional: None,
            fee: None,
            liquidity: None,
            client_order_id: None,
            time,
            extensions: Extensions::new(),
        }
    }
}

/// What the account holds of one instrument or outcome.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Position {
    /// The instrument or outcome.
    pub key: MarketKey,
    /// The size held, in the instrument's native quantity: positive long,
    /// negative short. A position in an event's outcome is never negative,
    /// since each outcome has its own key.
    pub size: Decimal,
    /// The average price paid.
    pub average_price: Option<Decimal>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Position {
    /// `size` of `key`.
    pub fn new(key: MarketKey, size: Decimal) -> Self {
        Self {
            key,
            size,
            average_price: None,
            extensions: Extensions::new(),
        }
    }
}

/// What the account holds of one asset.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Balance {
    /// The asset, as the venue names it.
    pub asset: String,
    /// The whole balance.
    pub total: Option<Decimal>,
    /// What is free to trade.
    pub available: Option<Decimal>,
    /// The venue's own facts.
    pub extensions: Extensions,
}

impl Balance {
    /// The balance of `asset`, with no amounts yet.
    pub fn new(asset: impl Into<String>) -> Self {
        Self {
            asset: asset.into(),
            total: None,
            available: None,
            extensions: Extensions::new(),
        }
    }
}

/// What [`Trading::events`] reports.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TradingEvent {
    /// The venue accepted, changed or ended an order.
    Ack(Order),
    /// An order filled, in part or in full.
    Fill(Fill),
    /// Reconcile through [`Trading::open_orders`] and the venue's trade
    /// history: the stream cannot vouch for what it reported or missed before
    /// this point.
    ///
    /// It is the stream's first item, comes again after every reconnect of
    /// the connection that carries fills, and comes when the venue voids a fill
    /// it already reported.
    Resync,
}
