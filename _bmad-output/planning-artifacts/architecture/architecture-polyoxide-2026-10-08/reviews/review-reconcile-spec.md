---
review: reconcile-spec
target: ../ARCHITECTURE-SPINE.md
inputs:
  - ../../../../specs/spec-venue-extensibility/SPEC.md
  - ../../../../specs/spec-venue-extensibility/divergences.md
  - ../../../../specs/spec-venue-extensibility/duplication-inventory.md
  - ../../../../specs/spec-venue-extensibility/venue-landscape.md
  - ../../../../specs/spec-venue-extensibility/registration-points.md
  - ../../../../specs/spec-venue-extensibility/glossary.md
date: 2026-10-08
---

# Spine ↔ spec reconciliation

**Verdict: not ready to hand to parallel epics.** The spine fixes the big shapes well: the
crate layers, one send loop, one `Supervisor`, open venue keys and release staging. But it
contradicts two DFR rows (D3 and D8), its own dependency rules disagree on where the socket
crate gets the retriable-status rule, and five quiet spec requirements have no owner. Those
five are: venue-only fields staying reachable, CAP-10's header pin, custody of
behaviour-carrying tests beyond supervision, the success signal's allowed-touch set, and the
CLI credential plumbing. Each gap is a place where two independently built epics would choose
incompatibly, or where the Kalshi skeleton would be forced into a foundation edit.

Scope: only spec content that bears on cross-unit consistency. Content the spec covers
adequately with no divergence risk is listed once in the coverage tables at the end and not
discussed.

Code facts used below were checked on this worktree (base `e3d8c3e`).

---

## Findings

### F1 — critical — HTTP hook inputs cannot express the venue-landscape rows; AD-8 as drawn drops DFR D3

**Spec.**
- venue-landscape.md: "The traits, the throttle interface and the socket blocks must accommodate every column below."
- Limiter row:
  - Polymarket: "Cloudflare window quotas per path and per IP".
  - Binance: "weight depends on route and parameters; funding routes have a separate bucket".
  - Kalshi: "separate read and write" buckets.
- REST auth row:
  - L2: HMAC of "`ts+method+path[+body]`".
  - Kalshi: "RSA-PSS/SHA-256 of `timestamp_ms + METHOD + path-without-query`".
  - L1: EIP-712.
- divergences.md D3: "Binance weight table, next-minute hold, 418 ban path, weightless funding bucket".
- CLAUDE.md (an adopted companion) gives two more Binance rules:
  - the weight count is "raised by every response's `X-MBX-USED-WEIGHT-1M` but only for the minute its request was charged in";
  - "A `429` with no retry left and no `Retry-After` holds every request to the next minute".
- SPEC CAP-2 success: "A test outside the foundation crate builds a Kalshi-style token-cost bucket without editing the foundation."
- SPEC success signal: the Kalshi skeleton lands with "no foundation edits".

**Spine.**
- AD-8's diagram shows `throttle.acquire(cost)`, `authenticator.headers(attempt)` and `throttle.observe(response)`, then `Done or Fail --> return`.
- AD-5 binds "every trait in `polyoxide-venue`, plus `Throttle` and `Protocol`". It does not bind `Authenticator`.

**Code today.**
- `RateLimiter::acquire(&self, path: &str, method: Option<&Method>)` is keyed on path and method, not a cost (`polyoxide-core/src/rate_limit.rs:403`).
- `SignerLimiter::acquire(TradingRequest)` is keyed on the request kind (`signer_limit.rs:365`).
- Binance takes `let charge = self.budget.acquire(cost)` and later calls `record_used(charge, used)` (`polyoxide-binance/src/usdm/request.rs:95-108`). Its next-minute hold, `(None, None) => hold_until_next_minute()`, applies only when no retry is left (`:136-137`).
- `sign_clob_auth` is `async`, through alloy's `sign_hash` (`polyoxide-clob/src/core/eip712.rs:419-428`).

**Why units diverge.** The foundation epic fixes these signatures first. The Polymarket and
Binance migrations then have three options, all of them bad:
- pass the path or the charge ticket through side channels;
- edit core, which defeats the "no foundation edits" test for Kalshi;
- keep their own loops, which breaks AD-8.

Two failures in the drawn flow need no epic to choose them:
- **Binance's final-attempt hold is lost.** The flow returns on `Fail` before any hold, so the
  client-wide next-minute hold on the last attempt silently disappears. Unlike sockets, which
  have AD-12, no HTTP test-migration gate guards this.
- **An async signer has nowhere to run.** If `Authenticator::headers` is synchronous, the L1
  `/auth/api-key` signature must either run outside the loop (contradicting AD-8's last
  bullet) or block on an async signer.

