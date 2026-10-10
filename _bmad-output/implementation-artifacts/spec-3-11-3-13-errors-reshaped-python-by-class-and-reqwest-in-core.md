---
title: 'Stories 3.11, 3.12 and 3.13: HTTP errors reshaped around one ApiError, Python exceptions by class, and reqwest owned by core'
type: 'refactor'
created: '2026-10-10'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '14084076cf33fc4ddf039638d681d50404b282b9'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-3-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- **The loop's `Fail` returns `Ok(response)`, the same as `Done`** (`polyoxide-core/src/send.rs:114`, deferred from bundle F). AD-8 says it returns core's `ApiError`, carrying the status, headers, body and parsed `Retry-After`.
- **H14: the error body is decoded in several places per crate.** Data decodes twice: `error.rs:58`, and accounting through `get_bytes`, which never tries the v2 envelope. Relay also decodes twice: the `RequestError` impl and its own reader at `client.rs:283-309`.
- **`ApiError` cannot carry what AD-15 needs.**
  - It has no headers and no `Retry-After`, so the `RateLimited` class has no wait to report.
  - `Validation` holds both a server 400 and a local refusal, so a local refusal classes as `VenueRefusal`.
  - Clob builds a fake status `0` for any failure of its Gamma dependency.
- **H15: four crates use `impl_api_error_conversions!`, and clob writes the same conversions by hand.**
  - The macro expands to bare `reqwest::` and `url::` paths, so every caller must depend on reqwest directly.
  - Relay keeps duplicate homes for the same failures: `Reqwest`, `UrlParse` and `SerdeJson` beside `Api(Network | Url | Serialization)`.
- **Inherent `is_retriable` and `retry_after` disagree with `Classify`.** They exist on `ApiError`, `ClobError`, `DataApiError`, `PerpsError`, `VenueError` and `BinanceError`.
- **C4: Python picks an exception by substring-matching the error's Display text** (`polyoxide-py/src/error.rs:42-57`). A server message containing "429" or "timeout" picks the wrong exception.
- **AD-18 is not enforced.** Seven members besides core depend on reqwest directly: gamma, data, perps, clob, relay, binance, and test-support (as a dev-dependency). Any of them could change reqwest's features for all the others, which is how the 0.37.0 gzip regression happened. No test pins the header set a request goes out with.

**Approach:**
- **Story 3.11:**
  - Core's `ApiError` gains `Response(Box<ErrorResponse>)`, which carries the status, headers, body, message and parsed `Retry-After`. It replaces the four variants derived from a status.
  - `Validation` becomes local-only, classed `InvalidRequest`.
  - The loop's `Fail` returns `Err(ApiError::Response)`.
  - Each Polymarket module's `From<ApiError>` becomes its one decode function.
  - The conversion macro and the inherent retriability methods go.
- **Story 3.12:**
  - Binance's `From<ApiError>` becomes its decode function, keeping the D14 overrides.
  - `polyoxide-py` picks each exception from `Classify::class()`, with data v2 still mapped by `code`.
- **Story 3.13:**
  - Core re-exports `reqwest` and is the only member to declare it.
  - A fence test fails on any other direct dependency.
  - Each HTTP module gets a mock test pinning one request's full header set, which CI runs in the venue crate alone and in the workspace build.

