---
review: reconcile
target: ../ARCHITECTURE-SPINE.md
inputs:
  - ../.memlog.md (architecture decision log, 53 lines)
  - ../../../../../CLAUDE.md (standing rules)
  - ../../../../specs/spec-venue-extensibility/{SPEC.md,divergences.md,duplication-inventory.md,registration-points.md,.memlog.md}
  - code at e3d8c3e (worktree aidanb/restructure)
date: 2026-10-08
---

# Reconciliation review: spine vs memlog vs CLAUDE.md

## Verdict

**Not ready to bind epics.** The spine is faithful to the memlog on nearly every decision, and
it keeps the core HTTP retry rules from CLAUDE.md: observe before decide, observe on the last
attempt, Retry-After only lengthens the wait, and cooldowns only extend. Four places would still
lead parallel agents to break mutation-tested behaviour or to deadlock:

- **AD-14** removes the `auth-gated` verdict and the transient-retry mapping.
- **AD-10** gives `Throttle::acquire` one cost with no route. That cannot carry the two
  rate-limit layers, which count different things, and it invites the `allow_burst` 2x regression.
- **AD-11 and AD-12 contradict each other on perps.** AD-11 requires a `Disconnected` for every
  outage, but perps has no such event and its suite asserts that `Reconnected` comes directly.
- **AD-3 and AD-8 do not separate the error classifier from the loop's retry set.** That opens
  the way to retrying 5xx responses on order POSTs.

AD-16 also leaves the memlog's "error and builder types may change in R1" out, so AD-15 has no
release in which it can land.

## Findings at a glance

| ID | Sev | Area | One line |
|---|---|---|---|
| F1 | High | AD-14 / CLAUDE.md nightly | Tag set drops `auth-gated` (conflates it with `unauthorized`), has no class for 5xx/425/timeouts/WS drops, no tag→verdict map, and untagged panics fall to `real` (the expensive direction) |
| F2 | High | AD-8, AD-10 / CLAUDE.md rate limits, D1, D2 | `acquire(cost)` is a route-less cost: it cannot key Cloudflare's path+method buckets, cannot charge requests on one layer while charging orders on another, and cannot correlate Binance's charge minute. A cost >1 hitting `quota()`'s depth-1 bucket is refused forever; the obvious fix (add `allow_burst`) restores the 2x over-permit. D1/D2 are not cited |
| F3 | High | AD-11 vs AD-12, D8, memlog L31 | "Exactly one Disconnected then one Reconnected" is impossible for perps (no `Disconnected`; 17 inline tests assert `Reconnected` directly), so the AD-12 gate cannot pass unchanged; the memlog decision "perps Retry and SequenceRegressed are protocol events" is missing |
| F4 | High | AD-3, AD-8 / CLAUDE.md Retriability, D17 | Spine never says the loop's retry set (429, plus 425 for Polymarket) is narrower than the classifier's 408/425/429/5xx; a default `RetryPolicy` built from AD-3's rule would retry 5xx on non-idempotent order POSTs. AD-3 also puts 425 in the foundation, against D17 |
| F5 | Med-High | AD-9 / CLAUDE.md Binance, memlog L25/L28 | 418 is modelled as a `RetryPolicy` lengthening a retry; in code it is not retried (Fail + client-wide cooldown). The next-minute hold applies when *no retry is left*, so it depends on the retry decision, which runs after `observe` |
| F6 | Med-High | AD-16 / memlog L36 | Drops "error and builder types may change" from R1 and adds "nothing else breaking" to R2, so AD-15 / DRIFT R7 have no release; DRIFT R6 (feature rename) is a public rename placed in R1; moving Polymarket code out of core changes `polyoxide_core` paths (R2-only); stage names R1/R2 collide with DRIFT R1/R2 |
| F7 | Med | AD-1, AD-2, AD-3 / memlog L17 | `polyoxide-ws` holds the handshake "classifier" (seed) and needs 408/425/429/5xx (DRIFT R1), but AD-3 says only `polyoxide-venue` defines that rule and L17/AD-2 forbid ws→venue; AD-1's prose arrow implies ws→venue |
| F8 | Med | CLAUDE.md README doctests | 9 `include_str!` README doctest sites (incl. the root README); consolidation into `polyoxide-polymarket` silently deletes six of them unless the spine pins where they go |
| F9 | Med | AD-2 / CLAUDE.md rustls `std` note | Crate boundaries become feature boundaries; CI builds only `--all-features`, so a module that compiles only via unification passes CI; AD-2 checks two cargo trees, no per-module builds |
| F10 | Med | CLAUDE.md as live input; memlog L12 | CLAUDE.md is auto-loaded into every parallel agent and contradicts the spine in ≥8 places (rtds/sports "depend on NOTHING", Binance "not in the umbrella", Publishing Order list, "Twelve crates" graph, …); spine neither lists superseded rules nor puts CLAUDE.md's lists under AD-13 |
| F11 | Med | AD-8 / CLAUDE.md "retries away is invisible" | Soaks match target `polyoxide_core` **and level WARN** and text; spine pins target+text, not level. Permit release before sleep and the non-retry WARNs (418 ban, no-retry-left hold) are unpinned |
| F12 | Med | AD-5, Stack / CI | MSRV 1.91 is not a CI gate (every job uses `dtolnay/rust-toolchain@stable`), so AD-5's dynosaur fallback trigger "fails … gates on MSRV 1.91" can never fire |
| F13 | Med | AD-13 / CAP-8, CLAUDE.md nightly-schema | nightly-schema watch list, exclusions, spec ids and `spec:<id>` labels not derived; CAP-8's "live_*.rs lacks a nightly row" check missing; publish-order dev-dep rule; CI-scripts job missing from flowchart; new CI checks silently withhold releases; first foundation merge before the publish script makes main unreleasable |
| F14 | Med | AD-10 / CLAUDE.md effective-quota tests | `documented_*_limits` tests call core-private `resolve_specs`; moving tables out of core needs a public introspection seam or the tests weaken to presence checks. The window-quota engine (`quota()`, `RESERVED_FRACTION`, governor) has no home in the seed |
| F15 | Med | memlog L10 constraint | "Behaviour-carrying tests move, not weaken" appears only as AD-12 (supervision); kill-outcome, effective-quota, agreement, drift-detector and classifier tests have no rule |
| F16 | Low-Med | AD-8 Authenticator | `authenticator.headers(attempt)` omits method/path/query/body; signing needs them, and Binance signed routes sign the query string |
| F17 | Low-Med | AD-1 / CLAUDE.md test-server | Published `test-server` features expose fixtures to downstream (CLI tests use `polyoxide_binance::usdm::ws::fixtures`); those cannot come from dev-only `polyoxide-test-support` |
| F18 | Low | memlog L50/L51 assumptions; CLAUDE.md Testing Conventions | Assumptions rendered as conventions without a flag; the test-target pattern does not cover `live_session_keys`, `v2_*`, `supervision*`, `mock_api`; `#[ignore]` on live tests unpinned |
| F19 | Low | AD-10 vs AD-5 | `Arc<dyn Throttle>`: an RPITIT trait is not dyn-compatible; it must be the dynosaur wrapper |
| F20 | Low | Conventions / CLAUDE.md rustdoc gate | `src/shared/` creates `pub(crate)` items; the `private_intra_doc_links` rule is not a convention row |
| F21 | Low | Invented content | Deferred "per-module default features" and R2 "nothing else breaking" were never decided; the Kalshi demo host in nightly needs credentials (see F1) |
| F22 | Low | CAP-10 / CLAUDE.md gzip | Merged builder (H6) is where `gzip` is most likely re-forced; the "unset = reqwest default" rule is unpinned |
| F23 | Low | AD-15 / D15 | The CAP-4 class of FAK/FOK kill outcomes is unstated; tagging them `venue-refusal` → `real` would file issues for non-faults |
| F24 | Nit | misc | frontmatter `sources` omits the architecture memlog; AD-2 attributes the gzip leak to dependency inclusion (it was a feature-unification change to wire behaviour, CAP-10); `api.rs` vs CLAUDE.md's `api/` directory; "one enum per module" is ambiguous for the socket tier |

