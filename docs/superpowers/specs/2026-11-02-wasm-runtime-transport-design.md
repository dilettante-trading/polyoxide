# wasm32 through an injected runtime and transport — design

Status: approved by the owner section by section, 2026-11-02; awaiting the owner's review of this
text. It is the design record for building polyoxide for `wasm32-unknown-unknown`, for prader-rs's
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
- `getrandom` 0.3.4's `wasm_js` backend needs the `wasm_js` feature and, in the final binary,
  `--cfg getrandom_backend="wasm_js"`; the feature alone is a compile error.
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
  because the browser implementations' `SendWrapper` panics off its thread. The consumer's final
  binary sets `--cfg getrandom_backend="wasm_js"`; polyoxide enables the feature (D10).

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
  /// `None` when `d` passed first.
  pub async fn timeout<F: Future>(rt: &dyn Runtime, d: Duration, f: F) -> Option<F::Output>;
  ```

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
    and forwarders, sports, rtds and the CLOB's bare tier. governor keeps its own quanta clock.
  - Wall-clock reads are not injected: `UnixMillis::now` (`polyoxide-venue/src/time.rs`) and
    `shared::auth::current_timestamp` read `web_time::SystemTime`.
  - A workspace `clippy.toml` `disallowed-methods` list bans `tokio::spawn`, `tokio::time::sleep`,
    `sleep_until`, `timeout`, `interval`, `tokio::time::Instant::now`, `std::time::Instant::now`
    and `std::time::SystemTime::now`. `polyoxide-rt`'s implementations, `polyoxide-ws`'s
    `test-server` module (native-only, tokio-built) and test code (`#[cfg(test)]`, `tests/`)
    allow it locally.

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
    `status()`, `headers()`, `url()`, and async `bytes()`, `text()` and `json::<T>()`, each
    returning `Result<_, ApiError>`. The loop decides on the head, releases the permit, then
    reads an error body, as `send-loop.md` requires.
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
  `#[non_exhaustive]`) and a boxed `Send + Sync` source. It implements `Classify`: `Request` is
  `InvalidRequest`, every other kind `Network`. `ReqwestTransport` maps a reqwest error by the
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
  - `Kind` gains `Unsupported`: an operation the transport cannot perform (a handshake header,
    a control frame). A transport reports it as `tungstenite::Error::Io` with
    `io::ErrorKind::Unsupported`, which `WsError::kind` reads as `Kind::Unsupported`.
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
    behaviour does not change, and its own `Protocol::connect` override and `dial` are removed.
    An injected transport replaces the dial, D20 included. The CLOB keeps mapping a
    `Kind::ConnectTimeout` to its own `ConnectTimeout { url, timeout }`.

- **D8. The browser implementations** (wasm32 only), each containing its `!Send` JS handles in
  `send_wrapper::SendWrapper`:
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
    `ping()` fails with `Kind::Unsupported` on such a transport. The live suite confirms
    `LIST_SUBSCRIPTIONS` on `fstream` and `docs/specs/binance/OBSERVED.md` records it.
  - Sports (`sees_server_pings == false`): the story first captures the feed for 30 minutes
    with no game live: the longest gap between data frames, and whether a text `ping` is
    answered, recorded in `docs/specs/sports/OBSERVED.md`. If the longest gap is under the 45 s
    default `stale_after`, nothing changes. Otherwise, on such a transport, a text keepalive the
    capture shows answered becomes sports' liveness; failing that, sports' staleness check is
    off on that transport, and a lost connection ends through the close event.
  - The CLOB (text `PING`) and perps (JSON `ping`) are unchanged. No in-scope socket sends a
    handshake header.

- **D10. Target gating.** A feature never turns on a target-specific dependency; each manifest
  declares those under `[target.'cfg(...)'.dependencies]`.
  - Native only: tokio `rt` and `time` (`polyoxide-rt`), tokio `net`, tokio-tungstenite and
    rustls (`polyoxide-ws`), and with them `polyoxide-ws`'s `test-server` feature.
  - wasm32 only: `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys` (the socket and
    event types), `gloo-timers` (`futures`), `send_wrapper` (`futures`), and `getrandom` 0.3 with
    `wasm_js` (in `polyoxide-rt`, so feature unification reaches rand and tungstenite);
    `web-time` for `polyoxide-venue`.
  - Both: `tungstenite` (the message vocabulary; `polyoxide-ws` declares it directly),
    `futures-util`, tokio `sync` and `macros`, `web-time` in `polyoxide-rt`.
  - `polyoxide-polymarket`'s `clob-ws` stops enabling `tokio/net` (the dial lives in ws, D7), and
    `polyoxide-binance`'s `usdm-ws` stops enabling `tokio/rt`. core drops tokio `time`.
  - `ci/dep-allowlists/` stays pinned to `x86_64-unknown-linux-gnu` and is regenerated for the
    native changes; a `wasm32-unknown-unknown` set is added for the supported crates' trees.