**Fix.** Tighten AD-8 with the hook contract, and add `Authenticator` to AD-5's Binds:
- `acquire(&RequestMeta { method, path, query, cost }) -> Ticket`. The cost is
  venue-defined, or a `u32` plus a bucket key, so a venue can name its read/write or funding
  bucket.
- `observe(&Ticket, &ResponseMeta { status, headers }, &AttemptInfo { attempt, last })`. It
  runs before `decide` and may extend the client-wide cooldown. Alternatively, `decide` may
  return `Fail { hold }`, applied to the shared cooldown. Pick one.
- `Authenticator::headers(&RequestMeta + body bytes, attempt) -> impl Future + Send`.
- Make CAP-2's out-of-foundation Kalshi-style bucket test, with separate read and write
  buckets and integer token costs, a named gate of the core epic.

### F2 — high — AD-11's outage markers contradict D8, and with AD-12 cannot pass

**Spec.**
- divergences.md D8: "Event enums; binance's `Disconnected{path}` → `Reconnected{path}` invariant … **Perps has no `Disconnected`.** Rtds signals reconnects by a new `Snapshot`."
- SPEC Assumptions extend the invariant to the CLOB only: "The CLOB's outage markers keep the Disconnected-then-Reconnected invariant, as Binance's do."
- Non-goal: "Unifying the deliberate divergences."

**Spine.**
- AD-11: "`Supervisor` emits exactly one `Disconnected` then one `Reconnected` per outage". This binds perps, usdm, clob and Kalshi.
- AD-12: the perps suites "move without changes to their assertions".

**Code today.**
- The perps `Event` has `Reconnected` and no `Disconnected` (`polyoxide-perps/src/ws/event.rs:115`).
- The suite asserts that the very next event after an outage is `Event::Reconnected` (`supervised.rs:697, 730, 756, 788, 842, 1070, 1097`).

**Why units diverge.** The `polyoxide-ws` epic implements unconditional markers, and the
perps migration then fails AD-12. Its agent can only break AD-11, break AD-12, or add
`Disconnected` to perps' public enum. The last is an unannounced behaviour change for prader's
perps feed.

**Fix.** Rewrite AD-11 so markers are declared by the `Protocol`:
- If a `Protocol` emits `Disconnected`, the `Supervisor` guarantees exactly one `Disconnected`
  followed by exactly one `Reconnected` per outage (Binance per path, clob).
- Otherwise it emits only `Reconnected` after replay (perps).
- Kalshi chooses; recommend the paired form.

### F3 — high — `polyoxide-ws` cannot reach the rule AD-3 says only `polyoxide-venue` may define

**Spec.**
- DRIFT R1: socket reconnects "Retry 408/425/429/5xx everywhere, matching core's HTTP rule".
- CAP-3: "handshake-status and close classification" exist once among the socket blocks.
- CAP-4: "one retriable-status rule serve[s] all venues".
- SPEC constraint: the shared socket crate "may depend **only** on tokio, tokio-tungstenite, futures-util, thiserror, tracing, rustls … and optionally serde/serde_json".

**Spine.** Its rules disagree with each other:
- AD-1's rule text, "facades → venue crates → `polyoxide-core` / `polyoxide-ws` → `polyoxide-venue`", reads as allowing a `ws → venue` edge.
- The mermaid graph has no `ws → venue` edge.
- AD-2's dependency list for `polyoxide-ws` excludes `polyoxide-venue`.
- AD-3 says the retriable-status rule and the `Retry-After` parser live in `polyoxide-venue`, and "No other crate defines any of these".

**Why units diverge.** The ws epic has three choices:
- depend on `polyoxide-venue`, which breaks AD-2 and the spec's dependency list and pulls in `rust_decimal` and `dynosaur`;
- re-define 408/425/429/5xx, a second copy of inventory H12 that breaks AD-3;
- leave handshake classification to each module, so four copies return and W7 is violated.

AD-14's `network` tag has the same problem with close classification. The nightly classifier
today treats close codes 1001/1011/1012/1013, and a TLS EOF without `close_notify`, as
transient. Those facts live at the socket layer.

**Fix.** Pick one option and correct AD-1's text and graph to match:
- **(a)** Make `polyoxide-venue`'s records and traits a default feature. `polyoxide-ws` then
  depends on it with `default-features = false`, which gives the status rule, the
  `Retry-After` parser and the classification interface only. Amend the spec's dependency
  list to match.
- **(b)** `polyoxide-ws` takes the status rule as an injected predicate and returns neutral
  `HandshakeClass` / `CloseClass` values. Each module maps them one-to-one into the CAP-4
  interface, in one place.

