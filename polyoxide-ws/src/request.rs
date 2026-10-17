//! Request/answer correlation on one connection (W12).

use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

use crate::{Connection, WsError};

/// What a frame read while a request awaits its answer is, as the caller's
/// matcher reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Correlated<A> {
    /// The answer awaited, which ends the wait.
    Answer(A),
    /// An answer to another request, which nobody awaits any more, such as
    /// one that arrived after its request timed out. Dropped.
    Uncorrelated,
    /// Not an answer: kept, and yielded by the stream afterwards, if its
    /// [`Mode`](crate::Mode) yields it.
    Push,
}

/// A connection's request ids and pacing.
#[derive(Debug)]
pub(crate) struct Requests {
    next_id: u64,
    min_interval: Option<Duration>,
    answer_timeout: Option<Duration>,
    last_request: Option<Instant>,
}

impl Default for Requests {
    fn default() -> Self {
        Self {
            next_id: 1,
            min_interval: None,
            answer_timeout: None,
            last_request: None,
        }
    }
}

/// Requests and their answers.
///
/// Ids count from 1. A request is sent no sooner than its minimum interval
/// after the previous one, when one is set, and then reads until the caller's
/// matcher recognises its answer. Every other frame read meanwhile is kept and
/// yielded by the stream afterwards, in the order it arrived; an answer to
/// another request is dropped. With an answer timeout set, a request whose
/// send and wait together outlast it fails as
/// [`Kind::NoAnswer`](crate::Kind::NoAnswer), so a peer that stops reading
/// cannot hang it; without one, it waits as long as the connection stays
/// open. A server close while waiting fails the request with the close, as
/// does any request after it, and a request after a failed read fails as
/// [`Kind::AlreadyClosed`](crate::Kind::AlreadyClosed) without sending.
///
/// The envelope is the caller's: a builder writes the request for its id, and
/// a matcher reads each frame. Neither request nor its wait is cancel-safe,
/// and after a [`NoAnswer`](crate::Kind::NoAnswer) the server's state is
/// unknown: replace the connection rather than reuse it.
///
/// The basis is one socket crate's former `request` and `await_answer`
/// (its bare tier's, on this correlation since Story 4.7), with its 10 s
/// answer timeout and 200 ms pacing, and another socket crate's former
/// `take_id` and `await_response` (`ws/client.rs`, Story 4.6), which set
/// neither.
impl Connection {
    /// Send requests no sooner than `interval` apart. Only
    /// [`request`](Self::request) is paced: a frame sent with
    /// [`send`](Self::send), under an id from [`take_id`](Self::take_id) or
    /// not, neither waits nor counts.
    pub fn min_request_interval(mut self, interval: Duration) -> Self {
        self.requests.min_interval = Some(interval);
        self
    }

    /// Give up on a request after `timeout`, its send and its wait for the
    /// answer together, and on [`await_answer`](Self::await_answer)'s wait.
    pub fn answer_timeout(mut self, timeout: Duration) -> Self {
        self.requests.answer_timeout = Some(timeout);
        self
    }

    /// The id the next request will take.
    pub fn next_id(&self) -> u64 {
        self.requests.next_id
    }

    /// Take the next id, for a request sent another way. A request sent that
    /// way is not paced by [`min_request_interval`](Self::min_request_interval),
    /// and does not delay the next paced one.
    pub fn take_id(&mut self) -> u64 {
        let id = self.requests.next_id;
        self.requests.next_id += 1;
        id
    }

