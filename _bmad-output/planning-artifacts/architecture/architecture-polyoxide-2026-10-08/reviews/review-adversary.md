---
reviewer: adversary lens
target: ARCHITECTURE-SPINE.md (22 ADs, status draft, 2026-10-08)
binds: SPEC-venue-extensibility + companions, CLAUDE.md, workspace at e3d8c3e
lens: "construct two units one level down that each obey every AD to the letter yet still build incompatibly"
date: 2026-10-08
---

# Adversary review: incompatible pairs under the spine

## Verdict

**Not ready for parallel build.** The spine fixes the *order* of the send loop and the
*existence* of the Supervisor, the throttle, the classification interface and `Extensions`.
It does not fix the *shapes* that pass between epics, or *who owns* the state those shapes
carry. Fifteen pairs of epics can each obey all 22 ADs and still fail to fit together.
Seven are High, because each one either stops a parallel merge from compiling or silently
undoes a mutation-tested rule (a ban leaking across shared budgets, a widened user
subscription, a lost fill, a 503 filed as a real fault).

Every hole closes with a tightened or new AD. None needs a new decision from the user
except where noted (P5's class set, P4's queue policy).

## Method

The units are the epics the spine implies: S1 (foundation crates, send loop, Supervisor with
perps and Binance migrated, DRIFT fixes, registration and CI), S2 (renames, all merged
through one integration session), and S3 (traits, clob `Supervisor`, Kalshi skeleton). Per
the memlog, parallel loom agents may build them in separate worktrees. For each pair:

1. Name unit A and unit B.
2. Show that each obeys every AD that touches the point. ADs not cited are silent on it.
3. Show the incompatibility concretely, as types or flows, with code evidence from the
   current tree.
4. Rate it, and propose AD text that closes it.

Severity: **High** means the units cannot merge, or merging silently breaks a protected
behaviour. **Medium** means they merge but drift, or the next epic must rework one of them.
**Low** means friction only.

## Pairs at a glance

| # | Unit A | Unit B | What clashes | Severity | Closes with |
|---|---|---|---|---|---|
| P1 | S1 core send loop | S1 Binance throttle adapter | Two owners of the cooldown: the loop's per-client one and the per-IP budget's own | **High** | New AD-23; tighten AD-8, AD-9 |
| P2 | S1 core send loop (`RequestMeta`, `Charge`) | S1 Binance and Polymarket throttle adapters | The shapes of `cost` and `Charge`, the refusal type, and who computes cost | **High** | Tighten AD-8, AD-10 |
| P3 | S1 Supervisor with the Binance migration | S3 clob `Supervisor` (market and user) | What empty membership means: close the path, stay open, or subscribe to *every* market | **High** | Tighten AD-11 |
| P4 | S3 clob `Supervisor` (user channel) | S3 clob trading-trait implementation | A full queue stalls pings and staleness, which drops the socket and loses fills | **High** | Tighten AD-11; new AD-24 |
| P5 | S1 Binance error enums | S1 Polymarket error enums, test-support tags, Python mapping | No class for 5xx, 408, 425, timeouts or decode failures, and an unbound `VenueRefusal.code` type | **High** | Tighten AD-15, AD-14 |
| P6 | S1 test-support (the panic hook) | S1/S2 live-test migrations and the CI-scripts epic | A hook cannot see a typed error; nextest runs one process per test; untagged means `real`, yet regexes are the fallback | **High** | Tighten AD-14 |
| P7 | S1 composed-throttle epic (tables stay in core) | S2 "Polymarket leaves core" epic | `documented_*_limits` assert through a private `#[cfg(test)]` resolver, so the S2 move cannot keep its assertions | **High** | Tighten AD-12, AD-10, AD-16 |
| P8 | S3 `polyoxide-venue` keys | S3 Binance and Polymarket gamma/clob trait implementations, Kalshi skeleton | No canonical id; two modules build keys for one outcome; Kalshi ids contain `:` | Medium-High | Tighten AD-4 |
| P9 | S3 `polyoxide-venue` `Extensions` | S3 clob trading and venue trait implementations | Bounds, equality, duplicate inserts, and request extensions silently ignored | Medium-High | Tighten AD-19 |
| P10 | S3 market-data trait epic | S3 trading trait epic | Overlapping records in one crate (`Side`, `Level`, `Instrument`, size units, the source of positions) | Medium | Tighten AD-3, AD-7 |
| P11 | S1 registration epic (generated CLAUDE.md) | S1 ws-kit, test-support and error epics (AD-21 hand edits) | Two writers of the same CLAUDE.md lines | Medium | Tighten AD-13, AD-21 |
| P12 | S1 `polyoxide-ws` error type | S1 per-venue socket error enums | The fence leaves `WsError` classification with no single home, so drop codes are classified per venue | Medium | Tighten AD-3, AD-15 |
| P13 | S1 registration metadata (per crate) | S2 consolidation into `polyoxide-polymarket` | Nightly rows are per crate; S2 needs them per test target | Medium | Tighten AD-13 |
| P14 | S2 clob module move | S2 gamma, data and perps module moves | Whether "Polymarket's 425 policy" is per venue or per module | Medium-Low | Tighten AD-17 |
| P15 | S3 Kalshi skeleton (Rust `const`) | S1 registration check (metadata) | Two declarations of the venue id | Low | Tighten AD-4, AD-13 |

---

## P1 — Two owners of the client-wide cooldown (High)