## Part 1 — memlog decision coverage

| memlog line | Decision | Spine | Status |
|---|---|---|---|
| L10 | Inherited constraints | AD-2, AD-8, AD-10, AD-11, AD-16, conventions | **Partial.** Not restated: prader migrates only from crates.io; behaviour-carrying tests move, not weaken (F15); ids opaque or ≥u64; venue-only fields stay reachable |
| L11 | One crate per venue; multi-venue umbrella; Kalshi lands with no foundation edits | conventions, AD-16 | **Partial.** The Kalshi "no foundation edits, no copied infra" success signal is not restated. AD-13 would make the skeleton edit `nightly-schema.yml`, README and INDEX (F13) |
| L12 | Walkthrough → in-repo markdown guide referenced from CLAUDE.md | CAP-11 row: "the agent guide (rendering of this spine)" | **Partial.** Location and the CLAUDE.md reference are missing (F10) |
| L14 | Foundation split B | AD-1, AD-2, seed | Covered |
| L15 | Classification out of core | AD-3 | Covered |
| L17 | venue is the bottom crate; core→venue; **ws does not**; socket-only modules → venue | AD-3; AD-1 | **Distorted.** AD-1's prose arrow "core / ws → venue" implies ws→venue; "ws does not" is not stated (F7) |
| L18 | Open string-tagged identity | AD-4 | Covered |
| L19 | RPITIT + dynosaur, async-trait fallback | AD-5 | Covered. The fallback trigger is not checkable (F12) |
| L21 | Base trait + capability traits | AD-6 | Covered |
| L22 | Outcome keys | AD-7 | Covered |
| L23 | Streaming traits deferred | Deferred | Covered |
| L24 | One send loop, hook order | AD-8 | Covered. The signature inherits the gaps in F2 and F16 |
| L25 | Wait = max(backoff, Retry-After); policy lengthens | AD-9 | Covered. Inherits the 418 mis-model (F5) |
| L26 | One composed throttle per client | AD-10 | Covered (F19 nit) |
| L27 | Builder computes cost; refuse a cost that can never fit | AD-10 | Covered. Cost shape undecided (F2) |
| L29 | `Supervisor<P: Protocol>` | AD-11 | Covered |
| L30 | One Disconnected then one Reconnected; per-attempt auth; wall-clock pings | AD-11 | Covered, but conflicts with L31 for perps (F3). "never on a quiet tick" became "never *only* on a quiet tick" |
| L31 | Migration gate; Binance one supervisor per path; rotation = `max_connection_age`; paced replay hook; **perps Retry and SequenceRegressed are protocol events** | AD-11, AD-12 | **Partial.** The last clause is missing from the Protocol hook list (F3) |
| L32 | Sports and rtds outside the Supervisor | AD-11 | Covered |
| L33 | Registration = members + `[package.metadata.polyoxide]` | AD-13 | Covered |
| L34 | One publish-order script; three CI checks | AD-13 | Covered (F13 adds what CAP-8 also asked for) |
| L35 | Failure-tag reporter | AD-14 | Covered. The substance is unsafe (F1) |
| L36 | R1/R2/R3; **"error and builder types may change" in R1** | AD-16 | **Distorted.** The clause is dropped, and "nothing else breaking" is added to R2 (F6) |
| L37 | Merge when green; deliberate bump | AD-16 | Covered |
| L38–L39 | Names; layout | Conventions | Covered (F24 nit on `api/`) |
| L40 | Features; **clob `ws` → `clob-ws` in R2** | Conventions | **Partial.** The R2 placement of feature renames is not stated. It matters for DRIFT R6 (F6) |
| L41 | Dependency direction | AD-1 | Covered (see L17) |
| L42–L47 | Errors, records, Secret, umbrella, CLI/Python, tracing | AD-15, conventions | Covered |
| L48 | Deferred list | Deferred | Covered, plus one invented item (F21) |
| L50, L51 | **Assumptions:** test-target names; lowercase venue ids and product = module | Conventions rows | **Visible but unflagged** (F18) |

