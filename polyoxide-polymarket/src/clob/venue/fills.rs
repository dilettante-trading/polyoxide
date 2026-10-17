//! The events adapter: `Trading::events` for the CLOB, from the supervised
//! user socket's `UserEvent`s to the trait's `TradingEvent`s.
//!
//! A stream opens its own socket the first time it is polled. It resolves the
//! maker address the client trades from (its signature type's, as `place` and
//! `positions` do: a client built for a proxy wallet follows that wallet's
//! fills), starts the user socket on every market of the
//! account (`None`) with the account's credentials, and yields a leading
//! `Resync` once that first connect attempt has ended, whether the socket went
//! live or began in an outage, so a consumer reconciles before it trusts the
//! stream. It then maps each socket event in order:
//!
//! - an order message is an ack, with the client order id the ledger holds;
//! - a `MATCHED` trade is one fill per order of the account's in it, the
//!   taker's first (the trade's `owner` is the taker's API key) and then each
//!   maker entry at the account's maker address, ignoring ASCII case;
//! - a `FAILED` trade voids a fill already reported, and the trait has no
//!   event that takes a fill back, so it is a `Resync` and the consumer
//!   reconciles; `MINED`,
//!   `CONFIRMED` and `RETRYING` yield nothing, so a trade is reported once;
//! - a reconnect is a `Resync`, and a disconnect that carries an error yields
//!   that error;
//! - text that does not parse is an error item and its message is skipped.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Mutex;

use futures_util::StreamExt;
use polyoxide_venue::{
    ClassifiedError, Extensions, Fill, FillId, Liquidity, Order, OrderId, OrderStatus, Side,
    TradeId, TradingEvent, TradingEvents, UnixMillis,
};
use rust_decimal::Decimal;

use super::decode;
use super::trading::{decimal_text, key_of, lock, side_text, ClientIds, FillExt, OrderExt};
use crate::clob::error::ClobError;
use crate::clob::ws::{
    ApiCredentials, ClobWsError, DisconnectReason, OrderEventType, OrderMessage, SupervisedUserWs,
    TradeMessage, TradeStatus, UserEvent, UserMessage,
};
use crate::clob::Clob;

type Item = Result<TradingEvent, ClassifiedError>;

/// The stream `Trading::events` returns for `clob`.
pub(super) fn events(clob: &Clob) -> TradingEvents {
    TradingEvents::new(futures_util::stream::unfold(
        State {
            clob: clob.clone(),
            phase: Phase::Start,
            pending: VecDeque::new(),
        },
        step,
    ))
}

struct State {
    clob: Clob,
    phase: Phase,
    /// Items made from one socket event that the stream has not yet yielded.
    pending: VecDeque<Item>,
}

enum Phase {
    /// Not yet polled: the socket is not open.
    Start,
    Live(Live),
    /// The socket ended, or could not be opened.
    Done,
}

struct Live {
    socket: Pin<Box<SupervisedUserWs>>,
    /// The account's API key, which a trade's `owner` is when it took.
    api_key: String,
    /// The account's maker address, which a maker entry names when it rested.
    maker: String,
}

async fn step(mut state: State) -> Option<(Item, State)> {
    loop {
        if let Some(item) = state.pending.pop_front() {
            return Some((item, state));
        }
        match std::mem::replace(&mut state.phase, Phase::Done) {
            Phase::Done => return None,
            Phase::Start => match open(&state.clob).await {
                Ok(live) => {
                    state.pending.push_back(Ok(TradingEvent::Resync));
                    state.phase = Phase::Live(live);
                }
                // The only item, then the stream ends.
                Err(err) => state.pending.push_back(Err(err)),
            },
            Phase::Live(mut live) => {
                // The socket ended, as after an error recovery stopped on.
                let Some(item) = live.socket.next().await else {
                    continue;
                };
                translate(
                    item,
                    &live.api_key,
                    &live.maker,
                    &state.clob.ids,
                    &mut state.pending,
                );
                state.phase = Phase::Live(live);
            }
        }
    }
}

/// Resolves the maker address and starts the socket on every market.
async fn open(clob: &Clob) -> Result<Live, ClassifiedError> {
    let account = clob
        .account
        .as_ref()
        .ok_or_else(|| ClobError::validation("Account required for the events stream"))?;
    // The address `place` trades from: the client's own signature type's.
    let maker = clob
        .resolve_maker_address(None, clob.signature_type, account)
        .await?;
    let credentials = account.credentials();
    let socket = clob
        .user_socket
        .clone()
        .start(
            None,
            ApiCredentials::new(
                credentials.key.clone(),
                credentials.secret.clone(),
                credentials.passphrase.clone(),
            ),
        )
        .await?;
    Ok(Live {
        socket: Box::pin(socket),
        api_key: credentials.key.clone(),
        maker: maker.to_string(),
    })
}

