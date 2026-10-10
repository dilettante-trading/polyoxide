//! A local WebSocket server that plays a script per connection, and the
//! helpers the offline suites share (W13, W15). Behind `test-server`; not for
//! consumers.
//!
//! The basis is the four venue servers: `polyoxide-sports`'
//! `src/test_server.rs`, `polyoxide-perps`' `ws/test_server.rs`,
//! `polyoxide-binance`'s `usdm/ws/test_server.rs` and `polyoxide-rtds`'
//! `src/test_server.rs`. Their shared skeleton is here: one script per
//! connection with the last repeating, every action and recorder any of them
//! has, and the interval idiom. What stays with each venue is its protocol's
//! answer, passed to [`ScriptedServer::start_answering`] as a closure with
//! per-connection state, and its fixtures.

mod helpers;

use std::{
    error::Error,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    time::{Instant, Interval},
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        http::{HeaderName, HeaderValue, StatusCode},
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
    WebSocketStream,
};

pub use helpers::{drain, fast, next_event, next_item, WINDOW};

/// How long [`ScriptedServer::wait_for`] polls before it panics.
const WAIT_FOR: Duration = Duration::from_secs(5);
/// How long the server waits for the client's reply to its own close.
const CLOSE_REPLY_WAIT: Duration = Duration::from_secs(1);

/// How the server behaves on one connection.
///
/// By default it completes the handshake, sends [`send`](Self::send) at once,
/// and then holds the connection open, recording what the client sends and
/// answering its text frames, until the client goes.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Accept the TCP connection and drop it without a handshake.
    pub reject: bool,
    /// Accept the TCP connection and never answer the handshake.
    pub stall: bool,
    /// Answer the WebSocket upgrade with this HTTP status instead of `101`.
    /// It must not be a 2xx, which the handshake rejects as a malformed
    /// refusal.
    pub refuse_upgrade: Option<u16>,
    /// Headers to send with [`refuse_upgrade`](Self::refuse_upgrade)'s
    /// status, such as `Retry-After`.
    pub refuse_headers: Vec<(String, String)>,
    /// Read the client's first text frame, record it and answer it, before
    /// sending anything. A test that has seen a scripted frame then knows the
    /// first frame is recorded.
    pub first_frame_first: bool,
    /// Frames to send, in order: at once, or after the first text frame with
    /// [`first_frame_first`](Self::first_frame_first).
    pub send: Vec<Message>,
    /// While the connection is held, resend the first of `send` on this
    /// cadence, starting at once: steady traffic.
    pub push_every: Option<Duration>,
    /// While the connection is held, send a protocol ping on this cadence,
    /// the first one period in, each carrying its sequence number from 1 as
    /// four big-endian bytes.
    pub ping_every: Option<Duration>,
    /// Close after sending `send`, then wait a second for the client's reply.
    pub close_after: bool,
    /// The close code and reason [`close_after`](Self::close_after) and
    /// [`drop_after`](Self::drop_after) send; no code when `None`.
    pub close_code: Option<(u16, String)>,
    /// Close this long after the handshake, whatever else is happening.
    pub drop_after: Option<Duration>,
    /// After sending `send`, stop reading and writing: a dead socket that
    /// answers neither requests nor pings. Only [`drop_after`](Self::drop_after)
    /// still closes it.
    pub go_silent: bool,
    /// After sending `send`, drop the TCP connection without a close frame:
    /// a connection reset.
    pub cut: bool,
}

impl Script {
    /// Hold the connection open, send nothing, and record what the client
    /// sends.
    pub fn hold() -> Self {
        Self::default()
    }

    /// Send these text frames, then hold the connection open.
    pub fn frames(frames: &[&str]) -> Self {
        Self::messages(
            frames
                .iter()
                .map(|frame| Message::Text((*frame).into()))
                .collect(),
        )
    }

    /// Send these frames, of any kind, then hold the connection open.
    pub fn messages(send: Vec<Message>) -> Self {
        Self {
            send,
            ..Self::default()
        }
    }

    /// Hold the connection open, sending only protocol pings.
    pub fn pings_only(every: Duration) -> Self {
        Self::hold().ping_every(every)
    }

    /// Complete the handshake, then close at once.
    pub fn close_at_once() -> Self {
        Self::hold().then_close()
    }

    /// Drop the TCP connection without a handshake.
    pub fn reject() -> Self {
        Self {
            reject: true,
            ..Self::default()
        }
    }

    /// Accept the TCP connection and never answer the handshake.
    pub fn stall() -> Self {
        Self {
            stall: true,
            ..Self::default()
        }
    }

    /// Answer the WebSocket upgrade with an HTTP status, such as `404`, as a
    /// server that refuses the request does.
    pub fn refuse_upgrade(status: u16) -> Self {
        Self {
            refuse_upgrade: Some(status),
            ..Self::default()
        }
    }

