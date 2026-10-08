---
review: update rubric (scoped to the amended rules)
target: ../ARCHITECTURE-SPINE.md (25 ADs; AD-10, AD-13, AD-15, AD-16, AD-22, AD-25 amended in place)
inputs:
  - ../.memlog.md lines 126-136 (four pending questions, the update run, five amendment decisions)
  - ../ARCHITECTURE-GUIDE.md
  - ../../../epics.md (70 stories)
  - ../../../../specs/spec-venue-extensibility/SPEC.md
  - workspace at e3d8c3e: polyoxide-core/src, polyoxide-{perps,rtds,sports}/src/**/error.rs, polyoxide-binance/src/usdm/ws/error.rs, Cargo.toml, .gitignore
date: 2026-10-08
---

# Update rubric: the six amended rules

## Verdict

**Not ready.** The spine reflects every amendment the memlog records, and adds nothing the memlog
lacks. But two amended rules do not yet do their job:

- **The CAP-7 gate is too narrow (AD-16).** The narrowed grep no longer proves "no Polymarket
  identifiers in the foundation". The `clob_default`, `gamma_default`, `PERPS_GENERAL` and
  `Poly-RateLimit-Tier` identifiers in today's core would all pass it.
- **The secrets are declared twice (AD-13).** The new `live.<target>.secrets` list is a second
  declaration of names the credential loaders already take from the test (AD-14), and nothing
  checks that the two agree. A mismatch produces exactly the silent skip that tags were brought in
  to end.

Three more problems are medium:

- **The socket transport table is incomplete and has no owner (AD-15).**
- **The authenticated bucket sizing has no mechanism (AD-10).**
- **The new-venue touch set still cannot be checked as written (AD-13).**

Separately, the SPEC, the guide and epics.md disagree with the amended rules in eight places.

The AD-22 and AD-25 amendments are sound.

## 1. Memlog fidelity

| Memlog | Decision | Spine | Status |
| --- | --- | --- | --- |
| 131 | `live.<target>.secrets`; generated nightly secret wiring; touch set adds `scripts/capture_<venue>_*.py` | AD-13 L296, L307, L322 | faithful |
| 132 | CAP-7 gate: venue ids as whole identifiers, `venue.product:` prefixes, never bare product ids, exception list | AD-16 L428 | faithful (the rule itself is the problem, F1) |
| 133 | removal gate before any removing story; five-crate limit counts crate names absent from crates.io | AD-22 L509; AD-25 L543 | faithful |
| 134 | `Url`/`HttpFormat`/`AttackAttempt`/non-EOF `Tls` → `InvalidRequest`, never retried; TLS EOF → `Network` | AD-15 L380-382 | faithful |
| 134 | authenticated-endpoint sizing; unauthenticated fallback provisional in `OBSERVED.md` | AD-10 L231 | faithful; AD-10's **Prevents** does not add "copied, unmeasured limits" (memlog 134's stated divergence) |
| 135 | generated regions add CLAUDE.md's dependency graph, nightly list, schema exclusions, and the secret wiring; guide gaps become a spine amendment with `docs/ARCHITECTURE.md` regenerated | AD-13 L303-309, L323 | faithful |
| 136 | guide recipe step 8 and the live-test secrets text synced | guide L193-200, L169 | **partial**: step 8 gained the capture scripts but not the spine amendment or `docs/ARCHITECTURE.md`; step 3 was not synced with AD-10 (F5) |

Nothing was dropped, and nothing was invented. The spine's frontmatter `spec_amendments` lists
four spec amendments. The update changes two more spec-facing facts, the success-signal touch list
and the CAP-7 grep scope, and neither is listed or offered (F5).

## 2. Each amended rule: enforceable, and does it prevent its divergence?