/// Maps one socket event to the items it yields, in order.
fn translate(
    item: Result<UserEvent, ClobWsError>,
    api_key: &str,
    maker: &str,
    ids: &Mutex<ClientIds>,
    out: &mut VecDeque<Item>,
) {
    match item {
        Err(err) => out.push_back(Err(err.into())),
        Ok(UserEvent::Reconnected) => out.push_back(Ok(TradingEvent::Resync)),
        Ok(UserEvent::Disconnected {
            reason: DisconnectReason::Error(err),
        }) => out.push_back(Err(err.into())),
        // The `Resync` after the reconnect follows either way.
        Ok(UserEvent::Disconnected { .. }) => {}
        Ok(UserEvent::Message(UserMessage::Order(message))) => {
            out.push_back(ack(&message, ids).map(TradingEvent::Ack));
        }
        Ok(UserEvent::Message(UserMessage::Trade(trade))) => {
            on_trade(&trade, api_key, maker, ids, out);
        }
    }
}

fn on_trade(
    trade: &TradeMessage,
    api_key: &str,
    maker: &str,
    ids: &Mutex<ClientIds>,
    out: &mut VecDeque<Item>,
) {
    match trade.status {
        // A trade is reported once, when the CLOB matches it.
        TradeStatus::Matched => match fills(trade, api_key, maker, ids) {
            Ok(fills) => out.extend(fills.into_iter().map(|fill| Ok(TradingEvent::Fill(fill)))),
            Err(err) => out.push_back(Err(err)),
        },
        // Settlement gave up: the fill reported at `MATCHED` did not happen.
        TradeStatus::Failed => out.push_back(Ok(TradingEvent::Resync)),
        TradeStatus::Mined | TradeStatus::Confirmed | TradeStatus::Retrying => {}
    }
}

/// An order message as the ack it is. The message does not carry the time in
/// force, so a FAK remainder the venue cancelled is acked `Cancelled` though
/// `place` returned it `Killed`.
fn ack(message: &OrderMessage, ids: &Mutex<ClientIds>) -> Result<Order, ClassifiedError> {
    let size = decimal_text(&message.original_size, "an original size")?;
    let filled = decimal_text(&message.size_matched, "a matched size")?;
    let status = match message.order_type {
        OrderEventType::Placement => OrderStatus::Open,
        OrderEventType::Update if filled >= size => OrderStatus::Filled,
        OrderEventType::Update => OrderStatus::Open,
        OrderEventType::Cancellation => OrderStatus::Cancelled,
    };
    // Only a placement's time is the order's creation.
    let created = match message.order_type {
        OrderEventType::Placement => Some(millis(&message.timestamp)?),
        OrderEventType::Update | OrderEventType::Cancellation => None,
    };
    let id = OrderId::new(message.id.as_str());
    let mut order = Order::new(
        id.clone(),
        key_of(&message.asset_id)?,
        side_text(&message.side)?,
        decimal_text(&message.price, "a price")?,
        size,
        status,
    );
    order.filled = filled;
    order.created = created;
    order.client_order_id = {
        let mut ids = lock(ids);
        let client = ids.client_of(&id);
        // An order that ended has no more acks to carry its client id.
        if matches!(status, OrderStatus::Filled | OrderStatus::Cancelled) {
            ids.remove(&id);
        }
        client
    };
    order.extensions = Extensions::of(OrderExt {
        condition_id: Some(message.market.clone()),
        outcome: Some(message.outcome.clone()),
        ..OrderExt::default()
    });
    Ok(order)
}

