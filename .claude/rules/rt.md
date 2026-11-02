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

## The wall clock and the runtime gate

The runtime is injected; the wall clock is not. A timestamp a venue reads is the real time
whichever executor runs the client, so `UnixMillis::now` and `shared::auth::current_timestamp`
read `SystemTime::now()` from `std` on every target but wasm32, where `polyoxide-venue` takes
`web_time`'s, and `polyoxide-polymarket` reads `polyoxide_rt::SystemTime`, which is `web_time`'s
type (`std`'s on every target but wasm32). Both lines end
`// runtime-gate: the wall clock is never injected`.

`scripts/runtime_gate.py` fails on any other `Instant::now` or `SystemTime::now`, called or passed
as a function value, on `tokio::spawn`, on any `tokio::task::spawn*` (`spawn_blocking` and
`spawn_local` too) and on any `tokio::time` path, however imported (`use tokio::{spawn, ..}` and
`use tokio::task;` are hits), in the production code of the crates that use this one
(`ci-gates.md`); `polyoxide-rt` implements the runtime and is the one crate it does not scan. A
timer or a spawn there goes through the `Runtime` the client was built with: `runtime.sleep`,
`runtime.now`, `polyoxide_rt::spawn`, `polyoxide_rt::timeout` and `polyoxide_rt::interval`.

## The browser runtime (`src/browser.rs`)

`BrowserRuntime` is the `wasm32` default of `default_runtime`; a host's is `TokioRuntime`. It holds
no state: `spawn` is `wasm_bindgen_futures::spawn_local`, `now` is `web_time::Instant::now`
(`performance.now`), and `sleep` chains gloo-timers' `TimeoutFuture` in steps of at most `i32::MAX`
ms, because `setTimeout` fires at once for a larger delay. The arithmetic (`step_ms`, `steps`: the
remainder rounded up to a millisecond, so a sleep is never short) is plain Rust built on every
target, so the host's tests pin it. Each timer's JS handle sits in a `send_wrapper::SendWrapper`:
the `Send` bound of `Runtime` holds, and the handle is only touched on the one thread a browser
build has. A build with atomics (threads) is unsupported, because the wrapper panics off its thread.
The supported contexts are the main thread, a dedicated Worker and a SharedWorker, and a Worker is
the recommended one: Chrome throttles a hidden tab's main-thread timers to once a second, and to
once a minute after five minutes, which breaks ping cadences.

Dependencies are per target: tokio's `rt` and `time` are the host's, and `gloo-timers`,
`send_wrapper`, `wasm-bindgen-futures` and `getrandom` 0.3 with `wasm_js` are `wasm32`'s.
`getrandom` is here so feature unification reaches `rand` and `tungstenite` from every crate that
builds this one; the final binary still sets `--cfg getrandom_backend="wasm_js"`, which
`scripts/wasm_check.sh` sets itself.
