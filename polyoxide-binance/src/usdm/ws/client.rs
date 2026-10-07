//! The bare tier: one connection on one path, control methods, and a `Stream`
//! of updates.

use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use tokio::{net::TcpStream, time::Instant};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::CloseFrame, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::usdm::ws::{
    ensure_crypto_provider, error::UsdmWsError, event::Update, stream::StreamName, StreamPath,
    MAX_NAMES_PER_REQUEST, MAX_STREAMS_PER_CONNECTION, MIN_REQUEST_INTERVAL, USDM_WS_BASE,
};

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
/// protocol pings are answered while the caller polls. Binance closes a
/// connection after 24 hours; the supervised tier
/// ([`UsdmWsBuilder`](crate::usdm::ws::UsdmWsBuilder)) replaces it before
/// then, and this tier does not.
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
    inner: WebSocketStream<MaybeTlsStream<TcpStream>>,
    path: StreamPath,
    streams: Vec<StreamName>,
    next_id: u64,
    buffered: VecDeque<Result<Update, UsdmWsError>>,
    last_request: Option<Instant>,
    last_inbound: Instant,
    answer_timeout: Duration,
    close_frame: Option<(Option<u16>, String)>,
}

impl std::fmt::Debug for UsdmWs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UsdmWs")
            .field("path", &self.path)
            .field("streams", &self.streams.len())
            .field("buffered", &self.buffered.len())
            .finish_non_exhaustive()
    }
}

/// What a text frame that is not an update is.
enum Answer {
    /// `{"result": .., "id": n}`.
    Result { id: Option<u64>, result: Value },
    /// `{"error": {"code", "msg"}, "id": n}`.
    Error {
        id: Option<u64>,
        code: i64,
        msg: String,
    },
}

