//! The CLOB's supervised sockets: a channel's connection run by the socket
//! kit's `Supervisor` (AD-11), which reconnects it, replays its membership and
//! marks every outage, where the bare tier ends on the first failure.
//!
//! This file holds what the supervised sockets share: the rule for the URL
//! they take, their timing, the reason an outage gives, their recovery, and
//! the filter their decode applies to a frame's text. Each channel's protocol,
//! builder, events and membership calls are in its own file.

use std::time::Duration;

use polyoxide_venue::Classify;
use polyoxide_ws::{tungstenite::Message, Kind, Outage, Recovery, Timing};

use super::{
    client::{frame_text, DEFAULT_CONNECT_TIMEOUT},
    ClobWsError,
};

mod market;
mod user;

pub use market::{
    ClobMarketProtocol, MarketChange, MarketEvent, MarketMembership, MarketWsBuilder,
    SupervisedMarketWs,
};
pub use user::{
    ClobUserProtocol, SupervisedUserWs, UserChange, UserEvent, UserMembership, UserWsBuilder,
};

/// Why a supervised connection went down, in a `Disconnected` event.
///
/// Mapped from the socket kit's `Outage` as Binance's own `DisconnectReason`
/// is. The market and user sockets never rotate, so [`Rotation`](Self::Rotation)
/// is kept for completeness.
#[derive(Debug)]
#[non_exhaustive]
pub enum DisconnectReason {
    /// The server closed the connection, with the code and reason it sent, or
    /// ended the stream with neither.
    Closed {
        /// The close code, when a close frame carried one.
        code: Option<u16>,
        /// The close reason, empty when none was given.
        reason: String,
    },
    /// A failure: a read, a ping or a decoded frame that recovery sent to a
    /// reconnect, or the failed first connection attempt of a socket that
    /// starts in an outage.
    Error(ClobWsError),
    /// Nothing arrived for the staleness window, or a membership change or
    /// replay went unanswered for it.
    Stale {
        /// The silence, less any time the consumer's queue was full.
        silent: Duration,
    },
    /// Replaced on purpose at the connection's maximum age.
    Rotation,
}

/// The URL a supervised socket takes: `wss://`, or `ws://` to a loopback host
/// (`127.0.0.1`, `[::1]` or `localhost`), so a test reaches a local server and
/// credentials never travel unencrypted to a remote one. Anything else,
/// and a URL that does not parse, is refused with
/// [`ClobWsError::InvalidMessage`]. The bare tier keeps its `wss://`-only rule.
pub(super) fn checked_url(url: String) -> Result<String, ClobWsError> {
    let allowed = url::Url::parse(&url).is_ok_and(|parsed| match parsed.scheme() {
        "wss" => true,
        "ws" => match parsed.host() {
            Some(url::Host::Domain(host)) => host == "localhost",
            Some(url::Host::Ipv4(ip)) => ip == std::net::Ipv4Addr::LOCALHOST,
            Some(url::Host::Ipv6(ip)) => ip == std::net::Ipv6Addr::LOCALHOST,
            None => false,
        },
        _ => false,
    });
    if allowed {
        Ok(url)
    } else {
        Err(ClobWsError::InvalidMessage(format!(
            "the URL must be wss://, or ws:// to a loopback host: {url}"
        )))
    }
}

/// The timing both supervised sockets start from: a text `PING` every
/// 10 s, stale after 30 s without a frame, backoff from 500 ms to 60 s, and a
/// 10 s connect timeout for each address and each step. No rotation.
pub(super) fn default_timing() -> Timing {
    let mut timing = Timing::default();
    timing.ping_interval = Some(Duration::from_secs(10));
    timing.stale_after = Duration::from_secs(30);
    timing.initial_backoff = Duration::from_millis(500);
    timing.max_backoff = Duration::from_secs(60);
    timing.connect_timeout = DEFAULT_CONNECT_TIMEOUT;
    timing
}

/// The reason an outage gives: the kit's `Outage` as a [`DisconnectReason`]. An
/// outage a later kit adds reads as a stall of no length.
pub(super) fn disconnect_reason(outage: Outage<ClobWsError>) -> DisconnectReason {
    match outage {
        Outage::Closed { code, reason } => DisconnectReason::Closed { code, reason },
        Outage::Error(err) => DisconnectReason::Error(err),
        Outage::Stale { silent } => DisconnectReason::Stale { silent },
        Outage::Rotation => DisconnectReason::Rotation,
        _ => DisconnectReason::Stale {
            silent: Duration::ZERO,
        },
    }
}

