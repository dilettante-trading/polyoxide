//! The bare tier: one connection on spot's one path, control methods, and a
//! `Stream` of updates.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};

use crate::{
    shared::ws::bare::Bare,
    spot::ws::{
        error::SpotWsError, event::Update, protocol::SpotStreams, stream::StreamName, StreamPath,
        SPOT_WS_BASE,
    },
};

#[cfg(test)]
use crate::spot::ws::{MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL};

/// How the bare tier waits for a connection.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long the bare tier waits for a request's answer.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);

/// A bare connection to spot's combined stream.
///
/// Requests carry at most 1024 names, the connection's cap, and are sent no
/// faster than one per 500 ms; each waits for the answer carrying its id, and
/// updates read meanwhile are kept and yielded by the stream afterwards. A
/// stream that would take the connection past 1024 streams is refused before
/// anything is sent. The control methods are not cancel-safe, and after a
/// `NoAnswer` the server's membership is unknown: replace the connection
/// rather than reuse it, as the supervised tier does. The server's protocol
/// pings are answered while the caller polls, and its close is answered with
/// the close reply. The stream ends after the server's close, or after the
/// first transport error it yields; a control call after that fails, with the
/// close as [`SpotWsError::Closed`] or, after an error, as
/// [`SpotWsError::Connect`] whose kind is `AlreadyClosed`, and sends nothing.
/// Binance closes a connection after 24 hours; the supervised tier
/// ([`SpotWsBuilder`](crate::spot::ws::SpotWsBuilder)) replaces it before then,
/// and this tier does not.
///
/// ```no_run
/// use futures_util::StreamExt;
/// use polyoxide_binance::spot::{types::Symbol, ws::{SpotWs, StreamName}};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let btc = Symbol::new("BTCUSDT")?;
/// let mut ws = SpotWs::connect([StreamName::AggTrade(btc)]).await?;
/// while let Some(update) = ws.next().await {
///     println!("{}", serde_json::to_string(&update?)?);
/// }
/// # Ok(())
/// # }
/// ```
pub struct SpotWs {
    bare: Bare<SpotStreams>,
}

impl std::fmt::Debug for SpotWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpotWs")
            .field("streams", &self.bare.streams().len())
            .field("connection", &self.bare.connection())
            .finish_non_exhaustive()
    }
}