| AD | Amendment | Enforceable? | Prevents the stated divergence? |
| --- | --- | --- | --- |
| AD-10 | size from an authenticated limits endpoint; provisional fallback | partly. A mock test can pin "adopts `/account/limits`", but the mechanism is unspecified | no. See F4: two stories can build incompatible halves |
| AD-13 | `secrets` in live metadata; generated secret wiring | yes, for the wiring | no. See F2: the declaration can disagree with what the test loads |
| AD-13 | wider generated regions | yes, except the marker syntax, which is invalid YAML for the workflow region (F6) | yes |
| AD-13 | touch set adds capture scripts and a spine amendment with the guide regenerated | no. "A spine amendment" names no paths, and "regenerated" cannot be told apart from a hand edit (F6) | partly. The capture-script gap is closed, but others remain (F6) |
| AD-15 | socket transport errors → `InvalidRequest` / `Network` | yes for the four named variants | partly. See F3: an unlisted variant that perps and rtds pin as fatal, and no owner for "never retried" |
| AD-16 | CAP-7 grep: whole-identifier venue ids, key prefixes, no product ids | yes | **no**. It fixes the `events` false positive by giving up the true positives (F1) |
| AD-22 | removal gate lands before any removing story | yes. Story 1.7 is in Epic 1, which merges first, and no Epic 1 or Epic 2 story removes a public Rust item | yes |
| AD-25 | five-crate limit counts crate names absent from crates.io | yes | yes. One ambiguity remains (L1) |

## 3. Findings

### F1 — high — AD-16's CAP-7 gate no longer finds Polymarket code in the foundation

The amended gate greps venue ids "as a whole identifier" and `venue.product:` prefixes, and never
bare product ids. Measured on today's `polyoxide-core/src`:

- every `polymarket` occurrence is a whole word, but S1 puts the hooks in a `polymarket` module
  whose types (`PolymarketRetryPolicy`, `polymarket_tier`, …) are compound identifiers.
  `\bpolymarket\b` does not match inside them;
- the actual Polymarket residue in core is product-shaped: `clob_default` (26), `gamma_default`
  (8), `perps_default` (6), `PERPS_GENERAL` (3) and the `Poly-RateLimit-*` headers. None of these
  contains a venue id or a key prefix, so all of them pass.

So the gate now proves only "no bare word `polymarket`, `binance` or `kalshi`". SPEC CAP-7 success
(SPEC L67, "no Polymarket identifiers") and epics FR7 (L56, "no venue identifiers") promise more
than that.

There was no need to narrow the venue-id half: no venue id is an English word. The false positives
came only from generic product ids such as `events` and `data`.

**Fix:**
- grep venue ids case-insensitively as substrings, or as identifier segments split on `_` and on
  case changes;
- grep product ids as identifier segments, skipping a checked-in list of generic product ids
  (`data`, `events`);
- keep the exception list for prose and doc examples.

Alternatively, if the narrow gate is intended, restate SPEC CAP-7 and FR7 to match it.

### F2 — high — Declared secrets and loaded env names can disagree, and a mismatch is a silent skip

AD-13 L296 makes `live.<target>.secrets` the source of the nightly wiring. AD-14 L333 and Story
2.3 (epics L478) have the credential loaders take their env names from the test. Nothing ties the
two together.

Suppose a test loads `KALSHI_DEMO_KEY_ID` while its metadata declares `KALSHI_DEMO_API_KEY`. The
workflow never exports the name the test reads, so the loader prints `auth-gated`, and AD-14's
table skips the test silently, every night. The amendment exists so that Kalshi's signed live tests
(Stories 8.2 and 8.4) actually run, and this mismatch defeats it without any signal.

**Fix:** add to AD-13's "CI fails when" list: a credential loader in a live target names an env
var that is not in that target's declared `secrets`. Two ways to implement it:
- the loaders accept only names from a per-target constant generated from metadata;
- or `scripts/live_unwraps.py`'s sibling greps the loader calls.

Also state that `secrets` holds exact names, not `<VENUE>_<ENV>_*` globs: Actions cannot glob
secrets.

### F3 — medium — AD-15's socket transport table omits a variant two crates pin as fatal, and "never retried" has no owner

**The variant.** Today's fatal sets are:
- perps (`polyoxide-perps/src/ws/error.rs:122-126`): `Url | Tls | HttpFormat | AlreadyClosed | AttackAttempt`;
- rtds (`polyoxide-rtds/src/error.rs:142-149`): the same set plus `Http`.

rtds pins `AlreadyClosed` as fatal in `a_permanent_transport_failure_is_fatal_not_a_retry_loop`
(L198), which is a protected suite under AD-12. Binance (`usdm/ws/error.rs:131`) and sports
(`error.rs:91-95`) let it fall through to reconnect. AD-15 names neither the variant nor the
divergence, so Stories 2.2 and 4.3 must pick, and either choice breaks one crate's "behaviour
unchanged" suite.