**Unit A: S1 core send-loop epic.** It implements AD-8 literally. The hook contract has
three methods (`acquire`, `observe`, `decide`) and no way to start a cooldown on a throttle.
"The loop then applies **one** client-wide cooldown" therefore becomes a `Cooldown` field on
`HttpClient` (the core primitive, per AD-9), and the loop awaits it before `acquire`.
`with_base_url` clones share it, as they share the limiter today.

**Unit B: S1 Binance throttle adapter.** It implements AD-10 literally: "the venue composes
its layers inside it", meaning the weight minute and the funding bucket inside one
`DynThrottle`. It keeps D3 and the H4 cooldown. The budget's own cooldown stays inside the
throttle for two reasons, both in today's code:

- The budget is shared by every `Usdm` in the process (`UsdmBuilder::weight_budget`,
  `polyoxide-binance/src/usdm/mod.rs:138`), because Binance bans per IP. Each `Usdm` builds
  its own `HttpClient`.
- The funding path re-checks the cooldown *inside* `acquire` after sleeping on its slot
  (`weight.rs`: `if !self.in_cooldown() { return Charge { minute: None } }`). Without that
  re-check, a cooldown that begins while a request sleeps on a slot is ignored.

**Both obey every AD.** AD-8: the loop applies one cooldown per response. AD-9: both use the
core primitive, extend only, and re-check after waking. AD-10: one composed throttle per
client. AD-18: each keeps its own client.

**The incompatibility:**

```text
Usdm a ──HttpClient a (Cooldown a)──┐
                                     ├── shared WeightBudget (its own cooldown: never set)
Usdm b ──HttpClient b (Cooldown b)──┘

a receives 418, Retry-After 7200
  loop: decide → Fail{hold: 7200s} → Cooldown a.extend(7200s)
  b: Cooldown b is clear → acquire succeeds → b sends into the ban → Binance extends the ban
```

- The 418 hold no longer reaches the other clients on the same IP. That is the D3
  behaviour, which the spec says stays per venue.
- If B instead starts its own cooldown in `observe`, it must re-derive the delay there,
  because `observe` runs **before** `decide`. One response then yields two delays, which
  AD-8 and AD-9 forbid.
- If B leaves it out, the funding re-check reads a cooldown that nothing ever sets.

The same split hits Polymarket. Today the cooldown lives in `RateLimiter`
(`rate_limit.rs:117`, `cooldown_until`), and `note_rate_limited` drives it. A unit that
moves it into `HttpClient` and a unit that keeps it in the Cloudflare layer both comply.

**Proposed AD-23 — The cooldown is throttle state:**

> - `Throttle` has a fourth method, `hold(&self, delay: Duration)`. The loop calls it exactly
>   once per response that `decide` answers with `Retry(delay)` or `Fail { hold: Some(_) }`,
>   after `decide`, with that value. `HttpClient` holds no cooldown state.
> - `acquire` waits out the hold before charging any layer, and re-checks it after any wait
>   of its own.
> - A throttle shared by several clients shares its hold: Binance's per-IP budget, and
>   `with_base_url` siblings.
> - In a composed throttle a hold stops every layer.
> - `observe` records server counts and tiers only; it never starts a hold.
> - The hold's ceiling (Binance's 3 days) is a parameter of the core primitive, set per
>   throttle.

Amend AD-8's flowchart to `decide → throttle.hold(d) → warn → release permit → sleep`.

---

## P2 — The shapes of `RequestMeta.cost` and `Charge`, and who computes cost (High)

**Unit A: S1 core send-loop epic.** It transcribes AD-8 and AD-10 as written. `cost` is
singular: "a venue-declared layer id, `u32` units, and an `exact` flag". `Charge` has to
cross `Arc<DynThrottle>`, so it cannot be an associated type: a `dyn` type must name its
associated types, which would make `HttpClient` generic over each venue's charge. A
natural reading:

```rust
pub struct Cost { pub layer: LayerId, pub units: u32, pub exact: bool }
pub struct RequestMeta<'a> { method: &'a Method, path: &'a str, query: &'a [(String, String)], cost: Cost }
pub struct Charge { pub cost: Cost, pub at: tokio::time::Instant }
fn acquire(&self, meta: &RequestMeta<'_>) -> impl Future<Output = Charge> + Send; // AD-8 shows no Result
```

**Unit B1: S1 Binance throttle adapter.**
- `record_used(charge, used)` applies the `X-MBX-USED-WEIGHT-1M` count only when
  `state.minute == charge.minute`, the **UTC clock minute** taken from an injectable clock
  (`weight.rs`, `Charge { minute: Option<u64> }`, with `Clock::Manual` and `Clock::Tokio`
  in its tests).
- A `tokio::time::Instant` cannot give that minute: it is monotonic and not wall-clock.
- Funding requests charge **no** weight layer at all.

**Unit B2: S1 Polymarket composed throttle.**
- One `POST /orders` batch of N charges the Cloudflare layer **1 request** and the
  per-signer *order* bucket **N orders**.
- `cancel-all` charges 1 request and `1+N` cancels with `exact = false`
  (`signer_limit.rs:112-176`).
- The signer layer is two buckets (`TradingBucket::{Order, Cancel}`).
- Refusal is fallible: `limiter.acquire(*request).await?` yields
  `ClobError::BurstCapacityExceeded(#[from] polyoxide_core::BurstCapacityExceeded)`
  (`clob/src/error.rs:65`).

**All three obey every AD.** A reads AD-10's cost definition literally. B1 and B2 satisfy
"the venue composes its layers inside it" and "`acquire` refuses … any cost a layer can never
hold". AD-15 lets B2 keep its variant. Nothing says **who computes cost**: the memlog says
"request builders", but the spine does not. B1 could resolve the weight inside the throttle
from `meta.query["limit"]`, while the namespace epic (H7, H9) builds requests with a default
cost. Both comply.