    /// Send this header with the refused upgrade.
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.refuse_headers
            .push((name.to_owned(), value.to_owned()));
        self
    }

    /// Wait for the client's first text frame before sending.
    pub fn after_first_frame(mut self) -> Self {
        self.first_frame_first = true;
        self
    }

    /// Close after sending, instead of holding the connection open.
    pub fn then_close(mut self) -> Self {
        self.close_after = true;
        self
    }

    /// Close, when closing, with this code and reason.
    pub fn close_with(mut self, code: u16, reason: &str) -> Self {
        self.close_code = Some((code, reason.to_owned()));
        self
    }

    /// Resend the first frame of `send` on this cadence.
    pub fn push_every(mut self, every: Duration) -> Self {
        self.push_every = Some(every);
        self
    }

    /// Send a protocol ping on this cadence.
    pub fn ping_every(mut self, every: Duration) -> Self {
        self.ping_every = Some(every);
        self
    }

    /// Close this long after the handshake.
    pub fn drop_after(mut self, after: Duration) -> Self {
        self.drop_after = Some(after);
        self
    }

    /// Fall silent for good after sending.
    pub fn go_silent(mut self) -> Self {
        self.go_silent = true;
        self
    }

    /// Drop the TCP connection without a close frame after sending.
    pub fn then_cut(mut self) -> Self {
        self.cut = true;
        self
    }
}

/// One text frame the client sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Received {
    /// Which connection, counted from zero.
    pub connection: usize,
    /// When it arrived.
    pub at: Instant,
    /// The frame.
    pub text: String,
}

#[derive(Default)]
struct Recorder {
    accepted_at: Mutex<Vec<Instant>>,
    handshakes: AtomicUsize,
    paths: Mutex<Vec<String>>,
    received: Mutex<Vec<Received>>,
    first_frames: Mutex<Vec<String>>,
    pongs: Mutex<Vec<Vec<u8>>>,
    pings: AtomicUsize,
    closes: AtomicUsize,
    close_replies: AtomicUsize,
    client_ended: AtomicUsize,
    ended: AtomicUsize,
}

/// A running local server.
pub struct ScriptedServer {
    /// The `ws://` URL to connect to; any path is accepted.
    pub url: String,
    recorder: Arc<Recorder>,
}