**Decisions (Claude, as the user's delegate, 2026-10-10; one-line reasons).** J builds on F's, G's and I's surfaces as their specs and the code describe them. Every **[RISK]** a consumer can see goes into `deferred-work.md` for Story 4.11's release notes and prader.

*Story 3.11: core*
- **`polyoxide_core::ErrorResponse`** is a `#[non_exhaustive] #[derive(Debug, Clone)]` struct with public fields:
  - `status: StatusCode`, `headers: HeaderMap` and `body: String`;
  - `message: String`: Polymarket's `error` field, else its `message` field, else the body verbatim, as `from_status_and_body` reads today;
  - `retry_after: Option<Duration>`: `parse_retry_after(header, Duration::MAX)`, so it is unclamped (surfaced, never slept on), and a zero is `None`.

  Its two constructors are `ErrorResponse::new(status, headers, body)` and `ErrorResponse::read(reqwest::Response).await`. An unreadable body is `""`, as today, and the untruncated body still logs at debug.
  *Reason:* AD-8 and the memlog name exactly these fields; a box keeps `ApiError` small for clippy's `result_large_err`.
- **`ApiError` becomes `#[non_exhaustive]`, with these variants:**
  - `Response(Box<ErrorResponse>)`;
  - `Validation(String)`, local only;
  - `Network(reqwest::Error)`, `Serialization(serde_json::Error)`, `Url(url::ParseError)`, `Refused(Refused)` and `Sign(Box<dyn Error + Send + Sync>)`, each unchanged.

  `Api`, `Authentication`, `RateLimit` and `Timeout` are removed. `ApiError::from_response(Response)` keeps its name and returns `Response`. `from_status_and_body` is removed, and `ErrorResponse::new` replaces it. Displays:
  - `Response` displays `API error: <status as u16> - <message>`, the old `Api` text;
  - every other variant keeps its own.
  *Reason:* AD-15 lets S1 change variants, and the status decides the class, so one status-bearing variant replaces four that only re-encoded the status.
- **Classifying `ApiError::Response`** works exactly as `V2Error` does:
  - `class_for_status(status).unwrap_or(Class::Decode).with_retry_after(retry_after)`;
  - `code` is `None`, since `code` is never the HTTP status;
  - `is_fault()` is false only for 451;
  - `retry_after()` is the parsed wait, whatever the status.

  A 2xx `Response`, such as a ping whose body says it is not ok, is `Decode`.

  `Validation` is `InvalidRequest`. Every status keeps its class.

  [RISK] A local refusal, in clob or relay, changes from `VenueRefusal` to `InvalidRequest`. A Polymarket 429 now reports its `Retry-After` in `RateLimited { retry_after }`, where it reported `None`.
- **The loop (`HttpClient::send`).**
  - `Outcome::Fail` returns `Err(ApiError::Response(ErrorResponse::read(response)))`, after the hold and its WARN, as today.
  - `Outcome::Done` returns `Ok(response)` whatever the status.
  - `Request::send_raw`, `get_bytes` and `health` turn a non-2xx `Done` response, which only a custom policy produces, into `E::from(ApiError::from_response(..))`. So a non-2xx response still never reaches a caller as data.
  - Lines up to the hold's application (`send.rs:84-97`, cited in the MUTANTS ledger) stay where they are.

  *Reason:* AD-8, and the deferred entry for 3.11.
- **`RequestError` loses `from_response`.** It becomes `pub trait RequestError: From<ApiError> + Debug {}`, with a blanket impl for every such type, so every hand-written impl goes. The decode lives in each module's `From<ApiError>`, so a `?` anywhere decodes.
  *Reason:* one function per module (D14), and no second path to forget.
- **Removed from core:**
  - `impl_api_error_conversions!`;
  - the inherent `ApiError::is_retriable`. Callers use `polyoxide_venue::Classify`.

  The socket error types keep their inherent methods until Epic 4. No crate re-exports `Classify`; a consumer calling `is_retriable()` imports `polyoxide_venue::Classify`.

*Story 3.11: modules.* Each module's enum gains or keeps `#[non_exhaustive]`, and its one decode function is its `From<ApiError>`. Foreign errors enter only through `ApiError`, by `.map_err(ApiError::from)?` or by core's paths. So each module drops its `From<reqwest::Error>`, `From<url::ParseError>` and `From<serde_json::Error>`, macro-generated or hand-written, unless a non-HTTP local use needs one.
- **gamma:** `GammaError { Api(#[from] ApiError) }` gains `#[non_exhaustive]`. Its body format is core's. `post_json` calls `?` in place of `from_response`.
- **data:** `From<ApiError>` decodes a `Response` whose body has the v2 envelope as `V2(V2Error)`, and passes anything else through as `Api`. `V2Error::from_parts` takes `&ErrorResponse`.
  - Accounting's `get_bytes` path therefore also yields `V2` for a v2 body. That fixes the second path.
  - The inherent `DataApiError::is_retriable` (the server's flag) and `retry_after` go; `trace_id` stays.
  - [RISK] A caller of `is_retriable()` now gets the class's answer. The server's flag stays readable on `V2Error`.
- **perps:** `From<ApiError>` decodes a `Response` with `{status:"err", error, ref}` as `Venue(VenueError)`. The ping's not-ok body becomes an `ErrorResponse` built with `new`. The inherent `is_retriable` and `retry_after` go from `PerpsError` and `VenueError`; `code` stays.
- **clob:** `From<ApiError>` stays the one decode, and gains:
  - a `Response` with status 400 goes to `classify_order_kill`, giving `FakUnmatched`, `FokUnfilled` or `Api`;
  - `Refused` maps to `BurstCapacityExceeded` and `Sign` is downcast to the clob error, both unchanged.

  `pub(crate) from_response` folds into it. `service()` and status 0 go. A Gamma failure is the new variant `#[cfg(feature = "gamma")] Gamma(polyoxide_gamma::GammaError)`, whose class is the gamma error's. Only `ClobError::is_retriable` goes. The cited `error.rs` lines move, so the ledger is re-cited and re-proved.
  [RISK] A Gamma 5xx during clob's profile lookup is now `Unavailable` (retriable), not `VenueRefusal`.
- **relay:** `RelayError` gains `#[non_exhaustive]` and loses `Reqwest`, `UrlParse` and `SerdeJson`. Those failures arrive as `Api(Network | Url | Serialization)`.
  - The private send helper calls `?` on `HttpClient::send` and no longer reads the body itself, which makes one path.
  - `Signer` and `MissingSigner` stay.
  - Its builder's `url(&str) -> Result` keeps its signature, now failing with `Api(Url)`.
- **Clob, relay and umbrella tests are rewritten, not their assertions.** Tests that matched a removed variant match `Api(ApiError::Response(r)) if r.status == …`. Each status keeps its class, and the Story 2.2 table tests are rewritten row for row.

*Story 3.12*
- **Binance's `From<ApiError>` is its one decode.** It replaces the `RequestError` impl and `from_response_parts`, and keeps today's status-first order and variants:
  - 418 is `IpBanned`, with its `Retry-After` clamped to 3 days and its WARN log;
  - 429 is `RateLimited`;
  - 451 is `RegionBlocked`;
  - 403 is `Forbidden`;
  - otherwise a `{code, msg}` body is `Venue`, and anything else is `Api(Response)` with its message clipped (`clip`).

  The classes are unchanged: 403, 418 and 451 are `Restricted` (D14), and only 451 is not a fault. The inherent `is_retriable` and `retry_after` go; `code` stays. The changed variants are those inside `Api`.
- **Python exceptions follow `Class` one to one** (`exception_for(&impl Classify)` in `polyoxide-py/src/error.rs`):

  | Class | Exception | Status |
  |---|---|---|
  | `Network` | `NetworkError` | existing |
  | `Unavailable` | `UnavailableError` | new |
  | `RateLimited` | `RateLimitError` | existing |
  | `Unauthorized` | `AuthenticationError` | existing |
  | `InvalidRequest` | `ValidationError` | existing |
  | `VenueRefusal` | `ApiError` | existing |
  | `Restricted` | `RestrictedError` | new |
  | `Decode` | `DecodeError` | new |

  An unknown class, which can arise because `Class` is non-exhaustive, gets bare `PolyoxideError`.
  - Data v2 keeps its by-code mapping exactly.
  - `TimeoutError` stays. Only v2's `request_timeout` raises it, and it now subclasses `UnavailableError`.
  - The six attributes are unchanged: set for a v2 error, `None` otherwise.
  - `polyoxide-py` depends on `polyoxide-venue`.

  *Reason:* the five existing names keep their closest class, so the v2 by-code tests pass unchanged; the three classes with no exception get one.
  [RISK] Raised types change:
  - a server 400 raises `ApiError` (was `ValidationError`);
  - 408 and 5xx raise `UnavailableError` (were `TimeoutError` and `ApiError`);
  - 418, 451 and Binance's 403 raise `RestrictedError` (Binance is not bound into Python);
  - a decode failure and data's `Pagination` raise `DecodeError` (were `ApiError` and `PolyoxideError`);
  - `Refused`, `Sign`, `Url` and clob's local errors raise `ValidationError`;
  - a transport timeout raises `NetworkError`.

*Story 3.13*
- **Core re-exports reqwest.**
  - `pub use reqwest;` in `polyoxide-core/src/lib.rs`, documented as the client core sends with: venue crates name its types through this path and never depend on it themselves (AD-18).
  - The reqwest entry, with its features `json`, `rustls-tls` and `gzip` and `default-features = false`, moves from `[workspace.dependencies]` into `polyoxide-core/Cargo.toml`.
  - gamma, data, perps, clob, relay and binance drop reqwest from `[dependencies]`, and test-support drops it from `[dev-dependencies]`. Their src, tests and examples use `polyoxide_core::reqwest`.
  - Public signatures keep the same types under the new path: clob's `send_raw -> Response` and relay's `HeaderMap` returns.
- **The fence.** `test_dependency_fences.py` gains a reqwest fence over every workspace member except `polyoxide-core`, covering every dependency kind. Its live test also asserts that core does declare reqwest, so the fence is not vacuous. A synthetic-fixture test names the offending member. Transitive copies, such as alloy's reqwest 0.13, are not direct dependencies and pass.
- **The header pins.**
  - `polyoxide_test_support::query::headers_sent(path, fire)` mirrors `pairs_sent`, returning every header of the one request as ordered `(lowercased name, value)` pairs.
  - Each of gamma, data, perps, clob, relay and binance gets `tests/headers.rs`. It sends one unauthenticated `GET`, chosen and named in the test, and asserts the exact set: `accept: */*`, `accept-encoding: gzip`, and `host` equal to the mock server's host and port, with nothing else. If the minimal build sends a different set, the literal is what that build sends, recorded in Implementation Notes, and the workspace build must match it.
  - CI's `features` job gains one step that runs `cargo test -p polyoxide-<m> --no-default-features --test headers` once per module, never several `-p` at once, which would unify their features. The workspace build runs the same tests under nextest.
  - `test_ci_workflow.py` asserts that the step names all six modules and `--no-default-features`.

  *Reason:* one literal expected set, passing in both builds, is the "identical headers" proof.

*Records (AD-21, AD-16)*
- **CLAUDE.md is edited in the commit that supersedes each rule:**
  - "Error hierarchy": the `impl_api_error_conversions!` paragraph rewritten;
  - "Every error implements `Classify`": the inherent methods, status 0 and `Validation`;
  - "Retriability";
  - "Python bindings": exceptions by class;
  - the gzip and "only core sends" paragraphs: reqwest declared in core;
  - CI jobs: the `features` step.
- **The removal gate.** Each reported removal goes into `docs/s1-removals.md` with its replacement.
- **Release notes.** `deferred-work.md` gets one entry for 4.11 and prader per story, listing every changed variant and each [RISK] above.

**Commits.** Each commit builds and passes fmt, clippy, the tests and the docs.
- **J0** `test:` Wire-level class pins for gamma, data, perps, clob, relay and binance (`tests/error_classes.rs`).
  - Each sends one request per status against mockito, on a fresh client per status (a 429 or 418 hold must not reach the next case), with `max_retries` 0: 400, 401, 403, 404, 408, 418, 425, 429 with no `Retry-After`, 451, 500, 503, and a 2xx body that does not decode.
  - It asserts `class()` and `is_fault()` through `Classify` only.
  - It passes unchanged through every later commit.
- **J1** Core's reshape, together with whatever each crate needs to keep building with its classes intact (3.11 core).
- **J2** The Polymarket modules' decode functions, the inherent methods removed, clob's `Gamma` variant and the ledger re-proved (3.11).
- **J3** Binance's decode (3.12).
- **J4** Python by class (3.12).
- **J5** reqwest owned by core, and the fence (3.13).
- **J6** The header pins and the CI step (3.13).

J1 may be split further; J0 must come first.

## Boundaries & Constraints

**Always:**
- The status decides the class before the body. The only overrides are D14 (Binance's 403 as `Restricted`) and D15 (the kills are not faults).
- Every status keeps the class it has today. J0 proves it on the wire, and the Story 2.2 table tests prove it per variant.
- `Refused` maps to `BurstCapacityExceeded`, and `Sign` downcasts to the venue's own error. Both keep their tests.
- `classify_order_kill` keeps its logic. Its ledger rows move with it and are re-proved by mutation.
- Each error type stays in each crate's `lib.rs` `const _` `Classify` list and in `test_classify_coverage.py`. The failure-tag twins (test-support's and binance's `tests/failure_tags.rs`) keep their tags, rebuilt from the new variants.
- The MUTANTS ledger: any commit that moves a cited line (`docs/MUTANTS.md`) re-cites it there and in `test_mutants_ledger.py`, and re-proves the row: apply the mutation, see the named tests fail, restore.
- A 451 is not a fault on every HTTP path.

**Never:**
- No catch-all variant, and no new status-derived variants.
- No shim, alias or deprecated re-export for removed items (S1, no shims).
- No `Classify` re-export from venue crates.
- Socket error types and their inherent methods stay unchanged (Epic 4).
- No change to any retry policy, throttle or hold.
- No crate other than core declares reqwest, in any dependency kind.
- No header-test expectation computed from the client under test. The expected set is a literal.
- Never weaken a gate: CI jobs, the fence, the removal gate, the ledger.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Status error | 503 on gamma, data v1, perps, clob or relay | `<M>Error::Api(ApiError::Response(r))`, with `r.status` 503, the class `Unavailable`, and `is_retriable()` true | N/A |
| 429 with a wait | Polymarket 429 with `Retry-After: 7`, retries exhausted | `RateLimited { retry_after: Some(7s) }`; `retry_after()` is `Some(7s)` | `Retry-After: 0` gives `None` |
| Server 400 | Clob 400 `{"error":"bad"}` | `Api(Response)`, class `VenueRefusal`, a fault | N/A |
| Kill | Clob 400 with a FAK kill message | `FakUnmatched`, `VenueRefusal`, not a fault | N/A |
| Local refusal | Relay or clob validation before sending | `Api(Validation)`, class `InvalidRequest`, nothing sent | N/A |
| Refused batch | Signer cost above burst | `ClobError::BurstCapacityExceeded`, nothing sent | N/A |
| v2 body | Data accounting route returns a 503 v2 envelope | `DataApiError::V2`, `Unavailable` with the v2 code | Not v2-shaped: `Api(Response)` |
| Perps venue body | 400 `{status:"err",error,ref}` | `PerpsError::Venue` | N/A |
| Clob's Gamma failure | Profile lookup gets a gamma 404 | `ClobError::Gamma(..)`, class `VenueRefusal` | N/A |
| Binance | 403, 418, 451, then 400 `{code:-1121,msg}` | `Forbidden`, `IpBanned` (WARN logged), `RegionBlocked`, then `Venue { code: -1121 }`. The first three are `Restricted`, and only 451 is not a fault | N/A |
| `Fail` vs `Done` | A policy returns `Fail` on a 200, or `Done` on a 404 | `send` gives `Err(Response 200)`, or `Ok(404)`, which `send_raw` turns into `E::from(Response 404)` | N/A |
| Python | Each class, and data v2's codes | The class table above; v2 by code as before | Unknown class: `PolyoxideError` |
| Header pin | One GET per module, minimal build and workspace build | Exactly `accept: */*`, `accept-encoding: gzip` and `host` | Any extra or changed header fails |
| Fence | A member other than core lists reqwest in any dependency kind | The fence test fails, naming the member | Core's own entry is required |

</frozen-after-approval>

## Code Map

- `polyoxide-core/src/error.rs` -- `ApiError` (:7-51), `from_response` (:55), `from_status_and_body` (:70-88, removed), `is_retriable` (:108, removed), `classify_reqwest` (:132, kept), `status_class` (:167), `Classify` (:189-208), table test `every_variant_classifies` (:424).
- `polyoxide-core/src/send.rs` -- the loop. The `Done | Fail | Retry(_)` arm (:114) returns `Ok(response)`; split it. Ledger lines :84, :91, :95 and :97 must not move.
- `polyoxide-core/src/request.rs` -- `RequestError` (:61-64); `send_raw` (:180-198) calls `E::from_response`.
- `polyoxide-core/src/health.rs` -- `health` (:73) calls `E::from_response`.
- `polyoxide-core/src/client.rs` -- `get_bytes` (:140) and `retry_after_header` (:18). The ledger cites :416 and :435, so re-cite them if lines above them move.
- `polyoxide-core/src/macros.rs` -- `impl_api_error_conversions!` (:33-48), removed.
- `polyoxide-core/src/lib.rs` -- exports (:82-108). Add `ErrorResponse` and `pub use reqwest;`.
- `polyoxide-gamma/src/error.rs`, `api/markets.rs:156` -- `post_json`'s direct `from_response`.
- `polyoxide-data/src/error.rs` (:32 `is_retriable`, :49 `retry_after`, :58-70 decode, :105 macro), `src/v2/error.rs` (:86 `from_parts`, :108 `Classify`), `src/api/accounting.rs:23` (the `get_bytes` path).
- `polyoxide-perps/src/error.rs` (:56 `VenueError::from_parts`, :76 and :83 `is_retriable`, :107 decode, :166 macro), `src/api/health.rs:41-48` (the 2xx not-ok ping).
- `polyoxide-clob/src/error.rs` -- `classify_order_kill` (:91-102), `from_response` (:106-118), `is_retriable` (:131), `validation`/`service` (:143-154), `From<ApiError>` (:161-174), `burst_from_refused` (:182), `From` impls (:215-243), `Classify` (:249-284). Ledger rows are at :92, :95, :113, :116, :375, :387, :395, :408, :419 and :450. `src/client.rs:564` and `:930` call `service`.
- `polyoxide-relay/src/error.rs` (variants, `RequestError` :44), `src/client.rs:283-309` (the second decode path), `:293`, `:324` and `:353` (helpers returning `Response`), `src/config.rs:4` (the `HeaderMap` returns).
- `polyoxide-binance/src/error.rs` -- `from_response_parts` (:76-92), the `RequestError` impl (:134-148), `core_error` and `clip` (:160-171), `Classify` (:184-220), the macro (:222). `src/weight.rs:495` and `src/usdm/policy.rs:6` use reqwest types.
- `polyoxide/src/lib.rs:140-160, 350` -- `PolymarketError` and its table test.
- `polyoxide-py/src/error.rs` -- the exceptions (:5-11), `gamma_err`/`data_err`/`clob_err` (:13-38), `map_api_err` (:42-57), `with_details` (:62-78), registration (:80-92).
- `polyoxide-py/python/polyoxide/__init__.py:11-17,84-90` and `__init__.pyi:13-55` -- exports and stubs. `polyoxide-py/README.md:164-194` -- the mapping table.
- `polyoxide-py/tests/test_data_v2_offline.py:530-620`, `tests/test_live_api.py:554-571` -- the type assertions.
- `polyoxide-test-support/src/query.rs:73` -- `pairs_sent`, the shape for `headers_sent`. `tests/failure_tags.rs`, `tests/token_cost_throttle.rs:18` and `tests/observe.rs:108` use reqwest.
- Every workspace member's `Cargo.toml` -- reqwest at core :21, gamma :20, data :21, perps :23, clob :35, relay :28, binance :24, and test-support's dev-dependencies at :41. The workspace entry is the root `Cargo.toml:41`.
- Examples using reqwest: gamma `cf_burst_probe.rs` and `gamma_batch_ceiling.rs`, data `v2_soak/main.rs`, perps `info_soak.rs`, binance `weight_probe.rs`, plus binance `tests/live_api.rs:185,245`.
- `.github/scripts/tests/test_dependency_fences.py` (`FENCED` :22, `problems` :45, the live test :57, the fixture test :73), `test_ci_workflow.py:113,120`, `.github/workflows/ci.yml:92-104` (the `features` job).
- `docs/MUTANTS.md` and `.github/scripts/tests/test_mutants_ledger.py:75-84` -- the clob `error.rs` rows.
- Readme and doc tests: `polyoxide-data/README.md:82` (inherent `is_retriable`/`retry_after`) and `polyoxide-clob/src/lib.rs:69-74`.

## Tasks & Acceptance

**Execution:**
- [x] `polyoxide-{gamma,data,perps,clob,relay,binance}/tests/error_classes.rs` -- J0 wire-level class pins -- the reshape's invariant, proven before any change.
- [x] `polyoxide-core/src/{error,send,request,health,client,macros,lib}.rs` -- J1 as decided. Tests: the class table rewritten row for row; `Fail` against `Done` on both statuses; `ErrorResponse` parsing of message and `Retry-After` (a 0 is `None`, no clamp); the `Response` Display -- AD-8, AD-15.
- [x] `polyoxide-test-support/tests/failure_tags.rs` -- rebuild the per-status twins from `ErrorResponse`, with the same tags -- the classifier contract.
- [x] `polyoxide-{gamma,data,perps,clob,relay}/src/**` and their tests, plus `polyoxide/src/lib.rs` -- J2 decode functions and removals as decided; clob's `Gamma` variant -- 3.11.
- [x] `docs/MUTANTS.md`, `.github/scripts/tests/test_mutants_ledger.py` -- re-cite and re-prove every moved row -- ledger rule.
- [x] `polyoxide-binance/src/error.rs` and its tests -- J3 -- 3.12.
- [x] `polyoxide-py/src/error.rs`, `Cargo.toml`, `python/polyoxide/__init__.py{,i}`, `README.md` and `tests/` -- J4. Add an offline test (`tests/test_errors_offline.py`, on the local-server helper of `test_data_v2_offline.py`) that raises each class through gamma or data v1 and asserts the exact type -- 3.12.
- [x] `polyoxide-core/Cargo.toml`, root `Cargo.toml`, the six venue manifests, `polyoxide-test-support/Cargo.toml`, and every `reqwest::` path outside core -- J5 -- 3.13.
- [x] `.github/scripts/tests/test_dependency_fences.py` -- the reqwest fence, with its live and fixture tests -- 3.13.
- [x] `polyoxide-test-support/src/query.rs`, the six `tests/headers.rs`, `.github/workflows/ci.yml` (the `features` job), `.github/scripts/tests/test_ci_workflow.py` -- J6 -- 3.13.
- [x] `CLAUDE.md`, `docs/s1-removals.md`, `_bmad-output/implementation-artifacts/deferred-work.md` -- in the commits that change each rule, as decided -- AD-21, AD-16.

**Acceptance Criteria:**
- Given J0's tests, when they run at every later commit, then each passes with its assertions unchanged.
- Given any Polymarket or Binance HTTP failure, when a caller matches it, then it arrives through its module's single `From<ApiError>`, and no crate reads an error body anywhere else (`grep` shows no `from_status_and_body`, no `RequestError` impls and no body read in relay's helper).
- Given the workspace, when `cargo tree -e normal,dev,build --workspace -i reqwest@0.12` runs, then `polyoxide-core` is the only direct dependent.
- Given each module's `tests/headers.rs`, when CI runs it alone with `--no-default-features` and under the workspace build, then both pass against one literal header set.
- Given `polyoxide-py`, when the offline suites and `test_stub_consistency.py` run, then they pass, with each class's exception asserted exactly and v2's by-code tests unchanged.
- Given the removal gate, when it runs against `v0.38.1`, then every reported removal is listed in `docs/s1-removals.md`.

## Implementation Notes

Commits: J0 `718a052`, J1 `8eee873`, J2 `e6df815`, J3 `fcf2c21`, J4 `d43c651`, J5 `4b002fd`, J6 `4debdb1`. J0's six files are unchanged from J0 to J6, and every commit was built, linted, documented and tested before it was made.

- **Where the work landed against the commit plan.** Dropping `RequestError::from_response` (J1) leaves no other place to decode, so every module's `From<ApiError>` became its decode in J1, Binance's included, or J0's classes (Binance's 403 above all) would not have held there. J1 also brought clob's `Gamma` variant, since status `0` could no longer be built, and removed `ClobError::is_retriable` with clob's `From<reqwest | url | serde_json>`, so clob's `error.rs` moved once and the ledger was re-cited and proved once (rows (e), (e), case and (e), only a 400, on top of `718a052`; (e), only a 400 also names the new unit test `the_decode_splits_out_a_kill_only_on_a_400`). J2 is the Polymarket removals, J3 Binance's inherent methods.
- **The loop.** `Outcome::Retry` with no retry left is treated as `Fail` and returns `ApiError::Response`; core's `DefaultRetryPolicy` returns `Retry` for a last-attempt 429, which would otherwise have been handed back as data. `send_raw` still logs `Request failed` at ERROR for a response failure, and only for one, as before. `impl From<ErrorResponse> for ApiError` was added for the call sites that build one.
- **Binance's 418 WARN** under `polyoxide_binance` no longer names the path, since the decode sees the response and not the request: it reads `418: IP banned: <body>`. The send loop's hold WARN under `polyoxide_core` names the path, and `tests/ban_log.rs` now asserts the two together.
- **Assertions that changed with a [RISK]**: relay's `each_relay_route_s_429_holds_the_next_request` (`polyoxide-relay/tests/mock_api.rs`) expects `RateLimited { retry_after: Some(4s) }` where it expected `None`; data's `the_servers_retryable_flag_overrides_the_status_heuristic` is renamed `..._is_surfaced_and_the_status_decides` and asserts the flag on `V2Error` and the class's answer; the v2 pagination test in Python expects `DecodeError`. Every other rewritten test keeps its assertion against the new variant.
- **Header pins.** The minimal build of each module sends exactly `accept: */*`, `accept-encoding: gzip` and `host`, the same set as the workspace build, so the literal is the spec's. `cargo-hack` is not installed locally; the each-feature gate was approximated by checking the touched crates and features one at a time (clob without `gamma`, with `ws`, with `keychain`; perps and Binance with `ws`; relay and core with `keychain`; gamma and data with `specta`; the umbrella with no default features and with `gamma`, `data`, `perps`).
- **The removal gate was not run** (the lead runs it once). The keys in `docs/s1-removals.md` are predicted from cargo-semver-checks 0.51.0's lint templates: `enum_variant_missing` for `ApiError`'s four variants and relay's three, `inherent_method_missing` for `ApiError::from_status_and_body`, the five `is_retriable` methods, `trait_method_missing: method from_response of trait RequestError`, and `declarative_macro_missing: macro impl_api_error_conversions`. The inherent `retry_after` of `DataApiError`, `PerpsError` and `BinanceError` is not listed: `inherent_method_missing` counts a trait impl's method of the same name, and each type's `Classify` impl defines `retry_after`. Correct any key the gate prints differently.

## Spec Change Log

## Review Triage Log

All three layers ran on `14084076`..`167d158` (the diff excludes `_bmad-output`):
- blind: 15 findings;
- edge-case: 9;
- verification-gap: 2 gaps and 4 other findings.

That is 30 findings: 15 patched in 7 fixes, 14 rejected and 1 deferred. Blind's header-pin finding is patched for its module list and rejected for its other three parts. No finding needs the spec changed, so there is no loopback. CLAUDE.md's patches are applied, not deferred (AD-21, as in bundles F, G and I). The removal gate ran before review: 33 removals, all listed, plus 15 other changes S1 allows.

**Re-verified at `0d434a3`** (local Rust 1.95):
- fmt, clippy `-D warnings` and `cargo doc -D warnings` are clean;
- 2,368 workspace tests pass;
- the six header pins pass in their minimal builds;
- `cargo hack --each-feature` is clean;
- `.github/scripts` has 908 passing and `polyoxide-py` 344;
- `live_unwraps` and `gen_registry --check` are clean.

The removal gate is not re-run, since the patches remove nothing. MSRV 1.91 is not checked, because no local toolchain is installed.

The one Python failure in the first run, `test_live_api.py::TestDataV2Sync::test_market_routes`, was a live 503 `request_timeout` from the Data API. It passed twice on re-run, and is already in deferred-work.

**Patched** (by the implementer, re-engaged):
- **Medium: a failed response's body is read while its concurrency permit is held** (verification-gap other, blind, edge-case: three reports of one defect). `send.rs`'s `Fail | Retry(_)` arm awaits `ErrorResponse::read` with `permit` alive. A 2xx body is read by the caller after the permit drops. A slow error body therefore holds one of relay's two slots. The permit is now dropped first.
- **Medium: `ErrorResponse`'s derived `Debug` is unbounded** (blind, edge-case). `Request::send_raw` logs `Request failed: {:?}` at ERROR for every answered failure. So each line carries every response header (`set-cookie` included) and the body, twice when the body is not JSON. Before, core logged the message once and no headers. `Debug` is now hand-written: the status, `retry_after`, the message and body through `truncate_for_log`, and only the header names.
- **Medium: Binance's non-venue arm keeps the unclipped body** (verification-gap gap and other, edge-case). The old `core_error` kept only a clipped message. `every_kept_body_is_clipped` read only `message`. The body is now clipped too, and the test asserts it.
- **Medium: the header pins' module list is hard-coded three times** (blind). A new venue crate on core's client, Kalshi for instance, could ship with no `tests/headers.rs` and stay green. `test_ci_workflow.py` now derives the list from cargo metadata.
- **Low: `health`'s and `get_bytes`' own non-2xx branches have no test** (verification-gap gap). The default policy now fails a non-2xx inside the loop, so both branches run only under a `Done`-on-failure policy. Deleting either check would leave CI green. Tests now run both under `Inverted`.
- **Low: Binance reads `Retry-After` twice, with different clamps** (verification-gap other, blind). The non-venue arm surfaced core's unclamped reading, while 429 and 418 surfaced the 3-day clamp. That arm now carries the clamped reading.
- **Low: stale or inaccurate docs** (blind ×3, plus the docstring half of verification-gap's v2 finding). This covers six things:
  - two comments and CLAUDE.md's sports paragraph cited core's removed inherent `is_retriable`;
  - CLAUDE.md's guide paragraph did not record `impl_api_error_conversions!` going;
  - CLAUDE.md's `parse_retry_after` sentence did not name `ErrorResponse`'s unclamped read;
  - the fence's and the header pins' docstrings misstated 0.37.0, which was a code default in core's builder, not a feature switched on elsewhere;
  - the s1-removals line for relay's `Reqwest` said "a body that failed to read", which is true only of a 2xx body;
  - the docstrings of Python's `ApiError` and `ValidationError` did not mention data v2's by-code mapping.

  The release notes now name the removed inherent `retry_after`s and the direct `polyoxide-venue` dependency that `Classify` needs.

**Deferred:**
- **Python exceptions leave `status` and `retry_after` as `None` for every non-v2 error** (blind). This predates J, and the spec keeps the six attributes unchanged. Every `ErrorResponse` now carries both values, so filling them in is a follow-up (deferred-work).

**Rejected:**
- **False: Python's v2 fallback ignores the class** (blind, edge-case, verification-gap other). The frozen intent keeps data v2's mapping by `code` exactly, and Story 3.12's AC says so. The docstrings that contradicted it are patched above.
- **False: relay's private `send` dropped the non-2xx check** (blind). Relay installs `PolymarketRetryPolicy`, which is `Done` only on a 2xx, and its builder has no way to install another policy, so the case cannot be reached.
- **False: Python's changed exceptions are undocumented** (blind). The Story 3.12 release-notes entry in deferred-work lists every changed raised type ((6) [RISK]), and the CHANGELOG is written from it at 4.11.
- **False: the removals ledger is only predicted** (blind). The gate ran (`api_removals.py check --baseline v0.38.1`): 33 removals, all listed. The removed inherent `retry_after`s are not reported because each type's `Classify` impl defines `retry_after`.
- **Rejected by the frozen decision: `Classify` is not re-exported** (blind). The spec decides "No `Classify` re-export from venue crates". The release notes now name the dependency.
- **Low: `ErrorResponse` keeps no URL or path, so Binance's 418 WARN lost its path** (blind). Core's hold WARN still names the path, and every 418 holds. Adding a field adds public surface, for a log line that already exists.
- **Low: `ClobError::Gamma` drops which lookup failed** (blind). It is rare, and keeping the operation needs a new variant shape.
- **Low: the header pins check only the last request, only a GET, and `--no-default-features` is a no-op for five crates** (blind). The AC asks for one request. `-p <crate>` alone is what isolates features; a default feature is not. No client sends two requests for one call.
- **Low (pre-existing): a JSON body with a non-string `error` and a string `message` keeps the raw body as its message** (edge-case). The logic is unchanged from `from_status_and_body`.
- **Low (pre-existing): Binance's `Venue` arm has no `retry_after`** (edge-case). It had none before. Adding one adds a field.
- **Low: three refusals made after a response are `Validation`, so they class `InvalidRequest` and not `Decode`** (edge-case). They are clob's malformed `minimum_tick_size`, clob's malformed proxy address from Gamma, and relay's malformed relay-payload address. Each refuses before the order or transaction is sent; both classes are non-retriable and tag `real`; the venue data is rarely malformed. A decode representation for them is more than a direct correction.
- **Low: `headers_sent` would panic on mockito's thread for a non-ASCII header value** (edge-case). No polyoxide request sends one.
- **Low: perps' not-ok ping message is now the body, and a `status: err` body becomes `Venue`** (edge-case). The body carries the not-ok status verbatim, and the class is `Decode` either way.

## Design Notes

**Why `From<ApiError>` is the decode function.** thiserror's `#[from]` would wrap without decoding, so a `?` on core's error would skip the venue body and leave a second path to decode in. Clob already decodes in a hand-written `From` for `Refused` and `Sign`. Gamma and relay have no venue body beyond core's `error`/`message`, so their derived `#[from]` is their decode.

```rust
impl From<ApiError> for PerpsError {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::Response(r) => match VenueError::from_response(&r) {
                Some(venue) => Self::Venue(venue),
                None => Self::Api(ApiError::Response(r)),
            },
            other => Self::Api(other),
        }
    }
}
```

**Why one literal header set.** The 0.37.0 regression was invisible to every test because none compared a minimal build with a unified one. A literal set that both builds must match makes any feature leak, whichever crate it comes from, a failing test.

## Verification

**Commands:**
- `cargo fmt --all -- --check` -- expected: clean.
- `cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean.
- `CARGO_INCREMENTAL=0 cargo test --all-features --workspace` -- expected: all pass.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean.
- For each module m in gamma, data, perps, clob, relay and binance, run `cargo test -p polyoxide-$m --no-default-features --test headers` -- expected: pass.
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: clean.
- `cd .github/scripts && uv run pytest tests/` -- expected: pass, the fence and the ledger included.
- `cd polyoxide-py && uv run --reinstall-package polyoxide pytest tests/` -- expected: pass.
- `python3 scripts/gen_registry.py --check` and `python3 scripts/live_unwraps.py` -- expected: clean.
- `python3 scripts/api_removals.py check --baseline v0.38.1` -- expected: no unlisted removal (run once, by the lead; then `rm -rf target/semver-checks`).