**The incompatibility:**

- **Types.** B1 cannot recover its minute from A's `Charge`. B2 cannot express two layer
  charges in one `Cost`. A's `acquire` cannot return B2's refusal. These are compile-level
  clashes between parallel S1 worktrees.
- **Who computes cost.** If both the builder and the throttle compute, there are two
  weight tables. If neither does, `klines?limit=1000` (weight 5) is charged 1, and the
  budget under-counts toward a 418.
- **Refusal.** If core returns a refusal as `ApiError::Refused`, then
  `ClobError::Api(ApiError::Refused)` is what callers get, and
  `ClobError::BurstCapacityExceeded` is never constructed again. That is the same
  dead-variant hole that R7 records for relay, and callers matching on it stop matching
  silently.

**Proposed text, tightening AD-8 and AD-10:**

> ```rust
> pub struct Cost { pub layer: LayerId, pub units: u32, pub exact: bool }      // LayerId(&'static str), venue-declared
> pub struct RequestMeta<'a> { pub method: &'a Method, pub path: &'a str,
>                              pub query: &'a [(String, String)], pub costs: &'a [Cost] }
> pub struct Charge { pub layers: SmallVec<[LayerCharge; 2]> }                // opaque to core: passed acquire → observe unchanged
> pub struct LayerCharge { pub layer: LayerId, pub units: u32, pub window: Option<u64> } // window: layer-defined (Binance: UTC minute)
> fn acquire(&self, m: &RequestMeta<'_>) -> impl Future<Output = Result<Charge, Refused>> + Send;
> pub struct Refused { pub layer: LayerId, pub units: u32, pub capacity: u32 }
> ```
>
> - A layer that counts **requests** resolves its bucket from method and path, and charges 1
>   per attempt with no `costs` entry.
> - Every other layer charges exactly what `costs` names for it, and nothing when there is
>   no entry (Binance funding charges no weight).
> - `costs` is computed once, by the venue's request builder, from its route table
>   (`Route::cost`, `TradingRequest::cost`). A throttle never derives cost from the path or
>   the query.
> - The loop surfaces `Refused` as `ApiError::Refused`. Each module's single decode function
>   maps it to that module's existing variant (`ClobError::BurstCapacityExceeded`), which
>   classifies as `InvalidRequest` and is never retriable.

---

## P3 — What "empty membership" means is not a `Protocol` decision (High)

**Unit A: S1 Supervisor epic, migrating Binance.** AD-12's protected Binance suites pin this
(`tests/supervision.rs`, `supervision_edges.rs`, and the current `run_path`/`recover`):

- When a path's last stream leaves **mid-outage**, the path emits `Reconnected { path }` and
  ends (`Recovered::NotWanted`, `supervised.rs:767-770`).
- When a healthy path empties, the task ends with no marker (`pump`:
  `if streams.iter().all(|s| remove.contains(s)) { streams.clear(); … return End::Closed }`).

AD-11 says "Binance runs one supervisor per routed path" and "a declaring `Protocol` pairs
them even when a routed path empties mid-outage". The only place to honour that is the
generic `Supervisor`. So A builds "empty membership ends the supervisor, paired if
mid-outage" into `Supervisor<P>` and stores membership as `Vec<P::Member>`.

**Unit B: S3 clob `Supervisor` epic (CAP-12).** It implements `Protocol` for both clob
channels. Today:

- **Market channel.** An empty-membership socket is valid and stays open under the 10 s
  `PING`, accepting later subscribes (`clob/src/ws/subscription.rs:248-251`; CLAUDE.md,
  "verified live 2026-09-09").
- **User channel.** *No* filter means **every market**, and an *empty* filter means **no
  market**. A protected test exists because "collapsing an empty filter into no filter would
  silently widen a subscription" (`subscription.rs:315-328`).

**Both obey every AD.** AD-11's hook list has membership *frames* and paced replay, but no
emptiness or membership *representation* hook. AD-12 binds A's behaviour. CAP-12 binds B's
replay.

**The incompatibility:**

- Under A's generic rule, unsubscribing the last asset ends B's market stream. Today it
  stays connected, which is a behaviour change no AD records.
- B's user-channel "all markets" cannot be stored as `Vec<Market>`. It has to become an
  empty vector, which A's Supervisor treats as "not wanted", or a sentinel member that A
  replays as a real market id.
- If B instead overrides emptiness by keeping the connection open, then the Binance router
  must close a path itself. Closing a supervisor **mid-outage** goes through `close()`, which
  yields no `Reconnected`. That breaks the invariant prader folds outages on.

**Proposed text, tightening AD-11:**

> - Membership is `P::Membership`, an associated type that the Supervisor stores, hands to
>   `P::replay` after each connect, and never inspects.
> - `P::wanted(&P::Membership) -> bool` decides whether a connection is wanted.
>   - When it turns false on a healthy connection, the Supervisor closes it politely and ends
>     with no marker.
>   - When it turns false mid-outage, a declaring `Protocol` first yields the pairing
>     `Reconnected`.
>   - Binance answers false on empty. Perps and the clob market channel answer true: they
>     stay open under pings.
> - The clob user channel's membership is `Option<Vec<Market>>`. `None` means every market;
>   `Some(vec![])` is never collapsed into `None`, and a test pins it.
> - A routing layer (Binance) ends a path only by emptying its membership, never by
>   `close()`.