/// The fills of a matched trade: the taker's, when the account took, then
/// each maker entry of the account's.
fn fills(
    trade: &TradeMessage,
    api_key: &str,
    maker: &str,
    ids: &Mutex<ClientIds>,
) -> Result<Vec<Fill>, ClassifiedError> {
    let side = side_text(&trade.side)?;
    let time = millis(&trade.timestamp)?;
    let mut out = Vec::new();
    if trade.owner.as_deref() == Some(api_key) {
        let order = OrderId::new(trade.taker_order_id.as_str());
        let mut fill = Fill::new(
            fill_id(&trade.id, &order),
            order,
            key_of(&trade.asset_id)?,
            side,
            decimal_text(&trade.price, "a price")?,
            decimal_text(&trade.size, "a size")?,
            time,
        );
        fill.liquidity = Some(Liquidity::Taker);
        fill.extensions = Extensions::of(FillExt {
            condition_id: Some(trade.market.clone()),
            outcome: Some(trade.outcome.clone()),
            taker_order_id: Some(trade.taker_order_id.clone()),
            transaction_hash: trade.transaction_hash.clone(),
            fee_rate_bps: None,
        });
        out.push(fill);
    }
    for entry in &trade.maker_orders {
        if !entry.maker_address.eq_ignore_ascii_case(maker) {
            continue;
        }
        let order = OrderId::new(entry.order_id.as_str());
        // On the trade's own token the maker is the other side; on the
        // complementary token (a mint or merge match) it is the same side.
        let side = if entry.asset_id == trade.asset_id {
            opposite(side)
        } else {
            side
        };
        let mut fill = Fill::new(
            fill_id(&trade.id, &order),
            order,
            key_of(&entry.asset_id)?,
            side,
            decimal_text(&entry.price, "a price")?,
            decimal_text(&entry.matched_amount, "a matched amount")?,
            time,
        );
        fill.liquidity = Some(Liquidity::Maker);
        fill.extensions = Extensions::of(FillExt {
            condition_id: Some(trade.market.clone()),
            outcome: Some(trade.outcome.clone()),
            taker_order_id: Some(trade.taker_order_id.clone()),
            transaction_hash: trade.transaction_hash.clone(),
            fee_rate_bps: entry
                .fee_rate_bps
                .as_deref()
                .and_then(|bps| bps.parse::<Decimal>().ok()),
        });
        out.push(fill);
    }
    if out.is_empty() {
        return Err(decode("a trade that names none of the account's orders"));
    }
    let trade_id = TradeId::new(trade.id.as_str());
    for fill in &mut out {
        fill.trade_id = Some(trade_id.clone());
        fill.client_order_id = lock(ids).client_of(&fill.order_id);
    }
    Ok(out)
}

/// A fill's id: the trade it is part of and the order it fills.
fn fill_id(trade: &str, order: &OrderId) -> FillId {
    FillId::new(format!("{trade}:{order}"))
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Buy => Side::Sell,
        Side::Sell => Side::Buy,
    }
}

/// A time the CLOB sent as text, in milliseconds.
fn millis(text: &str) -> Result<UnixMillis, ClassifiedError> {
    text.parse()
        .map(UnixMillis)
        .map_err(|_| decode("a timestamp that is not a number of milliseconds"))
}

#[cfg(test)]
mod tests {
    use polyoxide_venue::{Class, ClientOrderId};
    use serde_json::{json, Value};

    use super::*;

    const KEY: &str = "api-key";
    const MAKER: &str = "0xabc0000000000000000000000000000000000001";
    const OTHER: &str = "0xdead000000000000000000000000000000000002";

    fn ids() -> Mutex<ClientIds> {
        Mutex::new(ClientIds::default())
    }

    fn d(text: &str) -> Decimal {
        text.parse().unwrap()
    }

    fn trade(status: &str, owner: &str, makers: Vec<Value>) -> TradeMessage {
        serde_json::from_value(json!({
            "event_type": "trade",
            "id": "t1",
            "asset_id": "100",
            "market": "0xcondition",
            "outcome": "Yes",
            "price": "0.52",
            "size": "4",
            "side": "BUY",
            "status": status,
            "taker_order_id": "taker-1",
            "maker_orders": makers,
            "owner": owner,
            "transaction_hash": "0xhash",
            "timestamp": "1792000000123"
        }))
        .unwrap()
    }

    fn maker(order: &str, address: &str, asset: &str, price: &str) -> Value {
        json!({
            "order_id": order,
            "maker_address": address,
            "matched_amount": "4",
            "fee_rate_bps": "30",
            "asset_id": asset,
            "price": price
        })
    }

    fn order_message(kind: &str, matched: &str, timestamp: &str) -> OrderMessage {
        serde_json::from_value(json!({
            "event_type": "order",
            "id": "0xo1",
            "asset_id": "100",
            "market": "0xcondition",
            "outcome": "Yes",
            "price": "0.52",
            "side": "BUY",
            "original_size": "10",
            "size_matched": matched,
            "type": kind,
            "order_owner": "api-key",
            "timestamp": timestamp
        }))
        .unwrap()
    }