## Part 2 — Findings in detail

### F1 (High): AD-14's tag taxonomy loses CLAUDE.md's verdicts and retry behaviour

**Evidence**
- CLAUDE.md:398-405 defines four verdicts with distinct actions:
  - `auth-gated` (`POLYMARKET_* env vars required` panics): silently skipped.
  - `environmental`: logged and skipped.
  - `transient`: "retried up to 2× with `cargo nextest --retries 2`".
  - `real`: files an issue.
  - CLAUDE.md:405 tells agents how to turn on auth tests by editing `AUTH_GATED_RE`.
- `.github/scripts/classify_failures.py`:
  - `classify()` checks auth-gated first.
  - `_cmd_merge` promotes a transient that is still failing to `real`.
  - It writes `auth-gated.txt` and `environmental.txt`.
  - It covers 27 tests in `tests/test_classify_failures.py`, including `test_classify_auth_gated_takes_precedence_over_transient`, `test_every_retriable_arm_classifies_transient`, `test_a_dropped_websocket_is_transient` and `test_cli_merge_promotes_persistent_transients_to_real`.
- Spine AD-14: "prints `polyoxide-class=<network|rate-limited|unauthorized|invalid|venue-refusal|environmental>` … The classifier matches only these tags."

**Gaps**
1. **No `auth-gated` tag.** Missing credentials is a precondition failure, and no request is ever made. `unauthorized` is a 401 or 403 from the venue. If `unauthorized` maps to skip, a signing regression (L1/L2/order EIP-712, sigtype 3) becomes permanently silent. If it maps to `real`, the ~25 + 8 credential-less clob and relay tests file issues every night. Today's design keeps these apart, and the tags merge them.
2. **No class for retriable server faults.** CAP-4's "retriable" is a flag, not one of the tags. A 503, a 425, `ApiError::Timeout` or a WebSocket close code 1001/1011/1012/1013 is neither `network` nor `rate-limited`, so these land in `venue-refusal`, `invalid` or untagged, and all of those mean `real`.
3. **No tag→verdict table.** AD-14 does not say which tags are retried (`--retries 2`), which are skipped and which file issues. It also does not keep the `merge` promotion.
4. **Untagged failures become `real`.** Examples:
   - `.unwrap()` or `.expect()` on a venue error;
   - assertion text;
   - the test-level conventions that are not errors at all: `server ended the connection`, `legitimately time out` and `no qualifying market`.

   The classifier itself warns: "Misclassifying transient-as-real is the expensive direction: it files an issue immediately."
5. **Kalshi.** The seed's nightly hits a "Kalshi demo host". The Kalshi WebSocket needs API-key auth even for market data (spec memlog L22), so without secrets that row needs exactly the `auth-gated` class.

**Fix.** Rewrite AD-14 with an explicit table: tag → verdict → nightly action.
- Add `auth-gated`, emitted only by the test-support credential loaders when env or keychain credentials are absent. Keep it distinct from `unauthorized`, which is `real`.
- Add `transient`, or derive it from the CAP-4 `is_retriable()` flag (429, 425, 408, 5xx, network, WebSocket drop codes).
- Keep `environmental` as an explicit reporter call (`skip_environmental(reason)`) for world-state conditions.
- Keep `merge`'s promotion.
- Require the reporter to work without per-call discipline: a panic hook installed by a test-support attribute or by `#[ctor]`, or a tag carried in the error enums' `Debug`/`Display`.
- Keep the regexes as a fallback until every `tests/live_*.rs` is migrated, and add a CI check that no live test uses bare `unwrap()` on a venue call.
- Port the 27 classifier tests to the tag table, as behaviour-carrying tests that move rather than get deleted. Replace CLAUDE.md:405's `AUTH_GATED_RE` instruction.

### F2 (High): `acquire(cost)` cannot express two layers that count different things

