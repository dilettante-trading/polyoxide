//! The supervisor on an injected runtime and an in-memory socket transport: a
//! connect, a replay and a reconnect with no TCP and no TLS.

use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Sink, Stream, StreamExt};
use polyoxide_rt::{BoxFuture, Instant, Runtime, TokioRuntime};
use polyoxide_ws::{
    tungstenite::{self, http, Message},
    BoxSocket, Capabilities, Connection, Decoded, Kind, Mode, Outage, Protocol, Queue,
    SocketTransport, Supervised, Supervisor, Timing, WsError,
};
use tokio::sync::mpsc;

/// A runtime that counts what the supervisor asks of it, and runs it on
/// tokio's.
#[derive(Debug, Default)]
struct CountingRuntime {
    inner: TokioRuntime,
    spawns: AtomicUsize,
    sleeps: AtomicUsize,
    /// How long each sleep asked for, in order.
    slept: Mutex<Vec<Duration>>,
}

impl Runtime for CountingRuntime {
    fn spawn(&self, task: BoxFuture<()>) {
        self.spawns.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.sleeps.fetch_add(1, Ordering::SeqCst);
        self.slept.lock().unwrap().push(duration);
        self.inner.sleep(duration)
    }

    fn yield_now(&self) -> BoxFuture<()> {
        Box::pin(tokio::task::yield_now())
    }

    fn now(&self) -> Instant {
        self.inner.now()
    }
}

/// The server's end of one in-memory connection.
struct Peer {
    to_client: mpsc::UnboundedSender<Message>,
    from_client: mpsc::UnboundedReceiver<Message>,
}

/// The client's end: frames the peer sent come out of the stream, and frames
/// sent into the sink go to the peer.
struct MemorySocket {
    incoming: mpsc::UnboundedReceiver<Message>,
    outgoing: mpsc::UnboundedSender<Message>,
}

impl Stream for MemorySocket {
    type Item = Result<Message, tungstenite::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.incoming.poll_recv(cx).map(|message| message.map(Ok))
    }
}