    /// Send the request `build` writes for the next id, and read until
    /// `answer` recognises its answer.
    pub async fn request<A>(
        &mut self,
        build: impl FnOnce(u64) -> Message,
        mut answer: impl FnMut(u64, &Message) -> Correlated<A>,
    ) -> Result<A, WsError> {
        // A connection the server closed reports its close, not a send error,
        // and one whose read failed sends nothing.
        if let Some(closed) = self.closed() {
            return Err(closed);
        }
        if self.is_finished() {
            return Err(already_closed());
        }
        if let (Some(last), Some(interval)) =
            (self.requests.last_request, self.requests.min_interval)
        {
            // An interval too long to add is one never reached.
            if let Some(at) = last.checked_add(interval) {
                tokio::time::sleep_until(at).await;
            }
        }
        let id = self.take_id();
        let timeout = self.requests.answer_timeout;
        within(timeout, async {
            self.send(build(id)).await?;
            self.requests.last_request = Some(Instant::now());
            self.read_until(&mut |message: &Message| answer(id, message))
                .await
        })
        .await
    }

    /// Read until `answer` recognises a frame as the answer awaited, keeping
    /// pushes for the stream, within the answer timeout when one is set.
    ///
    /// For an answer to something [`request`](Self::request) did not send,
    /// such as a protocol ping's pong.
    pub async fn await_answer<A>(
        &mut self,
        mut answer: impl FnMut(&Message) -> Correlated<A>,
    ) -> Result<A, WsError> {
        let timeout = self.requests.answer_timeout;
        within(timeout, self.read_until(&mut answer)).await
    }

    async fn read_until<A>(
        &mut self,
        answer: &mut impl FnMut(&Message) -> Correlated<A>,
    ) -> Result<A, WsError> {
        loop {
            if let Some(closed) = self.closed() {
                return Err(closed);
            }
            if self.is_finished() {
                return Err(already_closed());
            }
            let message = match self.socket().next().await {
                Some(Ok(message)) => message,
                Some(Err(err)) => {
                    self.fail();
                    return Err(err.into());
                }
                None => {
                    self.record_close(None);
                    continue;
                }
            };
            self.touch();
            if let Message::Close(_) = message {
                self.record_close(Some(&message));
                self.keep(message);
                self.flush_reply().await;
                continue;
            }
            match answer(&message) {
                Correlated::Answer(answer) => return Ok(answer),
                Correlated::Uncorrelated => {
                    tracing::debug!(?message, "dropped an answer nobody awaits");
                }
                Correlated::Push => self.keep(message),
            }
        }
    }
}

/// Run `exchange` within `timeout`, if one is set, failing as `NoAnswer`.
async fn within<A>(
    timeout: Option<Duration>,
    exchange: impl Future<Output = Result<A, WsError>>,
) -> Result<A, WsError> {
    match timeout {
        Some(timeout) => tokio::time::timeout(timeout, exchange)
            .await
            .map_err(|_| WsError::no_answer(timeout))?,
        None => exchange.await,
    }
}

