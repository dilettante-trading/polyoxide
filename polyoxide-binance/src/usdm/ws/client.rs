//! The bare tier: one connection on one path, control methods, and a `Stream`
//! of updates.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
#[cfg(test)]
use futures_util::StreamExt;
#[cfg(test)]
use tokio::time::Instant;

use crate::{
    shared::ws::bare::Bare,
    usdm::ws::{
        error::UsdmWsError, event::Update, protocol::UsdmStreams, stream::StreamName, StreamPath,
        USDM_WS_BASE,
    },
};

#[cfg(test)]
use crate::usdm::ws::MIN_REQUEST_INTERVAL;

/// How long the bare tier waits for a connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the bare tier waits for a request's answer.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// A bare connection to one path's combined stream.
///
/// Requests are batched at most 200 names each and sent no faster than one
/// per 200 ms; each waits for the answer carrying its id, and updates read
/// meanwhile are kept and yielded by the stream afterwards. A stream for the
/// other path, or one that would take the connection past 1024 streams, is
/// refused before anything is sent. The control methods are not cancel-safe,
/// and after a `NoAnswer` the server's membership is unknown: replace the
/// connection rather than reuse it, as the supervised tier does. The server's
/// protocol pings are answered while the caller polls, and its close is
/// answered with the close reply. The stream ends after the server's close,
/// or after the first transport error it yields; a control call after that
/// fails, with the close as [`UsdmWsError::Closed`] or, after an error, as
/// [`UsdmWsError::Connect`] whose kind is `AlreadyClosed`, and sends nothing.
/// Binance closes a connection after 24 hours;
/// the supervised tier ([`UsdmWsBuilder`](crate::usdm::ws::UsdmWsBuilder))
/// replaces it before then, and this tier does not.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::usdm::{types::Symbol, ws::{StreamName, StreamPath, UsdmWs}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::AggTrade(btc)]).await?;
/// while let Some(update) = ws.next().await {
///     println!("{}", serde_json::to_string(&update?)?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct UsdmWs {
    inner: Bare<UsdmStreams>,
}

impl std::fmt::Debug for UsdmWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsdmWs")
            .field("path", &self.inner.path())
            .field("streams", &self.inner.streams().len())
            .field("connection", self.inner.connection())
            .finish_non_exhaustive()
    }
}

impl UsdmWs {
    /// Connect to `path` on the production host and subscribe.
    pub async fn connect(
        path: StreamPath,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, UsdmWsError> {
        Self::connect_to(USDM_WS_BASE, path, streams).await
    }

    /// Connect to `path` on another host, such as a local test server, and
    /// subscribe.
    pub async fn connect_to(
        base_url: &str,
        path: StreamPath,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, UsdmWsError> {
        let streams: Vec<StreamName> = streams.into_iter().collect();
        let mut ws = Self::open(base_url, path, CONNECT_TIMEOUT, ANSWER_TIMEOUT).await?;
        ws.subscribe(&streams).await?;
        Ok(ws)
    }

    /// Open a connection with no streams.
    pub(crate) async fn open(
        base_url: &str,
        path: StreamPath,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, UsdmWsError> {
        Ok(Self {
            inner: Bare::open(base_url, path, connect_timeout, answer_timeout).await?,
        })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with [`UsdmWsError::WrongPath`] or [`UsdmWsError::TooManyStreams`]
    /// before sending anything, and with [`UsdmWsError::Refused`] when the
    /// server answers with an error; batches answered before it stay
    /// subscribed, and [`streams`](Self::streams) says which.
    pub async fn subscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        let path = self.inner.path();
        if let Some(stream) = streams.iter().find(|s| s.path() != path) {
            return Err(UsdmWsError::WrongPath {
                stream: stream.clone(),
                path,
            });
        }
        self.inner.subscribe(streams).await
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub async fn unsubscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        self.inner.unsubscribe(streams).await
    }

    /// The stream names the server says this connection carries.
    pub async fn list_subscriptions(&mut self) -> Result<Vec<String>, UsdmWsError> {
        self.inner.list_subscriptions().await
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub async fn ping(&mut self) -> Result<Duration, UsdmWsError> {
        self.inner.ping().await
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), UsdmWsError> {
        self.inner.close().await
    }

    /// The streams this connection carries.
    pub fn streams(&self) -> &[StreamName] {
        self.inner.streams()
    }

    /// The path this connection is on.
    pub fn path(&self) -> StreamPath {
        self.inner.path()
    }

    /// The server's close code and reason, once it has closed the connection
    /// or the stream has ended without a close frame.
    #[cfg(test)]
    fn close_frame(&self) -> Option<(Option<u16>, String)> {
        self.inner.close_frame()
    }
}

impl Stream for UsdmWs {
    type Item = Result<Update, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usdm::{
        types::Symbol,
        ws::{
            event::Payload,
            fixtures,
            scripted::{Script, ScriptedServer},
            stream::{DepthLevels, DepthSpeed},
        },
    };

    fn agg(symbol: &str) -> StreamName {
        StreamName::AggTrade(Symbol::new(symbol).unwrap())
    }

    #[tokio::test]
    async fn connect_subscribes_on_its_path_and_yields_updates() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![fixtures::AGG_TRADE.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
        assert_eq!(server.paths(), ["/market/stream"]);
        assert_eq!(
            server.requests()[0]["params"],
            serde_json::json!(["btcusdt@aggTrade"])
        );
    }

    #[tokio::test]
    async fn a_stream_for_the_other_path_is_refused_before_sending() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let depth = StreamName::PartialDepth(
            Symbol::new("BTCUSDT").unwrap(),
            DepthLevels::Five,
            DepthSpeed::Ms100,
        );
        let err = ws.subscribe(&[depth]).await.unwrap_err();
        assert!(matches!(
            err,
            UsdmWsError::WrongPath {
                path: StreamPath::Market,
                ..
            }
        ));
        assert_eq!(server.requests().len(), 1);
    }

    #[tokio::test]
    async fn requests_carry_at_most_200_names_and_are_paced() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let streams: Vec<StreamName> = (0..450).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
        let start = Instant::now();
        let ws = UsdmWs::connect_to(&server.url, StreamPath::Market, streams.clone())
            .await
            .unwrap();
        assert_eq!(ws.streams().len(), 450);
        let sizes: Vec<usize> = server
            .requests()
            .iter()
            .map(|r| r["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [200, 200, 50]);
        assert!(
            start.elapsed() >= MIN_REQUEST_INTERVAL * 2,
            "three requests in {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn the_1025th_stream_is_refused_and_nothing_is_sent() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let streams: Vec<StreamName> = (0..1024).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect();
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, streams)
            .await
            .unwrap();
        let sent = server.requests().len();
        let err = ws.subscribe(&[agg("BTCUSDT")]).await.unwrap_err();
        assert!(matches!(
            err,
            UsdmWsError::TooManyStreams { limit: 1024, .. }
        ));
        assert_eq!(server.requests().len(), sent);
        // Already-subscribed names do not count twice.
        ws.subscribe(&[agg("ZZ0000USDT")]).await.unwrap();
    }

    #[tokio::test]
    async fn an_error_answer_is_refused_and_the_connection_kept() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, UsdmWsError::Refused { code: 2, .. }),
            "{err:?}"
        );
        assert_eq!(ws.streams(), &[agg("BTCUSDT")]);
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["btcusdt@aggTrade"]);
    }

