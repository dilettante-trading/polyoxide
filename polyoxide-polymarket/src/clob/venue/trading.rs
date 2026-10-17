//! `Trading` for the CLOB: orders placed and cancelled through the native
//! routes, the account's orders, positions and balance read back, and the
//! in-process ledger of client order ids. The events stream is in `fills`.
//!
//! This module also holds the options a request carries to the CLOB
//! ([`ClobOrderOptions`]) and the CLOB's facts that ride beside each trading
//! record, one `<Record>Ext` type per record (NFR8).

use std::collections::BTreeMap;

use polyoxide_venue::UnixMillis;
use rust_decimal::Decimal;

use crate::clob::types::TickSize;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use alloy::primitives::hex;
use polyoxide_venue::{
    Balance, Class, ClassifiedError, ClientOrderId, Extensions, KillReason, MarketKey, Order,
    OrderId, OrderRequest, OrderStatus, Position, Side, TimeInForce, Trading, TradingEvents,
};
use rust_decimal::prelude::ToPrimitive;

use crate::clob::core::eip712::order_hash;
use crate::clob::error::ClobError;
use crate::clob::types::{Order as NativeOrder, OrderKind, OrderSide, PartialCreateOrderOptions};
use crate::clob::{Clob, CreateOrderParams};
use crate::shared::keys;

/// Options only the CLOB reads, carried in an
/// [`OrderRequest`](polyoxide_venue::OrderRequest)'s `extensions`.
///
/// A request carrying any other extension type is refused. Each field is a
/// fact the caller already holds, so the client does not fetch it: it looks a
/// missing `neg_risk` or `tick_size` up. A V2 order signs no fee rate, so
/// there are no fee terms here.
///
/// It is not `Eq`, because [`TickSize`] is not.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct ClobOrderOptions {
    /// Whether the market is in a negative-risk event, which selects the
    /// exchange contract the order is signed for.
    pub neg_risk: Option<bool>,
    /// The market's tick size now in force.
    pub tick_size: Option<TickSize>,
    /// When a good-til-cancelled order expires. Set, it becomes a
    /// good-til-date order; the CLOB reads whole seconds, so the time is
    /// rounded down to one.
    pub expiration: Option<UnixMillis>,
}

/// The CLOB's facts about an order, beside
/// [`Order`](polyoxide_venue::Order)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct OrderExt {
    /// The CLOB's own status word: `live`, `matched` or `delayed` from a
    /// placement, `LIVE`, `MATCHED`, `CANCELED` and the rest from a listing.
    pub status: Option<String>,
    /// The condition id of the market the order is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The maker address: the proxy or deposit wallet, not the signing key.
    pub maker_address: Option<String>,
    /// When the order expires, when it does.
    pub expiration: Option<UnixMillis>,
    /// Whether the order was signed for the negative-risk exchange.
    pub neg_risk: Option<bool>,
    /// The ids of the trades the order took part in.
    pub trade_ids: Vec<String>,
    /// The settling transactions' hashes a placement reported.
    pub transaction_hashes: Vec<String>,
}

/// The CLOB's facts about a fill, beside
/// [`Fill`](polyoxide_venue::Fill)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct FillExt {
    /// The condition id of the market the trade is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The taker's order id.
    pub taker_order_id: Option<String>,
    /// The settling transaction's hash, once the CLOB reports one.
    pub transaction_hash: Option<String>,
    /// The maker's fee rate in basis points. A taker's fill has none: the fee
    /// is taken on chain at match time.
    pub fee_rate_bps: Option<Decimal>,
}

/// The CLOB's facts about a position, beside
/// [`Position`](polyoxide_venue::Position)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct PositionExt {
    /// The condition id of the market the outcome is in.
    pub condition_id: Option<String>,
    /// The outcome's label, such as `Yes`.
    pub outcome: Option<String>,
    /// The outcome's index in its market.
    pub outcome_index: Option<u32>,
    /// The outcome's current price.
    pub current_price: Option<Decimal>,
    /// The position's current value.
    pub current_value: Option<Decimal>,
    /// The position's initial value.
    pub initial_value: Option<Decimal>,
    /// The cash profit and loss.
    pub cash_pnl: Option<Decimal>,
    /// The realized profit and loss.
    pub realized_pnl: Option<Decimal>,
    /// Whether the position can be redeemed.
    pub redeemable: Option<bool>,
    /// Whether the position can be merged.
    pub mergeable: Option<bool>,
    /// Whether the market is in a negative-risk event.
    pub negative_risk: Option<bool>,
    /// The market's title.
    pub title: Option<String>,
    /// When the market ends, as the Data API writes it.
    pub end_date: Option<String>,
}

