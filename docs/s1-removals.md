---
baseline: v0.38.1
---

# S1 removals

Every public item that stage S1 removes, against the S1 start tag above. Consolidated items
break at their old paths, with no shim or re-export (AD-16), so prader-rs and every other
consumer migrates from this list.

CI's removals job runs `python3 scripts/api_removals.py check --baseline v0.38.1`. It fails
on any removal that cargo-semver-checks reports and this list does not hold, and on any path
under "Doc-hidden paths consumers import" that no longer compiles. The list is cumulative
until the S1 release: an entry stays after its PR merges.

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

- `polyoxide-core inherent_method_missing: RateLimiter::clob_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::clob_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::gamma_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::gamma_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::data_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::data_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::relay_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::relay_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-core inherent_method_missing: RateLimiter::perps_default (src/rate_limit.rs)` Story 3.2: use `polyoxide_core::polymarket::perps_limits()`, which returns the same table; build any other table with `WindowQuotaTable`.
- `polyoxide-clob module_missing: mod polyoxide_clob::request (src/request.rs)` Story 3.4: clob's namespaces build core's one request builder, `polyoxide_core::Request<T, polyoxide_clob::ClobError>`, which every namespace method now returns.
- `polyoxide-clob struct_missing: struct polyoxide_clob::request::Request (src/request.rs)` Story 3.4: use `polyoxide_core::Request<T, polyoxide_clob::ClobError>`, which every namespace method now returns, with the same `query`, `send` and `send_raw`.
- `polyoxide-clob enum_missing: enum polyoxide_clob::request::AuthMode (src/request.rs)` Story 3.4: nothing to name; each namespace method signs its request itself (L1 for key creation, L2 for the rest), and a signature produced elsewhere goes through `Clob::create_api_key_with_signature` or `Clob::derive_api_key_with_signature`.
- `polyoxide-relay enum_variant_missing: variant RelayError::RateLimit (src/error.rs)` Story 3.5 (DRIFT R7): it was never constructed; a relayer 429 is `RelayError::Api(ApiError::RateLimit(_))`, classed `RateLimited`.
- `polyoxide-relay enum_variant_missing: variant RelayError::Core (src/error.rs)` Story 3.5 (DRIFT R7): `RelayError::Api` now wraps `ApiError` itself, and `From<ApiError>` builds it.
- `polyoxide-binance struct_missing: struct polyoxide_binance::usdm::request::WeightedRequest (src/usdm/request.rs)` Story 3.6: each route returns its own builder over core's `Request`, with the same `cost()` and `send()`: `GetTime`, `GetExchangeInfo`, `GetFundingInfo`, `GetTicker24h`, `GetTickers24h`, `GetPremiumIndex`, `GetPremiumIndices` and `GetOpenInterest`, beside the existing `GetKlines`, `GetFundingRate`, `GetAggTrades` and `GetDepth`.
- `polyoxide-binance struct_missing: struct polyoxide_binance::usdm::WeightedRequest (src/usdm/request.rs)` Story 3.6: the re-export of `usdm::request::WeightedRequest`, gone with it; each route returns its own builder over core's `Request`, as the entry above lists.
- `polyoxide-core inherent_method_missing: HttpClient::should_retry (src/client.rs)` Stories 3.4 to 3.6: send through `HttpClient::send`, whose client's `RetryPolicy` decides (`polymarket::PolymarketRetryPolicy` retries 429 and 425); for the delay alone, `RetryConfig::retry_delay`.
- `polyoxide-core inherent_method_missing: HttpClient::note_rate_limited (src/client.rs)` Stories 3.4 and 3.5: send through `HttpClient::send`, which applies the policy's hold to the throttle; to hold a throttle by hand, `Throttle::hold`, or `RateLimiter::begin_cooldown`.
- `polyoxide-core inherent_method_missing: HttpClient::acquire_rate_limit (src/client.rs)` Stories 3.4 and 3.5: send through `HttpClient::send`, which charges the throttle for every attempt; to charge a `RateLimiter` by hand, `RateLimiter::acquire`.
- `polyoxide-perps struct_missing: struct polyoxide_perps::types::UnknownVariant (src/types.rs)` Story 3.9: use `polyoxide_venue::UnknownVariant`, the one copy, with the same public fields, `Display` and class; every perps enum's and `ws::Channel`'s `FromStr::Err` is now that type.
- `polyoxide-binance struct_missing: struct polyoxide_binance::usdm::types::UnknownVariant (src/usdm/types.rs)` Story 3.9: use `polyoxide_venue::UnknownVariant`, the one copy, with the same public fields, `Display` and class; `Interval`'s and `DepthLimit`'s `FromStr::Err` is now that type.
- `polyoxide-core enum_variant_missing: variant ApiError::Api (src/error.rs)` Story 3.11: an unsuccessful response is `ApiError::Response(Box<ErrorResponse>)`, whatever its status; read `status`, `message`, `body`, `headers` and `retry_after` from the `ErrorResponse`, and the class through `polyoxide_venue::Classify`.
- `polyoxide-core enum_variant_missing: variant ApiError::Authentication (src/error.rs)` Story 3.11: a 401 or 403 is `ApiError::Response`, whose class is `Class::Unauthorized`; match `ApiError::Response(r) if r.status == StatusCode::UNAUTHORIZED` for the status itself.
- `polyoxide-core enum_variant_missing: variant ApiError::RateLimit (src/error.rs)` Story 3.11: a 429 is `ApiError::Response`, whose class is `Class::RateLimited` and now carries the response's `Retry-After`, as `ErrorResponse::retry_after` and `Classify::retry_after` do.
- `polyoxide-core enum_variant_missing: variant ApiError::Timeout (src/error.rs)` Story 3.11: a 408 is `ApiError::Response`, whose class is `Class::Unavailable`; a transport timeout stays `ApiError::Network`.
- `polyoxide-core inherent_method_missing: ApiError::from_status_and_body (src/error.rs)` Story 3.11: build the response with `ErrorResponse::new(status, headers, body)`, which reads the message and `Retry-After`, and convert it with `ApiError::from`.
- `polyoxide-core inherent_method_missing: ApiError::is_retriable (src/error.rs)` Story 3.11: import `polyoxide_venue::Classify` and call its `is_retriable`, which reads the error's class; a transport failure that is neither a connect nor a timeout is now retriable, as its class always said.
- `polyoxide-core trait_method_missing: method from_response of trait RequestError (src/request.rs)` Story 3.11: `RequestError` is a marker implemented for every `From<ApiError> + Debug` type; a failed response reaches that `From` as `ApiError::Response`, so a venue decodes its body there.
- `polyoxide-clob inherent_method_missing: ClobError::is_retriable (src/error.rs)` Story 3.11: import `polyoxide_venue::Classify` and call its `is_retriable`; the FAK and FOK kills and every local failure still answer `false`.
- `polyoxide-core declarative_macro_missing: macro impl_api_error_conversions (src/macros.rs)` Story 3.11: nothing to generate; a crate's error type wraps `ApiError`, its `From<ApiError>` is its decode, and a foreign error enters through `ApiError` (`.map_err(ApiError::from)?`).
- `polyoxide-data inherent_method_missing: DataApiError::is_retriable (src/error.rs)` Story 3.11: import `polyoxide_venue::Classify` and call its `is_retriable`, which reads the class, so a v2 error's status decides; the server's own flag stays readable as `V2Error::retryable`.
- `polyoxide-perps inherent_method_missing: PerpsError::is_retriable (src/error.rs)` Story 3.11: import `polyoxide_venue::Classify` and call its `is_retriable`, which answers as the inherent method did.
- `polyoxide-perps inherent_method_missing: VenueError::is_retriable (src/error.rs)` Story 3.11: import `polyoxide_venue::Classify` and call its `is_retriable`, which answers as the inherent method did.
- `polyoxide-relay enum_variant_missing: variant RelayError::Reqwest (src/error.rs)` Story 3.11: a transport failure, or a 2xx body that failed to read, is `RelayError::Api(ApiError::Network(_))`; an error response whose body failed to read is `RelayError::Api(ApiError::Response(_))` with an empty body.
- `polyoxide-relay enum_variant_missing: variant RelayError::UrlParse (src/error.rs)` Story 3.11: a URL that does not parse, `RelayClientBuilder::url` included, is `RelayError::Api(ApiError::Url(_))`.
- `polyoxide-relay enum_variant_missing: variant RelayError::SerdeJson (src/error.rs)` Story 3.11: JSON that does not serialise or decode is `RelayError::Api(ApiError::Serialization(_))`.
- `polyoxide-binance inherent_method_missing: BinanceError::is_retriable (src/error.rs)` Story 3.12: import `polyoxide_venue::Classify` and call its `is_retriable`, which answers as the inherent method did; `Classify::retry_after` gives a `429`'s or a ban's wait, as the inherent `retry_after` did.

