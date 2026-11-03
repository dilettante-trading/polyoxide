# wasm32 through an injected runtime and transport — design

Status: approved by the owner, 2026-11-02, with the plan-time additions in §1.3. It is the design record for building polyoxide for `wasm32-unknown-unknown`, for prader-rs's
browser build of its terminal (prader-app-core in a Web Worker). Read it with CLAUDE.md and
`.claude/rules/{send-loop,sockets,errors,clients,ci-gates,binance,polymarket-sports}.md`. Where
this spec and an earlier record disagree, this spec binds. prader has not decided to ship its web
build; this work does not wait on that decision unless the owner says so.

## 1. Scope, stage and sources

**1.1 Goal.** The crates and features prader's web build uses compile for
`wasm32-unknown-unknown` and run in a browser, with native behaviour unchanged. polyoxide stops
assuming tokio, tokio-tungstenite and a native reqwest: every timer, clock read and spawn goes
through an injected `Runtime`, every HTTP exchange through an injected `HttpTransport`, and every
socket dial through an injected `SocketTransport`. polyoxide ships a native and a browser
implementation of each and picks one by target, so a consumer injects nothing unless it wants its
own.

**1.2 Stage.** The workspace stays in S3. The change is breaking (D11), so the release is 0.45.0,
in a version-bump commit of its own on `main`, and every removed or re-typed public item gets its
line in `docs/s3-removals.md`. It adds a foundation crate, `polyoxide-rt` (D2), which publishes
first, so it needs a spine amendment (D15).

**1.3 The owner's choices (2026-11-02).**

- Scope: exactly prader's set (D1). Kalshi stays native-only; prader's web build gets Kalshi
  through prader-server, which runs `polyoxide-kalshi` natively.
- Approach: an injected runtime and transport, not target cfgs inside each crate, and not
  separate web crates.
- The transport covers sockets and HTTP: core's reqwest client moves behind `HttpTransport`.
- polyoxide ships the browser implementations, each the default on its target.
- Injection is per builder, with the target's default when nothing is injected; there is no
  process-wide install.
- Verification: a browser smoke suite on every push and a live browser job nightly (D13, D14).
- Later the same day: `polyoxide-rt` gains an `Interval` (D2), at prader's request, so the whole
  stack shares one. prader's owner confirmed the Kalshi split: prader-server relays Kalshi's
  public books, trades and candles; the signed socket and the user's key stay desktop-only.
- Execution is the supervised option; the plan is `docs/plans/2026-11-02-wasm-runtime-transport.md`.

**1.4 Sources (measured 2026-11-02).**

- A wasm32 `cargo check --keep-going` on Rust 1.99.0 (the flake's rust-overlay pin plus the
  wasm32 std), on `1d0a395a`: `polyoxide-venue` (`market,trading`) is clean; `polyoxide-core`
  fails four times (`client.rs` `gzip`, `timeout`, `reqwest::redirect`; `error.rs` `is_connect`);
  `polyoxide-ws`, `polyoxide-binance` (`usdm-ws`) and `polyoxide-polymarket`
  (`clob,gamma,data,perps-ws,sports,clob-ws`) stop in their dependencies: `mio` (tokio `net`),
  `rustls-native-certs`, `ring`'s build script and `getrandom` 0.2, so their own code was not
  checked.
- Browser rules, probed with `Origin: https://prader.example`: `clob`, `gamma`, `data`, perps and
  `fapi` answer `Access-Control-Allow-Origin: *`; the CLOB preflight for `POST /order` with the
  `POLY_*` headers is allowed (`max-age` 30 days). Only perps sets
  `Access-Control-Expose-Headers` (`Retry-After, x-pmp-pod`), so a browser cannot read
  `Retry-After` from `clob`, `gamma` or `data`, nor `X-MBX-USED-WEIGHT-1M` from `fapi`. Every
  socket host in scope (`ws-subscriptions-clob` market and user, `ws-live-data`, `sports-api`,
  `ws.perpetuals`, `fstream`) upgrades (101). Kalshi production REST answers 403 to that Origin
  and 200 to `kalshi.com`; its socket answers 401 to both, since it requires signed handshake
  headers.
- `getrandom` 0.3.4's `wasm_js` feature alone selects the browser backend on
  `wasm32-unknown-unknown` (`src/backends.rs:188-192`; its error text, which says the feature is
  insufficient, is stale). Corrected after the run (owner, 2026-11-03): this line first said the
  feature alone was a compile error, which holds only for older 0.3.x. getrandom 0.2, reached
  through alloy's `rand_core`, takes its `js` feature and no flag.
