---
paths:
  - "polyoxide-core/**"
  - "polyoxide-polymarket/src/shared/{mod,policy,auth}.rs"
  - "polyoxide-polymarket/src/clob/authenticator.rs"
  - "polyoxide-polymarket/src/{clob,gamma,data,relay,perps}/client.rs"
  - "polyoxide-binance/src/usdm/{policy,request}.rs"
  - "polyoxide-kalshi/src/predictions/{auth,request}.rs"
  - "**/tests/{send_loop,health,mock_request}.rs"
  - "**/tests/headers_*.rs"
---

# The send loop

`HttpClient::send` (`polyoxide-core/src/send.rs`) is core's only retry loop; `Request::send_raw`,
`get_bytes` and `health` run on it. Never add a second retry or sleep loop, or bring back
`HttpClient::should_retry`, `note_rate_limited` or `acquire_rate_limit` (removed,
`docs/s1-removals.md`). `acquire_concurrency` stays public, because tests in core, gamma, data,
perps, clob, relay and Binance observe the permit through it. The loop's own suite,
`polyoxide-polymarket/tests/send_loop.rs`, runs on Polymarket's policy.

Core and its loop name no venue. A venue's retry policy, rate-limit tables, throttle and signing
live in its crate. Polymarket's live in `polyoxide_polymarket::shared`, each item built by the
features of the modules that use it: `PolymarketRetryPolicy` by any of `clob`, `gamma`, `data`,
`relay` and `perps`; each `*_limits` table by its own module's; `ClobThrottle`, `signer_cost` and
the signer layer by `clob`; the HMAC `Signer` and the session-signer types by `clob` or `relay`.

## One attempt

1. Take a concurrency permit.
2. Run the client's hooks (`polyoxide-core/src/hooks.rs`): `Throttle::acquire` charges the attempt
   (a `RateLimiter` counts one request, found by method and path; a `Cost` charges any other
   layer), an optional `Authenticator` signs it, it is sent, `Throttle::observe` sees the
   response, and `RetryPolicy::decide` returns `Done`, `Retry(wait)` or `Fail` with an optional
   hold, which is applied whatever the outcome.
3. A retry releases the permit and sleeps, on the client's runtime (`HttpClient::runtime()`), the
   longer of the policy's wait and the loop's floor, `RetryConfig::retry_delay(attempt)`, never
   past `max_retries`.

- A transport error is neither observed nor retried. A refused cost returns `ApiError::Refused`
  with nothing sent.
- `Done` returns the response whatever its status. `Fail`, or a retry with none left, returns
  `ApiError::Response` (AD-8) after the hold and its WARN, reading the body after the attempt's
  permit is released. `ErrorResponse`'s `Debug` names the headers without their values and
  truncates the message and body, since the `Request failed` ERROR line prints it.
- `send_raw`, `get_bytes` and `health` turn a non-2xx `Done`, which only a custom policy produces,
  into the same error, so a failed response never reaches a caller as data.
- `HttpClient` holds one `Arc<DynThrottle<'static>>` and one `Arc<DynRetryPolicy<'static>>`
  (dynosaur), shared with its `with_base_url` siblings, so a 429 on one host holds them all. It
  holds its `Arc<dyn polyoxide_rt::Runtime>` the same way, which the retry sleep and `health`'s
  stopwatch go through.
- A signing failure is `ApiError::Sign`; `ClobError`'s hand-written `From<ApiError>` takes it back
  out, as it turns a signer-layer `ApiError::Refused` into `BurstCapacityExceeded`.
- Core's `Request::send` (which clob's and Binance's requests go through) and relay's `post_json`
  log a decode failure as one ERROR line through `polyoxide_core::decode_json`.

## Requests per venue

- Core's `Request<T, E>` is the one request builder: a method, a JSON body serialised once
  through `serde_json::Value`, an `Authenticator` and the `Cost`s a route charges. Clob's
  namespaces return `Request<T, ClobError>`; its L1, L1-signed and L2 authenticators are in the
  private `polyoxide-polymarket/src/clob/authenticator.rs`; its trading requests carry
  `shared::signer_cost`.
- Clob's client installs `shared::ClobThrottle` and `PolymarketRetryPolicy`, and keeps a handle on
  the throttle's `SignerLimiter` for `tier()` and `rate_limit_status()`.
