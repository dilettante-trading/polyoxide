//! What the supervised socket yields, and the frames Kalshi sends it in.

use std::time::Duration;

use polyoxide_venue::{Level, UnixMillis};
use polyoxide_ws::Outage;
use rust_decimal::Decimal;

use crate::predictions::{
    types::{BookSide, PriceLevel},
    ws::PredictionsWsError,
};

/// What a supervised order-book socket yields.
///
/// Every price is in YES pricing, so a book is one ladder: the bids are YES offers to buy
/// and the asks are the NO bids, priced as the YES offers to sell they are.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// A market's whole book: the first frame of its subscription, and the first after
    /// each reconnect.
    Snapshot(OrderbookSnapshot),
    /// One change to one price level of a book.
    Delta(OrderbookDelta),
    /// The connection went down. One per outage; [`Reconnected`](Self::Reconnected)
    /// follows once the subscription has been replayed, unless the stream ends on an
    /// error it cannot recover from.
    Disconnected {
        /// Why the connection went down.
        reason: DisconnectReason,
    },
    /// The connection is back and the subscription replayed. A fresh snapshot of every
    /// market follows, and what the socket carried in between is lost, so a consumer
    /// rebuilds its books from those.
    Reconnected,
}

/// A market's whole book, in YES pricing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderbookSnapshot {
    /// The market's ticker.
    pub market_ticker: String,
    /// The market's id.
    pub market_id: String,
    /// The YES bids, highest price first.
    pub bids: Vec<Level>,
    /// The YES asks, lowest price first: the NO bids, in YES pricing.
    pub asks: Vec<Level>,
}

/// One change to one price level of a book, in YES pricing.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct OrderbookDelta {
    /// The market's ticker.
    pub market_ticker: String,
    /// The side of the ladder: a YES level is a bid, and a NO level, in YES pricing, an
    /// ask.
    pub side: BookSide,
    /// The level's price.
    pub price: Decimal,
    /// The change to the size resting there: negative when contracts left.
    pub delta: Decimal,
    /// When Kalshi recorded the change, when it said.
    pub ts: Option<UnixMillis>,
    /// The `client_order_id` of the account's own order that caused the change, when it
    /// did.
    pub client_order_id: Option<String>,
}

/// Why a connection went down, in a `Disconnected` event: the kit's `Outage`.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed the connection, with the code and reason it sent, or ended the
    /// stream with neither.
    Closed {
        /// The close code, when a close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A failure: a read, a ping, or a frame that recovery sent to a reconnect, such as a
    /// gap in a subscription's sequence.
    Error(PredictionsWsError),
    /// Nothing arrived for the staleness window, or a membership change went unanswered
    /// for it.
    Stale {
        /// The silence.
        silent: Duration,
    },
    /// Replaced on purpose at the connection's maximum age. This socket never rotates.
    Rotation,
}

impl From<Outage<PredictionsWsError>> for DisconnectReason {
    fn from(outage: Outage<PredictionsWsError>) -> Self {
        match outage {
            Outage::Closed { code, reason } => Self::Closed { code, reason },
            Outage::Error(err) => Self::Error(err),
            Outage::Stale { silent } => Self::Stale { silent },
            Outage::Rotation => Self::Rotation,
            // An outage a later kit adds reads as a stall of no length.
            _ => Self::Stale {
                silent: Duration::ZERO,
            },
        }
    }
}

fn levels(wire: Vec<PriceLevel>) -> Vec<Level> {
    wire.into_iter()
        .map(|level| Level::new(level.price, level.count))
        .collect()
}

impl From<frames::SnapshotMsg> for OrderbookSnapshot {
    fn from(msg: frames::SnapshotMsg) -> Self {
        let mut bids = levels(msg.yes_dollars_fp);
        bids.sort_by_key(|level| std::cmp::Reverse(level.price));
        let mut asks = levels(msg.no_dollars_fp);
        asks.sort_by_key(|level| level.price);
        Self {
            market_ticker: msg.market_ticker,
            market_id: msg.market_id,
            bids,
            asks,
        }
    }
}

impl From<frames::DeltaMsg> for OrderbookDelta {
    fn from(msg: frames::DeltaMsg) -> Self {
        let side = match msg.side {
            frames::WireSide::Yes => BookSide::Bid,
            frames::WireSide::No => BookSide::Ask,
        };
        Self {
            market_ticker: msg.market_ticker,
            side,
            price: msg.price_dollars,
            delta: msg.delta_fp,
            ts: msg.ts_ms.map(UnixMillis),
            client_order_id: msg.client_order_id,
        }
    }
}