### F4 — high — "Venue-only fields stay reachable" has no mechanism; order and trade ids and kill outcomes are unfixed

**Spec.**
- SPEC constraint: "Normalization loses no venue information: … ids are opaque or at least `u64` … venue-only fields stay reachable."
- CAP-5: "venue-only facts stay reachable".
- CAP-6: "Venue-specific order options and kill outcomes (FAK/FOK) are preserved, not flattened."
- glossary.md, **Venue-options slot**: "A typed place in a normalized request or record for fields only one venue has".
- venue-landscape.md:
  - prader's `PerpVenueFacts` is a tagged enum;
  - Kalshi uses UUID order and trade ids;
  - Binance trade ids exceed `u32`.

**Spine.**
- The conventions fix `Option`, `Decimal`, `UnixMillis`, the `MarketKey` id as `Arc<str>`, and "native wire types keep their fields as sent".
- Nothing covers:
  - the slot itself;
  - the id types of orders, trades and fills;
  - how a FAK/FOK kill surfaces through the trading trait.

**Why units diverge.** The CAP-5 implementations for clob, perps and usdm may be built by
different agents.
- **The slot.** The obvious shape, a tagged enum in `polyoxide-venue` modelled on prader's, is
  closed. Kalshi would have to edit the foundation, which fails the success signal. AD-4
  rejected a closed enum for keys for exactly this reason.
- **Ids.** A `u64` trade id on Binance cannot hold Kalshi's UUIDs.
- **Kill outcomes.** One epic returns `Err(ClobError::FakUnmatched)`, another
  `Ok(Ack { status: Killed })`. CAP-6's Kalshi mapping table cannot be written consistently
  against either.

**Fix.** Add an AD for the venue-options slot. Normalized records and requests carry
venue-only data through an associated type on the trait (static path), or through a typed
extension retrieved by downcast for `Dyn` users. Never through an enum in `polyoxide-venue`.
Then:
- add a convention row: order, trade and fill ids in normalized records are opaque
  `Arc<str>` newtypes;
- state the kill-outcome rule, for example: "a FAK/FOK kill is an `Ok` result whose terminal
  status is `Killed { reason }`; the native client keeps its `ClobError` variants".

### F5 — high — CAP-10 has no owning rule and no CI shape that can detect the leak

**Spec.**
- CAP-10 success: "A test pins each client's request headers, including `Accept-Encoding`, whichever venue features are enabled."
- registration-points.md: "adding Binance turned on `reqwest`'s `gzip` for every crate … shipped a regression in 0.37.0".
- Why: "One venue's `reqwest` feature changed every other client's wire behaviour."

**Spine.**
- The capability map lists "CAP-10 | per-client settings; optional dependencies | AD-2, AD-1".
- AD-2 is about keeping `reqwest` and `alloy` out of socket-only builds, not about header stability.
- No rule says who may depend on `reqwest`, `tokio-tungstenite` or `rustls`, or turn on their features.
- No header-pin test is required.
- The CI diagram runs one feature set. The all-features workspace build is the maximally
  unified build, where the 0.37.0 class of leak is invisible because gzip is on everywhere.

**Why units diverge.**
- Kalshi, or a later venue, adds `reqwest = { features = [...] }` directly and changes every
  client in a unified build.
- The core epic makes gzip a core feature enabled by whichever venue wants it, which recreates
  0.37.0.
- Each venue epic writes its own header test, or skips it.

**Fix.** Add a new AD, or extend AD-2:
- Only `polyoxide-core` depends on `reqwest`, and only `polyoxide-ws` on
  `tokio-tungstenite` / `rustls`. Their features are declared once, unconditionally, in
  those crates. CI fails on a direct dependency from a venue crate.
- Every client builder sets `Accept-Encoding` explicitly.
- Each venue crate has a mock test pinning the full header set of one request per module.
  CI runs it in two builds:
  - the venue crate alone, with minimal features;
  - `-p polyoxide --features full`.

### F6 — high — Behaviour-carrying tests: only the supervision suites are gated

**Spec.** SPEC constraint: "Tests that carry behaviour move; they are not deleted or weakened.
That covers mutation-tested rate-limit rules, supervision invariants, kill-outcome
classification, effective-quota agreement tests, spec and wire agreement tests, and live drift
detectors."

**Spine.** AD-12 covers the perps and Binance supervision suites and nothing else.

**Why units diverge.**
- **Effective-quota tests.** The `documented_*_limits` tests (about 1,100 lines,
  venue-landscape.md) are touched by two epics. R1 replaces `HttpClient`'s
  `Option<RateLimiter>` with a `Throttle`; R2 moves the tables into `polyoxide-polymarket`.
  Each can assume the other preserved "the effective quota a request resolves to" (CLAUDE.md).