- quanta 0.12.6 (governor's clock) reads `globalThis.performance`, so it works in a Worker.
- prader-rs (HEAD `3c8acb698`) uses none of `send_raw`, `ErrorResponse::read`,
  `ApiError::from_response`, `retry_after_header` or `Pong::response`.

## 2. Decisions (binding)

- **D1. The supported set.** On `wasm32-unknown-unknown` in a browser (the main thread, a
  dedicated Worker or a SharedWorker), without atomics: `polyoxide-rt`; `polyoxide-venue`
  (`market`, `trading`); `polyoxide-core` (without `keychain`); `polyoxide-ws`;
  `polyoxide-binance` (`usdm`, `usdm-ws`); `polyoxide-polymarket` (`clob`, `gamma`, `data`,
  `perps`, `perps-ws`, `sports`, `clob-ws`). Everything else is native-only and documented as
  such: `polyoxide-kalshi`, `relay`, `rtds`, the `polyoxide` umbrella, `polyoxide-cli`,
  `polyoxide-py`, and every `keychain` feature. A Worker is the recommended context, because a
  hidden tab throttles main-thread timers (in Chrome to once a second, and to once a minute after
  five minutes), which breaks ping cadences. A threaded wasm build (atomics) is unsupported,
  because the browser implementations' `SendWrapper` panics off its thread. polyoxide requires getrandom
  0.3.4 and enables `wasm_js` (D10), so the consumer's final binary needs no
  `--cfg getrandom_backend` flag (owner, 2026-11-03).

- **D2. `polyoxide-rt`.** A new published crate, first in publish order, depending on nothing in
  the workspace and naming no venue (it joins `scripts/venue_gate.py`'s foundation list).

  ```rust
  pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
  pub use web_time::{Instant, SystemTime, UNIX_EPOCH}; // std's types on native

  pub trait Runtime: Send + Sync + fmt::Debug + 'static {
      /// Run `task` to completion, detached.
      fn spawn(&self, task: BoxFuture<()>);
      /// Complete after `duration`. Any duration, `Duration::MAX` included, is valid.
      fn sleep(&self, duration: Duration) -> BoxFuture<()>;
      /// The runtime's monotonic clock.
      fn now(&self) -> Instant;
      fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> { /* sleep(deadline - now) */ }
  }

  pub fn default_runtime() -> Arc<dyn Runtime>; // TokioRuntime on native, BrowserRuntime on wasm32
  pub fn spawn<T: Send + 'static>(rt: &dyn Runtime, f: impl Future<Output = T> + Send + 'static) -> Task<T>;
  /// `None` when `d` passed first. `f` is polled before the timer, so a ready future wins even
  /// at `Duration::ZERO`.
  pub async fn timeout<F: Future>(rt: &dyn Runtime, d: Duration, f: F) -> Option<F::Output>;
  /// Ticks every `period`, the first at once. Panics when `period` is zero.
  pub fn interval(rt: Arc<dyn Runtime>, period: Duration) -> Interval;
  ```

  `Interval` has `async fn tick(&mut self) -> Instant` (the deadline it was due at),
  `reset(&mut self)` (the next tick a period from now), `period(&self) -> Duration` and
  `set_missed_tick(&mut self, MissedTick)`. `MissedTick` is `#[non_exhaustive]` with tokio's
  three behaviours and its default: `Burst` (default: fire the missed ticks at once), `Delay`
  (the next tick a period after the late one) and `Skip` (the next tick on the original grid). It
  is built on `sleep_until` and `now`, so `TokioRuntime`'s paused clock drives it. The zero-period
  panic message is `interval period must be non-zero`. Dropping a `Task` detaches it, as tokio's
  `JoinHandle` does.

  `Task<T>` is a `Future<Output = Option<T>>` with `abort()` and `is_finished()`, built by
  `spawn` from `futures`' `Abortable`, a oneshot for the output and a completion flag, so a
  `Runtime` implementor writes only a detached spawn. `None` is a task that was aborted or whose
  output never arrived (a panic). `polyoxide-rt` has no public error type, so the `Classify`
  sweep has nothing to cover there, and it does not depend on `polyoxide-venue` (which would
  reach ws through it, against AD-2). `TokioRuntime` is a unit struct over `tokio::spawn`,
  `tokio::time::sleep` and `tokio::time::Instant::now().into_std()`, so it behaves as the code
  does today and the paused-clock tests keep working. `BrowserRuntime` (D8) is the wasm32
  default.

- **D3. Where the runtime is injected, and its reach.**
  - `ClientConfig` carries `runtime: Option<Arc<dyn Runtime>>`; `client_config_setters!` adds
    `runtime(..)` to every HTTP builder; `HttpClientBuilder` gets `runtime(..)`; `HttpClient`
    holds the `Arc` and shares it with its `with_base_url` siblings, as it shares its throttle.
    `PolymarketBuilder` gains `runtime(..)` and `http_transport(..)`, passed on to its CLOB,
    Gamma and Data clients, and `ClobBuilder::build` gives `user_socket` the client's runtime
    when the socket builder has none (owner, 2026-11-03). `client_config_setters!` names the
    runtime through `$crate::__rt`, a `#[doc(hidden)] pub use polyoxide_rt as __rt` in core, so a
    crate invoking it needs no `polyoxide-rt` dependency of its own.
  - Each socket builder (perps, Binance, the CLOB's market socket and `user_socket`, sports,
    rtds, Kalshi) gets `runtime(..)` and `socket_transport(..)` from a new
    `polyoxide_ws::transport_setters!`, beside `timing_setters!`.
  - `Supervisor::new(protocol, timing)` keeps its signature and gains `.runtime(..)` and
    `.socket_transport(..)`, defaulting by target. `Connection::new(socket, mode)` uses the default
    runtime; `Connection::with_runtime(socket, mode, runtime)` is what venue code calls.
  - Every timer, monotonic clock read and spawn in `polyoxide-ws`, `polyoxide-core`,
    `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide-kalshi` goes through the injected
    runtime: the throttle, holds, capacity buckets, the send loop's retry sleep, `health`'s
    stopwatch, the supervisor and its pings, request correlation, the bare tier, Binance's router
    and forwarders, sports, rtds and the CLOB's bare tier. A window quota's governor limiter runs on a governor `Clock` built
    on the runtime's `now()`, so its check and its `runtime.sleep` read one clock (owner,
    2026-11-02): on a paused tokio clock, quanta's real clock barely moves while the sleep
    advances virtual time, so a quanta-clocked check spins and jumps virtual time by minutes. On
    wasm32 the runtime's `now()` reads `performance.now()`, as quanta did.
  - Wall-clock reads are not injected: `UnixMillis::now` (`polyoxide-venue/src/time.rs`) and
    `shared::auth::current_timestamp` read `web_time::SystemTime`.
  - `scripts/runtime_gate.py`, beside `venue_gate.py`, holds the line, not clippy: a
    `disallowed-methods` config would also fire in every test and example, which use tokio
    legitimately. It scans `src/**/*.rs` of `polyoxide-venue`, `-core`, `-ws`, `-polymarket`,
    `-binance` and `-kalshi`. It skips `#[cfg(test)]` modules, `polyoxide-ws/src/test_server/`
    and each crate's feature-gated `scripted.rs` fixtures. It fails on `tokio::spawn`,
    `tokio::task::spawn`, any `tokio::time::` path, `Instant::now(` and `SystemTime::now(`.
    As built (Story 6's review), it also fails on every `tokio::spawn*` and `tokio::task::spawn*`
    (`spawn_blocking` and `spawn_local` included), on `Instant::now` and `SystemTime::now` named
    as function values, on grouped, aliased and `extern crate` renames of tokio, and it scans
    code under `cfg(any(test, feature = ..))`.
    A line ending `// runtime-gate: <reason>` is exempt, as `live_unwraps.py`'s opt-out is.
    `polyoxide-rt` is not scanned, since it implements the runtime.
    `.github/scripts/tests/test_runtime_gate.py` runs it in CI's scripts job and proves it
    fires on each banned path.

- **D4. `HttpTransport` and `Response` (core).**

  ```rust
  pub trait HttpTransport: Send + Sync + fmt::Debug + 'static {
      fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>>;
  }
  #[non_exhaustive]
  pub struct HttpRequest { pub method: Method, pub url: Url, pub headers: HeaderMap,
                           pub body: Option<Bytes>, pub timeout: Duration }
  ```

  - The loop builds the URL with its query (form-urlencoded, as reqwest's `.query()` does today,
    which the `query_setters_*` suites pin) and always fills `timeout`: the request's own, else
    the client's. The timeout bounds the whole exchange, body included, for every transport.
  - `polyoxide_core::Response` is an owned head and a lazy body:
    `Response::new(status, headers, url, body: BoxFuture<Result<Bytes, TransportError>>)`,
    `status()`, `headers()`, `url()`, and async `bytes()`, `text()` (UTF-8, invalid sequences
    replaced) and `json::<T>()`, each returning `Result<_, ApiError>`. The loop decides on the
    head, releases the permit, then reads an error body, as `send-loop.md` requires.
    `ErrorResponse`'s fields and `ErrorResponse::new(status, headers, body)` do not change.
  - `HttpClient::send`, `Request::send_raw`, clob's `ListOrders::send_raw` and
    `Pong::response` return it; `ErrorResponse::read` and `ApiError::from_response` take it;
    `retry_after_header` takes `&HeaderMap`. `HttpClient`'s public `client: reqwest::Client`
    field is removed (nothing outside core reads it) in favour of a private
    `Arc<dyn HttpTransport>`.
  - `ClientConfig` carries `http_transport: Option<Arc<dyn HttpTransport>>`;
    `client_config_setters!` adds `http_transport(..)`. With none set, `build` makes the target's
    default: `ReqwestTransport` from the config's timeout, pool size and `gzip` (unset, as now),
    with `connect_timeout` 10 s and no redirects; or `FetchTransport` on wasm32 (D8). `pool_size`
    and `gzip` feed only the native default. The `headers_<module>.rs` suites run on it
    unchanged.
  - `ReqwestTransport::new(reqwest::Client)` is public, for a consumer with its own client.

- **D5. Transport errors.** `ApiError::Network(reqwest::Error)` becomes
  `ApiError::Transport(#[from] TransportError)`. `TransportError` is `#[non_exhaustive]`, built
  by `TransportError::new(kind, source)` so a custom transport can make one, with
  `kind() -> TransportKind` (`Connect`, `Timeout`, `Body`, `Request`, `Other`, itself
  `#[non_exhaustive]` and `Debug, Clone, Copy, PartialEq, Eq`) and a boxed `Send + Sync` source,
  so a caller that matched `ApiError::Network(e) if e.is_timeout()` matches
  `ApiError::Transport(e) if e.kind() == TransportKind::Timeout`. Its `Display` is
  `<kind>: <source>`, the kind in lower case (`connect`, `timeout`, `body`, `request`,
  `other`), and `ApiError::Transport`'s is `Transport error: <that>`. It implements `Classify`:
  `Request` is `InvalidRequest`, every other kind `Network`. No `From<reqwest::Error>` exists
  anywhere: `ReqwestTransport` maps through a private function. `ReqwestTransport` maps a reqwest error by the
  rule `classify_reqwest` holds today (timeout, connect, body, or an I/O error or body failure in
  the source chain, are failures in transit), so a body that broke off mid-read stays `Network`.
  `classify_reqwest` is removed. `ApiError`'s `Classify` arm delegates. `TransportError` joins
  core's `const _` assertion.

- **D6. Vocabulary.** core re-exports the `http` crate as `polyoxide_core::http`, and venue
  code, tests and examples name `Method`, `HeaderMap`, `HeaderValue`, `StatusCode` and
  `header::*` through it (the same types reqwest re-exports, so a path change only).
  `pub use reqwest` stays, on native only, for `ReqwestTransport::new` and for the tests that
  open raw clients. core stays the only member that declares reqwest (AD-18), and declares
  `http` and `bytes`.

- **D7. `SocketTransport` (ws).**

  ```rust
  pub trait SocketIo: Stream<Item = Result<Message, tungstenite::Error>>
      + Sink<Message, Error = tungstenite::Error> + Send + Unpin {}
  pub type BoxSocket = Box<dyn SocketIo>;

  pub trait SocketTransport: Send + Sync + fmt::Debug + 'static {
      fn connect(&self, request: http::Request<()>, timeout: Duration, runtime: Arc<dyn Runtime>)
          -> BoxFuture<Result<BoxSocket, WsError>>;
      fn capabilities(&self) -> Capabilities;
  }
  #[non_exhaustive]
  pub struct Capabilities { pub client_pings: bool, pub sees_server_pings: bool,
                            pub handshake_headers: bool }
  ```

  - The transport bounds its own steps with the runtime it is given, so one transport serves
    clients with different runtimes. A timed-out connect is `Kind::ConnectTimeout`.
  - `Capabilities` is built by `Capabilities::new(client_pings, sees_server_pings,
    handshake_headers)` (a `const fn`), and `Capabilities::FULL` is all `true`.
  - `Kind` gains `Unsupported`: an operation the transport cannot perform (a handshake header,
    a control frame). A transport reports it as `tungstenite::Error::Io` with
    `io::ErrorKind::Unsupported`, which `WsError::kind` reads as `Kind::Unsupported`. The browser
    transport's messages are `a handshake header is not supported by the browser transport`,
    and the same with `a ping frame`, `a pong frame` and `a raw frame`.
    `retrying_can_fix` returns `false` for it and `impl_ws_classification!` classes it
    `InvalidRequest`, not a fault of the venue's.
  - `BoxSocket` replaces the `Socket` alias. The free `connect(request, timeout)` dials through
    the target's default transport and runtime (prader-client calls it for prader-server).
    `connect_over(request, TcpStream)` is native-only.
  - `Protocol::connect` becomes
    `fn connect(&mut self, transport: Arc<dyn SocketTransport>, runtime: Arc<dyn Runtime>, timeout: Duration) -> impl Future<Output = Result<BoxSocket, Self::Error>> + Send`,
    by default dialling the handshake through the transport.
  - `TungsteniteTransport` is the native default (`connect_async` through the kit's
    `ensure_crypto_provider`; capabilities all `true`). `TungsteniteTransport::per_address()` is
    the CLOB's dial (D20) moved into ws unchanged: resolve, interleave IPv6 and IPv4, bound each
    address and each step by the timeout. The CLOB's builders default to it, so its native
    behaviour does not change. Its `Protocol::connect` override and `dial` shrink to thin
    wrappers over it, kept only for the `ConnectTimeout` mapping below.
    An injected transport replaces the dial, D20 included. The CLOB keeps mapping a
    `Kind::ConnectTimeout` to its own `ConnectTimeout { url, timeout }`.

- **D8. The browser implementations** (wasm32 only), each containing its `!Send` JS handles and
  futures in `send_wrapper::SendWrapper` (owner, 2026-11-02: alloy's `?Send` `sign_hash` future,
  which the CLOB's signing awaits, is wrapped the same way on wasm32):
  - `BrowserRuntime` (`polyoxide-rt`): `spawn` is `wasm_bindgen_futures::spawn_local`; `sleep` is
    gloo-timers' `TimeoutFuture`, chained in steps of at most `i32::MAX` ms, because
    `setTimeout` fires at once for a larger delay; `now` is `web_time::Instant::now`
    (`performance.now()`).
  - `FetchTransport` (core): reqwest's wasm client (fetch) with the request's timeout set per
    request. A fetch that fails (a network failure, and a CORS refusal, which the browser does
    not tell apart) is `Connect`; a failed body read `Body`. The browser negotiates gzip and
    follows redirects.
  - `WebSocketTransport` (ws): `web_sys::WebSocket` with `binaryType` `arraybuffer`; its events
    feed an unbounded channel read as the `Stream` (a browser socket cannot apply
    backpressure). A close event yields `Message::Close` with its code and reason, then ends the
    stream; an error event yields an I/O error. Sending `Text` or `Binary` is immediate;
    `Close` sends its code when it is 1000 or 3000–4999 and 1000 otherwise; `Ping`, `Pong` and
    `Frame` are `Kind::Unsupported`. A handshake request with a header beyond those tungstenite
    generates (a `Sec-WebSocket-Protocol` becomes the subprotocol list) is refused before
    dialling, as `Kind::Unsupported`. A refused upgrade shows no status in a browser, so it
    arrives as a lost connection, not `Kind::Refused`. Capabilities: all `false`.

- **D9. Adapting to a transport's capabilities.**
  - Binance (`client_pings == false`): the supervised protocol pings with a
    `{"method":"LIST_SUBSCRIPTIONS","id":n}` request through its request correlation (paced, as
    every request is, by `MIN_REQUEST_INTERVAL`), and its answer is liveness. The bare client's
    `ping()` fails with `Kind::Unsupported` on such a transport. A live test in `live_usdm_ws`
    confirms `LIST_SUBSCRIPTIONS` on `fstream` (D17), and `docs/specs/binance/OBSERVED.md`
    records it. If `fstream` leaves it unanswered, the protocol's `ping()` returns `None` on
    such a transport instead, liveness comes from data frames only, and the README's browser
    gaps say a quiet stream reconnects at `stale_after`; that is not escalated.
  - Sports (`sees_server_pings == false`): the story first captures the feed for 30 minutes
    with no game live: the longest gap between data frames, and whether a text `ping` is
    answered, recorded in `docs/specs/sports/OBSERVED.md`. If the longest gap is under the 45 s
    default `stale_after`, nothing changes. Otherwise, on such a transport, a text keepalive the
    capture shows answered becomes sports' liveness; failing that, sports' staleness check is
    off on that transport, and a lost connection ends through the close event.
    Decided (owner, 2026-11-02): the capture (19:06:55Z–19:36:56Z) found a game live throughout
    (longest data gap 10.6 s, protocol pings every 15 s) and both text pings unanswered, so B is
    out and the quiet-hour gap is unmeasured. Sports takes the last rule (C) without a rerun: on a
    transport whose `sees_server_pings` is `false` its staleness check is off, and a lost
    connection ends through the close event. `docs/specs/sports/OBSERVED.md` records the live
    capture's facts and says the quiet-hour gap and a quiet-hour text-ping answer are unmeasured.
    A browser socket left half-open goes undetected, which the README's browser gaps say.
    With the check off, no timer is armed at all (`std::future::pending`), rather than a
    `sleep(Duration::MAX)` re-armed on every frame (owner, 2026-11-03).
  - The CLOB (text `PING`) and perps (JSON `ping`) are unchanged. No in-scope socket sends a
    handshake header.

- **D10. Target gating.** A feature never turns on a target-specific dependency; each manifest
  declares those under `[target.'cfg(...)'.dependencies]`.
  - Native only: tokio `rt` and `time` (`polyoxide-rt`), tokio `net`, tokio-tungstenite and
    rustls (`polyoxide-ws`), and with them `polyoxide-ws`'s `test-server` feature.
  - wasm32 only: `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys` (the socket and
    event types), `gloo-timers` (`futures`), `send_wrapper` (`futures`), and `getrandom` 0.3.4 or later with
    `wasm_js` (in `polyoxide-rt`, so feature unification reaches rand and tungstenite);
    `web-time` for `polyoxide-venue`.
  - Both: `tungstenite` (the message vocabulary; `polyoxide-ws` declares it directly),
    `futures-util`, tokio `sync` and `macros`, `web-time` in `polyoxide-rt`.
  - `polyoxide-polymarket`'s `clob-ws` stops enabling `tokio/net` (the dial lives in ws, D7), and
    `polyoxide-binance`'s `usdm-ws` stops enabling `tokio/rt`. core drops tokio `time`, and so do
    the venue crates' normal dependencies, whose production code no longer uses it (owner,
    2026-11-03).
  - `ci/dep-allowlists/` stays pinned to `x86_64-unknown-linux-gnu` and is regenerated for the
    native changes; a `wasm32-unknown-unknown` set is added for the supported crates' trees.

- **D11. What breaks (0.45.0).** Each gets its `docs/s3-removals.md` line: `HttpClient::client`;
  `ApiError::Network`; `classify_reqwest`; `polyoxide_ws::Socket`; the return types of
  `HttpClient::send`, `Request::send_raw`, `ListOrders::send_raw`, `connect` and `connect_over`;
  `Pong::response`'s type; the types of `Connection::new` and
  `Connection::last_inbound`; the parameters of `ErrorResponse::read`, `ApiError::from_response`,
  `retry_after_header` and `Protocol::connect`. Everything added (`polyoxide-rt`, the transport
  traits and types, `Kind::Unsupported`, the setters, `Connection::with_runtime`,
  `polyoxide_core::http`) is additive. prader-rs uses none of the broken items (1.4).

- **D12. Native testing.** Every existing suite runs on the native defaults, unchanged in what it
  asserts. Two new tests prove injection is honoured: a recording `HttpTransport` drives one
  retry and one hold through the loop; an in-memory `SocketTransport` drives a `Supervisor`
  through a connect, a replay and a reconnect without TCP. Test bodies changed by D6's paths and
  D11's types each get a `[[drift]]` entry in `ci/test-body-diff.toml` (AD-12), naming a new
  DRIFT row, `R22`, in `_bmad-output/specs/spec-venue-extensibility/divergences.md`: the
  wasm32 seams change the names and types tests use, never what they assert, except that a row
  asserting an `ApiError::Network` case no transport can produce goes with the variant (owner,
  2026-11-02: core's `every_variant_classifies` loses its reqwest decode, 503 and 404 rows; the
  decode case is re-pinned on `Response::json`, and status stays covered by the
  `ApiError::Response` rows). The story that
  first changes a test body adds the row; later stories cite it. Story 2 is that story (owner,
  2026-11-02): with the quota on the runtime's clock (D3), the quota's pacing shows in virtual
  time, so `polymarket_limits`' `an_expired_cooldown_stops_delaying` advances virtual time past
  the quota's refill before its second acquire and keeps its `< 1 ms` assertion; R22's text
  covers a paused-clock test whose setup advances past a wait the runtime now sees. Story 4
  fixes core's `mock_request.rs` `retries_on_429_then_succeeds`, which mockito answers from the
  success mock first so it never retries: it reorders the mocks, proves the test red without
  the retry, and takes a `[[drift]]` entry citing R22. After the run (owner, 2026-11-03),
  `polymarket_throttle`'s `the_hold_survives_a_tier_change` takes its start instant before
  `throttle.hold(..)`, not after, since a scheduling stall between the two shortened its reading
  under load. Its `[[drift]]` entry names a new DRIFT row, R23, for a test that measured from the
  wrong instant. Divergence row D20's text
  moves the CLOB's dial into `TungsteniteTransport::per_address()`. Each moved or re-proved line
  in `docs/MUTANTS.md` moves with it.

- **D13. The per-push `wasm` job** (the tenth CI job; the CLAUDE.md table gains its row).
  - `scripts/wasm_check.sh` (no arguments; stops at the first failure) runs `cargo check` and
    `cargo clippy -- -D warnings` for `wasm32-unknown-unknown` over D1's set, crate by crate
    with their features named. It sets `RUSTFLAGS` empty itself, replacing any inherited value,
    since an inherited `RUSTFLAGS` (a host `target-cpu`) would break the target. It sets no
    getrandom flag, so the job proves `wasm_js` alone selects the backend (owner, 2026-11-03;
    `scripts/wasm_smoke.sh` and `scripts/wasm_live.sh` likewise). With `--target`, cargo does not pass
    `RUSTFLAGS` to build scripts, so the host cache is untouched.
  - D3's `runtime_gate.py` runs in the existing CI scripts job.
  - `scripts/wasm_smoke.sh` starts the smoke server, exports its addresses, and runs
    wasm-bindgen-test in headless Chrome inside a dedicated Worker
    (`wasm_bindgen_test_configure!(run_in_dedicated_worker)`), then stops the server. The suite
    lives in a new `publish = false` crate, `polyoxide-wasm-smoke`:
    - its native bin `wasm-smoke-server` binds `127.0.0.1:0` for HTTP (mockito, every route
      answering CORS headers, one 429 that exposes `Retry-After` and one that does not) and for
      a socket (`polyoxide-ws`'s `test-server`, one scripted connection), and writes
      `POLYOXIDE_SMOKE_HTTP=<url>` and `POLYOXIDE_SMOKE_WS=<url>` lines to the file its one
      argument names;
    - its wasm32 tests, `tests/browser_smoke.rs`, read both through `env!`, so a build without
      them fails to compile rather than passing empty. They cover `BrowserRuntime` (sleep,
      `sleep_until`, `timeout`, `interval`, spawn, abort, `is_finished`, a monotonic `now`, and
      a sleep past `i32::MAX` ms that has not fired after a second), `FetchTransport` (a 200, a
      timeout, both 429s read through `ErrorResponse`) and `WebSocketTransport` (a text echo, a
      close with its code, and a refused handshake header and ping frame). Story 7's review
      added three browser-only paths no host test reaches (owner, 2026-11-02): a CLOB signature
      (`sign_clob_auth` or `sign_order`, through the `SendWrapper`ed `sign_hash`), a body read
      that outlives `FetchTransport`'s deadline (`Timeout`), and a window-quota wait that paces
      on the runtime's clock (Story 9 measured that quanta's own clock does not panic in a
      Worker, as 1.4 says; the test proves the limiter reads the injected runtime's clock).
  - CI's job installs the wasm32 target and `wasm-bindgen-cli` at the version `Cargo.lock` pins
    for `wasm-bindgen`, and uses the runner's Chrome and chromedriver. Locally, the flake gains
    the wasm32 target (in the story that first builds for it) and chromium, chromedriver and
    the same `wasm-bindgen-cli` (in this one); `flake.lock` does not change.

- **D14. The nightly live browser job,** `live-wasm-browser`, generated through
  `scripts/gen_registry.py` from `polyoxide-wasm-smoke`'s metadata (the generator learns a
  wasm-bindgen runner). Per host in 1.4: CORS, the exposed headers and the socket upgrade, one
  public GET per HTTP host, one socket per socket host, and Binance's `LIST_SUBSCRIPTIONS`
  keepalive. It needs no secrets. Failures are tagged through `polyoxide-test-support`, whose
  tags build on wasm32 (its keychain loaders become native-only). The live tests,
  `polyoxide-wasm-smoke/tests/browser_live.rs`, are gated by a `live` feature, not `#[ignore]`,
  and run through `scripts/wasm_live.sh`. The story proves a tag line from a failing browser
  test reaches the runner's output, which the nightly classifier reads. Its limits are D17's.

- **D15. Documentation and rules.**
  - A "WebAssembly" section in README.md: D1's set and contexts, the getrandom flag, injection
    with the defaults by target, and the browser's gaps: `Retry-After` unreadable on `clob`,
    `gamma` and `data` (the loop's floor applies); Binance's used weight unreadable, so
    `WeightBudget` runs on local charges and tabs sharing an IP cannot see each other (one
    SharedWorker, or a lower `weight_budget`); redirects followed; no handshake headers or
    control frames; a refused upgrade seen as a lost connection; a preflight per new
    authenticated path.
  - Each host's `OBSERVED.md` records 1.4's browser facts.
  - Rules files change in the commits that change their rules (AD-21): `sockets.md` (the
    transport, the kit's dependencies, D20's new home), `send-loop.md` (the transport, the
    `Response`, the header pins on the default), `errors.md` (`TransportError`,
    `Kind::Unsupported`), `clients.md` (the new setters), `ci-gates.md` (the `wasm` job,
    `runtime_gate.py`), `binance.md` and `polymarket-sports.md` (D9), plus CLAUDE.md's workspace list,
    `polyoxide-ws`'s dependency sentence, the jobs table and a new `rt.md`.
  - A spine amendment, `spine-amendments/wasm-runtime-transport.md` beside the spine, adds
    `polyoxide-rt` to the foundation, an AD for the three injection seams and the
    `SendWrapper` containment, and the dependency changes. Merging it into the spine and
    regenerating `docs/ARCHITECTURE.md` is the owner's; no story edits either.

- **D16. Sequencing.** Every commit builds, and an API change lands with all its callers. No
  story bumps a version or publishes.
  1. `polyoxide-rt` with `TokioRuntime`, `Task`, `timeout` and `Interval`, and its crate
     plumbing (publish order, manifests, registry metadata, venue gate, allowlists).
  2. core, and the HTTP side of the venue crates, onto the runtime: D3's core sites, the
     `runtime` setters, and the throttles, limits and clients in polymarket, binance and kalshi.
  3. `polyoxide_core::http` and the move of every `reqwest::` vocabulary path to it (D6), with
     DRIFT row `R22` and the drift entries. No behaviour or API changes.
  4. `HttpTransport`, `ReqwestTransport`, `Response` and `TransportError` (D4, D5), with every
     venue caller, the removals lines and the drift entries.
  5. `SocketTransport`, `TungsteniteTransport` with `per_address`, `BoxSocket`,
    `Kind::Unsupported`, and the kit and every socket crate on the runtime and transport (D3,
    D7), with D20's row, the removals lines and the drift entries.
  6. The wall clock through `web-time`, `runtime_gate.py` and the sweep of what it finds (D3).
  7. D10's gating and D8's browser implementations, the flake's wasm32 target, and
     `scripts/wasm_check.sh`.
  8. D9: the Binance keepalive with its live check, and the sports capture, then its rule.
  9. D13: the `wasm` job, `polyoxide-wasm-smoke`, the smoke server, `scripts/wasm_smoke.sh` and
     the flake's browser tools.
  10. D14: the nightly job, the generator's runner, test-support's wasm32 tags,
      `scripts/wasm_live.sh` and the `OBSERVED.md` facts.
  11. D15: the README, the rules files, CLAUDE.md and the spine amendment.

- **D17. Limits on the outside services this work calls.**
  - The nightly browser job, per host: one GET (no preflight) bounded by 10 s, and one socket
    held until its first data frame or 20 s. Each check retries once after 5 s. A second
    failure is `real`, except a socket that opened and then dropped, which is `transient`. The
    job's timeout is 15 minutes. As built and accepted (owner, 2026-11-03): `wasm_live.sh`
    first reads Binance's status natively with one GET to `fapi`, so a 451 from a US runner is
    tagged `environmental` (two `fapi` GETs on a night Binance answers), and its transient
    retry pass may open a socket host that keeps dropping up to four times.
  - Story 8's `LIST_SUBSCRIPTIONS` live test: one socket on `fstream`'s `/market` path,
    subscribed to `btcusdt@markPrice@1s`, sending the request six times 20 s apart (well above
    the 200 ms pacing). Each answer is due within 5 s. An unanswered one selects D9's fallback,
    and is recorded, not escalated. The test stays in `live_usdm_ws` and repeats nightly, as
    the keepalive's drift check (owner, 2026-11-03).
  - Story 8's sports capture: one unauthenticated socket for 30 minutes, at a time the feed
    reports no game live, sending a text `ping` at minutes 10 and 20 and nothing else. A drop
    reconnects once and extends the capture by the time lost. A second drop, or a game live
    throughout, stops the story and goes to the owner (it did on 2026-11-02; D9 records the
    decision).
  - The smoke server is local, and nothing in a story's checks calls an outside host otherwise.

## 3. Gates

- fmt, clippy `-D warnings`, nextest and doctests over the workspace with all features, rustdoc
  `-D warnings`, the msrv build, the features job, `test_body_diff.py --baseline v0.40.0`,
  `api_removals.py check --baseline v0.40.0`, `publish_order.py check-manifests` with the
  package dry run, the CI scripts' suite (fences, allowlists, venue gate, generated regions, the
  rules files), the Python bindings, and from story 7 the `wasm` job.
- Commits by pathspec, conventional, never `--no-verify`.

## 4. Owner actions

- During story 8, nothing unless D17 stops it: the sports capture and the `LIST_SUBSCRIPTIONS`
  check use public, unauthenticated sockets.
- Before the merge: CI's python job has been red on `main` since Data API v1 was retired
  upstream (every v1 route 410, 2026-11-02). A separate session skips the v1 Python live tests,
  and a later spec retires v1 across the crates. The 0.45.0 release needs it green.
- Landing (owner, 2026-11-03): Binance spot (plans 1 and 2, REST and sockets) reached `main`
  first. `main` is merged into this branch, and spot is moved onto the seams there: its clients
  and socket builders take `runtime(..)` and `socket_transport(..)` like usdm, its code names
  `polyoxide_core::http`, `Response` and `TransportError`, and `runtime_gate.py` scans it.
  Spot stays native-only: D1's set does not change, since spec 1.4 never probed CORS or the socket
  upgrade on spot's hosts, and the README's WebAssembly section says spot is native-only. The
  merge passes the native gates in the worktree. The wasm job runs once `main` has this branch's
  `flake.nix`, because a worktree's `direnv exec .` loads the main checkout's flake; a red wasm
  job is then fixed forward on `main`.
- After the merge: read the first nightly's `live-wasm-browser` job for a real Binance 451,
  since the preflight's tagging was tested only on a forced one.
- After the run: merge the spine amendment and regenerate `docs/ARCHITECTURE.md`; the 0.45.0
  version-bump commit on `main` and its release; prader-rs's bump to it.
