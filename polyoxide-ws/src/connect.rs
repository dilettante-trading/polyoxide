//! Opening a connection within a timeout (W8), and whether retrying a failed
//! one can help (W7).

use std::{future::Future, time::Duration};

use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::client::IntoClientRequest, MaybeTlsStream, WebSocketStream,
};

use crate::{ensure_crypto_provider, Kind, WsError};

/// How long one connection attempt may take unless a venue says otherwise:
/// the TCP connect, the TLS handshake and the WebSocket upgrade together.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// An open connection, over TLS or plain TCP.
pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Open a connection, giving up after `timeout`.
///
/// `request` is a URL, or a handshake request with its own headers, built
/// afresh for each attempt so any authentication in it is recomputed. The
/// rustls provider is installed first ([`ensure_crypto_provider`]). A
/// connection not open within `timeout` fails as
/// [`Kind::ConnectTimeout`]; every other failure is the transport's, such as
/// [`Kind::Refused`] for an upgrade the server answered with an HTTP status.
///
/// The basis is `polyoxide-sports`' `open` (`src/client.rs`) and
/// `polyoxide-binance`'s former connect in `UsdmWs::open`
/// (`usdm/ws/client.rs`, on this function since Story 4.7). A venue whose
/// connect is its own, as `polyoxide-clob`'s per-address connect is and
/// sports' is until Story 4.10, bounds it with [`with_timeout`] instead.
pub async fn connect<R>(request: R, timeout: Duration) -> Result<Socket, WsError>
where
    R: IntoClientRequest + Unpin,
{
    ensure_crypto_provider();
    let (socket, _response) = with_timeout(timeout, connect_async(request)).await??;
    Ok(socket)
}

/// Run a caller's own connect future, giving up after `timeout` with
/// [`Kind::ConnectTimeout`].
///
/// The future's own output comes back as it is, so its errors stay the
/// caller's. Install the rustls provider before a TLS connect of its own
/// ([`ensure_crypto_provider`]).
pub async fn with_timeout<F: Future>(timeout: Duration, connect: F) -> Result<F::Output, WsError> {
    tokio::time::timeout(timeout, connect)
        .await
        .map_err(|_| WsError::connect_timeout(timeout))
}

