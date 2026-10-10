# polyoxide-ws

The WebSocket kit the polyoxide socket clients share: the rustls provider
install, the reconnect `Backoff`, the supervision `Timing` and its builder
setters, the wait until the next deadline, and `WsError`, the one socket
error, read through its `Kind`.

It depends on neither `polyoxide-core` nor `polyoxide-venue`, and builds no
HTTP or signing stack, so a credential-free feed built on it stays light.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-ws/).

## Installation

```toml
[dependencies]
polyoxide-ws = "0.38"
```

## Usage

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
