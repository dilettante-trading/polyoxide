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
