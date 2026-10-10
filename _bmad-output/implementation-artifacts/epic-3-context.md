# Epic 3 Context: One HTTP path with venue-supplied throttles

<!-- Compiled from planning artifacts. Edit freely. Regenerate with compile-epic-context if planning docs change. -->

## Goal

Stage S1. Epic 3 starts after Story 2.6 and runs in parallel with Epic 4. Today the send-and-retry loop is copied seven times: core's `Request` and `get_bytes`, clob, relay three times, and Binance. Four sends bypass every loop. `HttpClient` names `RateLimiter`, so a new window-quota venue must edit core. Relay's errors do not wrap `ApiError`, and one venue's reqwest features changed every client's headers. This epic puts every HTTP request on one loop in `polyoxide-core`, with three venue hooks: `Throttle`, `RetryPolicy` and `Authenticator`. The hold becomes throttle state. Core exports window-quota and capacity-bucket primitives, so a Kalshi-style bucket needs no core edit. Clients share one vocabulary for builders, namespaces, health pings, setters and wire enums. HTTP errors are reshaped onto `ApiError` and the eight classes, and only core depends on reqwest.

## Stories

- Story 3.1: The one send loop and its hook contract
- Story 3.2: The public window-quota table
- Story 3.3: Capacity buckets and Polymarket's composed throttle
- Story 3.4: clob on the one loop
- Story 3.5: relay on the one loop, gating holes closed
- Story 3.6: Binance on the one loop
- Story 3.7: One client builder, one namespace pattern, one health ping
- Story 3.8: One query-setter macro
- Story 3.9: One wire-enum vocabulary
- Story 3.10: One Retry-After parser and one retriable-status rule
- Story 3.11: Polymarket HTTP errors reshaped
- Story 3.12: Binance HTTP errors reshaped, and Python maps by class
- Story 3.13: Transport ownership for HTTP and venue isolation

## Requirements & Constraints

- **Behaviour is preserved unless a DRIFT row says otherwise.** Each row lands as its own commit, named for its row.
  - **R4.** Four `Retry-After` rules exist today:
    - core: f64 > 0;
    - Binance: f64 > 0, clamped to 3 days;
    - data v2: f64 ≥ 0;
    - perps: u64 only.

    One parser replaces them. Zero, negative and unparseable values never shorten the client's own backoff, and the upper clamp is a parameter. Users can see this change, so the release notes name it.
  - **R7.** Relay moves onto the loop, and `RelayError` wraps `ApiError`.
    - It drops `Api(String)`, and the `Core` and `RateLimit` variants that are never constructed.
    - It gains mock tests for 429 retry and hold on each route.
  - **R8.** Four requests skip the loop today:
    - gamma's `post_json`, which takes the limiter before the permit, never retries and gives no 429 feedback;
    - the clob ping, which is ungated;
    - the gamma and data pings, which skip 429 feedback.

    All four go through the loop or core's `health`, and a test pins each one.
  - **R10.** Every venue logs `Retriable status <code> on <path>, retry <n> after <ms>ms` at `WARN`, under the target prefix `polyoxide_core`. Holds and bans that are not retries warn there too. The soaks match `Retriable status 429`, so keep that line, or move the soaks to a structured signal in the same change.
- **CAP-1 mutants.** Each must fail a test and be recorded in `docs/MUTANTS.md`:
  - dropping the hold on a last-attempt 429;
  - skipping `observe` on the last attempt;
  - a policy returning a zero wait.

  Story 3.1 also closes rule (a)'s "not yet covered" sites.
- **CAP-2 gate.** A test outside core builds a Kalshi-style throttle and passes with no edit to core. The throttle has separate read and write buckets and integer token costs.
- **Removals.** Consolidated items break at their old paths, with no shim or re-export. Examples:
  - clob's `request` module;
  - `WeightedRequest`;
  - the per-crate `UnknownVariant`s;
  - `impl_api_error_conversions!`;
  - `retry_after_header`;
  - `HttpClient`'s loop-building methods.

  Every removed or changed public item goes in `docs/s1-removals.md`, which is cumulative against `v0.38.1`. Error enum names do not change until S2. Changed variants are listed for prader.