- **Agreement helpers.** CAP-9's consolidation can fold perps' forked OpenAPI synthesiser
  onto data's simpler one. The fork adds `$ref`-nullable, enum and example handling, and
  `OBSERVED_EXTRA` (inventory T4). That weakens perps' spec agreement without editing a single
  assertion.
- **HTTP behaviour tests.** These sit outside AD-12:
  - Binance's mock tests for the 418 ban and the next-minute hold;
  - core's `note_rate_limited`-before-`should_retry` mutation tests;
  - `classify_order_kill`.

**Fix.** Generalise AD-12 into a "behaviour-suite custody" AD:
- List the protected suites:
  - supervision;
  - `documented_*_limits`;
  - the mutation-tested limiter, cooldown and `Retry-After` rules;
  - `classify_order_kill`;
  - spec and wire agreement, including the allow-lists and stale-excuse checks;
  - the live drift detectors.
- Moves keep test names and assertions.
- A consolidated helper must be a superset of every fork it replaces.
- Each moving PR reports per-suite test counts before and after.
- Mutation-tested rules are re-mutated after the move.

### F7 — high — Release staging collides with the Polymarket-in-core move list; R2 atomicity is unstated

**Spec.**
- venue-landscape.md, "Polymarket code in `polyoxide-core` today": "CAP-7's grep test requires all of this to leave the venue-neutral foundation. It moves into `polyoxide-polymarket`".
- CAP-7 success: "A grep test finds no Polymarket identifiers in the venue-neutral foundation."
- SPEC constraint: breaking changes ship "in a lockstep minor bump", and prader migrates only from crates.io.

**Spine.**
- AD-16, R1: "Public paths are unchanged."
- AD-16, R2: "every public rename or move … and nothing else breaking".
- AD-10: "Venue limiter tables live in their venue crate."
- The grep test appears nowhere.
- The spine's own memlog says R1 "error and builder types may change". The spine text dropped that clause.

**Code today.** `polyoxide-core` publicly re-exports these (`src/lib.rs:66-74`):
- `Signer` and `Base64Format`;
- `DepositWalletRole` and `SessionSignerScope`;
- `signer_limit::*`;
- `RateLimiter`, including its `*_default` tables.

**Why units diverge.**
- **Where the move list goes in R1.** There is no `polyoxide-polymarket` to receive these
  items, and moving them changes public paths. The R1 foundation epic may:
  - leave them in core, so AD-10 and the grep test fail in R1;
  - create an interim crate, an extra rename for prader;
  - push them into `polyoxide-clob`, although relay also uses `Signer`.

  The R2 epic may assume R1 already did it.
- **Breaking changes that are not renames.** AD-15's error-enum changes and the builder's
  limiter knob becoming a `Throttle` are breaking. An agent cannot tell whether they belong
  in R1 ("paths unchanged") or R2 ("nothing else breaking").
- **Partial renames.** With "epics merge to main whenever green", a patch release cut while R2
  is half-merged ships half the renames. That is the double prader migration AD-16 exists to
  prevent.

**Fix.**
- AD-16: "R1 may change types and signatures (error enums, builder knobs, the limiter
  interface) but not paths. The venue-landscape move list leaves core in R2, with the
  consolidation. The CAP-7 grep test becomes a CI gate in R2."
- State how R2 stays atomic. Either R2 epics merge to an integration branch, or releases from
  main freeze from the first R2 merge until the R2 bump, with patches cut from an R1 release
  branch.
- Rename the release stages, for example Stage A/B/C. AD-16 currently reads "R1's release
  notes name DRIFT R1, R2 and R4", which collides with divergences.md's R1–R10.

### F8 — high — The success signal's allowed-touch set is undefined, and registration and classification force edits elsewhere

**Spec.**
- Success signal: the skeleton lands "touching only its own directory and the registration source, with no foundation edits and no copied infrastructure".
- CAP-8: "workspace members, publish order, nightly live rows, **schema watch list and exclusions, classifier patterns**, and the docs crate table" derive from that source or are checked against it.
- registration-points.md lists, among others:
  - the umbrella features and the CLI enum arms;
  - the nightly-schema watch list and its triplicated exclusions;
  - `AUTH_GATED_RE` (`POLYMARKET_*` only).
- venue-landscape.md:
  - Kalshi's OpenAPI "fits the mirror and nightly-schema pattern";
  - the Kalshi socket needs an API key "**even for market data**".

