//! Binance's socket plumbing, shared by every product's `ws` module.
//!
//! What `usdm::ws` has that is not USDⓈ-M's: the request correlation, the
//! bare connection, the kit `Protocol` with Binance's recovery rows, the
//! router with its forwarding queue, and the stop. It is generic over a
//! product's [`StreamSet`] (its stream names, paths, frame decode and limits)
//! and [`SocketError`] (the constructors it needs of the product's error, and a
//! [`Shape`] view of its variants), so a second product's streams are a second
//! marker type and not a second copy.

// The test-only `scripted` below is a second `mod` item over `usdm::ws`'s scripted
// server, which is private to `usdm::ws` and not copied. The allow sits here, not on
// the item, because the test-bodies gate reads an item's attributes as the gate of
// the file it declares.
#![allow(clippy::duplicate_mod)]

use std::{fmt, time::Duration};

pub(crate) mod bare;
pub(crate) mod protocol;
pub(crate) mod request;
pub(crate) mod supervised;

/// Streams one connection may carry. Both products' hosts close a connection
/// at its 1025th stream.
pub(crate) const MAX_STREAMS_PER_CONNECTION: usize = 1024;

/// Ten minutes under Binance's documented 24-hour connection lifetime.
pub(crate) const DEFAULT_MAX_CONNECTION_AGE: Duration = Duration::from_secs(23 * 3600 + 50 * 60);

/// Events the consumer may leave unread, across every path, before the paths
/// stop reading.
pub(crate) const EVENT_BUFFER: usize = 1024;

/// What one product says about its streams: the types it names them and their
/// frames with, and the limits its host enforces.
///
/// A product implements it on a marker type. The shared code is generic over
/// that marker, so nothing in it names a product.
pub(crate) trait StreamSet: Sized + Send + Sync + 'static {
    /// A stream's name, the only way to name one.
    type Name: Clone + Eq + std::hash::Hash + fmt::Display + fmt::Debug + Send + Sync + 'static;
    /// Which routed path a connection is on.
    type Path: Copy + Eq + std::hash::Hash + fmt::Display + fmt::Debug + Send + Sync + 'static;
    /// One decoded frame.
    type Update: Send + 'static;
    /// The product's socket error.
    type Error: SocketError<Path = Self::Path>;
    /// What the supervised stream yields.
    type Event: Send + 'static;

    /// Every path, in the order a first connect opens them.
    const PATHS: &'static [Self::Path];
    /// Stream names in one `SUBSCRIBE` or `UNSUBSCRIBE`.
    const MAX_NAMES_PER_REQUEST: usize;
    /// The shortest gap between two requests on one connection.
    const MIN_REQUEST_INTERVAL: Duration;

    /// The path a stream rides.
    fn path(name: &Self::Name) -> Self::Path;
    /// The combined-stream URL of `path` on `base`.
    fn url(base: &str, path: Self::Path) -> String;
    /// One text frame as an update: the product's `Update::from_json`.
    fn decode(text: &str) -> Result<Self::Update, Self::Error>;
    /// `Event::Update(Box::new(update))`.
    fn update(update: Self::Update) -> Self::Event;
    /// `Event::Disconnected { path, reason }`, the reason read from the kit's outage.
    fn disconnected(path: Self::Path, outage: polyoxide_ws::Outage<Self::Error>) -> Self::Event;
    /// `Event::Reconnected { path }`.
    fn reconnected(path: Self::Path) -> Self::Event;
}

/// A product's socket error, as the shared code reads it: which variant it is,
/// without the variant's fields, so recovery and the router match on a view and
/// not on a product's enum.
pub(crate) enum Shape<'a> {
    /// The kit's transport error, which recovery reads through its `Kind`.
    Connect(&'a polyoxide_ws::WsError),
    /// The connection did not open in time.
    ConnectTimeout,
    /// The server closed the connection.
    Closed,
    /// The server answered a request with an error.
    Refused,
    /// No answer to a request.
    NoAnswer,
    /// An answer that was not the documented shape.
    Response,
    /// A subscribe past the connection's cap, refused before sending.
    TooManyStreams,
    /// A stream for the other path, refused before sending.
    WrongPath,
    /// A frame that did not decode.
    Frame,
    /// The supervised tier has stopped.
    Stopped,
}

/// What the shared code needs of a product's error: constructors for the errors
/// it makes itself, and a [`Shape`] of the variant.
pub(crate) trait SocketError:
    From<polyoxide_ws::WsError>
    + polyoxide_venue::Classify
    + fmt::Display
    + fmt::Debug
    + Send
    + Sync
    + 'static
{
    /// The product's path type.
    type Path;
    /// The product's `StopCause`.
    type Cause: Clone + fmt::Debug + Send + Sync + 'static;

    /// The server answered a request with `{"error": {"code", "msg"}}`.
    fn refused(code: i64, msg: String) -> Self;
    /// No answer to request `id` within `timeout`; id 0 is a ping.
    fn no_answer(id: u64, timeout: Duration) -> Self;
    /// An answer to request `id` that was not the documented shape.
    fn response(id: u64, raw: String) -> Self;
    /// A subscribe that would take `path`'s connection past `limit` streams.
    fn too_many_streams(path: Self::Path, limit: usize) -> Self;
    /// The supervised tier has stopped, on `cause` when it is known.
    fn stopped(cause: Option<Self::Cause>) -> Self;
    /// `StopCause::of(self)`.
    fn cause(&self) -> Self::Cause;
    /// Which variant this is.
    fn shape(&self) -> Shape<'_>;
}