- **Test custody.**
  - Moved tests keep their names and assertions. A moving PR reports suite counts before and after it, and re-runs each `docs/MUTANTS.md` mutant at its new file and line.
  - These suites are protected:
    - `documented_*_limits`;
    - the limiter, cooldown and `Retry-After` rules;
    - `classify_order_kill` and the burst-capacity tests;
    - spec agreement (including `query_keys_sent`) and wire agreement;
    - the serde round-trips;
    - the Story 2.2 class tables;
    - the Python stub and getter guards.
  - Three rules have no mutant on record but are still protected: the signer layer's `allow_burst`, `RESERVED_FRACTION`, and Binance's 418 hold.
  - A suite that S2 will move must assert only through public API first. That rewrite is its own commit, as with Story 3.2's move to `effective_quota`.
- **Standing rules.** CLAUDE.md is updated in the change that supersedes each rule:
  - Story 3.1 rewrites "`note_rate_limited` before `should_retry`" and the text on the 429/425 retry set.
  - Story 3.11 rewrites the `impl_api_error_conversions!` paragraph.
- **Isolation.**
  - Each HTTP module has one mock test pinning a request's full header set, `Accept-Encoding` included.
  - It runs in the venue crate alone, with minimal features, and under `cargo test --workspace --all-features`. Both runs must give identical headers.
  - Builders leave `gzip` unset and keep their default concurrency of 2, 4 or 8.
- **New CI checks.**
  - Only core depends directly on reqwest. Alloy's transitive copy does not count.
  - No venue crate depends directly on governor.

## Technical Decisions

- **Hooks, all in core.**
  - `Throttle`:
    - `acquire(&RequestMeta { method, path, query, costs: &[Cost] }) -> Result<Charge, Refused>`, async;
    - `observe(&Charge, &ResponseMeta { status, headers }, &AttemptInfo { attempt, retries_left })`, sync, run on every response;
    - `hold(delay)`, sync.
  - `RetryPolicy::decide(&ResponseMeta, &AttemptInfo, schedule: &RetryConfig) -> Decision { outcome: Done | Retry(wait) | Fail, hold: Option<Duration> }`, sync (A3-1). The schedule is the loop's own `RetryConfig`, so a policy sizes its hold from the one schedule the client was configured with. It never keeps a copy, and it can never shorten the loop's floor.
  - `Authenticator::sign(&mut RequestParts { method, path, query, headers, body, timeout }, attempt) -> Result<(), ApiError>`, async (A3-2). A signing failure ends the request before anything is sent.
  - `RequestParts::timeout: Option<Duration>` bounds each attempt in place of the client's timeout, and `None` keeps the client's (A3-3). It is per attempt, like the client's own timeout. Relay's two session-signer posts set 300 s, because the venue broadcasts the batch before answering.
  - All three:
    - write async methods as `fn … -> impl Future<Output = …> + Send`;
    - are `Send + Sync`, never `'static`, with no associated consts;
    - carry `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`, held as `Arc<Dyn<Trait><'static>>`.

    No venue's error type enters a hook signature, so each `Dyn` wrapper stays one type the loop holds. If dynosaur fails the MSRV, clippy or rustdoc gates, every trait moves to async-trait in one change.
- **The loop.**
  - Each attempt runs permit, `acquire`, `sign`, send, `observe`, `decide`, hold, log.
  - It sleeps `max(backoff floored by Retry-After, wait)`. The floor belongs to the loop, never to a policy.
  - It calls `hold(h)` once whenever `hold` is `Some`, retried or not.
  - It releases the permit before sleeping.
  - A transport error that gets no response skips `observe` and `decide`, is not retried, and is classed `Network`.
  - `Fail` returns `ApiError`, carrying status, headers, body and the parsed `Retry-After`. `Refused` returns `ApiError::Refused`.
  - A signing failure is returned unchanged as `ApiError::Sign(Box<dyn Error + Send + Sync>)`. It is classed `InvalidRequest` and never retried (A3-2). A venue's authenticator boxes its own error into `Sign`, and the venue's `From<ApiError>` downcasts it back, so an L1 signer failure still reaches the caller as `ClobError::Alloy`.
  - `get_bytes` and decode-and-log (H3) use the same loop.
- **Health ping (A3-5).**
  - `HttpClient::health::<E: RequestError>(&self, path: &str, costs: &[Cost]) -> Result<Pong, E>`.
  - `Pong { round_trip: Duration, response: reqwest::Response }` is `#[non_exhaustive]`.
  - It sends one `GET` on the send loop, charging `costs`, so a ping takes the permit, the throttle, the retry and the hold. Binance passes its ping's weight this way, because a request-counting layer cannot find it from the path.
  - `round_trip` covers only the attempt that answered, timed from its signing by a core-private authenticator. Every venue's latency therefore leaves out the waits, which settles R8's open question.
  - The response comes back unread, because perps checks `{"status":"ok"}` and Binance checks `{}`. A final non-2xx becomes `E::from_response`.
