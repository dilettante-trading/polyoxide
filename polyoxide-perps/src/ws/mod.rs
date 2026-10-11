//! WebSocket streaming of the public Perps channels.
//!
//! One multiplexed connection to [`WS_URL`]. Requests are
//! `{ "id", "req": "sub" | "unsub" | "post", … }`; push frames are
//! `{ "ch", "ts", "ets", "sq", "data" }`. The server sends a protocol Ping
//! every 10 s and closes a connection that stops answering them
//! (`1011 pong_timeout`, after about 70 s); the pongs go out while the
//! connection is read, so a bare [`PerpsWs`] must be read steadily.
//! [`SupervisedPerpsWs`] reads it, sends the application ping on the wall
//! clock and also reconnects.
//! Everything the published AsyncAPI gets wrong about the wire is in
//! `docs/specs/perps/OBSERVED.md`.

mod builder;
pub mod channel;
pub mod client;
pub mod error;
pub mod event;
pub mod frame;
mod protocol;
#[cfg(test)]
mod scripted;

pub use builder::PerpsWsBuilder;
pub use channel::{Channel, StreamDepth};
pub use client::PerpsWs;
pub use error::{PerpsWsError, Refusal};
pub use event::{Event, Frame, Payload, Update};
pub use frame::{BboData, BookData, StatisticsData, TickerData, TradeData};
pub use protocol::{MembershipChange, PerpsMembership, PerpsProtocol};

/// A supervised connection: a `Stream` of [`Event`]s that survives drops and
/// stalls by reconnecting and replaying its subscriptions, run by the kit's
/// `Supervisor` on [`PerpsProtocol`]. Build one with [`PerpsWsBuilder`].
///
/// It keeps the application ping going on the wall clock, even while the
/// consumer is behind; the queue holds 1024 unread events, after which the
/// task reads nothing more until the consumer catches up, and that wait does
/// not count as silence. It ends after an error it cannot recover from,
/// which it yields first; a frame that did not decode is yielded as an error
/// and does not end it. Change its channels through
/// [`membership`](polyoxide_ws::Supervised::membership) and
/// [`PerpsMembership`], from a task other than the one reading the stream.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_perps::{types::InstrumentId, ws::{Channel, Event, PerpsMembership, PerpsWsBuilder}};
///
/// # async fn example() -> Result<(), polyoxide_perps::ws::PerpsWsError> {
/// let mut ws = PerpsWsBuilder::new().connect([Channel::Book(InstrumentId(1), Default::default())]).await?;
/// let membership = ws.membership();
/// while let Some(event) = ws.next().await {
///     match event? {
///         Event::Update(update) => println!("{} sq={}", update.channel, update.sq),
///         Event::Reconnected | Event::SequenceRegressed { .. } => println!("resync the book"),
///         _ => {}
///     }
/// }
/// # membership.subscribe([Channel::Bbo(InstrumentId(2))]).await?;
/// # Ok(())
/// # }
/// ```
pub type SupervisedPerpsWs = polyoxide_ws::Supervised<PerpsProtocol>;

/// The production WebSocket endpoint.
pub const WS_URL: &str = "wss://ws.perpetuals.polymarket.com/v1/ws";

/// Parse one text frame as the stream would. For the crate's own
/// integration tests; not API.
#[doc(hidden)]
pub fn frame_from_text_for_tests(text: &str) -> Result<Frame, PerpsWsError> {
    match frame::Incoming::parse(text) {
        Ok(frame::Incoming::Push(push)) => Frame::from_push(push, text),
        Ok(frame::Incoming::Response(_)) | Err(_) => Err(PerpsWsError::Unrecognised {
            raw: text.to_owned(),
        }),
    }
}

/// What [`incoming_from_text_for_tests`] decodes a text frame into. For the
/// crate's own integration tests; not API.
#[doc(hidden)]
#[derive(Debug)]
pub enum IncomingForTests {
    /// A push frame, decoded as the stream would.
    Push(Frame),
    /// A `sub`/`unsub` response: `(accepted, error)` per requested channel.
    Statuses(Vec<(bool, Option<String>)>),
    /// A ping response.
    Pong {
        /// Whether the server answered `ok`.
        ok: bool,
        /// The sequence stamp it carried.
        sq: Option<u64>,
    },
}

/// Parse one text frame through the private envelopes, including the
/// response decoders the control methods use. For the crate's own
/// integration tests; not API.
#[doc(hidden)]
pub fn incoming_from_text_for_tests(text: &str) -> Result<IncomingForTests, PerpsWsError> {
    let incoming = frame::Incoming::parse(text).map_err(|_| PerpsWsError::Unrecognised {
        raw: text.to_owned(),
    })?;
    match incoming {
        frame::Incoming::Push(push) => Frame::from_push(push, text).map(IncomingForTests::Push),
        frame::Incoming::Response(response) => {
            let malformed = || PerpsWsError::Response {
                id: response.id.unwrap_or_default(),
                raw: response.data.to_string(),
            };
            if response.data.is_array() {
                let statuses = response.statuses().map_err(|_| malformed())?;
                Ok(IncomingForTests::Statuses(
                    statuses.into_iter().map(|s| (s.is_ok(), s.error)).collect(),
                ))
            } else {
                let pong = response.pong().map_err(|_| malformed())?;
                Ok(IncomingForTests::Pong {
                    ok: pong.is_ok(),
                    sq: pong.sq,
                })
            }
        }
    }
}
