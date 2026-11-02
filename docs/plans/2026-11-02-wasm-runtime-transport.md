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

**Story 2 → Stories 4, 5, 7.**

- `polyoxide_core::ClientConfig` gains `pub runtime: Option<Arc<dyn polyoxide_rt::Runtime>>` (default `None`); `client_config_setters!` gains `pub fn runtime(mut self, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Self`, writing `self.$config.runtime`.
- `HttpClientBuilder::runtime(mut self, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Self`; with none set, `build` uses `polyoxide_rt::default_runtime()`.
- `HttpClient::runtime(&self) -> &Arc<dyn polyoxide_rt::Runtime>`, shared with `with_base_url` siblings.

**Story 3 → Stories 4 to 11.**

- `pub use http;` in `polyoxide-core/src/lib.rs`, so `polyoxide_core::http::{Method, HeaderMap, HeaderValue, StatusCode, header}`.
- DRIFT row `R22` in `_bmad-output/specs/spec-venue-extensibility/divergences.md`, under the DRIFT table: "The wasm32 seams (spec `2026-11-02-wasm-runtime-transport-design.md`) change the paths and types tests name, never what they assert." Story 3's heading line for the table gains "R22 2026-11-02".

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
- `#[cfg(not(target_arch = "wasm32"))] pub struct ReqwestTransport`: `pub fn new(client: reqwest::Client) -> Self`, `impl HttpTransport`.
- `ClientConfig` gains `pub http_transport: Option<Arc<dyn HttpTransport>>`; `client_config_setters!` gains `pub fn http_transport(mut self, transport: Arc<dyn HttpTransport>) -> Self`; `HttpClientBuilder::http_transport(..)` likewise.
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
- `Kind::Unsupported` (unit variant), read from `tungstenite::Error::Io` whose kind is `std::io::ErrorKind::Unsupported`; `retrying_can_fix` → `false`; `polyoxide_venue::impl_ws_classification!` → `InvalidRequest`, not a fault.
- Re-typed: `connect<R: IntoClientRequest + Unpin>(request: R, timeout: Duration) -> Result<BoxSocket, WsError>`, `connect_over(..) -> Result<BoxSocket, WsError>` (native only), `Connection::new(socket: BoxSocket, mode: Mode)`; `Socket` removed.
- `Connection::with_runtime(socket: BoxSocket, mode: Mode, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Connection`.
- `Supervisor::runtime(self, runtime: Arc<dyn polyoxide_rt::Runtime>) -> Self`, `Supervisor::socket_transport(self, transport: Arc<dyn SocketTransport>) -> Self`.
- `Protocol::connect(&mut self, transport: Arc<dyn SocketTransport>, runtime: Arc<dyn polyoxide_rt::Runtime>, timeout: Duration) -> impl Future<Output = Result<BoxSocket, Self::Error>> + Send`.
- `polyoxide_ws::transport_setters! { runtime; socket_transport; }`, invoked in a builder's `impl` as `timing_setters!` is, writing `self.runtime: Option<Arc<dyn Runtime>>` and `self.socket_transport: Option<Arc<dyn SocketTransport>>`; each socket builder's `build` falls back to `default_runtime()` and its default transport (the CLOB's: `TungsteniteTransport::per_address()`).

**Story 6 → Stories 9, 11.** `scripts/runtime_gate.py`, no arguments, exit 0 clean and 1 on a hit, each hit printed as `<path>:<line>: <banned path>`; the opt-out comment is `// runtime-gate: <reason>`.

**Story 7 → Stories 8 to 10.**

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

- Crate `polyoxide-wasm-smoke` (`publish = false`), bin `wasm-smoke-server` taking one argument, the path of the env file it writes: the lines `POLYOXIDE_SMOKE_HTTP=<url>` and `POLYOXIDE_SMOKE_WS=<url>`.
- `scripts/wasm_smoke.sh`: no arguments; starts the server, exports both variables, runs `cargo test -p polyoxide-wasm-smoke --target wasm32-unknown-unknown --test browser_smoke` under `wasm-bindgen-test-runner` with headless Chrome, and stops the server on exit.
- The CI job id `wasm`, name `WebAssembly`, running `scripts/wasm_check.sh` then `scripts/wasm_smoke.sh`.

**Story 10 → Story 11.** `scripts/wasm_live.sh`, no arguments, runs `browser_live` with `--features live`; the nightly job `live-wasm-browser`.

## Stories

- **Story 1** — `polyoxide-rt`: the runtime trait, tasks, timeouts, intervals and `TokioRuntime` (spec D2).

  Creates the foundation crate with `Runtime`, `default_runtime`, `spawn` and `Task`, `timeout`, `interval` with `MissedTick`, and `TokioRuntime`, and wires it into the workspace: member, workspace dependencies, registry metadata, `FOUNDATION`, publish order (which `publish_order.py` computes), the allowlists, the rules file and CLAUDE.md's table and workspace list. The tests run on tokio's paused clock and pin each behaviour: sleep and `sleep_until`, `timeout` at zero with a ready future, the three missed-tick behaviours, the first tick at once, the zero-period panic, abort, `is_finished`, and that a dropped `Task` keeps running. No other crate uses it yet.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-rt/Cargo.toml`, `polyoxide-rt/README.md`, `polyoxide-rt/src/lib.rs`, `polyoxide-rt/src/runtime.rs`, `polyoxide-rt/src/task.rs`, `polyoxide-rt/src/time.rs`, `polyoxide-rt/src/tokio.rs`, `polyoxide-rt/tests/tokio_runtime.rs`, `Cargo.toml`, `Cargo.lock`, `scripts/gen_registry.py`, `.claude/rules/rt.md`, `CLAUDE.md`, `README.md`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 1 → Stories 2 to 11"; uses nothing from earlier stories
  - **Verify:** `cargo test -p polyoxide-rt && cargo clippy -p polyoxide-rt --all-targets -- -D warnings && python3 scripts/venue_gate.py && python3 scripts/gen_registry.py --check && python3 scripts/publish_order.py check-manifests && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
  - **Review:** deep (a new published foundation crate and its public API)
  - **Reading:** spec §1.1, §1.3, D1, D2, D10, D15, D16; `Cargo.toml`, `polyoxide-venue/Cargo.toml` (a foundation manifest's metadata), `scripts/gen_registry.py:100-160`, `scripts/venue_gate.py:1-60`, `.claude/rules/release.md`, `.claude/rules/ci-gates.md`, `.github/scripts/tests/test_claude_rules.py`, `polyoxide-core/src/hold.rs` (a paused-clock test style)

- **Story 2** — core and the HTTP side of the venue crates on the injected runtime (spec D3).

  Adds `ClientConfig::runtime`, the `runtime` setter in `client_config_setters!` and on `HttpClientBuilder`, and `HttpClient::runtime()`, and moves every clock read, sleep and spawn in core (the loop's retry sleep, holds, capacity buckets, the window quotas' waits, `health`'s stopwatch, the client's spawns) and in the HTTP modules of polymarket, binance and kalshi (their clients, throttles, limits and signer buckets) onto the client's runtime. governor keeps its quanta clock. A new test builds a client on a counting runtime over `TokioRuntime` and proves a 429 retry sleeps through it. Behaviour is unchanged; every existing suite passes as is.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/hold.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/tests/runtime_injection.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/api/health.rs`, `polyoxide-polymarket/src/data/client.rs`, `polyoxide-polymarket/src/gamma/client.rs`, `polyoxide-polymarket/src/gamma/api/health.rs`, `polyoxide-polymarket/src/perps/client.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/mod.rs`, `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/shared/signer_limit.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/mod.rs`, `polyoxide-binance/src/usdm/request.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-kalshi/Cargo.toml`, `polyoxide-kalshi/src/predictions/mod.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `.claude/rules/clients.md`, `.claude/rules/rate-limits.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 2 → Stories 4, 5, 7"; uses Story 1's `Runtime`, `default_runtime`, `spawn`, `Task`, `timeout` and `Instant`
  - **Verify:** `cargo test -p polyoxide-core -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-kalshi --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && bash -c 'cd .github/scripts && uv run pytest tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the send loop, holds and rate limiters; landmine notes on quotas and waits apply)
  - **Reading:** spec D3, D12; `.claude/rules/send-loop.md`, `.claude/rules/rate-limits.md`, `.claude/rules/clients.md`; `polyoxide-core/src/send.rs`, `polyoxide-core/src/hold.rs`, `polyoxide-core/src/capacity.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/client.rs:1-140,240-300`; the venue files in **Files**, at their clock and sleep sites

- **Story 3** — `polyoxide_core::http` and the move of every reqwest vocabulary path (spec D6).

  Re-exports `http` from core and moves every `reqwest::Method`, `reqwest::header::*`, `reqwest::StatusCode` and `HeaderValue` path in src, tests and examples to `polyoxide_core::http` (or `http` inside core). Types are identical, so nothing else changes. `reqwest::Client`, `Response`, `Error`, `RequestBuilder`, `redirect` and `get` stay for story 4, and alloy's reqwest in relay is untouched. Adds DRIFT row `R22` and a `[[drift]]` entry for each test body the move changes, and updates `send-loop.md`'s vocabulary sentence.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/hooks.rs`, `polyoxide-core/src/rate_limit.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/tests/mock_request.rs`, `polyoxide-binance/examples/weight_probe.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/src/usdm/policy.rs`, `polyoxide-binance/src/usdm/weight.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/auth.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/src/predictions/request.rs`, `polyoxide-kalshi/src/predictions/throttle.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `polyoxide-polymarket/examples/cf_burst_probe.rs`, `polyoxide-polymarket/examples/gamma_batch_ceiling.rs`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-polymarket/src/client.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/api/auth.rs`, `polyoxide-polymarket/src/clob/api/markets.rs`, `polyoxide-polymarket/src/clob/api/notifications.rs`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/authenticator.rs`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/data/error.rs`, `polyoxide-polymarket/src/data/v2/error.rs`, `polyoxide-polymarket/src/gamma/api/markets.rs`, `polyoxide-polymarket/src/gamma/error.rs`, `polyoxide-polymarket/src/perps/api/public.rs`, `polyoxide-polymarket/src/perps/error.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/config.rs`, `polyoxide-polymarket/src/relay/error.rs`, `polyoxide-polymarket/src/shared/limits.rs`, `polyoxide-polymarket/src/shared/policy.rs`, `polyoxide-polymarket/src/shared/signer_limit.rs`, `polyoxide-polymarket/tests/polymarket_limits.rs`, `polyoxide-polymarket/tests/polymarket_throttle.rs`, `polyoxide-polymarket/tests/send_loop.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/tests/failure_tags.rs`, `polyoxide-test-support/tests/token_cost_throttle.rs`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`, `.claude/rules/send-loop.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 3 → Stories 4 to 11"; uses nothing from earlier stories
  - **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && python3 scripts/test_body_diff.py --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** light (a path-only move; no behaviour changes)
  - **Reading:** spec D6, D12; `.claude/rules/send-loop.md` (the reqwest section), `.claude/rules/ci-gates.md` (test bodies), `ci/test-body-diff.toml:60-110`, `_bmad-output/specs/spec-venue-extensibility/divergences.md` (the DRIFT table's heading and its last three rows), `scripts/test_body_diff.py:1-80`

- **Story 4** — `HttpTransport`, `Response` and `TransportError` (spec D4, D5).

  Creates `transport.rs` and `response.rs`, moves `HttpClient` from its public `reqwest::Client` to a private `Arc<dyn HttpTransport>` built from the config (or injected), and has the loop build an `HttpRequest` with its URL, query and timeout, decide on the head, release the permit and then read an error body. Re-types every public item spec D11 lists for HTTP, replaces `ApiError::Network` and `classify_reqwest` with `ApiError::Transport` and `ReqwestTransport`'s private mapping, and updates every caller: gamma's `post_json`, relay's helper, clob's `send_raw`s, every ping's body check, the error decodes and the examples. A new test drives one retry and one hold through a recording transport. The header pins and query-setter suites pass unchanged on the default. Adds the removals lines, the drift entries and the rules text.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-core/Cargo.toml`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/response.rs`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/send.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/tests/health.rs`, `polyoxide-core/tests/mock_request.rs`, `polyoxide-core/tests/transport_injection.rs`, `polyoxide-binance/examples/weight_probe.rs`, `polyoxide-binance/src/usdm/api/health.rs`, `polyoxide-binance/src/usdm/error.rs`, `polyoxide-binance/tests/live_usdm.rs`, `polyoxide-kalshi/examples/read_soak.rs`, `polyoxide-kalshi/src/predictions/api/health.rs`, `polyoxide-kalshi/src/predictions/error.rs`, `polyoxide-kalshi/tests/live_predictions.rs`, `polyoxide-kalshi/tests/mock_predictions.rs`, `polyoxide-polymarket/examples/cf_burst_probe.rs`, `polyoxide-polymarket/examples/gamma_batch_ceiling.rs`, `polyoxide-polymarket/examples/info_soak.rs`, `polyoxide-polymarket/examples/v2_soak/main.rs`, `polyoxide-polymarket/src/clob/api/account.rs`, `polyoxide-polymarket/src/clob/api/orders.rs`, `polyoxide-polymarket/src/clob/error.rs`, `polyoxide-polymarket/src/gamma/api/markets.rs`, `polyoxide-polymarket/src/perps/api/health.rs`, `polyoxide-polymarket/src/perps/error.rs`, `polyoxide-polymarket/src/relay/client.rs`, `polyoxide-polymarket/src/relay/error.rs`, `polyoxide-polymarket/tests/mock_clob.rs`, `polyoxide-polymarket/tests/mock_relay.rs`, `polyoxide-polymarket/tests/send_loop.rs`, `polyoxide-test-support/src/query.rs`, `polyoxide-test-support/tests/failure_tags.rs`, `docs/s3-removals.md`, `docs/MUTANTS.md`, `ci/test-body-diff.toml`, `.claude/rules/send-loop.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 4 → Stories 7, 9, 10"; uses Story 1's `BoxFuture`, Story 2's `HttpClient::runtime()` and `timeout`, and Story 3's `polyoxide_core::http` and row `R22`
  - **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test -p polyoxide-polymarket --no-default-features --features clob --test headers_clob && cargo test -p polyoxide-binance --no-default-features --features usdm --test headers_usdm && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the send loop's seam, public error types and a breaking API change; landmine notes on gzip and foreign-error `From` impls apply)
  - **Reading:** spec D4, D5, D6, D11, D12; `.claude/rules/send-loop.md`, `.claude/rules/errors.md`, `.claude/rules/clients.md`; `polyoxide-core/src/send.rs`, `polyoxide-core/src/request.rs`, `polyoxide-core/src/health.rs`, `polyoxide-core/src/error.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/config.rs`, `polyoxide-core/src/macros.rs`; `polyoxide-polymarket/src/relay/client.rs` (its send helper), `polyoxide-polymarket/src/gamma/api/markets.rs` (`post_json`), `polyoxide-polymarket/src/clob/api/orders.rs:170-340`; `docs/s3-removals.md` (its format), `docs/MUTANTS.md` (rows citing core's loop or error)

- **Story 5** — `SocketTransport`, `BoxSocket` and the kit and every socket on the runtime (spec D3, D7).

  Creates `polyoxide-ws/src/transport.rs` with the trait, `BoxSocket`, `Capabilities`, `TungsteniteTransport` (its plain dial, and `per_address()`, the CLOB's dial moved unchanged), `default_socket_transport` and `transport_setters!`, and adds `Kind::Unsupported`. Re-types `connect`, `connect_over` and `Connection::new`, adds `Connection::with_runtime`, `Supervisor::runtime` and `Supervisor::socket_transport`, and gives `Protocol::connect` the transport and runtime. Moves every clock read, sleep and spawn in the kit and in every socket module (CLOB bare and supervised, perps, rtds, sports, Binance's router and forwarders, Kalshi) onto the injected runtime, and gives each socket builder the setters. The CLOB's `Protocol::connect` override and `dial` go; its builders default to `per_address()`. polymarket's `clob-ws` stops enabling `tokio/net`, and binance's `usdm-ws` stops enabling `tokio/rt`. A new test drives a `Supervisor` through a connect, a replay and a reconnect on an in-memory transport. Adds the venue macro's `Unsupported` row, D20's new text, the removals lines, the drift entries and the rules text.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-ws/Cargo.toml`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/error.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs`, `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/supervisor/tests.rs`, `polyoxide-ws/src/test_server/mod.rs`, `polyoxide-ws/src/test_server/helpers.rs`, `polyoxide-ws/src/test_server/tests.rs`, `polyoxide-ws/examples/close_reply_probe.rs`, `polyoxide-ws/tests/transport_injection.rs`, `polyoxide-venue/src/socket.rs`, `polyoxide-venue/src/ws_table.rs`, `polyoxide-venue/tests/ws_classification.rs`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-polymarket/src/clob/client.rs`, `polyoxide-polymarket/src/clob/ws/client.rs`, `polyoxide-polymarket/src/clob/ws/error.rs`, `polyoxide-polymarket/src/clob/ws/mod.rs`, `polyoxide-polymarket/src/clob/ws/scripted.rs`, `polyoxide-polymarket/src/clob/ws/supervised/market.rs`, `polyoxide-polymarket/src/clob/ws/supervised/mod.rs`, `polyoxide-polymarket/src/clob/ws/supervised/user.rs`, `polyoxide-polymarket/src/perps/ws/builder.rs`, `polyoxide-polymarket/src/perps/ws/client.rs`, `polyoxide-polymarket/src/perps/ws/error.rs`, `polyoxide-polymarket/src/perps/ws/mod.rs`, `polyoxide-polymarket/src/perps/ws/protocol.rs`, `polyoxide-polymarket/src/perps/ws/scripted.rs`, `polyoxide-polymarket/src/rtds/client.rs`, `polyoxide-polymarket/src/rtds/error.rs`, `polyoxide-polymarket/src/rtds/supervisor.rs`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/error.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `polyoxide-polymarket/tests/bare_sports.rs`, `polyoxide-polymarket/tests/failure_tags_rtds.rs`, `polyoxide-polymarket/tests/failure_tags_sports.rs`, `polyoxide-polymarket/tests/live_clob_ws.rs`, `polyoxide-polymarket/tests/live_sports.rs`, `polyoxide-polymarket/tests/supervision_rtds.rs`, `polyoxide-polymarket/tests/supervision_sports.rs`, `polyoxide-polymarket/tests/trading_clob.rs`, `polyoxide-binance/Cargo.toml`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/src/usdm/ws/error.rs`, `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/scripted.rs`, `polyoxide-binance/src/usdm/ws/supervised.rs`, `polyoxide-binance/tests/failure_tags_usdm.rs`, `polyoxide-binance/tests/live_usdm_ws.rs`, `polyoxide-binance/tests/supervision_usdm.rs`, `polyoxide-binance/tests/supervision_usdm_edges.rs`, `polyoxide-binance/tests/supervision_usdm_kit.rs`, `polyoxide-kalshi/src/predictions/ws/builder.rs`, `polyoxide-kalshi/src/predictions/ws/error.rs`, `polyoxide-kalshi/src/predictions/ws/mod.rs`, `polyoxide-kalshi/src/predictions/ws/protocol.rs`, `polyoxide-kalshi/src/predictions/ws/scripted.rs`, `polyoxide-kalshi/tests/live_predictions_ws.rs`, `polyoxide-kalshi/tests/supervision_predictions.rs`, `docs/s3-removals.md`, `docs/MUTANTS.md`, `ci/test-body-diff.toml`, `_bmad-output/specs/spec-venue-extensibility/divergences.md`, `.claude/rules/sockets.md`, `.claude/rules/polymarket-clob-ws.md`, `.claude/rules/errors.md`, `CLAUDE.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 5 → Stories 7 to 10"; uses Story 1's `Runtime`, `BoxFuture`, `default_runtime`, `spawn`, `Task`, `timeout`, `interval` and `Instant`, and Story 3's row `R22`
  - **Verify:** `cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo hack check -p polyoxide-polymarket -p polyoxide-binance -p polyoxide-ws --each-feature --no-dev-deps && python3 scripts/venue_gate.py && python3 scripts/test_body_diff.py --baseline v0.40.0 && python3 scripts/api_removals.py check --baseline v0.40.0 && bash -c 'cd .github/scripts && uv run pytest tests/test_classify_coverage.py tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the supervisor's guarantees, the CLOB's dial and a breaking API change; the `connect_async(` landmine applies)
  - **Reading:** spec D3, D7, D11, D12; `.claude/rules/sockets.md`, `.claude/rules/polymarket-clob-ws.md`, `.claude/rules/errors.md` (socket errors); `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/bare.rs`, `polyoxide-ws/src/request.rs`, `polyoxide-ws/src/timing.rs:120-200`, `polyoxide-ws/src/supervisor/mod.rs`, `polyoxide-ws/src/supervisor/task.rs`, `polyoxide-ws/src/error.rs`; `polyoxide-polymarket/src/clob/ws/client.rs:1-140` (the dial), `polyoxide-binance/src/usdm/ws/supervised.rs:200-260,480-500`; `polyoxide-venue/src/socket.rs`; `_bmad-output/specs/spec-venue-extensibility/divergences.md` (row D20)

- **Story 6** — The wall clock through `web-time`, and `runtime_gate.py` (spec D3).

  `UnixMillis::now` and `shared::auth::current_timestamp` read `web_time::SystemTime` (venue gains a wasm32-only `web-time`; polymarket reads `polyoxide_rt::SystemTime`). Adds `scripts/runtime_gate.py`, scoped as spec D3 says, and its pytest, which proves it fires on each banned path, skips `#[cfg(test)]` modules, the test server and the `scripted.rs` fixtures, and honours the opt-out. Fixes whatever stragglers it finds after stories 2 and 5; any it finds is a deviation to report. Updates `ci-gates.md` and `rt.md`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-venue/Cargo.toml`, `polyoxide-venue/src/time.rs`, `polyoxide-polymarket/src/shared/auth.rs`, `scripts/runtime_gate.py`, `.github/scripts/tests/test_runtime_gate.py`, `.claude/rules/ci-gates.md`, `.claude/rules/rt.md`, `Cargo.lock`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 6 → Stories 9, 11"; uses Story 1's `SystemTime` and `UNIX_EPOCH`
  - **Verify:** `python3 scripts/runtime_gate.py && cargo test -p polyoxide-venue -p polyoxide-polymarket --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && bash -c 'cd .github/scripts && uv run pytest tests/test_runtime_gate.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** light (the signing timestamp keeps its value; the gate is a script with its own tests)
  - **Reading:** spec D3, D10; `scripts/venue_gate.py`, `scripts/live_unwraps.py:1-80` (the opt-out convention), `.github/scripts/tests/` (a venue-gate test as the model), `polyoxide-venue/src/time.rs`, `polyoxide-polymarket/src/shared/auth.rs`

- **Story 7** — The target gating and the three browser implementations (spec D8, D10).

  Moves each native-only dependency into a `cfg(not(target_arch = "wasm32"))` table and adds the wasm32-only ones (spec D10), with `getrandom`'s `wasm_js` in `polyoxide-rt`. Adds `BrowserRuntime` (`spawn_local`; gloo-timers sleeps chained in steps of at most `i32::MAX` ms; `performance.now`), `FetchTransport` (reqwest's wasm client, `SendWrapper`'d, the timeout per request, failures mapped per spec D8) and `WebSocketTransport` (web-sys, `binaryType` arraybuffer, events through an unbounded channel, closes mapped to 1000 or 3000–4999, headers and control frames refused with the contract's messages), each the wasm32 default of `default_runtime`, the config's transport and `default_socket_transport`. Adds the wasm32 target to the flake's toolchain and `scripts/wasm_check.sh`, which passes for spec D1's whole set. Native builds are unchanged.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-rt/Cargo.toml`, `polyoxide-rt/src/lib.rs`, `polyoxide-rt/src/browser.rs`, `polyoxide-core/Cargo.toml`, `polyoxide-core/src/lib.rs`, `polyoxide-core/src/fetch.rs`, `polyoxide-core/src/client.rs`, `polyoxide-core/src/error.rs`, `polyoxide-ws/Cargo.toml`, `polyoxide-ws/src/lib.rs`, `polyoxide-ws/src/browser.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/connect.rs`, `polyoxide-ws/src/tls.rs`, `polyoxide-venue/Cargo.toml`, `polyoxide-polymarket/Cargo.toml`, `polyoxide-binance/Cargo.toml`, `Cargo.toml`, `Cargo.lock`, `flake.nix`, `scripts/wasm_check.sh`, `.claude/rules/rt.md`, `.claude/rules/sockets.md`, `.claude/rules/send-loop.md`, `CLAUDE.md`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 7 → Stories 8 to 10"; uses Story 1's `Runtime` and `default_runtime`, Story 4's `HttpTransport`, `Response` and `TransportError`, and Story 5's `SocketTransport`, `Capabilities` and `Kind::Unsupported`
  - **Verify:** `bash scripts/wasm_check.sh && cargo test --workspace --all-features && cargo clippy --workspace --all-targets --all-features -- -D warnings && msrv-cargo check --workspace --all-features && bash -c 'cd .github/scripts && uv run pytest tests/test_dependency_fences.py tests/test_dep_allowlists.py tests/test_claude_rules.py -q'`
  - **Review:** deep (the browser implementations' contracts and `SendWrapper` soundness)
  - **Reading:** spec D1, D7, D8, D10, D13; `polyoxide-rt/src/*.rs`, `polyoxide-core/src/transport.rs`, `polyoxide-core/src/response.rs`, `polyoxide-ws/src/transport.rs`, `polyoxide-ws/src/error.rs`, the five manifests in **Files**, `flake.nix:20-70`; `.claude/rules/sockets.md` (TLS needs a nudge)

- **Story 8** — The Binance keepalive and sports liveness on a transport without pings (spec D9, D17).

  Binance's supervised protocol pings with `LIST_SUBSCRIPTIONS` through its request correlation when its transport's `client_pings` is false, and its bare `ping()` refuses with `Kind::Unsupported` there; a scripted-server test pins both on a test transport reporting browser capabilities. The live test checks the request on `fstream` within spec D17's limits, and its result picks D9's main rule or its fallback. Sports: adds a `--cadence` mode to the capture script, runs it within D17's limits, records the result, then applies D9's rule for a transport whose `sees_server_pings` is false, with a scripted test. Both findings go under the contract's `OBSERVED.md` headings, and `binance.md` and `polymarket-sports.md` record the rules. A D17 stop goes to the owner.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/client.rs`, `polyoxide-binance/tests/supervision_usdm_kit.rs`, `polyoxide-binance/tests/live_usdm_ws.rs`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `polyoxide-polymarket/tests/supervision_sports.rs`, `scripts/capture_polymarket_sports.py`, `docs/specs/binance/OBSERVED.md`, `docs/specs/sports/OBSERVED.md`, `.claude/rules/binance.md`, `.claude/rules/polymarket-sports.md`
  - **Interfaces:** exposes "Story 8 → Story 10"; uses Story 5's `Capabilities`, `Kind::Unsupported` and `transport_setters!`
  - **Verify:** `cargo test -p polyoxide-binance -p polyoxide-polymarket --all-features && cargo test -p polyoxide-binance --features usdm-ws --test live_usdm_ws -- --ignored list_subscriptions_answers_as_a_keepalive && python3 scripts/live_unwraps.py && bash scripts/wasm_check.sh`
  - **Review:** deep (liveness rules, from live measurements)
  - **Reading:** spec D9, D17; `.claude/rules/binance.md`, `.claude/rules/polymarket-sports.md`, `.claude/rules/live-tests.md`; `polyoxide-binance/src/usdm/ws/protocol.rs`, `polyoxide-binance/src/usdm/ws/client.rs:260-320`, `polyoxide-polymarket/src/sports/client.rs`, `polyoxide-polymarket/src/sports/supervised.rs`, `scripts/capture_polymarket_sports.py`, `scripts/capture_common.py`; the last 40 lines of each `OBSERVED.md` in **Files**

- **Story 9** — The per-push browser smoke suite and the `wasm` CI job (spec D13).

  Creates `polyoxide-wasm-smoke`: its native `wasm-smoke-server` (mockito routes with CORS headers, the two 429s, one scripted socket on the kit's test server) and its wasm32 `tests/browser_smoke.rs`, which runs in a dedicated Worker and covers `BrowserRuntime`, `FetchTransport` and `WebSocketTransport` as spec D13 lists. Adds `scripts/wasm_smoke.sh`, the flake's chromium, chromedriver and `wasm-bindgen-cli` (the version `Cargo.lock` pins), and the `wasm` job in `ci.yml` (wasm32 target, `wasm-bindgen-cli`, the runner's Chrome, then both scripts), and adds `runtime_gate.py` to the CI scripts job. Updates the jobs table in CLAUDE.md and `ci-gates.md`.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-wasm-smoke/Cargo.toml`, `polyoxide-wasm-smoke/src/lib.rs`, `polyoxide-wasm-smoke/src/bin/wasm-smoke-server.rs`, `polyoxide-wasm-smoke/tests/browser_smoke.rs`, `scripts/wasm_smoke.sh`, `Cargo.toml`, `Cargo.lock`, `flake.nix`, `.github/workflows/ci.yml`, `.claude/rules/ci-gates.md`, `CLAUDE.md`, `ci/dep-allowlists/`
  - **Interfaces:** exposes "Story 9 → Story 10"; uses Story 6's `runtime_gate.py` and Story 7's `BrowserRuntime`, `FetchTransport`, `WebSocketTransport`, the contract's messages and `scripts/wasm_check.sh`
  - **Verify:** `bash scripts/wasm_check.sh && bash scripts/wasm_smoke.sh && cargo test --workspace --all-features && python3 scripts/gen_registry.py --check && bash -c 'cd .github/scripts && uv run pytest tests/ -q'`
  - **Review:** deep (a new CI gate and the browser behaviour it proves)
  - **Reading:** spec D13; `.github/workflows/ci.yml`, `.claude/rules/ci-gates.md`, `.claude/rules/testing.md`, `flake.nix`, `polyoxide-ws/src/test_server/mod.rs:1-120`, `polyoxide-rt/src/browser.rs`, `polyoxide-core/src/fetch.rs`, `polyoxide-ws/src/browser.rs`

- **Story 10** — The nightly live browser job (spec D14, D17).

  Adds `tests/browser_live.rs` behind a `live` feature (per host in spec §1.4: CORS, exposed headers, the socket upgrade, one public GET, one socket and Binance's keepalive, within D17's limits and with its retry and tag rules), `scripts/wasm_live.sh`, a wasm-bindgen runner in `gen_registry.py` and the `live-wasm-browser` job it generates in `nightly-behavioral.yml`. `polyoxide-test-support`'s tags build on wasm32, with its keychain loaders native-only. The story proves a tag line from a failing browser test reaches the runner's output. Records spec §1.4's browser facts in each host's `OBSERVED.md` under the contract's heading.

  - **Developer:** intermediate-dev
  - **Files:** `polyoxide-wasm-smoke/Cargo.toml`, `polyoxide-wasm-smoke/tests/browser_live.rs`, `scripts/wasm_live.sh`, `scripts/gen_registry.py`, `.github/workflows/nightly-behavioral.yml`, `CLAUDE.md`, `SELF-HEALING.md`, `polyoxide-test-support/Cargo.toml`, `polyoxide-test-support/src/lib.rs`, `docs/specs/clob/OBSERVED.md`, `docs/specs/gamma/OBSERVED.md`, `docs/specs/data-v2/OBSERVED.md`, `docs/specs/perps/OBSERVED.md`, `docs/specs/binance/OBSERVED.md`, `docs/specs/kalshi/OBSERVED.md`, `docs/specs/sports/OBSERVED.md`, `docs/specs/rtds/OBSERVED.md`, `.claude/rules/live-tests.md`, `Cargo.lock`, `ci/dep-allowlists/`
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
  - **Review:** light (docs and an amendment for the owner to merge)
  - **Reading:** spec D1, D15; README.md, CLAUDE.md, `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/spine-amendments/epic-9.md`, the rules files in **Files**

<!-- plan_join: the stories below are generated from the drafts; edit the drafts, then re-join -->
