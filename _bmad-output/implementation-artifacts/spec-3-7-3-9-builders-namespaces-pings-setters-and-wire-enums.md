---
title: 'Stories 3.7, 3.8 and 3.9: One client builder, namespace pattern and health ping; one query-setter macro; one wire-enum vocabulary'
type: 'refactor'
created: '2026-10-09'
status: 'in-progress'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'be83cfc8ded1d8d8418594575dc5b98e3d75e181'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-3-context.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:**
- **H6.** Six client builders each declare and set `base_url`, `timeout_ms`, `pool_size`, the retry config and `max_concurrent` by hand: 27 knob methods. Relay has only two of them, and Binance alone sets `gzip(true)`.
- **H7.** About 40 namespace accessors are hand-written `X { http_client: self.http_client.clone() }`.
- **H8.** Six pings are hand-written. They differ in what their latency includes and in what they check:
  - gamma, data and clob run on `HttpClient::send` (G's R8) with three copies of a private `Stopwatch` authenticator, and time the attempt that answered;
  - perps, Binance and relay time the whole call, permit, throttle and backoff waits included. Only Binance's docs say so; perps' and relay's call it the round-trip time.
- **H9.** 417 query setters are hand-written: gamma 196, data 153, clob 56 and Binance 12. Only perps uses a macro, its `pub(crate)` `setter!`.
- **H10.** Seven enum macros (four names) are spread over five crates, and perps and Binance each define their own `UnknownVariant`.
- **H11.** Perps and Binance each decode positional decimal arrays their own way.
- **H16.** Five copies compute "now in Unix ms".

**Approach:** One shared definition for each concern.
- **Story 3.7:**
  - core gains `ClientConfig` and `client_config_setters!`, and every builder holds a `ClientConfig`;
  - core gains `namespaces!` for the namespace accessors;
  - core gains `HttpClient::health`, one ping through the send loop, and every venue's `ping` calls it.
- **Story 3.8:** core gains `query_setters!`, promoted from perps' `setter!`, and `csv`, moved from data v2. A golden test pins every setter's key and value before any setter moves.
- **Story 3.9:** `polyoxide-venue` gains:
  - `UnknownVariant`;
  - `open_enum!`, `wire_enum!` and `specta_as_string!`;
  - `UnixMillis::now()`;
  - behind a new `decimal` feature, the positional decimal helpers.

  The per-crate copies are removed.

**Decisions (Claude, as the user's delegate, 2026-10-09; one-line reasons).** I builds on F's and G's surfaces as their specs describe them. Every **[RISK]** that a consumer can see goes into `deferred-work.md` for 4.11's release notes and prader.

*Story 3.7: the builder (H6)*
- **`polyoxide_core::ClientConfig`.**
  - Its fields are `pub base_url: String`, `timeout_ms: u64`, `pool_size: usize`, `retry_config: Option<RetryConfig>` and `max_concurrent: Option<usize>`, plus a private default concurrency.
  - `ClientConfig::new(base_url, default_max_concurrent)` fills in `DEFAULT_TIMEOUT_MS` and `DEFAULT_POOL_SIZE`.
  - `http_builder() -> HttpClientBuilder` applies the timeout, the pool size, the concurrency (the set value, else the default) and the retry config if set. It sets no throttle, policy or `gzip`.

  *Reason:* the venue still installs its own throttle and policy (F, G), so the config holds only transport settings.
- **`client_config_setters!(config)`, invoked inside each builder's `impl`, generates the five knob methods.**
  - Their names and signatures are unchanged: `base_url(impl Into<String>)`, `timeout_ms(u64)`, `pool_size(usize)`, `with_retry_config(RetryConfig)` and `max_concurrent(usize)`.
  - *Reason:* data's `paged_builder_methods!` already uses this idiom.
- **Default concurrency is unchanged:**
  - relay 2; gamma, data, perps and Binance 4; clob 8;
  - perps' and Binance's public `DEFAULT_MAX_CONCURRENT` constants stay;
  - perps and Binance each gain the `test_default_concurrency_limit_is_4` test that gamma, data, clob (8) and relay (2) already have.
- **Binance drops `.gzip(true)`.** Core enables reqwest's `gzip` feature, so leaving it unset asks for gzip exactly as `true` did. `a_gzip_body_is_requested_and_decoded` pins it. This meets the AC's "leaves `gzip` unset".
- **Venue knobs stay with their venue,** because they configure the venue, not the transport:
  - data's `pnl_base_url` and `rankings_base_url`;
  - perps' `with_rate_limiter` (F keeps it);
  - clob's `chain`, `signature_type`, `builder_code`, `gamma` and `with_account`;
  - relay's `url`, `chain_id`, account, auth and wallet setters;
  - Binance's `weight_budget`.
- **Relay gains `base_url`, `timeout_ms` and `pool_size` (additive). It keeps `url(&str) -> Result`, which validates early and adds the trailing slash. `build` normalises either.**
  - Relay's defaults (30 s, 10) are the ones it gets from `HttpClientBuilder` today.
  - **[RISK]** Relay then has two setters for its base URL.
- **[RISK] Eleven inline builder tests read `builder.config.x` where they read `builder.x`:** gamma 6, data 3, clob 2. Their names and asserted values are unchanged.

*Story 3.7: namespaces (H7) and the ping (H8)*
- **`namespaces!` generates accessors that clone the listed client fields into same-named fields of the namespace struct.**
  - One accessor may map a field from another, as in `pnl: PnlApi { http_client: pnl_http_client }`.
  - It covers 34 accessors: gamma 9, data 15, perps 4, Binance 3 (`http`) and clob 3 (`markets`, `health` and `public_rewards`).
  - The namespace structs stay hand-declared, since their paths are public API.
- **[RISK] Eight accessors stay hand-written. None is a field-clone accessor, though AC 3.7 says "no crate defines its own accessors":**
  - data's `user(addr)` takes an argument, `positions` is an alias and `traded` is a wrapper;
  - each of clob's five account-gated accessors (`orders`, `account_api`, `notifications`, `rewards` and `auth`) projects a different part of the account and fails with its own message.
- **`HttpClient::health::<E: RequestError>(&self, path: &str, costs: &[Cost]) -> Result<Pong, E>`.**
  - It sends one GET through `send`, so the ping takes the permit, the throttle, the retry and the hold.
  - A core-private timing authenticator times the attempt that answered.
  - A final non-2xx response becomes `E::from_response`.
  - `Pong { round_trip: Duration, response: reqwest::Response }` is `#[non_exhaustive]`.
  - It replaces G's three private timing authenticators.
  - Record the signature in `spine-amendments/epic-3.md` as A3-5. It adds `costs` (for Binance's weight) to AD's `health(path)`.
- **Each `ping` keeps its public signature, its path and its body check, and every venue returns `round_trip`: one meaning for every ping.**
  - Paths: gamma `/status`, data and clob `base_url.path()`, perps `/v1/info/ping`, Binance `/fapi/v1/ping` (costed from `Route::Ping`), relay `base_url.path()` (so a path prefix is kept).
  - gamma, data and clob: unchanged since G.
  - **[RISK]** perps, Binance and relay stop including the permit, throttle and backoff waits. Binance's doc line "Includes any wait for the budget" goes. This closes G's deferred-work entry on relay's ping timing, and settles R8's "whether a ping times only its last attempt": it does, everywhere.

  Body checks:
  - perps decodes the body with `decode_json` and checks `status == "ok"`;
  - Binance decodes `{}`.

- **[RISK] `ping` stays a one-line delegation, not a macro.** AC 3.7 says the accessors and `health(path)` "come from core macros". A macro around a one-line call would also have to carry each crate's doctested docs.
- **Relay implements `RequestError`.** Its `from_response` is `Self::Api(ApiError::from_response(response).await)`, core's one mapping. Relay's private `send` keeps reading the body itself, because it logs a failed POST's body at ERROR. The two differ only when the body cannot be read: `send` gives `Reqwest` (classed `Network`), and `from_response` gives the status's error with an empty message.

*Story 3.8: setters (H9)*
- **`polyoxide_core::query_setters!` takes a list of setters.**
  - Each writes to `self.request`, or to `self.<field>` when the invocation starts `self.<field>;`. Data v2's paged builders use `self.inner`; its ten others (`v2/api/boards.rs` 4, `markets.rs` 1, `wallet.rs` 5) write `self.request`.
  - What the field is: core's `Request` (gamma, data, and clob since G, whose authenticated builders carry `.authenticator(l2)`), data v2's `Paged`, and Binance's private `Routed<T>`, whose inherent `query(&'static str, impl ToString)` replaces on repeat and which has no `query_many`.
  - The arms:
    - `name: T => "k"`;
    - `name: impl Into<String> => "k"`;
    - `name: many T => "k"`, which repeats the key;
    - `name: csv T => "k"`;
    - `name: csv<I, S> => "k"`, which keeps data v2's explicit generics;
    - `name(arg: T) => "k" = expr`;
    - `name(arg: T) => "k" if cond`;
    - `name(arg: T) => csv "k" = expr`.
  - Generated bodies call `.query` or `.query_many` on the field, bringing `QueryBuilder` into scope with `as _`. An inherent `query` wins, so core's append and the replace-on-repeat of G's Binance builders both survive.
  - Perps' `setter!` goes.
- **`csv` moves from data v2 (`v2/envelope.rs:139`) to `polyoxide_core::csv`, with its two tests.**
  - Its rule, "omitted when the joined value is empty", also governs data v1's 16 comma-joined setters.
  - **[RISK]** Today a v1 setter given exactly `[""]` sends `key=`. It will now send nothing.
- **Proof first.** One golden test per crate calls every query setter with a typed value (`5u32`, `1.5f64`) and asserts the exact ordered `(key, value)` pairs. The crates are gamma, data (v1 and v2), clob and Binance.
  - It lands green on the hand-written setters before any setter moves, and stays unchanged after.
  - So a changed name or argument type fails to compile, and a changed key, value or order fails the test.
  - **[RISK]** AC 3.8 names "each crate's `query_keys_sent` spec-agreement tests". Only data v2 and perps have them, and they check keys only. For gamma, data v1, clob and Binance the proof is this golden test, not agreement with an OpenAPI mirror. Their mirrors disagree with their servers in places (gamma's `OBSERVED.md`).
- **`polyoxide_test_support::query::pairs_sent(path, fire)` returns the ordered pairs off a mock server.** It asserts the request was sent and does not require the body to decode.
- **[RISK] Seven setters stay hand-written. They are not single query writes, though the AC says "no hand-written setter remains":**
  - Binance's `GetKlines::limit` and `GetDepth::limit`, which also re-price the route;
  - gamma's `GetManyMarkets::include_tag` and the `limit` and `offset` of `QueryByInformation` and `QueryAbridged`. They store fields that `send` writes, and those builders hold no `Request`.
- **Signatures are unchanged.**
  - Generics stay generics, `impl Trait` stays `impl Trait`, and `&Symbol` stays a reference.
  - Parameter names may become `value`, which is not API.

*Story 3.9: the vocabulary (H10, H11, H16)*
- **`polyoxide_venue::UnknownVariant { pub type_name: &'static str, pub value: String }`.**
  - It derives `Debug`, `Clone`, `PartialEq` and `Eq`.
  - Its `Display` (`{value:?} is not a valid {type_name}`) and `Error` are written by hand, so venue's default build still depends on nothing.
  - It classes as `InvalidRequest`.
  - Perps' and Binance's copies are removed. Perps' `Channel` parser builds the venue type.
- **`wire_enum!` (closed).**
  - Syntax: `$vis enum Name { $(#[attr])* Variant => "wire", … }`.
  - It derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `::serde::Serialize` and `::serde::Deserialize`, with renames.
  - It generates `ALL`, `as_str(self) -> &'static str`, `Display` and `FromStr<Err = UnknownVariant>`.
  - It adds no `#[non_exhaustive]` itself; data's ten call sites pass it as an attribute, as today.
  - Perps 7, Binance 2 and data 10 use it.
  - **[RISK]** Data's ten closed enums change in two ways:
    - `FromStr::Err` goes from `String` (`unknown X: v`) to `UnknownVariant` (`"v" is not a valid X`). Python maps the error itself (`choice`), so Python is unaffected.
    - `as_str(&self)` becomes `as_str(self)`. Method calls are unchanged, but `T::as_str` over `&T` no longer compiles. Binance's `weight_probe` needs the by-value form.
- **`open_enum!`.**
  - It generates:
    - a `#[non_exhaustive]` enum with an `Other(String)` variant;
    - `ALL`, `as_str(&self) -> &str` and `from_wire(&str) -> Self`;
    - `Display`, `FromStr<Err = Infallible>`, and `Serialize` and `Deserialize` as a string.
  - Gamma 3, data 4, Binance 3 and relay 3 use it.
  - Relay's enums gain `ALL`, and the others gain relay's `from_wire`. Both are additive, and no existing method is removed.
- **Specta.**
  - The venue macros emit no specta code. A `cfg(feature = "specta")` inside an exported macro is evaluated in every calling crate. Binance, perps and relay declare no such feature, so `unexpected_cfgs` would fail `-D warnings` there.
  - `polyoxide_venue::specta_as_string!(A, B, …)` emits the `#[cfg(feature = "specta")] impl specta::Type` (as `String`) that gamma's and data's open enums carry today. Each of those two crates invokes it once.
  - Data's closed enums pass `#[cfg_attr(feature = "specta", derive(specta::Type))]` as an attribute, as their macro emits today.
- **The macros expand to `::serde` paths.** So the calling crate depends on serde, as all five already do. Venue takes serde and serde_json only as dev-dependencies, for its own macro tests.
- **The positional decimal helpers live in `polyoxide_venue::positional`, behind a new venue feature `decimal`.**
  - The feature turns on `dep:rust_decimal` (with `serde-with-str`) and `dep:serde`.
  - The helpers:
    - `DecimalStr(pub Decimal)` decodes through `rust_decimal::serde::str` and encodes through `Display`;
    - `element(seq, index, expecting)` reads one element;
    - `drain(seq)` skips the rest.
  - Perps and Binance enable `decimal`.
  - Perps keeps its tuple-derived exact arity: `KlineWire` with `DecimalStr` fields, and `(u64, DecimalStr)` and `(DecimalStr, DecimalStr)` for the two-element rows.
  - Binance keeps its visitors, which tolerate extra elements.
  - *Reason:* sports and rtds keep building nothing new through venue, which venue's `Cargo.toml` comment promises. Record this in `spine-amendments/epic-3.md` as A3-4: AD-3's rust_decimal and serde become optional.
  - **[RISK]** Perps' positional rows now accept scientific notation (`"1e-5"`), as every other perps decimal field already does through `rust_decimal::serde::str`. A refusal's error text changes too.
- **`polyoxide_venue::UnixMillis(pub u64)`.**
  - It derives `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` and `Default`.
  - `now()` returns 0 when the clock reads before 1970, as clob and Binance return today.
  - It has no serde impls; records add them in S3.
  - Its adopters:
    - clob's two order timestamps (private `build_order_v2` then takes `u64`);
    - Binance's `Clock::System`;
    - perps' live `now_ms`;
    - Binance's live `unix_minute` (`/ 60_000`, the same minute);
    - test-support's `ms_into_minute`.
  - `scripts/live_unwraps.baseline.json` is lowered in the same commit: perps `live_api` opt-outs go from 1 to 0, and Binance's from 6 to 5.
- **Venue's `lib.rs` gains a `const _` `Classify` assertion listing `UnknownVariant`.** In `test_classify_coverage.py`:
  - `ASSERTION` also accepts `crate::Classify`;
  - the known-types rows move `UnknownVariant` from perps and Binance to a new venue row.
- **Venue's `[package.metadata.polyoxide] readme` and its `description` name the new items, and `gen_registry.py --write` regenerates the regions.**

## Boundaries & Constraints

**Always:**
- **Sequencing.**
  - Bundle I starts only after bundle G is reviewed and committed, and after all of F's commits.
  - The Code Map was re-checked against G's landed code at `be83cfc`. Re-check any reference before relying on it, since I's own commits move lines.
- **Commit at each boundary in Tasks (I1–I11), on this branch only.**
  - Never push, tag or switch branches.
  - End every commit message with these two trailer lines:
    - `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
    - `Claude-Session: https://claude.ai/code/session_0151gQKQKPaCTrVDWo6gvWt3`
  - Each commit builds and passes the suites it touches.
- **Behaviour is preserved, except the named [RISK] items.** These stay unchanged:
  - every setter's name, argument type, key, value and order;
  - every default (concurrency 2, 4 or 8; timeout; pool; retry config);
  - every client's throttle, policy and retry set;
  - every ping's path, gating, latency semantics and body check;
  - every enum's wire spellings.
- **NFR7.**
  - Moved and rewritten tests keep their names and asserted values.
  - Report per-target counts before and after every commit that adds, moves or drops tests, from `cargo test -p <crate> --all-features <target> -- --list | grep -c ': test$'`.
  - Append new tests at the end of existing files. New suites go in new files.
- **The mutant ledger.**
  - No `docs/MUTANTS.md`-cited line may move. Put new core code in new files (`config.rs`, `health.rs`), and edit `macros.rs` and `lib.rs` only by appending.
  - Cited lines sit below the ping tests in data's, clob's, Binance's and perps' `tests/mock_api.rs`, in core `client.rs` (:416, :435), core `send.rs`, `hooks.rs`, Binance `usdm/policy.rs` and clob `error.rs`. Edit a ping test in place only with no net line change; otherwise append a new test.
  - If a cited line moves anyway, re-prove its row and update `test_mutants_ledger.py` in the same commit.
- **Removals.**
  - Each removed public item goes in `docs/s1-removals.md`, keyed exactly as the gate prints it, with its story and replacement.
  - Each changed one, and each consumer-visible [RISK] item, goes in `deferred-work.md` in the commit that causes it.
- **AD-3.**
  - Venue's default build depends on nothing.
  - `decimal` adds rust_decimal and serde only.
  - The rtds and sports dependency trees are unchanged.
- **AD-21.** CLAUDE.md is rewritten in the commit that changes the rule it states.
- **Rustdoc.** `pub` docs never link a `pub(crate)` item. Macro-generated docs carry no intra-doc links into core or venue, since they expand in other crates.
- **Disk.**
  - Build with `CARGO_INCREMENTAL=0` and `-j 4`.
  - Run `scripts/api_removals.py` once, at the end, then delete `target/semver-checks`.
- **mockito.** An `expect(0)` on a route with a query needs `match_query(Matcher::Any)`, and every mock with an `expect` is asserted.

**Never:**
- **Change any of these:** the throttles, policies, limit tables, `RESERVED_FRACTION`, the signer layer's `allow_burst`, or Binance's `Route::cost`.
- **Set `gzip` in any client builder or in `ClientConfig`.** The four soak examples keep their `.gzip(false)`.
- **Add a shim or re-export for a removed item** (`UnknownVariant` at its old paths included).
- **Unify the venues' types:** perps' and Binance's `Interval` (D16), or their `Kline` and `Level` structs.
- **Generate or move a namespace struct,** since its path is public.
- **Do 3.10–3.13's work:** reshape an error enum, remove an inherent `is_retriable`, or fence reqwest.
- **Hand-edit** `docs/ARCHITECTURE.md` or a generated region.

## I/O & Edge-Case Matrix

| Scenario | Input | Outcome |
|---|---|---|
| Builder defaults | gamma, data, perps, Binance; clob; relay | 4, 8 and 2 concurrent; 30 s; 10 idle; core's `RetryConfig::default()` |
| Binance gzip | any request | `Accept-Encoding` includes `gzip`, as before |
| Ping, healthy | gamma `/status` 200 | `Ok(round_trip)` of that attempt |
| Ping retried | 429, then 200, with a 300 ms floor | Retried. `round_trip` < 300 ms on every venue, while the whole call takes ≥ 300 ms. The next request is held |
| Ping refused | 503 | The venue's `from_response` error, as today |
| Ping gated | the only permit held | Waits (every venue; clob since G) |
| Perps ping body | 200 `{"status":"degraded"}` | `ApiError::Api { status: 200, .. }`, as today |
| Relay prefix | base `https://h/prefix/` | The ping hits `/prefix/` |
| `csv` | `[]`; `["a","b"]`; `[""]` | Omitted; `a,b`; omitted (data v1 sent `key=`) [RISK] |
| `open(true)` | gamma `ListMarkets` | `closed=false` |
| `parent_entity_type(Unknown)` | gamma `ListComments` | No parameter |
| Filtered csv | data v1 `activity_type([Unknown])` | No parameter |
| Binance repeat | `.limit(5).limit(9)` on klines | `limit=9`, once |
| Closed, unknown | `"2m".parse::<perps::Interval>()` | `UnknownVariant`, `"\"2m\" is not a valid Interval"`, `InvalidRequest` |
| Closed, unknown, data | `"NOPE".parse::<FilterType>()` | `Err(UnknownVariant)` (was `Err(String)`) [RISK] |
| Open, unknown | `"STATE_QUEUED"` (relay) | `Other("STATE_QUEUED")`, written back verbatim |
| Positional arity | perps kline, 8 elements; Binance kline, 13 | Refused; accepted (both as today) |
| Positional exponent | perps level `["1e-5","2"]` | Accepted (was refused) [RISK] |
| Clock before 1970 | `UnixMillis::now()` | `UnixMillis(0)` |

</frozen-after-approval>

## Code Map

Line numbers are taken at `be83cfc`, after bundle G.

- **Core**
  - `polyoxide-core/src/client.rs`:
    - `DEFAULT_TIMEOUT_MS` :28, `DEFAULT_POOL_SIZE` :30;
    - `HttpClientBuilder` :165-287 (`gzip` :254, `Default` :289-293 → `new(String::new())`).
    - I adds nothing here; MUTANTS rows (b) cite its tests at :416 and :435.
  - `send.rs`: `HttpClient::send` :52, which returns `Ok(Response)` on `Fail` (:114-126). MUTANTS rows (a), (f) and (g) cite :84, :91, :95 and :97.
  - `hooks.rs`: `Cost` :42, `RequestParts` :153 (with G's public `timeout`, so `health` builds it with `RequestParts::new`), `Authenticator` :248. MUTANTS row (a), the policy, cites :297.
  - `request.rs`: `QueryBuilder` :14-58 (`query`, `query_many`), `RequestError` :61-64, `Request` :72-81 with G's `method`, `authenticator`, `with_cost` and `body`.
  - `macros.rs`: `impl_api_error_conversions!` :34. I appends below it.
  - `lib.rs`: re-exports :75-95.
  - Tests: lib 133, `mock_request` 15, `send_loop` 17. `mock_request.rs:23` `TestError` is the pattern for a test `RequestError`.
- **Builders** (each has its fields, `new`, five knobs and `build`):
  - gamma `client.rs`:
    - struct :98-104, knobs :118-147;
    - `build` :150-163 (`4` at :156);
    - 11 tests, 6 of them reading fields (:177-231).
  - data `client.rs`:
    - struct :198-206, knobs :226-277 (pnl and rankings at :237 and :248);
    - `build` :280-302 (`4` at :286; siblings :294-295);
    - tests :328, :334, :347, :353 and :373, 3 of them reading fields.
  - perps `client.rs`:
    - `DEFAULT_MAX_CONCURRENT` :18;
    - struct :67-74 (`rate_limiter`), knobs :89-122, `build` :125-140;
    - tests :148 and :157.
  - clob `client.rs`:
    - struct :829-841, knobs :868-952 (`base_url` :868, `timeout_ms` :874, `pool_size` :880, `with_retry_config` :941, `max_concurrent` :949);
    - `build` :955-1002 (`8` at :966; the default Gamma at :983-989);
    - tests :1067, :1073 and :1093.
  - relay `client.rs`:
    - struct :1828-1838 (`base_url`, `chain_id`, `account`, G's `auth: Option<Secret<AuthConfig>>`, the wallet fields, `retry_config`, `max_concurrent`; no timeout or pool fields);
    - `Default` :1840-1855 (reads `RELAYER_URL` and `CHAIN_ID`);
    - `url` :1885-1892, knobs :1948-1959, `build` :1964-1999 (re-adds the trailing slash at :1965-1968; `2` at :1977; installs `relay_limits()` and `PolymarketRetryPolicy`);
    - tests: `test_ping` :2008 (hits the live host) and :2015.
  - Binance `usdm/mod.rs`:
    - `DEFAULT_MAX_CONCURRENT` :27;
    - struct :77-84, knobs :99-133, `weight_budget` :137, `build` :148-166 (`.gzip(true)` :158; installs the budget as throttle and `UsdmRetryPolicy`);
    - tests :174, :180 and :189.
  - Callers that need no change: `polyoxide-py/src/clients/{gamma,data}.rs` (:670, :759, :910, :1001), the umbrella's `PolymarketBuilder` (`polyoxide/src/lib.rs:300-338`) and the CLI.
- **Accessors**:
  - gamma `client.rs:33-94` (9);
  - data `client.rs:60-194` (18, 15 of them through the macro; `approvals` at :158-166 carries `#[deprecated]` and `#[allow(deprecated)]`, which `namespaces!` passes through);
  - perps `client.rs:38-63` (4);
  - Binance `usdm/mod.rs:48-67` (3, each cloning `http` only; `Usdm.budget` :33 is read only by `weight_budget()` :70);
  - clob `client.rs` :101, :108 and :178 (field clones of `http_client`), and the account-gated :131 `Orders {http_client, l2}`, :144 `AccountApi {http_client, l2, signature_type, target}`, :159 `Notifications` and :185 `Rewards` `{http_client, l2, signature_type}`, and :199 `Auth {http_client, wallet, l2, chain_id}`.
- **Pings**:
  - gamma `api/health.rs:58-77` (G's `Stopwatch` :15), test :93 `ping_waits_on_the_shared_request_gate`;
  - data `api/health.rs:68-88` (`Stopwatch` :16);
  - clob `api/health.rs:61-82` (`Stopwatch` :16), test :137 `ping_waits_on_the_shared_request_gate`;
  - perps `api/health.rs:37-51` (decodes `Ping`, checks `status == "ok"`);
  - Binance `usdm/api/health.rs:31-35` (`Routed::<Empty>::new(&self.http, Route::Ping)`; doc :28-30 says it includes the budget wait);
  - relay `client.rs:389-394` (doc :373-388).
  - Mock tests:
    - gamma `mock_api` :1070, :1086, :1108;
    - data :907, :928, :949;
    - clob :84 `health_ping_returns_latency`, :2946, :2967;
    - perps :14;
    - Binance :62;
    - G's `a_429_on_ping_holds_the_next_request` (gamma :1442, data :1741, clob :4284).
  - Relay's ping is route 0 of `every_relay_route()` (`tests/mock_api.rs:1726-1732`), so `each_relay_route_s_429_holds_the_next_request` (:1955) pins its `Api(ApiError::RateLimit(_))`.
- **Setters**
  - The 417 hand-written query setters, by file:
    - gamma: `api/` events 88, markets 60, search 13, series 12, comments 8, tags 8, sports 7;
    - data v1: `api/` users 37, combos 12, trades 11, leaderboard 7, market_positions 6, builders 4, holders 2, pnl 2, rankings 2, misc 1, open_interest 1;
    - data v2: `v2/api/` wallet 23, feeds 22, boards 13, markets 10;
    - clob: `api/` rewards 38, account 13, orders 4, markets 1. Since G they write a `request: polyoxide_core::Request<T, ClobError>`, and the authenticated builders carry `.authenticator(self.l2.clone())` (`api/orders.rs:30`, `api/account.rs:31-33`), so I4's golden test builds `ListOrders`, `ListClobTrades`, `ListBuilderTrades` and the `Rewards` builders from a `Clob` with an `Account`;
    - Binance: `usdm/api/market.rs` 12, writing a private `request: Routed<T>` (`usdm/request.rs:21-26`, inherent `query` :39-46).
  - 64 builders own them: gamma 16, data 33, clob 11, Binance 4.
  - Argument types: `impl Into<String>` 102, `bool` 81, `u32` 63, `impl IntoIterator<Item = impl ToString>` 34, `f64` 29, `i64` 22, `impl IntoIterator<Item = i64>` 12, `u64` 9, generic `I` 9, and 54 enums and others.
  - By arm:
    - `many`: 32 (gamma).
    - `csv`: 27:
      - data v1: 14 plain, plus `combos.rs:57` and `users.rs:336`, which filter `Unknown`;
      - data v2: 2 with `impl Trait` and 9 with `<I, S>` (`v2/api/markets.rs:125`; `feeds.rs` :66, :79, :157, :170, :236; `wallet.rs` :127, :209, :330).
    - `= expr`: gamma `markets.rs:536` `open` (`!open`); clob `rewards.rs` :472, :478, :604 and :610 (`.as_str()`).
    - `if`: gamma `comments.rs:95` `parent_entity_type`.
  - Hand-written, as named in the Decisions:
    - Binance `market.rs` :156 `GetKlines::limit` and :269 `GetDepth::limit`;
    - gamma `markets.rs` :196, :202, :234, :240 and :284 (they store fields; G's `post_json` sends them through `RequestParts`).
  - Perps' `setter!` is at `api/mod.rs:31-49`, with 21 invocations: exchange 3, market 10, public 8.
  - Data v2's `csv` is at `v2/envelope.rs:139`, with tests :157 and :163. `Paged::query` (inherent) is at :87.
  - Existing key agreement:
    - data `tests/v2_spec_agreement.rs`: `query_keys_sent` :215, `ROUTES` :227, `every_route_sends_exactly_the_documented_parameters` :507;
    - perps `tests/spec_agreement.rs`: :222, :232, `every_builder_sends_exactly_the_documented_query_keys` :389.
  - `polyoxide-test-support/src/query.rs`: `keys_sent` :26-63 (it requires a decoding body). Gamma, clob and Binance take test-support without the `query` feature (`Cargo.toml` :33, :58, :41).
- **Enums, positional serde and time**
  - `polyoxide-venue`:
    - `Cargo.toml` has no dependencies, and its comment promises none until needed;
    - `lib.rs`: `#![warn(missing_docs)]` :27, re-exports :37-43, no `Classify` assertion;
    - 17 tests.
  - The macro copies:
    - gamma `types.rs`: `open_enum!` :10-77, with `ProtocolVersion` :80, `ResolutionStatus` :93 and `HomeAway` :107; 81 tests.
    - data `v2/types/common.rs`:
      - `open_enum!` :5 (`TradeSide`, `ActivitySide`, `ActivityType`, `PositionStatus`, at :149-209);
      - `closed_enum!` :78 (10 enums, at :238-378), with `&self` `as_str` and `Err = String`.
    - perps `types.rs`:
      - `UnknownVariant` :28-46;
      - `wire_enum!` :50-84 (no variant attributes), with 7 enums at :86-122;
      - 9 tests (`an_unknown_spelling_names_the_type_and_the_value` :315).
    - Binance `usdm/types.rs`:
      - `UnknownVariant` :90-106;
      - `wire_enum!` :110-145 (`Interval` :204, `DepthLimit` :216);
      - `open_enum!` :148-200 (`ContractType` :225, `SymbolStatus` :239, `UnderlyingType` :258);
      - 12 tests (:1069 builds an `UnknownVariant`).
    - relay `types.rs`: `open_string_enum!` :198-261, `pub(crate) use` :263, `TransactionState` :265 (`from_wire`, no `ALL`); 24 tests.
    - relay `session_signers.rs`: :19, :61, :86; 6 tests.
  - Other users:
    - perps `ws/channel.rs:6`, :77-83 (`Channel: FromStr<Err = UnknownVariant>`);
    - the `Classify` assertions at perps `lib.rs:40` and Binance `lib.rs:40`.
  - Positional serde:
    - perps `types.rs`: `parse_decimal` :164, `KlineWire` :190, `Kline` :192-222, `MarkPoint` :233-247, `Level` :259-273; tests :347, :358, :364, :369.
    - Binance `usdm/types.rs`: `DecimalStr` :845, `next` :853, `drain` :862, `Kline` :867-918, `Level` :1020-1053; tests :1188, :1201.
  - Unix-ms "now":
    - clob `client.rs` :346-349 and :444-447 (`u128`), with `build_order_v2` :624 (`timestamp_ms: u128` :636; test :1016);
    - Binance `weight.rs` `Clock::System` :251-253 in `Clock::now_ms` :249 (`Clock` :237; `Clock::Manual` is `cfg(test)`);
    - perps `tests/live_api.rs:22`;
    - Binance `tests/live_api.rs:100`;
    - `polyoxide-test-support/src/minute.rs:15`.
- **Tests that pin 3.9:**
  - perps `tests/{spec_agreement,wire_agreement,ws_wire_agreement}.rs` (5/1/3);
  - Binance `tests/{wire_agreement,ws_wire_agreement}.rs` (2/2);
  - gamma `tests/wire_agreement.rs` (20);
  - data `tests/{v2_enum_wire,v2_wire_agreement,v2_spec_agreement}.rs` (5/1/6);
  - cli `tests/data_v2.rs` (24);
  - `polyoxide-py` `test_stub_consistency.py` and `test_data_v2_offline.py`.
- **Gates and docs**
  - `.github/scripts/tests/test_classify_coverage.py`: `ASSERTION` :44; the known-types rows (perps and Binance list `UnknownVariant`); `test_the_vocabulary_crate_is_swept_but_classified_error_is_exempt`.
  - `scripts/live_unwraps.baseline.json`: perps `live_api` 1, Binance `live_api` 6.
  - `docs/s1-removals.md`, plus F's and G's keys.
  - `spine-amendments/epic-3.md`: F's A3-1 and A3-2, and G's A3-3.
  - CLAUDE.md:
    - Key Patterns: builders, namespaces, request fluency;
    - "both `UnknownVariant`s";
    - "Each crate's `lib.rs` lists its error types";
    - the gzip paragraph.

## Tasks & Acceptance

**Execution:**
- [ ] **I0 — before any edit, record the baseline.**
  - Take `--list` counts for every target below, and `cargo tree -e normal` for rtds and sports. At `be83cfc` (by attribute) they are:
    - core lib 133, `mock_request` 15, `send_loop` 17;
    - venue 17;
    - gamma lib 140, `mock_api` 45, `wire_agreement` 20;
    - data lib 86, `mock_api` 40, `v2_spec_agreement` 6, `v2_enum_wire` 5;
    - perps lib 79, `mock_api` 15, `spec_agreement` 5;
    - clob lib 346, `mock_api` 111;
    - relay lib 117, `mock_api` 50;
    - Binance lib 79, `mock_api` 28;
    - test-support lib 86.
- [ ] **I1 — Story 3.7: one builder.**
  - New `polyoxide-core/src/config.rs` holds `ClientConfig`. `client_config_setters!` is appended to `macros.rs`. Both are exported.
  - Core tests:
    - `a_new_config_holds_the_core_defaults`;
    - `http_builder_applies_every_knob_that_is_set`;
    - `an_unset_concurrency_takes_the_clients_default`;
    - `the_config_leaves_gzip_unset` (mockito: `Accept-Encoding` matches `gzip`);
    - `client_config_setters_set_each_knob`.
  - The six builders hold `config: ClientConfig` and invoke the macro. Binance drops `.gzip(true)` and its doc line.
  - Relay gains three knobs and keeps `url`.
  - The eleven field-reading tests read `builder.config.x`.
  - Append `test_default_concurrency_limit_is_4` to perps `client.rs` and Binance `usdm/mod.rs`. They observe the permit through `acquire_concurrency`, so CLAUDE.md's "`acquire_concurrency` stays public, because tests in … observe the permit" names perps and Binance too, as does deferred-work's `acquire_concurrency` entry (append an update; do not edit the entry).
  - Relay's new `timeout_ms` is the seam G's deferred-work entry on the session-signer timeout asks for. Append `a_session_signer_post_outlasts_the_client_timeout` to relay `tests/mock_api.rs`:
    - a client with `timeout_ms(100)`, against a server that answers a session-signer authorization after 400 ms, succeeds;
    - it fails when `parts.timeout = timeout;` in `post_json` is deleted (prove it, then restore);
    - append a deferred-work line saying that entry is done.
  - CLAUDE.md: builder pattern.
- [ ] **I2 — Story 3.7: one namespace pattern.**
  - Append `namespaces!` to `macros.rs`, with the test `namespaces_clone_the_listed_fields_and_map_one_from_another`.
  - The 34 accessors adopt it.
  - CLAUDE.md: namespaces.
- [ ] **I3 — Story 3.7: one health ping.**
  - New `polyoxide-core/src/health.rs` holds `HttpClient::health` and `Pong`.
  - New `polyoxide-core/tests/health.rs`:
    - `health_reports_the_round_trip_of_the_attempt_that_answered`;
    - `health_waits_for_the_permit`;
    - `a_non_2xx_health_is_the_callers_error`;
    - `a_429_on_health_holds_the_next_request`;
    - `health_charges_its_costs`.
  - The six pings call it, as the Decisions say. Delete G's three `Stopwatch` authenticators. Binance's ping doc loses "Includes any wait for the budget".
  - Append `a_retried_ping_reports_the_answering_attempt` to perps', Binance's and relay's `tests/mock_api.rs`: a 429 then a 200 with a 300 ms floor; the returned latency is under 300 ms while the call takes at least 300 ms. It fails on today's whole-call timing.
  - `impl RequestError for RelayError`.
  - Add A3-5 to `spine-amendments/epic-3.md`.
  - CLAUDE.md (AD-21): the "One send loop" paragraph's sentence on the pings and their private `Stopwatch` becomes `HttpClient::health`; the R8 sentence's list of direct `HttpClient::send` callers loses the pings.
  - deferred-work:
    - `Pong` and relay's `RequestError` impl;
    - [RISK] perps', Binance's and relay's pings no longer include the waits;
    - a line saying G's relay-ping-timing entry is done, and that R8's "whether a ping times only its last attempt" is settled.
- [ ] **I4 — Story 3.8: the proof.**
  - `query::pairs_sent`, with tests `pairs_sent_reads_every_pair_in_order` and `pairs_sent_does_not_need_the_body_to_decode`.
  - New `tests/query_setters.rs` in gamma, data, clob and Binance. Each holds one table-driven test, `every_setter_sends_its_key_and_value`, that names the builder and path on failure and covers every builder in the Code Map.
  - Gamma, clob and Binance take test-support's `query` feature.
  - It must be green on the hand-written setters, and must fail when one key or value is edited by hand (try one, then revert).
- [ ] **I5 — Story 3.8: the macro, and perps.**
  - Append `query_setters!` to `macros.rs`. Add `polyoxide_core::csv`, with data's two tests moved under their own names.
  - One core unit test per arm, `query_setters_<arm>`, reading `Request.query`.
  - Perps' 21 invocations convert, and `setter!` goes.
- [ ] **I6 — Story 3.8: gamma (196).**
- [ ] **I7 — Story 3.8: data (153).**
  - v1 and v2 convert. v2's `csv` goes, and v2 uses `self.inner;`.
  - The message names the `[""]` change.
  - deferred-work: the `[""]` change.
- [ ] **I8 — Story 3.8: clob (56) and Binance (10).**
  - The two Binance `limit`s stay.
  - Remove unused `QueryBuilder` imports.
  - CLAUDE.md: request builder fluency.
- [ ] **I9 — Story 3.9: one wire-enum vocabulary.**
  - Venue gains `UnknownVariant`, `open_enum!`, `wire_enum!` and `specta_as_string!`, plus serde and serde_json as dev-dependencies.
  - Venue tests:
    - `an_unknown_variant_names_the_type_and_the_value`;
    - `an_unknown_variant_is_an_invalid_request`;
    - `a_wire_enum_round_trips_and_refuses_an_unknown_spelling`;
    - `an_open_enum_keeps_an_unknown_value_verbatim`;
    - `from_wire_and_from_str_agree`.
  - The 32 call sites convert:
    - gamma 3, data 14, perps 7, Binance 5 and relay 3;
    - each `Name {` becomes `pub enum Name {`;
    - data passes `#[non_exhaustive]` and its specta `cfg_attr`;
    - gamma and data call `specta_as_string!`.
  - Delete the seven macros and two `UnknownVariant`s. Venue's `lib.rs` gains its `const _` assertion.
  - In `test_classify_coverage.py`, the regex accepts `crate::Classify` and the rows move, with a synthetic case for `crate::Classify`.
  - Removals (predicted; copy the gate's exact keys):
    - `polyoxide-perps struct_missing: struct polyoxide_perps::types::UnknownVariant (src/types.rs)`;
    - `polyoxide-binance struct_missing: struct polyoxide_binance::usdm::types::UnknownVariant (src/usdm/types.rs)`.
  - CLAUDE.md: "both `UnknownVariant`s" and the assertion sentence.
  - deferred-work: data's closed-enum `Err` and `as_str`, the new `from_wire` and `ALL`, and the new `FromStr::Err` paths.
- [ ] **I10 — Story 3.9: positional decimal serde.**
  - The `decimal` feature and `positional`, with venue tests:
    - `a_decimal_str_keeps_every_digit`;
    - `element_names_the_missing_index`;
    - `drain_skips_the_rest`.
  - Perps (removing `parse_decimal`) and Binance (removing `DecimalStr`, `next` and `drain`) adopt it.
  - Add A3-4.
  - deferred-work: perps' exponent row.
- [ ] **I11 — Story 3.9: `UnixMillis::now()`.**
  - The type, with the test `now_is_after_2026_and_never_runs_backwards_between_two_calls`.
  - The adopters adopt it, and `live_unwraps.py --lower` runs.
  - The venue metadata and `description` change, and `gen_registry.py --write` runs.

**Acceptance Criteria:**
- **The macros.** Given the workspace, when `rg 'macro_rules! (setter|open_enum|wire_enum|closed_enum|open_string_enum)'` runs, then it finds only venue's and core's definitions, and only venue defines `UnknownVariant`.
- **The setters.** Given gamma, data, clob and Binance, when a body of the form `self.(request|inner) = self.(request|inner).query(..); self` is searched for outside `polyoxide-core`, then none remains but the seven named exceptions.
- **The builders.** Given every client builder, when its knobs are searched for, then `base_url`, `timeout_ms`, `pool_size`, `with_retry_config` and `max_concurrent` come only from `client_config_setters!`.
- **The proof.** Given I4's golden tests, when they run after I8, then they pass unchanged. Data v2's and perps' `query_keys_sent` tests, and every serde round-trip and wire-agreement test, also pass unchanged.
- **The pings.** Given every venue's `ping`, when it runs, then it goes through `HttpClient::health`. Each I/O row above has a test.
- **The gates.**
  - The removal gate reports only F's keys, G's keys and I's two.
  - `test_classify_coverage.py`, `test_mutants_ledger.py`, `live_unwraps.py` and `gen_registry.py --check` pass.
- **The fence.** Given `cargo tree -e normal -p polyoxide-rtds` and `-p polyoxide-sports`, when they are compared with I0, then they are identical.

## Design Notes

```rust
impl GammaBuilder { polyoxide_core::client_config_setters!(config); }

impl DataApi {
    polyoxide_core::namespaces! { http_client;
        /// Get trades namespace
        trades: Trades,
        /// Get PnL namespace ...
        pnl: PnlApi { http_client: pnl_http_client },
    }
}

impl ListTrades {                       // data v2
    polyoxide_core::query_setters! { self.inner;
        /// Only trades by this proxy wallet.
        user: impl Into<String> => "user",
        /// Only trades in these markets ... An empty list is omitted.
        conditions: csv<I, S> => "condition",
        /// Only fills on this side.
        side: TradeSide => "side",
    }
}
// gamma: open(open: bool) => "closed" = !open,
//        parent_entity_type(t: ParentEntityType) => "parent_entity_type" if t != ParentEntityType::Unknown,
// data v1: activity_type(types: impl IntoIterator<Item = ActivityType>)
//              => csv "type" = types.into_iter().filter(|t| *t != ActivityType::Unknown),

polyoxide_venue::wire_enum! {
    /// Side of a trade or position.
    pub enum Side { Long => "long", Short => "short" }
}
```

`namespaces!` passes each accessor's attributes through (data's `approvals` keeps `#[deprecated]` and `#[allow(deprecated)]`). `query_setters!` brings `QueryBuilder` into scope as `#[allow(unused_imports)] use $crate::QueryBuilder as _;`, since on Binance's `Routed` the inherent `query` is chosen and the import goes unused under `-D warnings`.

Arm order matters: macro_rules does not backtrack inside a fragment. The literal arms (`impl Into<String>`, `many`, `csv<I, S>`, `csv`) come before the `$ty:ty` arm. Implement the list as a tt-muncher, so the field selector is in scope for each setter.

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo test -p <crate> --all-features -j 4` for venue, core, gamma, data, perps, clob, relay, binance, test-support and cli -- expected: green. This includes the README doctests.
- `cargo test -p <crate> --all-features <target> -- --list | grep -c ': test$'` -- expected, against I0:
  - core lib +5 (I1), +1 (I2), +2 csv and about 8 arms (I5); new `health` 5;
  - venue +5 (I9), +3 (I10), +1 (I11);
  - perps lib +1, `mock_api` +1; Binance lib +1, `mock_api` +1; relay `mock_api` +2 (I1, I3);
  - data lib −2 (I5);
  - test-support lib +2;
  - new `query_setters` targets: 1 each in gamma, data, clob and Binance;
  - every other target unchanged.
- `cargo clippy --workspace --all-targets --all-features -j 4 -- -D warnings`, then `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean.
- `cargo hack check -p polyoxide-venue -p polyoxide-core -p polyoxide-perps -p polyoxide-binance --each-feature --no-dev-deps`, and `cargo +1.91 check --workspace` if that toolchain is installed -- expected: clean.
- `cargo tree -e normal -p polyoxide-rtds` and `-p polyoxide-sports` -- expected: identical to I0.
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: green.
- `python3 scripts/live_unwraps.py && python3 scripts/gen_registry.py --check` -- expected: exit 0.
- `cd polyoxide-py && uv sync --reinstall-package polyoxide && uv run pytest tests/ -q` -- expected: green.
- `python3 scripts/api_removals.py check --baseline v0.38.1`, once at the end, then `rm -rf target/semver-checks` -- expected: only listed keys.

## Implementation Notes

## Spec Change Log

## Review Triage Log
