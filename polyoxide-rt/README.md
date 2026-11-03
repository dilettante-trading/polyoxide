# polyoxide-rt

The runtime seam of the polyoxide crates. Code that sleeps, reads the clock or spawns a task takes a `Runtime` instead of calling a runtime's own functions, so a runtime can be injected rather than assumed.

A `Runtime` is a detached `spawn`, a `sleep`, a monotonic `now` and a `sleep_until` with a default. `spawn` turns any future into a `Task`, which resolves to its output, or to nothing when it was aborted or panicked, and which keeps running when it is dropped. `timeout` awaits a future for a bounded time and polls it before the timer, so a ready future wins at a zero duration. `interval` ticks at once and then every period, with a choice of what to do about ticks it was too late for: fire them together, delay the next one, or skip them. `TokioRuntime` is the native runtime, built on tokio's own clock so a paused clock drives it, and `default_runtime` returns it.

`Instant`, `SystemTime` and `UNIX_EPOCH` are re-exported from `web-time`: the standard library's own types on native.

The crate depends on nothing in the workspace and names no venue, so it publishes first and every other crate can use it.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-rt/).

## Installation

Add `polyoxide-rt` to the dependencies of your crate's manifest, at the version of the other polyoxide crates you use.