- **Retry sets (AD-17, D17).**
  - Core's default policy retries 429.
  - Polymarket's one policy retries 429 and 425, and gamma, data, perps, clob and relay all share it.
  - No policy retries 5xx or 408, because order POSTs are not idempotent. Narrowing a policy needs a DRIFT row.
  - `polyoxide-venue`'s 408/425/429/5xx rule answers callers only.
  - `RetryConfig::backoff` is never unified with the socket `Backoff` (D13).
- **Holds (AD-9, AD-23).**
  - Only a 429 or a venue ban sets a hold.
    - A 425 waits for its own request only.
    - Every 429 holds, even with no retry left.
    - Core's 429 hold is `retry_delay(0)`, taken from the schedule `decide` receives, and its wait is `retry_delay(attempt)`.
  - Binance:
    - a 429 with a retry left holds for `max(wait, Retry-After)`;
    - a 429 with none left holds for its `Retry-After`, or until the next UTC minute when it has none;
    - a 418 is `Fail`, holding for its `Retry-After`, or 2 minutes when it has none;
    - a 425 is not retried.
  - `HttpClient` holds no cooldown.
  - `acquire` waits out the hold before charging, and re-checks it after any wait of its own.
  - A hold only ever extends. It is one handle shared by every layer, and it survives replacing or resizing a layer.
  - A shared throttle shares its hold: `with_base_url` siblings, and every `Usdm` on one `WeightBudget`.
  - `observe` records counts and tiers only.
  - The ceiling is a parameter of each throttle (3 days for Binance).
- **Costs (AD-10).**
  - `Cost { layer: LayerId(&'static str), units: u32, exact: bool }`.
  - `Charge` is opaque to the loop. It holds `LayerCharge { layer, units, window: Option<u64> }`, and for Binance `window` is the UTC minute.
  - `Refused { layer, units, capacity }`.
  - A request-counting layer finds its bucket from method and path and charges 1 per attempt. Every other layer charges exactly its `costs` entry.
  - The request builder computes `costs` from its route table, never the throttle.
- **Bucket models.**
  - Window quota: depth 1, a tenth reserved, never `allow_burst` (D1, D2).
  - Capacity bucket: `allow_burst(capacity)`, which the per-signer layer keeps (D1). Making the two alike restores the 2x over-permit.
  - Binance's weight minute stays in `polyoxide-binance` and uses only core's hold (D3).
- **Core's public primitives.**
  - The `WindowQuotaTable` builder:
    - buckets shared across routes (the ledger bucket spans `/trades`, `/orders`, `/notifications` and `/order`);
    - prefix and exact matching, scoped by method;
    - `effective_quota(method, path)`, which returns every bucket a request awaits, including the general bucket.
  - The capacity bucket refuses any cost it can never hold. Its `resize(capacity, refill)` keeps the tokens, clamped, and keeps the hold.
  - A provisionally sized throttle waits rather than returning `Refused`.
  - Binance's funding bucket uses `WindowQuotaTable`.
- **Polymarket in S1.**
  - Its tables, signer layer, policy and hooks live in core's `polymarket` module.
  - Its composed throttle charges Cloudflare 1 request and the signer layer N orders. `observe` adopts `Poly-RateLimit-Tier` on every status.
  - Each decode maps `ApiError::Refused` to the module's existing variant, such as `BurstCapacityExceeded`, classed `InvalidRequest`.
- **Authentication.**
  - Clob's L2 `Authenticator` signs every attempt, and L1 signing runs async inside `sign`.
  - Clob's and relay's credentials are held in `Secret<T>`.
  - Relay's alloy gas estimation stays outside the loop, as the documented exception.
- **Shared vocabulary.**
  - Core provides the builder macro (H6) and the namespace accessors (H7). The health ping (H8) is the `health` method above.
  - Core also provides a query-setter macro promoted from perps' `setter!` (H9). Every setter keeps its name, its argument type and its query key.
  - `polyoxide-venue` provides `open_enum!`, `wire_enum!`, `UnknownVariant`, positional decimal serde and `UnixMillis::now()` (H10, H11, H16).
  - Perps and Binance keep separate `Interval` enums (D16).
