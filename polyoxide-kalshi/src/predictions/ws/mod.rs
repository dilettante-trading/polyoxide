//! The order-book socket of the predictions exchange.
//!
//! One connection to [`PRODUCTION_WS`] or [`DEMO_WS`] carries the `orderbook_delta`
//! channel: a snapshot of each subscribed market's book, then the changes to it, with
//! every price in YES pricing (the subscription asks for `use_yes_price`, so a NO bid is
//! reported as the YES ask it is). Kalshi refuses a handshake that carries no API-key
//! headers, public data included, so a socket is built from `Credentials`, and every
//! connection attempt signs its own handshake: a signature carries its timestamp, and a
//! reconnect that replayed an old one would be refused.
//!
//! The socket is run by the socket kit's `Supervisor`. It pings every 10 s, counts every
//! frame as liveness, reconnects after a drop, a stall or a gap in a subscription's
//! `seq`, and replays the whole membership as one `subscribe`. Every outage yields one
//! `Event::Disconnected` and one `Event::Reconnected`, and what the socket carried in
//! between is lost, so a consumer rebuilds its books from the fresh snapshots that
//! follow.
//!
//! Errors are `PredictionsWsError`, classified through the one socket table.

mod error;
mod event;

pub use error::PredictionsWsError;
#[doc(hidden)]
pub use event::frames;
pub use event::{DisconnectReason, Event, OrderbookDelta, OrderbookSnapshot};

/// The production WebSocket endpoint.
pub const PRODUCTION_WS: &str = "wss://external-api-ws.kalshi.com/trade-api/ws/v2";

/// The demo WebSocket endpoint. Keys do not cross environments: a demo socket takes the
/// demo key.
pub const DEMO_WS: &str = "wss://external-api-ws.demo.kalshi.co/trade-api/ws/v2";
