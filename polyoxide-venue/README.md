# polyoxide-venue

The vocabulary every polyoxide crate shares: the eight error classes, the
`Classify` trait every public error type implements, the status and close-code
maps, the one `Retry-After` parser, and `Secret`.

It depends on nothing, so the credential-free socket crates (`polyoxide-rtds`,
`polyoxide-sports`) can use it without building an HTTP or signing stack.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-venue/).

## Installation

```toml
[dependencies]
polyoxide-venue = "0.38"
```

## Usage

One retry policy serves every polyoxide error, whichever crate it came from:

```rust
use std::time::Duration;
use polyoxide_venue::{retry_delay, Class, Classify};

fn next_attempt<E: Classify>(err: &E, backoff: Duration) -> Option<Duration> {
    if !err.is_retriable() {
        return None;
    }
    // A server's `Retry-After` may lengthen the wait, never shorten it.
    Some(retry_delay(err.retry_after(), backoff))
}

#[derive(Debug)]
struct Throttled;

impl std::fmt::Display for Throttled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("throttled")
    }
}

impl std::error::Error for Throttled {}

impl Classify for Throttled {
    fn class(&self) -> Class {
        Class::RateLimited { retry_after: Some(Duration::from_secs(2)) }
    }
}

assert_eq!(
    next_attempt(&Throttled, Duration::from_millis(500)),
    Some(Duration::from_secs(2))
);
```

A caller that handles errors from several crates at once can convert each into
a `ClassifiedError` with `?`, which keeps the class, whether the error is a
fault, and the wait it asked for.