---

## P4 — Backpressure stalls the read and ping side, so fills are lost (High)

**Unit A: S3 clob `Supervisor` epic (user channel).** It obeys AD-11:

- It pings on the wall clock.
- Staleness counts inbound frames.
- It "never drops a decoded event: a full queue backpressures".

It ports the proven shape: in both shipped supervisors the emit is
`events.send(…).await` *inside* the read arm, and the ping is checked only at the top of
the loop (`binance/usdm/ws/supervised.rs:878-1000`; `perps/src/ws/supervised.rs:399-525`).
Staleness reads `ws.last_inbound()`, which is refreshed only when a frame is **read**
(`usdm/ws/client.rs:332-395`).

**Unit B: S3 clob trading-trait epic (CAP-6).** It exposes "order acks and fills as an async
event stream" (AD-5 shape) by adapting A's event stream, and obeys AD-19 and AD-20. Its
consumer is prader, whose documented requirement is an **unbounded** channel "because
dropping a fill desyncs inventory" (venue-landscape, prader trading seam).

**Both obey every AD.** AD-11 requires backpressure; it does not say what backpressure may
stop. CAP-6 does not say what feeds the stream.

**The incompatibility, as a flow:**

```text
consumer slow ─▶ queue full (1024) ─▶ pump parked in send().await
  ├─ no PING sent           (ping is checked only at loop top)
  ├─ no frames read         (last_inbound frozen; frames pile up in the kernel buffer)
  └─ unblocks ─▶ top of loop: silent ≥ stale_after ─▶ End::Lost(Stale)
                 ─▶ socket dropped WITH unread fill frames in its buffer
                 ─▶ reconnect ─▶ user channel replays nothing (asyncapi-user.json: no snapshot/replay)
```

- AD-11's "never drops a decoded event" holds, yet a fill is lost because it was never
  decoded.
- The clob host also resets a socket that goes about 125 s without `PING`.
- No AD says who reconciles fills after a user-channel `Reconnected`: the trading
  implementation or the consumer.

**Proposed text, tightening AD-11:**

> - The ping schedule and the staleness clock never wait on the consumer.
> - While the Supervisor is blocked on a full queue it keeps sending pings, and stops only
>   reading. Time spent blocked does not count as silence.
> - Each `Protocol` declares its queue: `Bounded(n)` (perps, Binance, the clob market
>   channel) or `Unbounded` (the clob user channel, which carries acks and fills).

**Proposed AD-24 — The trading event stream is lossless at its boundary:**

> - A `Trading` implementation feeds its event stream from an unbounded queue.
> - After every `Reconnected` on a channel that carries fills, it yields
>   `TradingEvent::Resync` before any later fill. The consumer then calls `open_orders()` and
>   the venue's trade history to reconcile.
> - The trait documents that the venue replays nothing.

(Choosing `Unbounded` versus a caller-sized bound is the one user decision here.)

---

## P5 — The classification set has no class for 5xx, 408, 425, timeouts or decode failures (High)

**Unit A: S1 Binance error epic (AD-15).** Binance answers a 5xx with its JSON body
`{code: -1001, msg}`. Today that becomes `BinanceError::Venue { status, code: i64, msg }`
(`binance/src/error.rs:19-26`). The same status with an HTML body becomes
`Api(ApiError::Api { status: 502 })`. Reading AD-15's list, `VenueRefusal { code }` is the
only class with room for a venue code, so A maps the first case to
`VenueRefusal { code: -1001 }` (is_fault true) and the second to `Network`.

**Unit B1: S1 Polymarket error epics (clob, gamma, data).** A Polymarket 5xx carries no
code, so B1 maps it to `Network`. Data v2's `ErrorCode` is a **string** open enum
(`data/src/v2/error.rs:59`, `"invalid_request"`), and Kalshi codes are strings too.
Binance's are `i64`. B1 therefore types `VenueRefusal.code` as a string type, while A, if it
lands first, has already typed it as `i64`.

**Unit B2: S1 test-support tag reporter (AD-14).** It derives `transient` from
`is_retriable`. AD-15 does not say whether `is_retriable` is a provided method computed from
the class or a per-enum method. B2 chooses the provided one:
`matches!(class, Network | RateLimited)`.

**Unit B3: Python mapping (C4, "by classification class").** It has no class from which to
raise its existing `TimeoutError`, which data v2 raises today for `RequestTimeout`
(`polyoxide-py/src/error.rs:27`).

**All obey every AD.** AD-15 names six classes plus `is_fault`, and does not map statuses
to classes. AD-3 places the 408/425/429/5xx rule "for callers' is it retriable", but does
not bind it to the classes. AD-17 governs only the loop. AD-14 says only that `transient`
comes from `is_retriable`.

**The incompatibility:**

- The same live 503 is `transient` on clob and `real` on Binance (`VenueRefusal`, not
  retriable). The nightly files an issue for Binance, which is the expensive direction the
  classifier comment warns about (`classify_failures.py`, the TRANSIENT_RES preamble).
- prader's generic classifier, which CAP-4 promises is "written once", retries a clob 503 and
  surfaces a Binance 503 to the operator.
- `VenueRefusal.code` cannot have one type that both the Binance and data v2 epics accept.
- A decode failure (`ApiError::Serialization`) has no class. A wire drift, which the drift
  detectors exist to catch, lands wherever each epic guesses.
- prader's existing `UpstreamUnavailable` (venue-landscape) has no counterpart.

**Proposed text, tightening AD-15 and AD-14:**