**Evidence**
- CLAUDE.md:161-171: "Two rate limit layers, counting different things." Cloudflare is keyed on client IP and counts **requests**. The per-signer layer is keyed on signer and counts **orders**, and it charges batches their full size.
- CLAUDE.md:181: `quota()` "deliberately does not call `allow_burst`, leaving capacity at governor's default of one token."
- CLAUDE.md:185: `signer_limit.rs` "must keep its `allow_burst`". "Making the two modules 'consistent' would reintroduce the bug."
- Code:
  - `polyoxide-core/src/rate_limit.rs:403` `acquire(&self, path, method)` selects endpoint buckets by route and charges one token each.
  - `signer_limit.rs:365` `acquire(TradingRequest)` uses `until_n_ready(cost)`, where `InsufficientCapacity` means "can never fit".
  - `polyoxide-clob/src/request.rs:210-217` charges both per attempt, and `:262-267` observes the tier header on every status.
  - `polyoxide-binance/src/usdm/request.rs:94,106` has `let charge = self.budget.acquire(cost)` … `self.budget.record_used(charge, used)`. CLAUDE.md:276 says this is "only for the minute its request was charged in". The funding bucket is "450 per 5 minutes, depth one" and costs no weight.
- `signer_limit.rs:170` `cost_is_exact()`: cancel-all costs are not knowable client-side.
- Spine AD-8 diagram: `throttle.acquire(cost)` … `throttle.observe(response)`. AD-10: "Each request builder computes its request's cost." Neither AD-10 nor the conventions cite D1 or D2.

**Gaps**
1. A route-less `cost` cannot select the Cloudflare endpoint bucket.
2. A scalar cost forwarded to the composed throttle charges N to a depth-1 Cloudflare bucket. AD-10's own rule then refuses every batch order (N>1) as "a cost the bucket can never hold". The repair an agent will reach for is `allow_burst` on `quota()`, which is the 2x over-permit CLAUDE.md:181-183 documents.
3. `observe(response)` gets no receipt from `acquire`, so Binance cannot attribute used-weight to the charged minute.
4. A cost can be a lower bound (`cost_is_exact`).

**Fix.** Decide the cost shape in AD-10:
- **Option A:** a core-defined `RequestCost { method, path, units: SmallVec<(LayerId, u32)>, exact: bool }`. Each layer reads only its own units, and Cloudflare always charges 1.
- **Option B:** an associated `Cost` type. That makes `HttpClient` generic, which `Arc<DynThrottle>` resists.

`acquire` returns a `Charge` receipt that is passed to `observe`. Add a rule to AD-10:
- Composition keeps each layer's own model (D1, D2).
- A published window quota means depth 1 and a reserved tenth (`RESERVED_FRACTION`).
- A published capacity (signer, Kalshi `bucket_capacity`) means `allow_burst(capacity)`.

The Kalshi skeleton builds a capacity-published bucket, so CAP-11's guide needs this rule stated here.

### F3 (High): The Supervisor outage invariant contradicts the perps migration gate

**Evidence**
- Spine AD-11: "`Supervisor` emits exactly one `Disconnected` then one `Reconnected` per outage." AD-12: perps and Binance suites "move without changes to their assertions."
- divergences.md D8: "Perps has no `Disconnected`."
- `polyoxide-perps/src/ws/event.rs:103-126`: `Event` has only `Update`, `Unknown`, `Reconnected` and `SequenceRegressed`.
- The perps suite is 17 tests **inline** in `polyoxide-perps/src/ws/supervised.rs`, the file the migration deletes. Examples at `:697`, `:730` and `:756`: `assert!(matches!(next_event(&mut ws).await, Event::Reconnected))` immediately after updates. A `Disconnected` emitted first fails every one of them.
- Binance's invariant includes the edge "even when the path's last stream leaves mid-outage" (CLAUDE.md:306), which `tests/supervision_edges.rs` pins. Under "one supervisor per routed path", that path's supervisor may be dropped mid-outage.
- memlog L31: "perps Retry and SequenceRegressed are protocol events". It is absent from AD-11's Protocol hook list. `PerpsWsError::recovery` (`polyoxide-perps/src/ws/error.rs:131-137`) returns `Recovery::Retry` for `message_rate_limited`, which is neither reconnect nor fatal.

**Fix**
- State the invariant as: "for each outage the Supervisor yields at most one `Disconnected` (only where the venue's event enum has one) and exactly one `Reconnected`. A venue whose enum has a `Disconnected` must pair them, including when a routed path's membership empties mid-outage."
- Add Protocol hooks for venue-specific recovery (`Retry` without reconnect) and for protocol-originated events (`SequenceRegressed`).
- Name the suites the AD-12 gate covers and where they relocate:
  - perps `src/ws/supervised.rs` `#[cfg(test)]` (17);
  - binance `tests/supervision.rs` (21);
  - binance `tests/supervision_edges.rs`;
  - binance `usdm/ws/client.rs` inline (9).

### F4 (High): The loop's retry set is not the classifier

**Evidence**
- CLAUDE.md:157: `is_retriable()` "is the canonical classifier for callers' retry policies … The crates' *own* retry loop is narrower — `HttpClient::should_retry` only ever retries `429` and `425`."
- `polyoxide-core/src/client.rs:132`: `let retriable = status == TOO_MANY_REQUESTS || status == TOO_EARLY`.
- divergences.md D17: 425 "belongs to the Polymarket response policy, not the foundation."
- Spine AD-3 places "the retriable-status rule (408/425/429/5xx)" in `polyoxide-venue`, and the seed puts a "425 policy" in `polymarket/shared`. The spine never says which rule the loop uses.