    #[tokio::test]
    async fn a_ping_is_answered_and_unsubscribe_leaves_the_rest() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = UsdmWs::connect_to(
            &server.url,
            StreamPath::Market,
            [agg("BTCUSDT"), agg("ETHUSDT")],
        )
        .await
        .unwrap();
        assert!(ws.ping().await.unwrap() < Duration::from_secs(1));
        ws.unsubscribe(&[agg("BTCUSDT")]).await.unwrap();
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["ethusdt@aggTrade"]);
        // The supervised tier replays from this list after a reconnect.
        assert_eq!(ws.streams(), &[agg("ETHUSDT")]);
    }

    #[tokio::test]
    async fn the_server_s_close_code_is_reported() {
        let server = ScriptedServer::start(vec![Script {
            close_after: true,
            close_code: Some((1008, "Too many requests".into())),
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        assert!(ws.next().await.is_none());
        assert_eq!(
            ws.close_frame(),
            Some((Some(1008), "Too many requests".to_owned()))
        );
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, UsdmWsError::Connect(_) | UsdmWsError::Closed { .. }),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_server_close_is_answered_with_the_close_reply_drift_r3() {
        // DRIFT R3: RFC 6455's close reply. tungstenite queues it as it reads
        // the close and sends it only on a later read or flush, which a
        // stream that has ended never makes.
        let server = ScriptedServer::start(vec![Script {
            close_after: true,
            close_code: Some((1011, "Internal error".into())),
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        assert!(ws.next().await.is_none());
        // Held open and never read again, as a caller that saw the end
        // would leave it.
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
        drop(ws);
    }

    #[tokio::test]
    async fn a_late_answer_is_not_taken_for_the_next_and_updates_read_meanwhile_are_kept() {
        // Request 2 is refused, but its answer is held until request 3 has
        // been sent, by when request 2 has timed out. Request 3 must skip that
        // answer and take its own, and the update sent ahead of them is kept.
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("bbbusdt@aggTrade".into(), 2, "Invalid request".into())],
            hold_answer: Some((2, fixtures::AGG_TRADE.into())),
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::open(
            &server.url,
            StreamPath::Market,
            CONNECT_TIMEOUT,
            Duration::from_millis(200),
        )
        .await
        .unwrap();
        ws.subscribe(&[agg("AAAUSDT")]).await.unwrap();
        let err = ws.subscribe(&[agg("BBBUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, UsdmWsError::NoAnswer { id: 2, .. }),
            "{err:?}"
        );
        ws.subscribe(&[agg("CCCUSDT")]).await.unwrap();
        assert_eq!(ws.streams(), &[agg("AAAUSDT"), agg("CCCUSDT")]);
        let update = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("the update read during request 3 was dropped")
            .unwrap()
            .unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
    }

    #[tokio::test]
    async fn an_answer_nobody_awaits_is_not_yielded() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![
                r#"{"result":null,"id":99}"#.into(),
                fixtures::AGG_TRADE.into(),
            ],
            ..Default::default()
        }])
        .await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
    }
}
