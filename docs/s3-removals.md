---
baseline: v0.40.0
---

# S3 removals

Every public item that stage S3 removes, against the S3 start tag above, the S2 release.
S3 is for additions (the venue traits, a supervised clob socket, the Kalshi skeleton), and
its removals are few, such as clob's never-constructed `ClobWsError::{ConnectionClosed,
Authentication}` and the wasm32 plan's breaks (`HttpClient::client`, `ApiError::Network` and
the re-typed HTTP responses). The story that removes an item adds its entry here when it
lands. Each removal goes with no shim or re-export (AD-16), so prader-rs and every other
consumer migrates from this list. [`s1-removals.md`](s1-removals.md) is S1's, and
S2's moves are in [`rename-manifest.md`](rename-manifest.md).

The commit after the S2 release moves the workspace to stage S3, and from it CI's removals
job runs `python3 scripts/api_removals.py check --baseline v0.40.0`, which reads this list
because the stage is S3. It fails on any removal that cargo-semver-checks reports and this
list does not hold, and on any path under "Doc-hidden paths consumers import" that no
longer compiles; the release's semver job imports the same paths. The list is cumulative
until the S3 release: an entry stays after its PR merges.

To remove a public item in a PR:

1. Run the command above, or read the removals job's log. It prints each unlisted removal as
   a key, `<crate> <lint id>: <item> (<file>)`, where the file is relative to the crate. A
   crate deleted from the workspace is keyed `<crate> crate_missing`.
2. Add each key under "Removed" as one list entry, in backticks and exactly as printed, then
   the story and what a consumer uses instead. An entry with nothing after its key is
   refused.
3. To remove a path listed under "Doc-hidden paths consumers import", append `**Removed**`
   to its entry, then the story and what a consumer uses instead. To remove one item of a
   `{...}` group, give it a line of its own first.

## Removed

- `polyoxide-core struct_pub_field_missing: field client of struct HttpClient (src/client.rs)` Story 4: `HttpClient` holds a private `Arc<dyn HttpTransport>`; send through `HttpClient::send`, and to send on a reqwest client of your own build `ReqwestTransport::new(client)` and inject it with `HttpClientBuilder::http_transport`, a builder's `http_transport` setter or `ClientConfig::http_transport`.
- `polyoxide-core enum_variant_missing: variant ApiError::Network (src/error.rs)` Story 4: a failure to get a response, or to read its body, is `ApiError::Transport(TransportError)`; where you matched `ApiError::Network(e) if e.is_timeout()` match `ApiError::Transport(e) if e.kind() == TransportKind::Timeout`, and likewise `is_connect` is `TransportKind::Connect`.
- `polyoxide-core function_missing: function polyoxide_core::error::classify_reqwest (src/error.rs)` Story 4: classify the error itself, `Classify::class` on `ApiError` or on `TransportError`, which holds the same rule: only a `TransportKind::Request` is `InvalidRequest`, every other kind `Network`.
- `polyoxide-core inherent_method_changed: HttpClient::send (src/send.rs)` Story 4: it returns `Result<polyoxide_core::Response, ApiError>`, an owned head and a lazy body with `status`, `headers`, `url`, `bytes`, `text` and `json`, the last three returning `ApiError`; a `reqwest::Response` is no longer produced.
- `polyoxide-core inherent_method_changed: Request::send_raw (src/request.rs)` Story 4: it returns `Result<polyoxide_core::Response, E>`, read as `HttpClient::send`'s response above.
- `polyoxide-polymarket inherent_method_changed: ListOrders::send_raw (src/clob/api/orders.rs)` Story 4: it returns `Result<polyoxide_core::Response, ClobError>`; `.text().await?` still reads the body, its error now `ApiError`.
- `polyoxide-core struct_pub_field_changed: field response of struct Pong (src/health.rs)` Story 4: it is a `polyoxide_core::Response`; `status()`, `headers()` and `text().await` read as before, the last returning `ApiError`.
- `polyoxide-core inherent_method_changed: ErrorResponse::read (src/error.rs)` Story 4: it takes a `polyoxide_core::Response`; read an error response from a raw reqwest one with `ErrorResponse::new(status, headers, body)`.
- `polyoxide-core inherent_method_changed: ApiError::from_response (src/error.rs)` Story 4: it takes a `polyoxide_core::Response`; from a raw reqwest response, build `ErrorResponse::new(status, headers, body)` and convert it with `ApiError::from`.
- `polyoxide-core function_changed: retry_after_header (src/client.rs)` Story 4: it takes `&HeaderMap`, not a response; pass `response.headers()`.

## Changes the gate does not key

cargo-semver-checks 0.51.0 reports no lint for the signature changes below (it counts
parameters and does not compare types or aliases; `Protocol::connect` also postdates the
baseline, since it first shipped in v0.41.0), so none has a key to list under "Removed"; the
removals job prints no line for them, and they are listed here for the consumer who migrates
from this file. Each is one of the wasm32 plan's Story 5 but the last, which is issue #87's.

- `polyoxide_ws::Socket` (a type alias, which semver-checks does not see): the alias is removed; a connection is a `polyoxide_ws::BoxSocket`, which `connect` and `connect_over` return and `Connection::new` takes.
- `polyoxide_ws::connect`: its return is re-typed, from `Socket` to `BoxSocket`; it dials through the target's default `SocketTransport` and runtime.
- `polyoxide_ws::connect_over`: its return is re-typed, from `Socket` to `BoxSocket`, and it is native only.
- `polyoxide_ws::Connection::new`: its `socket` parameter is re-typed, from `Socket` to `BoxSocket`; `Connection::with_runtime` also takes the runtime.
- `polyoxide_ws::Connection::last_inbound`: its return is re-typed, from `tokio::time::Instant` to `polyoxide_rt::Instant`, which is `std::time::Instant` on native.
- `polyoxide_ws::Protocol::connect`: its parameters change, from `(timeout)` to `(transport, runtime, timeout)`, and its return, from `Socket` to `BoxSocket`; a protocol that dialled for itself chooses a transport that makes its dial.
- Auto traits, which `auto_trait_impl_removed` does report as a major change and the removals gate allows: `polyoxide_ws::Connection` and every type holding one (`WebSocket`, `WebSocketWithPing`, `UsdmWs`, `Rtds`, `SupervisedRtds`, `SupervisedSportsWs`, `PerpsWs`, `SportsWs`) are no longer `Sync`, since a `BoxSocket` is `Send + Unpin` only; send one to another task, and share it through a `Mutex`.
- Auto traits: `polyoxide_ws::Supervisor` and `Supervised` are no longer `UnwindSafe` or `RefUnwindSafe`, since they hold a `dyn Runtime`, and likewise the builders and clients that hold a runtime or transport (such as `UsdmWsBuilder`, `PerpsBuilder`, `RtdsBuilder`, `WeightBudget`); wrap one in `AssertUnwindSafe` where a `catch_unwind` needs it.
- `polyoxide_core::RequestParts::body` (#87): the field is re-typed, from `Option<String>` to `Option<bytes::Bytes>`, so a body need not be text. Set it with `Some(text.into())`: a `String`, a `Vec<u8>` and a `&'static [u8]` each convert. `parts.body.as_deref()` is now an `Option<&[u8]>`; an authenticator that signs the body reads its bytes. `Request::body` and every venue request send, and sign, the bytes they did before.