> - **Classes:**
>   - `Network`: no response (connect, timeout, reset, TLS EOF, DNS).
>   - `Unavailable { code }`: 408, 425 or 5xx, whatever the body.
>   - `RateLimited { retry_after }`.
>   - `Unauthorized`.
>   - `InvalidRequest`.
>   - `VenueRefusal { code }`: a 4xx caused by the request, carrying a venue code.
>   - `Restricted`: 451, and Binance's WAF 403.
>   - `Decode`: a 2xx body that does not parse.
> - Each class carries `is_fault`.
> - `code` is `Option<Arc<str>>`; a numeric code is rendered in decimal (`"-1121"`).
> - **The status decides the class before the body does.** A venue may override a status
>   only through a DFR row named in its decode function (Binance 403 → `Restricted`).
> - `is_retriable()` is a provided method that venues do not implement: true for `Network`,
>   `Unavailable` and `RateLimited`.
> - A server-sent `retryable` flag (data v2) is surfaced, not obeyed.
> - AD-14 tags map from the class alone: `Network`, `Unavailable` and `RateLimited` are
>   `transient`; `Restricted` is `environmental`; everything else is `real`.
> - Python exceptions map one-to-one from the class.

This amends the spec's six CAP-4 questions; offer the spec update.

---

## P6 — The panic hook cannot see the error it is supposed to tag (High)

**Unit A: S1 test-support epic (AD-14).** It ships "a panic hook [that] prints
`polyoxide-class=<tag>`". A Rust panic hook receives only `PanicHookInfo`: a payload
(`&str` or `String`) and a location, never the typed error. `.unwrap()` on
`Err(ClobError::…)` hands the hook the `Debug` text. A therefore has two options:

- (a) infer the tag from that text, which is the regex table AD-14 exists to retire; or
- (b) have tagging helpers stash the class in a thread-local that the hook prints.

Both comply. Separately, nightly runs `cargo nextest` (`nightly-behavioral.yml:71`), which
runs **one process per test**, so a hook installed once per binary does not exist. Each
test process installs it only if that test calls a test-support function first.

**Unit B: S1/S2 live-test migrations (each venue), and the CI-scripts epic
(`classify_failures.py`).** They read AD-14 as "the hook tags failures" (the memlog says
"without per-call discipline"), so tests keep `.unwrap()` and `.expect()`. The CI-scripts
epic implements the table's last row, "`real`: everything else, **including untagged
failures**", and also "the regexes remain as a fallback".

**Both obey every AD.** AD-14 states the hook, the table and the fallback; it does not say
how a class reaches the hook, or what an untagged failure is when the regexes would match
it.

**The incompatibility:**

- With A's option (b), every `.unwrap()` in B's tests prints no tag. The CI-scripts epic
  must then choose between AD-14's two sentences, "untagged → `real`" and "regex fallback".
- If it chooses the first, every 429, 5xx and connect timeout in an unmigrated test becomes
  an issue filed immediately: the exact regression issue #32 records.
- With A's option (a), the shared regex file that CAP-8 removes is still the classifier.
- A test that panics before calling any loader has no hook at all under nextest.

**Proposed text, tightening AD-14:**

> - Tags are emitted at the failure site, never inferred from a payload.
>   - test-support provides `ResultExt::or_fail(self, ctx)` for
>     `Result<T, E: polyoxide_venue::Classify>`, which prints `polyoxide-class=<tag>` to
>     stderr, then panics with `ctx` and the error.
>   - The credential loaders print `auth-gated`.
>   - `environmental(reason)` prints `environmental`.
> - Each of these installs the hook through a `Once`, chained to the previous hook, so it
>   works under nextest's process-per-test model.
> - `classify_failures.py` precedence: the last tag line wins; with no tag, the regex table;
>   with no match, `real`.
> - A CI check lists every `tests/live_*.rs` that unwraps a polyoxide `Result` without
>   `or_fail`. The list only shrinks, and the regex table is deleted, with the 27 classifier
>   tests moved to the tag table, when it is empty.

---

## P7 — S1 leaves `documented_*_limits` unmovable, and S2 cannot move them under AD-12 (High)

**Unit A: S1 composed-throttle epic.**
- Per AD-16, Polymarket code leaves core only in S2, so in S1 the tables stay in core and
  so do their tests. No move means AD-12 is not triggered, and A leaves the tests alone.
- A adds the public `effective_quota(method, path)` that AD-10 requires.
- The tests still assert through the **private, `#[cfg(test)]`** `resolve_specs` and
  `inner.limits`, and the tables are still built from private `Bucket`, `EndpointLimit`,
  `simple_limit` and `dual_limit` (`rate_limit.rs:41-110`, `:309-345`, `:420`).

**Unit B: S2 "Polymarket leaves core" epic.**
- AD-10 and AD-16 require it to move `RateLimiter::{clob,gamma,data,relay,perps}_default`
  and about 1,100 lines of `documented_*_limits` into `polyoxide-polymarket`.
- AD-12 requires "a move keeps test names **and assertions**".
- `#[cfg(test)]` items of core are invisible to another crate's tests, and the tables cannot
  be built outside core at all, because the construction helpers are private.

**Both obey every AD.** A meets AD-10 (it exposes `effective_quota`) and AD-16 (no public
moves in S1). B meets AD-10 and AD-16 by moving tables out in S2.

**The incompatibility:**

- B must rewrite assertions (breaking AD-12) or add a public table-construction API to core
  in S2.
- That API is a foundation change no AD names. If the clob and data module moves each add
  their own in parallel in the S2 integration session, core has two table builders.