    fn run(trade: &TradeMessage, ids: &Mutex<ClientIds>) -> VecDeque<Item> {
        let mut out = VecDeque::new();
        on_trade(trade, KEY, MAKER, ids, &mut out);
        out
    }

    fn fill_of(item: &Item) -> &Fill {
        match item {
            Ok(TradingEvent::Fill(fill)) => fill,
            other => panic!("expected a fill, got {other:?}"),
        }
    }

    fn translated(item: Result<UserEvent, ClobWsError>) -> VecDeque<Item> {
        let mut out = VecDeque::new();
        translate(item, KEY, MAKER, &ids(), &mut out);
        out
    }

    #[test]
    fn a_taker_and_a_maker_of_the_account_each_get_a_fill() {
        let trade = trade(
            "MATCHED",
            KEY,
            vec![
                maker("m1", OTHER, "100", "0.52"),
                // The account's address in another case, on the complementary
                // token: a mint or merge match.
                maker(
                    "m2",
                    "0xABC0000000000000000000000000000000000001",
                    "101",
                    "0.48",
                ),
            ],
        );

        let out = run(&trade, &ids());

        assert_eq!(out.len(), 2);
        let taker = fill_of(&out[0]);
        assert_eq!(taker.id.as_str(), "t1:taker-1");
        assert_eq!(taker.order_id.as_str(), "taker-1");
        assert_eq!(taker.trade_id, Some(TradeId::new("t1")));
        assert_eq!(taker.key, key_of("100").unwrap());
        assert_eq!(taker.side, Side::Buy);
        assert_eq!(taker.price, d("0.52"));
        assert_eq!(taker.size, d("4"));
        assert_eq!(taker.liquidity, Some(Liquidity::Taker));
        assert_eq!(taker.time, UnixMillis(1_792_000_000_123));
        assert_eq!(taker.fee, None);
        assert_eq!(taker.notional, None);
        let ext = taker.extensions.get::<FillExt>().unwrap();
        assert_eq!(ext.transaction_hash.as_deref(), Some("0xhash"));
        assert_eq!(ext.fee_rate_bps, None);

        let maker = fill_of(&out[1]);
        assert_eq!(maker.id.as_str(), "t1:m2");
        assert_eq!(maker.order_id.as_str(), "m2");
        assert_eq!(maker.key, key_of("101").unwrap());
        // The complementary token's order takes the trade's own side.
        assert_eq!(maker.side, Side::Buy);
        assert_eq!(maker.price, d("0.48"));
        assert_eq!(maker.liquidity, Some(Liquidity::Maker));
        let ext = maker.extensions.get::<FillExt>().unwrap();
        assert_eq!(ext.fee_rate_bps, Some(d("30")));
        assert_eq!(ext.taker_order_id.as_deref(), Some("taker-1"));
    }

    #[test]
    fn a_maker_on_the_trades_own_token_takes_the_opposite_side() {
        let trade = trade(
            "MATCHED",
            "someone-else",
            vec![maker("m1", MAKER, "100", "0.52")],
        );

        let out = run(&trade, &ids());

        assert_eq!(out.len(), 1);
        let fill = fill_of(&out[0]);
        assert_eq!(fill.side, Side::Sell);
        assert_eq!(fill.liquidity, Some(Liquidity::Maker));
    }

