# Rubric review: ARCHITECTURE-SPINE.md (polyoxide venue extensibility)

Reviewed 2026-10-08 against the nine-point good-spine checklist. Spine line numbers refer to
`ARCHITECTURE-SPINE.md` as of this review. Code references are to the worktree at `e3d8c3e` plus
staged planning files.

**Verdict: not ready to hand to parallel epics.** The skeleton is sound and its brownfield
anchors mostly check out. Eight high findings remain, all at seams that separate epics build
independently. Four are in the S1 kernel: cooldown semantics, the `Throttle` signature, the
test-support edges and the classification classes. The other four are coverage gaps: inventory
rows with no home, the AD-2 check, the AD-12 suite list and the AD-14 tagging mechanism. There
are no critical findings.

## Summary

| Id | Sev | Checklist | One line |
|---|---|---|---|
| RB-01 | high | 5, 6, 8 | AD-8/AD-9 cooldown changes today's behaviour; the final-attempt 429 hold is undefined for core and Polymarket; CAP-1's mutation test contradicts AD-8's order |
| RB-02 | high | 1, 2, 8 | The `Throttle` contract cannot be implemented as written: return type, `Charge` type, multi-layer cost and cooldown ownership are all open |
| RB-03 | high | 8, 5 | AD-1's "only these edges" forbids edges that AD-14, the CLI and Python need; the test-support arrows point the wrong way |
| RB-04 | high | 1, 6 | AD-15 has no class for 5xx, decode failures or timeouts; `is_fault` and the interface's methods are undefined |
| RB-05 | high | 1, 6 | Inventory rows H3, H6–H9 and H16 (H9 is about 2,000 lines) have no home, nor do W5, W6, W9, W11 or R3's bare tier |
| RB-06 | high | 2, 6 | The AD-2 allowlist passes whatever a module declares, and only rtds and sports are checked |
| RB-07 | high | 6, 5, 2 | AD-12 omits the rtds and sports supervision suites (about 46 tests) and the golden-vector and binding guards; "re-mutated" cannot be checked |
| RB-08 | high | 2 | AD-14's panic hook cannot classify an `unwrap()`; the regex fallback has no sunset |
| RB-09 | medium | 5 | AD-17 does not say which Polymarket modules retry 425; today every core-loop crate does |
| RB-10 | medium | 1, 8 | S1's interim homes for Polymarket hooks, socket type names and error-enum renames are all undecided |
| RB-11 | medium | 2, 7 | AD-13: the venue-id check has no data source, nightly rows cannot be split per suite, SELF-HEALING.md is missing, and Kalshi's touch list is unsatisfiable |
| RB-12 | medium | 4 | AD-5 uses pre-0.3 dynosaur syntax that 0.3.1 rejects; `Send + Sync` supertraits and associated consts are unaddressed |
| RB-13 | medium | 2 | AD-9's "never shorten" floor is not owned by the loop |
| RB-14 | medium | 1, 3 | The trading trait's event-stream shape is unfixed; the streaming deferral leaves it open |
| RB-15 | medium | 1, 6 | The capacity-bucket primitive (CAP-2's Kalshi bucket and the signer layer) has no home |
| RB-16 | medium | 7 | Umbrella migration: the fate of `Polymarket`/`PolymarketBuilder`, the per-module features and `full` is undecided |
| RB-17 | medium | 7 | Release mechanics: retired crate names, version-bump ownership and the S2 path map |
| RB-18 | medium | 6 | No rule binds the DFR rows; D13 is at risk |
| RB-19 | medium | 7 | Kalshi demo-host credentials in nightly; the CAP-7 grep gate's pattern and scope |
| RB-20 | medium | 1 | `MarketKey`'s wire form (serde, FromStr, case, parse rule) is undecided |
| RB-21 | medium | 1 | `Extensions` bounds and record derives are undecided |
| RB-22 | low | 1 | AD-8: `RequestParts` has no headers; transport errors without a response are unspecified |
| RB-23 | low | 5 | AD-9 misstates the condition for Binance's next-minute hold |
| RB-24 | low | 2 | AD-18's second build cannot include Kalshi |
| RB-25 | low | 7 | AD-22's gates must be jobs in the `CI` workflow to withhold the tag |
| RB-26 | low | 5 | Conventions vs brownfield: CLI depth, credential kinds, `keychain`/`parquet`/intra-crate `gamma` features, error-enum naming |
| RB-27 | low | 8 | The Python "map by class" convention vs today's data v2 mapping by `code` |
| RB-28 | low | 8 | AD-21's table reads as exhaustive but is not; precedence between spine and guide is unstated |
| RB-29 | low | 4 | Stack omits named tech the ADs rely on |
| RB-30 | low | 9 | A few rationale fragments; spec amendments are recorded only in the memlog |
| RB-31 | low | 7 | No Open Questions section |

---

## High

### RB-01 — high — Cooldown semantics: today's behaviour changes, the final 429 hold is undefined, and CAP-1's test is inverted

**Evidence.**

- **Spine.**
  - AD-8 (l.150): "The loop then applies **one** client-wide cooldown, equal to the retry delay
    or the `Fail` hold, unconditionally."
  - AD-9 (l.170-171) defines a `Fail` hold only for a 418 and for Binance's last 429.
  - The AD-8 diagram (l.159-160) puts the cooldown *after* `policy.decide`.
- **Code today.**
  - `polyoxide-core/src/client.rs:171-176`: `note_rate_limited` "is a no-op for any status other
    than 429". A 425 retry does not cool the client.
  - `client.rs:178`: the cooldown is `retry_delay(0, …)`, attempt 0. The request's own wait is
    `retry_delay(attempt, …)` (`:136`).
  - `request.rs:151-155`: the cooldown runs "before `should_retry`, and unconditionally", so a
    request that is out of attempts still cools the client.
  - No test pins that last behaviour in core. `tests/mock_request.rs:77`
    (`exhausts_retries_returns_rate_limit_error`) builds no limiter.
- **Spec.** CAP-1 success: "A mutation test fails if 429 feedback moves after the retry
  decision." AD-8 now places the feedback, the cooldown, after the decision by design.

**Why units diverge.** If the core epic implements AD-8 literally, three things change silently:

- Core's default policy returns `Fail { hold: None }` on the last 429. This drops the rule
  CLAUDE.md calls load-bearing (about 16 doomed sibling requests prolonging a Cloudflare 1015 ban).
- Every Polymarket 425 retry freezes the whole client, cancels included.
- Cooldowns grow with the attempt number.

Neither DRIFT row lists any of these changes, and the CAP-1 test cannot be written as the spec
words it.

**Fix.** Amend AD-9:

- "Every 429 produces a client-wide cooldown, retried or not. On `Fail` the hold is at least the
  delay a retry would have had (core: `max(backoff(0), Retry-After)`)."
- "Only 429 and the venue's ban or hold statuses (418) cool the client. A 425 `Retry` waits
  per-request only." Alternatively, record the change as a DRIFT-style decision with a
  release-notes line.
- State whether the cooldown uses `backoff(0)` or `backoff(attempt)`.

Restate CAP-1's test in AD-8's terms. A mutant that drops the `Fail` hold on a last 429 fails a
test, and so does a mutant that applies the cooldown after the sleep. Add a core test that pins
sibling backpressure after retries run out (AD-12 custody).

### RB-02 — high — The `Throttle` contract cannot be implemented as written

**Evidence.**

- **Return type.** AD-8 (l.146) gives `acquire(&RequestMeta{…}) -> Charge`, but AD-10 (l.184)
  says "`acquire` refuses, as non-retriable, any cost a layer can never hold". It needs a
  `Result`, and the refusal's error type and class are unnamed.
- **The `Charge` type.** AD-10 (l.179) holds the throttle as one `Arc<DynThrottle>`, so `Charge`
  cannot be an associated type: a dyn type would need `Charge = X` per venue. Yet Binance's
  charge must carry its UTC minute (`binance/src/usdm/request.rs:94,104`
  `record_used(charge, used)`), and Polymarket's must say which layers were charged.
- **Cost.** AD-10 (l.180): "`cost` is per-layer units: a venue-declared layer id, `u32` units,
  and an `exact` flag". That is one triple. A clob order POST charges the Cloudflare layer
  (1 request) and the signer layer (N orders). Is it a list? What do layers it omits charge?
- **Who computes cost.** Memlog l.27 says "request builders compute the request's cost". The
  spine does not say this.
- **Cooldown ownership.** The `Throttle` has only `acquire` and `observe`, yet AD-8 says "the loop
  applies" a cooldown. AD-10 (l.185) puts the cooldown inside the window-quota engine, and
  today's cooldown is awaited inside the limiter's `acquire` (`core/src/rate_limit.rs:381`).
  Binance's `WeightBudget` has its own cooldown (`begin_cooldown`, `hold_until_next_minute`).

**Why units diverge.** The core epic writes the trait. The Polymarket, Binance and Kalshi throttle
epics each need a different shape, and the first to land fixes it for the others.

**Fix.** Write the signatures into AD-8:

```
acquire(&RequestMeta{method, path, query, cost: &[LayerCost]}) -> Result<Charge, CapacityExceeded>
```

- Layers absent from `cost` are charged 1 if they count requests and 0 otherwise.
- `Charge` is a concrete core type carrying an opaque venue payload.
- `CapacityExceeded` is classified `InvalidRequest` and is never retriable.
- The cooldown is one core primitive held by the loop. It is awaited after the permit and before
  `acquire`.
- Throttles never keep their own cooldown. Binance's next-minute hold becomes a `Fail` hold.
- Request builders compute `cost`.

### RB-03 — high — AD-1's graph forbids edges that other ADs need, and the test-support arrows are reversed

**Evidence.**

- AD-1 (l.68): "Only the edges in the diagram exist."
- The diagram (l.59-60) draws `ts -.-> pm` and `ts -.-> bn`. Every other arrow means "depends
  on", so this reads as test-support depending on the venue crates.
- If built that way, it is a dev-dependency cycle in which the venue crate's types differ between
  its own test build and test-support.
- Test-support's AD-14 duties need edges the diagram lacks:
  - the class reporter needs the classification interface, so `ts → polyoxide-venue`;
  - the credential loaders over `StoredCredential` need core's keychain, so `ts → polyoxide-core`;
  - the soak harness needs core's `effective_quota`.
- The facades need missing edges too:
  - the CLI's `credentials` command is generic over `StoredCredential` in core (Conventions
    l.378), and today the CLI depends on core directly (`polyoxide-cli/Cargo.toml`, feature
    `keychain`);
  - Python maps errors by classification class (l.383), which needs the trait in scope.