## Doc-hidden paths consumers import

cargo-semver-checks treats `#[doc(hidden)]` items as private, so it never reports their
removal. The removals job imports each item below, and the release's semver job does too.
Each entry names the features it needs after its path, and the entries of one crate that
name the same features are built together, with those features and no others, so an entry
cannot compile through another's. `a::b::{C, D}` lists `a::b::C` and `a::b::D`.

The items are the ones the CLI's tests and each crate's own tests import:

- `polyoxide_binance::usdm::ws::fixtures::{AGG_TRADE, ALL, ALL_MARK_PRICES, ALL_TICKERS, BOOK_TICKER, KLINE, MARK_PRICE, PARTIAL_DEPTH, TICKER}` `test-server`
- `polyoxide_binance::usdm::ws::test_server::{Script, ScriptedServer}` `test-server`
- `polyoxide_perps::ws::test_server` `test-server`
- `polyoxide_perps::ws::{frame_from_text_for_tests, incoming_from_text_for_tests, IncomingForTests}` `ws`
- `polyoxide_rtds::fixtures::{REJECTED_SUBSCRIPTION, TWAP_THIRTY_UPDATE}` `test-fixtures`
- `polyoxide_rtds::test_server::{Script, ScriptedServer}` `test-fixtures`
- `polyoxide_sports::fixtures::{CRICKET, CRICKET_FINISHED, ESPORTS, LEAGUE_WITH_SPACE, SOCCER, TENNIS_EVENT_STATE}` `test-server`
- `polyoxide_sports::test_server::{Script, ScriptedServer}` `test-server`

cargo-semver-checks 0.51.0 does not check type aliases either, so the public ones are imported
the same way:

- `polyoxide_clob::DynSigner`
- `polyoxide_clob::account::DynSigner`
- `polyoxide_data::v2::PageStream`
- `polyoxide_relay::DynSigner`