**Spine.**
- AD-13 makes the README table and INDEX "checked", so they stay hand-edited.
- The only registration is `members` plus per-crate `[package.metadata.polyoxide]`.
- The AD-14 tag set is `network|rate-limited|unauthorized|invalid|venue-refusal|environmental`.
- The umbrella exposes `{venue, polymarket, binance}`.

**Why units diverge.** The Kalshi skeleton epic is judged on edits the registration epic decides.
- **(a) Hand-edited docs.** Checked-not-generated README and INDEX tables mean the skeleton
  must edit files outside its directory.
- **(b) Mirrors with no crate.** `docs/specs/bridge` and `combos-rfq` belong to no crate, yet
  `nightly-schema.yml:52-63` watches both. No crate's metadata can declare them. Deriving the
  watch list therefore either drops them silently (a live drift detector lost; see F6) or
  keeps a second list, which contradicts "the only registration". The exclusions (sports
  AsyncAPI, rtds, the undocumented hosts, Binance) have no stated home either.
- **(c) No auth-gated class.** Today a missing credential is skipped silently through
  `AUTH_GATED_RE` (`classify_failures.py:25`). Under tags-only matching it becomes
  `unauthorized`, which either files missing secrets as real failures or hides real 401s. The
  Kalshi socket test needs a key even for market data, so the skeleton would need a classifier
  edit.
- **(d) "Environmental" has no source.** CAP-4's six questions include no environmental
  answer, yet AD-14 maps 451 to `environmental` "through the venue's own classification". And
  sports' "legitimately time out" is a test's verdict, not an error value.
- **(e) Hand-written facade lists.** The umbrella and CLI lists are hand-maintained; nothing
  says whether the skeleton joins them.

**Fix.**
- AD-13: an explicit allowed-touch list for a new venue:
  - the `members` line;
  - the crate's own directory;
  - `docs/specs/<venue>/`;
  - files generated by a script (README table, INDEX, the CLAUDE.md crate list), with CI
    checking that the generated output matches what is committed.
- A workspace-level registration for crate-less mirrors and exclusions, for example
  `[workspace.metadata.polyoxide.mirrors]`.
- Deferred: "Kalshi joins the umbrella and the CLI at Kalshi integration."
- AD-14:
  - add an `auth-gated` tag (skipped silently), emitted by a test-support `require_env`
    helper that names the venue's variables;
  - let a test declare `environmental` directly;
  - give the CAP-4 interface a geo-refusal answer, or state that 451 is
    `venue-refusal` with a code the reporter maps.

### F9 — medium — AD-3 and AD-15 widen the credential-free footprint beyond the spec's constraint

**Spec.**
- SPEC constraint: "Enabling only such a module builds the shared socket crate's dependencies and the module's own, **nothing more**."
- D18.
- CAP-3 success, which checks only `reqwest` and `alloy`.

**Spine.**
- AD-15 makes every module, sports and rtds included, implement the `polyoxide-venue` interface.
- AD-3 gives `polyoxide-venue` the dependencies `rust_decimal`, `serde`, `thiserror`, `futures-core` and `dynosaur`.
- AD-2's CI check only greps for `reqwest` and `alloy`.
- The seed puts `Signer` (hmac, sha2, base64) and the signer limiter (governor) in `polyoxide-polymarket/src/shared/`.

**Code today.** `polyoxide-sports/Cargo.toml` has no `rust_decimal`.

**Why units diverge.** AD-2's check passes while `-F sports` builds `rust_decimal`, `dynosaur`
and `futures-core`. Unless `shared/` is cfg-gated, it also builds hmac, sha2 and governor. The
consolidation epic has no rule telling it to gate `shared/`.

**Fix.** Either record this as an accepted deviation and amend the spec constraint, or apply
F3(a)'s minimal feature. Tighten AD-2's check to an allow-list: the `-F rtds` and `-F sports`
trees may contain only
- the `polyoxide-ws` dependencies,
- the minimal `polyoxide-venue`, and
- the module's own declared dependencies.

Add a rule: every `shared/` item is gated by the features that use it.

### F10 — medium — Duplication-inventory rows with no stated home

**Spec.**
- Success signal: "Every row in `duplication-inventory.md` has exactly one definition."
- CAP-1: "No crate defines its own `open_enum!`, `wire_enum!`, `UnknownVariant` or builder knobs."

**Spine.** The seed gives core "macros", and AD-3 gives `polyoxide-venue` its list. The
spine is silent on these rows:

