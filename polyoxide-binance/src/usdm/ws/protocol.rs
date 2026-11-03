//! USDⓈ-M's stream set for Binance's socket protocol, which lives in
//! `shared::ws` and is generic over it. The supervised tier runs one
//! supervisor per path.

use std::time::Duration;

use polyoxide_ws::Outage;
#[cfg(test)]
use polyoxide_ws::{tungstenite::Message, Decoded, Offline, Protocol, Recovery, WsError};

use crate::{
    shared::ws::StreamSet,
    usdm::ws::{
        error::UsdmWsError,
        event::Update,
        stream::StreamName,
        supervised::{DisconnectReason, Event},
        StreamPath, MAX_NAMES_PER_REQUEST, MIN_REQUEST_INTERVAL,
    },
};

/// USDⓈ-M's streams: two paths, `/market` and `/public`, 200 names a request
/// and one request per 200 ms.
pub(crate) struct UsdmStreams;

impl StreamSet for UsdmStreams {
    type Name = StreamName;
    type Path = StreamPath;
    type Update = Update;
    type Error = UsdmWsError;
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

    fn decode(text: &str) -> Result<Update, UsdmWsError> {
        Update::from_json(text)
    }

    fn update(update: Update) -> Event {
        Event::Update(Box::new(update))
    }