/// The error for a connection used after its read failed.
fn already_closed() -> WsError {
    WsError::from(tokio_tungstenite::tungstenite::Error::AlreadyClosed)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use tokio_tungstenite::tungstenite::protocol::{frame::coding::CloseCode, CloseFrame};

    use super::*;
    use crate::{
        connect,
        test_server::{next_item, Script, ScriptedServer},
        Kind, Mode, DEFAULT_CONNECT_TIMEOUT,
    };

    fn text(value: Value) -> Message {
        Message::Text(value.to_string().into())
    }

    fn parse(message: &Message) -> Option<Value> {
        match message {
            Message::Text(text) => serde_json::from_str(text).ok(),
            _ => None,
        }
    }

    /// A server that holds each connection and answers with `answer`, the
    /// same stateless answerer on every connection.
    async fn answering<A>(answer: A) -> ScriptedServer
    where
        A: FnMut(&str) -> Vec<Message> + Clone + Send + 'static,
    {
        ScriptedServer::start_answering(vec![Script::hold()], move |_| answer.clone()).await
    }

    async fn open(server: &ScriptedServer) -> Connection {
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect("connect");
        Connection::new(socket, Mode::Data)
    }

    async fn next_text(connection: &mut Connection) -> Value {
        let message = tokio::time::timeout(Duration::from_secs(2), connection.next())
            .await
            .expect("a frame within 2 s")
            .expect("the stream is open")
            .expect("not an error");
        parse(&message).expect("a JSON text frame")
    }

    /// One venue's envelope: `{"id", "req", "chs"}` answered by
    /// `{"id", "data"}`, with pushes `{"ch", "sq", "data"}`.
    mod perps {
        use super::*;

        pub(super) fn subscribe(channels: &'static [&'static str]) -> impl FnOnce(u64) -> Message {
            move |id| text(json!({"id": id, "req": "sub", "chs": channels}))
        }

        pub(super) fn answer(id: u64, message: &Message) -> Correlated<Value> {
            match parse(message) {
                Some(frame) if frame.get("ch").is_some() => Correlated::Push,
                Some(frame) if frame["id"] == id => Correlated::Answer(frame["data"].clone()),
                Some(frame) if frame.get("id").is_some() => Correlated::Uncorrelated,
                _ => Correlated::Push,
            }
        }

        pub(super) fn push(sq: u64) -> Message {
            text(json!({"ch": "bbo::1", "ts": 1, "sq": sq, "data": {}}))
        }

        pub(super) fn statuses(request: &str) -> Message {
            let request: Value = serde_json::from_str(request).unwrap();
            let ok: Vec<Value> = request["chs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|_| json!({"status": "ok"}))
                .collect();
            text(json!({"id": request["id"], "data": ok}))
        }
    }

    /// Another venue's envelope: `{"method", "params", "id"}` answered by
    /// `{"result", "id"}` or `{"error", "id"}`, with pushes `{"stream", "data"}`.
    mod binance {
        use super::*;

        pub(super) fn subscribe(name: &'static str) -> impl FnOnce(u64) -> Message {
            move |id| text(json!({"method": "SUBSCRIBE", "params": [name], "id": id}))
        }

        pub(super) fn answer(id: u64, message: &Message) -> Correlated<Result<Value, i64>> {
            match parse(message) {
                Some(frame) if frame.get("stream").is_some() => Correlated::Push,
                Some(frame) if frame["id"] == id => Correlated::Answer(match frame.get("error") {
                    Some(error) => Err(error["code"].as_i64().unwrap()),
                    None => Ok(frame["result"].clone()),
                }),
                Some(_) => Correlated::Uncorrelated,
                None => Correlated::Push,
            }
        }

        pub(super) fn push(n: u64) -> Message {
            text(json!({"stream": "btcusdt@aggTrade", "data": {"a": n}}))
        }

        pub(super) fn result(request: &str) -> Message {
            let request: Value = serde_json::from_str(request).unwrap();
            text(json!({"result": null, "id": request["id"]}))
        }
    }

    #[tokio::test]
    async fn ids_count_from_one() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut connection = open(&server).await;
        assert_eq!(connection.next_id(), 1);
        assert_eq!(connection.take_id(), 1);
        assert_eq!(connection.take_id(), 2);
        assert_eq!(connection.next_id(), 3);
    }

    #[tokio::test]
    async fn a_request_after_a_failed_read_sends_nothing() {
        let server = ScriptedServer::start(vec![Script::hold().then_cut()]).await;
        let mut connection = open(&server).await;
        assert!(next_item(&mut connection).await.is_err());
        let err = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect_err("the connection is gone");
        assert_eq!(err.kind(), Kind::AlreadyClosed);
        assert_eq!(connection.next_id(), 1, "no id was taken");
    }

    #[tokio::test]
    async fn an_interval_too_long_to_add_is_never_waited() {
        let server = answering(|request: &str| vec![binance::result(request)]).await;
        let mut connection = open(&server)
            .await
            .min_request_interval(Duration::MAX)
            .answer_timeout(Duration::from_secs(2));
        for name in ["a@aggTrade", "b@aggTrade"] {
            let result = connection
                .request(binance::subscribe(name), binance::answer)
                .await
                .expect("an answer, not a panic or a wait");
            assert_eq!(result, Ok(Value::Null));
        }
    }

    #[tokio::test]
    async fn a_request_refreshes_last_inbound() {
        let server = answering(|request: &str| vec![binance::result(request)]).await;
        let mut connection = open(&server).await;
        let before = connection.last_inbound();
        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect("an answer");
        assert_eq!(result, Ok(Value::Null));
        assert!(
            connection.last_inbound() > before,
            "the answer, read by the request, must count as inbound"
        );
    }

    #[tokio::test]
    async fn all_mode_yields_a_close_read_while_awaiting() {
        let server = answering(|_: &str| {
            vec![Message::Close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "Too many requests".into(),
            }))]
        })
        .await;
        let socket = connect(server.url.as_str(), DEFAULT_CONNECT_TIMEOUT)
            .await
            .expect("connect");
        let mut connection = Connection::new(socket, Mode::All);
        let err = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect_err("closed");
        assert!(
            matches!(
                err.kind(),
                Kind::Closed {
                    code: Some(1008),
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(matches!(
            next_item(&mut connection).await,
            Ok(Message::Close(Some(_)))
        ));
        assert!(connection.next().await.is_none());
    }

    #[tokio::test]
    async fn perps_pushes_read_before_the_answer_are_kept_and_yielded_in_order() {
        let server = answering(|request: &str| {
            vec![perps::push(10), perps::push(11), perps::statuses(request)]
        })
        .await;
        let mut connection = open(&server).await;
        let data = connection
            .request(perps::subscribe(&["bbo::1", "trades::1"]), perps::answer)
            .await
            .expect("an answer");
        assert_eq!(data, json!([{"status": "ok"}, {"status": "ok"}]));
        assert_eq!(next_text(&mut connection).await["sq"], 10);
        assert_eq!(next_text(&mut connection).await["sq"], 11);
    }

    #[tokio::test]
    async fn perps_an_answer_to_another_request_is_dropped() {
        let server = answering(|request: &str| {
            vec![
                text(json!({"id": 99, "data": {"status": "ok"}})),
                perps::statuses(request),
                perps::push(12),
            ]
        })
        .await;
        let mut connection = open(&server).await;
        connection
            .request(perps::subscribe(&["bbo::1"]), perps::answer)
            .await
            .expect("its own answer");
        assert_eq!(
            next_text(&mut connection).await["sq"],
            12,
            "the stray answer must not be yielded"
        );
    }

    #[tokio::test]
    async fn binance_requests_are_paced() {
        let interval = Duration::from_millis(50);
        let server = answering(|request: &str| vec![binance::result(request)]).await;
        let mut connection = open(&server).await.min_request_interval(interval);
        let start = Instant::now();
        for name in ["a@aggTrade", "b@aggTrade", "c@aggTrade"] {
            let result = connection
                .request(binance::subscribe(name), binance::answer)
                .await
                .expect("an answer");
            assert_eq!(result, Ok(Value::Null));
        }
        assert!(
            start.elapsed() >= interval * 2,
            "three requests in {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn binance_an_unanswered_request_is_no_answer_and_pushes_meanwhile_are_kept() {
        let after = Duration::from_millis(200);
        let server = answering(|_: &str| vec![binance::push(1), binance::push(2)]).await;
        let mut connection = open(&server).await.answer_timeout(after);
        let err = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect_err("nothing answers it");
        assert_eq!(err.kind(), Kind::NoAnswer { after });
        assert_eq!(next_text(&mut connection).await["data"]["a"], 1);
        assert_eq!(next_text(&mut connection).await["data"]["a"], 2);
    }

    #[tokio::test]
    async fn binance_a_late_answer_is_not_taken_for_the_next() {
        // Request 2's answer is held until request 3 arrives, by when request
        // 2 has timed out. Request 3 must skip it and take its own, and the
        // push sent ahead of both is kept.
        let server = ScriptedServer::start_answering(vec![Script::hold()], |_| {
            let mut held: Option<Message> = None;
            move |request: &str| {
                let value: Value = serde_json::from_str(request).unwrap();
                match value["id"].as_u64() {
                    Some(2) => {
                        held = Some(text(
                            json!({"error": {"code": 2, "msg": "Invalid request"}, "id": 2}),
                        ));
                        Vec::new()
                    }
                    Some(3) => vec![
                        binance::push(7),
                        held.take().expect("request 2's answer"),
                        binance::result(request),
                    ],
                    _ => vec![binance::result(request)],
                }
            }
        })
        .await;
        let mut connection = open(&server)
            .await
            .answer_timeout(Duration::from_millis(200));
        for (name, expected) in [
            ("a@aggTrade", Ok(Ok(Value::Null))),
            ("b@aggTrade", Err(())),
            ("c@aggTrade", Ok(Ok(Value::Null))),
        ] {
            let result = connection
                .request(binance::subscribe(name), binance::answer)
                .await
                .map_err(|err| assert!(matches!(err.kind(), Kind::NoAnswer { .. }), "{err:?}"));
            assert_eq!(result, expected, "{name}");
        }
        let push = tokio::time::timeout(Duration::from_secs(2), connection.next())
            .await
            .expect("the push read during request 3 was dropped");
        assert_eq!(parse(&push.unwrap().unwrap()).unwrap()["data"]["a"], 7);
    }

    #[tokio::test]
    async fn binance_an_error_answer_is_the_callers_to_read() {
        let server = answering(|request: &str| {
            let value: Value = serde_json::from_str(request).unwrap();
            vec![text(
                json!({"error": {"code": 2, "msg": "Invalid request"}, "id": value["id"]}),
            )]
        })
        .await;
        let mut connection = open(&server).await;
        let result = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect("an answer");
        assert_eq!(result, Err(2));
    }

    #[tokio::test]
    async fn a_protocol_ping_awaits_its_pong_without_taking_an_id() {
        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut connection = open(&server).await.answer_timeout(Duration::from_secs(2));
        connection
            .send(Message::Ping(Vec::new().into()))
            .await
            .expect("ping");
        connection
            .await_answer(|message| match message {
                Message::Pong(_) => Correlated::Answer(()),
                _ => Correlated::Push,
            })
            .await
            .expect("a pong");
        assert_eq!(connection.next_id(), 1);
    }

    #[tokio::test]
    async fn a_close_while_awaiting_is_answered() {
        // DRIFT R3 on the request path: the reply goes out though nothing
        // reads the connection again.
        let server = ScriptedServer::start(vec![Script::hold()
            .after_first_frame()
            .then_close()
            .close_with(1008, "Too many requests")])
        .await;
        let mut connection = open(&server).await;
        let err = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect_err("closed");
        assert!(
            matches!(
                err.kind(),
                Kind::Closed {
                    code: Some(1008),
                    ..
                }
            ),
            "{err:?}"
        );
        server
            .wait_for("the close reply", |s| s.close_reply_count() == 1)
            .await;
    }

    #[tokio::test]
    async fn a_close_while_awaiting_fails_the_request_and_every_later_one() {
        let server = answering(|_: &str| {
            vec![Message::Close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "Too many requests".into(),
            }))]
        })
        .await;
        let mut connection = open(&server).await;
        let closed = Kind::Closed {
            code: Some(1008),
            reason: "Too many requests",
        };
        let err = connection
            .request(binance::subscribe("a@aggTrade"), binance::answer)
            .await
            .expect_err("closed");
        assert_eq!(err.kind(), closed);
        let again = connection
            .request(binance::subscribe("b@aggTrade"), binance::answer)
            .await
            .expect_err("still closed");
        assert_eq!(again.kind(), closed);
        assert_eq!(connection.next_id(), 2, "nothing was sent after the close");
        assert!(connection.next().await.is_none());
    }
}