| Row | What is unsettled |
| --- | --- |
| H10/H11 (enum macros, positional decimal serde) | Core or `polyoxide-venue`? Normalized records in `polyoxide-venue` need open enums for side and time in force, but cannot depend on core. |
| H14/H15 (error-body parse, `ApiError` wrapping) | Does core's `ApiError` survive AD-15's "No venue-wide catch-all error"? What does the send loop return on `Fail`? Where does D14's per-venue body decoding plug in? |
| W12 (request/answer correlation) | Unplaced. |
| W15 (supervision test helpers) | `polyoxide-ws` `test-server` or `polyoxide-test-support`? |
| T7 (minute-boundary waits) | Unplaced. |
| T9 (Python capture-script helpers) | No home at all, because they are not Rust. |
| C4 (Python error mapping, "replaced by CAP-4") | Unplaced. |
| H5 (reserve-a-tenth, depth-one slot) | Must not be applied to `signer_limit`, which D1 says keeps `allow_burst`. |

**Why units diverge.** Two epics each create one definition:
- `next_event` written both in ws `test-server` and in test-support;
- the Polymarket epic keeps `impl_api_error_conversions!` while Binance decodes directly;
- the H5 helper is applied to the signer buckets, which reverses D1.

**Fix.** Add an "inventory row → home" table for the ambiguous rows, as a convention or a
companion. State the send loop's error output: either a core
`HttpError { status, headers, body, retry_after }` that each module enum wraps with `#[from]`,
or a venue `ErrorDecoder` hook.

### F11 — medium — Socket DFR rows that the Protocol hook list does not carry; event delivery

**Spec.**
- D12: "Clob decodes Binary frames as text; others skip them."
- D5: "Binance counts any frame. Perps counts frames plus an ok pong." CLAUDE.md adds that Binance counts pongs and the server's pings.
- D10: "Perps `Recovery::Retry` and `SequenceRegressed`".
- Inventory W7: `Recovery` exists "in rtds, perps (adds `Retry`) and binance".
- CAP-6: fills arrive "as an async event stream".
- venue-landscape.md (prader): "unbounded because dropping a fill desyncs inventory".

**Spine.** AD-11 lists "decode" and "the liveness predicate" without saying which frames reach
them. Nothing covers `Recovery`'s variants, or whether the `Supervisor` may drop events when
the consumer lags.

**Why units diverge.**
- The ws epic (R1) filters Binary, Ping and Pong before decode and liveness, as perps and
  Binance do today. The clob supervisor (R3+, CAP-12) then needs a foundation edit, and so do
  the liveness rules that count pongs.
- A shared `Recovery` without `Retry` loses D10. AD-12 catches this, but late.
- The trading epic builds the fill stream on a `Supervisor` whose drop policy is unstated.

**Fix.** Extend AD-11:
- `decode` and the liveness predicate see every inbound frame: Text, Binary, Ping, Pong and Close.
- A `Protocol` may return a venue-specific retry `Recovery`.
- The `Supervisor` never drops a decoded event; a full queue applies backpressure.

### F12 — medium — 425: the classification rule versus the loop's own retry set

**Spec.**
- D17: "`425 Too Early` treated as retriable … belongs to the Polymarket response policy, not the foundation (CAP-7's no-Polymarket-identifiers test)."
- H12, CAP-4 and R1 call for one 408/425/429/5xx rule.
- CLAUDE.md: "`HttpClient::should_retry` only ever retries `429` and `425`".

**Spine.**
- AD-3's rule in `polyoxide-venue` includes 425.
- The seed puts the "425 policy" in Polymarket's `shared/`.
- AD-8 has venues supply the `RetryPolicy`, but no default is stated.

**Code today.**
- Binance's loop retries only 429 (`usdm/request.rs:126-147`).
- Core retries 429 and 425 (`client.rs:132`).

**Why units diverge.** If the core epic's default `RetryPolicy` retries whatever
`is_retriable()` accepts, Binance, and later Kalshi, start retrying 5xx, 408 and 425
unannounced. If the default instead retries 429 and 425, Polymarket's matching-engine rule
sits in the foundation, against D17.

**Fix.** State in AD-3 and AD-8:
- The shared rule answers only CAP-4's "is retriable" question for callers.
- The loop's own retry set is the venue's `RetryPolicy`. Core's default retries 429 only, and
  Polymarket's policy adds 425.

Add the CAP-7 grep test to AD-13's list of CI failures.

### F13 — medium — The shared CLI credential plumbing has no home

**Spec.** SPEC constraint: "Shared code owns the mechanism: keyring access, secret redaction,
and **the plumbing for CLI `credentials` store, show and delete**. Each venue owns its
credential type: fields, validation, service name."

**Spine.**
- The Secrets convention covers `Secret<T>`, the keychain in core, and venues owning their
  type and service name.
- Nothing covers the CLI plumbing.
- `polyoxide <venue> <module> <verb>` has no slot for `credentials`.