- `polyoxide-kalshi` has no edges in the diagram, so under l.68 it may depend on nothing.

**Fix.** Redraw the test-support arrows as `pm -.dev.-> ts` and `bn -.dev.-> ts`, and add
`ts → venue` and `ts → core` (with `keychain`). State that test-support never depends on a venue
crate.

Either add facade → venue and facade → core edges, or add the rule "each venue crate re-exports
`polyoxide_venue` and the core types its public API names". Then phrase l.68 per layer: "a venue
crate's edges are core, ws and venue". That covers Kalshi.

### RB-04 — high — The classification interface leaves the commonest errors unclassified

**Evidence.**

- AD-15 (l.283) has six classes: `Network`, `RateLimited`, `Unauthorized`, `InvalidRequest`,
  `VenueRefusal{code}` and `Restricted`, plus `is_fault`.
- Nothing classifies:
  - **5xx or upstream unavailable.** prader's `perps_err` has `UpstreamUnavailable`
    (`venue-landscape.md:53`).
  - **A decode failure on a 2xx.** This is schema drift; `Request::send` maps serde errors to
    `ApiError` at `core/src/request.rs:114-118`.
  - **A timeout.** Python has a `TimeoutError` exception (`polyoxide-py/src/error.rs:11`), and
    data v2 has `ErrorCode::RequestTimeout`.