impl Sink<Message> for MemorySocket {
    type Error = tungstenite::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.outgoing
            .send(item)
            .map_err(|_| tungstenite::Error::ConnectionClosed)
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Opens an in-memory connection for each dial and hands its server end to the
/// test, recording what it was asked to dial.
#[derive(Debug, Clone)]
struct MemoryTransport {
    peers: mpsc::UnboundedSender<Peer>,
    dials: Arc<Mutex<Vec<(String, Duration)>>>,
}

impl SocketTransport for MemoryTransport {
    fn connect(
        &self,
        request: http::Request<()>,
        timeout: Duration,
        _runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        self.dials
            .lock()
            .unwrap()
            .push((request.uri().to_string(), timeout));
        let (to_client, incoming) = mpsc::unbounded_channel();
        let (outgoing, from_client) = mpsc::unbounded_channel();
        let peers = self.peers.clone();
        Box::pin(async move {
            peers
                .send(Peer {
                    to_client,
                    from_client,
                })
                .map_err(|_| WsError::from(tungstenite::Error::ConnectionClosed))?;
            Ok::<BoxSocket, WsError>(Box::new(MemorySocket { incoming, outgoing }))
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::FULL
    }
}

/// A transport whose every dial times out.
#[derive(Debug)]
struct Refusing;

impl SocketTransport for Refusing {
    fn connect(
        &self,
        _request: http::Request<()>,
        _timeout: Duration,
        _runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        Box::pin(async {
            Err::<BoxSocket, WsError>(WsError::connect_timeout(Duration::from_secs(7)))
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(false, false, false)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Data(String),
    Disconnected,
    Reconnected,
}

#[derive(Debug, thiserror::Error)]
enum FeedError {
    #[error(transparent)]
    Socket(#[from] WsError),
}

/// A feed that subscribes with one text frame per topic, treats every text
/// frame as data and sends no pings.
struct Feed;

impl Protocol for Feed {
    type Event = Event;
    type Error = FeedError;
    type Membership = Vec<String>;
    type Change = ();
    type Ack = ();

    fn handshake(&mut self) -> Result<http::Request<()>, FeedError> {
        Ok(http::Request::builder()
            .uri("ws://memory.invalid/feed")
            .body(())
            .expect("a valid request"))
    }

    async fn replay(
        &mut self,
        connection: &mut Connection,
        topics: &Vec<String>,
    ) -> Result<(), FeedError> {
        for topic in topics {
            connection
                .send(Message::Text(format!("sub:{topic}").into()))
                .await?;
        }
        Ok(())
    }

    async fn apply(
        &mut self,
        _connection: &mut Connection,
        _topics: &mut Vec<String>,
        _change: (),
    ) -> Result<(), FeedError> {
        Ok(())
    }

    fn decode(&mut self, frame: &Message) -> Decoded<Event, FeedError> {
        match frame {
            Message::Text(text) => Decoded::Event(Event::Data(text.to_string())),
            _ => Decoded::Nothing,
        }
    }

    fn is_live(&self, _frame: &Message, _decoded: &Decoded<Event, FeedError>) -> bool {
        true
    }

    fn delivered(&self, _frame: &Message, decoded: &Decoded<Event, FeedError>) -> bool {
        matches!(decoded, Decoded::Event(_))
    }

    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        None
    }

    fn is_retriable(_err: &FeedError) -> bool {
        true
    }

    fn disconnected(&mut self, _outage: Outage<FeedError>) -> Option<Event> {
        Some(Event::Disconnected)
    }

    fn reconnected(&mut self) -> Event {
        Event::Reconnected
    }

    fn queue(&self) -> Queue {
        Queue::Unbounded
    }
}

fn timing() -> Timing {
    let mut timing = Timing::default();
    timing.ping_interval = None;
    timing.stale_after = Duration::from_secs(5);
    timing.initial_backoff = Duration::from_millis(5);
    timing.max_backoff = Duration::from_millis(20);
    timing.connect_timeout = Duration::from_secs(7);
    timing
}

async fn next(ws: &mut Supervised<Feed>) -> Event {
    tokio::time::timeout(Duration::from_secs(5), ws.next())
        .await
        .expect("an event within 5 s")
        .expect("the stream is open")
        .expect("not an error")
}

#[tokio::test]
async fn a_supervisor_connects_replays_and_reconnects_over_an_injected_transport() {
    let (peers, mut accepted) = mpsc::unbounded_channel();
    let dials = Arc::new(Mutex::new(Vec::new()));
    let transport = MemoryTransport {
        peers,
        dials: Arc::clone(&dials),
    };
    let runtime = Arc::new(CountingRuntime::default());

    let mut ws = Supervisor::new(Feed, timing())
        .runtime(runtime.clone())
        .socket_transport(Arc::new(transport))
        .connect(vec!["a".to_owned()])
        .await
        .expect("the first attempt connects over the injected transport");

    let mut first = accepted.recv().await.expect("the first dial");
    assert_eq!(
        first.from_client.recv().await,
        Some(Message::Text("sub:a".into())),
        "the membership is replayed on connect"
    );
    first.to_client.send(Message::Text("one".into())).unwrap();
    assert_eq!(next(&mut ws).await, Event::Data("one".into()));

    // The server closes: an outage, a second dial and a second replay.
    first.to_client.send(Message::Close(None)).unwrap();
    assert_eq!(next(&mut ws).await, Event::Disconnected);
    let mut second = accepted.recv().await.expect("the reconnect dials again");
    assert_eq!(
        second.from_client.recv().await,
        Some(Message::Text("sub:a".into())),
        "the membership is replayed on the reconnect"
    );
    assert_eq!(next(&mut ws).await, Event::Reconnected);
    second.to_client.send(Message::Text("two".into())).unwrap();
    assert_eq!(next(&mut ws).await, Event::Data("two".into()));

    // The guard is scoped, not dropped: it must not outlive the awaits below.
    {
        let seen = dials.lock().unwrap();
        assert_eq!(seen.len(), 2, "one dial per attempt: {seen:?}");
        assert!(
            seen.iter()
                .all(|(uri, timeout)| uri == "ws://memory.invalid/feed"
                    && *timeout == Duration::from_secs(7)),
            "each dial gets the handshake's request and the timing's connect timeout: {seen:?}"
        );
        assert!(
            runtime.spawns.load(Ordering::SeqCst) >= 1,
            "the supervision task is spawned on the injected runtime"
        );
        assert!(
            runtime.sleeps.load(Ordering::SeqCst) >= 1,
            "the supervisor's waits are the injected runtime's sleeps"
        );
        let slept = runtime.slept.lock().unwrap();
        assert!(
            slept.contains(&timing().initial_backoff),
            "the reconnect waits the backoff's first step on the injected runtime: {slept:?}"
        );
    }
    ws.close().await.expect("close");
}

#[tokio::test]
async fn a_dial_the_transport_fails_is_the_first_attempts_error() {
    let Err(err) = Supervisor::new(Feed, timing())
        .socket_transport(Arc::new(Refusing))
        .connect(vec!["a".to_owned()])
        .await
    else {
        panic!("a transport that times out every dial must not connect");
    };
    let FeedError::Socket(err) = err;
    assert_eq!(
        err.kind(),
        Kind::ConnectTimeout {
            after: Duration::from_secs(7)
        }
    );
}

/// A connection on an in-memory socket and tokio's own clock, which the
/// paused clock of `#[tokio::test(start_paused = true)]` drives.
fn paced_connection(interval: Option<Duration>) -> Connection {
    let (_to_client, incoming) = mpsc::unbounded_channel();
    let (outgoing, _from_client) = mpsc::unbounded_channel();
    let socket: BoxSocket = Box::new(MemorySocket { incoming, outgoing });
    let connection = Connection::with_runtime(socket, Mode::Data, Arc::new(TokioRuntime));
    match interval {
        Some(interval) => connection.min_request_interval(interval),
        None => connection,
    }
}

#[tokio::test(start_paused = true)]
async fn take_paced_id_waits_out_the_interval_from_the_last_paced_send() {
    let mut connection = paced_connection(Some(Duration::from_millis(200)));

    // No paced send yet: the id is taken, and now is the last paced send.
    assert_eq!(connection.take_paced_id(), Some(1));
    // Within the interval of it: refused, and no id is taken.
    assert_eq!(connection.take_paced_id(), None);
    assert_eq!(connection.next_id(), 2, "a refused call takes no id");

    tokio::time::advance(Duration::from_millis(199)).await;
    assert_eq!(
        connection.take_paced_id(),
        None,
        "one millisecond short of the interval"
    );

    // The interval has passed on the runtime's clock: the next id, and the
    // wait restarts from this send.
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(connection.take_paced_id(), Some(2));
    assert_eq!(connection.take_paced_id(), None);
    tokio::time::advance(Duration::from_millis(200)).await;
    assert_eq!(connection.take_paced_id(), Some(3));
    assert_eq!(connection.next_id(), 4);
}

#[tokio::test(start_paused = true)]
async fn take_paced_id_without_an_interval_never_refuses() {
    let mut connection = paced_connection(None);
    assert_eq!(connection.take_paced_id(), Some(1));
    assert_eq!(connection.take_paced_id(), Some(2));
    // An interval too long to add is one never reached, as for `request`.
    let mut connection = paced_connection(Some(Duration::MAX));
    assert_eq!(connection.take_paced_id(), Some(1));
    assert_eq!(connection.take_paced_id(), Some(2));
}