impl SpotWs {
    /// Connect to the production host and subscribe.
    pub async fn connect(
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, SpotWsError> {
        Self::connect_to(SPOT_WS_BASE, streams).await
    }

    /// Connect to another host, such as a local test server, and subscribe.
    /// The client appends `/stream` to `base_url`.
    pub async fn connect_to(
        base_url: &str,
        streams: impl IntoIterator<Item = StreamName>,
    ) -> Result<Self, SpotWsError> {
        let streams: Vec<StreamName> = streams.into_iter().collect();
        let mut ws = Self::open(base_url, CONNECT_TIMEOUT, ANSWER_TIMEOUT).await?;
        ws.subscribe(&streams).await?;
        Ok(ws)
    }

    /// Open a connection with no streams.
    pub(crate) async fn open(
        base_url: &str,
        connect_timeout: Duration,
        answer_timeout: Duration,
    ) -> Result<Self, SpotWsError> {
        let bare = Bare::open(
            base_url,
            StreamPath::Stream,
            connect_timeout,
            answer_timeout,
        )
        .await?;
        Ok(Self { bare })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with [`SpotWsError::TooManyStreams`] before sending anything, and
    /// with [`SpotWsError::Refused`] when the server answers with an error;
    /// batches answered before it stay subscribed, and
    /// [`streams`](Self::streams) says which.
    pub async fn subscribe(&mut self, streams: &[StreamName]) -> Result<(), SpotWsError> {
        self.bare.subscribe(streams).await
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub async fn unsubscribe(&mut self, streams: &[StreamName]) -> Result<(), SpotWsError> {
        self.bare.unsubscribe(streams).await
    }

    /// The stream names the server says this connection carries.
    pub async fn list_subscriptions(&mut self) -> Result<Vec<String>, SpotWsError> {
        self.bare.list_subscriptions().await
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub async fn ping(&mut self) -> Result<Duration, SpotWsError> {
        self.bare.ping().await
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), SpotWsError> {
        self.bare.close().await
    }

    /// The streams this connection carries.
    pub fn streams(&self) -> &[StreamName] {
        self.bare.streams()
    }

    /// The server's close code and reason, once it has closed the connection
    /// or the stream has ended without a close frame.
    #[cfg(test)]
    fn close_frame(&self) -> Option<(Option<u16>, String)> {
        self.bare.close_frame()
    }
}

impl Stream for SpotWs {
    type Item = Result<Update, SpotWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.bare.poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spot::{
        types::Symbol,
        ws::{
            event::Payload,
            fixtures,
            scripted::{Script, ScriptedServer},
        },
    };

    fn agg(symbol: &str) -> StreamName {
        StreamName::AggTrade(Symbol::new(symbol).unwrap())
    }

    fn many(n: usize) -> Vec<StreamName> {
        (0..n).map(|i| agg(&format!("ZZ{i:04}USDT"))).collect()
    }

    #[test]
    fn the_documented_limits_are_the_measured_ones() {
        // docs/specs/binance/OBSERVED.md, `## Spot`: 1024 names in one request
        // held, 1024 streams per connection, one request every 500 ms held for
        // 45 s where one every 250 ms was closed with 1008.
        assert_eq!(MAX_NAMES_PER_REQUEST, 1024);
        assert_eq!(MAX_STREAMS_PER_CONNECTION, 1024);
        assert_eq!(MIN_REQUEST_INTERVAL, Duration::from_millis(500));
        assert_eq!(SPOT_WS_BASE, "wss://stream.binance.com:9443");
        assert_eq!(CONNECT_TIMEOUT, Duration::from_secs(10));
        assert_eq!(ANSWER_TIMEOUT, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn connect_subscribes_on_the_one_path_and_yields_updates() {
        let server = ScriptedServer::start(vec![Script {
            pushes: vec![fixtures::AGG_TRADE.into()],
            ..Default::default()
        }])
        .await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
        assert_eq!(server.paths(), ["/stream"]);
        assert_eq!(
            server.requests()[0]["params"],
            serde_json::json!(["btcusdt@aggTrade"])
        );
    }

    #[tokio::test]
    async fn a_request_carries_all_1024_names() {
        // Bug: batching at USDⓈ-M's 200 names, which sends six requests where
        // one is measured to hold, and replays slowly at 500 ms apiece.
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let ws = SpotWs::connect_to(&server.url, many(1024)).await.unwrap();
        assert_eq!(ws.streams().len(), 1024);
        let sizes: Vec<usize> = server
            .requests()
            .iter()
            .map(|r| r["params"].as_array().unwrap().len())
            .collect();
        assert_eq!(sizes, [1024]);
    }

    #[tokio::test]
    async fn requests_are_paced_at_500_ms() {
        // Bug: a pace of 250 ms, which the host closed with 1008 "Too many
        // requests" once the pings and pongs were counted. The gap is read off
        // the server's clock against the measured 500 ms, not the constant.
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("AAAUSDT")])
            .await
            .unwrap();
        ws.subscribe(&[agg("BBBUSDT")]).await.unwrap();
        ws.subscribe(&[agg("CCCUSDT")]).await.unwrap();
        let received = server.received();
        assert_eq!(received.len(), 3);
        for pair in received.windows(2) {
            let gap = pair[1].at - pair[0].at;
            assert!(gap >= Duration::from_millis(490), "requests {gap:?} apart");
        }
    }

    #[tokio::test]
    async fn the_1025th_stream_is_refused_and_nothing_is_sent() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = SpotWs::connect_to(&server.url, many(1024)).await.unwrap();
        let sent = server.requests().len();
        let err = ws.subscribe(&[agg("BTCUSDT")]).await.unwrap_err();
        assert!(
            matches!(
                err,
                SpotWsError::TooManyStreams {
                    path: StreamPath::Stream,
                    limit: 1024
                }
            ),
            "{err:?}"
        );
        assert_eq!(server.requests().len(), sent);
        // Already-subscribed names do not count twice.
        ws.subscribe(&[agg("ZZ0000USDT")]).await.unwrap();
    }

    #[tokio::test]
    async fn connecting_with_1025_names_sends_nothing() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let err = SpotWs::connect_to(&server.url, many(1025))
            .await
            .unwrap_err();
        assert!(
            matches!(err, SpotWsError::TooManyStreams { limit: 1024, .. }),
            "{err:?}"
        );
        assert!(server.requests().is_empty());
    }

    #[tokio::test]
    async fn an_error_answer_is_refused_and_the_connection_kept() {
        let server = ScriptedServer::start(vec![Script {
            refuse: vec![("ethusdt@aggTrade".into(), 2, "Invalid request".into())],
            ..Default::default()
        }])
        .await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, SpotWsError::Refused { code: 2, .. }),
            "{err:?}"
        );
        assert_eq!(ws.streams(), &[agg("BTCUSDT")]);
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["btcusdt@aggTrade"]);
    }

    #[tokio::test]
    async fn a_ping_is_answered_and_unsubscribe_leaves_the_rest() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT"), agg("ETHUSDT")])
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
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        assert!(ws.next().await.is_none());
        assert_eq!(
            ws.close_frame(),
            Some((Some(1008), "Too many requests".to_owned()))
        );
        let err = ws.subscribe(&[agg("ETHUSDT")]).await.unwrap_err();
        assert!(
            matches!(err, SpotWsError::Connect(_) | SpotWsError::Closed { .. }),
            "{err:?}"
        );
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
        let mut ws = SpotWs::connect_to(&server.url, [agg("BTCUSDT")])
            .await
            .unwrap();
        let update = ws.next().await.unwrap().unwrap();
        assert!(matches!(update.payload, Payload::AggTrade(_)));
    }
}