impl Answer {
    fn parse(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        if object.contains_key("stream") {
            return None;
        }
        let id = object.get("id").and_then(Value::as_u64);
        if let Some(error) = object.get("error") {
            return Some(Self::Error {
                id,
                code: error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                msg: error
                    .get("msg")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
        if object.contains_key("result") || object.contains_key("id") {
            return Some(Self::Result {
                id,
                result: object.get("result").cloned().unwrap_or(Value::Null),
            });
        }
        None
    }

    fn id(&self) -> Option<u64> {
        match self {
            Self::Result { id, .. } | Self::Error { id, .. } => *id,
        }
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
        ensure_crypto_provider();
        let (inner, _) = tokio::time::timeout(connect_timeout, connect_async(path.url(base_url)))
            .await
            .map_err(|_| UsdmWsError::ConnectTimeout(connect_timeout))??;
        Ok(Self {
            inner,
            path,
            streams: Vec::new(),
            next_id: 1,
            buffered: VecDeque::new(),
            last_request: None,
            last_inbound: Instant::now(),
            answer_timeout,
            close_frame: None,
        })
    }

    /// Subscribe to more streams. A stream already subscribed is skipped.
    ///
    /// Fails with [`UsdmWsError::WrongPath`] or [`UsdmWsError::TooManyStreams`]
    /// before sending anything, and with [`UsdmWsError::Refused`] when the
    /// server answers with an error; batches answered before it stay
    /// subscribed, and [`streams`](Self::streams) says which.
    pub async fn subscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        if let Some(stream) = streams.iter().find(|s| s.path() != self.path) {
            return Err(UsdmWsError::WrongPath {
                stream: stream.clone(),
                path: self.path,
            });
        }
        let mut new: Vec<StreamName> = Vec::new();
        for stream in streams {
            if !self.streams.contains(stream) && !new.contains(stream) {
                new.push(stream.clone());
            }
        }
        if self.streams.len() + new.len() > MAX_STREAMS_PER_CONNECTION {
            return Err(UsdmWsError::TooManyStreams {
                path: self.path,
                limit: MAX_STREAMS_PER_CONNECTION,
            });
        }
        for batch in new.chunks(MAX_NAMES_PER_REQUEST) {
            self.request("SUBSCRIBE", Some(batch)).await?;
            self.streams.extend(batch.iter().cloned());
        }
        Ok(())
    }

    /// Unsubscribe from streams. A stream not subscribed is skipped.
    pub async fn unsubscribe(&mut self, streams: &[StreamName]) -> Result<(), UsdmWsError> {
        let mut gone: Vec<StreamName> = Vec::new();
        for stream in streams {
            if self.streams.contains(stream) && !gone.contains(stream) {
                gone.push(stream.clone());
            }
        }
        for batch in gone.chunks(MAX_NAMES_PER_REQUEST) {
            self.request("UNSUBSCRIBE", Some(batch)).await?;
            self.streams.retain(|s| !batch.contains(s));
        }
        Ok(())
    }

    /// The stream names the server says this connection carries.
    pub async fn list_subscriptions(&mut self) -> Result<Vec<String>, UsdmWsError> {
        let id = self.next_id;
        let result = self.request("LIST_SUBSCRIPTIONS", None).await?;
        serde_json::from_value(result.clone()).map_err(|_| UsdmWsError::Response {
            id,
            raw: result.to_string(),
        })
    }

    /// Send a protocol ping and wait for the pong, returning the round trip.
    pub async fn ping(&mut self) -> Result<Duration, UsdmWsError> {
        let start = Instant::now();
        self.send_ping().await?;
        tokio::time::timeout(self.answer_timeout, self.await_pong())
            .await
            .map_err(|_| UsdmWsError::NoAnswer {
                id: 0,
                timeout: self.answer_timeout,
            })??;
        Ok(start.elapsed())
    }

    /// Close the connection.
    pub async fn close(&mut self) -> Result<(), UsdmWsError> {
        self.inner.close(None).await?;
        Ok(())
    }

    /// The streams this connection carries.
    pub fn streams(&self) -> &[StreamName] {
        &self.streams
    }

    /// The path this connection is on.
    pub fn path(&self) -> StreamPath {
        self.path
    }

    /// Send a protocol ping without waiting; the pong arrives as any inbound
    /// frame does and refreshes [`last_inbound`](Self::last_inbound).
    pub(crate) async fn send_ping(&mut self) -> Result<(), UsdmWsError> {
        self.inner.send(Message::Ping(Vec::new().into())).await?;
        Ok(())
    }

    /// When the last frame of any kind arrived, pings and pongs included.
    pub(crate) fn last_inbound(&self) -> Instant {
        self.last_inbound
    }

    /// The server's close code and reason, once it has closed the connection.
    pub(crate) fn close_frame(&self) -> Option<(Option<u16>, String)> {
        self.close_frame.clone()
    }

    fn record_close(&mut self, frame: Option<CloseFrame>) {
        self.close_frame = Some(match frame {
            Some(frame) => (Some(u16::from(frame.code)), frame.reason.to_string()),
            None => (None, String::new()),
        });
    }

    fn closed_error(&self) -> UsdmWsError {
        let (code, reason) = self.close_frame.clone().unwrap_or((None, String::new()));
        UsdmWsError::Closed { code, reason }
    }

    /// Send one request, paced, and wait for its answer's `result`.
    async fn request(
        &mut self,
        method: &str,
        params: Option<&[StreamName]>,
    ) -> Result<Value, UsdmWsError> {
        // A connection the server closed reports its close code, not a send
        // error.
        if self.close_frame.is_some() {
            return Err(self.closed_error());
        }
        if let Some(last) = self.last_request {
            tokio::time::sleep_until(last + MIN_REQUEST_INTERVAL).await;
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut body = json!({ "method": method, "id": id });
        if let Some(params) = params {
            let names: Vec<String> = params.iter().map(ToString::to_string).collect();
            body["params"] = json!(names);
        }
        self.inner
            .send(Message::Text(body.to_string().into()))
            .await?;
        self.last_request = Some(Instant::now());
        tokio::time::timeout(self.answer_timeout, self.await_answer(id))
            .await
            .map_err(|_| UsdmWsError::NoAnswer {
                id,
                timeout: self.answer_timeout,
            })?
    }

    /// Read until the answer to `id`, keeping updates read meanwhile.
    async fn await_answer(&mut self, id: u64) -> Result<Value, UsdmWsError> {
        loop {
            let message = match self.inner.next().await {
                None => return Err(self.closed_error()),
                Some(Err(err)) => return Err(err.into()),
                Some(Ok(message)) => message,
            };
            self.last_inbound = Instant::now();
            match message {
                Message::Text(text) => match Answer::parse(&text) {
                    Some(answer) if answer.id() == Some(id) => {
                        return match answer {
                            Answer::Result { result, .. } => Ok(result),
                            Answer::Error { code, msg, .. } => {
                                Err(UsdmWsError::Refused { code, msg })
                            }
                        };
                    }
                    Some(other) => {
                        tracing::debug!(id = ?other.id(), "uncorrelated answer while awaiting {id}");
                    }
                    None => self.buffered.push_back(Update::from_json(&text)),
                },
                Message::Close(frame) => {
                    self.record_close(frame);
                    return Err(self.closed_error());
                }
                _ => {}
            }
        }
    }

    /// Read until a pong, keeping updates read meanwhile.
    async fn await_pong(&mut self) -> Result<(), UsdmWsError> {
        loop {
            let message = match self.inner.next().await {
                None => return Err(self.closed_error()),
                Some(Err(err)) => return Err(err.into()),
                Some(Ok(message)) => message,
            };
            self.last_inbound = Instant::now();
            match message {
                Message::Pong(_) => return Ok(()),
                Message::Text(text) if Answer::parse(&text).is_none() => {
                    self.buffered.push_back(Update::from_json(&text));
                }
                Message::Close(frame) => {
                    self.record_close(frame);
                    return Err(self.closed_error());
                }
                _ => {}
            }
        }
    }
}

impl Stream for UsdmWs {
    type Item = Result<Update, UsdmWsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(kept) = self.buffered.pop_front() {
            return Poll::Ready(Some(kept));
        }
        loop {
            let message = match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(message))) => message,
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err.into()))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            };
            self.last_inbound = Instant::now();
            match message {
                Message::Text(text) => match Update::from_json(&text) {
                    Ok(update) => return Poll::Ready(Some(Ok(update))),
                    // An answer nobody is waiting for, such as one to a
                    // request whose caller gave up.
                    Err(_) if Answer::parse(&text).is_some() => continue,
                    Err(err) => return Poll::Ready(Some(Err(err))),
                },
                Message::Close(frame) => {
                    tracing::debug!(?frame, "binance stream closed by the server");
                    self.record_close(frame);
                    return Poll::Ready(None);
                }
                // Pings are answered by tungstenite while it reads; pongs only
                // refresh `last_inbound`.
                _ => continue,
            }
        }
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
            stream::{DepthLevels, DepthSpeed},
            test_server::{Script, ScriptedServer},
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
