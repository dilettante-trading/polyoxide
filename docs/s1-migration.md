# Migrating to polyoxide 0.39 (S1)

polyoxide 0.39.0 is stage S1 of the multi-venue restructure. S1 consolidates internals: one
error vocabulary, one HTTP send loop and one WebSocket kit under every venue crate. Crate names
stay, and so do almost all type names, builders and namespace methods. The crate merges and
renames come in S2, with a guide of their own. This guide is for code on 0.38.x (the last
release is 0.38.2; the removal baseline is 0.38.1).

Three crates are on crates.io for the first time:

- `polyoxide-venue`: the error classes and the `Classify` trait every error implements, plus
  `Secret`, `UnixMillis`, `UnknownVariant` and the wire-enum macros. It depends on nothing by
  default.
- `polyoxide-ws`: the WebSocket kit that perps, Binance, rtds, sports and clob now run on.
  Depend on it to name its types, such as `polyoxide_ws::Kind`, or to write a socket client of
  your own on its parts: `connect`, `connect_over`, the bare `Connection`, `Backoff`, `Timing`,
  and the generic `Supervisor<P: Protocol>` with its `Supervised<P>` stream and
  `MembershipHandle<P>`. Classify its `WsError` in your crate with
  `polyoxide_venue::impl_ws_classification!`. It depends on neither core nor venue.
- `polyoxide-cli`: `cargo install polyoxide-cli` installs the `polyoxide` binary.

Each removed path, with its replacement, is in [`docs/s1-removals.md`](s1-removals.md). Removed
items have no shim or deprecated alias, so the compiler finds every call site. This guide
explains the patterns behind that list, the shapes that changed, and the changes that compile
unchanged but behave differently.

## Upgrade checklist

1. Move every `polyoxide*` dependency to `0.39` together.
2. Add `polyoxide-venue = "0.39"` and `use polyoxide_venue::Classify;` wherever you call
   `is_retriable()` or `retry_after()` on an error.
3. Match `ApiError::Response(r)` where you matched `ApiError::{Api, Authentication, RateLimit,
   Timeout}`, and give a wildcard arm to every match on `ApiError`, `GammaError`, `RelayError`
   and clob's `WebSocketError`, which are now `#[non_exhaustive]`.
4. Read a socket's transport error through `.kind()` instead of matching `tungstenite::Error`
   (add `polyoxide-ws = "0.39"` to name `Kind`).
5. Binance: write `UsdmWsError::Stopped { .. }`. Perps: `use polyoxide_perps::ws::PerpsMembership;`
   before `membership().subscribe(..)`.