/// Whether reconnecting can help after a connect failed this way.
///
/// A refused upgrade is decided by `retriable_status`, the status rule. Pass
/// `polyoxide-venue`'s, which `Classify::is_retriable` reads too, so the two
/// can never disagree on a status:
/// `|status| class_for_handshake_status(status).is_retriable()`.
///
/// A bad URL, an upgrade request that could not be built, a refused TLS
/// configuration, an attack detection and a capacity limit replay the same
/// failure on every attempt, so they are `false`. Everything else, an I/O
/// failure, a timeout or a lost connection among them, is `true`.
///
/// This decides only whether a reconnect can help, not what a failure is:
/// that is the venue's classification. What to do after
/// [`Kind::AlreadyClosed`] (D19) and what a close code means stay with each
/// venue's recovery, so both are `true` here.
///
/// The basis is `polyoxide-sports`' `SportsError::retrying_can_fix`
/// (`src/error.rs`), with the status rule injected, since this crate cannot
/// see `polyoxide-venue`. sports keeps its own until Story 4.10, as it
/// retries a `Capacity` failure, which this gives up on.
pub fn retrying_can_fix(err: &WsError, retriable_status: impl Fn(u16) -> bool) -> bool {
    match err.kind() {
        Kind::Refused { status, .. } => retriable_status(status),
        Kind::Url | Kind::HttpFormat | Kind::Tls | Kind::AttackAttempt | Kind::Capacity => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use tokio_tungstenite::tungstenite::{
        self,
        error::{CapacityError, ProtocolError, TlsError, UrlError},
        http,
    };

    use super::*;
    use crate::test_server::{Script, ScriptedServer};

    /// A stand-in for `polyoxide-venue`'s status rule, which this crate
    /// cannot see; venue's table test checks the real one against the
    /// classification.
    fn rule(status: u16) -> bool {
        matches!(status, 408 | 425 | 429 | 500..=599)
    }

    fn refused(status: u16) -> WsError {
        WsError::from(tungstenite::Error::Http(
            http::Response::builder().status(status).body(None).unwrap(),
        ))
    }

    #[tokio::test(start_paused = true)]
    async fn a_hung_handshake_times_out_at_the_default() {
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let err = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect_err("a server that never upgrades must not connect");
        assert_eq!(
            err.kind(),
            Kind::ConnectTimeout {
                after: Duration::from_secs(10)
            }
        );
    }

    #[tokio::test]
    async fn a_refused_upgrade_keeps_its_status_and_retry_after() {
        let server = ScriptedServer::start(vec![
            Script::refuse_upgrade(429).with_header("Retry-After", "7")
        ])
        .await;
        let err = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect_err("a refused upgrade must not connect");
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status: 429,
                retry_after: Some("7")
            }
        );
    }

    #[tokio::test]
    async fn a_503_upgrade_is_worth_retrying_and_a_404_is_not() {
        let server = ScriptedServer::start(vec![
            Script::refuse_upgrade(503),
            Script::refuse_upgrade(404),
        ])
        .await;
        let unwell = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect_err("503");
        assert_eq!(
            unwell.kind(),
            Kind::Refused {
                status: 503,
                retry_after: None
            }
        );
        assert!(retrying_can_fix(&unwell, rule));

        let missing = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect_err("404");
        assert!(!retrying_can_fix(&missing, rule));
    }

    #[tokio::test]
    async fn a_server_that_upgrades_connects() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect("connect");
    }

    #[tokio::test]
    async fn with_timeout_bounds_a_callers_own_future_and_keeps_its_output() {
        let after = Duration::from_millis(20);
        let err = with_timeout(after, std::future::pending::<()>())
            .await
            .expect_err("a future that never finishes must time out");
        assert_eq!(err.kind(), Kind::ConnectTimeout { after });

        let output: Result<u8, &str> = with_timeout(after, async { Err("its own error") })
            .await
            .expect("a finished future is not a timeout");
        assert_eq!(output, Err("its own error"));
    }

    #[test]
    fn the_status_rule_decides_a_refused_upgrade() {
        for status in [301, 401, 403, 404, 418, 451, 600] {
            assert!(!retrying_can_fix(&refused(status), rule), "{status}");
        }
        for status in [408, 425, 429, 500, 502, 503] {
            assert!(retrying_can_fix(&refused(status), rule), "{status}");
        }
    }

    #[test]
    fn a_failure_that_replays_on_every_attempt_is_not_retried() {
        let bad_header = http::header::HeaderName::from_bytes(b"in valid").unwrap_err();
        for err in [
            tungstenite::Error::Url(UrlError::NoHostName),
            tungstenite::Error::HttpFormat(bad_header.into()),
            tungstenite::Error::Tls(TlsError::InvalidDnsName),
            tungstenite::Error::AttackAttempt,
            tungstenite::Error::Capacity(CapacityError::TooManyHeaders),
        ] {
            let err = WsError::from(err);
            assert!(!retrying_can_fix(&err, |_| true), "{err:?}");
        }
    }

    #[test]
    fn a_transport_failure_or_timeout_is_retried() {
        let after = Duration::from_secs(10);
        for err in [
            WsError::from(tungstenite::Error::Io(
                std::io::ErrorKind::ConnectionReset.into(),
            )),
            WsError::from(tungstenite::Error::Protocol(
                ProtocolError::ResetWithoutClosingHandshake,
            )),
            WsError::from(tungstenite::Error::ConnectionClosed),
            WsError::from(tungstenite::Error::Utf8),
            WsError::connect_timeout(after),
            WsError::no_answer(after),
            // D19 and a close code's meaning are the venue's to decide.
            WsError::from(tungstenite::Error::AlreadyClosed),
            WsError::closed(Some(1008), "policy"),
        ] {
            assert!(retrying_can_fix(&err, |_| false), "{err:?}");
        }
    }
}