/// Answers nothing.
fn no_answer(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send {
    |_| Vec::new()
}

impl ScriptedServer {
    /// Start a server that applies `scripts[n]` to the n-th connection,
    /// repeating the last script for every connection after it, and answers
    /// nothing.
    pub async fn start(scripts: Vec<Script>) -> Self {
        Self::start_answering(scripts, no_answer).await
    }

    /// Start a server as [`start`](Self::start) does, answering each text
    /// frame the client sends with the frames a protocol's answer returns.
    ///
    /// `answer` is called once per connection with the connection's index,
    /// counted from zero, and returns that connection's answerer, which owns
    /// whatever state the protocol keeps per connection: the streams
    /// subscribed, a request held back, how many were answered.
    pub async fn start_answering<F, A>(scripts: Vec<Script>, answer: F) -> Self
    where
        F: Fn(usize) -> A + Send + 'static,
        A: FnMut(&str) -> Vec<Message> + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("ws://{}", listener.local_addr().expect("local address"));
        let recorder = Arc::new(Recorder::default());
        let shared = Arc::clone(&recorder);
        tokio::spawn(async move {
            let mut index = 0;
            while let Ok((stream, _)) = listener.accept().await {
                shared.accepted_at.lock().unwrap().push(Instant::now());
                let script = scripts
                    .get(index)
                    .or_else(|| scripts.last())
                    .cloned()
                    .unwrap_or_default();
                let answerer = answer(index);
                let recorder = Arc::clone(&shared);
                tokio::spawn(async move {
                    let _ = serve(stream, index, script, answerer, &recorder).await;
                    recorder.ended.fetch_add(1, Ordering::SeqCst);
                });
                index += 1;
            }
        });
        Self { url, recorder }
    }

    /// Connections accepted, counted at TCP accept, before any handshake.
    ///
    /// It can run ahead of everything recorded inside a connection: the
    /// frames arrive later, in that connection's task. Use
    /// [`wait_for`](Self::wait_for) on the later observation to order them.
    pub fn connection_count(&self) -> usize {
        self.recorder.accepted_at.lock().unwrap().len()
    }

    /// When each connection was accepted, in order.
    pub fn accepted_at(&self) -> Vec<Instant> {
        self.recorder.accepted_at.lock().unwrap().clone()
    }

    /// Handshakes completed.
    pub fn handshake_count(&self) -> usize {
        self.recorder.handshakes.load(Ordering::SeqCst)
    }

    /// The request path of each connection that completed a handshake.
    pub fn paths(&self) -> Vec<String> {
        self.recorder.paths.lock().unwrap().clone()
    }

    /// Every text frame received, in arrival order, across connections.
    pub fn received(&self) -> Vec<Received> {
        self.recorder.received.lock().unwrap().clone()
    }

    /// The text of every text frame received, in arrival order, across
    /// connections: [`received`](Self::received) without the connection and
    /// the time.
    pub fn client_frames(&self) -> Vec<String> {
        self.received().into_iter().map(|r| r.text).collect()
    }

    /// The first text frame of each connection, such as its subscription, in
    /// order.
    pub fn first_frames(&self) -> Vec<String> {
        self.recorder.first_frames.lock().unwrap().clone()
    }

    /// How many text frames received match `predicate`, such as the
    /// application pings a client sends.
    pub fn count_frames(&self, predicate: impl Fn(&str) -> bool) -> usize {
        self.recorder
            .received
            .lock()
            .unwrap()
            .iter()
            .filter(|r| predicate(&r.text))
            .count()
    }

    /// The payload of every pong received, in order, across connections.
    pub fn pongs(&self) -> Vec<Vec<u8>> {
        self.recorder.pongs.lock().unwrap().clone()
    }

    /// Protocol pings received from the client.
    pub fn ping_count(&self) -> usize {
        self.recorder.pings.load(Ordering::SeqCst)
    }

    /// Held connections the client closed with a close frame of its own.
    pub fn close_count(&self) -> usize {
        self.recorder.closes.load(Ordering::SeqCst)
    }

    /// Close frames the client sent in reply to the server's own.
    pub fn close_reply_count(&self) -> usize {
        self.recorder.close_replies.load(Ordering::SeqCst)
    }

    /// Held connections the client ended, by a close frame, the end of the
    /// stream, or a read error.
    pub fn client_ended_count(&self) -> usize {
        self.recorder.client_ended.load(Ordering::SeqCst)
    }

    /// Connections that have ended, however they ended.
    pub fn ended_count(&self) -> usize {
        self.recorder.ended.load(Ordering::SeqCst)
    }

    /// Poll until `predicate` holds, or panic naming `label` after five
    /// seconds.
    ///
    /// Every observation lands asynchronously in a per-connection task, so a
    /// bare assert after an action races that task, and a fixed sleep trades
    /// the race for a slow test that still flakes.
    pub async fn wait_for(&self, label: &str, predicate: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + WAIT_FOR;
        while Instant::now() < deadline {
            if predicate(self) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("timed out waiting for {label}");
    }
}

type Socket = WebSocketStream<TcpStream>;
type Failure = Box<dyn Error + Send + Sync>;

/// Wait for the next tick of an optional interval, or for ever.
async fn tick(interval: &mut Option<Interval>) {
    match interval {
        Some(interval) => {
            interval.tick().await;
        }
        None => std::future::pending().await,
    }
}

/// Wait until an optional deadline, or for ever.
async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// What one connection records as the client's frames arrive.
struct Tape<'a> {
    recorder: &'a Recorder,
    connection: usize,
    first: bool,
}

impl Tape<'_> {
    /// Record a frame the client sent: a text frame, a ping or a pong.
    fn record(&mut self, message: &Message) {
        match message {
            Message::Text(text) => {
                let text = text.to_string();
                if self.first {
                    self.recorder
                        .first_frames
                        .lock()
                        .unwrap()
                        .push(text.clone());
                    self.first = false;
                }
                self.recorder.received.lock().unwrap().push(Received {
                    connection: self.connection,
                    at: Instant::now(),
                    text,
                });
            }
            Message::Ping(_) => {
                self.recorder.pings.fetch_add(1, Ordering::SeqCst);
            }
            Message::Pong(payload) => {
                self.recorder.pongs.lock().unwrap().push(payload.to_vec());
            }
            _ => {}
        }
    }
}

/// Close with `frame`, and count the client's reply if it comes within a
/// second. Frames the client sent before it read the close arrive ahead of
/// its reply, and are recorded as any other.
async fn close(ws: &mut Socket, frame: Option<CloseFrame>, tape: &mut Tape<'_>) {
    if ws.close(frame).await.is_err() {
        return;
    }
    let deadline = Instant::now() + CLOSE_REPLY_WAIT;
    while let Ok(Some(Ok(message))) = tokio::time::timeout_at(deadline, ws.next()).await {
        if let Message::Close(_) = message {
            tape.recorder.close_replies.fetch_add(1, Ordering::SeqCst);
            return;
        }
        tape.record(&message);
    }
}