**Code today.** The CLI hardcodes `polyoxide_clob::KEYCHAIN_SERVICE` and its field names
(`polyoxide-cli/src/commands/credentials/mod.rs:116-125`).

**Why units diverge.**
- The R2 CLI epic picks one of `polyoxide credentials --venue x` and
  `polyoxide polymarket credentials`.
- The consolidation epic keeps a per-type `from_keychain`.
- Kalshi integration must later edit the CLI.

**Fix.** Add a convention row:
- A `StoredCredential` trait in core's keychain, covering the service, the field names,
  which fields are secret, and validation.
- The command is `polyoxide <venue> credentials <store|show|delete>`, generic over that trait.

### F14 — medium — The drift-detector layout is not fixed, but CAP-11's guide is "a rendering of this spine"

**Spec.**
- CAP-11's guide covers "the drift-detector pattern" and "limiter measurement".
- glossary.md lists the pattern's parts:
  - `OBSERVED.md`;
  - fixtures with `PROVENANCE.md`;
  - a capture script;
  - wire- and spec-agreement tests;
  - a live no-unmodelled-keys test.

**Spine.**
- Only the test-target names are fixed.
- The capability map gives CAP-11 to "the agent guide (rendering of this spine)".

**Why units diverge.**
- One crate per venue means `polyoxide-polymarket/tests/fixtures/` must be partitioned by
  module.
- Nothing fixes:
  - the fixture paths;
  - the `OBSERVED.md` location (today flat, `docs/specs/<host>/`);
  - the capture-script names;
  - the spec ids.
- If the R2 epic moves `docs/specs/` to a venue-first layout, the `spec:<id>` labels (matched
  by label intersection) change and open drift issues are orphaned.

**Fix.** Add convention rows:
- `tests/fixtures/<module>/` with `PROVENANCE.md`;
- the `OBSERVED.md` path;
- `scripts/capture_<venue>_<module>.py`;
- the live drift-test name;
- "`docs/specs/` layout and spec ids do not change in R2, unless the labels migrate in the same change".

### F15 — medium — The spine's companions omit three spec files

The spine frontmatter `companions:` lists SPEC, divergences, duplication-inventory and
CLAUDE.md. It omits:
- `venue-landscape.md`, which holds the "must accommodate every column" mandate and the
  move-out-of-core list;
- `registration-points.md`, the list behind AD-13;
- `glossary.md`, including the venue-options slot.

A builder who follows the spine's companions never reads them. **Fix:** add all three.

### F16 — low — Record that AD-6 supersedes the spec's per-product-class wording

**Spec.**
- CAP-5: "through traits defined per product class".
- Assumptions: "Traits are organised by product class."

**Spine.** AD-6 defines one base `MarketData` plus capability traits. This is a sound
refinement, and it meets CAP-5's success criterion.

**Fix.** Say in AD-6 that it supersedes the spec assumption, so a trait-epic agent reading the
spec does not build a base trait per product class.

### Low-severity notes

- **L1:** DRIFT R4's "the upper clamp is a parameter" (Binance clamps to 3 days) is missing from AD-3 and AD-9.
- **L2:** AD-8 does not fix the retry line's level. WARN is how the CLI's stderr subscriber makes recoveries visible. Moving Binance and perps logs to the `polyoxide_core` and `polyoxide_ws` targets changes users' `RUST_LOG` filters, so put it in the release notes.
- **L3:** CAP-4's "enforced at compile time" has no mechanism. Suggest a `type Error: Classify` bound on every `polyoxide-venue` trait, and the same bound on the send loop's error parameter.
- **L4:** H6's default concurrency (2, 4 or 8) is behaviour. The shared builder must keep each client's default.
- **L5:** AD-5 binds `Protocol`. State that it is never used through `dyn`, or `polyoxide-ws` needs `dynosaur`, which is outside the spec's dependency list.
- **L6:** Verify that the crates.io token can publish *new* crates (`polyoxide-venue`, `-ws`, `-polymarket` and `-cli`, which has never been published). An update-only token makes the R1 release fail, and per CLAUDE.md a failed release step is easy to miss.

---

## Contradictions between spine and spec

1. **AD-11 vs D8 and the non-goal on unifying divergences.** Unconditional outage markers for
   perps, which has none, and AD-12 then cannot pass (F2).
2. **AD-8's Fail→return vs D3.** Binance's client-wide next-minute hold on the final attempt is
   lost, and its charge-ticket correlation is not expressible (F1).
3. **AD-1, AD-2 and AD-3 vs each other, and vs R1/CAP-3.** The socket crate needs a rule it may
   neither import nor redefine (F3).