- The shared ledger bucket (900/10s across `/trades`, `/orders`, `/notifications` and
  `/order`, `rate_limit.rs:453-499`) needs bucket sharing *across entries*, which a naive
  per-route builder loses. That is a 4x over-permit that the moved tests would no longer
  see, because their assertions were rewritten.

**Proposed text, tightening AD-12, AD-10 and AD-16:**

> - **S1 makes every protected suite that S2 will move "move-ready".** It asserts only
>   through public API, and the rewrite is its own commit, with per-suite counts and
>   re-mutation.
> - For the limit tables, S1 adds core's public `WindowQuotaTable` builder. It supports
>   buckets shared across routes, prefix and exact matching, and method scoping, and
>   `effective_quota` returns every bucket a request awaits, the general bucket included.
> - In S2 a move changes only file locations and `use` paths. The PR attaches the diff of
>   the moved test bodies with paths normalised, and that diff is empty.

---

## P8 — No canonical key, and two modules build keys for one outcome (Medium-High)

**Unit A: S3 `polyoxide-venue` keys epic (AD-4).** The id is "an opaque `Arc<str>`", and
`venue.product:id` is "written" text. A provides `impl FromStr for MarketKey` that only
splits the syntax, since validation belongs to the venue, and derives `Eq` and `Hash` on the
bytes. It splits at the last `:`.

**Unit B1: S3 Binance trait epic.** Its typed constructor goes through `Symbol::new`, which
**uppercases ASCII** and accepts Unicode alphanumerics (`usdm/types.rs:44-53`).
`"binance.usdm:btcusdt".parse()` and `usdm::key(Symbol::new("btcusdt")?)` are therefore
unequal keys for one instrument. A consumer's `HashMap<MarketKey, _>` holds both, and a
quote returned under the canonical key never matches the parsed key it was asked for.

**Unit B2: S3 Polymarket gamma trait epic (the `EventGroups` capability).** The Conventions
say "product id = module name", so the keys it builds are `polymarket.gamma:<token>`. AD-7
spells the same outcome `polymarket.clob:<token>`. Even if B2 follows AD-7, under
`cargo hack --each-feature` (AD-22) the `-F gamma` build has no clob code, so B2 builds
`polymarket.clob:` keys **with its own constructor**. That is a second validator for one
product.

**Unit B3: S3 Kalshi skeleton.** `kalshi.events:<ticker>:yes|no` contains a second `:`.
Splitting at the last `:` yields the id `yes`.

**All obey every AD.** AD-4 says "each venue … validates its own ids", but not that a key
can only be built through validation, not where within a venue the constructor lives, and
not which `:` ends the product.

**Proposed text, tightening AD-4:**

> - `MarketKey` has no public constructor from text or from parts.
> - `polyoxide-venue` offers `RawKey::parse(&str)`, which splits at the first `.` and the
>   first `:`; the id is the remainder and may contain `:`.
> - Only a product's own constructors produce a `MarketKey`, and they canonicalize the id
>   (Binance uppercases ASCII; Kalshi `<TICKER>:yes|no`). So `Eq` is canonical equality.
> - One product's constructors live in one place: for Polymarket, `src/shared/keys.rs`,
>   compiled under every feature that names the product (`any(clob, gamma, data)`).
> - "Product id = module name" applies to the module that **trades** the product; gamma and
>   data refer to clob's outcomes with clob's keys.
> - The umbrella provides `polyoxide::parse_key(&str)`, dispatching on the enabled venues.

---

## P9 — `Extensions`: bounds, equality, duplicates, and request options silently dropped (Medium-High)

**Unit A: S3 `polyoxide-venue` `Extensions` epic (AD-19).** It implements "a type-map;
`ext::<T>() -> Option<&T>`" in the usual way:
`HashMap<TypeId, Box<dyn Any + Send + Sync>>`. That is not `Clone`, compares no contents,
and on `insert` replaces any existing value.

**Unit B: S3 clob trading epic.**

- It derives `Clone` and `PartialEq` on `Fill` and `OrderAck`. It needs `Clone` to fan out
  to the native socket and trading consumers, and `PartialEq` for golden tests built from
  py-clob-client vectors. This does not compile against A, unless A adds an always-equal
  `PartialEq`. That makes every record golden test blind to venue facts, the repo's
  recurring "shape-only test" class.
- It reads `req.ext::<ClobOrderOptions>()` for `neg_risk`, `expiration` and fee terms, and
  ignores any other extension type.
- A consumer that passes `KalshiOrderOptions`, or a perps options struct, through a generic
  function to the clob `Trading` impl has its options **silently ignored**. For example, an
  intended post-only or GTD order is placed as a plain GTC.

**Unit C: S3 clob market-data epic and gamma `EventGroups` epic.** Each defines a
"Polymarket instrument ext" in its own module (`clob::venue::InstrumentExt` and
`gamma::venue::InstrumentExt`). A consumer reading one type gets `None` on records built by
the other path.

**All obey every AD.** AD-19 fixes the mechanism, not the bounds, the equality, the
ownership of ext types, duplicate handling, or what a venue does with an extension it does
not read.

**Proposed text, tightening AD-19:**

> - `Extensions: Clone + Debug + Default + PartialEq + Send + Sync`. An insertable `T` is
>   `Clone + Debug + PartialEq + Send + Sync + 'static`, and `PartialEq` compares contents,
>   never always-true.
> - Each product defines one ext type per record type (`<Record>Ext`) in `src/shared/` or
>   the product's `venue.rs`. Any module that enriches a record fills that struct's `Option`
>   fields; inserting a type already present is an error.
> - On **requests**, a venue reads only its declared option types, and refuses with
>   `InvalidRequest` a request that carries any other extension. `Extensions` exposes the
>   `TypeId`s it holds, so a venue can check.
> - `Extensions` has no serde; facades print native types.