/// The CLOB's facts about a balance, beside
/// [`Balance`](polyoxide_venue::Balance)'s fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct BalanceExt {
    /// Each spender's allowance, by the spender's address, divided by 10^6
    /// as the balance is. An unlimited approval is larger than a `Decimal`
    /// holds and reads as `Decimal::MAX`.
    pub allowances: BTreeMap<String, Decimal>,
}

/// The most client order ids the ledger holds. The oldest goes first.
const LEDGER_BOUND: usize = 10_000;

/// The in-process record of the client order ids of the orders a `Clob`
/// placed.
///
/// The CLOB stores no client id, so the client keeps the ones it was given,
/// by the order's id. Every clone of a `Clob` shares one ledger, and so does
/// every events stream it opens. It is lost when the process ends, and an
/// order placed elsewhere has no entry.
#[derive(Debug, Default)]
pub(crate) struct ClientIds {
    by_order: HashMap<OrderId, (ClientOrderId, u64)>,
    by_client: HashMap<ClientOrderId, OrderId>,
    by_age: BTreeMap<u64, OrderId>,
    next_age: u64,
}

impl ClientIds {
    /// The order holding `client`, when one does.
    pub(crate) fn holder(&self, client: &ClientOrderId) -> Option<OrderId> {
        self.by_client.get(client).cloned()
    }

    /// Records that `order` carries `client`, dropping the oldest entry once
    /// the ledger is full. Refuses with the order already holding `client`.
    pub(crate) fn claim(&mut self, order: OrderId, client: ClientOrderId) -> Result<(), OrderId> {
        if let Some(holder) = self.by_client.get(&client) {
            return Err(holder.clone());
        }
        self.remove(&order);
        while self.by_order.len() >= LEDGER_BOUND {
            let Some((_, oldest)) = self.by_age.pop_first() else {
                break;
            };
            if let Some((evicted, _)) = self.by_order.remove(&oldest) {
                self.by_client.remove(&evicted);
            }
        }
        self.next_age += 1;
        self.by_age.insert(self.next_age, order.clone());
        self.by_client.insert(client.clone(), order.clone());
        self.by_order.insert(order, (client, self.next_age));
        Ok(())
    }

    /// Moves an entry to the id the CLOB gave the order, keeping its age.
    pub(crate) fn rekey(&mut self, from: &OrderId, to: OrderId) {
        if let Some((client, age)) = self.by_order.remove(from) {
            self.by_age.insert(age, to.clone());
            self.by_client.insert(client.clone(), to.clone());
            self.by_order.insert(to, (client, age));
        }
    }

    /// Drops the entry of `order`.
    pub(crate) fn remove(&mut self, order: &OrderId) {
        if let Some((client, age)) = self.by_order.remove(order) {
            self.by_client.remove(&client);
            self.by_age.remove(&age);
        }
    }
}

/// Locks the ledger. No code panics while holding it, so a poisoned lock
/// still holds whole maps and is read as it is.
pub(crate) fn lock(ids: &Mutex<ClientIds>) -> MutexGuard<'_, ClientIds> {
    ids.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A request refused before anything was sent.
fn refuse(message: impl Into<String>) -> ClassifiedError {
    ClassifiedError::new(Class::InvalidRequest, message.into())
}

/// The CLOB answered, and what it answered is a refusal.
fn refusal(message: impl Into<String>) -> ClassifiedError {
    ClassifiedError::new(Class::VenueRefusal { code: None }, message.into())
}

fn duplicate(client: &ClientOrderId, holder: &OrderId) -> ClassifiedError {
    refuse(format!(
        "client order id `{client}` is already in use by order `{holder}`"
    ))
}