- Gamma's `post_json` decodes its own body, so it calls `HttpClient::send` directly (DRIFT R8).
- Relay calls `HttpClient::send` through one private helper that reads no body itself: a failure
  is the loop's `ApiError::Response` as `RelayError::Api`, and a POST's is also logged at ERROR.
  It signs with `RelayGetAuth` (the route's own path) or `RelayPostAuth` (the joined path and
  body, then the caller's extra headers, then `Content-Type`). Its auth checks and allow-lists
  run before the send, so a refused call spends no permit and no token. The two session-signer
  posts bound each attempt with `RequestParts::timeout` (300 s). Relay's alloy gas estimation goes
  to the chain's RPC node and is the one request outside the loop.
- Binance's route builders send core's `Request` over its `WeightBudget` throttle and
  `UsdmRetryPolicy`.

## Pings

Every venue's `ping` calls `HttpClient::health(path, costs)` (`polyoxide-core/src/health.rs`): one
`GET` on the send loop that returns a `Pong { round_trip, response }`. `round_trip` is the attempt
that answered, timed on the client's runtime clock from its signing by a core-private
authenticator, so it leaves out the permit, throttle and backoff waits; a final non-2xx is
`ApiError::Response` as the caller's error.
Each ping keeps its path (gamma `/status`; data, clob and relay their base URL's path, so relay
keeps a prefix; perps `/v1/info/ping`; Binance `/fapi/v1/ping`, costed from `Route::Ping`) and its
body check (perps `status == "ok"`, Binance `{}`).

## Retry policies

The loop retries only what the client's policy decides; callers decide with
`Classify::is_retriable()` (`errors.md`).

- Core's `DefaultRetryPolicy` retries `429`. A client built straight on `HttpClientBuilder` gets
  it, so it does not retry a `425`.
- `polyoxide_polymarket::shared::PolymarketRetryPolicy`, which gamma, data, perps, clob and relay
  install, retries `429` and `425`.
- Binance's `UsdmRetryPolicy` retries a `429` and never a `425` (`binance.md`).
- No policy retries 5xx or 408 (AD-17).

## The transport

`HttpClient` sends every attempt through a private `Arc<dyn HttpTransport>`
(`polyoxide-core/src/transport.rs`). The loop builds an `HttpRequest`: the base URL joined with
the path and the query form-urlencoded onto it as reqwest's `.query()` did, the method, the
signed headers, the body, and a `timeout` that is always set (the request's own, else the
client's) and bounds the whole exchange, body included. The transport answers with a `Response`,
an owned head and a lazy body read by `bytes()`, `text()` and `json()`, each into `ApiError`. The
loop decides on the head, releases the permit, then reads an error body.

- `ClientConfig::http_transport`, the `http_transport(..)` setter of `client_config_setters!` and
  `HttpClientBuilder::http_transport(..)` inject one. With none set, `build` makes
  `ReqwestTransport` from the builder's timeout, pool size and `gzip` (unset), a 10 s connect
  timeout and no redirects. `pool_size` and `gzip` feed only that native default.
- A transport that cannot produce a response is `ApiError::Transport(TransportError)`, neither
  observed nor retried. `TransportError::kind()` is `Connect`, `Timeout`, `Body`, `Request` or
  `Other`; only `Request` is `InvalidRequest`, so a body that broke off mid-read is `Network`.
  `ReqwestTransport` maps a reqwest error through one private function: no foreign-error `From`
  impl exists (`errors.md`).
- `Pong::response`, `Request::send_raw` and clob's `ListOrders::send_raw` return core's
  `Response`, and `ErrorResponse::read`, `ApiError::from_response` and `retry_after_header` take
  its head.
- `polyoxide-core/tests/transport_injection.rs` drives a retry, a hold, the permit rule and a
  transport failure through a recording transport.

## reqwest, gzip and the header pins

- Core is the only member that declares reqwest (AD-18): it enables `json`, `rustls-tls` and
  `gzip` with no default features and re-exports the crate as `polyoxide_core::reqwest`, through
  which every other member's src, tests and examples name its client types (`Client`, `Response`,
  `RequestBuilder`). The request vocabulary, `Method`, `StatusCode`, `HeaderMap`, `HeaderValue`
  and `header::*`, they name through `polyoxide_core::http`, core's re-export of the `http` crate:
  the very types reqwest re-exports, so a path change only. Nothing but `ReqwestTransport` sends
  through reqwest: `HttpClient` holds a transport (`## The transport`).
  `test_dependency_fences.py` fails on any other member's direct dependency, so no crate can
  change reqwest's features for the rest.
- `HttpClientBuilder::gzip` is unset by default, which leaves reqwest's default: every client asks
  for gzip (`exchangeInfo` is 1.15 MB raw, 51 KB gzipped). Never call `.gzip(...)` on a client
  builder or give `ClientConfig` a gzip knob: 0.37.0 forced it off and took compression away from
  consumers whose workspace enabled the feature (prader-rs among them) until 0.37.1 restored it.
  Only four rate-limit examples pin `.gzip(false)`, on bare clients, so their soaks keep the wire
  they were measured on.
- Each HTTP module has a `tests/headers_<module>.rs` (polymarket's `clob`, `gamma`, `data`,
  `relay` and `perps`, Binance's `usdm`, Kalshi's `predictions`) that sends one unauthenticated
  `GET` through `polyoxide_test_support::query::headers_sent` and asserts exactly `accept: */*`,
  `accept-encoding: gzip` and `host`. CI runs it in the module's minimal build, in the workspace
  build and beside `polyoxide/full`, so a feature another member or another venue switches on
  fails one of the three.

## The browser's fetch (`polyoxide-core/src/fetch.rs`)

`FetchTransport` is the `wasm32` default of the config's `http_transport`; a host's is `ReqwestTransport`. It sends through reqwest's wasm client (the page's `fetch`) with the request's timeout set per request, which bounds the whole exchange, body included. A `fetch` that fails is `Connect` (a network failure and a CORS refusal look alike to a page), a failed body read is `Body`, a timeout is `Timeout` wherever it struck, and a request the browser would not build is `Request`; `kind_of` holds the rule and the host's tests pin it. The pool size and the gzip setting feed only the host default: the browser negotiates compression itself, follows redirects, and shows a page only the response headers the server exposes, so a `Retry-After` is unreadable unless it is. The header pins of the `headers_<module>` suites hold the host default only. The exchange sits in a `SendWrapper`, as in `rt.md`.