---

## P10 — Two trait epics define overlapping records in one crate (Medium)

**Unit A: S3 market-data traits epic.** It adds `Quote`, `Book`, `Level { price, size }`,
`BookSide { Bid, Ask }` and `Instrument { tick_size, min_size, … }`.

**Unit B: S3 trading trait epic.** It adds `Side { Buy, Sell }`, its own
`PriceLevel`/`TickSize` for order validation, and `Position`.

**Both obey every AD.** AD-3 puts every record in `polyoxide-venue` and forbids *other
crates* from defining them, but says nothing about two modules of that one crate. AD-6 and
AD-7 fix keys and book terms, not shared primitives.

**The incompatibility:**

- Two price-level types, two side enums, and two sources of tick size: the market listing,
  and the order-time lookup that Polymarket changes live (`tick_size_change`).
- `size` has no unit. Polymarket's market **BUY** is an amount in USDC, while its SELL and
  limit orders are in shares; Kalshi counts contracts; Binance counts the base asset.
- `Position` has two possible sources on Polymarket. The data API reports holdings with
  average price; clob `/balance-allowance` reports a per-token balance only. The clob
  `Trading` impl can use the first only with `-F data`, an undeclared feature coupling.

**Proposed text, tightening AD-3 and AD-7:**

> - S3 opens with one records story in `polyoxide-venue`: `Side { Buy, Sell }` (orders,
>   fills, trade aggressor), `Level`, `Instrument` (the **only** carrier of tick size, min
>   size and `SizeUnit`), and `UnixMillis`.
> - Trait epics add only the records their own trait names, and import these.
> - `size` is in the instrument's native quantity. A quote-currency amount is a separate
>   `notional` field and is never placed in `size`.
> - Polymarket's `Trading::positions` reads the data API, so the `Trading` impl is gated on
>   `all(clob, data)`, declared in Cargo metadata.

---

## P11 — Two writers of the same CLAUDE.md lines (Medium)

**Unit A: S1 registration epic (AD-13).** It makes "CLAUDE.md's crate list and publishing
order" generated output, with a CI check that "generated output differs from what is
committed" fails.

**Unit B: S1 ws-kit epic, test-support epic and error epic (AD-21).** Each "updates
CLAUDE.md in the same change" for the rule it supersedes. The first row, "rtds and sports
depend on nothing in-workspace", is **the crate-graph line itself**
(`polyoxide-rtds (… depends on NOTHING in-workspace)`) and the Publishing Order paragraph.
Those are exactly the lines A now generates.

**Both obey every AD.**

**The incompatibility:**

- If B lands after A, B's hand edit fails A's CI check.
- If A lands after B, the generator overwrites B's edit, and B's rule text survives only if
  someone moves it into a metadata README line.
- Five parallel S1 epics edit one file; S2's integration session adds two more rows.

**Proposed text, tightening AD-13 and AD-21:**

> - Generated regions are delimited by `<!-- generated:begin <id> -->` /
>   `<!-- generated:end <id> -->`, and only `gen_registry.py` writes inside them. An AD-21
>   change that falls inside one is made through metadata or the generator template.
> - The generator merges first in S1, and no AD-21 CLAUDE.md edit merges before it.
> - Each superseded rule is rewritten only within its own `##` section.

---

## P12 — The fence leaves socket-error classification with no single home (Medium)

**Unit A: S1 `polyoxide-ws` epic.** By AD-1 and AD-2, `polyoxide-ws` depends on neither
`polyoxide-core` nor `polyoxide-venue`. So its `WsError` (connect timeout, handshake
refusal with status, close with code, TLS EOF) cannot implement the classification
interface.

**Unit B: S1 perps, Binance, rtds and sports socket error epics (AD-15).** Each "tier" enum
wraps `WsError` and implements the classification interface, so each writes its own
`WsError` → class mapping. Under the orphan rule, no crate other than a venue crate can.

**Both obey every AD.**

**The incompatibility:** five copies of the W7 classifier return, and they can disagree.
Perps maps a 1011 close to `Network`, while Binance maps it to `VenueRefusal`. AD-14's
transient set (drop codes 1001/1011/1012/1013 and TLS EOF) then depends on which venue
dropped. That is the DRIFT R1 class again, this time for closes rather than handshakes.

**Proposed text, tightening AD-3 and AD-15:**

> - `polyoxide-venue` exports `impl_ws_classification!(VenueWsError::Ws)`, a `macro_rules!`
>   that expands in the venue crate against `::polyoxide_ws::WsError`, so `polyoxide-venue`
>   needs no dependency on `polyoxide-ws`.
> - It holds the one `WsError` → class table, including the drop codes, and every socket
>   error enum uses it.

---

## P13 — Nightly metadata is per crate, but S2 needs it per test target (Medium)

**Unit A: S1 registration epic.** It writes AD-13's "each crate's
`[package.metadata.polyoxide]` (nightly timeout and suite …)" on the twelve S1 crates. One
crate holds one suite, except clob, which has two rows today: `live` at 15 min and
`session-keys` at 40 min (`nightly-behavioral.yml:50-59`).

