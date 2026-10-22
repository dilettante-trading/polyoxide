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
3. A retry releases the permit and sleeps the longer of the policy's wait and the loop's floor,
   `RetryConfig::retry_delay(attempt)`, never past `max_retries`.

- A transport error is neither observed nor retried. A refused cost returns `ApiError::Refused`
  with nothing sent.
- `Done` returns the response whatever its status. `Fail`, or a retry with none left, returns
  `ApiError::Response` (AD-8) after the hold and its WARN, reading the body after the attempt's
  permit is released. `ErrorResponse`'s `Debug` names the headers without their values and
  truncates the message and body, since the `Request failed` ERROR line prints it.
- `send_raw`, `get_bytes` and `health` turn a non-2xx `Done`, which only a custom policy produces,
  into the same error, so a failed response never reaches a caller as data.
- `HttpClient` holds one `Arc<DynThrottle<'static>>` and one `Arc<DynRetryPolicy<'static>>`
  (dynosaur), shared with its `with_base_url` siblings, so a 429 on one host holds them all.
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
that answered, timed from its signing by a core-private authenticator, so it leaves out the
permit, throttle and backoff waits; a final non-2xx is `ApiError::Response` as the caller's error.
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

## reqwest, gzip and the header pins

- Core is the only member that declares reqwest (AD-18): it enables `json`, `rustls-tls` and
  `gzip` with no default features and re-exports the crate as `polyoxide_core::reqwest`, through
  which every other member's src, tests and examples name its types. No crate but core sends
  through `HttpClient`'s reqwest client. `test_dependency_fences.py` fails on any other member's
  direct dependency, so no crate can change reqwest's features for the rest.
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