/// What the supervisor does after a failure of a supervised socket:
///
/// - a frame that does not decode, a [`Json`](ClobWsError::Json) error or an
///   [`InvalidMessage`](ClobWsError::InvalidMessage), is skipped: the stream
///   yields the error and keeps the connection;
/// - an [`Authentication`](ClobWsError::Authentication) failure stops the
///   stream, since retrying cannot fix bad credentials;
/// - a connection used after it closed, a [`Connection`](ClobWsError::Connection)
///   whose kit error's kind is `Kind::AlreadyClosed`, reconnects, as Binance's
///   and sports' recovery rows do (D19 of `divergences.md`). The socket table
///   classes a use after close `InvalidRequest`, so the class rule alone would
///   stop on it, and a ping that races a server close would end the stream;
/// - any other failure reconnects exactly when its class is retriable, so a
///   refused upgrade follows the socket table, which agrees with the kit's
///   `retrying_can_fix` on every status, and a server close reconnects unless
///   its code is one the venue refuses with.
///
/// The error's class decides; no code here matches over `tungstenite::Error`.
pub(super) fn recover(err: &ClobWsError) -> Recovery {
    match err {
        ClobWsError::Json(_) | ClobWsError::InvalidMessage(_) => Recovery::Skip,
        ClobWsError::Authentication(_) => Recovery::Stop,
        ClobWsError::Connection(err) if matches!(err.kind(), Kind::AlreadyClosed) => {
            Recovery::Reconnect
        }
        err if err.is_retriable() => Recovery::Reconnect,
        _ => Recovery::Stop,
    }
}

/// The text of a data frame that can carry an event, or `None` for every frame
/// the supervised decode leaves alone: a Close, Ping or Pong, a Binary frame
/// that is not UTF-8, and the keep-alives and acknowledgements the venue sends
/// as text, which are `PONG`, `{}`, an empty text, and any text without an
/// `event_type`, as the bare tier skips them. A Binary frame is decoded as
/// text (divergence D12).
pub(super) fn data_text(frame: &Message) -> Option<&str> {
    let text = frame_text(frame)?;
    if text == "PONG" || text == "{}" || text.is_empty() || !text.contains("event_type") {
        return None;
    }
    Some(text)
}

/// Whether a data frame is a JSON array of events, read past leading whitespace;
/// the market and user decoders share the rule.
pub(super) fn is_array_frame(text: &str) -> bool {
    text.trim_start().starts_with('[')
}

#[cfg(test)]
mod tests {
    use polyoxide_venue::Class;
    use polyoxide_ws::{
        tungstenite::{self, error, http},
        WsError,
    };

    use super::*;

    /// A transport failure as the connection reports it, through the kit.
    fn transport(err: tungstenite::Error) -> ClobWsError {
        ClobWsError::from(WsError::from(err))
    }

