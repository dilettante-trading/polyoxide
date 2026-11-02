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