## Doc-hidden paths consumers import

cargo-semver-checks treats `#[doc(hidden)]` items as private, and 0.51.0 does not check type
aliases, so it never reports their removal. The removals job imports each item below, and
the release's semver job does too. Each entry names the features it needs after its path,
and the entries of one crate that name the same features are built together, with those
features and no others, so an entry cannot compile through another's. `a::b::{C, D}` lists
`a::b::C` and `a::b::D`.

The items are the ones the CLI's tests and each crate's own tests import, at their S2 paths:
the live entries of [`rename-manifest.md`](rename-manifest.md)'s list at the S2 release.

- `polyoxide_binance::usdm::ws::fixtures::{AGG_TRADE, ALL, ALL_MARK_PRICES, ALL_TICKERS, BOOK_TICKER, KLINE, MARK_PRICE, PARTIAL_DEPTH, TICKER}` `usdm-ws` `test-server`
- `polyoxide_polymarket::perps::ws::{frame_from_text_for_tests, incoming_from_text_for_tests, IncomingForTests}` `perps-ws`
- `polyoxide_polymarket::rtds::fixtures::{REJECTED_SUBSCRIPTION, TWAP_THIRTY_UPDATE}` `rtds` `test-server`
- `polyoxide_polymarket::sports::fixtures::{CRICKET, CRICKET_FINISHED, ESPORTS, LEAGUE_WITH_SPACE, SOCCER, TENNIS_EVENT_STATE}` `sports` `test-server`

The public type aliases, imported the same way:

- `polyoxide_polymarket::clob::DynSigner` `clob`
- `polyoxide_polymarket::clob::account::DynSigner` `clob`
- `polyoxide_polymarket::data::v2::PageStream` `data`
- `polyoxide_polymarket::perps::ws::SupervisedPerpsWs` `perps-ws`
- `polyoxide_polymarket::relay::DynSigner` `relay`