**Gaps**
- An agent writing the default `RetryPolicy` will naturally reuse AD-3's rule. That retries 5xx and 408 on every route, including `POST /order` and `POST /orders`. Those are non-idempotent and could double-place orders.
- AD-3's "(…425…)" also contradicts D17 without a recorded decision. Binance today retries 425 through core's `should_retry`, so moving 425 into Polymarket's policy is an unlisted Binance behaviour change.

**Fix.** Add to AD-8 and AD-3:
- `is_retriable` (408/425/429/5xx/network) answers callers.
- The loop retries only what its venue's `RetryPolicy` names. The default is 429, Polymarket's adds 425, and no policy retries 5xx or 408.
- Record a D17 decision on whether 425 stays in the venue-neutral classification rule, with Binance's 425 behaviour stated.

### F5 (Med-High): AD-9 mis-models Binance's 418 and next-minute hold

**Evidence**
- `polyoxide-binance/src/usdm/request.rs:111-126`: on a 418, "starts a cooldown … and is **not retried**" and returns `Err`.
- `:132-151`: `let retry = should_retry(..); match (retry, asked) { (None, None) => self.budget.hold_until_next_minute(), _ => begin_cooldown(max(retry, asked)) }`. The client-wide cooldown depends on the retry decision.
- CLAUDE.md:278-280: "a `429` or `418` held as a client-wide cooldown … A `429` with no retry left and no `Retry-After` holds every request to the next minute."
- `polyoxide-core/src/client.rs:139-145` (`retry_delay`): "Shared by `should_retry` and `note_rate_limited` so a single response cannot produce one delay for the request that saw it and a different one for the client-wide cooldown it triggers."
- Spine AD-9: "A `RetryPolicy` may lengthen it (Binance's next-minute hold, the 418 ban)." AD-8 runs `observe` before `decide`. memlog L28 claims these were "checked against" the hooks.

**Gaps**
- Read literally, AD-9 makes a 418 a `Retry(ban)`, which parks a request for up to 3 days where today it fails fast.
- The no-retry-left hold is client-wide and occurs on `Fail`, where there is no retry wait to lengthen.
- `observe` cannot know whether a retry remains.
- The "one response, one delay" invariant is split across two hooks.

**Fix**
- 418 is `Fail`, and its ban is a throttle cooldown started in `observe`.
- Either pass `observe` the attempt and remaining-retries context, or add a post-decision hook (`throttle.after_decision(decision, delay)`) so the cooldown equals the retry wait, or the next-minute hold when no retry remains.
- Pin the "one response, one delay" rule in AD-9. Amend memlog L28.

### F6 (Med-High): AD-16 drops one memlog clause and adds another