**Unit B: S2 consolidation epic.** `polyoxide-polymarket` holds about ten live targets
(`live_clob`, `live_clob_ws`, `live_clob_session_keys`, `live_gamma`, `live_sports` at
20 min, …), each with its own features and timeout.

**Both obey every AD.**

**The incompatibility:** B must change A's metadata schema and generator inside the S2
integration session. A row's identity also changes from `-p polyoxide-clob --test live_api`
to `-p polyoxide-polymarket --test live_clob`, and libtest-json names change with it
(`crate::binary$test`). Open nightly tracking issues keyed on the old names therefore never
close.

**Proposed text, tightening AD-13:**

> - Live metadata is keyed by test target from S1 onward:
>   `[package.metadata.polyoxide.live.<target>] = { suite, timeout, features }`. Rows group
>   by (crate, suite).
> - S2 moves entries, never the schema.
> - The S2 release closes or renames open `nightly-behavioral` issues whose test identity
>   changed.

---

## P14 — Is "Polymarket's 425 policy" per venue or per module? (Medium-Low)

**Unit A: S2 clob module move.** It adopts "Polymarket's `RetryPolicy` adds 425" (AD-17).
D17 says 425 is the *matching engine's* signal, so A gives it to clob only.

**Unit B: S2 gamma, data and perps module moves.** They read "Polymarket's" as venue-wide
and install the 425 policy, or the reverse. Today every core-based client retries 425
(`core/src/client.rs:132`), so either reading changes behaviour for some modules.

**Both obey every AD.**

**The incompatibility:** different 425 retry sets across Polymarket modules, a change users
can see, and no DRIFT row records it.

**Proposed text, tightening AD-17:**

> - Polymarket's `RetryPolicy` is one instance in `src/shared/`, used by every Polymarket
>   module that runs on core's loop (clob, gamma, data, relay, perps). That keeps today's
>   behaviour.
> - Narrowing it to clob would be a DRIFT row, and needs one.

---

## P15 — Two declarations of the venue id (Low)

**Unit A: S3 Kalshi skeleton.** It declares `pub const VENUE: &str = "kalshi"` in Rust
(AD-4).

**Unit B: S1 registration check.** AD-13's "CI fails when venue ids collide" reads Cargo
metadata, because a Python script cannot read Rust consts reliably. So it adds
`venue = "kalshi"` to `[package.metadata.polyoxide]`.

**Both obey every AD.** The two declarations can disagree, and the check then passes on the
metadata while the code collides.

**Proposed text, tightening AD-4 and AD-13:**

> - Venue and product ids are declared in metadata.
> - Each venue crate's `build.rs` emits its consts from that metadata, or one unit test per
>   crate asserts that the consts equal the metadata, parsed from `Cargo.toml`.

---

## Consolidated AD changes

| Change | Pairs closed |
|---|---|
| **New AD-23**: the cooldown is throttle state (`Throttle::hold`); `HttpClient` holds none | P1 |
| **AD-8 and AD-10 tightened**: `RequestMeta.costs: &[Cost]`, opaque `Charge` with layer windows, `acquire -> Result<Charge, Refused>`, cost computed only by the builder, `Refused` mapped to existing variants | P2 |
| **AD-11 tightened**: `P::Membership` opaque, `P::wanted`, `Option` membership for the clob user channel, routers end paths only by emptying them | P3 |
| **AD-11 tightened**: pings and staleness never wait on the consumer; per-`Protocol` queue policy | P4 |
| **New AD-24**: the trading event stream is unbounded, with `Resync` after each fills-channel `Reconnected` | P4 |
| **AD-15 and AD-14 tightened**: classes add `Unavailable` and `Decode`; status before body; `code: Option<Arc<str>>`; provided `is_retriable`; tags from class alone; Python maps by class | P5 |
| **AD-14 tightened**: tags emitted at the failure site (`or_fail`), hooks installed through `Once`, tag/regex/`real` precedence, a shrinking allowlist | P6 |
| **AD-12, AD-10 and AD-16 tightened**: S1 makes moving suites move-ready; core's public `WindowQuotaTable` builder; S2 moves change paths only | P7 |
| **AD-4 tightened**: `RawKey` parsing, canonical constructors only, one key module per product, first-`:` split, umbrella `parse_key` | P8, P15 |
| **AD-19 tightened**: bounds, content equality, one ext type per record per product, unknown request extensions refused | P9 |
| **AD-3 and AD-7 tightened**: a shared records story opens S3; `size` units; Polymarket positions from the data API | P10 |
| **AD-13 and AD-21 tightened**: generated regions with markers, the generator first, per-target live metadata | P11, P13 |
| **AD-3 and AD-15 tightened**: `impl_ws_classification!` in `polyoxide-venue` | P12 |
| **AD-17 tightened**: one Polymarket `RetryPolicy` for every module on core's loop | P14 |

## Where the spine already holds

These looked like candidate pairs and did not survive. They need no change.

- **Permit and limiter order.** AD-8's order (permit, then `acquire`) is the order both
  Binance (`usdm/request.rs:94-95`, chosen so a charge stays next to its send) and clob
  (`clob/src/request.rs:209-217`) already use.
- **Per-attempt signing.** `Authenticator::sign(&mut RequestParts, attempt)` covers clob's
  fresh L2 timestamp on every retry, and Kalshi's millisecond-timestamped RSA signature.
- **Binance rotation as an outage.** Rotation is pinned by AD-12's protected suites
  (`Disconnected { reason: Rotation }` then `Reconnected`). No compliant Supervisor can make
  it seamless without failing them.
- **Perps' missing `Disconnected`.** It is settled by "only for a declaring `Protocol`".