- **D11. What breaks (0.45.0).** Each gets its `docs/s3-removals.md` line: `HttpClient::client`;
  `ApiError::Network`; `classify_reqwest`; `polyoxide_ws::Socket`; the return types of
  `HttpClient::send`, `Request::send_raw`, `ListOrders::send_raw`, `connect` and `connect_over`;
  `Pong::response`'s type; the parameters of `ErrorResponse::read`, `ApiError::from_response`,
  `retry_after_header` and `Protocol::connect`. Everything added (`polyoxide-rt`, the transport
  traits and types, `Kind::Unsupported`, the setters, `Connection::with_runtime`,
  `polyoxide_core::http`) is additive. prader-rs uses none of the broken items (1.4).

- **D12. Native testing.** Every existing suite runs on the native defaults, unchanged in what it
  asserts. Two new tests prove injection is honoured: a recording `HttpTransport` drives one
  retry and one hold through the loop; an in-memory `SocketTransport` drives a `Supervisor`
  through a connect, a replay and a reconnect without TCP. Test bodies changed by D6's paths and
  D11's types each get a `[[drift]]` entry in `ci/test-body-diff.toml` (AD-12). Each moved or
  re-proved line in `docs/MUTANTS.md` moves with it.

- **D13. The per-push `wasm` job** (the tenth CI job; the CLAUDE.md table gains its row).
  - A wasm32 `cargo check` of D1's set, crate by crate with their features named, with
    `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`.
  - `cargo clippy` with D3's `disallowed-methods` over the supported crates, native and wasm32.
  - wasm-bindgen-test in headless Chrome, inside a dedicated Worker, from a new `publish = false`
    crate, `polyoxide-wasm-smoke`: `BrowserRuntime` (sleep, `sleep_until`, `timeout`, spawn,
    abort, `is_finished`, a monotonic `now`, a sleep past `i32::MAX` ms that does not fire
    early); `FetchTransport` and `WebSocketTransport` against the crate's native smoke server,
    started in the same step: canned HTTP routes with CORS headers, a 429 with `Retry-After`
    exposed and one without, and a scripted socket from `polyoxide-ws`'s `test-server`.
  - The flake gains the wasm32 target, chromium, chromedriver and a `wasm-bindgen-cli` locked to
    the `wasm-bindgen` crate's version.

- **D14. The nightly live browser job,** `live-wasm-browser`, generated through
  `scripts/gen_registry.py` from `polyoxide-wasm-smoke`'s metadata (the generator learns a
  wasm-bindgen runner). Per host in 1.4: CORS, the exposed headers and the socket upgrade, one
  public GET per HTTP host, one socket per socket host, and Binance's `LIST_SUBSCRIPTIONS`
  keepalive. It needs no secrets. Failures are tagged through `polyoxide-test-support`, whose
  tags build on wasm32 (its keychain loaders become native-only). The live tests are gated by a
  `live` feature, not `#[ignore]`.

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
    `Kind::Unsupported`), `clients.md` (the new setters), `ci-gates.md` (the `wasm` job, the
    lint), `binance.md` and `polymarket-sports.md` (D9), plus CLAUDE.md's workspace list,
    `polyoxide-ws`'s dependency sentence, the jobs table and a new `rt.md`.
  - A spine amendment adds `polyoxide-rt` to the foundation, an AD for the three injection seams
    and the `SendWrapper` containment, and the dependency changes; `docs/ARCHITECTURE.md` is
    regenerated from it, never edited.

- **D16. Sequencing.** Every commit builds. An API change lands with all its callers.
  1. `polyoxide-rt` with `TokioRuntime`, and its crate plumbing (publish order, manifests,
     registry metadata, venue gate, allowlists).
  2. core onto the runtime (D3's core sites and setters).
  3. `HttpTransport`, `ReqwestTransport`, `Response`, `TransportError` and `polyoxide_core::http`,
     with every venue caller, the removals lines and the drift entries (D4–D6).
  4. `SocketTransport`, `TungsteniteTransport` with `per_address`, `BoxSocket`, and the kit and
     every socket crate on the runtime and transport (D3, D7).
  5. The wall clock through `web-time`, the `disallowed-methods` lint and the sweep of what it
     finds.
  6. D10's gating and D8's browser implementations, then D9 (Binance; the sports capture first).
  7. D13: the `wasm` job, the flake, the smoke server and the offline browser suite.
  8. D14: the nightly job, the generator's runner, test-support's wasm32 tags, the `OBSERVED.md`
     facts.
  9. D15's docs, rules and spine amendment, and the 0.45.0 bump with its changelog.

## 3. Gates

- fmt, clippy `-D warnings`, nextest and doctests over the workspace with all features, rustdoc
  `-D warnings`, the msrv build, the features job, `test_body_diff.py --baseline v0.40.0`,
  `api_removals.py check --baseline v0.40.0`, `publish_order.py check-manifests` with the
  package dry run, the CI scripts' suite (fences, allowlists, venue gate, generated regions, the
  rules files), the Python bindings, and from story 7 the `wasm` job.
- Commits by pathspec, conventional, never `--no-verify`.

## 4. Owner actions

- Review this spec.
- Approve the spine amendment (D15) before story 9 merges.
- During story 6, nothing: the sports capture and the `LIST_SUBSCRIPTIONS` check use public,
  unauthenticated sockets.
- After the run: the 0.45.0 release, and prader-rs's bump to it.