**The rest of the table.** It also leaves these variants unassigned: non-EOF `Io` (rustls reports
certificate rejection as `Io`, per sports' comment), `Protocol`, `Capacity`, `Utf8`, and close
codes outside 1001/1011/1012/1013.

**The wording.** "TLS EOF" is an `Io(UnexpectedEof)`, not a `Tls` variant, so "non-EOF `Tls`"
reads as if the EOF case lived inside `Tls`. In practice "non-EOF `Tls`" means every `Tls` error.

**The owner.** "Never retried" has none. AD-11 L259 gives the `Supervisor` only the injected
status rule, and `polyoxide-ws` cannot see `Class`. That leaves three choices:
- Story 4.5 hard-codes the four variants in `polyoxide-ws`, which is a second copy of the table;
- or each `Protocol` decides, which is the per-venue divergence the amendment meant to stop;
- or sports (Story 4.8) keeps its own `retrying_can_fix`.

**Fix:**
- name `AlreadyClosed` as `InvalidRequest` (the perps/rtds behaviour; a DRIFT row for Binance and sports);
- map every other `Io` to `Network`;
- say that the Supervisor's reconnect decision for transport errors is an injected predicate,
  like the status rule, and that every caller passes the predicate generated by
  `impl_ws_classification!`.

### F4 — medium — AD-10's authenticated sizing has no mechanism, and Story 3.3 may not leave room for one

AD-10 L231 says a venue "sizes its buckets from that endpoint". It does not say:

- whether that happens at build time (an async build, or a lazy first fetch) or as a runtime
  resize of a live bucket;
- how the `/account/limits` request is itself throttled before any sizes are known;
- which layer model the unauthenticated fallback uses. Story 8.2 (epics L1661) takes "the published
  Basic tier", which is a per-second rate. Under AD-10's own split, that makes it a window quota
  (depth 1, a tenth reserved), but `/account/limits` returns a bucket capacity (memlog 61), which
  is a capacity bucket. The two models behave differently.

Story 3.3 (epics L694) specifies core's capacity bucket as "capacity, refill, refusal". It has no
runtime-sized or resizable constructor, and its CAP-2 test builds fixed sizes. If 3.3 ships
const-sized buckets, Story 8.2 has to edit core, which breaks the "no foundation edits" success
signal.

**Fix:** state in AD-10 that the capacity bucket takes runtime sizes. Pick one of two shapes:
- the venue's composed throttle may swap a layer's bucket while keeping the shared hold;
- or the client is built after an authenticated fetch made with fallback sizes.

Also state that the provisional fallback uses the model of what the venue publishes, and add
"copied, unmeasured limits" to AD-10's **Prevents**. Add a matching AC to Story 3.3.

### F5 — medium — The SPEC, the guide and epics.md disagree with the amended rules

Every place found:

| Where | Says | Amended rule |
| --- | --- | --- |
| SPEC L137-142, success-signal touch list | members and pin, `Cargo.lock`, crate dir, `docs/specs/kalshi/`, mirrors entry, generated regions, spine amendment | AD-13 L322-323 also allows `scripts/capture_<venue>_*.py` and `docs/ARCHITECTURE.md` regenerated |
| SPEC L67, CAP-7 success | "no Polymarket identifiers in the venue-neutral foundation" | AD-16 L428 checks only venue ids and key prefixes (F1) |
| spine frontmatter L12 `spec_amendments` | four entries | lacks the touch-list and CAP-7 scope changes, and the update offered no spec update |
| guide L193-200, recipe step 8 "Touch nothing outside these" | no spine amendment, no `docs/ARCHITECTURE.md` | AD-13 L323. Step 8 also contradicts the guide's own L176 instruction to record gaps as spine amendments |
| guide L182, recipe step 3 | "never copy [limits] from the venue's published docs" | AD-10 L231 sizes from the venue's endpoint and allows a published-tier fallback marked provisional; Story 8.2 (L1661) uses the published Basic tier |
| guide L108, `InvalidRequest` row; spine AD-15 L375 | "a client-side refusal (`Refused`, an undeclared extension, local validation)" | AD-15 L382 adds transport variants. `AttackAttempt` is raised by server bytes during the handshake, so the definition should list them |
| epics L1617, Epic 8 intro | "the **pending** AD-13 amendment" | the amendment has now been applied |
| epics L56, FR7 | "no venue identifiers in the foundation crates" | narrower (F1) |

These rows agree with the amended rules:
- Story 1.4 (L320) and Story 1.5 (L337-341) on the generated regions and secrets;
- Story 1.7 (L394) on the gate timing;
- Story 4.3 (L963-969) on the socket table;
- Story 5.2 (L1190) on the gate;
- Story 8.1 (L1640) and Story 8.2 (L1660-1661).

### F6 — medium — The new-venue touch set still cannot be checked mechanically

- **"A spine amendment" names no path.** The spine lives at
  `_bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/` (not
  gitignored). An amendment through `bmad-architecture` writes `ARCHITECTURE-SPINE.md`,
  `.memlog.md`, `reviews/*.md` and the run-folder `ARCHITECTURE-GUIDE.md`. Story 8.1's touch-list
  check needs those paths.
- **Story 8.5 contradicts itself.** It requires a `bmad-spec` record of the success signal
  (L1736), which writes `_bmad-output/specs/spec-venue-extensibility/{SPEC.md,.memlog.md}`. Yet the
  same story asserts that the combined diff stays inside the AD-13 set (L1732).
- **"Regenerated from the spine" cannot be checked.** No script renders the guide (`gen_registry.py`
  does not), so a hand edit to `docs/ARCHITECTURE.md` passes.
- **Third-party pins.** The workspace pins every third-party dependency in
  `[workspace.dependencies]`. Kalshi needs an RSA-PSS crate that is not pinned today, and AD-13
  allows only "its" pin. AD-2's per-module allowlist file has no stated location; if it is a central
  file, a Kalshi module entry is outside the set too.
- **Marker syntax.** AD-13's markers are `<!-- generated:begin <id> -->`, which is not valid YAML,
  and the amendment puts a generated region in `nightly-behavioral.yml`. Stories 1.4 and 1.5 must
  each invent a comment form.

**Fix:**
- list the spine-amendment paths, and allow the spec record or move it out of Epic 8's PRs;
- either add a guide renderer, or state that a reviewer confirms the regeneration;
- allow a venue to add third-party `[workspace.dependencies]` lines, or require it to declare them
  in its own manifest;
- place allowlists in the crate directory;
- define the marker per file type (`# generated:begin <id>` in YAML).

## 4. Lower findings (not in the top six)

- **L1 — low.** The five-crate count is ambiguous about non-published crates. Story 1.2 (L278)
  fails the run "if more than five crate names are absent from crates.io", which does not say
  whether `publish = false` members count (`polyoxide-py`, `polyoxide-test-support`, later
  `polyoxide-kalshi`). Counted, S1 sits at exactly five (venue, ws, cli, test-support, py). AD-25
  says "adds", which implies publishable only. Story 4.11 (L1152) counts three. The Operations
  bullet (L179) still says "at most five new crates". Say "publishable crate names". Also consider
  checking the count on the PR rather than failing the release run after the bump is on `main`.
- **L2 — low.** The generated secret region in one workflow file presumably exports every venue's
  secrets to every matrix row. Nothing says whether rows get only their declared secrets.
- **L3 — low.** "Until a soak confirms it" (AD-10) does not say which host. The Kalshi skeleton
  runs against the demo host, whose limits need not match production's.
- **L4 — low.** The guide's superseded-rules row (L218) says "publishing order and crate graph",
  while AD-21 and the amended AD-13 also generate the nightly lists and schema exclusions. This
  predates the update, but the guide sync could have caught it.

## 5. Checked and consistent

- **AD-22 and AD-16.** S1's epic order omits the removal gate, but the order does not conflict with
  "before any removing story". No Epic 1 or Epic 2 story removes a public Rust item. Story 2.10's
  removal of Binance's hand-rolled client is in a Python capture script.
- **AD-13 and AD-21.** "Only the generator writes generated regions" agrees with the amendment;
  `docs/ARCHITECTURE.md` is not a generated region, so the single-writer claim refers to the
  spine.
- **AD-13 and the S2 schema freeze.** Adding `secrets` to the `live.<target>` schema before S1
  implementation does not breach "S2 moves entries, never the schema".
- **AD-15 and AD-14.** Network → `transient` covers CLAUDE.md's dropped-socket list (resets, TLS
  EOF, 1001/1011/1012/1013). `InvalidRequest` → `real` matches today's fatal treatment.
- **AD-15 and today's code.** The four named variants are exactly the shared core of today's fatal
  sets in perps, Binance, rtds and sports, so the amendment preserves behaviour for those.
- **AD-25 and Story 1.2.** The publish list stays per (crate, version), while the cap counts names.