/// Send the script's frames, then close, cut or fall silent if it says to.
/// Whether the connection is still held.
async fn send_scripted(
    ws: &mut Socket,
    script: &Script,
    close_frame: Option<CloseFrame>,
    drop_at: Option<Instant>,
    tape: &mut Tape<'_>,
) -> Result<bool, Failure> {
    for message in &script.send {
        ws.send(message.clone()).await?;
    }
    if script.cut {
        // Dropping the TCP stream under the socket ends it with no close
        // frame; the caller drops the socket on `false`.
        return Ok(false);
    }
    if script.close_after {
        close(ws, close_frame, tape).await;
        return Ok(false);
    }
    if script.go_silent {
        // Hold the socket without reading or writing it, so nothing is
        // answered, pings included, until `drop_after`, if it is set.
        until(drop_at).await;
        close(ws, close_frame, tape).await;
        return Ok(false);
    }
    Ok(true)
}

/// Complete the handshake, recording the request's path, or refuse it.
async fn handshake(stream: TcpStream, script: &Script) -> Option<(Socket, String)> {
    let path = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&path);
    let refusal = script.refuse_upgrade.map(|status| {
        let headers = script.refuse_headers.clone();
        (status, headers)
    });
    // The return type is tungstenite's `Callback` contract; it cannot be
    // boxed.
    #[allow(clippy::result_large_err)]
    let callback =
        move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            *seen.lock().unwrap() = request.uri().path().to_owned();
            let Some((status, headers)) = refusal else {
                return Ok(response);
            };
            let mut refused = ErrorResponse::new(None);
            *refused.status_mut() = StatusCode::from_u16(status).expect("an HTTP status");
            for (name, value) in headers {
                refused.headers_mut().insert(
                    HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
                    HeaderValue::from_str(&value).expect("a header value"),
                );
            }
            Err(refused)
        };
    // A refusal writes its response, then reports the handshake as failed.
    let ws = accept_hdr_async(stream, callback).await.ok()?;
    let path = path.lock().unwrap().clone();
    Some((ws, path))
}

async fn serve<A>(
    stream: TcpStream,
    index: usize,
    script: Script,
    mut answer: A,
    recorder: &Recorder,
) -> Result<(), Failure>
where
    A: FnMut(&str) -> Vec<Message>,
{
    if script.reject {
        drop(stream);
        return Ok(());
    }
    if script.stall {
        // Hold the socket without reading it until the test ends.
        let _held = stream;
        std::future::pending::<()>().await;
        return Ok(());
    }
    let Some((mut ws, path)) = handshake(stream, &script).await else {
        return Ok(());
    };
    recorder.handshakes.fetch_add(1, Ordering::SeqCst);
    recorder.paths.lock().unwrap().push(path);

    let close_frame = script.close_code.clone().map(|(code, reason)| CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    });
    let drop_at = script.drop_after.map(|after| Instant::now() + after);
    let mut tape = Tape {
        recorder,
        connection: index,
        first: true,
    };
    let mut sent = false;
    if !script.first_frame_first {
        if !send_scripted(&mut ws, &script, close_frame.clone(), drop_at, &mut tape).await? {
            return Ok(());
        }
        sent = true;
    }
    let mut pushes = sent
        .then(|| script.push_every.map(tokio::time::interval))
        .flatten();
    let mut pings = script
        .ping_every
        .map(|every| tokio::time::interval_at(Instant::now() + every, every));
    let mut sequence: u32 = 0;

    loop {
        tokio::select! {
            () = until(drop_at) => {
                close(&mut ws, close_frame.clone(), &mut tape).await;
                return Ok(());
            }
            () = tick(&mut pings) => {
                sequence += 1;
                ws.send(Message::Ping(sequence.to_be_bytes().to_vec().into())).await?;
            }
            () = tick(&mut pushes) => {
                if let Some(push) = script.send.first() {
                    ws.send(push.clone()).await?;
                }
            }
            message = ws.next() => match message {
                Some(Ok(message)) => {
                    // Recorded before anything is answered or sent, so a test
                    // that has seen the server's reply knows the frame is
                    // recorded.
                    tape.record(&message);
                    match message {
                        Message::Text(text) => {
                            for reply in answer(text.as_str()) {
                                ws.send(reply).await?;
                            }
                            if !sent {
                                sent = true;
                                if !send_scripted(&mut ws, &script, close_frame.clone(), drop_at, &mut tape)
                                    .await?
                                {
                                    return Ok(());
                                }
                                pushes = script.push_every.map(tokio::time::interval);
                            }
                        }
                        Message::Close(_) => {
                            recorder.closes.fetch_add(1, Ordering::SeqCst);
                            recorder.client_ended.fetch_add(1, Ordering::SeqCst);
                            // tungstenite queued the reply as it read the
                            // close; send it before the socket goes.
                            let _ = ws.flush().await;
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                Some(Err(_)) | None => {
                    recorder.client_ended.fetch_add(1, Ordering::SeqCst);
                    return Ok(());
                }
            },
        }
    }
}

#[cfg(test)]
mod tests;
