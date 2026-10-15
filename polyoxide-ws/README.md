# polyoxide-ws

The WebSocket kit the polyoxide socket clients share, one copy of each block
they would otherwise each carry:

- `ensure_crypto_provider` installs the process-default rustls provider a
  `tokio-tungstenite` connect needs.
- `Backoff` is the reconnect delay schedule, and `Timing` holds a supervised
  connection's ping, staleness, backoff, connect and rotation timings, with
  `timing_setters!` for a builder and `next_wait` for the wait until the next
  deadline.
- `connect` opens a connection within a timeout (`DEFAULT_CONNECT_TIMEOUT`,
  10 s), `connect_over` upgrades a TCP connection the caller opened itself,
  `with_timeout` bounds a connect of the caller's own, and
  `retrying_can_fix` says whether a reconnect can help after one failed, by a
  status rule the caller passes in.
- `Connection` is the bare tier: one connection as a `Stream` of its frames,
  in `Mode::Data` (text and binary) or `Mode::All` (every frame). It answers
  the server's close with the RFC 6455 reply, says how the server closed, and
  correlates requests with their answers through a caller's builder and
  matcher (`Correlated`), with optional pacing and an answer timeout.
- `Supervisor` runs one supervised connection for a venue's `Protocol`: pings
  on the wall clock, even while the consumer's queue is full, staleness,
  reconnect with backoff and a replay of the membership, rotation, outage
  markers, and membership changes through a `MembershipHandle`, all as one
  `Supervised` stream. A venue keeps only what is its own: its frames, its
  ping, what counts as liveness, how it recovers and whether it announces an
  outage.
- `WsError` is the one socket error, read through its `Kind`.
  `polyoxide_venue::impl_ws_classification!` classifies it in the venue crate
  that wraps it.
- `tungstenite` is re-exported, so a venue names `Message` and `CloseFrame`
  through this crate.

The `test-server` feature adds a doc-hidden scripted local server and the
helpers the offline suites share. It is for tests, not consumers.

It depends on neither `polyoxide-core` nor `polyoxide-venue`, and builds no
HTTP or signing stack, so a credential-free feed built on it stays light. It
logs under the `polyoxide_ws` target and never installs a subscriber.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-ws/).

## Installation

```toml
[dependencies]
polyoxide-ws = "0.40"
```

## Usage

A request on the bare tier, then the frames that follow it:

```rust,no_run
use std::time::Duration;
use futures_util::StreamExt;
use polyoxide_ws::{connect, tungstenite::Message, Connection, Correlated, Mode, DEFAULT_CONNECT_TIMEOUT};

# async fn run() -> Result<(), polyoxide_ws::WsError> {
let socket = connect("wss://example.com/ws", DEFAULT_CONNECT_TIMEOUT).await?;
let mut connection = Connection::new(socket, Mode::Data)
    .min_request_interval(Duration::from_millis(200))
    .answer_timeout(Duration::from_secs(10));

connection
    .request(
        |id| Message::Text(format!(r#"{{"id":{id},"op":"subscribe"}}"#).into()),
        |id, frame| match frame {
            Message::Text(text) if text.contains(&format!(r#""id":{id}"#)) => Correlated::Answer(()),
            // Anything else is kept, and yielded below in the order it came.
            _ => Correlated::Push,
        },
    )
    .await?;

while let Some(frame) = connection.next().await {
    println!("{:?}", frame?);
}
if let Some(closed) = connection.closed() {
    eprintln!("{closed}");
}
# Ok(())
# }
```

A supervised builder takes the shared timings, and its reconnect loop waits
out the schedule they describe:

```rust
use std::time::Duration;
use polyoxide_ws::Timing;

#[derive(Default)]
pub struct FeedBuilder {
    timing: Timing,
}

impl FeedBuilder {
    polyoxide_ws::timing_setters! {
        /// How often to ping, whatever the traffic.
        ping_interval;
        /// Reconnect delay bounds.
        backoff;
    }
}

let timing = FeedBuilder::default()
    .backoff(Duration::from_millis(100), Duration::from_millis(300))
    .timing;
let mut backoff = timing.backoff();
let waits: Vec<_> = (0..4).map(|_| backoff.take()).collect();
assert_eq!(waits, [100, 200, 300, 300].map(Duration::from_millis));

// A connection that delivered something starts the schedule over.
backoff.after_connection_ended(true);
assert_eq!(backoff.take(), Duration::from_millis(100));
```