- `is_fault` is defined only negatively, for kill outcomes (AD-20).
- The interface's method set is not written down: class, `is_retriable()` (AD-3) and
  `retry_after()`. Nor does the spine say whether the class enum is `#[non_exhaustive]`, which
  decides whether prader can match it exhaustively, CAP-4's goal.

**Why units diverge.** Seven module epics each map a 503 and a serde error onto some class: one
picks `Network`, another `VenueRefusal{503}`. The generic consumer classifier CAP-4 promises then
gives different answers per venue.

**Fix.**

- Add `Upstream { status }` for 5xx and `Decode` for an unparseable success body.
- State that a timeout is `Network`.
- Define `is_fault`: true unless the venue answered as designed.
- Fix the interface: `fn class(&self) -> Class`, `fn is_retriable(&self) -> bool` (the AD-3 rule
  over status and class), `fn retry_after(&self) -> Option<Duration>`, and a venue code typed as
  `Option<Arc<str>>`.
- Decide whether `Class` is exhaustive.

### RB-05 — high — Duplication-inventory rows with no home

**Evidence.**

- The Shared-code homes table (l.385-398) says it places "the duplication-inventory rows the ADs
  above do not place". It omits:
  - H3: the decode-and-log helper;
  - H6: builder knobs (`base_url`, `timeout_ms`, `pool_size`, `with_retry_config`,
    `max_concurrent`, limiter, gzip), which CAP-1's success line names: "No crate defines its own
    … builder knobs";
  - H7: namespace accessors (about 430 lines);
  - H8: the six hand-written `ping`s;
  - H9: query setters (about 2,000 lines, the largest row);
  - H16: the Unix-ms "now".
- Core's `macros.rs` holds only `impl_api_error_conversions!`, so no setter or knob macro exists
  to ratify.
- The socket rows W5 (deadline arithmetic), W6 (the error wrapper), W9 (close capture) and W11
  (the bare-tier `poll_next`) are missing from `polyoxide-ws`'s seed contents (l.428). So is
  DRIFT R3's "shared bare tier" that sends the close reply.
- The success signal requires that "every row … has exactly one definition".

**Why units diverge.** The S2 Polymarket consolidation and the S1 Binance migration each invent a
builder and setter vocabulary, which leaves two definitions.

**Fix.** Add these rows:

