//! Spot's side of the shared socket plumbing (`shared::ws`): the stream set
//! that the generic connection, protocol and router are written over, and the
//! error's view they match on. Spot has one path, so the error never has a
//! wrong-path shape.

use std::time::Duration;

use polyoxide_ws::Outage;

use crate::{
    shared::ws::{Shape, SocketError, StreamSet},
    spot::ws::{
        error::{SpotWsError, StopCause},
        event::Update,
        stream::StreamName,
        supervised::{DisconnectReason, Event},
        StreamPath, MAX_NAMES_PER_REQUEST, MIN_REQUEST_INTERVAL,
    },
};

/// Spot's stream set: one path, `/stream`, up to 1024 names in a request and
/// one request per 500 ms.
pub(crate) struct SpotStreams;

/// Binance's kit `Protocol` on spot's one path. Only the unit tests name it:
/// the shared `connect` builds the protocol for the stream set.
#[cfg(test)]
pub(crate) type SpotProtocol = crate::shared::ws::protocol::BinanceProtocol<SpotStreams>;

impl StreamSet for SpotStreams {
    type Name = StreamName;
    type Path = StreamPath;
    type Update = Update;
    type Error = SpotWsError;
    type Event = Event;

    const PATHS: &'static [StreamPath] = StreamPath::ALL;
    const MAX_NAMES_PER_REQUEST: usize = MAX_NAMES_PER_REQUEST;
    const MIN_REQUEST_INTERVAL: Duration = MIN_REQUEST_INTERVAL;

    fn path(name: &StreamName) -> StreamPath {
        name.path()
    }

    fn url(base: &str, path: StreamPath) -> String {
        path.url(base)
    }

    fn decode(text: &str) -> Result<Update, SpotWsError> {
        Update::from_json(text)
    }

    fn update(update: Update) -> Event {
        Event::Update(Box::new(update))
    }

    fn disconnected(path: StreamPath, outage: Outage<SpotWsError>) -> Event {
        let reason = match outage {
            Outage::Closed { code, reason } => DisconnectReason::Closed { code, reason },
            Outage::Error(err) => DisconnectReason::Error(err),
            Outage::Rotation => DisconnectReason::Rotation,
            // `Stale`, and any outage a later kit adds.
            _ => DisconnectReason::Stale,
        };
        Event::Disconnected { path, reason }
    }

    fn reconnected(path: StreamPath) -> Event {
        Event::Reconnected { path }
    }
}

impl SocketError for SpotWsError {
    type Path = StreamPath;
    type Cause = StopCause;

    fn refused(code: i64, msg: String) -> Self {
        Self::Refused { code, msg }
    }

    fn no_answer(id: u64, timeout: Duration) -> Self {
        Self::NoAnswer { id, timeout }
    }

    fn response(id: u64, raw: String) -> Self {
        Self::Response { id, raw }
    }

    fn too_many_streams(path: StreamPath, limit: usize) -> Self {
        Self::TooManyStreams { path, limit }
    }

    fn stopped(cause: Option<StopCause>) -> Self {
        Self::Stopped { cause }
    }

    fn cause(&self) -> StopCause {
        StopCause::of(self)
    }