6. Python: recheck every `except` clause against [Python](#python).
7. Move log filters for retries, holds and reconnects to the `polyoxide_core` and `polyoxide_ws`
   targets ([Logging](#logging)).

## Behaviour you will notice

| Change | Crates | What to do |
|---|---|---|
| A reconnect refused with 408, 425, 429 or 5xx is retried (DRIFT R1): perps adds 408 and 425, and rtds no longer ends `run()` on a 503 or 429 at reconnect | perps, rtds | Nothing; watch the reconnect WARN, or the class, where you relied on `run()` returning. |
| Every connect gives up after 10 s by default (R2) | perps (bare and supervised), rtds | Raise `connect_timeout` on `PerpsWsBuilder` or `RtdsBuilder` for a slow network; the bare tiers always use 10 s. |
| The server's close is answered with the RFC 6455 close reply (R3) | perps, Binance, rtds, clob | Nothing. |
| A zero `Retry-After` is no wait, `None` where it was `Some(0s)`; fractional and padded values parse; a huge one is `Duration::MAX` (R4) | every HTTP crate, Python v2 errors | Treat `None` as "the server asked for no wait". |
| A reconnect after a refusal waits the longer of its backoff step and the refusal's `Retry-After`, never past the backoff ceiling (R11) | perps, Binance, rtds, sports | Raise the `backoff` maximum if a longer `Retry-After` must be honoured in full. |
| A 429's `Retry-After` is in its class and in `Classify::retry_after`; a socket error's `retry_after()` is any refused upgrade's | gamma, data, perps, clob, relay, every socket crate | Read it there instead of parsing headers. |
| An upgrade refused with 451 is `Restricted` and not a fault | every socket crate | Key alerting on `is_fault()`. |
| A refusal made before sending (`ApiError::Validation`) is `InvalidRequest`, where it was `VenueRefusal`; a server 400 stays `VenueRefusal` | clob, relay | Recheck class-based handling. |
| A relayer 429 is `RateLimited` and a 503 `Unavailable`, where every relayer failure was `VenueRefusal` (R7) | relay | Retry on the class. |
| `is_retriable()` on `DataApiError::V2` follows the status, not the server's `retryable` flag | data | Read `V2Error::retryable` for the server's own answer. |
| An `ApiError::Network` that broke off mid-body is retriable | every HTTP crate | Judge order resubmission yourself, as before. |
| A Gamma failure during clob's profile lookup is `ClobError::Gamma`, classed as the Gamma error, so a Gamma 5xx is retriable | clob | Match `ClobError::Gamma`. |
| Health pings, and gamma's `query_by_information` and `query_abridged`, retry 429 and 425 and hold the client on a 429; the clob ping waits for a permit and the throttle (R8) | clob, gamma, data | Nothing. |
| `ping()` latency leaves out the permit, throttle and backoff waits | perps, Binance, relay | Expect lower readings under load. |
| A client built directly on `HttpClientBuilder` no longer retries a 425 | core | Install `polymarket::PolymarketRetryPolicy` to keep it. |
| Python exceptions are chosen by class, and several raised types change | Python | See [Python](#python). |
| A data v1 comma-joined setter given exactly `[""]` sends nothing, where it sent `key=` | data | Nothing, unless you relied on the empty key. |
| Perps klines, mark points and book levels accept scientific notation (`"1e-5"`) | perps | Nothing. |
| Retry, hold, decode and reconnect log lines change target and text | clob, relay, Binance, perps, rtds | See [Logging](#logging). |
| A consumer that leaves the queue full keeps the client pinging, but the host may close the connection after its own pong limit (perps about 70 s, Binance 10 min as documented); the stream reconnects and keeps the queued events | perps, Binance | Read promptly; on perps, a minute behind can cost a `Reconnected`. |
| A Binance subscription change the server leaves unanswered for `stale_after` is a `Stale` outage, and its caller gets `Ok` | Binance | Watch `Event::Disconnected`. |
| A Binance replay is bounded as a whole by `stale_after`, its 200 ms pacing included | Binance | Keep `stale_after` above about 1 s on a path carrying 1024 streams. |
| The bare `UsdmWs` and `Rtds` end after the first transport error they yield, and clob's `WebSocket` after its first error of any kind | Binance, rtds, clob | Treat that `Err` as the end of the stream. |
| Unsubscribing every perps channel keeps the connection, and a reconnect replays nothing, where it ended the stream with `EmptySubscription` | perps | Call `close()` to end it. |

## Errors and classification

### `Classify`

Every public error type implements `polyoxide_venue::Classify`, the socket errors and the
auxiliary types (`V2Error`, `VenueError`, `BurstCapacityExceeded`, `Refused`,
`ParseTickSizeError`, `UnknownVariant`, `KeychainError`, `PolymarketError`) included:

```rust
pub trait Classify: std::error::Error {
    fn class(&self) -> Class;
    fn is_fault(&self) -> bool { true }
    fn retry_after(&self) -> Option<Duration> { self.class().retry_after() }
    fn is_retriable(&self) -> bool { self.class().is_retriable() }
}
```

`Class` is `#[non_exhaustive]`. A status decides the class before any body does:

| `Class` | Produced by |
|---|---|
| `Network` | no response: a failed connect, a timeout, a reset, DNS, a dropped socket, a close with 1000, 1001, 1006, 1011–1013 or no code |
| `Unavailable { code }` | 408, 425, 5xx |
| `RateLimited { retry_after }` | 429 |
| `Unauthorized` | 401, 403 |
| `InvalidRequest` | the client refused or was misused: validation, a bad URL, a refused cost, a signing failure, a socket used after it closed |
| `VenueRefusal { code }` | any other 4xx; a close with 1002, 1003, 1007–1010 or 4000–4999 |
| `Restricted` | 418, 451, Binance's firewall 403 |
| `Decode` | a response or frame that did not parse; a status outside 400–599 |

`is_retriable()` is true for `Network`, `Unavailable` and `RateLimited`. It describes the
failure, not whether resubmitting an order is safe. `is_fault()` is false only where the venue
answered as designed: clob's FAK and FOK kills, and a 451 region block on HTTP or on a socket
upgrade. Key alerting on it. `retry_after()` is the wait the server asked for, never `Some` of
zero.

No crate re-exports the trait. Depend on `polyoxide-venue` directly, at the version the
polyoxide crates use: a copy at another version is a different trait, which your errors do not
implement.

```toml
[dependencies]
polyoxide-clob = "0.39"
polyoxide-venue = "0.39"
```

```rust
use polyoxide_venue::{Class, Classify};

if err.is_retriable() {
    let wait = err.retry_after().unwrap_or(fallback);
    // back off, then retry if the operation is safe to repeat
}
if let Class::RateLimited { retry_after } = err.class() { /* … */ }
```

The inherent methods are gone: `is_retriable` on core's `ApiError`, `ClobError`, `DataApiError`,
`PerpsError`, perps' `VenueError` and `BinanceError`, and `retry_after` on `DataApiError`,
`PerpsError` and `BinanceError`. The trait methods of the same names replace them. The public
`retry_after` fields of `ErrorResponse`, `V2Error` and `VenueError` stay, so `err.retry_after` is
the field and `err.retry_after()` the trait. The trait answers as the inherent methods did,
except:

- `DataApiError::V2`: the status decides; the server's flag stays readable as `V2Error::retryable`.
- `ApiError::Network` that is neither a connect failure nor a timeout (a body cut off mid-read):
  retriable.
- A Polymarket 429: `retry_after()` is its `Retry-After`, where it was `None`.

`polyoxide_venue::ClassifiedError` is new: built from any `Classify` error with `?` or `.into()`,
it keeps the `class`, `is_fault()`, `retry_after()` and the original as `source`, for code that
handles several venues' errors as one type. It does not implement `Classify`; read its `class`
field.

### Core's `ApiError`

`ApiError` is `#[non_exhaustive]`. Every unsuccessful response, whatever its status, is
`ApiError::Response(Box<ErrorResponse>)`; `Api`, `Authentication`, `RateLimit` and `Timeout` are
gone. The other variants are `Validation`, `Network`, `Serialization`, `Url`, and two new ones,
both `InvalidRequest` and never retried: `Refused(Refused)`, a cost the throttle refused before
sending, and `Sign(Box<dyn Error + Send + Sync>)`, a signing failure.

`ErrorResponse` carries public `status: StatusCode`, `headers: HeaderMap`, `body: String`,
`message: String` (the body's `error` or `message` field) and `retry_after: Option<Duration>`
(unclamped, a zero `None`). Build one with `ErrorResponse::new(status, headers, body)`, which
replaces `ApiError::from_status_and_body`, or `ErrorResponse::read(response).await`, and convert
it with `ApiError::from`. `ApiError::from_response` stays.

```rust
// 0.38
match err {
    ApiError::RateLimit(msg) => …,
    ApiError::Authentication(msg) => …,
    ApiError::Timeout => …,
    ApiError::Api { status, message } if status >= 500 => …,
    _ => …,
}

// 0.39
match &err {
    ApiError::Response(r) if r.status == 429 => …, // r.retry_after
    ApiError::Response(r) if r.status == 401 || r.status == 403 => …,
    ApiError::Response(r) if r.status == 408 => …,
    ApiError::Response(r) if r.status.is_server_error() => …, // r.message, r.body
    _ => …,
}
```

Every response error displays `API error: <status> - <message>`. A 401 that read
`Authentication failed: …`, a 429 that read `Rate limit exceeded: …` and a 408 that read
`Request timeout` now read `API error: 401 - …` and so on. Code that matches error text should
match the status or the class instead.

`RequestError` is a marker, implemented for every `From<ApiError> + Debug` type, and its
`from_response` is gone. Each crate's `From<ApiError>` is its one decode, so `?` on an
`ApiError` decodes the venue's body whichever route produced it. `impl_api_error_conversions!` is
gone, and no crate's error implements `From<reqwest::Error>`, `From<url::ParseError>` or
`From<serde_json::Error>`: route a foreign error through `ApiError`
(`.map_err(ApiError::from)?`).

### Each crate's errors

- **gamma**: `GammaError` is `#[non_exhaustive]`; its one variant is still `Api(ApiError)`.
- **data**: `From<ApiError>` turns a v2 error body into `DataApiError::V2`, by its shape, on any
  route, so `data.accounting().snapshot(..)` now yields `V2` for a v2 body, where it yielded
  `Api`.
- **perps**: `From<ApiError>` turns a `{status: "err", error, ref}` body into `PerpsError::Venue`.
- **clob**: `ClobError::Gamma(GammaError)` (feature `gamma`) is new, for a failure of clob's
  Gamma dependency. It displays `Gamma error: <gamma's error>`, where clob built a status-`0`
  `Api` error displaying `API error: 0 - Failed to fetch user profile: …`. `From<ApiError>` is
  hand-written: a 400 kill becomes `FakUnmatched` or `FokUnfilled`, a refused signer cost
  `BurstCapacityExceeded`, and a `Sign` the clob error inside it, so callers see the variants they
  saw before.
- **relay** (DRIFT R7): `RelayError` is `#[non_exhaustive]`, and `RelayError::Api` wraps core's
  `ApiError` where it held a `String`. See below.
- **Binance**: `BinanceError`'s variants and classes are unchanged (`IpBanned` with its
  `Retry-After` clamped to 3 days, `RateLimited`, `RegionBlocked`, `Forbidden`, `Venue`). A body
  in no shape it knows is `Api(ApiError::Response(_))`, its message clipped. `code()` stays.
- **umbrella**: `PolymarketError` implements `Classify` by delegating to the error it wraps;
  `Config` is `InvalidRequest`. The umbrella does not re-export `polyoxide_venue`.

Relay's failures, before and after:

```rust
// 0.38
match err {
    RelayError::Api(text) => …,        // every relayer failure, as text
    RelayError::Reqwest(e) => …,
    RelayError::UrlParse(e) => …,
    _ => …,
}

// 0.39
use polyoxide_core::ApiError;
match err {
    RelayError::Api(ApiError::Response(r)) => …,   // r.status, r.message, r.body
    RelayError::Api(ApiError::Validation(msg)) => …, // refused before sending
    RelayError::Api(ApiError::Network(e)) => …,
    RelayError::Signer(msg) => …,
    _ => …,
}
```

`Reqwest` is `Api(Network)` (an error response whose body failed to read is `Api(Response)` with
an empty body), `UrlParse` is `Api(Url)` (from `RelayClientBuilder::url` too), `SerdeJson` is
`Api(Serialization)`, `Core` is `Api`, and the never-built `RateLimit` is `Api(Response)` with
status 429. The refusals made before sending (validation, keychain, chain configuration,
headers, gas estimation) keep their messages but display `Validation error: …` where they
displayed `Relayer API error: …`; an HTTP failure displays `API error: <status> - <message>` where
it displayed `Relayer API error: <path> failed: <body>`.

### Python

Every exception is chosen by the error's class, never by its message, except a Data API v2
error, which still maps by its `code`:

| Class | Exception |
|---|---|
| `Network` | `NetworkError` |
| `Unavailable` | `UnavailableError` (new) |
| `RateLimited` | `RateLimitError` |
| `Unauthorized` | `AuthenticationError` |
| `InvalidRequest` | `ValidationError` |
| `VenueRefusal` | `ApiError` |
| `Restricted` | `RestrictedError` (new) |
| `Decode` | `DecodeError` (new) |

A class the bindings do not know raises a bare `PolyoxideError`. `TimeoutError` now subclasses
`UnavailableError`, and only v2's `request_timeout` raises it. Raised types that change:

- a server 400 raises `ApiError` (was `ValidationError`);
- a 408 or a 5xx raises `UnavailableError` (was `TimeoutError` or `ApiError`);
- a 418 or a 451 raises `RestrictedError` (was `ApiError`);
- a body that does not decode, and data's `Pagination`, raise `DecodeError` (was `ApiError` or
  `PolyoxideError`);
- a refused cost, a signing failure, a URL that does not parse and clob's local errors raise
  `ValidationError` (some were `ApiError`);
- a transport timeout raises `NetworkError` (was `TimeoutError` or `NetworkError`, by its text);
- a message that merely contains "429", "timeout" or "connection" no longer changes the type.

The six attributes (`status`, `code`, `retryable`, `trace_id`, `parameter`, `retry_after`) are
unchanged and are `None` unless the error came from a v2 route. A v2 `retry_after` of zero is now
`None` where it was `0.0`.

## HTTP behaviour

### One send loop

Every request from gamma, data, perps, clob, relay and Binance now goes through core's
`HttpClient::send`: it takes a concurrency permit, charges the throttle, signs, sends, lets the
throttle observe the response, and asks the client's `RetryPolicy` whether to finish, retry or
fail. Relay's gas estimation, which goes to the chain's RPC node, is the one request outside it.
What retries:

- gamma, data, perps, clob and relay install `polymarket::PolymarketRetryPolicy`: a 429 and a 425
  are retried, and a 429 holds every request on the client. Unchanged from 0.38.
- Binance retries a 429, waiting the longer of its backoff and the `Retry-After`. A 429 with no
  retry left and no `Retry-After` holds to the next minute; a 418 fails and holds its
  `Retry-After`, or two minutes; a 425 is not retried. A 429 with a retry left now takes two
  jitter draws of the same backoff where it took one, within the same 0.75–1.25 band.
- A client built on `HttpClientBuilder` with no policy gets `DefaultRetryPolicy`, which retries
  a 429 only. 0.38 also retried a 425 there.
- No policy retries a 5xx or a 408. Drive your own retries from `is_retriable()`.

A hold is applied whether or not its request retries, and a `Retry-After` only ever lengthens a
wait. A request that waited on a bucket now waits out any hold set meanwhile before it is sent
(`RateLimiter::acquire`, `SignerLimiter::acquire`, `ClobThrottle`). On clob, a 429 holds the
per-signer buckets as well as the IP table, and `Poly-RateLimit-*` headers are read from every
response, not only trading responses, so `Clob::rate_limit_status()` and the tier would follow a
non-trading response that carried them.

`Request::send_raw`, `get_bytes` and `health` return `ApiError::Response` for any non-2xx, so a
failed response never reaches you as data.

If you build your own client, core now exports the loop's parts: `HttpClient::send`,
`HttpClient::health` and `Pong`, `ClientConfig`, the hooks (`Throttle`, `RetryPolicy`,
`Authenticator`, `Cost`, `RequestParts` with its per-attempt `timeout`, `Decision`, `Outcome`,
`DefaultRetryPolicy`, `NoThrottle`), `Hold`, `CapacityBucket`, `WindowQuotaTable`,
`polymarket::{ClobThrottle, clob_throttle, PolymarketRetryPolicy, signer_cost}`,
`RetryConfig::attempt_info`, `Hold::is_held` and `decode_json`. Core's `Request<T, E>` gains
`method`, `body`, `authenticator` and `with_cost`. Removed: `HttpClient::should_retry`,
`note_rate_limited` and `acquire_rate_limit` (send through `HttpClient::send`), and
`RateLimiter::{clob, gamma, data, relay, perps}_default` (use `polymarket::clob_limits()` and its
siblings). `HttpClient::acquire_concurrency` stays. `HttpClientBuilder` is no longer `UnwindSafe`
or `RefUnwindSafe`, since it holds the throttle and policy as trait objects.

### `Retry-After` (DRIFT R4)

Every `Retry-After` is read by one parser, `polyoxide_venue::parse_retry_after`:

- A zero (and data v2's `-0`) is no wait. `VenueError::retry_after`, `V2Error::retry_after` and
  `retry_after()` on `PerpsError` and `DataApiError` are `None` where they were `Some(0s)`.
- Perps reads a fractional value (`1.5` is 1.5 s; it was `None`).
- Core's retry floor honours a padded value (`" 2 "`; it was ignored).
- A data v2 value of `1e300` is `Duration::MAX` instead of a panic, and a perps value past `u64`
  is `Duration::MAX` instead of `None`.

The clamps are unchanged: core's `max_backoff_ms` on the wait the loop sleeps, 3 days on
Binance's, and none on the value an error surfaces.

### Health pings

Every `ping()` keeps its signature and returns the round trip of the attempt that answered, timed
from its signing, so it leaves out the waits for a permit, the throttle and a retry's backoff.
That is new for perps, Binance and relay; a retried ping reports its last attempt. The clob,
gamma and data pings, and gamma's `query_by_information` and `query_abridged`, now run on the
loop (DRIFT R8): a 429 or 425 is retried, a 429 holds the client, the clob ping waits for a permit
and the IP throttle, which it skipped, and gamma's `post_json` takes the permit before the
throttle. Perps' and Binance's pings no longer log `Request failed` on a non-2xx; the error they
return is unchanged.

### Logging

Retries, holds and decode failures now log under the `polyoxide_core` target, whichever crate
sent the request, and the supervised sockets' reconnects under `polyoxide_ws`.

| Line | 0.38 | 0.39 |
|---|---|---|
| Retry WARN | `polyoxide_core` for gamma and data, `polyoxide_clob` and `polyoxide_relay` for theirs; Binance `429 on <path>, retry <n> after <cooldown>` | `polyoxide_core`, `Retriable status <code> on <path>, retry <n> after <ms>ms`, for every client; the wait shown is the longer of the sleep and the hold; relay names the joined path (`/submit`), where it named the endpoint (`submit`) |
| Hold without a retry (DRIFT R10) | Binance only, `429 on <path>, no retry left: …` | `polyoxide_core`, `Status <code> on <path>, not retried: every request held <ms>ms`, for every client |
| Binance 418 | `polyoxide_binance`, `418 on <path>: IP banned, every request held <ban>: <body>` | `polyoxide_binance`, `418: IP banned: <body>`, plus core's hold line with the path |
| Decode failure | core and clob logged two ERROR lines (`Deserialization failed`, `Failed to deserialize`); clob, relay and Binance logged under their own targets | `polyoxide_core`, one `Failed to decode <path>: <err>: <body>` |
| `Request failed: <error>` ERROR | `polyoxide_core` for gamma and data, `polyoxide_clob` for clob | `polyoxide_core`, Binance included; the error's `Debug` lists header names without values and truncates message and body |
| Perps reconnects | `polyoxide_perps` | `polyoxide_ws`: `WebSocket connection lost, reconnecting`, `… stale, reconnecting`, `… closed, reconnecting`, `WebSocket reconnect failed, retrying`; the rate-limited subscribe's retry stays under `polyoxide_perps` |
| Binance reconnects | `polyoxide_binance`, `binance stream lost, reconnecting` with `path` | `polyoxide_ws`, the same wording as perps and without the path; `Event::Disconnected { path }` still carries it |
| rtds server close (bare tier, DEBUG) | `polyoxide_rtds` | `polyoxide_ws` |

Relay's POST failure ERROR (`Request to <endpoint> failed with status …`), rtds' and sports' own
reconnect lines stay under their crates.

## Builders, requests and types

### Builders

Every venue builder holds a `polyoxide_core::ClientConfig`, and its five transport setters
(`base_url`, `timeout_ms`, `pool_size`, `with_retry_config`, `max_concurrent`) keep their names
and signatures, now from `polyoxide_core::client_config_setters!`. `RelayClientBuilder` gains
`base_url`, `timeout_ms` and `pool_size` (defaults 30 s and 10 idle connections, as before).
Relay then has two setters for its base URL: `url(&str) -> Result` checks the URL at once, and
`base_url` leaves the check to `build`. Both add the trailing slash, so a path prefix is kept;
use one. Binance's builder no longer calls `.gzip(true)`: core enables reqwest's `gzip` feature,
so every client still asks for gzip.

### Requests and namespaces

- **clob**: every namespace method returns `polyoxide_core::Request<T, ClobError>` in place of
  `polyoxide_clob::request::Request<T>`, with the same `query`, `send` and `send_raw`. Only code
  that names the type changes. The `request` module and `AuthMode` are gone.
- **Binance**: the eight methods that returned `WeightedRequest<T>` return per-route builders
  (`GetTime`, `GetExchangeInfo`, `GetFundingInfo`, `GetTicker24h`, `GetTickers24h`,
  `GetPremiumIndex`, `GetPremiumIndices`, `GetOpenInterest`) with the same `cost()` and `send()`.
  `WeightBudget` implements core's `Throttle`, and `weight::{WEIGHT_LAYER, FUNDING_LAYER}` and
  `From<weight::Cost> for polyoxide_core::Cost` are new.
- Namespace accessors come from the new `polyoxide_core::namespaces!`, and query setters from
  the new `polyoxide_core::query_setters!` and `polyoxide_core::csv`. Names, argument types and
  query keys are unchanged, with one exception: data v1's comma-joined setters (`market`,
  `eventId` and the like on the users, trades, open-interest and combos builders,
  `ListActivity::activity_type`, `ListComboPositions::status`) omit the parameter when the joined
  value is empty, so exactly `[""]` sends nothing where it sent `key=`.

### Wire enums

- `polyoxide_venue::UnknownVariant` is the one `FromStr` error, with the same fields, message and
  class as the perps and Binance copies it replaces. It is the `FromStr::Err` of perps' seven
  enums and `ws::Channel`, and of Binance's `Interval` and `DepthLimit`.
- Data's ten closed v2 enums (`TimePeriod`, `ComboPositionStatus`, `PositionSortBy`,
  `ComboPositionSortBy`, `ActivitySortBy`, `FilterType`, `LeaderboardBoard`, `PnlInterval`,
  `PnlFidelity`, `PricesInterval`): `FromStr::Err` goes from `String` (`unknown X: v`) to
  `UnknownVariant` (`"v" is not a valid X`), and `as_str(&self)` becomes `as_str(self)`. A method
  call is unchanged, but `X::as_str` passed where a `fn(&X)` is expected no longer compiles.
- Additive: relay's `TransactionState`, `SessionSignerAuthorizationStatus` and
  `SessionSignerRevocationStatus` gain `ALL`; gamma's `ProtocolVersion`, `ResolutionStatus` and
  `HomeAway`, data's four open v2 enums and Binance's `ContractType`, `SymbolStatus` and
  `UnderlyingType` gain `from_wire`. Wire spellings, derives and serde are unchanged.
- The macros themselves, `polyoxide_venue::{wire_enum!, open_enum!, specta_as_string!}`, are
  public for your own enums.

### Decimals, time and credentials

- `polyoxide-venue`'s `decimal` feature adds `positional::{DecimalStr, element, drain}`. Perps'
  positional rows (`Kline`, `MarkPoint`, `Level`) decode their decimals as every other perps
  decimal does, so `"1e-5"` is accepted where it was refused, with the same arity; a refusal's
  text changes. Binance's rows decode as before.
- `polyoxide_venue::UnixMillis(pub u64)` and `UnixMillis::now()` are new, with no serde. Every
  clock that reads it returns what it did, `0` before 1970 included.
- Clob's `Account` and relay's `RelayClient`, `RelayClientBuilder` and `BuilderAccount` hold their
  credentials in `polyoxide_venue::Secret`, so their `Debug` prints `Secret([REDACTED])` where it
  printed the credential type's redacted fields. `Credentials` and `BuilderConfig` keep their
  public `String` fields and serde, `Account::credentials()` still returns `&Credentials`, and
  `BuilderAccount::auth_config()` still returns `Option<&AuthConfig>`.

### reqwest and features

`polyoxide-core` is the only crate that depends on reqwest (0.12, features `json`, `rustls-tls`
and `gzip`, no defaults), and re-exports it as `polyoxide_core::reqwest`. gamma, data, perps,
clob, relay and Binance no longer declare it; the reqwest types in their signatures (clob's
`send_raw` returning `Response`, relay's `HeaderMap`) are the same types. If you name reqwest
types, depend on reqwest 0.12 yourself or use `polyoxide_core::reqwest`. If you relied on a
polyoxide crate turning on a reqwest feature in your build, you now get core's set and no other.
No header any client sends changes.

The socket crates' transport belongs to `polyoxide-ws`: perps, Binance, rtds, sports and clob
declare neither tokio-tungstenite nor rustls themselves. Name tungstenite's types through
`polyoxide_ws::tungstenite`. If you open your own tungstenite sockets beside polyoxide's, call
`polyoxide_ws::ensure_crypto_provider()` first, as the live suites now do (DRIFT R9).

The scripted test servers moved to `polyoxide_ws::test_server` (feature `test-server`,
doc-hidden). Perps' `test-server` feature is gone; Binance's and sports' `test-server` and rtds'
`test-fixtures` stay, for their captured frames.

## Sockets

### What every socket crate shares

Each socket error wraps `polyoxide_ws::WsError` where it boxed `tungstenite::Error`:
`WebSocketError::Connection` (clob), `PerpsWsError::Connection`, `UsdmWsError::Connect`,
`RtdsError::Connection`, and `SportsError::{Connect, Transport}`'s `source`. `WsError` is opaque:
read it through `kind()`, whose `#[non_exhaustive]` `polyoxide_ws::Kind` has `Io`, `Protocol`,
`ConnectionClosed`, `Closed { code, reason }`, `Refused { status, retry_after }`, `Url`,
`HttpFormat`, `Tls`, `AttackAttempt`, `Capacity`, `AlreadyClosed`, `ConnectTimeout { after }`,
`NoAnswer { after }` and `Other`.

```rust
// 0.38
WebSocketError::Connection(err) => match *err {
    tungstenite::Error::Http(ref response) if response.status() == 429 => …,
    _ => …,
},

// 0.39
WebSocketError::Connection(err) => match err.kind() {
    polyoxide_ws::Kind::Refused { status: 429, retry_after } => …, // retry_after: Option<&str>
    _ => …,
},
```

For most decisions `Classify` is enough and needs no `polyoxide-ws` dependency. `From<tungstenite::Error>`
is gone from `WebSocketError`, `PerpsWsError`, `UsdmWsError` and `RtdsError`; build one with
`WebSocketError::from(polyoxide_ws::WsError::from(err))`, and a sports error with
`SportsError::Transport { source: WsError::from(err) }`. Display text is unchanged, since a
transport `WsError` displays tungstenite's own text. `Debug` changes:
`Connection(WsError(Transport(..)))` where it was `Connection(..)`.

Classification is one table for every socket crate. A refused upgrade follows the HTTP status rule
(a 418 or 451 is `Restricted`), a server close follows the close-code map in the
[`Classify`](#classify) table, `AlreadyClosed`, `Capacity`, `Url`, `Tls`, `HttpFormat` and
`AttackAttempt` are `InvalidRequest`, and the rest `Network`. Two changes reach every socket
error: an upgrade refused with 451 is not a fault, and `retry_after()` is any refused upgrade's
`Retry-After`, unclamped and a zero `None`, though only a 429's class carries it.

The behaviour changes, by crate:

| | perps | Binance | rtds | sports | clob |
|---|---|---|---|---|---|
| R1: reconnect retries 408, 425, 429, 5xx refusals | new (adds 408, 425) | as before | new | as before | no reconnect |
| R2: 10 s connect timeout | new | as before | new | as before | as before (per address) |
| R3: close reply sent | new | new (bare tier) | new | as before | new |
| R11: reconnect waits out `Retry-After` | new | new | new | new | no reconnect |
| 451 refusal not a fault | new | new | new | new | new |

### perps

Kept: `PerpsWsBuilder` and `connect(channels)`, which still refuses an empty list with
`EmptySubscription` before connecting; the setters keep their names and gain `connect_timeout`.
`SupervisedPerpsWs` is now `pub type SupervisedPerpsWs = polyoxide_ws::Supervised<PerpsProtocol>`,
with the same `Stream` item, `membership()` and `close()`. `membership()` returns
`polyoxide_ws::MembershipHandle<PerpsProtocol>`, whose `subscribe` and `unsubscribe` come from the
trait `polyoxide_perps::ws::PerpsMembership` and answer as before (`Refused`, `Stalled`,
`Stopped`); `apply` takes a `MembershipChange`. `polyoxide::prelude` re-exports `PerpsMembership`
under `perps-ws`.

```rust
use polyoxide_perps::ws::{PerpsMembership, PerpsWsBuilder};

let ws = PerpsWsBuilder::new().connect(channels).await?;
ws.membership().subscribe([more]).await?; // needs the trait in scope
```

Removed: the `ws::supervised` module, `ws::MembershipHandle`, `ws::Recovery` and
`PerpsWsError::recovery()`. The stream's own reconnect decision is internal now; read the class
through `Classify` to decide for yourself.

Behaviour, beyond the table: the application ping goes out on the wall clock, without awaiting
its pong and also while the 1024-event queue is full, and time spent waiting on a full queue no
longer counts as silence. Any text frame still counts as liveness, and a pong that is not `ok`
still reconnects. While the queue is full the client reads nothing, so it answers none of the
host's 10 s protocol pings; the host closed such a client with `1011 pong_timeout` after about
70 s, which reconnects with the queued events kept. An empty membership stays wanted: unsubscribing
every channel keeps the connection, and a reconnect replays nothing.

### Binance

Kept at `usdm::ws`: `UsdmWsBuilder` (setters unchanged), `SupervisedUsdmWs`, `MembershipHandle`,
`Event`, `DisconnectReason` and `UsdmWs`. `UsdmWsError::Connect` wraps `WsError`; `From<WsError>`
sends the kit's server close to `Closed` and its connect timeout to `ConnectTimeout`, and the
handshake 403 is still `Restricted`. `Recovery` and `UsdmWsError::recovery()` are gone: match
`UsdmWsError::Frame { .. }` for the one error the stream yields and goes on past, where you read
`SkipFrame`. `UsdmWsError::NoAnswer { id: 0, .. }` also means a replay the server left unanswered
for `stale_after`.

`UsdmWsError::Stopped` carries the error its path stopped on:

```rust
// 0.38
Err(UsdmWsError::Stopped) => …,

// 0.39
Err(UsdmWsError::Stopped { cause }) => {
    if let Some(cause) = cause {
        // cause.class, cause.is_fault, cause.retry_after, cause.message
    }
}
```

Its class, fault and wait are the cause's, and its text appends the cause's, so a stream stopped
by a 451 refusal is not a fault. The cause is recorded once the router has the path's stop: a
change in flight when the path stops, or a `close()` made right after the stopping error, can get
`Stopped { cause: None }` or `Ok`.

Behaviour: a change the server leaves unanswered for `stale_after` is a
`DisconnectReason::Stale` outage (or `Closed` when the connection fails under it), not
`Error(NoAnswer)`, and its caller still gets `Ok`; the change is replayed. A slow consumer never
goes stale, since pings continue, but the host may close it after its own pong limit (10 min,
documented), which reconnects. A rotation resets the backoff after a connection that delivered.
A replay is bounded as a whole by `stale_after`, so `stale_after` must exceed its pacing, 200 ms
per 200 streams after the first 200. The bare `UsdmWs` ends after the first transport error it
yields; a control call after that sends nothing and fails with `Connect` of kind `AlreadyClosed`,
or `Closed` after a server close.

### rtds

`RtdsError::Connection` wraps `WsError`; a connect timeout is a `Connection` whose kind is
`ConnectTimeout`. `Recovery` and `recovery()` stay: `AlreadyClosed` is fatal, `Capacity`
reconnects, and a refused handshake follows the status rule, so a 408, 425, 429 or 5xx now
reconnects. `RtdsBuilder::connect_timeout` is new (10 s; the bare `Rtds::connect` uses 10 s).
The bare `Rtds` ends after the first transport error it yields.

### sports

`SportsError::{Connect, Transport}` carry `WsError` in `source`:

```rust
// 0.38
SportsError::Connect { source } => match *source {
    tungstenite::Error::Http(ref response) => …,
    _ => …,
},

// 0.39
SportsError::Connect { source } => match source.kind() {
    polyoxide_ws::Kind::Refused { status, .. } => …,
    _ => …,
},
```

A connect that fails on `Capacity` is still retried. sports no longer depends on rustls or
tokio-tungstenite.

### clob

`WebSocketError` is `#[non_exhaustive]`, and `Connection` holds `WsError`. A host that resolves to
no address is now `Connection` of kind `Io`, classed `Network`, with the text `WebSocket connection
error: IO error: no addresses resolved for <url>`, where it was `InvalidMessage`, classed
`InvalidRequest`. A 429 refusal's class carries its `Retry-After`. The server's close is answered
by `WebSocket` and by `WebSocketWithPing::run`, which returns `Ok`; `close()` returns `Ok` after a
server close; and the stream ends after the first error it yields. clob does not reconnect, so R1
and R11 do not apply.

## Known gaps in 0.39

- `PerpsWsError::Stopped` carries no cause: a perps stream stopped by a 451 refusal reports
  `Stopped`, `InvalidRequest`, a fault.
- `RtdsError::Server { status_code: 451, .. }`, a subscription rejected with 451, is still a
  fault.
- Binance's reconnect log lines do not name the path; the outage events do.
- Python exceptions other than Data API v2's leave `status` and `retry_after` as `None`, though
  the Rust error carries both.
