# wasm32 Through an Injected Runtime and Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Under the supervised option, `/flow-supervise` runs it story by story.

**Goal:** prader's set of polyoxide crates and features builds for `wasm32-unknown-unknown` and runs in a browser Worker, with native behaviour unchanged, because every timer, clock read and spawn goes through an injected `Runtime`, every HTTP exchange through an injected `HttpTransport` and every socket dial through an injected `SocketTransport`, each with a native and a browser default picked by target.

**Architecture:** A new foundation crate, `polyoxide-rt`, holds the `Runtime` trait, `Task`, `timeout` and `Interval`, with `TokioRuntime` (native) and `BrowserRuntime` (wasm32). core gains `HttpTransport`, an owned `Response` and `TransportError`, with `ReqwestTransport` and `FetchTransport`; ws gains `SocketTransport` and `BoxSocket`, with `TungsteniteTransport` (the CLOB's per-address dial as one of its modes) and `WebSocketTransport`. Builders carry the runtime and transports; nothing is process-wide. `Send` stays on every trait: the browser implementations contain their JS handles in `SendWrapper`. The native refactor lands first, API break by API break, then the target gating and browser implementations, then the browser CI. The 0.45.0 release is the owner's, after the run.

**Tech Stack:** Rust (MSRV 1.91, dev shell 1.99.0), tokio, reqwest 0.12, tokio-tungstenite 0.26, tungstenite, futures-util, web-time, send_wrapper, gloo-timers, wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys, wasm-bindgen-test with headless Chrome, mockito, the kit's scripted server, Python 3 scripts.

**Spec:** `docs/superpowers/specs/2026-11-02-wasm-runtime-transport-design.md`. Read it with CLAUDE.md and the rules files each story's **Reading** line names. Where the spec and an earlier record disagree, the spec binds.

---

## Rules for every story

- **Run commands with the dev shell's prefix**, `nix develop --command`. A story's gates are the profile's; its **Verify** line is the developer's done-check.
- **Every commit builds and passes**, so an API change lands in one commit with every caller, in src, tests, examples and docs (spec D16).
- **A changed test body needs a `[[drift]]` entry** in `ci/test-body-diff.toml`, naming DRIFT row `R22` and the diff's sha256 (spec D12); a lost test needs a `[[retired]]` entry. A changed body changes names and types, never what the test asserts. git pairs only tracked files, so stage a moved file before running `test_body_diff.py`.
- **A removed or re-typed public item gets its line in `docs/s3-removals.md`** in the story that removes it (spec D11).
- **The foundation crates name no venue** (`python3 scripts/venue_gate.py`): `polyoxide-rt`, `polyoxide-venue`, `polyoxide-core`, `polyoxide-ws` and `polyoxide-test-support` use made-up words in docs and tests.
- **No `From<reqwest::Error>`, `From<serde_json::Error>` or `From<url::ParseError>`** anywhere; `ReqwestTransport` maps through a private function (spec D5).
- **`connect_async(` appears only inside `polyoxide-ws`**: in `TungsteniteTransport` and the test server.
- **Never call `.gzip(`** on a client builder; the default `ReqwestTransport` leaves reqwest's default, as core does today.
- **Generated regions are regenerated, never edited**: `python3 scripts/gen_registry.py --write` after a `[package.metadata.polyoxide]` change. `docs/ARCHITECTURE.md` and the spine are never edited; story 11 writes an amendment beside the spine.
- **A rules file or CLAUDE.md rule changes in the commit that changes the rule** (AD-21).
- **README text is prose with no code fence**, since the crate and module READMEs are doctests.
- **Regenerate `ci/dep-allowlists/`** with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` after a dependency change, and say why in the commit.
- **Live checks are the spec's (D17)** and need no credentials; a live run with credentials is never part of a story.
- **Nothing publishes.** No `cargo publish` without `--dry-run`, no `finish_release.sh`, no `gh workflow run Release`, no version bump.
- **Commit by pathspec**, conventional, naming the story, with `Cargo.lock` when cargo changes it, never `--no-verify`. End every commit message with the session's `Co-Authored-By` and `Claude-Session` lines.

## File structure

**Story 1: `polyoxide-rt`.**

- `polyoxide-rt/` (create): `Cargo.toml`, `README.md`, `src/lib.rs` (the re-exports, `BoxFuture`, `default_runtime`), `src/runtime.rs` (`Runtime`), `src/task.rs` (`spawn`, `Task`), `src/time.rs` (`timeout`, `interval`, `Interval`, `MissedTick`), `src/tokio.rs` (`TokioRuntime`, native only), `tests/tokio_runtime.rs`.
- `Cargo.toml` (modify): the member and the `[workspace.dependencies]` entries; `scripts/gen_registry.py` (modify): `FOUNDATION`; `.claude/rules/rt.md` (create) and CLAUDE.md's rules table and workspace list.

**Story 2: the runtime in core and on the HTTP side.**

- `polyoxide-core/src/{config,macros,client,send,health,hold,capacity,rate_limit}.rs` (modify): the injected runtime replaces tokio's clock, sleeps and spawns; `polyoxide-core/tests/runtime_injection.rs` (create).
- The HTTP-side sites in `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide-kalshi` (modify), read from the client's runtime.

**Story 3: `polyoxide_core::http`.** `polyoxide-core/src/lib.rs` re-exports `http`; every `reqwest::` vocabulary path in src, tests and examples moves to it.

**Story 4: the HTTP transport.**

- `polyoxide-core/src/transport.rs` (create): `HttpTransport`, `HttpRequest`, `ReqwestTransport`, `TransportError`, `TransportKind`; `polyoxide-core/src/response.rs` (create): `Response`; `polyoxide-core/tests/transport_injection.rs` (create).
- The loop, the request builder, health, errors, the config and every caller (modify).

**Story 5: the socket transport.**

- `polyoxide-ws/src/transport.rs` (create): `SocketTransport`, `SocketIo`, `BoxSocket`, `Capabilities`, `TungsteniteTransport`, `default_socket_transport`, `transport_setters!`; `polyoxide-ws/tests/transport_injection.rs` (create).
- The kit (modify) and every socket module in `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide-kalshi` (modify); `polyoxide-venue`'s socket classification (modify).

**Story 6: the wall clock and the gate.** `polyoxide-venue/src/time.rs`, `polyoxide-polymarket/src/shared/auth.rs` (modify); `scripts/runtime_gate.py`, `.github/scripts/tests/test_runtime_gate.py` (create).

**Story 7: the browser implementations.** `polyoxide-rt/src/browser.rs`, `polyoxide-core/src/fetch.rs`, `polyoxide-ws/src/browser.rs` (create); the target-gated manifests; `flake.nix` (the wasm32 target); `scripts/wasm_check.sh` (create).

**Story 8: capability adaptations.** `polyoxide-binance/src/usdm/ws/protocol.rs` and its live test; `scripts/capture_polymarket_sports.py`'s cadence mode, `polyoxide-polymarket/src/sports/` and the two `OBSERVED.md` sections.

**Story 9: the browser smoke suite.** `polyoxide-wasm-smoke/` (create): `Cargo.toml`, `src/lib.rs`, `src/bin/wasm-smoke-server.rs`, `tests/browser_smoke.rs`; `scripts/wasm_smoke.sh` (create); `.github/workflows/ci.yml` (the `wasm` job); `flake.nix` (the browser tools).

**Story 10: the nightly browser job.** `polyoxide-wasm-smoke/tests/browser_live.rs`, `scripts/wasm_live.sh` (create); `scripts/gen_registry.py` (the runner); `polyoxide-test-support` (wasm32 tags); the hosts' `OBSERVED.md`.

**Story 11: docs and the amendment.** `README.md`, the rules files and CLAUDE.md text not yet changed, and `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md` (create).

## Names contract

Every name one story defines for another, with its type, signature and visibility, and every text a later story quotes. A drafter who needs a cross-story name this section lacks stops and asks; it never invents one. Rust blocks are in the form `cargo fmt` leaves.

**Story 1 → Stories 2 to 11.** Crate `polyoxide-rt` (lib `polyoxide_rt`), version the workspace's, `[package.metadata.polyoxide]` filled as the other foundation crates' are, added to `gen_registry.py`'s `FOUNDATION` tuple. Every crate that uses it declares `polyoxide-rt = { workspace = true }` itself; core does not re-export it.

```rust
pub type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'static>>;
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

pub trait Runtime: Send + Sync + std::fmt::Debug + 'static {
    fn spawn(&self, task: BoxFuture<()>);
    fn sleep(&self, duration: std::time::Duration) -> BoxFuture<()>;
    fn now(&self) -> Instant;
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> {
        self.sleep(deadline.saturating_duration_since(self.now()))
    }
}

pub fn default_runtime() -> std::sync::Arc<dyn Runtime>;
pub fn spawn<T, F>(runtime: &dyn Runtime, future: F) -> Task<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static;
pub async fn timeout<F: std::future::Future>(
    runtime: &dyn Runtime,
    duration: std::time::Duration,
    future: F,
) -> Option<F::Output>;
pub fn interval(runtime: std::sync::Arc<dyn Runtime>, period: std::time::Duration) -> Interval;
```

- `pub struct Task<T>`: `impl<T> Future for Task<T> { type Output = Option<T>; }`, `pub fn abort(&self)`, `pub fn is_finished(&self) -> bool`; `Send` when `T: Send`; dropping it detaches the task.
- `pub struct Interval`: `pub async fn tick(&mut self) -> Instant`, `pub fn reset(&mut self)`, `pub fn period(&self) -> Duration`, `pub fn set_missed_tick(&mut self, behaviour: MissedTick)`.
- `#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)] #[non_exhaustive] pub enum MissedTick { #[default] Burst, Delay, Skip }`.
- Panic message for a zero period: `interval period must be non-zero`.
- `#[derive(Debug, Clone, Copy, Default)] pub struct TokioRuntime;` (`#[cfg(not(target_arch = "wasm32"))]`), `impl Runtime`.
- `.claude/rules/rt.md`, `paths:` `polyoxide-rt/**`.
- `[workspace.dependencies]` gains `polyoxide-rt` (path and version, as the other members'), `web-time = "1"` and `futures-channel = "0.3"`; a later story's manifest names them with `{ workspace = true }`.
- `default_runtime` has only its native arm here; Story 7 adds the wasm32 arm.

**Story 2 → Stories 4, 5, 7.**

- `polyoxide_core::ClientConfig` gains `pub runtime: Option<Arc<dyn polyoxide_rt::Runtime>>` (default `None`); `client_config_setters!` gains `pub fn runtime(mut self, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Self`, writing `self.$config.runtime`.
- `HttpClientBuilder::runtime(mut self, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Self`; with none set, `build` uses `polyoxide_rt::default_runtime()`.
- `HttpClient::runtime(&self) -> &Arc<dyn polyoxide_rt::Runtime>`, shared with `with_base_url` siblings.
- `ClobBuilder::build` passes its runtime to the Gamma and Data clients it builds, so they share the CLOB's (spec D3: every timer goes through the injected runtime); Story 4 does the same for its `http_transport`.

**Story 3 → Stories 4 to 11.**

- `pub use http;` in `polyoxide-core/src/lib.rs`, so `polyoxide_core::http::{Method, HeaderMap, HeaderValue, StatusCode, header}`.
- DRIFT row `R22` in `_bmad-output/specs/spec-venue-extensibility/divergences.md`, under the DRIFT table: "The wasm32 seams (spec `2026-11-02-wasm-runtime-transport-design.md`) change the paths and types tests name, never what they assert, except that a row asserting an `ApiError::Network` case no transport can produce goes with the variant, its decode case re-pinned on `Response::json`." Story 3's heading line for the table gains "R22 2026-11-02".

**Story 4 → Stories 7, 9, 10.** In `polyoxide-core`, exported at the crate root:

```rust
pub trait HttpTransport: Send + Sync + std::fmt::Debug + 'static {
    fn send(&self, request: HttpRequest) -> polyoxide_rt::BoxFuture<Result<Response, TransportError>>;
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HttpRequest {
    pub method: http::Method,
    pub url: url::Url,
    pub headers: http::HeaderMap,
    pub body: Option<bytes::Bytes>,
    pub timeout: std::time::Duration,
}
```

- `pub struct Response`: `pub fn new(status: StatusCode, headers: HeaderMap, url: Url, body: BoxFuture<Result<Bytes, TransportError>>) -> Self`, `pub fn status(&self) -> StatusCode`, `pub fn headers(&self) -> &HeaderMap`, `pub fn url(&self) -> &Url`, `pub async fn bytes(self) -> Result<Bytes, ApiError>`, `pub async fn text(self) -> Result<String, ApiError>`, `pub async fn json<T: DeserializeOwned>(self) -> Result<T, ApiError>`.
- `#[non_exhaustive] pub struct TransportError`: `pub fn new(kind: TransportKind, source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self`, `pub fn kind(&self) -> TransportKind`; `Display` `<kind>: <source>`; `Classify` per spec D5; listed in core's `const _` assertion.
- `#[derive(Debug, Clone, Copy, PartialEq, Eq)] #[non_exhaustive] pub enum TransportKind { Connect, Timeout, Body, Request, Other }`, its `Display` `connect`, `timeout`, `body`, `request`, `other`.
- `ApiError::Transport(#[from] TransportError)`, `#[error("Transport error: {0}")]`; `ApiError::Network` and `classify_reqwest` removed.
- `pub struct ReqwestTransport` (Story 7 gates it `#[cfg(not(target_arch = "wasm32"))]`): `pub fn new(client: reqwest::Client) -> Self`, `impl HttpTransport`.
- `ClientConfig` gains `pub http_transport: Option<Arc<dyn HttpTransport>>`; `client_config_setters!` gains `pub fn http_transport(mut self, transport: Arc<dyn HttpTransport>) -> Self`; `HttpClientBuilder::http_transport(..)` likewise. `ClobBuilder::build` passes its `http_transport` to the Gamma and Data clients it builds (spec §1.1: every HTTP exchange through the injected transport).
- `HttpRequest` has no public constructor: only core builds one. A test drives a transport through an `HttpClient` built with `http_transport(..)`.
- `pub use reqwest` and `ReqwestTransport` stay ungated here; Story 7 gates both to native and adds the wasm32 default transport.
- `TransportKind` maps from reqwest by `classify_reqwest`'s rule today (spec D5); a reqwest decode error (a corrupt compressed body) becomes `Body`, so `Network`, since a `TransportError` classes only as `InvalidRequest` or `Network`.
- Re-typed: `HttpClient::send -> Result<Response, ApiError>`, `Request::send_raw -> Result<Response, E>`, `Pong { round_trip, response: Response }`, `ErrorResponse::read(response: Response)`, `ApiError::from_response(response: Response)`, `retry_after_header(headers: &HeaderMap) -> Option<String>`; clob's `ListOrders::send_raw -> Result<polyoxide_core::Response, ClobError>`.

**Story 5 → Stories 7 to 10.** In `polyoxide-ws`, exported at the crate root:

```rust
pub trait SocketIo:
    futures_util::Stream<Item = Result<tungstenite::Message, tungstenite::Error>>
    + futures_util::Sink<tungstenite::Message, Error = tungstenite::Error>
    + Send
    + Unpin
{
}
impl<T> SocketIo for T where
    T: futures_util::Stream<Item = Result<tungstenite::Message, tungstenite::Error>>
        + futures_util::Sink<tungstenite::Message, Error = tungstenite::Error>
        + Send
        + Unpin
{
}
pub type BoxSocket = Box<dyn SocketIo>;

pub trait SocketTransport: Send + Sync + std::fmt::Debug + 'static {
    fn connect(
        &self,
        request: tungstenite::http::Request<()>,
        timeout: std::time::Duration,
        runtime: std::sync::Arc<dyn polyoxide_rt::Runtime>,
    ) -> polyoxide_rt::BoxFuture<Result<BoxSocket, WsError>>;
    fn capabilities(&self) -> Capabilities;
}
```

- `#[derive(Debug, Clone, Copy, PartialEq, Eq)] #[non_exhaustive] pub struct Capabilities { pub client_pings: bool, pub sees_server_pings: bool, pub handshake_headers: bool }`, `pub const fn new(client_pings: bool, sees_server_pings: bool, handshake_headers: bool) -> Self`, `pub const FULL: Capabilities`.
- `pub fn default_socket_transport() -> Arc<dyn SocketTransport>`.
- `#[cfg(not(target_arch = "wasm32"))] #[derive(Debug, Clone, Copy, Default)] pub struct TungsteniteTransport`: `pub const fn new() -> Self`, `pub const fn per_address() -> Self`.
- `Kind::Unsupported` (unit variant), read from `tungstenite::Error::Io` whose kind is `std::io::ErrorKind::Unsupported`; `retrying_can_fix` → `false`; `polyoxide_venue::impl_ws_classification!` → `InvalidRequest`. `ws_is_fault` stays `true` for it, as for `Url` and `AlreadyClosed`: the fault is the caller's, not the venue's; `false` stays reserved for a 451.
- Re-typed: `connect<R: IntoClientRequest + Unpin>(request: R, timeout: Duration) -> Result<BoxSocket, WsError>`, `connect_over(..) -> Result<BoxSocket, WsError>` (native only), `Connection::new(socket: BoxSocket, mode: Mode)`; `Socket` removed.
- `Connection::with_runtime(socket: BoxSocket, mode: Mode, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Connection`; `pub fn runtime(&self) -> &Arc<dyn polyoxide_rt::Runtime>` on `Connection`; `Connection::last_inbound` re-typed to return `polyoxide_rt::Instant` (its removals line in Story 5).
- `pub fn take_paced_id(&mut self) -> Option<u64>` on `Connection` (in `polyoxide-ws/src/request.rs`), for a synchronous hook such as `Protocol::ping`: `None` while `min_request_interval` has not passed since the last paced send on the connection's runtime clock; otherwise it takes the next request id and records now as the last paced send, so the next `request` waits from it. A test pins both arms.
- `default_socket_transport` and `TungsteniteTransport` are native-only here; Story 7 adds the wasm32 arm. `with_timeout(timeout, future)` keeps its signature, on `default_runtime()`.
- The CLOB keeps a thin `Protocol::connect` override on both its protocols that only maps `Kind::ConnectTimeout` to `ClobWsError::ConnectTimeout { url, timeout }` (spec D7); its `dial` goes.
- Bare clients without a builder (`PerpsWs`, `UsdmWs`, `SportsWs`, `Rtds::connect*`) dial on the defaults (the free `connect`, or a module's own `open_with(default_socket_transport(), default_runtime())`); a bare client reads its capabilities from `default_socket_transport()`.
- `Supervisor::runtime(self, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Self`, `Supervisor::socket_transport(self, transport: Arc<dyn SocketTransport>) -> Self`.
- `Protocol::connect(&mut self, transport: Arc<dyn SocketTransport>, runtime: Arc<dyn polyoxide_rt::Runtime>, timeout: Duration) -> impl Future<Output = Result<BoxSocket, Self::Error>> + Send`.
- `polyoxide_ws::transport_setters! { runtime; socket_transport; }`, invoked in a builder's `impl` as `timing_setters!` is, writing `self.runtime: Option<Arc<dyn Runtime>>` and `self.socket_transport: Option<Arc<dyn SocketTransport>>`; each socket builder's `build` falls back to `default_runtime()` and its default transport (the CLOB's: `TungsteniteTransport::per_address()`).

**Story 6 → Stories 9, 11.** `scripts/runtime_gate.py`, no arguments, exit 0 clean and 1 on a hit, each hit printed as `<path>:<line>: <banned path>`; the opt-out comment is `// runtime-gate: <reason>`.

**Story 7 → Stories 8 to 10.** Each type below is exported at its crate's root.

- `#[cfg(target_arch = "wasm32")] #[derive(Debug, Clone, Copy, Default)] pub struct BrowserRuntime;` in `polyoxide-rt`.
- `#[cfg(target_arch = "wasm32")] #[derive(Debug, Clone, Default)] pub struct FetchTransport` in `polyoxide-core`, `pub fn new() -> Self`.
- `#[cfg(target_arch = "wasm32")] #[derive(Debug, Clone, Copy, Default)] pub struct WebSocketTransport` in `polyoxide-ws`, `pub const fn new() -> Self`, capabilities all `false`.
- The `Kind::Unsupported` messages, verbatim: `a handshake header is not supported by the browser transport`, `a ping frame is not supported by the browser transport`, `a pong frame is not supported by the browser transport`, `a raw frame is not supported by the browser transport`.
- `scripts/wasm_check.sh`: no arguments, `set -euo pipefail`, sets `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'` itself, then for each of spec D1's crates, with its features named, `cargo check` and `cargo clippy -- -D warnings` with `--target wasm32-unknown-unknown`.

**Story 8 → Story 10.**

- The live test `live_usdm_ws::list_subscriptions_answers_as_a_keepalive`, `#[ignore]`d.
- `scripts/capture_polymarket_sports.py --cadence <minutes>`, writing its summary to stdout.
- Sections headed `## Browser transport (2026-11)` in `docs/specs/binance/OBSERVED.md` and `docs/specs/sports/OBSERVED.md`; story 10 appends its facts under the same heading in each host's file, creating it where absent.

**Story 9 → Story 10.**

- Crate `polyoxide-wasm-smoke` (`publish = false`), bin `wasm-smoke-server` taking one argument, the path of the env file it writes: the lines `POLYOXIDE_SMOKE_HTTP=<url>`, `POLYOXIDE_SMOKE_WS=<url>` and `POLYOXIDE_SMOKE_SLOW=<url>` (a second HTTP server for the slow route, so it delays no other request); `polyoxide-wasm-smoke/webdriver.json` holds headless Chrome's arguments.
- `scripts/wasm_smoke.sh`: no arguments; starts the server, exports the three variables, runs `cargo test -p polyoxide-wasm-smoke --target wasm32-unknown-unknown --test browser_smoke` under `wasm-bindgen-test-runner` with headless Chrome, and stops the server on exit.
- The CI job id `wasm`, name `WebAssembly`, running `scripts/wasm_check.sh` then `scripts/wasm_smoke.sh`.

**Story 10 → Story 11.** `scripts/wasm_live.sh`, no arguments, runs `browser_live` with `--features live`; the nightly job `live-wasm-browser`.

**Texts a later story quotes from an earlier one.** A fix job leaves each unchanged (only its own story's fixer may change it, and then says so):

- Story 1: `polyoxide-rt/Cargo.toml` as Task 1.6 leaves it (its `[dependencies]` and native `[target]` tables, its `notes`), quoted by Story 7.
- Story 2: the `ClobBuilder::build` Gamma and Data fallback blocks, quoted by Story 4; `polyoxide-polymarket/Cargo.toml`'s `polyoxide-rt` line and its comment, quoted by Story 5.
- Story 3: the `.claude/rules/ci-gates.md` `[[drift]]` bullet's R22 sentence, extended by Stories 4 and 5; Story 4's extension of it, extended by Story 5.
- Story 5: `UsdmWs::ping`'s head (`let runtime = …; let start = runtime.now();`) in `polyoxide-binance/src/usdm/ws/client.rs`, and sports `connect`'s head (`let runtime`, `let transport`, `open_with(..)`) and `supervised.rs`'s `stale` lines, quoted by Story 8; `polyoxide-ws/src/lib.rs`'s `pub use transport::…` line and `polyoxide-ws/Cargo.toml`'s `notes`, quoted by Story 7.
- Story 7: the crate-root exports and the `wasm_check.sh` crate list, used by Stories 9 and 10.

**Design stage:** /flow-draft session ef3b08b8-e823-4336-8d03-8fa4af2d7b11, 2026-11-02T09:04:10Z to 2026-11-02T10:05:03Z UTC: $61.24 logged, $61.66 with estimated thinking; 22069 plan lines, $2.78 per 1,000 lines.

## Stories

- **Story 1** — `polyoxide-rt`: the runtime trait, tasks, timeouts, intervals and `TokioRuntime` (spec D2).

  Creates the foundation crate with `Runtime`, `default_runtime`, `spawn` and `Task`, `timeout`, `interval` with `MissedTick`, and `TokioRuntime`, and wires it into the workspace: member, workspace dependencies, registry metadata, `FOUNDATION`, publish order (which `publish_order.py` computes), the allowlists, the rules file and CLAUDE.md's table and workspace list. The tests run on tokio's paused clock and pin each behaviour: sleep and `sleep_until`, `timeout` at zero with a ready future, the three missed-tick behaviours, the first tick at once, the zero-period panic, abort, `is_finished`, and that a dropped `Task` keeps running. No other crate uses it yet.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-rt/Cargo.toml`, `polyoxide-rt/README.md`, `polyoxide-rt/src/lib.rs`, `polyoxide-rt/src/runtime.rs`, `polyoxide-rt/src/task.rs`, `polyoxide-rt/src/time.rs`, `polyoxide-rt/src/tokio.rs`, `polyoxide-rt/tests/tokio_runtime.rs`, `Cargo.toml`, `Cargo.lock`, `scripts/gen_registry.py`, `.claude/rules/rt.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `README.md`, `scripts/venue_gate.py`, `.github/scripts/tests/test_publish_order.py`, `.github/scripts/tests/test_venue_gate.py`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 1 → Stories 2 to 11"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-rt && cargo clippy -p polyoxide-rt --all-targets -- -D warnings && python3 scripts/venue_gate.py && python3 scripts/gen_registry.py --check && python3 scripts/publish_order.py check-manifests && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
  - **Review:** deep (a new published foundation crate and its public API)
  - **Reading:** spec §1.1, §1.3, D1, D2, D10, D15, D16; `Cargo.toml`, `polyoxide-venue/Cargo.toml` (a foundation manifest's metadata), `scripts/gen_registry.py:100-160`, `scripts/venue_gate.py:1-60`, `.claude/rules/release.md`, `.claude/rules/ci-gates.md`, `.github/scripts/tests/test_claude_rules.py`, `polyoxide-core/src/hold.rs` (a paused-clock test style)

- **Story 2** — core and the HTTP side of the venue crates on the injected runtime (spec D3).

  Adds `ClientConfig::runtime`, the `runtime` setter in `client_config_setters!` and on `HttpClientBuilder`, and `HttpClient::runtime()`, and moves every clock read, sleep and spawn in core (the loop's retry sleep, holds, capacity buckets, the window quotas' waits, `health`'s stopwatch, the client's spawns) and in the HTTP modules of polymarket, binance and kalshi (their clients, throttles, limits and signer buckets) onto the client's runtime. governor keeps its quanta clock. A new test builds a client on a counting runtime over `TokioRuntime` and proves a 429 retry sleeps through it. Behaviour is unchanged; every existing suite passes as is.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/hold.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/tests/runtime_injection.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/api/health.rs`, `polyoxide-polymarket/src/data/client.rs`, `polyoxide-polymarket/src/gamma/client.rs`, `polyoxide-polymarket/src/gamma/api/health.rs`, `polyoxide-polymarket/src/perps/client.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/mod.rs`, `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/shared/signer_limit.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/shared/throttle.rs`, `.claude/rules/clients.md`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/binance.md`, `CLAUDE.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 2 → Stories 4, 5, 7"; uses Story 1's `Runtime`, `default_runtime`, `TokioRuntime`, `BoxFuture` and `Instant`
  - **Verify:** `cargo test -p polyoxide-core -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-kalshi --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the send loop, holds and rate limiters; landmine notes on quotas and waits apply)
  - **Reading:** spec D3, D12; `.claude/rules/send-loop.md`, `.claude/rules/rate-limits.md`, `.claude/rules/clients.md`; `polyoxide-core/src/send.rs`, `polyoxide-core/src/hold.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/client.rs:1-140,240-300`; the venue files in **Files**, at their clock and sleep sites

- **Story 3** — `polyoxide_core::http` and the move of every reqwest vocabulary path (spec D6).

  Re-exports `http` from core and moves every `reqwest::Method`, `reqwest::header::*`, `reqwest::StatusCode` and `HeaderValue` path in src, tests and examples to `polyoxide_core::http` (or `http` inside core). Types are identical, so nothing else changes. `reqwest::Client`, `Response`, `Error`, `RequestBuilder`, `redirect` and `get` stay for story 4, and alloy's reqwest in relay is untouched. Adds DRIFT row `R22` and a `[[drift]]` entry for each test body the move changes, and updates `send-loop.md`'s vocabulary sentence.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/hooks.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/tests/mock_request.rs`, `polyoxide-binance/examples/weight_probe.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/policy.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/auth.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `polyoxide-polymarket/examples/cf_burst_probe.rs`, `polyoxide-polymarket/examples/gamma_batch_ceiling.rs`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-polymarket/src/client.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/api/auth.rs`, `polyoxide-polymarket/src/clob/api/markets.rs`, `polyoxide-polymarket/src/clob/api/notifications.rs`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/authenticator.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/data/error.rs`, `polyoxide-polymarket/src/data/v2/error.rs`, `polyoxide-polymarket/src/gamma/api/markets.rs`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/perps/api/public.rs`, `polyoxide-polymarket/src/perps/error.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/config.rs`, `polyoxide-polymarket/src/relay/error.rs`, `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/shared/policy.rs`, `polyoxide-polymarket/src/shared/signer_limit.rs`, `polyoxide-polymarket/tests/polymarket_limits.rs`, `polyoxide-polymarket/tests/polymarket_throttle.rs`, `polyoxide-polymarket/tests/send_loop.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/tests/failure_tags.rs`, `polyoxide-test-support/tests/token_cost_throttle.rs`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`, `.claude/rules/send-loop.md`, `.claude/rules/ci-gates.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 3 → Stories 4 to 11"; uses nothing from earlier stories
  - **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** light (a path-only move; no behaviour changes)
  - **Reading:** spec D6, D12; `.claude/rules/send-loop.md` (the reqwest section), `.claude/rules/ci-gates.md` (test bodies), `ci/test-body-diff.toml:60-110`, `_bmad-output/specs/spec-venue-extensibility/divergences.md` (the DRIFT table's heading and its last three rows), `scripts/test_body_diff.py:1-80`

- **Story 4** — `HttpTransport`, `Response` and `TransportError` (spec D4, D5).

  Creates `transport.rs` and `response.rs`, moves `HttpClient` from its public `reqwest::Client` to a private `Arc<dyn HttpTransport>` built from the config (or injected), and has the loop build an `HttpRequest` with its URL, query and timeout, decide on the head, release the permit and then read an error body. Re-types every public item spec D11 lists for HTTP, replaces `ApiError::Network` and `classify_reqwest` with `ApiError::Transport` and `ReqwestTransport`'s private mapping, and updates every caller: gamma's `post_json`, relay's helper, clob's `send_raw`s, every ping's body check, the error decodes and the examples. A new test drives one retry and one hold through a recording transport. The header pins and query-setter suites pass unchanged on the default. Adds the removals lines, the drift entries and the rules text.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/response.rs`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/tests/health.rs`, `polyoxide-core/tests/mock_request.rs`, `polyoxide-core/tests/transport_injection.rs`, `polyoxide-binance/examples/weight_probe.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-polymarket/examples/cf_burst_probe.rs`, `polyoxide-polymarket/examples/gamma_batch_ceiling.rs`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/gamma/api/markets.rs`, `polyoxide-polymarket/src/perps/api/health.rs`, `polyoxide-polymarket/src/perps/error.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/error.rs`, `polyoxide-polymarket/tests/mock_clob.rs`, `polyoxide-polymarket/tests/mock_relay.rs`, `polyoxide-polymarket/tests/send_loop.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/tests/failure_tags.rs`, `polyoxide-core/README.md`, `polyoxide-polymarket/src/clob/client.rs`, `docs/s3-removals.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `ci/test-body-diff.toml`, `.claude/rules/send-loop.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`, `.claude/rules/live-tests.md`, `.claude/rules/ci-gates.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 4 → Stories 7, 9, 10"; uses Story 1's `BoxFuture`, Story 2's `HttpClient::runtime()` and `timeout`, and Story 3's `polyoxide_core::http` and row `R22`
  - **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test -p polyoxide-polymarket --no-default-features --features clob --test headers_clob && cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the send loop's seam, public error types and a breaking API change; landmine notes on gzip and foreign-error `From` impls apply)
  - **Reading:** spec D4, D5, D6, D11, D12; `.claude/rules/send-loop.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`; `polyoxide-core/src/send.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`; `polyoxide-polymarket/src/relay/client.rs` (its send helper), `polyoxide-polymarket/src/gamma/api/markets.rs` (`post_json`), `polyoxide-polymarket/src/clob/api/orders.rs:170-340`; `docs/s3-removals.md` (its format), `docs/MUTANTS.md` (rows citing core's loop or error)

- **Story 5** — `SocketTransport`, `BoxSocket` and the kit and every socket on the runtime (spec D3, D7).

  Creates `polyoxide-ws/src/transport.rs` with the trait, `BoxSocket`, `Capabilities`, `TungsteniteTransport` (its plain dial, and `per_address()`, the CLOB's dial moved unchanged), `default_socket_transport` and `transport_setters!`, and adds `Kind::Unsupported`. Re-types `connect`, `connect_over` and `Connection::new`, adds `Connection::with_runtime`, `Supervisor::runtime` and `Supervisor::socket_transport`, and gives `Protocol::connect` the transport and runtime. Moves every clock read, sleep and spawn in the kit and in every socket module (CLOB bare and supervised, perps, rtds, sports, Binance's router and forwarders, Kalshi) onto the injected runtime, and gives each socket builder the setters. The CLOB's `Protocol::connect` override and `dial` go; its builders default to `per_address()`. polymarket's `clob-ws` stops enabling `tokio/net`, and binance's `usdm-ws` stops enabling `tokio/rt`. A new test drives a `Supervisor` through a connect, a replay and a reconnect on an in-memory transport. Adds the venue macro's `Unsupported` row, D20's new text, the removals lines, the drift entries and the rules text.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-ws/Cargo.toml`, `polyoxide-ws/README.md`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/error.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs`, `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`, `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/helpers.rs`, `polyoxide-ws/src/test_server/tests.rs`, `polyoxide-ws/examples/close_reply_probe.rs`, `polyoxide-ws/tests/transport_injection.rs`, `polyoxide-venue/src/socket.rs`, `polyoxide-venue/src/ws_table.rs`, `polyoxide-venue/tests/ws_classification.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/ws/client.rs`, `polyoxide-polymarket/src/clob/ws/error.rs`, `polyoxide-polymarket/src/clob/ws/mod.rs`, `polyoxide-polymarket/src/clob/ws/scripted.rs`, `polyoxide-polymarket/src/clob/ws/supervised/market.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, `polyoxide-polymarket/src/perps/ws/builder.rs`, `polyoxide-polymarket/src/perps/ws/client.rs`, `polyoxide-polymarket/src/perps/ws/error.rs`, `polyoxide-polymarket/src/perps/ws/mod.rs`, `polyoxide-polymarket/src/perps/ws/protocol.rs`, `polyoxide-polymarket/src/perps/ws/scripted.rs`, `polyoxide-polymarket/src/rtds/client.rs`, `polyoxide-polymarket/src/rtds/error.rs`, `polyoxide-polymarket/src/rtds/supervisor.rs`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/error.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `polyoxide-polymarket/tests/bare_sports.rs`, `polyoxide-polymarket/tests/failure_tags_rtds.rs`, `polyoxide-polymarket/tests/failure_tags_sports.rs`, `polyoxide-polymarket/tests/live_clob_ws.rs`, `polyoxide-polymarket/tests/live_sports.rs`, `polyoxide-polymarket/tests/supervision_rtds.rs`, `polyoxide-polymarket/tests/supervision_sports.rs`, `polyoxide-polymarket/tests/trading_clob.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/scripted.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/live_usdm_ws.rs`, `polyoxide-binance/tests/supervision_usdm.rs`, `polyoxide-binance/tests/supervision_usdm_edges.rs`, `polyoxide-binance/tests/supervision_usdm_kit.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `docs/s3-removals.md`, `docs/MUTANTS.md`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`, `.claude/rules/sockets.md`, `.claude/rules/polymarket-clob-ws.md`, `.claude/rules/errors.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 5 → Stories 7 to 10"; uses Story 1's `Runtime`, `BoxFuture`, `default_runtime`, `spawn`, `Task`, `timeout`, `interval` and `Instant`, and Story 3's row `R22`
  - **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo hack check -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-ws --each-feature --no-dev-deps && python3 scripts/venue_gate.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the supervisor's guarantees, the CLOB's dial and a breaking API change; the `connect_async(` landmine applies)
  - **Reading:** spec D3, D7, D11, D12; `.claude/rules/sockets.md`, `.claude/rules/polymarket-clob-ws.md`, `.claude/rules/errors.md` (socket errors); `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs:120-200`, `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/error.rs`; `polyoxide-polymarket/src/clob/ws/client.rs:1-140` (the dial), `polyoxide-binance/src/usdm/ws/supervised.rs:200-260,480-500`; `polyoxide-venue/src/socket.rs`; `_bmad-output/specs/spec-venue-extensibility/divergences.md` (row D20)

- **Story 6** — The wall clock through `web-time`, and `runtime_gate.py` (spec D3).

  `UnixMillis::now` and `shared::auth::current_timestamp` read `web_time::SystemTime` (venue gains a wasm32-only `web-time`; polymarket reads `polyoxide_rt::SystemTime`). Adds `scripts/runtime_gate.py`, scoped as spec D3 says, and its pytest, which proves it fires on each banned path, skips `#[cfg(test)]` modules, the test server and the `scripted.rs` fixtures, and honours the opt-out. Fixes whatever stragglers it finds after stories 2 and 5; any it finds is a deviation to report. Updates `ci-gates.md` and `rt.md`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-venue/Cargo.toml`, `polyoxide-venue/README.md`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/time.rs`, `polyoxide-polymarket/src/shared/auth.rs`, `.claude/rules/venue-traits.md`, `CLAUDE.md`, `README.md`, `scripts/runtime_gate.py`, `.github/scripts/tests/test_runtime_gate.py`, `.claude/rules/ci-gates.md`, `.claude/rules/rt.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 6 → Stories 9, 11"; uses Story 1's `SystemTime` and `UNIX_EPOCH`
  - **Verify:** `python3 scripts/runtime_gate.py && cargo test -p polyoxide-venue -p polyoxide-polymarket --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && bash -c 'cd .github/scripts && uv run pytest tests/test_runtime_gate.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** light (the signing timestamp keeps its value; the gate is a script with its own tests)
  - **Reading:** spec D3, D10; `scripts/venue_gate.py`, `scripts/live_unwraps.py:1-80` (the opt-out convention), `.github/scripts/tests/` (a venue-gate test as the model), `polyoxide-venue/src/time.rs`, `polyoxide-polymarket/src/shared/auth.rs`

- **Story 7** — The target gating and the three browser implementations (spec D8, D10).

  Moves each native-only dependency into a `cfg(not(target_arch = "wasm32"))` table and adds the wasm32-only ones (spec D10), with `getrandom`'s `wasm_js` in `polyoxide-rt`. Adds `BrowserRuntime` (`spawn_local`; gloo-timers sleeps chained in steps of at most `i32::MAX` ms; `performance.now`), `FetchTransport` (reqwest's wasm client, `SendWrapper`'d, the timeout per request, failures mapped per spec D8) and `WebSocketTransport` (web-sys, `binaryType` arraybuffer, events through an unbounded channel, closes mapped to 1000 or 3000–4999, headers and control frames refused with the contract's messages), each the wasm32 default of `default_runtime`, the config's transport and `default_socket_transport`. Adds the wasm32 target to the flake's toolchain and `scripts/wasm_check.sh`, which passes for spec D1's whole set. Native builds are unchanged.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-rt/Cargo.toml`, `polyoxide-rt/src/lib.rs`, `polyoxide-rt/src/browser.rs`, `polyoxide-core/Cargo.toml`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/fetch.rs`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/error.rs`, `polyoxide-ws/Cargo.toml`, `polyoxide-ws/README.md`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/browser.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/tls.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/error.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-venue/Cargo.toml`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-binance/Cargo.toml`, `Cargo.toml`, `Cargo.lock`, `flake.nix`, `scripts/wasm_check.sh`, `.claude/rules/rt.md`, `.claude/rules/sockets.md`, `.claude/rules/send-loop.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `README.md`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 7 → Stories 8 to 10"; uses Story 1's `Runtime` and `default_runtime`, Story 4's `HttpTransport`, `Response` and `TransportError`, and Story 5's `SocketTransport`, `Capabilities` and `Kind::Unsupported`
  - **Verify:** `bash scripts/wasm_check.sh && cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && msrv-cargo check --workspace --all-features && bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the browser implementations' contracts and `SendWrapper` soundness)
  - **Reading:** spec D1, D7, D8, D10, D13; `polyoxide-rt/src/*.rs`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/response.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/error.rs`, the five manifests in **Files**, `flake.nix:20-70`; `.claude/rules/sockets.md` (TLS needs a nudge)

- **Story 8** — The Binance keepalive and sports liveness on a transport without pings (spec D9, D17).

  Binance's supervised protocol pings with `LIST_SUBSCRIPTIONS` through its request correlation when its transport's `client_pings` is false, and its bare `ping()` refuses with `Kind::Unsupported` there; a scripted-server test pins both on a test transport reporting browser capabilities. The live test checks the request on `fstream` within spec D17's limits, and its result picks D9's main rule or its fallback. Sports: adds a `--cadence` mode to the capture script, runs it within D17's limits, records the result, then applies D9's rule for a transport whose `sees_server_pings` is false, with a scripted test. Both findings go under the contract's `OBSERVED.md` headings, and `binance.md` and `polymarket-sports.md` record the rules. A D17 stop goes to the owner.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/tests/supervision_usdm_kit.rs`, `polyoxide-binance/tests/live_usdm_ws.rs`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `polyoxide-polymarket/tests/supervision_sports.rs`, `scripts/capture_polymarket_sports.py`, `docs/specs/binance/OBSERVED.md`, `docs/specs/sports/OBSERVED.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `.claude/rules/binance.md`, `.claude/rules/polymarket-sports.md`
  - **Interfaces:** exposes "Story 8 → Story 10"; uses Story 5's `Capabilities`, `Kind::Unsupported` and `transport_setters!`
  - **Verify:** `cargo test -p polyoxide-binance -p polyoxide-polymarket --all-features && cargo test -p polyoxide-binance --features usdm-ws --test live_usdm_ws -- --ignored list_subscriptions_answers_as_a_keepalive && python3 scripts/live_unwraps.py && bash scripts/wasm_check.sh`
  - **Review:** deep (liveness rules, from live measurements)
  - **Reading:** spec D9, D17; `.claude/rules/binance.md`, `.claude/rules/polymarket-sports.md`, `.claude/rules/live-tests.md`; `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/client.rs:260-320`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `scripts/capture_polymarket_sports.py`, `scripts/capture_common.py`; the last 40 lines of each `OBSERVED.md` in **Files**

- **Story 9** — The per-push browser smoke suite and the `wasm` CI job (spec D13).

  Creates `polyoxide-wasm-smoke`: its native `wasm-smoke-server` (mockito routes with CORS headers, the two 429s, one scripted socket on the kit's test server) and its wasm32 `tests/browser_smoke.rs`, which runs in a dedicated Worker and covers `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` as spec D13 lists. Adds `scripts/wasm_smoke.sh`, the flake's chromium, chromedriver and `wasm-bindgen-cli` (the version `Cargo.lock` pins), and the `wasm` job in `ci.yml` (wasm32 target, `wasm-bindgen-cli`, the runner's Chrome, then both scripts), and adds `runtime_gate.py` to the CI scripts job. Updates the jobs table in CLAUDE.md and `ci-gates.md`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-wasm-smoke/Cargo.toml`, `polyoxide-wasm-smoke/src/lib.rs`, `polyoxide-wasm-smoke/src/bin/wasm-smoke-server.rs`, `polyoxide-wasm-smoke/tests/browser_smoke.rs`, `polyoxide-wasm-smoke/webdriver.json`, `scripts/wasm_smoke.sh`, `Cargo.toml`, `Cargo.lock`, `flake.nix`, `.github/workflows/ci.yml`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `README.md`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 9 → Story 10"; uses Story 6's `runtime_gate.py` and Story 7's `BrowserRuntime`, `FetchTransport`, `WebSocketTransport`, the contract's messages and `scripts/wasm_check.sh`
  - **Verify:** `bash scripts/wasm_check.sh && bash scripts/wasm_smoke.sh && cargo test --workspace --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
  - **Review:** deep (a new CI gate and the browser behaviour it proves)
  - **Reading:** spec D13; `.github/workflows/ci.yml`, `.claude/rules/ci-gates.md`, `.claude/rules/testing.md`, `flake.nix`, `polyoxide-ws/src/test_server/mod.rs:1-120`, `polyoxide-rt/src/browser.rs`, `polyoxide-core/src/fetch.rs`, `polyoxide-ws/src/browser.rs`

- **Story 10** — The nightly live browser job (spec D14, D17).

  Adds `tests/browser_live.rs` behind a `live` feature (per host in spec §1.4: CORS, exposed headers, the socket upgrade, one public GET, one socket and Binance's keepalive, within D17's limits and with its retry and tag rules), `scripts/wasm_live.sh`, a wasm-bindgen runner in `gen_registry.py` and the `live-wasm-browser` job it generates in `nightly-behavioral.yml`. `polyoxide-test-support`'s tags build on wasm32, with its keychain loaders native-only. The story proves a tag line from a failing browser test reaches the runner's output. Records spec §1.4's browser facts in each host's `OBSERVED.md` under the contract's heading.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-wasm-smoke/Cargo.toml`, `polyoxide-wasm-smoke/tests/browser_live.rs`, `scripts/wasm_live.sh`, `scripts/gen_registry.py`, `.github/workflows/nightly-behavioral.yml`, `CLAUDE.md`, `SELF-HEALING.md`, `polyoxide-test-support/Cargo.toml`, `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/src/tag.rs`, `.github/scripts/tests/test_gen_registry.py`, `.github/scripts/tests/test_live_registry.py`, `docs/specs/clob/OBSERVED.md`, `docs/specs/gamma/OBSERVED.md`, `docs/specs/data-v2/OBSERVED.md`, `docs/specs/perps/OBSERVED.md`, `docs/specs/binance/OBSERVED.md`, `docs/specs/kalshi/OBSERVED.md`, `docs/specs/sports/OBSERVED.md`, `docs/specs/rtds/OBSERVED.md`, `.claude/rules/live-tests.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 10 → Story 11"; uses Story 7's implementations, Story 8's keepalive and `OBSERVED.md` headings, and Story 9's crate and env-file contract
  - **Verify:** `bash scripts/wasm_live.sh && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
  - **Review:** deep (a nightly classifier contract and live limits)
  - **Reading:** spec §1.4, D14, D17; `.claude/rules/live-tests.md`, `scripts/gen_registry.py` (the nightly job generation), `.github/workflows/nightly-behavioral.yml` (its generated region), `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/Cargo.toml`, `polyoxide-wasm-smoke/tests/browser_smoke.rs`

- **Story 11** — The README, the remaining rules text and the spine amendment (spec D15).

  Writes README.md's WebAssembly section: spec D1's set and contexts, the getrandom flag, injection and the defaults by target, and every browser gap spec D15 lists. Brings CLAUDE.md and the rules files in line with any rule earlier stories left unstated. Writes the spine amendment, `spine-amendments/wasm-runtime-transport.md`, in the format of `epic-9.md`: `polyoxide-rt` in the foundation, an AD for the three injection seams and the `SendWrapper` containment, and the dependency changes. Edits neither the spine nor `docs/ARCHITECTURE.md`.

  - **Developer:** intermediate-dev
  - **Files:** `README.md`, `CLAUDE.md`, `.claude/rules/rt.md`, `.claude/rules/sockets.md`, `.claude/rules/send-loop.md`, `.claude/rules/clients.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md`
  - **Interfaces:** exposes nothing; uses every earlier story's names as built
  - **Verify:** `cargo test --doc --workspace --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
  - **Review:** light (docs and an amendment for the owner to merge; its one landmine marker, `From<reqwest::Error>`, is README prose stating the rule, not code)
  - **Reading:** spec D1, D15; README.md, CLAUDE.md, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md`, the rules files in **Files**

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->

## Story 1: `polyoxide-rt`: the runtime trait, tasks, timeouts, intervals and `TokioRuntime`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-rt/Cargo.toml`, `polyoxide-rt/README.md`, `polyoxide-rt/src/lib.rs`, `polyoxide-rt/src/runtime.rs`, `polyoxide-rt/src/task.rs`, `polyoxide-rt/src/time.rs`, `polyoxide-rt/src/tokio.rs`, `polyoxide-rt/tests/tokio_runtime.rs`, `Cargo.toml`, `Cargo.lock`, `scripts/gen_registry.py`, `.claude/rules/rt.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `README.md`, `scripts/venue_gate.py`, `.github/scripts/tests/test_publish_order.py`, `.github/scripts/tests/test_venue_gate.py`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 1 → Stories 2 to 11"; uses nothing from earlier stories
- **Verify:** `cargo test -p polyoxide-rt && cargo clippy -p polyoxide-rt --all-targets -- -D warnings && python3 scripts/venue_gate.py && python3 scripts/gen_registry.py --check && python3 scripts/publish_order.py check-manifests && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
- **Review:** deep (a new published foundation crate and its public API)

This story creates the foundation crate `polyoxide-rt` and wires it into the workspace, so that every later story can write its timers, clock reads and spawns against an injected `Runtime`. It holds the `Runtime` trait (a detached `spawn`, `sleep`, `now`, and a defaulted `sleep_until`), `default_runtime`, `spawn` with the `Task` it returns, `timeout`, `interval` with `Interval` and `MissedTick`, and `TokioRuntime`, which runs on tokio's own clock so the paused-clock tests of the other crates keep working. The crate depends on nothing in the workspace and names no venue. Its dependencies are `web-time` (the clock types, std's own on native), `futures-util` and `futures-channel` (the `Task`), and, off wasm32 only, tokio's `rt` and `time`. Nothing else uses the crate yet. `default_runtime` has only its native arm here; Story 7's task that adds `BrowserRuntime` adds the `wasm32` arm. The workspace plumbing lands first (member, workspace dependencies, registry metadata, `FOUNDATION`, publish order), the code in four tasks, then the crate README, the rules file, CLAUDE.md's rules table and venue-gate sentence, and the dependency allowlist. No code here holds a landmine marker, so the story needs no `effort: xhigh`.

### Task 1.1: The crate, its registry entry and the workspace plumbing

**Files:**
- Create: `polyoxide-rt/Cargo.toml`
- Create: `polyoxide-rt/src/lib.rs`
- Create: `polyoxide-rt/tests/tokio_runtime.rs`
- Modify: `Cargo.toml` (`[workspace] members`, `[workspace.dependencies]`)
- Modify: `scripts/gen_registry.py:124`
- Modify: `.github/scripts/tests/test_publish_order.py:934-937`
- Modify: `.github/scripts/tests/test_venue_gate.py` (after `test_a_clean_tree_passes`)
- Regenerate: `README.md`, `CLAUDE.md` (generated regions), `Cargo.lock`

- [ ] **Step 1: Write the failing tests**

In `.github/scripts/tests/test_publish_order.py`, replace

```python
    # polyoxide-venue depends on nothing, and core on venue alone.
    assert order[:2] == ["polyoxide-venue", "polyoxide-core"]
```

with

```python
    # polyoxide-rt and polyoxide-venue depend on nothing, and ties break by name; core needs
    # venue (and, from a later story, rt).
    assert order[:3] == ["polyoxide-rt", "polyoxide-venue", "polyoxide-core"]
```

In `.github/scripts/tests/test_venue_gate.py`, after

```python
    result = run(tree(tmp_path, {"polyoxide-core/src/send.rs": "// One send loop.\n"}))
    assert (result.status, result.hits, result.stale) == (0, [], [])
```

insert

```python


def test_the_runtime_crate_is_read_by_the_gate(tmp_path: Path) -> None:
    root = tree(tmp_path, {"polyoxide-rt/src/time.rs": "/// A timer.\n/// As binance does.\n"})
    assert findings(run(root)) == [("polyoxide-rt/src/time.rs", 2, "binance", "binance")]
```

Create `polyoxide-rt/tests/tokio_runtime.rs`:

```rust
//! The runtime seam, driven through `TokioRuntime` on tokio's paused clock.

use polyoxide_rt::{Instant, SystemTime, UNIX_EPOCH};

#[test]
fn the_clock_types_are_stds_on_native() {
    let _: std::time::Instant = Instant::now();
    let _: std::time::SystemTime = SystemTime::now();
    assert_eq!(UNIX_EPOCH, std::time::UNIX_EPOCH);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_publish_order.py::test_real_workspace_order tests/test_venue_gate.py::test_the_runtime_crate_is_read_by_the_gate -q'`

Expected: two failures. The first prints `AssertionError: assert ['polyoxide-venue', 'polyoxide-core', 'polyoxide-ws'] == ['polyoxide-rt', 'polyoxide-venue', 'polyoxide-core']`; the second prints `assert [] == [('polyoxide-rt/src/time.rs', 2, 'binance', 'binance')]`, because the gate does not read the crate yet.

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `error: package ID specification 'polyoxide-rt' did not match any packages`.

- [ ] **Step 3: Implement**

In the root `Cargo.toml`, replace

```toml
  "polyoxide-py",
  "polyoxide-test-support",
```

with

```toml
  "polyoxide-py",
  "polyoxide-rt",
  "polyoxide-test-support",
```

and replace

```toml
polyoxide-venue = { path = "polyoxide-venue", version = "0.44.0" }
```

with

```toml
polyoxide-rt = { path = "polyoxide-rt", version = "0.44.0" }
polyoxide-venue = { path = "polyoxide-venue", version = "0.44.0" }
```

and replace

```toml
fastrand = "2"
```

with

```toml
fastrand = "2"
# The clock vocabulary of `polyoxide-rt`: std's own types on native, a browser-backed
# `Instant` and `SystemTime` on wasm32.
web-time = "1"
```

In `scripts/gen_registry.py`, replace

```python
FOUNDATION = ("polyoxide-venue", "polyoxide-core", "polyoxide-ws", "polyoxide-test-support")
```

with

```python
FOUNDATION = ("polyoxide-venue", "polyoxide-core", "polyoxide-ws", "polyoxide-rt",
              "polyoxide-test-support")
```

Create `polyoxide-rt/Cargo.toml`:

```toml
[package]
name = "polyoxide-rt"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "The runtime seam the polyoxide crates share: an injected clock, sleep and spawn, with the tasks, timeouts and intervals built on them"
keywords = ["async", "runtime", "timer", "wasm"]
categories = ["asynchronous"]

# Nothing in the workspace, so it publishes first and every crate can use it
# (AD-2). Each task below adds the dependencies it first needs.
[dependencies]
web-time = { workspace = true }

# Registration, read by scripts/gen_registry.py: the README line and the notes
# CLAUDE.md's crate list carries. Run `python3 scripts/gen_registry.py --write`
# after an edit.
[package.metadata.polyoxide]
readme = "The runtime seam: an injected clock, sleep and spawn, with the tasks, timeouts and intervals built on them"
notes = ["names no venue, which `scripts/venue_gate.py` holds"]
```

Create `polyoxide-rt/src/lib.rs`:

```rust
//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates: the clock types and the future
//! alias every other module of this crate is written against.

#![warn(missing_docs)]

use std::future::Future;
use std::pin::Pin;

pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

/// An owned, sendable future: what a runtime spawns and sleeps on.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
```

Then:

1. `nix develop --command cargo check -p polyoxide-rt` (adds the crate to `Cargo.lock`).
2. `nix develop --command python3 scripts/gen_registry.py --write` (adds the crate's row to `README.md`, and its line, "Eleven crates" and the new publish order to CLAUDE.md's generated regions; edit none of them by hand).
3. `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_publish_order.py tests/test_venue_gate.py -q'`

Expected: `passed`, no failures.

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `running 1 test` / `test the_clock_types_are_stds_on_native ... ok` / `test result: ok. 1 passed; 0 failed`.

Run: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/publish_order.py check-manifests'`

Expected: each exits 0 with no diagnostics.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-rt/Cargo.toml polyoxide-rt/src/lib.rs polyoxide-rt/tests/tokio_runtime.rs
git commit -m "feat(rt): add the polyoxide-rt crate and its workspace plumbing (story 1)" -- polyoxide-rt Cargo.toml Cargo.lock scripts/gen_registry.py README.md CLAUDE.md .github/scripts/tests/test_publish_order.py .github/scripts/tests/test_venue_gate.py
```

### Task 1.2: `Runtime`, `TokioRuntime` and `default_runtime`

**Files:**
- Create: `polyoxide-rt/src/runtime.rs`
- Create: `polyoxide-rt/src/tokio.rs`
- Modify: `polyoxide-rt/src/lib.rs`
- Modify: `polyoxide-rt/Cargo.toml`
- Test: `polyoxide-rt/tests/tokio_runtime.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-rt/Cargo.toml`, add the test dependencies directly before the `# Registration, read by scripts/gen_registry.py` comment (the tests need tokio's macros and paused clock; Step 3 adds the runtime's own tokio dependency):

```toml
[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt", "test-util", "time"] }

```

In `polyoxide-rt/tests/tokio_runtime.rs`, replace

```rust
use polyoxide_rt::{Instant, SystemTime, UNIX_EPOCH};
```

with

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use polyoxide_rt::{
    default_runtime, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime, UNIX_EPOCH,
};

/// The paused clock wakes a timer on its millisecond, so a measured wait is its
/// duration plus less than this.
const TICK: Duration = Duration::from_millis(5);

fn assert_near(actual: Duration, expected: Duration) {
    assert!(
        actual >= expected && actual < expected + TICK,
        "measured {actual:?}, expected {expected:?}"
    );
}

/// A runtime that records what it is asked, to pin the trait's own behaviour:
/// its sleeps are always ready, its clock stands ten seconds after `base`, and
/// its spawn is tokio's.
#[derive(Debug)]
struct Scripted {
    base: Instant,
    spawned: AtomicUsize,
    slept: Mutex<Vec<Duration>>,
}

impl Scripted {
    fn new() -> Self {
        Self {
            base: Instant::now(),
            spawned: AtomicUsize::new(0),
            slept: Mutex::new(Vec::new()),
        }
    }
}

impl Runtime for Scripted {
    fn spawn(&self, task: BoxFuture<()>) {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.slept.lock().unwrap().push(duration);
        Box::pin(async {})
    }

    fn now(&self) -> Instant {
        self.base + Duration::from_secs(10)
    }
}
```

and append at the end of the file:

```rust

#[tokio::test(start_paused = true)]
async fn sleep_completes_after_its_duration() {
    let rt = TokioRuntime;
    let start = rt.now();
    rt.sleep(Duration::from_secs(5)).await;
    assert_near(rt.now() - start, Duration::from_secs(5));
}

#[tokio::test(start_paused = true)]
async fn sleep_until_waits_for_the_deadline() {
    let rt = TokioRuntime;
    let start = rt.now();
    rt.sleep_until(start + Duration::from_secs(3)).await;
    assert_near(rt.now() - start, Duration::from_secs(3));
}

#[tokio::test(start_paused = true)]
async fn sleep_until_a_passed_deadline_returns_at_once() {
    let rt = TokioRuntime;
    let deadline = rt.now();
    tokio::time::advance(Duration::from_secs(9)).await;
    let before = rt.now();
    rt.sleep_until(deadline).await;
    assert!(
        rt.now() - before < TICK,
        "a deadline already passed still waited {:?}",
        rt.now() - before
    );
}

#[tokio::test(start_paused = true)]
async fn the_default_runtime_runs_on_tokios_clock() {
    let rt = default_runtime();
    let start = rt.now();
    tokio::time::advance(Duration::from_secs(7)).await;
    assert_near(rt.now() - start, Duration::from_secs(7));
    rt.sleep(Duration::from_secs(2)).await;
    assert_near(rt.now() - start, Duration::from_secs(9));
}

#[test]
fn the_default_sleep_until_sleeps_the_time_left() {
    let rt = Scripted::new();
    drop(rt.sleep_until(rt.base + Duration::from_secs(14)));
    assert_eq!(*rt.slept.lock().unwrap(), vec![Duration::from_secs(4)]);
}

#[test]
fn the_default_sleep_until_a_passed_deadline_sleeps_zero() {
    let rt = Scripted::new();
    drop(rt.sleep_until(rt.base + Duration::from_secs(3)));
    assert_eq!(*rt.slept.lock().unwrap(), vec![Duration::ZERO]);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `error[E0432]: unresolved imports `polyoxide_rt::default_runtime`, `polyoxide_rt::Runtime`, `polyoxide_rt::TokioRuntime``.

- [ ] **Step 3: Implement**

In `polyoxide-rt/Cargo.toml`, replace

```toml
[dependencies]
web-time = { workspace = true }
```

with

```toml
[dependencies]
web-time = { workspace = true }

# Native only (spec D10): `rt` and `time` are `TokioRuntime`'s spawn and clock.
# The browser has its own runtime and builds no tokio.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { workspace = true, features = ["rt", "time"] }
```

Create `polyoxide-rt/src/runtime.rs`:

```rust
//! The `Runtime` trait.

use std::fmt;
use std::time::Duration;

use crate::{BoxFuture, Instant};

/// What a polyoxide crate needs of an async runtime: a detached spawn, a
/// sleep, and a monotonic clock. Everything else in this crate (`spawn`'s
/// `Task`, `timeout`, `Interval`) is built on these, so an implementor writes
/// only these.
///
/// A client takes its runtime from its builder, and falls back to
/// `default_runtime()`; nothing is process-wide.
pub trait Runtime: Send + Sync + fmt::Debug + 'static {
    /// Run `task` to completion, detached.
    fn spawn(&self, task: BoxFuture<()>);

    /// Complete after `duration`. Any duration, `Duration::MAX` included, is
    /// valid.
    fn sleep(&self, duration: Duration) -> BoxFuture<()>;

    /// The runtime's monotonic clock.
    fn now(&self) -> Instant;

    /// Complete at `deadline`, at once when it has passed.
    fn sleep_until(&self, deadline: Instant) -> BoxFuture<()> {
        self.sleep(deadline.saturating_duration_since(self.now()))
    }
}
```

Create `polyoxide-rt/src/tokio.rs`:

```rust
//! `TokioRuntime`, the native runtime.

use std::time::Duration;

use crate::{BoxFuture, Instant, Runtime};

/// The runtime over tokio's spawn, sleep and clock.
///
/// It reads tokio's clock, so a test on `tokio::time::pause` (or
/// `#[tokio::test(start_paused = true)]`) drives it. `spawn` panics outside a
/// tokio runtime, as `tokio::spawn` does.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRuntime;

impl Runtime for TokioRuntime {
    fn spawn(&self, task: BoxFuture<()>) {
        ::tokio::spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        Box::pin(::tokio::time::sleep(duration))
    }

    fn now(&self) -> Instant {
        ::tokio::time::Instant::now().into_std()
    }
}
```

Replace the whole of `polyoxide-rt/src/lib.rs` with:

```rust
//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates: the `Runtime` trait, which a
//! crate takes instead of calling a runtime's own functions, and `TokioRuntime`,
//! its native implementation.

#![warn(missing_docs)]

mod runtime;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use runtime::Runtime;
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
pub use self::tokio::TokioRuntime;

/// An owned, sendable future: what a [`Runtime`] spawns and sleeps on.
pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

/// The runtime a client uses when none is injected: `TokioRuntime`.
///
/// Only its native arm exists so far; the wasm32 arm arrives with the browser
/// runtime.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}
```

Run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `running 7 tests`, each `ok`, `test result: ok. 7 passed; 0 failed`.

Run: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py'`

Expected: exits 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-rt/src/runtime.rs polyoxide-rt/src/tokio.rs
git commit -m "feat(rt): add Runtime, TokioRuntime and default_runtime (story 1)" -- polyoxide-rt Cargo.lock
```

### Task 1.3: `spawn` and `Task`

**Files:**
- Create: `polyoxide-rt/src/task.rs`
- Modify: `polyoxide-rt/src/lib.rs`
- Modify: `polyoxide-rt/Cargo.toml`
- Modify: `Cargo.toml` (`[workspace.dependencies]`)
- Test: `polyoxide-rt/tests/tokio_runtime.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-rt/tests/tokio_runtime.rs`, replace

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use polyoxide_rt::{
    default_runtime, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime, UNIX_EPOCH,
};
```

with

```rust
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use polyoxide_rt::{
    default_runtime, spawn, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime, UNIX_EPOCH,
};
```

and append at the end of the file:

```rust

#[tokio::test(start_paused = true)]
async fn spawn_goes_through_the_runtime_it_is_given() {
    let rt = Scripted::new();
    let task = spawn(&rt, async { 5 });
    assert_eq!(rt.spawned.load(Ordering::SeqCst), 1);
    assert_eq!(task.await, Some(5));
}

#[tokio::test(start_paused = true)]
async fn a_task_yields_its_output() {
    let task = spawn(&TokioRuntime, async {
        TokioRuntime.sleep(Duration::from_secs(2)).await;
        "done"
    });
    assert_eq!(task.await, Some("done"));
}

#[tokio::test(start_paused = true)]
async fn abort_stops_a_task_before_its_side_effect() {
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let mut task = spawn(&TokioRuntime, async move {
        TokioRuntime.sleep(Duration::from_secs(10)).await;
        flag.store(true, Ordering::SeqCst);
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished(), "the task is still sleeping");

    task.abort();
    assert_eq!((&mut task).await, None);
    TokioRuntime.sleep(Duration::from_secs(60)).await;

    assert!(!ran.load(Ordering::SeqCst), "an aborted task still ran to its end");
    assert!(task.is_finished(), "an aborted task is not finished");
}

#[tokio::test(start_paused = true)]
async fn is_finished_turns_true_when_the_task_ends() {
    let task = spawn(&TokioRuntime, async {
        TokioRuntime.sleep(Duration::from_secs(5)).await;
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished(), "the task is still sleeping");

    // The task is not polled here, so only the task's own end can have set it.
    TokioRuntime.sleep(Duration::from_secs(6)).await;
    assert!(task.is_finished(), "a task that ended is not finished");
    assert_eq!(task.await, Some(()));
}

#[tokio::test(start_paused = true)]
async fn a_dropped_task_keeps_running() {
    let ran = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&ran);
    let task = spawn(&TokioRuntime, async move {
        TokioRuntime.sleep(Duration::from_secs(5)).await;
        flag.store(true, Ordering::SeqCst);
    });
    drop(task);

    TokioRuntime.sleep(Duration::from_secs(10)).await;
    assert!(ran.load(Ordering::SeqCst), "dropping the task stopped it");
}

async fn fail_on_purpose() -> u32 {
    panic!("the task fails on purpose")
}

#[tokio::test(start_paused = true)]
async fn a_panicking_task_ends_with_no_output() {
    let task = spawn(&TokioRuntime, fail_on_purpose());
    TokioRuntime.sleep(Duration::from_secs(1)).await;
    assert!(task.is_finished(), "a task that panicked is not finished");
    assert_eq!(task.await, None);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `error[E0432]: unresolved import `polyoxide_rt::spawn``.

- [ ] **Step 3: Implement**

In the root `Cargo.toml`, replace

```toml
futures-util = "0.3"
```

with

```toml
futures-util = "0.3"
# The oneshot of `polyoxide-rt`'s `Task`.
futures-channel = "0.3"
```

In `polyoxide-rt/Cargo.toml`, replace

```toml
[dependencies]
web-time = { workspace = true }
```

with

```toml
[dependencies]
# `Task`: an abortable future, and a oneshot for its output.
futures-channel = { workspace = true }
futures-util = { workspace = true }
web-time = { workspace = true }
```

Create `polyoxide-rt/src/task.rs`:

```rust
//! `spawn` and the `Task` it returns.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_channel::oneshot;
use futures_util::future::{AbortHandle, Abortable};

use crate::Runtime;

/// A spawned future, as a future of its output.
///
/// It resolves to `Some(output)`, or to `None` when the task was aborted or
/// its output never arrived (it panicked, or its runtime dropped it). Dropping
/// a `Task` detaches the task, as dropping tokio's `JoinHandle` does; only
/// [`Task::abort`] stops it.
pub struct Task<T> {
    abort: AbortHandle,
    output: oneshot::Receiver<T>,
    finished: Arc<AtomicBool>,
}

impl<T> Task<T> {
    /// Stop the task at its next suspension point. A task that has already
    /// ended is unaffected.
    pub fn abort(&self) {
        self.abort.abort();
    }

    /// Whether the task has ended: it completed, was aborted and dropped by its
    /// runtime, or panicked. A task just aborted is not finished until its
    /// runtime has dropped it.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

impl<T> Future for Task<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        match Pin::new(&mut self.output).poll(cx) {
            Poll::Ready(output) => {
                self.finished.store(true, Ordering::Release);
                Poll::Ready(output.ok())
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T> fmt::Debug for Task<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Task")
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

/// Sets the task's flag when dropped, so a panic, an abort and a runtime that
/// drops the task unpolled all end it.
struct Finish(Arc<AtomicBool>);

impl Drop for Finish {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// Spawn `future` on `runtime`, which needs only its detached spawn.
pub fn spawn<T, F>(runtime: &dyn Runtime, future: F) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (output_tx, output_rx) = oneshot::channel();
    let (abort, registration) = AbortHandle::new_pair();
    let finished = Arc::new(AtomicBool::new(false));
    let finish = Finish(Arc::clone(&finished));
    let run = Abortable::new(
        async move {
            let value = future.await;
            // Flag the end before the output is visible, so a reader woken by
            // the output sees the task finished.
            drop(finish);
            let _ = output_tx.send(value);
        },
        registration,
    );
    runtime.spawn(Box::pin(async move {
        let _ = run.await;
    }));
    Task {
        abort,
        output: output_rx,
        finished,
    }
}
```

In `polyoxide-rt/src/lib.rs`, replace

```rust
mod runtime;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;
```

with

```rust
mod runtime;
mod task;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;
```

and replace

```rust
pub use runtime::Runtime;
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
```

with

```rust
pub use runtime::Runtime;
pub use task::{spawn, Task};
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
```

Run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `running 13 tests`, each `ok` (the panicking task prints its `the task fails on purpose` panic message to stderr, which is expected), `test result: ok. 13 passed; 0 failed`.

Run: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-rt --all-features'`

Expected: exits 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-rt/src/task.rs
git commit -m "feat(rt): add spawn and Task (story 1)" -- polyoxide-rt Cargo.toml Cargo.lock
```

### Task 1.4: `timeout`

**Files:**
- Create: `polyoxide-rt/src/time.rs`
- Modify: `polyoxide-rt/src/lib.rs`
- Test: `polyoxide-rt/tests/tokio_runtime.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-rt/tests/tokio_runtime.rs`, replace

```rust
use polyoxide_rt::{
    default_runtime, spawn, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime, UNIX_EPOCH,
};
```

with

```rust
use polyoxide_rt::{
    default_runtime, spawn, timeout, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime,
    UNIX_EPOCH,
};
```

and append at the end of the file:

```rust

#[tokio::test(start_paused = true)]
async fn timeout_returns_the_output_of_a_future_that_finishes_in_time() {
    let start = TokioRuntime.now();
    let output = timeout(&TokioRuntime, Duration::from_secs(10), async {
        TokioRuntime.sleep(Duration::from_secs(2)).await;
        7
    })
    .await;
    assert_eq!(output, Some(7));
    assert_near(TokioRuntime.now() - start, Duration::from_secs(2));
}

#[tokio::test(start_paused = true)]
async fn timeout_gives_up_at_the_deadline() {
    let start = TokioRuntime.now();
    let output = timeout(
        &TokioRuntime,
        Duration::from_secs(10),
        TokioRuntime.sleep(Duration::from_secs(60)),
    )
    .await;
    assert_eq!(output, None);
    assert_near(TokioRuntime.now() - start, Duration::from_secs(10));
}

#[tokio::test(start_paused = true)]
async fn timeout_at_zero_lets_a_ready_future_win() {
    assert_eq!(
        timeout(&TokioRuntime, Duration::ZERO, async { 42 }).await,
        Some(42)
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_at_zero_expires_a_pending_future() {
    assert_eq!(
        timeout(&TokioRuntime, Duration::ZERO, std::future::pending::<()>()).await,
        None
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_accepts_the_maximum_duration() {
    assert_eq!(
        timeout(&TokioRuntime, Duration::MAX, async { 1 }).await,
        Some(1)
    );
    // A timer of the longest length does not fire within a year.
    let year = Duration::from_secs(86_400 * 365);
    let never = timeout(&TokioRuntime, Duration::MAX, std::future::pending::<()>());
    assert!(tokio::time::timeout(year, never).await.is_err());
}

#[tokio::test]
async fn timeout_polls_the_future_before_the_timer() {
    // This runtime's sleeps are always ready, so only the order of the two
    // polls lets the future's output through.
    let rt = Scripted::new();
    assert_eq!(
        timeout(&rt, Duration::from_secs(3), async { 9 }).await,
        Some(9)
    );
}

#[tokio::test]
async fn timeout_sleeps_on_the_runtime_it_is_given() {
    let rt = Scripted::new();
    let output = timeout(&rt, Duration::from_secs(3), std::future::pending::<()>()).await;
    assert_eq!(output, None);
    assert_eq!(*rt.slept.lock().unwrap(), vec![Duration::from_secs(3)]);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `error[E0432]: unresolved import `polyoxide_rt::timeout``.

- [ ] **Step 3: Implement**

Create `polyoxide-rt/src/time.rs`:

```rust
//! Timeouts, on an injected `Runtime`.

use std::future::{poll_fn, Future};
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use crate::Runtime;

/// Await `future` for at most `duration` on `runtime`'s clock: its output, or
/// `None` when `duration` passed first.
///
/// `future` is polled before the timer, so a ready future wins even at
/// `Duration::ZERO`. Any duration, `Duration::MAX` included, is valid.
pub async fn timeout<F: Future>(
    runtime: &dyn Runtime,
    duration: Duration,
    future: F,
) -> Option<F::Output> {
    let mut future = pin!(future);
    let mut deadline = runtime.sleep(duration);
    poll_fn(|cx| {
        if let Poll::Ready(output) = future.as_mut().poll(cx) {
            return Poll::Ready(Some(output));
        }
        deadline.as_mut().poll(cx).map(|()| None)
    })
    .await
}
```

In `polyoxide-rt/src/lib.rs`, replace

```rust
mod task;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;
```

with

```rust
mod task;
mod time;
#[cfg(not(target_arch = "wasm32"))]
mod tokio;
```

and replace

```rust
pub use task::{spawn, Task};
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
```

with

```rust
pub use task::{spawn, Task};
pub use time::timeout;
pub use web_time::{Instant, SystemTime, UNIX_EPOCH};
```

Run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `running 20 tests`, each `ok`, `test result: ok. 20 passed; 0 failed`.

Run: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-rt --all-features'`

Expected: exits 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-rt/src/time.rs
git commit -m "feat(rt): add timeout (story 1)" -- polyoxide-rt
```

### Task 1.5: `interval`, `Interval` and `MissedTick`

**Files:**
- Modify: `polyoxide-rt/src/time.rs`
- Modify: `polyoxide-rt/src/lib.rs`
- Test: `polyoxide-rt/tests/tokio_runtime.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-rt/tests/tokio_runtime.rs`, replace

```rust
use polyoxide_rt::{
    default_runtime, spawn, timeout, BoxFuture, Instant, Runtime, SystemTime, TokioRuntime,
    UNIX_EPOCH,
};
```

with

```rust
use polyoxide_rt::{
    default_runtime, interval, spawn, timeout, BoxFuture, Instant, MissedTick, Runtime, SystemTime,
    TokioRuntime, UNIX_EPOCH,
};
```

and append at the end of the file:

```rust

const PERIOD: Duration = Duration::from_secs(10);

fn shared() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}

#[tokio::test(start_paused = true)]
async fn the_first_tick_is_at_once() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    assert_eq!(ticks.tick().await, start);
    assert!(rt.now() - start < TICK, "the first tick waited");
}

#[tokio::test(start_paused = true)]
async fn ticks_come_one_period_apart() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    assert_eq!(ticks.tick().await, start);
    assert_eq!(ticks.tick().await, start + PERIOD);
    assert_eq!(ticks.tick().await, start + 2 * PERIOD);
    assert_near(rt.now() - start, 2 * PERIOD);
}

#[tokio::test(start_paused = true)]
async fn a_late_interval_bursts_by_default() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    ticks.tick().await;
    rt.sleep(Duration::from_secs(35)).await;
    let late = rt.now();

    // The ticks due at 10, 20 and 30 seconds all fire at once.
    assert_eq!(ticks.tick().await, start + PERIOD);
    assert_eq!(ticks.tick().await, start + 2 * PERIOD);
    assert_eq!(ticks.tick().await, start + 3 * PERIOD);
    assert!(rt.now() - late < TICK, "the missed ticks did not fire at once");

    assert_eq!(ticks.tick().await, start + 4 * PERIOD);
    assert_near(rt.now() - start, 4 * PERIOD);
}

#[tokio::test(start_paused = true)]
async fn delay_starts_the_next_period_after_the_late_tick() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    ticks.set_missed_tick(MissedTick::Delay);
    ticks.tick().await;
    rt.sleep(Duration::from_secs(35)).await;
    let late = rt.now();

    assert_eq!(ticks.tick().await, start + PERIOD);
    assert!(rt.now() - late < TICK, "the late tick waited");

    // The next one is a period after the late tick, not on the old grid.
    let next = ticks.tick().await;
    assert_near(next - late, PERIOD);
    assert_near(rt.now() - late, PERIOD);
}

#[tokio::test(start_paused = true)]
async fn skip_keeps_the_original_grid() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    ticks.set_missed_tick(MissedTick::Skip);
    ticks.tick().await;
    rt.sleep(Duration::from_secs(35)).await;
    let late = rt.now();

    assert_eq!(ticks.tick().await, start + PERIOD);
    assert!(rt.now() - late < TICK, "the late tick waited");

    // The ticks due at 20 and 30 seconds are skipped; the next is at 40.
    assert_eq!(ticks.tick().await, start + 4 * PERIOD);
    assert_near(rt.now() - start, 4 * PERIOD);
}

#[tokio::test(start_paused = true)]
async fn a_tick_a_few_milliseconds_late_is_on_time() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    ticks.set_missed_tick(MissedTick::Delay);
    ticks.tick().await;
    rt.sleep(Duration::from_millis(10_003)).await;

    // Scheduling jitter does not move the grid, even for `Delay`.
    assert_eq!(ticks.tick().await, start + PERIOD);
    assert_eq!(ticks.tick().await, start + 2 * PERIOD);
}

#[tokio::test(start_paused = true)]
async fn reset_starts_a_new_period_from_now() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    ticks.tick().await;
    rt.sleep(Duration::from_secs(4)).await;
    ticks.reset();

    ticks.tick().await;
    assert_near(rt.now() - start, Duration::from_secs(14));
}

#[tokio::test(start_paused = true)]
async fn a_cancelled_tick_loses_nothing() {
    let rt = shared();
    let start = rt.now();
    let mut ticks = interval(Arc::clone(&rt), PERIOD);
    ticks.tick().await;

    let cancelled = tokio::time::timeout(Duration::from_secs(4), ticks.tick()).await;
    assert!(cancelled.is_err(), "the tick came before its period");

    assert_eq!(ticks.tick().await, start + PERIOD);
    assert_near(rt.now() - start, PERIOD);
}

#[tokio::test(start_paused = true)]
async fn an_interval_reports_its_period_and_defaults_to_burst() {
    let ticks = interval(shared(), Duration::from_secs(3));
    assert_eq!(ticks.period(), Duration::from_secs(3));
    assert_eq!(MissedTick::default(), MissedTick::Burst);
}

#[test]
#[should_panic(expected = "interval period must be non-zero")]
fn a_zero_period_panics() {
    let _ticks = interval(shared(), Duration::ZERO);
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `error[E0432]: unresolved imports `polyoxide_rt::interval`, `polyoxide_rt::MissedTick``.

- [ ] **Step 3: Implement**

In `polyoxide-rt/src/time.rs`, replace

```rust
use std::future::{poll_fn, Future};
use std::pin::pin;
use std::task::Poll;
use std::time::Duration;

use crate::Runtime;
```

with

```rust
use std::future::{poll_fn, Future};
use std::pin::pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use crate::{Instant, Runtime};

/// A tick this close to its deadline is on time, so scheduling jitter does not
/// shift the grid of `MissedTick::Delay`; tokio's interval makes the same
/// allowance.
const TOLERANCE: Duration = Duration::from_millis(5);

/// What `instant` plus an unrepresentably long wait stands for: thirty years.
const FAR: Duration = Duration::from_secs(86_400 * 365 * 30);
```

and append at the end of the file:

```rust

/// What an [`Interval`] does about ticks it was too late to take on time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissedTick {
    /// Fire the missed ticks one after another at once, then go back to the
    /// original grid.
    #[default]
    Burst,
    /// Start the next period when the late tick fires, so the grid moves.
    Delay,
    /// Drop the missed ticks and fire the next one on the original grid.
    Skip,
}

/// Ticks every `period`, the first at once, on `runtime`'s clock.
///
/// # Panics
///
/// Panics with `interval period must be non-zero` when `period` is zero.
pub fn interval(runtime: Arc<dyn Runtime>, period: Duration) -> Interval {
    assert!(!period.is_zero(), "interval period must be non-zero");
    let next = runtime.now();
    Interval {
        runtime,
        period,
        next,
        behaviour: MissedTick::default(),
    }
}

/// A stream of ticks a period apart; built by [`interval`].
///
/// It is built on the runtime's `sleep_until` and `now`, so a runtime's paused
/// clock drives it.
#[derive(Debug)]
pub struct Interval {
    runtime: Arc<dyn Runtime>,
    period: Duration,
    next: Instant,
    behaviour: MissedTick,
}

impl Interval {
    /// Wait for the next tick and return the deadline it was due at. Dropping
    /// the future before it completes loses nothing: the next call waits for
    /// the same tick.
    pub async fn tick(&mut self) -> Instant {
        let due = self.next;
        self.runtime.sleep_until(due).await;
        self.next = self.after(due, self.runtime.now());
        due
    }

    /// Start a new period now: the next tick is one period from this call.
    pub fn reset(&mut self) {
        self.next = later(self.runtime.now(), self.period);
    }

    /// The time between ticks.
    pub fn period(&self) -> Duration {
        self.period
    }

    /// Choose what to do about ticks that were missed.
    pub fn set_missed_tick(&mut self, behaviour: MissedTick) {
        self.behaviour = behaviour;
    }

    /// The deadline after the tick due at `due`, taken at `now`.
    fn after(&self, due: Instant, now: Instant) -> Instant {
        let next = later(due, self.period);
        if now.saturating_duration_since(due) <= TOLERANCE {
            return next;
        }
        match self.behaviour {
            MissedTick::Burst => next,
            MissedTick::Delay => later(now, self.period),
            MissedTick::Skip => {
                if next > now {
                    return next;
                }
                let behind = now.saturating_duration_since(due).as_nanos();
                let periods = behind / self.period.as_nanos();
                u32::try_from(periods + 1)
                    .ok()
                    .and_then(|whole| self.period.checked_mul(whole))
                    .and_then(|grid| due.checked_add(grid))
                    .unwrap_or_else(|| later(now, self.period))
            }
        }
    }
}

/// `instant` plus `wait`, capped at thirty years where the sum would overflow.
fn later(instant: Instant, wait: Duration) -> Instant {
    instant.checked_add(wait).unwrap_or_else(|| instant + FAR)
}
```

In `polyoxide-rt/src/lib.rs`, replace

```rust
pub use time::timeout;
```

with

```rust
pub use time::{interval, timeout, Interval, MissedTick};
```

Run `nix develop --command cargo fmt --all`.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-rt`

Expected: `running 30 tests`, each `ok`, `test result: ok. 30 passed; 0 failed`.

Run: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-rt --all-features'`

Expected: exits 0.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(rt): add interval, Interval and MissedTick (story 1)" -- polyoxide-rt
```

### Task 1.6: The README, the rules, CLAUDE.md and the allowlist

**Files:**
- Create: `polyoxide-rt/README.md`
- Create: `.claude/rules/rt.md`
- Create: `ci/dep-allowlists/polyoxide-rt.txt`
- Modify: `polyoxide-rt/src/lib.rs`
- Modify: `polyoxide-rt/Cargo.toml`
- Modify: `CLAUDE.md:26,87`
- Modify: `.claude/rules/ci-gates.md:105-106`
- Modify: `scripts/venue_gate.py:3-5`
- Regenerate: `README.md`, `CLAUDE.md` (generated regions)

- [ ] **Step 1: Write the failing checks**

Create `.claude/rules/rt.md`:

```markdown
---
paths:
  - "polyoxide-rt/**"
---

# The runtime seam (`polyoxide-rt`)

- It is a foundation crate: it depends on nothing in the workspace, so it publishes first, and it
  names no venue (`scripts/venue_gate.py` reads it with `polyoxide-venue`, `-core`, `-ws` and
  `-test-support`). It has no public error type, so the `Classify` sweep has nothing to cover.
- `Runtime` is the whole seam: a detached `spawn`, `sleep`, `now`, and `sleep_until`, whose default
  is `sleep(deadline - now)`. An implementor writes only those; `Task`, `timeout` and `Interval`
  are built on them and add nothing to the trait. A client takes its runtime from its builder and
  falls back to `default_runtime()`; there is no process-wide install.
- `Task<T>` resolves to `Option<T>`: `None` when it was aborted or its output never arrived (a
  panic). Dropping a `Task` detaches it, as dropping tokio's `JoinHandle` does; only `abort()`
  stops it.
- `timeout` polls the future before the timer, so a ready future wins even at `Duration::ZERO`.
  `Runtime::sleep` takes any duration, `Duration::MAX` included.
- `Interval` ticks at once, then every period, on `sleep_until` and `now`, so a runtime's paused
  clock drives it. A zero period panics with `interval period must be non-zero`. A tick within
  5 ms of its deadline is on time, so jitter never moves `MissedTick::Delay`'s grid. `tick` is
  cancel safe: a dropped tick leaves the same tick due.
- The clock vocabulary is `web-time`'s, re-exported as `Instant`, `SystemTime` and `UNIX_EPOCH`:
  std's own types on native.
- `TokioRuntime` (off wasm32) is a unit struct over tokio's spawn, sleep and clock, so tokio's
  paused clock drives it; its `spawn` panics outside a tokio runtime, as `tokio::spawn` does.
  `default_runtime()` is `TokioRuntime` off wasm32. tokio is declared only under
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`.
- The tests are `tests/tokio_runtime.rs`, on `#[tokio::test(start_paused = true)]` with a
  millisecond-rounding tolerance, and a `Scripted` runtime that records what the trait's defaults
  and the helpers ask of it.
- The crate README is prose with no code fence, since `lib.rs` compiles it as a doctest.
```

Create the empty file `ci/dep-allowlists/polyoxide-rt.txt` (`touch ci/dep-allowlists/polyoxide-rt.txt`).

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_dep_allowlists.py -q -k "claude_md_lists or polyoxide-rt"'`

Expected: two failures. `test_claude_md_lists_every_rules_file_once` prints `rules files CLAUDE.md's table does not list: ['rt.md']`, and `test_each_allowlist_is_the_tree_cargo_builds[polyoxide-rt]` prints `the tree and its allowlist differ:` followed by `polyoxide-rt.txt builds futures-channel, which its allowlist does not list` and one line for each other crate in the tree.

- [ ] **Step 3: Implement**

In `CLAUDE.md`, replace

```
| `sockets.md` | the `polyoxide-ws` kit, its `Supervisor`, TLS, the scripted test server |
```

with

```
| `sockets.md` | the `polyoxide-ws` kit, its `Supervisor`, TLS, the scripted test server |
| `rt.md` | `polyoxide-rt`: the `Runtime` trait, `Task`, `timeout`, `Interval`, `TokioRuntime` |
```

and replace, in the "The foundation crates name no venue" rule,

```
under `polyoxide-venue`, `-core`, `-ws` and `-test-support` for each venue and product id
```

with

```
under `polyoxide-venue`, `-core`, `-ws`, `-rt` and `-test-support` for each venue and product id
```

In `.claude/rules/ci-gates.md`, replace

```
`scripts/venue_gate.py` reads every file under `polyoxide-venue`, `-core`, `-ws` and
`-test-support`, case-insensitively, for:
```

with

```
`scripts/venue_gate.py` reads every file under `polyoxide-venue`, `-core`, `-ws`, `-rt` and
`-test-support`, case-insensitively, for:
```

In `scripts/venue_gate.py`, replace, in the module docstring,

```
`polyoxide-venue`, `polyoxide-core`, `polyoxide-ws` and `polyoxide-test-support` are
what every venue builds on, so none of them may carry a venue's code, rules or
defaults. This script reads every file under them, case-insensitively, for
```

with

```
`polyoxide-venue`, `polyoxide-core`, `polyoxide-ws`, `polyoxide-rt` and
`polyoxide-test-support` are what every venue builds on, so none of them may carry a
venue's code, rules or defaults. This script reads every file under them,
case-insensitively, for
```

In `polyoxide-rt/Cargo.toml`, replace

```toml
notes = ["names no venue, which `scripts/venue_gate.py` holds"]
```

with

```toml
notes = ["names no venue, which `scripts/venue_gate.py` holds, and builds only futures-channel, futures-util and web-time, with tokio (`rt` and `time`) off wasm32"]
```

Create `polyoxide-rt/README.md` (prose only, no code fence):

```markdown
# polyoxide-rt

The runtime seam of the polyoxide crates. Code that sleeps, reads the clock or spawns a task takes a `Runtime` instead of calling a runtime's own functions, so a runtime can be injected rather than assumed.

A `Runtime` is a detached `spawn`, a `sleep`, a monotonic `now` and a `sleep_until` with a default. `spawn` turns any future into a `Task`, which resolves to its output, or to nothing when it was aborted or panicked, and which keeps running when it is dropped. `timeout` awaits a future for a bounded time and polls it before the timer, so a ready future wins at a zero duration. `interval` ticks at once and then every period, with a choice of what to do about ticks it was too late for: fire them together, delay the next one, or skip them. `TokioRuntime` is the native runtime, built on tokio's own clock so a paused clock drives it, and `default_runtime` returns it.

`Instant`, `SystemTime` and `UNIX_EPOCH` are re-exported from `web-time`: the standard library's own types on native.

The crate depends on nothing in the workspace and names no venue, so it publishes first and every other crate can use it.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-rt/).

## Installation

Add `polyoxide-rt` to the dependencies of your crate's manifest, at the version of the other polyoxide crates you use.
```

Replace the crate doc and its header in `polyoxide-rt/src/lib.rs`: replace

```rust
//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates: the `Runtime` trait, which a
//! crate takes instead of calling a runtime's own functions, and `TokioRuntime`,
//! its native implementation.

#![warn(missing_docs)]
```

with

```rust
//! # polyoxide-rt
//!
//! The runtime seam of the polyoxide crates. A crate that sleeps, reads the
//! clock or spawns takes a [`Runtime`] instead of calling a runtime's own
//! functions, so a runtime can be injected rather than assumed.
//!
//! - [`Runtime`] is a detached `spawn`, a `sleep`, a monotonic `now` and a
//!   defaulted `sleep_until`. An implementor writes only those.
//! - [`spawn`] turns any future into a [`Task`], which resolves to `Some`
//!   output, or to `None` when it was aborted or panicked, and keeps running
//!   when it is dropped.
//! - [`timeout`] bounds a future by a duration on a runtime's clock, polling
//!   the future first, so a ready future wins at `Duration::ZERO`.
//! - [`interval`] ticks at once and then every period, and an [`Interval`]
//!   takes a [`MissedTick`] to say what to do about ticks it was late for.
//! - `TokioRuntime` is the native runtime, on tokio's own clock so a paused
//!   clock drives it, and [`default_runtime`] returns it off wasm32.
//! - [`Instant`], [`SystemTime`] and [`UNIX_EPOCH`] are `web-time`'s: std's own
//!   types on native.
//!
//! ```
//! use std::time::Duration;
//!
//! use polyoxide_rt::{default_runtime, spawn, timeout};
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     let runtime = default_runtime();
//!
//!     let task = spawn(&*runtime, async { 6 * 7 });
//!     assert_eq!(task.await, Some(42));
//!
//!     let slow = runtime.sleep(Duration::from_secs(60));
//!     let waited = timeout(&*runtime, Duration::from_millis(10), slow).await;
//!     assert_eq!(waited, None);
//! }
//! ```
//!
//! The crate depends on nothing in the workspace and names no venue, so it
//! publishes first and every other crate can use it.

#![warn(missing_docs)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
```

Then:

1. `nix develop --command cargo fmt --all`.
2. `nix develop --command python3 scripts/gen_registry.py --write` (regenerates the crate's `README.md` row and CLAUDE.md's graph line from the new `notes`).
3. `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`, then `git status --short ci/dep-allowlists`: it lists only `polyoxide-rt.txt`, since no existing crate's tree changed. Its content is the crates `cargo tree -p polyoxide-rt -e normal --target x86_64-unknown-linux-gnu` builds; none may be on the socket-only deny-list, and none may be a venue crate.

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/ -q'`

Expected: all tests pass.

Run: `nix develop --command bash -c 'cargo fmt --all -- --check && cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings && cargo test -p polyoxide-rt --all-features && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-rt --all-features && python3 scripts/gen_registry.py --check && python3 scripts/venue_gate.py && python3 scripts/live_unwraps.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/publish_order.py check-manifests'`

Expected: exits 0; `cargo test` prints `test result: ok. 30 passed; 0 failed` for `tokio_runtime` and `test result: ok. 1 passed; 0 failed` for the doctests (the crate-level example; the README holds no code).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-rt/README.md .claude/rules/rt.md ci/dep-allowlists/polyoxide-rt.txt
git commit -m "feat(rt): document polyoxide-rt, add its rules file and pin its dependency tree (story 1)

The new crate's normal dependency tree is pinned in ci/dep-allowlists/polyoxide-rt.txt:
futures-channel, futures-util, web-time and, off wasm32, tokio with rt and time." -- polyoxide-rt .claude/rules/rt.md .claude/rules/ci-gates.md ci/dep-allowlists/polyoxide-rt.txt CLAUDE.md README.md scripts/venue_gate.py
```

## Story 2: Core and the HTTP side of the venue crates on the injected runtime

- **Developer:** intermediate-dev
- **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/hold.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/tests/runtime_injection.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/api/health.rs`, `polyoxide-polymarket/src/data/client.rs`, `polyoxide-polymarket/src/gamma/client.rs`, `polyoxide-polymarket/src/gamma/api/health.rs`, `polyoxide-polymarket/src/perps/client.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/mod.rs`, `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/shared/signer_limit.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-polymarket/src/shared/mod.rs`, `polyoxide-polymarket/src/shared/throttle.rs`, `.claude/rules/clients.md`, `.claude/rules/rate-limits.md`, `.claude/rules/send-loop.md`, `.claude/rules/binance.md`, `CLAUDE.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `Cargo.lock`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 2 → Stories 4, 5, 7"; uses Story 1's `Runtime`, `default_runtime`, `TokioRuntime`, `BoxFuture` and `Instant`
- **Verify:** `cargo test -p polyoxide-core -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-kalshi --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
- **Review:** deep (the send loop, holds and rate limiters; landmine notes on quotas and waits apply)

Gives every HTTP client an injected `Runtime` and moves each clock read and sleep on the HTTP side onto it, with behaviour unchanged: `ClientConfig::runtime`, the `runtime` setter in `client_config_setters!`, `HttpClientBuilder::runtime` and `HttpClient::runtime()` (the contract's names), then the send loop's retry sleep, `health`'s stopwatch, holds, capacity buckets, the window quotas' bucket waits, Polymarket's tables and signer buckets, Binance's `WeightBudget` and Kalshi's `PredictionsThrottle`. The contract fixes only the config, setter, builder and client names; the rest is this story's own and no later story names it. A `Hold` carries the `Runtime` its throttle keeps time on (`Hold::unbounded_on`, `Hold::with_ceiling_on`, `Hold::runtime`), because every layer of one throttle already shares one hold: a `CapacityBucket` and a `RateLimiter` read their clock and sleep on their hold's runtime, so one throttle never mixes two clocks, and every existing constructor keeps its signature and takes `default_runtime()`. A venue's `build` resolves its runtime once (`ClientConfig::runtime_or_default`), hands it to `HttpClientBuilder::runtime` and builds its throttle on it with a new `_on` twin (`clob_limits_on`, `gamma_limits_on`, `data_limits_on`, `relay_limits_on`, `perps_limits_on`, `clob_throttle_on`, `SignerLimiter::new_on`, `WeightBudget::new_on`; Kalshi's `PredictionsThrottle::new_on` is crate-private). governor keeps its quanta clock, but its `until_ready` sleeps on `futures-timer`, so a window quota's bucket wait becomes governor's `check` plus a sleep on the runtime (`WindowQuotaTable::with_runtime` names it). No `tokio::spawn` exists on the HTTP side, so nothing is spawned through the runtime here.

This story edits the throttles' quotas and waits, so the landmine notes on quotas and waits apply, and the supervisor should dispatch its developer with `effort: xhigh`. No quota gains depth and `quota()`, `sustained_slots` and the reserve are untouched; a `Retry-After` still only extends a wait and `Hold::wait` still re-reads its deadline after waking; the one retry loop stays `HttpClient::send`, and the window quotas' wait loop only replaces governor's own one-for-one inside `Throttle::acquire`; no gzip knob and no `connect_async` are touched. Task 2.5's mutants hold `allow_burst(` and the reserve constant as throwaway `sed` text, restored by `git checkout` in the same command.

Every existing test body is unchanged, which is why no `[[drift]]` entry and no row `R22` appear here: a test module that read `Instant` from `use super::*` gets a one-line `use tokio::time::Instant;` instead, and `test_body_diff.py` drops `use` statements. Only the unit tests of `hold.rs`, `capacity.rs` and Binance's `weight.rs` need it. The rows of `docs/MUTANTS.md` and the citations of `.github/scripts/tests/test_mutants_ledger.py` that point into `client.rs`, `hold.rs`, `rate_limit.rs` and `signer_limit.rs` move with this story's edits; that script is not a story gate, so Tasks 2.1 to 2.4 leave it red and Task 2.5 moves the citations and proves the mutants again. Because core, polymarket, binance and kalshi gain a dependency on `polyoxide-rt`, Task 2.1 regenerates `ci/dep-allowlists/` and CLAUDE.md's generated crate graph.

### Task 2.1: The runtime on the config, the builders and the client

**Files:**
- Create: `polyoxide-core/tests/runtime_injection.rs`
- Modify: `polyoxide-core/Cargo.toml:18`
- Modify: `polyoxide-core/src/config.rs:1-58`
- Modify: `polyoxide-core/src/macros.rs:3-65`
- Modify: `polyoxide-core/src/client.rs:1-293`
- Modify: `polyoxide-polymarket/Cargo.toml:22-156`
- Modify: `polyoxide-binance/Cargo.toml:18-37`
- Modify: `polyoxide-kalshi/Cargo.toml:19-42`
- Modify: `.claude/rules/clients.md:18-23`
- Test: `polyoxide-core/tests/runtime_injection.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-core/tests/runtime_injection.rs`. Core names no venue, so its tests use made-up hosts.

```rust
//! The runtime a client is built on, and the plumbing that carries it: from a
//! config or a builder to the client and its `with_base_url` siblings. Tests
//! further down prove that each wait and clock read of core goes through it.

use std::sync::Arc;

use polyoxide_core::{ClientConfig, HttpClientBuilder};
use polyoxide_rt::{Runtime, TokioRuntime};

fn tokio_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}

#[test]
fn a_config_runtime_is_the_clients() {
    let runtime = tokio_runtime();
    let mut config = ClientConfig::new("https://example.test", 4);
    assert!(config.runtime.is_none(), "a new config names no runtime");
    config.runtime = Some(Arc::clone(&runtime));

    let http = config.http_builder().build().unwrap();
    assert!(Arc::ptr_eq(http.runtime(), &runtime));
}

#[test]
fn a_builders_runtime_is_shared_with_its_siblings() {
    let runtime = tokio_runtime();
    let http = HttpClientBuilder::new("https://example.test")
        .runtime(Arc::clone(&runtime))
        .build()
        .unwrap();
    let sibling = http.with_base_url("https://other.example.test").unwrap();

    assert!(Arc::ptr_eq(http.runtime(), &runtime));
    assert!(Arc::ptr_eq(sibling.runtime(), &runtime));
}

#[test]
fn an_unset_runtime_is_the_targets_default() {
    // Native targets default to tokio's runtime.
    let http = HttpClientBuilder::new("https://example.test")
        .build()
        .unwrap();
    assert_eq!(format!("{:?}", http.runtime()), "TokioRuntime");

    let config = ClientConfig::new("https://example.test", 4);
    assert_eq!(format!("{:?}", config.runtime_or_default()), "TokioRuntime");
}

struct Knobs {
    config: ClientConfig,
}

// Only `runtime` is read here; the other setters are the macro's.
#[allow(dead_code)]
impl Knobs {
    polyoxide_core::client_config_setters!(config);
}

#[test]
fn the_runtime_setter_writes_the_config() {
    let runtime = tokio_runtime();
    let knobs = Knobs {
        config: ClientConfig::new("https://example.test", 4),
    }
    .runtime(Arc::clone(&runtime));

    let set = knobs
        .config
        .runtime
        .as_ref()
        .expect("the setter wrote the config");
    assert!(Arc::ptr_eq(set, &runtime));
    assert!(Arc::ptr_eq(&knobs.config.runtime_or_default(), &runtime));
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-core --test runtime_injection`

Expected: it does not compile. The first error is ``error[E0432]: unresolved import `polyoxide_rt` `` (core does not depend on the crate yet), and the `runtime` method, `runtime_or_default` and `ClientConfig::runtime` follow as unresolved.

- [ ] **Step 3: Implement**

**3a. Core depends on `polyoxide-rt`.** Current (line 18 of `polyoxide-core/Cargo.toml`):

```toml
polyoxide-venue = { workspace = true }
```

Replace with:

```toml
polyoxide-venue = { workspace = true }
# The runtime every wait, clock read and spawn goes through. Core does not
# re-export it: a crate that names `Runtime` declares it itself.
polyoxide-rt = { workspace = true }
```

**3b. `polyoxide-core/src/config.rs`.** Current (lines 1-4):

```rust
//! The transport settings every client builder holds.

use crate::client::{HttpClientBuilder, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS};
use crate::rate_limit::RetryConfig;
```

Replace with:

```rust
//! The transport settings every client builder holds.

use std::sync::Arc;

use polyoxide_rt::Runtime;

use crate::client::{HttpClientBuilder, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS};
use crate::rate_limit::RetryConfig;
```

Current (lines 6-14):

```rust
/// The transport settings a client builder holds: the base URL, the timeout,
/// the connection pool, the retry schedule and the concurrency budget.
///
/// A venue's builder holds one, generates its five setters with
/// [`client_config_setters!`](crate::client_config_setters), and builds its
/// [`HttpClient`](crate::HttpClient) from [`http_builder`](Self::http_builder),
/// adding the throttle and the retry policy it installs itself. The config
/// holds no throttle, policy or `gzip` setting, since those belong to the
/// venue.
```

Replace with:

```rust
/// The transport settings a client builder holds: the base URL, the timeout,
/// the connection pool, the retry schedule, the concurrency budget and the
/// runtime.
///
/// A venue's builder holds one, generates its six setters with
/// [`client_config_setters!`](crate::client_config_setters), and builds its
/// [`HttpClient`](crate::HttpClient) from [`http_builder`](Self::http_builder),
/// adding the throttle and the retry policy it installs itself, the throttle
/// on the runtime [`runtime_or_default`](Self::runtime_or_default) names. The
/// config holds no throttle, policy or `gzip` setting, since those belong to
/// the venue.
```

Current (lines 25-27):

```rust
    /// In-flight requests allowed; `None` keeps the client's default.
    pub max_concurrent: Option<usize>,
    default_max_concurrent: usize,
```

Replace with:

```rust
    /// In-flight requests allowed; `None` keeps the client's default.
    pub max_concurrent: Option<usize>,
    /// The runtime every wait, clock read and spawn of the client goes
    /// through; `None` takes [`polyoxide_rt::default_runtime`].
    pub runtime: Option<Arc<dyn Runtime>>,
    default_max_concurrent: usize,
```

Current (lines 39-41, in `new`):

```rust
            retry_config: None,
            max_concurrent: None,
            default_max_concurrent,
```

Replace with:

```rust
            retry_config: None,
            max_concurrent: None,
            runtime: None,
            default_max_concurrent,
```

Current (lines 45-57, `http_builder`):

```rust
    /// An [`HttpClientBuilder`] for the base URL, with the timeout, the pool
    /// size, the concurrency (the set value, else the client's default) and
    /// the retry config if one is set. It sets no throttle, policy or `gzip`.
    pub fn http_builder(&self) -> HttpClientBuilder {
        let builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(self.default_max_concurrent));
        match &self.retry_config {
            Some(config) => builder.with_retry_config(config.clone()),
            None => builder,
        }
    }
```

Replace with:

```rust
    /// An [`HttpClientBuilder`] for the base URL, with the timeout, the pool
    /// size, the concurrency (the set value, else the client's default), the
    /// runtime and the retry config if they are set. It sets no throttle,
    /// policy or `gzip`.
    pub fn http_builder(&self) -> HttpClientBuilder {
        let builder = HttpClientBuilder::new(&self.base_url)
            .timeout_ms(self.timeout_ms)
            .pool_size(self.pool_size)
            .with_max_concurrent(self.max_concurrent.unwrap_or(self.default_max_concurrent));
        let builder = match &self.runtime {
            Some(runtime) => builder.runtime(Arc::clone(runtime)),
            None => builder,
        };
        match &self.retry_config {
            Some(config) => builder.with_retry_config(config.clone()),
            None => builder,
        }
    }

    /// The runtime this config names, else the target's default
    /// ([`polyoxide_rt::default_runtime`]).
    ///
    /// A venue's `build` calls it once and gives the result both to
    /// [`HttpClientBuilder::runtime`](crate::HttpClientBuilder::runtime) and
    /// to the throttle it installs, so the client and its throttle keep time
    /// on one runtime.
    pub fn runtime_or_default(&self) -> Arc<dyn Runtime> {
        self.runtime
            .clone()
            .unwrap_or_else(polyoxide_rt::default_runtime)
    }
```

**3c. `polyoxide-core/src/macros.rs`.** Current (lines 3-5):

```rust
/// Generates a client builder's five transport setters over a
/// `ClientConfig` field: `base_url`, `timeout_ms`, `pool_size`,
/// `with_retry_config` and `max_concurrent`.
```

Replace with:

```rust
/// Generates a client builder's six transport setters over a
/// `ClientConfig` field: `base_url`, `timeout_ms`, `pool_size`,
/// `with_retry_config`, `max_concurrent` and `runtime`.
///
/// `runtime` names `polyoxide_rt::Runtime`, which core does not re-export, so
/// the invoking crate declares `polyoxide-rt` itself.
```

Current (lines 60-65, the end of the macro's setters):

```rust
        pub fn max_concurrent(mut self, max: usize) -> Self {
            self.$config.max_concurrent = Some(max);
            self
        }
    };
}
```

Replace with:

```rust
        pub fn max_concurrent(mut self, max: usize) -> Self {
            self.$config.max_concurrent = Some(max);
            self
        }

        /// Set the runtime the client's waits, clock reads and spawns go
        /// through. Unset, the target's default is used
        /// (`polyoxide_rt::default_runtime`).
        pub fn runtime(mut self, runtime: ::std::sync::Arc<dyn ::polyoxide_rt::Runtime>) -> Self {
            self.$config.runtime = Some(runtime);
            self
        }
    };
}
```

**3d. `polyoxide-core/src/client.rs`.** Current (lines 4-6):

```rust
use reqwest::Method;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use url::Url;
```

Replace with:

```rust
use polyoxide_rt::Runtime;
use reqwest::Method;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use url::Url;
```

Current (lines 43-46):

```rust
    pub(crate) throttle: Arc<DynThrottle<'static>>,
    pub(crate) policy: Arc<DynRetryPolicy<'static>>,
    pub(crate) retry_config: RetryConfig,
    concurrency_limiter: Option<Arc<Semaphore>>,
```

Replace with:

```rust
    pub(crate) throttle: Arc<DynThrottle<'static>>,
    pub(crate) policy: Arc<DynRetryPolicy<'static>>,
    pub(crate) retry_config: RetryConfig,
    pub(crate) runtime: Arc<dyn Runtime>,
    concurrency_limiter: Option<Arc<Semaphore>>,
```

Current (lines 56-57, in `Debug`):

```rust
            .field("retry_config", &self.retry_config)
            .field("concurrency_limiter", &self.concurrency_limiter)
```

Replace with:

```rust
            .field("retry_config", &self.retry_config)
            .field("runtime", &self.runtime)
            .field("concurrency_limiter", &self.concurrency_limiter)
```

Current (line 66, in `with_base_url`'s doc):

```rust
    /// (and so its hold), retry policy, retry config, and concurrency limiter
```

Replace with:

```rust
    /// (and so its hold), retry policy, retry config, runtime, and concurrency limiter
```

Current (line 97):

```rust
    /// Acquire a concurrency permit, if a limiter is configured.
```

Replace with:

```rust
    /// The runtime this client's waits, clock reads and spawns go through:
    /// the one its builder was given, else the target's default. Shared with
    /// every [`with_base_url`](Self::with_base_url) sibling.
    pub fn runtime(&self) -> &Arc<dyn Runtime> {
        &self.runtime
    }

    /// Acquire a concurrency permit, if a limiter is configured.
```

Current (lines 171-174, in `HttpClientBuilder`):

```rust
    retry_config: RetryConfig,
    max_concurrent: Option<usize>,
    gzip: Option<bool>,
}
```

Replace with:

```rust
    retry_config: RetryConfig,
    max_concurrent: Option<usize>,
    runtime: Option<Arc<dyn Runtime>>,
    gzip: Option<bool>,
}
```

Current (lines 186-188, in `HttpClientBuilder::new`):

```rust
            max_concurrent: None,
            gzip: None,
        }
```

Replace with:

```rust
            max_concurrent: None,
            runtime: None,
            gzip: None,
        }
```

Current (lines 242-245):

```rust
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }
```

Replace with:

```rust
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = Some(max);
        self
    }

    /// Set the runtime this client's waits, clock reads and spawns go
    /// through, shared with every [`with_base_url`](HttpClient::with_base_url)
    /// sibling.
    ///
    /// Default: [`polyoxide_rt::default_runtime`]. A throttle installed with
    /// [`with_throttle`](Self::with_throttle) keeps time on the runtime it was
    /// built on: build it on this one, as the venue crates do.
    pub fn runtime(mut self, runtime: Arc<dyn Runtime>) -> Self {
        self.runtime = Some(runtime);
        self
    }
```

Current (lines 283-284, in `build`):

```rust
            retry_config: self.retry_config,
            concurrency_limiter: self.max_concurrent.map(|n| Arc::new(Semaphore::new(n))),
```

Replace with:

```rust
            retry_config: self.retry_config,
            runtime: self.runtime.unwrap_or_else(polyoxide_rt::default_runtime),
            concurrency_limiter: self.max_concurrent.map(|n| Arc::new(Semaphore::new(n))),
```

**3e. The three venue crates declare `polyoxide-rt`**, since `client_config_setters!` now names its `Runtime` in every builder that invokes it. In `polyoxide-polymarket/Cargo.toml`, each of the five HTTP features gains `"dep:polyoxide-rt"`. Current (lines 22-23):

```toml
clob = [
    "dep:polyoxide-core",
```

Replace with:

```toml
clob = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (lines 51-52):

```toml
gamma = [
    "dep:polyoxide-core",
```

Replace with:

```toml
gamma = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (lines 64-65):

```toml
data = [
    "dep:polyoxide-core",
```

Replace with:

```toml
data = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (lines 76-77):

```toml
relay = [
    "dep:polyoxide-core",
```

Replace with:

```toml
relay = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (lines 94-95):

```toml
perps = [
    "dep:polyoxide-core",
```

Replace with:

```toml
perps = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (line 156):

```toml
polyoxide-core = { workspace = true, optional = true }
```

Replace with:

```toml
polyoxide-core = { workspace = true, optional = true }
# The runtime the HTTP modules' clients and throttles keep time on. The socket
# modules (`rtds`, `sports`) build none of it until the socket kit moves on to
# it.
polyoxide-rt = { workspace = true, optional = true }
```

In `polyoxide-binance/Cargo.toml`, current (lines 18-19):

```toml
usdm = [
    "dep:polyoxide-core",
```

Replace with:

```toml
usdm = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (line 37):

```toml
polyoxide-core = { workspace = true, optional = true }
```

Replace with:

```toml
polyoxide-core = { workspace = true, optional = true }
# The runtime the client and its weight budget keep time on.
polyoxide-rt = { workspace = true, optional = true }
```

In `polyoxide-kalshi/Cargo.toml`, current (lines 19-20):

```toml
predictions = [
    "dep:polyoxide-core",
```

Replace with:

```toml
predictions = [
    "dep:polyoxide-core",
    "dep:polyoxide-rt",
```

Current (line 42):

```toml
polyoxide-core = { workspace = true, optional = true }
```

Replace with:

```toml
polyoxide-core = { workspace = true, optional = true }
# The runtime the client and its Read throttle keep time on.
polyoxide-rt = { workspace = true, optional = true }
```

**3f. The rule.** In `.claude/rules/clients.md`, current (lines 18-23):

```markdown
- Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry
  config, concurrency, and the client's default concurrency: relay 2; gamma, data, perps, Binance
  and Kalshi 4; clob 8) and gets its five transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config` and `max_concurrent`, from
  `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()`
  and adds the throttle and retry policy the venue installs; the config sets no `gzip`.
```

Replace with:

```markdown
- Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry
  config, concurrency, runtime, and the client's default concurrency: relay 2; gamma, data, perps,
  Binance and Kalshi 4; clob 8) and gets its six transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config`, `max_concurrent` and `runtime`, from
  `polyoxide_core::client_config_setters!`; `runtime` names `polyoxide_rt::Runtime`, which core
  does not re-export, so the venue crate declares `polyoxide-rt` itself. Its `build` starts from
  `ClientConfig::http_builder()` and adds the throttle and retry policy the venue installs; the
  config sets no `gzip`.
```

**3g. Regenerate what the new dependency changes.** The build rewrites `Cargo.lock`; the allowlists and the crate graph are written by their scripts:

```bash
nix develop --command cargo fmt --all
nix develop --command cargo check -p polyoxide-core -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-kalshi --all-features --all-targets
nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
python3 scripts/gen_registry.py --write
python3 scripts/gen_registry.py --check
git diff --stat -- Cargo.lock ci/dep-allowlists CLAUDE.md docs/ARCHITECTURE.md README.md
```

Expected: `gen_registry.py --check` exits 0. Each changed allowlist differs only by added lines (`polyoxide-rt` and the crates its manifest brings that the file lacked). `CLAUDE.md` differs only inside the `claude-graph` region, in the `needs:` of `polyoxide-core`, `polyoxide-polymarket`, `polyoxide-binance` and `polyoxide-kalshi`, which now list `polyoxide-rt`. If `docs/ARCHITECTURE.md` or `README.md` also change, they are generated: add them to the commit below.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-core --test runtime_injection`

Expected: `test result: ok. 4 passed; 0 failed`.

Then run `nix develop --command cargo clippy -p polyoxide-core -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-kalshi --all-targets --all-features -- -D warnings` (no output beyond `Finished`) and `nix develop --command cargo test -p polyoxide-core --lib` (the config, client and macro tests still pass).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-core/tests/runtime_injection.rs
git commit -m "feat(core): the runtime a client is built on, from ClientConfig to HttpClient (wasm story 2.1)" -m "ClientConfig gains runtime and runtime_or_default, client_config_setters! a runtime setter, HttpClientBuilder::runtime and HttpClient::runtime. core, polymarket, binance and kalshi now depend on polyoxide-rt, so ci/dep-allowlists and CLAUDE.md's crate graph are regenerated." -- polyoxide-core/Cargo.toml polyoxide-core/src/config.rs polyoxide-core/src/macros.rs polyoxide-core/src/client.rs polyoxide-core/tests/runtime_injection.rs polyoxide-polymarket/Cargo.toml polyoxide-binance/Cargo.toml polyoxide-kalshi/Cargo.toml .claude/rules/clients.md Cargo.lock ci/dep-allowlists CLAUDE.md
```

### Task 2.2: The send loop's retry sleep and `health`'s stopwatch

**Files:**
- Modify: `polyoxide-core/src/send.rs:112`
- Modify: `polyoxide-core/src/health.rs:1-82`
- Modify: `polyoxide-core/tests/runtime_injection.rs:1-12`
- Modify: `.claude/rules/send-loop.md:36-48`
- Modify: `.claude/rules/send-loop.md:76-79`
- Test: `polyoxide-core/tests/runtime_injection.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-core/tests/runtime_injection.rs`, current (lines 6-12):

```rust
use std::sync::Arc;

use polyoxide_core::{ClientConfig, HttpClientBuilder};
use polyoxide_rt::{Runtime, TokioRuntime};

fn tokio_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}
```

Replace with:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use polyoxide_core::{ApiError, ClientConfig, HttpClientBuilder, RetryConfig};
use polyoxide_rt::{BoxFuture, Instant, Runtime, TokioRuntime};

/// A runtime over tokio's that records its sleeps, in order, and counts its
/// clock reads. `sleep_until` is the trait's default, so it counts as a sleep
/// and a read.
#[derive(Debug, Default)]
struct Counting {
    inner: TokioRuntime,
    sleeps: Mutex<Vec<Duration>>,
    reads: AtomicUsize,
}

impl Counting {
    fn sleeps(&self) -> Vec<Duration> {
        self.sleeps.lock().unwrap().clone()
    }

    fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

impl Runtime for Counting {
    fn spawn(&self, task: BoxFuture<()>) {
        self.inner.spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.sleeps.lock().unwrap().push(duration);
        self.inner.sleep(duration)
    }

    fn now(&self) -> Instant {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.now()
    }
}

fn tokio_runtime() -> Arc<dyn Runtime> {
    Arc::new(TokioRuntime)
}
```

Append to the end of the file:

```rust

#[tokio::test]
async fn a_429_retry_sleeps_through_the_injected_runtime() {
    let mut server = mockito::Server::new_async().await;
    // mockito matches in reverse creation order: the success mock goes first,
    // so the 429 mock, made second, answers first and then stands down.
    let success = server
        .mock("GET", "/rows")
        .with_status(200)
        .with_body("ok")
        .create_async()
        .await;
    let throttled = server
        .mock("GET", "/rows")
        .with_status(429)
        .expect_at_most(1)
        .create_async()
        .await;

    let counting = Arc::new(Counting::default());
    let http = HttpClientBuilder::new(server.url())
        .runtime(counting.clone())
        .with_retry_config(RetryConfig {
            max_retries: 1,
            initial_backoff_ms: 40,
            max_backoff_ms: 1_000,
        })
        .build()
        .unwrap();

    let body = http.get_bytes("/rows", &[]).await.unwrap();
    assert_eq!(body, b"ok".to_vec());
    throttled.assert_async().await;
    success.assert_async().await;

    // The retry's floor is attempt 0's backoff: 40 ms, less or plus a quarter.
    let slept = counting.sleeps();
    assert!(
        slept
            .iter()
            .any(|d| (Duration::from_millis(30)..=Duration::from_millis(50)).contains(d)),
        "no sleep of the retry's backoff went through the runtime: {slept:?}"
    );
}

#[tokio::test]
async fn health_times_its_attempt_on_the_injected_runtime() {
    let mut server = mockito::Server::new_async().await;
    let _status = server
        .mock("GET", "/status")
        .with_status(200)
        .create_async()
        .await;

    let counting = Arc::new(Counting::default());
    let http = HttpClientBuilder::new(server.url())
        .runtime(counting.clone())
        .build()
        .unwrap();

    http.health::<ApiError>("/status", &[]).await.unwrap();

    // The stopwatch reads the clock when the attempt is signed and again when
    // the answer is in.
    assert!(
        counting.reads() >= 2,
        "the stopwatch read the runtime's clock {} times",
        counting.reads()
    );
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-core --test runtime_injection`

Expected: it compiles, and `a_429_retry_sleeps_through_the_injected_runtime` fails with ``no sleep of the retry's backoff went through the runtime: []`` and `health_times_its_attempt_on_the_injected_runtime` with ``the stopwatch read the runtime's clock 0 times``; `test result: FAILED. 4 passed; 2 failed`.

- [ ] **Step 3: Implement**

**`polyoxide-core/src/send.rs`.** Current (line 112):

```rust
                    tokio::time::sleep(sleep).await;
```

Replace with:

```rust
                    self.runtime.sleep(sleep).await;
```

Nothing else in `send.rs` moves, so the lines `docs/MUTANTS.md` cites in it (84, 91, 95, 97) stay where they are.

**`polyoxide-core/src/health.rs`.** Current (lines 3-11):

```rust
use std::sync::Mutex;
use std::time::{Duration, Instant};

use reqwest::{Method, Response};

use crate::client::HttpClient;
use crate::error::ApiError;
use crate::hooks::{Authenticator, Cost, DynAuthenticator, RequestParts};
use crate::request::RequestError;
```

Replace with:

```rust
use std::sync::{Arc, Mutex};
use std::time::Duration;

use polyoxide_rt::{Instant, Runtime};
use reqwest::{Method, Response};

use crate::client::HttpClient;
use crate::error::ApiError;
use crate::hooks::{Authenticator, Cost, DynAuthenticator, RequestParts};
use crate::request::RequestError;
```

Current (lines 25-47):

```rust
/// Notes when an attempt is signed, which is just before it is sent.
#[derive(Default)]
struct Stopwatch(Mutex<Option<Instant>>);

impl Stopwatch {
    /// Time since the last attempt was signed.
    fn elapsed(&self) -> Duration {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map_or(Duration::ZERO, |sent| sent.elapsed())
    }
}

impl Authenticator for Stopwatch {
    async fn sign(&self, _parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());
        Ok(())
    }
}
```

Replace with:

```rust
/// Notes when an attempt is signed, which is just before it is sent, on the
/// client's runtime clock.
struct Stopwatch {
    runtime: Arc<dyn Runtime>,
    signed: Mutex<Option<Instant>>,
}

impl Stopwatch {
    fn new(runtime: Arc<dyn Runtime>) -> Self {
        Self {
            runtime,
            signed: Mutex::new(None),
        }
    }

    /// Time since the last attempt was signed.
    fn elapsed(&self) -> Duration {
        self.signed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .map_or(Duration::ZERO, |sent| {
                self.runtime.now().saturating_duration_since(sent)
            })
    }
}

impl Authenticator for Stopwatch {
    async fn sign(&self, _parts: &mut RequestParts, _attempt: u32) -> Result<(), ApiError> {
        *self
            .signed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(self.runtime.now());
        Ok(())
    }
}
```

Current (line 62, in `health`):

```rust
        let stopwatch = Stopwatch::default();
```

Replace with:

```rust
        let stopwatch = Stopwatch::new(Arc::clone(&self.runtime));
```

**The rule.** In `.claude/rules/send-loop.md`, current (lines 36-37):

```markdown
3. A retry releases the permit and sleeps the longer of the policy's wait and the loop's floor,
   `RetryConfig::retry_delay(attempt)`, never past `max_retries`.
```

Replace with:

```markdown
3. A retry releases the permit and sleeps, on the client's runtime (`HttpClient::runtime()`), the
   longer of the policy's wait and the loop's floor, `RetryConfig::retry_delay(attempt)`, never
   past `max_retries`.
```

Current (lines 47-48):

```markdown
- `HttpClient` holds one `Arc<DynThrottle<'static>>` and one `Arc<DynRetryPolicy<'static>>`
  (dynosaur), shared with its `with_base_url` siblings, so a 429 on one host holds them all.
```

Replace with:

```markdown
- `HttpClient` holds one `Arc<DynThrottle<'static>>` and one `Arc<DynRetryPolicy<'static>>`
  (dynosaur), shared with its `with_base_url` siblings, so a 429 on one host holds them all. It
  holds its `Arc<dyn polyoxide_rt::Runtime>` the same way, which the retry sleep and `health`'s
  stopwatch go through.
```

Current (lines 77-79):

```markdown
`GET` on the send loop that returns a `Pong { round_trip, response }`. `round_trip` is the attempt
that answered, timed from its signing by a core-private authenticator, so it leaves out the
permit, throttle and backoff waits; a final non-2xx is `ApiError::Response` as the caller's error.
```

Replace with:

```markdown
`GET` on the send loop that returns a `Pong { round_trip, response }`. `round_trip` is the attempt
that answered, timed on the client's runtime clock from its signing by a core-private
authenticator, so it leaves out the permit, throttle and backoff waits; a final non-2xx is
`ApiError::Response` as the caller's error.
```

- [ ] **Step 4: Run them to see them pass**

Run `nix develop --command cargo fmt --all` (it rewraps what this task wrote and nothing else), then `nix develop --command cargo test -p polyoxide-core --test runtime_injection --test health --test mock_request`.

Expected: every test passes, `runtime_injection` with `6 passed`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(core): the retry sleep and health's stopwatch go through the client's runtime (wasm story 2.2)" -- polyoxide-core/src/send.rs polyoxide-core/src/health.rs polyoxide-core/tests/runtime_injection.rs .claude/rules/send-loop.md
```

### Task 2.3: Holds, capacity buckets and window quotas on the runtime

**Files:**
- Modify: `polyoxide-core/src/hold.rs:1-134`
- Modify: `polyoxide-core/src/capacity.rs:1-284`
- Modify: `polyoxide-core/src/rate_limit.rs:6-580`
- Modify: `polyoxide-core/tests/runtime_injection.rs:11-11`
- Modify: `.claude/rules/rate-limits.md:58-76`
- Test: `polyoxide-core/tests/runtime_injection.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-core/tests/runtime_injection.rs`, current (line 11):

```rust
use polyoxide_core::{ApiError, ClientConfig, HttpClientBuilder, RetryConfig};
```

Replace with:

```rust
use polyoxide_core::{
    ApiError, CapacityBucket, ClientConfig, Hold, HttpClientBuilder, LayerId, RetryConfig,
    WindowQuotaTable,
};
```

Append to the end of the file:

```rust

#[tokio::test]
async fn a_hold_reads_the_clock_and_sleeps_on_its_runtime() {
    let counting = Arc::new(Counting::default());
    let hold = Hold::unbounded_on(counting.clone());

    hold.extend(Duration::from_millis(60));
    hold.wait().await;

    let slept = counting.sleeps();
    assert!(
        slept
            .iter()
            .any(|d| *d > Duration::ZERO && *d <= Duration::from_millis(60)),
        "the wait for the hold did not sleep on the runtime: {slept:?}"
    );
    assert!(
        counting.reads() > 0,
        "the hold never read the runtime's clock"
    );
}

#[tokio::test]
async fn a_ceiling_clamps_what_the_runtime_sleeps() {
    let counting = Arc::new(Counting::default());
    let hold = Hold::with_ceiling_on(Duration::from_millis(40), counting.clone());

    hold.extend(Duration::from_secs(3_600));
    hold.wait().await;

    let slept = counting.sleeps();
    assert!(
        !slept.is_empty() && slept.iter().all(|d| *d <= Duration::from_millis(40)),
        "an hour's hold was not clamped to the 40 ms ceiling: {slept:?}"
    );
}

#[tokio::test]
async fn a_bucket_refills_and_waits_on_its_holds_runtime() {
    let counting = Arc::new(Counting::default());
    let hold = Hold::unbounded_on(counting.clone());
    let bucket = CapacityBucket::new(LayerId("test"), 1, 20, hold);

    bucket.acquire(1, true).await.unwrap();
    bucket.acquire(1, true).await.unwrap();

    // One token refills in 50 ms at 20 a second.
    let slept = counting.sleeps();
    assert!(
        slept
            .iter()
            .any(|d| *d > Duration::ZERO && *d <= Duration::from_millis(50)),
        "the wait for a token did not sleep on the runtime: {slept:?}"
    );
    assert!(
        counting.reads() > 0,
        "the bucket never read the runtime's clock"
    );
}

#[tokio::test]
async fn a_window_quota_waits_on_its_runtime() {
    let counting = Arc::new(Counting::default());
    let mut table = WindowQuotaTable::new(11, Duration::from_secs(1));
    table.with_runtime(counting.clone());
    let limiter = table.build();

    limiter.acquire("/rows", None).await;
    limiter.acquire("/rows", None).await;

    // The general bucket holds one token and paces the rest.
    let interval = WindowQuotaTable::paced_interval(11, Duration::from_secs(1));
    let slept = counting.sleeps();
    assert!(
        slept.iter().any(|d| *d > Duration::ZERO && *d <= interval),
        "the second request did not wait for its bucket on the runtime: {slept:?}"
    );
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-core --test runtime_injection`

Expected: it does not compile: ``no function or associated item named `unbounded_on` found for struct `Hold` ``, the same for `with_ceiling_on`, and ``no method named `with_runtime` found for mutable reference `&mut WindowQuotaTable` ``.

- [ ] **Step 3: Implement**

**`polyoxide-core/src/hold.rs`.** Replace everything above the line `#[cfg(test)]` (lines 1-128) with the following. The text is the file's own, with the runtime added to the struct, its constructors, `extend`, `wait`, `is_held` and `Debug`:

```rust
//! [`Hold`]: what the server said, shared by every layer of a throttle.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use polyoxide_rt::{Instant, Runtime};

/// Thirty years: where a deadline that would overflow the clock lands instead.
const FAR_FUTURE: Duration = Duration::from_secs(86_400 * 365 * 30);

/// A deadline before which no request on a throttle may proceed.
///
/// A throttle's buckets model the quota a venue *publishes*; a hold is what
/// the server actually just said. They disagree more often than the tables
/// suggest — Cloudflare's `error code: 1015` is an IP-scoped block with its
/// own window, and it answers 429 no matter how many tokens the buckets still
/// hold. So a 429 holds every request sharing the throttle, not just the one
/// that saw it.
///
/// A hold only ever extends: several concurrent requests typically see the
/// same 429 within a few milliseconds of each other, and taking the most
/// recent value would let whichever response carried the smallest delay
/// release all of them early. [`wait`](Self::wait) re-reads the deadline
/// after waking, so a hold extended mid-wait is honoured in full.
///
/// A hold keeps time on a [`Runtime`](polyoxide_rt::Runtime): it reads its
/// deadline off the runtime's clock and its waits sleep on the runtime.
/// [`unbounded`](Self::unbounded) and [`with_ceiling`](Self::with_ceiling)
/// take the target's default runtime, [`unbounded_on`](Self::unbounded_on) and
/// [`with_ceiling_on`](Self::with_ceiling_on) the one given. Every layer that
/// waits and shares the hold keeps time on the hold's runtime, so one throttle
/// never mixes two clocks.
///
/// A clone shares the deadline. Every layer of one throttle holds a clone of
/// one `Hold`, so replacing or resizing a layer keeps it.
#[derive(Clone)]
pub struct Hold {
    inner: Arc<HoldInner>,
}

struct HoldInner {
    until: Mutex<Option<Instant>>,
    ceiling: Option<Duration>,
    runtime: Arc<dyn Runtime>,
}

impl Hold {
    /// A hold with no ceiling, on the target's default runtime: every
    /// [`extend`](Self::extend) is taken as asked.
    pub fn unbounded() -> Self {
        Self::unbounded_on(polyoxide_rt::default_runtime())
    }

    /// As [`unbounded`](Self::unbounded), keeping time on `runtime`.
    pub fn unbounded_on(runtime: Arc<dyn Runtime>) -> Self {
        Self::new(None, runtime)
    }

    /// A hold that clamps every [`extend`](Self::extend) to `ceiling`, so a
    /// server asking for longer holds for `ceiling`, on the target's default
    /// runtime.
    pub fn with_ceiling(ceiling: Duration) -> Self {
        Self::with_ceiling_on(ceiling, polyoxide_rt::default_runtime())
    }

    /// As [`with_ceiling`](Self::with_ceiling), keeping time on `runtime`.
    pub fn with_ceiling_on(ceiling: Duration, runtime: Arc<dyn Runtime>) -> Self {
        Self::new(Some(ceiling), runtime)
    }

    fn new(ceiling: Option<Duration>, runtime: Arc<dyn Runtime>) -> Self {
        Self {
            inner: Arc::new(HoldInner {
                until: Mutex::new(None),
                ceiling,
                runtime,
            }),
        }
    }

    /// The runtime this hold keeps time on, which every layer sharing the
    /// hold keeps time on too.
    pub fn runtime(&self) -> &Arc<dyn Runtime> {
        &self.inner.runtime
    }

    /// Hold every request for `delay` from now, clamped to the ceiling.
    ///
    /// Extends a hold in force but never shortens one. A delay too large for
    /// the clock holds for thirty years rather than panicking.
    pub fn extend(&self, delay: Duration) {
        let delay = self
            .inner
            .ceiling
            .map_or(delay, |ceiling| delay.min(ceiling));
        let now = self.inner.runtime.now();
        let until = now.checked_add(delay).unwrap_or(now + FAR_FUTURE);
        let mut slot = self.lock();
        if slot.is_none_or(|current| until > current) {
            *slot = Some(until);
        }
    }

    /// Wait out the hold in force, if any.
    pub async fn wait(&self) {
        loop {
            // Read the deadline and release the guard before awaiting. Holding
            // a `std::sync::MutexGuard` across an await makes the future
            // `!Send`, which every throttle's `acquire` needs it to be.
            let deadline = *self.lock();
            let Some(deadline) = deadline else { return };
            if deadline <= self.inner.runtime.now() {
                return;
            }
            // Loop rather than return after sleeping: a sibling's 429 can push
            // the deadline out while we wait, and waking into a still-active
            // block is how the storm restarts.
            self.inner.runtime.sleep_until(deadline).await;
        }
    }

    /// Whether a hold is in force now.
    pub fn is_held(&self) -> bool {
        self.lock()
            .is_some_and(|until| until > self.inner.runtime.now())
    }

    /// A poison-tolerant lock on the deadline.
    ///
    /// A panic elsewhere must not turn the throttle into a permanent outage;
    /// the worst a torn write can cost here is one early or late wakeup.
    fn lock(&self) -> MutexGuard<'_, Option<Instant>> {
        self.inner
            .until
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for Hold {
    /// [`Hold::unbounded`].
    fn default() -> Self {
        Self::unbounded()
    }
}

impl std::fmt::Debug for Hold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let remaining = (*self.lock())
            .and_then(|until| until.checked_duration_since(self.inner.runtime.now()));
        f.debug_struct("Hold")
            .field("remaining", &remaining)
            .field("ceiling", &self.inner.ceiling)
            .finish()
    }
}
```

The unit tests read `Instant` from `use super::*;`, which was tokio's and is now the runtime's. They measure the paused tokio clock, so name tokio's. Current (lines 130-132):

```rust
#[cfg(test)]
mod tests {
    use super::*;
```

Replace with:

```rust
#[cfg(test)]
mod tests {
    use tokio::time::Instant;

    use super::*;
```

**`polyoxide-core/src/capacity.rs`.** Current (lines 4-8):

```rust
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::Notify;
use tokio::time::Instant;
```

Replace with:

```rust
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use polyoxide_rt::Instant;
use tokio::sync::Notify;
```

Current (lines 42-49, the bucket's doc):

```rust
/// # The hold
///
/// [`acquire`](Self::acquire) waits out the bucket's [`Hold`] before
/// charging, and again after any wait of its own, so a hold set while it
/// waited for tokens is honoured. Every layer of one throttle shares one
/// `Hold`, and [`resize`](Self::resize) keeps it.
///
/// A clone shares the tokens and the hold.
```

Replace with:

```rust
/// # The hold
///
/// [`acquire`](Self::acquire) waits out the bucket's [`Hold`] before
/// charging, and again after any wait of its own, so a hold set while it
/// waited for tokens is honoured. Every layer of one throttle shares one
/// `Hold`, and [`resize`](Self::resize) keeps it.
///
/// The bucket keeps time on its hold's [`runtime`](Hold::runtime): its refill
/// reads that clock and its waits for tokens sleep on it.
///
/// A clone shares the tokens and the hold.
```

Current (lines 120-130, in `build`):

```rust
        assert!(refill_per_sec > 0, "a bucket that never refills");
        Self {
            inner: Arc::new(Inner {
                layer,
                hold,
                state: Mutex::new(State {
                    capacity,
                    refill_per_sec,
                    tokens: f64::from(capacity),
                    updated: Instant::now(),
                    provisional,
```

Replace with:

```rust
        assert!(refill_per_sec > 0, "a bucket that never refills");
        let updated = hold.runtime().now();
        Self {
            inner: Arc::new(Inner {
                layer,
                hold,
                state: Mutex::new(State {
                    capacity,
                    refill_per_sec,
                    tokens: f64::from(capacity),
                    updated,
                    provisional,
```

Current (line 174, in `acquire`):

```rust
                Step::WaitFor(wait) => tokio::time::sleep(wait).await,
```

Replace with:

```rust
                Step::WaitFor(wait) => self.inner.hold.runtime().sleep(wait).await,
```

Current (lines 196-198, in `step`):

```rust
        let mut state = self.lock();
        state.refill(Instant::now());
        let units = if units > state.capacity {
```

Replace with:

```rust
        let mut state = self.lock();
        state.refill(self.inner.hold.runtime().now());
        let units = if units > state.capacity {
```

Current (lines 235-237, in `resize`):

```rust
            let mut state = self.lock();
            state.refill(Instant::now());
            state.capacity = capacity;
```

Replace with:

```rust
            let mut state = self.lock();
            state.refill(self.inner.hold.runtime().now());
            state.capacity = capacity;
```

Current (lines 280-284, the unit tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const LAYER: LayerId = LayerId("test");
```

Replace with:

```rust
#[cfg(test)]
mod tests {
    use tokio::time::Instant;

    use super::*;

    const LAYER: LayerId = LayerId("test");
```

**`polyoxide-core/src/rate_limit.rs`.** Current (lines 10-13):

```rust
use crate::hold::Hold;
use crate::hooks::{AttemptInfo, Charge, Refused, RequestMeta, ResponseMeta, Throttle};
use governor::Quota;
use reqwest::Method;
```

Replace with:

```rust
use crate::hold::Hold;
use crate::hooks::{AttemptInfo, Charge, Refused, RequestMeta, ResponseMeta, Throttle};
use governor::clock::Clock;
use governor::Quota;
use polyoxide_rt::Runtime;
use reqwest::Method;
```

Current (lines 66-74, `Bucket::effective`):

```rust
    fn effective(&self) -> EffectiveQuota {
        EffectiveQuota {
            bucket: self.id,
            count: self.spec.count,
            period: self.spec.period,
            quota: self.quota,
        }
    }
}
```

Replace with:

```rust
    fn effective(&self) -> EffectiveQuota {
        EffectiveQuota {
            bucket: self.id,
            count: self.spec.count,
            period: self.spec.period,
            quota: self.quota,
        }
    }

    /// Wait until the bucket has a token and take it, sleeping on `runtime`.
    ///
    /// This is governor's `until_ready` with its timer swapped for the
    /// runtime's: `check` the bucket and, when it is not ready, sleep the time
    /// `NotUntil` reports against governor's own clock, then `check` again.
    /// Governor keeps its clock, and the bucket keeps its depth of one token;
    /// only the sleep moved.
    async fn until_ready(&self, runtime: &dyn Runtime) {
        loop {
            let wait = match self.limiter.check() {
                Ok(()) => return,
                Err(not_until) => not_until.wait_time_from(self.limiter.clock().now()),
            };
            runtime.sleep(wait).await;
        }
    }
}
```

Current (lines 518-523, `with_hold`):

```rust
    /// Wait out `hold` instead of a hold of the limiter's own, so that
    /// every layer sharing it holds together. Default: [`Hold::unbounded`].
    pub fn with_hold(&mut self, hold: Hold) -> &mut Self {
        self.hold = hold;
        self
    }
```

Replace with:

```rust
    /// Wait out `hold` instead of a hold of the limiter's own, so that
    /// every layer sharing it holds together. Default: [`Hold::unbounded`].
    /// The limiter keeps time on the hold's runtime.
    pub fn with_hold(&mut self, hold: Hold) -> &mut Self {
        self.hold = hold;
        self
    }

    /// Keep time on `runtime`: the limiter's waits for a bucket and for its
    /// hold sleep on it. This gives the table a hold of its own on `runtime`,
    /// replacing the one it has, so call it before
    /// [`with_hold`](Self::with_hold), whose hold brings its own runtime.
    /// Default: the target's default runtime.
    pub fn with_runtime(&mut self, runtime: Arc<dyn Runtime>) -> &mut Self {
        self.hold = Hold::unbounded_on(runtime);
        self
    }
```

Current (lines 562-580, `RateLimiter::acquire`):

```rust
    /// Await the appropriate limiter(s) for this endpoint.
    ///
    /// Waits out any hold a previous 429 imposed, then awaits the default
    /// (general) limiter, then additionally awaits the first matching
    /// endpoint-specific limiter (burst + sustained), then waits out the hold
    /// again, in case it moved while this call waited on a bucket.
    pub async fn acquire(&self, path: &str, method: Option<&Method>) {
        self.inner.hold.wait().await;
        self.inner.general.limiter.until_ready().await;

        if let Some(limit) = self.row_for(path, method) {
            for bucket in &limit.buckets {
                bucket.limiter.until_ready().await;
            }
        }
        // AD-23: a hold set while this call waited on a bucket is honoured,
        // rather than sending into it.
        self.inner.hold.wait().await;
    }
```

Replace with:

```rust
    /// Await the appropriate limiter(s) for this endpoint.
    ///
    /// Waits out any hold a previous 429 imposed, then awaits the default
    /// (general) limiter, then additionally awaits the first matching
    /// endpoint-specific limiter (burst + sustained), then waits out the hold
    /// again, in case it moved while this call waited on a bucket. Every wait
    /// sleeps on the hold's runtime.
    pub async fn acquire(&self, path: &str, method: Option<&Method>) {
        let runtime: &dyn Runtime = self.inner.hold.runtime().as_ref();
        self.inner.hold.wait().await;
        self.inner.general.until_ready(runtime).await;

        if let Some(limit) = self.row_for(path, method) {
            for bucket in &limit.buckets {
                bucket.until_ready(runtime).await;
            }
        }
        // AD-23: a hold set while this call waited on a bucket is honoured,
        // rather than sending into it.
        self.inner.hold.wait().await;
    }
```

**The rules.** In `.claude/rules/rate-limits.md`, current (lines 58-64, the last bullet of "Window quotas"):

```markdown
- **Where the rows come from.** A surface Polymarket publishes figures for has its rows transcribed
  from https://docs.polymarket.com/api-reference/rate-limits and pinned row for row by the
  `documented_*_limits` tests; `quota()` aims a tenth under each. Where a host publishes no figures
  (Data v2) or sits behind CloudFront (perps), the rows were measured against the live host over
  distinct URLs and recorded in its `OBSERVED.md` (below): several v2 routes are CDN-cached, and a
  repeated URL is answered by CloudFront without reaching the origin, so a soak that repeats URLs
  reports a clean run at any rate.
```

Replace with:

```markdown
- **Where the rows come from.** A surface Polymarket publishes figures for has its rows transcribed
  from https://docs.polymarket.com/api-reference/rate-limits and pinned row for row by the
  `documented_*_limits` tests; `quota()` aims a tenth under each. Where a host publishes no figures
  (Data v2) or sits behind CloudFront (perps), the rows were measured against the live host over
  distinct URLs and recorded in its `OBSERVED.md` (below): several v2 routes are CDN-cached, and a
  repeated URL is answered by CloudFront without reaching the origin, so a soak that repeats URLs
  reports a clean run at any rate.
- **A bucket's wait sleeps on the runtime, and governor keeps its clock.** governor's
  `until_ready` sleeps on its own timer, so `Bucket::until_ready` (`polyoxide-core/src/rate_limit.rs`)
  loops on `check`, sleeps the time `NotUntil::wait_time_from` reports against governor's quanta
  clock, and checks again, on the runtime of the table's hold (`WindowQuotaTable::with_runtime`).
  The bucket's depth, its refill and the reserve are unchanged.
```

Current (line 76):

```markdown
  not sent into. Every layer of one throttle shares one `Hold`; `Hold::with_ceiling` clamps it.
```

Replace with:

```markdown
  not sent into. Every layer of one throttle shares one `Hold`; `Hold::with_ceiling` clamps it.
  The hold also carries the throttle's `Runtime` (`Hold::unbounded_on`, `Hold::with_ceiling_on`,
  `Hold::runtime`): it reads its deadline off that clock, and its waits and those of every bucket
  sharing it sleep on it, so a client built on an injected runtime never waits on another.
```

- [ ] **Step 4: Run them to see them pass**

Run `nix develop --command cargo fmt --all` (it rewraps what this task wrote and nothing else), then `nix develop --command cargo test -p polyoxide-core`.

Expected: every test passes, `runtime_injection` with `10 passed`, and the unit tests of `hold`, `capacity` and `rate_limit` unchanged and green.

Then run `nix develop --command cargo test -p polyoxide-polymarket --all-features --test polymarket_limits --test polymarket_throttle --test send_loop` (the tables and the CLOB's throttle, which still build on the default runtime) and `nix develop --command cargo test -p polyoxide-binance -p polyoxide-kalshi --all-features --lib`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(core): holds, capacity buckets and window quotas keep time on a runtime (wasm story 2.3)" -m "A Hold carries the runtime its throttle keeps time on, and CapacityBucket and RateLimiter read their clock and sleep on their hold's. governor's until_ready, which sleeps on its own timer, becomes check plus a sleep on the runtime; its quanta clock, every bucket's depth and the reserve are unchanged. Existing constructors keep their signatures and take the default runtime." -- polyoxide-core/src/hold.rs polyoxide-core/src/capacity.rs polyoxide-core/src/rate_limit.rs polyoxide-core/tests/runtime_injection.rs .claude/rules/rate-limits.md
```

### Task 2.4: Polymarket's tables, throttle and builders

**Files:**
- Modify: `polyoxide-polymarket/src/shared/limits.rs:6-316`
- Modify: `polyoxide-polymarket/src/shared/throttle.rs:3-125`
- Modify: `polyoxide-polymarket/src/shared/mod.rs:6-53`
- Modify: `polyoxide-polymarket/src/shared/signer_limit.rs:337-345`
- Modify: `polyoxide-polymarket/src/clob/client.rs:944-985`
- Modify: `polyoxide-polymarket/src/clob/client.rs:1012-1015`
- Modify: `polyoxide-polymarket/src/gamma/client.rs:72-92`
- Modify: `polyoxide-polymarket/src/data/client.rs:179-183`
- Modify: `polyoxide-polymarket/src/data/client.rs:217-220`
- Modify: `polyoxide-polymarket/src/perps/client.rs:56-96`
- Modify: `polyoxide-polymarket/src/relay/client.rs:2031-2036`
- Modify: `polyoxide-polymarket/src/relay/client.rs:2057-2061`
- Modify: `.claude/rules/rate-limits.md:121-124`
- Test: `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/gamma/client.rs`, `polyoxide-polymarket/src/data/client.rs`, `polyoxide-polymarket/src/perps/client.rs`, `polyoxide-polymarket/src/relay/client.rs`

- [ ] **Step 1: Write the failing tests**

Append to the end of `polyoxide-polymarket/src/shared/limits.rs`. Each table, and the CLOB's throttle with its signer buckets, must keep time on the runtime it is built on:

```rust

#[cfg(test)]
mod runtime_tests {
    //! Each table, and the CLOB's throttle, keeps time on the runtime it is
    //! built on.

    use polyoxide_rt::TokioRuntime;

    use super::*;

    fn injected() -> Arc<dyn Runtime> {
        Arc::new(TokioRuntime)
    }

    /// A runtime over tokio's that records its sleeps.
    #[cfg(feature = "clob")]
    #[derive(Debug, Default)]
    struct Counting {
        inner: TokioRuntime,
        sleeps: std::sync::Mutex<Vec<Duration>>,
    }

    #[cfg(feature = "clob")]
    impl Runtime for Counting {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.sleeps.lock().unwrap().push(duration);
            self.inner.sleep(duration)
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    #[cfg(feature = "clob")]
    #[test]
    fn the_clob_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = clob_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "gamma")]
    #[test]
    fn the_gamma_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = gamma_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "data")]
    #[test]
    fn the_data_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = data_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "relay")]
    #[test]
    fn the_relay_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = relay_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "perps")]
    #[test]
    fn the_perps_table_keeps_time_on_its_runtime() {
        let runtime = injected();
        let limiter = perps_limits_on(Arc::clone(&runtime));
        assert!(Arc::ptr_eq(limiter.hold().runtime(), &runtime));
    }

    #[cfg(feature = "clob")]
    #[tokio::test]
    async fn a_hold_on_the_clob_throttle_is_waited_out_on_its_runtime() {
        use polyoxide_core::hooks::{RequestMeta, Throttle};

        let counting = Arc::new(Counting::default());
        let throttle = crate::shared::clob_throttle_on(counting.clone());
        // The IP table and both signer buckets share one hold, on one runtime.
        assert!(Arc::ptr_eq(
            throttle.signer().hold().runtime(),
            throttle.table().hold().runtime()
        ));

        throttle.hold(Duration::from_millis(60));
        let method = polyoxide_core::reqwest::Method::GET;
        throttle
            .acquire(&RequestMeta {
                method: &method,
                path: "/ok",
                query: &[],
                costs: &[],
            })
            .await
            .unwrap();

        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept
                .iter()
                .any(|d| *d > Duration::ZERO && *d <= Duration::from_millis(60)),
            "the hold was not waited out on the runtime: {slept:?}"
        );
    }
}
```

The builders' tests, one in each client's test module. In `polyoxide-polymarket/src/gamma/client.rs`, current (lines 89-92):

```rust
mod tests {
    use super::*;
    use polyoxide_core::{RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS};

```

Replace with:

```rust
mod tests {
    use super::*;
    use polyoxide_core::{RetryConfig, DEFAULT_POOL_SIZE, DEFAULT_TIMEOUT_MS};

    #[test]
    fn the_builders_runtime_is_the_clients() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let gamma = Gamma::builder().runtime(runtime.clone()).build().unwrap();
        assert!(std::sync::Arc::ptr_eq(
            gamma.http_client.runtime(),
            &runtime
        ));
    }

```

In `polyoxide-polymarket/src/data/client.rs`, current (lines 217-220):

```rust
mod tests {
    use super::*;
    use polyoxide_core::RetryConfig;

```

Replace with:

```rust
mod tests {
    use super::*;
    use polyoxide_core::RetryConfig;

    #[test]
    fn the_builders_runtime_is_every_hosts() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let data = DataApi::builder().runtime(runtime.clone()).build().unwrap();
        for client in [
            &data.http_client,
            &data.pnl_http_client,
            &data.rankings_http_client,
        ] {
            assert!(std::sync::Arc::ptr_eq(client.runtime(), &runtime));
        }
    }

```

In `polyoxide-polymarket/src/perps/client.rs`, current (lines 93-96):

```rust
mod tests {
    use super::*;

    #[test]
```

Replace with:

```rust
mod tests {
    use super::*;

    #[test]
    fn the_builders_runtime_is_the_clients() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let perps = Perps::builder().runtime(runtime.clone()).build().unwrap();
        assert!(std::sync::Arc::ptr_eq(
            perps.http_client.runtime(),
            &runtime
        ));
    }

    #[test]
```

In `polyoxide-polymarket/src/relay/client.rs`, current (lines 2057-2061):

```rust
mod tests {
    use super::*;
    use alloy::primitives::address;
    use polyoxide_core::RetryConfig;

```

Replace with:

```rust
mod tests {
    use super::*;
    use alloy::primitives::address;
    use polyoxide_core::RetryConfig;

    #[test]
    fn the_builders_runtime_is_the_clients() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let client = RelayClient::builder()
            .unwrap()
            .runtime(runtime.clone())
            .build()
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            client.http_client.runtime(),
            &runtime
        ));
    }

```

In `polyoxide-polymarket/src/clob/client.rs`, current (lines 1012-1015):

```rust
mod tests {
    use super::*;
    use polyoxide_core::RetryConfig;

```

Replace with:

```rust
mod tests {
    use super::*;
    use polyoxide_core::RetryConfig;

    #[test]
    fn the_builders_runtime_reaches_the_client_its_signer_buckets_and_its_fallbacks() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let clob = ClobBuilder::new().runtime(runtime.clone()).build().unwrap();
        assert!(std::sync::Arc::ptr_eq(clob.http_client.runtime(), &runtime));
        assert!(std::sync::Arc::ptr_eq(
            clob.signer_limiter.hold().runtime(),
            &runtime
        ));
        #[cfg(feature = "gamma")]
        assert!(std::sync::Arc::ptr_eq(
            clob.gamma.http_client.runtime(),
            &runtime
        ));
        #[cfg(feature = "data")]
        assert!(std::sync::Arc::ptr_eq(
            clob.data.http_client.runtime(),
            &runtime
        ));
    }

```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib runtime`

Expected: it does not compile: ``cannot find function `clob_limits_on` in this scope`` (and the four other `_on` tables), ``cannot find function `clob_throttle_on` in module `crate::shared` ``.

- [ ] **Step 3: Implement**

**`polyoxide-polymarket/src/shared/limits.rs`.** Current (lines 6-13):

```rust
use std::time::Duration;

#[cfg(feature = "clob")]
use polyoxide_core::rate_limit::Matching;
use polyoxide_core::rate_limit::{RateLimiter, WindowQuotaTable};
#[cfg(feature = "clob")]
use polyoxide_core::reqwest::Method;
```

Replace with:

```rust
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "clob")]
use polyoxide_core::rate_limit::Matching;
use polyoxide_core::rate_limit::{RateLimiter, WindowQuotaTable};
#[cfg(feature = "clob")]
use polyoxide_core::reqwest::Method;
use polyoxide_rt::Runtime;
```

Current (lines 60-63, the head of `clob_limits`, under its long doc comment):

```rust
#[cfg(feature = "clob")]
pub fn clob_limits() -> RateLimiter {
    let get = Some(Method::GET);
    let mut table = WindowQuotaTable::new(9_000, TEN_SECONDS);
```

Replace with:

```rust
#[cfg(feature = "clob")]
pub fn clob_limits() -> RateLimiter {
    clob_limits_on(polyoxide_rt::default_runtime())
}

/// [`clob_limits`], keeping time on `runtime`: the table's waits for a bucket
/// and for its hold sleep on it, and so do the signer buckets of a
/// [`ClobThrottle`](crate::shared::ClobThrottle) composed over its hold.
#[cfg(feature = "clob")]
pub fn clob_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let get = Some(Method::GET);
    let mut table = WindowQuotaTable::new(9_000, TEN_SECONDS);
    table.with_runtime(runtime);
```

Current (lines 153-155):

```rust
#[cfg(feature = "gamma")]
pub fn gamma_limits() -> RateLimiter {
    let mut table = WindowQuotaTable::new(4_000, TEN_SECONDS);
```

Replace with:

```rust
#[cfg(feature = "gamma")]
pub fn gamma_limits() -> RateLimiter {
    gamma_limits_on(polyoxide_rt::default_runtime())
}

/// [`gamma_limits`], keeping time on `runtime`.
#[cfg(feature = "gamma")]
pub fn gamma_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let mut table = WindowQuotaTable::new(4_000, TEN_SECONDS);
    table.with_runtime(runtime);
```

Current (lines 193-195):

```rust
#[cfg(feature = "data")]
pub fn data_limits() -> RateLimiter {
    let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
```

Replace with:

```rust
#[cfg(feature = "data")]
pub fn data_limits() -> RateLimiter {
    data_limits_on(polyoxide_rt::default_runtime())
}

/// [`data_limits`], keeping time on `runtime`.
#[cfg(feature = "data")]
pub fn data_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let mut table = WindowQuotaTable::new(1_000, TEN_SECONDS);
    table.with_runtime(runtime);
```

Current (lines 218-221):

```rust
#[cfg(feature = "relay")]
pub fn relay_limits() -> RateLimiter {
    WindowQuotaTable::new(25, Duration::from_secs(60)).build()
}
```

Replace with:

```rust
#[cfg(feature = "relay")]
pub fn relay_limits() -> RateLimiter {
    relay_limits_on(polyoxide_rt::default_runtime())
}

/// [`relay_limits`], keeping time on `runtime`.
#[cfg(feature = "relay")]
pub fn relay_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    let mut table = WindowQuotaTable::new(25, Duration::from_secs(60));
    table.with_runtime(runtime);
    table.build()
}
```

Current (lines 248-251, the head of `perps_limits`):

```rust
#[cfg(feature = "perps")]
pub fn perps_limits() -> RateLimiter {
    const PIN_KLINES: u32 = 30;
    const PIN_TRADES: u32 = 10;
```

Replace with:

```rust
#[cfg(feature = "perps")]
pub fn perps_limits() -> RateLimiter {
    perps_limits_on(polyoxide_rt::default_runtime())
}

/// [`perps_limits`], keeping time on `runtime`.
#[cfg(feature = "perps")]
pub fn perps_limits_on(runtime: Arc<dyn Runtime>) -> RateLimiter {
    const PIN_KLINES: u32 = 30;
    const PIN_TRADES: u32 = 10;
```

Current (line 260):

```rust
    let mut table = WindowQuotaTable::new(PERPS_GENERAL, TEN_SECONDS);
```

Replace with:

```rust
    let mut table = WindowQuotaTable::new(PERPS_GENERAL, TEN_SECONDS);
    table.with_runtime(runtime);
```

**`polyoxide-polymarket/src/shared/throttle.rs`.** Current (lines 3-12):

```rust
use std::time::Duration;

use polyoxide_core::hold::Hold;
use polyoxide_core::hooks::{
    AttemptInfo, Charge, Cost, LayerCharge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle,
};
use polyoxide_core::rate_limit::RateLimiter;

use super::clob_limits;
use super::signer_limit::{SignerLimiter, TradingBucket, TradingRequest};
```

Replace with:

```rust
use std::sync::Arc;
use std::time::Duration;

use polyoxide_core::hold::Hold;
use polyoxide_core::hooks::{
    AttemptInfo, Charge, Cost, LayerCharge, LayerId, Refused, RequestMeta, ResponseMeta, Throttle,
};
use polyoxide_core::rate_limit::RateLimiter;
use polyoxide_rt::Runtime;

use super::signer_limit::{SignerLimiter, TradingBucket, TradingRequest};
use super::{clob_limits, clob_limits_on};
```

Current (lines 121-125, the end of the file):

```rust
/// The CLOB's throttle: [`clob_limits`] and a [`SignerLimiter`] at the
/// tightest tier, over one hold.
pub fn clob_throttle() -> ClobThrottle {
    ClobThrottle::new(clob_limits(), SignerLimiter::new())
}
```

Replace with:

```rust
/// The CLOB's throttle: [`clob_limits`] and a [`SignerLimiter`] at the
/// tightest tier, over one hold.
pub fn clob_throttle() -> ClobThrottle {
    ClobThrottle::new(clob_limits(), SignerLimiter::new())
}

/// [`clob_throttle`], keeping time on `runtime`: the IP table, both signer
/// buckets and the hold between them.
pub fn clob_throttle_on(runtime: Arc<dyn Runtime>) -> ClobThrottle {
    ClobThrottle::new(
        clob_limits_on(Arc::clone(&runtime)),
        SignerLimiter::new_on(runtime),
    )
}
```

**`polyoxide-polymarket/src/shared/mod.rs`.** Current (lines 6-8):

```rust
//! - The five window-quota tables, one per API surface: [`clob_limits`],
//!   [`gamma_limits`], [`data_limits`], [`relay_limits`] and [`perps_limits`],
//!   each behind its module's feature.
```

Replace with:

```rust
//! - The five window-quota tables, one per API surface: [`clob_limits`],
//!   [`gamma_limits`], [`data_limits`], [`relay_limits`] and [`perps_limits`],
//!   each behind its module's feature, and each with an `_on` twin
//!   ([`clob_limits_on`] and so on) that keeps time on a given runtime.
```

Current (lines 39-53):

```rust
#[cfg(feature = "clob")]
pub use limits::clob_limits;
#[cfg(feature = "data")]
pub use limits::data_limits;
#[cfg(feature = "gamma")]
pub use limits::gamma_limits;
#[cfg(feature = "perps")]
pub use limits::perps_limits;
#[cfg(feature = "relay")]
pub use limits::relay_limits;
pub use policy::PolymarketRetryPolicy;
#[cfg(feature = "clob")]
pub use throttle::{
    clob_throttle, signer_cost, ClobThrottle, CLOUDFLARE, SIGNER_CANCEL, SIGNER_ORDER,
};
```

Replace with:

```rust
#[cfg(feature = "clob")]
pub use limits::{clob_limits, clob_limits_on};
#[cfg(feature = "data")]
pub use limits::{data_limits, data_limits_on};
#[cfg(feature = "gamma")]
pub use limits::{gamma_limits, gamma_limits_on};
#[cfg(feature = "perps")]
pub use limits::{perps_limits, perps_limits_on};
#[cfg(feature = "relay")]
pub use limits::{relay_limits, relay_limits_on};
pub use policy::PolymarketRetryPolicy;
#[cfg(feature = "clob")]
pub use throttle::{
    clob_throttle, clob_throttle_on, signer_cost, ClobThrottle, CLOUDFLARE, SIGNER_CANCEL,
    SIGNER_ORDER,
};
```

**`polyoxide-polymarket/src/shared/signer_limit.rs`.** Current (lines 337-345, `SignerLimiter::new`):

```rust
    /// Create a limiter at the default (tightest) tier, with a hold of its
    /// own.
    pub fn new() -> Self {
        Self::build(
            Tier::default(),
            RateLimitStatus::default(),
            Hold::unbounded(),
        )
    }
```

Replace with:

```rust
    /// Create a limiter at the default (tightest) tier, with a hold of its
    /// own.
    pub fn new() -> Self {
        Self::new_on(polyoxide_rt::default_runtime())
    }

    /// As [`new`](Self::new), with a hold of its own that keeps time on
    /// `runtime`, as both buckets do.
    pub fn new_on(runtime: std::sync::Arc<dyn polyoxide_rt::Runtime>) -> Self {
        Self::build(
            Tier::default(),
            RateLimitStatus::default(),
            Hold::unbounded_on(runtime),
        )
    }
```

**The builders.** In `polyoxide-polymarket/src/clob/client.rs`, current (lines 944-955):

```rust
    pub fn build(self) -> Result<Clob, ClobError> {
        // The IP table and the per-signer buckets over one hold, so a 429
        // stops both; the client keeps a handle on the signer layer for the
        // tier and telemetry it adopts.
        let throttle = shared::clob_throttle();
        let signer_limiter = throttle.signer().clone();
        let http_client = self
            .config
            .http_builder()
            .with_throttle(throttle)
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

Replace with:

```rust
    pub fn build(self) -> Result<Clob, ClobError> {
        // The client and its throttle keep time on one runtime.
        let runtime = self.config.runtime_or_default();
        // The IP table and the per-signer buckets over one hold, so a 429
        // stops both; the client keeps a handle on the signer layer for the
        // tier and telemetry it adopts.
        let throttle = shared::clob_throttle_on(runtime.clone());
        let signer_limiter = throttle.signer().clone();
        let http_client = self
            .config
            .http_builder()
            .runtime(runtime.clone())
            .with_throttle(throttle)
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

Current (lines 968-972, the Gamma fallback):

```rust
            crate::gamma::Gamma::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .build()
                .map_err(ClobError::Gamma)?
```

Replace with:

```rust
            crate::gamma::Gamma::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .runtime(runtime.clone())
                .build()
                .map_err(ClobError::Gamma)?
```

Current (lines 979-983, the Data fallback):

```rust
            crate::data::DataApi::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .build()
                .map_err(ClobError::Data)?
```

Replace with:

```rust
            crate::data::DataApi::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .runtime(runtime)
                .build()
                .map_err(ClobError::Data)?
```

The two `.runtime(...)` lines on the fallbacks are the contract line "`ClobBuilder::build` passes its runtime to the Gamma and Data clients it builds, so they share the CLOB's" (spec D3). `the_builders_runtime_reaches_the_client_its_signer_buckets_and_its_fallbacks` pins it: with no `gamma` or `data` set on the builder, `ClobBuilder::build` makes both, and the test asserts by `Arc::ptr_eq` that their clients' runtime is the very `Arc` given to the builder. Without the two lines each fallback builds on `default_runtime()`, a different `Arc` from the test's, so the assertions fail; a counting-runtime test is not needed for this, because the clock reads and sleeps of a Gamma or Data client already go through `HttpClient::runtime()`, which Tasks 2.2 and 2.3 prove. A `Gamma` or `Data` client the caller passes to the builder keeps the runtime it was built with.

In `polyoxide-polymarket/src/gamma/client.rs`, current (lines 72-76):

```rust
        let http_client = self
            .config
            .http_builder()
            .with_rate_limiter(shared::gamma_limits())
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

Replace with:

```rust
        let runtime = self.config.runtime_or_default();
        let http_client = self
            .config
            .http_builder()
            .runtime(runtime.clone())
            .with_rate_limiter(shared::gamma_limits_on(runtime))
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

In `polyoxide-polymarket/src/data/client.rs`, current (lines 179-183):

```rust
        let http_client = self
            .config
            .http_builder()
            .with_rate_limiter(shared::data_limits())
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

Replace with:

```rust
        let runtime = self.config.runtime_or_default();
        let http_client = self
            .config
            .http_builder()
            .runtime(runtime.clone())
            .with_rate_limiter(shared::data_limits_on(runtime))
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

In `polyoxide-polymarket/src/perps/client.rs`, current (lines 56-67):

```rust
pub struct PerpsBuilder {
    config: ClientConfig,
    rate_limiter: Option<RateLimiter>,
}

impl PerpsBuilder {
    fn new() -> Self {
        Self {
            config: ClientConfig::new(DEFAULT_BASE_URL, DEFAULT_MAX_CONCURRENT),
            rate_limiter: Some(shared::perps_limits()),
        }
    }
```

Replace with:

```rust
pub struct PerpsBuilder {
    config: ClientConfig,
    /// `None` builds the default table, `shared::perps_limits_on`, on the
    /// client's runtime.
    rate_limiter: Option<RateLimiter>,
}

impl PerpsBuilder {
    fn new() -> Self {
        Self {
            config: ClientConfig::new(DEFAULT_BASE_URL, DEFAULT_MAX_CONCURRENT),
            rate_limiter: None,
        }
    }
```

Current (lines 70-90):

```rust
    /// Replace the rate limiter.
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Perps, PerpsError> {
        let mut builder = self
            .config
            .http_builder()
            .with_retry_policy(PolymarketRetryPolicy);
        if let Some(limiter) = self.rate_limiter {
            builder = builder.with_rate_limiter(limiter);
        }
        Ok(Perps {
            http_client: builder.build()?,
        })
    }
```

Replace with:

```rust
    /// Replace the rate limiter. It keeps time on the runtime it was built
    /// on, so build it with [`shared::perps_limits_on`] when the client has a
    /// runtime of its own.
    pub fn with_rate_limiter(mut self, limiter: RateLimiter) -> Self {
        self.rate_limiter = Some(limiter);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<Perps, PerpsError> {
        let runtime = self.config.runtime_or_default();
        let limiter = self
            .rate_limiter
            .unwrap_or_else(|| shared::perps_limits_on(runtime.clone()));
        Ok(Perps {
            http_client: self
                .config
                .http_builder()
                .runtime(runtime)
                .with_retry_policy(PolymarketRetryPolicy)
                .with_rate_limiter(limiter)
                .build()?,
        })
    }
```

In `polyoxide-polymarket/src/relay/client.rs`, current (lines 2031-2036):

```rust
        let http_client = self
            .config
            .http_builder()
            .with_rate_limiter(shared::relay_limits())
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

Replace with:

```rust
        let runtime = self.config.runtime_or_default();
        let http_client = self
            .config
            .http_builder()
            .runtime(runtime.clone())
            .with_rate_limiter(shared::relay_limits_on(runtime))
            .with_retry_policy(PolymarketRetryPolicy)
            .build()?;
```

**The rule.** In `.claude/rules/rate-limits.md`, current (lines 121-124):

```markdown
- `shared::ClobThrottle` (`shared::clob_throttle()`, `polyoxide-polymarket/src/shared/throttle.rs`)
  composes the two layers over one `Hold`: each attempt is charged one request against
  `clob_limits()` by method and path, then the signer bucket its `shared::signer_cost` names.
  Every trading route names its bucket with `shared::signer_cost`.
```

Replace with:

```markdown
- `shared::ClobThrottle` (`shared::clob_throttle()`, `polyoxide-polymarket/src/shared/throttle.rs`)
  composes the two layers over one `Hold`: each attempt is charged one request against
  `clob_limits()` by method and path, then the signer bucket its `shared::signer_cost` names.
  Every trading route names its bucket with `shared::signer_cost`.
- Each table and the throttle has an `_on` twin that keeps time on a given runtime (`clob_limits_on`,
  `gamma_limits_on`, `data_limits_on`, `relay_limits_on`, `perps_limits_on`, `clob_throttle_on`,
  `SignerLimiter::new_on`). Every builder resolves its runtime once with
  `ClientConfig::runtime_or_default` and builds its table or throttle with the twin, so a client
  and its throttle share one runtime; the plain constructors take the target's default.
```

- [ ] **Step 4: Run them to see them pass**

Run `nix develop --command cargo fmt --all` (it rewraps what this task wrote and nothing else), then `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib runtime`.

Expected: the eleven new tests pass: the five tables' `the_…_table_keeps_time_on_its_runtime`, `a_hold_on_the_clob_throttle_is_waited_out_on_its_runtime`, and the five builders' `the_builders_runtime_…`.

Then run `nix develop --command cargo test -p polyoxide-polymarket --all-features` (the whole crate on its default runtime, the mock, limits, throttle and send-loop suites included) and, for the feature builds the new code is gated by, `nix develop --command cargo check -p polyoxide-polymarket --no-default-features --features gamma`, then the same with `data`, `perps`, `relay` and `clob` in turn.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): the tables, the CLOB throttle and the builders keep time on the client's runtime (wasm story 2.4)" -m "Each window-quota table, ClobThrottle and SignerLimiter gain an _on twin; each HTTP builder resolves its runtime once and builds its client and its throttle on it, and ClobBuilder::build hands it to the Gamma and Data clients it builds. The plain constructors are unchanged and take the default runtime." -- polyoxide-polymarket/src/shared/limits.rs polyoxide-polymarket/src/shared/throttle.rs polyoxide-polymarket/src/shared/mod.rs polyoxide-polymarket/src/shared/signer_limit.rs polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/src/gamma/client.rs polyoxide-polymarket/src/data/client.rs polyoxide-polymarket/src/perps/client.rs polyoxide-polymarket/src/relay/client.rs .claude/rules/rate-limits.md
```

### Task 2.5: Move `docs/MUTANTS.md`'s citations and prove the mutants again

Tasks 2.1, 2.3 and 2.4 moved lines that `docs/MUTANTS.md` cites in `polyoxide-core/src/client.rs`, `hold.rs`, `rate_limit.rs` and `polyoxide-polymarket/src/shared/signer_limit.rs`, and changed the code of one cited line (`Hold::wait`'s sleep). `test_mutants_ledger.py` fails until the citations follow, and the repo's rule is to move the row and prove its mutant again. `send.rs` did not move: its cited lines are all above the one line Task 2.2 changed.

**Files:**
- Modify: `docs/MUTANTS.md:1-175`
- Modify: `.github/scripts/tests/test_mutants_ledger.py:1-120`
- Test: `.github/scripts/tests/test_mutants_ledger.py`

- [ ] **Step 1: Write the failing test**

The ledger test is the failing test: it reads each cited line.

- [ ] **Step 2: Run it to see it fail**

Run: `bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`

Expected: failures naming the moved citations, among them `polyoxide-core/src/hold.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/src/client.rs` and `polyoxide-polymarket/src/shared/signer_limit.rs`, each reporting that the line no longer holds the snippet it was cited for.

- [ ] **Step 3: Implement**

Move every citation that points into the five files, in the ledger and in the test's table, in one pass. A snippet that occurs once in its file is found by search; one that does not (`}`, and the second `self.inner.hold.wait().await;` of `acquire`) keeps its distance from the entry before it. `Hold::wait`'s sleep is the one cited line whose code changed. Run from the repository root:

```bash
python3 - <<'PY'
import json
import re
import sys
from pathlib import Path

LEDGER = Path("docs/MUTANTS.md")
TABLE = Path(".github/scripts/tests/test_mutants_ledger.py")
TOUCHED = (
    "polyoxide-core/src/send.rs",
    "polyoxide-core/src/hold.rs",
    "polyoxide-core/src/rate_limit.rs",
    "polyoxide-core/src/client.rs",
    "polyoxide-polymarket/src/shared/signer_limit.rs",
)
# The one cited line whose code changed.
CHANGED = {
    ("polyoxide-core/src/hold.rs", 91): "self.inner.runtime.sleep_until(deadline).await;",
}
ENTRY = re.compile(r'^    \("(?P<file>[^"]+)", (?P<line>\d+)\): (?P<snippet>.+),$', re.M)
CITATION = re.compile(r"(polyoxide[a-z-]*/[A-Za-z0-9_/.-]+\.rs):(\d+)(?:-(\d+))?")

table = TABLE.read_text()
entries = {}
for m in ENTRY.finditer(table):
    if m["file"] in TOUCHED:
        old = int(m["line"])
        entries.setdefault(m["file"], {})[old] = CHANGED.get(
            (m["file"], old), json.loads(m["snippet"])
        )

moves = {}
for file, wanted in entries.items():
    lines = Path(file).read_text().splitlines()
    previous = None
    for old in sorted(wanted):
        snippet = wanted[old]
        hits = [i + 1 for i, line in enumerate(lines) if snippet in line]
        if len(hits) == 1:
            new = hits[0]
        elif previous is not None:
            new = previous[1] + old - previous[0]
        else:
            sys.exit(f"{file}:{old}: {snippet!r} is not unique and has no entry before it")
        if not 0 < new <= len(lines) or snippet not in lines[new - 1]:
            sys.exit(f"{file}:{old}: {snippet!r} is not on line {new}")
        moves[(file, old)] = (new, snippet)
        previous = (old, new)
        print(f"{file}:{old} -> {new}")


def entry_line(m):
    key = (m["file"], int(m["line"]))
    if key not in moves:
        return m[0]
    new, snippet = moves[key]
    return f'    ("{m["file"]}", {new}): {json.dumps(snippet)},'


def citation(m):
    file = m[1]
    if file not in TOUCHED:
        return m[0]
    ends = []
    for old in (m[2], m[3]):
        if old is not None:
            ends.append(str(moves[(file, int(old))][0]))
    return f"{file}:" + "-".join(ends)


TABLE.write_text(ENTRY.sub(entry_line, table))
LEDGER.write_text(CITATION.sub(citation, LEDGER.read_text()))
PY
git diff --stat -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```

Expected: it prints one `file:old -> new` line for each cited line of the five files (those of `send.rs` map to themselves) and changes only citations: `git diff` of the two files shows no line whose text differs apart from its line numbers and the one changed snippet of `hold.rs`.

Run: `bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`

Expected: it passes.

Now prove each mutant again. `prove` applies a mutation with `sed`, runs the row's tests under it, requires every named test to fail, restores the file with `git checkout`, and requires them to pass again. The mutations are the ledger's own, written as `sed` text; one holds `allow_burst(` and one the reserve constant, and each is undone in the same call:

```bash
bash <<'SH'
set -uo pipefail
prove() {
  local file=$1 expr=$2 pkgs=$3; shift 3
  sed -i "$expr" "$file"
  if git diff --quiet -- "$file"; then echo "the mutation did not apply to $file"; return 1; fi
  local out
  out=$(nix develop --command cargo test -j 4 $pkgs -- "$@" 2>&1)
  git checkout -- "$file"
  local name
  for name in "$@"; do
    grep -Eq "^test .*${name} \.\.\. FAILED" <<<"$out" || { echo "no failure for $name under the mutant of $file"; return 1; }
  done
  nix develop --command cargo test -j 4 $pkgs -- "$@" || return 1
  echo "proved: $file, $*"
}

RA=polyoxide-venue/src/retry_after.rs
HOLD=polyoxide-core/src/hold.rs
RATE=polyoxide-core/src/rate_limit.rs
SIGNER=polyoxide-polymarket/src/shared/signer_limit.rs
B='s/requested\.map_or(computed, |requested| requested\.max(computed))/requested.unwrap_or(computed)/'

# (b) Retry-After only extends a wait.
prove $RA "$B" "-p polyoxide-core -p polyoxide-venue --lib" \
  retry_after_below_our_own_backoff_does_not_shorten_the_wait a_requested_wait_only_ever_lengthens_the_backoff || exit 1

# (b), a zero Retry-After: the parser refuses it twice, and the delay is the third.
Z="s/secs <= 0\.0 {/secs < 0.0 {/; s/Some(wait)\.filter(|wait| !wait\.is_zero())/Some(wait)/; $B"
prove $RA "$Z" "-p polyoxide-core -p polyoxide-venue --lib" \
  retry_after_below_our_own_backoff_does_not_shorten_the_wait retry_after_zero_still_backs_off_exponentially_across_attempts every_parser_row || exit 1
prove $RA "$Z" "-p polyoxide-polymarket --all-features --test mock_data" \
  retry_after_zero_does_not_turn_the_retry_loop_into_a_hot_loop a_429_makes_the_next_request_wait_even_though_it_never_saw_one a_429_on_the_pnl_host_holds_the_data_host || exit 1

# (c) cooldowns only extend.
C='s/if slot\.is_none_or(|current| until > current) {/if true {/'
prove $HOLD "$C" "-p polyoxide-core --lib" a_hold_never_shortens || exit 1
prove $HOLD "$C" "-p polyoxide-polymarket --all-features --test polymarket_limits" a_shorter_cooldown_never_cuts_a_longer_one_short || exit 1

# (c), a hold extended mid-wait: wait sleeps once.
M='s/self\.inner\.runtime\.sleep_until(deadline)\.await;/self.inner.runtime.sleep_until(deadline).await; return;/'
prove $HOLD "$M" "-p polyoxide-core --lib" a_hold_extended_mid_wait_is_honoured_in_full || exit 1

# (c), the re-check after a bucket wait (AD-23): delete the second wait of acquire.
LINE=$(grep -n 'self\.inner\.hold\.wait()\.await;' $RATE | tail -1 | cut -d: -f1)
prove $RATE "${LINE}d" "-p polyoxide-core --lib" a_hold_set_during_a_bucket_wait_is_honoured || exit 1

# (d) quota() leaves depth at one token.
D='s/\.expect("quota interval must be non-zero")/.expect("quota interval must be non-zero").allow_burst(std::num::NonZeroU32::new(count).unwrap())/'
prove $RATE "$D" "-p polyoxide-core --lib" no_quota_admits_more_than_its_published_count_in_one_window every_quota_reserves_headroom_below_the_published_count || exit 1
prove $RATE "$D" "-p polyoxide-polymarket --all-features --test polymarket_limits" every_configured_bucket_satisfies_the_quota_it_publishes || exit 1

# (i) the per-signer buckets hold their published burst.
prove $SIGNER 's/let capacity = tier\.burst(bucket)\.max(1);/let capacity = 1;/' "-p polyoxide-polymarket --all-features --lib" \
  a_batch_within_capacity_is_admitted adopting_a_higher_tier_admits_a_batch_that_was_impossible the_order_and_cancel_buckets_are_independent batch_cost_is_charged_in_full_not_as_one_request || exit 1

# (j) a window quota aims at 90% of its published count: the first mutant fails only the first test.
prove $RATE 's/const RESERVED_FRACTION: u32 = 10;/const RESERVED_FRACTION: u32 = 20;/' "-p polyoxide-core --lib" a_published_150_per_10s_admits_135_per_window || exit 1
prove $RATE 's/let target = count\.saturating_sub(count\.div_ceil(RESERVED_FRACTION));/let target = count;/' "-p polyoxide-core --lib" \
  a_published_150_per_10s_admits_135_per_window every_quota_reserves_headroom_below_the_published_count || exit 1

git diff --exit-code -- polyoxide-core polyoxide-venue polyoxide-polymarket && echo "every mutant restored"
SH
```

Expected: every `prove` ends with a `proved:` line, the last line is `every mutant restored`, and `git status --short` lists only `docs/MUTANTS.md` and `.github/scripts/tests/test_mutants_ledger.py`. A `no failure for …` line names a row whose tests did not fail under the mutant, or whose mutant did not compile: stop and read it.

Record the re-proof. Run from the repository root:

```bash
python3 - <<'PY'
import subprocess
from pathlib import Path

ledger = Path("docs/MUTANTS.md")
head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()
paragraph = (
    "The wasm32 plan's story 2 put `polyoxide-core`'s holds, capacity buckets and window quotas, "
    "and the clients that own them, on an injected runtime, which moved lines of `client.rs`, "
    "`hold.rs`, `rate_limit.rs` and `polyoxide-polymarket/src/shared/signer_limit.rs` and changed "
    "`Hold::wait`'s sleep; rows (b), (b), a zero `Retry-After`, (c), (c), a hold extended mid-wait, "
    "(c), the re-check after a bucket wait, (d), (i) and (j) were proved again on top of "
    f"`{head}`, on Rust 1.99.0 with `-j 4`: each row's tests failed with its mutant in place and "
    "passed again once the file was restored.\n\n"
)
text = ledger.read_text()
marker = "## The rules\n"
assert text.count(marker) == 1
ledger.write_text(text.replace(marker, paragraph + marker))
PY
git diff -- docs/MUTANTS.md | head -40
```

- [ ] **Step 4: Run it to see it pass**

Run: `bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`

Expected: it passes.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(mutants): move the rows the runtime moved and prove their mutants again (wasm story 2.5)" -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```

### Task 2.6: Binance's weight budget and builder

**Files:**
- Modify: `polyoxide-binance/src/usdm/weight.rs:17`
- Modify: `polyoxide-binance/src/usdm/weight.rs:235-242`
- Modify: `polyoxide-binance/src/usdm/weight.rs:267-277`
- Modify: `polyoxide-binance/src/usdm/weight.rs:289`
- Modify: `polyoxide-binance/src/usdm/weight.rs:315-318`
- Modify: `polyoxide-binance/src/usdm/weight.rs:339-340`
- Modify: `polyoxide-binance/src/usdm/weight.rs:351`
- Modify: `polyoxide-binance/src/usdm/weight.rs:424-425`
- Modify: `polyoxide-binance/src/usdm/weight.rs:506-508`
- Modify: `polyoxide-binance/src/usdm/weight.rs:795-799`
- Modify: `polyoxide-binance/src/usdm/mod.rs:121-132`
- Modify: `polyoxide-binance/src/usdm/mod.rs:136-139`
- Modify: `.claude/rules/binance.md:45-48`
- Modify: `.claude/rules/rate-limits.md:135`
- Test: `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/src/usdm/mod.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-binance/src/usdm/weight.rs`, the unit tests read `Instant` from `use super::*;`, which becomes the runtime's in Step 3; they measure tokio's paused clock, so name it. Current (lines 506-508):

```rust
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
```

Replace with:

```rust
mod tests {
    use tokio::time::Instant;

    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
```

Current (lines 795-799, the end of the file):

```rust
        tokio::time::sleep(Duration::from_millis(100)).await;
        budget.begin_cooldown(Duration::from_secs(10));
        assert_eq!(waiter.await.unwrap(), Duration::from_millis(10_100));
    }
}
```

Replace with:

```rust
        tokio::time::sleep(Duration::from_millis(100)).await;
        budget.begin_cooldown(Duration::from_secs(10));
        assert_eq!(waiter.await.unwrap(), Duration::from_millis(10_100));
    }

    /// A runtime over tokio's that records its sleeps, so a paused-clock test
    /// can say which waits went through it.
    #[derive(Debug, Default)]
    struct Counting {
        inner: polyoxide_rt::TokioRuntime,
        sleeps: Mutex<Vec<Duration>>,
    }

    impl Runtime for Counting {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.sleeps.lock().unwrap().push(duration);
            self.inner.sleep(duration)
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_wait_for_the_next_minute_is_slept_on_the_budgets_runtime() {
        let counting = Arc::new(Counting::default());
        let budget = WeightBudget::with_clock_on(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::Tokio {
                start: Instant::now(),
                start_ms: ON_A_MINUTE + 15_000,
            },
            counting.clone(),
        );
        for _ in 0..54 {
            budget.acquire(Cost::Weight(40)).await;
        }
        budget.acquire(Cost::Weight(1)).await;

        // 15 s into the minute, 45 s are left of it.
        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept.contains(&Duration::from_secs(45)),
            "the wait for the next minute did not sleep on the runtime: {slept:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn funding_slots_are_slept_on_the_budgets_runtime() {
        let counting = Arc::new(Counting::default());
        let budget = WeightBudget::new_on(counting.clone());
        budget.acquire(Cost::Funding).await;
        budget.acquire(Cost::Funding).await;

        // The second request waits one slot of the funding pace.
        let interval =
            WindowQuotaTable::paced_interval(PUBLISHED_FUNDING_PER_FIVE_MINUTES, FUNDING_PERIOD);
        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept
                .iter()
                .any(|d| *d <= interval && *d + Duration::from_millis(1) >= interval),
            "no sleep of one funding slot ({interval:?}) went through the runtime: {slept:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_cooldown_is_waited_out_on_the_budgets_runtime() {
        let counting = Arc::new(Counting::default());
        let budget = WeightBudget::new_on(counting.clone());

        budget.begin_cooldown(Duration::from_secs(7));
        budget.acquire(Cost::Weight(1)).await;

        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept
                .iter()
                .any(|d| *d > Duration::from_secs(6) && *d <= Duration::from_secs(7)),
            "the cooldown was not waited out on the runtime: {slept:?}"
        );
    }
}
```

In `polyoxide-binance/src/usdm/mod.rs`, current (lines 136-139):

```rust
mod tests {
    use super::*;

    #[test]
```

Replace with:

```rust
mod tests {
    use super::*;

    #[test]
    fn the_builders_runtime_is_the_clients_and_the_budgets() {
        let runtime: std::sync::Arc<dyn polyoxide_rt::Runtime> =
            std::sync::Arc::new(polyoxide_rt::TokioRuntime);
        let usdm = Usdm::builder().runtime(runtime.clone()).build().unwrap();
        assert!(std::sync::Arc::ptr_eq(usdm.http.runtime(), &runtime));
        assert!(std::sync::Arc::ptr_eq(
            usdm.weight_budget().runtime(),
            &runtime
        ));
    }

    #[test]
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features --lib`

Expected: it does not compile: ``no function or associated item named `with_clock_on` found for struct `WeightBudget` ``, ``no function or associated item named `new_on` ``, and ``no method named `runtime` found for reference `&WeightBudget` ``.

- [ ] **Step 3: Implement**

**`polyoxide-binance/src/usdm/weight.rs`.** Current (line 17):

```rust
use tokio::time::Instant;
```

Replace with:

```rust
use polyoxide_rt::{Instant, Runtime};
```

The paused-time test clock keeps tokio's own `Instant`, from a test-only module. Current (lines 235-242, the head of `Clock`):

```rust
/// Where the current UTC minute comes from.
#[derive(Debug, Clone)]
enum Clock {
    /// The system clock, which is what Binance's minute follows.
    System,
    /// Wall time derived from tokio's clock, so a paused-time test controls it.
    #[cfg(test)]
    Tokio { start: Instant, start_ms: u64 },
```

Replace with:

```rust
/// The clock a paused-time test drives: tokio's own, which `Clock::Tokio`
/// derives wall time from.
#[cfg(test)]
mod paused {
    pub(super) use tokio::time::Instant;
}

/// Where the current UTC minute comes from.
#[derive(Debug, Clone)]
enum Clock {
    /// The system clock, which is what Binance's minute follows.
    System,
    /// Wall time derived from tokio's clock, so a paused-time test controls it.
    #[cfg(test)]
    Tokio {
        start: paused::Instant,
        start_ms: u64,
    },
```

Current (lines 267-277, `new` and the head of `with_clock`):

```rust
    /// A budget for Binance's published limits, less the reserve.
    pub fn new() -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::System,
        )
    }

    fn with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self {
        Self {
```

Replace with:

```rust
    /// A budget for Binance's published limits, less the reserve, on the
    /// target's default runtime.
    pub fn new() -> Self {
        Self::with_clock(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::System,
        )
    }

    /// As [`new`](Self::new), keeping time on `runtime`: its cooldown, its
    /// funding pace and its wait for the next minute all sleep on it.
    pub fn new_on(runtime: Arc<dyn Runtime>) -> Self {
        Self::with_clock_on(
            PUBLISHED_WEIGHT_PER_MINUTE,
            PUBLISHED_FUNDING_PER_FIVE_MINUTES,
            Clock::System,
            runtime,
        )
    }

    fn with_clock(weight_per_minute: u32, funding_per_five_minutes: u32, clock: Clock) -> Self {
        Self::with_clock_on(
            weight_per_minute,
            funding_per_five_minutes,
            clock,
            polyoxide_rt::default_runtime(),
        )
    }

    fn with_clock_on(
        weight_per_minute: u32,
        funding_per_five_minutes: u32,
        clock: Clock,
        runtime: Arc<dyn Runtime>,
    ) -> Self {
        Self {
```

Current (line 289):

```rust
                hold: Hold::with_ceiling(MAX_COOLDOWN),
```

Replace with:

```rust
                hold: Hold::with_ceiling_on(MAX_COOLDOWN, runtime),
```

Current (lines 315-318):

```rust
    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute
    }
```

Replace with:

```rust
    /// The most weight this budget spends in one minute.
    pub fn per_minute(&self) -> u32 {
        self.inner.per_minute
    }

    /// The runtime this budget keeps time on: its hold's.
    pub(crate) fn runtime(&self) -> &Arc<dyn Runtime> {
        self.inner.hold.runtime()
    }
```

Current (lines 339-340, in `acquire`):

```rust
                let slot = self.reserve_funding_slot();
                tokio::time::sleep_until(slot).await;
```

Replace with:

```rust
                let slot = self.reserve_funding_slot();
                self.runtime().sleep_until(slot).await;
```

Current (line 351):

```rust
                    Err(until_next_minute) => tokio::time::sleep(until_next_minute).await,
```

Replace with:

```rust
                    Err(until_next_minute) => self.runtime().sleep(until_next_minute).await,
```

Current (lines 424-425, in `reserve_funding_slot`):

```rust
    fn reserve_funding_slot(&self) -> Instant {
        let now = Instant::now();
```

Replace with:

```rust
    fn reserve_funding_slot(&self) -> Instant {
        let now = self.runtime().now();
```

The unit tests' helper `budget` (lines 513-522) is unchanged: its `Instant` is the tokio one the test module now names.

**`polyoxide-binance/src/usdm/mod.rs`.** Current (lines 121-132):

```rust
    pub fn build(self) -> Result<Usdm, UsdmError> {
        let budget = self.budget.unwrap_or_default();
        let http = self
            .config
            .http_builder()
            .with_throttle(budget.clone())
            .with_retry_policy(UsdmRetryPolicy {
                budget: budget.clone(),
            })
            .build()?;
        Ok(Usdm { http, budget })
    }
```

Replace with:

```rust
    pub fn build(self) -> Result<Usdm, UsdmError> {
        // The client and the budget it makes keep time on one runtime. A budget
        // passed to `weight_budget` brings its own, which then stands.
        let runtime = self.config.runtime_or_default();
        let budget = self
            .budget
            .unwrap_or_else(|| WeightBudget::new_on(runtime.clone()));
        let http = self
            .config
            .http_builder()
            .runtime(runtime)
            .with_throttle(budget.clone())
            .with_retry_policy(UsdmRetryPolicy {
                budget: budget.clone(),
            })
            .build()?;
        Ok(Usdm { http, budget })
    }
```

**The rules.** In `.claude/rules/binance.md`, current (lines 45-48):

```markdown
The budget is the client's `Throttle` on core's send loop: each route builder (`GetKlines`,
`GetTime` and the rest, over a private `Routed<T>`) hands core's `Request` its weight as a `Cost`
on `usdm::weight::WEIGHT_LAYER` or `FUNDING_LAYER`, and every `Usdm` built with one budget shares
its hold. Pass one with `UsdmBuilder::weight_budget`.
```

Replace with:

```markdown
The budget is the client's `Throttle` on core's send loop: each route builder (`GetKlines`,
`GetTime` and the rest, over a private `Routed<T>`) hands core's `Request` its weight as a `Cost`
on `usdm::weight::WEIGHT_LAYER` or `FUNDING_LAYER`, and every `Usdm` built with one budget shares
its hold. Pass one with `UsdmBuilder::weight_budget`. The budget keeps time on its hold's runtime
(its cooldown, its funding pace and its wait for the next minute sleep on it): the builder makes
one with `WeightBudget::new_on` the client's runtime, and a budget passed in keeps its own.
```

In `.claude/rules/rate-limits.md`, current (line 135):

```markdown
- **Binance**: `usdm::WeightBudget` (`binance.md`).
```

Replace with:

```markdown
- **Binance**: `usdm::WeightBudget` (`binance.md`), whose `new_on` keeps its hold, its funding pace
  and its minute waits on a given runtime.
```

- [ ] **Step 4: Run them to see them pass**

Run `nix develop --command cargo fmt --all` (it rewraps what this task wrote and nothing else), then `nix develop --command cargo test -p polyoxide-binance --all-features`.

Expected: every test passes, the paused-clock tests of `weight` and `policy` unchanged, and the new ones (`the_wait_for_the_next_minute_is_slept_on_the_budgets_runtime`, `funding_slots_are_slept_on_the_budgets_runtime`, `a_cooldown_is_waited_out_on_the_budgets_runtime`, `the_builders_runtime_is_the_clients_and_the_budgets`) included. Then `nix develop --command cargo check -p polyoxide-binance --no-default-features --features usdm`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(binance): the weight budget and its builder keep time on the client's runtime (wasm story 2.6)" -m "WeightBudget::new_on keeps its hold, its funding pace and its wait for the next minute on a given runtime; UsdmBuilder::build makes its budget on the client's runtime. WeightBudget::new is unchanged and takes the default runtime." -- polyoxide-binance/src/usdm/weight.rs polyoxide-binance/src/usdm/mod.rs .claude/rules/binance.md .claude/rules/rate-limits.md
```

### Task 2.7: Kalshi's throttle and builder

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:19-30`
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:93-96`
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:681-696`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs:192-197`
- Modify: `polyoxide-kalshi/src/predictions/mod.rs:211-214`
- Modify: `.claude/rules/rate-limits.md:136-140`
- Modify: `.claude/rules/clients.md:18-26`
- Test: `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/src/predictions/mod.rs`

- [ ] **Step 1: Write the failing tests**

In `polyoxide-kalshi/src/predictions/throttle.rs`, current (lines 681-696, the head of the paused-clock tests):

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
    use polyoxide_core::{Cost, LayerId};
    use tokio::time::Instant;

    use super::*;

    fn read(units: u32) -> Cost {
        Cost {
            layer: READ,
            units,
            exact: true,
        }
    }
```

Replace with:

```rust
#[cfg(test)]
mod tests {
    use polyoxide_core::reqwest::Method;
    use polyoxide_core::{Cost, LayerId};
    use tokio::time::Instant;

    use super::*;

    /// A runtime over tokio's that records its sleeps, so a paused-clock test
    /// can say which waits went through it.
    #[derive(Debug, Default)]
    struct Counting {
        inner: polyoxide_rt::TokioRuntime,
        sleeps: std::sync::Mutex<Vec<Duration>>,
    }

    impl Runtime for Counting {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.sleeps.lock().unwrap().push(duration);
            self.inner.sleep(duration)
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    fn read(units: u32) -> Cost {
        Cost {
            layer: READ,
            units,
            exact: true,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_throttle_on_a_runtime_waits_for_tokens_through_it() {
        let counting = Arc::new(Counting::default());
        let throttle = PredictionsThrottle::new_on(counting.clone());
        spend(&throttle, 60).await;
        spend(&throttle, 1).await;

        // The 61st read finds the bucket empty, and ten tokens at 200 a
        // second take 50 ms.
        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept
                .iter()
                .any(|d| (Duration::from_millis(49)..=Duration::from_millis(50)).contains(d)),
            "the wait for tokens did not sleep on the runtime: {slept:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_hold_on_a_throttle_on_a_runtime_is_waited_out_on_it() {
        let counting = Arc::new(Counting::default());
        let throttle = PredictionsThrottle::new_on(counting.clone());

        throttle.hold(Duration::from_secs(7));
        charge(&throttle, &[read(10)]).await.unwrap();

        let slept = counting.sleeps.lock().unwrap().clone();
        assert!(
            slept
                .iter()
                .any(|d| *d > Duration::from_secs(6) && *d <= Duration::from_secs(7)),
            "the hold was not waited out on the runtime: {slept:?}"
        );
    }
```

`charge` and `spend` are the module's helpers further down; they are in scope.

In `polyoxide-kalshi/src/predictions/mod.rs`, current (lines 211-214):

```rust
mod tests {
    use super::*;

    #[test]
```

Replace with:

```rust
mod tests {
    use super::*;

    #[test]
    fn the_builders_runtime_is_the_clients_and_the_throttles() {
        let runtime: Arc<dyn polyoxide_rt::Runtime> = Arc::new(polyoxide_rt::TokioRuntime);
        let kalshi = Predictions::builder()
            .runtime(runtime.clone())
            .build()
            .unwrap();
        assert!(Arc::ptr_eq(kalshi.inner.http.runtime(), &runtime));
        assert!(Arc::ptr_eq(
            kalshi.inner.throttle.bucket().hold().runtime(),
            &runtime
        ));
    }

    #[test]
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib runtime`

Expected: it does not compile: ``no function or associated item named `new_on` found for struct `PredictionsThrottle` ``.

- [ ] **Step 3: Implement**

**`polyoxide-kalshi/src/predictions/throttle.rs`.** Current (lines 19-30):

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, CapacityBucket, Charge, Hold, LayerCharge, Refused, Request, RequestMeta,
    ResponseMeta, Throttle,
};

use crate::predictions::request::{Route, DEFAULT_COST, READ};
```

Replace with:

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use polyoxide_core::{
    AttemptInfo, CapacityBucket, Charge, Hold, LayerCharge, Refused, Request, RequestMeta,
    ResponseMeta, Throttle,
};
use polyoxide_rt::Runtime;

use crate::predictions::request::{Route, DEFAULT_COST, READ};
```

Current (lines 93-96, `PredictionsThrottle::new`):

```rust
    /// A throttle with a full Read bucket at Basic's sizing and a public bucket at the unsigned
    /// client's.
    pub(crate) fn new() -> Self {
        let hold = Hold::unbounded();
```

Replace with:

```rust
    /// A throttle with a full Read bucket at Basic's sizing and a public bucket at the unsigned
    /// client's, on the target's default runtime.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::new_on(polyoxide_rt::default_runtime())
    }

    /// As `new`, keeping time on `runtime`: both buckets share the one hold, which carries it.
    pub(crate) fn new_on(runtime: Arc<dyn Runtime>) -> Self {
        let hold = Hold::unbounded_on(runtime);
```

**`polyoxide-kalshi/src/predictions/mod.rs`.** Current (lines 192-197):

```rust
        let throttle = PredictionsThrottle::new();
        let http = self
            .config
            .http_builder()
            .with_throttle(throttle.clone())
            .build()?;
```

Replace with:

```rust
        let runtime = self.config.runtime_or_default();
        let throttle = PredictionsThrottle::new_on(runtime.clone());
        let http = self
            .config
            .http_builder()
            .runtime(runtime)
            .with_throttle(throttle.clone())
            .build()?;
```

**The rules.** In `.claude/rules/rate-limits.md`, current (lines 136-140):

```markdown
- **Kalshi**: the unsigned client's Read bucket is depth 600 refilling 180 tokens a second (20
  reads a second measured clean, less a tenth). A keyed client's public reads draw on a second
  bucket sized as the unsigned client's, so only `GET /account/limits` draws on the key's bucket,
  which holds its published capacity like the signer buckets (the signed probe ran on the demo
  host; production signed public reads are unmeasured).
```

Replace with:

```markdown
- **Kalshi**: the unsigned client's Read bucket is depth 600 refilling 180 tokens a second (20
  reads a second measured clean, less a tenth). A keyed client's public reads draw on a second
  bucket sized as the unsigned client's, so only `GET /account/limits` draws on the key's bucket,
  which holds its published capacity like the signer buckets (the signed probe ran on the demo
  host; production signed public reads are unmeasured). Both buckets share one `Hold` on the
  client's runtime (`PredictionsThrottle::new_on`).
```

In `.claude/rules/clients.md`, every venue now builds its throttle on the client's runtime. Current (lines 18-26, as Task 2.1 left them):

```markdown
- Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry
  config, concurrency, runtime, and the client's default concurrency: relay 2; gamma, data, perps,
  Binance and Kalshi 4; clob 8) and gets its six transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config`, `max_concurrent` and `runtime`, from
  `polyoxide_core::client_config_setters!`; `runtime` names `polyoxide_rt::Runtime`, which core
  does not re-export, so the venue crate declares `polyoxide-rt` itself. Its `build` starts from
  `ClientConfig::http_builder()` and adds the throttle and retry policy the venue installs; the
  config sets no `gzip`.
```

Replace with:

```markdown
- Each venue builder holds a `polyoxide_core::ClientConfig` (base URL, timeout, pool size, retry
  config, concurrency, runtime, and the client's default concurrency: relay 2; gamma, data, perps,
  Binance and Kalshi 4; clob 8) and gets its six transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config`, `max_concurrent` and `runtime`, from
  `polyoxide_core::client_config_setters!`; `runtime` names `polyoxide_rt::Runtime`, which core
  does not re-export, so the venue crate declares `polyoxide-rt` itself. Its `build` resolves the
  runtime once with `ClientConfig::runtime_or_default()`, starts from
  `ClientConfig::http_builder()` and gives it `HttpClientBuilder::runtime`, and adds the throttle
  (built on the same runtime) and retry policy the venue installs; the config sets no `gzip`.
```

- [ ] **Step 4: Run them to see them pass**

Run `nix develop --command cargo fmt --all` (it rewraps what this task wrote and nothing else), then `nix develop --command cargo test -p polyoxide-kalshi --all-features`.

Expected: every test passes, the paused-clock throttle tests unchanged and the new ones (`a_throttle_on_a_runtime_waits_for_tokens_through_it`, `a_hold_on_a_throttle_on_a_runtime_is_waited_out_on_it`, `the_builders_runtime_is_the_clients_and_the_throttles`) included.

Then run the story's own gates over everything it touched: `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`, `nix develop --command cargo test -p polyoxide-core -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-kalshi --all-features`, `python3 scripts/test_body_diff.py --baseline v0.40.0` (no `[[drift]]` entry is needed: only `use` lines of test modules changed), and `bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_claude_rules.py tests/test_mutants_ledger.py -q'`.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(kalshi): the Read throttle and the builder keep time on the client's runtime (wasm story 2.7)" -m "PredictionsThrottle::new_on makes both Read buckets and their hold on a given runtime; PredictionsBuilder::build resolves its runtime once and builds the client and the throttle on it." -- polyoxide-kalshi/src/predictions/throttle.rs polyoxide-kalshi/src/predictions/mod.rs .claude/rules/rate-limits.md .claude/rules/clients.md
```

## Story 3: `polyoxide_core::http` and the move of every reqwest vocabulary path

- **Developer:** intermediate-dev
- **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/hooks.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/tests/mock_request.rs`, `polyoxide-binance/examples/weight_probe.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/policy.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/auth.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `polyoxide-polymarket/examples/cf_burst_probe.rs`, `polyoxide-polymarket/examples/gamma_batch_ceiling.rs`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-polymarket/src/client.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/api/auth.rs`, `polyoxide-polymarket/src/clob/api/markets.rs`, `polyoxide-polymarket/src/clob/api/notifications.rs`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/authenticator.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/data/error.rs`, `polyoxide-polymarket/src/data/v2/error.rs`, `polyoxide-polymarket/src/gamma/api/markets.rs`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/perps/api/public.rs`, `polyoxide-polymarket/src/perps/error.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/config.rs`, `polyoxide-polymarket/src/relay/error.rs`, `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/shared/policy.rs`, `polyoxide-polymarket/src/shared/signer_limit.rs`, `polyoxide-polymarket/tests/polymarket_limits.rs`, `polyoxide-polymarket/tests/polymarket_throttle.rs`, `polyoxide-polymarket/tests/send_loop.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/tests/failure_tags.rs`, `polyoxide-test-support/tests/token_cost_throttle.rs`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`, `.claude/rules/send-loop.md`, `.claude/rules/ci-gates.md`, `Cargo.lock`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 3 → Stories 4 to 11"; uses nothing from earlier stories
- **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
- **Review:** light (a path-only move; no behaviour changes)

Core re-exports the `http` crate as `polyoxide_core::http`, and every `Method`, `StatusCode`, `HeaderMap`, `HeaderValue` and `header::*` path in src, tests and examples moves to it (inside core, to `http` itself). Reqwest 0.12 re-exports these very types, so nothing else changes: `Client`, `Response`, `RequestBuilder`, `redirect` and `get` keep their `reqwest` paths for story 4, and alloy's reqwest in relay is untouched. The story adds DRIFT row `R22` and a `[[drift]]` entry for each test body the move changes, and updates `send-loop.md`'s vocabulary sentence. It holds no landmine marker. Every task leaves the tree green because `pub use reqwest;` stays: a crate not yet moved still compiles, and the sweep each task runs names what is left.

**The sweep.** One regular expression finds every vocabulary path still on reqwest (it ignores `Client`, `Response`, `RequestBuilder`, `redirect`, `get` and `Error`). Each task runs it over its crate before and after:

```bash
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- '<crate>/*.rs'
```

At the start of the story it prints 18 lines in `polyoxide-core`, 9 in `polyoxide-binance`, 5 in `polyoxide-kalshi`, 52 in `polyoxide-polymarket` and 2 in `polyoxide-test-support`. Files the entry lists that hold no vocabulary path (core's `send.rs`, the `live_*` suites, `read_soak`, `cf_burst_probe`, `gamma_batch_ceiling`, `info_soak` and test-support's `query.rs` use only `Client`, `Response`, `RequestBuilder` or `get`) need no edit.

**The drift entries.** `ci/test-body-diff.toml` compares against `v0.40.0`. A test item whose body or signature names one of the moved paths differs after normalising (a `use` line is dropped, so a changed import alone is not a diff), and gets one `[[drift]]` entry naming row `R22` and the diff's sha256. The script prints each diff as `::error::<item> differs at <file>:<line> after normalising (sha256 <64 hex digits>)`; the sha256 comes from that line, never from a guess. The item names this story lists are the expected ones: an item the script prints that the list lacks gets an entry too, and a listed item the script does not print (it is newer than `v0.40.0`) gets none, since an entry that matches nothing fails.

### Task 3.1: core re-exports `http`, and core's own paths move

**Files:**
- Modify: `polyoxide-core/Cargo.toml:23`
- Modify: `polyoxide-core/src/lib.rs:80-87,123-126`
- Modify: `polyoxide-core/src/client.rs:4,8,298`
- Modify: `polyoxide-core/src/error.rs:4-5`
- Modify: `polyoxide-core/src/health.rs:6`
- Modify: `polyoxide-core/src/hooks.rs:20-21`
- Modify: `polyoxide-core/src/rate_limit.rs:13,404`
- Modify: `polyoxide-core/src/request.rs:4-5`
- Modify: `polyoxide-core/tests/mock_request.rs:5,452,553,575,615,619`
- Modify: `_bmad-output/specs/spec-venue-extensibility/divergences.md:38,65`
- Modify: `ci/test-body-diff.toml` (append)
- Modify: `.claude/rules/send-loop.md:98-102`
- Modify: `.claude/rules/ci-gates.md:188,200-201`
- Modify: `Cargo.lock` (cargo writes it)
- Test: `polyoxide-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

In `polyoxide-core/src/lib.rs`, the test module opens with these lines (123-126):

```rust
#[cfg(test)]
mod tests {
    use super::*;

```

Add this test directly after the `use super::*;` line (before `test_truncate_for_log_short_string_unchanged`):

```rust
    /// `polyoxide_core::http` names the types reqwest sends with: each value
    /// below is assigned to the reqwest-typed binding with no conversion, so a
    /// re-export of any other version of the crate would not compile.
    #[test]
    fn the_http_vocabulary_is_the_type_reqwest_sends() {
        let method: reqwest::Method = http::Method::POST;
        let status: reqwest::StatusCode = http::StatusCode::TOO_MANY_REQUESTS;
        let mut headers: reqwest::header::HeaderMap = http::HeaderMap::new();
        headers.insert(
            http::header::RETRY_AFTER,
            http::HeaderValue::from_static("3"),
        );

        assert_eq!(method, reqwest::Method::POST);
        assert_eq!(status.as_u16(), 429);
        assert_eq!(
            headers[reqwest::header::RETRY_AFTER].to_str().unwrap(),
            "3"
        );
    }

```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-core --lib the_http_vocabulary`

Expected: the build fails with an `error[E0433]` naming `http` as an unresolved crate or module (the test module sees no `http`), then `could not compile `polyoxide-core` (lib test)`.

Run the sweep over core: `git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-core/*.rs'`. Expected: 18 lines, in `src/client.rs`, `src/error.rs`, `src/health.rs`, `src/hooks.rs`, `src/rate_limit.rs`, `src/request.rs` and `tests/mock_request.rs`.

- [ ] **Step 3: Implement**

(a) `polyoxide-core/Cargo.toml`: the dependency the vocabulary comes from. After the line (23)

```toml
reqwest = { version = "0.12", features = ["json", "rustls-tls", "gzip"], default-features = false }
```

add

```toml
# The request vocabulary reqwest re-exports (`Method`, `StatusCode`, `HeaderMap`,
# `HeaderValue`, `header`), re-exported as `polyoxide_core::http`. Reqwest 0.12
# is built on `http` 1, so these are the same types.
http = "1"
```

(b) `polyoxide-core/src/lib.rs`: the lines (80-87)

```rust
/// The HTTP client core sends with, re-exported whole.
///
/// Venue crates name its types (`Response`, `StatusCode`, `HeaderMap`,
/// `Method`) through this path and never depend on reqwest themselves, so
/// core alone decides its features and every client sends the same headers
/// (AD-18). CI's dependency fence fails when any other member declares it.
pub use reqwest;
```

become

```rust
/// The request vocabulary core's HTTP client speaks: `Method`, `StatusCode`,
/// `HeaderMap`, `HeaderValue` and `header::*`.
///
/// Venue code, tests and examples name these through this path, never through
/// reqwest. They are the very types reqwest re-exports, so moving a path here
/// changes no type.
pub use http;

/// The HTTP client core sends with, re-exported whole.
///
/// Venue crates name its client types (`Client`, `Response`, `RequestBuilder`)
/// through this path and never depend on reqwest themselves, so core alone
/// decides its features and every client sends the same headers (AD-18). CI's
/// dependency fence fails when any other member declares it.
pub use reqwest;
```

(c) Core's own sources name `http` directly, which resolves to the dependency in every module but the crate root. Replace these lines, each single line for single line:

| File:line | Current | New |
|---|---|---|
| `src/client.rs:4` | `use reqwest::Method;` | `use http::Method;` |
| `src/client.rs:8` | `use reqwest::header::RETRY_AFTER;` | `use http::header::RETRY_AFTER;` |
| `src/client.rs:298` | `    use reqwest::StatusCode;` | `    use http::StatusCode;` |
| `src/error.rs:4` | `use reqwest::header::{HeaderMap, RETRY_AFTER};` | `use http::header::{HeaderMap, RETRY_AFTER};` |
| `src/error.rs:5` | `use reqwest::StatusCode;` | `use http::StatusCode;` |
| `src/hooks.rs:20` | `use reqwest::header::{HeaderMap, RETRY_AFTER};` | `use http::header::{HeaderMap, RETRY_AFTER};` |
| `src/hooks.rs:21` | `use reqwest::{Method, StatusCode};` | `use http::{Method, StatusCode};` |
| `src/rate_limit.rs:13` | `use reqwest::Method;` | `use http::Method;` |
| `src/rate_limit.rs:404` | `/// use reqwest::Method;` | `/// use polyoxide_core::http::Method;` |

Two imports mix a moved type with `Response`, which stays on reqwest for story 4, so each splits in two. In `src/health.rs`, line 6

```rust
use reqwest::{Method, Response};
```

becomes

```rust
use http::Method;
use reqwest::Response;
```

and in `src/request.rs`, lines 4-5

```rust
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use reqwest::{Method, Response};
```

become

```rust
use http::header::{HeaderValue, CONTENT_TYPE};
use http::Method;
use reqwest::Response;
```

(d) `polyoxide-core/tests/mock_request.rs`: an integration test names core through `polyoxide_core`. Replace

| Line | Current | New |
|---|---|---|
| 5 | `use reqwest::StatusCode;` | `use polyoxide_core::http::StatusCode;` |
| 452 | `        .method(reqwest::Method::POST)` | `        .method(polyoxide_core::http::Method::POST)` |
| 553 | `        .method(reqwest::Method::DELETE)` | `        .method(polyoxide_core::http::Method::DELETE)` |
| 575 | `    let mut parts = polyoxide_core::RequestParts::new(reqwest::Method::GET, "/slow");` | `    let mut parts = polyoxide_core::RequestParts::new(polyoxide_core::http::Method::GET, "/slow");` |
| 615 | `    let mut parts = polyoxide_core::RequestParts::new(reqwest::Method::GET, "/slow");` | `    let mut parts = polyoxide_core::RequestParts::new(polyoxide_core::http::Method::GET, "/slow");` |
| 619 | `    assert_eq!(response.status(), reqwest::StatusCode::OK);` | `    assert_eq!(response.status(), polyoxide_core::http::StatusCode::OK);` |

(e) DRIFT row `R22`. In `_bmad-output/specs/spec-venue-extensibility/divergences.md`, the heading (line 38)

```
## DRIFT: decided 2026-10-08 (R11 2026-10-12, R12 2026-10-16, R13 and R14 2026-10-18, R16 and R17 2026-10-21, R18 2026-10-23, R19, R20 and R21 2026-11-01)
```

becomes

```
## DRIFT: decided 2026-10-08 (R11 2026-10-12, R12 2026-10-16, R13 and R14 2026-10-18, R16 and R17 2026-10-21, R18 2026-10-23, R19, R20 and R21 2026-11-01, R22 2026-11-02)
```

and a row ends the table, after R21's row (line 65), which closes with the text `Both are acknowledged by `[[drift]]`, the first moving from R17. |`. Add on the next line:

```
| R22 | The wasm32 seams (spec `2026-11-02-wasm-runtime-transport-design.md`) change the paths and types tests name, never what they assert, except that a row asserting an `ApiError::Network` case no transport can produce goes with the variant, its decode case re-pinned on `Response::json`. | the test items `ci/test-body-diff.toml`'s `[[drift]]` entries name with row `R22` | Applied by the plan's stories 3, 4 and 5. Story 3 moves `Method`, `StatusCode`, `HeaderMap`, `HeaderValue` and `header::*` to `polyoxide_core::http`, the same types reqwest re-exports; stories 4 and 5 re-type responses and sockets. Each changed test body keeps its assertions and is acknowledged by its sha256; a later story that changes a test body again cites this row. |
```

(f) `.claude/rules/send-loop.md`: the bullet (lines 98-102)

```
- Core is the only member that declares reqwest (AD-18): it enables `json`, `rustls-tls` and
  `gzip` with no default features and re-exports the crate as `polyoxide_core::reqwest`, through
  which every other member's src, tests and examples name its types. No crate but core sends
  through `HttpClient`'s reqwest client. `test_dependency_fences.py` fails on any other member's
  direct dependency, so no crate can change reqwest's features for the rest.
```

becomes

```
- Core is the only member that declares reqwest (AD-18): it enables `json`, `rustls-tls` and
  `gzip` with no default features and re-exports the crate as `polyoxide_core::reqwest`, through
  which every other member's src, tests and examples name its client types (`Client`, `Response`,
  `RequestBuilder`). The request vocabulary, `Method`, `StatusCode`, `HeaderMap`, `HeaderValue`
  and `header::*`, they name through `polyoxide_core::http`, core's re-export of the `http` crate:
  the very types reqwest re-exports, so a path change only. No crate but core sends
  through `HttpClient`'s reqwest client. `test_dependency_fences.py` fails on any other member's
  direct dependency, so no crate can change reqwest's features for the rest.
```

(g) Format, build (cargo adds `http` to `polyoxide-core`'s entry in `Cargo.lock`), and find core's diffs:

```bash
nix develop --command cargo fmt --all
nix develop --command cargo build -p polyoxide-core --all-features
python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | grep '::error::'
```

Expected: one `::error::` line for each of these four items of `polyoxide-core/tests/mock_request.rs`, each with its sha256: `a_post_sends_its_body_serialised_once`, `a_request_s_costs_reach_the_throttle`, `a_request_parts_timeout_bounds_its_attempt` and `a_request_parts_timeout_outlasts_the_client_s`. Append to `ci/test-body-diff.toml` one table for each, with the sha256 its line printed:

```toml
[[drift]]
file = "polyoxide-core/tests/mock_request.rs"
item = "a_post_sends_its_body_serialised_once"
row = "R22"
sha256 = "<the 64 hex digits its ::error:: line printed>"
```

(the other three tables differ only in `item` and `sha256`).

(h) `.claude/rules/ci-gates.md` states the count of `[[drift]]` entries and lists them (AD-21: the commit that adds entries edits that text). Its line 188 reads

```
- **The 27 `[[drift]]` entries:** `mock_gamma`'s `get_many_fans_out_closed_true_and_false`
```

and becomes

```
- **The 31 `[[drift]]` entries:** `mock_gamma`'s `get_many_fans_out_closed_true_and_false`
```

(27 and the four tables just appended; if `grep -c '^\[\[drift\]\]' ci/test-body-diff.toml` prints another number, write that one). The bullet's last two lines (200-201)

```
  `instrument_id` (R20); and `perps::types`' enum test, moved again, and `spec_agreement_perps`'s
  `every_enum_matches_the_spec`, for `InstrumentCategory`'s undocumented `pre_ipo` (R21).
```

become

```
  `instrument_id` (R20); and `perps::types`' enum test, moved again, and `spec_agreement_perps`'s
  `every_enum_matches_the_spec`, for `InstrumentCategory`'s undocumented `pre_ipo` (R21). The
  test bodies the wasm32 seams' stories re-typed cite R22: story 3's, which name `Method` and
  `StatusCode` through `polyoxide_core::http`, are core's `mock_request` items
  `a_post_sends_its_body_serialised_once`, `a_request_s_costs_reach_the_throttle`,
  `a_request_parts_timeout_bounds_its_attempt` and
  `a_request_parts_timeout_outlasts_the_client_s`.
```

Stories 3.2 and 3.4 extend that sentence, and stories 4 and 5 add their items after it with their own count.

Then confirm the allowlists did not move: `http` is already in every HTTP crate's tree through reqwest, so the lists name the same crates:

```bash
bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
git status --short ci/dep-allowlists
```

Expected: no output. If a file did change, add it to the commit and say in the message which crate joined the tree and why.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test -p polyoxide-core --all-features
nix develop --command cargo clippy -p polyoxide-core --all-targets --all-features -- -D warnings
python3 scripts/test_body_diff.py --baseline v0.40.0
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-core/*.rs'
```

Expected: the format check is silent; `test tests::the_http_vocabulary_is_the_type_reqwest_sends ... ok` and every other core test passes; clippy is clean; `test_body_diff.py` exits 0 and prints four `Acknowledged as DRIFT R22` lines; the sweep prints nothing (exit 1).

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(core): re-export http as polyoxide_core::http and move core's paths to it (wasm story 3.1)" -- polyoxide-core/Cargo.toml polyoxide-core/src/lib.rs polyoxide-core/src/client.rs polyoxide-core/src/error.rs polyoxide-core/src/health.rs polyoxide-core/src/hooks.rs polyoxide-core/src/rate_limit.rs polyoxide-core/src/request.rs polyoxide-core/tests/mock_request.rs _bmad-output/specs/spec-venue-extensibility/divergences.md ci/test-body-diff.toml .claude/rules/send-loop.md .claude/rules/ci-gates.md Cargo.lock
```

### Task 3.2: `polyoxide-binance` names the vocabulary through core's `http`

**Files:**
- Modify: `polyoxide-binance/examples/weight_probe.rs:91`
- Modify: `polyoxide-binance/src/usdm/error.rs:108,209,212,217`
- Modify: `polyoxide-binance/src/usdm/policy.rs:5,73`
- Modify: `polyoxide-binance/src/usdm/weight.rs:495`
- Modify: `polyoxide-binance/tests/failure_tags_usdm.rs:128`
- Modify: `ci/test-body-diff.toml` (append)
- Modify: `.claude/rules/ci-gates.md:188,200-201`
- Test: the existing `polyoxide-binance` suites and `ci/test-body-diff.toml`

- [ ] **Step 1: Write the failing check**

The move is a path change with no new behaviour, so its red state is the sweep:

```bash
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-binance/*.rs'
```

- [ ] **Step 2: Run it to see it fail**

Expected: 9 lines, all `polyoxide_core::reqwest::...` paths: `examples/weight_probe.rs:91` (`StatusCode::OK`), `src/usdm/error.rs:108`, `209`, `212` and `217`, `src/usdm/policy.rs:5` and `73`, `src/usdm/weight.rs:495` and `tests/failure_tags_usdm.rs:128`.

- [ ] **Step 3: Implement**

Every hit has the one shape `polyoxide_core::reqwest::(Method|StatusCode|header)`, so one substitution moves them all (`Client` and the other reqwest paths in `live_usdm.rs` and `weight_probe.rs` do not match it and stay):

```bash
git grep -lE 'polyoxide_core::reqwest::(Method|StatusCode|header)\b' -- 'polyoxide-binance/*.rs' \
  | xargs sed -i -E 's#polyoxide_core::reqwest::(Method|StatusCode|header)\b#polyoxide_core::http::\1#g'
```

The lines it changes, with their current text:

| File:line | Current | New |
|---|---|---|
| `examples/weight_probe.rs:91` | `    if status != polyoxide_core::reqwest::StatusCode::OK {` | `    if status != polyoxide_core::http::StatusCode::OK {` |
| `src/usdm/error.rs:108` | `                .get(polyoxide_core::reqwest::header::RETRY_AFTER)` | `                .get(polyoxide_core::http::header::RETRY_AFTER)` |
| `src/usdm/error.rs:209` | `        let mut headers = polyoxide_core::reqwest::header::HeaderMap::new();` | `        let mut headers = polyoxide_core::http::header::HeaderMap::new();` |
| `src/usdm/error.rs:212` | `                polyoxide_core::reqwest::header::RETRY_AFTER,` | `                polyoxide_core::http::header::RETRY_AFTER,` |
| `src/usdm/error.rs:217` | `            polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),` | `            polyoxide_core::http::StatusCode::from_u16(status).unwrap(),` |
| `src/usdm/policy.rs:5` | `use polyoxide_core::reqwest::StatusCode;` | `use polyoxide_core::http::StatusCode;` |
| `src/usdm/policy.rs:73` | `    use polyoxide_core::reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};` | `    use polyoxide_core::http::header::{HeaderMap, HeaderValue, RETRY_AFTER};` |
| `src/usdm/weight.rs:495` | `fn used_weight(headers: &polyoxide_core::reqwest::header::HeaderMap) -> Option<u32> {` | `fn used_weight(headers: &polyoxide_core::http::header::HeaderMap) -> Option<u32> {` |
| `tests/failure_tags_usdm.rs:128` | `        polyoxide_core::reqwest::StatusCode::from_u16(status).unwrap(),` | `        polyoxide_core::http::StatusCode::from_u16(status).unwrap(),` |

Then format, and find the diffs:

```bash
nix develop --command cargo fmt --all
python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | grep '::error::'
```

Expected: one `::error::` line, with its sha256, for each of `polyoxide-binance/src/usdm/error.rs`'s `tests::response` and `polyoxide-binance/tests/failure_tags_usdm.rs`'s `raw` (the helpers that build a response; no other binance test body names a moved path). Append a table for each to `ci/test-body-diff.toml`, with the sha256 its line printed:

```toml
[[drift]]
file = "polyoxide-binance/src/usdm/error.rs"
item = "tests::response"
row = "R22"
sha256 = "<the 64 hex digits its ::error:: line printed>"

[[drift]]
file = "polyoxide-binance/tests/failure_tags_usdm.rs"
item = "raw"
row = "R22"
sha256 = "<the 64 hex digits its ::error:: line printed>"
```

Then `.claude/rules/ci-gates.md` follows (AD-21). In its line 188, the count in "The 31 `[[drift]]` entries" becomes 33 (or the number `grep -c '^\[\[drift\]\]' ci/test-body-diff.toml` prints), and in the R22 sentence Task 3.1 added, the end

```
  `a_request_parts_timeout_outlasts_the_client_s`.
```

becomes

```
  `a_request_parts_timeout_outlasts_the_client_s`; binance's `usdm::error` `tests::response` and
  `failure_tags_usdm`'s `raw`.
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test -p polyoxide-binance --all-features
nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings
python3 scripts/test_body_diff.py --baseline v0.40.0
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-binance/*.rs'
```

Expected: the format check is silent; the binance mock, header-pin and unit suites pass (the `live_*` suites are `#[ignore]`d and stay unrun); clippy is clean, with `examples/weight_probe.rs` built; `test_body_diff.py` exits 0 and prints two more `Acknowledged as DRIFT R22` lines; the sweep prints nothing.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(binance): name the HTTP vocabulary through polyoxide_core::http (wasm story 3.2)" -- polyoxide-binance/examples/weight_probe.rs polyoxide-binance/src/usdm/error.rs polyoxide-binance/src/usdm/policy.rs polyoxide-binance/src/usdm/weight.rs polyoxide-binance/tests/failure_tags_usdm.rs ci/test-body-diff.toml .claude/rules/ci-gates.md
```

### Task 3.3: `polyoxide-kalshi` names the vocabulary through core's `http`

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/auth.rs:19`
- Modify: `polyoxide-kalshi/src/predictions/error.rs:154-155`
- Modify: `polyoxide-kalshi/src/predictions/request.rs:486`
- Modify: `polyoxide-kalshi/src/predictions/throttle.rs:683`
- Test: the existing `polyoxide-kalshi` suites

- [ ] **Step 1: Write the failing check**

```bash
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-kalshi/*.rs'
```

- [ ] **Step 2: Run it to see it fail**

Expected: 5 lines: `src/predictions/auth.rs:19`, `src/predictions/error.rs:154` and `155`, `src/predictions/request.rs:486` and `src/predictions/throttle.rs:683`.

- [ ] **Step 3: Implement**

```bash
git grep -lE 'polyoxide_core::reqwest::(Method|StatusCode|header)\b' -- 'polyoxide-kalshi/*.rs' \
  | xargs sed -i -E 's#polyoxide_core::reqwest::(Method|StatusCode|header)\b#polyoxide_core::http::\1#g'
nix develop --command cargo fmt --all
```

The lines it changes, with their current text. Four are `use` lines inside a `#[cfg(test)]` module, so no test body changes and the script reports no new diff:

| File:line | Current | New |
|---|---|---|
| `src/predictions/auth.rs:19` | `use polyoxide_core::reqwest::header::{HeaderName, HeaderValue};` | `use polyoxide_core::http::header::{HeaderName, HeaderValue};` |
| `src/predictions/error.rs:154` | `    use polyoxide_core::reqwest::header::{HeaderMap, RETRY_AFTER};` | `    use polyoxide_core::http::header::{HeaderMap, RETRY_AFTER};` |
| `src/predictions/error.rs:155` | `    use polyoxide_core::reqwest::StatusCode;` | `    use polyoxide_core::http::StatusCode;` |
| `src/predictions/request.rs:486` | `    use polyoxide_core::reqwest::header::{HeaderName, HeaderValue};` | `    use polyoxide_core::http::header::{HeaderName, HeaderValue};` |
| `src/predictions/throttle.rs:683` | `    use polyoxide_core::reqwest::Method;` | `    use polyoxide_core::http::Method;` |

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test -p polyoxide-kalshi --all-features
nix develop --command cargo clippy -p polyoxide-kalshi --all-targets --all-features -- -D warnings
python3 scripts/test_body_diff.py --baseline v0.40.0
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-kalshi/*.rs'
```

Expected: the format check is silent; the kalshi suites pass; clippy is clean; `test_body_diff.py` exits 0 with no new acknowledgement; the sweep prints nothing.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(kalshi): name the HTTP vocabulary through polyoxide_core::http (wasm story 3.3)" -- polyoxide-kalshi/src/predictions/auth.rs polyoxide-kalshi/src/predictions/error.rs polyoxide-kalshi/src/predictions/request.rs polyoxide-kalshi/src/predictions/throttle.rs
```

### Task 3.4: `polyoxide-polymarket` names the vocabulary through core's `http`

**Files:**
- Modify: `polyoxide-polymarket/examples/v2_soak/main.rs:56`
- Modify: `polyoxide-polymarket/src/client.rs:187-188`
- Modify: `polyoxide-polymarket/src/clob/api/account.rs:7`, `auth.rs:3`, `markets.rs:3`, `notifications.rs:3`, `orders.rs:5` (all under `polyoxide-polymarket/src/clob/api/`)
- Modify: `polyoxide-polymarket/src/clob/authenticator.rs:11,161`
- Modify: `polyoxide-polymarket/src/clob/client.rs:4`
- Modify: `polyoxide-polymarket/src/clob/error.rs:284,668`
- Modify: `polyoxide-polymarket/src/data/error.rs:93`
- Modify: `polyoxide-polymarket/src/data/v2/error.rs:135,138,143`
- Modify: `polyoxide-polymarket/src/gamma/api/markets.rs:3-4`
- Modify: `polyoxide-polymarket/src/gamma/error.rs:46`
- Modify: `polyoxide-polymarket/src/perps/api/public.rs:6`
- Modify: `polyoxide-polymarket/src/perps/error.rs:149,152,157`
- Modify: `polyoxide-polymarket/src/relay/client.rs:21,1382,1387,1390,1789,1808,1868,1881-1882`
- Modify: `polyoxide-polymarket/src/relay/config.rs:4`
- Modify: `polyoxide-polymarket/src/relay/error.rs:70`
- Modify: `polyoxide-polymarket/src/shared/limits.rs:12,274`
- Modify: `polyoxide-polymarket/src/shared/policy.rs:11,72,88,181,183`
- Modify: `polyoxide-polymarket/src/shared/signer_limit.rs:19,213,388,463,594`
- Modify: `polyoxide-polymarket/tests/polymarket_limits.rs:10`
- Modify: `polyoxide-polymarket/tests/polymarket_throttle.rs:8-9`
- Modify: `polyoxide-polymarket/tests/send_loop.rs:10,582`
- Modify: `ci/test-body-diff.toml` (append)
- Modify: `.claude/rules/ci-gates.md:188,200-201`
- Test: the existing `polyoxide-polymarket` suites

- [ ] **Step 1: Write the failing check**

```bash
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-polymarket/*.rs'
```

- [ ] **Step 2: Run it to see it fail**

Expected: 52 lines in the files this task lists, every one on the `polyoxide_core::reqwest` path or on the `reqwest` alias that `policy.rs`, `signer_limit.rs` and `send_loop.rs` import.

- [ ] **Step 3: Implement**

(a) The one-shape lines, `polyoxide_core::reqwest::(Method|StatusCode|header)`, move with one substitution. Lines naming `polyoxide_core::reqwest::Response` and `::Client` (`clob/api/orders.rs:313`, `relay/client.rs:293,323,352`, `relay/error.rs:132`) do not match it and stay for story 4:

```bash
git grep -lE 'polyoxide_core::reqwest::(Method|StatusCode|header)\b' -- 'polyoxide-polymarket/*.rs' \
  | xargs sed -i -E 's#polyoxide_core::reqwest::(Method|StatusCode|header)\b#polyoxide_core::http::\1#g'
```

(b) The forms the substitution cannot reach, edited by hand. In `src/perps/api/public.rs` (line 6)

```rust
use polyoxide_core::{reqwest::Method, ApiError, HttpClient, QueryBuilder, Request};
```

becomes

```rust
use polyoxide_core::{http::Method, ApiError, HttpClient, QueryBuilder, Request};
```

In `tests/polymarket_throttle.rs`, line 9

```rust
use polyoxide_core::reqwest::{Method, StatusCode};
```

becomes

```rust
use polyoxide_core::http::{Method, StatusCode};
```

In `tests/send_loop.rs`, line 10

```rust
use polyoxide_core::reqwest::{self, Method, StatusCode};
```

becomes

```rust
use polyoxide_core::http::{Method, StatusCode};
```

and line 582

```rust
    let headers = reqwest::header::HeaderMap::new();
```

becomes

```rust
    let headers = polyoxide_core::http::header::HeaderMap::new();
```

(this file has local variables named `http`, so the full path avoids reading as one). In `src/shared/policy.rs`, the test module's alias (line 72) `    use polyoxide_core::reqwest;` becomes `    use polyoxide_core::http;`, and the three bare paths

| Line | Current | New |
|---|---|---|
| 88 | `        let headers = reqwest::header::HeaderMap::new();` | `        let headers = http::header::HeaderMap::new();` |
| 181 | `        let headers = reqwest::header::HeaderMap::new();` | `        let headers = http::header::HeaderMap::new();` |
| 183 | `            status: reqwest::StatusCode::TOO_MANY_REQUESTS,` | `            status: http::StatusCode::TOO_MANY_REQUESTS,` |

In `src/shared/signer_limit.rs`, the module-level alias and its users

| Line | Current | New |
|---|---|---|
| 19 | `use polyoxide_core::reqwest;` | `use polyoxide_core::http;` |
| 213 | `    pub fn from_headers(headers: &reqwest::header::HeaderMap) -> Self {` | `    pub fn from_headers(headers: &http::header::HeaderMap) -> Self {` |
| 388 | `    pub fn observe(&self, headers: &reqwest::header::HeaderMap) {` | `    pub fn observe(&self, headers: &http::header::HeaderMap) {` |
| 463 | `    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};` | `    use http::header::{HeaderMap, HeaderName, HeaderValue};` |
| 594 | `    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};` | `    use http::header::{HeaderMap, HeaderName, HeaderValue};` |

(the two test modules reach `http` through `use super::*`, as they reached `reqwest`). The comment at line 625 that says reqwest lowercases header names stays: it describes reqwest.

Every edit keeps the line count of `src/clob/error.rs`, `src/shared/signer_limit.rs`, `tests/send_loop.rs`, `tests/polymarket_limits.rs` and `src/shared/limits.rs`, so the line citations in `docs/MUTANTS.md` hold; Step 4 proves it.

(c) Format, and find the diffs:

```bash
nix develop --command cargo fmt --all
python3 scripts/test_body_diff.py --baseline v0.40.0 2>&1 | grep '::error::'
```

Expected: one `::error::` line, with its sha256, for each of these ten items, all helpers or tests whose bodies name a moved path:

- `polyoxide-polymarket/src/clob/error.rs`: `tests::response` and `tests::raw`;
- `polyoxide-polymarket/src/data/error.rs`: `tests::response`;
- `polyoxide-polymarket/src/data/v2/error.rs`: `tests::parts`;
- `polyoxide-polymarket/src/gamma/error.rs`: `tests::response`;
- `polyoxide-polymarket/src/perps/error.rs`: `tests::response`;
- `polyoxide-polymarket/src/relay/error.rs`: `tests::response`;
- `polyoxide-polymarket/src/shared/policy.rs`: `tests::retries` and `tests::retries_a_429`;
- `polyoxide-polymarket/tests/send_loop.rs`: `a_429_with_no_retry_left_is_fail_with_its_hold`.

Append a table for each to `ci/test-body-diff.toml`, with the sha256 its line printed, in this form:

```toml
[[drift]]
file = "polyoxide-polymarket/src/clob/error.rs"
item = "tests::response"
row = "R22"
sha256 = "<the 64 hex digits its ::error:: line printed>"
```

Then `.claude/rules/ci-gates.md` follows (AD-21). In its line 188, the count in "The 33 `[[drift]]` entries" becomes 43 (or the number `grep -c '^\[\[drift\]\]' ci/test-body-diff.toml` prints), and in the R22 sentence the end

```
  `failure_tags_usdm`'s `raw`.
```

becomes

```
  `failure_tags_usdm`'s `raw`; polymarket's `clob::error` `tests::response` and `tests::raw`,
  `data::error`'s, `gamma::error`'s, `perps::error`'s and `relay::error`'s `tests::response`,
  `data::v2::error`'s `tests::parts`, `shared::policy`'s `tests::retries` and
  `tests::retries_a_429`, and `send_loop`'s `a_429_with_no_retry_left_is_fail_with_its_hold`.
```

- [ ] **Step 4
: Run it to see it pass**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test -p polyoxide-polymarket --all-features
nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings
python3 scripts/test_body_diff.py --baseline v0.40.0
bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- 'polyoxide-polymarket/*.rs'
```

Expected: the format check is silent; the polymarket suites pass, `send_loop` and `polymarket_throttle` among them; clippy is clean, with `examples/v2_soak` built; `test_body_diff.py` exits 0 and prints ten more `Acknowledged as DRIFT R22` lines; the ledger test passes, so no cited line moved; the sweep prints nothing.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(polymarket): name the HTTP vocabulary through polyoxide_core::http (wasm story 3.4)" -- polyoxide-polymarket/examples/v2_soak/main.rs polyoxide-polymarket/src/client.rs polyoxide-polymarket/src/clob polyoxide-polymarket/src/data polyoxide-polymarket/src/gamma polyoxide-polymarket/src/perps polyoxide-polymarket/src/relay polyoxide-polymarket/src/shared polyoxide-polymarket/tests/polymarket_limits.rs polyoxide-polymarket/tests/polymarket_throttle.rs polyoxide-polymarket/tests/send_loop.rs ci/test-body-diff.toml .claude/rules/ci-gates.md
```

### Task 3.5: `polyoxide-test-support`, and the sweep of the workspace

**Files:**
- Modify: `polyoxide-test-support/tests/failure_tags.rs:14`
- Modify: `polyoxide-test-support/tests/token_cost_throttle.rs:14`
- Test: the existing `polyoxide-test-support` suites, and the story's gates

- [ ] **Step 1: Write the failing check**

```bash
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- '*.rs'
```

- [ ] **Step 2: Run it to see it fail**

Expected: 2 lines, the whole of what is left in the workspace: `polyoxide-test-support/tests/failure_tags.rs:14` (`use polyoxide_core::reqwest::StatusCode;`) and `polyoxide-test-support/tests/token_cost_throttle.rs:14` (`use polyoxide_core::reqwest::{Method, StatusCode};`).

- [ ] **Step 3: Implement**

In `polyoxide-test-support/tests/failure_tags.rs`, line 14

```rust
use polyoxide_core::reqwest::StatusCode;
```

becomes

```rust
use polyoxide_core::http::StatusCode;
```

In `polyoxide-test-support/tests/token_cost_throttle.rs`, line 14

```rust
use polyoxide_core::reqwest::{Method, StatusCode};
```

becomes

```rust
use polyoxide_core::http::{Method, StatusCode};
```

`failure_tags.rs` keeps `polyoxide_core::reqwest::Client` and `polyoxide_core::reqwest::get` (the local sockets that make real transport errors) and `src/query.rs` keeps `reqwest::get` and `reqwest::Client`: they are story 4's. Both edits are `use` lines, so no test body changes and the script reports no new diff.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo test -p polyoxide-test-support --all-features
nix develop --command cargo clippy -p polyoxide-test-support --all-targets --all-features -- -D warnings
git grep -nE 'reqwest::(\{[^}]*)?\b(Method|StatusCode|HeaderMap|HeaderValue|header)\b|use polyoxide_core::reqwest;' -- '*.rs'
```

Expected: the format check is silent; test-support's suites pass; clippy is clean; the sweep over the whole workspace prints nothing (exit 1).

Then the story's gates, in order:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop --command cargo test --workspace --all-features
bash -c 'RUSTDOCFLAGS="-D warnings" nix develop --command cargo doc --no-deps --workspace --all-features'
python3 scripts/gen_registry.py --check
python3 scripts/venue_gate.py
python3 scripts/live_unwraps.py
python3 scripts/test_body_diff.py --baseline v0.40.0
bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py tests/test_mutants_ledger.py -q'
```

Expected: every command exits 0; `test_body_diff.py` prints sixteen `Acknowledged as DRIFT R22` lines, fewer by each listed item `v0.40.0` did not hold; the dependency fences hold with `http` declared by core alone and the allowlists unchanged.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(test-support): name the HTTP vocabulary through polyoxide_core::http (wasm story 3.5)" -- polyoxide-test-support/tests/failure_tags.rs polyoxide-test-support/tests/token_cost_throttle.rs
```

## Story 4: `HttpTransport`, `Response` and `TransportError` (spec D4, D5)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/response.rs`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/tests/health.rs`, `polyoxide-core/tests/mock_request.rs`, `polyoxide-core/tests/transport_injection.rs`, `polyoxide-binance/examples/weight_probe.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-polymarket/examples/cf_burst_probe.rs`, `polyoxide-polymarket/examples/gamma_batch_ceiling.rs`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/gamma/api/markets.rs`, `polyoxide-polymarket/src/perps/api/health.rs`, `polyoxide-polymarket/src/perps/error.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/error.rs`, `polyoxide-polymarket/tests/mock_clob.rs`, `polyoxide-polymarket/tests/mock_relay.rs`, `polyoxide-polymarket/tests/send_loop.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/tests/failure_tags.rs`, `polyoxide-core/README.md`, `polyoxide-polymarket/src/clob/client.rs`, `docs/s3-removals.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `ci/test-body-diff.toml`, `.claude/rules/send-loop.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`, `.claude/rules/live-tests.md`, `.claude/rules/ci-gates.md`, `Cargo.lock`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 4 → Stories 7, 9, 10"; uses Story 1's `BoxFuture`, Story 2's `HttpClient::runtime()` and `timeout`, and Story 3's `polyoxide_core::http` and row `R22`
- **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test -p polyoxide-polymarket --no-default-features --features clob --test headers_clob && cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_claude_rules.py -q'`
- **Review:** deep (the send loop's seam, public error types and a breaking API change; landmine notes on gzip and foreign-error `From` impls apply)

Core's send loop stops owning a reqwest client. A new `HttpTransport` takes one built `HttpRequest` (method, full URL with the form-urlencoded query, signed headers, body, and a timeout that is always set) and answers with core's own `Response`, an owned head and a lazy body, or with a `TransportError` whose `TransportKind` says where the exchange broke. `HttpClient` holds a private `Arc<dyn HttpTransport>`, built from the config's timeout, pool size and `gzip` as `ReqwestTransport` (native), or injected through `ClientConfig::http_transport`, the `client_config_setters!` setter and `HttpClientBuilder::http_transport`. `ApiError::Network` and `classify_reqwest` go; `ApiError::Transport` replaces them, and `ReqwestTransport` maps a reqwest error through one private function, by the rule `classify_reqwest` held (a body that broke off mid-read stays `Network`). The loop still decides on the head, releases the permit, then reads an error body. Story 4 re-types every public item spec D11 lists for HTTP (`HttpClient::send`, `Request::send_raw`, clob's `ListOrders::send_raw`, `Pong::response`, `ErrorResponse::read`, `ApiError::from_response`, `retry_after_header`) and moves every caller with them in one commit: gamma's `post_json`, relay's helper and its ten body reads, clob's `send_raw`s, every ping's body check, the error decodes' tests, the live suites' raw reads. The code holds two landmine markers, so the supervisor should dispatch its developer with `effort: xhigh`: `ReqwestTransport::from_settings` carries the one existing `.gzip(` call over from `HttpClientBuilder::build` unchanged (called only when the builder's `gzip` is set, so an unset client keeps reqwest's default and asks for gzip), and the private `map_error` stands where a foreign-error `From` impl would be: none is written. Task 4.1 adds the new types beside the old ones, with their own tests; Task 4.2 switches the client and every caller in one commit, with the removals lines, the drift entries and the rules text that commit needs, and proves the injection with a recording transport that drives one retry and one hold through the loop.

### Task 4.1: The transport types, beside what exists

**Files:**
- Create: `polyoxide-core/src/transport.rs`
- Create: `polyoxide-core/src/response.rs`
- Modify: `polyoxide-core/Cargo.toml:17-34`
- Modify: `polyoxide-core/src/lib.rs:1-125`
- Modify: `polyoxide-core/src/error.rs:1-225`
- Modify: `Cargo.lock`
- Test: `polyoxide-core/src/transport.rs`, `polyoxide-core/src/response.rs`

- [ ] **Step 1: Write the failing tests**

Edit 1, `polyoxide-core/Cargo.toml`: core declares `bytes` itself (reqwest already builds it, so no crate joins the tree). Skip this edit if story 3 already declared it (`grep -n '^bytes' polyoxide-core/Cargo.toml`). Current (line 28):

```toml
url = { workspace = true }
```

Replace with:

```toml
url = { workspace = true }
# The body type of `HttpRequest` and `Response`; reqwest builds it already.
bytes = "1"
```

Edit 2, `polyoxide-core/src/lib.rs`: declare the two modules. Current (lines 56-57):

```rust
pub mod request;
pub mod send;
```

Replace with:

```rust
pub mod request;
pub mod response;
pub mod send;
pub mod transport;
```

Create `polyoxide-core/src/response.rs` with its tests only, which the implementation in Step 3 is put above:

```rust
#[cfg(test)]
mod tests {
    use http::HeaderValue;
    use polyoxide_venue::{Class, Classify};

    use super::*;
    use crate::transport::TransportKind;

    fn url() -> Url {
        Url::parse("https://api.example.test/v1/rows").unwrap()
    }

    /// A 200 carrying `body`, with one header.
    fn answering(body: &'static [u8]) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert("x-served-by", HeaderValue::from_static("test"));
        Response::new(
            StatusCode::OK,
            headers,
            url(),
            Box::pin(async move { Ok(Bytes::from_static(body)) }),
        )
    }

    /// A 200 whose body never arrives.
    fn cut_off() -> Response {
        Response::new(
            StatusCode::OK,
            HeaderMap::new(),
            url(),
            Box::pin(async { Err::<Bytes, _>(TransportError::new(TransportKind::Body, "cut off")) }),
        )
    }

    #[test]
    fn the_head_reads_without_the_body() {
        let response = answering(b"{}");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.url(), &url());
        assert_eq!(
            response
                .headers()
                .get("x-served-by")
                .and_then(|value| value.to_str().ok()),
            Some("test")
        );
    }

    #[tokio::test]
    async fn bytes_text_and_json_read_the_same_body() {
        const BODY: &[u8] = br#"{"rows":[1,2]}"#;
        assert_eq!(
            answering(BODY).bytes().await.unwrap(),
            Bytes::from_static(BODY)
        );
        assert_eq!(answering(BODY).text().await.unwrap(), r#"{"rows":[1,2]}"#);
        assert_eq!(
            answering(BODY).json::<serde_json::Value>().await.unwrap(),
            serde_json::json!({"rows": [1, 2]})
        );
    }

    #[tokio::test]
    async fn text_replaces_invalid_utf8() {
        assert_eq!(answering(b"h\xFFi").text().await.unwrap(), "h\u{FFFD}i");
    }

    #[tokio::test]
    async fn a_body_that_is_not_json_is_a_serialization_error_of_the_decode_class() {
        let err = answering(b"not json")
            .json::<serde_json::Value>()
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Serialization(_)), "{err:?}");
        assert_eq!(err.class(), Class::Decode);
    }

    #[tokio::test]
    async fn a_body_that_fails_to_arrive_is_the_transport_error_whichever_way_it_is_read() {
        for err in [
            cut_off().bytes().await.unwrap_err(),
            cut_off().text().await.unwrap_err(),
            cut_off().json::<serde_json::Value>().await.unwrap_err(),
        ] {
            assert!(
                matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Body),
                "{err:?}"
            );
            assert_eq!(err.class(), Class::Network);
        }
    }

    #[test]
    fn debug_names_the_headers_and_never_their_values() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer s3cr3t"));
        let response = Response::new(
            StatusCode::OK,
            headers,
            url(),
            Box::pin(async { Ok(Bytes::new()) }),
        );
        let debug = format!("{response:?}");
        assert!(debug.contains("authorization"), "{debug}");
        assert!(!debug.contains("s3cr3t"), "{debug}");
        assert!(debug.contains("200"), "{debug}");
    }
}
```

Create `polyoxide-core/src/transport.rs` with its tests only, which the implementation in Step 3 is put above:

```rust
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use http::{HeaderValue, StatusCode};

    use super::*;
    use crate::error::ApiError;

    #[test]
    fn a_kind_displays_in_lower_case() {
        for (kind, text) in [
            (TransportKind::Connect, "connect"),
            (TransportKind::Timeout, "timeout"),
            (TransportKind::Body, "body"),
            (TransportKind::Request, "request"),
            (TransportKind::Other, "other"),
        ] {
            assert_eq!(kind.to_string(), text);
        }
    }

    #[test]
    fn an_error_displays_its_kind_then_its_source() {
        let err = TransportError::new(TransportKind::Timeout, "no answer");
        assert_eq!(err.kind(), TransportKind::Timeout);
        assert_eq!(err.to_string(), "timeout: no answer");
        assert_eq!(
            std::error::Error::source(&err).map(ToString::to_string),
            Some("no answer".to_owned())
        );
        assert_eq!(
            ApiError::from(err).to_string(),
            "Transport error: timeout: no answer"
        );
    }

    #[test]
    fn only_a_request_that_could_not_be_built_is_not_a_network_failure() {
        for kind in [
            TransportKind::Connect,
            TransportKind::Timeout,
            TransportKind::Body,
            TransportKind::Other,
        ] {
            let err = TransportError::new(kind, "x");
            assert_eq!(err.class(), Class::Network, "{kind}");
            assert!(err.is_retriable(), "{kind}");
            assert!(err.is_fault(), "{kind}");
        }
        let err = TransportError::new(TransportKind::Request, "x");
        assert_eq!(err.class(), Class::InvalidRequest);
        assert!(!err.is_retriable());
    }

    fn request(url: &str) -> HttpRequest {
        HttpRequest {
            method: Method::GET,
            url: Url::parse(url).unwrap(),
            headers: HeaderMap::new(),
            body: None,
            timeout: Duration::from_secs(5),
        }
    }

    /// A transport on a client with no proxy, so a proxy set in the
    /// environment cannot answer for the local addresses below.
    fn transport() -> ReqwestTransport {
        ReqwestTransport::new(reqwest::Client::builder().no_proxy().build().unwrap())
    }

    #[tokio::test]
    async fn the_head_comes_back_whole_and_the_body_is_read_on_demand() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/v1/rows")
            .match_query(mockito::Matcher::UrlEncoded("limit".into(), "2".into()))
            .match_header("x-trace", "t-1")
            .with_status(201)
            .with_header("x-served-by", "mock")
            .with_body("rows")
            .create_async()
            .await;
        let mut sent = request(&format!("{}/v1/rows?limit=2", server.url()));
        sent.headers
            .insert("x-trace", HeaderValue::from_static("t-1"));

        let response = transport().send(sent.clone()).await.unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            response
                .headers()
                .get("x-served-by")
                .and_then(|value| value.to_str().ok()),
            Some("mock")
        );
        assert_eq!(response.url(), &sent.url);
        assert_eq!(response.text().await.unwrap(), "rows");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn the_method_and_the_body_are_sent() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/v1/order")
            .match_body(r#"{"size":1}"#)
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;
        let mut sent = request(&format!("{}/v1/order", server.url()));
        sent.method = Method::POST;
        sent.body = Some(Bytes::from_static(br#"{"size":1}"#));

        let response = transport().send(sent).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn an_error_status_is_a_response_and_never_a_transport_error() {
        let mut server = mockito::Server::new_async().await;
        let _down = server
            .mock("GET", "/v1/rows")
            .with_status(503)
            .with_body("down")
            .create_async()
            .await;

        let response = transport()
            .send(request(&format!("{}/v1/rows", server.url())))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.text().await.unwrap(), "down");
    }

    #[tokio::test]
    async fn a_refused_connection_is_a_connect_failure() {
        // Bind a port, then free it, so nothing is listening there.
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let err = transport()
            .send(request(&format!("http://127.0.0.1:{port}/")))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), TransportKind::Connect, "{err:?}");
        assert_eq!(err.class(), Class::Network);
    }

    #[tokio::test]
    async fn a_host_that_never_answers_is_a_timeout() {
        // Accepted by the kernel, never answered.
        let silent = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut sent = request(&format!("http://{}/", silent.local_addr().unwrap()));
        sent.timeout = Duration::from_millis(200);

        let err = transport().send(sent).await.unwrap_err();

        assert_eq!(err.kind(), TransportKind::Timeout, "{err:?}");
        assert_eq!(err.class(), Class::Network);
    }

    /// A server that answers with a head promising 100 bytes and six of them,
    /// then holds the connection for `hold` before closing it.
    fn short_body_server(hold: Duration) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.read(&mut [0; 1024]);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"a\":");
            std::thread::sleep(hold);
        });
        (url, server)
    }

    #[tokio::test]
    async fn a_body_that_stops_short_is_a_body_failure() {
        let (url, server) = short_body_server(Duration::ZERO);

        let response = transport().send(request(&url)).await.unwrap();
        let err = response.bytes().await.unwrap_err();
        server.join().unwrap();

        assert!(
            matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Body),
            "{err:?}"
        );
        assert_eq!(err.class(), Class::Network);
    }

    #[tokio::test]
    async fn the_timeout_bounds_the_body_as_well_as_the_head() {
        let (url, server) = short_body_server(Duration::from_millis(1_500));
        let mut sent = request(&url);
        sent.timeout = Duration::from_millis(300);

        // The head arrives at once, so the send succeeds; the body then
        // stalls past the timeout.
        let response = transport().send(sent).await.unwrap();
        let err = response.bytes().await.unwrap_err();
        server.join().unwrap();

        assert!(
            matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Timeout),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_scheme_the_client_refuses_is_a_request_failure() {
        let err = transport()
            .send(request("ftp://127.0.0.1/file"))
            .await
            .unwrap_err();

        assert_eq!(err.kind(), TransportKind::Request, "{err:?}");
        assert_eq!(err.class(), Class::InvalidRequest);
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-core --all-features --lib transport response`

Expected: the crate's tests do not compile. The errors include `error[E0432]: unresolved import `crate::transport::TransportKind``, `error[E0433]: failed to resolve: use of undeclared type `TransportKind`` and `error[E0412]: cannot find type `Response` in this scope`, from the two new files' tests.

- [ ] **Step 3: Implement**

Edit 1, `polyoxide-core/src/response.rs`: put this above `#[cfg(test)]`.

```rust
//! The response an [`HttpTransport`](crate::HttpTransport) hands the send
//! loop: an owned head and a lazy body.

use std::fmt;

use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use polyoxide_rt::BoxFuture;
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::ApiError;
use crate::transport::TransportError;

/// A response, as a transport answers one: the status, headers and URL are
/// here, and the body is read on demand.
///
/// The send loop decides on the head, releases the attempt's permit, and only
/// then reads an error's body, so a slow body holds no slot. Reading consumes
/// the response: [`bytes`](Self::bytes), [`text`](Self::text) and
/// [`json`](Self::json) each return an [`ApiError`], a transport failure
/// ([`ApiError::Transport`]) or, for the last, a body that does not parse
/// ([`ApiError::Serialization`]).
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    url: Url,
    body: BoxFuture<Result<Bytes, TransportError>>,
}

impl Response {
    /// A response with its head and a future that reads its body.
    ///
    /// `url` is the URL the response answers, after any redirect the
    /// transport followed.
    pub fn new(
        status: StatusCode,
        headers: HeaderMap,
        url: Url,
        body: BoxFuture<Result<Bytes, TransportError>>,
    ) -> Self {
        Self {
            status,
            headers,
            url,
            body,
        }
    }

    /// The status.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The URL the response answers.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Read the body whole.
    ///
    /// # Errors
    ///
    /// [`ApiError::Transport`] when the body could not be read.
    pub async fn bytes(self) -> Result<Bytes, ApiError> {
        self.body.await.map_err(ApiError::from)
    }

    /// Read the body as text: UTF-8, with each invalid sequence replaced by
    /// `U+FFFD`.
    ///
    /// # Errors
    ///
    /// [`ApiError::Transport`] when the body could not be read.
    pub async fn text(self) -> Result<String, ApiError> {
        let bytes = self.bytes().await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Read the body and parse it as JSON.
    ///
    /// # Errors
    ///
    /// [`ApiError::Transport`] when the body could not be read, and
    /// [`ApiError::Serialization`] when it does not parse as `T`.
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, ApiError> {
        let bytes = self.bytes().await?;
        serde_json::from_slice(&bytes).map_err(ApiError::from)
    }
}

/// Written by hand: the body is a future, which prints nothing useful, and a
/// header's value (a token, a cookie) must never reach a log line.
impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header_names: Vec<&str> = self.headers.keys().map(|name| name.as_str()).collect();
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("url", &self.url.as_str())
            .field("headers", &header_names)
            .finish_non_exhaustive()
    }
}
```

Edit 2, `polyoxide-core/src/transport.rs`: put this above `#[cfg(test)]`.

```rust
//! The seam between the send loop and the network.
//!
//! [`HttpTransport`] takes one built [`HttpRequest`] and answers with a
//! [`Response`], or with a [`TransportError`] when no response head arrived.
//! The send loop decides everything else: whether a status is a failure,
//! whether to retry, what to hold. A client sends on the target's default
//! transport unless one is injected.

use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, Method};
use polyoxide_rt::BoxFuture;
use polyoxide_venue::{Class, Classify};
use thiserror::Error;
use url::Url;

use crate::response::Response;

/// One attempt's request, as the send loop builds it.
///
/// Only core builds one: the struct is `non_exhaustive` and has no public
/// constructor, so a transport reads the fields it is handed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HttpRequest {
    /// The HTTP method.
    pub method: Method,
    /// The full URL: the base URL joined with the path, with the query
    /// form-urlencoded onto it.
    pub url: Url,
    /// The headers, signed.
    pub headers: HeaderMap,
    /// The body, sent as given.
    pub body: Option<Bytes>,
    /// How long the whole exchange may take, the body's read included: the
    /// request's own timeout, else the client's. Always set.
    pub timeout: Duration,
}

/// Sends one request and answers with its response.
///
/// The send loop is the only caller. A client holds one transport and shares
/// it with its `with_base_url` siblings.
pub trait HttpTransport: Send + Sync + fmt::Debug + 'static {
    /// Send `request` and answer with its response, whatever its status.
    ///
    /// An implementation:
    ///
    /// - returns a [`Response`] for every status, an error status included:
    ///   judging a status is the send loop's job, never the transport's;
    /// - fails with a [`TransportError`] only when no response head arrived;
    /// - bounds the whole exchange, the body's read included, by
    ///   `request.timeout`;
    /// - leaves the body unread until the [`Response`] is asked for it.
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>>;
}

/// Where a transport's exchange broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportKind {
    /// The connection could not be made.
    Connect,
    /// The timeout ran out.
    Timeout,
    /// The body could not be read to its end.
    Body,
    /// The request could not be built, or sent as built.
    Request,
    /// Anything else.
    Other,
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Connect => "connect",
            Self::Timeout => "timeout",
            Self::Body => "body",
            Self::Request => "request",
            Self::Other => "other",
        })
    }
}

/// Why no response arrived, or why its body could not be read.
///
/// [`kind`](Self::kind) says where the exchange broke, and the boxed source is
/// the transport's own error, kept for its message and its chain. A custom
/// [`HttpTransport`] builds one with [`new`](Self::new). Its class is
/// [`Class::Network`] for every kind but [`TransportKind::Request`], a request
/// that could not be built, which is [`Class::InvalidRequest`].
#[derive(Debug, Error)]
#[non_exhaustive]
#[error("{kind}: {source}")]
pub struct TransportError {
    kind: TransportKind,
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl TransportError {
    /// A transport failure of `kind`, caused by `source`.
    pub fn new(
        kind: TransportKind,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            kind,
            source: source.into(),
        }
    }

    /// Where the exchange broke.
    pub fn kind(&self) -> TransportKind {
        self.kind
    }
}

impl Classify for TransportError {
    fn class(&self) -> Class {
        match self.kind {
            TransportKind::Request => Class::InvalidRequest,
            TransportKind::Connect
            | TransportKind::Timeout
            | TransportKind::Body
            | TransportKind::Other => Class::Network,
        }
    }
}

/// The transport the native targets use: reqwest.
///
/// The client builder makes one from its timeout, pool size and `gzip`; build
/// one with [`new`](Self::new) to send on a client of your own.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// A transport that sends on `client`.
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl HttpTransport for ReqwestTransport {
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        let mut builder = self
            .client
            .request(request.method, request.url)
            .headers(request.headers)
            .timeout(request.timeout);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        Box::pin(async move {
            let response = builder.send().await.map_err(map_error)?;
            let status = response.status();
            let headers = response.headers().clone();
            let url = response.url().clone();
            Ok(Response::new(
                status,
                headers,
                url,
                Box::pin(async move { response.bytes().await.map_err(map_error) }),
            ))
        })
    }
}

/// The transport error for a reqwest one, by the rule core has always used: a
/// timeout, a failed connect, a body that broke off (reqwest labels that one
/// a decode error), or a request that could not be built; anything else is a
/// failure in transit, which is `Network` whatever its kind.
fn map_error(err: reqwest::Error) -> TransportError {
    let kind = if err.is_timeout() {
        TransportKind::Timeout
    } else if err.is_connect() {
        TransportKind::Connect
    } else if err.is_builder() {
        TransportKind::Request
    } else if err.is_body() || err.is_decode() {
        TransportKind::Body
    } else {
        TransportKind::Other
    };
    TransportError::new(kind, err)
}
```

Edit 3, `polyoxide-core/src/error.rs`: the new variant, beside `Network` until Task 4.2 removes it. Current (lines 101-103):

```rust
    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
```

Replace with:

```rust
    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// The transport failed: no response arrived, or its body could not be
    /// read. Its class is its kind's.
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
```

Edit 4, same file: the import. Current (line 8):

```rust
use crate::hooks::Refused;
```

Replace with:

```rust
use crate::hooks::Refused;
use crate::transport::TransportError;
```

Edit 5, same file: the class. Current (line 204):

```rust
            Self::Network(err) => classify_reqwest(err),
```

Replace with:

```rust
            Self::Network(err) => classify_reqwest(err),
            Self::Transport(err) => err.class(),
```

Edit 6, `polyoxide-core/src/lib.rs`: the crate docs. Current (line 8):

```rust
//! - HTTP client configuration
```

Replace with:

```rust
//! - HTTP client configuration
//! - The transport seam, [`HttpTransport`], with the [`Response`] it answers
//!   with and the [`TransportError`] it fails with, so the send loop names no
//!   HTTP library
```

Edit 7, same file: the re-exports. Current (lines 106-107):

```rust
pub use request::{QueryBuilder, Request, RequestError};
pub use send::decode_json;
```

Replace with:

```rust
pub use request::{QueryBuilder, Request, RequestError};
pub use response::Response;
pub use send::decode_json;
pub use transport::ReqwestTransport;
pub use transport::{HttpRequest, HttpTransport, TransportError, TransportKind};
```

Edit 8, same file: the `Classify` assertion. Current (lines 117-118):

```rust
    is::<ApiError>();
    is::<Refused>();
```

Replace with:

```rust
    is::<ApiError>();
    is::<Refused>();
    is::<TransportError>();
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`

Run: `nix develop --command cargo test -p polyoxide-core --all-features`

Expected: every suite reports `test result: ok`; the lib suite includes the six new `response::tests` and eleven new `transport::tests`, among them `the_timeout_bounds_the_body_as_well_as_the_head` and `a_body_that_stops_short_is_a_body_failure`.

Run: `nix develop --command cargo clippy -p polyoxide-core --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-core --all-features'`

Expected: no warnings.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_dep_allowlists.py -q'`

Expected: `git status --short ci/dep-allowlists` prints nothing (`bytes` and `http` are already in core's tree through reqwest, so no crate joins any list), and the three suites pass, `TransportError` being listed in `lib.rs`.

Run: `nix develop --command python3 scripts/venue_gate.py`

Expected: exit 0: the new files name no venue.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-core/src/transport.rs polyoxide-core/src/response.rs
git commit -m "feat(core): HttpTransport, Response and TransportError beside the reqwest paths (wasm plan, story 4)" -- polyoxide-core/Cargo.toml polyoxide-core/src/lib.rs polyoxide-core/src/error.rs polyoxide-core/src/transport.rs polyoxide-core/src/response.rs Cargo.lock ci/dep-allowlists
```

### Task 4.2: The client sends on the transport, and every caller follows

**Files:**
- Create: `polyoxide-core/tests/transport_injection.rs`
- Modify: `polyoxide-core/src/transport.rs`
- Modify: `polyoxide-core/src/client.rs:1-290`
- Modify: `polyoxide-core/src/config.rs:1-201`
- Modify: `polyoxide-core/src/macros.rs:1-62`
- Modify: `polyoxide-core/src/send.rs:1-160`
- Modify: `polyoxide-core/src/request.rs:1-180`
- Modify: `polyoxide-core/src/health.rs:1-15`
- Modify: `polyoxide-core/src/error.rs:60-600`
- Modify: `polyoxide-core/README.md:1-60`
- Modify: `polyoxide-polymarket/src/clob/client.rs:944-1020`
- Modify: `polyoxide-polymarket/src/gamma/api/markets.rs:160-175`
- Modify: `polyoxide-polymarket/src/relay/client.rs:285-1841`
- Modify: `polyoxide-polymarket/src/relay/error.rs:30-145`
- Modify: `polyoxide-polymarket/src/clob/api/orders.rs:305-316`
- Modify: `polyoxide-polymarket/src/clob/api/account.rs:75-90`
- Modify: `polyoxide-polymarket/src/perps/api/health.rs:35-50`
- Modify: `polyoxide-binance/src/usdm/api/health.rs:30-40`
- Modify: `polyoxide-binance/tests/live_usdm.rs:15-210`
- Modify: `polyoxide-kalshi/tests/live_predictions.rs:20-565`
- Modify: `polyoxide-core/tests/health.rs:225-235`
- Modify: `polyoxide-core/tests/mock_request.rs:575-590`
- Modify: `polyoxide-polymarket/tests/send_loop.rs:405-420`
- Modify: `polyoxide-polymarket/tests/mock_relay.rs:2415-2430`
- Modify: `polyoxide-test-support/tests/failure_tags.rs:1-178`
- Modify: `docs/s3-removals.md:30-35`
- Modify: `docs/MUTANTS.md:1-184`
- Modify: `.github/scripts/tests/test_mutants_ledger.py:15-120`
- Modify: `ci/test-body-diff.toml:90-257`
- Modify: `.claude/rules/send-loop.md:90-110`
- Modify: `.claude/rules/errors.md:30-100`
- Modify: `.claude/rules/clients.md:10-25`
- Modify: `.claude/rules/live-tests.md:40-48`
- Modify: `.claude/rules/ci-gates.md:183-205`
- Test: `polyoxide-core/tests/transport_injection.rs`, `polyoxide-core/src/config.rs`, `polyoxide-polymarket/src/clob/client.rs`

- [ ] **Step 1: Write the failing tests**

Create `polyoxide-core/tests/transport_injection.rs`. A recording transport answers every attempt, so nothing is dialled: the host below does not resolve, so a request that reached reqwest could not have been answered.

```rust
//! The send loop on a transport the test injects. A recording transport
//! answers every attempt, so nothing is dialled, and the loop's retry, hold
//! and permit rules run on what it returns.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use polyoxide_core::http::{HeaderMap, HeaderValue, Method, StatusCode};
use polyoxide_core::{
    ApiError, AttemptInfo, Charge, HttpClient, HttpClientBuilder, HttpRequest, HttpTransport,
    Refused, RequestMeta, RequestParts, Response, ResponseMeta, RetryConfig, Throttle,
    TransportError, TransportKind,
};
use polyoxide_rt::BoxFuture;
use polyoxide_venue::{Class, Classify};

/// One scripted answer.
#[derive(Debug)]
struct Reply {
    status: u16,
    headers: Vec<(&'static str, &'static str)>,
    body: &'static str,
}

fn reply(status: u16, headers: &[(&'static str, &'static str)], body: &'static str) -> Reply {
    Reply {
        status,
        headers: headers.to_vec(),
        body,
    }
}

/// Answers each request with the next scripted reply and keeps the requests
/// it saw.
#[derive(Debug, Default)]
struct Recording {
    replies: Mutex<VecDeque<Reply>>,
    seen: Mutex<Vec<HttpRequest>>,
}

impl Recording {
    fn answering(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::default(),
        })
    }

    fn seen(&self) -> Vec<HttpRequest> {
        self.seen.lock().unwrap().clone()
    }
}

impl HttpTransport for Recording {
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        let url = request.url.clone();
        self.seen.lock().unwrap().push(request);
        let reply = self.replies.lock().unwrap().pop_front();
        Box::pin(async move {
            let Some(reply) = reply else {
                return Err(TransportError::new(
                    TransportKind::Other,
                    "no scripted reply left",
                ));
            };
            let mut headers = HeaderMap::new();
            for (name, value) in reply.headers {
                headers.insert(name, HeaderValue::from_static(value));
            }
            let body = Bytes::from_static(reply.body.as_bytes());
            Ok(Response::new(
                StatusCode::from_u16(reply.status).expect("a valid status"),
                headers,
                url,
                Box::pin(async move { Ok(body) }),
            ))
        })
    }
}

/// A transport whose every attempt fails, counting them.
#[derive(Debug, Default)]
struct Failing(Mutex<usize>);

impl HttpTransport for Failing {
    fn send(&self, _request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        *self.0.lock().unwrap() += 1;
        Box::pin(async {
            Err(TransportError::new(
                TransportKind::Timeout,
                "no answer in time",
            ))
        })
    }
}

/// Answers 503 with a body that, when it is read, tries to take the client's
/// only concurrency permit and records whether it could.
#[derive(Default)]
struct Probing {
    client: Arc<OnceLock<HttpClient>>,
    permit_was_free: Arc<Mutex<Option<bool>>>,
}

/// Written by hand: the client holds this transport and this holds the client,
/// so a derived `Debug` would print forever.
impl std::fmt::Debug for Probing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Probing").finish_non_exhaustive()
    }
}

impl HttpTransport for Probing {
    fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
        let client = Arc::clone(&self.client);
        let permit_was_free = Arc::clone(&self.permit_was_free);
        Box::pin(async move {
            Ok(Response::new(
                StatusCode::SERVICE_UNAVAILABLE,
                HeaderMap::new(),
                request.url,
                Box::pin(async move {
                    let http = client
                        .get()
                        .expect("the client is set before it sends")
                        .clone();
                    let runtime = http.runtime().clone();
                    let granted = polyoxide_rt::timeout(
                        &*runtime,
                        Duration::from_millis(200),
                        http.acquire_concurrency(),
                    )
                    .await
                    .is_some();
                    *permit_was_free.lock().unwrap() = Some(granted);
                    Ok(Bytes::from_static(b"down"))
                }),
            ))
        })
    }
}

/// A throttle that charges nothing, counts the responses it observes and
/// keeps each hold it is told of.
#[derive(Clone, Default)]
struct Spy {
    holds: Arc<Mutex<Vec<Duration>>>,
    observed: Arc<AtomicUsize>,
}

impl Throttle for Spy {
    async fn acquire(&self, _meta: &RequestMeta<'_>) -> Result<Charge, Refused> {
        Ok(Charge::none())
    }

    fn observe(&self, _charge: &Charge, _response: &ResponseMeta<'_>, _attempt: &AttemptInfo) {
        self.observed.fetch_add(1, Ordering::SeqCst);
    }

    fn hold(&self, delay: Duration) {
        self.holds.lock().unwrap().push(delay);
    }
}

fn quick(max_retries: u32) -> RetryConfig {
    RetryConfig {
        max_retries,
        initial_backoff_ms: 1,
        max_backoff_ms: 10_000,
    }
}

#[tokio::test]
async fn a_retry_is_sent_on_the_injected_transport_and_nothing_is_dialled() {
    let transport = Recording::answering(vec![reply(429, &[], "slow down"), reply(200, &[], "ok")]);
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .with_retry_config(quick(1))
        .timeout_ms(7_000)
        .build()
        .unwrap();

    let query = [
        ("limit".to_owned(), "2".to_owned()),
        ("q".to_owned(), "x y".to_owned()),
    ];
    let body = http.get_bytes("/v1/rows", &query).await.unwrap();

    assert_eq!(body, b"ok");
    let seen = transport.seen();
    assert_eq!(seen.len(), 2, "the request and its one retry");
    for request in &seen {
        assert_eq!(request.method, Method::GET);
        // The query is form-urlencoded onto the URL, a space as `+`, as
        // reqwest's `.query()` always did.
        assert_eq!(
            request.url.as_str(),
            "https://api.example.test/v1/rows?limit=2&q=x+y"
        );
        assert_eq!(request.timeout, Duration::from_millis(7_000));
        assert_eq!(request.body, None);
    }
}

#[tokio::test]
async fn a_request_carries_its_own_body_headers_and_timeout() {
    let transport = Recording::answering(vec![reply(200, &[], "ok")]);
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .build()
        .unwrap();

    let mut parts = RequestParts::new(Method::POST, "/v1/order");
    parts
        .headers
        .insert("x-signed", HeaderValue::from_static("yes"));
    parts.body = Some(r#"{"size":1}"#.to_owned());
    parts.timeout = Some(Duration::from_secs(300));
    let response = http.send(parts, &[], None).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.url().as_str(), "https://api.example.test/v1/order");
    let seen = transport.seen();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].method, Method::POST);
    assert_eq!(
        seen[0]
            .headers
            .get("x-signed")
            .and_then(|value| value.to_str().ok()),
        Some("yes")
    );
    assert_eq!(seen[0].body, Some(Bytes::from_static(br#"{"size":1}"#)));
    assert_eq!(
        seen[0].timeout,
        Duration::from_secs(300),
        "the request's own timeout replaces the client's 30 s"
    );
}

#[tokio::test]
async fn a_429_on_the_injected_transport_holds_the_throttle_and_reads_its_body() {
    let transport = Recording::answering(vec![reply(
        429,
        &[("retry-after", "2")],
        "slow down",
    )]);
    let spy = Spy::default();
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .with_throttle(spy.clone())
        .with_retry_config(quick(0))
        .build()
        .unwrap();

    let err = http.get_bytes("/v1/rows", &[]).await.unwrap_err();

    // The head the transport sent reached the policy, which held the throttle
    // for the `Retry-After` it carried.
    assert_eq!(*spy.holds.lock().unwrap(), [Duration::from_secs(2)]);
    // The body the transport left lazy was read for the error.
    match err {
        ApiError::Response(response) => {
            assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.body, "slow down");
            assert_eq!(response.retry_after, Some(Duration::from_secs(2)));
        }
        other => panic!("expected the 429 as ApiError::Response, got {other:?}"),
    }
    assert_eq!(transport.seen().len(), 1, "no retry was left");
}

#[tokio::test]
async fn an_error_body_is_read_after_the_permit_is_released() {
    let probing = Arc::new(Probing::default());
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(probing.clone())
        .with_max_concurrent(1)
        .build()
        .unwrap();
    probing
        .client
        .set(http.clone())
        .expect("the client is set once");

    let err = http.get_bytes("/v1/rows", &[]).await.unwrap_err();

    assert!(
        matches!(&err, ApiError::Response(r)
            if r.status == StatusCode::SERVICE_UNAVAILABLE && r.body == "down"),
        "{err:?}"
    );
    assert_eq!(
        *probing.permit_was_free.lock().unwrap(),
        Some(true),
        "the body was read while the attempt still held the only permit"
    );
}

#[tokio::test]
async fn a_transport_failure_is_not_observed_and_not_retried() {
    let transport = Arc::new(Failing::default());
    let spy = Spy::default();
    let http = HttpClientBuilder::new("https://api.example.test")
        .http_transport(transport.clone())
        .with_throttle(spy.clone())
        .with_retry_config(quick(3))
        .build()
        .unwrap();

    let err = http.get_bytes("/v1/rows", &[]).await.unwrap_err();

    assert!(
        matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Timeout),
        "{err:?}"
    );
    assert_eq!(err.class(), Class::Network);
    assert_eq!(
        err.to_string(),
        "Transport error: timeout: no answer in time"
    );
    assert_eq!(*transport.0.lock().unwrap(), 1, "no retry");
    assert_eq!(spy.observed.load(Ordering::SeqCst), 0, "nothing to observe");
    assert!(spy.holds.lock().unwrap().is_empty());
}
```

Append these to `mod tests` of `polyoxide-core/src/config.rs`, after its last test (`the_core_defaults_are_thirty_seconds_and_ten_idle`), inside the module:

```rust
    /// A transport that fails every attempt, so a request that reaches it
    /// proves the setter's value is the one the client sends on.
    #[derive(Debug)]
    struct Refusing;

    impl crate::HttpTransport for Refusing {
        fn send(
            &self,
            _request: crate::HttpRequest,
        ) -> polyoxide_rt::BoxFuture<Result<crate::Response, crate::TransportError>> {
            Box::pin(async {
                Err(crate::TransportError::new(
                    crate::TransportKind::Other,
                    "refused by the test",
                ))
            })
        }
    }

    #[tokio::test]
    async fn an_injected_transport_is_the_one_the_client_sends_on() {
        let knobs = Knobs {
            config: ClientConfig::new("https://example.com", 4),
        }
        .http_transport(std::sync::Arc::new(Refusing));
        assert!(knobs.config.http_transport.is_some());

        let http = knobs.config.http_builder().build().unwrap();
        let err = http.get_bytes("/", &[]).await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Transport error: other: refused by the test"
        );
    }
```

Add these tests to `mod tests` of `polyoxide-polymarket/src/clob/client.rs`, immediately before `    #[test]` and `    fn build_order_v2_sets_builder_and_timestamp() {` (after the runtime test story 2 added). A recording transport that answers every request with a failure sees a Gamma and a Data request made through a `Clob` built with it; without Edit 63a those two clients build their own reqwest transport, so the recorder sees nothing and both tests fail.

```rust
    /// Keeps the URL of each request it is asked to send, and fails it, so
    /// the test needs no response to build.
    #[cfg(any(feature = "gamma", feature = "data"))]
    #[derive(Debug, Default)]
    struct Recorder(std::sync::Mutex<Vec<String>>);

    #[cfg(any(feature = "gamma", feature = "data"))]
    impl polyoxide_core::HttpTransport for Recorder {
        fn send(
            &self,
            request: polyoxide_core::HttpRequest,
        ) -> polyoxide_rt::BoxFuture<
            Result<polyoxide_core::Response, polyoxide_core::TransportError>,
        > {
            self.0.lock().unwrap().push(request.url.to_string());
            Box::pin(async {
                Err(polyoxide_core::TransportError::new(
                    polyoxide_core::TransportKind::Other,
                    "recorded, not sent",
                ))
            })
        }
    }

    #[cfg(feature = "gamma")]
    #[tokio::test]
    async fn the_builders_transport_reaches_the_gamma_client_it_builds() {
        let recorder = std::sync::Arc::new(Recorder::default());
        let clob = ClobBuilder::new()
            .http_transport(recorder.clone())
            .build()
            .unwrap();

        let err = clob
            .gamma
            .http_client
            .get_bytes("/markets", &[])
            .await
            .unwrap_err();

        assert!(
            matches!(err, polyoxide_core::ApiError::Transport(_)),
            "{err:?}"
        );
        let seen = recorder.0.lock().unwrap().clone();
        assert!(!seen.is_empty(), "the Gamma client sent nothing on it");
        assert!(seen.iter().all(|url| url.ends_with("/markets")), "{seen:?}");
    }

    #[cfg(feature = "data")]
    #[tokio::test]
    async fn the_builders_transport_reaches_the_data_client_it_builds() {
        let recorder = std::sync::Arc::new(Recorder::default());
        let clob = ClobBuilder::new()
            .http_transport(recorder.clone())
            .build()
            .unwrap();

        let err = clob
            .data
            .http_client
            .get_bytes("/positions", &[])
            .await
            .unwrap_err();

        assert!(
            matches!(err, polyoxide_core::ApiError::Transport(_)),
            "{err:?}"
        );
        let seen = recorder.0.lock().unwrap().clone();
        assert!(!seen.is_empty(), "the Data client sent nothing on it");
        assert!(
            seen.iter().all(|url| url.ends_with("/positions")),
            "{seen:?}"
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-core --all-features --test transport_injection`

Expected: the suite does not compile: `error[E0599]: no method named `http_transport` found for struct `HttpClientBuilder` in the current scope`, once for each of the five tests that set one.

Run: `nix develop --command cargo test -p polyoxide-core --all-features --lib config`

Expected: the lib's tests do not compile: `error[E0599]: no method named `http_transport` found for struct `Knobs` in the current scope`.

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib clob::client::tests::the_builders_transport`

Expected: it does not compile: ``error[E0599]: no method named `http_transport` found for struct `ClobBuilder` in the current scope``, once for each test. With the setter in (Edit 18) but Edit 63a not yet made, both tests compile and fail on ``the Gamma client sent nothing on it`` and ``the Data client sent nothing on it``, since each client sent on its own reqwest transport.

- [ ] **Step 3: Implement**

The line numbers in the `Current` labels are the base commit's; story 2, story 3 and Task 4.1 have moved them, so find each block by its text.

**3a. Core: the default transport, the client and the config.**

Edit 1, `polyoxide-core/src/transport.rs`: the native default, built from a builder's settings. Current (the end of `impl ReqwestTransport`):

```rust
    /// A transport that sends on `client`.
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}
```

Replace with:

```rust
    /// A transport that sends on `client`.
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// The default native transport, built from a client builder's settings:
    /// its timeout, its idle connections per host and its `gzip` choice
    /// (unset keeps reqwest's default, so every client asks for gzip), with a
    /// ten second connect timeout and no redirects.
    pub(crate) fn from_settings(
        timeout: Duration,
        pool_size: usize,
        gzip: Option<bool>,
    ) -> Result<Self, TransportError> {
        let mut builder = reqwest::Client::builder();
        if let Some(enabled) = gzip {
            builder = builder.gzip(enabled);
        }
        let client = builder
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(pool_size)
            .build()
            .map_err(map_error)?;
        Ok(Self::new(client))
    }
}

/// The default transport, built from a client builder's settings. The browser
/// story gates this and `ReqwestTransport` to the native targets and adds the
/// wasm32 twin, over `fetch`.
pub(crate) fn default_transport(
    timeout: Duration,
    pool_size: usize,
    gzip: Option<bool>,
) -> Result<std::sync::Arc<dyn HttpTransport>, TransportError> {
    Ok(std::sync::Arc::new(ReqwestTransport::from_settings(
        timeout, pool_size, gzip,
    )?))
}
```

Add this test to `mod tests` of the same file, after `a_scheme_the_client_refuses_is_a_request_failure`. It passes before and after the edit above: it pins the redirect policy that moves with the code.

```rust
    #[tokio::test]
    async fn the_default_transport_follows_no_redirect() {
        let mut server = mockito::Server::new_async().await;
        let _moved = server
            .mock("GET", "/moved")
            .with_status(302)
            .with_header("location", "/elsewhere")
            .create_async()
            .await;
        let elsewhere = server
            .mock("GET", "/elsewhere")
            .with_status(200)
            .expect(0)
            .create_async()
            .await;
        let http = crate::HttpClientBuilder::new(server.url())
            .build()
            .unwrap();

        let err = http
            .send(
                crate::RequestParts::new(Method::GET, "/moved"),
                &[],
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(&err, ApiError::Response(r) if r.status == StatusCode::FOUND),
            "{err:?}"
        );
        elsewhere.assert_async().await;
    }
```

Edit 2, `polyoxide-core/src/client.rs`: the import. Current (line 15):

```rust
use crate::rate_limit::{RateLimiter, RetryConfig};
```

Replace with:

```rust
use crate::rate_limit::{RateLimiter, RetryConfig};
use crate::transport::{default_transport, HttpTransport};
```

Edit 3, same file: `retry_after_header` reads a header map. Current (lines 17-25):

```rust
/// Extract the `Retry-After` header value as a string, if present and valid UTF-8.
pub fn retry_after_header(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(RETRY_AFTER)?
        .to_str()
        .ok()
        .map(String::from)
}
```

Replace with:

```rust
/// Extract the `Retry-After` header value as a string, if present and valid UTF-8.
pub fn retry_after_header(headers: &http::HeaderMap) -> Option<String> {
    headers.get(RETRY_AFTER)?.to_str().ok().map(String::from)
}
```

Edit 4, same file: the struct's documentation. Current (lines 34-36):

```rust
/// This is the common structure used by all API clients to hold the
/// configured reqwest client, base URL, and the hooks its send loop,
/// [`send`](Self::send), runs on every request.
```

Replace with:

```rust
/// This is the common structure used by all API clients to hold the
/// transport every attempt is sent on, the base URL, and the hooks its send
/// loop, [`send`](Self::send), runs on every request.
```

Edit 5, same file: the field. Current (lines 39-40):

```rust
    /// The underlying reqwest HTTP client
    pub client: reqwest::Client,
```

Replace with:

```rust
    /// The transport every attempt is sent on, shared with every
    /// [`with_base_url`](Self::with_base_url) sibling.
    pub(crate) transport: Arc<dyn HttpTransport>,
    /// How long an attempt may take unless its request sets its own.
    pub(crate) timeout: Duration,
```

Edit 6, same file: `Debug`. Current (line 54):

```rust
            .field("client", &self.client)
```

Replace with:

```rust
            .field("transport", &self.transport)
```

Edit 7, same file: `with_base_url`'s documentation. Current (line 65):

```rust
    /// The underlying reqwest client (and so its connection pool), throttle
```

Replace with:

```rust
    /// The transport (and so its connection pool), throttle
```

Edit 8, same file: the builder's field. Current (line 173):

```rust
    gzip: Option<bool>,
```

Replace with:

```rust
    gzip: Option<bool>,
    http_transport: Option<Arc<dyn HttpTransport>>,
```

Edit 9, same file: `new`. Current (line 187):

```rust
            gzip: None,
```

Replace with:

```rust
            gzip: None,
            http_transport: None,
```

Edit 10, same file: the setter, before `build`. Current (line 259):

```rust
    /// Build the HTTP client.
```

Replace with:

```rust
    /// Send every attempt on `transport` instead of the target's default,
    /// shared with every [`with_base_url`](HttpClient::with_base_url) sibling.
    ///
    /// The builder's timeout still travels with each request, as its
    /// `HttpRequest::timeout`, but [`pool_size`](Self::pool_size) and
    /// [`gzip`](Self::gzip) feed only the default `ReqwestTransport`: a
    /// transport you inject owns its own connections and compression.
    pub fn http_transport(mut self, transport: Arc<dyn HttpTransport>) -> Self {
        self.http_transport = Some(transport);
        self
    }

    /// Build the HTTP client.
```

Edit 11, same file: `build` makes the transport. Current (lines 260-272):

```rust
    pub fn build(self) -> Result<HttpClient, ApiError> {
        let mut builder = reqwest::Client::builder();
        if let Some(enabled) = self.gzip {
            builder = builder.gzip(enabled);
        }
        let client = builder
            .timeout(Duration::from_millis(self.timeout_ms))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .pool_max_idle_per_host(self.pool_size)
            .build()?;

        let base_url = Url::parse(&self.base_url)?;
```

Replace with:

```rust
    pub fn build(self) -> Result<HttpClient, ApiError> {
        let timeout = Duration::from_millis(self.timeout_ms);
        let transport = match self.http_transport {
            Some(transport) => transport,
            None => default_transport(timeout, self.pool_size, self.gzip)?,
        };

        let base_url = Url::parse(&self.base_url)?;
```

Edit 12, same file: the struct literal. Current (lines 274-276):

```rust
        Ok(HttpClient {
            client,
            base_url,
```

Replace with:

```rust
        Ok(HttpClient {
            transport,
            timeout,
            base_url,
```

Edit 13, `polyoxide-core/src/config.rs`: the field. Current (lines 25-26):

```rust
    /// In-flight requests allowed; `None` keeps the client's default.
    pub max_concurrent: Option<usize>,
```

Replace with:

```rust
    /// In-flight requests allowed; `None` keeps the client's default.
    pub max_concurrent: Option<usize>,
    /// The transport every attempt is sent on; `None` keeps the target's
    /// default, which is built from the timeout, the pool size and the
    /// client's `gzip` (`ReqwestTransport` on the native targets).
    pub http_transport: Option<std::sync::Arc<dyn crate::transport::HttpTransport>>,
```

Edit 14, same file: `new`. Current (line 40):

```rust
            max_concurrent: None,
```

Replace with:

```rust
            max_concurrent: None,
            http_transport: None,
```

Edit 15, same file: `http_builder` carries the transport. Insert this immediately before the function's final `match &self.retry_config {` expression, after the `let builder = ...;` statement that precedes it:

```rust
        let builder = match &self.http_transport {
            Some(transport) => builder.http_transport(std::sync::Arc::clone(transport)),
            None => builder,
        };
```

Edit 16, same file: the documentation of `ClientConfig` says it "generates its five setters with": drop the count (`five`, or `six` where story 2 changed it) before `setters`, so the sentence reads `generates its setters with [`client_config_setters!`](crate::client_config_setters)`.

Edit 17, same file: the timeout test now sees a transport error. Current (lines 114-117):

```rust
        assert!(
            matches!(&err, ApiError::Network(e) if e.is_timeout()),
            "expected a timeout, got {err:?}"
        );
```

Replace with:

```rust
        assert!(
            matches!(&err, ApiError::Transport(e) if e.kind() == crate::TransportKind::Timeout),
            "expected a timeout, got {err:?}"
        );
```

Edit 18, `polyoxide-core/src/macros.rs`: the setter. Current (lines 57-60, the `max_concurrent` setter, the last one before the macro's closing `};`; story 2's `runtime` setter may follow it, and this goes directly after `max_concurrent`'s closing brace):

```rust
        pub fn max_concurrent(mut self, max: usize) -> Self {
            self.$config.max_concurrent = Some(max);
            self
        }
```

Replace with:

```rust
        pub fn max_concurrent(mut self, max: usize) -> Self {
            self.$config.max_concurrent = Some(max);
            self
        }

        /// Send every attempt on `transport` instead of the target's default,
        /// which is built from the timeout, the pool size and `gzip`.
        pub fn http_transport(
            mut self,
            transport: ::std::sync::Arc<dyn $crate::HttpTransport>,
        ) -> Self {
            self.$config.http_transport = Some(transport);
            self
        }
```

Edit 19, same file: the macro's first documentation paragraph (it begins `/// Generates a client builder's` and ends before the first blank `///` line) lists the setters: add `http_transport` to the list, as `base_url`, `timeout_ms`, `pool_size`, `with_retry_config`, `max_concurrent`, `runtime` and `http_transport`, and drop the count word.

**3b. Core: the loop, the request builder, the ping and the errors.**

Edit 20, `polyoxide-core/src/send.rs`: the imports. Current (lines 3-4):

```rust
use reqwest::Response;
use serde::de::DeserializeOwned;
```

Replace with:

```rust
use serde::de::DeserializeOwned;
```

Edit 21, same file: the crate imports. Current (lines 9-12):

```rust
use crate::hooks::{
    Authenticator, Cost, DynAuthenticator, Outcome, RequestMeta, RequestParts, ResponseMeta,
    RetryPolicy, Throttle,
};
```

Replace with:

```rust
use crate::hooks::{
    Authenticator, Cost, DynAuthenticator, Outcome, RequestMeta, RequestParts, ResponseMeta,
    RetryPolicy, Throttle,
};
use crate::{response::Response, transport::HttpRequest};
```

Edit 22, same file: the error list. Current (lines 47-48):

```rust
    /// - [`ApiError::Network`] when no response arrives. That attempt is
    ///   neither observed nor retried.
```

Replace with:

```rust
    /// - [`ApiError::Transport`] when no response arrives. That attempt is
    ///   neither observed nor retried.
```

Edit 23, same file: the attempt is sent on the transport. Current (line 76):

```rust
            let response = self.request(&url, signed).send().await?;
```

Replace with:

```rust
            let response = self.transport.send(self.http_request(&url, signed)).await?;
```

Edit 24, same file: the request builder. Current (lines 143-159):

```rust
    /// One attempt's request, built from its signed parts.
    fn request(&self, url: &Url, parts: RequestParts) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .request(parts.method, url.clone())
            .headers(parts.headers);
        if !parts.query.is_empty() {
            request = request.query(&parts.query);
        }
        if let Some(body) = parts.body {
            request = request.body(body);
        }
        if let Some(timeout) = parts.timeout {
            request = request.timeout(timeout);
        }
        request
    }
```

Replace with:

```rust
    /// One attempt's request, built from its signed parts: the URL with the
    /// query form-urlencoded onto it, as reqwest's `.query()` did, and a
    /// timeout that is always set, the request's own else the client's.
    fn http_request(&self, url: &Url, parts: RequestParts) -> HttpRequest {
        let mut url = url.clone();
        if !parts.query.is_empty() {
            url.query_pairs_mut().extend_pairs(&parts.query);
        }
        HttpRequest {
            method: parts.method,
            url,
            headers: parts.headers,
            body: parts.body.map(Into::into),
            timeout: parts.timeout.unwrap_or(self.timeout),
        }
    }
```

Edit 25, `polyoxide-core/src/request.rs`: the `Response` import. Remove `Response` from the `reqwest` import (at the base commit `use reqwest::{Method, Response};`; after story 3 `Method` comes from `http`, so the line is `use reqwest::Response;` and goes entirely), then add `use crate::response::Response;` beside the other `crate::` imports, after `use crate::hooks::{Cost, DynAuthenticator, RequestParts};`.

Edit 26, same file: `send` reads the body through `ApiError`. Current (lines 173-176):

```rust
        let text = response
            .text()
            .await
            .map_err(|e| E::from(ApiError::from(e)))?;
```

Replace with:

```rust
        let text = response.text().await.map_err(E::from)?;
```

Edit 27, `polyoxide-core/src/health.rs`: the `Response` import. Remove `Response` from the `reqwest` import (at the base commit `use reqwest::{Method, Response};`; after story 3 the line is `use reqwest::Response;` and goes entirely), then add `use crate::response::Response;` after `use crate::request::RequestError;`.

Edit 28, `polyoxide-core/src/error.rs`: `ErrorResponse::read` takes core's response. Current (line 63):

```rust
    pub async fn read(response: reqwest::Response) -> Self {
```

Replace with:

```rust
    pub async fn read(response: crate::Response) -> Self {
```

Edit 29, same file: `ApiError::from_response`. Current (line 127):

```rust
    pub async fn from_response(response: reqwest::Response) -> Self {
```

Replace with:

```rust
    pub async fn from_response(response: crate::Response) -> Self {
```

Edit 30, same file: `Network` goes. Current (lines 101-104, with the blank line after):

```rust
    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

```

Replace with nothing: delete those four lines, so `Transport` follows `Validation`'s variant directly.

Edit 31, same file: `classify_reqwest` and its helper go; `status_class` stays. Current (lines 138-178):

```rust
/// The class of a transport error from `reqwest`.
///
/// A failure in transit is [`Class::Network`]: a timeout, a failed connect, or
/// any error with an I/O error or a body-read failure in its source chain.
/// reqwest reports a body that broke off mid-read as a decode error, so these
/// are checked first. An error that carries an HTTP status follows the status
/// rule. Then a request that could not be built is [`Class::InvalidRequest`],
/// a body that arrived whole but did not decode is [`Class::Decode`], and
/// anything else is [`Class::Network`].
pub fn classify_reqwest(err: &reqwest::Error) -> Class {
    if failed_in_transit(err) {
        Class::Network
    } else if let Some(status) = err.status() {
        status_class(status.as_u16())
    } else if err.is_builder() {
        Class::InvalidRequest
    } else if err.is_decode() {
        Class::Decode
    } else {
        Class::Network
    }
}

/// Whether a reqwest error is the connection failing, however reqwest labels it.
fn failed_in_transit(err: &reqwest::Error) -> bool {
    if err.is_timeout() || err.is_connect() || err.is_body() {
        return true;
    }
    let mut source = std::error::Error::source(err);
    while let Some(inner) = source {
        if inner.is::<std::io::Error>()
            || inner
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_body)
        {
            return true;
        }
        source = inner.source();
    }
    false
}

```

Replace with nothing: delete those forty-two lines (the last of them blank), up to the doc comment of `status_class`.

Edit 32, same file: the class arm. Current (lines 204-205, the `Network` arm and the `Transport` arm Task 4.1 added after it):

```rust
            Self::Network(err) => classify_reqwest(err),
            Self::Transport(err) => err.class(),
```

Replace with:

```rust
            Self::Transport(err) => err.class(),
```

Edit 33, same file, tests: the helper for a transport failure. Current (lines 238-240):

```rust
    fn api(status: u16) -> ApiError {
        response(status, "", None).into()
    }
```

Replace with:

```rust
    fn api(status: u16) -> ApiError {
        response(status, "", None).into()
    }

    /// A transport failure of `kind`, caused by `source`.
    fn transport(
        kind: TransportKind,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> ApiError {
        ApiError::Transport(TransportError::new(kind, source))
    }
```

and add `use crate::transport::TransportKind;` after `use super::*;` at the top of `mod tests`.

Edit 34, same file, tests: the fixture keeps the reqwest errors that become a transport error's source. Current (lines 378-387):

```rust
    /// One reqwest error of each kind the rule tells apart.
    struct ReqwestErrors {
        builder: reqwest::Error,
        decode: reqwest::Error,
        cut_off: reqwest::Error,
        connect: reqwest::Error,
        timeout: reqwest::Error,
        unavailable: reqwest::Error,
        not_found: reqwest::Error,
    }
```

Replace with:

```rust
    /// One reqwest error of each kind a transport reports, to be the source
    /// of a `TransportError` of that kind.
    struct ReqwestErrors {
        builder: reqwest::Error,
        cut_off: reqwest::Error,
        connect: reqwest::Error,
        timeout: reqwest::Error,
    }
```

Edit 35, same file, tests: the server fixtures for the two errors that no longer exist. Current (lines 398-426):

```rust
        let mut server = mockito::Server::new_async().await;
        let _not_json = server
            .mock("GET", "/not-json")
            .with_body("not json")
            .create_async()
            .await;
        let _down = server
            .mock("GET", "/down")
            .with_status(503)
            .create_async()
            .await;
        let _missing = server
            .mock("GET", "/missing")
            .with_status(404)
            .create_async()
            .await;
        let get = |path: &str| client.get(format!("{}{path}", server.url())).send();

        let decode = get("/not-json")
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap_err();
        assert!(decode.is_decode());

        let status = |response: reqwest::Response| response.error_for_status().unwrap_err();
        let unavailable = status(get("/down").await.unwrap());
        let not_found = status(get("/missing").await.unwrap());

```

Replace with nothing: delete those thirty lines (the last of them blank; a reqwest decode error and a status error cannot come out of a transport: the body is read raw and a status is a response, so `Response::json`'s decode is `ApiError::Serialization` and a status is `ApiError::Response`, both already rows below).

Edit 36, same file, tests: the fixture's value. Current (lines 474-482):

```rust
        ReqwestErrors {
            builder,
            decode,
            cut_off,
            connect,
            timeout,
            unavailable,
            not_found,
        }
```

Replace with:

```rust
        ReqwestErrors {
            builder,
            cut_off,
            connect,
            timeout,
        }
```

Edit 37, same file, tests: the rows. Current (lines 567-599):

```rust
            (
                ApiError::Network(reqwest.builder),
                Class::InvalidRequest,
                true,
                None,
            ),
            (ApiError::Network(reqwest.decode), Class::Decode, true, None),
            // Labelled a decode error by reqwest, but the connection broke.
            (
                ApiError::Network(reqwest.cut_off),
                Class::Network,
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.connect),
                Class::Network,
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.timeout),
                Class::Network,
                true,
                None,
            ),
            (
                ApiError::Network(reqwest.unavailable),
                unavailable,
                true,
                None,
            ),
            (ApiError::Network(reqwest.not_found), refusal, true, None),
```

Replace with:

```rust
            (
                transport(TransportKind::Request, reqwest.builder),
                Class::InvalidRequest,
                true,
                None,
            ),
            // Labelled a decode error by reqwest, but the connection broke.
            (
                transport(TransportKind::Body, reqwest.cut_off),
                Class::Network,
                true,
                None,
            ),
            (
                transport(TransportKind::Connect, reqwest.connect),
                Class::Network,
                true,
                None,
            ),
            (
                transport(TransportKind::Timeout, reqwest.timeout),
                Class::Network,
                true,
                None,
            ),
            (
                transport(TransportKind::Other, "no answer"),
                Class::Network,
                true,
                None,
            ),
```

Edit 38, `polyoxide-core/tests/health.rs`: a ping's transport failure. Current (lines 231-234):

```rust
    assert!(
        matches!(&err, PingError::Api(ApiError::Network(_))),
        "{err:?}"
    );
```

Replace with:

```rust
    assert!(
        matches!(&err, PingError::Api(ApiError::Transport(_))),
        "{err:?}"
    );
```

Edit 39, `polyoxide-core/tests/mock_request.rs`: a request's timeout. Current (lines 581-584):

```rust
    assert!(
        matches!(&err, ApiError::Network(e) if e.is_timeout()),
        "{err:?}"
    );
```

Replace with:

```rust
    assert!(
        matches!(&err, ApiError::Transport(e) if e.kind() == polyoxide_core::TransportKind::Timeout),
        "{err:?}"
    );
```

Edit 40, `polyoxide-core/README.md`: the module table's `client` row. Current (line 11):

```markdown
| `client` | `HttpClient` / `HttpClientBuilder` -- configurable reqwest wrapper with base URL, connection pooling, concurrency limiting, and 429 retry logic |
```

Replace with:

```markdown
| `client` | `HttpClient` / `HttpClientBuilder` -- configurable client over an injected or default transport, with base URL, connection pooling, concurrency limiting, and 429 retry logic |
| `transport` | `HttpTransport`, the seam the send loop sends through, with `HttpRequest`, `TransportError` and `TransportKind`, and `ReqwestTransport`, the native default |
| `response` | `Response` -- a transport's answer, an owned head and a lazy body read by `bytes`, `text` and `json` |
```

Edit 41, same file: the `error` row. Current (line 12):

```markdown
| `error` | `ApiError` -- shared error enum (Response, Validation, Network, Serialization, Url, Refused, Sign), classed through `polyoxide_venue::Classify`; `ErrorResponse` -- an unsuccessful response read whole (status, headers, body, message, `Retry-After`), which `ApiError::Response` carries |
```

Replace with:

```markdown
| `error` | `ApiError` -- shared error enum (Response, Validation, Transport, Serialization, Url, Refused, Sign), classed through `polyoxide_venue::Classify`; `ErrorResponse` -- an unsuccessful response read whole (status, headers, body, message, `Retry-After`), which `ApiError::Response` carries |
```

Edit 42, same file: the error hierarchy's sentence. In the paragraph under `## Error hierarchy`, change `as `ApiError::Network`, `Url` and `Serialization`:` to `as `ApiError::Transport`, `Url` and `Serialization`:`.

Edit 43, same file: the key exports. Current (the block's HTTP lines):

```rust
// HTTP client
use polyoxide_core::{HttpClient, HttpClientBuilder, DEFAULT_TIMEOUT_MS, DEFAULT_POOL_SIZE};
```

Replace with:

```rust
// HTTP client
use polyoxide_core::{HttpClient, HttpClientBuilder, DEFAULT_TIMEOUT_MS, DEFAULT_POOL_SIZE};

// The transport it sends on
use polyoxide_core::{HttpRequest, HttpTransport, Response, TransportError, TransportKind};
```

**3c. The venue crates' callers.**

Edit 44, `polyoxide-polymarket/src/gamma/api/markets.rs`: `post_json` reads the body through `ApiError`. Current (lines 168-172):

```rust
    let text = response
        .text()
        .await
        .map_err(|e| GammaError::Api(ApiError::from(e)))?;
    serde_json::from_str(&text).map_err(|e| GammaError::Api(ApiError::from(e)))
```

Replace with:

```rust
    let text = response.text().await?;
    serde_json::from_str(&text).map_err(|e| GammaError::Api(ApiError::from(e)))
```

Edit 45, `polyoxide-polymarket/src/relay/client.rs`: the helper's return type. Current (lines 292-293):

```rust
        log_failure: Option<&str>,
    ) -> Result<polyoxide_core::reqwest::Response, RelayError> {
```

Replace with:

```rust
        log_failure: Option<&str>,
    ) -> Result<polyoxide_core::Response, RelayError> {
```

Edit 46, same file: `get`. Current (line 323):

```rust
    async fn get(&self, url: &Url) -> Result<polyoxide_core::reqwest::Response, RelayError> {
```

Replace with:

```rust
    async fn get(&self, url: &Url) -> Result<polyoxide_core::Response, RelayError> {
```

Edit 47, same file: `get_authed`. Current (lines 351-352):

```rust
        allow_relayer_api_key: bool,
    ) -> Result<polyoxide_core::reqwest::Response, RelayError> {
```

Replace with:

```rust
        allow_relayer_api_key: bool,
    ) -> Result<polyoxide_core::Response, RelayError> {
```

Edit 48, same file: `get_nonce`. Current (line 408):

```rust
        let data = resp.json::<NonceResponse>().await.map_err(ApiError::from)?;
```

Replace with:

```rust
        let data = resp.json::<NonceResponse>().await?;
```

Edit 49, same file: `get_transaction`. Current (lines 423-425):

```rust
        resp.json::<RelayerTransaction>()
            .await
            .map_err(|e| ApiError::from(e).into())
```

Replace with:

```rust
        resp.json::<RelayerTransaction>()
            .await
            .map_err(RelayError::from)
```

Edit 50, same file: `list_transactions`. Current (lines 444-446):

```rust
        resp.json::<Vec<RelayerTransaction>>()
            .await
            .map_err(|e| ApiError::from(e).into())
```

Replace with:

```rust
        resp.json::<Vec<RelayerTransaction>>()
            .await
            .map_err(RelayError::from)
```

Edit 51, same file: `list_relayer_api_keys`. Current (lines 465-467):

```rust
        resp.json::<Vec<RelayerApiKey>>()
            .await
            .map_err(|e| ApiError::from(e).into())
```

Replace with:

```rust
        resp.json::<Vec<RelayerApiKey>>()
            .await
            .map_err(RelayError::from)
```

Edit 52, same file: `get_deployed`. Current (lines 477-481):

```rust
        let data = resp
            .json::<DeployedResponse>()
            .await
            .map_err(ApiError::from)?;
        Ok(data.deployed)
```

Replace with:

```rust
        let data = resp.json::<DeployedResponse>().await?;
        Ok(data.deployed)
```

Edit 53, same file: `get_deployed_typed`. Current (lines 503-508):

```rust
        Ok(resp
            .json::<DeployedResponse>()
            .await
            .map_err(ApiError::from)?
            .deployed)
```

Replace with:

```rust
        Ok(resp.json::<DeployedResponse>().await?.deployed)
```

Edit 54, same file: `get_execute_params`. Current (lines 530-535):

```rust
        Ok(resp
            .json::<ExecuteParams>()
            .await
            .map_err(ApiError::from)?
            .nonce)
```

Replace with:

```rust
        Ok(resp.json::<ExecuteParams>().await?.nonce)
```

Edit 55, same file: `get_gasless_transaction`. Current (lines 557-559):

```rust
        resp.json::<GaslessTransaction>()
            .await
            .map_err(|e| ApiError::from(e).into())
```

Replace with:

```rust
        resp.json::<GaslessTransaction>()
            .await
            .map_err(RelayError::from)
```

Edit 56, same file: the relay payload. Current (line 652):

```rust
        let data = resp.json::<RelayPayload>().await.map_err(ApiError::from)?;
```

Replace with:

```rust
        let data = resp.json::<RelayPayload>().await?;
```

Edit 57, same file: `post_json`. Current (line 1839):

```rust
        let response_text = resp.text().await.map_err(ApiError::from)?;
```

Replace with:

```rust
        let response_text = resp.text().await?;
```

Edit 58, `polyoxide-polymarket/src/relay/error.rs`: the `Classify` documentation. Current (lines 37-38):

```rust
/// classes it: a relayer response by its status, a transport failure by
/// core's reqwest rule, a URL failure or a local refusal an `InvalidRequest`,
```

Replace with:

```rust
/// classes it: a relayer response by its status, a transport failure by its
/// kind (only a request that could not be built is not `Network`), a URL
/// failure or a local refusal an `InvalidRequest`,
```

Edit 59, same file, tests: the first row of `every_variant_classifies`. Current (lines 131-141):

```rust
    fn every_variant_classifies() {
        let builder = polyoxide_core::reqwest::Client::new()
            .get("not a url")
            .build()
            .unwrap_err();
        // Was `Reqwest`, `UrlParse` and `SerdeJson` until Story 3.11.
        let rows = [
            (
                RelayError::Api(ApiError::Network(builder)),
                Class::InvalidRequest,
            ),
```

Replace with:

```rust
    fn every_variant_classifies() {
        // Was `Reqwest`, `UrlParse` and `SerdeJson` until Story 3.11.
        let rows = [
            (
                RelayError::Api(ApiError::Transport(polyoxide_core::TransportError::new(
                    polyoxide_core::TransportKind::Request,
                    "not a url",
                ))),
                Class::InvalidRequest,
            ),
```

Edit 60, `polyoxide-polymarket/src/clob/api/orders.rs`: `ListOrders::send_raw`. Current (line 313):

```rust
    pub async fn send_raw(self) -> Result<polyoxide_core::reqwest::Response, ClobError> {
```

Replace with:

```rust
    pub async fn send_raw(self) -> Result<polyoxide_core::Response, ClobError> {
```

Edit 61, `polyoxide-polymarket/src/clob/api/account.rs`: the empty-body read. Current (lines 79-84):

```rust
        let text = request
            .send_raw()
            .await?
            .text()
            .await
            .map_err(ApiError::from)?;
```

Replace with:

```rust
        let text = request.send_raw().await?.text().await?;
```

Edit 62, `polyoxide-polymarket/src/perps/api/health.rs`: the ping's body check. Current (line 43):

```rust
        let text = pong.response.text().await.map_err(ApiError::from)?;
```

Replace with:

```rust
        let text = pong.response.text().await?;
```

Edit 63, `polyoxide-binance/src/usdm/api/health.rs`: the ping's body check. Current (line 36):

```rust
        let text = pong.response.text().await.map_err(ApiError::from)?;
```

Replace with:

```rust
        let text = pong.response.text().await?;
```

Edit 63a, `polyoxide-polymarket/src/clob/client.rs`: `ClobBuilder::build` passes its `http_transport` to the Gamma and Data clients it builds, as story 2 passes its runtime, so every HTTP exchange of a `Clob` goes through the injected transport. The lines below are as story 2 left them. Current (the Gamma fallback):

```rust
            crate::gamma::Gamma::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .runtime(runtime.clone())
                .build()
                .map_err(ClobError::Gamma)?
```

Replace with:

```rust
            let mut builder = crate::gamma::Gamma::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .runtime(runtime.clone());
            if let Some(transport) = &self.config.http_transport {
                builder = builder.http_transport(transport.clone());
            }
            builder.build().map_err(ClobError::Gamma)?
```

Current (the Data fallback):

```rust
            crate::data::DataApi::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .runtime(runtime)
                .build()
                .map_err(ClobError::Data)?
```

Replace with:

```rust
            let mut builder = crate::data::DataApi::builder()
                .timeout_ms(self.config.timeout_ms)
                .pool_size(self.config.pool_size)
                .runtime(runtime);
            if let Some(transport) = &self.config.http_transport {
                builder = builder.http_transport(transport.clone());
            }
            builder.build().map_err(ClobError::Data)?
```

(The CLOB's own client already carries the transport: `self.config.http_builder()` copies it, Edit 15.) A Gamma or Data client the caller passes to `ClobBuilder::gamma` or `ClobBuilder::data` keeps its own.

**3d. The tests that named the old types.**

Edit 64, `polyoxide-polymarket/tests/send_loop.rs`: the transport error. Current (line 412):

```rust
    assert!(matches!(err, ApiError::Network(_)), "{err:?}");
```

Replace with:

```rust
    assert!(matches!(err, ApiError::Transport(_)), "{err:?}");
```

Edit 65, `polyoxide-polymarket/tests/mock_relay.rs`: `relay_timeout_ms_bounds_a_plain_request`. Current (lines 2423-2427):

```rust
        matches!(
            &err,
            polyoxide_polymarket::relay::RelayError::Api(polyoxide_core::ApiError::Network(e)) if e.is_timeout()
        ),
```

Replace with:

```rust
        matches!(
            &err,
            polyoxide_polymarket::relay::RelayError::Api(polyoxide_core::ApiError::Transport(e)) if e.kind() == polyoxide_core::TransportKind::Timeout
        ),
```

Edit 66, `polyoxide-test-support/tests/failure_tags.rs`: the two transport failures go through a client now. Current (line 13):

```rust
use polyoxide_core::{ApiError, ErrorResponse};
```

Replace with:

```rust
use polyoxide_core::{ApiError, ErrorResponse, HttpClientBuilder, TransportKind};
```

Edit 67, same file: `network_timeout`. Current (lines 136-157):

```rust
/// A request to a listener that accepts and never answers, with a timeout
/// shorter than the wait: reqwest's timeout, whose Debug names `TimedOut`.
#[tokio::test]
async fn network_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/ok", listener.local_addr().unwrap());
    let _held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(socket);
    });
    let client = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_millis(200))
        .build()
        .unwrap();
    let source = client.get(&url).send().await.unwrap_err();
    assert!(source.is_timeout(), "{source:?}");
    let err = ApiError::Network(source);
    assert!(format!("{err:?}").contains("TimedOut"), "{err:?}");
    assert!(
        err.to_string().contains("error sending request for url"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Transient);
}
```

Replace with:

```rust
/// A request to a listener that accepts and never answers, with a timeout
/// shorter than the wait: the transport's timeout, whose source's Debug names
/// `TimedOut`.
#[tokio::test]
async fn network_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let _held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(socket);
    });
    let client = HttpClientBuilder::new(base).timeout_ms(200).build().unwrap();
    let err = client.get_bytes("/ok", &[]).await.unwrap_err();
    assert!(
        matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Timeout),
        "{err:?}"
    );
    assert!(format!("{err:?}").contains("TimedOut"), "{err:?}");
    assert!(
        err.to_string().contains("error sending request for url"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Transient);
}
```

Edit 68, same file: `network_connect`. Current (lines 159-178):

```rust
/// A request to a port nothing listens on: reqwest's connect failure, whose
/// Debug names hyper-util's `Connect` kind.
#[tokio::test]
async fn network_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/ok", listener.local_addr().unwrap());
    drop(listener);
    let source = polyoxide_core::reqwest::get(&url).await.unwrap_err();
    assert!(source.is_connect(), "{source:?}");
    let err = ApiError::Network(source);
    assert!(
        format!("{err:?}").contains("hyper_util::client::legacy::Error(Connect"),
        "{err:?}"
    );
    assert!(
        err.to_string().contains("error sending request for url"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Transient);
}
```

Replace with:

```rust
/// A request to a port nothing listens on: the transport's connect failure,
/// whose source's Debug names hyper-util's `Connect` kind.
#[tokio::test]
async fn network_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let client = HttpClientBuilder::new(base).build().unwrap();
    let err = client.get_bytes("/ok", &[]).await.unwrap_err();
    assert!(
        matches!(&err, ApiError::Transport(e) if e.kind() == TransportKind::Connect),
        "{err:?}"
    );
    assert!(
        format!("{err:?}").contains("hyper_util::client::legacy::Error(Connect"),
        "{err:?}"
    );
    assert!(
        err.to_string().contains("error sending request for url"),
        "{err}"
    );
    assert_eq!(tag(&err), Tag::Transient);
}
```

Edit 69, `polyoxide-binance/tests/live_usdm.rs`: the imports. Current (line 24):

```rust
use polyoxide_core::ApiError;
```

Replace with:

```rust
use polyoxide_core::{ApiError, TransportError, TransportKind};
```

Edit 70, same file: `raw` reads through core's types. Current (lines 186-206):

```rust
/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("https://fapi.binance.com{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    let status = response.status();
    // A refusal fails as core classifies its status, so its tag says what the
    // nightly does with it: a 451 is environmental, a 429 or a 5xx transient,
    // and a 403 from the firewall or a 418 ban real.
    if !status.is_success() {
        fail(path, &ApiError::from_response(response).await);
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}
```

Replace with:

```rust
/// A raw reqwest failure as core's transport error, so it fails by the class
/// core gives it: a refused connection or a timeout is `Network`, hence
/// transient.
fn transport(err: polyoxide_core::reqwest::Error) -> ApiError {
    let kind = if err.is_timeout() {
        TransportKind::Timeout
    } else if err.is_connect() {
        TransportKind::Connect
    } else if err.is_builder() {
        TransportKind::Request
    } else {
        TransportKind::Other
    };
    ApiError::Transport(TransportError::new(kind, err))
}

/// A raw reqwest response as core's [`polyoxide_core::Response`], so a refusal
/// is read as core reads one and fails by its status's class.
fn core_response(response: polyoxide_core::reqwest::Response) -> polyoxide_core::Response {
    let status = response.status();
    let headers = response.headers().clone();
    let url = response.url().clone();
    polyoxide_core::Response::new(
        status,
        headers,
        url,
        Box::pin(async move {
            response
                .bytes()
                .await
                .map_err(|err| TransportError::new(TransportKind::Body, err))
        }),
    )
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("https://fapi.binance.com{path}"))
        .send()
        .await
        .map_err(transport)
        .or_fail(path);
    let status = response.status();
    // A refusal fails as core classifies its status, so its tag says what the
    // nightly does with it: a 451 is environmental, a 429 or a 5xx transient,
    // and a 403 from the firewall or a 418 ban real.
    if !status.is_success() {
        fail(path, &ApiError::from_response(core_response(response)).await);
    }
    response.text().await.map_err(transport).or_fail(path)
}
```

Edit 71, `polyoxide-kalshi/tests/live_predictions.rs`: the imports. Current (line 21):

```rust
use polyoxide_core::ApiError;
```

Replace with:

```rust
use polyoxide_core::{ApiError, TransportError, TransportKind};
```

Edit 72, same file: `raw`. Current (lines 227-245):

```rust
/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("{PRODUCTION}{path}"))
        .send()
        .await
        .map_err(ApiError::Network)
        .or_fail(path);
    // A refusal fails as core classifies its status, so its tag says what the nightly does with
    // it: a 429 or a 5xx is transient.
    if !response.status().is_success() {
        fail(path, &ApiError::from_response(response).await);
    }
    response
        .text()
        .await
        .map_err(ApiError::Network)
        .or_fail(path)
}
```

Replace with:

```rust
/// A raw reqwest failure as core's transport error, so it fails by the class core gives it: a
/// refused connection or a timeout is `Network`, hence transient.
fn transport(err: polyoxide_core::reqwest::Error) -> ApiError {
    let kind = if err.is_timeout() {
        TransportKind::Timeout
    } else if err.is_connect() {
        TransportKind::Connect
    } else if err.is_builder() {
        TransportKind::Request
    } else {
        TransportKind::Other
    };
    ApiError::Transport(TransportError::new(kind, err))
}

/// A raw reqwest response as core's [`polyoxide_core::Response`], so a refusal is read as core
/// reads one and fails by its status's class.
fn core_response(response: polyoxide_core::reqwest::Response) -> polyoxide_core::Response {
    let status = response.status();
    let headers = response.headers().clone();
    let url = response.url().clone();
    polyoxide_core::Response::new(
        status,
        headers,
        url,
        Box::pin(async move {
            response
                .bytes()
                .await
                .map_err(|err| TransportError::new(TransportKind::Body, err))
        }),
    )
}

/// Fetches a route's raw JSON, outside the client, for the key comparison.
async fn raw(http: &polyoxide_core::reqwest::Client, path: &str) -> String {
    let response = http
        .get(format!("{PRODUCTION}{path}"))
        .send()
        .await
        .map_err(transport)
        .or_fail(path);
    // A refusal fails as core classifies its status, so its tag says what the nightly does with
    // it: a 429 or a 5xx is transient.
    if !response.status().is_success() {
        fail(path, &ApiError::from_response(core_response(response)).await);
    }
    response.text().await.map_err(transport).or_fail(path)
}
```

Edit 73, same file: the first raw client of `live_responses_carry_no_unmodelled_keys`. Current (lines 289-293):

```rust
    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(ApiError::Network)
        .or_fail("a reqwest client");
```

Replace with:

```rust
    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(transport)
        .or_fail("a reqwest client");
```

Edit 74, same file: the raw client of `live_catalog_responses_carry_no_unmodelled_keys`. Current (lines 558-562):

```rust
    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(ApiError::Network)
        .or_fail("a reqwest client");
```

Replace with:

```rust
    let http = polyoxide_core::reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(transport)
        .or_fail("a reqwest client");
```

**3e. The removals, the drift, the mutants ledger and the rules.**

Edit 75, `docs/s3-removals.md`: the removals. Run `nix develop --command python3 scripts/api_removals.py check --baseline v0.40.0`: it prints each removal the list does not hold as a key, `<crate> <lint id>: <item> (<file>)`, and exits 1. Current (lines 31-33):

```markdown
## Removed

Nothing yet.
```

Replace with the entries below, each key exactly as the command printed it (the keys here are the expected ones; use the printed key where it differs). Re-run the command until it exits 0. cargo-semver-checks does not see a re-typed return or parameter, so the last seven entries (keys of that form, kept descriptive) are the ones the command may report as `Listed ... but not reported` in a `::warning::`, which does not fail it; keep them, since spec D11 lists each break.

```markdown
## Removed

- `polyoxide-core struct_pub_field_missing: field client of struct HttpClient (src/client.rs)` Story 4: `HttpClient` holds a private `Arc<dyn HttpTransport>`; send through `HttpClient::send`, and to send on a reqwest client of your own build `ReqwestTransport::new(client)` and inject it with `HttpClientBuilder::http_transport`, a builder's `http_transport` setter or `ClientConfig::http_transport`.
- `polyoxide-core enum_variant_missing: variant ApiError::Network (src/error.rs)` Story 4: a failure to get a response, or to read its body, is `ApiError::Transport(TransportError)`; where you matched `ApiError::Network(e) if e.is_timeout()` match `ApiError::Transport(e) if e.kind() == TransportKind::Timeout`, and likewise `is_connect` is `TransportKind::Connect`.
- `polyoxide-core function_missing: function polyoxide_core::error::classify_reqwest (src/error.rs)` Story 4: classify the error itself, `Classify::class` on `ApiError` or on `TransportError`, which holds the same rule: only a `TransportKind::Request` is `InvalidRequest`, every other kind `Network`.
- `polyoxide-core inherent_method_changed: HttpClient::send (src/send.rs)` Story 4: it returns `Result<polyoxide_core::Response, ApiError>`, an owned head and a lazy body with `status`, `headers`, `url`, `bytes`, `text` and `json`, the last three returning `ApiError`; a `reqwest::Response` is no longer produced.
- `polyoxide-core inherent_method_changed: Request::send_raw (src/request.rs)` Story 4: it returns `Result<polyoxide_core::Response, E>`, read as `HttpClient::send`'s response above.
- `polyoxide-polymarket inherent_method_changed: ListOrders::send_raw (src/clob/api/orders.rs)` Story 4: it returns `Result<polyoxide_core::Response, ClobError>`; `.text().await?` still reads the body, its error now `ApiError`.
- `polyoxide-core struct_pub_field_changed: field response of struct Pong (src/health.rs)` Story 4: it is a `polyoxide_core::Response`; `status()`, `headers()` and `text().await` read as before, the last returning `ApiError`.
- `polyoxide-core inherent_method_changed: ErrorResponse::read (src/error.rs)` Story 4: it takes a `polyoxide_core::Response`; read an error response from a raw reqwest one with `ErrorResponse::new(status, headers, body)`.
- `polyoxide-core inherent_method_changed: ApiError::from_response (src/error.rs)` Story 4: it takes a `polyoxide_core::Response`; from a raw reqwest response, build `ErrorResponse::new(status, headers, body)` and convert it with `ApiError::from`.
- `polyoxide-core function_changed: retry_after_header (src/client.rs)` Story 4: it takes `&HeaderMap`, not a response; pass `response.headers()`.
```

Edit 76, `ci/test-body-diff.toml`: the drift entries. Run `git add -N polyoxide-core/tests/transport_injection.rs` if it is not tracked yet (git pairs only tracked files), then `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`. It exits 1 and prints, for each baseline test item this task changed, `::error::<old file:line item> differs at <new file>:<line> after normalising (sha256 <hex>). ...` with the diff below it. Each such item gets a `[[drift]]` entry at the end of `ci/test-body-diff.toml`, naming DRIFT row `R22` and the sha256 it printed, in this form:

```toml
[[drift]]
file = "<the baseline file as the script prints it>"
item = "<the item as the script prints it>"
# kind = "struct"   (only for an item that is not a fn, as the script prints it)
row = "R22"
sha256 = "<the sha256 as the script prints it>"
```

An item that story 3 already has an entry for is reported as stale (`that item has no diff whose sha256 is <old>`) beside the new diff: replace that entry's `sha256` with the new one instead of adding a second entry. The items expected, which are the ones this task changed: `polyoxide-core/src/config.rs`'s `tests::http_builder_applies_every_knob_that_is_set`; `polyoxide-core/src/error.rs`'s `tests::ReqwestErrors` (kind `struct`), `tests::reqwest_errors` and `tests::every_variant_classifies` (its decode, unavailable and not-found rows asserted an `ApiError::Network` case no transport can produce, so they go with the variant, which spec D12 and DRIFT row R22 cover: the decode case is re-pinned on `Response::json`, an `ApiError::Serialization` in `response::tests`, and a status is an `ApiError::Response`, already a row beside them; the entry cites R22 on that ground); `polyoxide-core/tests/health.rs`'s `a_transport_failure_on_health_is_the_callers_error`; `polyoxide-core/tests/mock_request.rs`'s `a_request_parts_timeout_bounds_its_attempt`; `polyoxide-polymarket/tests/send_loop.rs`'s `a_transport_error_is_not_retried_and_skips_observe_and_decide`; `polyoxide-polymarket/tests/mock_relay.rs`'s `relay_timeout_ms_bounds_a_plain_request`; `polyoxide-polymarket/src/relay/error.rs`'s `tests::every_variant_classifies`; `polyoxide-test-support/tests/failure_tags.rs`'s `network_timeout` and `network_connect`; `polyoxide-binance/tests/live_usdm.rs`'s `raw`; and `polyoxide-kalshi/tests/live_predictions.rs`'s `raw`, `live_responses_carry_no_unmodelled_keys` and `live_catalog_responses_carry_no_unmodelled_keys`. The script's output is the authority: an item it does not list needs no entry, and a lost item (it reports `has no counterpart at HEAD`) means a test was deleted by mistake: restore it. Run the script again until it exits 0.

Edit 77, `docs/MUTANTS.md` and `.github/scripts/tests/test_mutants_ledger.py`: the lines this task moved. Run `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'`. Each failure names a cited `file:line` whose code is no longer there. Find where it went:

```bash
grep -n "self.throttle.observe(&charge, &meta, &info);\|self.throttle.hold(hold);\|Outcome::Retry(wait) if info.retries_left > 0 => {\|let sleep = floor.max(wait);" polyoxide-core/src/send.rs
grep -n "fn retry_after_below_our_own_backoff_does_not_shorten_the_wait\|fn retry_after_zero_still_backs_off_exponentially_across_attempts" polyoxide-core/src/client.rs
```

and change the line number of each moved citation in two places: its key in `SNIPPETS` in `.github/scripts/tests/test_mutants_ledger.py`, and its citation in the rows of `docs/MUTANTS.md` that name it (rows (a), (f) and (g) cite the four lines of `send.rs`, and the two rows (b) cite the two tests of `client.rs`, which this task moved down). Re-prove the mutant of each row whose citation moved, as the file's header says: apply its Mutation column, run its tests with `nix develop --command cargo test -p <crate> <target> -- <test names>` and see every one fail, restore the line and see them pass. Then add one sentence to the file's header paragraph, in its form (`Rows ... were proved again on <date> on top of <short sha>, after story 4 of the wasm plan moved ...`), with the date and `git rev-parse --short HEAD`, naming the rows and files that moved. If the test passes at once, no citation moved and nothing changes in either file.

Edit 78, `.claude/rules/send-loop.md`: a section for the transport. Insert this before the heading `## reqwest, gzip and the header pins`:

```markdown
## The transport

`HttpClient` sends every attempt through a private `Arc<dyn HttpTransport>`
(`polyoxide-core/src/transport.rs`). The loop builds an `HttpRequest`: the base URL joined with
the path and the query form-urlencoded onto it as reqwest's `.query()` did, the method, the
signed headers, the body, and a `timeout` that is always set (the request's own, else the
client's) and bounds the whole exchange, body included. The transport answers with a `Response`,
an owned head and a lazy body read by `bytes()`, `text()` and `json()`, each into `ApiError`. The
loop decides on the head, releases the permit, then reads an error body.

- `ClientConfig::http_transport`, the `http_transport(..)` setter of `client_config_setters!` and
  `HttpClientBuilder::http_transport(..)` inject one. With none set, `build` makes
  `ReqwestTransport` from the builder's timeout, pool size and `gzip` (unset), a 10 s connect
  timeout and no redirects. `pool_size` and `gzip` feed only that native default.
- A transport that cannot produce a response is `ApiError::Transport(TransportError)`, neither
  observed nor retried. `TransportError::kind()` is `Connect`, `Timeout`, `Body`, `Request` or
  `Other`; only `Request` is `InvalidRequest`, so a body that broke off mid-read is `Network`.
  `ReqwestTransport` maps a reqwest error through one private function: no foreign-error `From`
  impl exists (`errors.md`).
- `Pong::response`, `Request::send_raw` and clob's `ListOrders::send_raw` return core's
  `Response`, and `ErrorResponse::read`, `ApiError::from_response` and `retry_after_header` take
  its head.
- `polyoxide-core/tests/transport_injection.rs` drives a retry, a hold, the permit rule and a
  transport failure through a recording transport.
```

Edit 79, same file: the reqwest sentence in the first bullet of `## reqwest, gzip and the header pins`. The sentence `No crate but core sends through `HttpClient`'s reqwest client.` (it wraps across two lines) becomes `Nothing but `ReqwestTransport` sends through reqwest: `HttpClient` holds a transport (`## The transport`).`

Edit 80, `.claude/rules/errors.md`: three sentences. In the `RelayError::Api` bullet, `Api(Network | Url | Serialization)` becomes `Api(Transport | Url | Serialization)`. The sentence in the `Classify` section that begins `A reqwest error that broke off mid-body is `Network`` (it wraps lines) becomes `A body that broke off mid-read is a `TransportKind::Body` failure, `Network`, though reqwest labels it a decode error.` And add this bullet after the one that begins `- `ApiError::Response` is classed by its status`:

```markdown
- `ApiError::Transport(TransportError)` is classed by its kind: `Request`, a request that could not
  be built, is `InvalidRequest`, and `Connect`, `Timeout`, `Body` and `Other` are `Network`.
  `TransportError` is built by `TransportError::new(kind, source)`, so a custom transport can
  make one, and is in core's `const _` assertion. `ApiError::Network` and `classify_reqwest` are
  gone.
```

Edit 81, `.claude/rules/clients.md`: a bullet before `- A venue's own knobs stay on its builder:`:

```markdown
- `http_transport(..)`, from the same macro, injects the `Arc<dyn HttpTransport>` every attempt is
  sent on; with none set, `build` makes `ReqwestTransport` (`send-loop.md`), and `pool_size` and
  the client's `gzip` feed only that native default.
```

Edit 82, `.claude/rules/live-tests.md`: the foreign-error sentence. Current (line 43):

```markdown
- Wrap a foreign error in the venue's own error first (`ApiError::Network`,
```

Replace with:

```markdown
- Wrap a foreign error in the venue's own error first (`ApiError::Transport`,
```

Edit 83, `.claude/rules/ci-gates.md`: the `[[drift]]` count and list (AD-21: the commit that adds entries edits that text). Run `grep -c '^\[\[drift\]\]' ci/test-body-diff.toml`. In the bullet that begins `- **The ` and ends with `` `[[drift]]` entries:** ``, the count (43 once story 3 has landed) becomes the number that command prints. Then the bullet's last sentence, which story 3 left ending

```
  `tests::retries_a_429`, and `send_loop`'s `a_429_with_no_retry_left_is_fail_with_its_hold`.
```

becomes

```
  `tests::retries_a_429`, and `send_loop`'s `a_429_with_no_retry_left_is_fail_with_its_hold`.
  The test bodies story 4 re-typed also cite R22, on the ground spec D12 gives (a row asserting an
  `ApiError::Network` case no transport can produce goes with the variant, its decode case
  re-pinned on `Response::json`): core's `config` test
  `http_builder_applies_every_knob_that_is_set`, `error`'s `ReqwestErrors`, `reqwest_errors` and
  `every_variant_classifies`, `health`'s `a_transport_failure_on_health_is_the_callers_error`
  and `mock_request`'s `a_request_parts_timeout_bounds_its_attempt` (story 3's entry, its hash
  replaced); polymarket's `send_loop`
  `a_transport_error_is_not_retried_and_skips_observe_and_decide`, `mock_relay`'s
  `relay_timeout_ms_bounds_a_plain_request` and `relay::error`'s `every_variant_classifies`;
  test-support's `failure_tags` `network_timeout` and `network_connect`; and the live suites'
  `raw` of `live_usdm` and `live_predictions`, with `live_predictions`'s
  `live_responses_carry_no_unmodelled_keys` and `live_catalog_responses_carry_no_unmodelled_keys`.
```

If the sentence does not end as shown (story 5's own edit to this bullet may sit after it), add the paragraph after the R22 sentence instead. List exactly the items `scripts/test_body_diff.py` made entries for in Edit 76.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`

Run: `nix develop --command cargo test -p polyoxide-core --all-features`

Expected: every suite reports `test result: ok`, among them `transport_injection` with five tests and the lib's `config::tests::an_injected_transport_is_the_one_the_client_sends_on`, `transport::tests::the_default_transport_follows_no_redirect` and the gzip tests of `client.rs` (`gzip_follows_reqwest_unless_set`, `gzip_false_asks_for_no_compression`, `gzip_asks_for_and_decodes_a_compressed_body`), which pin the default transport's compression unchanged.

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib clob::client::tests::the_builders_transport`

Expected: `the_builders_transport_reaches_the_gamma_client_it_builds` and `the_builders_transport_reaches_the_data_client_it_builds` pass.

Run: `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`

Expected: no warnings.

Run: `nix develop --command cargo test --workspace --all-features`

Expected: every suite reports `test result: ok`.

Run each header pin on the default transport, unchanged:

```bash
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features clob --test headers_clob
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features gamma --test headers_gamma
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features data --test headers_data
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features relay --test headers_relay
nix develop --command cargo test -p polyoxide-polymarket --no-default-features --features perps --test headers_perps
nix develop --command cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm
nix develop --command cargo test -p polyoxide-kalshi --no-default-features --features predictions --test headers_predictions
```

Expected: each reports `test result: ok. 1 passed`.

Run the query-setter suites, unchanged:

```bash
nix develop --command cargo test -p polyoxide-polymarket --all-features --test query_setters_clob --test query_setters_gamma --test query_setters_data
nix develop --command cargo test -p polyoxide-binance --all-features --test query_setters_usdm
nix develop --command cargo test -p polyoxide-kalshi --all-features --test query_setters_predictions
```

Expected: every suite reports `test result: ok`.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --all-features'`

Expected: no warnings.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_claude_rules.py tests/test_mutants_ledger.py tests/test_dep_allowlists.py -q'`

Expected: every test passes.

Run the story's script gates:

```bash
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command python3 scripts/api_removals.py check --baseline v0.40.0
```

Expected: each exits 0 (`api_removals.py` may print `::warning::` lines for entries cargo-semver-checks does not report).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-core/tests/transport_injection.rs
git commit -m "feat(core)!: the send loop sends through an injected HttpTransport, ApiError::Network becomes ApiError::Transport (wasm plan, story 4)" -- polyoxide-core polyoxide-polymarket polyoxide-binance polyoxide-kalshi polyoxide-test-support docs/s3-removals.md docs/MUTANTS.md ci/test-body-diff.toml .github/scripts/tests/test_mutants_ledger.py .claude/rules/send-loop.md .claude/rules/errors.md .claude/rules/clients.md .claude/rules/live-tests.md .claude/rules/ci-gates.md
```

## Story 5: `SocketTransport`, `BoxSocket` and the kit and every socket on the runtime

- **Developer:** intermediate-dev
- **Files:** `polyoxide-ws/Cargo.toml`, `polyoxide-ws/README.md`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/error.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs`, `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`, `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/helpers.rs`, `polyoxide-ws/src/test_server/tests.rs`, `polyoxide-ws/examples/close_reply_probe.rs`, `polyoxide-ws/tests/transport_injection.rs`, `polyoxide-venue/src/socket.rs`, `polyoxide-venue/src/ws_table.rs`, `polyoxide-venue/tests/ws_classification.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/ws/client.rs`, `polyoxide-polymarket/src/clob/ws/error.rs`, `polyoxide-polymarket/src/clob/ws/mod.rs`, `polyoxide-polymarket/src/clob/ws/scripted.rs`, `polyoxide-polymarket/src/clob/ws/supervised/market.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, `polyoxide-polymarket/src/perps/ws/builder.rs`, `polyoxide-polymarket/src/perps/ws/client.rs`, `polyoxide-polymarket/src/perps/ws/error.rs`, `polyoxide-polymarket/src/perps/ws/mod.rs`, `polyoxide-polymarket/src/perps/ws/protocol.rs`, `polyoxide-polymarket/src/perps/ws/scripted.rs`, `polyoxide-polymarket/src/rtds/client.rs`, `polyoxide-polymarket/src/rtds/error.rs`, `polyoxide-polymarket/src/rtds/supervisor.rs`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/error.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `polyoxide-polymarket/tests/bare_sports.rs`, `polyoxide-polymarket/tests/failure_tags_rtds.rs`, `polyoxide-polymarket/tests/failure_tags_sports.rs`, `polyoxide-polymarket/tests/live_clob_ws.rs`, `polyoxide-polymarket/tests/live_sports.rs`, `polyoxide-polymarket/tests/supervision_rtds.rs`, `polyoxide-polymarket/tests/supervision_sports.rs`, `polyoxide-polymarket/tests/trading_clob.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/scripted.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/live_usdm_ws.rs`, `polyoxide-binance/tests/supervision_usdm.rs`, `polyoxide-binance/tests/supervision_usdm_edges.rs`, `polyoxide-binance/tests/supervision_usdm_kit.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `docs/s3-removals.md`, `docs/MUTANTS.md`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`, `.claude/rules/sockets.md`, `.claude/rules/polymarket-clob-ws.md`, `.claude/rules/errors.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `Cargo.lock`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 5 → Stories 7 to 10"; uses Story 1's `Runtime`, `BoxFuture`, `default_runtime`, `spawn`, `Task`, `timeout`, `interval` and `Instant`, and Story 3's row `R22`
- **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo hack check -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-ws --each-feature --no-dev-deps && python3 scripts/venue_gate.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
- **Review:** deep (the supervisor's guarantees, the CLOB's dial and a breaking API change; the `connect_async(` landmine applies)

This story gives `polyoxide-ws` its transport seam and puts every timer, clock read and spawn of the kit and of the socket modules on the injected runtime, so a socket dials through a `SocketTransport` and waits through a `polyoxide_rt::Runtime` that its builder, or the target, chooses. Its code holds the landmine marker `connect_async(`: `TungsteniteTransport` in `polyoxide-ws/src/transport.rs` is the one place outside the test server that calls it, after `ensure_crypto_provider`, and `polyoxide-polymarket`'s `dial` goes. The supervisor should dispatch its developer with `effort: xhigh`. Task 5.1 adds the seam without touching a caller: `SocketIo`, `BoxSocket`, `SocketTransport`, `Capabilities`, the plain `TungsteniteTransport`, `default_socket_transport`, `transport_setters!`, `Kind::Unsupported` (a caller's fault, so `ws_is_fault` stays `true` for it, as for `Url` and `AlreadyClosed`) and the socket table's row for it; `default_socket_transport` and `TungsteniteTransport` are native only until Story 7 adds the wasm32 arm. Task 5.2 is the one breaking commit, since the re-typed `connect`, `connect_over`, `Connection::new` and `Protocol::connect` land with every caller: `BoxSocket` replaces `Socket`, `Connection::with_runtime`, `Connection::runtime()`, `Connection::take_paced_id` (the paced id Story 8's synchronous ping takes) and `Supervisor::runtime` and `socket_transport` put the kit's clocks and spawns on the runtime, `Connection::last_inbound` returns the runtime's `Instant`, `with_timeout` keeps its signature on `default_runtime()`, the CLOB's per-address dial moves into `TungsteniteTransport::per_address()` with its tests, the CLOB's builders default to it and keep a thin `Protocol::connect` override that only maps `Kind::ConnectTimeout` to `ClobWsError::ConnectTimeout`, and a new test drives a `Supervisor` through a connect, a replay and a reconnect on an in-memory transport. Tasks 5.3 to 5.5 move the remaining clock reads and spawns of the polymarket, Binance and Kalshi socket modules onto the runtime and give each socket builder its setters; a bare client without a builder dials through the free `connect`, on the defaults. Every task ends with the story's gates passing.

### Task 5.1: The transport seam, with no caller changed

**Files:**
- Create: `polyoxide-ws/src/transport.rs`
- Modify: `polyoxide-ws/Cargo.toml:17-27,43-45`
- Modify: `polyoxide-ws/src/lib.rs:16-19,46-72`
- Modify: `polyoxide-ws/src/error.rs:85-99,133-155,220-250`
- Modify: `polyoxide-ws/src/connect.rs:4-12,62-102,267-280`
- Modify: `polyoxide-ws/src/timing.rs:85`
- Modify: `polyoxide-venue/src/ws_table.rs:28-37,115-124`
- Modify: `polyoxide-venue/src/socket.rs:1-12`
- Test: `polyoxide-venue/tests/ws_classification.rs`
- Modify (regenerated): `Cargo.lock`, `ci/dep-allowlists/`

- [ ] **Step 1: Write the failing tests**

Three rows first. In `polyoxide-venue/tests/ws_classification.rs`, end `every_other_kind()` with a row for the new kind. The current end of the function is

```rust
        (
            transport(tungstenite::Error::AlreadyClosed),
            Kind::AlreadyClosed,
            Class::InvalidRequest,
        ),
    ]
}
```

and becomes

```rust
        (
            transport(tungstenite::Error::AlreadyClosed),
            Kind::AlreadyClosed,
            Class::InvalidRequest,
        ),
        (
            transport(tungstenite::Error::Io(
                std::io::ErrorKind::Unsupported.into(),
            )),
            Kind::Unsupported,
            Class::InvalidRequest,
        ),
    ]
}
```

In `polyoxide-ws/src/error.rs`, `every_transport_error_has_its_kind` has this first row

```rust
            (
                tungstenite::Error::Io(std::io::ErrorKind::UnexpectedEof.into()),
                Kind::Io,
            ),
```

Add after it

```rust
            (
                tungstenite::Error::Io(std::io::ErrorKind::Unsupported.into()),
                Kind::Unsupported,
            ),
```

In `polyoxide-ws/src/connect.rs`, the array of `a_failure_that_replays_on_every_attempt_is_not_retried` is

```rust
            tungstenite::Error::AttackAttempt,
            tungstenite::Error::Capacity(CapacityError::TooManyHeaders),
        ] {
```

and becomes

```rust
            tungstenite::Error::AttackAttempt,
            tungstenite::Error::Capacity(CapacityError::TooManyHeaders),
            tungstenite::Error::Io(std::io::ErrorKind::Unsupported.into()),
        ] {
```

Then create `polyoxide-ws/src/transport.rs` holding only its test module, so the tests for the new items exist before the items do:

```rust
#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use futures_util::StreamExt;
    use polyoxide_rt::{default_runtime, BoxFuture, Instant, Runtime};

    use super::*;
    use crate::{
        test_server::{Script, ScriptedServer},
        tungstenite::{client::IntoClientRequest, Message},
        Kind,
    };

    /// A runtime whose clock has already run out: every sleep is over at once.
    #[derive(Debug)]
    struct Elapsed;

    impl Runtime for Elapsed {
        fn spawn(&self, _task: BoxFuture<()>) {}

        fn sleep(&self, _duration: Duration) -> BoxFuture<()> {
            Box::pin(std::future::ready(()))
        }

        fn now(&self) -> Instant {
            Instant::now()
        }
    }

    #[test]
    fn capabilities_are_what_they_were_built_with() {
        let caps = Capabilities::new(false, true, false);
        assert!(!caps.client_pings);
        assert!(caps.sees_server_pings);
        assert!(!caps.handshake_headers);
        assert_eq!(Capabilities::FULL, Capabilities::new(true, true, true));
        assert_ne!(Capabilities::FULL, caps);
    }

    #[test]
    fn the_native_default_transport_can_do_everything() {
        assert_eq!(
            default_socket_transport().capabilities(),
            Capabilities::FULL
        );
        assert_eq!(
            TungsteniteTransport::new().capabilities(),
            Capabilities::FULL
        );
    }

    #[tokio::test]
    async fn the_plain_transport_dials_and_the_socket_reads_a_frame() {
        let server = ScriptedServer::start(vec![Script::frames(&["hello"])]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let mut socket = TungsteniteTransport::new()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
            .expect("connect");
        let first = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("a frame within the window")
            .expect("the stream has not ended")
            .expect("a frame, not an error");
        assert_eq!(first, Message::Text("hello".into()));
    }

    #[tokio::test]
    async fn a_refused_upgrade_keeps_its_status() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(503)]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let Err(err) = TungsteniteTransport::new()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
        else {
            panic!("a refused upgrade must not connect");
        };
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status: 503,
                retry_after: None
            }
        );
    }

    #[tokio::test]
    async fn the_transport_bounds_its_connect_with_the_runtime_it_is_given() {
        // The server never answers the upgrade and the timeout is an hour, so
        // only the given runtime's timer, which is already over, can end it.
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let after = Duration::from_secs(3600);
        let Err(err) = TungsteniteTransport::new()
            .connect(request, after, Arc::new(Elapsed))
            .await
        else {
            panic!("a stalled handshake must not connect");
        };
        assert_eq!(err.kind(), Kind::ConnectTimeout { after });
    }

    #[test]
    fn the_setters_write_the_builders_fields() {
        #[derive(Default)]
        struct Builder {
            runtime: Option<Arc<dyn Runtime>>,
            socket_transport: Option<Arc<dyn SocketTransport>>,
        }

        impl Builder {
            crate::transport_setters! {
                /// The runtime.
                runtime;
                /// The transport.
                socket_transport;
            }
        }

        let bare = Builder::default();
        assert!(bare.runtime.is_none() && bare.socket_transport.is_none());
        let set = Builder::default()
            .runtime(default_runtime())
            .socket_transport(default_socket_transport());
        assert!(set.runtime.is_some() && set.socket_transport.is_some());
    }
}
```

Add `mod transport;` after `mod tls;` in `polyoxide-ws/src/lib.rs` so the file compiles into the crate.

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-ws --all-features transport`
Expected: a compile error, not a test failure. It lists `cannot find type \`Capabilities\` in this scope`, `cannot find function \`default_socket_transport\``, `cannot find struct \`TungsteniteTransport\``, `cannot find trait \`SocketTransport\``, the unresolved macro `transport_setters` and `no variant or associated item named \`Unsupported\` found for enum \`Kind\``. Run `nix develop --command cargo test -p polyoxide-venue --test ws_classification` too: it fails to compile on `Kind::Unsupported`.

- [ ] **Step 3: Implement**

`polyoxide-ws/Cargo.toml`: declare the runtime beside the other dependencies. After the line `futures-util = { workspace = true, features = ["sink"] }` add

```toml
# The injected runtime every timer, clock read and spawn of the kit goes through.
polyoxide-rt = { workspace = true }
```

and replace the `notes` line under `[package.metadata.polyoxide]` (the `readme` line stays) with

```toml
notes = ["depends on neither `polyoxide-core` nor `polyoxide-venue`, and builds only polyoxide-rt, tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls"]
```

CLAUDE.md's crate list is a generated region, so the `gen_registry.py --write` below rewrites the `polyoxide-ws` bullet (its `needs:` now names `polyoxide-rt`, from cargo metadata, and its notes) and nothing is edited there by hand.

`polyoxide-ws/src/error.rs`: add the kind and read it. After the `AlreadyClosed` variant of `Kind`

```rust
    /// The connection was used after it closed.
    AlreadyClosed,
```

add

```rust
    /// The transport cannot perform the operation, such as a handshake header
    /// or a control frame on a transport that has none. A transport reports it
    /// as an I/O error of kind `std::io::ErrorKind::Unsupported`, so reconnecting
    /// never fixes it.
    Unsupported,
```

and in `WsError::kind`, replace

```rust
                Ws::Io(_) => Kind::Io,
```

with

```rust
                Ws::Io(err) if err.kind() == std::io::ErrorKind::Unsupported => Kind::Unsupported,
                Ws::Io(_) => Kind::Io,
```

`polyoxide-ws/src/connect.rs`: three edits. Replace the imports

```rust
use std::{future::Future, time::Duration};

use tokio::net::TcpStream;
```

with

```rust
use std::{future::Future, time::Duration};

use polyoxide_rt::{default_runtime, Runtime};
use tokio::net::TcpStream;
```

Replace the body of `with_timeout`

```rust
pub async fn with_timeout<F: Future>(timeout: Duration, connect: F) -> Result<F::Output, WsError> {
    tokio::time::timeout(timeout, connect)
        .await
        .map_err(|_| WsError::connect_timeout(timeout))
}
```

with

```rust
pub async fn with_timeout<F: Future>(timeout: Duration, connect: F) -> Result<F::Output, WsError> {
    let runtime = default_runtime();
    bounded(runtime.as_ref(), timeout, connect).await
}

/// Run `future` within `timeout` on `runtime`, giving up with
/// [`Kind::ConnectTimeout`]. A transport bounds each of its own steps with the
/// runtime it is given.
pub(crate) async fn bounded<F: Future>(
    runtime: &dyn Runtime,
    timeout: Duration,
    future: F,
) -> Result<F::Output, WsError> {
    polyoxide_rt::timeout(runtime, timeout, future)
        .await
        .ok_or_else(|| WsError::connect_timeout(timeout))
}
```

and the last arm of `retrying_can_fix`

```rust
        Kind::Url | Kind::HttpFormat | Kind::Tls | Kind::AttackAttempt | Kind::Capacity => false,
```

with

```rust
        Kind::Url
        | Kind::HttpFormat
        | Kind::Tls
        | Kind::AttackAttempt
        | Kind::Capacity
        | Kind::Unsupported => false,
```

In the doc comment above it, change "A bad URL, an upgrade request that could not be built, a refused TLS configuration, an attack detection and a capacity limit replay the same failure on every attempt, so they are `false`." to end "..., an attack detection, a capacity limit and an operation the transport cannot perform replay the same failure on every attempt, so they are `false`."

`polyoxide-ws/src/timing.rs:85`: in the `next_wait` doc, replace `` which `tokio::time::timeout` `` with `which a timeout` so the line reads "... gives `Duration::MAX`, which a timeout treats as never."

`polyoxide-ws/src/lib.rs`: in the crate docs, replace the bullet

```rust
//! - [`connect`] opens a connection within a timeout, [`connect_over`]
//!   upgrades a TCP connection a venue opened itself, [`with_timeout`] bounds
//!   a venue's own connect, and [`retrying_can_fix`] says whether a
//!   reconnect can help after one failed.
```

with

```rust
//! - [`SocketTransport`] is how a connection is dialled, giving a
//!   [`BoxSocket`] and reporting its [`Capabilities`]; [`TungsteniteTransport`]
//!   is the native one, and [`transport_setters!`] gives a builder its
//!   `runtime` and `socket_transport` setters.
//! - [`connect`] opens a connection within a timeout, [`connect_over`]
//!   upgrades a TCP connection a venue opened itself, [`with_timeout`] bounds
//!   a venue's own connect, and [`retrying_can_fix`] says whether a
//!   reconnect can help after one failed.
```

and add the exports. After `pub use tls::ensure_crypto_provider;` add

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use transport::TungsteniteTransport;
pub use transport::{default_socket_transport, BoxSocket, Capabilities, SocketIo, SocketTransport};
```

Now the implementation of `polyoxide-ws/src/transport.rs`. Put this above the `#[cfg(test)] mod tests` block Step 1 wrote:

```rust
//! How a connection is dialled: the socket transport seam, and the native
//! transport behind it.

use std::{fmt, sync::Arc, time::Duration};

use futures_util::{Sink, Stream};
use polyoxide_rt::{BoxFuture, Runtime};

use crate::{
    tungstenite::{self, http::Request, Message},
    WsError,
};

/// An open connection as the kit reads and writes it: a stream of frames and a
/// sink for them, both failing with tungstenite's error.
///
/// Every type that is both, and `Send + Unpin`, is one, so a transport returns
/// its own socket boxed as a [`BoxSocket`].
pub trait SocketIo:
    Stream<Item = Result<Message, tungstenite::Error>>
    + Sink<Message, Error = tungstenite::Error>
    + Send
    + Unpin
{
}

impl<T> SocketIo for T where
    T: Stream<Item = Result<Message, tungstenite::Error>>
        + Sink<Message, Error = tungstenite::Error>
        + Send
        + Unpin
{
}

/// An open connection, boxed: what a [`SocketTransport`] returns and
/// [`Connection`](crate::Connection) reads.
pub type BoxSocket = Box<dyn SocketIo>;

/// How a WebSocket connection is dialled.
///
/// A builder holds one, or leaves it to [`default_socket_transport`], so a
/// consumer injects nothing unless it wants its own dial. The transport
/// bounds each of its own steps with the `runtime` it is given, so one
/// transport serves clients with different runtimes, and a connect that does
/// not open within `timeout` fails as [`Kind::ConnectTimeout`](crate::Kind::ConnectTimeout).
/// Every other failure is the transport's: a refused upgrade is a
/// [`Kind::Refused`](crate::Kind::Refused) where the transport can see its
/// status.
pub trait SocketTransport: Send + Sync + fmt::Debug + 'static {
    /// Open the connection `request` describes, within `timeout`.
    fn connect(
        &self,
        request: Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>>;

    /// What this transport can do, so a protocol adapts instead of failing.
    fn capabilities(&self) -> Capabilities;
}

/// What a [`SocketTransport`] can do.
///
/// A protocol reads it to choose how it keeps a connection alive: a transport
/// that cannot send a ping frame gets an application-level ping, and one that
/// never shows the server's ping frames cannot count them as liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Capabilities {
    /// Whether the client can send ping and pong frames.
    pub client_pings: bool,
    /// Whether the server's ping frames reach the reader as messages.
    pub sees_server_pings: bool,
    /// Whether the handshake request may carry headers beyond the ones the
    /// WebSocket protocol generates.
    pub handshake_headers: bool,
}

impl Capabilities {
    /// Every capability: what a native socket has.
    pub const FULL: Capabilities = Capabilities::new(true, true, true);

    /// A transport's capabilities.
    pub const fn new(
        client_pings: bool,
        sees_server_pings: bool,
        handshake_headers: bool,
    ) -> Self {
        Self {
            client_pings,
            sees_server_pings,
            handshake_headers,
        }
    }
}

/// The transport a builder uses when none was injected: the native one on this
/// target.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_socket_transport() -> Arc<dyn SocketTransport> {
    Arc::new(TungsteniteTransport::new())
}

/// The native transport: tokio-tungstenite over TCP, with TLS from rustls.
///
/// It installs the rustls `CryptoProvider` before it dials
/// ([`ensure_crypto_provider`](crate::ensure_crypto_provider)), since a
/// consumer that builds two rustls backends gets no default and a direct
/// connect would panic. Its capabilities are all `true`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct TungsteniteTransport;

#[cfg(not(target_arch = "wasm32"))]
impl TungsteniteTransport {
    /// The plain dial: tungstenite's own connect, which resolves the host and
    /// connects to the first address it returns.
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SocketTransport for TungsteniteTransport {
    fn connect(
        &self,
        request: Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        Box::pin(async move {
            crate::ensure_crypto_provider();
            let (socket, _response) = crate::connect::bounded(
                runtime.as_ref(),
                timeout,
                tokio_tungstenite::connect_async(request),
            )
            .await??;
            Ok::<BoxSocket, WsError>(Box::new(socket))
        })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::FULL
    }
}

/// Generates the transport setters a socket builder offers, each writing a
/// field of the builder: `runtime(Arc<dyn Runtime>)` writes `self.runtime`, an
/// `Option<Arc<dyn polyoxide_rt::Runtime>>`, and `socket_transport(Arc<dyn
/// SocketTransport>)` writes `self.socket_transport`, an
/// `Option<Arc<dyn SocketTransport>>`.
///
/// Invoke it in the builder's `impl` as [`timing_setters!`](crate::timing_setters)
/// is, listing the setters wanted, each with its own doc comment. The
/// invoking crate declares `polyoxide-rt`. A builder's connect falls back to
/// [`default_runtime`](polyoxide_rt::default_runtime) and its default
/// transport for a field left `None`.
///
/// ```
/// use std::sync::Arc;
///
/// #[derive(Default)]
/// pub struct FeedBuilder {
///     runtime: Option<Arc<dyn polyoxide_rt::Runtime>>,
///     socket_transport: Option<Arc<dyn polyoxide_ws::SocketTransport>>,
/// }
///
/// impl FeedBuilder {
///     polyoxide_ws::transport_setters! {
///         /// The runtime the feed's timers and tasks run on.
///         runtime;
///         /// How the feed dials.
///         socket_transport;
///     }
/// }
///
/// let builder = FeedBuilder::default()
///     .runtime(polyoxide_rt::default_runtime())
///     .socket_transport(polyoxide_ws::default_socket_transport());
/// assert!(builder.runtime.is_some() && builder.socket_transport.is_some());
/// ```
#[macro_export]
macro_rules! transport_setters {
    () => {};
    ($(#[$meta:meta])* runtime; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn runtime(
            mut self,
            runtime: ::std::sync::Arc<dyn ::polyoxide_rt::Runtime>,
        ) -> Self {
            self.runtime = ::core::option::Option::Some(runtime);
            self
        }
        $crate::transport_setters! { $($rest)* }
    };
    ($(#[$meta:meta])* socket_transport; $($rest:tt)*) => {
        $(#[$meta])*
        pub fn socket_transport(
            mut self,
            transport: ::std::sync::Arc<dyn $crate::SocketTransport>,
        ) -> Self {
            self.socket_transport = ::core::option::Option::Some(transport);
            self
        }
        $crate::transport_setters! { $($rest)* }
    };
}
```

`polyoxide-venue`: in `polyoxide-venue/src/ws_table.rs` add the kind to the `InvalidRequest` arm of `ws_class`. Replace

```rust
                | ::polyoxide_ws::Kind::Capacity
                | ::polyoxide_ws::Kind::AlreadyClosed => $crate::Class::InvalidRequest,
```

with

```rust
                | ::polyoxide_ws::Kind::Capacity
                | ::polyoxide_ws::Kind::AlreadyClosed
                | ::polyoxide_ws::Kind::Unsupported => $crate::Class::InvalidRequest,
```

and in the macro's doc table replace the row

```rust
/// | `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity`, `AlreadyClosed` | [`InvalidRequest`](crate::Class::InvalidRequest) |
```

with

```rust
/// | `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity`, `AlreadyClosed`, `Unsupported` (an operation the transport cannot perform) | [`InvalidRequest`](crate::Class::InvalidRequest) |
```

In the same file's doc paragraph on `ws_is_fault`, replace "a 418 ban is a fault the client earned." with "a 418 ban is a fault the client earned, and an `Unsupported` is the caller's, so it is a fault too." (`ws_is_fault` itself is unchanged: it is `false` only for a 451, and the `Unsupported` row of `every_other_kind()` asserts it is `true`.)

In `polyoxide-venue/src/socket.rs`'s module docs, replace "a capacity limit and a use after close are [`InvalidRequest`](Class::InvalidRequest);" with "a capacity limit, a use after close and an operation the transport cannot perform are [`InvalidRequest`](Class::InvalidRequest);".

Regenerate what the dependency and the metadata changed, then check the diff names only the new dependency's tree:

Run: `nix develop --command python3 scripts/gen_registry.py --write`
Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`
Run: `git diff --stat ci/dep-allowlists/`
Expected: the `polyoxide-ws` files, and only them, gain `polyoxide-rt` and its dependencies. A file of another crate in the diff is a mistake: stop and report it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`
Run: `nix develop --command cargo test -p polyoxide-ws -p polyoxide-venue --all-features`
Expected: `test result: ok.` for every target, the doctest of `transport_setters!` included.
Run: `nix develop --command cargo clippy -p polyoxide-ws -p polyoxide-venue --all-targets --all-features -- -D warnings`
Expected: no warnings.
Run: `nix develop --command python3 scripts/venue_gate.py` and `nix develop --command python3 scripts/gen_registry.py --check`
Expected: both exit 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-ws/src/transport.rs
git commit -m "feat(ws): the socket transport seam and Kind::Unsupported (wasm32 story 5)

SocketTransport, BoxSocket, Capabilities, the plain TungsteniteTransport,
default_socket_transport and transport_setters!, with the socket table's row
for the new kind. polyoxide-ws now depends on polyoxide-rt, so the allowlists
of its trees are regenerated." -- polyoxide-ws/Cargo.toml polyoxide-ws/src/lib.rs polyoxide-ws/src/transport.rs polyoxide-ws/src/connect.rs polyoxide-ws/src/error.rs polyoxide-ws/src/timing.rs polyoxide-venue/src/ws_table.rs polyoxide-venue/src/socket.rs polyoxide-venue/tests/ws_classification.rs CLAUDE.md docs/ARCHITECTURE.md Cargo.lock ci/dep-allowlists
```

### Task 5.2: BoxSocket, the kit on the runtime and the CLOB's dial in the kit

**Files:**
- Modify: `polyoxide-ws/src/transport.rs`
- Modify: `polyoxide-ws/src/connect.rs:1-60`
- Modify: `polyoxide-ws/src/bare.rs:4-14,60-70,84-97,104-109,129-144,160-201,203-258`
- Modify: `polyoxide-ws/src/request.rs:1-9,25-32,70-132,187-198`
- Modify: `polyoxide-ws/src/supervisor/mod.rs:5-21,148-168,360-410,419-461`
- Modify: `polyoxide-ws/src/supervisor/task.rs:4-17,158-200,218-267,269-302,353-362,424-478,486-651`
- Modify: `polyoxide-ws/src/lib.rs:58-72`
- Modify: `polyoxide-ws/README.md:12-17`
- Modify: `polyoxide-ws/src/supervisor/tests.rs:1105-1160`
- Create: `polyoxide-ws/tests/transport_injection.rs`
- Modify: `polyoxide-polymarket/Cargo.toml`
- Modify: `polyoxide-polymarket/src/clob/ws/client.rs:1-148,539-672,860-945,1228-1272,1400-1500`
- Modify: `polyoxide-polymarket/src/clob/ws/supervised/market.rs:5-22,154-169,287-366,520-540`
- Modify: `polyoxide-polymarket/src/clob/ws/supervised/user.rs:25-42,205-223,356-434`
- Modify: `polyoxide-polymarket/src/clob/client.rs:932-941`
- Modify: `polyoxide-polymarket/tests/live_sports.rs:31-60`
- Modify: `docs/s3-removals.md`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md:36`, `.claude/rules/sockets.md`, `.claude/rules/polymarket-clob-ws.md`, `.claude/rules/errors.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`
- Modify (regenerated): `Cargo.lock`, `ci/dep-allowlists/`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-ws/tests/transport_injection.rs`. It drives a `Supervisor` over a transport that opens no socket, on a runtime that counts what it is asked, through a connect, a replay, a server close and a reconnect:

```rust
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
}

impl Runtime for CountingRuntime {
    fn spawn(&self, task: BoxFuture<()>) {
        self.spawns.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn(task);
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        self.sleeps.fetch_add(1, Ordering::SeqCst);
        self.inner.sleep(duration)
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
    drop(seen);
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
    let connection =
        Connection::with_runtime(socket, Mode::Data, Arc::new(TokioRuntime::default()));
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
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-ws --all-features --test transport_injection`
Expected: a compile error. It prints ``error[E0599]: no method named `runtime` found for struct `Supervisor<Feed>` in the current scope`` and the same for `socket_transport`, ``no function or associated item named `with_runtime` found for struct `Connection` `` and ``no method named `take_paced_id` found for struct `Connection` ``; the wording of the rest varies by rustc.

- [ ] **Step 3: Implement**

**3a. Cargo.** In `polyoxide-polymarket/Cargo.toml`, remove `"tokio/net",` from the `clob-ws` feature: the dial moves into the kit, which owns tokio's `net`. The list becomes

```toml
clob-ws = [
    "clob",
    "dep:polyoxide-ws",
    "dep:futures-util",
    "polyoxide-venue/trading",
    "tokio/macros",
    "tokio/sync",
]
```

Declare the runtime as a plain dependency, which every socket module needs whatever HTTP features are on. Story 2 declared it optional, behind the five HTTP features, so the edit replaces its line and comment and drops the five feature entries. In `[dependencies]`, replace

```toml
# The runtime the HTTP modules' clients and throttles keep time on. The socket
# modules (`rtds`, `sports`) build none of it until the socket kit moves on to
# it.
polyoxide-rt = { workspace = true, optional = true }
```

with

```toml
# The runtime every module keeps time on: the HTTP clients and throttles, and
# the socket modules' clocks, timers and spawns.
polyoxide-rt = { workspace = true }
```

Then delete the line `    "dep:polyoxide-rt",` from each of the `clob`, `gamma`, `data`, `relay` and `perps` feature lists (a dependency that is not optional cannot be named with `dep:`, and each of those lines is the one Story 2 added directly after `"dep:polyoxide-core",`). `polyoxide-kalshi` needs no manifest change here (its dependency stays optional, under `predictions`); `polyoxide-binance`'s dependency becomes non-optional in its own task, below.

**3b. The kit's connect.** In `polyoxide-ws/src/connect.rs`, replace everything from the top of the file through the end of `connect_over` (the line before the doc comment of `with_timeout`) with

```rust
//! Opening a connection within a timeout (W8), and whether retrying a failed
//! one can help (W7).

use std::{future::Future, time::Duration};

use polyoxide_rt::{default_runtime, Runtime};
#[cfg(not(target_arch = "wasm32"))]
use tokio::net::TcpStream;
#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::client_async_tls;

use crate::{
    default_socket_transport, tungstenite::client::IntoClientRequest, BoxSocket, Kind, WsError,
};

/// How long one connection attempt may take unless a venue says otherwise:
/// the TCP connect, the TLS handshake and the WebSocket upgrade together.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Open a connection through the target's default [`SocketTransport`](crate::SocketTransport)
/// and runtime, giving up after `timeout`.
///
/// `request` is a URL, or a handshake request with its own headers, built
/// afresh for each attempt so any authentication in it is recomputed. A
/// connection not open within `timeout` fails as [`Kind::ConnectTimeout`];
/// every other failure is the transport's, such as [`Kind::Refused`] for an
/// upgrade the server answered with an HTTP status. A caller that injects its
/// own transport or runtime opens its connections through them instead.
pub async fn connect<R>(request: R, timeout: Duration) -> Result<BoxSocket, WsError>
where
    R: IntoClientRequest + Unpin,
{
    let request = request.into_client_request()?;
    default_socket_transport()
        .connect(request, timeout, default_runtime())
        .await
}

/// Upgrade a TCP connection the caller opened itself: the TLS handshake when
/// `request` is a `wss://` URL, then the WebSocket upgrade.
///
/// For a transport whose connect is its own: one that resolves the host and
/// dials each address itself, and hands the first TCP connection that opens
/// to this. The rustls provider is installed first
/// ([`ensure_crypto_provider`](crate::ensure_crypto_provider)). It has no
/// timeout of its own, so bound it with [`with_timeout`]. A refused upgrade
/// fails as [`Kind::Refused`], as it does through [`connect`]. Native only.
#[cfg(not(target_arch = "wasm32"))]
pub async fn connect_over<R>(request: R, tcp: TcpStream) -> Result<BoxSocket, WsError>
where
    R: IntoClientRequest + Unpin,
{
    crate::ensure_crypto_provider();
    let (socket, _response) = client_async_tls(request, tcp).await?;
    Ok(Box::new(socket))
}

```

`with_timeout`, `bounded` and `retrying_can_fix` that follow stay as Task 5.1 left them. In the doc comment of `with_timeout`, replace

```rust
/// The future's own output comes back as it is, so its errors stay the
/// caller's. Install the rustls provider before a TLS connect of its own
/// ([`ensure_crypto_provider`]).
```

with

```rust
/// The future's own output comes back as it is, so its errors stay the
/// caller's. It runs on the target's default runtime; a transport bounds its
/// own steps on the runtime it is given instead. Install the rustls provider
/// before a TLS connect of its own ([`ensure_crypto_provider`]).
```

**3c. The bare tier.** In `polyoxide-ws/src/bare.rs`:

Replace the imports

```rust
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{SinkExt, Stream, StreamExt};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

use crate::{request::Requests, Socket, WsError};
```

with

```rust
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use futures_util::{SinkExt, Stream, StreamExt};
use polyoxide_rt::{default_runtime, Instant, Runtime};
use tokio_tungstenite::tungstenite::Message;

use crate::{request::Requests, BoxSocket, WsError};
```

In the `Connection` struct replace the first fields

```rust
pub struct Connection {
    socket: Socket,
    mode: Mode,
    buffered: VecDeque<Message>,
    last_inbound: Instant,
```

with

```rust
pub struct Connection {
    socket: BoxSocket,
    mode: Mode,
    /// The runtime the connection's clock reads, pacing and timeouts use.
    pub(crate) runtime: Arc<dyn Runtime>,
    buffered: VecDeque<Message>,
    last_inbound: Instant,
```

Replace `new` with `new` and `with_runtime`:

```rust
impl Connection {
    /// Read `socket` in `mode`, on the target's default runtime.
    pub fn new(socket: BoxSocket, mode: Mode) -> Self {
        Self::with_runtime(socket, mode, default_runtime())
    }

    /// Read `socket` in `mode`, on `runtime`: its clock stamps the frames, and
    /// it paces and bounds the connection's requests.
    pub fn with_runtime(socket: BoxSocket, mode: Mode, runtime: Arc<dyn Runtime>) -> Self {
        Self {
            socket,
            mode,
            last_inbound: runtime.now(),
            runtime,
            buffered: VecDeque::new(),
            close: None,
            replying: false,
            finished: false,
            requests: Requests::default(),
        }
    }

    /// The runtime this connection runs on, for a protocol's own timers.
    pub fn runtime(&self) -> &Arc<dyn Runtime> {
        &self.runtime
    }
```

Leave `mode()` as it is. `last_inbound` keeps its body; its return type is now the runtime's `Instant`, which is `std::time::Instant` on native (it was `tokio::time::Instant`). `close` sends a close frame through the sink, which is what tokio-tungstenite's own `close` does. Replace

```rust
        // Sends the close frame and flushes it.
        self.socket.close(None).await?;
        Ok(())
```

with

```rust
        // Sends the close frame and flushes it.
        self.socket.send(Message::Close(None)).await?;
        Ok(())
```

In `flush_reply`, name the sink's item so inference never has to guess for the boxed trait object. Replace

```rust
            if let Err(err) = self.socket.flush().await {
```

with

```rust
            if let Err(err) = SinkExt::<Message>::flush(&mut self.socket).await {
```

Replace `socket()` and `touch()`:

```rust
    /// The underlying socket, for the request module's reads.
    pub(crate) fn socket(&mut self) -> &mut BoxSocket {
        &mut self.socket
    }

    /// Note that a frame arrived now.
    pub(crate) fn touch(&mut self) {
        self.last_inbound = self.runtime.now();
    }
```

In `poll_next`, name the item in the two flushes. Replace

```rust
                match this.socket.poll_flush_unpin(cx) {
```

with

```rust
                match SinkExt::<Message>::poll_flush_unpin(&mut this.socket, cx) {
```

and

```rust
                    let _ = this.socket.poll_flush_unpin(cx);
```

with

```rust
                    let _ = SinkExt::<Message>::poll_flush_unpin(&mut this.socket, cx);
```

**3d. Request correlation.** In `polyoxide-ws/src/request.rs`:

Replace the imports

```rust
use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
```

with

```rust
use std::{future::Future, sync::Arc, time::Duration};

use futures_util::StreamExt;
use polyoxide_rt::{Instant, Runtime};
use tokio_tungstenite::tungstenite::Message;
```

Replace the pacing and the bound inside `request`

```rust
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
```

with

```rust
        let runtime = Arc::clone(&self.runtime);
        if let (Some(last), Some(interval)) =
            (self.requests.last_request, self.requests.min_interval)
        {
            // An interval too long to add is one never reached.
            if let Some(at) = last.checked_add(interval) {
                runtime.sleep_until(at).await;
            }
        }
        let id = self.take_id();
        let timeout = self.requests.answer_timeout;
        within(runtime.as_ref(), timeout, async {
            self.send(build(id)).await?;
            self.requests.last_request = Some(self.runtime.now());
            self.read_until(&mut |message: &Message| answer(id, message))
                .await
        })
        .await
```

In `await_answer`, replace `within(timeout, self.read_until(&mut answer)).await` with

```rust
        let runtime = Arc::clone(&self.runtime);
        within(runtime.as_ref(), timeout, self.read_until(&mut answer)).await
```

(the line `let timeout = self.requests.answer_timeout;` stays above it). Replace `within`:

```rust
/// Run `exchange` within `timeout`, if one is set, failing as `NoAnswer`.
async fn within<A>(
    runtime: &dyn Runtime,
    timeout: Option<Duration>,
    exchange: impl Future<Output = Result<A, WsError>>,
) -> Result<A, WsError> {
    match timeout {
        Some(timeout) => polyoxide_rt::timeout(runtime, timeout, exchange)
            .await
            .ok_or_else(|| WsError::no_answer(timeout))?,
        None => exchange.await,
    }
}
```

Add `take_paced_id` directly after `take_id`, which Story 8's synchronous `Protocol::ping` calls where it cannot await `request`'s wait. Replace

```rust
    pub fn take_id(&mut self) -> u64 {
        let id = self.requests.next_id;
        self.requests.next_id += 1;
        id
    }
```

with

```rust
    pub fn take_id(&mut self) -> u64 {
        let id = self.requests.next_id;
        self.requests.next_id += 1;
        id
    }

    /// Take the next id for a request that is sent now and paced like
    /// [`request`](Self::request), without waiting: for a synchronous hook
    /// such as a protocol's ping, which cannot await.
    ///
    /// `None`, taking no id, while [`min_request_interval`](Self::min_request_interval)
    /// has not passed since the last paced send on the connection's runtime
    /// clock. Otherwise the next id, and now is recorded as the last paced
    /// send, so the next [`request`](Self::request) waits from it. The caller
    /// must send the frame it builds for the id; an id taken and not sent
    /// still delays the next paced request. With no interval set, or one too
    /// long to add (never reached, as for `request`), it always takes the id.
    pub fn take_paced_id(&mut self) -> Option<u64> {
        let now = self.runtime.now();
        let held_back = match (self.requests.last_request, self.requests.min_interval) {
            (Some(last), Some(interval)) => last.checked_add(interval).is_some_and(|at| now < at),
            _ => false,
        };
        if held_back {
            return None;
        }
        self.requests.last_request = Some(now);
        Some(self.take_id())
    }
```

and in `min_request_interval`'s doc comment replace

```rust
    /// Send requests no sooner than `interval` apart. Only
    /// [`request`](Self::request) is paced: a frame sent with
    /// [`send`](Self::send), under an id from [`take_id`](Self::take_id) or
    /// not, neither waits nor counts.
```

with

```rust
    /// Send requests no sooner than `interval` apart. Only
    /// [`request`](Self::request) and [`take_paced_id`](Self::take_paced_id)
    /// are paced: a frame sent with [`send`](Self::send), under an id from
    /// [`take_id`](Self::take_id) or not, neither waits nor counts.
```

The step's new tests, `take_paced_id_waits_out_the_interval_from_the_last_paced_send` and `take_paced_id_without_an_interval_never_refuses`, pin both arms on tokio's paused clock, which `TokioRuntime::now` reads.

**3e. The supervisor.** In `polyoxide-ws/src/supervisor/mod.rs`, replace the imports

```rust
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
use tokio::sync::{mpsc, oneshot};

use crate::{
    tungstenite::{self, handshake::client::Request, Message},
    Connection, Socket, Timing, WsError,
};
```

with

```rust
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
use polyoxide_rt::{default_runtime, Runtime};
use tokio::sync::{mpsc, oneshot};

use crate::{
    default_socket_transport,
    tungstenite::{self, handshake::client::Request, Message},
    BoxSocket, Connection, SocketTransport, Timing, WsError,
};
```

Replace `Protocol::connect` and its doc

```rust
    /// Open the connection for one attempt, within `timeout`.
    ///
    /// The [`Supervisor`] calls it for every attempt, the first and each
    /// reconnect, with [`Timing::connect_timeout`]. By default it builds
    /// [`handshake`](Self::handshake)'s request first, so a failure there is
    /// the attempt's, and dials it with the kit's [`connect`](crate::connect).
    /// A protocol that dials its venue a step at a time, such as a venue that
    /// tries each resolved address in turn, overrides it: it bounds each step
    /// by `timeout` itself, upgrades with [`connect_over`](crate::connect_over),
    /// and may keep `handshake` for the request it upgrades with. An error is a
    /// failed attempt, which [`recover`](Self::recover) decides on.
    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<Socket, Self::Error>> + Send {
        let request = self.handshake();
        async move {
            let socket = crate::connect(request?, timeout).await?;
            Ok::<_, Self::Error>(socket)
        }
    }
```

with

```rust
    /// Open the connection for one attempt, within `timeout`.
    ///
    /// The [`Supervisor`] calls it for every attempt, the first and each
    /// reconnect, with its [`SocketTransport`] and runtime and
    /// [`Timing::connect_timeout`]. By default it builds
    /// [`handshake`](Self::handshake)'s request first, so a failure there is
    /// the attempt's, and dials it through the transport. A protocol that
    /// wants another dial injects a transport that makes it, such as one that
    /// tries each resolved address in turn
    /// ([`TungsteniteTransport::per_address`](crate::TungsteniteTransport::per_address));
    /// it overrides this only to shape the transport's error. An error is a
    /// failed attempt, which [`recover`](Self::recover) decides on.
    fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> impl Future<Output = Result<BoxSocket, Self::Error>> + Send {
        let request = self.handshake();
        async move {
            let socket = transport.connect(request?, timeout, runtime).await?;
            Ok::<_, Self::Error>(socket)
        }
    }
```

Replace the `Supervisor` struct and its `impl` through `spawn`

```rust
#[derive(Debug)]
pub struct Supervisor<P: Protocol> {
    protocol: P,
    timing: Timing,
}

impl<P: Protocol> Supervisor<P> {
    /// A supervisor for `protocol` on `timing`.
    pub fn new(protocol: P, timing: Timing) -> Self {
        Self { protocol, timing }
    }
```

with

```rust
#[derive(Debug)]
pub struct Supervisor<P: Protocol> {
    protocol: P,
    timing: Timing,
    runtime: Option<Arc<dyn Runtime>>,
    transport: Option<Arc<dyn SocketTransport>>,
}

impl<P: Protocol> Supervisor<P> {
    /// A supervisor for `protocol` on `timing`, on the target's default runtime
    /// and socket transport until [`runtime`](Self::runtime) and
    /// [`socket_transport`](Self::socket_transport) say otherwise.
    pub fn new(protocol: P, timing: Timing) -> Self {
        Self {
            protocol,
            timing,
            runtime: None,
            transport: None,
        }
    }

    /// Run the task's timers, clock reads and spawn on `runtime`.
    pub fn runtime(mut self, runtime: Arc<dyn Runtime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Dial every attempt through `transport`.
    pub fn socket_transport(mut self, transport: Arc<dyn SocketTransport>) -> Self {
        self.transport = Some(transport);
        self
    }

    /// The runtime and transport this supervisor runs on: the injected ones,
    /// or the target's defaults.
    fn injected(&self) -> (Arc<dyn Runtime>, Arc<dyn SocketTransport>) {
        (
            self.runtime.clone().unwrap_or_else(default_runtime),
            self.transport
                .clone()
                .unwrap_or_else(default_socket_transport),
        )
    }
```

and `connect`, `start` and `spawn` with

```rust
    pub async fn connect(
        mut self,
        mut membership: P::Membership,
    ) -> Result<Supervised<P>, P::Error> {
        let (runtime, transport) = self.injected();
        let live = task::attempt(
            &mut self.protocol,
            &self.timing,
            &mut membership,
            &transport,
            &runtime,
        )
        .await?;
        Ok(self.spawn(membership, Start::Up(Box::new(live)), runtime, transport))
    }

    /// As [`connect`](Self::connect), except that a first failure
    /// [`Protocol::recover`] would retry starts the stream in an outage: the
    /// protocol's [`disconnected`](Protocol::disconnected) for it, then the
    /// backoff and the attempts that follow. Only a failure it would
    /// [`Stop`](Recovery::Stop) on is returned.
    pub async fn start(mut self, mut membership: P::Membership) -> Result<Supervised<P>, P::Error> {
        let (runtime, transport) = self.injected();
        match task::attempt(
            &mut self.protocol,
            &self.timing,
            &mut membership,
            &transport,
            &runtime,
        )
        .await
        {
            Ok(live) => Ok(self.spawn(membership, Start::Up(Box::new(live)), runtime, transport)),
            Err(err) => match self.protocol.recover(&err) {
                Recovery::Stop => Err(err),
                Recovery::Reconnect | Recovery::Skip => {
                    Ok(self.spawn(membership, Start::Outage(err), runtime, transport))
                }
            },
        }
    }

    fn spawn(
        self,
        membership: P::Membership,
        start: Start<P>,
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn SocketTransport>,
    ) -> Supervised<P> {
        let (events, receiver) = task::queue(self.protocol.queue());
        let (commands, commands_rx) = mpsc::channel(COMMANDS);
        let task = Task::new(
            self.protocol,
            self.timing,
            membership,
            events,
            commands_rx,
            Arc::clone(&runtime),
            transport,
        );
        Supervised {
            events: receiver,
            commands,
            task: polyoxide_rt::spawn(runtime.as_ref(), task.run(start)),
        }
    }
}
```

(keep the doc comment of `connect` above the new `connect`). In `Supervised`, replace the field `task: tokio::task::JoinHandle<bool>,` with `task: polyoxide_rt::Task<bool>,` and in `Supervised::close` replace

```rust
        match task.await {
            Ok(false) => Ok(()),
            Ok(true) | Err(_) => Err(P::stopped()),
        }
```

with

```rust
        match task.await {
            Some(false) => Ok(()),
            Some(true) | None => Err(P::stopped()),
        }
```

In the `Supervisor` doc comment, add a paragraph before "The basis is":

```rust
/// The task runs on the injected [`runtime`](Self::runtime) and dials through
/// the injected [`socket_transport`](Self::socket_transport), each the
/// target's default when none is given.
///
```

**3f. The supervision task.** In `polyoxide-ws/src/supervisor/task.rs`:

Replace the imports

```rust
use std::{
    collections::VecDeque,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::StreamExt;
use tokio::{
    sync::{mpsc, oneshot},
    time::Instant,
};

use super::{Decoded, Offline, Outage, Protocol, Queue, Recovery};
use crate::{Backoff, Connection, Kind, Mode, Timing, WsError};
```

with

```rust
use std::{
    collections::VecDeque,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::StreamExt;
use polyoxide_rt::{timeout, Instant, Runtime};
use tokio::sync::{mpsc, oneshot};

use super::{Decoded, Offline, Outage, Protocol, Queue, Recovery};
use crate::{Backoff, Connection, Kind, Mode, SocketTransport, Timing, WsError};
```

Replace `Clock::new`

```rust
impl Clock {
    fn new() -> Self {
        let now = Instant::now();
```

with

```rust
impl Clock {
    fn new(now: Instant) -> Self {
```

(the struct literal below it, `Self { opened: now, last_ping: now, last_live: now }`, stays). Replace `attempt`:

```rust
pub(super) async fn attempt<P: Protocol>(
    protocol: &mut P,
    timing: &Timing,
    membership: &mut P::Membership,
    transport: &Arc<dyn SocketTransport>,
    runtime: &Arc<dyn Runtime>,
) -> Result<Connection, P::Error> {
    let socket = protocol
        .connect(
            Arc::clone(transport),
            Arc::clone(runtime),
            timing.connect_timeout,
        )
        .await?;
    let mut connection = protocol.configure(Connection::with_runtime(
        socket,
        Mode::All,
        Arc::clone(runtime),
    ));
    protocol.reset();
    match timeout(
        runtime.as_ref(),
        timing.stale_after,
        protocol.replay(&mut connection, membership),
    )
    .await
    {
        Some(replayed) => replayed.map(|()| connection),
        None => Err(P::stalled(timing.stale_after)),
    }
}
```

Replace the three helpers that bound a send, each taking the runtime first:

```rust
async fn ping<P: Protocol>(
    runtime: &dyn Runtime,
    protocol: &mut P,
    timing: &Timing,
    connection: &mut Connection,
) -> Result<(), P::Error> {
    if connection.is_finished() {
        return Ok(());
    }
    let Some(ping) = protocol.ping(connection) else {
        return Ok(());
    };
    match timeout(runtime, timing.stale_after, connection.send(ping)).await {
        Some(sent) => sent.map_err(P::Error::from),
        None => Err(P::stalled(timing.stale_after)),
    }
}
```

```rust
async fn follow_up<P: Protocol>(
    runtime: &dyn Runtime,
    protocol: &mut P,
    timing: &Timing,
    connection: &mut Connection,
) -> Result<(), P::Error> {
    if connection.is_finished() {
        return Ok(());
    }
    let Some(frame) = protocol.follow_up() else {
        return Ok(());
    };
    match timeout(runtime, timing.stale_after, connection.send(frame)).await {
        Some(sent) => sent.map_err(P::Error::from),
        None => Err(P::stalled(timing.stale_after)),
    }
}
```

```rust
/// Send a close frame, but not wait on a peer that has stopped reading.
async fn close_politely(runtime: &dyn Runtime, connection: &mut Connection) {
    let _ = timeout(runtime, CLOSE_WAIT, connection.close()).await;
}
```

(keep each function's doc comment). Replace the `Task` struct and `new` (everything from `pub(super) struct Task<P: Protocol> {` through the end of `fn new`) with

```rust
pub(super) struct Task<P: Protocol> {
    protocol: P,
    timing: Timing,
    membership: P::Membership,
    backoff: Backoff,
    events: Events<Item<P>>,
    commands: mpsc::Receiver<Command<P>>,
    /// Items decoded or declared and not yet queued, oldest first. A full
    /// queue holds them here, and the connection is not read meanwhile.
    pending: VecDeque<Item<P>>,
    /// Changes made during an outage that the protocol held, applied in
    /// order on the next connection.
    held: VecDeque<Command<P>>,
    /// The runtime every wait, clock read and timeout of the task goes
    /// through.
    runtime: Arc<dyn Runtime>,
    /// How each attempt dials.
    transport: Arc<dyn SocketTransport>,
}

impl<P: Protocol> Task<P> {
    pub(super) fn new(
        protocol: P,
        timing: Timing,
        membership: P::Membership,
        events: Events<Item<P>>,
        commands: mpsc::Receiver<Command<P>>,
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn SocketTransport>,
    ) -> Self {
        Self {
            protocol,
            backoff: timing.backoff(),
            timing,
            membership,
            events,
            commands,
            pending: VecDeque::new(),
            held: VecDeque::new(),
            runtime,
            transport,
        }
    }

    /// The time since `at` on the task's runtime.
    fn since(&self, at: Instant) -> Duration {
        self.runtime.now().saturating_duration_since(at)
    }
```

In `run`, the three `close_politely(&mut connection).await;` calls (in `End::Gone`, `End::NotWanted` and `End::Rotation`) become

```rust
                    close_politely(self.runtime.as_ref(), &mut connection).await;
```

In `reconnect`, replace

```rust
            let sleep = tokio::time::sleep(delay);
            tokio::pin!(sleep);
```

with

```rust
            let mut sleep = self.runtime.sleep(delay);
```

and replace

```rust
                attempted = attempt(&mut self.protocol, &self.timing, &mut self.membership) => attempted,
```

with

```rust
                attempted = attempt(
                    &mut self.protocol,
                    &self.timing,
                    &mut self.membership,
                    &self.transport,
                    &self.runtime,
                ) => attempted,
```

(`() = &mut sleep => break,` stays: a boxed future is `Unpin`.) In `pump`, replace the lines

```rust
        let mut clock = Clock::new();
```

with

```rust
        let mut clock = Clock::new(self.runtime.now());
```

and the block from `if let Some(age) = self.timing.max_connection_age {` through the `wait` computation

```rust
            if let Some(age) = self.timing.max_connection_age {
                if !kept && clock.opened.elapsed() >= age {
                    return End::Rotation;
                }
            }
            if let Some(interval) = self.timing.ping_interval {
                if clock.last_ping.elapsed() >= interval {
                    if let Err(err) = ping(&mut self.protocol, &self.timing, connection).await {
                        return End::Failed(err);
                    }
                    clock.last_ping = Instant::now();
                }
            }
            let silent = clock.last_live.elapsed();
            if !kept && silent >= self.timing.stale_after {
                return End::Stale(silent);
            }
            let wait =
                self.timing
                    .next_wait(clock.last_ping.elapsed(), silent, clock.opened.elapsed());
```

with

```rust
            if let Some(age) = self.timing.max_connection_age {
                if !kept && self.since(clock.opened) >= age {
                    return End::Rotation;
                }
            }
            if let Some(interval) = self.timing.ping_interval {
                if self.since(clock.last_ping) >= interval {
                    if let Err(err) = ping(
                        self.runtime.as_ref(),
                        &mut self.protocol,
                        &self.timing,
                        connection,
                    )
                    .await
                    {
                        return End::Failed(err);
                    }
                    clock.last_ping = self.runtime.now();
                }
            }
            let silent = self.since(clock.last_live);
            if !kept && silent >= self.timing.stale_after {
                return End::Stale(silent);
            }
            let wait = self.timing.next_wait(
                self.since(clock.last_ping),
                silent,
                self.since(clock.opened),
            );
```

In the same `select!`, replace the read arm

```rust
                next = tokio::time::timeout(wait, connection.next()) => match next {
                    // A deadline fell due; the top of the loop sees to it.
                    Err(_) => {}
                    Ok(None) => {
```

with

```rust
                next = timeout(self.runtime.as_ref(), wait, connection.next()) => match next {
                    // A deadline fell due; the top of the loop sees to it.
                    None => {}
                    Some(None) => {
```

and the two arms after it, `Ok(Some(Err(err))) => return End::Failed(err.into()),` and `Ok(Some(Ok(frame))) => {`, become `Some(Some(Err(err))) => return End::Failed(err.into()),` and `Some(Some(Ok(frame))) => {`. Inside that arm replace `clock.last_live = Instant::now();` with `clock.last_live = self.runtime.now();` and

```rust
                        let sent = follow_up(&mut self.protocol, &self.timing, connection).await;
```

with

```rust
                        let sent = follow_up(
                            self.runtime.as_ref(),
                            &mut self.protocol,
                            &self.timing,
                            connection,
                        )
                        .await;
```

In `apply`, replace

```rust
            applied = tokio::time::timeout(
                self.timing.stale_after,
                self.protocol.apply(connection, &mut self.membership, change),
            ) => applied,
        };
        match applied {
            Ok(answer) => {
                let _ = reply.send(answer);
            }
            Err(_) => {
                let silent = clock.last_live.elapsed();
```

with

```rust
            applied = timeout(
                self.runtime.as_ref(),
                self.timing.stale_after,
                self.protocol.apply(connection, &mut self.membership, change),
            ) => applied,
        };
        match applied {
            Some(answer) => {
                let _ = reply.send(answer);
            }
            None => {
                let silent = self.since(clock.last_live);
```

In `deliver`, replace `let blocked = Instant::now();` with `let blocked = self.runtime.now();`, `(Some(interval), None) => interval.saturating_sub(clock.last_ping.elapsed()),` with `(Some(interval), None) => interval.saturating_sub(self.since(clock.last_ping)),`, `() = tokio::time::sleep(next_ping) => {` with `() = self.runtime.sleep(next_ping) => {`, the ping call in that arm with

```rust
                        if let Err(err) = ping(
                            self.runtime.as_ref(),
                            &mut self.protocol,
                            &self.timing,
                            connection,
                        )
                        .await
                        {
                            lost = Some(err);
                        }
                        clock.last_ping = self.runtime.now();
```

and `clock.last_live += blocked.elapsed();` with `clock.last_live += self.since(blocked);`.

**3g. The kit's exports.** In `polyoxide-ws/src/lib.rs`, replace

```rust
pub use connect::{
    connect, connect_over, retrying_can_fix, with_timeout, Socket, DEFAULT_CONNECT_TIMEOUT,
};
```

with

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use connect::connect_over;
pub use connect::{connect, retrying_can_fix, with_timeout, DEFAULT_CONNECT_TIMEOUT};
```

**3h. `TungsteniteTransport::per_address`.** The CLOB's dial moves into `polyoxide-ws/src/transport.rs` unchanged in what it does: it resolves every address, interleaves the families (RFC 8305), and bounds the TCP connect and then the upgrade of each address by the connect timeout. Replace the unit struct and its `new`

```rust
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct TungsteniteTransport;

#[cfg(not(target_arch = "wasm32"))]
impl TungsteniteTransport {
    /// The plain dial: tungstenite's own connect, which resolves the host and
    /// connects to the first address it returns.
    pub const fn new() -> Self {
        Self
    }
}
```

with

```rust
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct TungsteniteTransport {
    per_address: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl TungsteniteTransport {
    /// The plain dial: tungstenite's own connect, which resolves the host and
    /// connects to the first address it returns.
    pub const fn new() -> Self {
        Self { per_address: false }
    }

    /// The dial of a host that has several addresses, some of which may not
    /// answer: it resolves every address up front, tries each in
    /// family-interleaved order (IPv6 and IPv4 alternating, RFC 8305 section 4),
    /// and bounds the TCP connect and, separately, the TLS and WebSocket
    /// upgrade of each by the connect timeout, so a blackholed route costs one
    /// timeout and not one per address. It falls back to the next address on
    /// any failure but a refused upgrade, which it returns at once: another
    /// address would repeat the refusal, and its own failure would hide this
    /// one's status and `Retry-After`.
    pub const fn per_address() -> Self {
        Self { per_address: true }
    }
}
```

Replace the `connect` of its `SocketTransport` impl with

```rust
    fn connect(
        &self,
        request: Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        let per_address = self.per_address;
        Box::pin(async move {
            crate::ensure_crypto_provider();
            if per_address {
                return dial_per_address(request, timeout, runtime.as_ref()).await;
            }
            let (socket, _response) = crate::connect::bounded(
                runtime.as_ref(),
                timeout,
                tokio_tungstenite::connect_async(request),
            )
            .await??;
            Ok::<BoxSocket, WsError>(Box::new(socket))
        })
    }
```

and add, below the `impl SocketTransport for TungsteniteTransport` block and above `transport_setters!`:

```rust
/// Interleave IPv6 and IPv4 addresses, Happy-Eyeballs style (RFC 8305 §4).
///
/// A lookup typically returns all AAAA records before all A records, so a
/// network with broken IPv6 would burn one full timeout per v6 address before
/// ever trying v4. Alternating families bounds the damage to a single timeout.
#[cfg(not(target_arch = "wasm32"))]
fn interleave_address_families(addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    let (v6, v4): (Vec<_>, Vec<_>) = addrs.into_iter().partition(SocketAddr::is_ipv6);
    let mut out = Vec::with_capacity(v6.len() + v4.len());
    let (mut v6, mut v4) = (v6.into_iter(), v4.into_iter());
    loop {
        match (v6.next(), v4.next()) {
            (None, None) => return out,
            (a, b) => out.extend(a.into_iter().chain(b)),
        }
    }
}

/// An I/O failure while connecting, as the kit reports a transport's.
#[cfg(not(target_arch = "wasm32"))]
fn io_error(err: std::io::Error) -> WsError {
    WsError::from(tungstenite::Error::Io(err))
}

/// A host that resolved to no address: a DNS miss, so an I/O failure like any
/// other, not a refusal of the request.
#[cfg(not(target_arch = "wasm32"))]
fn no_addresses(uri: &Uri) -> WsError {
    io_error(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("no addresses resolved for {uri}"),
    ))
}

/// What a request names to dial: an address as written, or a name to resolve.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, PartialEq, Eq)]
enum Target<'a> {
    Ip(IpAddr),
    Name(&'a str),
}

/// The host a URI names and the port to dial: the URI's own, or its scheme's
/// default (80 for `ws://`, 443 otherwise). `None` when it has no host.
#[cfg(not(target_arch = "wasm32"))]
fn dial_target(uri: &Uri) -> Option<(Target<'_>, u16)> {
    let host = uri.host()?;
    let port = uri.port_u16().unwrap_or(match uri.scheme_str() {
        Some("ws") => 80,
        _ => 443,
    });
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    Some(match literal.parse::<IpAddr>() {
        Ok(ip) => (Target::Ip(ip), port),
        Err(_) => (Target::Name(host), port),
    })
}

/// Open `request` address by address, within `timeout` for each step.
#[cfg(not(target_arch = "wasm32"))]
async fn dial_per_address(
    request: Request<()>,
    timeout: Duration,
    runtime: &dyn Runtime,
) -> Result<BoxSocket, WsError> {
    let uri = request.uri().clone();
    let (target, port) = dial_target(&uri)
        .ok_or_else(|| WsError::from(tungstenite::Error::Url(UrlError::NoHostName)))?;

    // An address as written needs no lookup; only a name is resolved.
    let addrs: Vec<SocketAddr> = match target {
        Target::Ip(ip) => vec![SocketAddr::new(ip, port)],
        Target::Name(name) => tokio::net::lookup_host((name, port))
            .await
            .map_err(io_error)?
            .collect(),
    };

    let mut last_err = no_addresses(&uri);
    for addr in interleave_address_families(addrs) {
        let stream = match crate::connect::bounded(runtime, timeout, TcpStream::connect(addr))
            .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(err)) => {
                last_err = io_error(err);
                continue;
            }
            Err(err) => {
                last_err = err;
                continue;
            }
        };
        // TCP is up; bound the TLS and WebSocket handshake by the same budget.
        match crate::connect::bounded(
            runtime,
            timeout,
            crate::connect_over(request.clone(), stream),
        )
        .await
        {
            Ok(Ok(socket)) => return Ok(socket),
            // The host answered and refused the upgrade.
            Ok(Err(err)) if matches!(err.kind(), Kind::Refused { .. }) => return Err(err),
            Ok(Err(err)) => last_err = err,
            Err(err) => last_err = err,
        }
    }
    Err(last_err)
}
```

and replace the imports of `transport.rs`

```rust
use std::{fmt, sync::Arc, time::Duration};

use futures_util::{Sink, Stream};
use polyoxide_rt::{BoxFuture, Runtime};

use crate::{
    tungstenite::{self, http::Request, Message},
    WsError,
};
```

with

```rust
use std::{fmt, sync::Arc, time::Duration};
#[cfg(not(target_arch = "wasm32"))]
use std::net::{IpAddr, SocketAddr};

use futures_util::{Sink, Stream};
use polyoxide_rt::{BoxFuture, Runtime};
#[cfg(not(target_arch = "wasm32"))]
use tokio::net::TcpStream;

#[cfg(not(target_arch = "wasm32"))]
use crate::{
    tungstenite::{error::UrlError, http::Uri},
    Kind,
};
use crate::{
    tungstenite::{self, http::Request, Message},
    WsError,
};
```

Add the dial's tests to `transport.rs` as a module beside `tests`. Two of them are the CLOB's `address_families_are_interleaved` and `single_family_lists_pass_through_unchanged`, moved with their bodies, so they live in a module named as it was, `connect_tests`:

```rust
#[cfg(all(test, not(target_arch = "wasm32")))]
mod connect_tests {
    use std::{net::SocketAddr, time::Duration};

    use futures_util::StreamExt;
    use polyoxide_rt::default_runtime;

    use super::*;
    use crate::{
        test_server::{Script, ScriptedServer},
        tungstenite::client::IntoClientRequest,
        Kind,
    };

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn address_families_are_interleaved() {
        // Resolvers return all AAAA then all A; a broken v6 network must not
        // pay one timeout per v6 address before the first v4 attempt.
        let addrs = vec![
            addr("[2001:db8::1]:443"),
            addr("[2001:db8::2]:443"),
            addr("192.0.2.1:443"),
            addr("192.0.2.2:443"),
            addr("192.0.2.3:443"),
        ];
        let out = interleave_address_families(addrs);
        assert_eq!(
            out,
            vec![
                addr("[2001:db8::1]:443"),
                addr("192.0.2.1:443"),
                addr("[2001:db8::2]:443"),
                addr("192.0.2.2:443"),
                addr("192.0.2.3:443"),
            ]
        );
    }

    #[test]
    fn single_family_lists_pass_through_unchanged() {
        let v4 = vec![addr("192.0.2.1:443"), addr("192.0.2.2:443")];
        assert_eq!(interleave_address_families(v4.clone()), v4);
        assert!(interleave_address_families(Vec::new()).is_empty());
    }

    #[test]
    fn a_uri_without_a_port_dials_its_schemes_default() {
        for (uri, port) in [
            ("ws://127.0.0.1/", 80),
            ("wss://127.0.0.1/", 443),
            ("ws://127.0.0.1:9/", 9),
            ("wss://example.com/ws/market", 443),
        ] {
            let parsed: Uri = uri.parse().unwrap();
            let (_, dialled) = dial_target(&parsed).expect(uri);
            assert_eq!(dialled, port, "{uri}");
        }
        assert_eq!(
            dial_target(&"ws://[::1]/".parse().unwrap()),
            Some((Target::Ip(std::net::Ipv6Addr::LOCALHOST.into()), 80))
        );
        assert_eq!(
            dial_target(&"wss://example.com/x".parse().unwrap()),
            Some((Target::Name("example.com"), 443))
        );
    }

    #[test]
    fn a_dns_miss_is_an_io_failure_and_says_what_it_could_not_resolve() {
        let err = no_addresses(
            &"wss://ws.example.invalid/feed"
                .parse()
                .expect("a valid uri"),
        );
        assert_eq!(err.kind(), Kind::Io);
        assert_eq!(
            err.to_string(),
            "IO error: no addresses resolved for wss://ws.example.invalid/feed"
        );
    }

    #[tokio::test]
    async fn per_address_returns_the_upgraded_socket_with_nothing_read() {
        let server = ScriptedServer::start(vec![Script::frames(&["hello"])]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let mut socket = TungsteniteTransport::per_address()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
            .expect("connect");
        let first = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("a frame within the window")
            .expect("the stream has not ended")
            .expect("a frame, not an error");
        assert_eq!(first, Message::Text("hello".into()));
    }

    #[tokio::test]
    async fn per_address_returns_a_refused_upgrade_at_once() {
        let server = ScriptedServer::start(vec![Script::refuse_upgrade(503)]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let Err(err) = TungsteniteTransport::per_address()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
        else {
            panic!("a refused upgrade must not connect");
        };
        assert_eq!(
            err.kind(),
            Kind::Refused {
                status: 503,
                retry_after: None
            }
        );
        assert_eq!(server.connection_count(), 1, "no other address is tried");
    }

    #[tokio::test]
    async fn per_address_bounds_the_handshake_by_the_timeout() {
        let server = ScriptedServer::start(vec![Script::stall()]).await;
        let request = server.url.as_str().into_client_request().unwrap();
        let after = Duration::from_millis(200);
        let Err(err) = TungsteniteTransport::per_address()
            .connect(request, after, default_runtime())
            .await
        else {
            panic!("a stalled handshake must not connect");
        };
        assert_eq!(err.kind(), Kind::ConnectTimeout { after });
    }

    #[tokio::test]
    async fn a_request_naming_no_host_is_a_url_failure_before_any_dial() {
        let request = Request::builder().uri("/feed").body(()).unwrap();
        let Err(err) = TungsteniteTransport::per_address()
            .connect(request, Duration::from_secs(2), default_runtime())
            .await
        else {
            panic!("a request with no host must not connect");
        };
        assert_eq!(err.kind(), Kind::Url);
    }
}
```

**3i. The supervisor's test protocol.** In `polyoxide-ws/src/supervisor/tests.rs`, `Dialer::connect` takes the new parameters and dials through the transport it is handed. Replace

```rust
    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<crate::Socket, TestError>> + Send {
```

with

```rust
    fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> impl Future<Output = Result<BoxSocket, TestError>> + Send {
```

and the line `let socket = crate::connect(request, timeout).await?;` with `let socket = transport.connect(request, timeout, runtime).await?;`. The names come in through the file's `use super::*;`.

**3j. The CLOB's dial.** In `polyoxide-polymarket/src/clob/ws/client.rs`, replace the imports (lines 1–14)

```rust
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_ws::{
    connect_over,
    tungstenite::{self, Message},
    with_timeout, Connection, Kind, Mode, Socket, WsError,
};
use tokio::net::TcpStream;
```

with

```rust
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_rt::{default_runtime, Runtime};
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, Message},
    BoxSocket, Connection, Kind, Mode, SocketTransport, TungsteniteTransport, WsError,
};
```

Delete the functions `interleave_address_families`, `io_error`, `no_addresses` and `dial_target`, and the old `dial`: everything from the doc comment `/// Interleave IPv6 and IPv4 addresses, Happy-Eyeballs style (RFC 8305 §4).` through the closing brace of `dial`, the line before `/// Open a TLS WebSocket connection through [`dial`], read through the kit's`. Put in their place

```rust
/// The transport a CLOB socket dials with unless one was injected: the kit's
/// per-address dial (divergence row D20).
pub(super) fn clob_transport(
    injected: Option<Arc<dyn SocketTransport>>,
) -> Arc<dyn SocketTransport> {
    injected.unwrap_or_else(|| Arc::new(TungsteniteTransport::per_address()))
}

/// Open `url` through `transport`, each step bounded by `connect_timeout`.
///
/// With the default transport that is the CLOB's own connect (divergence row
/// D20), which the kit now holds as `TungsteniteTransport::per_address`:
/// [`polyoxide_ws::connect`] resolves DNS and connects to only the *first*
/// address, and a blackholed route (the classic broken-IPv6 case) burns the
/// whole timeout there. A step that outlived its bound is the CLOB's own
/// `ConnectTimeout`, which names the URL and says the timeout is per address
/// and per step. The bare tier reads the socket through [`connect_ws`]; the
/// supervised sockets return it from [`polyoxide_ws::Protocol::connect`].
pub(super) async fn open_socket(
    url: &str,
    connect_timeout: Duration,
    transport: Arc<dyn SocketTransport>,
    runtime: Arc<dyn Runtime>,
) -> Result<BoxSocket, ClobWsError> {
    let request = url
        .into_client_request()
        .map_err(|err| ClobWsError::Connection(WsError::from(err)))?;
    transport
        .connect(request, connect_timeout, runtime)
        .await
        .map_err(|err| match err.kind() {
            Kind::ConnectTimeout { .. } => ClobWsError::ConnectTimeout {
                url: url.to_string(),
                timeout: connect_timeout,
            },
            _ => ClobWsError::Connection(err),
        })
}

/// Open `url` on the per-address dial and the target's default runtime.
pub(super) async fn dial(url: &str, connect_timeout: Duration) -> Result<BoxSocket, ClobWsError> {
    open_socket(
        url,
        connect_timeout,
        clob_transport(None),
        default_runtime(),
    )
    .await
}
```

`connect_ws` keeps its body (it calls `dial` and reads in `Mode::Data`); add after it the injected twin

```rust
/// As [`connect_ws`], dialling through `transport` and running on `runtime`.
async fn connect_ws_with(
    url: &str,
    connect_timeout: Duration,
    transport: Arc<dyn SocketTransport>,
    runtime: Arc<dyn Runtime>,
) -> Result<Connection, ClobWsError> {
    let socket = open_socket(url, connect_timeout, transport, Arc::clone(&runtime)).await?;
    Ok(Connection::with_runtime(socket, Mode::Data, runtime))
}
```

Give `WebSocketBuilder` the setters and use them. Replace its struct and `new`

```rust
pub struct WebSocketBuilder {
    market_url: String,
    user_url: String,
    ping_interval: Option<Duration>,
    connect_timeout: Duration,
}
```

with

```rust
pub struct WebSocketBuilder {
    market_url: String,
    user_url: String,
    ping_interval: Option<Duration>,
    connect_timeout: Duration,
    runtime: Option<Arc<dyn Runtime>>,
    socket_transport: Option<Arc<dyn SocketTransport>>,
}
```

and in `new` add the two fields after `connect_timeout: DEFAULT_CONNECT_TIMEOUT,`: `runtime: None,` and `socket_transport: None,`. After the `connect_timeout` setter add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the connection's timers and keep-alive run on. By
        /// default the target's.
        runtime;
        /// How each connect dials. By default the kit's per-address dial,
        /// which tries every address the host resolves to.
        socket_transport;
    }

    /// The connection to `url` on this builder's transport and runtime.
    async fn open(&self, url: &str) -> Result<Connection, ClobWsError> {
        connect_ws_with(
            url,
            self.connect_timeout,
            clob_transport(self.socket_transport.clone()),
            self.runtime.clone().unwrap_or_else(default_runtime),
        )
        .await
    }
```

In `WebSocketBuilder::connect_market` replace `let mut ws = connect_ws(&self.market_url, self.connect_timeout).await?;` with `let mut ws = self.open(&self.market_url).await?;` and in `connect_user_subscription` replace `let mut ws = connect_ws(&self.user_url, self.connect_timeout).await?;` with `let mut ws = self.open(&self.user_url).await?;`.

In the tests of this file: delete `address_families_are_interleaved` and `single_family_lists_pass_through_unchanged` and the helper `fn addr` from `mod connect_tests` (they moved), and delete `a_url_without_a_port_dials_its_schemes_default` from `mod dial_tests` (it moved as `a_uri_without_a_port_dials_its_schemes_default`). Replace `a_dns_miss_is_a_network_failure`:

```rust
    #[test]
    fn a_dns_miss_is_a_network_failure() {
        // A host that resolves to no address was an `InvalidMessage`, an
        // `InvalidRequest` that nothing retried, though a DNS miss passes.
        let err = ClobWsError::from(WsError::from(polyoxide_ws::tungstenite::Error::Io(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no addresses resolved for wss://ws-subscriptions-clob.polymarket.com/ws/market",
            ),
        )));
        assert!(matches!(err, ClobWsError::Connection(_)), "{err:?}");
        assert_eq!(err.class(), Class::Network);
        assert!(err.is_retriable());
        assert_eq!(
            err.to_string(),
            "WebSocket connection error: IO error: no addresses resolved for \
             wss://ws-subscriptions-clob.polymarket.com/ws/market"
        );
    }
```

(the remaining `dial_tests` and the others call `dial` and `connect_ws` as they did.)

In `polyoxide-polymarket/src/clob/ws/supervised/market.rs`, replace the imports

```rust
use std::{future::Future, time::Duration};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery, Socket,
    Supervised, Supervisor, Timing, WsError,
};
```

with

```rust
use std::{future::Future, sync::Arc, time::Duration};

use polyoxide_rt::{default_runtime, Runtime};
use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    BoxSocket, Connection, Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery,
    SocketTransport, Supervised, Supervisor, Timing, WsError,
};
```

and `client::{dial, validate_subscription_count},` with `client::{clob_transport, open_socket, validate_subscription_count},`. In `impl Protocol for ClobMarketProtocol`, replace the `handshake` doc and the `connect`

```rust
    /// The upgrade request for the URL. The supervisor does not send it:
    /// `connect` is overridden, and the per-address dial upgrades each
    /// address's connection from the URL itself. The trait requires it.
```

with `/// The upgrade request for the URL, which the supervisor dials through its transport.` and

```rust
    /// clob's per-address dial (D20), where the kit's default dials only
    /// the first address `handshake`'s request names.
    async fn connect(&mut self, timeout: Duration) -> Result<Socket, ClobWsError> {
        dial(&self.url, timeout).await
    }
```

with

```rust
    /// Dials the URL through the supervisor's transport, which the builder
    /// defaults to the per-address dial (D20). It is overridden only to map a
    /// step that outlived its bound to clob's own `ConnectTimeout`, which names
    /// the URL, where the default would return the kit's error wrapped in
    /// `Connection`.
    async fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> Result<BoxSocket, ClobWsError> {
        open_socket(&self.url, timeout, transport, runtime).await
    }
```

In the struct doc of `ClobMarketProtocol`, change "The connection is dialled by the CLOB's own per-address connect (D20)." to "The connection is dialled through the supervisor's transport, by default the kit's per-address connect (D20)." Give `MarketWsBuilder` the fields and setters: in its struct add `runtime: Option<Arc<dyn Runtime>>,` and `socket_transport: Option<Arc<dyn SocketTransport>>,` after `timing: Timing,`; in `new` add `runtime: None, socket_transport: None,` after `timing: default_timing(),`; after the `timing_setters!` block add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the supervision task and its timers run on. By default
        /// the target's.
        runtime;
        /// How each attempt dials. By default the kit's per-address dial,
        /// which tries every address the host resolves to (D20).
        socket_transport;
    }
```

and replace in `connect`

```rust
        Supervisor::new(ClobMarketProtocol::new(self.url, self.options), self.timing)
            .connect(membership)
            .await
```

with

```rust
        Supervisor::new(ClobMarketProtocol::new(self.url, self.options), self.timing)
            .runtime(self.runtime.unwrap_or_else(default_runtime))
            .socket_transport(clob_transport(self.socket_transport))
            .connect(membership)
            .await
```

In the `tests` module of `market.rs` add `use crate::clob::ws::client::dial;` (its tests call `dial` through `use super::*;`, which no longer brings it in).

In `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, replace the imports

```rust
use std::{collections::HashSet, future::Future, time::Duration};

use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request, Message},
    Connection, Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery, Socket,
    Supervisor, Timing, WsError,
};
```

with

```rust
use std::{collections::HashSet, future::Future, sync::Arc, time::Duration};

use polyoxide_rt::{default_runtime, Runtime};
use polyoxide_venue::Classify;
use polyoxide_ws::{
    tungstenite::{client::IntoClientRequest, handshake::client::Request, Message},
    BoxSocket, Connection, Decoded, MembershipHandle, Offline, Outage, Protocol, Queue, Recovery,
    SocketTransport, Supervisor, Timing, WsError,
};
```

and `client::{dial, validate_subscription_count},` with `client::{clob_transport, open_socket, validate_subscription_count},`. Replace the `handshake` doc (the same three lines as in `market.rs`) with `/// The upgrade request for the URL, which the supervisor dials through its transport.` and `connect`

```rust
    /// Dial through clob's per-address connect (D20), so every address of a
    /// blackholed route is tried within the connect timeout per step.
    fn connect(
        &mut self,
        timeout: Duration,
    ) -> impl Future<Output = Result<Socket, ClobWsError>> + Send {
        let url = self.url.clone();
        async move { dial(&url, timeout).await }
    }
```

with

```rust
    /// Dial through the supervisor's transport, which the builder defaults to
    /// the per-address connect (D20), so every address of a blackholed route
    /// is tried within the connect timeout per step. Overridden only to map a
    /// timed-out step to clob's own `ConnectTimeout`.
    fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> impl Future<Output = Result<BoxSocket, ClobWsError>> + Send {
        let url = self.url.clone();
        async move { open_socket(&url, timeout, transport, runtime).await }
    }
```

Give `UserWsBuilder` its fields: after `timing: Timing,` in the struct add `runtime: Option<Arc<dyn Runtime>>,` and `socket_transport: Option<Arc<dyn SocketTransport>>,`; in `new` add `runtime: None, socket_transport: None,` after `timing: default_timing(),`; after the `timing_setters!` block add the same `transport_setters!` invocation as in `market.rs` (docs: "...the supervision task and its timers..." and "...How each attempt dials..."). Replace both `Supervisor::new(self.protocol(credentials), self.timing)` lines (in `connect` and `start`) with `self.supervisor(credentials)` and add the method beside `protocol`:

```rust
    /// The supervisor for a connection with `credentials`, on this builder's
    /// runtime and transport.
    fn supervisor(self, credentials: ApiCredentials) -> Supervisor<ClobUserProtocol> {
        let protocol = self.protocol(credentials);
        Supervisor::new(protocol, self.timing)
            .runtime(self.runtime.unwrap_or_else(default_runtime))
            .socket_transport(clob_transport(self.socket_transport))
    }
```

In the `tests` module of `user.rs` add `use crate::clob::ws::client::dial;`.

In `polyoxide-polymarket/src/clob/client.rs`, the doc of `ClobBuilder::user_socket` gains a sentence naming where the runtime and transport go. Replace

```rust
    /// The builder holds no credential: a stream builds them from the account
    /// when it opens its socket.
    #[cfg(feature = "clob-ws")]
    pub fn user_socket(mut self, socket: crate::clob::ws::UserWsBuilder) -> Self {
```

with

```rust
    /// The builder holds no credential: a stream builds them from the account
    /// when it opens its socket. Its `runtime` and `socket_transport` setters
    /// choose the runtime and the dial of the stream's socket, which does not
    /// follow the client's own runtime.
    #[cfg(feature = "clob-ws")]
    pub fn user_socket(mut self, socket: crate::clob::ws::UserWsBuilder) -> Self {
```

In `polyoxide-polymarket/tests/live_sports.rs`, import `BoxSocket` where `Socket` was (line 33, in the `polyoxide_ws::{ ... Kind, Socket, WsError, }` list) and change `async fn connect_raw() -> Socket {` to `async fn connect_raw() -> BoxSocket {`.

**3k. The removals, the drifted tests, the divergence and the rules text.** Each lands in this commit, because the commit removes and re-types what they name.

In `docs/s3-removals.md`, under `## Removed`, replace `Nothing yet.` when it is still there, and add these entries after whatever is listed: first copy every key `python3 scripts/api_removals.py check --baseline v0.40.0` prints (run it, since cargo-semver-checks cannot see a type alias or a changed signature, it may print none), then these, each a path in backticks with the story and what a consumer uses instead:

```markdown
- `polyoxide_ws::Socket`: Story 5 of the wasm32 plan removed the alias; a connection is a `polyoxide_ws::BoxSocket`, which `connect` and `connect_over` return and `Connection::new` takes.
- `polyoxide_ws::connect`: Story 5 re-typed its return, from `Socket` to `BoxSocket`; it dials through the target's default `SocketTransport` and runtime.
- `polyoxide_ws::connect_over`: Story 5 re-typed its return, from `Socket` to `BoxSocket`, and made it native only.
- `polyoxide_ws::Connection::new`: Story 5 re-typed its `socket` parameter, from `Socket` to `BoxSocket`; `Connection::with_runtime` also takes the runtime.
- `polyoxide_ws::Connection::last_inbound`: Story 5 re-typed its return, from `tokio::time::Instant` to `polyoxide_rt::Instant`, which is `std::time::Instant` on native.
- `polyoxide_ws::Protocol::connect`: Story 5 changed its parameters, from `(timeout)` to `(transport, runtime, timeout)`, and its return, from `Socket` to `BoxSocket`; a protocol that dialled for itself chooses a transport that makes its dial.
```

Test bodies the story changed or moved need `[[drift]]` entries naming DRIFT row `R22` (Story 3 added it). Stage the new and moved files, then ask the script what it sees:

Run: `git add -N polyoxide-ws/src/transport.rs polyoxide-ws/tests/transport_injection.rs`
Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`

It prints each changed or lost item with the sha256 of its diff. Expect `a_dns_miss_is_a_network_failure` in the CLOB's `client.rs` (its body changed) and `a_url_without_a_port_dials_its_schemes_default` (its subject became a URI and it moved with a new name). For each item it names as changed, append to `ci/test-body-diff.toml` an entry in the shape of the existing ones, with the file, item and sha256 exactly as the script prints them:

```toml
[[drift]]
file = "<the baseline file the script prints>"
item = "<the item path the script prints>"
row = "R22"
sha256 = "<the sha256 the script prints>"
```

`address_families_are_interleaved` and `single_family_lists_pass_through_unchanged` moved with their bodies to a module of the same name and are paired by their module path, so they need no entry. An item the script reports as lost that this task did not delete is a mistake: stop and report it. Run the script again and expect it to pass.

In `_bmad-output/specs/spec-venue-extensibility/divergences.md`, replace the row of D20 (line 36, from `| D20 |` to its closing ` |`) with

```markdown
| D20 | Clob's per-address connect: it resolves every address, interleaves the families (RFC 8305) and dials each in turn | `polyoxide_ws::connect` dials only the first address, so a blackholed route (broken IPv6) would spend the whole timeout there. `polyoxide_ws::TungsteniteTransport::per_address()` is the dial, moved into the kit unchanged: each step, the TCP connect and then the TLS and upgrade, is bounded by the connect timeout on the injected runtime, so `connect_timeout` keeps meaning per address and per step; the upgrade goes through `polyoxide_ws::connect_over` (Story 4.9). Clob's builders default their `socket_transport` to it, so the bare tier and both supervised sockets dial each address within `connect_timeout` per step; `ClobMarketProtocol` and `ClobUserProtocol` keep a `Protocol::connect` that only maps a timed-out step to `ClobWsError::ConnectTimeout { url, timeout }` (Epic 7, and the wasm32 plan's Story 5). An injected transport replaces the dial, D20 included. |
```

In `.claude/rules/sockets.md`: replace the first paragraph's clause "builds only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls, and re-exports `tungstenite`" with "builds only `polyoxide-rt`, tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls, and re-exports `tungstenite`"; in "## Blocks" replace "`connect_over` (the upgrade of a TCP connection a venue dialled itself)," with "`connect_over` (native only: the upgrade of a TCP connection a venue dialled itself),"; replace in the supervisor list

```markdown
- a handshake built per attempt; `connect`, the dial of each attempt (by default it dials the
  handshake's request; a venue that tries each address overrides it); `configure` of each fresh
  connection, such as Binance's 200 ms request pacing;
```

with

```markdown
- a handshake built per attempt; `connect`, the dial of each attempt, which gets the supervisor's
  `SocketTransport` and runtime (by default it dials the handshake's request through the
  transport; a venue that tries each address defaults its builder's transport to
  `TungsteniteTransport::per_address()` instead of dialling for itself); `configure` of each
  fresh connection, such as Binance's 200 ms request pacing;
```

replace "Its per-address connect is its own (D20), each step\n    bounded by `with_timeout` and upgraded through `connect_over`;" with "Its per-address connect is\n    `TungsteniteTransport::per_address()` (D20), each step bounded by the connect timeout on the\n    injected runtime and upgraded through `connect_over`;"; in "## TLS needs a nudge" replace "Code that\ncalls `tokio_tungstenite::connect_async` directly must do the same or it panics, so connect\nthrough the kit, and never give a crate its own `ensure_crypto_provider` copy." with "Code that\ncalls `tokio_tungstenite::connect_async` directly must do the same or it panics, so connect\nthrough the kit: `TungsteniteTransport` is the one caller of it, after `ensure_crypto_provider`,\nand no crate gets its own `ensure_crypto_provider` copy."; and add this section after "## Blocks":

```markdown
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
  default transport is `TungsteniteTransport::per_address()`.
- `Supervisor::new` keeps its signature and gains `.runtime(..)` and `.socket_transport(..)`.
  `Connection::new` runs on the default runtime and `Connection::with_runtime` on the one given,
  which `Connection::runtime()` returns for a protocol's own timers.
```

In `.claude/rules/polymarket-clob-ws.md` replace

```markdown
- The connection is dialled by clob's per-address `dial` through `Protocol::connect` (D20), so
  every address of a blackholed route is tried within `connect_timeout` per step under supervision
  too.
```

with

```markdown
- The connection is dialled by the kit's `TungsteniteTransport::per_address()` (D20), the default
  `socket_transport` of the market and user builders and of the bare tier's, so every address of a
  blackholed route is tried within `connect_timeout` per step under supervision too. Their
  `Protocol::connect` only maps a timed-out step to `ClobWsError::ConnectTimeout`. A
  `socket_transport` set on the builder replaces the dial.
```

In `.claude/rules/errors.md`, after the bullet "- No crate matches over `tungstenite::Error`: every socket error is on the one socket table." add

```markdown
- `Kind::Unsupported`, an operation the transport cannot perform (a handshake header or a control
  frame on a transport that has none), is `InvalidRequest` on the socket table and not retriable,
  so a recovery that follows the class stops on it.
```

In `.claude/rules/ci-gates.md`, the bullet that opens "- **The N `[[drift]]` entries:**" lists every `[[drift]]` entry of `ci/test-body-diff.toml` (Stories 3 and 4 already changed its number and added clauses for their R22 entries). Set its number to what `grep -c '^\[\[drift\]\]' ci/test-body-diff.toml` prints once this task's entries are in, and replace the full stop that closes the bullet's list with the clause below, naming exactly the items this task's entries cover (the two the script printed above; drop one the script did not print):

```markdown
; and, for the wasm32 seams' Story 5, the CLOB's `a_dns_miss_is_a_network_failure`, whose body changed with its dial, and `a_url_without_a_port_dials_its_schemes_default`, whose subject became a URI when it moved to the kit (R22).
```

In `CLAUDE.md`, the crate list is generated (Task 5.1 regenerated the `polyoxide-ws` bullet, and `gen_registry.py --write` below does polymarket's), so only the hand-written rule changes. In "Patterns every crate follows", replace the **Sockets** bullet's "never `tokio_tungstenite::connect_async` directly: the kit installs the rustls `CryptoProvider` a direct connect panics without." with "never `tokio_tungstenite::connect_async` directly: the kit's `TungsteniteTransport`, the native default `SocketTransport`, is its one caller, and installs the rustls `CryptoProvider` a direct connect panics without. A builder's `runtime(..)` and `socket_transport(..)` replace the runtime and the dial."

Regenerate the dependency allowlists (polymarket's `clob-ws` no longer builds tokio's `net`) and the registry, and check the diff:

Run: `nix develop --command python3 scripts/gen_registry.py --write`
Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`
Run: `git diff --stat ci/dep-allowlists/`
Expected: only `polyoxide-polymarket` files change, and only by `polyoxide-rt` joining their trees and `tokio`'s `net` leaving `@clob-ws`'s.

**3l. The README.** In `polyoxide-ws/README.md`, the list of the kit's blocks gains the transport seam. Replace

```text
  `with_timeout` bounds a connect of the caller's own, and
  `retrying_can_fix` says whether a reconnect can help after one failed, by a
  status rule the caller passes in.
```

with

```text
  `with_timeout` bounds a connect of the caller's own, and
  `retrying_can_fix` says whether a reconnect can help after one failed, by a
  status rule the caller passes in.
- The transport seam decides how a connection is dialled: `SocketTransport` is
  the dial, `BoxSocket` the connection it returns, and `Capabilities` says
  what the transport's sockets can do (send pings, see the server's pings,
  carry handshake headers). `TungsteniteTransport` is the native dial and
  `default_socket_transport` the target's choice. `transport_setters!` gives a
  builder its `runtime` and `socket_transport` setters, `Connection::with_runtime`
  and the `Supervisor`'s `runtime` and `socket_transport` setters put a
  connection's and a supervisor's clocks, timers and spawns on the injected
  runtime and its dial on the injected transport, and a transport that cannot
  do what a caller asks fails as `Kind::Unsupported`.
```

The block is prose: the README is compiled as a doctest, and a fence would be compiled with it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`
Run: `nix develop --command cargo test -p polyoxide-ws --all-features`
Expected: `test result: ok.` for every target, `transport_injection`'s four tests (the supervisor's two and the two of `take_paced_id`) and `connect_tests` among them.
Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features`
Expected: `test result: ok.` for every target; the CLOB's `dial_tests`, `a_hung_handshake_times_out_after_one_steps_timeout` and `the_supervisor_dials_with_clobs_per_address_connect` pass unchanged.
Run: `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: no warnings. A caller this task missed fails to compile here on `Socket`, `connect_over`'s or `connect`'s old type or `Protocol::connect`'s old signature: fix it the way the call sites above are fixed and name it in the commit.
Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`, `nix develop --command python3 scripts/api_removals.py check --baseline v0.40.0`, `nix develop --command python3 scripts/venue_gate.py`, `nix develop --command python3 scripts/gen_registry.py --check`
Expected: all exit 0.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-ws/tests/transport_injection.rs
git commit -m "feat(ws)!: BoxSocket, and the kit and the CLOB's dial on the injected runtime and transport (wasm32 story 5)

connect, connect_over, Connection::new and Protocol::connect are re-typed and
Socket is removed. Connection::with_runtime, Supervisor::runtime and
socket_transport put the kit's clocks and spawns on the runtime, and
Connection::take_paced_id takes a paced id without waiting. The CLOB's
per-address dial moves into TungsteniteTransport::per_address() with its
tests, and the CLOB's builders default to it. Adds the removals lines and the
R22 drift entries. The allowlists are regenerated: clob-ws no longer builds
tokio's net, and polymarket depends on polyoxide-rt." -- polyoxide-ws/src/transport.rs polyoxide-ws/src/connect.rs polyoxide-ws/src/bare.rs polyoxide-ws/src/request.rs polyoxide-ws/src/lib.rs polyoxide-ws/README.md polyoxide-ws/src/supervisor/mod.rs polyoxide-ws/src/supervisor/task.rs polyoxide-ws/src/supervisor/tests.rs polyoxide-ws/tests/transport_injection.rs polyoxide-polymarket/Cargo.toml polyoxide-polymarket/src/clob/client.rs polyoxide-polymarket/src/clob/ws/client.rs polyoxide-polymarket/src/clob/ws/supervised/market.rs polyoxide-polymarket/src/clob/ws/supervised/user.rs polyoxide-polymarket/tests/live_sports.rs docs/s3-removals.md ci/test-body-diff.toml _bmad-output/specs/spec-venue-extensibility/divergences.md .claude/rules/sockets.md .claude/rules/polymarket-clob-ws.md .claude/rules/errors.md .claude/rules/ci-gates.md CLAUDE.md docs/ARCHITECTURE.md Cargo.lock ci/dep-allowlists
```

### Task 5.3: Polymarket's perps, rtds, sports and bare CLOB sockets on the runtime

**Files:**
- Modify: `polyoxide-polymarket/src/clob/ws/client.rs:803-825`
- Modify: `polyoxide-polymarket/src/perps/ws/builder.rs:1-91`
- Modify: `polyoxide-polymarket/src/perps/ws/protocol.rs:204-215`
- Modify: `polyoxide-polymarket/src/rtds/client.rs:1-12,60-100`
- Modify: `polyoxide-polymarket/src/rtds/supervisor.rs:8-20,32-60,106-290`
- Modify: `polyoxide-polymarket/src/sports/client.rs:1-37`
- Modify: `polyoxide-polymarket/src/sports/supervised.rs:1-20,72-176,209-226,228-282,309-351`
- Modify: `docs/MUTANTS.md:156`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module of `polyoxide-polymarket/src/sports/supervised.rs` (it has `use super::*;`) a test that a staleness timer runs on the injected runtime. A runtime on which an hour is already over makes a feed that is merely quiet stale at once, which tokio's own timer, set to an hour, would not; every other wait, the connect's among them, is tokio's:

```rust
    /// A runtime on which an hour is over at once; every other wait is tokio's.
    #[derive(Debug, Default)]
    struct HourIsOver(polyoxide_rt::TokioRuntime);

    impl Runtime for HourIsOver {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.0.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            if duration == Duration::from_secs(3600) {
                Box::pin(std::future::ready(()))
            } else {
                self.0.sleep(duration)
            }
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.0.now()
        }
    }

    #[tokio::test]
    async fn the_staleness_window_is_the_injected_runtimes_timer() {
        use polyoxide_ws::test_server::{Script, ScriptedServer};

        let server = ScriptedServer::start(vec![Script::hold()]).await;
        let mut feed = SportsWsBuilder::new()
            .url(server.url.clone())
            .stale_after(Duration::from_secs(3600))
            .runtime(Arc::new(HourIsOver::default()))
            .connect()
            .await
            .unwrap();
        let item = tokio::time::timeout(Duration::from_secs(3), feed.next())
            .await
            .expect("a quiet feed on a spent hour is stale at once")
            .expect("the feed is open");
        assert!(
            matches!(
                item,
                Ok(Event::Disconnected {
                    reason: SportsError::Stale { .. }
                })
            ),
            "{item:?}"
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib the_staleness_window_is_the_injected_runtimes_timer`
Expected: a compile error, ``no method named `runtime` found for struct `SportsWsBuilder` ``.

- [ ] **Step 3: Implement**

**The CLOB's bare tier.** In `polyoxide-polymarket/src/clob/ws/client.rs`, `WebSocketWithPing::run` paces its ping on the connection's runtime. Replace

```rust
        // Not `interval`: it panics on a zero period, and on `Duration::MAX`
        // once a tick is late. `sleep` falls back to a far-future deadline.
        let period = self.ping_interval.max(Duration::from_millis(1));
        // The first ping goes out at once.
        let ping = tokio::time::sleep(Duration::ZERO);
        tokio::pin!(ping);

        loop {
            tokio::select! {
                () = &mut ping => {
                    ping.set(tokio::time::sleep(period));
```

with

```rust
        // Not `interval`: it panics on a zero period, and on `Duration::MAX`
        // once a tick is late. A runtime's `sleep` takes any duration.
        let runtime = Arc::clone(self.inner.runtime());
        let period = self.ping_interval.max(Duration::from_millis(1));
        // The first ping goes out at once.
        let mut ping = runtime.sleep(Duration::ZERO);

        loop {
            tokio::select! {
                () = &mut ping => {
                    ping = runtime.sleep(period);
```

**perps.** In `polyoxide-polymarket/src/perps/ws/builder.rs` replace the imports and the struct

```rust
use polyoxide_ws::{Supervisor, Timing};

use crate::perps::ws::{
    channel::Channel, error::PerpsWsError, protocol::PerpsProtocol, SupervisedPerpsWs, WS_URL,
};

/// Builder for a supervised connection.
#[derive(Debug, Clone)]
pub struct PerpsWsBuilder {
    url: String,
    timing: Timing,
}
```

with

```rust
use std::sync::Arc;

use polyoxide_rt::{default_runtime, Runtime};
use polyoxide_ws::{default_socket_transport, SocketTransport, Supervisor, Timing};

use crate::perps::ws::{
    channel::Channel, error::PerpsWsError, protocol::PerpsProtocol, SupervisedPerpsWs, WS_URL,
};

/// Builder for a supervised connection.
#[derive(Debug, Clone)]
pub struct PerpsWsBuilder {
    url: String,
    timing: Timing,
    runtime: Option<Arc<dyn Runtime>>,
    socket_transport: Option<Arc<dyn SocketTransport>>,
}
```

in `new` add `runtime: None, socket_transport: None,` after `timing: Timing::default(),`; after the `timing_setters!` block add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the supervision task, its pings and the retries of a
        /// rate-limited subscribe run on. By default the target's.
        runtime;
        /// How each attempt dials. By default the target's.
        socket_transport;
    }
```

and in `connect` replace

```rust
        Supervisor::new(PerpsProtocol::new(self.url, self.timing), self.timing)
            .connect(channels)
            .await
```

with

```rust
        Supervisor::new(PerpsProtocol::new(self.url, self.timing), self.timing)
            .runtime(self.runtime.unwrap_or_else(default_runtime))
            .socket_transport(
                self.socket_transport
                    .unwrap_or_else(default_socket_transport),
            )
            .connect(channels)
            .await
```

In `polyoxide-polymarket/src/perps/ws/protocol.rs`, the retry of a rate-limited subscribe sleeps on the connection's runtime. Replace `tokio::time::sleep(delay).await;` with `connection.runtime().sleep(delay).await;`.

**rtds.** In `polyoxide-polymarket/src/rtds/client.rs`, replace the import

```rust
use polyoxide_ws::{connect, tungstenite::Message, Connection, Mode, DEFAULT_CONNECT_TIMEOUT};
```

with

```rust
use std::sync::Arc;

use polyoxide_rt::{default_runtime, Runtime};
use polyoxide_ws::{
    default_socket_transport,
    tungstenite::{client::IntoClientRequest, Message},
    Connection, Mode, SocketTransport, WsError, DEFAULT_CONNECT_TIMEOUT,
};
```

and replace `open` with `open` and `open_with`:

```rust
    /// Connect to `url` within `connect_timeout`, through the target's default
    /// transport and runtime, and subscribe.
    pub(crate) async fn open(
        url: &str,
        subscriptions: impl IntoIterator<Item = Subscription>,
        connect_timeout: Duration,
    ) -> Result<Self, RtdsError> {
        Self::open_with(
            url,
            subscriptions,
            connect_timeout,
            default_socket_transport(),
            default_runtime(),
        )
        .await
    }

    /// Connect to `url` within `connect_timeout`, through `transport`, which
    /// installs the TLS provider when it is the native one, on `runtime`, and
    /// subscribe.
    pub(crate) async fn open_with(
        url: &str,
        subscriptions: impl IntoIterator<Item = Subscription>,
        connect_timeout: Duration,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
    ) -> Result<Self, RtdsError> {
        let request = SubscriptionRequest::new(subscriptions);
        validate_subscriptions(request.subscriptions())?;

        let handshake = url.into_client_request().map_err(WsError::from)?;
        let socket = transport
            .connect(handshake, connect_timeout, Arc::clone(&runtime))
            .await?;
        let mut inner = Connection::with_runtime(socket, Mode::Data, runtime);
        let frame = serde_json::to_string(&request)
            .map_err(|e| RtdsError::json(format!("{request:?}"), e))?;
        inner.send(Message::Text(frame.into())).await?;

        Ok(Self {
            inner,
            subscriptions: request.subscriptions().to_vec(),
        })
    }
```

In `polyoxide-polymarket/src/rtds/supervisor.rs`: replace the imports

```rust
use std::{future::Future, time::Duration};

use futures_util::StreamExt;
use polyoxide_venue::Classify;
use polyoxide_ws::{Backoff, DEFAULT_CONNECT_TIMEOUT};
use tokio::time::{timeout, Instant};
```

with

```rust
use std::{future::Future, sync::Arc, time::Duration};

use futures_util::StreamExt;
use polyoxide_rt::{default_runtime, timeout, Instant, Runtime};
use polyoxide_venue::Classify;
use polyoxide_ws::{default_socket_transport, Backoff, SocketTransport, DEFAULT_CONNECT_TIMEOUT};
```

add the two fields to `RtdsBuilder` after `connect_timeout: Duration,` (`runtime: Option<Arc<dyn Runtime>>,` and `socket_transport: Option<Arc<dyn SocketTransport>>,`) and `runtime: None, socket_transport: None,` to `new` after `connect_timeout: DEFAULT_CONNECT_TIMEOUT,`. After the `connect_timeout` setter add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the stream's timers, keep-alive and reconnect waits run
        /// on. By default the target's.
        runtime;
        /// How each connect dials. By default the target's.
        socket_transport;
    }
```

Replace `connect`:

```rust
    pub async fn connect(
        self,
        subscriptions: impl IntoIterator<Item = Subscription>,
    ) -> Result<SupervisedRtds, RtdsError> {
        let subscriptions: Vec<_> = subscriptions.into_iter().collect();
        let runtime = self.runtime.clone().unwrap_or_else(default_runtime);
        let transport = self
            .socket_transport
            .clone()
            .unwrap_or_else(default_socket_transport);
        let stream = Rtds::open_with(
            &self.url,
            subscriptions.clone(),
            self.connect_timeout,
            Arc::clone(&transport),
            Arc::clone(&runtime),
        )
        .await?;
        Ok(SupervisedRtds {
            config: self,
            subscriptions,
            stream,
            delivered: false,
            runtime,
            transport,
        })
    }
```

Add two fields to `SupervisedRtds` after `delivered: bool,`:

```rust
    /// The runtime its waits and clock reads go through.
    runtime: Arc<dyn Runtime>,
    /// How a reconnect dials.
    transport: Arc<dyn SocketTransport>,
```

and a helper at the top of `impl SupervisedRtds`:

```rust
    /// The time since `at` on this connection's runtime.
    fn since(&self, at: Instant) -> Duration {
        self.runtime.now().saturating_duration_since(at)
    }

```

In `run`, replace `tokio::time::sleep(delay).await;` with `self.runtime.sleep(delay).await;` and the reconnect's

```rust
                        let attempt = Rtds::open(
                            &self.config.url,
                            self.subscriptions.clone(),
                            self.config.connect_timeout,
                        );
```

with

```rust
                        let attempt = Rtds::open_with(
                            &self.config.url,
                            self.subscriptions.clone(),
                            self.config.connect_timeout,
                            Arc::clone(&self.transport),
                            Arc::clone(&self.runtime),
                        );
```

Replace the body of `pump` from `let mut last_frame = Instant::now();` to the end of the `loop`:

```rust
        let mut last_frame = self.runtime.now();
        let mut last_ping = self.runtime.now();

        loop {
            match timeout(self.runtime.as_ref(), tick, self.stream.next()).await {
                // No frame this tick. Check for death first, then keep alive.
                None => {
                    let elapsed = self.since(last_frame);
                    if elapsed >= self.config.stale_after {
                        return Err(RtdsError::Stalled { elapsed });
                    }
                    if self.since(last_ping) >= self.config.ping_interval {
                        self.stream.ping().await?;
                        last_ping = self.runtime.now();
                    }
                }
                Some(None) => return Err(RtdsError::ConnectionClosed),
                // A bad frame is not a bad connection. Surface it and keep
                // reading, or one unparseable message ends a 24/7 feed.
                Some(Some(Err(err))) if err.recovery() == Recovery::SkipFrame => {
                    last_frame = self.runtime.now();
                    tracing::warn!(%err, "skipping an RTDS frame this client could not read");
                }
                Some(Some(Err(err))) => return Err(err),
                Some(Some(Ok(event))) => {
                    last_frame = self.runtime.now();
                    self.delivered = true;
                    handler(event).await?;
                }
            }
        }
```

**sports.** In `polyoxide-polymarket/src/sports/client.rs` replace the imports and `open`:

```rust
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_rt::{default_runtime, Runtime};
pub(crate) use polyoxide_ws::DEFAULT_CONNECT_TIMEOUT;
use polyoxide_ws::{
    default_socket_transport,
    tungstenite::{client::IntoClientRequest, protocol::CloseFrame, Message},
    Connection, Mode, SocketTransport, WsError,
};
```

```rust
/// Open one connection through the target's default transport and runtime,
/// bounded by `connect_timeout`, and read it through the kit's [`Connection`].
///
/// Every frame is read, pings included, since they are the feed's liveness
/// signal (D5), and the server's close is answered with the RFC 6455 reply
/// before the stream yields it. Takes the URL by value so the supervised tier
/// can box the future.
pub(crate) async fn open(
    url: String,
    connect_timeout: Duration,
) -> Result<Connection, SportsError> {
    open_with(
        url,
        connect_timeout,
        default_socket_transport(),
        default_runtime(),
    )
    .await
}

/// As [`open`], through `transport` and on `runtime`.
pub(crate) async fn open_with(
    url: String,
    connect_timeout: Duration,
    transport: Arc<dyn SocketTransport>,
    runtime: Arc<dyn Runtime>,
) -> Result<Connection, SportsError> {
    let request = url
        .as_str()
        .into_client_request()
        .map_err(|err| SportsError::connect(WsError::from(err)))?;
    let socket = transport
        .connect(request, connect_timeout, Arc::clone(&runtime))
        .await
        .map_err(SportsError::connect)?;
    Ok(Connection::with_runtime(socket, Mode::All, runtime))
}
```

In `polyoxide-polymarket/src/sports/supervised.rs` replace the imports

```rust
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{Backoff, Connection, Kind};
use tokio::time::{sleep, Instant, Sleep};

use crate::sports::{
    client::{classify, closed, open, Inbound, DEFAULT_CONNECT_TIMEOUT, SPORTS_WS_URL},
```

with

```rust
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use polyoxide_rt::{default_runtime, BoxFuture, Runtime};
use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{default_socket_transport, Backoff, Connection, Kind, SocketTransport};

use crate::sports::{
    client::{classify, closed, open_with, Inbound, DEFAULT_CONNECT_TIMEOUT, SPORTS_WS_URL},
```

Add `runtime: Option<Arc<dyn Runtime>>,` and `socket_transport: Option<Arc<dyn SocketTransport>>,` to `SportsWsBuilder` after `connect_timeout: Duration,` and `runtime: None, socket_transport: None,` to `new` after `connect_timeout: DEFAULT_CONNECT_TIMEOUT,`; after the `connect_timeout` setter add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the feed's staleness timer and reconnect waits run on.
        /// By default the target's.
        runtime;
        /// How each connect dials. By default the target's.
        socket_transport;
    }
```

Replace `connect`:

```rust
    pub async fn connect(self) -> Result<SupervisedSportsWs, SportsError> {
        let runtime = self.runtime.clone().unwrap_or_else(default_runtime);
        let transport = self
            .socket_transport
            .clone()
            .unwrap_or_else(default_socket_transport);
        let connection = open_with(
            self.url.clone(),
            self.connect_timeout,
            Arc::clone(&transport),
            Arc::clone(&runtime),
        )
        .await?;
        Ok(SupervisedSportsWs {
            backoff: Backoff::new(self.initial_backoff, self.max_backoff),
            state: State::reading(Box::new(connection), self.stale_after),
            config: self,
            runtime,
            transport,
        })
    }
```

Replace the state's timers and `State::reading`:

```rust
enum State {
    /// Reading an open connection.
    Reading {
        connection: Box<Connection>,
        stale: BoxFuture<()>,
        received: bool,
    },
    /// Waiting out a backoff delay.
    Waiting(BoxFuture<()>),
    /// A connection attempt in flight.
    Connecting(Attempt),
    /// A reconnect was refused for good. The stream yields nothing more.
    Ended,
}

impl State {
    fn reading(connection: Box<Connection>, stale_after: Duration) -> Self {
        let stale = connection.runtime().sleep(stale_after);
        State::Reading {
            connection,
            stale,
            received: false,
        }
    }
}
```

Add two fields to `SupervisedSportsWs` after `state: State,`

```rust
    /// The runtime its backoff waits go through.
    runtime: Arc<dyn Runtime>,
    /// How a reconnect dials.
    transport: Arc<dyn SocketTransport>,
```

and in `lose` replace `self.state = State::Waiting(Box::pin(sleep(delay)));` with `self.state = State::Waiting(self.runtime.sleep(delay));`. In `poll_next` replace the `Step::Attempt` arm

```rust
                Step::Attempt => {
                    this.state = State::Connecting(Box::pin(open(
                        this.config.url.clone(),
                        this.config.connect_timeout,
                    )));
                }
```

with

```rust
                Step::Attempt => {
                    this.state = State::Connecting(Box::pin(open_with(
                        this.config.url.clone(),
                        this.config.connect_timeout,
                        Arc::clone(&this.transport),
                        Arc::clone(&this.runtime),
                    )));
                }
```

and in `Step::AttemptFailed(error)`'s last arm replace `this.state = State::Waiting(Box::pin(sleep(delay)));` with `this.state = State::Waiting(this.runtime.sleep(delay));`. In `poll_reading` change the parameter `stale: &mut Pin<Box<Sleep>>,` to `stale: &mut BoxFuture<()>,` and replace

```rust
            // A limit too large to add, such as `Duration::MAX`, means never
            // stale: the timer `sleep` armed lies at its far-future fallback.
            if let Some(deadline) = Instant::now().checked_add(stale_after) {
                stale.as_mut().reset(deadline);
            }
```

with

```rust
            // A limit too large to add, such as `Duration::MAX`, means never
            // stale: a runtime's `sleep` takes any duration.
            *stale = connection.runtime().sleep(stale_after);
```

**The mutants table.** The moved tests change two of the lines `docs/MUTANTS.md` cites in row (b). Find where they are now:

Run: `grep -n 'drift_r11' polyoxide-polymarket/src/sports/supervised.rs polyoxide-polymarket/src/perps/ws/builder.rs`

In the row of `docs/MUTANTS.md` line 156, replace `polyoxide-polymarket/src/sports/supervised.rs:419` and `polyoxide-polymarket/src/perps/ws/builder.rs:710` with the lines the grep prints for `a_reconnect_waits_out_a_refusals_retry_after_drift_r11` and `a_refusal_on_reconnect_waits_out_its_retry_after_drift_r11`. Re-prove the row's mutant on those two tests: change `polyoxide-ws/src/backoff.rs:84`'s `self.take().max(floor.min(self.max))` to `self.take()`, run `nix develop --command cargo test -p polyoxide-polymarket --all-features --lib -- drift_r11` and see both fail, then restore the line and see them pass.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`
Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features`
Expected: `test result: ok.` for every target, `the_staleness_window_is_the_injected_runtimes_timer` among them.
Run: `nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings`
Expected: no warnings.
Run: `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): the perps, rtds, sports and bare CLOB sockets on the injected runtime (wasm32 story 5)

Each socket builder gets runtime(..) and socket_transport(..); every timer,
clock read and sleep of these modules goes through the runtime. Moves the two
MUTANTS lines the edits shifted." -- polyoxide-polymarket/src/clob/ws/client.rs polyoxide-polymarket/src/perps/ws/builder.rs polyoxide-polymarket/src/perps/ws/protocol.rs polyoxide-polymarket/src/rtds/client.rs polyoxide-polymarket/src/rtds/supervisor.rs polyoxide-polymarket/src/sports/client.rs polyoxide-polymarket/src/sports/supervised.rs docs/MUTANTS.md
```

### Task 5.4: Binance's router, forwarders and bare tier on the runtime

**Files:**
- Modify: `polyoxide-binance/Cargo.toml`
- Modify: `polyoxide-binance/src/usdm/ws/supervised.rs:8-27,107-132,134-247,382-441,467-504,672-748`
- Modify: `polyoxide-binance/src/usdm/ws/client.rs:1-14,276-290,354-360`
- Modify (regenerated): `Cargo.lock`, `ci/dep-allowlists/`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module of `polyoxide-binance/src/usdm/ws/supervised.rs` (it has `use super::*;`) a test that the router's tasks are spawned on the injected runtime. A runtime that counts spawns proves the router and a forwarder reach it:

```rust
    /// A runtime that counts its spawns and runs them on tokio's.
    #[derive(Debug, Default)]
    struct CountingRuntime {
        inner: polyoxide_rt::TokioRuntime,
        spawns: std::sync::atomic::AtomicUsize,
    }

    impl Runtime for CountingRuntime {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.spawns
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.inner.spawn(task);
        }

        fn sleep(&self, duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            self.inner.sleep(duration)
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.inner.now()
        }
    }

    #[tokio::test]
    async fn the_router_and_each_path_are_spawned_on_the_injected_runtime() {
        use crate::usdm::ws::scripted::{Script, ScriptedServer};

        let server = ScriptedServer::start(vec![Script::default()]).await;
        let runtime = std::sync::Arc::new(CountingRuntime::default());
        let btc = crate::usdm::types::Symbol::new("BTCUSDT").unwrap();
        let feed = UsdmWsBuilder::new()
            .base_url(server.url.clone())
            .runtime(runtime.clone())
            .streams([StreamName::MarkPrice(btc)])
            .connect()
            .await
            .expect("connect");
        // One spawn for the path's supervision task, one for its forwarder and
        // one for the router.
        assert_eq!(
            runtime.spawns.load(std::sync::atomic::Ordering::SeqCst),
            3
        );
        feed.close().await.expect("close");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features --lib the_router_and_each_path_are_spawned_on_the_injected_runtime`
Expected: a compile error, ``no method named `runtime` found for struct `UsdmWsBuilder` ``.

- [ ] **Step 3: Implement**

`polyoxide-binance/Cargo.toml`: the router no longer needs tokio's `rt`. Replace

```toml
usdm-ws = ["usdm", "dep:polyoxide-ws", "dep:futures-util", "tokio/rt", "tokio/macros"]
```

with

```toml
usdm-ws = ["usdm", "dep:polyoxide-ws", "dep:futures-util", "tokio/macros"]
```

Run `grep -n 'polyoxide-rt' polyoxide-binance/Cargo.toml`: when Story 2 declared the dependency with `optional = true`, delete that key and every `dep:polyoxide-rt` it added to a feature list; when it is absent, add `polyoxide-rt = { workspace = true }` under `polyoxide-venue = { workspace = true }`.

`polyoxide-binance/src/usdm/ws/supervised.rs`: replace the imports

```rust
use futures_util::{Stream, StreamExt};
use polyoxide_ws::{MembershipHandle as PathHandle, Supervised, Supervisor, Timing};
use tokio::sync::{mpsc, oneshot, watch};
```

with

```rust
use futures_util::{Stream, StreamExt};
use polyoxide_rt::{default_runtime, spawn, Runtime, Task};
use polyoxide_ws::{
    default_socket_transport, MembershipHandle as PathHandle, SocketTransport, Supervised,
    Supervisor, Timing,
};
use tokio::sync::{mpsc, oneshot, watch};
```

Add the two fields to `UsdmWsBuilder` after `streams: Vec<StreamName>,` (`runtime: Option<Arc<dyn Runtime>>,` and `socket_transport: Option<Arc<dyn SocketTransport>>,`) and `runtime: None, socket_transport: None,` after `streams: Vec::new(),` in `new`. After the `timing_setters!` block add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the paths' supervision tasks, the router and its
        /// forwarders run on. By default the target's.
        runtime;
        /// How each path's connection dials. By default the target's.
        socket_transport;
    }
```

In `connect`, resolve the defaults first. Replace the start

```rust
    pub async fn connect(self) -> Result<SupervisedUsdmWs, UsdmWsError> {
        let mut membership: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
```

with

```rust
    pub async fn connect(mut self) -> Result<SupervisedUsdmWs, UsdmWsError> {
        // Resolved once, so every path's supervisor and the router share them.
        let runtime = Arc::clone(self.runtime.get_or_insert_with(default_runtime));
        self.socket_transport
            .get_or_insert_with(default_socket_transport);
        let mut membership: HashMap<StreamPath, Vec<StreamName>> = HashMap::new();
```

and replace the router's construction and the spawn

```rust
        let mut router = Router {
            config: self,
            paths: HashMap::new(),
```

with

```rust
        let mut router = Router {
            config: self,
            runtime: Arc::clone(&runtime),
            paths: HashMap::new(),
```

and `let task = tokio::spawn(route(router, commands_rx, ended_rx));` with `let task = spawn(runtime.as_ref(), route(router, commands_rx, ended_rx));`. Replace `supervisor`:

```rust
    /// The supervisor for one path's connection.
    fn supervisor(&self, path: StreamPath) -> Supervisor<UsdmProtocol> {
        let supervisor =
            Supervisor::new(UsdmProtocol::new(self.base_url.clone(), path), self.timing);
        let supervisor = match &self.runtime {
            Some(runtime) => supervisor.runtime(Arc::clone(runtime)),
            None => supervisor,
        };
        match &self.socket_transport {
            Some(transport) => supervisor.socket_transport(Arc::clone(transport)),
            None => supervisor,
        }
    }
```

In `SupervisedUsdmWs` replace `task: tokio::task::JoinHandle<Result<(), UsdmWsError>>,` with `task: Task<Result<(), UsdmWsError>>,` and in `close` replace

```rust
        match task.await {
            Ok(result) => result,
            Err(_) => Err(stop.error()),
        }
```

with

```rust
        match task.await {
            Some(result) => result,
            None => Err(stop.error()),
        }
```

In `Router` add `runtime: Arc<dyn Runtime>,` after `config: UsdmWsBuilder,` and change `forwarders: Vec<tokio::task::JoinHandle<()>>,` to `forwarders: Vec<Task<()>>,`. In `adopt` replace

```rust
        self.forwarders.push(tokio::spawn(forward(
            path,
            self.generation,
            supervised,
            self.events.clone(),
            self.closing.subscribe(),
            self.ended.clone(),
        )));
```

with

```rust
        self.forwarders.push(spawn(
            self.runtime.as_ref(),
            forward(
                path,
                self.generation,
                supervised,
                self.events.clone(),
                self.closing.subscribe(),
                self.ended.clone(),
            ),
        ));
```

Binance's bare tier reads its ping's round trip on the connection's runtime. In `polyoxide-binance/src/usdm/ws/client.rs` remove the line `use tokio::time::Instant;` and replace `ping`'s body

```rust
        let start = Instant::now();
        self.send_ping().await?;
        self.connection
            .await_answer(|message| match message {
                Message::Pong(_) => Correlated::Answer(()),
                _ => Correlated::Push,
            })
            .await
            .map_err(|err| no_answer(err, 0))?;
        Ok(start.elapsed())
```

with

```rust
        let runtime = std::sync::Arc::clone(self.connection.runtime());
        let start = runtime.now();
        self.send_ping().await?;
        self.connection
            .await_answer(|message| match message {
                Message::Pong(_) => Correlated::Answer(()),
                _ => Correlated::Push,
            })
            .await
            .map_err(|err| no_answer(err, 0))?;
        Ok(runtime.now().saturating_duration_since(start))
```

and add `use tokio::time::Instant;` to the `tests` module of the same file, next to `use super::*;`, since its pacing test reads `tokio::time::Instant::now()` through the glob import it loses.

Regenerate the allowlists and check the diff:

Run: `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'`
Run: `git diff --stat ci/dep-allowlists/`
Expected: only `polyoxide-binance@usdm-ws` changes, by `tokio`'s `rt` feature leaving it (and `polyoxide-rt` joining when Story 2 did not already add it). Anything else is a mistake: stop and report it.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`
Run: `nix develop --command cargo test -p polyoxide-binance --all-features`
Expected: `test result: ok.` for every target, including `supervision_usdm`, `supervision_usdm_edges` and `supervision_usdm_kit` unchanged and the new test.
Run: `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings`
Expected: no warnings.
Run: `nix develop --command cargo hack check -p polyoxide-binance --each-feature --no-dev-deps`
Expected: every feature builds alone.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(binance): the router, forwarders and bare tier on the injected runtime (wasm32 story 5)

UsdmWsBuilder gets runtime(..) and socket_transport(..); the router and each
forwarder are spawned on the runtime, and usdm-ws no longer enables tokio's
rt. The usdm-ws allowlist is regenerated for that." -- polyoxide-binance/Cargo.toml polyoxide-binance/src/usdm/ws/supervised.rs polyoxide-binance/src/usdm/ws/client.rs Cargo.lock ci/dep-allowlists
```

### Task 5.5: Kalshi's socket on the runtime

**Files:**
- Modify: `polyoxide-kalshi/src/predictions/ws/builder.rs:1-125`
- Modify: `polyoxide-kalshi/src/predictions/ws/protocol.rs:3-30,84-94,186-262,368-378,415-430`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module of `polyoxide-kalshi/src/predictions/ws/protocol.rs` (after `snapshots_that_do_not_arrive_are_a_timeout_on_the_first_frame_past_the_window`) a test that a re-snapshot's age is read on the protocol's own runtime, not the process clock: a runtime whose clock never moves keeps a snapshot from ever being overdue.

```rust
    /// A runtime whose clock never moves.
    #[derive(Debug)]
    struct Frozen(polyoxide_rt::Instant);

    impl polyoxide_rt::Runtime for Frozen {
        fn spawn(&self, _task: polyoxide_rt::BoxFuture<()>) {}

        fn sleep(&self, _duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            Box::pin(std::future::pending())
        }

        fn now(&self) -> polyoxide_rt::Instant {
            self.0
        }
    }

    #[test]
    fn a_resnapshots_age_is_the_protocols_runtime_clock() {
        // Bug: the age read the process clock, so no runtime could drive it.
        let mut protocol = on_the_book().runtime(std::sync::Arc::new(Frozen(
            polyoxide_rt::Instant::now(),
        )));
        protocol.stale_after = Duration::from_millis(300);
        gap_on_the_book(&mut protocol);
        std::thread::sleep(Duration::from_millis(350));
        let pong = || Message::Pong(Vec::new().into());
        assert!(
            matches!(protocol.decode(&pong()), Decoded::Nothing),
            "no time has passed on the protocol's own clock"
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features --lib a_resnapshots_age_is_the_protocols_runtime_clock`
Expected: a compile error, ``no method named `runtime` found for struct `PredictionsProtocol` ``.

- [ ] **Step 3: Implement**

Run `grep -n 'polyoxide-rt' polyoxide-kalshi/Cargo.toml`: Story 2 declares the dependency for the HTTP side. `predictions-ws` turns `predictions` on, so a declaration under `predictions` suffices; when the line is absent, add `polyoxide-rt = { workspace = true }` under `polyoxide-ws`'s declaration.

`polyoxide-kalshi/src/predictions/ws/protocol.rs`: replace the imports

```rust
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    future::Future,
    time::Duration,
};

use polyoxide_venue::Classify;
```

with

```rust
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    future::Future,
    sync::Arc,
    time::Duration,
};

use polyoxide_rt::{default_runtime, Instant, Runtime};
use polyoxide_venue::Classify;
```

and remove the line `use tokio::time::Instant;`. Add the runtime to the protocol: after the field `change_refused: Option<Refusal>,` (the last field of `PredictionsProtocol`) add

```rust
    /// The runtime whose clock times a re-snapshot, so a paused or injected
    /// clock drives it.
    runtime: Arc<dyn Runtime>,
```

In `new` add `runtime: default_runtime(),` after `change_refused: None,`, and add the setter after `configured`:

```rust
    /// The runtime whose clock times a re-snapshot.
    pub(crate) fn runtime(mut self, runtime: Arc<dyn Runtime>) -> Self {
        self.runtime = runtime;
        self
    }
```

In `snapshot_overdue` replace `let waited = resync.queued_at.elapsed();` with `let waited = self.runtime.now().saturating_duration_since(resync.queued_at);` and in `begin_resync` replace `queued_at: Instant::now(),` with `queued_at: self.runtime.now(),`.

`polyoxide-kalshi/src/predictions/ws/builder.rs`: replace the imports and struct

```rust
use std::{collections::BTreeSet, time::Duration};

use polyoxide_ws::{Supervisor, Timing};
```

with

```rust
use std::{collections::BTreeSet, sync::Arc, time::Duration};

use polyoxide_rt::{default_runtime, Runtime};
use polyoxide_ws::{default_socket_transport, SocketTransport, Supervisor, Timing};
```

and add `runtime: Option<Arc<dyn Runtime>>,` and `socket_transport: Option<Arc<dyn SocketTransport>>,` to `PredictionsWsBuilder` after `channels: BTreeSet<Channel>,`, and `runtime: None, socket_transport: None,` to `new` after the `channels:` line. After the `timing_setters!` block add

```rust
    polyoxide_ws::transport_setters! {
        /// The runtime the supervision task, its pings and a re-snapshot's
        /// clock run on. By default the target's.
        runtime;
        /// How each attempt dials. By default the target's.
        socket_transport;
    }
```

and in `connect` replace

```rust
        let protocol = PredictionsProtocol::new(self.url, credentials)
            .configured(self.channels, self.timing.stale_after);
        Supervisor::new(protocol, self.timing)
            .connect(membership)
            .await
```

with

```rust
        let runtime = self.runtime.unwrap_or_else(default_runtime);
        let protocol = PredictionsProtocol::new(self.url, credentials)
            .configured(self.channels, self.timing.stale_after)
            .runtime(Arc::clone(&runtime));
        Supervisor::new(protocol, self.timing)
            .runtime(runtime)
            .socket_transport(
                self.socket_transport
                    .unwrap_or_else(default_socket_transport),
            )
            .connect(membership)
            .await
```

Last, compile every suite of the workspace against the new API. The live and failure-tag suites (`live_usdm_ws`, `live_predictions_ws`, `live_clob_ws`, `failure_tags_*`) open raw sockets through `polyoxide_ws::connect`, which returns a `BoxSocket` that reads and writes as the `Socket` did, so none is expected to break; `cargo clippy --workspace --all-targets` in Step 4 names any that does, and each is fixed the way `live_sports.rs` was.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo fmt --all`
Run: `nix develop --command cargo test -p polyoxide-kalshi --all-features`
Expected: `test result: ok.` for every target; `snapshots_that_do_not_arrive_are_a_timeout_on_the_first_frame_past_the_window` passes unchanged on tokio's paused clock, since `TokioRuntime::now` reads it.
Run: `nix develop --command cargo test --workspace --all-features`
Expected: `test result: ok.` for every target.
Run: `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: no warnings.
Run: `nix develop --command cargo hack check -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-ws --each-feature --no-dev-deps`
Expected: every feature of each crate builds alone.
Run: `nix develop --command python3 scripts/venue_gate.py && nix develop --command python3 scripts/live_unwraps.py && nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0 && nix develop --command python3 scripts/api_removals.py check --baseline v0.40.0 && nix develop --command python3 scripts/gen_registry.py --check`
Expected: all exit 0.
Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(kalshi): the socket on the injected runtime (wasm32 story 5)

PredictionsWsBuilder gets runtime(..) and socket_transport(..), and a
re-snapshot's age is read on the runtime's clock." -- polyoxide-kalshi/src/predictions/ws/builder.rs polyoxide-kalshi/src/predictions/ws/protocol.rs
```

## Story 6: The wall clock through `web-time`, and `runtime_gate.py`

- **Developer:** intermediate-dev
- **Files:** `polyoxide-venue/Cargo.toml`, `polyoxide-venue/README.md`, `polyoxide-venue/src/lib.rs`, `polyoxide-venue/src/time.rs`, `polyoxide-polymarket/src/shared/auth.rs`, `.claude/rules/venue-traits.md`, `CLAUDE.md`, `README.md`, `scripts/runtime_gate.py`, `.github/scripts/tests/test_runtime_gate.py`, `.claude/rules/ci-gates.md`, `.claude/rules/rt.md`, `Cargo.lock`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 6 → Stories 9, 11"; uses Story 1's `SystemTime` and `UNIX_EPOCH`
- **Verify:** `python3 scripts/runtime_gate.py && cargo test -p polyoxide-venue -p polyoxide-polymarket --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && bash -c 'cd .github/scripts && uv run pytest tests/test_runtime_gate.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
- **Review:** light (the signing timestamp keeps its value; the gate is a script with its own tests)

The wall clock is the one clock the runtime does not inject (spec D3): a timestamp a venue reads is the real time, whichever executor runs the client. Task 6.1 makes the two places that read it, `UnixMillis::now` and `shared::auth::current_timestamp`, read `web_time`'s `SystemTime` (`polyoxide-venue` through a wasm32-only dependency, since `std`'s clock does not exist there and `web_time` re-exports `std`'s types elsewhere; `polyoxide-polymarket` through `polyoxide_rt::SystemTime`), and marks both reads as the gate's exemptions. Task 6.2 states the new wasm32-only dependency in the four places that say the crate "depends on nothing" by default, and regenerates the registry text. Task 6.3 adds `scripts/runtime_gate.py`, which holds every other timer, monotonic clock read and spawn in the six scanned crates to the injected runtime, with its pytest and the rules text. The gate scans the code Stories 2 and 5 moved; any site it still finds in task 6.3 is a straggler those stories missed, which the developer fixes and reports to the supervisor as a deviation. No landmine marker is in this story's code. Task 6.1 relies on Story 1's `web-time` entry in `[workspace.dependencies]` and on Story 2 having declared `polyoxide-rt` for every `polyoxide-polymarket` feature that builds `shared::auth` (`clob` and `relay`).

### Task 6.1: Read the wall clock through `web_time`

**Files:**
- Modify: `polyoxide-venue/Cargo.toml:24-26`
- Modify: `polyoxide-venue/src/time.rs:3,13-15`
- Modify: `polyoxide-polymarket/src/shared/auth.rs:12,19-24,108-117`
- Modify: `Cargo.lock` (cargo writes it)

- [ ] **Step 1: Write the test that pins the timestamp, and check the two premises**

The change swaps a type and keeps every value, so the test pins the value: the signing timestamp sits between two reads of the runtime crate's own clock. In `polyoxide-polymarket/src/shared/auth.rs`, in `mod tests`, add this test after `test_current_timestamp`, whose current text is:

```rust
    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        // Should be a reasonable Unix timestamp (after 2020)
        assert!(ts > 1_600_000_000);
    }
```

New text, the old test unchanged and the new one after it:

```rust
    #[test]
    fn test_current_timestamp() {
        let ts = current_timestamp();
        // Should be a reasonable Unix timestamp (after 2020)
        assert!(ts > 1_600_000_000);
    }

    #[test]
    fn current_timestamp_sits_between_two_reads_of_the_runtime_crates_clock() {
        let read = || {
            polyoxide_rt::SystemTime::now()
                .duration_since(polyoxide_rt::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs())
        };
        let before = read();
        let ts = current_timestamp();
        let after = read();
        assert!(before <= ts && ts <= after, "{before} <= {ts} <= {after}");
    }
```

Check the two premises:

```bash
grep -n '^web-time' Cargo.toml
grep -n 'polyoxide-rt' polyoxide-polymarket/Cargo.toml
```

The first prints Story 1's `[workspace.dependencies]` line for `web-time`; the second prints the `polyoxide-rt` dependency and the features that enable it, which must include `clob` and `relay`. If either prints nothing, stop and report: the story depends on them and must not add them.

- [ ] **Step 2: Run it to see it pass before the change**

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features -- current_timestamp_sits_between_two_reads_of_the_runtime_crates_clock`
Expected: `test shared::auth::tests::current_timestamp_sits_between_two_reads_of_the_runtime_crates_clock ... ok`. It passes before the change because `polyoxide_rt::SystemTime` is `std`'s on a native target: the test fixes the value that the change must keep. A compile error `use of unresolved module or unlinked crate polyoxide_rt` means the second premise failed.

- [ ] **Step 3: Implement**

`polyoxide-venue/Cargo.toml`: a dependency for wasm32 only, since no feature may turn on a target-specific dependency (spec D10). The current text is:

```toml
rust_decimal = { workspace = true, optional = true }
serde = { workspace = true, optional = true }

[features]
```

New text:

```toml
rust_decimal = { workspace = true, optional = true }
serde = { workspace = true, optional = true }

# `std` has no wall clock on wasm32-unknown-unknown; `web-time` reads the
# browser's. Declared for that target alone (a feature never turns on a
# target-specific dependency), so no native tree changes.
[target.'cfg(target_arch = "wasm32")'.dependencies]
web-time = { workspace = true }

[features]
```

`polyoxide-venue/src/time.rs`: the import and the read. The current import is:

```rust
use std::time::{SystemTime, UNIX_EPOCH};
```

New text:

```rust
// `web_time` re-exports `std`'s types on every target but wasm32, where it
// reads the browser's clock.
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(target_arch = "wasm32")]
use web_time::{SystemTime, UNIX_EPOCH};
```

The current read is:

```rust
    pub fn now() -> Self {
        Self::at(SystemTime::now())
    }
```

New text (the wall clock is never injected, so the gate's opt-out names why):

```rust
    pub fn now() -> Self {
        Self::at(SystemTime::now()) // runtime-gate: the wall clock is never injected
    }
```

`polyoxide-polymarket/src/shared/auth.rs`: the import. The current text is:

```rust
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
```

New text:

```rust
use polyoxide_rt::{SystemTime, UNIX_EPOCH};
use sha2::Sha256;
```

and the read. The current text is:

```rust
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must not be before Unix epoch")
        .as_secs()
}
```

New text:

```rust
pub fn current_timestamp() -> u64 {
    let now = SystemTime::now(); // runtime-gate: the wall clock is never injected
    now.duration_since(UNIX_EPOCH)
        .expect("system time must not be before Unix epoch")
        .as_secs()
}
```

Run `nix develop --command cargo fmt --all`; it must leave both files as written here.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-venue --all-features -- time::`
Expected: `now_is_after_2026_and_never_runs_backwards_between_two_calls ... ok`, `a_clock_before_1970_reads_zero ... ok` and `test result: ok`.

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features -- current_timestamp`
Expected: `test_current_timestamp ... ok`, `current_timestamp_sits_between_two_reads_of_the_runtime_crates_clock ... ok` and `test result: ok`.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'`
Expected: all pass with `ci/dep-allowlists/` untouched, because the lists are pinned to `x86_64-unknown-linux-gnu` and the new dependency is wasm32-only. If a list differs, regenerate it with `uv run python -m tests.test_dep_allowlists --write` in `.github/scripts` and put the reason in the commit message.

Run: `nix develop --command cargo clippy -p polyoxide-venue -p polyoxide-polymarket --all-targets --all-features -- -D warnings`
Expected: no output but `Finished`.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor(venue,polymarket): read the wall clock through web-time (story 6)" -- polyoxide-venue/Cargo.toml polyoxide-venue/src/time.rs polyoxide-polymarket/src/shared/auth.rs Cargo.lock
```

### Task 6.2: Say the wasm32-only dependency where the crate claims to depend on nothing

Task 6.1 gave `polyoxide-venue` a dependency on wasm32. Four texts still say the crate depends on nothing by default; each keeps its claim true on native and adds the wasm32 clause. The registry text in `CLAUDE.md` is generated from the first.

**Files:**
- Modify: `polyoxide-venue/Cargo.toml:66`
- Modify: `polyoxide-venue/README.md:25-26`
- Modify: `polyoxide-venue/src/lib.rs:46-47`
- Modify: `.claude/rules/venue-traits.md:17`
- Modify: `CLAUDE.md` (written by `scripts/gen_registry.py --write`, never by hand)
- Modify: `README.md` (written by `scripts/gen_registry.py --write` if the generator changes it)

- [ ] **Step 1: Change the registry source**

In `polyoxide-venue/Cargo.toml`, the `notes` line under `[package.metadata.polyoxide]` ends with the clause below. Change this text:

```toml
by default it depends on nothing, and it has no HTTP, socket or signing dependency"]
```

to:

```toml
by default it depends on nothing, on wasm32 only `web-time`, and it has no HTTP, socket or signing dependency"]
```

The rest of the line stays as it is.

- [ ] **Step 2: Run it to see it fail**

Run: `python3 scripts/gen_registry.py --check`
Expected: exit 1, naming `CLAUDE.md`: its generated crate line differs from what the generator now produces.

- [ ] **Step 3: Implement**

`polyoxide-venue/README.md`, a prose paragraph with no code fence. The current text is:

```text
By default it depends on nothing, so a credential-free socket client can use it
without building an HTTP or signing stack.
```

New text:

```text
By default it depends on nothing (on wasm32 it depends only on `web-time`, which
reads the browser's wall clock), so a credential-free socket client can use it
without building an HTTP or signing stack.
```

`polyoxide-venue/src/lib.rs`, the crate docs. The current text is:

```rust
//! By default the crate depends on nothing, so a credential-free socket crate
//! can use it without building an HTTP or signing stack. The enum macros name
```

New text:

```rust
//! By default the crate depends on nothing (on wasm32, only `web-time`, which
//! reads the browser's wall clock), so a credential-free socket crate can use
//! it without building an HTTP or signing stack. The enum macros name
```

`.claude/rules/venue-traits.md`. The current text is:

```markdown
`polyoxide-venue` depends on nothing by default and has no HTTP, socket or signing dependency;
```

New text:

```markdown
`polyoxide-venue` depends on nothing by default (on wasm32, only `web-time`, for the wall clock) and has no HTTP, socket or signing dependency;
```

Then regenerate: `python3 scripts/gen_registry.py --write`. It rewrites the generated crate line in `CLAUDE.md` (and `README.md`, if its region changes) and nothing else. Do not edit either file by hand.

- [ ] **Step 4: Run it to see it pass**

Run: `python3 scripts/gen_registry.py --check`
Expected: exit 0.

Run: `git diff --stat -- CLAUDE.md README.md`
Expected: `CLAUDE.md` changed by the one line that now ends `by default it depends on nothing, on wasm32 only \`web-time\`, and it has no HTTP, socket or signing dependency`; `README.md` changed only inside a generated region, or not at all.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
Expected: all pass.

Run: `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-venue --all-features'` and `nix develop --command cargo test -p polyoxide-venue --all-features --doc`
Expected: no warnings; the README is a doctest and has no code fence, so it compiles nothing and `test result: ok`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(venue): say the crate's wasm32-only web-time where it claims no dependencies (story 6)" -- polyoxide-venue/Cargo.toml polyoxide-venue/README.md polyoxide-venue/src/lib.rs .claude/rules/venue-traits.md CLAUDE.md README.md
```

### Task 6.3: `scripts/runtime_gate.py`, its pytest and the rules text

**Files:**
- Create: `scripts/runtime_gate.py`
- Create: `.github/scripts/tests/test_runtime_gate.py`
- Modify: `.claude/rules/ci-gates.md:5,96-101,121-123`
- Modify: `.claude/rules/rt.md` (append)
- Modify: every source file the gate's first run reports (the sweep in Step 3)

- [ ] **Step 1: Write the failing tests**

`.github/scripts/tests/test_runtime_gate.py`:

```python
"""scripts/runtime_gate.py, the gate that holds time and tasks to the injected runtime (D3).

The synthetic trees cover each banned path, each skip and the opt-out. The real-tree tests
run the gate on the workspace, which must be clean, and prove it reads every scanned crate
and that the two wall-clock reads in tree are exempt by their opt-outs.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
CORE = "polyoxide-core/src/x.rs"
BAD = "fn f() { tokio::spawn(work()); }\n"


def _load(name: str):
    """A module from `scripts/`, which lives outside this uv project."""
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, REPO / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


gate = _load("runtime_gate")


def tree(root: Path, files: dict[str, str]) -> Path:
    for path, text in files.items():
        (root / path).parent.mkdir(parents=True, exist_ok=True)
        (root / path).write_text(text)
    return root


def found(root: Path, **options) -> list[tuple[str, int, str]]:
    return [(h.path, h.line, h.banned) for h in gate.scan(root, **options)]


# --- the banned paths ----------------------------------------------------------


@pytest.mark.parametrize(("code", "banned"), [
    ("tokio::spawn(async {});", "tokio::spawn"),
    ("tokio::task::spawn(work());", "tokio::task::spawn"),
    ("tokio::time::sleep(wait).await;", "tokio::time::"),
    ("use tokio::time::{sleep, Instant};", "tokio::time::"),
    ("let started = Instant::now();", "Instant::now("),
    ("let started = std::time::Instant::now();", "Instant::now("),
    ("let at = web_time::SystemTime::now ();", "SystemTime::now("),
])
def test_each_banned_path_is_a_hit(tmp_path: Path, code: str, banned: str) -> None:
    root = tree(tmp_path, {CORE: f"fn f() {{\n    {code}\n}}\n"})
    assert found(root) == [(CORE, 2, banned)]


@pytest.mark.parametrize("crate", gate.CRATES)
def test_a_hit_is_found_in_each_scanned_crate(tmp_path: Path, crate: str) -> None:
    path = f"{crate}/src/deep/x.rs"
    assert found(tree(tmp_path, {path: BAD})) == [(path, 1, "tokio::spawn")]


def test_a_path_that_spells_two_banned_paths_is_two_hits(tmp_path: Path) -> None:
    root = tree(tmp_path, {CORE: "let t = tokio::time::Instant::now();\n"})
    assert found(root) == [(CORE, 1, "Instant::now("), (CORE, 1, "tokio::time::")]


def test_what_is_not_a_scanned_crates_source_is_not_read(tmp_path: Path) -> None:
    line = "fn f() { tokio::spawn(work()); let _ = Instant::now(); }\n"
    paths = ("polyoxide-rt/src/tokio.rs", "polyoxide-cli/src/main.rs",
             "polyoxide-test-support/src/lib.rs", "polyoxide-core/tests/send.rs",
             "polyoxide-core/examples/probe.rs", "polyoxide-core/benches/send.rs",
             "polyoxide-core/build.rs")
    assert found(tree(tmp_path, {path: line for path in paths})) == []


def test_code_that_goes_through_the_runtime_is_clean(tmp_path: Path) -> None:
    text = (
        "async fn f(runtime: &dyn Runtime) {\n"
        "    runtime.sleep(wait).await;\n"
        "    let now = runtime.now();\n"
        "    let task = polyoxide_rt::spawn(runtime, work());\n"
        "    let _ = polyoxide_rt::timeout(runtime, wait, work()).await;\n"
        "}\n"
    )
    assert found(tree(tmp_path, {CORE: text})) == []


def test_comments_and_string_literals_are_not_code(tmp_path: Path) -> None:
    text = (
        "// tokio::spawn(task);\n"
        "/// Calls `Instant::now()` once.\n"
        'let s = "SystemTime::now() and tokio::time::sleep";\n'
        'let r = r#"Instant::now()"#;\n'
        "/* tokio::task::spawn(x) */\n"
    )
    assert found(tree(tmp_path, {CORE: text})) == []


# --- what the gate skips -------------------------------------------------------

AFTER_A_TEST_MODULE = (
    "pub fn before() {}\n"
    "\n"
    "#[cfg(test)]\n"
    "mod tests {\n"
    "    use super::*;\n"
    "    #[tokio::test]\n"
    "    async fn t() {\n"
    "        tokio::time::sleep(D).await;\n"
    "        let _ = Instant::now();\n"
    "    }\n"
    "}\n"
    "\n"
    "pub fn after() {\n"
    "    let _ = Instant::now();\n"
    "}\n"
)


def test_a_cfg_test_module_is_skipped_and_the_code_after_it_is_not(tmp_path: Path) -> None:
    assert found(tree(tmp_path, {CORE: AFTER_A_TEST_MODULE})) == [(CORE, 14, "Instant::now(")]


@pytest.mark.parametrize(("predicate", "skipped"), [
    ("test", True),
    ('any(test, feature = "scripted")', True),
    ('all(test, feature = "x")', True),
    ("not(test)", False),
    ('feature = "test-server"', False),
    ('any(not(test), feature = "x")', False),
    ('target_arch = "wasm32"', False),
])
def test_only_a_cfg_naming_test_outside_not_skips_a_module(
        tmp_path: Path, predicate: str, skipped: bool) -> None:
    text = f"#[cfg({predicate})]\nmod m {{\n    fn f() {{ tokio::spawn(work()); }}\n}}\n"
    expected = [] if skipped else [(CORE, 3, "tokio::spawn")]
    assert found(tree(tmp_path, {CORE: text})) == expected


def test_a_cfg_test_use_and_fn_are_skipped_and_the_next_item_is_not(tmp_path: Path) -> None:
    text = (
        "#[cfg(test)]\n"
        "use tokio::time::{sleep, Instant};\n"
        "#[cfg(test)]\n"
        "fn helper() -> Result<(), E> {\n"
        "    tokio::spawn(work());\n"
        "    Ok(())\n"
        "}\n"
        "use tokio::time::sleep;\n"
    )
    assert found(tree(tmp_path, {CORE: text})) == [(CORE, 8, "tokio::time::")]


def test_the_file_a_cfg_test_mod_declares_is_skipped_beside_lib_and_mod_rs(
        tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-core/src/lib.rs": "#[cfg(test)]\nmod tests;\nmod live;\n",
        "polyoxide-core/src/tests.rs": BAD,
        "polyoxide-core/src/tests/inner.rs": BAD,
        "polyoxide-core/src/live.rs": BAD,
        "polyoxide-ws/src/supervisor/mod.rs": "#[cfg(test)]\nmod tests;\n",
        "polyoxide-ws/src/supervisor/tests.rs": BAD,
    })
    assert found(root) == [("polyoxide-core/src/live.rs", 1, "tokio::spawn")]


def test_the_file_a_plain_file_declares_is_in_its_own_directory(tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-core/src/send.rs": "#[cfg(test)]\nmod tests;\n",
        "polyoxide-core/src/send/tests.rs": BAD,
        "polyoxide-core/src/tests.rs": BAD,
    })
    assert found(root) == [("polyoxide-core/src/tests.rs", 1, "tokio::spawn")]


def test_an_inner_cfg_test_before_the_first_item_skips_the_file_and_its_module(
        tmp_path: Path) -> None:
    root = tree(tmp_path, {
        "polyoxide-core/src/helper.rs": "#![cfg(test)]\n" + BAD,
        "polyoxide-core/src/helper/more.rs": BAD,
        "polyoxide-core/src/real.rs": "#![allow(dead_code)]\n" + BAD,
        "polyoxide-core/src/late.rs": BAD + "mod m {\n    #![cfg(test)]\n}\n",
    })
    assert found(root) == [("polyoxide-core/src/late.rs", 1, "tokio::spawn"),
                           ("polyoxide-core/src/real.rs", 2, "tokio::spawn")]


def test_the_test_server_and_every_scripted_fixture_are_skipped(tmp_path: Path) -> None:
    skipped = ("polyoxide-ws/src/test_server/mod.rs", "polyoxide-ws/src/test_server/helpers.rs",
               "polyoxide-binance/src/usdm/ws/scripted.rs",
               "polyoxide-kalshi/src/predictions/ws/scripted.rs")
    files = {path: BAD for path in skipped}
    # A test server outside the kit's, and the kit's other files, are read.
    files["polyoxide-ws/src/connect.rs"] = BAD
    files["polyoxide-core/src/test_server/mod.rs"] = BAD
    assert found(tree(tmp_path, files)) == [
        ("polyoxide-core/src/test_server/mod.rs", 1, "tokio::spawn"),
        ("polyoxide-ws/src/connect.rs", 1, "tokio::spawn")]


# --- the opt-out ---------------------------------------------------------------


def test_a_line_ending_runtime_gate_with_a_reason_is_exempt_and_nothing_else_is(
        tmp_path: Path) -> None:
    text = (
        "let a = SystemTime::now(); // runtime-gate: the wall clock, never injected\n"
        "let b = SystemTime::now(); // runtime-gate:\n"
        "let c = SystemTime::now(); // live-unwraps: not this gate's opt-out\n"
        "let d = SystemTime::now();\n"
        "// runtime-gate: a comment above does not exempt the line below\n"
        "let e = SystemTime::now();\n"
    )
    root = tree(tmp_path, {CORE: text})
    assert found(root) == [(CORE, n, "SystemTime::now(") for n in (2, 3, 4, 6)]
    assert [n for _, n, _ in found(root, opt_out=False)] == [1, 2, 3, 4, 6]


# --- the command ---------------------------------------------------------------


def test_main_exits_zero_and_prints_nothing_on_a_clean_tree(tmp_path: Path, capsys) -> None:
    tree(tmp_path, {CORE: "fn f() {}\n"})
    assert gate.main([], root=tmp_path) == 0
    captured = capsys.readouterr()
    assert (captured.out, captured.err) == ("", "")


def test_main_exits_one_and_prints_each_hit_as_path_line_banned_path(
        tmp_path: Path, capsys) -> None:
    tree(tmp_path, {CORE: "fn f() {}\nfn g() { tokio::spawn(work()); }\n"})
    assert gate.main([], root=tmp_path) == 1
    captured = capsys.readouterr()
    assert captured.out == "polyoxide-core/src/x.rs:2: tokio::spawn\n"
    assert "1 hit(s)" in captured.err


# --- the real tree -------------------------------------------------------------


def test_the_real_tree_is_clean() -> None:
    assert found(REPO) == []


def test_the_real_tree_is_read_in_every_scanned_crate() -> None:
    crates = {path.relative_to(REPO).parts[0] for path in gate.source_files(REPO)}
    assert crates == set(gate.CRATES)


def test_the_real_trees_wall_clock_reads_are_exempt_by_their_opt_outs() -> None:
    unexempted = {(path, banned) for path, _, banned in found(REPO, opt_out=False)}
    assert {("polyoxide-venue/src/time.rs", "SystemTime::now("),
            ("polyoxide-polymarket/src/shared/auth.rs", "SystemTime::now(")} <= unexempted
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_runtime_gate.py -q'`
Expected: a collection error, `FileNotFoundError: [Errno 2] No such file or directory: '<repo>/scripts/runtime_gate.py'`, and `1 error`.

- [ ] **Step 3: Implement**

`scripts/runtime_gate.py`, whole file:

```python
#!/usr/bin/env python3
"""The runtime gate: production code reaches time and tasks through the injected runtime.

Every timer, monotonic clock read and spawn in `polyoxide-venue`, `-core`, `-ws`,
`-polymarket`, `-binance` and `-kalshi` goes through the `polyoxide_rt::Runtime` its client
or socket was built with, so a build for a browser, which has neither tokio's clock nor its
executor, injects its own. This script holds that line over the `src/**/*.rs` of those six
crates. Clippy's `disallowed-methods` cannot: it would fire in every test and example too,
which use tokio legitimately. It reads each file's code (comments and the contents of string
literals are not code) for

  - `tokio::spawn` and `tokio::task::spawn`;
  - any `tokio::time` path (`tokio::time::sleep`, `use tokio::time::{..}`);
  - `Instant::now(` and `SystemTime::now(`, however qualified.

It skips what is not production code:

  - an item under an attribute whose `cfg` names `test` outside a `not(..)` (`#[cfg(test)]`,
    `#[cfg(any(test, feature = ".."))]`), and, for a `mod x;` declaration, the file it
    declares (`x.rs` or `x/mod.rs` beside the declaring file) and every file under it;
  - a file whose first item is `#![cfg(test)]`, and every file under its module;
  - `polyoxide-ws/src/test_server/` and every file named `scripted.rs`.

`#[path = ".."]` is not followed. `polyoxide-rt` is not scanned: it implements the runtime.

A line ending `// runtime-gate: <reason>` is exempt, as `scripts/live_unwraps.py`'s opt-out
is. The wall clock is the case in tree: a timestamp sent to a venue is never injected.

Each hit is printed on stdout as `<path>:<line>: <banned path>`, the path from the repository
root. Exit codes: 0 when there is no hit, 1 when there is one.

Stdlib only.

Usage:
    python3 scripts/runtime_gate.py
"""
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_registry  # noqa: E402

REPO = gen_registry.REPO
CRATES = ("polyoxide-venue", "polyoxide-core", "polyoxide-ws", "polyoxide-polymarket",
          "polyoxide-binance", "polyoxide-kalshi")
TEST_SERVER = "polyoxide-ws/src/test_server"
FIXTURE = "scripted.rs"
# Each banned path, as it is printed, and the code that spells it.
BANNED = (
    ("tokio::spawn", re.compile(r"\btokio\s*::\s*spawn\b")),
    ("tokio::task::spawn", re.compile(r"\btokio\s*::\s*task\s*::\s*spawn\b")),
    ("tokio::time::", re.compile(r"\btokio\s*::\s*time\b")),
    ("Instant::now(", re.compile(r"\bInstant\s*::\s*now\s*\(")),
    ("SystemTime::now(", re.compile(r"\bSystemTime\s*::\s*now\s*\(")),
)
OPT_OUT = re.compile(r"^\s*runtime-gate:\s*\S")
CFG = re.compile(r"#\s*(!?)\s*\[\s*cfg\s*\(")
MOD_NAME = re.compile(r"\bmod\s+(\w+)\s*$")
INNER_ATTRIBUTES = re.compile(r"(?:\s*#!\[[^\]]*\])*\s*")
# The files whose module directory is their own directory.
ROOT_FILES = frozenset({"mod.rs", "lib.rs", "main.rs"})


@dataclass(frozen=True, order=True)
class Hit:
    path: str
    line: int
    banned: str


@dataclass(frozen=True)
class Source:
    """One file as the gate reads it."""

    code: str  # the code, test items blanked, offsets and lines as in the file
    comments: list[str]  # the `//` comment text of each line
    test_modules: tuple[str, ...]  # `mod x;` declarations under a test `cfg`
    test_file: bool  # `#![cfg(test)]` before the first item


def names_test(predicate: str) -> bool:
    """Whether a `cfg` predicate names `test` outside every `not(..)`."""
    nots: list[bool] = []
    previous = ""
    for token in re.findall(r"[A-Za-z_][A-Za-z0-9_]*|[()]", predicate):
        if token == "(":
            nots.append(previous == "not")
        elif token == ")":
            if nots:
                nots.pop()
        elif token == "test" and not any(nots):
            return True
        previous = token
    return False


def _close(code: str, start: int) -> int:
    """The index just past the bracket that closes the one at `start`."""
    depth = 0
    for i in range(start, len(code)):
        if code[i] in "([{":
            depth += 1
        elif code[i] in ")]}":
            depth -= 1
            if depth == 0:
                return i + 1
    return len(code)


def _item(code: str, start: int) -> tuple[int, str, str]:
    """Where the item that starts at `start` ends, its header, and the character that
    ends the header (`{`, `;` or empty at the end of the file)."""
    i = start
    while i < len(code):
        if code[i] in "([":
            i = _close(code, i)
        elif code[i] == "{":
            return _close(code, i), code[start:i], "{"
        elif code[i] == ";":
            return i + 1, code[start:i], ";"
        else:
            i += 1
    return len(code), code[start:], ""


def read(text: str) -> Source:
    kinds = gen_registry.lex(text)
    code = gen_registry.view(text, kinds, gen_registry.CODE)
    comments = gen_registry.view(text, kinds, gen_registry.LINE_COMMENT).split("\n")
    spans: list[tuple[int, int]] = []
    modules: list[str] = []
    test_file = False
    for match in CFG.finditer(code):
        close = _close(code, match.end() - 1)
        if not names_test(code[match.end():close - 1]):
            continue
        if match[1]:
            test_file = test_file or bool(INNER_ATTRIBUTES.fullmatch(code[:match.start()]))
            continue
        bracket = code.find("]", close)
        if bracket < 0:
            continue
        end, header, stop = _item(code, bracket + 1)
        spans.append((match.start(), end))
        declared = MOD_NAME.search(header)
        if stop == ";" and declared:
            modules.append(declared[1])
    chars = list(code)
    for start, end in spans:
        for i in range(start, end):
            if chars[i] != "\n":
                chars[i] = " "
    return Source("".join(chars), comments, tuple(modules), test_file)


def source_files(root: Path = REPO) -> list[Path]:
    """The `src/**/*.rs` of each scanned crate under `root`."""
    return [file for crate in CRATES for file in sorted((root / crate / "src").rglob("*.rs"))]


def scan(root: Path = REPO, opt_out: bool = True) -> list[Hit]:
    """Every banned path in `root`'s production code, less the lines that opt out
    (all of them when `opt_out` is false)."""
    files = source_files(root)
    sources = {file: read(file.read_text(encoding="utf-8")) for file in files}
    skipped_files: set[Path] = set()
    skipped_dirs: set[Path] = set()
    for file, source in sources.items():
        module_dir = file.parent if file.name in ROOT_FILES else file.parent / file.stem
        if source.test_file:
            skipped_files.add(file)
            skipped_dirs.add(module_dir)
        for name in source.test_modules:
            skipped_files.add(module_dir / f"{name}.rs")
            skipped_dirs.add(module_dir / name)
    hits: set[Hit] = set()
    for file, source in sources.items():
        path = file.relative_to(root).as_posix()
        if (file in skipped_files or skipped_dirs & set(file.parents)
                or path.startswith(TEST_SERVER + "/") or file.name == FIXTURE):
            continue
        for banned, pattern in BANNED:
            for match in pattern.finditer(source.code):
                line = source.code.count("\n", 0, match.start())
                if opt_out and OPT_OUT.match(source.comments[line]):
                    continue
                hits.add(Hit(path, line + 1, banned))
    return sorted(hits)


def main(argv: list[str] | None = None, root: Path = REPO, out=None, err=None) -> int:
    argparse.ArgumentParser(description=__doc__.splitlines()[0]).parse_args(argv)
    out, err = out or sys.stdout, err or sys.stderr
    hits = scan(root)
    for hit in hits:
        print(f"{hit.path}:{hit.line}: {hit.banned}", file=out)
    if hits:
        print(f"runtime gate: {len(hits)} hit(s). Route each through the injected runtime, "
              f"or end the line with `// runtime-gate: <reason>`.", file=err)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

Then run the gate on the tree: `python3 scripts/runtime_gate.py`. It must print nothing and exit 0. Each line it prints is a straggler Stories 2 to 5 missed: fix it in its file and list it in the report to the supervisor as a deviation, with the story that should have moved it. Fix a site by its kind, with the `Arc<dyn polyoxide_rt::Runtime>` that the owning client, builder, supervisor or connection already holds (`HttpClient::runtime()`, the socket builder's `runtime`, `Connection::with_runtime`, `Supervisor::runtime`):

- a spawn: `polyoxide_rt::spawn(&*runtime, future)`, which returns a `Task` (dropping it detaches it, as `tokio::spawn`'s handle does);
- a sleep or a deadline: `runtime.sleep(duration).await` or `runtime.sleep_until(deadline).await`; a timeout: `polyoxide_rt::timeout(&*runtime, duration, future).await`; a ticker: `polyoxide_rt::interval(runtime.clone(), period)`;
- a monotonic clock read: `runtime.now()`;
- a wall-clock read (`SystemTime::now()`) that is a timestamp the venue reads: read `polyoxide_rt::SystemTime` and end the line with `// runtime-gate: the wall clock is never injected`; any other `SystemTime::now()` is a monotonic need and goes through `runtime.now()`.

A fix changes no test's assertion. Never end a line with the opt-out to quiet a timer or a spawn.

`.claude/rules/ci-gates.md`: three edits. The frontmatter line, currently:

```
  - "scripts/{api_removals,test_body_diff,venue_gate,gen_registry,public_api}.py"
```

becomes:

```
  - "scripts/{api_removals,test_body_diff,venue_gate,runtime_gate,gen_registry,public_api}.py"
```

After the `test_venue_gate.py` bullet, whose text is:

```
- `test_venue_gate.py` runs `scripts/venue_gate.py` (CAP-7) on the real tree, and on v0.39.0's
  core, which it must catch, so the job checks out every tag.
```

add:

```
- `test_runtime_gate.py` runs `scripts/runtime_gate.py` on the real tree, which must stay clean,
  and on synthetic trees for each banned path, each skip and the opt-out.
```

and before the `## removals` heading, after the venue gate's closing paragraph (it ends "excepted by `polyoxide-polymarket` and `polyoxide-binance`)."), add:

```
## The runtime gate

`scripts/runtime_gate.py` (no arguments; exit 0 when clean, 1 on a hit, each hit printed as
`<path>:<line>: <banned path>`) holds every timer, monotonic clock read and spawn in the
`src/**/*.rs` of `polyoxide-venue`, `-core`, `-ws`, `-polymarket`, `-binance` and `-kalshi` to the
injected `polyoxide_rt::Runtime`, so a browser build can supply its own. It fails on
`tokio::spawn`, `tokio::task::spawn`, any `tokio::time` path, `Instant::now(` and
`SystemTime::now(`, and not clippy, whose `disallowed-methods` would also fire in every test and
example, which use tokio legitimately. It skips an item under a `cfg` that names `test` outside a
`not(..)`, the file a `mod x;` under one declares, `polyoxide-ws/src/test_server/` and every
`scripted.rs`; `polyoxide-rt` implements the runtime and is not scanned. A line ending
`// runtime-gate: <reason>` is exempt, as `live_unwraps.py`'s opt-out is: the two in tree are the
wall-clock reads, `UnixMillis::now` and `shared::auth::current_timestamp`, which are never
injected. The scripts job runs it through `test_runtime_gate.py`.
```

`.claude/rules/rt.md`: append this section after the file's last line (leave one blank line before it):

```
## The wall clock and the runtime gate

The runtime is injected; the wall clock is not. A timestamp a venue reads is the real time
whichever executor runs the client, so `UnixMillis::now` and `shared::auth::current_timestamp`
read `SystemTime::now()` from `std` on every target but wasm32, where `polyoxide-venue` takes
`web_time`'s, and `polyoxide-polymarket` reads `polyoxide_rt::SystemTime`, which is `web_time`'s type
(`std`'s on every target but wasm32). Both lines end `// runtime-gate: the wall clock is never injected`.

`scripts/runtime_gate.py` fails on any other `Instant::now(` or `SystemTime::now(`, on
`tokio::spawn`, `tokio::task::spawn` and on any `tokio::time` path in the production code of the
crates that use this one (`ci-gates.md`); `polyoxide-rt` implements the runtime and is the one
crate it does not scan. A timer or a spawn there goes through the `Runtime` the client was built
with: `runtime.sleep`, `runtime.now`, `polyoxide_rt::spawn`, `polyoxide_rt::timeout` and
`polyoxide_rt::interval`.
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_runtime_gate.py -q'`
Expected: `36 passed`.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py tests/test_dep_allowlists.py -q'`
Expected: all pass; `ci-gates.md`'s new glob matches `scripts/runtime_gate.py`, and `rt.md`'s own `paths:` are untouched.

Run: `python3 scripts/runtime_gate.py; echo "exit $?"`
Expected: `exit 0` and no other output.

Run the story's gates: `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`, `nix develop --command cargo test -p polyoxide-venue -p polyoxide-polymarket --all-features`, `python3 scripts/venue_gate.py`, `python3 scripts/gen_registry.py --check`, `python3 scripts/live_unwraps.py` and `python3 scripts/test_body_diff.py --baseline v0.40.0`. Expected: each exits 0. Every file Step 3's sweep changed is also in the gates' packages; if the sweep changed a test body, its `[[drift]]` entry names DRIFT row `R22`.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/runtime_gate.py .github/scripts/tests/test_runtime_gate.py
git commit -m "feat(ci): hold time and tasks to the injected runtime with runtime_gate.py (story 6)" -- scripts/runtime_gate.py .github/scripts/tests/test_runtime_gate.py .claude/rules/ci-gates.md .claude/rules/rt.md <each source file the sweep changed>
```

## Story 7: The target gating and the three browser implementations (spec D8, D10)

- **Developer:** intermediate-dev
- **Files:** `polyoxide-rt/Cargo.toml`, `polyoxide-rt/src/lib.rs`, `polyoxide-rt/src/browser.rs`, `polyoxide-core/Cargo.toml`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/fetch.rs`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/error.rs`, `polyoxide-ws/Cargo.toml`, `polyoxide-ws/README.md`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/browser.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/tls.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/error.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-venue/Cargo.toml`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-binance/Cargo.toml`, `Cargo.toml`, `Cargo.lock`, `flake.nix`, `scripts/wasm_check.sh`, `.claude/rules/rt.md`, `.claude/rules/sockets.md`, `.claude/rules/send-loop.md`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `README.md`, `.github/scripts/tests/test_dep_allowlists.py`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 7 → Stories 8 to 10"; uses Story 1's `Runtime` and `default_runtime`, Story 4's `HttpTransport`, `Response` and `TransportError`, and Story 5's `SocketTransport`, `Capabilities` and `Kind::Unsupported`
- **Verify:** `bash scripts/wasm_check.sh && cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && msrv-cargo check --workspace --all-features && bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
- **Review:** deep (the browser implementations' contracts and `SendWrapper` soundness)

Makes the supported crates (spec D1) build for `wasm32-unknown-unknown` and gives that target its three defaults. Each native-only dependency moves into a `cfg(not(target_arch = "wasm32"))` table (tokio `rt` and `time` in `polyoxide-rt` already sit in one, from story 1; reqwest's native build, `rustls`, `tokio-tungstenite` and tokio `net` elsewhere), and each wasm32-only one into a `cfg(target_arch = "wasm32")` table (spec D10), with `getrandom`'s `wasm_js` in `polyoxide-rt`. `BrowserRuntime` (`spawn_local`; gloo-timers sleeps chained in steps of at most `i32::MAX` ms; `performance.now` through `web_time`), `FetchTransport` (reqwest's wasm client, `SendWrapper`'d, the request's timeout set per request, failures mapped per spec D8) and `WebSocketTransport` (web-sys, `binaryType` `arraybuffer`, events through an unbounded channel, closes mapped to 1000 or 3000-4999, handshake headers and control frames refused with the contract's messages) each become the wasm32 default of `default_runtime`, the config's transport and `default_socket_transport`. Each browser file keeps its decisions (which step lengths, which failure kind, which request or frame is refused, which close code goes out) in plain Rust built on every target, so the host's tests pin them; only the code that touches `web_sys` and `fetch` is wasm32-only, and the browser smoke suite (story 9) drives it. The flake's toolchain gains the wasm32 target, and `scripts/wasm_check.sh` checks and lints spec D1's whole set. Two additions beyond spec D10 are needed for that set to build or to behave: `polyoxide-polymarket` turns on `getrandom` 0.2's `js` feature on wasm32 (alloy's signer reaches it through `rand_core` 0.6, and 0.2 refuses `wasm32-unknown-unknown` without it), and core turns on `fastrand`'s `js` feature there (without it the retry jitter has one fixed seed). Each of the two additions carries a one-line reason in its manifest comment and in its commit message. The dependency test gains the `wasm32-unknown-unknown` set spec D10 asks for, under `ci/dep-allowlists/wasm32-unknown-unknown/`, with its files generated and its check shown red before green; the host's files stay pinned to `x86_64-unknown-linux-gnu`. Native builds are unchanged. No code in this story holds a landmine marker.

### Task 7.1: The wasm32 target in the dev shell, the shared declarations and the check script

**Files:**
- Modify: `flake.nix:40-45`
- Modify: `Cargo.toml:49` (the `tokio-tungstenite` line of `[workspace.dependencies]`)
- Modify: `CLAUDE.md` (the **Dev shell** paragraph)
- Create: `scripts/wasm_check.sh`

- [ ] **Step 1: Write the failing check**

The check is the target's presence in the dev shell's toolchain.

```bash
nix develop --command bash -c 'ls "$(rustc --print target-libdir --target wasm32-unknown-unknown)"'
```

- [ ] **Step 2: Run it to see it fail**

Run the command above. It prints:

```
ls: cannot access '/nix/store/<hash>-rust-minimal-1.99.0/lib/rustlib/wasm32-unknown-unknown/lib': No such file or directory
```

- [ ] **Step 3: Implement**

In `flake.nix`, give the toolchain the target. Replace

```nix
          rust = pkgs.rust-bin.stable."1.99.0".minimal.override {
            extensions = [
              "clippy"
              "rustfmt"
            ];
          };
```

with

```nix
          rust = pkgs.rust-bin.stable."1.99.0".minimal.override {
            extensions = [
              "clippy"
              "rustfmt"
            ];
            # The browser build (`scripts/wasm_check.sh`).
            targets = [ "wasm32-unknown-unknown" ];
          };
```

`flake.lock` does not change.

In the root `Cargo.toml`, add the shared declarations under `[workspace.dependencies]`, directly after the line

```toml
tokio-tungstenite = { version = "0.26", features = ["rustls-tls-native-roots"] }
```

```toml
# The message and handshake-request vocabulary, on every target; `polyoxide-ws`
# declares it directly so `wasm32` builds it without `tokio-tungstenite`. The
# native transport's own copy is `tokio-tungstenite`'s, the same crate.
tungstenite = { version = "0.26", default-features = false, features = ["handshake"] }
# The browser implementations (D8), each declared by its crate under
# `[target.'cfg(target_arch = "wasm32")'.dependencies]`.
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = "0.3"
gloo-timers = "0.3"
send_wrapper = "0.6"
```

In `CLAUDE.md`, the **Dev shell** paragraph states what the shell puts on `PATH`; the flake now adds a target. Replace

```
puts Rust 1.99.0 with clippy and rustfmt on `PATH`
```

with

```
puts Rust 1.99.0 with clippy, rustfmt and the `wasm32-unknown-unknown` target on `PATH`
```

Create `scripts/wasm_check.sh`:

```bash
#!/usr/bin/env bash
# Check and lint the crates that support wasm32-unknown-unknown (spec D1), crate
# by crate with their features named. The first failure stops it.
#
# `cargo check` and `cargo clippy` both run, since clippy -D warnings is the
# gate CI holds the native build to. The browser's getrandom backend is chosen
# by a cfg flag, not a feature, so this script sets RUSTFLAGS itself, replacing
# any inherited value (a host `target-cpu`, say, would override a config
# file's and break the target); CARGO_ENCODED_RUSTFLAGS outranks RUSTFLAGS, so
# it goes too. With `--target`, cargo does not pass RUSTFLAGS to build scripts,
# so the host build cache is untouched.
#
# Native-only and out of the set: polyoxide-kalshi, the relay, rtds, the
# polyoxide umbrella, polyoxide-cli, polyoxide-py and every `keychain` feature.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

unset CARGO_ENCODED_RUSTFLAGS
export RUSTFLAGS='--cfg getrandom_backend="wasm_js"'

target=wasm32-unknown-unknown

check() {
    local package=$1
    shift
    echo "==> ${package} $*"
    cargo check --package "${package}" --target "${target}" "$@"
    cargo clippy --package "${package}" --target "${target}" "$@" -- -D warnings
}

check polyoxide-rt
check polyoxide-venue --features market,trading
check polyoxide-core
check polyoxide-ws
check polyoxide-binance --no-default-features --features usdm,usdm-ws
check polyoxide-polymarket --no-default-features \
    --features clob,gamma,data,perps,perps-ws,sports,clob-ws
```

Then `chmod +x scripts/wasm_check.sh`. The script cannot pass until Tasks 7.2 to 7.5 land; Task 7.8 runs it whole.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command bash -c 'ls "$(rustc --print target-libdir --target wasm32-unknown-unknown)" | head -3'
nix develop --command bash -n scripts/wasm_check.sh
```

The first prints library files (`libcore-<hash>.rlib` among them); the second prints nothing.

- [ ] **Step 5: Commit**

```bash
git add -N scripts/wasm_check.sh
git commit -m "build(wasm): add the wasm32 target to the dev shell and scripts/wasm_check.sh (story 7)" -- flake.nix Cargo.toml CLAUDE.md scripts/wasm_check.sh
```

### Task 7.2: `BrowserRuntime` and `polyoxide-rt`'s target tables

**Files:**
- Create: `polyoxide-rt/src/browser.rs`
- Modify: `polyoxide-rt/src/lib.rs` (the `mod` list and `default_runtime`)
- Modify: `polyoxide-rt/Cargo.toml` (a `wasm32` target table, and `notes`)
- Modify: `CLAUDE.md` (the generated crate list, by `gen_registry.py --write`)
- Test: `polyoxide-rt/src/browser.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-rt/src/browser.rs` with the tests only, and add `mod browser;` to `polyoxide-rt/src/lib.rs` beside the other `mod` lines.

```rust
//! The browser's runtime, and the sleep arithmetic its tests pin on the host.

#[cfg(any(target_arch = "wasm32", test))]
mod chunks {
    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use super::{step_ms, steps};

        /// The longest delay `setTimeout` takes: a larger one fires at once.
        const LONGEST: u32 = i32::MAX as u32;

        #[test]
        fn a_step_is_the_remainder_rounded_up_to_a_millisecond() {
            assert_eq!(step_ms(Duration::ZERO), 0);
            assert_eq!(step_ms(Duration::from_nanos(1)), 1);
            assert_eq!(step_ms(Duration::from_millis(1)), 1);
            assert_eq!(step_ms(Duration::from_nanos(1_000_001)), 2);
            assert_eq!(step_ms(Duration::from_secs(90)), 90_000);
        }

        #[test]
        fn a_step_never_exceeds_what_set_timeout_accepts() {
            assert_eq!(step_ms(Duration::from_millis(u64::from(LONGEST))), LONGEST);
            assert_eq!(
                step_ms(Duration::from_millis(u64::from(LONGEST) + 1)),
                LONGEST
            );
            assert_eq!(step_ms(Duration::MAX), LONGEST);
        }

        #[test]
        fn a_long_sleep_is_chained_in_whole_steps() {
            let total = Duration::from_millis(2 * u64::from(LONGEST) + 5);
            assert_eq!(steps(total).collect::<Vec<_>>(), [LONGEST, LONGEST, 5]);
        }

        #[test]
        fn the_last_step_is_rounded_up_so_a_sleep_is_never_short() {
            let total = Duration::from_nanos(u64::from(LONGEST) * 1_000_000 + 1);
            assert_eq!(steps(total).collect::<Vec<_>>(), [LONGEST, 1]);
        }

        #[test]
        fn no_time_is_no_step_and_a_sub_millisecond_sleep_is_one() {
            assert_eq!(steps(Duration::ZERO).count(), 0);
            assert_eq!(steps(Duration::from_micros(10)).collect::<Vec<_>>(), [1]);
        }

        #[test]
        fn the_longest_sleep_starts_with_full_steps() {
            let first: Vec<u32> = steps(Duration::MAX).take(2).collect();
            assert_eq!(first, [LONGEST, LONGEST]);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod native_default {
    #[test]
    fn the_host_default_stays_tokio() {
        assert_eq!(format!("{:?}", crate::default_runtime()), "TokioRuntime");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-rt --all-features --lib browser::
```

It fails to compile:

```
error[E0432]: unresolved imports `super::step_ms`, `super::steps`
```

- [ ] **Step 3: Implement**

In `polyoxide-rt/src/browser.rs`, insert the two functions inside `mod chunks`, directly after `mod chunks {` and before `#[cfg(test)]`:

```rust
    use std::time::Duration;

    /// The next `setTimeout` for a sleep with `remaining` left, in
    /// milliseconds: the rest rounded up to a whole millisecond, at most
    /// `i32::MAX`, since the browser fires a timer with a larger delay at once.
    pub(super) fn step_ms(remaining: Duration) -> u32 {
        let millis = remaining.as_nanos().div_ceil(1_000_000);
        millis.min(i32::MAX as u128) as u32
    }

    /// The timers that, run one after another, sleep `duration`: none for no
    /// time, one per `i32::MAX` milliseconds, the last rounded up.
    pub(super) fn steps(duration: Duration) -> impl Iterator<Item = u32> {
        let mut remaining = duration;
        std::iter::from_fn(move || {
            if remaining.is_zero() {
                return None;
            }
            let step = step_ms(remaining);
            remaining = remaining.saturating_sub(Duration::from_millis(u64::from(step)));
            Some(step)
        })
    }
```

Then add, after the closing brace of `mod chunks` and before `mod native_default`'s attribute:

```rust
#[cfg(target_arch = "wasm32")]
mod web {
    use std::time::Duration;

    use gloo_timers::future::TimeoutFuture;
    use send_wrapper::SendWrapper;

    use super::chunks::steps;
    use crate::{BoxFuture, Instant, Runtime};

    /// The runtime of a browser page or Worker: `spawn_local` for tasks,
    /// `setTimeout` for sleeps and `performance.now` for the clock.
    ///
    /// A timer's JS handle is `!Send`, so each sleep sits in a
    /// `SendWrapper`: the `Send` bound of [`Runtime`] holds, and the handle is
    /// only ever touched on the thread that made it. That is the one thread a
    /// browser build has; a build with atomics (threads) is unsupported, since
    /// the wrapper panics off its thread.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct BrowserRuntime;

    impl Runtime for BrowserRuntime {
        fn spawn(&self, task: BoxFuture<()>) {
            wasm_bindgen_futures::spawn_local(task);
        }

        fn sleep(&self, duration: Duration) -> BoxFuture<()> {
            Box::pin(SendWrapper::new(async move {
                // `setTimeout` takes an `i32`, so a long sleep is a chain.
                for step in steps(duration) {
                    TimeoutFuture::new(step).await;
                }
            }))
        }

        fn now(&self) -> Instant {
            Instant::now()
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::BrowserRuntime;
```

In `polyoxide-rt/src/lib.rs`, re-export it and make the default pick by target. Add, beside the other `pub use` lines:

```rust
#[cfg(target_arch = "wasm32")]
pub use browser::BrowserRuntime;
```

and replace the whole `default_runtime` function (its doc comment and body; its signature is `pub fn default_runtime() -> std::sync::Arc<dyn Runtime>`) with:

```rust
/// The runtime a client uses when none is injected: `TokioRuntime` on a host
/// target and `BrowserRuntime` on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_runtime() -> std::sync::Arc<dyn Runtime> {
    std::sync::Arc::new(TokioRuntime)
}

/// The runtime a client uses when none is injected: `TokioRuntime` on a host
/// target and `BrowserRuntime` on `wasm32`.
#[cfg(target_arch = "wasm32")]
pub fn default_runtime() -> std::sync::Arc<dyn Runtime> {
    std::sync::Arc::new(BrowserRuntime)
}
```

In `polyoxide-rt/Cargo.toml`, Story 1 left `tokio` in a native table, so `[dependencies]` stays as it is and only the browser's table is new. Story 1's text there is:

```toml
[dependencies]
# `Task`: an abortable future, and a oneshot for its output.
futures-channel = { workspace = true }
futures-util = { workspace = true }
web-time = { workspace = true }

# Native only (spec D10): `rt` and `time` are `TokioRuntime`'s spawn and clock.
# The browser has its own runtime and builds no tokio.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { workspace = true, features = ["rt", "time"] }
```

Directly after that native table, add:

```toml

# `BrowserRuntime` (D8). `getrandom` 0.3's `wasm_js` backend is on here so
# feature unification reaches `rand` and `tungstenite` from every crate that
# builds this one; the final binary still sets
# `--cfg getrandom_backend="wasm_js"`.
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
gloo-timers = { workspace = true, features = ["futures"] }
send_wrapper = { workspace = true, features = ["futures"] }
wasm-bindgen-futures = { workspace = true }
```

If `[dev-dependencies]` names `tokio` with `test-util`, it stays: tests run on the host.

Then replace the crate's `notes` line under `[package.metadata.polyoxide]` (Story 1's)

```toml
notes = ["names no venue, which `scripts/venue_gate.py` holds, and builds only futures-channel, futures-util and web-time, with tokio (`rt` and `time`) off wasm32"]
```

with

```toml
notes = ["names no venue, which `scripts/venue_gate.py` holds, and builds futures-channel, futures-util and web-time on every target, tokio (`rt` and `time`) on a host, and getrandom, gloo-timers, send_wrapper and wasm-bindgen-futures on `wasm32`"]
```

and regenerate CLAUDE.md's crate list from it:

```bash
nix develop --command python3 scripts/gen_registry.py --write
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-rt --all-features
nix develop --command cargo clippy -p polyoxide-rt --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-rt --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-rt --target wasm32-unknown-unknown
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo clippy -p polyoxide-rt --target wasm32-unknown-unknown -- -D warnings
```

The first prints the seven new tests under `browser::` as `ok` beside Story 1's, and `test result: ok`. The rest finish with no warning; the last two print `Finished`.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-rt/src/browser.rs
git commit -m "feat(rt): add BrowserRuntime and the wasm32 dependency table (story 7)" -- polyoxide-rt/src/browser.rs polyoxide-rt/src/lib.rs polyoxide-rt/Cargo.toml CLAUDE.md Cargo.lock
```

### Task 7.3: `FetchTransport` and core's target tables

**Files:**
- Create: `polyoxide-core/src/fetch.rs`
- Modify: `polyoxide-core/src/lib.rs` (the `mod` list, `pub use reqwest;`, the exports)
- Modify: `polyoxide-core/src/client.rs` (`HttpClientBuilder::build`)
- Modify: `polyoxide-core/src/transport.rs` (the host-only gate on `ReqwestTransport`)
- Modify: `polyoxide-core/Cargo.toml`
- Test: `polyoxide-core/src/fetch.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-core/src/fetch.rs` with the tests only, and add `mod fetch;` to `polyoxide-core/src/lib.rs` between `pub mod error;` and `pub mod health;`.

```rust
//! The browser's HTTP transport.
//!
//! `FetchTransport`, built only for `wasm32`, sends through the platform's
//! `fetch`, by way of reqwest's wasm client. What it decides itself is which
//! `TransportKind` a failure is, and that is plain Rust, built on every
//! target so the host's tests pin it.

#[cfg(test)]
mod tests {
    use polyoxide_venue::Classify;

    use super::{kind_of, Stage};
    use crate::{TransportError, TransportKind};

    #[test]
    fn a_timeout_is_a_timeout_in_either_stage() {
        for stage in [Stage::Fetch, Stage::Body] {
            assert_eq!(kind_of(stage, true, false), TransportKind::Timeout);
        }
    }

    #[test]
    fn a_timeout_outranks_a_request_the_browser_refused() {
        assert_eq!(kind_of(Stage::Fetch, true, true), TransportKind::Timeout);
    }

    #[test]
    fn a_failed_fetch_is_a_connect_failure() {
        // A network failure and a CORS refusal look alike to a page.
        assert_eq!(kind_of(Stage::Fetch, false, false), TransportKind::Connect);
    }

    #[test]
    fn a_failed_body_read_is_a_body_failure() {
        assert_eq!(kind_of(Stage::Body, false, false), TransportKind::Body);
    }

    #[test]
    fn a_request_the_browser_refused_to_build_is_a_request_failure() {
        for stage in [Stage::Fetch, Stage::Body] {
            assert_eq!(kind_of(stage, false, true), TransportKind::Request);
        }
    }

    #[test]
    fn a_failed_fetch_is_retried_and_a_refused_request_is_not() {
        let lost = TransportError::new(kind_of(Stage::Fetch, false, false), "network error");
        assert!(lost.is_retriable());
        let refused = TransportError::new(kind_of(Stage::Fetch, false, true), "bad header value");
        assert!(!refused.is_retriable());
    }
}
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-core --all-features --lib fetch::
```

It fails to compile:

```
error[E0432]: unresolved imports `super::kind_of`, `super::Stage`
```

- [ ] **Step 3: Implement**

In `polyoxide-core/src/fetch.rs`, insert directly after the module docs and before `#[cfg(test)]`:

```rust
#[cfg(any(target_arch = "wasm32", test))]
use crate::TransportKind;

/// Where in an exchange a failure struck.
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Before the response head arrived.
    Fetch,
    /// While the body was being read.
    Body,
}

/// The kind of a failed exchange.
///
/// A timeout is a timeout wherever it struck. A request the browser would not
/// build (a header value it cannot carry) is the caller's: `Request`, which
/// no retry fixes. Any other failed `fetch` is `Connect`: the browser tells a
/// network failure and a CORS refusal apart for no page. A failed body read is
/// `Body`.
#[cfg(any(target_arch = "wasm32", test))]
fn kind_of(stage: Stage, timed_out: bool, malformed: bool) -> TransportKind {
    if timed_out {
        TransportKind::Timeout
    } else if malformed {
        TransportKind::Request
    } else {
        match stage {
            Stage::Fetch => TransportKind::Connect,
            Stage::Body => TransportKind::Body,
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use bytes::Bytes;
    use polyoxide_rt::BoxFuture;
    use send_wrapper::SendWrapper;

    use super::{kind_of, Stage};
    use crate::{HttpRequest, HttpTransport, Response, TransportError};

    /// An `HttpTransport` over the browser's `fetch`, the default of a
    /// `wasm32` client.
    ///
    /// The request's timeout is set per request and bounds the whole exchange,
    /// body included. The browser negotiates compression itself and follows
    /// redirects. Each exchange sits in a `SendWrapper`, since its JS handles
    /// are `!Send`: the `Send` bound of `HttpTransport` holds, and the handles
    /// stay on the one thread a browser build has (a build with atomics is
    /// unsupported).
    #[derive(Debug, Clone, Default)]
    #[non_exhaustive]
    pub struct FetchTransport {}

    impl FetchTransport {
        /// A transport over the page's `fetch`.
        pub fn new() -> Self {
            Self {}
        }
    }

    impl HttpTransport for FetchTransport {
        fn send(&self, request: HttpRequest) -> BoxFuture<Result<Response, TransportError>> {
            Box::pin(SendWrapper::new(fetch(request)))
        }
    }

    async fn fetch(request: HttpRequest) -> Result<Response, TransportError> {
        let mut builder = reqwest::Client::new()
            .request(request.method, request.url)
            .headers(request.headers)
            .timeout(request.timeout);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder
            .send()
            .await
            .map_err(|error| failure(Stage::Fetch, error))?;
        let status = response.status();
        let headers = response.headers().clone();
        let url = response.url().clone();
        // The body is read lazily, as `Response` promises; the reqwest response
        // keeps the timeout's abort guard alive until it is read or dropped.
        let body: BoxFuture<Result<Bytes, TransportError>> =
            Box::pin(SendWrapper::new(async move {
                response
                    .bytes()
                    .await
                    .map_err(|error| failure(Stage::Body, error))
            }));
        Ok(Response::new(status, headers, url, body))
    }

    fn failure(stage: Stage, error: reqwest::Error) -> TransportError {
        TransportError::new(
            kind_of(stage, error.is_timeout(), error.is_builder()),
            error,
        )
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::FetchTransport;
```

In `polyoxide-core/src/lib.rs`:

- Put the host-only attribute on the reqwest re-export (D6): the line `pub use reqwest;`, below its doc comment, becomes

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use reqwest;
```

- Beside the other `pub use` lines, add

```rust
#[cfg(target_arch = "wasm32")]
pub use fetch::FetchTransport;
```

- Story 4 left `ReqwestTransport` ungated, and its export line `pub use transport::ReqwestTransport;` too. Put `#[cfg(not(target_arch = "wasm32"))]` on that export line, as a line of its own directly above it.

In `polyoxide-core/src/transport.rs`, put `#[cfg(not(target_arch = "wasm32"))]` on each item that exists only for reqwest's host client, as a line of its own directly above the item (below its doc comment, above any `#[derive]`): `pub struct ReqwestTransport`, each `impl` block of `ReqwestTransport` (the inherent one and `impl HttpTransport for ReqwestTransport`), the private function that maps a reqwest error to a `TransportError` (it names `is_connect`, which reqwest's wasm client lacks), and the file's `#[cfg(test)] mod tests`, whose tests drive `ReqwestTransport` against a mock server. Where an item above already carries the attribute, leave it. If `cargo clippy` for wasm32 reports an `unused_imports` warning in the file, put the same attribute on each import it names. `HttpTransport`, `HttpRequest`, `TransportError` and `TransportKind` stay on every target.

In `polyoxide-core/src/client.rs`, `HttpClientBuilder::build` picks the transport: the injected `http_transport` when set, else (Story 4) a `ReqwestTransport` it builds from the config's timeout, pool size and `gzip` setting, with `connect_timeout` 10 s and no redirects. Keep the injected case as it is and make the default branch target-specific. Move into a private method, unchanged, every statement `build` runs only to make that default (the `reqwest::Client::builder()` chain, its `.build()` and the `ReqwestTransport::new(..)` call), prefixing the builder's fields with `self.`; reorder `build` so it calls the method before it moves any field out of `self`:

```rust
impl HttpClientBuilder {
    /// The transport a client uses when none is injected: reqwest's on a host
    /// target, the browser's `fetch` on `wasm32`.
    #[cfg(not(target_arch = "wasm32"))]
    fn default_transport(&self) -> Result<Arc<dyn HttpTransport>, ApiError> {
        // ... the statements moved out of `build`, ending in the
        // `ReqwestTransport::new(..)` call wrapped as:
        Ok(Arc::new(ReqwestTransport::new(client)))
    }

    /// The transport a client uses when none is injected: reqwest's on a host
    /// target, the browser's `fetch` on `wasm32`.
    #[cfg(target_arch = "wasm32")]
    fn default_transport(&self) -> Result<Arc<dyn HttpTransport>, ApiError> {
        Ok(Arc::new(crate::FetchTransport::new()))
    }
}
```

`pool_size` and `gzip` feed only the host default; on `wasm32` the builder's fields `pool_size` and `gzip` are read by nothing, so if `cargo clippy` for wasm32 reports them as `dead_code`, put `#[cfg_attr(target_arch = "wasm32", allow(dead_code))]` on each of the two fields.

In `polyoxide-core/Cargo.toml`, reqwest becomes a host dependency with a wasm32 build of its own, and the wasm32-only dependencies join. Remove the `reqwest = { version = "0.12", features = ["json", "rustls-tls", "gzip"], default-features = false }` line from `[dependencies]` (its comment moves with it) and add, after that table:

```toml
# The one HTTP client, and the only member that declares it (AD-18): every
# venue crate names its types through `polyoxide_core::reqwest`, so no other
# crate can change its features for the rest. `gzip` is on, so every client
# asks for compressed bodies unless a builder turns it off.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
reqwest = { version = "0.12", features = ["json", "rustls-tls", "gzip"], default-features = false }

# The browser's `fetch`, through reqwest's wasm client (`FetchTransport`): no
# TLS, no compression setting and no pool, since the browser has its own.
# Beyond spec D10: fastrand's `js` feature seeds the retry jitter, which otherwise has one fixed seed on wasm32.
[target.'cfg(target_arch = "wasm32")'.dependencies]
reqwest = { version = "0.12", default-features = false }
send_wrapper = { workspace = true, features = ["futures"] }
fastrand = { workspace = true, features = ["js"] }
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-core --all-features
nix develop --command cargo clippy -p polyoxide-core --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-core --all-features'
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-core --target wasm32-unknown-unknown
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo clippy -p polyoxide-core --target wasm32-unknown-unknown -- -D warnings
```

The first prints the six `fetch::tests` as `ok` and `test result: ok` for every other core test, unchanged. The last two finish with `Finished`. When the wasm32 check prints an error in code this task did not touch, apply the rule of Task 7.8 (gate the item that names a host-only crate, or put the clock, timer or spawn it names through the injected runtime), then rerun.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-core/src/fetch.rs
git commit -m "feat(core): add FetchTransport and gate reqwest's host build (story 7)" -m "Beyond spec D10, core turns on fastrand's js feature on wasm32: without it the retry jitter's generator has one fixed seed in every page." -- polyoxide-core/src/fetch.rs polyoxide-core/src/lib.rs polyoxide-core/src/client.rs polyoxide-core/src/transport.rs polyoxide-core/Cargo.toml Cargo.lock
```

### Task 7.4: `WebSocketTransport` and the kit's target tables

**Files:**
- Create: `polyoxide-ws/src/browser.rs`
- Modify: `polyoxide-ws/src/lib.rs`
- Modify: `polyoxide-ws/src/transport.rs`
- Modify: `polyoxide-ws/src/connect.rs`
- Modify: `polyoxide-ws/src/bare.rs:12`, `polyoxide-ws/src/error.rs:5`, `polyoxide-ws/src/request.rs:7` and `:202` (the `tungstenite` paths)
- Modify: `polyoxide-ws/Cargo.toml`
- Modify: `polyoxide-ws/README.md`
- Modify: `CLAUDE.md` (the generated crate list, by `gen_registry.py --write`)
- Test: `polyoxide-ws/src/browser.rs`

- [ ] **Step 1: Write the failing test**

Create `polyoxide-ws/src/browser.rs` with the tests only, and add `mod browser;` to `polyoxide-ws/src/lib.rs` after `mod bare;`.

```rust
//! The browser's socket transport.
//!
//! `WebSocketTransport`, built only for `wasm32`, dials with the platform's
//! `WebSocket`. A browser socket cannot set a handshake header, send a ping or
//! pong frame or apply backpressure. What the transport decides before it
//! touches the platform (which requests it refuses, which close codes it may
//! send, which frames it cannot) is plain Rust, built on every target so the
//! host's tests pin it.

#[cfg(any(target_arch = "wasm32", test))]
mod plain {
    #[cfg(test)]
    mod tests {
        use std::io;

        use crate::tungstenite::{
            client::IntoClientRequest,
            error::UrlError,
            http::{header, HeaderValue, Request},
            protocol::{frame::coding::CloseCode, frame::Frame, CloseFrame},
            Error, Message,
        };
        use crate::{retrying_can_fix, Kind, WsError};

        use super::{close_code, close_reason, dial_plan, outgoing, Dial, Outgoing};

        fn request(url: &str) -> Request<()> {
            url.into_client_request().expect("a valid WebSocket URL")
        }

        fn unsupported_message(err: &Error) -> Option<String> {
            match err {
                Error::Io(inner) if inner.kind() == io::ErrorKind::Unsupported => {
                    Some(inner.to_string())
                }
                _ => None,
            }
        }

        #[test]
        fn the_request_tungstenite_builds_is_dialled_by_url_alone() {
            let plan = dial_plan(&request("wss://stream.example.invalid/ws?x=1"));
            assert_eq!(
                plan.unwrap(),
                Dial {
                    url: "wss://stream.example.invalid/ws?x=1".to_owned(),
                    protocols: Vec::new(),
                }
            );
        }

        #[test]
        fn a_subprotocol_header_becomes_the_subprotocol_list() {
            let mut req = request("ws://stream.example.invalid/");
            req.headers_mut().append(
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static("json, v2"),
            );
            req.headers_mut()
                .append(header::SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("extra"));
            assert_eq!(dial_plan(&req).unwrap().protocols, ["json", "v2", "extra"]);
        }

        #[test]
        fn any_other_handshake_header_is_refused_with_its_message() {
            let mut req = request("wss://stream.example.invalid/");
            req.headers_mut()
                .insert("x-api-key", HeaderValue::from_static("secret"));
            let err = dial_plan(&req).unwrap_err();
            assert_eq!(
                unsupported_message(&err).as_deref(),
                Some("a handshake header is not supported by the browser transport")
            );
            assert_eq!(WsError::from(err).kind(), Kind::Unsupported);
        }

        #[test]
        fn a_url_that_is_not_a_websocket_url_is_refused_before_dialling() {
            let req = Request::builder()
                .uri("https://stream.example.invalid/")
                .body(())
                .unwrap();
            let err = dial_plan(&req).unwrap_err();
            assert!(matches!(err, Error::Url(UrlError::UnsupportedUrlScheme)));
            let err = WsError::from(err);
            assert_eq!(err.kind(), Kind::Url);
            assert!(!retrying_can_fix(&err, |_| true));
        }

        #[test]
        fn a_close_code_goes_out_only_when_a_browser_may_send_it() {
            for code in [1000, 3000, 3999, 4000, 4999] {
                assert_eq!(close_code(code), code, "{code}");
            }
            // A browser throws on every other code, 1001 and 1011 included.
            for code in [0, 999, 1001, 1002, 1005, 1006, 1011, 2999, 5000, u16::MAX] {
                assert_eq!(close_code(code), 1000, "{code}");
            }
        }

        #[test]
        fn a_close_reason_is_cut_to_the_browsers_123_bytes_on_a_char_boundary() {
            assert_eq!(close_reason("going away"), "going away");
            let exact = "a".repeat(123);
            assert_eq!(close_reason(&exact), exact);
            assert_eq!(close_reason(&"a".repeat(124)).len(), 123);
            // 100 two-byte characters: 123 falls inside one, so 122 bytes stay.
            let wide = "é".repeat(100);
            let cut = close_reason(&wide);
            assert_eq!(cut.len(), 122);
            assert!(wide.starts_with(cut));
        }

        #[test]
        fn text_binary_and_close_frames_are_sent() {
            assert_eq!(outgoing(&Message::text("hi")).unwrap(), Outgoing::Text("hi"));
            assert_eq!(
                outgoing(&Message::binary(vec![1, 2, 3])).unwrap(),
                Outgoing::Binary(&[1, 2, 3])
            );
            let away = Message::Close(Some(CloseFrame {
                code: CloseCode::from(1001),
                reason: "bye".into(),
            }));
            assert_eq!(
                outgoing(&away).unwrap(),
                Outgoing::Close {
                    code: 1000,
                    reason: "bye"
                }
            );
            let library = Message::Close(Some(CloseFrame {
                code: CloseCode::from(4000),
                reason: "".into(),
            }));
            assert_eq!(
                outgoing(&library).unwrap(),
                Outgoing::Close {
                    code: 4000,
                    reason: ""
                }
            );
            assert_eq!(
                outgoing(&Message::Close(None)).unwrap(),
                Outgoing::Close {
                    code: 1000,
                    reason: ""
                }
            );
        }

        #[test]
        fn ping_pong_and_raw_frames_are_refused_with_their_messages() {
            let cases = [
                (
                    Message::Ping(Vec::<u8>::new().into()),
                    "a ping frame is not supported by the browser transport",
                ),
                (
                    Message::Pong(Vec::<u8>::new().into()),
                    "a pong frame is not supported by the browser transport",
                ),
                (
                    Message::Frame(Frame::ping(Vec::<u8>::new())),
                    "a raw frame is not supported by the browser transport",
                ),
            ];
            for (message, text) in cases {
                let err = outgoing(&message).unwrap_err();
                assert_eq!(unsupported_message(&err).as_deref(), Some(text));
                assert_eq!(WsError::from(err).kind(), Kind::Unsupported);
            }
        }
    }
}
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command cargo test -p polyoxide-ws --all-features --lib browser::
```

It fails to compile:

```
error[E0432]: unresolved imports `super::close_code`, `super::close_reason`, `super::dial_plan`, `super::outgoing`, `super::Dial`, `super::Outgoing`
```

- [ ] **Step 3: Implement**

**(a) The decisions and the transport**, in `polyoxide-ws/src/browser.rs`. Insert inside `mod plain`, directly after `mod plain {` and before `#[cfg(test)]`:

```rust
    use std::io;

    use crate::tungstenite::{
        error::UrlError,
        http::{header, HeaderName, Request},
        Error, Message,
    };

    const HEADER_UNSUPPORTED: &str =
        "a handshake header is not supported by the browser transport";
    const PING_UNSUPPORTED: &str =
        "a ping frame is not supported by the browser transport";
    const PONG_UNSUPPORTED: &str =
        "a pong frame is not supported by the browser transport";
    const FRAME_UNSUPPORTED: &str =
        "a raw frame is not supported by the browser transport";

    /// The longest close reason a browser takes: it throws on more.
    const CLOSE_REASON_MAX: usize = 123;

    /// An operation the browser cannot perform, in the form `WsError::kind`
    /// reads as `Kind::Unsupported`.
    fn unsupported(message: &'static str) -> Error {
        Error::Io(io::Error::new(io::ErrorKind::Unsupported, message))
    }

    /// What the browser needs to open a socket.
    #[derive(Debug, PartialEq, Eq)]
    pub(super) struct Dial {
        pub(super) url: String,
        pub(super) protocols: Vec<String>,
    }

    /// The headers tungstenite generates for every handshake request, which
    /// the browser generates for itself.
    fn is_generated(name: &HeaderName) -> bool {
        [
            header::HOST,
            header::CONNECTION,
            header::UPGRADE,
            header::SEC_WEBSOCKET_VERSION,
            header::SEC_WEBSOCKET_KEY,
        ]
        .contains(name)
    }

    /// Read a handshake request as the browser can act on it.
    ///
    /// The URL must be `ws` or `wss`. A `Sec-WebSocket-Protocol` header
    /// becomes the subprotocol list; any other header beyond the generated
    /// ones is refused, since a browser cannot set it.
    pub(super) fn dial_plan(request: &Request<()>) -> Result<Dial, Error> {
        let uri = request.uri();
        if !matches!(uri.scheme_str(), Some("ws" | "wss")) {
            return Err(Error::Url(UrlError::UnsupportedUrlScheme));
        }
        let mut protocols = Vec::new();
        for (name, value) in request.headers() {
            if *name == header::SEC_WEBSOCKET_PROTOCOL {
                let list = value
                    .to_str()
                    .map_err(|_| unsupported(HEADER_UNSUPPORTED))?;
                protocols.extend(
                    list.split(',')
                        .map(str::trim)
                        .filter(|protocol| !protocol.is_empty())
                        .map(str::to_owned),
                );
            } else if !is_generated(name) {
                return Err(unsupported(HEADER_UNSUPPORTED));
            }
        }
        Ok(Dial {
            url: uri.to_string(),
            protocols,
        })
    }

    /// The close code a browser may send: 1000, or one in 3000-4999. A browser
    /// throws on any other, so any other goes out as 1000.
    pub(super) const fn close_code(code: u16) -> u16 {
        if matches!(code, 1000 | 3000..=4999) {
            code
        } else {
            1000
        }
    }

    /// The close reason cut to what a browser takes, on a character boundary.
    pub(super) fn close_reason(reason: &str) -> &str {
        &reason[..reason.floor_char_boundary(CLOSE_REASON_MAX)]
    }

    /// A frame the browser can send, with the form it takes there.
    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum Outgoing<'a> {
        Text(&'a str),
        Binary(&'a [u8]),
        Close { code: u16, reason: &'a str },
    }

    /// What sending `message` means to the browser, or why it cannot.
    pub(super) fn outgoing(message: &Message) -> Result<Outgoing<'_>, Error> {
        match message {
            Message::Text(text) => Ok(Outgoing::Text(text.as_str())),
            Message::Binary(bytes) => Ok(Outgoing::Binary(bytes.as_ref())),
            Message::Close(Some(frame)) => Ok(Outgoing::Close {
                code: close_code(u16::from(frame.code)),
                reason: close_reason(frame.reason.as_str()),
            }),
            Message::Close(None) => Ok(Outgoing::Close {
                code: 1000,
                reason: "",
            }),
            Message::Ping(_) => Err(unsupported(PING_UNSUPPORTED)),
            Message::Pong(_) => Err(unsupported(PONG_UNSUPPORTED)),
            Message::Frame(_) => Err(unsupported(FRAME_UNSUPPORTED)),
        }
    }
```

Then add, after the closing brace of `mod plain`:

```rust
#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        io,
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
        time::Duration,
    };

    use futures_util::{Sink, Stream};
    use polyoxide_rt::{BoxFuture, Runtime};
    use send_wrapper::SendWrapper;
    use tokio::sync::mpsc;
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

    use super::plain::{dial_plan, outgoing, Dial, Outgoing};
    use crate::tungstenite::{
        error::UrlError,
        http::Request,
        protocol::{frame::coding::CloseCode, CloseFrame},
        Error, Message,
    };
    use crate::{BoxSocket, Capabilities, SocketTransport, WsError};

    /// The browser could not open the socket; a refused upgrade, a blocked
    /// host and a lost network look alike to a page.
    const NOT_OPENED: &str = "the browser could not open the WebSocket connection";
    /// The browser reported an error event on an open socket.
    const FAILED: &str = "the browser reported a WebSocket error";

    /// What the platform's callbacks hand the socket.
    enum Incoming {
        Open,
        Message(Message),
        Failed,
        Closed(CloseFrame),
    }

    /// The four callbacks, kept alive as long as the socket.
    struct Callbacks {
        _open: Closure<dyn FnMut(Event)>,
        _message: Closure<dyn FnMut(MessageEvent)>,
        _error: Closure<dyn FnMut(Event)>,
        _close: Closure<dyn FnMut(CloseEvent)>,
    }

    /// A text or binary frame as the kit sees it; anything else a browser
    /// might deliver is dropped.
    fn decode(data: &JsValue) -> Option<Message> {
        if let Some(text) = data.as_string() {
            return Some(Message::text(text));
        }
        if data.is_instance_of::<js_sys::ArrayBuffer>() {
            return Some(Message::binary(js_sys::Uint8Array::new(data).to_vec()));
        }
        None
    }

    fn attach(ws: &WebSocket, tx: &mpsc::UnboundedSender<Incoming>) -> Callbacks {
        let sender = tx.clone();
        let open = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = sender.send(Incoming::Open);
        });
        ws.set_onopen(Some(open.as_ref().unchecked_ref()));

        let sender = tx.clone();
        let message = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(message) = decode(&event.data()) {
                let _ = sender.send(Incoming::Message(message));
            }
        });
        ws.set_onmessage(Some(message.as_ref().unchecked_ref()));

        let sender = tx.clone();
        let error = Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = sender.send(Incoming::Failed);
        });
        ws.set_onerror(Some(error.as_ref().unchecked_ref()));

        let sender = tx.clone();
        let close = Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
            let _ = sender.send(Incoming::Closed(CloseFrame {
                code: CloseCode::from(event.code()),
                reason: event.reason().into(),
            }));
        });
        ws.set_onclose(Some(close.as_ref().unchecked_ref()));

        Callbacks {
            _open: open,
            _message: message,
            _error: error,
            _close: close,
        }
    }

    fn js_error(error: JsValue) -> Error {
        Error::Io(io::Error::other(format!("{error:?}")))
    }

    /// One open browser socket. The `WebSocket` and its callbacks are `!Send`
    /// JS handles, so each sits in a `SendWrapper`: the socket is `Send`, as
    /// `BoxSocket` requires, and every touch of a handle (including its drop)
    /// is on the one thread a browser build has. Events arrive on an
    /// unbounded channel, since a browser socket cannot apply backpressure.
    struct BrowserSocket {
        ws: SendWrapper<WebSocket>,
        _callbacks: SendWrapper<Callbacks>,
        incoming: mpsc::UnboundedReceiver<Incoming>,
        ended: bool,
    }

    impl Unpin for BrowserSocket {}

    impl BrowserSocket {
        fn open(dial: &Dial) -> Result<Self, WsError> {
            let ws = if dial.protocols.is_empty() {
                WebSocket::new(&dial.url)
            } else {
                let list = js_sys::Array::new();
                for protocol in &dial.protocols {
                    list.push(&JsValue::from_str(protocol));
                }
                WebSocket::new_with_str_sequence(&dial.url, &list)
            }
            .map_err(|error| {
                WsError::from(Error::Url(UrlError::UnableToConnect(format!("{error:?}"))))
            })?;
            ws.set_binary_type(BinaryType::Arraybuffer);
            let (tx, incoming) = mpsc::unbounded_channel();
            let callbacks = attach(&ws, &tx);
            Ok(Self {
                ws: SendWrapper::new(ws),
                _callbacks: SendWrapper::new(callbacks),
                incoming,
                ended: false,
            })
        }

        /// Wait for the upgrade. A failure or close before it opens is a lost
        /// connection: a browser shows no status for a refused upgrade.
        async fn opened(&mut self) -> Result<(), WsError> {
            loop {
                match self.incoming.recv().await {
                    Some(Incoming::Open) => return Ok(()),
                    Some(Incoming::Message(_)) => {}
                    Some(Incoming::Failed) => {
                        return Err(WsError::from(Error::Io(io::Error::new(
                            io::ErrorKind::ConnectionRefused,
                            NOT_OPENED,
                        ))))
                    }
                    Some(Incoming::Closed(frame)) => {
                        return Err(WsError::from_close_frame(Some(&frame)))
                    }
                    None => return Err(WsError::from(Error::ConnectionClosed)),
                }
            }
        }

        fn writable(&self) -> Result<(), Error> {
            if self.ws.ready_state() >= WebSocket::CLOSING {
                Err(Error::AlreadyClosed)
            } else {
                Ok(())
            }
        }
    }

    impl Drop for BrowserSocket {
        fn drop(&mut self) {
            // Detach first, so a late event never reaches a freed closure.
            self.ws.set_onopen(None);
            self.ws.set_onmessage(None);
            self.ws.set_onerror(None);
            self.ws.set_onclose(None);
            if self.ws.ready_state() < WebSocket::CLOSING {
                let _ = self.ws.close();
            }
        }
    }

    impl Stream for BrowserSocket {
        type Item = Result<Message, Error>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let this = &mut *self;
            loop {
                if this.ended {
                    return Poll::Ready(None);
                }
                return match this.incoming.poll_recv(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(None) => {
                        this.ended = true;
                        Poll::Ready(None)
                    }
                    Poll::Ready(Some(Incoming::Open)) => continue,
                    Poll::Ready(Some(Incoming::Message(message))) => {
                        Poll::Ready(Some(Ok(message)))
                    }
                    Poll::Ready(Some(Incoming::Failed)) => {
                        Poll::Ready(Some(Err(Error::Io(io::Error::other(FAILED)))))
                    }
                    Poll::Ready(Some(Incoming::Closed(frame))) => {
                        this.ended = true;
                        Poll::Ready(Some(Ok(Message::Close(Some(frame)))))
                    }
                };
            }
        }
    }

    impl Sink<Message> for BrowserSocket {
        type Error = Error;

        fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            Poll::Ready(self.writable())
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Error> {
            let frame = outgoing(&item)?;
            self.writable()?;
            let sent = match frame {
                Outgoing::Text(text) => self.ws.send_with_str(text),
                Outgoing::Binary(bytes) => self.ws.send_with_u8_array(bytes),
                Outgoing::Close { code, reason } => {
                    self.ws.close_with_code_and_reason(code, reason)
                }
            };
            sent.map_err(js_error)
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            // A browser sends as it is told; there is nothing to flush.
            Poll::Ready(Ok(()))
        }

        fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
            if self.ws.ready_state() < WebSocket::CLOSING {
                let _ = self.ws.close();
            }
            Poll::Ready(Ok(()))
        }
    }

    /// A `SocketTransport` over the browser's `WebSocket`, the default of a
    /// `wasm32` socket.
    ///
    /// Its capabilities are all `false`: a browser cannot send a ping, reports
    /// no server ping to the page and takes no handshake header. A request
    /// with a header beyond those tungstenite generates (a
    /// `Sec-WebSocket-Protocol` becomes the subprotocol list) is refused as
    /// `Kind::Unsupported` before dialling, and so is sending a ping, pong or
    /// raw frame. A refused upgrade shows no status to a page, so it arrives
    /// as a lost connection, not `Kind::Refused`. A close event yields
    /// `Message::Close` with its code and reason, then the stream ends.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct WebSocketTransport;

    impl WebSocketTransport {
        /// A transport over the page's `WebSocket`.
        pub const fn new() -> Self {
            Self
        }
    }

    impl SocketTransport for WebSocketTransport {
        fn connect(
            &self,
            request: Request<()>,
            timeout: Duration,
            runtime: Arc<dyn Runtime>,
        ) -> BoxFuture<Result<BoxSocket, WsError>> {
            Box::pin(SendWrapper::new(async move {
                let dial = dial_plan(&request).map_err(WsError::from)?;
                let mut socket = BrowserSocket::open(&dial)?;
                let outcome = polyoxide_rt::timeout(&*runtime, timeout, socket.opened()).await;
                match outcome {
                    Some(Ok(())) => Ok(Box::new(socket) as BoxSocket),
                    Some(Err(error)) => Err(error),
                    // Dropping `socket` closes the half-open connection.
                    None => Err(WsError::connect_timeout(timeout)),
                }
            }))
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::new(false, false, false)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::WebSocketTransport;
```

**(b) The kit's `lib.rs`** (`polyoxide-ws/src/lib.rs`):

- Replace `mod tls;` with

```rust
#[cfg(not(target_arch = "wasm32"))]
mod tls;
```

- Replace `pub use tls::ensure_crypto_provider;` with

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use tls::ensure_crypto_provider;
```

- Replace the line `pub use tokio_tungstenite::tungstenite;` (keep its doc comment) with `pub use tungstenite;`: the direct dependency below is the same crate `tokio-tungstenite` re-exports, and it exists on every target.
- Remove `connect_over` from the braces of the `pub use connect::{ .. };` list and add below it

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use connect::connect_over;
#[cfg(target_arch = "wasm32")]
pub use browser::WebSocketTransport;
```

- Story 5 exports `TungsteniteTransport` beside the other transport names. It must be host-only, as core's `ReqwestTransport` export is: on `wasm32` the type does not exist, so an ungated export fails the check. Find the line:

```bash
grep -n -B1 'pub use transport::TungsteniteTransport;' polyoxide-ws/src/lib.rs
```

  It prints the attribute `#[cfg(not(target_arch = "wasm32"))]` on the line above the export, and the export is then done; when the attribute is absent, put it there as a line of its own, so the lines read

```rust
#[cfg(not(target_arch = "wasm32"))]
pub use transport::TungsteniteTransport;
pub use transport::{default_socket_transport, BoxSocket, Capabilities, SocketIo, SocketTransport};
```

- Gate the test server to hosts. Replace

```rust
#[cfg(any(test, feature = "test-server"))]
#[doc(hidden)]
pub mod test_server;
```

with

```rust
#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "test-server")))]
#[doc(hidden)]
pub mod test_server;
```

- In the crate docs, two intra-doc links name items that now exist on hosts only, and a link to an item the target lacks is unresolved on `wasm32`. In the bullet Story 5 wrote,

```rust
//!   [`BoxSocket`] and reporting its [`Capabilities`]; [`TungsteniteTransport`]
//!   is the native one, and [`transport_setters!`] gives a builder its
```

  replace the first line with

```rust
//!   [`BoxSocket`] and reporting its [`Capabilities`]; `TungsteniteTransport`
```

  and in the bullet that follows it replace `[`connect_over`]` with `` `connect_over` `` (the two names stay in code spans: they are host-only now).

- In the crate docs, after the paragraph that ends "rather than depending on it.", add:

```rust
//! On `wasm32` the kit builds no TLS or TCP stack: its transport is the
//! browser's `WebSocket`, `connect_over` and the scripted test server are
//! host-only, and the `test-server` feature turns on nothing.
```

**(c) The `tungstenite` paths.** With the re-export no longer through `tokio_tungstenite`, the four non-test lines that named it go through the crate. In `polyoxide-ws/src/bare.rs:12` replace `use tokio_tungstenite::tungstenite::Message;` with `use crate::tungstenite::Message;`; in `polyoxide-ws/src/error.rs:5` replace

```rust
use tokio_tungstenite::tungstenite::{self, http::header::RETRY_AFTER, protocol::CloseFrame};
```

with

```rust
use crate::tungstenite::{self, http::header::RETRY_AFTER, protocol::CloseFrame};
```

in `polyoxide-ws/src/request.rs:7` replace `use tokio_tungstenite::tungstenite::Message;` with `use crate::tungstenite::Message;`, and at `:202` replace `WsError::from(tokio_tungstenite::tungstenite::Error::AlreadyClosed)` with `WsError::from(crate::tungstenite::Error::AlreadyClosed)`. Leave the `use` lines inside `#[cfg(test)]` modules as they are: tests build on the host only, and a changed test body would need a drift entry. Then find what Story 5 added:

```bash
grep -rn 'tokio_tungstenite' polyoxide-ws/src
```

Every hit outside `connect.rs`' host-only items, `transport.rs`' host-only items (below), `tls.rs`, `test_server/` and `#[cfg(test)]` modules becomes `crate::tungstenite`.

**(d) `transport.rs`** (Story 5's file). `TungsteniteTransport`, its `impl SocketTransport`, its per-address dial helpers, and every `use` only they need (`tokio::net`, `tokio_tungstenite`, `std::net`, `ensure_crypto_provider`) get `#[cfg(not(target_arch = "wasm32"))]`. The items every target keeps are `SocketIo`, `BoxSocket`, `SocketTransport`, `Capabilities` and `transport_setters!`. Replace the single `default_socket_transport` (signature `pub fn default_socket_transport() -> Arc<dyn SocketTransport>`; replace its doc comment and body) with:

```rust
/// The transport a socket uses when none is injected: `TungsteniteTransport`
/// on a host target and `WebSocketTransport` on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_socket_transport() -> Arc<dyn SocketTransport> {
    Arc::new(TungsteniteTransport::new())
}

/// The transport a socket uses when none is injected: `TungsteniteTransport`
/// on a host target and `WebSocketTransport` on `wasm32`.
#[cfg(target_arch = "wasm32")]
pub fn default_socket_transport() -> Arc<dyn SocketTransport> {
    Arc::new(crate::WebSocketTransport::new())
}
```

**(e) `connect.rs`** (Story 5's file). `connect_over` and the `use` lines only it needs (`tokio::net::TcpStream`, `client_async_tls`, `ensure_crypto_provider`, and `MaybeTlsStream` or `WebSocketStream` if nothing else names them) get `#[cfg(not(target_arch = "wasm32"))]`. `connect`, `with_timeout`, `retrying_can_fix` and `DEFAULT_CONNECT_TIMEOUT` stay on every target, since `connect` dials through `default_socket_transport()`. The `#[cfg(test)]` module is not changed.

**(f) The manifest** (`polyoxide-ws/Cargo.toml`). In `[dependencies]`, remove the `rustls = { workspace = true }` line (with its comment, "Needed only to install a process-default CryptoProvider; see `tls.rs`.") and the `tokio-tungstenite = { workspace = true }` line, replace the `tokio = { workspace = true, features = [..] }` entry (and the comment above it) with the entry below, and add `tungstenite`. Declare `polyoxide-rt = { workspace = true }` there too if Story 5 has not.

```toml
# `macros` and `sync` are the Supervisor's: it waits with `tokio::select!`, and
# the consumer's queue and the membership commands run on `tokio::sync`
# channels. Its timers and tasks go through the injected runtime
# (`polyoxide-rt`), so no tokio timer or spawn feature is built.
tokio = { workspace = true, features = ["macros", "sync"] }
# The message and handshake vocabulary, on every target (`pub use`d as
# `polyoxide_ws::tungstenite`).
tungstenite = { workspace = true }
```

Then add, after `[dependencies]`:

```toml
# The host's transport: tokio's sockets, tokio-tungstenite over them, and the
# rustls provider install it needs (see `tls.rs`).
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
rustls = { workspace = true }
tokio = { workspace = true, features = ["net"] }
tokio-tungstenite = { workspace = true }

# The browser's transport (`WebSocketTransport`, D8): its `WebSocket`, the
# callbacks it needs and the wrapper that keeps their handles on one thread.
[target.'cfg(target_arch = "wasm32")'.dependencies]
js-sys = { workspace = true }
send_wrapper = { workspace = true, features = ["futures"] }
wasm-bindgen = { workspace = true }
web-sys = { workspace = true, features = ["BinaryType", "CloseEvent", "Event", "MessageEvent", "WebSocket"] }
```

In `[package.metadata.polyoxide]`, replace Story 5's `notes` line (the `readme` line stays)

```toml
notes = ["depends on neither `polyoxide-core` nor `polyoxide-venue`, and builds only polyoxide-rt, tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls"]
```

with

```toml
notes = ["depends on neither `polyoxide-core` nor `polyoxide-venue`; on a host it builds tokio, tokio-tungstenite and rustls, and on `wasm32` the browser's `WebSocket` through web-sys, with polyoxide-rt, tungstenite, futures-util, thiserror and tracing on both"]
```

and regenerate CLAUDE.md's crate list:

```bash
nix develop --command python3 scripts/gen_registry.py --write
```

In `polyoxide-ws/README.md` (prose only, no code fence: the file is a doctest), add a paragraph after the one that ends "never installs a subscriber.":

```markdown
On `wasm32` the kit builds no TLS or TCP stack. `WebSocketTransport`, the browser's
`WebSocket`, is the default socket transport there; `connect_over`,
`ensure_crypto_provider`, the scripted test server and `TungsteniteTransport` are built for
native targets only, and the `test-server` feature turns on nothing.
```

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command cargo test -p polyoxide-ws --all-features
nix develop --command cargo clippy -p polyoxide-ws --all-targets --all-features -- -D warnings
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-ws --all-features'
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-ws --target wasm32-unknown-unknown
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo clippy -p polyoxide-ws --target wasm32-unknown-unknown -- -D warnings
```

The first prints the eight `browser::plain::tests` as `ok` beside every other kit test, unchanged. `git diff --stat CLAUDE.md` after `gen_registry.py --write` shows the one changed crate-list line. The last two finish with `Finished`; when the wasm32 check prints an error in code this task did not touch, apply the rule of Task 7.8, then rerun.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-ws/src/browser.rs
git commit -m "feat(ws): add WebSocketTransport and gate the kit's host dependencies (story 7)" -- polyoxide-ws/src/browser.rs polyoxide-ws/src/lib.rs polyoxide-ws/src/transport.rs polyoxide-ws/src/connect.rs polyoxide-ws/src/bare.rs polyoxide-ws/src/error.rs polyoxide-ws/src/request.rs polyoxide-ws/Cargo.toml polyoxide-ws/README.md CLAUDE.md Cargo.lock
```

### Task 7.5: The venue crates' manifests for wasm32

**Files:**
- Modify: `polyoxide-polymarket/Cargo.toml`
- Check: `polyoxide-binance/Cargo.toml`, `polyoxide-venue/Cargo.toml`

- [ ] **Step 1: Write the failing check**

`polyoxide-polymarket`'s `clob` builds `alloy`, whose signer reaches `getrandom` 0.2, and 0.2 refuses `wasm32-unknown-unknown` unless its `js` feature is on.

```bash
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-polymarket --no-default-features --features clob --target wasm32-unknown-unknown
```

- [ ] **Step 2: Run it to see it fail**

It stops in `getrandom` 0.2.17:

```
error: the wasm*-unknown-unknown targets are not supported by default, you may need to enable the "js" feature. For more information see: https://docs.rs/getrandom/#webassembly-support
```

- [ ] **Step 3: Implement**

In `polyoxide-polymarket/Cargo.toml`, add after the `[dependencies]` table (before `[dev-dependencies]`):

```toml
# Beyond spec D10: alloy's signer needs getrandom 0.2, which refuses wasm32 without its `js` feature.
# (`polyoxide-rt` selects 0.3's backend by a cfg flag; the package rename keeps this entry apart from 0.3.)
[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom02 = { package = "getrandom", version = "0.2", features = ["js"] }
```

Then confirm Story 5 left no host-only tokio feature on either venue manifest's features (the browser build has no `tokio::net` or tokio runtime):

```bash
grep -n '"tokio/net"\|"tokio/rt"' polyoxide-polymarket/Cargo.toml polyoxide-binance/Cargo.toml
grep -n 'web-time' polyoxide-venue/Cargo.toml
```

The first prints nothing; if it prints a line, delete that entry from its feature list. The second prints Story 6's wasm32-only `web-time` entry in a `[target.'cfg(target_arch = "wasm32")'.dependencies]` table; the venue manifest needs no other change.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-venue --features market,trading --target wasm32-unknown-unknown
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-binance --no-default-features --features usdm,usdm-ws --target wasm32-unknown-unknown
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-polymarket --no-default-features --features clob,gamma,data,perps,perps-ws,sports,clob-ws --target wasm32-unknown-unknown
nix develop --command cargo test -p polyoxide-polymarket -p polyoxide-binance --all-features
```

Each `cargo check` prints `Finished`; the test run passes unchanged. When a wasm32 check prints an error in code no earlier task touched, apply the rule of Task 7.8, then rerun.

- [ ] **Step 5: Commit**

```bash
git commit -m "build(polymarket): turn on getrandom 0.2's js feature on wasm32 (story 7)" -m "Beyond spec D10: alloy's signer reaches getrandom 0.2 through rand_core 0.6, and 0.2 refuses wasm32-unknown-unknown unless its js feature is on." -- polyoxide-polymarket/Cargo.toml Cargo.lock
```

### Task 7.6: The rules text

**Files:**
- Modify: `.claude/rules/rt.md`
- Modify: `.claude/rules/sockets.md`
- Modify: `.claude/rules/send-loop.md`

- [ ] **Step 1: Write the failing check**

```bash
grep -q 'BrowserRuntime' .claude/rules/rt.md && grep -q 'WebSocketTransport' .claude/rules/sockets.md && grep -q 'FetchTransport' .claude/rules/send-loop.md
```

- [ ] **Step 2: Run it to see it fail**

It exits 1 with no output: none of the three files names a browser implementation yet.

- [ ] **Step 3: Implement**

Append to `.claude/rules/rt.md`:

```markdown

## The browser runtime (`src/browser.rs`)

`BrowserRuntime` is the `wasm32` default of `default_runtime`; a host's is `TokioRuntime`. It holds no state: `spawn` is `wasm_bindgen_futures::spawn_local`, `now` is `web_time::Instant::now` (`performance.now`), and `sleep` chains gloo-timers' `TimeoutFuture` in steps of at most `i32::MAX` ms, because `setTimeout` fires at once for a larger delay. The arithmetic (`step_ms`, `steps`: the remainder rounded up to a millisecond, so a sleep is never short) is plain Rust built on every target, so the host's tests pin it. Each timer's JS handle sits in a `send_wrapper::SendWrapper`: the `Send` bound of `Runtime` holds, and the handle is only touched on the one thread a browser build has. A build with atomics (threads) is unsupported, because the wrapper panics off its thread.

Dependencies are per target: tokio's `rt` and `time` are the host's, and `gloo-timers`, `send_wrapper`, `wasm-bindgen-futures` and `getrandom` 0.3 with `wasm_js` are `wasm32`'s. `getrandom` is here so feature unification reaches `rand` and `tungstenite` from every crate that builds this one; the final binary still sets `--cfg getrandom_backend="wasm_js"`, which `scripts/wasm_check.sh` sets itself.
```

In `.claude/rules/sockets.md`, the first paragraph states what the kit builds. Replace its sentence

```
It depends on neither `polyoxide-core` nor `polyoxide-venue`,
builds only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls, and re-exports
`tungstenite`.
```

(as Story 5 left it; keep its sense if the wording differs) with

```
It depends on neither `polyoxide-core` nor `polyoxide-venue`. On a host it builds tokio,
tokio-tungstenite and rustls; on `wasm32` it builds the browser's `WebSocket` through web-sys
instead (the dependencies are per target, in `Cargo.toml`'s target tables). On both it builds
`tungstenite`, which it declares directly and re-exports, futures-util, thiserror and tracing.
```

and append a section:

```markdown

## The browser transport (`src/browser.rs`)

`WebSocketTransport` is the `wasm32` default of `default_socket_transport`; a host's is `TungsteniteTransport`. Its capabilities are all `false`. It opens `web_sys::WebSocket` with `binaryType` `arraybuffer` and reads its events from an unbounded channel (a browser socket cannot apply backpressure); the `WebSocket` and its callbacks sit in `SendWrapper`s, as in `rt.md`. A close event yields `Message::Close` with its code and reason, then ends the stream; an error event yields an I/O error. `Close` goes out with its code when it is 1000 or in 3000-4999 and 1000 otherwise, its reason cut to 123 bytes, since a browser throws on anything else. A handshake request with a header beyond those tungstenite generates (a `Sec-WebSocket-Protocol` becomes the subprotocol list) is refused before dialling, and sending a ping, pong or raw frame is refused, each as `Kind::Unsupported` with the message `a handshake header`, `a ping frame`, `a pong frame` or `a raw frame` followed by `is not supported by the browser transport`. A refused upgrade shows no status to a page, so it arrives as a lost connection, never `Kind::Refused`.

What the transport decides (`dial_plan`, `close_code`, `close_reason`, `outgoing`) is plain Rust built on every target, so the host's tests pin it; only the `web_sys` code is `wasm32`'s. `connect_over`, `TungsteniteTransport`, `ensure_crypto_provider` and the scripted test server are host-only, and the `test-server` feature turns on nothing on `wasm32`.
```

Append to `.claude/rules/send-loop.md`:

```markdown

## The browser's fetch (`polyoxide-core/src/fetch.rs`)

`FetchTransport` is the `wasm32` default of the config's `http_transport`; a host's is `ReqwestTransport`. It sends through reqwest's wasm client (the page's `fetch`) with the request's timeout set per request, which bounds the whole exchange, body included. A `fetch` that fails is `Connect` (a network failure and a CORS refusal look alike to a page), a failed body read is `Body`, a timeout is `Timeout` wherever it struck, and a request the browser would not build is `Request`; `kind_of` holds the rule and the host's tests pin it. The pool size and the gzip setting feed only the host default: the browser negotiates compression itself, follows redirects, and shows a page only the response headers the server exposes, so a `Retry-After` is unreadable unless it is. The header pins of the `headers_<module>` suites hold the host default only. The exchange sits in a `SendWrapper`, as in `rt.md`.
```

- [ ] **Step 4: Run it to see it pass**

```bash
grep -q 'BrowserRuntime' .claude/rules/rt.md && grep -q 'WebSocketTransport' .claude/rules/sockets.md && grep -q 'FetchTransport' .claude/rules/send-loop.md && echo present
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'
```

The first prints `present`; the second ends with `passed`.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(rules): record the browser runtime and transports (story 7)" -- .claude/rules/rt.md .claude/rules/sockets.md .claude/rules/send-loop.md
```

### Task 7.7: The wasm32 dependency allowlists

Spec D10 keeps `ci/dep-allowlists/` pinned to `x86_64-unknown-linux-gnu` for the host's files and adds a `wasm32-unknown-unknown` set for the supported crates' trees (spec D1). This task adds that set to the dependency test and generates its files into `ci/dep-allowlists/wasm32-unknown-unknown/`, named as the host's are (`<crate>[@<features>].txt`). The test declares the set instead of globbing for it, so a missing file fails a test rather than vanishing from it, and it checks each file three ways: against `cargo tree --target wasm32-unknown-unknown`, against the deny-lists the host's files obey, and against two lists that keep it from passing vacuously (`WASM_NEVER`, the host-only crates the browser build must not reach, and `WASM_MUST_BUILD`, crates only the browser build reaches).

**Files:**
- Modify: `.github/scripts/tests/test_dep_allowlists.py`
- Modify: `.claude/rules/ci-gates.md` (the `test_dep_allowlists.py` bullet)
- Create: `ci/dep-allowlists/wasm32-unknown-unknown/` (thirteen files, written by the script in Step 3)

- [ ] **Step 1: Write the failing tests**

In `.github/scripts/tests/test_dep_allowlists.py`, make five edits.

(a) In the module docstring, replace the line

```
developer on another platform reads the same tree CI does.
```

with

```
developer on another platform reads the same tree CI does.

`ci/dep-allowlists/wasm32-unknown-unknown/` is the same listing for the browser
build (spec D10), taken with `--target wasm32-unknown-unknown`, for spec D1's
crates and features (`WASM_SET`); the host's files stay pinned to
`x86_64-unknown-linux-gnu`. Its files obey the deny-lists above, except that
`polyoxide-core`, the HTTP stack itself, is not fenced. No file may build
`WASM_NEVER`, the host-only crates (`tokio-tungstenite`, `rustls`), and each must
build the crates in `WASM_MUST_BUILD`, which only the browser build reaches, so
a target that silently resolved as the host's fails.
```

(b) Replace

```python
TARGET = "x86_64-unknown-linux-gnu"
```

with

```python
TARGET = "x86_64-unknown-linux-gnu"
WASM_TARGET = "wasm32-unknown-unknown"
WASM_ALLOWLISTS = ALLOWLISTS / WASM_TARGET
```

(c) Replace the head of `cargo_tree`:

```python
def cargo_tree(crate: str, features: tuple[str, ...]) -> set[str]:
    """The crate names `crate` builds with `features`."""
    command = ["cargo", "tree", "-p", crate, "-e", "normal", "--prefix", "none",
               *no_defaults(crate), "--target", TARGET, "--locked"]
```

with

```python
def cargo_tree(crate: str, features: tuple[str, ...], target: str = TARGET) -> set[str]:
    """The crate names `crate` builds with `features` for `target`."""
    command = ["cargo", "tree", "-p", crate, "-e", "normal", "--prefix", "none",
               *no_defaults(crate), "--target", target, "--locked"]
```

(d) Directly after the `MUST_BUILD` dictionary (the lines ending

```python
    "polyoxide@kalshi-predictions-ws.txt": "polyoxide-ws",
}
```
), add the browser set's declarations:

```python

# Spec D1's supported set for the browser build, one file each in
# `ci/dep-allowlists/wasm32-unknown-unknown/`. It is declared, not globbed, so a
# file that is missing fails a test.
WASM_SET: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("polyoxide-rt", ()),
    ("polyoxide-venue", ("market", "trading")),
    ("polyoxide-core", ()),
    ("polyoxide-ws", ()),
    ("polyoxide-binance", ("usdm",)),
    ("polyoxide-binance", ("usdm-ws",)),
    ("polyoxide-polymarket", ("clob",)),
    ("polyoxide-polymarket", ("clob-ws",)),
    ("polyoxide-polymarket", ("data",)),
    ("polyoxide-polymarket", ("gamma",)),
    ("polyoxide-polymarket", ("perps",)),
    ("polyoxide-polymarket", ("perps-ws",)),
    ("polyoxide-polymarket", ("sports",)),
)
# Spec D10's native-only crates: the kit's TLS and its socket client.
WASM_NEVER = ("rustls", "tokio-tungstenite")
# Crates only the browser build reaches (D10's wasm32 tables), per file.
WASM_MUST_BUILD = {
    "polyoxide-rt.txt": ("gloo-timers", "wasm-bindgen-futures"),
    "polyoxide-venue@market,trading.txt": ("web-time",),
    "polyoxide-core.txt": ("reqwest", "send_wrapper"),
    "polyoxide-ws.txt": ("tungstenite", "web-sys"),
    "polyoxide-binance@usdm-ws.txt": ("polyoxide-ws", "web-sys"),
    "polyoxide-polymarket@clob.txt": ("alloy", "send_wrapper"),
    "polyoxide-polymarket@perps.txt": ("reqwest",),
    "polyoxide-polymarket@sports.txt": ("polyoxide-ws", "web-sys"),
}
# A crate whose purpose is the stack the socket-only deny-list refuses.
UNFENCED = frozenset({"polyoxide-core"})
```

(e) Directly after the function `allowlists`

```python
def allowlists() -> list[Path]:
    return sorted(ALLOWLISTS.glob("*.txt"))
```

add

```python

def wasm_path(crate: str, features: tuple[str, ...]) -> Path:
    """The browser set's file for `crate` built with `features`."""
    named = f"@{','.join(features)}" if features else ""
    return WASM_ALLOWLISTS / f"{crate}{named}.txt"


def wasm_allowlists() -> list[Path]:
    return [wasm_path(crate, features) for crate, features in WASM_SET]
```

Then add these tests directly after `test_no_allowlist_admits_a_denied_crate`:

```python
def test_the_wasm_allowlists_exist() -> None:
    # The set is spec D1's, and the directory holds exactly it: a file that is
    # missing, or one the set does not name, fails here.
    crates = {crate for crate, _ in WASM_SET}
    assert crates == {"polyoxide-rt", "polyoxide-venue", "polyoxide-core", "polyoxide-ws",
                      "polyoxide-binance", "polyoxide-polymarket"}, crates
    assert len(set(WASM_SET)) == len(WASM_SET)

    def features_of(crate: str) -> set[str]:
        return {f for c, features in WASM_SET if c == crate for f in features}

    assert features_of("polyoxide-venue") == {"market", "trading"}
    assert features_of("polyoxide-binance") == {"usdm", "usdm-ws"}
    assert features_of("polyoxide-polymarket") == {
        "clob", "gamma", "data", "perps", "perps-ws", "sports", "clob-ws"}
    present = sorted(p.name for p in WASM_ALLOWLISTS.glob("*.txt"))
    assert present == sorted(p.name for p in wasm_allowlists()), present


@pytest.mark.parametrize("path", wasm_allowlists(), ids=lambda p: p.stem)
def test_each_wasm_allowlist_is_the_tree_cargo_builds(path: Path) -> None:
    crate, features = parse_name(path.stem)
    listed = read_allowlist(path)
    found = problems(f"wasm32/{path.name}", listed, cargo_tree(crate, features, WASM_TARGET))
    assert not found, "\n  ".join(["the tree and its allowlist differ:", *found])


@pytest.mark.parametrize("path", wasm_allowlists(), ids=lambda p: p.stem)
def test_no_wasm_allowlist_admits_a_denied_crate(path: Path) -> None:
    crate, features = parse_name(path.stem)
    refused = denied(crate, read_allowlist(path), features)
    assert refused == [], f"wasm32/{path.name} builds what its deny-list refuses: {refused}"


@pytest.mark.parametrize("path", wasm_allowlists(), ids=lambda p: p.stem)
def test_no_wasm_allowlist_builds_a_host_only_crate(path: Path) -> None:
    built = [name for name in read_allowlist(path) if name in WASM_NEVER]
    assert built == [], f"wasm32/{path.name} builds {built}, which spec D10 keeps off wasm32"


@pytest.mark.parametrize("name", sorted(WASM_MUST_BUILD))
def test_each_wasm_allowlist_builds_what_only_the_browser_builds(name: str) -> None:
    listed = read_allowlist(WASM_ALLOWLISTS / name)
    missing = [crate for crate in WASM_MUST_BUILD[name] if crate not in listed]
    assert missing == [], f"wasm32/{name} does not build {missing}"


def test_the_host_still_builds_what_the_browser_does_not() -> None:
    # Without this a `WASM_NEVER` crate that left every tree would pass the
    # browser check for no reason.
    host = read_allowlist(ALLOWLISTS / "polyoxide-ws.txt")
    assert [name for name in WASM_NEVER if name not in host] == [], host


def test_the_http_stack_crate_is_not_held_to_the_socket_only_deny_list() -> None:
    assert denied("polyoxide-core", ["governor", "reqwest", "sha2"]) == []
    # Only core: any other crate without a rule of its own keeps the deny-list.
    assert denied("polyoxide-rt", ["reqwest"]) == [f"reqwest ({SOCKET_ONLY})"]
```

- [ ] **Step 2: Run it to see it fail**

```bash
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'
```

The 48 tests whose names hold `wasm` fail, and so does the one that holds core out of the socket-only deny-list; the host's tests still pass. The directory does not exist yet:

```
FAILED tests/test_dep_allowlists.py::test_the_wasm_allowlists_exist - AssertionError: ['polyoxide-binance@usdm-ws.txt', ...
FAILED tests/test_dep_allowlists.py::test_each_wasm_allowlist_is_the_tree_cargo_builds[polyoxide-rt] - FileNotFoundError: ... 'polyoxide-rt.txt'
FAILED tests/test_dep_allowlists.py::test_the_http_stack_crate_is_not_held_to_the_socket_only_deny_list - AssertionError: assert ['governor (the socket-only deny-list)', 'reqwest (the socket-only deny-list)', ...] == []
...
49 failed, <n> passed
```

- [ ] **Step 3: Implement**

In `.github/scripts/tests/test_dep_allowlists.py`, give `denied` the exemption. Replace

```python
    rules = PER_FEATURE.get(crate)
    if rules is None:
```

with

```python
    if crate in UNFENCED:
        return []
    rules = PER_FEATURE.get(crate)
    if rules is None:
```

and make `write` regenerate both sets, creating the browser set's files when absent. Replace

```python
def write() -> None:
    """Rewrite every allowlist from the tree cargo reports now."""
    for path in allowlists():
        crate, features = parse_name(path.stem)
        named = f" --features {','.join(features)}" if features else ""
        defaults = "".join(f" {flag}" for flag in no_defaults(crate))
        header = HEADER.format(crate=crate, defaults=defaults, target=TARGET, features=named)
        path.write_text(header + "".join(f"{n}\n" for n in sorted(cargo_tree(crate, features))),
                        encoding="utf-8")
```

with

```python
def write() -> None:
    """Rewrite every allowlist from the tree cargo reports now: the host's files
    that exist, and the browser set `WASM_SET` declares (created when absent)."""
    WASM_ALLOWLISTS.mkdir(parents=True, exist_ok=True)
    files = [(path, TARGET) for path in allowlists()]
    files += [(path, WASM_TARGET) for path in wasm_allowlists()]
    for path, target in files:
        crate, features = parse_name(path.stem)
        named = f" --features {','.join(features)}" if features else ""
        defaults = "".join(f" {flag}" for flag in no_defaults(crate))
        header = HEADER.format(crate=crate, defaults=defaults, target=target, features=named)
        built = sorted(cargo_tree(crate, features, target))
        path.write_text(header + "".join(f"{n}\n" for n in built), encoding="utf-8")
```

In `.claude/rules/ci-gates.md`, the rule for the new set goes in the same commit as the set (AD-21). In the `test_dep_allowlists.py` bullet, after its last deny-list sub-bullet, whose final line is

```
    `@kalshi-predictions-ws` holds the socket kit.
```

add, before the `- `test_module_readmes.py`` bullet, a bullet of its own:

```markdown
- `test_dep_allowlists.py` also pins the browser build (spec D10): the files in
  `ci/dep-allowlists/wasm32-unknown-unknown/`, named as the host's are, hold
  `cargo tree -p <crate> -e normal --target wasm32-unknown-unknown` for spec D1's crates and
  features, which the test's `WASM_SET` declares (it does not glob, so a missing file fails a test).
  The host's files stay pinned to `x86_64-unknown-linux-gnu`. A browser file obeys the deny-lists
  above, except that `polyoxide-core`, the HTTP stack itself, is not held to the socket-only one
  (`UNFENCED`); no file may build `WASM_NEVER` (`tokio-tungstenite` and `rustls`, the host-only
  transport), and each file named in `WASM_MUST_BUILD` must build the crates only the browser
  build reaches (`gloo-timers`, `send_wrapper`, `web-sys` and the like), so a target that resolved
  as the host's fails. A dependency that belongs to the host goes in its manifest's
  `cfg(not(target_arch = "wasm32"))` table, never into a file here. `--write` regenerates both sets
  and creates the browser set's files when absent; say why in the commit.
```

Then generate the files, which also rewrites the host's files to what they already say:

```bash
nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
git status --short ci/dep-allowlists
```

- [ ] **Step 4: Run it to see it pass**

`git status` lists only the thirteen new files, each under `ci/dep-allowlists/wasm32-unknown-unknown/` (`polyoxide-rt.txt`, `polyoxide-venue@market,trading.txt`, `polyoxide-core.txt`, `polyoxide-ws.txt`, the two `polyoxide-binance@` files and the seven `polyoxide-polymarket@` files); no host file changes. Then:

```bash
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_claude_rules.py -q'
```

It prints `passed` for every test, none failed. If `test_each_wasm_allowlist_is_the_tree_cargo_builds` or the deny-list test names a crate in a tree, the manifest of the crate that pulls it in is wrong (a feature that turns on a host-only dependency, or a host-only dependency outside its `cfg(not(target_arch = "wasm32"))` table): fix that manifest, not the test or the file, and regenerate.

Prove the tree check can fail: drop one line from a generated file, run the test, then regenerate it.

```bash
sed -i '/^send_wrapper$/d' ci/dep-allowlists/wasm32-unknown-unknown/polyoxide-core.txt
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py -q'
```

Two tests fail: `test_each_wasm_allowlist_is_the_tree_cargo_builds[polyoxide-core]` prints `wasm32/polyoxide-core.txt builds send_wrapper, which its allowlist does not list`, and `test_each_wasm_allowlist_builds_what_only_the_browser_builds[polyoxide-core.txt]` prints `wasm32/polyoxide-core.txt does not build ['send_wrapper']`. Restore the file:

```bash
nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'
git diff --stat ci/dep-allowlists
```

The diff stat prints nothing.

- [ ] **Step 5: Commit**

```bash
git add -N ci/dep-allowlists/wasm32-unknown-unknown
git commit -m "test(wasm): pin the browser build's dependency trees under ci/dep-allowlists/wasm32-unknown-unknown (story 7)" -m "Spec D10 adds a wasm32-unknown-unknown allowlist set for the supported crates' trees (spec D1), beside the host's files, which stay pinned to x86_64-unknown-linux-gnu and do not change. The files are new, and the test fails on any of them that lists tokio-tungstenite or rustls, which spec D10 keeps off wasm32." -- .github/scripts/tests/test_dep_allowlists.py .claude/rules/ci-gates.md ci/dep-allowlists/wasm32-unknown-unknown
```

### Task 7.8: The whole set, and the story's gates

**Files:**
- Modify: whatever the check names (the rule below), and `ci/dep-allowlists/` when the dependency test names a change
- Test: `scripts/wasm_check.sh`

- [ ] **Step 1: Run the whole check**

```bash
nix develop --command bash scripts/wasm_check.sh
```

- [ ] **Step 2: Fix what it prints**

Each crate prints `==> <crate> <features>` and then its `cargo check` and `cargo clippy` runs. When it is clean, every crate in spec D1's set is `Finished` and the script exits 0; go to Step 3. When an error appears, it is code from an earlier story that names something a browser lacks, and the fix is one of two moves, never a behaviour change:

- an item that names a host-only crate or API (`tokio::net`, `tokio_tungstenite`, `rustls`, `reqwest`'s host client, `std::time::Instant::now()` or `SystemTime::now()`, `tokio::time`, `tokio::spawn`) is gated with `#[cfg(not(target_arch = "wasm32"))]` when it is host-only by design (an example, a native transport, a test fixture), or
- a clock read, sleep or spawn that Stories 2, 5 and 6 missed goes through the injected runtime (`polyoxide_rt`) or `polyoxide_rt::SystemTime`; `python3 scripts/runtime_gate.py` names such a line, which is also a deviation to report.

Rerun the script after each fix until it exits 0. A feature entry that turns on a host-only tokio feature (`tokio/net`, `tokio/rt`) is deleted from its manifest.

- [ ] **Step 3: Run the story's gates**

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop --command cargo test --workspace --all-features
nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --all-features'
nix develop --command msrv-cargo check --workspace --all-features
nix develop --command python3 scripts/gen_registry.py --check
nix develop --command python3 scripts/venue_gate.py
nix develop --command python3 scripts/live_unwraps.py
nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0
nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'
```

`cargo fmt` fails on import order in the files this story edited: run `nix develop --command cargo fmt --all` and rerun. The dependency test must pass without a change, in both sets: the host's files pin the `x86_64-unknown-linux-gnu` tree, where tokio's host features and reqwest's host build are what they were, and Task 7.7's `wasm32-unknown-unknown` files pin the browser tree. A gate in Step 2 that moves an item between target tables can change a tree: if the test names one, regenerate with `nix develop --command bash -c 'cd .github/scripts && uv run python -m tests.test_dep_allowlists --write'` and say why in the commit.

- [ ] **Step 4: Run it to see it pass**

```bash
nix develop --command bash scripts/wasm_check.sh && echo wasm-set-clean
```

It prints `wasm-set-clean` after the last crate's clippy run.

- [ ] **Step 5: Commit**

Commit only when Steps 2 and 3 changed files (name every changed path after `--`, with `Cargo.lock`); a clean run has nothing to commit.

```bash
git commit -m "fix(wasm): gate the host-only items the wasm32 check named (story 7)" -- <the paths Step 2 changed> Cargo.lock
```

## Story 8: The Binance keepalive and sports liveness on a transport without pings

- **Developer:** intermediate-dev
- **Files:** `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/tests/supervision_usdm_kit.rs`, `polyoxide-binance/tests/live_usdm_ws.rs`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `polyoxide-polymarket/tests/supervision_sports.rs`, `scripts/capture_polymarket_sports.py`, `docs/specs/binance/OBSERVED.md`, `docs/specs/sports/OBSERVED.md`, `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py`, `.claude/rules/binance.md`, `.claude/rules/polymarket-sports.md`
- **Interfaces:** exposes "Story 8 → Story 10"; uses Story 5's `Capabilities`, `Kind::Unsupported` and `transport_setters!`
- **Verify:** `cargo test -p polyoxide-binance -p polyoxide-polymarket --all-features && cargo test -p polyoxide-binance --features usdm-ws --test live_usdm_ws -- --ignored list_subscriptions_answers_as_a_keepalive && python3 scripts/live_unwraps.py && bash scripts/wasm_check.sh`
- **Review:** deep (liveness rules, from live measurements)

A browser socket cannot send or see a protocol ping (spec D9), so the two venues that depend on pings need another liveness signal on a transport whose `Capabilities` say so. Binance's supervised protocol sends `{"method":"LIST_SUBSCRIPTIONS","id":n}` where `client_pings` is false, paced as every request is (`Connection::take_paced_id`), and the answer is liveness; its bare `ping()` refuses with `Kind::Unsupported` there. Sports, where `sees_server_pings` is false, gets one of three rules chosen by a 30-minute capture: nothing changes (A), a text keepalive (B), or staleness off (C). Both choices rest on live measurements the developer makes inside D17's limits (a `LIST_SUBSCRIPTIONS` check on `fstream` and a capture of the sports feed), so the tasks below measure first and give complete code for each outcome: the developer applies the variant the measurement selects and deletes the others. Task 8.1 and Task 8.4 call outside hosts; a D17 stop (a second drop of the sports capture, a game live throughout) goes to the owner and ends the story there. The supervisor should dispatch the developer of Task 8.5 with `effort: xhigh`: it edits `sports/supervised.rs` after Stories 2 and 5 moved its timers and its dial onto the injected runtime, and variant B's quotes are the file as Story 5 leaves it (its timers are `BoxFuture<()>`s from `connection.runtime().sleep(..)`, as the gate `runtime_gate.py` requires of every timer in `src`); the edit moves a line `docs/MUTANTS.md` cites, which Task 8.5 moves back and proves again.

### Task 8.1: Check `LIST_SUBSCRIPTIONS` on `fstream` and record it

**Files:**
- Modify: `polyoxide-binance/tests/live_usdm_ws.rs` (append at the end)
- Modify: `docs/specs/binance/OBSERVED.md` (append a section)

- [ ] **Step 1: Write the test** (append to `polyoxide-binance/tests/live_usdm_ws.rs`, after `live_a_client_ping_is_answered`; the file's imports already hold everything it uses)

```rust
/// Spec D17: six requests, 20 s apart, each due within 5 s.
const KEEPALIVE_ROUNDS: usize = 6;
const KEEPALIVE_SPACING: Duration = Duration::from_secs(20);
const KEEPALIVE_ANSWER_WITHIN: Duration = Duration::from_secs(5);

/// A transport with no protocol pings keeps a connection alive by asking
/// `LIST_SUBSCRIPTIONS`, so `fstream` must answer it, quiet or not (D9). An
/// unanswered request is the finding under test: it selects D9's fallback and
/// is recorded in `docs/specs/binance/OBSERVED.md`.
#[tokio::test]
#[ignore]
async fn list_subscriptions_answers_as_a_keepalive() {
    let mut ws = UsdmWs::connect(StreamPath::Market, [StreamName::MarkPrice(btc())])
        .await
        .or_fail("connect");
    let mut unanswered = Vec::new();
    for round in 1..=KEEPALIVE_ROUNDS {
        if round > 1 {
            tokio::time::sleep(KEEPALIVE_SPACING).await;
        }
        let started = tokio::time::Instant::now();
        match tokio::time::timeout(KEEPALIVE_ANSWER_WITHIN, ws.list_subscriptions()).await {
            Ok(Ok(names)) => {
                eprintln!("round {round}: answered in {:?}", started.elapsed());
                assert_eq!(names, ["btcusdt@markPrice@1s"]);
            }
            Ok(Err(err)) => fail("list_subscriptions", &err),
            Err(_) => {
                eprintln!("round {round}: unanswered after {KEEPALIVE_ANSWER_WITHIN:?}");
                unanswered.push(round);
            }
        }
    }
    ws.close().await.or_fail("close");
    assert!(
        unanswered.is_empty(),
        "LIST_SUBSCRIPTIONS went unanswered in rounds {unanswered:?}"
    );
}
```

- [ ] **Step 2: Run it and read the result** (this is the measurement, one socket for about 100 s; it is the D17 check, run once, and once more only if the socket failed as `transient` before round 1)

Run: `nix develop --command cargo test -p polyoxide-binance --features usdm-ws --test live_usdm_ws -- --ignored --nocapture list_subscriptions_answers_as_a_keepalive`

Expected with the main rule holding: six `round N: answered in …` lines and `test result: ok. 1 passed`. If any round prints `unanswered`, the test fails on its last assertion; that is D9's fallback, not an escalation. Note the six round times and which rounds were unanswered. Task 8.3 applies the variant this result selects.

- [ ] **Step 3: Record it** (append to `docs/specs/binance/OBSERVED.md`, after the last line of its `## Streams` section; the heading is Story 10's to extend, so the heading text is exact)

Write the section from the run's output: the date of the run, and each round's line. Use this form, replacing the bracketed parts with the run's own lines (`answered` or `unanswered`, and the times printed):

```
## Browser transport (2026-11)

A browser's WebSocket cannot send a protocol ping, so a client there keeps a
connection alive with a request the server answers instead.

- `LIST_SUBSCRIPTIONS` as a keepalive: on `/market/stream` subscribed to
  `btcusdt@markPrice@1s`, six requests 20 s apart on [date of the run] were
  [all answered, in [the six times] / unanswered in rounds [the round numbers]].
  [An answer is the request's id with `result` the subscribed names, so it is
  a frame like any other and counts as liveness. / The host leaves the request
  unanswered on a connection with a stream subscribed, so a client with no
  protocol pings has only its data frames to count: a quiet stream is
  reconnected at `stale_after`.]
```

Keep only the bracketed alternative the run showed.

- [ ] **Step 4: Run it to see the suite still builds and the other live tests are untouched**

Run: `nix develop --command cargo test -p polyoxide-binance --features usdm-ws --test live_usdm_ws --no-run && nix develop --command python3 scripts/live_unwraps.py`
Expected: the test binary builds and `live_unwraps.py` exits 0 (the new test has no `unwrap` or `expect`, and its `assert!` messages name no error value).

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-binance/tests/live_usdm_ws.rs
git commit -m "test(binance): check LIST_SUBSCRIPTIONS answers as a keepalive on fstream (story 8)" -- polyoxide-binance/tests/live_usdm_ws.rs docs/specs/binance/OBSERVED.md
```

### Task 8.2: The bare client refuses a ping where the transport sends none

**Files:**
- Modify: `polyoxide-binance/src/usdm/ws/client.rs` (the `UsdmWs` struct, `open`, `ping` and its tests)

- [ ] **Step 1: Write the failing test** (in `client.rs`'s `tests` module, after `a_ping_is_answered_and_unsubscribe_leaves_the_rest`; `tests` is a child of the module, so it sets the private field the transport's capability fills)

```rust
    #[tokio::test]
    async fn a_ping_is_refused_where_the_transport_sends_none_and_nothing_is_sent_d9() {
        let server = ScriptedServer::start(vec![Script::default()]).await;
        let mut ws = UsdmWs::connect_to(&server.url, StreamPath::Market, [agg("BTCUSDT")])
            .await
            .unwrap();
        // What `open` reads from a transport whose capabilities say
        // `client_pings: false`, as a browser's does.
        ws.client_pings = false;
        let err = ws.ping().await.unwrap_err();
        assert!(
            matches!(&err, UsdmWsError::Connect(source) if matches!(source.kind(), Kind::Unsupported)),
            "{err:?}"
        );
        assert_eq!(server.ping_count(), 0, "a ping reached the server");
        // The refusal sends nothing and leaves the connection usable.
        assert_eq!(ws.list_subscriptions().await.unwrap(), ["btcusdt@aggTrade"]);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-binance --features usdm-ws --lib -- a_ping_is_refused_where_the_transport_sends_none`
Expected: FAIL to compile: ``error[E0609]: no field `client_pings` on type `UsdmWs` ``.

- [ ] **Step 3: Implement**

Add the field. Replace

```rust
pub struct UsdmWs {
    connection: Connection,
    path: StreamPath,
    streams: Vec<StreamName>,
}
```

with

```rust
pub struct UsdmWs {
    connection: Connection,
    path: StreamPath,
    streams: Vec<StreamName>,
    /// Whether the transport this connection runs on can send protocol pings
    /// (`Capabilities::client_pings`); a browser's cannot (D9).
    client_pings: bool,
}
```

In `open`, set it where the struct is built. Replace

```rust
        Ok(Self {
            connection,
            path,
            streams: Vec::new(),
```

with

```rust
        Ok(Self {
            connection,
            path,
            streams: Vec::new(),
            client_pings: polyoxide_ws::default_socket_transport()
                .capabilities()
                .client_pings,
```

(Story 5 changed what `open` dials through and builds the `Connection` with; keep that, and change only this struct literal. The literal's closing `})` is unchanged.)

Refuse the ping. Replace the head of `ping`

```rust
    pub async fn ping(&mut self) -> Result<Duration, UsdmWsError> {
        let runtime = std::sync::Arc::clone(self.connection.runtime());
        let start = runtime.now();
        self.send_ping().await?;
```

with

```rust
    pub async fn ping(&mut self) -> Result<Duration, UsdmWsError> {
        // A transport with no protocol pings, a browser's, is refused before
        // anything is sent: the failed send would end the connection, and a
        // caller that wants a keepalive asks `list_subscriptions` (D9).
        if !self.client_pings {
            return Err(UsdmWsError::from(WsError::from(
                polyoxide_ws::tungstenite::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "a ping frame is not supported by this transport",
                )),
            )));
        }
        let runtime = std::sync::Arc::clone(self.connection.runtime());
        let start = runtime.now();
        self.send_ping().await?;
```

(The quoted head is `ping` as Story 5 leaves it, with the clock on the connection's runtime; the refusal goes before its first line.) Update the method's doc: replace

```rust
    /// Send a protocol ping and wait for the pong, returning the round trip.
```

with

```rust
    /// Send a protocol ping and wait for the pong, returning the round trip.
    /// On a transport that sends no protocol pings (`client_pings` false, a
    /// browser's) it fails with [`Kind::Unsupported`] and sends nothing;
    /// [`list_subscriptions`](Self::list_subscriptions) is the keepalive
    /// there.
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-binance --features usdm-ws --lib -- usdm::ws::client`
Expected: PASS, including `a_ping_is_answered_and_unsubscribe_leaves_the_rest` (the default transport reports `client_pings: true` natively) and the new test.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(binance): the bare client refuses a ping on a transport that sends none (story 8)" -- polyoxide-binance/src/usdm/ws/client.rs
```

### Task 8.3: The supervised protocol's keepalive

**Files:**
- Modify: `polyoxide-binance/src/usdm/ws/client.rs` (`request_frame` visibility)
- Modify: `polyoxide-binance/src/usdm/ws/protocol.rs`
- Modify: `polyoxide-binance/tests/supervision_usdm_kit.rs` (append)
- Modify: `polyoxide-binance/tests/live_usdm_ws.rs` (fallback only)
- Modify: `.claude/rules/binance.md`

Apply the variant Task 8.1's result selected: **main** (every round answered) or **fallback** (any round unanswered). Steps 1 and 3 give both; delete the one that does not apply.

- [ ] **Step 1: Write the failing tests** (append to `polyoxide-binance/tests/supervision_usdm_kit.rs`; its imports already hold `Arc`, `Duration`, `Instant`, `StreamExt`, `Event`, `DisconnectReason`, `StreamPath`, `Script`, `ScriptedServer`, and the helpers `fast`, `btc_mark`, `steady`, `next_marker`)

Both variants start with the transport the tests inject, which dials as the native one does and reports what a browser's socket can do. The main variant's tests also read `MIN_REQUEST_INTERVAL`: add it to the file's `polyoxide_binance::usdm::ws::{…}` import (`ws::{fixtures, DisconnectReason, Event, MIN_REQUEST_INTERVAL, StreamName, …}`).

```rust
use polyoxide_rt::{BoxFuture, Runtime};
use polyoxide_ws::{
    tungstenite::http, BoxSocket, Capabilities, SocketTransport, TungsteniteTransport, WsError,
};

/// A transport that reports what a browser's socket can do (no protocol pings
/// either way, no handshake headers) and dials as the native one does.
#[derive(Debug)]
struct BrowserLike(TungsteniteTransport);

impl SocketTransport for BrowserLike {
    fn connect(
        &self,
        request: http::Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        self.0.connect(request, timeout, runtime)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(false, false, false)
    }
}

fn browser_like() -> Arc<dyn SocketTransport> {
    Arc::new(BrowserLike(TungsteniteTransport::new()))
}

/// The requests the server received whose method is `method`, in order.
fn requests_of(server: &ScriptedServer, method: &str) -> Vec<serde_json::Value> {
    server
        .requests()
        .into_iter()
        .filter(|request| request["method"] == method)
        .collect()
}

/// What the server's clock may read over the client's between two sends:
/// scheduling jitter, far below the 50 ms of an unpaced ping tick.
const JITTER: Duration = Duration::from_millis(20);

/// The ping tick of the tests' builder (`fast` sets 50 ms).
const TICK: Duration = Duration::from_millis(50);
```

Main variant:

```rust
#[tokio::test]
async fn a_transport_without_client_pings_keeps_a_quiet_path_alive_with_paced_list_subscriptions_d9(
) {
    // The path carries one quiet stream: only the answers to the keepalive
    // arrive, and they are what keeps it from going stale at 600 ms. The tick
    // is 50 ms and a request may follow another no sooner than 200 ms, so
    // three ticks in four are skipped.
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_millis(600))
        .socket_transport(browser_like())
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let quiet = tokio::time::timeout(Duration::from_millis(1500), ws.next()).await;
    assert!(quiet.is_err(), "the path went stale or ended: {quiet:?}");
    assert_eq!(server.connection_count(), 1);
    assert_eq!(server.ping_count(), 0, "a protocol ping was sent");
    let lists = requests_of(&server, "LIST_SUBSCRIPTIONS");
    assert!(
        lists.len() >= 4,
        "only {} keepalives in 1500 ms at a 200 ms pacing",
        lists.len()
    );
    // Each keepalive takes its own id, so a late answer is never taken for
    // the answer to a later request.
    let mut ids: Vec<u64> = server
        .requests()
        .iter()
        .map(|request| request["id"].as_u64().unwrap())
        .collect();
    let sent = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), sent, "an id was reused: {ids:?}");
    // Paced as every request is: no two requests the server saw, the
    // replay's `SUBSCRIBE` and the keepalives alike, are closer than
    // `MIN_REQUEST_INTERVAL`. An unpaced keepalive goes out at the first tick,
    // 50 ms after the subscription.
    for pair in server.received().windows(2) {
        let gap = pair[1].at.duration_since(pair[0].at);
        assert!(
            gap + JITTER >= MIN_REQUEST_INTERVAL,
            "{} and {} arrived only {gap:?} apart",
            pair[0].request,
            pair[1].request
        );
    }
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_keepalive_tick_inside_the_pacing_interval_is_skipped_and_the_next_one_sends_d9() {
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_millis(600))
        .socket_transport(browser_like())
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let quiet = tokio::time::timeout(Duration::from_millis(1500), ws.next()).await;
    assert!(quiet.is_err(), "the path went stale or ended: {quiet:?}");
    let received = server.received();
    let mut previous = received[0].at;
    let mut keepalives = 0;
    for request in &received[1..] {
        assert_eq!(request.request["method"], "LIST_SUBSCRIPTIONS");
        let gap = request.at.duration_since(previous);
        // The first tick past the interval sends: a skipped tick is not a
        // lost keepalive, so a gap is the interval and at most a tick or two
        // beyond it.
        assert!(
            gap <= MIN_REQUEST_INTERVAL + TICK * 2 + Duration::from_millis(100),
            "a skipped tick was not followed by a send at the next one: {gap:?}"
        );
        previous = request.at;
        keepalives += 1;
    }
    assert!(keepalives >= 4, "only {keepalives} keepalives in 1500 ms");
    ws.close().await.unwrap();
}

#[tokio::test]
async fn a_transport_with_client_pings_keeps_the_protocol_ping_d9() {
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let ws = fast(&server)
        .socket_transport(Arc::new(TungsteniteTransport::new()))
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    server
        .wait_for("two pings", |s| s.ping_count() >= 2)
        .await;
    assert!(requests_of(&server, "LIST_SUBSCRIPTIONS").is_empty());
    ws.close().await.unwrap();
}

#[tokio::test]
async fn an_unanswered_list_subscriptions_is_a_stale_outage_d9() {
    // After the subscription, the server answers nothing and sends nothing:
    // the keepalive is liveness only through its answer.
    let server = ScriptedServer::start(vec![Script {
        ignore_later_requests: true,
        ..Default::default()
    }])
    .await;
    let mut ws = fast(&server)
        .stale_after(Duration::from_millis(600))
        .socket_transport(browser_like())
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    match next_marker(&mut ws).await {
        Event::Disconnected {
            path,
            reason: DisconnectReason::Stale,
        } => assert_eq!(path, StreamPath::Market),
        other => panic!("expected a stale outage, got {other:?}"),
    }
    assert!(!requests_of(&server, "LIST_SUBSCRIPTIONS").is_empty());
    ws.close().await.unwrap();
}
```

Fallback variant (replaces the main variant's first, second and fourth tests, and the `JITTER` and `TICK` constants, which only they use; keep `a_transport_with_client_pings_keeps_the_protocol_ping_d9` as written, and with the fallback its assertion on `LIST_SUBSCRIPTIONS` still holds, so the fallback needs no `MIN_REQUEST_INTERVAL` import):

```rust
#[tokio::test]
async fn a_transport_without_client_pings_sends_no_ping_and_a_quiet_path_reconnects_at_stale_after_d9(
) {
    let server = ScriptedServer::start(vec![Script::default()]).await;
    let mut ws = fast(&server)
        .socket_transport(browser_like())
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    match next_marker(&mut ws).await {
        Event::Disconnected {
            path,
            reason: DisconnectReason::Stale,
        } => assert_eq!(path, StreamPath::Market),
        other => panic!("expected a stale outage, got {other:?}"),
    }
    assert!(matches!(next_marker(&mut ws).await, Event::Reconnected { .. }));
    assert_eq!(server.ping_count(), 0, "a protocol ping was sent");
    assert!(
        requests_of(&server, "LIST_SUBSCRIPTIONS").is_empty(),
        "a keepalive request was sent"
    );
    ws.close().await.unwrap();
}

#[tokio::test]
async fn data_frames_alone_keep_a_path_alive_without_client_pings_d9() {
    let server = ScriptedServer::start(vec![steady(fixtures::MARK_PRICE)]).await;
    let mut ws = fast(&server)
        .socket_transport(browser_like())
        .streams([btc_mark()])
        .connect()
        .await
        .unwrap();
    let deadline = Instant::now() + Duration::from_millis(900);
    while let Ok(item) = tokio::time::timeout_at(deadline, ws.next()).await {
        assert!(
            matches!(item.unwrap().unwrap(), Event::Update(_)),
            "an outage while data flowed"
        );
    }
    assert_eq!(server.connection_count(), 1);
    assert_eq!(server.ping_count(), 0);
    ws.close().await.unwrap();
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features --test supervision_usdm_kit -- _d9`
Expected, main variant: `a_transport_without_client_pings_keeps_a_quiet_path_alive_with_paced_list_subscriptions_d9` FAILS on ``a protocol ping was sent`` (the `BrowserLike` transport dials the native stack, so the unchanged protocol's `Ping` still reaches the server, and its pong keeps the path alive), `a_keepalive_tick_inside_the_pacing_interval_is_skipped_and_the_next_one_sends_d9` FAILS on ``only 0 keepalives in 1500 ms`` (nothing but the subscription is a request), `an_unanswered_list_subscriptions_is_a_stale_outage_d9` FAILS because the pongs to the protocol's `Ping` keep the path alive, so no stale outage comes (`next_marker` panics with ``an event within 3 s``), and `a_transport_with_client_pings_keeps_the_protocol_ping_d9` passes. Expected, fallback variant: `data_frames_alone_keep_a_path_alive_without_client_pings_d9` passes and the stale-outage test FAILS on ``a protocol ping was sent`` (`server.ping_count()` is above 0 before the change).

- [ ] **Step 3: Implement**

`polyoxide-binance/src/usdm/ws/client.rs`: make the request writer visible to the protocol. Replace

```rust
/// The request `method` with `params`, for request `id`.
fn request_frame(method: &str, params: Option<&[StreamName]>, id: u64) -> Message {
```

with

```rust
/// The request `method` with `params`, for request `id`.
pub(crate) fn request_frame(method: &str, params: Option<&[StreamName]>, id: u64) -> Message {
```

`polyoxide-binance/src/usdm/ws/protocol.rs`. Replace the head of the file's imports

```rust
use std::time::Duration;

use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    Connection, Decoded, Kind, Offline, Outage, Protocol, Queue, Recovery, WsError,
};

use crate::usdm::ws::{
    client::{request, Answer},
```

with

```rust
use std::{future::Future, sync::Arc, time::Duration};

use polyoxide_rt::Runtime;
use polyoxide_venue::{class_for_handshake_status, Classify};
use polyoxide_ws::{
    retrying_can_fix,
    tungstenite::{client::IntoClientRequest, handshake::client::Request as Handshake, Message},
    BoxSocket, Connection, Decoded, Kind, Offline, Outage, Protocol, Queue, Recovery,
    SocketTransport, WsError,
};

use crate::usdm::ws::{
    client::{request, request_frame, Answer},
```

(Keep any other import Stories 2 and 5 added to this block.) Replace the `D4` row of the type's doc

```rust
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock.
```

main variant:

```rust
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock; on a
///   transport that cannot send one (`Capabilities::client_pings` false, a
///   browser's) a `LIST_SUBSCRIPTIONS` request under an id of its own, paced
///   as every request is (a tick inside `MIN_REQUEST_INTERVAL` of the last
///   request sends nothing, and the next tick sends), whose answer is a frame
///   like any other and so is liveness (D9).
```

fallback variant:

```rust
/// - **D4, the ping:** an empty protocol `Ping` on the wall clock; none on a
///   transport that cannot send one (`Capabilities::client_pings` false, a
///   browser's), where only frames count as liveness and a quiet path
///   reconnects at `stale_after` (D9's fallback: `fstream` leaves
///   `LIST_SUBSCRIPTIONS` unanswered, `docs/specs/binance/OBSERVED.md`).
```

Add the field. Replace

```rust
pub(crate) struct UsdmProtocol {
    base_url: String,
    path: StreamPath,
}

impl UsdmProtocol {
    pub(crate) fn new(base_url: String, path: StreamPath) -> Self {
        Self { base_url, path }
    }
```

with

```rust
pub(crate) struct UsdmProtocol {
    base_url: String,
    path: StreamPath,
    /// Whether the transport of the connection last dialled can send protocol
    /// pings; read from its capabilities in `connect`.
    client_pings: bool,
}

impl UsdmProtocol {
    pub(crate) fn new(base_url: String, path: StreamPath) -> Self {
        Self {
            base_url,
            path,
            client_pings: true,
        }
    }
```

Add `connect`, ahead of `configure`. Replace

```rust
    /// Requests no faster than one per 200 ms. No answer timeout: the
```

with

```rust
    /// Dials the handshake through the transport, as the default does, and
    /// reads from the transport whether it can send a protocol ping (D9),
    /// which `ping` acts on for this connection.
    fn connect(
        &mut self,
        transport: Arc<dyn SocketTransport>,
        runtime: Arc<dyn Runtime>,
        timeout: Duration,
    ) -> impl Future<Output = Result<BoxSocket, UsdmWsError>> + Send {
        self.client_pings = transport.capabilities().client_pings;
        let request = self.handshake();
        async move {
            let socket = transport.connect(request?, timeout, runtime).await?;
            Ok::<_, UsdmWsError>(socket)
        }
    }

    /// Requests no faster than one per 200 ms. No answer timeout: the
```

Replace the ping hook

```rust
    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        Some(Message::Ping(Vec::new().into()))
    }
```

main variant:

```rust
    /// An empty protocol `Ping`; on a transport that cannot send one,
    /// `LIST_SUBSCRIPTIONS` under an id taken from the connection, so a late
    /// answer is never taken for the answer to a later request. The id is the
    /// connection's paced one (`take_paced_id`): a tick that comes within the
    /// connection's minimum request interval of the last request, the
    /// replay's included, sends nothing, and the next tick sends, as every
    /// request waits that interval (D9). The answer decodes to nothing and
    /// counts as liveness, as every frame does.
    fn ping(&mut self, connection: &mut Connection) -> Option<Message> {
        if self.client_pings {
            return Some(Message::Ping(Vec::new().into()));
        }
        let id = connection.take_paced_id()?;
        Some(request_frame("LIST_SUBSCRIPTIONS", None, id))
    }
```

fallback variant:

```rust
    /// An empty protocol `Ping`; none on a transport that cannot send one.
    fn ping(&mut self, _connection: &mut Connection) -> Option<Message> {
        self.client_pings
            .then(|| Message::Ping(Vec::new().into()))
    }
```

and, for the fallback only, drop the now-unused import: in the `client::{request, request_frame, Answer}` line write `client::{request, Answer}`, and leave `client.rs`'s `request_frame` as it was (private).

Add, in `protocol.rs`'s `tests` module after `an_answer_nobody_awaits_decodes_to_nothing_and_a_bad_frame_to_an_error`, a test that the keepalive's answer is nothing and is live (both variants; it pins that the answer needs no handling):

```rust
    #[test]
    fn the_answer_to_a_list_subscriptions_keepalive_is_nothing_and_is_live_d9() {
        let mut protocol = UsdmProtocol::new(USDM_WS_BASE.to_owned(), StreamPath::Market);
        let answer = Message::Text(r#"{"result":["btcusdt@markPrice@1s"],"id":7}"#.into());
        let decoded = protocol.decode(&answer);
        assert!(matches!(decoded, Decoded::Nothing));
        assert!(protocol.is_live(&answer, &decoded));
        assert!(!protocol.delivered(&answer, &decoded));
    }
```

Fallback variant only, in `polyoxide-binance/tests/live_usdm_ws.rs`: the test records what the host does, so it now pins the measured silence and fails when `fstream` starts answering (then D9's main rule applies). Replace its final assertion

```rust
    assert!(
        unanswered.is_empty(),
        "LIST_SUBSCRIPTIONS went unanswered in rounds {unanswered:?}"
    );
```

with

```rust
    assert_eq!(
        unanswered.len(),
        KEEPALIVE_ROUNDS,
        "fstream now answers LIST_SUBSCRIPTIONS (unanswered rounds: {unanswered:?}); \
         D9's main rule applies, so revisit the protocol's ping and OBSERVED.md"
    );
```

and replace the doc comment above the test's attribute lines (`/// A transport with no protocol pings … in `docs/specs/binance/OBSERVED.md`.`) with

```rust
/// D9's fallback was chosen because `fstream` leaves `LIST_SUBSCRIPTIONS`
/// unanswered (`docs/specs/binance/OBSERVED.md`). This pins that: if the host
/// starts answering it, the protocol's keepalive can use it.
```

`.claude/rules/binance.md`: in the `## Streams (`usdm-ws`)` section, replace the sentence in the `UsdmProtocol` bullet

```
  clock, also while the queue is full, so a slow consumer is never stale; staleness counting every
```

with, main variant

```
  clock, also while the queue is full, so a slow consumer is never stale (on a transport with no
  protocol pings, a browser's, the ping is a `LIST_SUBSCRIPTIONS` request under an id of its own,
  paced as every request is by `MIN_REQUEST_INTERVAL` (a tick inside it sends nothing and the next
  sends), and its answer is the liveness; the bare client's `ping()` fails with `Kind::Unsupported`
  there); staleness counting every
```

or, fallback variant

```
  clock, also while the queue is full, so a slow consumer is never stale (on a transport with no
  protocol pings, a browser's, it sends none, because `fstream` leaves `LIST_SUBSCRIPTIONS`
  unanswered, so only frames count and a quiet stream reconnects at `stale_after`; the bare
  client's `ping()` fails with `Kind::Unsupported` there); staleness counting every
```

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-binance --all-features`
Expected: PASS: every test in the crate, the `_d9` tests included.
Then: `nix develop --command cargo clippy -p polyoxide-binance --all-targets --all-features -- -D warnings` (no warnings) and `nix develop --command python3 scripts/live_unwraps.py` (exit 0).

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(binance): keep a path alive with LIST_SUBSCRIPTIONS where the transport sends no ping (story 8)" -- polyoxide-binance/src/usdm/ws/client.rs polyoxide-binance/src/usdm/ws/protocol.rs polyoxide-binance/tests/supervision_usdm_kit.rs polyoxide-binance/tests/live_usdm_ws.rs .claude/rules/binance.md
```

### Task 8.4: The sports capture's cadence mode, its run and its record

**Files:**
- Modify: `scripts/capture_polymarket_sports.py`
- Modify: `docs/specs/sports/OBSERVED.md` (append a section)

- [ ] **Step 1: Write the failing check** (a throwaway script run from the repo root; it drives `record_cadence` against a local server, so no outside host is called)

Run:

```bash
nix develop --command uv run --with websockets --with certifi python3 - <<'PY'
import asyncio
import sys

sys.path.insert(0, "scripts")
from websockets.asyncio.server import serve

import capture_polymarket_sports as c


async def run(handler, seconds, cadence):
    c.CAPTURE = c.Capture()
    async with serve(handler, "127.0.0.1", 0) as server:
        c.URL = f"ws://127.0.0.1:{server.sockets[0].getsockname()[1]}"
        await c.record_cadence(seconds, cadence)


# A quiet server that answers the text ping.
async def answering(ws):
    async for message in ws:
        if message == "ping":
            await ws.send("pong")


asyncio.run(run(answering, 3.5, 1.0))
assert len(c.CAPTURE.sent) == 3, c.CAPTURE.sent
assert [raw for _, _, raw in c.CAPTURE.replies] == ["pong"] * 3, c.CAPTURE.replies
assert c.CAPTURE.frames == [] and c.CAPTURE.drops == []
silence = c.longest_silence([t for t, _ in c.CAPTURE.frames], c.CAPTURE.segments)
assert 3.0 <= silence <= 4.5, silence
# The rule reads the silence a long capture saw: 100 s is past the 45 s limit.
assert c.rule_for(100.0, c.CAPTURE.sent, c.CAPTURE.replies).startswith("B")
assert c.rule_for(10.0, [], []).startswith("A")


# A quiet server that never answers.
async def mute(ws):
    async for _ in ws:
        pass


asyncio.run(run(mute, 2.5, 1.0))
assert len(c.CAPTURE.sent) == 2 and c.CAPTURE.replies == []
assert c.rule_for(100.0, c.CAPTURE.sent, c.CAPTURE.replies).startswith("C")

# One drop is reopened and the capture runs on for the time lost.
seen = {"n": 0}


async def drops_once(ws):
    seen["n"] += 1
    if seen["n"] == 1:
        await ws.close()
        return
    async for _ in ws:
        pass


asyncio.run(run(drops_once, 1.5, 10.0))
assert len(c.CAPTURE.drops) == 1 and len(c.CAPTURE.segments) == 2, c.CAPTURE.drops
assert c.CAPTURE.ended_by == "the time limit", c.CAPTURE.ended_by


# A second drop ends it, and says so.
async def always_drops(ws):
    await ws.close()


asyncio.run(run(always_drops, 5.0, 10.0))
assert len(c.CAPTURE.drops) == 2, c.CAPTURE.drops
assert c.CAPTURE.ended_by.startswith("a second drop"), c.CAPTURE.ended_by
assert c.rule_for(1.0, c.CAPTURE.sent, c.CAPTURE.replies).startswith("stopped")

# A data frame is told from an answer, and an answer to no ping is data.
assert c.is_update('{"gameId": 1, "score": "1-0"}') and not c.is_update("pong")
print("ok")
PY
```

- [ ] **Step 2: Run it to see it fail**

Expected: `AttributeError: module 'capture_polymarket_sports' has no attribute 'record_cadence'` (and no `ok`).

- [ ] **Step 3: Implement**

In the module docstring, replace

```
Usage:
    uv run --with websockets --with certifi python3 scripts/capture_polymarket_sports.py OUT_DIR [SECONDS]
```

with

```
Usage:
    uv run --with websockets --with certifi python3 scripts/capture_polymarket_sports.py OUT_DIR [SECONDS]
    uv run --with websockets --with certifi python3 scripts/capture_polymarket_sports.py --cadence MINUTES [SECONDS]
```

and replace the docstring's last paragraph

```
otherwise. Run it in a busy window, such as a weekend afternoon UTC, to
see the most sports.
"""
```

with

```
otherwise. Run it in a busy window, such as a weekend afternoon UTC, to
see the most sports.

`--cadence MINUTES` is the browser-transport capture (spec D9), and writes
nothing to disk. Run it with no game live. It sends the text `ping` every
MINUTES minutes (SECONDS defaults to 1800, so `--cadence 10` pings at minutes
10 and 20) and nothing else, and prints, to stdout, the longest stretch with
no data frame (what a browser watches, since it sees no protocol ping),
whether each text ping was answered, and the rule that follows: A (the longest
silence is under the 45 s staleness limit: nothing changes), B (every ping was
answered: a text keepalive becomes the liveness) or C (otherwise: no staleness
check on a transport that hides the server's pings). A dropped connection is
reopened once and the capture runs on for the time lost; a second drop ends it
with exit code 2, and the story stops and goes to the owner.
"""
```

Replace the import

```python
from websockets.exceptions import ConnectionClosed
```

with

```python
from websockets.exceptions import ConnectionClosed, InvalidHandshake
```

Replace the constant block

```python
URL = "wss://sports-api.polymarket.com/ws"
```

with

```python
URL = "wss://sports-api.polymarket.com/ws"

# The staleness limit the supervised feed defaults to, and how long after a
# text ping a non-update frame counts as its answer.
STALE_AFTER = 45.0
ANSWER_WINDOW = 10.0
```

Add the cadence fields. Replace

```python
        self.binary = 0
        self.ended_by = "the time limit"
```

with

```python
        self.binary = 0
        self.ended_by = "the time limit"
        # The cadence capture: when each text ping went out, each answer as
        # (ping time, arrival time, text), each drop as (time, why), and the
        # connected spans as (start, stop).
        self.sent: list[float] = []
        self.replies: list[tuple[float, float, str]] = []
        self.drops: list[tuple[float, str]] = []
        self.segments: list[tuple[float, float]] = []
```

Add the functions after `record` (before `json_type`):

```python
def is_update(raw: str) -> bool:
    """Whether a text frame is a match update, which every one is keyed on `gameId`."""
    try:
        frame = json.loads(raw)
    except json.JSONDecodeError:
        return False
    return isinstance(frame, dict) and "gameId" in frame


def note_frame(message: str | bytes) -> None:
    """File one received frame: a binary one counted, a text one that follows a
    text ping within ANSWER_WINDOW and is no match update as that ping's answer,
    any other as a data frame."""
    if not isinstance(message, str):
        CAPTURE.binary += 1
        return
    now = CAPTURE.elapsed()
    if CAPTURE.sent and now - CAPTURE.sent[-1] <= ANSWER_WINDOW and not is_update(message):
        CAPTURE.replies.append((CAPTURE.sent[-1], now, message[:200]))
    else:
        CAPTURE.frames.append((now, message))


async def record_cadence(seconds: float, cadence: float) -> None:
    """Receive for `seconds` of connected time, sending the text `ping` every
    `cadence` seconds and nothing else, into CAPTURE.

    A drop is recorded and the connection reopened once, and the end moves out
    by the time lost. A second drop ends the capture with `ended_by` saying so.
    """
    CAPTURE.started = time.monotonic()
    end = CAPTURE.started + seconds
    next_ping = CAPTURE.started + cadence
    dropped_at: float | None = None
    while end - time.monotonic() > 0:
        segment_start = CAPTURE.elapsed()
        try:
            async with capture_common.ws_session(
                URL, ping_interval=None, create_connection=PingRecorder
            ) as ws:
                segment_start = CAPTURE.elapsed()
                if dropped_at is not None:
                    end += time.monotonic() - dropped_at
                    dropped_at = None
                try:
                    while (remaining := end - time.monotonic()) > 0:
                        wait = min(remaining, max(next_ping - time.monotonic(), 0.0))
                        try:
                            message = await asyncio.wait_for(ws.recv(), wait)
                        except asyncio.TimeoutError:
                            if time.monotonic() >= next_ping and end - time.monotonic() > 0:
                                await ws.send("ping")
                                CAPTURE.sent.append(CAPTURE.elapsed())
                                next_ping += cadence
                            continue
                        note_frame(message)
                finally:
                    CAPTURE.segments.append((segment_start, CAPTURE.elapsed()))
        except (ConnectionClosed, InvalidHandshake, OSError) as err:
            CAPTURE.drops.append((CAPTURE.elapsed(), str(err)))
            if len(CAPTURE.drops) > 1:
                CAPTURE.ended_by = f"a second drop at {CAPTURE.elapsed()} s: {err}"
                return
            dropped_at = time.monotonic()


def longest_silence(frame_times: list[float], segments: list[tuple[float, float]]) -> float:
    """The longest stretch inside one connected span with no data frame: the
    silence a browser watches, since it sees no protocol ping."""
    longest = 0.0
    for start, stop in segments:
        marks = [start, *[t for t in frame_times if start <= t <= stop], stop]
        longest = max(longest, *(b - a for a, b in zip(marks, marks[1:])))
    return longest


def rule_for(
    silence: float, sent: list[float], replies: list[tuple[float, float, str]]
) -> str:
    """D9's sports rule for what the capture saw, as the letter and what it means."""
    if len(CAPTURE.drops) > 1:
        return "stopped: a second drop ended the capture; report to the owner"
    if silence < STALE_AFTER:
        return "A: the longest silence is under 45 s; nothing changes"
    answered = {ping for ping, _, _ in replies}
    if sent and all(ping in answered for ping in sent):
        return "B: every text ping was answered; a text keepalive is the liveness"
    return "C: a text ping went unanswered; no staleness check on such a transport"


def cadence_summary(seconds: float, cadence: float) -> list[str]:
    silence = longest_silence([t for t, _ in CAPTURE.frames], CAPTURE.segments)
    lines = [
        f"cadence capture: {seconds} s of connected time, text ping every {cadence} s",
        f"ended by: {CAPTURE.ended_by}",
        f"drops: {CAPTURE.drops}",
        f"data frames: {len(CAPTURE.frames)} (a game live shows here); binary frames: {CAPTURE.binary}",
        f"protocol pings at (s): {CAPTURE.pings}",
        f"longest silence between data frames: {silence:.1f} s",
    ]
    for ping in CAPTURE.sent:
        answers = [(at, raw) for sent, at, raw in CAPTURE.replies if sent == ping]
        if answers:
            at, raw = answers[0]
            lines.append(f"text ping at {ping} s: answered after {at - ping:.2f} s with {raw!r}")
        else:
            lines.append(f"text ping at {ping} s: unanswered within {ANSWER_WINDOW} s")
    lines.append(f"D9 sports rule: {rule_for(silence, CAPTURE.sent, CAPTURE.replies)}")
    return lines


def cadence_main(args: list[str], cadence_minutes: float) -> int:
    """Run the D9 capture and print its summary; the exit code is 2 when a
    second drop ended it."""
    seconds = int(args[0]) if args else 1800
    cadence = cadence_minutes * 60
    try:
        asyncio.run(record_cadence(seconds, cadence))
    except KeyboardInterrupt:
        CAPTURE.ended_by = f"Ctrl-C at {CAPTURE.elapsed()} s"
    print("\n".join(cadence_summary(seconds, cadence)))
    return 2 if len(CAPTURE.drops) > 1 else 0
```

Replace the head of `main`

```python
def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1])
    seconds = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    out.mkdir(parents=True, exist_ok=True)
```

with

```python
def main() -> None:
    args = sys.argv[1:]
    if "--cadence" in args:
        at = args.index("--cadence")
        try:
            minutes = float(args[at + 1])
        except (IndexError, ValueError):
            sys.exit("--cadence takes a number of minutes")
        del args[at : at + 2]
        sys.exit(cadence_main(args, minutes))
    if not args:
        sys.exit(__doc__)
    out = Path(args[0])
    seconds = int(args[1]) if len(args) > 1 else 300
    out.mkdir(parents=True, exist_ok=True)
```

- [ ] **Step 4: Run it to see it pass, then run the capture**

Re-run Step 1's script. Expected: it prints `ok`. Then `nix develop --command python3 -m py_compile scripts/capture_polymarket_sports.py` (exit 0).

The D17 capture, one unauthenticated socket for 30 minutes, at a time the feed reports no game live (the summary's data-frame count shows it afterwards; data frames throughout is the stop condition below):

Run: `nix develop --command uv run --with websockets --with certifi python3 scripts/capture_polymarket_sports.py --cadence 10 1800 | tee /tmp/sports-cadence.txt`
Expected: the summary, ending `D9 sports rule: <A|B|C>: …`; exit code 0. Exit code 2 (a second drop), or data frames in every minute of the capture (a game live throughout), stops the story: report the summary to the owner and do not continue.

- [ ] **Step 5: Record it** (append to `docs/specs/sports/OBSERVED.md`)

Write the section from the summary's lines; use this form, replacing the bracketed parts with the summary's own values:

```
## Browser transport (2026-11)

A browser's WebSocket does not show protocol pings to the page, so a client
there sees only the data and text frames. A capture on [date], 30 minutes at
[time] UTC with no game live, one connection, sending the text `ping` at
minutes 10 and 20 and nothing else:

- Longest stretch with no data frame: [the summary's "longest silence" value].
  [Data frames received: [count].] Protocol pings arrived every [interval from
  the summary's list] s, as before.
- Text `ping` at minute 10: [answered after [t] s with `[text]` / unanswered
  within 10 s]. At minute 20: [the same, from its line].
- [Drops: none. / One drop at [time], reopened; the capture ran on for the time
  lost.]

So on a transport that hides the server's pings the feed's liveness is [rule
A, B or C in words, as the summary's last line says it].
```

Keep only the alternatives the run showed.

Commit:

```bash
git commit -m "feat(scripts): capture the sports feed's silence and its answer to a text ping (story 8)" -- scripts/capture_polymarket_sports.py docs/specs/sports/OBSERVED.md
```

### Task 8.5: Sports' liveness on a transport that hides the server's pings

**Files:**
- Modify: `polyoxide-polymarket/src/sports/supervised.rs` (variants B and C)
- Modify: `polyoxide-polymarket/src/sports/client.rs` (variant B)
- Modify: `polyoxide-polymarket/tests/supervision_sports.rs` (append)
- Modify: `.claude/rules/polymarket-sports.md`
- Modify: `docs/specs/sports/OBSERVED.md` (a closing line)
- Modify: `docs/MUTANTS.md` (row (b), variants B and C)
- Modify: `.github/scripts/tests/test_mutants_ledger.py` (the citation of that row, variants B and C)

Apply the rule Task 8.4's summary printed (`D9 sports rule: A`, `B` or `C`), and delete the other variants' text. Each variant's tests use the same transport:

```rust
use std::sync::Arc;

use polyoxide_rt::{BoxFuture, Runtime};
use polyoxide_ws::{
    tungstenite::http, BoxSocket, Capabilities, SocketTransport, TungsteniteTransport, WsError,
};

/// A transport that reports what a browser's socket can do (no protocol pings
/// either way, no handshake headers) and dials as the native one does.
#[derive(Debug)]
struct BrowserLike(TungsteniteTransport);

impl SocketTransport for BrowserLike {
    fn connect(
        &self,
        request: http::Request<()>,
        timeout: Duration,
        runtime: Arc<dyn Runtime>,
    ) -> BoxFuture<Result<BoxSocket, WsError>> {
        self.0.connect(request, timeout, runtime)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new(false, false, false)
    }
}

fn browser_like() -> Arc<dyn SocketTransport> {
    Arc::new(BrowserLike(TungsteniteTransport::new()))
}
```

Put that block, then the variant's tests, at the end of `polyoxide-polymarket/tests/supervision_sports.rs` (the file already imports `Duration`, `StreamExt`, `Event`, `SportsError`, `SportsWsBuilder`, `SupervisedSportsWs`, `Script`, `ScriptedServer`, `Message`, `timeout`, `fixtures`, and defines `STALE`, `builder`, `next_item`, `label`).

- [ ] **Step 1: Write the failing tests**

Variant A (the longest silence is under 45 s, so nothing in the code changes; the tests pin that such a transport is supervised as before):

```rust
#[tokio::test]
async fn data_within_the_limit_keeps_a_transport_without_server_pings_alive_d9() {
    let server = ScriptedServer::start(vec![Script {
        send: vec![Message::Text(fixtures::SOCCER.into())],
        push_every: Some(Duration::from_millis(100)),
        ..Script::hold()
    }])
    .await;
    let mut feed = builder(&server)
        .socket_transport(browser_like())
        .connect()
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now() + STALE * 4;
    while let Ok(item) = tokio::time::timeout_at(deadline, feed.next()).await {
        assert_eq!(label(item.expect("the feed ended")), "kor");
    }
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn silence_is_still_stale_on_a_transport_without_server_pings_d9() {
    let server = ScriptedServer::start(vec![Script::hold(), Script::hold()]).await;
    let mut feed = builder(&server)
        .socket_transport(browser_like())
        .connect()
        .await
        .unwrap();
    match next_item(&mut feed).await {
        Ok(Event::Disconnected {
            reason: SportsError::Stale { after },
        }) => assert_eq!(after, STALE),
        other => panic!("expected a stale disconnect, got {other:?}"),
    }
}
```

Both pass before and after (rule A changes no code); they are the rule's scripted test. Skip to Step 4.

Variant C (staleness off):

```rust
#[tokio::test]
async fn a_transport_that_hides_the_servers_pings_never_goes_stale_d9() {
    // The server's pings flow, but the transport does not show them, so the
    // feed has nothing to count in a quiet hour and must not drop the
    // connection for it.
    let server = ScriptedServer::start(vec![Script::pings_only(Duration::from_millis(100))]).await;
    let mut feed = builder(&server)
        .socket_transport(browser_like())
        .connect()
        .await
        .unwrap();
    assert!(
        timeout(STALE * 4, feed.next()).await.is_err(),
        "went stale although the transport shows no ping"
    );
    assert_eq!(server.connection_count(), 1);
}

#[tokio::test]
async fn a_transport_that_shows_the_servers_pings_still_goes_stale_d9() {
    // The same silence on a transport that reports it sees the server's pings,
    // injected explicitly: the rule follows the capability, not the injection.
    let server = ScriptedServer::start(vec![Script::hold(), Script::hold()]).await;
    let mut feed = builder(&server)
        .socket_transport(Arc::new(TungsteniteTransport::new()))
        .connect()
        .await
        .unwrap();
    match next_item(&mut feed).await {
        Ok(Event::Disconnected {
            reason: SportsError::Stale { after },
        }) => assert_eq!(after, STALE),
        other => panic!("expected a stale disconnect, got {other:?}"),
    }
}

#[tokio::test]
async fn a_close_still_ends_a_feed_whose_staleness_is_off_d9() {
    let server = ScriptedServer::start(vec![Script::close_at_once(), Script::hold()]).await;
    let mut feed = builder(&server)
        .socket_transport(browser_like())
        .connect()
        .await
        .unwrap();
    assert!(matches!(
        next_item(&mut feed).await,
        Ok(Event::Disconnected {
            reason: SportsError::Closed { .. }
        })
    ));
    assert!(matches!(next_item(&mut feed).await, Ok(Event::Reconnected)));
}
```

Variant B (a text keepalive): the server answers the text `ping` with `pong`; use the reply text the capture recorded (the upstream document names `pong`).

```rust
/// A server that answers the text `ping` with `pong` and nothing else.
async fn answering_server() -> ScriptedServer {
    ScriptedServer::start_answering(vec![Script::hold()], |_| {
        |text: &str| {
            if text == "ping" {
                vec![Message::Text("pong".into())]
            } else {
                Vec::new()
            }
        }
    })
    .await
}

#[tokio::test]
async fn an_answered_text_keepalive_keeps_a_transport_without_server_pings_alive_d9() {
    // Stale after 300 ms, so the feed pings every 100 ms. Only the answers
    // arrive; none is yielded, and none lets the feed go stale.
    let server = answering_server().await;
    let mut feed = builder(&server)
        .socket_transport(browser_like())
        .connect()
        .await
        .unwrap();
    assert!(
        timeout(STALE * 4, feed.next()).await.is_err(),
        "the keepalive's answer was yielded, or the feed went stale"
    );
    assert_eq!(server.connection_count(), 1);
    let pings = server.count_frames(|frame| frame == "ping");
    assert!(pings >= 5, "only {pings} keepalives in 1200 ms at 100 ms");
}

#[tokio::test]
async fn an_unanswered_text_keepalive_is_not_liveness_d9() {
    let server = ScriptedServer::start(vec![Script::hold(), Script::hold()]).await;
    let mut feed = builder(&server)
        .socket_transport(browser_like())
        .connect()
        .await
        .unwrap();
    match next_item(&mut feed).await {
        Ok(Event::Disconnected {
            reason: SportsError::Stale { after },
        }) => assert_eq!(after, STALE),
        other => panic!("expected a stale disconnect, got {other:?}"),
    }
    assert!(server.count_frames(|frame| frame == "ping") >= 1);
}

#[tokio::test]
async fn a_transport_that_shows_the_servers_pings_sends_no_text_keepalive_d9() {
    let server = ScriptedServer::start(vec![Script::pings_only(Duration::from_millis(100))]).await;
    let mut feed = builder(&server)
        .socket_transport(Arc::new(TungsteniteTransport::new()))
        .connect()
        .await
        .unwrap();
    assert!(timeout(STALE * 4, feed.next()).await.is_err());
    assert!(
        server.client_frames().is_empty(),
        "sent {:?}",
        server.client_frames()
    );
}
```

and, in `polyoxide-polymarket/src/sports/client.rs`'s `tests` module, after `control_and_binary_frames_are_proof_of_life`:

```rust
    #[test]
    fn the_keepalive_reply_is_proof_of_life_and_other_text_is_not_d9() {
        assert!(matches!(
            classify(Message::Text(KEEPALIVE_REPLY.into())),
            Inbound::Alive
        ));
        assert!(matches!(
            classify(Message::Text("pong2".into())),
            Inbound::Undecodable(_)
        ));
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `nix develop --command cargo test -p polyoxide-polymarket --features sports,test-server --test supervision_sports -- _d9`
Expected: variant A: both pass already (rule A changes no code). Variant C: `a_transport_that_hides_the_servers_pings_never_goes_stale_d9` FAILS with ``went stale although the transport shows no ping``. Variant B: `an_answered_text_keepalive_keeps_a_transport_without_server_pings_alive_d9` FAILS with ``the keepalive's answer was yielded, or the feed went stale`` (nothing is sent), and the `client.rs` unit test fails to compile on ``cannot find value `KEEPALIVE_REPLY` ``.

- [ ] **Step 3: Implement**

Variant A: no code. Go to the rules text below.

Both B and C, in `supervised.rs` (both variants quote the file as Story 5 leaves it). Add the capability read to the builder. After `connect_timeout`'s method, insert

```rust
    /// Whether the transport this feed will dial sees the server's protocol
    /// pings (`Capabilities::sees_server_pings`): a browser's does not (D9).
    fn sees_server_pings(&self) -> bool {
        match &self.socket_transport {
            Some(transport) => transport.capabilities().sees_server_pings,
            None => polyoxide_ws::default_socket_transport()
                .capabilities()
                .sees_server_pings,
        }
    }
```

Variant C: replace the head of `connect`, as Story 5 leaves it,

```rust
    pub async fn connect(self) -> Result<SupervisedSportsWs, SportsError> {
        let runtime = self.runtime.clone().unwrap_or_else(default_runtime);
```

with

```rust
    pub async fn connect(mut self) -> Result<SupervisedSportsWs, SportsError> {
        // D9: a transport that hides the server's pings leaves a quiet feed
        // nothing to count, since no data arrives when nothing is live, and
        // the capture of 2026-11 showed no answer to a text ping. The limit is
        // off there, as `Duration::MAX` means never stale, and a lost
        // connection ends through its close event.
        if !self.sees_server_pings() {
            self.stale_after = Duration::MAX;
        }
        let runtime = self.runtime.clone().unwrap_or_else(default_runtime);
```

The `open_with(..)` call and the rest of `connect` stay as Story 5 has them.

and extend the `stale_after` setter's doc, after its `Data cannot set it: …` paragraph:

```rust
    ///
    /// On a transport that hides the server's pings, a browser's, there is
    /// nothing to count in a quiet hour, so the limit is not enforced there
    /// and this value is ignored.
```

Variant B, in `client.rs`: after `SPORTS_WS_URL` add

```rust
/// The text a feed on a transport that hides the server's pings sends to prove
/// the connection alive (D9), and the text the server answers it with, as the
/// capture in `docs/specs/sports/OBSERVED.md` recorded.
pub(crate) const KEEPALIVE_PING: &str = "ping";
pub(crate) const KEEPALIVE_REPLY: &str = "pong";
```

and in `classify`, before the `Message::Text(text) => match MatchUpdate::from_json(&text)` arm:

```rust
        Message::Text(text) if text.as_str() == KEEPALIVE_REPLY => Inbound::Alive,
```

Variant B, in `supervised.rs`. Extend the `client::{…}` import with `KEEPALIVE_PING`, and import `polyoxide_ws::{tungstenite::Message, WsError}` beside `Backoff, Connection, Kind`. Every timer below is the connection's runtime's, as `stale` is after Story 5: `connection.runtime().sleep(..)`, a `BoxFuture<()>`, with no `tokio::time` path and no `Instant`, which `runtime_gate.py` would refuse. Replace the state types

```rust
type Attempt = Pin<Box<dyn Future<Output = Result<Connection, SportsError>> + Send>>;

enum State {
    /// Reading an open connection.
    Reading {
        connection: Box<Connection>,
        stale: BoxFuture<()>,
        received: bool,
    },
```

with

```rust
type Attempt = Pin<Box<dyn Future<Output = Result<Connection, SportsError>> + Send>>;

/// A keepalive going out: the connection moves into the future and comes back
/// with the send's result.
type Keepalive = Pin<Box<dyn Future<Output = (Box<Connection>, Result<(), WsError>)> + Send>>;

enum State {
    /// Reading an open connection. `ping` fires when nothing arrived for the
    /// keepalive period, and is never polled when the feed sends no keepalive.
    Reading {
        connection: Box<Connection>,
        stale: BoxFuture<()>,
        ping: BoxFuture<()>,
        received: bool,
    },
    /// Sending the text keepalive (D9); `stale` and `received` wait for the
    /// connection to come back.
    Pinging {
        send: Keepalive,
        stale: BoxFuture<()>,
        received: bool,
    },
```

Replace `State::reading`

```rust
impl State {
    fn reading(connection: Box<Connection>, stale_after: Duration) -> Self {
        let stale = connection.runtime().sleep(stale_after);
        State::Reading {
            connection,
            stale,
            received: false,
        }
    }
}
```

with

```rust
impl State {
    fn reading(
        connection: Box<Connection>,
        stale_after: Duration,
        keepalive: Option<Duration>,
    ) -> Self {
        let stale = connection.runtime().sleep(stale_after);
        let ping = connection
            .runtime()
            .sleep(keepalive.unwrap_or(Duration::MAX));
        State::Reading {
            connection,
            stale,
            ping,
            received: false,
        }
    }
}
```

Add two `Step` variants. Replace

```rust
    Attempt,
    Connected(Box<Connection>),
    AttemptFailed(SportsError),
}
```

with

```rust
    Attempt,
    Connected(Box<Connection>),
    AttemptFailed(SportsError),
    /// The keepalive period passed with nothing received.
    Keepalive,
    /// The keepalive went out; the connection is back.
    Sent(Box<Connection>),
}
```

Add the period to the feed: after the last field of `SupervisedSportsWs` (Story 5 left it `transport: Arc<dyn SocketTransport>,`) add

```rust
    /// How long a quiet connection waits before sending the text keepalive:
    /// a third of the staleness limit on a transport that hides the server's
    /// pings, and none elsewhere (D9).
    keepalive: Option<Duration>,
```

and in `connect`, which Story 5 left as

```rust
        let connection = open_with(
            self.url.clone(),
```

and

```rust
            state: State::reading(Box::new(connection), self.stale_after),
            config: self,
            runtime,
            transport,
        })
```

replace the first with

```rust
        let keepalive = (!self.sees_server_pings()).then(|| self.stale_after / 3);
        let connection = open_with(
            self.url.clone(),
```

and the second with

```rust
            state: State::reading(Box::new(connection), self.stale_after, keepalive),
            keepalive,
            config: self,
            runtime,
            transport,
        })
```

`lose` counts a connection that was pinging as one that received something. Replace

```rust
        let received = matches!(self.state, State::Reading { received: true, .. });
```

with

```rust
        let received = matches!(
            self.state,
            State::Reading { received: true, .. } | State::Pinging { received: true, .. }
        );
```

In `poll_next`, replace the `Reading` arm

```rust
                State::Reading {
                    connection,
                    stale,
                    received,
                } => poll_reading(connection, stale, received, this.config.stale_after, cx),
```

with

```rust
                State::Reading {
                    connection,
                    stale,
                    ping,
                    received,
                } => poll_reading(
                    connection,
                    stale,
                    ping,
                    received,
                    this.config.stale_after,
                    this.keepalive,
                    cx,
                ),
                State::Pinging { send, .. } => match send.as_mut().poll(cx) {
                    Poll::Ready((connection, Ok(()))) => Step::Sent(connection),
                    Poll::Ready((_, Err(source))) => Step::Lost(SportsError::transport(source)),
                    Poll::Pending => Step::Pending,
                },
```

and add the two handlers beside `Step::Attempt`, after `Step::Lost(reason) => …`:

```rust
                Step::Keepalive => {
                    if let State::Reading {
                        mut connection,
                        stale,
                        received,
                        ..
                    } = std::mem::replace(&mut this.state, State::Ended)
                    {
                        let send: Keepalive = Box::pin(async move {
                            let sent = connection.send(Message::Text(KEEPALIVE_PING.into())).await;
                            (connection, sent)
                        });
                        this.state = State::Pinging {
                            send,
                            stale,
                            received,
                        };
                    }
                }
                Step::Sent(connection) => {
                    if let State::Pinging {
                        stale, received, ..
                    } = std::mem::replace(&mut this.state, State::Ended)
                    {
                        let ping = connection
                            .runtime()
                            .sleep(this.keepalive.unwrap_or(Duration::MAX));
                        this.state = State::Reading {
                            connection,
                            stale,
                            ping,
                            received,
                        };
                    }
                }
```

In the `Step::Connected` handler, replace

```rust
                    this.state = State::reading(connection, this.config.stale_after);
```

with

```rust
                    this.state =
                        State::reading(connection, this.config.stale_after, this.keepalive);
```

Replace `poll_reading`'s signature and its `Pending` arm. Replace

```rust
fn poll_reading(
    connection: &mut Connection,
    stale: &mut BoxFuture<()>,
    received: &mut bool,
    stale_after: Duration,
    cx: &mut Context<'_>,
) -> Step {
```

with

```rust
fn poll_reading(
    connection: &mut Connection,
    stale: &mut BoxFuture<()>,
    ping: &mut BoxFuture<()>,
    received: &mut bool,
    stale_after: Duration,
    keepalive: Option<Duration>,
    cx: &mut Context<'_>,
) -> Step {
```

and

```rust
            Poll::Pending => {
                return match stale.as_mut().poll(cx) {
                    Poll::Ready(()) => Step::Lost(SportsError::Stale { after: stale_after }),
                    Poll::Pending => Step::Pending,
                };
            }
```

with

```rust
            Poll::Pending => {
                if stale.as_mut().poll(cx).is_ready() {
                    return Step::Lost(SportsError::Stale { after: stale_after });
                }
                // Nothing arrived for a keepalive period: ask the server for
                // something, which the next frame (its answer) counts as
                // liveness. Without a keepalive the timer is never polled.
                if keepalive.is_some() && ping.as_mut().poll(cx).is_ready() {
                    return Step::Keepalive;
                }
                return Step::Pending;
            }
```

and, right after the line Story 5 left to reset `stale` on a frame (`*stale = connection.runtime().sleep(stale_after);`), inside the same `if !matches!(inbound, Inbound::Closed(_)) {` block, add

```rust
            if let Some(every) = keepalive {
                *ping = connection.runtime().sleep(every);
            }
```

Extend the struct's doc (`/// A connection that receives nothing, protocol pings included, for the staleness limit is treated as dead.`) with a paragraph, variant B:

```rust
///
/// On a transport that hides the server's pings, a browser's, a quiet
/// connection sends the text `ping` after a third of the staleness limit, and
/// the server's answer is the proof of life; an unanswered keepalive leaves the
/// connection to go stale as before (D9).
```

and for variant C:

```rust
///
/// On a transport that hides the server's pings, a browser's, nothing is
/// counted in a quiet hour, so the staleness limit is not enforced: a lost
/// connection ends through its close event or a failed read (D9).
```

Rules text. `.claude/rules/polymarket-sports.md`, section `## Pings are the liveness signal, the opposite of rtds`, after its first paragraph (ending `counting only data would drop a healthy connection every quiet hour.`) add, by variant:

A:

```
On a transport that hides the server's pings (`Capabilities::sees_server_pings` false, a
browser's), the feed is supervised as before: the 2026-11 capture (`docs/specs/sports/OBSERVED.md`,
"Browser transport") saw data at least every 45 s with no game live, so the limit holds on data
alone.
```

B:

```
On a transport that hides the server's pings (`Capabilities::sees_server_pings` false, a
browser's), a quiet connection sends the text `ping` after a third of `stale_after`, and the
server's `pong` is the proof of life (`classify` reads it as `Alive`): the 2026-11 capture
(`docs/specs/sports/OBSERVED.md`, "Browser transport") saw it answered. An unanswered keepalive
leaves the connection to go stale as before.
```

C:

```
On a transport that hides the server's pings (`Capabilities::sees_server_pings` false, a
browser's), the staleness limit is not enforced, because a quiet hour sends no data and the
2026-11 capture (`docs/specs/sports/OBSERVED.md`, "Browser transport") saw no answer to a text
`ping`: a lost connection ends through its close event or a failed read.
```

For variant B only, replace the bullet

```
- It is a `Stream` state machine with no background task, because no application message is ever
  sent to this host (only protocol pongs and a close reply). Pongs therefore go out only while the
  caller polls.
```

with

```
- It is a `Stream` state machine with no background task, because the only application message
  sent to this host is the text keepalive of a transport that hides the server's pings (otherwise
  only protocol pongs and a close reply). Pongs and the keepalive therefore go out only while the
  caller polls.
```

Append to the `docs/specs/sports/OBSERVED.md` section Task 8.4 wrote, one closing line (A, B or C):

```
The supervised feed's rule on such a transport is [A: unchanged, data at least every 45 s keeps
it up / B: a text `ping` after a third of `stale_after`, whose answer counts as liveness / C: no
staleness limit, a lost connection ends through its close event]; see
`.claude/rules/polymarket-sports.md`.
```

**The mutants table (variants B and C; variant A moves nothing).** `docs/MUTANTS.md` row (b) cites `polyoxide-polymarket/src/sports/supervised.rs`'s test `a_reconnect_waits_out_a_refusals_retry_after_drift_r11`, and `test_mutants_ledger.py` holds the same line. Variants B and C add lines above that test, so the citation moves. (The row's other citation into this story's files, `polyoxide-binance/tests/supervision_usdm_kit.rs`, stays: Task 8.3 only appended to it.) Move both and prove the mutant again. Run from the repository root:

```bash
python3 - <<'PY'
import re
import sys
from pathlib import Path

FILE = "polyoxide-polymarket/src/sports/supervised.rs"
NAME = "async fn a_reconnect_waits_out_a_refusals_retry_after_drift_r11("
LEDGER = Path("docs/MUTANTS.md")
TABLE = Path(".github/scripts/tests/test_mutants_ledger.py")

lines = Path(FILE).read_text().splitlines()
hits = [i + 1 for i, line in enumerate(lines) if NAME in line]
if len(hits) != 1:
    sys.exit(f"{NAME} is on {len(hits)} lines of {FILE}")
new = hits[0]

table = TABLE.read_text()
entry = re.compile(r'(\("' + re.escape(FILE) + r'", )(\d+)(\):)')
if len(entry.findall(table)) != 1:
    sys.exit(f"{TABLE} does not hold exactly one citation of {FILE}")
TABLE.write_text(entry.sub(lambda m: f"{m[1]}{new}{m[3]}", table))

# Story 5 moved the ledger's own citation of this test, and may not have moved
# the table's: each file's number is read and replaced on its own.
ledger = LEDGER.read_text()
cite = re.compile(re.escape(FILE) + r":(\d+)")
if len(cite.findall(ledger)) != 1:
    sys.exit(f"{LEDGER} does not cite {FILE} exactly once")
LEDGER.write_text(cite.sub(f"{FILE}:{new}", ledger))
print(f"{FILE}: {NAME} is now on line {new}")
PY
git diff -U0 -- docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py | grep '^[-+][^-+]' | cut -c1-120
```

Expected: one line saying where the test now is, and a diff whose only changed text is that line number in the two files. A `does not cite … exactly once` line means the rows no longer cite the file once: stop and read it.

Prove row (b)'s mutant again on this test, the way the row states it: `Backoff::take_at_least`'s `self.take().max(floor.min(self.max))` becomes `self.take()`, the test fails, and it passes once the line is restored:

```bash
bash <<'SH'
set -uo pipefail
FILE=polyoxide-ws/src/backoff.rs
TEST=a_reconnect_waits_out_a_refusals_retry_after_drift_r11
sed -i 's/self\.take()\.max(floor\.min(self\.max))/self.take()/' "$FILE"
if git diff --quiet -- "$FILE"; then echo "the mutation did not apply to $FILE"; exit 1; fi
out=$(nix develop --command cargo test -j 4 -p polyoxide-polymarket --all-features --lib -- "$TEST" 2>&1)
git checkout -- "$FILE"
grep -Eq "^test .*${TEST} \.\.\. FAILED" <<<"$out" || { echo "no failure for $TEST under the mutant"; exit 1; }
nix develop --command cargo test -j 4 -p polyoxide-polymarket --all-features --lib -- "$TEST" || exit 1
git diff --exit-code -- "$FILE" && echo "proved: $TEST, mutant restored"
SH
```

Expected: the last line is `proved: a_reconnect_waits_out_a_refusals_retry_after_drift_r11, mutant restored`. A `no failure for …` line means the test no longer holds the rule: stop and read it.

Record the re-proof: add one paragraph to `docs/MUTANTS.md` before its `## The rules` heading. Run from the repository root:

```bash
python3 - <<'PY'
import subprocess
from pathlib import Path

ledger = Path("docs/MUTANTS.md")
head = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], text=True).strip()
paragraph = (
    "The wasm32 plan's story 8 gave the sports feed a rule for a transport that hides the "
    "server's pings, which moved `polyoxide-polymarket/src/sports/supervised.rs`'s test of row "
    "(b), a socket reconnect; that row was proved again on top of "
    f"`{head}`, on Rust 1.99.0 with `-j 4`: the test failed with the mutant in place and "
    "passed again once the file was restored.\n\n"
)
text = ledger.read_text()
marker = "## The rules\n"
assert text.count(marker) == 1
ledger.write_text(text.replace(marker, paragraph + marker))
PY
```

- [ ] **Step 4: Run them to see them pass**

Run: `nix develop --command cargo test -p polyoxide-polymarket --all-features`
Expected: PASS: the crate's whole suite, the `_d9` tests included (and for B, `the_keepalive_reply_is_proof_of_life_and_other_text_is_not_d9` in the lib tests).
For B and C, also `bash -c 'cd .github/scripts && uv run pytest tests/test_mutants_ledger.py -q'` (passes: the citation follows the test).
Then: `nix develop --command cargo clippy -p polyoxide-polymarket --all-targets --all-features -- -D warnings` (no warnings), `nix develop --command python3 scripts/runtime_gate.py` (exit 0: the variant's timers and clock use the runtime's, as `stale` does), and the story's Verify command.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(polymarket): supervise the sports feed on a transport that hides the server's pings (story 8)" -- polyoxide-polymarket/src/sports/client.rs polyoxide-polymarket/src/sports/supervised.rs polyoxide-polymarket/tests/supervision_sports.rs .claude/rules/polymarket-sports.md docs/specs/sports/OBSERVED.md docs/MUTANTS.md .github/scripts/tests/test_mutants_ledger.py
```

## Story 9: The per-push browser smoke suite and the `wasm` CI job

- **Developer:** intermediate-dev
- **Files:** `polyoxide-wasm-smoke/Cargo.toml`, `polyoxide-wasm-smoke/src/lib.rs`, `polyoxide-wasm-smoke/src/bin/wasm-smoke-server.rs`, `polyoxide-wasm-smoke/tests/browser_smoke.rs`, `polyoxide-wasm-smoke/webdriver.json`, `scripts/wasm_smoke.sh`, `Cargo.toml`, `Cargo.lock`, `flake.nix`, `.github/workflows/ci.yml`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `README.md`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 9 → Story 10"; uses Story 6's `runtime_gate.py` and Story 7's `BrowserRuntime`, `FetchTransport`, `WebSocketTransport`, the contract's messages and `scripts/wasm_check.sh`
- **Verify:** `bash scripts/wasm_check.sh && bash scripts/wasm_smoke.sh && cargo test --workspace --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
- **Review:** deep (a new CI gate and the browser behaviour it proves)

This story builds the per-push proof that the browser implementations work in a real browser. It creates `polyoxide-wasm-smoke` (`publish = false`): a native bin, `wasm-smoke-server`, that serves mockito routes with CORS headers (a 200 and two 429s, one that exposes `Retry-After` and one that does not, on one server; a slow route on a server of its own, because mockito answers a slow route by sleeping its server's one thread, and wasm-bindgen-test may run a crate's async tests concurrently) and one scripted socket on the kit's test server, and writes the three addresses to the env file its one argument names (the contract's `POLYOXIDE_SMOKE_HTTP` and `POLYOXIDE_SMOKE_WS`, and a third line, `POLYOXIDE_SMOKE_SLOW`, which a reader that sources the file ignores); and a wasm32 test target, `tests/browser_smoke.rs`, that reads those addresses through `env!` (so a build without them fails to compile instead of passing empty) and runs in a dedicated Worker under headless Chrome. The suite covers `BrowserRuntime` (sleep, `sleep_until`, `timeout`, `interval`, spawn, abort, `is_finished`, a monotonic `now`, and a sleep past `i32::MAX` ms that has not fired after a second), `FetchTransport` (a 200, a timeout, both 429s read through `ErrorResponse`) and `WebSocketTransport` (a text echo, a close with its code, a refused handshake header and a refused ping frame). `scripts/wasm_smoke.sh` starts the server, exports the addresses, runs the suite through `wasm-bindgen-test-runner` and stops the server. `polyoxide-wasm-smoke/webdriver.json` hands headless Chrome the arguments it needs under the runner: `--no-sandbox` (on a runner whose kernel restricts unprivileged user namespaces, and in a container, Chrome cannot create its sandbox and exits at startup), `--disable-dev-shm-usage` (the runner's `/dev/shm` is small), and `--headless` and `--disable-gpu` so the file does not depend on the runner's defaults; cargo runs the test with the package root as its working directory, where the runner reads the file. The flake gains chromium, chromedriver and `wasm-bindgen-cli`; CI gains the `wasm` job (the tenth) and runs `scripts/runtime_gate.py` in its scripts job; CLAUDE.md's job table and `ci-gates.md` say so. None of this code holds a landmine marker. The server tests drive core's re-exported reqwest only to read responses; no `From<reqwest::Error>` is written.

### Task 9.1: The crate and its local server

**Files:**
- Create: `polyoxide-wasm-smoke/Cargo.toml`
- Create: `polyoxide-wasm-smoke/src/lib.rs`
- Create: `polyoxide-wasm-smoke/src/bin/wasm-smoke-server.rs`
- Modify: `Cargo.toml` (the `members` list)

- [ ] **Step 1: Write the failing test**

Create `polyoxide-wasm-smoke/Cargo.toml`:

```toml
[package]
name = "polyoxide-wasm-smoke"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "The browser smoke suite: the wasm32 runtime and transports run in headless Chrome against a local server on every push"
# Never published: it is a test harness, and nothing depends on it.
publish = false

# The message vocabulary, in the server (native) and the browser tests (wasm32).
[dependencies]
tungstenite = { version = "0.26", default-features = false }

# The server is native only, and its socket is the kit's scripted test server.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
mockito = { workspace = true }
polyoxide-ws = { workspace = true, features = ["test-server"] }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "time"] }

# The server's own tests read it as a client: core's re-exported reqwest for the routes,
# the kit's `connect` for the socket.
[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
futures-util = { workspace = true, features = ["sink"] }
polyoxide-core = { workspace = true }

# Registration, read by scripts/gen_registry.py: the README line and the notes CLAUDE.md's
# crate list carries. Run `python3 scripts/gen_registry.py --write` after an edit.
[package.metadata.polyoxide]
readme = "The browser smoke suite: the wasm32 runtime and transports run in headless Chrome against a local server on every push (`publish = false`)"
notes = ["native server, wasm32 tests; nothing depends on it"]
```

Create `polyoxide-wasm-smoke/src/lib.rs`:

```rust
//! The browser smoke suite's shared constants.
//!
//! The suite has two halves. `wasm-smoke-server`, a native bin, answers a handful of routes with
//! CORS headers and holds one scripted socket. `tests/browser_smoke.rs`, built for
//! `wasm32-unknown-unknown`, runs in a dedicated Worker under headless Chrome and calls them
//! through the browser implementations. The two halves agree on the paths, the 429's
//! `Retry-After` and the socket's commands here, and nowhere else.

#![warn(missing_docs)]

use std::time::Duration;

/// The environment variable holding the HTTP server's base URL, as the server writes it and the
/// browser tests read it at compile time.
pub const HTTP_ENV: &str = "POLYOXIDE_SMOKE_HTTP";
/// The environment variable holding the socket server's `ws://` URL.
pub const WS_ENV: &str = "POLYOXIDE_SMOKE_WS";
/// The environment variable holding the base URL of the server that answers [`SLOW_PATH`]. It is
/// a server of its own: mockito answers a slow route by sleeping its server's one thread, which
/// would hold up every other route on that server.
pub const SLOW_ENV: &str = "POLYOXIDE_SMOKE_SLOW";

/// A route answering 200 with [`OK_BODY`].
pub const OK_PATH: &str = "/ok";
/// The body of [`OK_PATH`].
pub const OK_BODY: &str = r#"{"ok":true}"#;

/// A route that answers 200 only after [`SLOW_DELAY`], on the server [`SLOW_ENV`] names.
pub const SLOW_PATH: &str = "/slow";
/// How long [`SLOW_PATH`] holds its answer back.
pub const SLOW_DELAY: Duration = Duration::from_millis(1500);

/// A route answering 429 whose `Retry-After` the browser may read, since the response names it
/// in `Access-Control-Expose-Headers`.
pub const LIMITED_EXPOSED_PATH: &str = "/limited-exposed";
/// A route answering 429 with a `Retry-After` the browser may not read: no
/// `Access-Control-Expose-Headers`.
pub const LIMITED_HIDDEN_PATH: &str = "/limited-hidden";
/// The body of both 429 routes.
pub const LIMITED_BODY: &str = r#"{"error":"slow down"}"#;
/// The seconds both 429 routes send as `Retry-After`.
pub const RETRY_AFTER_SECS: u64 = 7;

/// What the socket sends back for a text frame: this, then the frame.
pub const ECHO_PREFIX: &str = "echo:";
/// The text frame that makes the socket close.
pub const CLOSE_COMMAND: &str = "close";
/// The code of the close the socket sends for [`CLOSE_COMMAND`].
pub const CLOSE_CODE: u16 = 4001;
/// The reason of that close.
pub const CLOSE_REASON: &str = "smoke close";
```

Create `polyoxide-wasm-smoke/src/bin/wasm-smoke-server.rs` holding only the tests, which name the server that Step 3 writes:

```rust
//! The smoke suite's local server.

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use polyoxide_core::reqwest::{Client, Method, StatusCode};
    use polyoxide_wasm_smoke::{
        CLOSE_CODE, CLOSE_COMMAND, CLOSE_REASON, HTTP_ENV, LIMITED_EXPOSED_PATH,
        LIMITED_HIDDEN_PATH, OK_BODY, OK_PATH, RETRY_AFTER_SECS, SLOW_ENV, SLOW_PATH, WS_ENV,
    };
    use tungstenite::Message;

    use super::native::{start, write_env};

    #[tokio::test]
    async fn the_ok_route_answers_with_the_cors_header_a_browser_needs() {
        let running = start().await;
        let response = Client::new()
            .get(format!("{}{OK_PATH}", running.http))
            .send()
            .await
            .expect("the ok route answers");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(response.text().await.expect("a body"), OK_BODY);
    }

    #[tokio::test]
    async fn the_two_429s_differ_only_in_the_exposed_header() {
        let running = start().await;
        let client = Client::new();
        let exposed = client
            .get(format!("{}{LIMITED_EXPOSED_PATH}", running.http))
            .send()
            .await
            .expect("the exposed route answers");
        let hidden = client
            .get(format!("{}{LIMITED_HIDDEN_PATH}", running.http))
            .send()
            .await
            .expect("the hidden route answers");
        for response in [&exposed, &hidden] {
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()["access-control-allow-origin"], "*");
            assert_eq!(
                response.headers()["retry-after"],
                RETRY_AFTER_SECS.to_string().as_str()
            );
        }
        assert_eq!(
            exposed.headers()["access-control-expose-headers"],
            "Retry-After"
        );
        assert!(!hidden
            .headers()
            .contains_key("access-control-expose-headers"));
    }

    #[tokio::test]
    async fn a_preflight_is_allowed_for_any_path() {
        let running = start().await;
        let response = Client::new()
            .request(Method::OPTIONS, format!("{}/anything", running.http))
            .send()
            .await
            .expect("the preflight answers");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        assert_eq!(response.headers()["access-control-allow-headers"], "*");
    }

    #[tokio::test]
    async fn the_slow_route_outlasts_a_short_timeout() {
        let running = start().await;
        let pending = Client::new()
            .get(format!("{}{SLOW_PATH}", running.slow))
            .send();
        assert!(
            tokio::time::timeout(Duration::from_millis(300), pending)
                .await
                .is_err(),
            "the slow route must not answer within 300 ms"
        );
    }

    #[tokio::test]
    async fn a_slow_request_does_not_hold_up_the_other_routes() {
        let running = start().await;
        let client = Client::new();
        // Leave a slow request pending on the slow server, then ask the main one.
        let slow = tokio::spawn({
            let client = client.clone();
            let url = format!("{}{SLOW_PATH}", running.slow);
            async move { client.get(url).send().await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let started = std::time::Instant::now();
        let response = client
            .get(format!("{}{OK_PATH}", running.http))
            .send()
            .await
            .expect("the ok route answers while the slow one is pending");
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the ok route waited {:?} behind the slow one",
            started.elapsed()
        );
        slow.abort();
    }

    #[tokio::test]
    async fn the_slow_route_answers_once_its_delay_has_passed() {
        let running = start().await;
        let response = Client::new()
            .get(format!("{}{SLOW_PATH}", running.slow))
            .send()
            .await
            .expect("the slow route answers in the end");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
    }

    #[tokio::test]
    async fn the_socket_echoes_text_then_closes_with_its_code() {
        let running = start().await;
        let mut socket = polyoxide_ws::connect(running.ws.clone(), Duration::from_secs(5))
            .await
            .expect("the socket accepts a connection");

        socket
            .send(Message::Text("hello".into()))
            .await
            .expect("send");
        let reply = socket.next().await.expect("a reply").expect("a frame");
        assert_eq!(reply, Message::Text("echo:hello".into()));

        socket
            .send(Message::Text(CLOSE_COMMAND.into()))
            .await
            .expect("send");
        let Message::Close(Some(frame)) = socket.next().await.expect("a close").expect("a frame")
        else {
            panic!("the command must be answered with a close frame");
        };
        assert_eq!(u16::from(frame.code), CLOSE_CODE);
        assert_eq!(frame.reason.as_str(), CLOSE_REASON);
    }

    #[tokio::test]
    async fn the_env_file_names_every_address() {
        let running = start().await;
        let dir = std::env::temp_dir().join(format!("wasm-smoke-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp dir");
        let file = dir.join("smoke.env");
        write_env(&file, &running).expect("the env file is written");
        let text = std::fs::read_to_string(&file).expect("the env file is read");
        assert_eq!(
            text,
            format!(
                "{HTTP_ENV}={}\n{WS_ENV}={}\n{SLOW_ENV}={}\n",
                running.http, running.ws, running.slow
            )
        );
        assert!(running.http.starts_with("http://127.0.0.1:"));
        assert!(running.ws.starts_with("ws://127.0.0.1:"));
        assert!(running.slow.starts_with("http://127.0.0.1:"));
        assert_ne!(running.slow, running.http, "the slow route needs a server of its own");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
```

In the root `Cargo.toml`, add the member between `polyoxide-venue` and `polyoxide-ws` (Story 1 added `polyoxide-rt`; leave it):

```toml
  "polyoxide-venue",
  "polyoxide-ws",
]
```

becomes

```toml
  "polyoxide-venue",
  "polyoxide-wasm-smoke",
  "polyoxide-ws",
]
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command cargo test -p polyoxide-wasm-smoke`

Expected: the build fails with ``error[E0432]: unresolved import `super::native` `` (and the same for `start` and `write_env`), because the server does not exist yet.

- [ ] **Step 3: Implement**

Replace the first two lines of `polyoxide-wasm-smoke/src/bin/wasm-smoke-server.rs` (the `//! The smoke suite's local server.` doc line and the blank line after it) with the server, keeping the `tests` module below it unchanged:

```rust
//! The smoke suite's local server.
//!
//! It serves mockito routes with CORS headers, which a Worker's `fetch` needs to read them
//! cross-origin (the route paths and the 429's `Retry-After` are `polyoxide_wasm_smoke`'s), and
//! one scripted socket on `polyoxide-ws`'s test server that echoes text and closes on command.
//! The slow route is on a second mockito server, since mockito answers it by sleeping its
//! server's one thread. It writes `POLYOXIDE_SMOKE_HTTP=<url>`, `POLYOXIDE_SMOKE_WS=<url>` and
//! `POLYOXIDE_SMOKE_SLOW=<url>` to the file its one argument names, whole and once every server
//! listens, then runs until it is killed.
//!
//! On wasm32 it is an empty program: cargo builds a package's bins for an integration test, and
//! the browser tests live in the same package.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{io, path::Path};

    use mockito::{Matcher, Mock, Server, ServerGuard};
    use polyoxide_wasm_smoke::{
        CLOSE_CODE, CLOSE_COMMAND, CLOSE_REASON, ECHO_PREFIX, HTTP_ENV, LIMITED_BODY,
        LIMITED_EXPOSED_PATH, LIMITED_HIDDEN_PATH, OK_BODY, OK_PATH, RETRY_AFTER_SECS, SLOW_DELAY,
        SLOW_ENV, SLOW_PATH, WS_ENV,
    };
    use polyoxide_ws::test_server::{Script, ScriptedServer};
    use tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    };

    /// Every server, kept alive by holding them.
    pub(crate) struct Running {
        /// The HTTP server's base URL.
        pub(crate) http: String,
        /// The base URL of the HTTP server that holds the slow route.
        pub(crate) slow: String,
        /// The socket server's `ws://` URL.
        pub(crate) ws: String,
        _http: ServerGuard,
        _slow: ServerGuard,
        _mocks: Vec<Mock>,
        _ws: ScriptedServer,
    }

    /// The socket's answer to a text frame: a close for the command, an echo for anything else.
    fn answerer(_connection: usize) -> impl FnMut(&str) -> Vec<Message> + Send {
        |text: &str| {
            if text == CLOSE_COMMAND {
                vec![Message::Close(Some(CloseFrame {
                    code: CloseCode::from(CLOSE_CODE),
                    reason: CLOSE_REASON.into(),
                }))]
            } else {
                vec![Message::Text(format!("{ECHO_PREFIX}{text}").into())]
            }
        }
    }

    /// Start every server on `127.0.0.1`, each on a port of its own choosing.
    pub(crate) async fn start() -> Running {
        let mut server = Server::new_async().await;
        let mut slow_server = Server::new_async().await;
        let mut mocks = Vec::new();
        let retry_after = RETRY_AFTER_SECS.to_string();

        // A preflight for any path: the browser sends one before a request with a header
        // beyond the safelisted ones.
        mocks.push(
            server
                .mock("OPTIONS", Matcher::Any)
                .with_status(204)
                .with_header("access-control-allow-origin", "*")
                .with_header("access-control-allow-methods", "GET, POST, OPTIONS")
                .with_header("access-control-allow-headers", "*")
                .with_header("access-control-max-age", "600")
                .create_async()
                .await,
        );
        mocks.push(
            server
                .mock("GET", OK_PATH)
                .with_status(200)
                .with_header("access-control-allow-origin", "*")
                .with_header("content-type", "application/json")
                .with_body(OK_BODY)
                .create_async()
                .await,
        );
        // The callback sleeps its server's one thread, so every route on that server waits
        // behind it. wasm-bindgen-test may run the browser suite's async tests concurrently, so
        // the slow route has a server of its own and no other test's request waits on it.
        mocks.push(
            slow_server
                .mock("OPTIONS", Matcher::Any)
                .with_status(204)
                .with_header("access-control-allow-origin", "*")
                .with_header("access-control-allow-headers", "*")
                .create_async()
                .await,
        );
        mocks.push(
            slow_server
                .mock("GET", SLOW_PATH)
                .with_status(200)
                .with_header("access-control-allow-origin", "*")
                .with_body_from_request(|_| {
                    std::thread::sleep(SLOW_DELAY);
                    b"late".to_vec()
                })
                .create_async()
                .await,
        );
        mocks.push(
            server
                .mock("GET", LIMITED_EXPOSED_PATH)
                .with_status(429)
                .with_header("access-control-allow-origin", "*")
                .with_header("access-control-expose-headers", "Retry-After")
                .with_header("retry-after", &retry_after)
                .with_body(LIMITED_BODY)
                .create_async()
                .await,
        );
        mocks.push(
            server
                .mock("GET", LIMITED_HIDDEN_PATH)
                .with_status(429)
                .with_header("access-control-allow-origin", "*")
                .with_header("retry-after", &retry_after)
                .with_body(LIMITED_BODY)
                .create_async()
                .await,
        );

        // One script is repeated for every connection, so each test gets a fresh socket.
        let ws = ScriptedServer::start_answering(vec![Script::hold()], answerer).await;

        Running {
            http: server.url(),
            slow: slow_server.url(),
            ws: ws.url.clone(),
            _http: server,
            _slow: slow_server,
            _mocks: mocks,
            _ws: ws,
        }
    }

    /// Write every address to `path`, whole: a reader that sees the file sees all its lines.
    pub(crate) fn write_env(path: &Path, running: &Running) -> io::Result<()> {
        let staged = path.with_extension("partial");
        std::fs::write(
            &staged,
            format!(
                "{HTTP_ENV}={}\n{WS_ENV}={}\n{SLOW_ENV}={}\n",
                running.http, running.ws, running.slow
            ),
        )?;
        std::fs::rename(staged, path)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::{path::Path, process::ExitCode};

    let mut args = std::env::args().skip(1);
    let (Some(path), None) = (args.next(), args.next()) else {
        eprintln!("usage: wasm-smoke-server <env file>");
        return ExitCode::from(2);
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("wasm-smoke-server: no runtime: {err}");
            return ExitCode::from(1);
        }
    };
    runtime.block_on(async {
        let running = native::start().await;
        if let Err(err) = native::write_env(Path::new(&path), &running) {
            eprintln!("wasm-smoke-server: cannot write {path}: {err}");
            return ExitCode::from(1);
        }
        std::future::pending::<()>().await;
        ExitCode::SUCCESS
    })
}

#[cfg(target_arch = "wasm32")]
fn main() {}
```

Then run the generator for the new crate's registration (README.md's crate table and CLAUDE.md's crate list are generated regions) and let cargo write the lockfile:

Run: `nix develop --command cargo build -p polyoxide-wasm-smoke && nix develop --command python3 scripts/gen_registry.py --write`

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command cargo test -p polyoxide-wasm-smoke`

Expected: `test result: ok. 8 passed; 0 failed` for the bin's unit tests, and no test in the lib or the (not yet written) test targets.

Run: `nix develop --command cargo clippy -p polyoxide-wasm-smoke --all-targets --all-features -- -D warnings && nix develop --command python3 scripts/gen_registry.py --check`

Expected: no output from either.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-wasm-smoke
git commit -m "feat(wasm-smoke): the browser smoke crate and its local server (Story 9)" -- polyoxide-wasm-smoke Cargo.toml Cargo.lock README.md CLAUDE.md
```

### Task 9.2: The runner script, Chrome's arguments, the flake's browser tools and the first runtime tests

**Files:**
- Create: `scripts/wasm_smoke.sh`
- Create: `polyoxide-wasm-smoke/webdriver.json`
- Create: `polyoxide-wasm-smoke/tests/browser_smoke.rs`
- Modify: `polyoxide-wasm-smoke/Cargo.toml`
- Modify: `flake.nix`

- [ ] **Step 1: Write the failing test**

Create `scripts/wasm_smoke.sh`:

```bash
#!/usr/bin/env bash
# The per-push browser smoke suite (spec D13). It starts the local smoke server, exports the
# addresses the wasm32 tests read at compile time (`env!`), and runs tests/browser_smoke.rs in
# headless Chrome, inside a dedicated Worker, through wasm-bindgen-test-runner. The server stops
# when this script does.
#
# The runner reads Chrome's arguments from polyoxide-wasm-smoke/webdriver.json, in the working
# directory cargo gives the test (the package root). It starts Chrome with `--no-sandbox` there:
# on a runner whose kernel restricts unprivileged user namespaces, and in a container, Chrome
# cannot create its sandbox and exits before the driver connects.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ ! -f polyoxide-wasm-smoke/webdriver.json ]; then
  echo "wasm_smoke: polyoxide-wasm-smoke/webdriver.json is missing; the runner would start Chrome with its own arguments" >&2
  exit 1
fi

for tool in wasm-bindgen wasm-bindgen-test-runner; do
  if ! command -v "$tool" >/dev/null; then
    echo "wasm_smoke: $tool is not on PATH; run inside \`nix develop\` or install wasm-bindgen-cli" >&2
    exit 1
  fi
done

# The runner and the wasm-bindgen crate must be the same version, or the runner refuses the
# module.
locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [ "$locked" != "$installed" ]; then
  echo "wasm_smoke: wasm-bindgen-cli is $installed but Cargo.lock pins wasm-bindgen $locked; they must match" >&2
  exit 1
fi

# The runner takes its driver from CHROMEDRIVER. A GitHub runner has it beside CHROMEWEBDRIVER.
if [ -z "${CHROMEDRIVER:-}" ]; then
  if command -v chromedriver >/dev/null; then
    CHROMEDRIVER=$(command -v chromedriver)
  elif [ -x "${CHROMEWEBDRIVER:-}/chromedriver" ]; then
    CHROMEDRIVER="$CHROMEWEBDRIVER/chromedriver"
  else
    echo "wasm_smoke: no chromedriver on PATH, in CHROMEDRIVER or in CHROMEWEBDRIVER" >&2
    exit 1
  fi
  export CHROMEDRIVER
fi

work=$(mktemp -d)
server_pid=""
cleanup() {
  if [ -n "$server_pid" ]; then
    kill "$server_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

# The server is a host build, so it runs without the wasm32 flags set below.
cargo build --quiet -p polyoxide-wasm-smoke --bin wasm-smoke-server
target_dir=$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
"$target_dir/debug/wasm-smoke-server" "$work/smoke.env" &
server_pid=$!

for _ in $(seq 1 100); do
  [ -s "$work/smoke.env" ] && break
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo "wasm_smoke: the smoke server exited before it wrote its addresses" >&2
    exit 1
  fi
  sleep 0.1
done
if [ ! -s "$work/smoke.env" ]; then
  echo "wasm_smoke: the smoke server wrote no addresses within ten seconds" >&2
  exit 1
fi

# POLYOXIDE_SMOKE_HTTP, POLYOXIDE_SMOKE_WS and POLYOXIDE_SMOKE_SLOW, for the tests' `env!`.
set -a
. "$work/smoke.env"
set +a

RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
WASM_BINDGEN_TEST_TIMEOUT=120 \
  cargo test -p polyoxide-wasm-smoke --target wasm32-unknown-unknown --test browser_smoke
```

Create `polyoxide-wasm-smoke/webdriver.json`, the capabilities the runner hands chromedriver. The arguments are listed in full, so the file does not depend on the runner's own defaults:

```json
{
  "goog:chromeOptions": {
    "args": [
      "--headless",
      "--no-sandbox",
      "--disable-gpu",
      "--disable-dev-shm-usage"
    ]
  }
}
```

Create `polyoxide-wasm-smoke/tests/browser_smoke.rs`:

```rust
//! The browser smoke suite: `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` in
//! headless Chrome, inside a dedicated Worker, against `wasm-smoke-server`.
//!
//! Run it with `scripts/wasm_smoke.sh`, which starts the server and exports the addresses the
//! tests read with `env!`: a build without them does not compile, so the suite cannot pass empty.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test_configure;

wasm_bindgen_test_configure!(run_in_dedicated_worker);

mod runtime {
    use std::{sync::Arc, time::Duration};

    use polyoxide_rt::{BrowserRuntime, Runtime};
    use wasm_bindgen_test::wasm_bindgen_test;

    fn runtime() -> Arc<dyn Runtime> {
        Arc::new(BrowserRuntime)
    }

    #[wasm_bindgen_test]
    async fn sleep_waits_for_its_duration() {
        let rt = runtime();
        let start = rt.now();
        rt.sleep(Duration::from_millis(50)).await;
        let elapsed = rt.now().saturating_duration_since(start);
        assert!(elapsed >= Duration::from_millis(45), "slept {elapsed:?}");
        assert!(elapsed < Duration::from_secs(2), "slept {elapsed:?}");
    }

    #[wasm_bindgen_test]
    async fn sleep_until_waits_for_its_deadline() {
        let rt = runtime();
        let deadline = rt.now() + Duration::from_millis(50);
        rt.sleep_until(deadline).await;
        let early = deadline.saturating_duration_since(rt.now());
        assert!(early < Duration::from_millis(5), "woke {early:?} early");
    }

    #[wasm_bindgen_test]
    async fn sleep_until_a_deadline_already_past_returns_promptly() {
        let rt = runtime();
        let start = rt.now();
        rt.sleep_until(start).await;
        let elapsed = rt.now().saturating_duration_since(start);
        assert!(elapsed < Duration::from_secs(1), "waited {elapsed:?}");
    }

    #[wasm_bindgen_test]
    async fn now_is_monotonic_and_advances() {
        let rt = runtime();
        let mut last = rt.now();
        for _ in 0..1_000 {
            let next = rt.now();
            assert!(next >= last, "the clock went backwards: {last:?} then {next:?}");
            last = next;
        }
        let before = rt.now();
        rt.sleep(Duration::from_millis(20)).await;
        assert!(rt.now() > before, "the clock did not move across a sleep");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: exit 1 with `wasm_smoke: wasm-bindgen is not on PATH; run inside `nix develop` or install wasm-bindgen-cli`, because the dev shell has no browser tools yet. (The `webdriver.json` guard passes: Step 1 created the file.)

- [ ] **Step 3: Implement**

In `flake.nix`, give the dev shell the browser tools. Replace

```nix
              pkgs.uv
              pkgs.python3
            ];
```

with

```nix
              pkgs.uv
              pkgs.python3
              # The browser smoke suite (scripts/wasm_smoke.sh): the runner must be the
              # wasm-bindgen version Cargo.lock pins, which the script checks.
              pkgs.wasm-bindgen-cli
            ]
            # nixpkgs has no chromium for darwin; there the installed Chrome and a chromedriver
            # on PATH (or CHROMEDRIVER) do.
            ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              pkgs.chromium
              pkgs.chromedriver
            ];
```

In `polyoxide-wasm-smoke/Cargo.toml`, add the browser tests' dependencies after the native `dev-dependencies` table:

```toml
# The browser tests: the three implementations, and the vocabulary to drive them.
[target.'cfg(target_arch = "wasm32")'.dev-dependencies]
futures-util = { workspace = true, features = ["sink"] }
polyoxide-core = { workspace = true }
polyoxide-rt = { workspace = true }
polyoxide-ws = { workspace = true }
serde_json = { workspace = true }
wasm-bindgen-test = "0.3"
```

Then check which `wasm-bindgen-cli` the flake now provides against the one `Cargo.lock` pins:

Run: `nix develop --command bash -c 'wasm-bindgen --version; awk "/^name = \"wasm-bindgen\"\$/ { getline; print; exit }" Cargo.lock'`

If the two versions differ, move the lockfile to the flake's (`flake.lock` never changes): run `nix develop --command cargo update -p wasm-bindgen --precise <the flake's version>`, then `nix develop --command cargo tree -p polyoxide-wasm-smoke --target wasm32-unknown-unknown -i wasm-bindgen` and confirm `wasm-bindgen-test` resolved against it. The commit message says the lockfile moved to match the flake.

- [ ] **Step 4: Run it to see it pass**

First show the guard: run `nix develop --command bash -c 'mv polyoxide-wasm-smoke/webdriver.json polyoxide-wasm-smoke/webdriver.json.aside; bash scripts/wasm_smoke.sh; status=$?; mv polyoxide-wasm-smoke/webdriver.json.aside polyoxide-wasm-smoke/webdriver.json; exit $status'` and expect exit 1 with `wasm_smoke: polyoxide-wasm-smoke/webdriver.json is missing`. The script refuses rather than let the runner start Chrome on its own arguments, which fail on a runner that restricts user namespaces. `git status` shows the file back in place.

Then run `nix develop --command bash scripts/wasm_smoke.sh`.

Expected: the final lines are `test runtime::sleep_waits_for_its_duration ... ok` for each of the four tests and `test result: ok. 4 passed; 0 failed`, and no `wasm-smoke-server` process is left (`pgrep wasm-smoke-server` prints nothing).

- [ ] **Step 5: Commit**

```bash
git add -N scripts/wasm_smoke.sh polyoxide-wasm-smoke/webdriver.json polyoxide-wasm-smoke/tests/browser_smoke.rs
git commit -m "test(wasm-smoke): the runner script, Chrome's arguments, the flake's browser tools and the first BrowserRuntime tests (Story 9)" -- scripts/wasm_smoke.sh polyoxide-wasm-smoke/webdriver.json polyoxide-wasm-smoke/tests/browser_smoke.rs polyoxide-wasm-smoke/Cargo.toml flake.nix Cargo.lock
```

### Task 9.3: The rest of BrowserRuntime's tests

**Files:**
- Modify: `polyoxide-wasm-smoke/tests/browser_smoke.rs` (the `runtime` module)

- [ ] **Step 1: Write the failing test**

In `tests/browser_smoke.rs`, replace the `runtime` module's imports

```rust
    use std::{sync::Arc, time::Duration};

    use polyoxide_rt::{BrowserRuntime, Runtime};
    use wasm_bindgen_test::wasm_bindgen_test;
```

with

```rust
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };

    use polyoxide_rt::{interval, spawn, timeout, BrowserRuntime, MissedTick, Runtime};
    use wasm_bindgen_test::wasm_bindgen_test;
```

and append these tests inside the module, after `now_is_monotonic_and_advances`:

```rust

    #[wasm_bindgen_test]
    async fn timeout_expires_a_future_that_never_finishes() {
        let rt = runtime();
        let start = rt.now();
        let outcome = timeout(&*rt, Duration::from_millis(40), std::future::pending::<u8>()).await;
        assert_eq!(outcome, None);
        let elapsed = rt.now().saturating_duration_since(start);
        assert!(elapsed >= Duration::from_millis(35), "expired after {elapsed:?}");
    }

    #[wasm_bindgen_test]
    async fn timeout_returns_a_ready_future_even_at_zero() {
        let rt = runtime();
        assert_eq!(timeout(&*rt, Duration::ZERO, async { 4u8 }).await, Some(4));
        assert_eq!(timeout(&*rt, Duration::from_secs(5), async { 5u8 }).await, Some(5));
    }

    #[wasm_bindgen_test]
    async fn timeout_beats_a_slower_sleep() {
        let rt = runtime();
        let slow = rt.sleep(Duration::from_secs(5));
        assert_eq!(timeout(&*rt, Duration::from_millis(30), slow).await, None);
    }

    #[wasm_bindgen_test]
    async fn interval_ticks_at_once_then_every_period() {
        let rt = runtime();
        let period = Duration::from_millis(40);
        let mut ticks = interval(rt.clone(), period);
        assert_eq!(ticks.period(), period);
        let start = rt.now();
        ticks.tick().await;
        let first = rt.now().saturating_duration_since(start);
        assert!(first < Duration::from_millis(30), "the first tick took {first:?}");
        ticks.tick().await;
        ticks.tick().await;
        let three = rt.now().saturating_duration_since(start);
        assert!(three >= Duration::from_millis(75), "three ticks took {three:?}");
    }

    #[wasm_bindgen_test]
    async fn interval_reset_moves_the_next_tick_a_period_out() {
        let rt = runtime();
        let mut ticks = interval(rt.clone(), Duration::from_millis(40));
        ticks.tick().await;
        ticks.reset();
        let before = rt.now();
        ticks.tick().await;
        let waited = rt.now().saturating_duration_since(before);
        assert!(waited >= Duration::from_millis(35), "waited {waited:?} after a reset");
    }

    #[wasm_bindgen_test]
    async fn burst_fires_missed_ticks_at_once() {
        let rt = runtime();
        let mut ticks = interval(rt.clone(), Duration::from_millis(40));
        ticks.tick().await;
        rt.sleep(Duration::from_millis(130)).await;
        let before = rt.now();
        for _ in 0..3 {
            ticks.tick().await;
        }
        let took = rt.now().saturating_duration_since(before);
        assert!(took < Duration::from_millis(30), "three missed ticks took {took:?}");
    }

    #[wasm_bindgen_test]
    async fn delay_waits_a_period_after_a_late_tick() {
        let rt = runtime();
        let mut ticks = interval(rt.clone(), Duration::from_millis(40));
        ticks.set_missed_tick(MissedTick::Delay);
        ticks.tick().await;
        rt.sleep(Duration::from_millis(130)).await;
        ticks.tick().await;
        let before = rt.now();
        ticks.tick().await;
        let waited = rt.now().saturating_duration_since(before);
        assert!(waited >= Duration::from_millis(35), "waited {waited:?} after a late tick");
    }

    #[wasm_bindgen_test]
    async fn spawn_runs_the_future_and_returns_its_output() {
        let rt = runtime();
        let task = spawn(&*rt, async { 7u8 });
        assert_eq!(task.await, Some(7));
    }

    #[wasm_bindgen_test]
    async fn abort_stops_a_task_before_it_finishes() {
        let rt = runtime();
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);
        let sleeper = rt.clone();
        let task = spawn(&*rt, async move {
            sleeper.sleep(Duration::from_millis(200)).await;
            flag.store(true, Ordering::SeqCst);
        });
        rt.sleep(Duration::from_millis(20)).await;
        assert!(!task.is_finished(), "the task ended before it was aborted");
        task.abort();
        assert_eq!(task.await, None);
        rt.sleep(Duration::from_millis(400)).await;
        assert!(!ran.load(Ordering::SeqCst), "an aborted task must not run on");
    }

    #[wasm_bindgen_test]
    async fn is_finished_turns_true_once_the_task_ends() {
        let rt = runtime();
        let task = spawn(&*rt, async {});
        assert!(!task.is_finished(), "a spawned task cannot have run before the caller yields");
        rt.sleep(Duration::from_millis(30)).await;
        assert!(task.is_finished());
        assert_eq!(task.await, Some(()));
    }

    #[wasm_bindgen_test]
    async fn a_sleep_past_i32_max_millis_has_not_fired_after_a_second() {
        let rt = runtime();
        // setTimeout fires at once for a delay above i32::MAX ms; the runtime chains shorter ones.
        let past = Duration::from_millis(i32::MAX as u64 + 5_000);
        assert_eq!(timeout(&*rt, Duration::from_secs(1), rt.sleep(past)).await, None);
        assert_eq!(
            timeout(&*rt, Duration::from_millis(100), rt.sleep(Duration::MAX)).await,
            None
        );
    }
```

- [ ] **Step 2: Run it to see it fail**

Prove the suite can fail: swap in a runtime that never waits. In the `runtime` module, temporarily replace

```rust
    fn runtime() -> Arc<dyn Runtime> {
        Arc::new(BrowserRuntime)
    }
```

with

```rust
    #[derive(Debug)]
    struct Immediate(BrowserRuntime);

    impl Runtime for Immediate {
        fn spawn(&self, task: polyoxide_rt::BoxFuture<()>) {
            self.0.spawn(task)
        }
        fn sleep(&self, _duration: Duration) -> polyoxide_rt::BoxFuture<()> {
            Box::pin(async {})
        }
        fn now(&self) -> polyoxide_rt::Instant {
            self.0.now()
        }
    }

    fn runtime() -> Arc<dyn Runtime> {
        Arc::new(Immediate(BrowserRuntime))
    }
```

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: `test result: FAILED`, with failures including `runtime::sleep_waits_for_its_duration`, `runtime::timeout_beats_a_slower_sleep`, `runtime::interval_ticks_at_once_then_every_period`, `runtime::delay_waits_a_period_after_a_late_tick` and `runtime::a_sleep_past_i32_max_millis_has_not_fired_after_a_second`.

- [ ] **Step 3: Implement**

Undo the swap: restore `fn runtime()` to `Arc::new(BrowserRuntime)` and delete `Immediate` and its `impl`. The tests are the story's deliverable; `BrowserRuntime` is Story 7's.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: `test result: ok. 15 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(wasm-smoke): BrowserRuntime's timeout, interval, spawn and long-sleep tests (Story 9)" -- polyoxide-wasm-smoke/tests/browser_smoke.rs
```

### Task 9.4: FetchTransport against the local server

**Files:**
- Modify: `polyoxide-wasm-smoke/tests/browser_smoke.rs` (a `fetch` module)

- [ ] **Step 1: Write the failing test**

Append to `tests/browser_smoke.rs`:

```rust

mod fetch {
    use std::{sync::Arc, time::Duration};

    use polyoxide_core::{
        http::{header::RETRY_AFTER, StatusCode},
        ApiError, FetchTransport, HttpClientBuilder, Request, RetryConfig, TransportKind,
    };
    use polyoxide_rt::{BrowserRuntime, Runtime};
    use serde_json::Value;
    use wasm_bindgen_test::wasm_bindgen_test;

    const BASE: &str = env!("POLYOXIDE_SMOKE_HTTP");
    // The slow route's server is its own: the browser may run these tests concurrently, and a
    // sleeping slow route must not hold up another test's request.
    const SLOW_BASE: &str = env!("POLYOXIDE_SMOKE_SLOW");

    /// A request through an `HttpClient` on the fetch transport and the browser runtime, with no
    /// retries, so a 429 is the first answer and a timeout the only wait. `HttpRequest` has no
    /// public constructor, so the transport is driven the way a venue crate drives it: through a
    /// client built with `http_transport(..)`.
    fn request_to(base: &str, path: &str, timeout_ms: u64) -> Request<Value, ApiError> {
        let client = HttpClientBuilder::new(base)
            .runtime(Arc::new(BrowserRuntime))
            .http_transport(Arc::new(FetchTransport::new()))
            .with_retry_config(RetryConfig {
                max_retries: 0,
                initial_backoff_ms: 1,
                max_backoff_ms: 1,
            })
            .timeout_ms(timeout_ms)
            .build()
            .expect("a client on the fetch transport");
        Request::new(client, path)
    }

    fn request(path: &str, timeout_ms: u64) -> Request<Value, ApiError> {
        request_to(BASE, path, timeout_ms)
    }

    #[wasm_bindgen_test]
    async fn a_200_arrives_whole_through_fetch() {
        let response = request("/ok", 10_000)
            .send_raw()
            .await
            .expect("a 200 through fetch");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.expect("a body"), r#"{"ok":true}"#);
    }

    #[wasm_bindgen_test]
    async fn a_200_decodes_as_json() {
        let value = request("/ok", 10_000).send().await.expect("a decoded 200");
        assert_eq!(value["ok"], true);
    }

    #[wasm_bindgen_test]
    async fn a_slow_route_times_out_as_a_transport_timeout() {
        let start = BrowserRuntime.now();
        let err = request_to(SLOW_BASE, "/slow", 300)
            .send_raw()
            .await
            .err()
            .expect("the slow route must time out");
        match err {
            ApiError::Transport(err) => assert_eq!(err.kind(), TransportKind::Timeout, "{err}"),
            other => panic!("expected a transport timeout, got {other:?}"),
        }
        let elapsed = BrowserRuntime.now().saturating_duration_since(start);
        assert!(
            elapsed < Duration::from_millis(1400),
            "the timeout fired after {elapsed:?}, not before the server's 1500 ms answer"
        );
    }

    #[wasm_bindgen_test]
    async fn a_429_that_exposes_retry_after_carries_it_into_the_error_response() {
        let err = request("/limited-exposed", 10_000)
            .send_raw()
            .await
            .err()
            .expect("a 429 is an error");
        let ApiError::Response(response) = err else {
            panic!("expected an error response, got {err:?}");
        };
        assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.message, "slow down");
        assert!(response.headers.get(RETRY_AFTER).is_some());
        assert_eq!(response.retry_after, Some(Duration::from_secs(7)));
    }

    #[wasm_bindgen_test]
    async fn a_429_that_hides_retry_after_shows_none_to_the_browser() {
        let err = request("/limited-hidden", 10_000)
            .send_raw()
            .await
            .err()
            .expect("a 429 is an error");
        let ApiError::Response(response) = err else {
            panic!("expected an error response, got {err:?}");
        };
        assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.message, "slow down");
        // The server sent it, but a browser reads only what Access-Control-Expose-Headers names.
        assert!(response.headers.get(RETRY_AFTER).is_none());
        assert_eq!(response.retry_after, None);
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Prove the suite reads the server's CORS headers: in `wasm-smoke-server.rs`, temporarily delete the line `.with_header("access-control-expose-headers", "Retry-After")` from the `LIMITED_EXPOSED_PATH` route.

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: `fetch::a_429_that_exposes_retry_after_carries_it_into_the_error_response` fails on `assert!(response.headers.get(RETRY_AFTER).is_some())`, and every other test passes.

Then, with the line restored, delete `.with_header("access-control-allow-origin", "*")` from the `OK_PATH` route and run again.

Expected: `fetch::a_200_arrives_whole_through_fetch` and `fetch::a_200_decodes_as_json` fail, `.expect("a 200 through fetch")` printing a `Transport error: connect: ...` (a browser does not tell a CORS refusal from a network failure).

- [ ] **Step 3: Implement**

Restore both server lines. The transport is Story 7's; the server routes were written in Task 9.1.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: `test result: ok. 20 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(wasm-smoke): FetchTransport's 200, timeout and both 429s in the browser (Story 9)" -- polyoxide-wasm-smoke/tests/browser_smoke.rs
```

### Task 9.5: WebSocketTransport against the scripted socket

**Files:**
- Modify: `polyoxide-wasm-smoke/tests/browser_smoke.rs` (a `socket` module)

- [ ] **Step 1: Write the failing test**

Append to `tests/browser_smoke.rs`:

```rust

mod socket {
    use std::{sync::Arc, time::Duration};

    use futures_util::{SinkExt, StreamExt};
    use polyoxide_rt::{timeout, BrowserRuntime, Runtime};
    use polyoxide_ws::{BoxSocket, Kind, SocketTransport, WebSocketTransport, WsError};
    use tungstenite::{http::Request, Message};
    use wasm_bindgen_test::wasm_bindgen_test;

    const URL: &str = env!("POLYOXIDE_SMOKE_WS");
    const WAIT: Duration = Duration::from_secs(5);

    const NO_HEADER: &str = "a handshake header is not supported by the browser transport";
    const NO_PING: &str = "a ping frame is not supported by the browser transport";

    async fn open(header: Option<(&str, &str)>) -> Result<BoxSocket, WsError> {
        let mut request = Request::builder().uri(URL);
        if let Some((name, value)) = header {
            request = request.header(name, value);
        }
        let request = request.body(()).expect("a handshake request");
        let runtime: Arc<dyn Runtime> = Arc::new(BrowserRuntime);
        WebSocketTransport::new().connect(request, WAIT, runtime).await
    }

    async fn next(socket: &mut BoxSocket) -> Message {
        timeout(&BrowserRuntime, WAIT, socket.next())
            .await
            .expect("a frame within five seconds")
            .expect("the stream ended")
            .expect("a frame, not an error")
    }

    #[wasm_bindgen_test]
    async fn a_text_frame_is_echoed() {
        let mut socket = open(None).await.ok().expect("the socket opens");
        socket
            .send(Message::Text("hello".into()))
            .await
            .expect("send");
        assert_eq!(next(&mut socket).await, Message::Text("echo:hello".into()));
    }

    #[wasm_bindgen_test]
    async fn a_close_arrives_with_its_code_and_reason() {
        let mut socket = open(None).await.ok().expect("the socket opens");
        socket
            .send(Message::Text("close".into()))
            .await
            .expect("send");
        let Message::Close(Some(frame)) = next(&mut socket).await else {
            panic!("the command must be answered with a close frame");
        };
        assert_eq!(u16::from(frame.code), 4001);
        assert_eq!(frame.reason.as_str(), "smoke close");
        assert!(
            timeout(&BrowserRuntime, WAIT, socket.next())
                .await
                .expect("the stream ends within five seconds")
                .is_none(),
            "the stream ends after the close"
        );
    }

    #[wasm_bindgen_test]
    async fn a_handshake_header_is_refused_before_dialling() {
        let err = open(Some(("x-smoke", "1")))
            .await
            .err()
            .expect("a handshake header must be refused");
        assert_eq!(err.kind(), Kind::Unsupported, "{err}");
        assert!(err.to_string().contains(NO_HEADER), "{err}");
    }

    #[wasm_bindgen_test]
    async fn a_ping_frame_is_refused_and_the_socket_stays_usable() {
        let mut socket = open(None).await.ok().expect("the socket opens");
        let err = socket
            .send(Message::Ping(vec![1, 2, 3].into()))
            .await
            .expect_err("a ping frame must be refused");
        match err {
            tungstenite::Error::Io(io) => {
                assert_eq!(io.kind(), std::io::ErrorKind::Unsupported);
                assert!(io.to_string().contains(NO_PING), "{io}");
            }
            other => panic!("expected an I/O error, got {other:?}"),
        }
        socket
            .send(Message::Text("still".into()))
            .await
            .expect("send after the refusal");
        assert_eq!(next(&mut socket).await, Message::Text("echo:still".into()));
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Prove the handshake test is driven by the header: temporarily change `open(Some(("x-smoke", "1")))` to `open(None)` in `a_handshake_header_is_refused_before_dialling`.

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: that test fails at `.expect("a handshake header must be refused")`, because the socket opens.

Then, with it restored, change `ECHO_PREFIX` in `polyoxide-wasm-smoke/src/lib.rs` from `"echo:"` to `"eco:"` and run again.

Expected: `socket::a_text_frame_is_echoed` and `socket::a_ping_frame_is_refused_and_the_socket_stays_usable` fail on the `echo:` comparison, which names the literal the browser tests hold independently of the constant.

- [ ] **Step 3: Implement**

Restore `ECHO_PREFIX` to `"echo:"`. The transport is Story 7's, and the `tungstenite` the tests name is the one declared under `[dependencies]`, which an integration test may use, so no manifest changes.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash scripts/wasm_smoke.sh`

Expected: `test result: ok. 24 passed; 0 failed`, the last four being `socket::...`.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(wasm-smoke): WebSocketTransport's echo, close code and refusals in the browser (Story 9)" -- polyoxide-wasm-smoke/tests/browser_smoke.rs
```

### Task 9.6: The `wasm` CI job, the scripts job's runtime gate and the docs

**Files:**
- Modify: `.github/workflows/ci.yml` (the end of the file)
- Modify: `CLAUDE.md` (the dev shell paragraph, the CI jobs heading and table)
- Modify: `.claude/rules/ci-gates.md` (the front matter, the intro line and a `## wasm` section)
- Modify: `ci/dep-allowlists/` (regenerated only if the dependency tests ask)

- [ ] **Step 1: Write the failing test**

Run this check, which reads the three files the task edits:

```bash
nix develop --command bash -c 'cd .github/scripts && uv run python - <<"PY"
import pathlib
import yaml

ci = yaml.safe_load(pathlib.Path("../workflows/ci.yml").read_text())
job = ci["jobs"]["wasm"]
assert job["name"] == "WebAssembly", job["name"]
runs = [step["run"] for step in job["steps"] if "run" in step]
assert runs[-2:] == ["bash scripts/wasm_check.sh", "bash scripts/wasm_smoke.sh"], runs
assert any(step.get("with", {}).get("targets") == "wasm32-unknown-unknown" for step in job["steps"])
assert any(u.startswith("mozilla-actions/sccache-action") for u in (s.get("uses", "") for s in job["steps"]))
scripts = [step["run"] for step in ci["jobs"]["scripts"]["steps"] if "run" in step]
assert "python3 scripts/runtime_gate.py" in scripts, scripts
claude = pathlib.Path("../../CLAUDE.md").read_text()
assert "### The ten CI jobs" in claude
assert "\n| wasm |" in claude
gates = pathlib.Path("../../.claude/rules/ci-gates.md").read_text()
assert "CLAUDE.md lists the ten jobs" in gates and "\n## wasm\n" in gates
print("ok")
PY'
```

- [ ] **Step 2: Run it to see it fail**

Run the command from Step 1.

Expected: `KeyError: 'wasm'`.

- [ ] **Step 3: Implement**

In `.github/workflows/ci.yml`, replace the end of the `scripts` job

```yaml
      - run: uv run pytest tests/ -v
        working-directory: .github/scripts
```

with

```yaml
      - run: uv run pytest tests/ -v
        working-directory: .github/scripts
      # The runtime seam's line (spec D3): no timer, clock read or spawn outside the injected
      # runtime. It needs only python3.
      - run: python3 scripts/runtime_gate.py

  # The browser seam (spec D13): the supported crates build for wasm32-unknown-unknown, and the
  # three browser implementations run in headless Chrome, inside a Worker, against the local
  # smoke server. wasm-bindgen-cli must be the version Cargo.lock pins, so the version is read
  # from it. The runner's own Chrome and chromedriver do the browsing, started with the
  # arguments in polyoxide-wasm-smoke/webdriver.json.
  wasm:
    name: WebAssembly
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
          components: clippy
      - uses: mozilla-actions/sccache-action@v0.0.9
        with:
          version: "v0.14.0"
      - id: wasm-bindgen
        run: |
          version=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
          echo "version=$version" >> "$GITHUB_OUTPUT"
      - uses: taiki-e/install-action@v2
        with:
          tool: wasm-bindgen@${{ steps.wasm-bindgen.outputs.version }}
      - run: bash scripts/wasm_check.sh
      - run: bash scripts/wasm_smoke.sh
```

In `CLAUDE.md`, make four edits.

1. In the dev shell paragraph, replace `cargo-nextest, uv and python3, so` with `cargo-nextest, uv, python3, chromium, chromedriver (Linux) and the wasm-bindgen CLI whose version `Cargo.lock` pins, so`.

2. Replace the heading `### The nine CI jobs` with `### The ten CI jobs`.

3. In the `| CI scripts |` row, replace

```
`cd .github/scripts && uv sync && uv run pytest tests/` | dependency fences,
```

with

```
`cd .github/scripts && uv sync && uv run pytest tests/`, then `python3 scripts/runtime_gate.py` | dependency fences, a timer, clock read or spawn outside the injected runtime,
```

4. Add the tenth row after the `| CI scripts |` row:

```
| wasm | `bash scripts/wasm_check.sh && bash scripts/wasm_smoke.sh` (`ci-gates.md`) | a supported crate that does not build or lint for `wasm32-unknown-unknown`, or a browser smoke test that fails in headless Chrome |
```

In `.claude/rules/ci-gates.md`, replace the front matter's last glob

```
  - "flake.nix"
---
```

with

```
  - "flake.nix"
  - "polyoxide-wasm-smoke/**"
  - "scripts/wasm_*.sh"
---
```

replace `CLAUDE.md lists the nine jobs and the command each runs; this file says what each checks.` with `CLAUDE.md lists the ten jobs and the command each runs; this file says what each checks.`, and insert this section before `## removals`:

```
## wasm

`scripts/wasm_check.sh` runs `cargo check` and `cargo clippy -- -D warnings` for
`wasm32-unknown-unknown` over the supported set, crate by crate with its features named. It sets
`RUSTFLAGS='--cfg getrandom_backend="wasm_js"'` itself, replacing any inherited value, since an
inherited `RUSTFLAGS` (a host `target-cpu`) would override a config file's and break the target;
with `--target` cargo passes no `RUSTFLAGS` to build scripts, so the host cache is untouched. A
change to the supported set edits that script.

`scripts/wasm_smoke.sh` then runs `polyoxide-wasm-smoke`'s `tests/browser_smoke.rs` in headless
Chrome, inside a dedicated Worker, through `wasm-bindgen-test-runner`. It builds and starts the
native `wasm-smoke-server`, which binds `127.0.0.1:0` for mockito routes (every one answering CORS
headers: a 200 and two 429s, one exposing `Retry-After` and one not, on one server, and a slow
route on a server of its own, since mockito answers it by sleeping its server's one thread and
wasm-bindgen-test may run async tests concurrently) and for one scripted socket on
`polyoxide-ws`'s test server, and writes `POLYOXIDE_SMOKE_HTTP`, `POLYOXIDE_SMOKE_WS` and
`POLYOXIDE_SMOKE_SLOW` to the file its one argument names. The script exports them, and the tests
read them with `env!`, so a build without them does not compile and the suite cannot pass empty.
The runner starts Chrome with the arguments in `polyoxide-wasm-smoke/webdriver.json`
(`--headless`, `--no-sandbox`, `--disable-gpu`, `--disable-dev-shm-usage`): without `--no-sandbox`
Chrome cannot start where unprivileged user namespaces are restricted, as on a hosted runner, and
the script refuses to run without the file.
It refuses a `wasm-bindgen-cli` whose version is not the `wasm-bindgen` one `Cargo.lock` pins,
since the runner rejects a module built by another. The suite covers `BrowserRuntime` (sleep,
`sleep_until`, `timeout`, `interval`, spawn, abort, `is_finished`, a monotonic `now`, a sleep past
`i32::MAX` ms), `FetchTransport` (a 200, a timeout, both 429s read through `ErrorResponse`) and
`WebSocketTransport` (a text echo, a close with its code, a refused handshake header and ping
frame). Each test's failure was shown by breaking the one thing it pins, so a new test comes with
the same proof.

CI's job installs the wasm32 target and `wasm-bindgen-cli` at the locked version, and uses the
runner's Chrome and chromedriver. Locally, the flake provides chromium, chromedriver (Linux) and
the CLI; if the flake's CLI is not the locked version, move the lockfile to it with
`cargo update -p wasm-bindgen --precise <version>`. `scripts/runtime_gate.py` is not part of this
job: it runs in the scripts job.
```

Then run the dependency tests, which this story's `Cargo.lock` change touches:

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_dependency_fences.py tests/test_claude_rules.py -q'`

Expected: all pass. The new crate is `publish = false` and declares no crate a fence owns, so `ci/dep-allowlists/` should not change. If `test_dep_allowlists` reports a changed tree, regenerate with `cd .github/scripts && uv run python -m tests.test_dep_allowlists --write` and say in the commit message which dependency changed it.

- [ ] **Step 4: Run it to see it pass**

Run the command from Step 1.

Expected: `ok`.

Then run the story's gates: `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide-wasm-smoke --all-targets --all-features -- -D warnings`, `nix develop --command cargo test -p polyoxide-wasm-smoke --all-features`, `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-wasm-smoke --all-features'`, `nix develop --command python3 scripts/gen_registry.py --check`, `nix develop --command python3 scripts/venue_gate.py`, `nix develop --command python3 scripts/live_unwraps.py`, `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`, and the story's Verify line under `nix develop --command`.

Expected: every command exits 0.

- [ ] **Step 5: Commit**

```bash
git commit -m "ci(wasm): the WebAssembly job, the runtime gate in the scripts job and the docs for both (Story 9)" -- .github/workflows/ci.yml CLAUDE.md .claude/rules/ci-gates.md
```

## Story 10: The nightly live browser job

- **Developer:** intermediate-dev
- **Files:** `polyoxide-wasm-smoke/Cargo.toml`, `polyoxide-wasm-smoke/tests/browser_live.rs`, `scripts/wasm_live.sh`, `scripts/gen_registry.py`, `.github/workflows/nightly-behavioral.yml`, `CLAUDE.md`, `SELF-HEALING.md`, `polyoxide-test-support/Cargo.toml`, `polyoxide-test-support/src/lib.rs`, `polyoxide-test-support/src/tag.rs`, `.github/scripts/tests/test_gen_registry.py`, `.github/scripts/tests/test_live_registry.py`, `docs/specs/clob/OBSERVED.md`, `docs/specs/gamma/OBSERVED.md`, `docs/specs/data-v2/OBSERVED.md`, `docs/specs/perps/OBSERVED.md`, `docs/specs/binance/OBSERVED.md`, `docs/specs/kalshi/OBSERVED.md`, `docs/specs/sports/OBSERVED.md`, `docs/specs/rtds/OBSERVED.md`, `.claude/rules/live-tests.md`, `Cargo.lock`, `ci/dep-allowlists/`
- **Interfaces:** exposes "Story 10 → Story 11"; uses Story 7's implementations, Story 8's keepalive and `OBSERVED.md` headings, and Story 9's crate and env-file contract
- **Verify:** `bash scripts/wasm_live.sh && python3 scripts/gen_registry.py --check && python3 scripts/live_unwraps.py && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
- **Review:** deep (a nightly classifier contract and live limits)

This story adds the nightly browser job: the same three browser implementations as the smoke suite, now against the real hosts, once a night, with no secrets. `polyoxide-test-support`'s tags build on wasm32 (everything that reads a file, the keychain or a native clock stays native-only) and print their tag line through `console.error`, since std's stderr prints nothing in a browser and wasm-bindgen-test captures the console. `tests/browser_live.rs`, behind a `live` feature rather than `#[ignore]`, checks per host what spec §1.4 measured: one public GET per HTTP host through `FetchTransport` (perps' `x-pmp-pod` is readable and Binance's `X-MBX-USED-WEIGHT-1M` is not), one socket per socket host held until its first data frame or 20 s, and Binance's `LIST_SUBSCRIPTIONS` answered, each GET bounded by 10 s, each check retried once after 5 s, a second failure `real` except a socket that opened and then dropped, which is `transient` (spec D17). `scripts/wasm_live.sh` runs it through `wasm-bindgen-test-runner`, turns the runner's output into the libtest-json the classifier reads, proves with a probe test that a tag line from a failing browser test reaches the classifier, and runs one retry pass over what the classifier calls transient. `scripts/gen_registry.py` learns a `runner` key on a live entry; the one entry that uses it generates the `live-wasm-browser` job, and the browser facts are recorded in each host's `OBSERVED.md`. None of this code holds a landmine marker.


### Task 10.1: The tags build on wasm32

**Files:**
- Modify: `polyoxide-test-support/Cargo.toml`
- Modify: `polyoxide-test-support/src/lib.rs`
- Modify: `polyoxide-test-support/src/tag.rs`

- [ ] **Step 1: Write the failing test**

The test is the wasm32 build of the crate, which its browser users depend on:

```bash
nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-test-support --target wasm32-unknown-unknown
```

- [ ] **Step 2: Run it to see it fail**

Run the command from Step 1.

Expected: it fails while compiling a dependency that cannot build for `wasm32-unknown-unknown`: the keychain crates `polyoxide-core`'s `keychain` feature pulls in, which `polyoxide-test-support` always turns on.

- [ ] **Step 3: Implement**

In `polyoxide-test-support/Cargo.toml`, replace the `[dependencies]` table

```toml
[dependencies]
polyoxide-core = { workspace = true, features = ["keychain"] }
polyoxide-venue = { workspace = true }
dotenvy = "0.15"
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["time"] }
tracing = { workspace = true }
# `registration!`: the ids a crate declares in its own `Cargo.toml`.
toml = "0.8"
# `query`: the query keys a builder sends, read off a mock server.
mockito = { workspace = true, optional = true }
url = { workspace = true, optional = true }
# `soak`: the observer that counts the retry loops' warnings.
tracing-subscriber = { workspace = true, optional = true }
```

with

```toml
[dependencies]
polyoxide-venue = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
# `query`: the query keys a builder sends, read off a mock server.
mockito = { workspace = true, optional = true }
url = { workspace = true, optional = true }
# `soak`: the observer that counts the retry loops' warnings.
tracing-subscriber = { workspace = true, optional = true }

# Everything but the tags reads a file, the keychain or a native clock, so it builds off wasm32
# only: a browser test links the tags alone (spec D14).
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
polyoxide-core = { workspace = true, features = ["keychain"] }
dotenvy = "0.15"
tokio = { workspace = true, features = ["time"] }
# `registration!`: the ids a crate declares in its own `Cargo.toml`.
toml = "0.8"

# The tag line goes to the browser console, where wasm-bindgen-test captures it.
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
```

In `polyoxide-test-support/src/lib.rs`, replace the end of the crate docs and the module list. The docs end

```rust
//! Their failures panic with no tag, so the nightly classifier files them as
//! `real`: a wire or schema disagreement is a fault.

#![warn(missing_docs)]

pub mod agreement;
mod creds;
mod fixtures;
pub mod minute;
pub mod openapi;
#[cfg(feature = "query")]
pub mod query;
mod registration;
#[cfg(feature = "soak")]
pub mod soak;
mod tag;

pub use creds::{keychain, load_env, optional_env, Creds, Missing};
pub use fixtures::Fixtures;
pub use registration::Registration;
pub use tag::{
```

Replace that text with

```rust
//! Their failures panic with no tag, so the nightly classifier files them as
//! `real`: a wire or schema disagreement is a fault.
//!
//! # In a browser
//!
//! On `wasm32-unknown-unknown` only the tags build: [`fail`], [`environmental`],
//! [`transient`], [`tag_for`] and the extension traits. The loaders, the fixture,
//! agreement and soak helpers read files, the keychain and native clocks, and are
//! absent. The tag line goes to `console.error`, since std's stderr prints
//! nothing there and wasm-bindgen-test captures the console into the failing
//! test's output, where `scripts/wasm_live.sh` reads it.

#![warn(missing_docs)]

#[cfg(not(target_arch = "wasm32"))]
pub mod agreement;
#[cfg(not(target_arch = "wasm32"))]
mod creds;
#[cfg(not(target_arch = "wasm32"))]
mod fixtures;
#[cfg(not(target_arch = "wasm32"))]
pub mod minute;
#[cfg(not(target_arch = "wasm32"))]
pub mod openapi;
#[cfg(all(feature = "query", not(target_arch = "wasm32")))]
pub mod query;
#[cfg(not(target_arch = "wasm32"))]
mod registration;
#[cfg(all(feature = "soak", not(target_arch = "wasm32")))]
pub mod soak;
mod tag;

#[cfg(not(target_arch = "wasm32"))]
pub use creds::{keychain, load_env, optional_env, Creds, Missing};
#[cfg(not(target_arch = "wasm32"))]
pub use fixtures::Fixtures;
#[cfg(not(target_arch = "wasm32"))]
pub use registration::Registration;
pub use tag::{
```

In `polyoxide-test-support/src/tag.rs`, replace the hook's print

```rust
                // `eprintln!` rather than a raw stderr write, so the line joins
                // the test's captured output, ahead of the report below.
                eprintln!("{PREFIX}{tag}");
```

with

```rust
                // `emit` rather than a raw stderr write, so the line joins the
                // test's captured output, ahead of the report below.
                emit(&format!("{PREFIX}{tag}"));
```

and add `emit` and the console binding right after the existing line `static HOOK: Once = Once::new();`:

```rust
/// Print one line where the nightly's runner reads it: stderr, or on wasm32 the
/// browser console, since std's stderr prints nothing there and wasm-bindgen-test
/// captures `console.error` into the failing test's output.
fn emit(line: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{line}");
    #[cfg(target_arch = "wasm32")]
    console::error(line);
}

#[cfg(target_arch = "wasm32")]
mod console {
    use wasm_bindgen::prelude::wasm_bindgen;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = console)]
        pub fn error(message: &str);
    }
}
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check -p polyoxide-test-support --target wasm32-unknown-unknown`

Expected: `Finished` with no error.

Run: `nix develop --command env RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo clippy -p polyoxide-test-support --target wasm32-unknown-unknown -- -D warnings`

Expected: no warning.

Run the native suites, which the move must not change: `nix develop --command cargo test -p polyoxide-test-support --all-features`

Expected: every test passes, and `tests/tags.rs`'s checks of the stderr tag line are among them.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(test-support): the failure tags build on wasm32 and print through the browser console (Story 10)" -- polyoxide-test-support/Cargo.toml polyoxide-test-support/src/lib.rs polyoxide-test-support/src/tag.rs Cargo.lock
```

### Task 10.2: The `live` feature, the probe and `scripts/wasm_live.sh`

**Files:**
- Modify: `polyoxide-wasm-smoke/Cargo.toml`
- Create: `polyoxide-wasm-smoke/tests/browser_live.rs`
- Create: `scripts/wasm_live.sh`

- [ ] **Step 1: Write the failing test**

In `polyoxide-wasm-smoke/Cargo.toml`, give the crate its feature and the test target that needs it. Replace

```toml
[dependencies]
tungstenite = { version = "0.26", default-features = false }

# The server is native only, and its socket is the kit's scripted test server.
```

with

```toml
[dependencies]
tungstenite = { version = "0.26", default-features = false }

[features]
# The nightly's live checks (tests/browser_live.rs). They call real hosts, so they are off by
# default, and run through scripts/wasm_live.sh.
live = []

[[test]]
name = "browser_live"
required-features = ["live"]

# The server is native only, and its socket is the kit's scripted test server.
```

and, in the wasm32 `dev-dependencies` table, replace

```toml
polyoxide-rt = { workspace = true }
polyoxide-ws = { workspace = true }
```

with

```toml
polyoxide-rt = { workspace = true }
polyoxide-test-support = { path = "../polyoxide-test-support" }
polyoxide-venue = { workspace = true }
polyoxide-ws = { workspace = true }
```

Create `polyoxide-wasm-smoke/tests/browser_live.rs`:

```rust
//! The nightly's live browser checks (spec D14, D17): the browser implementations against the
//! real hosts, run by `scripts/wasm_live.sh` in headless Chrome inside a dedicated Worker.
//!
//! They are gated by the `live` feature, not `#[ignore]`: nextest does not run wasm, and the
//! script selects the target. A failure is tagged through `polyoxide-test-support`, whose tags
//! build on wasm32, so the classifier reads the same `polyoxide-class=<tag>` line it reads for the
//! native suites; an untagged panic is `real`.

#![cfg(all(target_arch = "wasm32", feature = "live"))]

use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_dedicated_worker);

/// The self-check `scripts/wasm_live.sh` runs first: built with `POLYOXIDE_LIVE_PROBE` set, this
/// test fails with a `transient` tag, and the script checks that the classifier reads that tag
/// from the runner's output. Built without it, the test passes at once.
#[wasm_bindgen_test]
fn probe_tag_line() {
    if option_env!("POLYOXIDE_LIVE_PROBE").is_none() {
        return;
    }
    polyoxide_test_support::transient("the runner's output must carry this test's tag line");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash scripts/wasm_live.sh`

Expected: `bash: scripts/wasm_live.sh: No such file or directory`, exit 127.

- [ ] **Step 3: Implement**

Create `scripts/wasm_live.sh`:

```bash
#!/usr/bin/env bash
# The nightly's browser job (spec D14, D17). It runs polyoxide-wasm-smoke's `browser_live` in
# headless Chrome through wasm-bindgen-test-runner and writes what the nightly classifier reads:
# libtest-json, since the classifier was written for nextest. A test that failed carries the
# runner's output for it, with the `polyoxide-class=<tag>` line the tags print.
#
#   target/wasm-live/first-pass.json   every test, as ok or failed
#   target/wasm-live/retry.json        the tests the classifier called transient, run once more
#
# Before the real run it runs a probe: a test that fails with a `transient` tag. The classifier
# must read that tag from the runner's output, or this script exits 1, since a job that cannot
# read its tags would file every failure as `real`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

out=target/wasm-live
binary='polyoxide-wasm-smoke::browser_live'
mkdir -p "$out"
rm -rf "$out"/*.log "$out"/*.json "$out"/probe "$out"/first-pass

for tool in wasm-bindgen wasm-bindgen-test-runner; do
  if ! command -v "$tool" >/dev/null; then
    echo "wasm_live: $tool is not on PATH; run inside \`nix develop\` or install wasm-bindgen-cli" >&2
    exit 1
  fi
done

# The runner and the wasm-bindgen crate must be the same version.
locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
installed=$(wasm-bindgen --version | awk '{ print $2 }')
if [ "$locked" != "$installed" ]; then
  echo "wasm_live: wasm-bindgen-cli is $installed but Cargo.lock pins wasm-bindgen $locked; they must match" >&2
  exit 1
fi

# The runner takes its driver from CHROMEDRIVER. A GitHub runner has it beside CHROMEWEBDRIVER.
if [ -z "${CHROMEDRIVER:-}" ]; then
  if command -v chromedriver >/dev/null; then
    CHROMEDRIVER=$(command -v chromedriver)
  elif [ -x "${CHROMEWEBDRIVER:-}/chromedriver" ]; then
    CHROMEDRIVER="$CHROMEWEBDRIVER/chromedriver"
  else
    echo "wasm_live: no chromedriver on PATH, in CHROMEDRIVER or in CHROMEWEBDRIVER" >&2
    exit 1
  fi
  export CHROMEDRIVER
fi

# run <log> [filter]: the live tests, whose output goes to <log> and to stderr. A failing test
# is the classifier's business, so a failing run is not this script's.
run() {
  local log=$1
  shift
  RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
  CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  WASM_BINDGEN_TEST_TIMEOUT=800 \
    cargo test -p polyoxide-wasm-smoke --target wasm32-unknown-unknown \
      --features live --test browser_live -- "$@" >"$log" 2>&1 </dev/null || true
  cat "$log" >&2
}

# convert <log> <json>: the runner's report as libtest-json events. A failed test's block is
# dedented, and the panic report the hook leaves is given the `thread '...' panicked at` shape the
# classifier looks for, so the tag line stays just above it.
convert() {
  python3 -I - "$1" "$2" <<'PY'
import json
import re
import sys

log, out = sys.argv[1], sys.argv[2]
BINARY = "polyoxide-wasm-smoke::browser_live"
RESULT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)\s*$")
HEADING = re.compile(r"^---- (\S+) output ----\s*$")
PANIC = re.compile(r"^panicked at ")

lines = open(log, encoding="utf-8", errors="replace").read().splitlines()

results = {}
for line in lines:
    found = RESULT.match(line.strip())
    if found:
        results[found[1]] = found[2]

blocks, current = {}, None
for line in lines:
    heading = HEADING.match(line.strip())
    if heading:
        current = heading[1]
        blocks[current] = []
        continue
    if current is None:
        continue
    text = line.strip()
    if text.startswith(("failures:", "test result:")):
        current = None
        continue
    if PANIC.match(text):
        text = "thread 'main' " + text
    blocks[current].append(text)

if not results:
    sys.exit("wasm_live: the runner printed no test results; the build or the runner failed")

with open(out, "w", encoding="utf-8") as sink:
    for name, outcome in results.items():
        if outcome == "ignored":
            continue
        event = {
            "type": "test",
            "name": f"{BINARY}${name.rsplit('::', 1)[-1]}",
            "event": "ok" if outcome == "ok" else "failed",
        }
        if outcome == "FAILED":
            event["stdout"] = "\n".join(blocks.get(name, [])) + "\n"
        sink.write(json.dumps(event) + "\n")
PY
}

# classify <json> <dir>: the classifier's own first pass, which writes retry-tests.txt and
# real-failures.txt into <dir>.
classify() {
  python3 -I .github/scripts/classify_failures.py classify --input "$1" --output-dir "$2"
}

POLYOXIDE_LIVE_PROBE=1 run "$out/probe.log" probe_tag_line
convert "$out/probe.log" "$out/probe.json"
classify "$out/probe.json" "$out/probe"
if ! grep -qxF "$binary\$probe_tag_line" "$out/probe/retry-tests.txt"; then
  echo "wasm_live: the probe's tag line did not reach the classifier; the runner's output is above" >&2
  exit 1
fi
echo "wasm_live: the probe's tag line reached the classifier" >&2

run "$out/first-pass.log"
convert "$out/first-pass.log" "$out/first-pass.json"
classify "$out/first-pass.json" "$out/first-pass"

# One more run of what the classifier calls transient, as the native suites' retry pass does.
: >"$out/retry.json"
if [ -s "$out/first-pass/retry-tests.txt" ]; then
  n=0
  while IFS= read -r name; do
    n=$((n + 1))
    run "$out/retry-$n.log" "${name#*\$}"
    convert "$out/retry-$n.log" "$out/retry-$n.json"
    cat "$out/retry-$n.json" >>"$out/retry.json"
  done <"$out/first-pass/retry-tests.txt"
fi

echo "wasm_live: wrote $out/first-pass.json and $out/retry.json" >&2
```

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash scripts/wasm_live.sh`

Expected: the stderr shows `wasm_live: the probe's tag line reached the classifier`, then the real run's `test browser_live::probe_tag_line ... ok`, and ends `wasm_live: wrote target/wasm-live/first-pass.json and target/wasm-live/retry.json`, exit 0. `cat target/wasm-live/first-pass.json` holds one `"event": "ok"` line named `polyoxide-wasm-smoke::browser_live$probe_tag_line`.

Prove the probe can fail: in `polyoxide-test-support/src/tag.rs`, temporarily make `emit` print nothing on wasm32 by replacing `console::error(line);` with `let _ = line;`.

Run: `nix develop --command bash scripts/wasm_live.sh`

Expected: exit 1 after printing `wasm_live: the probe's tag line did not reach the classifier; the runner's output is above`, the runner's output showing the probe failed with no `polyoxide-class=transient` line. Restore `console::error(line);`.

- [ ] **Step 5: Commit**

```bash
git add -N polyoxide-wasm-smoke/tests/browser_live.rs scripts/wasm_live.sh
git commit -m "test(wasm-smoke): the live feature, the tag-line probe and scripts/wasm_live.sh (Story 10)" -- polyoxide-wasm-smoke/Cargo.toml polyoxide-wasm-smoke/tests/browser_live.rs scripts/wasm_live.sh Cargo.lock
```

### Task 10.3: The live checks

**Files:**
- Modify: `polyoxide-wasm-smoke/tests/browser_live.rs`

- [ ] **Step 1: Write the failing test**

Replace the whole of `polyoxide-wasm-smoke/tests/browser_live.rs` with:

```rust
//! The nightly's live browser checks (spec D14, D17): the browser implementations against the
//! real hosts, run by `scripts/wasm_live.sh` in headless Chrome inside a dedicated Worker.
//!
//! They are gated by the `live` feature, not `#[ignore]`: nextest does not run wasm, and the
//! script selects the target. A failure is tagged through `polyoxide-test-support`, whose tags
//! build on wasm32, so the classifier reads the same `polyoxide-class=<tag>` line it reads for the
//! native suites; an untagged panic is `real`.
//!
//! Per D17 each check retries once after 5 s. A second failure is `real`, except a socket that
//! opened and then dropped, which is `transient`, and a host the runner cannot reach for a reason
//! of its own (a region block), which is `environmental`. A GET is bounded by 10 s, a socket held
//! until its first data frame or 20 s.

#![cfg(all(target_arch = "wasm32", feature = "live"))]

use std::{future::Future, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use polyoxide_core::{
    ApiError, FetchTransport, HttpClientBuilder, Request, Response, RetryConfig,
};
use polyoxide_rt::{timeout, BrowserRuntime, Runtime};
use polyoxide_test_support::{environmental, tag_for, transient, Tag};
use polyoxide_venue::Classify;
use polyoxide_ws::{BoxSocket, SocketTransport, WebSocketTransport};
use serde_json::Value;
use tungstenite::{http::Request as Upgrade, Message};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_dedicated_worker);

const GET_LIMIT_MS: u64 = 10_000;
const CONNECT_LIMIT: Duration = Duration::from_secs(10);
const SOCKET_LIMIT: Duration = Duration::from_secs(20);
const ANSWER_LIMIT: Duration = Duration::from_secs(5);
const RETRY_AFTER: Duration = Duration::from_secs(5);
/// Binance's pacing of requests on one socket is 200 ms.
const REQUEST_GAP: Duration = Duration::from_millis(250);

/// What a check found.
enum Failure {
    /// The expectation did not hold, or the host stayed down through the retry.
    Real(String),
    /// The socket opened and then dropped.
    Transient(String),
    /// The runner cannot test the host, such as a region block.
    Environmental(String),
}

/// Fails untagged, which the classifier files as `real`.
fn real(reason: &str) -> ! {
    panic!("{reason}")
}

/// Run `check`; on a failure other than an environmental one, run it once more after
/// [`RETRY_AFTER`]; a second failure fails the test with its tag.
async fn checked<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), Failure>>,
{
    let mut failure = match check().await {
        Ok(()) => return,
        Err(failure) => failure,
    };
    if !matches!(failure, Failure::Environmental(_)) {
        BrowserRuntime.sleep(RETRY_AFTER).await;
        match check().await {
            Ok(()) => return,
            Err(second) => failure = second,
        }
    }
    match failure {
        Failure::Environmental(reason) => environmental(&format!("{what}: {reason}")),
        Failure::Transient(reason) => transient(&format!("{what}: {reason}")),
        Failure::Real(reason) => real(&format!("{what}: {reason}")),
    }
}

struct Host {
    base: &'static str,
    path: &'static str,
}

const CLOB: Host = Host {
    base: "https://clob.polymarket.com",
    path: "/time",
};
const GAMMA: Host = Host {
    base: "https://gamma-api.polymarket.com",
    path: "/status",
};
const DATA: Host = Host {
    base: "https://data-api.polymarket.com",
    path: "/",
};
const PERPS: Host = Host {
    base: "https://api.perpetuals.polymarket.com",
    path: "/v1/info/ping",
};
const FAPI: Host = Host {
    base: "https://fapi.binance.com",
    path: "/fapi/v1/ping",
};

/// One public GET through `FetchTransport`, with no retry of its own. A cross-origin refusal and
/// a network failure look alike to a browser, and both fail here.
async fn get(host: &Host) -> Result<Response, Failure> {
    let client = HttpClientBuilder::new(host.base)
        .runtime(Arc::new(BrowserRuntime))
        .http_transport(Arc::new(FetchTransport::new()))
        .with_retry_config(RetryConfig {
            max_retries: 0,
            initial_backoff_ms: 1,
            max_backoff_ms: 1,
        })
        .timeout_ms(GET_LIMIT_MS)
        .build()
        .map_err(|err| Failure::Real(format!("building the client: {err}")))?;
    Request::<Value, ApiError>::new(client, host.path)
        .send_raw()
        .await
        .map_err(|err| {
            let detail = format!("GET {}{}: {err}", host.base, host.path);
            match tag_for(&err.class(), err.is_fault()) {
                Tag::Environmental => Failure::Environmental(detail),
                _ => Failure::Real(detail),
            }
        })
}

#[wasm_bindgen_test]
async fn get_clob() {
    checked("clob GET /time", || async { get(&CLOB).await.map(drop) }).await;
}

#[wasm_bindgen_test]
async fn get_gamma() {
    checked("gamma GET /status", || async { get(&GAMMA).await.map(drop) }).await;
}

#[wasm_bindgen_test]
async fn get_data() {
    checked("data GET /", || async { get(&DATA).await.map(drop) }).await;
}

/// The one host in scope that names a header in `Access-Control-Expose-Headers`: `x-pmp-pod` is
/// readable here, as `Retry-After` is.
#[wasm_bindgen_test]
async fn get_perps() {
    checked("perps GET /v1/info/ping", || async {
        let response = get(&PERPS).await?;
        if response.headers().get("x-pmp-pod").is_none() {
            return Err(Failure::Real(
                "x-pmp-pod is not readable: perps no longer exposes it".into(),
            ));
        }
        Ok(())
    })
    .await;
}

/// Binance sends `X-MBX-USED-WEIGHT-1M` and does not expose it, so a browser reads none: the
/// weight accounting cannot work there. If this fails, the host started exposing it.
#[wasm_bindgen_test]
async fn get_fapi() {
    checked("fapi GET /fapi/v1/ping", || async {
        let response = get(&FAPI).await?;
        if response.headers().get("x-mbx-used-weight-1m").is_some() {
            return Err(Failure::Real(
                "X-MBX-USED-WEIGHT-1M became readable: fapi now exposes it".into(),
            ));
        }
        Ok(())
    })
    .await;
}

struct SocketHost {
    url: &'static str,
    /// The first frame, for a host that sends nothing until it is spoken to.
    hello: Option<&'static str>,
}

const CLOB_MARKET: SocketHost = SocketHost {
    url: "wss://ws-subscriptions-clob.polymarket.com/ws/market",
    hello: Some("PING"),
};
const SPORTS: SocketHost = SocketHost {
    url: "wss://sports-api.polymarket.com/ws",
    // The feed sends data only while a match is live: held for the window, it has passed.
    hello: None,
};
const PERPS_WS: SocketHost = SocketHost {
    url: "wss://ws.perpetuals.polymarket.com/v1/ws",
    hello: Some(r#"{"id":1,"req":"post","op":{"type":"ping"}}"#),
};
const FSTREAM: SocketHost = SocketHost {
    url: "wss://fstream.binance.com/market/stream",
    hello: Some(r#"{"method":"SUBSCRIBE","params":["btcusdt@markPrice@1s"],"id":1}"#),
};

async fn open(url: &str) -> Result<BoxSocket, Failure> {
    let request = Upgrade::builder()
        .uri(url)
        .body(())
        .map_err(|err| Failure::Real(format!("building the upgrade request: {err}")))?;
    let runtime: Arc<dyn Runtime> = Arc::new(BrowserRuntime);
    WebSocketTransport::new()
        .connect(request, CONNECT_LIMIT, runtime)
        .await
        .map_err(|err| Failure::Real(format!("{url}: the upgrade failed: {err}")))
}

/// Open `host`, speak first if it needs it, and hold the socket until its first data frame or the
/// window ends. A socket that closes or errors first opened and then dropped.
async fn hold(host: &SocketHost) -> Result<(), Failure> {
    let mut socket = open(host.url).await?;
    if let Some(hello) = host.hello {
        socket
            .send(Message::Text(hello.into()))
            .await
            .map_err(|err| Failure::Real(format!("sending the first frame: {err}")))?;
    }
    let outcome = match timeout(&BrowserRuntime, SOCKET_LIMIT, socket.next()).await {
        None => Ok(()),
        Some(Some(Ok(Message::Text(_) | Message::Binary(_)))) => Ok(()),
        Some(Some(Ok(other))) => Err(Failure::Transient(format!(
            "the first frame was {other:?}, not data"
        ))),
        Some(Some(Err(err))) => Err(Failure::Transient(format!("the socket dropped: {err}"))),
        Some(None) => Err(Failure::Transient("the socket closed with no frame".into())),
    };
    let _ = socket.close().await;
    outcome
}

#[wasm_bindgen_test]
async fn socket_clob_market() {
    checked("clob market socket", || hold(&CLOB_MARKET)).await;
}

#[wasm_bindgen_test]
async fn socket_sports() {
    checked("sports socket", || hold(&SPORTS)).await;
}

#[wasm_bindgen_test]
async fn socket_perps() {
    checked("perps socket", || hold(&PERPS_WS)).await;
}

/// A browser cannot read the status of a refused upgrade, so a Binance region block (a 451 on the
/// REST host as well) shows as a failed connect: look at the REST host before calling it real.
async fn fstream() -> Result<(), Failure> {
    match hold(&FSTREAM).await {
        Err(Failure::Real(reason)) => match get(&FAPI).await {
            Err(Failure::Environmental(region)) => Err(Failure::Environmental(format!(
                "{reason}; and the REST host: {region}"
            ))),
            _ => Err(Failure::Real(reason)),
        },
        other => other,
    }
}

#[wasm_bindgen_test]
async fn socket_fstream() {
    checked("fstream socket", fstream).await;
}

/// Binance's `LIST_SUBSCRIPTIONS`, the request the supervised protocol's keepalive is, since a
/// browser cannot send a protocol ping: it must be answered, naming the stream subscribed.
async fn list_subscriptions() -> Result<(), Failure> {
    let mut socket = open(FSTREAM.url).await?;
    for (id, request) in [
        (
            1,
            r#"{"method":"SUBSCRIBE","params":["btcusdt@markPrice@1s"],"id":1}"#,
        ),
        (2, r#"{"method":"LIST_SUBSCRIPTIONS","id":2}"#),
    ] {
        if id > 1 {
            BrowserRuntime.sleep(REQUEST_GAP).await;
        }
        socket
            .send(Message::Text(request.into()))
            .await
            .map_err(|err| Failure::Real(format!("sending request {id}: {err}")))?;
    }
    let answered = timeout(&BrowserRuntime, ANSWER_LIMIT, async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                        continue;
                    };
                    if value["id"] == 2 {
                        return Ok(value);
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    return Err(Failure::Transient(format!("the socket dropped: {err}")))
                }
                None => {
                    return Err(Failure::Transient(
                        "the socket closed before it answered".into(),
                    ))
                }
            }
        }
    })
    .await;
    let _ = socket.close().await;
    match answered {
        None => Err(Failure::Real(
            "LIST_SUBSCRIPTIONS was not answered within 5 s".into(),
        )),
        Some(Err(failure)) => Err(failure),
        Some(Ok(value)) => {
            let listed = value["result"].as_array().is_some_and(|names| {
                names
                    .iter()
                    .any(|name| name.as_str() == Some("btcusdt@markPrice@1s"))
            });
            if listed {
                Ok(())
            } else {
                Err(Failure::Real(format!(
                    "LIST_SUBSCRIPTIONS answered without the stream: {value}"
                )))
            }
        }
    }
}

#[wasm_bindgen_test]
async fn fstream_keepalive() {
    checked("fstream LIST_SUBSCRIPTIONS", list_subscriptions).await;
}

/// The self-check `scripts/wasm_live.sh` runs first: built with `POLYOXIDE_LIVE_PROBE` set, this
/// test fails with a `transient` tag, and the script checks that the classifier reads that tag
/// from the runner's output. Built without it, the test passes at once.
#[wasm_bindgen_test]
fn probe_tag_line() {
    if option_env!("POLYOXIDE_LIVE_PROBE").is_none() {
        return;
    }
    transient("the runner's output must carry this test's tag line");
}
```

- [ ] **Step 2: Run it to see it fail**

Prove a failing live check reaches the classifier as `real`: temporarily change `GAMMA`'s `path` from `"/status"` to `"/status-missing"`.

Run: `nix develop --command bash scripts/wasm_live.sh`

Expected: after about 5 s of retry, `test browser_live::get_gamma ... FAILED`; then `grep -F get_gamma target/wasm-live/first-pass/real-failures.txt` prints `polyoxide-wasm-smoke::browser_live$get_gamma`, because the untagged panic is `real`. Restore `"/status"`.

- [ ] **Step 3: Implement**

The code is the Step 1 file; restore `GAMMA`'s path as above.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash scripts/wasm_live.sh`

Expected: exit 0, the runner reporting eleven tests, and `target/wasm-live/first-pass/real-failures.txt` empty. A name in `target/wasm-live/first-pass/environmental.txt` is a host the runner cannot test from where it is (a 451 from the Binance hosts on a US runner); a name in `retry-tests.txt` was transient and `target/wasm-live/retry.json` holds its second run.

- [ ] **Step 5: Commit**

```bash
git commit -m "test(wasm-smoke): the live browser checks per host, with D17's limits and retry (Story 10)" -- polyoxide-wasm-smoke/tests/browser_live.rs
```

### Task 10.4: The generator's `runner` key and the `live-wasm-browser` job

**Files:**
- Modify: `.github/scripts/tests/test_gen_registry.py`
- Modify: `.github/scripts/tests/test_live_registry.py`
- Modify: `scripts/gen_registry.py`
- Modify: `polyoxide-wasm-smoke/Cargo.toml`
- Modify: `.github/workflows/nightly-behavioral.yml` (generated)
- Modify: `CLAUDE.md` (generated region)
- Modify: `SELF-HEALING.md` (generated region)

- [ ] **Step 1: Write the failing test**

In `.github/scripts/tests/test_gen_registry.py`, let the `live` helper take a runner. Replace

```python
def live(suite: str = "live", timeout: int = 15, features=(), secrets=(), note=None) -> dict:
    entry = {"suite": suite, "timeout": timeout, "features": list(features),
             "secrets": list(secrets)}
    if note:
        entry["note"] = note
    return entry
```

with

```python
def live(suite: str = "live", timeout: int = 15, features=(), secrets=(), note=None,
         runner=None) -> dict:
    entry = {"suite": suite, "timeout": timeout, "features": list(features),
             "secrets": list(secrets)}
    if note:
        entry["note"] = note
    if runner:
        entry["runner"] = runner
    return entry
```

and add these tests after `test_one_job_per_crate_and_suite`:

```python


def _with_browser_job(metadata, workspace, manifests):
    table = manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]
    table["browser_live"] = live(suite="browser", features=["live"], runner="wasm-bindgen")


def test_a_runner_job_is_named_for_its_runner(tmp_path: Path) -> None:
    registry = build(tmp_path, _with_browser_job)
    jobs = {job.id: job for job in registry.jobs()}
    job = jobs["live-wasm-browser"]
    assert (job.crate, job.suite, job.runner, job.secrets) == (
        "polyoxide-omega", "browser", "wasm-bindgen", ())
    # The crate's native suite keeps its own job.
    assert jobs["live-polyoxide-omega-live"].runner is None


def test_a_runner_job_runs_its_script_and_classifies_what_it_writes(tmp_path: Path) -> None:
    import yaml

    registry = build(tmp_path, _with_browser_job)
    jobs = yaml.safe_load("\n".join(gen_registry.nightly_live_jobs(registry)))
    job = jobs["live-wasm-browser"]
    runs = [step["run"] for step in job["steps"] if "run" in step]
    assert "bash scripts/wasm_live.sh" in runs
    assert not any(step.get("uses") == "./.github/actions/live-suite" for step in job["steps"])
    assert job["timeout-minutes"] == 15
    assert "env" not in job
    merge = next(run for run in runs if "classify_failures.py merge" in run)
    assert "--output-dir artifacts/polyoxide-omega/merged" in merge
    upload = next(step for step in job["steps"]
                  if str(step.get("uses", "")).startswith("actions/upload-artifact"))
    assert upload["with"] == {"name": "behavioral-polyoxide-omega-browser",
                              "path": "artifacts/polyoxide-omega/"}
    native = jobs["live-polyoxide-omega-live"]
    assert native["steps"][1]["uses"] == "./.github/actions/live-suite"


def test_the_runner_job_is_listed_beside_the_native_ones(tmp_path: Path) -> None:
    registry = build(tmp_path, _with_browser_job)
    assert "  - live-wasm-browser" in gen_registry.nightly_aggregate_needs(registry)
    assert ("- `live-wasm-browser` (15 min): `browser_live` with `--features live`"
            in gen_registry.claude_nightly(registry))


@pytest.mark.parametrize(
    ("change", "message"),
    [
        ({"secrets": ["A_KEY"]}, "holds no secrets"),
        ({"runner": "selenium"}, "is not one of"),
    ],
)
def test_a_bad_runner_entry_fails(tmp_path: Path, change: dict, message: str) -> None:
    def bad(metadata, workspace, manifests):
        _with_browser_job(metadata, workspace, manifests)
        table = manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]
        table["browser_live"].update(change)

    with pytest.raises(RegistryError, match=message):
        build(tmp_path, bad)


def test_a_suite_may_not_mix_runners(tmp_path: Path) -> None:
    def mixed(metadata, workspace, manifests):
        table = manifests["polyoxide-omega"]["package"]["metadata"]["polyoxide"]["live"]
        table["browser_live"] = live(suite="live", features=["live"], runner="wasm-bindgen")

    with pytest.raises(RegistryError, match="mixes runners"):
        build(tmp_path, mixed)


def test_two_crates_may_not_share_a_runner_job(tmp_path: Path) -> None:
    def twice(metadata, workspace, manifests):
        _with_browser_job(metadata, workspace, manifests)
        table = manifests["polyoxide-alpha"]["package"]["metadata"]["polyoxide"]["live"]
        table["browser_live"] = live(suite="browser", features=["live"], runner="wasm-bindgen")

    with pytest.raises(RegistryError, match="live-wasm-browser is the job of more than one"):
        build(tmp_path, twice)
```

In `.github/scripts/tests/test_live_registry.py`, keep the nextest suites apart from a runner's. Replace

```python
def registered(registry) -> dict[tuple[str, str], object]:
    return {(crate.name, live.target): live for crate in registry.crates for live in crate.live}
```

with

```python
def registered(registry) -> dict[tuple[str, str], object]:
    """The nextest live entries: a `runner` entry is a wasm test the live-suite action cannot run."""
    return {(crate.name, live.target): live for crate in registry.crates for live in crate.live
            if live.runner is None}


def native_jobs(registry) -> list:
    return [job for job in registry.jobs() if job.runner is None]
```

in `test_the_generated_jobs_equal_the_registered_entries`, replace

```python
    assert list(jobs) == [job.id for job in REGISTRY.jobs()]
    selected = set()
    for job in REGISTRY.jobs():
        written = jobs[job.id]
```

with

```python
    assert list(jobs) == [job.id for job in native_jobs(REGISTRY)]
    selected = set()
    for job in native_jobs(REGISTRY):
        written = jobs[job.id]
```

in `test_each_job_carries_only_its_declared_secrets`, replace

```python
    jobs = workflow_jobs(WORKFLOW)
    for job in REGISTRY.jobs():
        env = jobs[job.id].get("env", {})
```

with

```python
    jobs = workflow_jobs(WORKFLOW)
    for job in native_jobs(REGISTRY):
        env = jobs[job.id].get("env", {})
```

and add this test after `test_the_aggregate_needs_every_live_job`:

```python


def test_each_runner_job_runs_its_script_and_is_needed_by_the_aggregate() -> None:
    runner_jobs = [job for job in REGISTRY.jobs() if job.runner is not None]
    assert [job.id for job in runner_jobs] == ["live-wasm-browser"]
    needs = WORKFLOW["jobs"]["aggregate"]["needs"]
    assert (REPO / "scripts" / "wasm_live.sh").is_file()
    for job in runner_jobs:
        written = WORKFLOW["jobs"][job.id]
        assert written["timeout-minutes"] == job.timeout
        assert "env" not in written
        runs = [step["run"] for step in written["steps"] if "run" in step]
        assert "bash scripts/wasm_live.sh" in runs
        assert job.id in needs
        package = next(p for p in gen_registry.publish_order.members(METADATA)
                       if p["name"] == job.crate)
        targets = {t["name"]: t for t in package["targets"] if t["kind"] == ["test"]}
        for live in job.targets:
            assert targets[live.target].get("required-features") == list(live.features)
```

- [ ] **Step 2: Run it to see it fail**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py tests/test_live_registry.py -q'`

Expected: collecting `test_live_registry.py` fails with `AttributeError: 'Live' object has no attribute 'runner'`, and the new `test_gen_registry.py` tests fail with `RegistryError: ... unknown key runner`.

- [ ] **Step 3: Implement**

In `scripts/gen_registry.py`:

1. In the module docstring, after the `note` line of the live table, replace

```
    secrets = ["NAME"]        # every env name the target reads, exactly
    note = "..."              # optional, for SELF-HEALING.md's table
```

with

```
    secrets = ["NAME"]        # every env name the target reads, exactly
    note = "..."              # optional, for SELF-HEALING.md's table
    runner = "wasm-bindgen"   # optional: the target is not a nextest `live_*` test but a wasm32
                              # browser test, run by the runner's script (RUNNERS); its job is
                              # `live-<word>-<suite>`, holds no secrets, and no suite mixes runners
```

2. Replace

```python
LIVE_KEYS = {"suite", "timeout", "features", "secrets", "note"}
```

with

```python
LIVE_KEYS = {"suite", "timeout", "features", "secrets", "note", "runner"}
# The runners a live entry may name: the word in the job's id (`live-<word>-<suite>`), and the
# script that runs the target and writes libtest-json for the classifier.
RUNNERS = {"wasm-bindgen": ("wasm", "scripts/wasm_live.sh")}
```

3. In `Live`, replace

```python
    secrets: tuple[str, ...] = ()
    note: str | None = None


@dataclass(frozen=True)
class GateException:
```

with

```python
    secrets: tuple[str, ...] = ()
    note: str | None = None
    runner: str | None = None


@dataclass(frozen=True)
class GateException:
```

4. In `Job`, replace

```python
    @property
    def id(self) -> str:
        return f"live-{self.crate}-{self.suite}"
```

with

```python
    @property
    def runner(self) -> str | None:
        """The runner of the job's targets, which `_crate` holds to one."""
        return self.targets[0].runner

    @property
    def id(self) -> str:
        if self.runner:
            return f"live-{RUNNERS[self.runner][0]}-{self.suite}"
        return f"live-{self.crate}-{self.suite}"
```

5. In `build`, replace

```python
    _check_products(crates)
    return Registry(crates, _mirrors(root, workspace, crates), _stage(workspace), _guide(root))
```

with

```python
    _check_products(crates)
    registry = Registry(crates, _mirrors(root, workspace, crates), _stage(workspace), _guide(root))
    _check_job_ids(registry)
    return registry
```

and add, after `_check_products`:

```python
def _check_job_ids(registry: Registry) -> None:
    owners: dict[str, list[str]] = {}
    for job in registry.jobs():
        owners.setdefault(job.id, []).append(job.crate)
    for job_id, crates in owners.items():
        if len(crates) > 1:
            raise RegistryError(f"{job_id} is the job of more than one crate: "
                                f"{', '.join(crates)}. A runner's job id has no crate in it, so "
                                f"give each crate its own suite.")
```

6. In `_crate`, replace

```python
    live = tuple(_live(f"{name}: [package.metadata.polyoxide.live.{target}]", target, entry)
                 for target, entry in _live_table(where, meta).items())
```

with

```python
    live = tuple(_live(f"{name}: [package.metadata.polyoxide.live.{target}]", target, entry)
                 for target, entry in _live_table(where, meta).items())
    _check_runners(where, live)
```

and add after `_live_table`:

```python
def _check_runners(where: str, live: tuple[Live, ...]) -> None:
    """A suite is one job, so its targets share a runner."""
    runners: dict[str, set[str | None]] = {}
    for entry in live:
        runners.setdefault(entry.suite, set()).add(entry.runner)
    for suite, found in runners.items():
        if len(found) > 1:
            raise RegistryError(f"{where} suite {suite!r} mixes runners "
                                f"({', '.join(sorted(map(str, found)))}); each suite is one job "
                                f"with one runner")
```

7. In `_live`, replace

```python
    return Live(target, _id(f"{where} suite", entry["suite"]), timeout,
                _strings(f"{where} features", entry["features"]), secrets, note)
```

with

```python
    runner = _string(f"{where} runner", entry.get("runner"), optional=True)
    if runner is not None and runner not in RUNNERS:
        raise RegistryError(f"{where} runner {runner!r} is not one of {', '.join(RUNNERS)}")
    if runner and secrets:
        raise RegistryError(f"{where} names a runner, whose job holds no secrets; a target that "
                            f"reads them is a native live test")
    return Live(target, _id(f"{where} suite", entry["suite"]), timeout,
                _strings(f"{where} features", entry["features"]), secrets, note, runner)
```

8. Before `nightly_live_jobs`, add the runner's job, and make `nightly_live_jobs` use it. Replace

```python
def nightly_live_jobs(r: Registry) -> list[str]:
    lines = []
    for job in r.jobs():
        if lines:
            lines.append("")
        lines += [
```

with

```python
def _runner_job(job: Job) -> list[str]:
    """A job for a target the live-suite action cannot run: the runner's script writes
    libtest-json, and the classifier's own two commands merge it into the artifact the aggregate
    reads. The script's retry pass stands in for nextest's, so the merge's retry file is the
    script's."""
    script = RUNNERS[job.runner][1]
    out = f"artifacts/{job.crate}"
    return [
        f"{job.id}:",
        f"  name: Live browser tests ({job.crate}, {job.suite})",
        "  runs-on: ubuntu-latest",
        f"  timeout-minutes: {job.timeout}",
        "  steps:",
        "    - uses: actions/checkout@v5",
        "    - uses: dtolnay/rust-toolchain@stable",
        "      with:",
        "        targets: wasm32-unknown-unknown",
        "    - id: wasm-bindgen",
        "      name: Read the wasm-bindgen version Cargo.lock pins",
        "      run: |",
        "        version=$(awk '/^name = \"wasm-bindgen\"$/ { getline; "
        "gsub(/version = |\"/, \"\"); print; exit }' Cargo.lock)",
        "        echo \"version=$version\" >> \"$GITHUB_OUTPUT\"",
        "    - uses: taiki-e/install-action@v2",
        "      with:",
        "        tool: wasm-bindgen@${{ steps.wasm-bindgen.outputs.version }}",
        "    - name: Run the browser tests",
        f"      run: bash {script}",
        "    - name: Classify and merge",
        "      run: |",
        f"        mkdir -p {out}",
        f"        cp target/wasm-live/first-pass.json {out}/first-pass.json",
        f"        cp target/wasm-live/retry.json {out}/retry.json",
        "        python3 .github/scripts/classify_failures.py classify \\",
        f"          --input {out}/first-pass.json \\",
        f"          --output-dir {out}/classified",
        "        python3 .github/scripts/classify_failures.py merge \\",
        f"          --first-pass {out}/first-pass.json \\",
        f"          --retry {out}/retry.json \\",
        f"          --output-dir {out}/merged",
        "    - name: Upload artifacts",
        "      if: always()",
        "      uses: actions/upload-artifact@v6",
        "      with:",
        f"        name: behavioral-{job.crate}-{job.suite}",
        f"        path: {out}/",
    ]


def nightly_live_jobs(r: Registry) -> list[str]:
    lines = []
    for job in r.jobs():
        if lines:
            lines.append("")
        if job.runner:
            lines += _runner_job(job)
            continue
        lines += [
```

In `polyoxide-wasm-smoke/Cargo.toml`, register the live target. Replace

```toml
notes = ["native server, wasm32 tests; nothing depends on it"]
```

with

```toml
notes = ["native server, wasm32 tests; nothing depends on it"]

# The nightly's browser job, `live-wasm-browser`. Its target is a wasm32 test the runner's script
# runs, not a nextest `live_*` binary, so it names its `runner`.
[package.metadata.polyoxide.live.browser_live]
suite = "browser"
timeout = 15
features = ["live"]
secrets = []
runner = "wasm-bindgen"
note = "wasm-bindgen-test in headless Chrome through scripts/wasm_live.sh, no secrets; 15-minute budget"
```

Then regenerate:

Run: `nix develop --command python3 scripts/gen_registry.py --write`

Expected: it writes `.github/workflows/nightly-behavioral.yml`, `CLAUDE.md` and `SELF-HEALING.md`. The workflow gains, after `live-polyoxide-cli-live`, the job below, and `aggregate`'s `needs` gains `- live-wasm-browser`:

```yaml
  live-wasm-browser:
    name: Live browser tests (polyoxide-wasm-smoke, browser)
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v5
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - id: wasm-bindgen
        name: Read the wasm-bindgen version Cargo.lock pins
        run: |
          version=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' Cargo.lock)
          echo "version=$version" >> "$GITHUB_OUTPUT"
      - uses: taiki-e/install-action@v2
        with:
          tool: wasm-bindgen@${{ steps.wasm-bindgen.outputs.version }}
      - name: Run the browser tests
        run: bash scripts/wasm_live.sh
      - name: Classify and merge
        run: |
          mkdir -p artifacts/polyoxide-wasm-smoke
          cp target/wasm-live/first-pass.json artifacts/polyoxide-wasm-smoke/first-pass.json
          cp target/wasm-live/retry.json artifacts/polyoxide-wasm-smoke/retry.json
          python3 .github/scripts/classify_failures.py classify \
            --input artifacts/polyoxide-wasm-smoke/first-pass.json \
            --output-dir artifacts/polyoxide-wasm-smoke/classified
          python3 .github/scripts/classify_failures.py merge \
            --first-pass artifacts/polyoxide-wasm-smoke/first-pass.json \
            --retry artifacts/polyoxide-wasm-smoke/retry.json \
            --output-dir artifacts/polyoxide-wasm-smoke/merged
      - name: Upload artifacts
        if: always()
        uses: actions/upload-artifact@v6
        with:
          name: behavioral-polyoxide-wasm-smoke-browser
          path: artifacts/polyoxide-wasm-smoke/
```

CLAUDE.md's nightly list gains ``- `live-wasm-browser` (15 min): `browser_live` with `--features live` ``, and SELF-HEALING.md's table gains the row for `polyoxide-wasm-smoke`.

- [ ] **Step 4: Run it to see it pass**

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_gen_registry.py tests/test_live_registry.py -q'`

Expected: every test passes, the new ones among them.

Run: `nix develop --command python3 scripts/gen_registry.py --check && nix develop --command python3 scripts/live_unwraps.py`

Expected: no output from either.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(ci): the generator's runner key and the live-wasm-browser nightly job (Story 10)" -- scripts/gen_registry.py .github/scripts/tests/test_gen_registry.py .github/scripts/tests/test_live_registry.py polyoxide-wasm-smoke/Cargo.toml .github/workflows/nightly-behavioral.yml CLAUDE.md SELF-HEALING.md
```

### Task 10.5: The browser facts and the rules text

**Files:**
- Modify: `docs/specs/clob/OBSERVED.md`
- Modify: `docs/specs/gamma/OBSERVED.md`
- Modify: `docs/specs/data-v2/OBSERVED.md`
- Modify: `docs/specs/perps/OBSERVED.md`
- Modify: `docs/specs/binance/OBSERVED.md`
- Modify: `docs/specs/kalshi/OBSERVED.md`
- Modify: `docs/specs/sports/OBSERVED.md`
- Modify: `docs/specs/rtds/OBSERVED.md`
- Modify: `.claude/rules/live-tests.md`
- Modify: `CLAUDE.md` (the live suites bullet)
- Modify: `SELF-HEALING.md` (the behavioral section's prose)

- [ ] **Step 1: Write the failing test**

```bash
for host in clob gamma data-v2 perps binance kalshi sports rtds; do
  file=docs/specs/$host/OBSERVED.md
  [ "$(grep -c '^## Browser transport (2026-11)$' "$file")" = 1 ] || { echo "$file: not one heading"; continue; }
  grep -q 'browser_live' "$file" || echo "$file: no browser_live fact"
done
grep -q 'live-wasm-browser' .claude/rules/live-tests.md || echo "live-tests.md: no browser job"
grep -q 'live-wasm-browser' CLAUDE.md && grep -q 'scripts/wasm_live.sh' CLAUDE.md || echo "CLAUDE.md: no browser exception"
grep -q 'live-wasm-browser' SELF-HEALING.md || echo "SELF-HEALING.md: no browser job"
```

- [ ] **Step 2: Run it to see it fail**

Run the loop from Step 1.

Expected: `docs/specs/clob/OBSERVED.md: not one heading` for each of clob, gamma, data-v2, perps, kalshi and rtds, `docs/specs/binance/OBSERVED.md: no browser_live fact` and the same for sports (their heading is Story 8's), and the three last lines.

- [ ] **Step 3: Implement**

Each file's section is headed `## Browser transport (2026-11)`. The `binance` and `sports` files already have that heading, which Story 8 wrote as the last section (confirm with `grep -n '^## ' <file> | tail -1`): append their block to the end of the file, under it, without a second heading. The other six files get the heading with their block, as written below.

`docs/specs/clob/OBSERVED.md`:

```markdown

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `clob.polymarket.com` answers `Access-Control-Allow-Origin: *`, and the preflight for
  `POST /order` with the `POLY_*` headers is allowed (`Access-Control-Max-Age` 30 days).
- It sets no `Access-Control-Expose-Headers`, so a browser cannot read `Retry-After` from it:
  `ErrorResponse::retry_after` is `None` there, though the server sent one.
- `ws-subscriptions-clob.polymarket.com` upgrades (101) on `/ws/market` and `/ws/user`. A browser
  sends no protocol pings and shows none, so the text `PING` is the only keepalive it has; the
  nightly job sends it on the market path and holds the socket until the first data frame or
  20 s.
```

`docs/specs/gamma/OBSERVED.md`:

```markdown

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `gamma-api.polymarket.com` answers `Access-Control-Allow-Origin: *` and sets no
  `Access-Control-Expose-Headers`, so a browser cannot read `Retry-After` from it.
```

`docs/specs/data-v2/OBSERVED.md`:

```markdown

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `data-api.polymarket.com` answers `Access-Control-Allow-Origin: *` and sets no
  `Access-Control-Expose-Headers`, so a browser cannot read `Retry-After` from it.
```

`docs/specs/perps/OBSERVED.md`:

```markdown

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `api.perpetuals.polymarket.com` answers `Access-Control-Allow-Origin: *`, and is the one host
  probed that sets `Access-Control-Expose-Headers`: `Retry-After, x-pmp-pod`. A browser reads both;
  the nightly job reads `x-pmp-pod` from `GET /v1/info/ping`.
- Its `Access-Control-Allow-Headers` is `content-type, authorization, polymarket-proxy,
  polymarket-secret, traceparent, tracestate, x-gc-*`, measured on `GET /v1/info/ping`: a browser
  request with any other header fails its preflight, so the perps header auth cannot be sent from
  a page whose origin the host does not allow it to.
- `ws.perpetuals.polymarket.com/v1/ws` upgrades (101). The browser sends no protocol pings, so
  the JSON `post/ping` is its keepalive; the nightly job sends it and holds the socket until the
  first data frame or 20 s.
```

`docs/specs/binance/OBSERVED.md`:

```markdown

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `fapi.binance.com` answers `Access-Control-Allow-Origin: *` (with `Access-Control-Allow-Headers:
  *` and `Access-Control-Allow-Credentials: true`) and sets no `Access-Control-Expose-Headers`: the
  response carries `X-MBX-USED-WEIGHT-1M`, and a browser does not show it. The weight header is
  unreadable in a browser, so the client's weight accounting has nothing to read there.
- `fstream.binance.com` upgrades (101) on `/market/stream`. A browser sends no protocol ping, so
  the supervised protocol's keepalive is the `LIST_SUBSCRIPTIONS` request: the nightly job
  subscribes to `btcusdt@markPrice@1s`, sends it 250 ms later and expects an answer with its id
  within 5 s whose `result` names the stream.
```

`docs/specs/kalshi/OBSERVED.md`:

```markdown

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02. Kalshi stays native-only (the
nightly browser job does not open it), and these are the reasons it cannot go in a page.

- Production REST answers 403 to that origin and 200 to `Origin: https://kalshi.com`, so a page
  on any other origin is refused.
- Its socket answers 401 to both origins, since it requires signed handshake headers, which the
  browser transport refuses to send (`a handshake header is not supported by the browser
  transport`).
```

`docs/specs/sports/OBSERVED.md`:

```markdown

Probed with `Origin: https://prader.example` on 2026-11-02, and checked nightly by
`polyoxide-wasm-smoke`'s `browser_live` (the `live-wasm-browser` job).

- `sports-api.polymarket.com/ws` upgrades (101). The server's protocol pings are not visible to a
  page, so the browser's only signal of a live connection is a data frame or a close event; the
  nightly job sends nothing, holds the socket until its first data frame or 20 s, and counts a
  close or an error before then as a drop (`transient`).
```

`docs/specs/rtds/OBSERVED.md`:

```markdown

## Browser transport (2026-11)

Probed with `Origin: https://prader.example` on 2026-11-02. `rtds` is native-only (spec D1), so
the nightly browser job does not open it.

- `ws-live-data.polymarket.com` upgrades (101) from a browser origin.
```

In `.claude/rules/live-tests.md`, add two globs to the front matter. Replace

```
  - "scripts/live_unwraps*"
  - ".env.example"
---
```

with

```
  - "scripts/live_unwraps*"
  - "scripts/wasm_live.sh"
  - "polyoxide-wasm-smoke/tests/browser_live.rs"
  - ".env.example"
---
```

and insert this section before `## Reading the environment`:

```
## The browser job

`live-wasm-browser` is the one nightly job that is not a nextest run. Its target, `polyoxide-wasm-smoke`'s
`tests/browser_live.rs`, is a wasm32 test behind the `live` feature (not `#[ignore]`, which nextest
reads), run by `scripts/wasm_live.sh` in headless Chrome inside a Worker; its live entry names
`runner = "wasm-bindgen"`, which `gen_registry.py` turns into this job in place of a live-suite
call (a runner job holds no secrets, and no suite mixes runners). The file is not a `live_*`
target, so `live_unwraps.py` and `test_live_registry.py`'s per-target checks do not read it; its
own rule is the same: a failure goes through the tags or is untagged on purpose.

- The checks are spec D17's: per host, one GET bounded by 10 s and one socket held until its first
  data frame or 20 s, each retried once after 5 s. A second failure is `real`, except a socket that
  opened and then dropped, which is `transient`, and a host the runner cannot test (a 451 from
  Binance's REST host, which a failed upgrade also shows as) is `environmental`.
- The tags build on wasm32 and print their line through `console.error`, which wasm-bindgen-test
  captures into the failing test's output. The script turns that output into libtest-json (the
  block dedented, its `panicked at` line given the `thread '...'` shape the classifier looks for),
  runs the classifier's own `classify` on it, and does one more run of the transient ones. Before
  all that it runs `probe_tag_line`, a test built to fail with a `transient` tag, and exits 1 unless
  the classifier lists it: a job that cannot read its tags would file every failure as `real`.
- The runner's output format is read, not assumed: if a wasm-bindgen release changes how a test's
  result or its output block is printed, the probe is what fails, in the script and so in the job.
```

In `CLAUDE.md`, replace the end of the live suites bullet

```
and counts as opted out.
```

with

```
and counts as opted out. The browser job (`live-wasm-browser`) is the exception: its target, `polyoxide-wasm-smoke`'s `browser_live`, is a wasm32 test behind the `live` feature that `scripts/wasm_live.sh` runs, tagged through the same crate (`live-tests.md`).
```

In `SELF-HEALING.md`, replace

```
and that is when North American leagues and weekend soccer are on.
```

with

```
and that is when North American leagues and weekend soccer are on.

One job is not a nextest run: `live-wasm-browser` runs `polyoxide-wasm-smoke`'s `browser_live`
in headless Chrome through `scripts/wasm_live.sh`, which hands the classifier the same
`polyoxide-class=<tag>` lines in libtest-json, and its artifact is read by the aggregate like the
others. It needs no secrets.
```

- [ ] **Step 4: Run it to see it pass**

Run the loop from Step 1.

Expected: no output.

Run the story's gates: `nix develop --command cargo fmt --all -- --check`, `nix develop --command cargo clippy -p polyoxide-test-support -p polyoxide-wasm-smoke --all-targets --all-features -- -D warnings`, `nix develop --command cargo test -p polyoxide-test-support -p polyoxide-wasm-smoke --all-features`, `nix develop --command bash -c 'RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p polyoxide-test-support -p polyoxide-wasm-smoke --all-features'`, `nix develop --command python3 scripts/gen_registry.py --check`, `nix develop --command python3 scripts/venue_gate.py`, `nix develop --command python3 scripts/live_unwraps.py`, `nix develop --command python3 scripts/test_body_diff.py --baseline v0.40.0`, `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/ -q'`, and the story's Verify line under `nix develop --command`.

Expected: every command exits 0. `venue_gate.py` reads `polyoxide-test-support`, so its new docs name no venue.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(wasm): the browser facts per host, the browser job's rules and its place in the nightly docs (Story 10)" -- docs/specs/clob/OBSERVED.md docs/specs/gamma/OBSERVED.md docs/specs/data-v2/OBSERVED.md docs/specs/perps/OBSERVED.md docs/specs/binance/OBSERVED.md docs/specs/kalshi/OBSERVED.md docs/specs/sports/OBSERVED.md docs/specs/rtds/OBSERVED.md .claude/rules/live-tests.md CLAUDE.md SELF-HEALING.md
```

## Story 11: The README, the remaining rules text and the spine amendment

- **Developer:** intermediate-dev
- **Files:** `README.md`, `CLAUDE.md`, `.claude/rules/rt.md`, `.claude/rules/sockets.md`, `.claude/rules/send-loop.md`, `.claude/rules/clients.md`, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md`
- **Interfaces:** exposes nothing; uses every earlier story's names as built
- **Verify:** `cargo test --doc --workspace --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
- **Review:** light (docs and an amendment for the owner to merge; its one landmine marker, `From<reqwest::Error>`, is README prose stating the rule, not code)

This story writes down what the earlier stories built, after they have: README.md's WebAssembly section (spec D1's set and contexts, the getrandom flag, injection and the defaults by target, and every browser gap D15 lists), the rules text and CLAUDE.md sentences that earlier stories left stating the old rule, and the spine amendment beside the spine. Earlier stories change a rules file in the commit that changes its rule (AD-21), so the rules tasks here are audits: each one greps for the old sentence and for the new one, and edits only what the grep finds, with the complete text given. A sentence an earlier story already fixed is left alone. The story edits neither the spine nor `docs/ARCHITECTURE.md`, never edits a generated region (the workspace list, the `polyoxide-ws` dependency sentence and the jobs are generated from the crates' metadata; a stale one is reported, not edited), and writes no code fence into README.md, which is a doctest. Its one landmine marker, `From<reqwest::Error>`, is README prose stating the rule, not code, so the developer needs no `effort: xhigh`. Task 11.1 reads Story 8's two `OBSERVED.md` sections for the two keepalive outcomes the README states.

### Task 11.1: The README's WebAssembly section

**Files:**
- Modify: `README.md` (insert before the `## License` heading)

- [ ] **Step 1: Write the check**

```bash
grep -n '^## WebAssembly$' README.md
sed -n '/^## Browser transport (2026-11)/,/^## /p' docs/specs/binance/OBSERVED.md docs/specs/sports/OBSERVED.md
```

The first command is the story's own check: the section exists once. The second prints Story 8's recorded outcomes for Binance's `LIST_SUBSCRIPTIONS` keepalive and for the sports feed, which decide two sentences below.

- [ ] **Step 2: Run it to see it fail**

Run the first command. Expected: no output and exit status 1.

Read the second command's output and note: (a) whether `fstream` answered `LIST_SUBSCRIPTIONS`; (b) for the sports feed, which of three outcomes was recorded: its longest gap between data frames was under the 45 s `stale_after` (nothing changed), a text keepalive was answered and became its liveness, or its staleness check is off on a transport that cannot see server pings.

- [ ] **Step 3: Implement**

In `README.md`, the text to anchor on is the end of the Gasless Redemptions section and the License heading:

```
## License

Licensed under either of [MIT](./LICENSE-MIT) or [Apache-2.0](./LICENSE-APACHE) at your option.
```

Insert the section below immediately before `## License`, with a blank line between it and the heading. It is prose: no code fence, no indented line, each paragraph and bullet on one line, because this file is compiled as a doctest. Drop or keep each conditional sentence as the outcomes of Step 2 say (the rule is under the section).

```
## WebAssembly

The crates and features below build for `wasm32-unknown-unknown` and run in a browser: on the main thread, in a dedicated Worker or in a SharedWorker, without atomics. `polyoxide-rt`; `polyoxide-venue` with `market` and `trading`; `polyoxide-core` without `keychain`; `polyoxide-ws`; `polyoxide-binance` with `usdm` and `usdm-ws`; `polyoxide-polymarket` with `clob`, `gamma`, `data`, `perps`, `perps-ws`, `sports` and `clob-ws`. Everything else is native only: `polyoxide-kalshi`, Polymarket's `relay` and `rtds` modules, the `polyoxide` umbrella, `polyoxide-cli`, `polyoxide-py` and every `keychain` feature.

Run the client in a Worker. A hidden tab throttles the timers of its main thread (in Chrome to once a second, and to once a minute after five minutes), which breaks ping cadences. A threaded build (with atomics) is unsupported, because the browser implementations keep their JavaScript handles in `SendWrapper`, which panics off the thread that made it.

The final binary must be built with `RUSTFLAGS` set to `--cfg getrandom_backend="wasm_js"`. polyoxide turns on getrandom 0.3's `wasm_js` feature, which compiles only with that flag.

Every timer, clock read and spawn goes through an injected `Runtime` (`polyoxide_rt`), every HTTP exchange through an injected `HttpTransport` (`polyoxide_core`) and every socket dial through an injected `SocketTransport` (`polyoxide_ws`). Each is injected on a builder, never process-wide: HTTP builders take `runtime(..)` and `http_transport(..)`, and socket builders take `runtime(..)` and `socket_transport(..)`. A builder with nothing injected uses its target's default. Natively that is `TokioRuntime`, `ReqwestTransport` and `TungsteniteTransport` (the CLOB's builders default to `TungsteniteTransport::per_address()`). On `wasm32-unknown-unknown` it is `BrowserRuntime`, `FetchTransport` and `WebSocketTransport`, so a consumer injects nothing unless it wants its own.

What a browser cannot do, and what polyoxide does about it:

- `Retry-After` is unreadable on the CLOB, Gamma and Data hosts, which do not expose it, so the send loop's floor applies in its place. Perps exposes it.
- Binance's used weight is unreadable, so `WeightBudget` runs on local charges and tabs that share an IP cannot see each other. Run one SharedWorker for them, or set a lower `weight_budget`.
- Redirects are followed, as fetch does; the native transport follows none.
- A fetch that fails is a `Connect` transport error whether the network failed or CORS refused it, since a browser does not tell them apart.
- There are no handshake headers and no control frames. A handshake header beyond those tungstenite generates, and a ping, pong or raw frame, fail with the socket error kind `Unsupported`, which is never retried.
- A refused WebSocket upgrade shows no status code, so it is seen as a lost connection and not as a refusal.
- Each new authenticated path costs one CORS preflight.
- Binance's supervised streams keep alive with a `LIST_SUBSCRIPTIONS` request, which the host answers, since a browser cannot send ping frames; the bare client's `ping()` fails with `Unsupported`.
- On Binance's streams a quiet connection is seen only through data frames, so a stream with no traffic reconnects when its `stale_after` passes: the host leaves the keepalive request unanswered.
- The sports feed, whose server pings a browser cannot see, keeps alive with a text keepalive that the host answers.
- The sports feed has no staleness check on this transport, so a lost connection ends through the socket's close event.
```

Which conditional bullets stay: of the two Binance bullets starting "Binance's supervised streams" and "On Binance's streams", keep the first when Step 2 (a) says `fstream` answered and the second when it did not, and delete the other. Of the two sports bullets, keep "The sports feed, whose server pings" when a text keepalive was answered, keep "The sports feed has no staleness check" when the check is off, and delete both when the longest gap was under 45 s.

- [ ] **Step 4: Run it to see it pass**

Run: `grep -c '^## WebAssembly$' README.md`
Expected: `1`.

Run: `nix develop --command cargo test --doc -p polyoxide --all-features`
Expected: `test result: ok` (the README is a doctest of the umbrella crate; a prose-only section adds none).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(readme): add the WebAssembly section (story 11)" -- README.md
```

### Task 11.2: CLAUDE.md states the runtime, transport and gate rules

**Files:**
- Modify: `CLAUDE.md` (the rules table, "Rules the gates hold", "Patterns every crate follows")

- [ ] **Step 1: Write the check**

```bash
missing() { grep -qF -- "$2" "$1" || echo "$1 lacks: $2"; }
stale() { if grep -qF -- "$2" "$1"; then echo "$1 still says: $2"; fi; }
stale CLAUDE.md 'name its types through `polyoxide_core::reqwest`'
missing CLAUDE.md 'Runtime and transports'
missing CLAUDE.md 'runtime_gate.py'
missing CLAUDE.md '| `rt.md` |'
stale CLAUDE.md 'the `polyoxide-ws` kit, its `Supervisor`, TLS, the scripted test server |'
stale CLAUDE.md 'retry policies, reqwest, gzip, the header pins |'
```

- [ ] **Step 2: Run it to see it fail**

Run the block in bash. Expected: one line for each statement an earlier story left stated the old way or unstated. At the story's start, before any earlier story's rules text, all six lines print; a line that is absent names a fix the earlier story already made, and its edit below is skipped.

- [ ] **Step 3: Implement**

For each line Step 2 printed, make the matching edit in `CLAUDE.md`.

`stale ... 'name its types through'`: in the "Dependency fences" bullet, replace

```
Only `polyoxide-core` declares `reqwest` (other members name its types through `polyoxide_core::reqwest`); only
```

with

```
Only `polyoxide-core` declares `reqwest` (other members name `Method`, `HeaderMap`, `StatusCode` and the rest through `polyoxide_core::http`; `polyoxide_core::reqwest` stays on native targets for `ReqwestTransport::new` and the tests that open raw clients); only
```

`missing 'Runtime and transports'`: after the **Sockets** bullet in "Patterns every crate follows" (the one ending "is not the shape to copy."), add:

```
- **Runtime and transports.** Every timer, monotonic clock read and spawn in `polyoxide-venue`, `-core`, `-ws`, `-polymarket`, `-binance` and `-kalshi` goes through an injected `polyoxide_rt::Runtime`, every HTTP exchange through an `HttpTransport` and every socket dial through a `SocketTransport`. Each is injected on the builder (`runtime(..)`, `http_transport(..)`, `socket_transport(..)`), never process-wide, and a builder with nothing injected takes its target's default: tokio, reqwest and tokio-tungstenite natively; `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` on `wasm32-unknown-unknown`. `Send` stays on every trait: the browser implementations contain their JavaScript handles in `SendWrapper`, so a threaded wasm build is unsupported. The wall clock is the one clock not injected. A wasm32 build sets `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'` (`scripts/wasm_check.sh` sets it itself); `polyoxide-rt` enables getrandom's `wasm_js` feature. `rt.md` has the rest.
```

`missing 'runtime_gate.py'`: after the **The foundation crates name no venue** bullet in "Rules the gates hold", add:

```
- **Time and tasks come from the runtime.** `scripts/runtime_gate.py` fails on `tokio::spawn`, `tokio::task::spawn`, any `tokio::time` path, `Instant::now(` and `SystemTime::now(` in the `src` of `polyoxide-venue`, `-core`, `-ws`, `-polymarket`, `-binance` and `-kalshi`, outside `cfg(test)` items, `polyoxide-ws/src/test_server/` and each `scripted.rs`. A line ending `// runtime-gate: <reason>` is exempt; the two in tree are the wall-clock reads (`ci-gates.md`).
```

`missing '| `rt.md` |'`: in the rules table, after the `sockets.md` row, add:

```
| `rt.md` | `polyoxide-rt`: the `Runtime`, tasks, timeouts and intervals, the browser runtime, the wall clock |
```

`stale ... 'the `polyoxide-ws` kit, its `Supervisor`, TLS'`: replace the `sockets.md` row's second cell with `the `polyoxide-ws` kit, its `Supervisor`, its transports, TLS, the scripted test server`.

`stale ... 'retry policies, reqwest, gzip'`: replace the `send-loop.md` row's second cell with `core's send loop, hooks, pings, retry policies, the HTTP transport and `Response`, reqwest, gzip, the header pins`.

Then check the generated text agrees: `grep -n 'builds only tokio' CLAUDE.md polyoxide-ws/Cargo.toml`. It must print nothing. A line it prints is the `polyoxide-ws` dependency sentence, which Story 5 owns (it comes from the crate's metadata `notes` and `polyoxide-ws` now depends on `polyoxide-rt`): do not edit it here, and put the file and line in the report to the supervisor.

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 block again. Expected: no output.

Run: `python3 scripts/gen_registry.py --check`
Expected: exit 0 (no generated region was touched).

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
Expected: all pass, the table listing every rules file once.

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(claude): state the runtime, transport and gate rules (story 11)" -- CLAUDE.md
```

### Task 11.3: The rules files state the transports and the browser

**Files:**
- Modify: `.claude/rules/rt.md`
- Modify: `.claude/rules/sockets.md:14-16,31-33,73-76,87-88` (the line numbers are today's, so find each by its text)
- Modify: `.claude/rules/send-loop.md` (the reqwest bullet, and a new section before `## Pings`)
- Modify: `.claude/rules/clients.md` (the builders bullet)

- [ ] **Step 1: Write the check**

```bash
missing() { grep -qF -- "$2" "$1" || echo "$1 lacks: $2"; }
stale() { if grep -qF -- "$2" "$1"; then echo "$1 still says: $2"; fi; }
missing .claude/rules/rt.md 'SendWrapper'
missing .claude/rules/rt.md 'getrandom_backend'
stale .claude/rules/sockets.md 'builds only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls'
missing .claude/rules/sockets.md '## Transports'
stale .claude/rules/sockets.md 'a venue that tries each address overrides it'
stale .claude/rules/sockets.md 'upgraded through `connect_over`'
stale .claude/rules/sockets.md 'which its `connect` and `connect_over` call'
stale .claude/rules/send-loop.md 'through which every other member'
missing .claude/rules/send-loop.md '## The transport'
stale .claude/rules/clients.md 'its five transport setters'
missing .claude/rules/clients.md 'transport_setters!'
```

- [ ] **Step 2: Run it to see it fail**

Run the block in bash. Expected: one line per statement an earlier story left stale or unstated; an absent line is a fix already made, whose edit below is skipped.

- [ ] **Step 3: Implement**

For each line Step 2 printed, make the matching edit.

`.claude/rules/rt.md`, `missing 'SendWrapper'` or `missing 'getrandom_backend'`: append this section after the file's last line, with a blank line before it (skip a paragraph whose key word the file already holds):

```
## The browser

`BrowserRuntime` (wasm32 only) spawns with `wasm_bindgen_futures::spawn_local`, sleeps with gloo-timers' `TimeoutFuture` in steps of at most `i32::MAX` ms (`setTimeout` fires at once for a larger delay) and reads `now` from `web_time::Instant` (`performance.now()`).

Every JavaScript handle a browser implementation holds is contained in `send_wrapper::SendWrapper`, so `Send` stays on every trait. The wrapper panics off its own thread, so a threaded (atomics) wasm build is unsupported. The supported contexts are the main thread, a dedicated Worker and a SharedWorker. A Worker is the recommended one: Chrome throttles a hidden tab's main-thread timers to once a second, and to once a minute after five minutes, which breaks ping cadences.

`polyoxide-rt` enables `getrandom` 0.3's `wasm_js` feature, so feature unification reaches rand and tungstenite. The consumer's final binary also sets `--cfg getrandom_backend="wasm_js"`: the feature alone is a compile error.
```

`.claude/rules/sockets.md`, four edits.

`stale 'builds only tokio, ...'`: replace

```
`polyoxide-ws` is the socket kit. It depends on neither `polyoxide-core` nor `polyoxide-venue`,
builds only tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls, and re-exports
`tungstenite`. `polyoxide_venue::impl_ws_classification!` classifies its `WsError` in each venue
crate (`errors.md`).
```

with

```
`polyoxide-ws` is the socket kit. It depends on neither `polyoxide-core` nor `polyoxide-venue`;
its one workspace dependency is `polyoxide-rt`. It builds tokio-tungstenite, rustls and tokio's
`net` only on native targets, and `web-sys` and the other browser bindings only on wasm32; on
both it builds futures-util, thiserror, tracing and `tungstenite`'s message vocabulary, which it
re-exports. `polyoxide_venue::impl_ws_classification!` classifies its `WsError` in each venue
crate (`errors.md`).
```

`missing '## Transports'`: insert this section between the end of `## Blocks` (the paragraph ending "and the scripted test server.") and `## The `Supervisor` (`src/supervisor/`)`:

```
## Transports

A socket is dialled through a `SocketTransport` (`polyoxide-ws/src/transport.rs`):
`connect(request, timeout, runtime)` returns a `BoxSocket`, a boxed `Stream` and `Sink` of
tungstenite `Message`s, so the bare tier and the `Supervisor` run on any transport, and
`capabilities()` says what a protocol may rely on. A transport bounds its own steps with the
runtime it is given, and a timed-out connect is `Kind::ConnectTimeout`.

- `TungsteniteTransport` (native only) is the default: `connect_async` after
  `ensure_crypto_provider`, capabilities all `true`. `TungsteniteTransport::per_address()` is the
  CLOB's dial (D20) moved into the kit unchanged: resolve, interleave IPv6 and IPv4, and bound
  each address and each step by the timeout. An injected transport replaces the dial, D20
  included. `connect_over(request, TcpStream)` is native only.
- `WebSocketTransport` (wasm32 only) is `web_sys::WebSocket` with `binaryType` `arraybuffer`; its
  events feed an unbounded channel read as the `Stream`, because a browser socket cannot apply
  backpressure. A close event yields `Message::Close` with its code and reason, then ends the
  stream; an error event yields an I/O error. `Ping`, `Pong` and `Frame` are `Kind::Unsupported`,
  as is a handshake header beyond those tungstenite generates (a `Sec-WebSocket-Protocol` becomes
  the subprotocol list). A refused upgrade shows no status in a browser, so it arrives as a lost
  connection, not `Kind::Refused`. Its capabilities are all `false`.
- `Capabilities { client_pings, sees_server_pings, handshake_headers }`: a protocol adapts to
  what its transport lacks (`binance.md`, `polymarket-sports.md`).
- `Kind::Unsupported` is an operation the transport cannot perform, read from an `io::Error` of
  kind `Unsupported`; `retrying_can_fix` is `false` for it and `impl_ws_classification!` classes
  it `InvalidRequest`, not a fault of the venue's.
- A socket builder gets `runtime(..)` and `socket_transport(..)` from `transport_setters!`,
  beside `timing_setters!`, and falls back to `default_runtime()` and the target's default
  transport. `Supervisor::new(protocol, timing)` gains `.runtime(..)` and `.socket_transport(..)`;
  `Connection::new(socket, mode)` uses the default runtime and `Connection::with_runtime` is what
  venue code calls. Every timer in the kit comes from the injected `Runtime`.
```

`stale 'a venue that tries each address overrides it'`: in the `Supervisor` section replace

```
- a handshake built per attempt; `connect`, the dial of each attempt (by default it dials the
  handshake's request; a venue that tries each address overrides it); `configure` of each fresh
  connection, such as Binance's 200 ms request pacing;
```

with

```
- a handshake built per attempt; `connect(transport, runtime, timeout)`, the dial of each attempt
  (by default it dials the handshake's request through the transport); `configure` of each fresh
  connection, such as Binance's 200 ms request pacing;
```

`stale 'upgraded through `connect_over`'`: in "Which socket runs on what" replace

```
  - the CLOB's bare tier: TLS, `WsError`, the bare tier (`Mode::Data`, so a server close is
    answered, DRIFT R3) and the test server. Its per-address connect is its own (D20), each step
    bounded by `with_timeout` and upgraded through `connect_over`; it keeps its own `select!`,
    its 10 s text `PING`, its outbound queue and Binary decoded as text (D12).
```

with

```
  - the CLOB's bare tier: TLS, `WsError`, the bare tier (`Mode::Data`, so a server close is
    answered, DRIFT R3) and the test server. Its per-address dial is
    `TungsteniteTransport::per_address()` (D20, moved into the kit), which its builders default
    to; it keeps its own `select!`, its 10 s text `PING`, its outbound queue and Binary decoded
    as text (D12).
```

`stale 'which its `connect` and `connect_over` call'`: in "TLS needs a nudge" replace "(`polyoxide_ws::ensure_crypto_provider`, which its `connect` and `connect_over` call)" with "(`polyoxide_ws::ensure_crypto_provider`, which `TungsteniteTransport` calls before it dials)", keeping the line wrapped as the file's other lines are.

`.claude/rules/send-loop.md`, two edits.

`stale 'through which every other member'`: replace

```
- Core is the only member that declares reqwest (AD-18): it enables `json`, `rustls-tls` and
  `gzip` with no default features and re-exports the crate as `polyoxide_core::reqwest`, through
  which every other member's src, tests and examples name its types. No crate but core sends
  through `HttpClient`'s reqwest client. `test_dependency_fences.py` fails on any other member's
  direct dependency, so no crate can change reqwest's features for the rest.
```

with

```
- Core is the only member that declares reqwest (AD-18): it enables `json`, `rustls-tls` and
  `gzip` with no default features, and only its transports send through it (`ReqwestTransport`
  natively; `FetchTransport`, reqwest's browser client, on wasm32). `polyoxide_core::reqwest` is
  re-exported on native targets, for `ReqwestTransport::new` and the tests that open raw clients;
  every other member's src, tests and examples name `Method`, `HeaderMap`, `HeaderValue`,
  `StatusCode` and `header::*` through `polyoxide_core::http`, the same types reqwest re-exports.
  `test_dependency_fences.py` fails on any other member's direct dependency, so no crate can
  change reqwest's features for the rest.
```

`missing '## The transport'`: insert this section before `## Pings`:

```
## The transport

The loop sends through the client's `Arc<dyn HttpTransport>` (`polyoxide-core/src/transport.rs`):
`send(HttpRequest)` returns a `Response` or a `TransportError`. The loop builds the URL with its
query (form-urlencoded, as reqwest's `.query()` does, which the `query_setters_*` suites pin) and
always fills `HttpRequest::timeout`, the request's own else the client's; the timeout bounds the
whole exchange, body included, for every transport. Every timer the loop and its hooks use (the
retry sleep, holds, capacity buckets, `health`'s stopwatch) comes from the client's `Runtime`
(`HttpClient::runtime()`), never tokio's.

- `Response` is an owned head and a lazy body: `status()`, `headers()`, `url()`, and async
  `bytes()`, `text()` and `json()`, each returning `ApiError`. The loop decides on the head,
  releases the permit, then reads an error body (see One attempt).
- `TransportError` carries a `TransportKind` (`Connect`, `Timeout`, `Body`, `Request`, `Other`)
  and a boxed source; `ApiError::Transport` wraps it. `Request` is `InvalidRequest`, every other
  kind `Network`. `ReqwestTransport` maps a reqwest error through a private function (no
  `From<reqwest::Error>` exists): a timeout, a connect failure, a body failure, or an I/O error
  in the source chain is a failure in transit, so a body that broke off mid-read stays `Network`.
- With no `http_transport(..)` injected, `build` makes the target's default: `ReqwestTransport`
  from the config's timeout, pool size and `gzip` (unset), with a 10 s connect timeout and no
  redirects; or `FetchTransport` on wasm32, whose browser negotiates gzip and follows redirects.
  `pool_size` and `gzip` feed only the native default, and the header pins below run on it.
```

`.claude/rules/clients.md`, `stale 'its five transport setters'` or `missing 'transport_setters!'`: in the Builders section replace

```
  and Kalshi 4; clob 8) and gets its five transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config` and `max_concurrent`, from
  `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()`
```

with

```
  and Kalshi 4; clob 8) and gets its seven transport setters, `base_url`, `timeout_ms`,
  `pool_size`, `with_retry_config`, `max_concurrent`, `runtime` and `http_transport`, from
  `polyoxide_core::client_config_setters!`. Its `build` starts from `ClientConfig::http_builder()`
```

and, when the file lacks `transport_setters!`, after the bullet that ends "the config sets no `gzip`." add:

```
- Each socket builder (perps, Binance, the CLOB's market socket and `user_socket`, sports, rtds,
  Kalshi) gets `runtime(..)` and `socket_transport(..)` from `polyoxide_ws::transport_setters!`,
  beside `timing_setters!`; its `build` falls back to `polyoxide_rt::default_runtime()` and the
  target's default transport (the CLOB's: `TungsteniteTransport::per_address()`). Nothing is
  installed process-wide.
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 block again. Expected: no output.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
Expected: all pass (no glob changed, no file added).

- [ ] **Step 5: Commit**

```bash
git commit -m "docs(rules): state the transports, the runtime and the browser in the rules (story 11)" -- .claude/rules/rt.md .claude/rules/sockets.md .claude/rules/send-loop.md .claude/rules/clients.md
```

### Task 11.4: The spine amendment

**Files:**
- Create: `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md`

- [ ] **Step 1: Write the check**

```bash
A=_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md
test -f "$A" && grep -c '^## AW-' "$A"
```

- [ ] **Step 2: Run it to see it fail**

Run the block. Expected: no output and exit status 1 (the file does not exist).

- [ ] **Step 3: Implement**

Create the file with exactly this text. Its format is `epic-9.md`'s: a title, the AD-21 sentence, then one section per amendment with its proposer, current rule, proposed rule, why and follow-up. The amendments are numbered AW-1 to AW-5 because the work is not an epic.

```
# Proposed spine amendments: wasm32 through an injected runtime and transport

Recorded per AD-21: the work records each proposed amendment here, and one session later merges this file into the spine and regenerates the guide. The design is `docs/superpowers/specs/2026-11-02-wasm-runtime-transport-design.md`, and the release is 0.45.0, an owner session after the run (spec §1.2 and D16).

## AW-1: `polyoxide-rt` is a foundation crate, first in the publish order (amends AD-3's layer list, CAP-7's foundation crates and AD-25's publish order)

- **Proposed by:** the owner, 2026-11-02, during the wasm32 design (spec §1.3, D2 and D10).
- **Current rule:**
  - The foundation crates are `polyoxide-venue`, `polyoxide-core`, `polyoxide-ws` and `polyoxide-test-support`, which name no venue (`scripts/venue_gate.py`'s `FOUNDATION`).
  - AD-25's publish loop publishes `polyoxide-venue` first.
- **Proposed rule:**
  - A fifth foundation crate, `polyoxide-rt`, holds the `Runtime` trait, `Task`, `timeout` and `Interval`, with `TokioRuntime` natively and `BrowserRuntime` on wasm32. It depends on nothing in the workspace, names no venue (it joins the venue gate's list) and has no public error type, so the `Classify` sweep has nothing to cover there.
  - It does not depend on `polyoxide-venue`, which would reach `polyoxide-ws` through it, against AD-2.
  - It publishes first, and `publish_order.py` computes that from `cargo metadata`. Every crate that uses it declares it; core does not re-export it.
- **Why:**
  - **Three foundation crates need one clock.** The send loop's holds and sleeps, the supervisor's pings and the socket kit's timers all had tokio's clock; injecting a runtime needs a crate below all three that none of them has to depend on the others to reach.
  - **A browser has no tokio clock or executor.** Putting the trait in a crate of its own keeps the vocabulary layer (AD-3) free of timers and keeps `polyoxide-venue` dependency-free by default.
- **Follow-up in the code:** Story 1 creates the crate and adds it to `FOUNDATION`, the workspace and the registry; Story 6 adds `runtime_gate.py`, which holds the crates that use it to the runtime. The merge session edits AD-3's layer list, CAP-7's foundation list and AD-25's order.

## AW-2: time, HTTP and sockets are injected per builder, by three seams (amends AD-18, AD-11 and AD-9)

- **Proposed by:** the owner, 2026-11-02 (spec §1.3, D3, D4, D5 and D7).
- **Current rule:**
  - AD-18: only `polyoxide-core` depends on reqwest and only `polyoxide-ws` on tokio-tungstenite and rustls; builders leave `gzip` unset.
  - AD-11: a venue's `Protocol` dials with its own `connect`, which by default dials the handshake's request.
  - AD-9: holds, waits and sleeps run on tokio's clock.
- **Proposed rule:**
  - Every timer, monotonic clock read and spawn in the venue, core, ws, Polymarket, Binance and Kalshi sources goes through an injected `Runtime`; every HTTP exchange goes through an injected `HttpTransport`; every socket dial goes through an injected `SocketTransport`. Each is injected on the builder (`runtime`, `http_transport` and `socket_transport`, from `client_config_setters!` and `transport_setters!`), with the target's default when nothing is injected, and nothing is process-wide.
  - `HttpTransport` returns an owned `Response`, an owned head with a lazy body, or a `TransportError` with a `TransportKind`, which replaces `ApiError::Network`. `ReqwestTransport` (native) and `FetchTransport` (wasm32) are the defaults; `ReqwestTransport` keeps AD-18's rules, its `gzip` unset and its pinned header set.
  - `SocketTransport` returns a `BoxSocket` and declares `Capabilities`. `Protocol::connect` takes the transport, the runtime and the timeout. `TungsteniteTransport` (native) and `WebSocketTransport` (wasm32) are the defaults, and the CLOB's per-address dial is `TungsteniteTransport::per_address()`, moved into the kit unchanged. `Kind::Unsupported` is an operation a transport cannot perform, an `InvalidRequest` that is never retried.
  - The wall clock is not injected: `UnixMillis::now` and `shared::auth::current_timestamp` read `web_time`'s `SystemTime`, which is `std`'s on every target but wasm32.
  - `Send` stays on every trait.
- **Why:**
  - **The owner chose injection over target cfgs inside each crate and over separate web crates** (spec §1.3), so one code path serves both targets and a consumer can supply its own runtime or transport.
  - **A transport is where the targets differ.** Fetch exposes no response headers beyond CORS's, follows redirects and cannot send a handshake header or a control frame; those differences belong behind a trait with declared capabilities, not in the send loop or the supervisor.
  - **Native behaviour is unchanged.** Every existing suite runs on the native defaults, and the two new injection tests drive a recording transport and an in-memory socket transport (spec D12).
- **Follow-up in the code:** Stories 2 to 5 land the runtime, `polyoxide_core::http`, the HTTP transport and the socket transport, each with every caller, and record the removed and re-typed items in `docs/s3-removals.md` (spec D11); Stories 7 and 8 add the browser implementations and the keepalive adaptations. The merge session edits AD-18, AD-11 and AD-9.

## AW-3: the browser implementations contain their JavaScript handles in `SendWrapper` (new rule, beside AD-18)

- **Proposed by:** the owner, 2026-11-02 (spec D1 and D8).
- **Current rule:** no rule: every trait the clients use is `Send`, and nothing holds a `!Send` handle.
- **Proposed rule:**
  - `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` hold each `!Send` JavaScript handle in `send_wrapper::SendWrapper`, so `Send` stays on every trait and no consumer sees a `!Send` future.
  - The supported contexts are a browser's main thread, a dedicated Worker and a SharedWorker, without atomics. A threaded wasm build is unsupported, because the wrapper panics off the thread that made it. A Worker is the recommended context, since a hidden tab throttles main-thread timers.
  - Only `polyoxide-rt`, `polyoxide-core` and `polyoxide-ws` host browser code, each target-gated in its own manifest.
- **Why:**
  - **The alternative is `?Send` everywhere.** Dropping `Send` from the traits would break every native consumer that spawns a client on a multi-threaded runtime, and the wasm32 target runs on one thread.
  - **The panic is the contract.** A handle used off its thread is a bug in the build, and the wrapper makes it loud.
- **Follow-up in the code:** Story 7 writes the three implementations and the smoke suite of Story 9 runs them in a Worker. The merge session adds the rule beside AD-18.

## AW-4: dependencies are target-gated, and polyoxide-ws depends on polyoxide-rt (amends AD-2 and AD-3)

- **Proposed by:** the owner, 2026-11-02 (spec D10).
- **Current rule:**
  - AD-2: `polyoxide-ws` depends only on tokio, tokio-tungstenite, futures-util, thiserror, tracing, rustls (`ring`, `std`), and optionally serde and serde_json.
  - AD-3: `polyoxide-venue` depends only on rust_decimal, serde, thiserror, futures-core and dynosaur.
- **Proposed rule:**
  - A feature never turns on a target-specific dependency: each manifest declares them under `[target.'cfg(...)'.dependencies]`.
  - Native only: tokio `rt` and `time` (in `polyoxide-rt`), tokio `net`, tokio-tungstenite and rustls (in `polyoxide-ws`, with its `test-server` feature). wasm32 only: wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys, gloo-timers, send_wrapper, `getrandom` 0.3 with `wasm_js` (in `polyoxide-rt`, so feature unification reaches rand and tungstenite) and, for `polyoxide-venue`, `web-time`. Both: `tungstenite`, futures-util, tokio `sync` and `macros`, and `web-time` in `polyoxide-rt`.
  - `polyoxide-ws` also depends on `polyoxide-rt`, the one workspace crate it depends on. `polyoxide-polymarket`'s `clob-ws` stops enabling `tokio/net`, `polyoxide-binance`'s `usdm-ws` stops enabling `tokio/rt`, and core drops tokio `time`.
  - Core stays the only member that declares reqwest: `polyoxide_core::http` re-exports the `http` crate for the vocabulary types, and `pub use reqwest` stays on native only, for `ReqwestTransport::new` and the tests that open raw clients.
  - `ci/dep-allowlists/` stays pinned to `x86_64-unknown-linux-gnu` and is regenerated for the native changes; a wasm32 set is added for the supported crates' trees.
- **Why:**
  - **The browser build compiles only what the browser has.** `mio`, `rustls-native-certs`, `ring`'s build script and `getrandom` 0.2 stop the wasm32 build in their dependencies today (spec §1.4); gating them by target is what lets the supported crates build.
  - **A feature cannot name a target.** Cargo unifies features across targets, so a target-specific dependency behind a feature would leak into the other target's tree.
- **Follow-up in the code:** Story 7 gates the manifests and Story 9's `wasm` job checks the supported set. The merge session edits AD-2's and AD-3's lists.

## AW-5: two CI gates, `runtime_gate.py` and the `wasm` job (amends AD-22)

- **Proposed by:** the owner, 2026-11-02 (spec D3 and D13).
- **Current rule:** AD-22 lists the CI jobs: `cargo hack`, the MSRV check, the publish dry run, the removal gate and the AD-2, AD-13 and AD-18 checks.
- **Proposed rule:**
  - `scripts/runtime_gate.py` runs in the scripts job through its pytest. It fails on `tokio::spawn`, `tokio::task::spawn`, any `tokio::time` path, `Instant::now(` and `SystemTime::now(` in the production source of the six crates that use the runtime, with the opt-out `// runtime-gate: <reason>`.
  - A `wasm` job (named WebAssembly) runs `scripts/wasm_check.sh`, a `cargo check` and a `cargo clippy -- -D warnings` of the supported set for `wasm32-unknown-unknown`, then `scripts/wasm_smoke.sh`, a browser smoke suite in headless Chrome. A nightly job, `live-wasm-browser`, runs a live suite from a browser against each host in scope.
  - Both withhold the release when red, as the other jobs do.
- **Why:**
  - **A lint cannot hold the runtime line.** Clippy's `disallowed-methods` would also fire in every test and example, which use tokio legitimately.
  - **The target must stay buildable.** A change that compiles natively and breaks the browser build is found by the per-push job, and a host that changes its browser rules (CORS, exposed headers, the socket upgrade) by the nightly one.
- **Follow-up in the code:** Story 6 adds the gate, Story 9 the `wasm` job and Story 10 the nightly job. The merge session edits AD-22.
```

- [ ] **Step 4: Run it to see it pass**

Run the Step 1 block. Expected: `5`.

Run: `grep -c '<!--' _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md`
Expected: `0` (the amendment holds no generated-region marker).

Run: `nix develop --command cargo test --doc --workspace --all-features`
Expected: `test result: ok` for every crate.

Run: `python3 scripts/gen_registry.py --check`
Expected: exit 0.

Run: `nix develop --command bash -c 'cd .github/scripts && uv run pytest tests/test_claude_rules.py -q'`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add -N _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md
git commit -m "docs(spine): propose the wasm32 runtime and transport amendments (story 11)" -- _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/wasm-runtime-transport.md
```