    #[test]
    fn a_trade_naming_none_of_the_accounts_orders_is_a_decode_error() {
        let trade = trade(
            "MATCHED",
            "someone-else",
            vec![maker("m1", OTHER, "100", "0.52")],
        );

        let out = run(&trade, &ids());

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].as_ref().unwrap_err().class, Class::Decode);
    }

    #[test]
    fn a_trade_with_unreadable_text_is_one_decode_error_and_no_fill() {
        let mut bad = trade("MATCHED", KEY, vec![maker("m2", MAKER, "100", "0.52")]);
        bad.timestamp = "soon".to_owned();

        let out = run(&bad, &ids());

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].as_ref().unwrap_err().class, Class::Decode);
    }

    #[test]
    fn only_a_matched_trade_is_a_fill_and_a_failed_one_asks_for_a_resync() {
        for (status, expected) in [("MINED", 0), ("CONFIRMED", 0), ("RETRYING", 0)] {
            let out = run(&trade(status, KEY, vec![]), &ids());
            assert_eq!(out.len(), expected, "{status}");
        }
        let matched = run(&trade("MATCHED", KEY, vec![]), &ids());
        assert_eq!(matched.len(), 1);
        assert!(matches!(matched[0], Ok(TradingEvent::Fill(_))));

        let failed = run(&trade("FAILED", KEY, vec![]), &ids());

        assert_eq!(failed.len(), 1);
        assert!(matches!(failed[0], Ok(TradingEvent::Resync)));
    }

    #[test]
    fn a_fill_carries_the_client_order_id_the_ledger_holds() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("taker-1"), ClientOrderId::new("mine"))
            .unwrap();

        let out = run(&trade("MATCHED", KEY, vec![]), &ids);

        assert_eq!(
            fill_of(&out[0]).client_order_id,
            Some(ClientOrderId::new("mine"))
        );
    }

    #[test]
    fn a_placement_is_an_open_order_created_at_the_messages_time() {
        let order = ack(&order_message("PLACEMENT", "0", "1792000000123"), &ids()).unwrap();

        assert_eq!(order.id.as_str(), "0xo1");
        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.created, Some(UnixMillis(1_792_000_000_123)));
        assert_eq!(order.size, d("10"));
        assert_eq!(order.filled, Decimal::ZERO);
        assert_eq!(order.side, Side::Buy);
        // The message does not carry the time in force.
        assert_eq!(order.time_in_force, None);
    }

    #[test]
    fn an_update_that_reaches_the_size_is_filled_and_drops_the_ledger_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        let order = ack(&order_message("UPDATE", "10", "1792000000999"), &ids).unwrap();

        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.filled, d("10"));
        assert_eq!(order.created, None);
        assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine")));
        assert_eq!(lock(&ids).holder(&ClientOrderId::new("mine")), None);
    }

    #[test]
    fn a_cancellation_is_cancelled_and_drops_the_ledger_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        let order = ack(&order_message("CANCELLATION", "0", "1792000000999"), &ids).unwrap();

        assert_eq!(order.status, OrderStatus::Cancelled);
        assert_eq!(order.client_order_id, Some(ClientOrderId::new("mine")));
        assert_eq!(lock(&ids).holder(&ClientOrderId::new("mine")), None);
    }

    #[test]
    fn a_partial_update_stays_open_and_keeps_its_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        // Only a placement's timestamp is read, so this one is not an error.
        let order = ack(&order_message("UPDATE", "4", "not read"), &ids).unwrap();

        assert_eq!(order.status, OrderStatus::Open);
        assert_eq!(order.filled, d("4"));
        assert_eq!(
            lock(&ids).holder(&ClientOrderId::new("mine")),
            Some(OrderId::new("0xo1"))
        );
    }

    #[test]
    fn a_placement_with_an_unreadable_time_is_a_decode_error_and_keeps_its_entry() {
        let ids = ids();
        lock(&ids)
            .claim(OrderId::new("0xo1"), ClientOrderId::new("mine"))
            .unwrap();

        let err = ack(&order_message("PLACEMENT", "0", "soon"), &ids).unwrap_err();

        assert_eq!(err.class, Class::Decode);
        assert!(lock(&ids).holder(&ClientOrderId::new("mine")).is_some());
    }

    #[test]
    fn a_reconnect_is_a_resync_and_a_disconnect_is_silent_unless_it_carries_an_error() {
        let reconnected = translated(Ok(UserEvent::Reconnected));
        assert_eq!(reconnected.len(), 1);
        assert!(matches!(reconnected[0], Ok(TradingEvent::Resync)));

        let rotated = translated(Ok(UserEvent::Disconnected {
            reason: DisconnectReason::Rotation,
        }));
        assert!(rotated.is_empty());

        let failed = translated(Ok(UserEvent::Disconnected {
            reason: DisconnectReason::Error(ClobWsError::ConnectionClosed),
        }));
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].as_ref().unwrap_err().class, Class::Network);

        let refused = translated(Err(ClobWsError::Authentication(
            "authentication failed".to_owned(),
        )));
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].as_ref().unwrap_err().class, Class::Unauthorized);
    }

    #[test]
    fn a_message_goes_to_the_adapter_of_its_kind() {
        let order = translated(Ok(UserEvent::Message(UserMessage::Order(order_message(
            "PLACEMENT",
            "0",
            "1792000000123",
        )))));
        assert!(matches!(order[0], Ok(TradingEvent::Ack(_))));

        let trade = translated(Ok(UserEvent::Message(UserMessage::Trade(trade(
            "MATCHED",
            KEY,
            vec![],
        )))));
        assert!(matches!(trade[0], Ok(TradingEvent::Fill(_))));
    }
}