#[cfg(test)]
#[path = "../../usdm/ws/scripted.rs"]
mod scripted;

#[cfg(test)]
mod tests {
    use polyoxide_venue::{Class, Classify};
    use polyoxide_ws::{Outage, WsError};
    use tokio::time::Instant;

    use super::{
        bare::Bare,
        scripted::{Script, ScriptedServer},
        *,
    };

    /// A stream set that is nothing like USDⓈ-M's: one path, three names a
    /// request, a request every 250 ms, slower than USDⓈ-M's 200 ms so a test can
    /// tell the two apart.
    struct Probe;

    #[derive(Debug, thiserror::Error)]
    enum ProbeError {
        #[error("transport: {0}")]
        Connect(#[source] WsError),
        #[error("refused: {code} {msg}")]
        Refused { code: i64, msg: String },
        #[error("no answer to {id} within {timeout:?}")]
        NoAnswer { id: u64, timeout: Duration },
        #[error("malformed answer to {id}: {raw}")]
        Response { id: u64, raw: String },
        #[error("{path} carries at most {limit}")]
        TooMany { path: &'static str, limit: usize },
        #[error("stopped")]
        Stopped(Option<String>),
    }

    impl From<WsError> for ProbeError {
        fn from(err: WsError) -> Self {
            Self::Connect(err)
        }
    }

    impl Classify for ProbeError {
        fn class(&self) -> Class {
            Class::Decode
        }
    }

    impl SocketError for ProbeError {
        type Path = &'static str;
        type Cause = String;

        fn refused(code: i64, msg: String) -> Self {
            Self::Refused { code, msg }
        }

        fn no_answer(id: u64, timeout: Duration) -> Self {
            Self::NoAnswer { id, timeout }
        }

        fn response(id: u64, raw: String) -> Self {
            Self::Response { id, raw }
        }

        fn too_many_streams(path: &'static str, limit: usize) -> Self {
            Self::TooMany { path, limit }
        }

        fn stopped(cause: Option<String>) -> Self {
            Self::Stopped(cause)
        }

        fn cause(&self) -> String {
            match self {
                Self::Stopped(cause) => cause.clone().unwrap_or_default(),
                other => other.to_string(),
            }
        }

        fn shape(&self) -> Shape<'_> {
            match self {
                Self::Connect(err) => Shape::Connect(err),
                Self::Refused { .. } => Shape::Refused,
                Self::NoAnswer { .. } => Shape::NoAnswer,
                Self::Response { .. } => Shape::Response,
                Self::TooMany { .. } => Shape::TooManyStreams,
                Self::Stopped(_) => Shape::Stopped,
            }
        }
    }

    impl StreamSet for Probe {
        type Name = String;
        type Path = &'static str;
        type Update = String;
        type Error = ProbeError;
        type Event = String;

        const PATHS: &'static [&'static str] = &["probe"];
        const MAX_NAMES_PER_REQUEST: usize = 3;
        const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(250);

        fn path(_name: &String) -> &'static str {
            "probe"
        }

        fn url(base: &str, path: &'static str) -> String {
            format!("{}/{path}", base.trim_end_matches('/'))
        }

        fn decode(text: &str) -> Result<String, ProbeError> {
            Ok(text.to_owned())
        }

        fn update(update: String) -> String {
            update
        }

        fn disconnected(path: &'static str, outage: Outage<ProbeError>) -> String {
            let _ = outage;
            format!("{path} down")
        }

        fn reconnected(path: &'static str) -> String {
            format!("{path} back")
        }
    }

    fn names(count: usize) -> Vec<String> {
        (0..count).map(|i| format!("n{i}@probe")).collect()
    }

    fn sizes(server: &ScriptedServer) -> Vec<usize> {
        server
            .requests()
            .iter()
            .map(|request| request["params"].as_array().unwrap().len())
            .collect()
    }

    #[tokio::test]
    async fn requests_follow_the_stream_sets_batch_size_and_pace() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = Bare::<Probe>::open(
            &server.url,
            "probe",
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        let wanted = names(7);
        let start = Instant::now();
        ws.subscribe(&wanted).await.unwrap();
        // Three names a request, not USDⓈ-M's 200, and each request at least
        // the set's interval after the last, not USDⓈ-M's 200 ms (three requests
        // at 200 ms would finish in 400 ms, under the 500 ms asserted here).
        assert_eq!(sizes(&server), [3, 3, 1]);
        assert!(
            start.elapsed() >= Probe::MIN_REQUEST_INTERVAL * 2,
            "three requests in {:?}",
            start.elapsed()
        );
        assert_eq!(server.paths(), ["/probe"]);
        assert_eq!(ws.streams(), wanted.as_slice());
    }

    #[tokio::test]
    async fn unsubscribe_batches_follow_the_stream_set_too() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = Bare::<Probe>::open(
            &server.url,
            "probe",
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        let wanted = names(4);
        ws.subscribe(&wanted).await.unwrap();
        ws.unsubscribe(&wanted).await.unwrap();
        assert_eq!(sizes(&server), [3, 1, 3, 1]);
        assert!(ws.streams().is_empty());
        assert_eq!(ws.path(), "probe");
    }
}