/// The order kind a request maps to, after the checks that need no lookup:
/// no reduce-only order, post-only and an expiration only with
/// good-til-cancelled.
fn order_kind(
    request: &OrderRequest,
    options: &ClobOrderOptions,
) -> Result<OrderKind, ClassifiedError> {
    if request.reduce_only {
        return Err(refuse("the CLOB has no reduce-only orders"));
    }
    let resting = request.time_in_force == TimeInForce::GoodTilCancelled;
    if request.post_only && !resting {
        return Err(refuse(
            "the CLOB supports post-only only with good-til-cancelled orders",
        ));
    }
    if options.expiration.is_some() && !resting {
        return Err(refuse(
            "the CLOB supports an expiration only with good-til-cancelled orders",
        ));
    }
    match request.time_in_force {
        TimeInForce::GoodTilCancelled if options.expiration.is_some() => Ok(OrderKind::Gtd),
        TimeInForce::GoodTilCancelled => Ok(OrderKind::Gtc),
        TimeInForce::ImmediateOrCancel => Ok(OrderKind::Fak),
        TimeInForce::FillOrKill => Ok(OrderKind::Fok),
        _ => Err(refuse("the CLOB cannot honour this time in force")),
    }
}

/// The price and size checks: the price lies strictly between 0 and 1 on the
/// tick, and the size has at most two decimals and is at least a hundredth.
fn check_price_and_size(
    price: Decimal,
    size: Decimal,
    tick: TickSize,
) -> Result<(), ClassifiedError> {
    let step = Decimal::new(1, tick.decimals());
    if price <= Decimal::ZERO || price >= Decimal::ONE {
        return Err(refuse(format!(
            "the price {price} must lie strictly between 0 and 1"
        )));
    }
    if !(price % step).is_zero() {
        return Err(refuse(format!(
            "the price {price} is not a whole multiple of the tick size {step}"
        )));
    }
    if size.normalize().scale() > 2 {
        return Err(refuse(format!(
            "the size {size} has more than two decimal places"
        )));
    }
    if size < Decimal::new(1, 2) {
        return Err(refuse(format!(
            "the size {size} is below the smallest order, 0.01 shares"
        )));
    }
    Ok(())
}

/// What a `matched` placement became, once the order was read back.
fn settled(time_in_force: TimeInForce, matched: Decimal, size: Decimal) -> OrderStatus {
    if matched >= size {
        return OrderStatus::Filled;
    }
    match time_in_force {
        TimeInForce::ImmediateOrCancel => OrderStatus::Killed {
            reason: KillReason::Unmatched,
        },
        TimeInForce::FillOrKill => OrderStatus::Killed {
            reason: KillReason::Unfillable,
        },
        _ => OrderStatus::Open,
    }
}

/// Drops the ledger entry of an order that ended at once or never existed.
/// After a `Network` or `Unavailable` failure the post may have reached the
/// venue, so the entry stays until the bound evicts it.
fn tidy(ids: &Mutex<ClientIds>, id: &OrderId, result: &Result<Order, ClassifiedError>) {
    let drop_entry = match result {
        Err(err) => !matches!(err.class, Class::Network | Class::Unavailable { .. }),
        Ok(order) => matches!(
            order.status,
            OrderStatus::Killed { .. } | OrderStatus::Filled
        ),
    };
    if drop_entry {
        let mut ids = lock(ids);
        ids.remove(id);
        if let Ok(order) = result {
            ids.remove(&order.id);
        }
    }
}

/// One placement in flight: the request, the options it carried and what the
/// client resolved before sending.
struct Posting<'a> {
    request: &'a OrderRequest,
    options: ClobOrderOptions,
    kind: OrderKind,
    neg_risk: bool,
    /// The order's EIP-712 hash, computed before it is signed.
    id: OrderId,
}

impl Posting<'_> {
    /// The order the request became, with the CLOB's facts in its extensions.
    fn order(&self, id: OrderId, status: OrderStatus, filled: Decimal, ext: OrderExt) -> Order {
        let mut order = Order::new(
            id,
            self.request.key.clone(),
            self.request.side,
            self.request.price,
            self.request.size,
            status,
        );
        order.filled = filled;
        order.time_in_force = Some(self.request.time_in_force);
        order.client_order_id = self.request.client_order_id.clone();
        order.extensions = Extensions::of(OrderExt {
            expiration: self.options.expiration,
            neg_risk: Some(self.neg_risk),
            ..ext
        });
        order
    }

    /// An order the CLOB killed on arrival.
    fn ended(&self, reason: KillReason) -> Order {
        self.order(
            self.id.clone(),
            OrderStatus::Killed { reason },
            Decimal::ZERO,
            OrderExt::default(),
        )
    }
}

