---
paths:
  - "polyoxide-ws/**"
  - "**/src/**/ws/**"
  - "polyoxide-polymarket/src/{rtds,sports}/**"
  - "polyoxide-venue/src/{socket,ws_table}.rs"
  - "**/tests/*_ws.rs"
  - "**/tests/ws_*.rs"
  - "**/tests/{supervision,bare}_*.rs"
---

# Sockets and the kit

`polyoxide-ws` is the socket kit. It depends on neither `polyoxide-core` nor `polyoxide-venue`.
On a host it builds tokio, tokio-tungstenite and rustls; on `wasm32` it builds the browser's `WebSocket` through web-sys
instead (the dependencies are per target, in `Cargo.toml`'s target tables). On both it builds
`polyoxide-rt` and `tungstenite`, which it declares directly and re-exports, futures-util,
thiserror and tracing. `polyoxide_venue::impl_ws_classification!` classifies its `WsError`
in each venue crate (`errors.md`).

## Blocks

One copy of each block the socket clients share: `ensure_crypto_provider`, `Backoff`, `Timing`
with `timing_setters!` and `next_wait`, `WsError` read through its `#[non_exhaustive]` `Kind`,
`connect`, `connect_over` (native only: the upgrade of a TCP connection a venue dialled itself),
`with_timeout`, `retrying_can_fix` (the status rule injected), the bare tier `Connection`
(`Mode::Data` or `Mode::All`; the server's close captured and its RFC 6455 reply flushed,
DRIFT R3) with request/answer correlation, and the scripted test server.

## Transports and the runtime

Every dial goes through a `SocketTransport` and every timer, clock read and spawn through a
`polyoxide_rt::Runtime`, each injected and each defaulted by target (`default_socket_transport`,
`polyoxide_rt::default_runtime`); nothing is process-wide.

- `SocketTransport::connect(request, timeout, runtime)` returns a `BoxSocket`, a boxed stream and
  sink of tungstenite messages. The transport bounds its own steps with the runtime it is given,
  and a timed-out connect is `Kind::ConnectTimeout`. `capabilities()` says whether it can send
  pings (`client_pings`), shows the server's (`sees_server_pings`) and accepts handshake headers
  (`handshake_headers`); `TungsteniteTransport`, the native one, has all three.
- `Kind::Unsupported` is an operation the transport cannot perform, read from an I/O error of kind
  `std::io::ErrorKind::Unsupported`. Reconnecting never fixes it, and the socket table classes it
  `InvalidRequest`.
- A builder's `runtime(..)` and `socket_transport(..)` come from `polyoxide_ws::transport_setters!`,
  beside `timing_setters!`, and its connect falls back to the target's defaults. The CLOB's
  default transport is `TungsteniteTransport::per_address()` on a host target and
  `default_socket_transport` on `wasm32`.
- `Supervisor::new` keeps its signature and gains `.runtime(..)` and `.socket_transport(..)`.
  `Connection::new` runs on the default runtime and `Connection::with_runtime` on the one given,
  which `Connection::runtime()` returns for a protocol's own timers.

## The `Supervisor` (`src/supervisor/`)

A venue describes its socket as a `Protocol` and gets a `Supervised<P>` stream with a
`MembershipHandle<P>`. The protocol supplies:

- a handshake built per attempt; `connect`, the dial of each attempt, which gets the supervisor's
  `SocketTransport` and runtime (by default it dials the handshake's request through the
  transport; a venue that tries each address defaults its builder's transport to
  `TungsteniteTransport::per_address()` instead of dialling for itself); `configure` of each
  fresh connection, such as Binance's 200 ms request pacing;
- `replay` and `apply` of its membership over the connection's request correlation; `decode` of
  every frame into `Decoded` events; its liveness (D5) and delivered (D6) predicates; its ping
  (D4); `wanted`;
- `recover` to `Recovery::{Reconnect, Skip, Stop}` (by default `Reconnect` exactly when its
  required `is_retriable` says so); `retry_after`, the wait a failure asked for, which perps and
  Binance answer from `Classify` (by default `None`);
- `follow_up`: one frame sent after a `decode` that leaves the connection up, bounded by
  `stale_after` and not awaited (by default none). A failed or timed-out send ends the connection
  as a failed ping does, and the protocol clears the frames it still holds in `reset`;
- `record` for a change made during an outage (held by default); its outage markers (D8);
  `disconnected`, owning its `Outage` so a marker can carry the error; and its `Queue`.

What the task guarantees (`src/supervisor/tests.rs` pins each rule):

- it pings on the wall clock even while a bounded queue is full (it then reads nothing, and the
  wait is not silence);
- it drops no decoded event, and yields at most one `Disconnected` and exactly one `Reconnected`
  per outage;
- it closes politely and ends when the membership stops being wanted on a healthy connection, and
  rotates at `max_connection_age`;
- it waits out a refusal's `Retry-After` before the next attempt, never less than the backoff's
  step and never past its ceiling (`Backoff::take_at_least`, DRIFT R11, which rtds and sports
  apply at their own reconnect waits);
- it bounds `replay` and `apply` by `stale_after` and replaces a connection whose request
  outlived it;
- it ends promptly when the consumer drops the stream.

## Which socket runs on what

- On the `Supervisor`: perps (`PerpsProtocol`), Binance (one supervisor per routed path, behind a
  router), Kalshi (`PredictionsProtocol`), and the CLOB's supervised market and user sockets.
- On the blocks, each keeping its own supervision shape (D7):
  - rtds: TLS, `Backoff`, connect with a timeout, `retrying_can_fix`, the bare tier's close reply
    and the test server;
  - sports: TLS, `Backoff`, a dial through `open_with` and the builder's `SocketTransport`, the
    bare tier (`Mode::All`, so pings count as liveness, D5) and `retrying_can_fix`, behind its own
    `Capacity` and `AlreadyClosed` (D19) rows in a private recovery function;
  - the CLOB's bare tier: TLS, `WsError`, the bare tier (`Mode::Data`, so a server close is
    answered, DRIFT R3) and the test server. Its per-address dial is
    `TungsteniteTransport::per_address()` (D20, moved into the kit), which its builders default
    to on a host target; it keeps its own `select!`, its 10 s text `PING`, its outbound queue and
    Binary decoded as text (D12).