- **`polyoxide-venue`'s dependencies (A3-4).**
  - Its default build depends on nothing, so rtds's and sports's `cargo tree -e normal` stay unchanged.
  - A `decimal` feature turns on `rust_decimal` (with `serde-with-str`) and `serde` for `polyoxide_venue::positional` (`DecimalStr`, `element`, `drain`). Perps and Binance enable it.
  - `wire_enum!`, `open_enum!` and `specta_as_string!` expand to `::serde` paths, so the crate that invokes them depends on serde itself. `wire_enum!` also needs serde's `derive` feature; `open_enum!` implements serde by hand.
  - Venue takes serde and serde_json as dev-dependencies only.
  - Any later dependency arrives behind a feature whenever a socket-only crate would otherwise build it.
- **Retry-After.**
  - Venues switch to Story 2.1's parser, and Binance passes its 3-day clamp.
  - Tests pin each case where the old parsers disagreed.
  - The three copies of the retriable-status rule (H12) go.
- **Errors (AD-15).**
  - **Shape.**
    - Each HTTP enum wraps `ApiError` with `#[from]`.
    - Each decodes its venue's body in one function (D14):
      - Polymarket: `error` or `message`;
      - data v2: its envelope;
      - perps: `{status, error, ref}`;
      - Binance: `{code, msg}`.
    - There is no catch-all variant.
    - Each `From<ApiError>` keeps the existing mappings back out: `Refused` to the module's variant, and `Sign` downcast to the venue's own error.
  - **Classification.**
    - The status decides the class before the body, through `polyoxide-venue`'s map.
    - The only override is Binance's WAF 403, which becomes `Restricted` (D14).
    - `code` holds the venue's own code, never the HTTP status.
  - **Kills (D15).** FAK and FOK kills are `VenueRefusal`. They are not faults and are never retried, and `classify_order_kill` is unchanged.
  - **Faults (A2-1).** A 451 is not a fault. A 418 ban and a WAF 403 are, so they tag `real`.
  - **Server hints.** A server's `retryable` flag is surfaced, not obeyed.
  - **Reshape changes.**
    - The reshape removes the inherent `is_retriable` and `retry_after`.
    - `ApiError::Validation` splits: a server 400 becomes `VenueRefusal`, and clob's local validation becomes `InvalidRequest`.
  - **Python.** Exceptions follow `Class` one to one, replacing the substring mapping (C4). Data v2 errors still map by `code`.

## Cross-Story Dependencies

- **Prerequisites.**
  - Story 2.6.
  - Story 1.7's removal gate.
  - Stories 2.1 and 2.2: `Classify`, the status rule, the parser and `Secret<T>`.
- **Epic 2's helpers.** Stories 2.8–2.10 have landed them, and the inventory marks T1–T9 resolved.
  - Story 3.8's `query_keys_sent` checks use test-support's `query` feature, and Story 3.9 uses the shared wire-agreement helpers.
  - Story 2.10's soak observer matches R10's line.
- **Within Epic 3.** Story 3.1 comes first.
  - Stories 3.2 and 3.3 build on its `Throttle` and hold.
  - Stories 3.4 to 3.6 build their policies on A3-1's `decide` signature.
  - Story 3.4 needs 3.3's composed throttle and A3-2's `ApiError::Sign`.
  - Story 3.6 needs 3.2's table.
  - Story 3.5 uses 3.7's `health` if it has landed, and the loop otherwise, and A3-3's per-attempt timeout.
  - Story 3.11 needs 3.1's `ApiError` and the `Refused` and `Sign` mappings from 3.3 and 3.4.
  - Story 3.12 follows 3.6 and 3.11.
- **Epic 4.**
  - It runs in parallel, and its kit takes the venue status rule as an injected function.
  - Story 4.10 folds its tungstenite and rustls check into Story 3.13's, if that check exists.
  - Story 4.11 cuts S1 once Epics 1–4 are done, with notes naming R4 and the moved log targets.
- **Spine merge.** Amendments A2-1 and A3-1 to A3-5 are accepted but not yet merged into the spine; one later session merges them and regenerates the guide. Until then they override the spine where the two differ.
- **Epic 5.**
  - Story 5.2 moves core's `polymarket` module into `polyoxide-polymarket`, with empty test-body diffs.
  - Error enum renames, such as `BinanceError` to `UsdmError`, wait for S2.