    /// An upgrade the server refused with this status.
    fn refused(status: u16) -> ClobWsError {
        transport(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    fn closed(code: Option<u16>) -> ClobWsError {
        ClobWsError::from(WsError::closed(code, ""))
    }

    #[test]
    fn recovery_follows_the_class_with_the_clob_rows() {
        let json = serde_json::from_str::<()>("{").unwrap_err();
        let rows = [
            // A frame that did not decode keeps the connection.
            (ClobWsError::Json(json), Recovery::Skip),
            (
                ClobWsError::InvalidMessage("a frame".into()),
                Recovery::Skip,
            ),
            // Retrying cannot fix bad credentials.
            (
                ClobWsError::Authentication("authentication failed".into()),
                Recovery::Stop,
            ),
            // A server close reconnects, but for the codes the venue refuses
            // with.
            (closed(Some(1000)), Recovery::Reconnect),
            (closed(Some(1001)), Recovery::Reconnect),
            (closed(Some(1011)), Recovery::Reconnect),
            (closed(None), Recovery::Reconnect),
            (closed(Some(1008)), Recovery::Stop),
            (closed(Some(1002)), Recovery::Stop),
            (closed(Some(4001)), Recovery::Stop),
            // The kit's own timeouts and the connect timeout of clob's dial.
            (
                ClobWsError::from(WsError::connect_timeout(Duration::from_secs(1))),
                Recovery::Reconnect,
            ),
            (
                ClobWsError::from(WsError::no_answer(Duration::from_secs(1))),
                Recovery::Reconnect,
            ),
            (
                ClobWsError::ConnectTimeout {
                    url: "wss://example.com".into(),
                    timeout: Duration::from_secs(1),
                },
                Recovery::Reconnect,
            ),
            (ClobWsError::ConnectionClosed, Recovery::Reconnect),
            (
                transport(tungstenite::Error::Io(
                    std::io::ErrorKind::ConnectionReset.into(),
                )),
                Recovery::Reconnect,
            ),
            // D19: a connection used after it closed reconnects, though its
            // class is `InvalidRequest`.
            (
                transport(tungstenite::Error::AlreadyClosed),
                Recovery::Reconnect,
            ),
            // The client's own refusals stop.
            (
                transport(tungstenite::Error::Url(error::UrlError::NoHostName)),
                Recovery::Stop,
            ),
            (ClobWsError::Url(url::ParseError::EmptyHost), Recovery::Stop),
            (ClobWsError::MembershipClosed, Recovery::Stop),
            // A refused upgrade follows its status: 408, 425, 429 and 5xx
            // reconnect, and any other refusal stops.
            (refused(408), Recovery::Reconnect),
            (refused(425), Recovery::Reconnect),
            (refused(429), Recovery::Reconnect),
            (refused(503), Recovery::Reconnect),
            (refused(401), Recovery::Stop),
            (refused(404), Recovery::Stop),
            (refused(418), Recovery::Stop),
            (refused(451), Recovery::Stop),
        ];
        for (err, expected) in rows {
            assert_eq!(recover(&err), expected, "{err:?}");
        }
    }

    #[test]
    fn a_refusal_the_class_calls_retriable_agrees_with_the_kits_rule() {
        // DRIFT R1: the socket table and `retrying_can_fix` agree on every
        // refused status, so recovery reads the class and not the status.
        for status in [400, 401, 403, 404, 408, 418, 425, 429, 451, 500, 502, 503] {
            let err = refused(status);
            let ClobWsError::Connection(inner) = &err else {
                panic!("a refusal is a connection error: {err:?}");
            };
            let by_kit = polyoxide_ws::retrying_can_fix(inner, |status| {
                polyoxide_venue::class_for_handshake_status(status).is_retriable()
            });
            assert_eq!(
                recover(&err) == Recovery::Reconnect,
                by_kit,
                "status {status}"
            );
        }
    }

    #[test]
    fn every_outage_maps_to_its_reason() {
        let down = disconnect_reason(Outage::Closed {
            code: Some(1001),
            reason: "restart".into(),
        });
        assert!(
            matches!(&down, DisconnectReason::Closed { code: Some(1001), reason }
                if reason == "restart"),
            "{down:?}"
        );
        let down = disconnect_reason(Outage::Error(ClobWsError::ConnectionClosed));
        assert!(
            matches!(down, DisconnectReason::Error(ClobWsError::ConnectionClosed)),
            "{down:?}"
        );
        let down = disconnect_reason(Outage::Stale {
            silent: Duration::from_secs(3),
        });
        assert!(
            matches!(&down, DisconnectReason::Stale { silent }
                if *silent == Duration::from_secs(3)),
            "{down:?}"
        );
        let down = disconnect_reason(Outage::Rotation);
        assert!(matches!(down, DisconnectReason::Rotation), "{down:?}");
    }

    #[test]
    fn the_url_rule_is_wss_or_a_loopback_ws() {
        for url in [
            "wss://ws-subscriptions-clob.polymarket.com/ws/market",
            "ws://127.0.0.1:8080/ws/market",
            "ws://localhost/ws/user",
            "ws://[::1]:8080",
        ] {
            assert_eq!(checked_url(url.to_owned()).expect(url), url);
        }
        for url in [
            "ws://ws-subscriptions-clob.polymarket.com/ws/market",
            "ws://127.0.0.2:8080",
            "ws://localhost.example.com",
            "ws://10.0.0.1",
            "http://localhost",
            "https://ws-subscriptions-clob.polymarket.com",
            "ws-subscriptions-clob.polymarket.com",
            "",
        ] {
            let err = checked_url(url.to_owned()).expect_err(url);
            assert!(
                matches!(&err, ClobWsError::InvalidMessage(text)
                    if text == &format!("the URL must be wss://, or ws:// to a loopback host: {url}")),
                "{err:?}"
            );
            assert_eq!(err.class(), Class::InvalidRequest);
        }
    }

    #[test]
    fn the_timing_is_the_documented_one() {
        let timing = default_timing();
        assert_eq!(timing.ping_interval, Some(Duration::from_secs(10)));
        assert_eq!(timing.stale_after, Duration::from_secs(30));
        assert_eq!(timing.initial_backoff, Duration::from_millis(500));
        assert_eq!(timing.max_backoff, Duration::from_secs(60));
        assert_eq!(timing.connect_timeout, Duration::from_secs(10));
        assert_eq!(timing.max_connection_age, None);
    }

    #[test]
    fn the_decode_filter_passes_events_and_skips_the_rest() {
        let book = r#"{"event_type":"book","asset_id":"a"}"#;
        assert_eq!(data_text(&Message::Text(book.into())), Some(book));
        // A Binary frame is decoded as text (divergence D12).
        assert_eq!(
            data_text(&Message::Binary(book.as_bytes().to_vec().into())),
            Some(book)
        );
        let array = r#"[{"event_type":"book","asset_id":"a"}]"#;
        assert_eq!(data_text(&Message::Text(array.into())), Some(array));

        for text in ["PONG", "{}", "", "[]", r#"{"subscribed":true}"#] {
            assert_eq!(data_text(&Message::Text(text.into())), None, "{text:?}");
        }
        // Not UTF-8, so no text; and the control frames carry none.
        assert_eq!(data_text(&Message::Binary(vec![0xff].into())), None);
        assert_eq!(data_text(&Message::Ping(Vec::new().into())), None);
        assert_eq!(data_text(&Message::Pong(Vec::new().into())), None);
        assert_eq!(data_text(&Message::Close(None)), None);
    }
}
