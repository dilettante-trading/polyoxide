---
paths:
  - "**/src/**/error.rs"
  - "**/src/lib.rs"
  - "polyoxide-venue/src/{class,status,retry_after,socket,ws_table}.rs"
  - "**/tests/error_classes_*.rs"
  - "**/tests/failure_tags*.rs"
  - "polyoxide-venue/tests/ws_classification.rs"
  - ".github/scripts/tests/test_classify_coverage.py"
---

# Errors and classification

## The hierarchy

- `ApiError` (core) is wrapped by each module's `<Module>Error`: `ClobError`, `GammaError`,
  `DataError`, `RelayError`, Binance's `UsdmError`, Kalshi's `PredictionsError`. A module's socket
  error is `<Module>WsError`. Every module's error enum is `#[non_exhaustive]`.
- An unsuccessful response is `ApiError::Response(Box<ErrorResponse>)`: the status, headers, body,
  the body's `error` or `message` field as `message`, and the unclamped `Retry-After` (a zero is
  `None`). `ApiError::Validation` is a refusal made before sending, and only that.
- Each module's `From<ApiError>` is its one decode (D14), so a `?` on core's error decodes the same
  way anywhere:
  - gamma's and relay's are the derived `#[from]`;
  - data's turns a v2 envelope into `V2`;
  - perps' turns a `{status: "err", error, ref}` body into `Venue`;
  - clob's turns a 400 kill into `FakUnmatched` or `FokUnfilled`, a 400 with
    `/v1/heartbeats`' `{error_msg, heartbeat_id}` body into `HeartbeatIdMismatch`, a refused
    signer cost into `BurstCapacityExceeded` (`polyoxide_polymarket::shared`, the signer layer's
    refusal), and a `Sign` back into its clob error;
  - Binance's goes by status first (`418`, `429`, `451`, `403`), then its `{code, msg}` body;
  - Kalshi's turns an unsuccessful response whose body has one of its three shapes
    (`{"error": {"code", "message", "details"}}`, `{"error": "<text>"}`, `{"msg": "<text>"}`) into
    `Venue`, and keeps any other as core's `Api`.
