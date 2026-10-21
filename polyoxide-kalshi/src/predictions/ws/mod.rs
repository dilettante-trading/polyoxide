//! The socket of the predictions exchange.
//!
//! One connection to [`PRODUCTION_WS`] or [`DEMO_WS`] carries the channels chosen with
//! [`PredictionsWsBuilder::channels`]: `orderbook_delta`, the default, a snapshot of each
//! subscribed market's book and then the changes to it, with every price in YES pricing (the
//! subscription asks for `use_yes_price`, so a NO bid is reported as the YES ask it is);
//! `ticker`, a market's last price, best bid and ask, volume and open interest; and `trade`,
//! each public trade. One set of tickers serves every channel, and Kalshi gives each channel
//! its own subscription, whose id the protocol keeps. Kalshi refuses a handshake that carries
//! no API-key headers, public data included, so a socket is built from `Credentials`, and every
//! connection attempt signs its own handshake: a signature carries its timestamp, and a
//! reconnect that replayed an old one would be refused.
//!
//! The socket is run by the socket kit's `Supervisor`. It pings every 10 s, counts every
//! frame as liveness, reconnects after a drop or a stall, and replays the whole membership as
//! one `subscribe` naming every channel. Every outage yields one `Event::Disconnected` and
//! one `Event::Reconnected`, and what the socket carried in between is lost, so a consumer
//! rebuilds its books from the fresh snapshots that follow. If Kalshi refuses that
//! `subscribe` with a code that does not reconnect (a market it will not serve, say), or
//! answers it with a frame that does not decode, the stream yields the error and ends, for
//! every market: a change made during an outage was answered `Ok`, so this is where its
//! caller learns of the refusal.
//!
//! Kalshi refuses a `ticker` or `trade` subscribe with no markets (code 14 on the demo host,
//! 2026-10-21); the protocol never sends one, so an empty ticker set is no subscription on any
//! channel: emptying the set unsubscribes every channel, and the first market added afterwards
//! subscribes afresh.
//!
//! A membership change goes to Kalshi as one command for each channel's subscription, in
//! channel order. Kalshi refusing a command fails the change with Kalshi's error and undoes it
//! in the membership. When the refusal follows a command of the same change that Kalshi
//! accepted, its subscriptions no longer match the membership, so the connection is replaced as
//! well: the stream yields `Event::Disconnected` with that error as the reason, and
//! `Event::Reconnected` once the replay has brought the subscriptions back to the membership
//! (an error of code 9 stops the stream instead, as it does wherever it arrives). A refusal of
//! the first command leaves them as they were, and the connection stays.
//!
//! A gap in the order book's `seq` is repaired on the same connection. The stream yields
//! `Event::Resyncing` for the whole membership, the socket asks Kalshi for a snapshot of every
//! market with `get_snapshot`, withholds each market's deltas until its snapshot has arrived,
//! and carries on from the snapshots, with no `Disconnected`. It reconnects instead if the
//! snapshots do not come within the staleness window, if Kalshi answers the request with an
//! error, or if a second gap comes while the first is being repaired. A gap on the `trade`
//! subscription, which has no snapshot, reconnects. `PredictionsMembership::snapshot` asks for
//! snapshots of subscribed markets by the same request.
//!
//! Errors are `PredictionsWsError`, classified through the one socket table.

pub mod book;
mod builder;
mod error;
mod event;
mod protocol;
#[cfg(test)]
mod scripted;

pub use book::{Books, OrderBook};
pub use builder::PredictionsWsBuilder;
pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{
    DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot, PublicTrade, TickerUpdate,
};
#[doc(hidden)]
pub use protocol::signed_request;
pub use protocol::{Change, Channel, PredictionsMembership, PredictionsProtocol};

/// A supervised connection: a `Stream` of [`Event`]s that survives drops and stalls by
/// reconnecting and replaying its subscription, and repairs a gap in the order book's
/// sequence in place, run by the kit's `Supervisor` on [`PredictionsProtocol`]. Build one
/// with [`PredictionsWsBuilder`].
///
/// It pings every 10 s on the wall clock, even while the consumer is behind; the queue
/// holds 1024 unread events, after which the task reads nothing more until the consumer
/// catches up, and that wait does not count as silence. It ends after an error it cannot
/// recover from (a handshake Kalshi refuses with a 401 or 403, an error frame with code 9,
/// or a replayed `subscribe` Kalshi refuses with a code that does not reconnect or answers
/// with a frame that does not decode), which it yields first; a frame that did not decode
/// anywhere else is yielded as an error and does not end it.
/// Change its markets through `membership()` and [`PredictionsMembership`], from a task
/// other than the one reading the stream.
pub type SupervisedPredictionsWs = polyoxide_ws::Supervised<PredictionsProtocol>;

/// The production WebSocket endpoint.
pub const PRODUCTION_WS: &str = "wss://external-api-ws.kalshi.com/trade-api/ws/v2";

/// The demo WebSocket endpoint. Keys do not cross environments: a demo socket takes the
/// demo key.
pub const DEMO_WS: &str = "wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2";