impl Clob {
    /// The market's tick size: the options', else the CLOB's.
    async fn trading_tick_size(
        &self,
        token: &keys::TokenId,
        options: &ClobOrderOptions,
    ) -> Result<TickSize, ClassifiedError> {
        if let Some(tick) = options.tick_size {
            return Ok(tick);
        }
        let response = self.markets().tick_size(token.as_str()).send().await?;
        let value: f64 = response
            .minimum_tick_size
            .parse()
            .map_err(|_| super::decode("a tick size that is not a number"))?;
        Ok(TickSize::try_from(value)?)
    }

    /// Signs and posts one order, and reads the answer into an [`Order`].
    async fn trading_submit(
        &self,
        posting: &Posting<'_>,
        native: &NativeOrder,
    ) -> Result<Order, ClassifiedError> {
        let signed = self.sign_order(native).await?;
        let response = match self
            .post_order(&signed, posting.kind, posting.request.post_only)
            .await
        {
            Ok(response) => response,
            // The kill outcomes are matched on the native variants, before
            // any conversion to a `ClassifiedError` (AD-20).
            Err(ClobError::FakUnmatched { .. }) => {
                return Ok(posting.ended(KillReason::Unmatched));
            }
            Err(ClobError::FokUnfilled { .. }) => {
                return Ok(posting.ended(KillReason::Unfillable));
            }
            Err(err) => return Err(err.into()),
        };
        if !response.success {
            return Err(refusal(
                response
                    .error_msg
                    .clone()
                    .unwrap_or_else(|| "the CLOB refused the order".to_owned()),
            ));
        }
        let id = match response.order_id.as_deref() {
            Some(venue) if !venue.is_empty() && venue != posting.id.as_str() => {
                let venue = OrderId::new(venue);
                tracing::warn!(
                    computed = %posting.id,
                    venue = %venue,
                    "the CLOB named an order other than the one computed; keeping its id"
                );
                lock(&self.ids).rekey(&posting.id, venue.clone());
                venue
            }
            _ => posting.id.clone(),
        };
        let ext = OrderExt {
            status: response.status.clone(),
            transaction_hashes: response.transaction_hashes.clone(),
            ..OrderExt::default()
        };
        let word = response.status.as_deref().map(str::to_ascii_lowercase);
        let (status, filled) = if word.as_deref() == Some("matched") {
            match self.trading_matched_size(&id).await {
                Ok(matched) => (
                    settled(posting.request.time_in_force, matched, posting.request.size),
                    matched,
                ),
                // The order exists on the venue, so an error would invite a
                // duplicate placement; the events stream reports the rest.
                Err(err) => {
                    tracing::warn!(
                        order = %id,
                        error = %err,
                        "could not read back a matched order; reporting it open"
                    );
                    (OrderStatus::Open, Decimal::ZERO)
                }
            }
        } else {
            (OrderStatus::Open, Decimal::ZERO)
        };
        Ok(posting.order(id, status, filled, ext))
    }

    /// What has matched of an order, from `GET /data/order/{id}`.
    async fn trading_matched_size(&self, id: &OrderId) -> Result<Decimal, ClassifiedError> {
        let row = self.orders()?.get(id.as_str()).send().await?;
        row.size_matched
            .parse()
            .map_err(|_| super::decode("a matched size that is not a decimal"))
    }
}