**Evidence**
- memlog L36: "R1 internals (…; public paths unchanged, **error and builder types may change**) … Rule: every public rename or move ships in R2 and nowhere else."
- Spine AD-16 R1: "Public paths are unchanged." R2: "…and **nothing else breaking**."
- AD-15 (one classified enum per module), DRIFT R7 (relay's `Api(String)` → CAP-4) and H6 (merged builder knobs) all change public types.
- DRIFT R6 renames the `test-fixtures` feature, which is public, yet AD-16 puts "the DRIFT fixes" in R1.
- AD-10 says venue tables live in venue crates, and CAP-7 has a grep test. But `RateLimiter::clob_default()` and the other tables, `SignerLimiter`, `TradingRequest` and `Signer` are `pub` in `polyoxide_core`, so moving them is a path change, which is R2.
- "R1's release notes name DRIFT R1, R2 and R4" uses the same labels for stages and DRIFT rows.

**Fix**
- Restore "error and builder types may change" to R1, and delete "nothing else breaking" or replace it with a decided list.
- Assign DRIFT R6 to R2.
- State that Polymarket code leaves core in R2. In R1 the Polymarket hooks live in core or in the existing crates, and the grep test is enforced from R2.
- Rename the stages, for example S1, S2 and S3.
- Pin that the publish-order switch (AD-13) merges **before or with** the first foundation crate. Otherwise `release.yml`'s hand list (`.github/workflows/release.yml:83`) cannot publish `polyoxide-venue` and `polyoxide-ws`, and "main stays releasable" is false.

### F7 (Med): `polyoxide-ws` cannot reach the retriable-status rule

**Evidence**
- memlog L17: "polyoxide-core depends on it, **polyoxide-ws does not**". AD-2's list of allowed ws dependencies excludes `polyoxide-venue`, and spec memlog L43 lists rust_decimal as never allowed in the socket crate.
- Seed: `polyoxide-ws/src/ # … connect, classifier, Supervisor`.
- DRIFT R1: sockets retry 408/425/429/5xx "matching core's HTTP rule".
- AD-3: "No other crate defines any of these."
- AD-1 prose: "→ `polyoxide-core` / `polyoxide-ws` → `polyoxide-venue`".

**Fix.** Pick one:
- The Supervisor asks the `Protocol` to classify a handshake refusal, and the venue applies `polyoxide-venue`'s rule.
- Or move the status predicate into a dependency-free spot both can reach.

Edit AD-1 to say "`polyoxide-ws` depends on neither `polyoxide-core` nor `polyoxide-venue`."

### F8 (Med): README doctests disappear on consolidation

**Evidence**
- `#[cfg(doctest)] #[doc = include_str!("../README.md")]` appears in gamma, perps, core, clob, binance, relay, data and sports. `polyoxide/src/lib.rs:76,81` also includes the root README.
- Memory note doc-drift-classes: "README examples ARE doctests … scoped test commands … will not catch it."
- The spine says nothing about READMEs after the move.

**Gap.** Merging clob, gamma, data, relay, perps and sports into one crate leaves one `../README.md`. The other six README doctest suites stop compiling, and nothing fails when that happens.

**Fix.** Add a convention row:
- Each module keeps `src/<module>/README.md`, included as `#[cfg(all(doctest, feature = "<module>"))]`, and the crate README links to them.
- Add a CI count check: the number of README files equals the number of doctest includes.
- The root README stays included by the umbrella crate.

### F9 (Med): Feature boundaries are unchecked

**Evidence**
- `.github/workflows/ci.yml:39-41` runs only `--all-features --workspace`.
- CLAUDE.md on rustls: "clob only compiles without it because `reqwest`/`alloy` turn it on transitively."
- AD-2 asserts only `cargo tree … -F rtds` and `-F sports` contain neither `reqwest` nor `alloy`.

**Gap.** Separate crates are each built alone today. As features of one crate, a module that leans on another module's feature, on `src/shared/`, or on a unified dependency feature compiles in CI and fails for a consumer enabling only `gamma` or only `sports`.

**Fix.** CI runs `cargo hack check -p polyoxide-polymarket --each-feature --no-dev-deps`, and the same for binance. AD-2's cargo-tree check becomes an allowlist per credential-free module, not a two-crate denylist.

### F10 (Med): CLAUDE.md contradicts the spine and is loaded into every agent

**Evidence.** CLAUDE.md is a spine companion and the auto-loaded context of every loom agent (memlog L12: "parallel loom agents may build epics concurrently"). It states, among others:
- :69-70 rtds and sports "depends on NOTHING in-workspace" (spec memlog L43: using `polyoxide-ws` "amends" this);
- :267 Binance "deliberately not in the `polyoxide` umbrella crate or `full`";
- :440-442 the hand-written Publishing Order;
- :55 "Twelve crates" and the graph;
- :416 "each with its own `ensure_crypto_provider` copy" and the clob `ws` feature;
- :405 `AUTH_GATED_RE`;
- the Error hierarchy and `impl_api_error_conversions!` paragraph;
- the Python `polyoxide.v2` namespace.

registration-points.md lists CLAUDE.md three times, but AD-13 checks only the README table and INDEX.md. memlog L12's decision (an in-repo markdown guide referenced from CLAUDE.md) is not in the spine.

**Fix**
- Add a "Superseded CLAUDE.md rules" table: each rule, its replacing AD, and the stage at which CLAUDE.md changes.
- Require each stage's epic to update CLAUDE.md in the same change.
- Bring CLAUDE.md's crate graph and Publishing Order under AD-13: generate them, or replace them with a pointer to `publish_order.py`.
- Name the guide's path and its CLAUDE.md reference in the CAP-11 row.

### F11 (Med): Retry-signal details AD-8 leaves open

**Evidence**
- `polyoxide-data/examples/common/mod.rs:179-180` and `polyoxide-perps/examples/info_soak.rs:346` filter on `target().starts_with("polyoxide_core") && level == WARN` plus the text `Retriable status 429`.
- CLAUDE.md:197: "the retry loops log a `WARN` and return `Ok` … Detection goes through a `tracing` subscriber." CLAUDE.md:87: the CLI shows `warn`.
- core, clob and binance all `drop(permit)` before sleeping (e.g. `polyoxide-core/src/request.rs:166`).
- Binance logs non-retry WARNs: "418 … IP banned, every request held" and "no retry left: every request held".

**Fix.** Pin the following in AD-8:
- `tracing::warn!` with the text `Retriable status <status> on <path>, retry <n> after <ms>ms`.
- The permit is released before the sleep.
- Holds and bans that are not retries also log at WARN on `polyoxide_core`. That is outside the "one retry line" count, but required for visibility.

### F12 (Med): MSRV 1.91 is not gated

**Evidence**
- Every CI, release and nightly job uses `dtolnay/rust-toolchain@stable`, and `grep 1.91 .github/` finds nothing.
- The memory note polyoxide-ci-gates says local and CI stable differ.
- Spine AD-5: "If dynosaur fails the rustdoc or clippy gates on MSRV 1.91". Stack: "Rust (MSRV, edition) 1.91".

**Fix.** Add an MSRV job (`cargo +1.91 check --workspace --all-features` plus `cargo +1.91 doc`), or restate the AD-5 trigger against the gates that actually run. Note that the job also gates the release (F13).

### F13 (Med): AD-13 omits parts of CAP-8, and so does the CI flowchart

**Evidence**
- CAP-8 success: "CI fails when … a `tests/live_*.rs` lacks a nightly row". CAP-8 intent includes the "schema watch list and exclusions, classifier patterns".
- CLAUDE.md:403 describes nightly-schema: eight OpenAPI plus four AsyncAPI specs, `spec:<id>` labels found by label intersection, and `.drift-acknowledged.json` fingerprints, with exclusions copied three times.
- `ci.yml:66-74` has a "CI Scripts" pytest job, which the spine's CI subgraph omits. The flowchart has no nightly-schema at all.
- CLAUDE.md:46: a red CI silently withholds the release tag.

**Gaps**
- The Kalshi skeleton (OpenAPI-published, spec memlog L24) would have to edit `nightly-schema.yml`, which breaks the "only its own directory and the registration source" signal.
- README and INDEX are checked, not generated, so the skeleton edits them too.
- Moving or renaming `docs/specs/` ids orphans `spec:<id>` issues and acknowledgements.
- `publish_order.py` must ignore path-only dev-dependencies, or `polyoxide-test-support` creates a cycle, and it must include versioned dev-dependencies, which crates.io resolves.
- Every new registration or MSRV check also withholds releases when red.

**Fix**
- Derive nightly-schema's watch list and exclusions from metadata. Freeze spec ids, or migrate labels and fingerprints together.
- Add the "every `tests/live_*.rs` maps to a nightly row" check.
- State the dev-dependency rule for publish order.
- Add CI Scripts and nightly-schema to the flowchart.
- Restate the release-withhold consequence beside the CI subgraph.
- List the shared files the Kalshi skeleton may touch.

### F14 (Med): Moving the limiter tables needs an introspection seam

**Evidence**
- `polyoxide-core/src/rate_limit.rs:754` `assert_matches_published` calls `rl.resolve_specs(..)`, which is `#[cfg(test)] fn` (private, `:420`).
- CLAUDE.md:170 says the effective-quota tests exist because presence-only tests let `/balance-allowance` go missing and `/closed-positions` sit at 66x its cap.
- The seed lists "limit tables" under `polymarket/shared` and "cooldown" under core, but no window-quota engine (governor, `quota()`, `RESERVED_FRACTION`). The Stack still lists governor.

**Gap.** Tables in a venue crate cannot reach core's private resolver, and the easy fallback is a presence check. If the engine follows the tables into Polymarket, Kalshi (or any window-quota venue) must copy it.

**Fix**
- Place the window-quota engine in the seed (core, venue-neutral) and expose `effective_quota(method, path)` publicly, or behind a `test-server`-style feature.
- State that the `documented_*_limits` tests move with the tables and keep asserting effective quota.

### F15 (Med): The general "tests move, not weaken" constraint is missing

**Evidence**
- memlog L10 and SPEC Constraints cover mutation-tested rate-limit rules, supervision invariants, kill-outcome classification, effective-quota tests, spec and wire agreement, and live drift detectors.
- The spine covers only supervision (AD-12).

**Fix.** Add an AD or convention row that lists each class with its destination crate or module and states "assertions unchanged unless a DRIFT row says otherwise". Sequence it so the DRIFT changes are separate commits naming their row. DRIFT R1 changes rtds's handshake-`Http` → `Fatal` behaviour (`polyoxide-rtds/src/error.rs:140-149`) and its tests.

### F16 (Low-Med): Authenticator inputs

**Evidence**
- clob signs `timestamp + method + path [+ body]` per attempt (`polyoxide-clob/src/request.rs:251`).
- Kalshi signs `timestamp_ms + METHOD + path-without-query` (spec memlog L21).
- Binance signed routes put `timestamp` and `signature` in the **query**.
- The spine's diagram has `authenticator.headers(attempt)`.

**Fix.** Give the hook the request parts (method, path, query, body, attempt) and let it add query parameters. Otherwise record that Binance signed routes will need a foundation edit, which conflicts with "no foundation edits" for the next venue that signs its query string.

### F17 (Low-Med): `test-server` exports versus dev-only test-support

**Evidence.** CLAUDE.md:83-85: `polyoxide-cli/tests/ws_binance.rs` uses `polyoxide_binance::usdm::ws::fixtures` (feature `test-server`). Perps' scripted server is "exposed by the `test-server` feature for downstream use". CAP-9 moves fixture loading to `polyoxide-test-support`, and AD-1 says that crate is only a path dev-dependency.

**Fix.** Pin that anything a published crate exposes behind `test-server` is built only from published crates (`polyoxide-ws`'s server and venue-local fixtures), never from `polyoxide-test-support`.

### F18 (Low): Assumptions shown as conventions

**Evidence.** memlog L50 and L51 are tagged `(assumption)`, yet the spine's "Test targets" and "Keys" rows present them as decided. Existing targets that the pattern does not fit:
- `live_session_keys`, which has its own 40-minute nightly row;
- `v2_spec_agreement` and `v2_wire_agreement`;
- `supervision` and `supervision_edges`;
- `mock_api`;
- the CLI's `ws_*` and `data_v2`.

CLAUDE.md:388 says live tests are `#[ignore]`d so CI skips them, and nightly runs `--run-ignored only`.

**Fix**
- Mark both rows "(assumption, memlog L50/L51)".
- Extend the pattern with a suffix slot (`live_<module>[_<suite>]`) and cover `mock_<module>` and `supervision_<module>[_edges]`.
- Add "every test in `tests/live_*.rs` is `#[ignore]`", with a CI check, since a missed `#[ignore]` hits live hosts on every push.

### F19 (Low): `Arc<dyn Throttle>`

AD-5 makes `Throttle` an RPITIT trait, which is not dyn-compatible, so a holder needs dynosaur's wrapper (`Arc<DynThrottle<'static>>`). AD-10 should name the wrapper.

### F20 (Low): rustdoc private links

CLAUDE.md:44 forbids a `pub` doc comment linking a `pub(crate)` item, which is an error under `-D warnings`. The `src/shared/` convention creates `pub(crate)` items that today are `pub` in core (`SignerLimiter`, `Signer`), and existing doc links point at them. Add a convention row restating the rule, and note that a red doc build withholds the release.

### F21 (Low): Invented or unsupported content

- Deferred: "Per-module default features beyond Polymarket's" was never decided in the memlog. Keep it only if it is recorded.
- AD-16's "nothing else breaking" (F6).
- The seed's "Kalshi demo host" in nightly implies Kalshi secrets in CI, which was never decided (F1).
- Everything else that is not literally in the memlog traces to SPEC or brownfield facts. That includes the Stack versions, which match `Cargo.toml`, the AD-2 cargo-tree check (CAP-3), AD-8's "health pings included" (DRIFT R8), and AD-15's "429 variants carry the parsed Retry-After" (CAP-4).

### F22 (Low): gzip default

CLAUDE.md:288: "`HttpClientBuilder::gzip` is unset by default, which leaves reqwest's default". 0.37.0 forced it off and regressed prader-rs. H6 merges six builders, which is the likeliest place for that regression to come back. Add to the CAP-10 row: the builder leaves `gzip` unset, plus a per-client `Accept-Encoding` header test (CAP-10's success criterion).

### F23 (Low): The class of kill outcomes

CLAUDE.md:159 says `FakUnmatched` and `FokUnfilled` "are deterministic and never retriable", and that they are not faults. AD-15 and AD-14 do not say which CAP-4 class they get. If they are tagged `venue-refusal` and that maps to `real`, the FAK/FOK live tests file issues. State their class (for example `venue-refusal` with `is_fault() == false`) and how the reporter treats it.

### F24 (Nits)

- Frontmatter `sources` lists the spec memlog but not `./.memlog.md`.
- AD-2's "class of the 0.37.0 gzip leak": that was feature unification changing wire behaviour (CAP-10), not a dependency entering a tree.
- The layout row says `api.rs`, while CLAUDE.md's Module Organization uses an `api/` directory with one file per namespace. Allow either.
- "One error enum per module" does not say whether the socket tier (`PerpsWsError`, `UsdmWsError`) merges into the module enum.

## Part 3 — CLAUDE.md load-bearing rules: preserved or at risk

| CLAUDE.md rule | Spine | Status |
|---|---|---|
| `note_rate_limited` before `should_retry`, unconditionally | AD-8 (observe before decide, on the last attempt) | Preserved |
| Retry-After only extends | AD-9 | Preserved |
| Cooldowns extend, never truncate; `await_cooldown` re-checks | AD-9 | Preserved |
| One response, one delay (retry wait = cooldown) | — | **At risk** (F5) |
| `quota()` without `allow_burst`; `signer_limit` keeps it (D1) | not cited | **At risk** (F2) |
| `RESERVED_FRACTION` (D2) | not cited | **At risk** (F2, F14) |
| Two layers count requests vs orders | AD-10 "composes" | **At risk** (F2) |
| Loop retries only 429/425 | — | **At risk** (F4) |
| Effective-quota agreement tests | — | **At risk** (F14) |
| 429 retried away is invisible; tracing detection | AD-8 (target, text) | Partial: level unpinned (F11) |
| Binance 418 / no-retry-left hold / charge-minute attribution | AD-9 (mis-modelled) | **At risk** (F2, F5) |
| rtds pump shape not copied; wall-clock pings | AD-11 | Preserved |
| Sports pings count as liveness; Stream shape | AD-11 (outside Supervisor) | Preserved |
| Perps keep-alive on the wall clock; pong is liveness | AD-11 liveness hook | Preserved |
| Binance Disconnected→Reconnected invariant (incl. path emptied mid-outage) | AD-11 | Partial (F3) |
| Perps has no Disconnected (D8) | AD-11 contradicts | **Broken** (F3) |
| rustdoc `-D warnings`; no pub links to `pub(crate)` | CI flowchart only | Partial (F20) |
| Red CI silently withholds the release | — | Missing (F13) |
| README examples are doctests | — | **At risk** (F8) |
| Live tests `#[ignore]` | — | Missing (F18) |
| mockito conventions (`tests/mock_api.rs`, `expect(0)` traps) | — | Not addressed; low risk if `mock_api` files move verbatim (F18) |
| Publishing order section | AD-13 script | Superseded, not recorded (F10) |
| Nightly verdicts auth-gated / environmental / transient / real | AD-14 | **Broken** (F1) |
| nightly-schema label intersection, acknowledgements, exclusions | — | Missing (F13) |
| `ensure_crypto_provider` before `connect_async`; rustls `std` | seed (TLS in ws), DRIFT R5/R9 | Preserved |
| clap `Vec<String>` needs `value_delimiter` | — | Not addressed. The R2 CLI rewrite re-creates every list flag; worth one line in the CLI convention row ("every list flag has a parse test that passes it") |
| CLI installs a WARN subscriber; libraries never do | Tracing row | Preserved |
| gzip unset = reqwest default | — | Missing (F22) |
| FAK/FOK kill outcomes are not faults | — | Unstated (F23) |