4. **AD-3/AD-15 vs the SPEC constraint "nothing more".** The credential-free footprint grows by
   `polyoxide-venue`'s dependencies, and AD-2's check is weaker than the constraint (F9).
5. **AD-3 (425 in the foundation rule) vs D17.** This is reconcilable only if the
   classification and retry-set split is written down (F12).
6. **AD-6 vs CAP-5's intent and the Assumption "organised by product class".** A deliberate
   refinement, but unrecorded (F16).
7. **AD-13 ("the only registration" is per-crate metadata) vs CAP-8's schema watch list.**
   Mirrors with no crate (bridge, combos-rfq) cannot be registered (F8b).
8. **AD-16 R1 ("public paths unchanged") vs AD-10 and the CAP-7 grep test.** Core publicly
   re-exports the Polymarket items that must leave it (F7).

---

## Coverage tables

### DFR rows (must stay per-venue)

| Row | Spine status |
| --- | --- |
| D1 `allow_burst` split | Not mentioned; at risk from H5 consolidation (F10) |
| D2 reserve a tenth | Left to the spec; fine |
| D3 Binance weight, hold, 418 ban, funding bucket | **Contradicted** by AD-8's flow (F1) |
| D4 ping scheduling | AD-11 (wall clock in the `Supervisor`; rtds outside it); fine |
| D5/D6 liveness, delivered | AD-11 predicates; their inputs are unfixed (F11) |
| D7 supervision shape | AD-11; fine |
| D8 event enums | **Contradicted** by AD-11 (F2) |
| D9 rotation, paced replay | AD-11 (`max_connection_age`, paced replay, one supervisor per path); fine |
| D10 perps `Retry`, `SequenceRegressed` | Unstated (F11) |
| D11 rtds snapshot semantics | rtds stays outside the `Supervisor`; fine |
| D12 clob Binary frames | Unstated (F11) |
| D13 HTTP vs socket backoff | Kept apart by AD-1's crate split; fine |
| D14 error-body formats | No decode hook stated (F10) |
| D15 FAK/FOK | Stays in the clob; its representation in the trading trait is unfixed (F4) |
| D16 `Interval` sets | Venue types; fine |
| D17 425 | Ambiguous (F12) |
| D18 nothing heavy | AD-2, weaker than the constraint (F9) |

### DRIFT decisions R1–R10

| Row | Spine status |
| --- | --- |
| R1 handshake statuses | The rule's home contradicts AD-2 and AD-3 (F3) |
| R2 connect timeout | Left to the spec, as a kit block; fine |
| R3 close reply | Left to the spec; fine |
| R4 one `Retry-After` parser | AD-3 and AD-9; clamp parameter missing (L1) |
| R5 workspace `rustls` | AD-2 (ws side); fine |
| R6 `test-server` | Convention; fine |
| R7 relay on the send path | AD-8 and AD-15; fine |
| R8 gating holes | AD-8; fine |
| R9 shared TLS function in live tests | Left to the spec; fine |
| R10 log line | AD-8; fine |

The release-notes naming collides with the release stages (F7).

### CAP success criteria

| CAP | Status |
| --- | --- |
| 1 | AD-8; vocabulary home unclear (F10) |
| 2 | AD-10; interface shape (F1) |
| 3 | AD-2, AD-11, AD-12; F3, F9, F11 |
| 4 | AD-3 and AD-15; environmental and auth-gated (F8d, F8c); L3 |
| 5 | AD-4, AD-6, AD-7; F4, F16 |
| 6 | AD-5 and AD-7; F4 |
| 7 | Conventions; grep test missing (F7, F12) |
| 8 | AD-13; F8 |
| 9 | `polyoxide-test-support`; T9 and superset rule (F10, F6) |
| 10 | **No owner** (F5) |
| 11 | F14 |
| 12 | AD-11; F11 |

### SPEC constraints

| Constraint | Status |
| --- | --- |
| No shims, lockstep bump, prader from crates.io | AD-16; R2 atomicity (F7) |
| Credential-free footprint | AD-2 partial (F9) |
| Per-attempt socket handshake auth | AD-11; fine |
| Per-request throttle cost | AD-10; shape (F1) |
| DFR rows stay per-venue | No general rule; F1, F2, F11, F12 |
| DRIFT rows applied as decided | Mostly; F3 |
| Credential storage split | Partial (F13) |
| Behaviour-carrying tests move | AD-12 only (F6) |
| Normalization loses nothing | Partial (F4) |
| CI gates | Fine |
| Soak log line | AD-8; fine (L2) |

### Success signal

| Part | Status |
| --- | --- |
| Kalshi skeleton with no foundation edits | At risk (F1, F8) |
| One definition per inventory row | Homes missing (F10) |