/// `Trading` on the CLOB, through the native order, cancel, Data and balance
/// routes and, for `events`, the supervised user socket.
///
/// Every method needs an account, and fails with the CLOB's own "Account
/// required" error without one. The address that trades is the client's
/// signature type's: `ClobBuilder::signature_type`, else the account target's.
/// `place` signs as it, and `positions` and `events` resolve their maker
/// address from it, so a client built for a proxy wallet trades and reads as
/// that wallet. The CLOB stores no client order id, so a
/// `Clob` remembers the ones it placed orders with, in memory: the ledger is
/// shared by its clones and by the streams `events` opens, holds at most
/// 10,000 entries, is lost when the process ends, and knows nothing of an
/// order placed elsewhere, which carries no client id on any record.
impl Trading for Clob {
    async fn place(&self, request: OrderRequest) -> Result<Order, ClassifiedError> {
        self.account
            .as_ref()
            .ok_or_else(|| ClobError::validation("Account required to place an order"))?;
        let token = keys::token_id(&request.key)?;
        request
            .extensions
            .ensure_declared(&[TypeId::of::<ClobOrderOptions>()])?;
        let options = request
            .extensions
            .get::<ClobOrderOptions>()
            .copied()
            .unwrap_or_default();
        let kind = order_kind(&request, &options)?;
        if let Some(client) = &request.client_order_id {
            let holder = lock(&self.ids).holder(client);
            if let Some(holder) = holder {
                return Err(duplicate(client, &holder));
            }
        }
        let tick = self.trading_tick_size(&token, &options).await?;
        check_price_and_size(request.price, request.size, tick)?;

        let price = request
            .price
            .to_f64()
            .ok_or_else(|| refuse("the price does not fit a float"))?;
        let size = request
            .size
            .to_f64()
            .ok_or_else(|| refuse("the size does not fit a float"))?;
        let params = CreateOrderParams {
            token_id: token.as_str().to_owned(),
            price,
            size,
            side: match request.side {
                Side::Buy => OrderSide::Buy,
                Side::Sell => OrderSide::Sell,
            },
            order_type: kind,
            post_only: request.post_only,
            expiration: options.expiration.map(|time| time.0 / 1_000),
            // The order is signed with the client's own signature type (the
            // builder's override, else the account target's), as `balances`
            // reads with it, and its maker is resolved from it: a client built
            // for a proxy wallet places from that wallet. No per-order funder.
            funder: None,
            signature_type: Some(self.signature_type),
        };
        // The options name what they know, so nothing is fetched twice.
        let native = self
            .create_order(
                &params,
                Some(PartialCreateOrderOptions {
                    tick_size: Some(tick),
                    neg_risk: options.neg_risk,
                }),
            )
            .await?;
        let neg_risk = native.neg_risk;
        // The order's id is its EIP-712 hash, which the client computes before
        // signing, for every signature type: a Deposit Wallet's order is signed
        // as an ERC-7739 envelope, but the exchange names it by this hash too.
        let hash = order_hash(&native, self.chain_id)?;
        let id = OrderId::new(format!("0x{}", hex::encode(hash)));
        if let Some(client) = &request.client_order_id {
            let claimed = lock(&self.ids).claim(id.clone(), client.clone());
            if let Err(holder) = claimed {
                return Err(duplicate(client, &holder));
            }
        }

        let posting = Posting {
            request: &request,
            options,
            kind,
            neg_risk,
            id: id.clone(),
        };
        let result = self.trading_submit(&posting, &native).await;
        tidy(&self.ids, &id, &result);
        result
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn cancel(&self, _id: &OrderId) -> Result<(), ClassifiedError> {
        Err(refusal("cancel is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn cancel_all(&self, _key: Option<&MarketKey>) -> Result<Vec<OrderId>, ClassifiedError> {
        Err(refusal("cancel_all is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn open_orders(&self, _key: Option<&MarketKey>) -> Result<Vec<Order>, ClassifiedError> {
        Err(refusal("open_orders is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn positions(&self) -> Result<Vec<Position>, ClassifiedError> {
        Err(refusal("positions is not implemented yet"))
    }

    // Temporary stub; Task 5.3 replaces it.
    async fn balances(&self) -> Result<Vec<Balance>, ClassifiedError> {
        Err(refusal("balances is not implemented yet"))
    }

    // Temporary stub; Task 5.4 replaces it.
    fn events(&self) -> TradingEvents {
        TradingEvents::new(futures_util::stream::empty::<
            Result<polyoxide_venue::TradingEvent, ClassifiedError>,
        >())
    }
}

#[cfg(test)]
mod place_tests {
    use super::*;
    use crate::shared::keys::TokenId;

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn order(n: usize) -> OrderId {
        OrderId::new(format!("0xorder{n}"))
    }

    fn client(n: usize) -> ClientOrderId {
        ClientOrderId::new(format!("client-{n}"))
    }

    fn request() -> OrderRequest {
        OrderRequest::new(
            keys::clob(&TokenId::new("100").unwrap()),
            Side::Buy,
            d("0.52"),
            d("10"),
        )
    }

    #[test]
    fn a_claimed_client_id_is_refused_to_the_next_order() {
        let mut ids = ClientIds::default();
        ids.claim(order(1), client(1)).unwrap();

        assert_eq!(ids.claim(order(2), client(1)), Err(order(1)));
        assert_eq!(ids.holder(&client(1)), Some(order(1)));
    }

    #[test]
    fn removing_an_order_frees_its_client_id() {
        let mut ids = ClientIds::default();
        ids.claim(order(1), client(1)).unwrap();
        ids.remove(&order(1));

        assert_eq!(ids.holder(&client(1)), None);
        ids.claim(order(2), client(1)).unwrap();
        assert_eq!(ids.holder(&client(1)), Some(order(2)));
    }

    #[test]
    fn the_ledger_drops_its_oldest_entry_past_its_bound() {
        let mut ids = ClientIds::default();
        for n in 0..=LEDGER_BOUND {
            ids.claim(order(n), client(n)).unwrap();
        }

        assert_eq!(ids.by_order.len(), LEDGER_BOUND);
        assert_eq!(ids.by_client.len(), LEDGER_BOUND);
        assert_eq!(ids.by_age.len(), LEDGER_BOUND);
        assert_eq!(ids.holder(&client(0)), None);
        assert_eq!(ids.holder(&client(1)), Some(order(1)));
        assert_eq!(ids.holder(&client(LEDGER_BOUND)), Some(order(LEDGER_BOUND)));
    }

    #[test]
    fn rekeying_moves_the_entry_and_keeps_its_client_id() {
        let mut ids = ClientIds::default();
        ids.claim(order(1), client(1)).unwrap();
        ids.rekey(&order(1), order(9));

        assert_eq!(ids.holder(&client(1)), Some(order(9)));
        ids.remove(&order(1));
        assert_eq!(ids.holder(&client(1)), Some(order(9)));
        ids.remove(&order(9));
        assert_eq!(ids.holder(&client(1)), None);
        assert!(ids.by_age.is_empty());
    }

    #[test]
    fn the_ledger_keeps_an_order_whose_post_may_have_reached_the_venue() {
        let cases = [
            (Class::Network, true),
            (Class::Unavailable { code: None }, true),
            (Class::VenueRefusal { code: None }, false),
            (Class::InvalidRequest, false),
        ];
        for (class, kept) in cases {
            let label = format!("{class:?}");
            let ids = Mutex::new(ClientIds::default());
            lock(&ids).claim(order(1), client(1)).unwrap();

            tidy(&ids, &order(1), &Err(ClassifiedError::new(class, "boom")));

            assert_eq!(lock(&ids).holder(&client(1)).is_some(), kept, "{label}");
        }
    }

    #[test]
    fn the_ledger_drops_an_order_that_ended_at_once_and_keeps_one_that_rests() {
        let ended = [
            (OrderStatus::Filled, false),
            (
                OrderStatus::Killed {
                    reason: KillReason::Unmatched,
                },
                false,
            ),
            (OrderStatus::Open, true),
        ];
        for (status, kept) in ended {
            let ids = Mutex::new(ClientIds::default());
            lock(&ids).claim(order(1), client(1)).unwrap();
            let placed = Order::new(
                order(1),
                request().key,
                Side::Buy,
                d("0.52"),
                d("10"),
                status,
            );

            tidy(&ids, &order(1), &Ok(placed));

            assert_eq!(lock(&ids).holder(&client(1)).is_some(), kept);
        }
    }

    #[test]
    fn a_price_must_sit_on_the_tick_and_strictly_between_zero_and_one() {
        for (price, tick) in [
            ("0.52", TickSize::Hundredth),
            ("0.01", TickSize::Hundredth),
            ("0.99", TickSize::Hundredth),
            ("0.525", TickSize::Thousandth),
            ("0.5", TickSize::Tenth),
            ("0.0001", TickSize::TenThousandth),
        ] {
            assert!(
                check_price_and_size(d(price), d("10"), tick).is_ok(),
                "{price}"
            );
        }
        for (price, tick, why) in [
            ("0", TickSize::Hundredth, "strictly between"),
            ("1", TickSize::Hundredth, "strictly between"),
            ("1.5", TickSize::Hundredth, "strictly between"),
            ("-0.1", TickSize::Hundredth, "strictly between"),
            ("0.525", TickSize::Hundredth, "tick size"),
            ("0.55", TickSize::Tenth, "tick size"),
        ] {
            let err = check_price_and_size(d(price), d("10"), tick).unwrap_err();
            assert_eq!(err.class, Class::InvalidRequest, "{price}");
            assert!(err.to_string().contains(why), "{price}: {err}");
        }
    }

    #[test]
    fn a_size_has_at_most_two_decimals_and_at_least_a_hundredth() {
        for size in ["0.01", "5", "5.1", "5.10", "5.00", "123.45"] {
            assert!(
                check_price_and_size(d("0.52"), d(size), TickSize::Hundredth).is_ok(),
                "{size}"
            );
        }
        for (size, why) in [
            ("5.001", "decimal places"),
            ("0.005", "decimal places"),
            ("0", "smallest order"),
            ("-1", "smallest order"),
        ] {
            let err = check_price_and_size(d("0.52"), d(size), TickSize::Hundredth).unwrap_err();
            assert_eq!(err.class, Class::InvalidRequest, "{size}");
            assert!(err.to_string().contains(why), "{size}: {err}");
        }
    }

    #[test]
    fn the_time_in_force_picks_the_order_kind_and_the_flags_it_allows() {
        let expiring = ClobOrderOptions {
            expiration: Some(UnixMillis(1_900_000_000_000)),
            ..ClobOrderOptions::default()
        };
        let plain = ClobOrderOptions::default();
        let with = |time_in_force, post_only, reduce_only| {
            let mut request = request();
            request.time_in_force = time_in_force;
            request.post_only = post_only;
            request.reduce_only = reduce_only;
            request
        };
        let gtc = TimeInForce::GoodTilCancelled;
        let ioc = TimeInForce::ImmediateOrCancel;
        let fok = TimeInForce::FillOrKill;

        assert_eq!(
            order_kind(&with(gtc, false, false), &plain).unwrap(),
            OrderKind::Gtc
        );
        assert_eq!(
            order_kind(&with(gtc, true, false), &plain).unwrap(),
            OrderKind::Gtc
        );
        assert_eq!(
            order_kind(&with(gtc, false, false), &expiring).unwrap(),
            OrderKind::Gtd
        );
        assert_eq!(
            order_kind(&with(ioc, false, false), &plain).unwrap(),
            OrderKind::Fak
        );
        assert_eq!(
            order_kind(&with(fok, false, false), &plain).unwrap(),
            OrderKind::Fok
        );
        for refused in [
            order_kind(&with(gtc, false, true), &plain),
            order_kind(&with(ioc, true, false), &plain),
            order_kind(&with(fok, false, false), &expiring),
        ] {
            assert_eq!(refused.unwrap_err().class, Class::InvalidRequest);
        }
    }

    #[test]
    fn a_matched_order_settles_by_its_time_in_force_and_what_matched() {
        let gtc = TimeInForce::GoodTilCancelled;
        let ioc = TimeInForce::ImmediateOrCancel;
        let fok = TimeInForce::FillOrKill;

        for time_in_force in [gtc, ioc, fok] {
            assert_eq!(
                settled(time_in_force, d("10"), d("10")),
                OrderStatus::Filled
            );
        }
        assert_eq!(
            settled(ioc, d("4"), d("10")),
            OrderStatus::Killed {
                reason: KillReason::Unmatched
            }
        );
        assert_eq!(
            settled(fok, d("4"), d("10")),
            OrderStatus::Killed {
                reason: KillReason::Unfillable
            }
        );
        assert_eq!(settled(gtc, d("4"), d("10")), OrderStatus::Open);
    }

    /// Hardhat account 0, and the Deposit Wallet it owns in py-sdk's fixtures.
    const KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    const WALLET: alloy::primitives::Address =
        alloy::primitives::address!("57ffbc34de23124faeb8387fcd689d314e57accd");

    /// A Deposit Wallet account can place: the order goes out as signature
    /// type 3 with the wallet as maker and signer, carries the signature
    /// `sign_order_as` gives, and `place` names it by `order_hash`, which no
    /// plain digest could (`compute_order_digest` refuses type 3). The test
    /// sits here because `order_hash` is crate-private.
    #[tokio::test]
    async fn a_deposit_wallet_account_places_a_type_3_order_named_by_its_hash() {
        use crate::clob::types::{SignatureType, SignedOrder};
        use crate::clob::{Account, ClobBuilder, Credentials, DepositWalletRole, SigningTarget};

        let posted = std::sync::Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let sink = posted.clone();
        let mut server = mockito::Server::new_async().await;
        // The answer names no order, so the id `place` returns is the one it
        // computed before signing.
        let post = server
            .mock("POST", "/order")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body_from_request(move |request| {
                sink.lock().unwrap().push(request.body().unwrap().clone());
                br#"{"success":true,"status":"live","transactionsHashes":[]}"#.to_vec()
            })
            .expect(1)
            .create_async()
            .await;
        let account = Account::new(
            KEY,
            Credentials {
                key: "test-key".into(),
                secret: "c2VjcmV0".into(),
                passphrase: "test-pass".into(),
            },
        )
        .unwrap()
        .with_target(SigningTarget::DepositWallet {
            wallet: WALLET,
            role: DepositWalletRole::Owner,
        });
        let clob = ClobBuilder::new()
            .base_url(server.url())
            .with_account(account)
            .build()
            .unwrap();
        let mut asked = request();
        asked
            .extensions
            .insert(ClobOrderOptions {
                tick_size: Some(TickSize::Hundredth),
                neg_risk: Some(false),
                ..ClobOrderOptions::default()
            })
            .unwrap();

        let placed = clob.place(asked).await.unwrap();

        post.assert_async().await;
        let body: serde_json::Value = serde_json::from_slice(&posted.lock().unwrap()[0]).unwrap();
        // The wire writes the salt as a number and the order holds it as text.
        let mut wire = body["order"].clone();
        wire["salt"] = serde_json::Value::String(wire["salt"].to_string());
        let signed: SignedOrder = serde_json::from_value(wire).unwrap();
        assert_eq!(signed.order.signature_type, SignatureType::Poly1271);
        assert_eq!(signed.order.maker, WALLET);
        assert_eq!(signed.order.signer, WALLET);
        let expected = clob.sign_order(&signed.order).await.unwrap();
        assert_eq!(signed.signature, expected.signature);
        assert_eq!(
            signed.signature.len(),
            2 + 317 * 2,
            "the owner's ERC-7739 envelope"
        );
        let hash = order_hash(&signed.order, clob.chain_id).unwrap();
        assert_eq!(placed.id, OrderId::new(format!("0x{}", hex::encode(hash))));
        assert_eq!(placed.status, OrderStatus::Open);
    }
}

#[cfg(test)]
mod option_tests {
    use polyoxide_venue::Extensions;

    use super::*;

    #[test]
    fn the_options_default_to_nothing_set() {
        assert_eq!(
            ClobOrderOptions::default(),
            ClobOrderOptions {
                neg_risk: None,
                tick_size: None,
                expiration: None,
            }
        );
    }

    #[test]
    fn the_options_ride_in_a_requests_extensions() {
        let options = ClobOrderOptions {
            neg_risk: Some(true),
            tick_size: Some(TickSize::Thousandth),
            expiration: Some(UnixMillis(1_792_000_000_000)),
        };
        let extensions = Extensions::of(options);
        assert_eq!(extensions.get::<ClobOrderOptions>(), Some(&options));
    }

    #[test]
    fn the_ext_types_hold_nothing_until_filled() {
        assert_eq!(OrderExt::default().trade_ids, Vec::<String>::new());
        assert_eq!(FillExt::default().fee_rate_bps, None);
        assert_eq!(PositionExt::default().redeemable, None);
        assert!(BalanceExt::default().allowances.is_empty());
    }
}