| Rows | Home |
|---|---|
| H6 | a core client-config type plus builder macro (keeping each client's default concurrency, AD-18) |
| H7, H9 | core macros (HTTP-only, so not `polyoxide-venue`) |
| H8 | a core `health(path)` that goes through the send loop |
| H3 | the core send loop |
| H16 | `UnixMillis::now()` in `polyoxide-venue` |
| W5, W6, W9, W11, R3's close reply | the `polyoxide-ws` bare tier |

### RB-06 — high — The AD-2 check passes whatever a module declares, and covers two modules

**Evidence.**

- AD-2 (l.82) checks the tree "against an allowlist: the `polyoxide-ws` dependencies, plus the
  `polyoxide-venue` dependencies, plus that module's declared dependencies".
- If `rtds = [..., "dep:polyoxide-core"]` is declared, core and reqwest are "declared
  dependencies" and the check passes. There is no deny-list.
- Only `-F rtds` and `-F sports` are checked. The SPEC constraint is broader: "Credential-free
  modules must not pull in signing stacks".
- After consolidation, gamma, data, perps, perps-ws, usdm and usdm-ws share a crate with clob's
  `alloy` and relay's HMAC. Today they are separate crates and cannot reach alloy at all.
- The spine also widens the spec's "nothing more" by `polyoxide-venue`'s dependencies. Sports
  gains `rust_decimal` and `dynosaur` (with syn); its `Cargo.toml` has neither today. Memlog
  l.58 records this, but no spec amendment does.

**Fix.**

- Commit a per-module allowlist file. CI diffs `cargo tree -e normal` against it.
- For socket-only modules, add a global deny-list: `reqwest`, `alloy*`, `polyoxide-core`,
  `governor`, `hmac`, `sha2`, `keyring`.
- For every credential-free feature, add a deny-list check (`alloy*`, `keyring`, `hmac`).
- Record the polyoxide-venue widening as a spec amendment.

### RB-07 — high — AD-12's protected list misses suites at risk in S1 and S2

**Evidence.**

- AD-12 (l.218) lists supervision as "perps inline (17), Binance `tests/supervision.rs`,
  `tests/supervision_edges.rs`, and the usdm ws client inline".
- It omits these, all of which move onto `polyoxide-ws` kit blocks in S1:

  | Crate | Location | Tests |
  |---|---|---|
  | rtds | `tests/supervision.rs` | 4 |
  | rtds | `src/supervisor.rs` | 15 |
  | sports | `tests/supervision.rs` | 15 |
  | sports | `tests/bare.rs` | 6 |
  | sports | `src/supervised.rs` | 6 |

  DRIFT R1 and R2 change rtds's reconnect, and CAP-3's success line says "Existing supervision
  tests pass with behaviour unchanged".
- It also omits:
  - the EIP-712 and session-key golden-vector tests (`clob/src/core/eip712.rs`,
    `tests/fixtures/session_keys/order_vectors.json`), which move with `Signer` in S2;
  - the Python guards `test_stub_consistency.py` and `every_v2_getter_reads_its_own_key`, which
    S2's module rename touches;
  - `.github/scripts/tests/test_diff_openapi.py` (56 tests), exercised by AD-13's
    watch-list derivation.
- "mutation-tested rules are re-mutated afterwards" (l.227) names no tool and no mutant list. The
  repo has no cargo-mutants config, so a reviewer cannot check it.
- "perps inline (17)": `polyoxide-perps/src/ws/supervised.rs` has 21 `#[test]`/`#[tokio::test]`
  (17 supervision plus 4 Backoff). Per-suite counts "before and after" will not match the
  stated baseline.

**Fix.**

- Make the list open: every moved test is protected. Name these suites in addition: rtds and
  sports supervision and bare, the golden vectors, the Python guards and `test_diff_openapi`.
- For each mutation-tested rule, record its mutant (file, line, the change, the test expected to
  fail) in `docs/ARCHITECTURE.md`. "Re-mutated" then means "each listed mutant still fails its
  test".
- Restate the perps count as 21 (17 + 4).

### RB-08 — high — AD-14's panic hook cannot produce the `transient` tag

**Evidence.**

- AD-14 (l.263): "A `polyoxide-test-support` panic hook prints `polyoxide-class=<tag>`". Memlog
  l.64 adds "tags failures without per-call discipline".
- A panic hook receives only the payload. `unwrap()` on a `Result<_, ClobError>` has already
  formatted the error into a `String`, so the type and its classification are gone. Deriving
  `transient` from `is_retriable` therefore needs an explicit test-support call at each fallible
  site, or the hook is back to parsing Display text.
- nextest runs each test in its own process, so something must install the hook in every test
  binary. AD-14 does not say what.
- l.272: "The regexes remain as a fallback until every live test reports through test-support."
  There is no stage, no check, and no rule against a new venue adding regexes, which CAP-8 forbids.

**Fix.**

- Specify the mechanism: a test-support extension trait (`.or_report()`) or a `live_test!`
  wrapper. It panics with the tag computed from the classification interface and installs the
  hook.
- AD-13's CI check fails a `tests/live_*.rs` that does not use it.
- Add: "No new regexes. The fallback is deleted in S2, when the last pre-restructure live file
  moves."

---

## Medium

### RB-09 — medium — AD-17 does not say which Polymarket modules retry 425

**Evidence.**

- AD-17 (l.304): "Core's default `RetryPolicy` retries 429 only. Polymarket's adds 425."
- Today `should_retry` retries 425 for every crate on core's loop (`core/src/client.rs:131`):
  gamma, data, perps, clob and relay (relay calls it at `client.rs:297,395,1840`).
- The data soak classifies 425 retries (`data/examples/common/mod.rs:271,296`).
- D17 calls 425 "Polymarket's matching-engine signal", which suggests clob only.

**Fix.** Name the modules that install Polymarket's policy. Either all Polymarket modules (no
behaviour change), or clob only, recorded as a user-visible change in the release notes.