/// The frames Kalshi sends, as the wire writes them, for the protocol's decode and for
/// the wire-agreement and live suites. Not API.
///
/// Each type is the AsyncAPI's schema (version 2.0.0) read against the captured frames in
/// `tests/fixtures/predictions/ws/`. Every optional key is an `Option` that is not written
/// when absent, so a frame round-trips with exactly the keys it came with, and a key the
/// types do not model shows up as unmodelled.
#[doc(hidden)]
pub mod frames {
    use rust_decimal::Decimal;
    use serde::{Deserialize, Serialize};

    use crate::predictions::types::PriceLevel;

    /// `subscribed`: the answer to a `subscribe`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Subscribed {
        /// Always `subscribed`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the `subscribe` it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The channel and the new subscription's id.
        pub msg: SubscribedMsg,
    }

    /// The body of `subscribed`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct SubscribedMsg {
        /// The channel subscribed.
        pub channel: String,
        /// The subscription's id.
        pub sid: u64,
    }

    /// `ok`: the answer to an `update_subscription`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Updated {
        /// Always `ok`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the command it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sid: Option<u64>,
        /// Its sequence number.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub seq: Option<u64>,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The markets the subscription holds after the update.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub msg: Option<UpdatedMsg>,
    }

    /// The body of `ok`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct UpdatedMsg {
        /// The tickers after the update.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub market_tickers: Option<Vec<String>>,
        /// The market ids after the update.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub market_ids: Option<Vec<String>>,
    }

    /// `unsubscribed`: the answer to an `unsubscribe`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Unsubscribed {
        /// Always `unsubscribed`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the `unsubscribe` it answers.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription that ended.
        pub sid: u64,
        /// Its last sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
    }

    /// `error`: a command Kalshi refused, or a problem on a subscription.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Rejected {
        /// Always `error`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The id of the command it answers; none for a problem on a subscription.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription, when the error is scoped to one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sid: Option<u64>,
        /// Its sequence number, on a scoped error.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub seq: Option<u64>,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The code and message.
        pub msg: RejectedMsg,
    }

    /// The body of `error`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct RejectedMsg {
        /// Kalshi's error code.
        pub code: u32,
        /// Kalshi's message.
        pub msg: String,
    }

    /// `orderbook_snapshot`: a market's whole book.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Snapshot {
        /// Always `orderbook_snapshot`.
        #[serde(rename = "type")]
        pub kind: String,
        /// The command it answers, for a snapshot that was asked for.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription.
        pub sid: u64,
        /// Its sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The book.
        pub msg: SnapshotMsg,
    }

    /// The body of `orderbook_snapshot`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct SnapshotMsg {
        /// The market's ticker.
        pub market_ticker: String,
        /// The market's id.
        pub market_id: String,
        /// The YES levels; the key is absent when the side has no offers.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub yes_dollars_fp: Vec<PriceLevel>,
        /// The NO levels, in YES pricing under `use_yes_price`; absent when the side has
        /// no offers.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub no_dollars_fp: Vec<PriceLevel>,
    }

    /// `orderbook_delta`: one change to one price level.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Delta {
        /// Always `orderbook_delta`.
        #[serde(rename = "type")]
        pub kind: String,
        /// Not documented for a delta; kept if Kalshi sends one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<u64>,
        /// The subscription.
        pub sid: u64,
        /// Its sequence number.
        pub seq: u64,
        /// When Kalshi queued the frame, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sending_ts_ms: Option<u64>,
        /// The change.
        pub msg: DeltaMsg,
    }

    /// The side of a delta as the wire spells it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum WireSide {
        /// A YES level: a bid.
        Yes,
        /// A NO level: an ask, when `use_yes_price` puts it in YES pricing.
        No,
    }

    /// The body of `orderbook_delta`.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct DeltaMsg {
        /// The market's ticker.
        pub market_ticker: String,
        /// The market's id.
        pub market_id: String,
        /// The level's price.
        #[serde(with = "rust_decimal::serde::str")]
        pub price_dollars: Decimal,
        /// The change to the size resting there, signed.
        #[serde(with = "rust_decimal::serde::str")]
        pub delta_fp: Decimal,
        /// The side.
        pub side: WireSide,
        /// The account's own order that caused the change, when it did.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_order_id: Option<String>,
        /// Deprecated: the time as RFC 3339. `ts_ms` replaces it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ts: Option<String>,
        /// When Kalshi recorded the change, in milliseconds.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ts_ms: Option<u64>,
        /// The subaccount of the account's own order that caused the change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subaccount: Option<u64>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The mirror's own example (`docs/specs/kalshi/asyncapi.yaml`, version 2.0.0), as the
    // `msg` of a snapshot, in the order Kalshi lists a side: ascending price.
    const SNAPSHOT_MSG: &str = r#"{"market_ticker":"FED-23DEC-T3.00","market_id":"9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1","yes_dollars_fp":[["0.0800","300.00"],["0.2200","333.00"]],"no_dollars_fp":[["0.5400","20.00"],["0.5600","146.00"]]}"#;

    fn level(price: &str, size: &str) -> Level {
        Level::new(price.parse().unwrap(), size.parse().unwrap())
    }

    fn snapshot(msg: &str) -> OrderbookSnapshot {
        OrderbookSnapshot::from(serde_json::from_str::<frames::SnapshotMsg>(msg).unwrap())
    }

    fn delta(msg: &str) -> OrderbookDelta {
        OrderbookDelta::from(serde_json::from_str::<frames::DeltaMsg>(msg).unwrap())
    }

    #[test]
    fn a_snapshot_is_the_yes_book_best_first() {
        let book = snapshot(SNAPSHOT_MSG);
        assert_eq!(book.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(book.market_id, "9b0f6b43-5b68-4f9f-9f02-9a2d1b8ac1a1");
        // Bids highest first; asks, the NO side in YES pricing, lowest first.
        assert_eq!(
            book.bids,
            [level("0.2200", "333.00"), level("0.0800", "300.00")]
        );
        assert_eq!(
            book.asks,
            [level("0.5400", "20.00"), level("0.5600", "146.00")]
        );
    }

    #[test]
    fn a_book_sent_the_other_way_round_is_sorted_the_same() {
        let reversed = r#"{"market_ticker":"A-1","market_id":"id","yes_dollars_fp":[["0.2200","333.00"],["0.0800","300.00"]],"no_dollars_fp":[["0.5600","146.00"],["0.5400","20.00"]]}"#;
        let book = snapshot(reversed);
        assert_eq!(
            book.bids,
            [level("0.2200", "333.00"), level("0.0800", "300.00")]
        );
        assert_eq!(
            book.asks,
            [level("0.5400", "20.00"), level("0.5600", "146.00")]
        );
    }

    #[test]
    fn a_side_with_no_offers_is_an_empty_side() {
        // Kalshi leaves the key out when a side has no offers.
        let one_sided =
            r#"{"market_ticker":"A-1","market_id":"id","yes_dollars_fp":[["0.2200","333.00"]]}"#;
        let book = snapshot(one_sided);
        assert_eq!(book.bids, [level("0.2200", "333.00")]);
        assert!(book.asks.is_empty());
    }

    #[test]
    fn a_yes_delta_is_a_bid_and_a_no_delta_an_ask_in_yes_pricing() {
        // The mirror's example delta: a YES level losing 54 contracts at 0.96.
        let yes = r#"{"market_ticker":"FED-23DEC-T3.00","market_id":"id","price_dollars":"0.9600","delta_fp":"-54.00","side":"yes","ts":"2022-11-22T20:44:01Z","ts_ms":1669149841000}"#;
        let bid = delta(yes);
        assert_eq!(bid.market_ticker, "FED-23DEC-T3.00");
        assert_eq!(bid.side, BookSide::Bid);
        assert_eq!(bid.price, "0.9600".parse::<Decimal>().unwrap());
        assert_eq!(bid.delta, "-54.00".parse::<Decimal>().unwrap());
        assert_eq!(bid.ts, Some(UnixMillis(1_669_149_841_000)));
        // `use_yes_price` reports the NO side at its YES price, so no flip is applied.
        let no = r#"{"market_ticker":"A-1","market_id":"id","price_dollars":"0.4000","delta_fp":"12.00","side":"no"}"#;
        let ask = delta(no);
        assert_eq!(ask.side, BookSide::Ask);
        assert_eq!(ask.price, "0.4000".parse::<Decimal>().unwrap());
        assert_eq!(ask.delta, "12.00".parse::<Decimal>().unwrap());
        assert_eq!(ask.ts, None);
    }

    #[test]
    fn a_deltas_optional_fields_are_kept_when_present() {
        let ours = r#"{"market_ticker":"A-1","market_id":"id","price_dollars":"0.5000","delta_fp":"1.00","side":"yes","client_order_id":"mine","subaccount":2}"#;
        assert_eq!(delta(ours).client_order_id.as_deref(), Some("mine"));
    }

    #[test]
    fn an_unknown_side_does_not_decode() {
        let sideways = r#"{"market_ticker":"A-1","market_id":"id","price_dollars":"0.5000","delta_fp":"1.00","side":"maybe"}"#;
        assert!(serde_json::from_str::<frames::DeltaMsg>(sideways).is_err());
    }
}