- A per-venue divergence row (D4–D19) stays at the venue; only identical or parametric copies
  were merged into the kit.
- Keep-alive runs on the wall clock for any host that requires pings. The rtds pump pings only on
  a quiet tick: do not copy its shape for such a host.

## TLS needs a nudge

`reqwest 0.12` (via core) and alloy's `reqwest 0.13` enable `ring` and `aws-lc-rs` on one shared
`rustls`, which then installs no default `CryptoProvider`. The kit installs one before connecting
(`polyoxide_ws::ensure_crypto_provider`, which `TungsteniteTransport` calls before it dials).
Code that calls `tokio_tungstenite::connect_async` directly must do the same or it panics, so
connect through the kit: `TungsteniteTransport` is the one caller of it, after
`ensure_crypto_provider`, and no crate gets its own `ensure_crypto_provider` copy.

`polyoxide-ws` takes `rustls` from the one `[workspace.dependencies]` entry, which declares `ring`
and `std`, so it relies on neither `reqwest` nor `alloy` to turn `std` on (clob once compiled only
because they did, DRIFT R5). Binance and `polyoxide-polymarket` declare neither `rustls` nor
tokio-tungstenite: the kit owns their transport, and Binance's, clob's and sports' live suites
open their raw sockets through the kit's `connect` (DRIFT R9).

## The scripted test server

`polyoxide_ws::test_server`, behind ws' `test-server` feature (doc-hidden, built only from ws' own
dependencies, never from `polyoxide-test-support`). Every venue's offline socket suites run on
it; each venue keeps only its protocol's answer and its fixtures.

- `ScriptedServer::start(Vec<Script>)` plays one script per connection, the last repeating;
  `start_answering` takes a closure called per connection that returns that connection's
  answerer, owning its protocol state.
- Actions: reject, stall, refuse-upgrade with headers, send, `after_first_frame` (record the
  client's first frame before sending), `push_every`, `ping_every` (big-endian sequence payloads),
  `close_after` with an optional code, `drop_after`, `go_silent`, hold (the default), and a cut
  with no close frame, after sending or on a schedule (`cut_after`).
- Recorders: frames per connection, first frames, paths, each handshake's request headers
  (`handshake_headers`: names lowercase, values as lossy UTF-8, in the order handshakes completed;
  a refused upgrade takes no slot, so an index is not a connection's index), closes, close
  replies, pongs, pings received, accept times and end counts. `wait_for` polls for 5 s.
- Helpers: `fast()` (a `Timing` of 50 ms pings, 300 ms staleness and 10 to 20 ms backoff),
  `next_item`, `next_event` and `drain`.

## The browser transport (`src/browser.rs`)

`WebSocketTransport` is the `wasm32` default of `default_socket_transport`; a host's is `TungsteniteTransport`. Its capabilities are all `false`. It opens `web_sys::WebSocket` with `binaryType` `arraybuffer` and reads its events from an unbounded channel (a browser socket cannot apply backpressure); the `WebSocket` and its callbacks sit in `SendWrapper`s, as in `rt.md`. A close event yields `Message::Close` with its code and reason, then ends the stream; an error event yields an I/O error. `Close` goes out with its code when it is 1000 or in 3000-4999 and 1000 otherwise, its reason cut to 123 bytes, since a browser throws on anything else. A handshake request with a header beyond those tungstenite generates (a `Sec-WebSocket-Protocol` becomes the subprotocol list) is refused before dialling, and sending a ping, pong or raw frame is refused, each as `Kind::Unsupported` with the message `a handshake header`, `a ping frame`, `a pong frame` or `a raw frame` followed by `is not supported by the browser transport`. A refused upgrade shows no status to a page, so it arrives as a lost connection, never `Kind::Refused`.

What the transport decides (`dial_plan`, `close_code`, `close_reason`, `outgoing`) is plain Rust built on every target, so the host's tests pin it; only the `web_sys` code is `wasm32`'s. `connect_over`, `TungsteniteTransport`, `ensure_crypto_provider` and the scripted test server are host-only, and the `test-server` feature turns on nothing on `wasm32`.