### RB-10 — medium — S1's interim decisions are left open

**Evidence.**

- **Hook homes.** Memlog l.63: "in S1 its hooks live in core or the existing crates". The spine
  dropped the sentence, leaving the choice open. The S1 epics need one home for Polymarket's
  `RetryPolicy`, its window tables and the signer layer: the send loop, the R7 relay move and the
  R8 gating. Otherwise there are five copies, one per crate.
- **Socket types.** AD-16 (l.291) says "public paths do not" change in S1, but S1 migrates perps
  and Binance onto a generic `Supervisor<P>`. Nothing says whether `SupervisedPerpsWs`,
  `SupervisedUsdmWs` and `MembershipHandle` keep their names (as newtypes or aliases) or become
  `Supervisor<…>`.
- **Error-enum names.** AD-15's example `ClobWsError` implies renames:
  `polyoxide-clob`'s `WebSocketError`, `BinanceError` → `UsdmError` per the module convention,
  and `DataApiError`. AD-21 puts AD-15 in S1, yet AD-16 puts "every public rename" in S2.

**Fix.**

- "In S1, Polymarket's hooks live in `polyoxide-core` under a `polymarket` module, and leave in
  S2."
- "S1 keeps every public socket type's name and path."
- "Error-enum renames are S2. S1 changes only variants."

### RB-11 — medium — AD-13's registration is missing data and its touch list cannot be met

**Evidence.**

- **Venue ids.** The id-collision check (l.250) has nothing to read. AD-4 makes venue ids Rust
  `const`s, and the `[package.metadata.polyoxide]` fields (l.238) do not include a venue id.
- **Nightly granularity.** "nightly timeout and suite" is one per crate. Today clob needs two
  rows: `session-keys` gets 40 minutes against the others' 15
  (`nightly-behavioral.yml:51-52`). One Polymarket crate with one timeout brings back the problem
  that row was split to fix.
- **SELF-HEALING.md.** It holds a third copy of the schema exclusions (`:112-123`) and a CI
  onboarding recipe (`:165-178`) that AD-14 and AD-13 make wrong. It is neither generated (l.244)
  nor listed in AD-21.
- **Kalshi's touch list.** l.251-256 omits:
  - `[workspace.dependencies]`, where the workspace pins every crate and dependency
    (`Cargo.toml:26-34`);
  - `Cargo.lock`;
  - `docs/ARCHITECTURE.md`, which CAP-11's success line requires the skeleton to amend.

**Fix.**

- Add `venue = "<id>"` and `products = [...]` to the metadata. A unit test asserts that the
  `const`s equal `env!`-read metadata.
- Make nightly metadata a list of `{suite, tests, timeout}`.
- Generate the relevant SELF-HEALING.md sections, or delete them and point to the generated files.
- Add the three items above to the touch list.

### RB-12 — medium — AD-5's dynosaur syntax is from memory and wrong for 0.3.1

**Evidence.**

- AD-5 (l.117): `#[dynosaur::dynosaur(Dyn<Trait>)]`.
- dynosaur_derive 0.3.1 rejects this: `src/lib.rs:48-51` reads "expected `= dyn(box) TraitName`;
  dynosaur 0.3 requires this". The required form is
  `#[dynosaur::dynosaur(pub DynThrottle = dyn(box) Throttle)]`. This was checked against the
  0.3.1 crate source downloaded from crates.io.
- The generated `DynX` wraps `dyn ErasedX`, which copies the trait's supertraits
  (`lib.rs:349-353`). `Arc<DynThrottle>` is therefore `Send + Sync` only if `Throttle: Send + Sync`.
  AD-5 states no supertraits.