    fn disconnected(path: StreamPath, outage: Outage<UsdmWsError>) -> Event {
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

/// Binance's protocol on one routed path of USDⓈ-M's streams. Only the unit
/// tests name it.
#[cfg(test)]
pub(crate) type UsdmProtocol = crate::shared::ws::protocol::BinanceProtocol<UsdmStreams>;

/// One change to a path's streams. Only the unit tests name it.
#[cfg(test)]
pub(crate) type Change = crate::shared::ws::protocol::Change<StreamName>;

#[cfg(test)]
mod tests {
    use polyoxide_ws::tungstenite::{self, http};

    use super::*;
    use crate::usdm::{types::Symbol, ws::USDM_WS_BASE};

    /// What the supervisor does about `err`, by Binance's recovery hook.
    fn recovery(err: &UsdmWsError) -> Recovery {
        UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market).recover(err)
    }

    fn http_error(status: u16) -> UsdmWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> UsdmWsError {
        UsdmWsError::from(WsError::from(err))
    }

    fn agg(symbol: &str) -> StreamName {
        StreamName::AggTrade(Symbol::new(symbol).unwrap())
    }

    #[test]
    fn a_handshake_refused_by_the_host_reconnects_and_one_refused_for_us_is_fatal() {
        assert_eq!(recovery(&http_error(503)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(429)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(408)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(425)), Recovery::Reconnect);
        assert_eq!(recovery(&http_error(451)), Recovery::Stop);
        assert_eq!(recovery(&http_error(404)), Recovery::Stop);
        assert_eq!(recovery(&http_error(418)), Recovery::Stop);
    }

    #[test]
    fn losing_the_connection_reconnects_and_a_bad_frame_is_skipped() {
        for err in [
            transport(tungstenite::Error::ConnectionClosed),
            UsdmWsError::ConnectTimeout(Duration::from_secs(10)),
            UsdmWsError::Closed {
                code: Some(1008),
                reason: "Too many requests".into(),
            },
            UsdmWsError::NoAnswer {
                id: 3,
                timeout: Duration::from_secs(10),
            },
            UsdmWsError::Response {
                id: 4,
                raw: "{\"id\":4}".into(),
            },
        ] {
            assert_eq!(recovery(&err), Recovery::Reconnect, "{err}");
        }
        let frame = UsdmWsError::Frame {
            stream: "btcusdt@aggTrade".into(),
            raw: "{}".into(),
            reason: "missing field".into(),
        };
        assert_eq!(recovery(&frame), Recovery::Skip);
    }

    #[test]
    fn a_refusal_and_a_request_the_client_refused_to_send_are_fatal() {
        let refused = UsdmWsError::Refused {
            code: 2,
            msg: "Invalid request".into(),
        };
        assert_eq!(recovery(&refused), Recovery::Stop);
        let full = UsdmWsError::TooManyStreams {
            path: StreamPath::Market,
            limit: 1024,
        };
        assert_eq!(recovery(&full), Recovery::Stop);
        assert_eq!(
            full.to_string(),
            "a market connection carries at most 1024 streams"
        );
        let wrong = UsdmWsError::WrongPath {
            stream: StreamName::BookTicker(Symbol::new("BTCUSDT").unwrap()),
            path: StreamPath::Market,
        };
        assert_eq!(recovery(&wrong), Recovery::Stop);
        assert_eq!(
            wrong.to_string(),
            "btcusdt@bookTicker rides the public path, not market"
        );
        let url = transport(tungstenite::Error::Url(
            tungstenite::error::UrlError::NoHostName,
        ));
        assert_eq!(recovery(&url), Recovery::Stop);
        assert_eq!(
            recovery(&UsdmWsError::Stopped { cause: None }),
            Recovery::Stop
        );
    }

    #[test]
    fn a_use_after_close_and_a_capacity_failure_reconnect_d19() {
        // D19 is Binance's: a connection used after it closed reconnects,
        // where perps and rtds stop. A frame or header past tungstenite's
        // limits does too, though `retrying_can_fix` alone gives up on it.
        assert_eq!(
            recovery(&transport(tungstenite::Error::AlreadyClosed)),
            Recovery::Reconnect
        );
        assert_eq!(
            recovery(&transport(tungstenite::Error::Capacity(
                tungstenite::error::CapacityError::TooManyHeaders
            ))),
            Recovery::Reconnect
        );
    }

    #[test]
    fn a_change_during_an_outage_is_recorded_and_answered_at_once() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let mut streams = vec![agg("BTCUSDT")];
        let recorded = protocol.record(
            &mut streams,
            Change::Subscribe(vec![agg("ETHUSDT"), agg("BTCUSDT"), agg("ETHUSDT")]),
        );
        assert!(matches!(recorded, Offline::Recorded(Ok(ref s)) if s.len() == 2));
        let recorded = protocol.record(&mut streams, Change::Unsubscribe(vec![agg("BTCUSDT")]));
        assert!(matches!(recorded, Offline::Recorded(Ok(ref s)) if *s == [agg("ETHUSDT")]));
        // The cap holds offline too, and refuses before recording anything.
        let mut full: Vec<StreamName> = (0..1024).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
        let recorded = protocol.record(&mut full, Change::Subscribe(vec![agg("BTCUSDT")]));
        assert!(matches!(
            recorded,
            Offline::Recorded(Err(UsdmWsError::TooManyStreams { limit: 1024, .. }))
        ));
        assert_eq!(full.len(), 1024);
    }

    #[test]
    fn an_answer_nobody_awaits_decodes_to_nothing_and_a_bad_frame_to_an_error() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let answer = Message::Text(r#"{"result":null,"id":99}"#.into());
        assert!(matches!(protocol.decode(&answer), Decoded::Nothing));
        let bad = Message::Text(
            r#"{"stream":"btcusdt@markPrice@1s","data":{"e":"markPriceUpdate"}}"#.into(),
        );
        let decoded = protocol.decode(&bad);
        assert!(matches!(decoded, Decoded::Error(UsdmWsError::Frame { .. })));
        assert!(!protocol.delivered(&bad, &decoded));
        assert!(protocol.is_live(&Message::Pong(Vec::new().into()), &Decoded::Nothing));
    }

    #[test]
    fn the_answer_to_a_list_subscriptions_keepalive_is_nothing_and_is_live_d9() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let answer = Message::Text(r#"{"result":["btcusdt@markPrice@1s"],"id":7}"#.into());
        let decoded = protocol.decode(&answer);
        assert!(matches!(decoded, Decoded::Nothing));
        assert!(protocol.is_live(&answer, &decoded));
        assert!(!protocol.delivered(&answer, &decoded));
    }
}
