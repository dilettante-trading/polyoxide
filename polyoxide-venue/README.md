# polyoxide-venue

The vocabulary every polyoxide crate shares: the eight error classes, the
`Classify` trait every public error type implements, the status and close-code
maps, the one `Retry-After` parser, `Secret`, the `wire_enum!` and `open_enum!`
macros with their `UnknownVariant`, `UnixMillis`, and, behind the `decimal`
feature, the positional decimal serde of rows sent as bare arrays.

Behind the `keys` feature, `MarketKey` names one instrument as
`venue.product:id`, canonical because only a product's `KeySpace` makes one.
Behind the `market` feature, the market-data traits (`MarketData`, which every
venue product implements, and the capability traits `Instruments`, `Trades`,
`Candles`, `Funding` and `EventGroups`, each with a `Dyn` wrapper) return the
shared records, each carrying a venue's own facts in its `Extensions`.
Behind the `trading` feature, `Trading` places and cancels orders, reads open
orders, positions and balances, and streams acknowledgements and fills as
`TradingEvents`, which begins with a `Resync`; `Extensions::ensure_declared` is
how a venue refuses an option it does not declare.

By default it depends on nothing, so a credential-free socket client can use it
without building an HTTP or signing stack.

More information about this crate can be found in the [crate documentation](https://docs.rs/polyoxide-venue/).

## Installation

```toml
[dependencies]
polyoxide-venue = "0.41"
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