- An associated `const` makes a trait dyn-incompatible ("consts make the trait not dyn
  compatible", `lib.rs:377`). That matters if AD-4's ids are written as trait consts.
- The memlog says dynosaur's MSRV is 1.75; its `Cargo.toml` says `rust-version = "1.84"`. That is
  still within 1.91.

**Fix.**

- Correct the attribute form.
- "Every dyn-held trait declares `Send + Sync + 'static` supertraits and no associated consts."
- Enumerate the dyn-held traits: `Throttle`, `RetryPolicy`, `Authenticator`, `MarketData`, the
  capability traits and the trading trait.

### RB-13 — medium — Nothing owns AD-9's "never shorten" floor

**Evidence.** AD-9 (l.169): "A retry delay is at least `max(own backoff, Retry-After)`. A policy
may lengthen it and never shorten it." Nothing says the loop enforces it. If each `RetryPolicy`
computes its own delay, the mutation-tested floor (`retry_after_below_our_own_backoff_…`,
`core/src/client.rs:509,532`) is per-venue again.

**Fix.** "The loop computes `floor = max(backoff(attempt), Retry-After)` and sleeps
`max(floor, policy delay)`. The core tests pin it with a policy that returns zero."

### RB-14 — medium — The trading trait's event-stream shape is unfixed

**Evidence.**

- CAP-6 delivers acks and fills "as an async event stream".
- AD-5 fixes only the `async fn` shape.
- The Deferred list (l.475) defers streaming *market-data* traits only.
- prader keeps its channel unbounded because "dropping a fill desyncs inventory"
  (`venue-landscape.md:56`).

**Fix.** Add to AD-5:

- "Streams are `fn events(&self) -> impl Stream<Item = Result<TradingEvent, E>> + Send + 'static`
  (futures-core), boxed for dyn."
- "Delivery backpressures and never drops, as AD-11 does."

### RB-15 — medium — The capacity-bucket primitive has no home

**Evidence.**

- AD-10 (l.183): capacity layers "use `allow_burst(capacity)`".
- CAP-2's success line: "A test outside the foundation crate builds a Kalshi-style token-cost
  bucket without editing the foundation."
- The only engine placed in core is the window-quota one (l.185, l.392). `signer_limit` (moving
  to polymarket in S2) and the Kalshi skeleton would each wrap `governor` directly. That gives
  two definitions of the same building block, and governor becomes a venue dependency.

**Fix.** "Core exports a public capacity bucket (`capacity`, `refill`, exact-cost refusal) over
the shared cooldown. Venue crates do not depend on governor."

### RB-16 — medium — The umbrella's migration is undecided

**Evidence.**

- Today `polyoxide` exports `Polymarket`, `PolymarketBuilder`, `PolymarketError` and a prelude
  (`polyoxide/src/lib.rs:86-169`).
- It has features `ws`, `rtds`, `perps`, `perps-ws`, `sports`, `keychain` and `full`
  (`polyoxide/Cargo.toml`).
- The spine names only `polymarket`, `binance` and `full` (l.381). It is silent on:
  - the fate of the unified client;
  - how an umbrella user enables rtds or the perps socket;
  - what `full` contains, which AD-18's header build relies on.

**Fix.** Decide:

- the unified client: kept at `polyoxide::polymarket::Polymarket` or removed;
- per-module passthrough features named `polymarket-<module>`, or none;
- `full` = every module of every venue, including the `-ws` features and `keychain`.

### RB-17 — medium — Release mechanics for the consumer

**Evidence.**

- Seven crate names stop publishing after S2: clob, gamma, data, relay, perps, rtds and sports.
  The spine does not say whether they get a final README-only release pointing to
  `polyoxide-polymarket` (not a shim) or are simply abandoned.
- Memlog l.37 says "a release is a deliberate version-bump commit, not a consequence of an epic
  merge". The spine omits this. Parallel epics merging to main could bump the version, and
  `release.yml` releases on any new version after CI succeeds.
- "No shims" leaves an old→new path map as the consumer's only migration aid. AD-16 (l.297)
  requires only DRIFT rows and log targets in the release notes. The changelog is generated by
  git-cliff (`cliff.toml`), so the map needs an owner.

**Fix.**

- Add to AD-16: "Epics never bump versions."
- "S2's release notes carry a generated old→new path table."
- "Retired crates get one final release whose README names the new path."

### RB-18 — medium — Nothing binds the DFR rows

**Evidence.**

- The spine cites D1, D2, D14 and D17 in passing. No AD says that `divergences.md` DFR rows
  stay at the venue's call site, which is the SPEC constraint and a non-goal.
- D13 is the clearest risk: core's `RetryConfig::backoff` against the socket `Backoff`, "Do not
  unify". In S1 both `polyoxide-ws` and core gain backoff code.

**Fix.** Add to AD-12 or a new AD: "Every DFR row in `divergences.md` binds every epic. A merge
keeps the behaviour-carrying branch at the venue. D13's two backoffs are never unified."

### RB-19 — medium — Environments and the CAP-7 gate

**Evidence.**

- The CI diagram (l.449) sends nightly to the "Kalshi demo host". Kalshi's socket needs an API
  key even for market data (`venue-landscape.md:13`).
- The spine names no secret, and no rule says test-support's credential loaders are generic over
  env names. A loader that names `KALSHI_*` or `POLYMARKET_*` puts venue identifiers into
  test-support, which the glossary counts as foundation.
- The CAP-7 grep gate (l.294) has no pattern and no crate scope. The glossary says the
  foundation holds no venue identifiers of any venue, while CAP-7 names only Polymarket's.

**Fix.**

- Secrets follow `<VENUE>_<ENV>_*`. Loaders take env names from the test.
- The grep gate runs over venue, core, ws and test-support for every venue id in the registration
  metadata (RB-11), case-insensitive, with a committed exception list.

### RB-20 — medium — `MarketKey`'s wire form is undecided

**Evidence.**

- AD-4: venue and product ids are `const`s. As `&'static str` they cannot be deserialized.
- AD-4 does not settle:
  - `Display`, `FromStr` and serde behaviour;
  - whether `binance.usdm:btcusdt` equals `…:BTCUSDT` (Binance symbols are case-folded,
    `venue-landscape.md:17`);
  - the parse rule for `kalshi.events:<ticker>:yes|no`: first `:`, then the first `.`;
  - what validates a key parsed from a string, since `polyoxide-venue` has no registry.
- The CLI and Python epics will each pick their own answers.

**Fix.**

- Venue and product are `Arc<str>` or a `Cow<'static, str>` newtype.
- `FromStr` splits at the first `:` and then the first `.`, and validates syntax only.
- Typed constructors canonicalise case, and equality is byte-wise on the canonical form.
- serde uses the string form.

### RB-21 — medium — `Extensions` bounds and record derives are undecided

**Evidence.** AD-19 does not say:

- whether an inserted type must be `Clone + Send + Sync + 'static`, as `http::Extensions`
  requires;
- whether normalized records derive `Clone`, `Debug`, `PartialEq` and `Serialize`;
- how `Extensions` behaves under each derive.

Two trait epics will differ.

**Fix.**

- "`Extensions` stores `Clone + Send + Sync + 'static` values."
- "Records derive `Clone` and `Debug`. Serde and `PartialEq` skip `Extensions`."

---

## Low

### RB-22 — low — AD-8: two gaps in the request contract

- `RequestParts{method, path, query, body}` (l.147) has no `headers`, though `sign` "may add
  headers". Add `headers`.
- Transport errors with no response are unspecified. Today they return immediately without a
  retry (`core/src/request.rs:145-148`). State: no `observe`, no `decide`, no retry, returned as
  a `Network` class error.

### RB-23 — low — AD-9 misstates Binance's hold

AD-9 (l.171): "Binance's no-retry-left 429 is `Fail { hold: next minute }`". The code holds to
the next minute only when there is no retry left **and** no `Retry-After`. Otherwise it holds for
`Retry-After` (`binance/src/usdm/request.rs:131-140`). A 418's default ban is 2 minutes
(`:113`). Restate both.

### RB-24 — low — AD-18's second build cannot include Kalshi

The second build is "`polyoxide --features full`" (l.315), but Kalshi is deferred from the
umbrella, so its header test can never run in it. Use `cargo test --workspace --all-features`
instead, which CI already runs (`ci.yml:41`).

### RB-25 — low — AD-22's gates must be in the `CI` workflow

"A red check withholds the release tag, as every CI check does" (l.364) holds only for jobs in
the workflow named `CI`, because `release.yml:4-7` triggers on `workflows: [CI]`. Say the new
gates are jobs in `ci.yml`.

### RB-26 — low — Conventions do not match the brownfield

- **CLI depth.** The CLI pattern `<venue> <module> <verb>` (l.382) cannot hold today's
  `clob prices download`.
- **Credential kinds.** `<venue> credentials <store|show|delete>` drops today's kind slot
  (`credentials store clob|builder`, `polyoxide-cli/src/commands/credentials/mod.rs:36-58`).
  Polymarket has two credential types.
- **Features.** The rule (l.372) omits:
  - `keychain` (clob, relay, core, cli, umbrella);
  - `parquet` (cli);
  - clob's `default = ["gamma"]` dependency, which becomes intra-crate;
  - that `-ws` exists only for modules with both transports (rtds and sports are socket-only).
- **Error-enum naming** (`<Module>Error`, `<Module>WsError`) is implied by AD-15's examples but
  stated nowhere.

### RB-27 — low — The Python error convention vs CLAUDE.md

l.383 says "errors mapped by classification class". CLAUDE.md says "A v2 error maps by `code`,
and every SDK exception carries status, code, retryable, trace_id, parameter and retry_after", as
implemented at `polyoxide-py/src/error.rs:17-33`. State which wins, and add it to AD-21's table.

### RB-28 — low — AD-21's table reads as exhaustive

AD-21's table (l.343-351) omits several CLAUDE.md rules the ADs supersede:

- `should_retry` retries 429 and 425 (AD-17);
- "call `note_rate_limited` before `should_retry`" (AD-8);
- the `tests/live_api.rs` and `tests/mock_api.rs` names (Conventions);
- the Module Organization layout;
- clob's `ws` feature.

The general rule covers them, but the table looks complete. Head it "including".

The spine also does not say whether it or `docs/ARCHITECTURE.md` wins once the guide lands. Add a
precedence line.

### RB-29 — low — Stack omits tech the ADs rely on

Stack (l.402-418) omits:

- `alloy` 1.1.2, the dependency AD-2 exists to fence out;
- `keyring` 3;
- `clap` 4.5;
- `futures-util` 0.3;
- cargo-nextest (CI and nightly);
- git-cliff (release notes);
- maturin and uv;
- tokio-tungstenite's `rustls-tls-native-roots` feature.

These versions were verified on crates.io on 2026-10-08:

| Crate | Version | Published | Workspace pin |
|---|---|---|---|
| dynosaur | 0.3.1 | 2026-07-03 | — |
| futures-core | 0.3.34 | 2026-08-11 | — |
| cargo-hack | 0.6.45 | 2026-05-30 | — |

The remaining Stack rows match the workspace manifest: tokio 1.41, reqwest 0.12,
tokio-tungstenite 0.26, rustls 0.23, rust_decimal 1.37, governor 0.8, thiserror 2.0,
mockito 1.7, pyo3 0.28, edition 2021, MSRV 1.91.

### RB-30 — low — Terseness

Move these rationale fragments to the memlog:

- AD-5 l.118, "so `polyoxide-ws` does not depend on dynosaur";
- AD-6 l.129, the traceability note;
- AD-17 l.306, "it never retried 425".

Separately, list in one frontmatter line the spec amendments the spine relies on: AD-6's trait
organisation, the `Restricted` class and `is_fault`, and AD-2's widening. Today the memlog only
says "offer the spec update".

### RB-31 — low — No Open Questions section

Checklist item 7 needs every dimension the spine owns to be decided, deferred or listed as an open
question. RB-10, RB-16, RB-17 and RB-20 are none of these. Add an Open Questions section, or
decide them.

---

## What checked out

- **AD-12's counts and files.**
  - 27 `classify_failures` tests (`.github/scripts/tests/test_classify_failures.py`).
  - `polyoxide-binance/tests/supervision.rs` (21) and `supervision_edges.rs` (7) exist.
  - The usdm ws client has inline tests.
- **AD-11.**
  - Perps emits `Event::Reconnected` and has no `Disconnected`, as AD-11 and D8 say.
  - Binance's `Disconnected{path, reason}` exists.
  - Both supervisors use bounded `send().await`, which backpressures.
- **AD-17.** Binance's loop retries only 429 (`usdm/request.rs:124-148`), so "it never retried
  425" is true.
- **AD-8's log line.** The text matches core's (`request.rs:159-165`). The soak harnesses match
  on `target().starts_with("polyoxide_core")` and WARN (`perps/examples/info_soak.rs:346`,
  `data/examples/common/mod.rs:179`), so module-path targets inside core still match.
- **Feature drift R6.** It is real: rtds uses `test-fixtures`, the others `test-server`.
- **Crate names.** `polyoxide-venue`, `-ws`, `-polymarket`, `-test-support`, `-kalshi` and `-cli`
  are all unclaimed on crates.io.
- **Release workflow.** It triggers on CI `workflow_run` success. `CRATES` omits
  `polyoxide-cli`, while `finish_release.sh` publishes it, as `registration-points.md` says.

## Checklist scorecard

| # | Item | Result |
|---|---|---|
| 1 | Fixes the divergence points | Partial: RB-02, RB-04, RB-05, RB-10, RB-14, RB-15, RB-20, RB-21 |
| 2 | Rules enforceable and effective | Mostly; weak in AD-2 (RB-06), AD-12 (RB-07), AD-14 (RB-08), AD-13 (RB-11) and AD-9 (RB-13) |
| 3 | Deferred items cannot cause divergence now | One leak: the streaming deferral leaves the trading stream open (RB-14) |
| 4 | Named tech verified | Versions verified; dynosaur's syntax was from memory (RB-12); Stack incomplete (RB-29) |
| 5 | Ratifies the brownfield | Contradicted in RB-01, RB-09, RB-23 and RB-26; the AD-12 counts in RB-07 |
| 6 | Covers the spec | Gaps: RB-05, RB-06, RB-07, RB-18; CAP-1's test (RB-01) |
| 7 | Operational envelope and migration | Gaps: RB-11, RB-16, RB-17, RB-19, RB-25, RB-31 |
| 8 | Internally consistent | RB-01, RB-02, RB-03, RB-10, RB-27, RB-28 |
| 9 | Terse | Good; minor (RB-30) |