- Foreign errors enter a module only through `ApiError`, by `.map_err(ApiError::from)?` or core's
  own paths: no module implements `From<reqwest::Error>`, `From<url::ParseError>` or
  `From<serde_json::Error>` (core's `impl_api_error_conversions!` is gone).
- `RequestError` is a marker for any `From<ApiError> + Debug` type.
- `RelayError::Api` wraps `ApiError` (DRIFT R7): a relayer failure is classed by its status; a
  transport, URL or JSON failure is `Api(Network | Url | Serialization)`; a refusal made before
  sending (validation, missing auth, a header that could not be built) is `Api(Validation)`
  through `RelayError::validation`, as clob's is.

## `Classify`

Every error implements `polyoxide_venue::Classify` (import it from there; no crate re-exports it),
inner and auxiliary types included: `V2Error`, `VenueError`, `BurstCapacityExceeded`, `Refused`,
`ParseTickSizeError`, Kalshi's `InvalidTicker` and `CredentialsError`, `polyoxide-venue`'s
`UnknownVariant` and `UnservedInterval`, `InvalidSymbol`, `InvalidStreamName`.

- `class()` is one of the eight `Class` variants of AD-15, decided by the status before the body.
  `Network`, `Unavailable` and `RateLimited` are retriable (`is_retriable()`): connection failures
  and timeouts, `408`, `425 Too Early`, `429` and 5xx. The class decides retriability for callers;
  the send loop retries only what its policy decides (`send-loop.md`).
- `is_fault()` is false only where the venue answered as designed: the FAK and FOK kills, clob's
  heartbeat ID mismatch, and a 451 region block wherever a status decides the class.
- `retry_after()` is the wait the server asked for.
- No error has an inherent `is_retriable` or `retry_after`, the socket errors included. Data's v2
  `retryable` flag is surfaced on `V2Error`, not obeyed. Binance's `418`, `451` and firewall `403`
  are `Restricted` (D14), not retriable.
- `ApiError::Response` is classed by its status (any status outside 400–599 is a `Decode`, such as
  a ping's 2xx whose body says it is not ok), with a `429`'s `Retry-After` as its wait and every
  status's `Retry-After` as `retry_after()`. `ApiError::Validation` and `Sign` are
  `InvalidRequest`.
- A failure of clob's Gamma dependency is `ClobError::Gamma`, classed as the Gamma error; a Data
  client clob could not build is `ClobError::Data`, classed as the Data error. A reqwest error
  that broke off mid-body is `Network`, though reqwest labels it a decode error.
- Each crate's `lib.rs` lists its error types in a `const _` assertion, gated like the types
  (`polyoxide-venue`'s names the trait `crate::Classify`), so a missing impl fails the build.
  `.github/scripts/tests/test_classify_coverage.py` fails when a public error type is missing from
  its crate's list (a venue crate gates one per module).
- Exempt from the sweep: venue's `ClassifiedError`, which must not implement the trait, and
  `polyoxide-ws`'s `WsError` (below).

## `Retry-After`

`polyoxide-venue` holds the status and close-code maps and the one `Retry-After` parser,
`parse_retry_after`. Every read goes through it with its caller's clamp: core's `max_backoff_ms`
for the wait the loop sleeps; none for core's `ErrorResponse::retry_after`, the wait an error
surfaces, which perps' `VenueError` and data's `V2Error` copy; Binance's 3 days for every arm of
its decode, a core `Response` it keeps included. A zero is no wait anywhere (DRIFT R4).

## Socket errors

- Venue does not depend on ws (AD-2) and ws does not depend on venue, so neither can implement the
  trait for `WsError`, and in a venue crate both trait and type are foreign (the orphan rule).
  Each venue error that wraps it classifies it through `polyoxide_venue::impl_ws_classification!()`,
  which expands in the invoking module to private `ws_class`, `ws_is_fault` and `ws_retry_after`
  over `polyoxide_ws::WsError`'s `Kind`; the venue's `Classify` impl calls them on its `WsError`
  arm.
- The wrappers: `PerpsWsError::Connection`, `UsdmWsError::Connect`,
  `PredictionsWsError::Connection`, `RtdsError::Connection`, `ClobWsError::Connection` and
  `SportsError::{Connect, Transport}`. Binance and sports map the kit's server close and connect
  timeout to their own `Closed` and `ConnectTimeout`, and Binance keeps its handshake `403` →
  `Restricted` row ahead of the table (D14). Clob maps a timed-out connect step to its own
  `ConnectTimeout { url, timeout }`, and a host that resolves to no address is an I/O failure,
  `Network`. So a 451 refusal is each one's one transport non-fault.
- No crate matches over `tungstenite::Error`: every socket error is on the one socket table.
- A refused upgrade follows `class_for_handshake_status` with its `Retry-After` read unclamped; a
  server close follows `class_for_close_code`; only a 451 refusal is not a fault. Every refusal's
  `Retry-After`, whatever its status, is the error's `retry_after()`, as HTTP's `ErrorResponse`
  surfaces it, though only a 429's class carries it. Each recovery retries a handshake refused
  with 408, 425, 429 or 5xx through `polyoxide_ws::retrying_can_fix` (DRIFT R1).
- `polyoxide-venue/tests/ws_classification.rs` covers every row, and that
  `polyoxide_ws::retrying_can_fix`, given venue's status rule, agrees with the class on every
  refused status.
- rustc does not warn about dead code expanded from another crate's macro, so
  `test_classify_coverage.py` is the guard: a type with a `WsError` field must sit in a file that
  expands the macro, and that file must call all three functions in code.

The Python bindings map errors by class, one class to one exception (`py.md`).