    fn shape(&self) -> Shape<'_> {
        match self {
            Self::Connect(err) => Shape::Connect(err),
            Self::ConnectTimeout(_) => Shape::ConnectTimeout,
            Self::Closed { .. } => Shape::Closed,
            Self::Refused { .. } => Shape::Refused,
            Self::NoAnswer { .. } => Shape::NoAnswer,
            Self::Response { .. } => Shape::Response,
            Self::TooManyStreams { .. } => Shape::TooManyStreams,
            Self::Frame { .. } => Shape::Frame,
            Self::Stopped { .. } => Shape::Stopped,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use polyoxide_venue::Classify;
    use polyoxide_ws::{tungstenite, Outage, Protocol, WsError};

    use super::*;
    use crate::spot::{types::Symbol, ws::fixtures};

    fn btc() -> Symbol {
        Symbol::new("BTCUSDT").unwrap()
    }

    #[test]
    fn the_stream_set_is_one_path_with_the_measured_limits() {
        assert_eq!(<SpotStreams as StreamSet>::PATHS, [StreamPath::Stream]);
        assert_eq!(<SpotStreams as StreamSet>::MAX_NAMES_PER_REQUEST, 1024);
        assert_eq!(
            <SpotStreams as StreamSet>::MIN_REQUEST_INTERVAL,
            Duration::from_millis(500)
        );
    }

    #[test]
    fn every_name_is_on_the_one_path() {
        for name in [
            StreamName::AggTrade(btc()),
            StreamName::AllMiniTickers,
            StreamName::BookTicker(btc()),
        ] {
            assert_eq!(SpotStreams::path(&name), StreamPath::Stream, "{name}");
        }
    }

    #[test]
    fn the_url_appends_the_one_path_to_the_base() {
        assert_eq!(
            SpotStreams::url("ws://127.0.0.1:9", StreamPath::Stream),
            "ws://127.0.0.1:9/stream"
        );
        assert_eq!(
            SpotStreams::url("ws://127.0.0.1:9/", StreamPath::Stream),
            "ws://127.0.0.1:9/stream"
        );
    }

    #[test]
    fn the_protocol_dials_the_stream_url() {
        let mut protocol = SpotProtocol::new("ws://127.0.0.1:9".to_owned(), StreamPath::Stream);
        let handshake = protocol.handshake().unwrap();
        assert_eq!(handshake.uri().to_string(), "ws://127.0.0.1:9/stream");
    }

    #[test]
    fn a_frame_decodes_as_an_update_and_a_bad_one_is_a_frame_error() {
        let update = SpotStreams::decode(fixtures::AGG_TRADE).unwrap();
        assert_eq!(update.stream.to_string(), "btcusdt@aggTrade");
        let err = SpotStreams::decode("not json").unwrap_err();
        assert!(matches!(err, SpotWsError::Frame { .. }), "{err:?}");
    }

    #[test]
    fn an_update_is_boxed_into_the_event() {
        let update = SpotStreams::decode(fixtures::AGG_TRADE).unwrap();
        let Event::Update(boxed) = SpotStreams::update(update) else {
            panic!("expected Event::Update");
        };
        assert_eq!(boxed.stream.to_string(), "btcusdt@aggTrade");
    }

    #[test]
    fn each_outage_reads_as_its_disconnect_reason() {
        let reason_of = |outage: Outage<SpotWsError>| {
            let Event::Disconnected { path, reason } =
                SpotStreams::disconnected(StreamPath::Stream, outage)
            else {
                panic!("expected Event::Disconnected");
            };
            assert_eq!(path, StreamPath::Stream);
            reason
        };
        assert!(matches!(
            reason_of(Outage::Closed {
                code: Some(1008),
                reason: "Too many requests".to_owned()
            }),
            DisconnectReason::Closed { code: Some(1008), reason } if reason == "Too many requests"
        ));
        assert!(matches!(
            reason_of(Outage::Error(SpotWsError::Refused {
                code: 2,
                msg: "Invalid request".to_owned()
            })),
            DisconnectReason::Error(SpotWsError::Refused { code: 2, .. })
        ));
        assert!(matches!(
            reason_of(Outage::Stale {
                silent: Duration::from_secs(30)
            }),
            DisconnectReason::Stale
        ));
        assert!(matches!(
            reason_of(Outage::Rotation),
            DisconnectReason::Rotation
        ));
    }

    #[test]
    fn a_return_names_its_path() {
        assert!(matches!(
            SpotStreams::reconnected(StreamPath::Stream),
            Event::Reconnected {
                path: StreamPath::Stream
            }
        ));
    }

    #[test]
    fn the_error_constructors_build_their_variants_and_the_shape_reads_them_back() {
        let connect = SpotWsError::from(WsError::from(tungstenite::Error::AlreadyClosed));
        assert!(matches!(connect.shape(), Shape::Connect(_)), "{connect:?}");
        assert!(matches!(
            SpotWsError::ConnectTimeout(Duration::from_secs(10)).shape(),
            Shape::ConnectTimeout
        ));
        assert!(matches!(
            SpotWsError::Closed {
                code: Some(1008),
                reason: String::new()
            }
            .shape(),
            Shape::Closed
        ));
        let refused = SpotWsError::refused(2, "Invalid request".to_owned());
        assert!(matches!(refused, SpotWsError::Refused { code: 2, .. }));
        assert!(matches!(refused.shape(), Shape::Refused));
        let no_answer = SpotWsError::no_answer(7, Duration::from_secs(10));
        assert!(matches!(no_answer, SpotWsError::NoAnswer { id: 7, .. }));
        assert!(matches!(no_answer.shape(), Shape::NoAnswer));
        let response = SpotWsError::response(8, "[]".to_owned());
        assert!(matches!(response, SpotWsError::Response { id: 8, .. }));
        assert!(matches!(response.shape(), Shape::Response));
        let many = SpotWsError::too_many_streams(StreamPath::Stream, 1024);
        assert!(matches!(
            many,
            SpotWsError::TooManyStreams {
                path: StreamPath::Stream,
                limit: 1024
            }
        ));
        assert!(matches!(many.shape(), Shape::TooManyStreams));
        let frame = SpotWsError::Frame {
            stream: String::new(),
            raw: String::new(),
            reason: String::new(),
        };
        assert!(matches!(frame.shape(), Shape::Frame));
    }

    #[test]
    fn a_stop_carries_the_cause_it_was_given_and_the_cause_reads_the_error() {
        let refused = SpotWsError::refused(2, "Invalid request".to_owned());
        let cause = <SpotWsError as SocketError>::cause(&refused);
        assert_eq!(cause, StopCause::of(&refused));
        assert_eq!(cause.message, refused.to_string());
        let stopped = SpotWsError::stopped(Some(cause.clone()));
        assert!(matches!(stopped.shape(), Shape::Stopped));
        assert_eq!(stopped.class(), refused.class());
        assert!(matches!(
            SpotWsError::stopped(None),
            SpotWsError::Stopped { cause: None }
        ));
    }
}
