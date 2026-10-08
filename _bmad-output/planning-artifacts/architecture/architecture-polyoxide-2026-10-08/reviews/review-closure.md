---
review: closure audit
target: ../ARCHITECTURE-SPINE.md (25 ADs, revised after the reviewer gate)
inputs:
  - ../.memlog.md (latest entries supersede earlier ones; user decisions at lines 78, 80, 81, 105-108)
  - review-reconcile-spec.md, review-reconcile-memlog-claude.md, review-rubric.md, review-verify-current.md, review-adversary.md, review-staging.md
  - workspace at e3d8c3e (polyoxide-core/src/{client,request}.rs, polyoxide-binance/src/{usdm/request,weight,error}.rs, every suite AD-12 counts, Cargo manifests, .github)
  - dynosaur 0.3.1 / dynosaur_derive 0.3.1 sources and a scratch crate compiled on rustc 1.95.0
  - cargo-semver-checks docs (Context7, /obi1kenobi/cargo-semver-checks)
date: 2026-10-08
---

# Closure audit: revised spine against the six reviews

## Verdict

**Close, not yet ready.** The revision closes 100 of the 110 prior findings, and closes them
in a way a builder reading only the spine would act on. The kernel seams the reviews
flagged (hook shapes, holds, cost, membership, classification classes, tags, custody,
registration, release mechanics) are now pinned, and the ratified HTTP behaviour in AD-9 matches
the code exactly.

Eight prior findings stay open, two of them medium. The revision also introduces or exposes
fifteen new problems. Three are high:

- **AD-5's `'static` supertrait does not compile with dynosaur 0.3.1.** This was verified.
  Every dyn-held trait fails, and AD-5's own fallback clause would then switch the workspace to
  `async-trait` for no reason.
- **The trait error type is undecided.** The held form `Arc<Dyn<Trait><'static>>` cannot be
  built for a trait with an associated error. AD-15 forbids the catch-all that would make it
  concrete.
- **The S2 tombstones break CI.** A `compile_error!` crate in the workspace fails every
  workspace build and AD-22's own `cargo publish --workspace --dry-run`. That withholds the very
  release that must carry it.

## Counts

| Review | Findings | CLOSED | DEFERRED | REJECTED-BY-DECISION | OPEN |
|---|---|---|---|---|---|
| reconcile-spec (F1-F16, L1-L6) | 22 | 20 | 0 | 0 | 2 |
| reconcile-memlog-claude (F1-F24) | 24 | 23 | 0 | 0 | 1 |
| rubric (RB-01-RB-31) | 31 | 28 | 0 | 0 | 3 |
| verify-current (F1-F6) | 6 | 5 | 1 | 0 | 0 |
| adversary (P1-P15) | 15 | 14 | 0 | 0 | 1 |
| staging (F1-F12) | 12 | 10 | 0 | 1 | 1 |
| **Total** | **110** | **100** | **1** | **1** | **8** |

New problems: 15 (3 high, 5 medium, 7 low).

Status rules used:
- **CLOSED** means a builder reading only the spine now acts correctly.
- Where a finding's concern is settled but its proposed fix was replaced by a later user
  decision, the row says CLOSED and names the decision. REJECTED-BY-DECISION is kept for a
  finding whose substance the user chose against.
- IDs are prefixed by review, because every review numbered its findings from F1:
  S = reconcile-spec, M = reconcile-memlog-claude, RB = rubric, V = verify-current,
  P = adversary, ST = staging.

---

## 1. Prior findings → status

### reconcile-spec

| ID | Status | Evidence |
|---|---|---|
| S-F1 | CLOSED | AD-5 Binds now names `Authenticator`. AD-8 pins `RequestMeta { method, path, query, costs }`, `Result<Charge, Refused>`, `observe(&Charge, &ResponseMeta, &AttemptInfo { attempt, retries_left })` and `sign(&mut RequestParts { …, headers, body }, attempt)`. AD-9 keeps Binance's last-attempt hold; AD-10 makes the capacity bucket public. Residual: the async-ness of `sign` is unstated (N9). |
| S-F2 | CLOSED | AD-11: "at most one `Disconnected` (only for a declaring `Protocol`) and exactly one `Reconnected`". |
| S-F3 | CLOSED | Option (b) chosen. AD-1: "`polyoxide-ws` depends on neither". AD-11 classifies "through the injected `polyoxide-venue` status rule". AD-3/AD-15 add `impl_ws_classification!`. |
| S-F4 | CLOSED | AD-19 (typed `Extensions`, opaque `Arc<str>` order, trade and fill ids) and AD-20 (`Ok(Killed { reason })`). |
| S-F5 | CLOSED | AD-18 (transport ownership, two-build header pin) and AD-22 (the check is a CI job). The sub-point "every builder sets `Accept-Encoding` explicitly" was superseded by memlog 66: gzip stays unset, so reqwest's default applies (CLAUDE.md 0.37.1). |
| S-F6 | CLOSED | AD-12 is now general custody, with an open list. |
| S-F7 | CLOSED | AD-16: stages renamed S1/S2/S3; Polymarket leaves core in S2; the CAP-7 grep gate starts in S2; S2 is atomic through a loom integration session. "Paths unchanged in R1" was superseded by memlog 107 (S1 breaks consolidated paths under a removal list). |
| S-F8 | CLOSED | AD-13 (generated regions, `[workspace.metadata.polyoxide.mirrors]`, a new-venue touch list), AD-14 (`auth-gated`, `environmental(reason)`), AD-15 (`Restricted`), and the Deferred list (Kalshi in the umbrella and CLI). |
| S-F9 | CLOSED | AD-2: per-module allowlist, two deny-lists, `src/shared/` feature-gated. The widening is recorded in frontmatter `spec_amendments`. |
| S-F10 | CLOSED | The shared-code homes table places every inventory row. AD-8: `Fail` returns core's `ApiError`. H5 is "window quotas only". |
| S-F11 | CLOSED | AD-11: decode and liveness see Text, Binary, Ping, Pong and Close; "venue-specific recovery"; "No drops". |
| S-F12 | CLOSED | AD-3: "answers callers only". AD-17: core retries 429, Polymarket 429 and 425, no policy retries 5xx or 408. The grep gate is in AD-16. |
| S-F13 | CLOSED | The Secrets row: a `StoredCredential` trait, and `polyoxide <venue> credentials <store\|show\|delete> <kind>`. |
| S-F14 | CLOSED | The drift-detector convention row, and AD-13 "What stays fixed" (spec ids and labels). |
| S-F15 | CLOSED | Frontmatter `companions` now lists venue-landscape, registration-points and glossary. |
| S-F16 | CLOSED | `spec_amendments: CAP-5 trait organisation (AD-6)`. |
| S-L1 | CLOSED | AD-3: "whose clamp is a parameter". AD-23: the hold ceiling is per throttle. |
| S-L2 | CLOSED | AD-8 fixes `WARN`. AD-16 release notes name the moved log targets. |
| S-L3 | **OPEN (high)** | Nothing binds trait methods' error types to `Classify` (only `or_fail`'s `E: Classify`, AD-14). See N3. |
| S-L4 | CLOSED | AD-18: "keep each client's default concurrency". Homes H6. |
| S-L5 | CLOSED | AD-5: "`Protocol` is generic-only". |
| S-L6 | **OPEN (low)** | Memlog 63's "S1 checks the crates.io token can publish new crates" is not in the spine. staging Q5 shows `publish-new` works (polyoxide-binance, 2026-10-07). The token's crate scope and expiry are still unread. |

### reconcile-memlog-claude

| ID | Status | Evidence |
|---|---|---|
| M-F1 | CLOSED | AD-14's tag → action table: `auth-gated`; `transient` with `--retries 2` and `merge` promotion; precedence; regex sunset; the 27 tests move. Residual: no reporter for the "server ended the connection" convention (N8). |
| M-F2 | CLOSED | AD-8/AD-10: `costs: &[Cost]`, per-layer charging, `Charge` passed to `observe`, and the window and capacity models stated (D1, D2). |
| M-F3 | CLOSED | AD-11: declared markers and "venue-specific recovery and protocol-originated events". AD-12 names the suites with counts. |
| M-F4 | CLOSED | AD-3 and AD-17. The finding's claim that "Binance today retries 425 through core's `should_retry`" is wrong: Binance calls `should_retry` only inside its 429 arm (`usdm/request.rs:127`). The spine's "Binance unchanged" is right. |
| M-F5 | CLOSED | AD-9: 418 is `Fail` with a hold; `observe` gets `retries_left`; `decide` returns the hold; "one response, one delay" (one `Decision`). |
| M-F6 | **OPEN (medium)** | The stage rename, R6 in S2 and Polymarket leaving core in S2 are closed. Memlog 63's "the publish-order script merges before or with the first new foundation crate" is missing. AD-16's S1 order names only the generator and the tag reporter. See the fix in §3. |
| M-F7 | CLOSED | AD-1 and AD-11 (injected rule). |
| M-F8 | CLOSED | The Module docs convention row (per-module README doctests plus a count check). |
| M-F9 | CLOSED | AD-22 (`cargo hack --each-feature`) and AD-2 (per-module allowlist). |
| M-F10 | CLOSED | AD-21's superseded-rules table, AD-13's generated regions, and `docs/ARCHITECTURE.md` with a CLAUDE.md pointer. |
| M-F11 | CLOSED | AD-8: `WARN` with fixed text, permit released before the sleep, non-retry holds and bans also warn. |
| M-F12 | CLOSED | AD-22 adds the MSRV job, and AD-5's trigger names it. |
| M-F13 | CLOSED | AD-13 (nightly-schema watch list derived, every live test has a row, dev-dependency rule) and the CI flowchart (CI scripts, nightly-schema, "red withholds the release"). The publish-order timing is tracked under M-F6. |
| M-F14 | CLOSED | AD-10's public `WindowQuotaTable` and `effective_quota`; AD-12 has S1 move suites to public API. |
| M-F15 | CLOSED | AD-12. |
| M-F16 | CLOSED | AD-8 `RequestParts { method, path, query, headers, body }` may add query parameters. |
| M-F17 | CLOSED | AD-1: "Anything published behind `test-server` is built only from published crates". |
| M-F18 | CLOSED | The test-target names were promoted to a decision with a suite slot (memlog 74). "Product id = module" is decided in AD-4. `#[ignore]` is in the conventions and checked by AD-13. |
| M-F19 | CLOSED | AD-10 `Arc<DynThrottle<'static>>`. |
| M-F20 | CLOSED | The Rustdoc convention row. |
| M-F21 | CLOSED | Both invented items are gone. Kalshi credentials are covered by `auth-gated` and `<VENUE>_<ENV>_*`. |
| M-F22 | CLOSED | AD-18: "Builders leave `gzip` unset". |
| M-F23 | CLOSED | AD-20: `VenueRefusal`, `is_fault` false, never retried. AD-14 tags from the class. |
| M-F24 | CLOSED | Sources list the memlog; AD-2 no longer mentions gzip; the layout row allows `api.rs` or `api/`; one enum per transport tier. |

### rubric

| ID | Status | Evidence |
|---|---|---|
| RB-01 | CLOSED | AD-9: every 429 holds; a 425 waits per request; hold `retry_delay(0)`, wait `retry_delay(attempt)`; the CAP-1 mutants are restated. Matches `client.rs:126-178` and `request.rs:155-167`. |
| RB-02 | CLOSED | AD-8, AD-10 and AD-23 give the full shapes; the builder computes cost. The "throttles keep no cooldown" sub-fix was replaced by memlog 87: the hold is throttle state. |
| RB-03 | CLOSED | AD-1's rules and graph: the dev-dependency arrows run venue → test-support; test-support → core and venue; facades → core and venue; the per-layer rule covers Kalshi. |
| RB-04 | CLOSED | AD-15 adds `Unavailable` and `Decode`, timeout counts as `Network`, `is_fault` is defined, `code: Option<Arc<str>>`, and `Class` is `#[non_exhaustive]`. Residual: no status map for other 4xx (N6). |
| RB-05 | CLOSED | The shared-code homes table covers H1-H16, W1-W15, T1-T9 and C1-C4. H2 is covered by AD-8 ("including health pings"); W14 by DRIFT R6 and the Features row. |
| RB-06 | CLOSED | AD-2: allowlist file plus both deny-lists. I checked that the socket deny-list is satisfiable today: `cargo tree -p polyoxide-rtds`/`-p polyoxide-sports` contain none of the denied crates. |
| RB-07 | CLOSED | AD-12's list is open. Counts verified: perps inline 21, binance `supervision.rs` 21, `supervision_edges.rs` 7, rtds 4 + 15, sports 15, `bare.rs` 6, inline 6, `classify_failures` 27; `test_diff_openapi` has 56. The mutant list goes in `docs/ARCHITECTURE.md` (see N12). |
| RB-08 | CLOSED | AD-14: `or_fail`, a chained `Once`, precedence, a shrinking list, no new regexes, sunset no later than S2. |
| RB-09 | CLOSED | AD-17: one policy, used by every Polymarket module on core's loop. |
| RB-10 | CLOSED | Hook homes: core's `polymarket` module (AD-16, AD-17). Error renames wait for S2 and S1 changes variants only (AD-16, conventions). Fix bullet 2 ("S1 keeps every socket type's name") was rejected by memlog 107: consolidated items break in S1 under the removal list. |
| RB-11 | CLOSED | AD-4 (ids in metadata plus a unit test), AD-13 (`live.<target>`, SELF-HEALING.md regions, Kalshi touch list with `[workspace.dependencies]`, `Cargo.lock` and `docs/ARCHITECTURE.md`). |
| RB-12 | CLOSED | AD-5 uses the 0.3 attribute form, `Send + Sync`, no associated consts, and enumerates the dyn-held traits. **Regression:** `'static` was added to the supertraits and does not compile (N1). |
| RB-13 | CLOSED | AD-8: "That floor belongs to the loop, never to a policy". AD-9's zero-wait mutant. |
| RB-14 | CLOSED | AD-5's stream form plus AD-24. "Backpressures" was superseded by memlog 106 (unbounded plus `Resync`). **Regression:** the stream form needs `Unpin` under dynosaur (N2). |
| RB-15 | CLOSED | AD-10: core exports a public capacity bucket; "Venue crates never depend on governor". |
| RB-16 | CLOSED | The Umbrella convention row: `Polymarket` moves, `PolymarketError` is removed, the builder returns `BuildError`, `polymarket-<module>`, and `full` is defined. |
| RB-17 | CLOSED | AD-25 (no bump on loom or integration branches) and AD-16 (rename manifest, tombstones). |
| RB-18 | CLOSED | AD-12: "Every DFR row … binds every epic. D13's two backoffs are never unified." |
| RB-19 | **OPEN (low)** | The credential half is closed (AD-14 env names, `auth-gated`, Kalshi `publish = false`). The CAP-7 grep gate still has no pattern or crate scope (AD-16 says only that it "starts here"). |
| RB-20 | CLOSED | AD-4: `RawKey::parse`, canonicalizing constructors only, `Cow<'static, str>` newtypes, serde as text. |
| RB-21 | CLOSED | AD-19's bounds and derives. "Serde and PartialEq skip Extensions" was replaced by memlog 93: contents compared, no serde. |
| RB-22 | CLOSED | AD-8: `headers` in `RequestParts`; a transport error skips `observe` and `decide` and classes as `Network`. |
| RB-23 | CLOSED | AD-9 states both Binance conditions and the 2-minute default. Verified at `usdm/request.rs:111-151` and `weight.rs:26`. |
| RB-24 | **OPEN (low)** | AD-18 from S2 is still "`polyoxide --features full`", which excludes the `publish = false` Kalshi skeleton (Deferred: Kalshi not in the umbrella), so its header test never runs unified. |
| RB-25 | CLOSED | AD-22: "These are jobs in `ci.yml`". |
| RB-26 | **OPEN (low)** | Closed: CLI depth (nested verbs), credential kinds, `keychain`/`parquet`, `-ws` only for dual-transport modules, error naming. Undecided: clob's default dependency on gamma (`polyoxide-clob` `default = ["gamma"]`) becoming intra-crate, i.e. whether `clob` implies `gamma` or its gamma code is gated `all(clob, gamma)`. |
| RB-27 | CLOSED | AD-15: one-to-one from `Class`, except data v2, which maps by `code`. |
| RB-28 | CLOSED | AD-21: "Superseded rules include"; the spine wins until the guide is regenerated. |
| RB-29 | CLOSED | Stack adds alloy, keyring, clap, futures-util, cargo-nextest and git-cliff. Nit: maturin, uv and tokio-tungstenite's `rustls-tls-native-roots` feature are still unlisted. |
| RB-30 | CLOSED | The rationale fragments are removed, and frontmatter has `spec_amendments`. |
| RB-31 | CLOSED | RB-10, RB-16, RB-17 and RB-20 are now decided. N3 and N6 are new undecided seams, and the spine still has no Open Questions section to hold them. |

### verify-current

| ID | Status | Evidence |
|---|---|---|
| V-F1 | CLOSED | AD-5: `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`, `Send + Sync`, `Arc<Dyn<Trait><'static>>`. **Regression:** N1. |
| V-F2 | CLOSED | AD-22 `resolver = "3"`; Stack lists alloy 1.1.2 and keyring 3. |
| V-F3 | DEFERRED | Deferred: "Dependency upgrades … The restructure keeps today's pins." |
| V-F4 | CLOSED | AD-8 Binds names the exception ("on-chain RPC through alloy providers"). AD-18: "Transitive copies (alloy's reqwest) do not count." |
| V-F5 | CLOSED | AD-11: the `Supervisor` races `reserve()` against the ping timer, and blocked time is not silence. |
| V-F6 | CLOSED | AD-8 "target prefix `polyoxide_core`"; Stack edition 2021 and resolver 3; AD-7's one-sided conversion covers orders (memlog 94). Kalshi Ed25519 belongs to deferred Kalshi credential storage. |

### adversary

| ID | Status | Evidence |
|---|---|---|
| P1 | CLOSED | AD-23 (`Throttle::hold`; a shared throttle shares its hold; `observe` records counts and tiers only). |
| P2 | CLOSED | AD-8 and AD-10 (`Cost`, `LayerId`, `Charge`/`LayerCharge { window }`, `Refused`, builder-computed `costs`, `Refused` → existing variant → `InvalidRequest`). |
| P3 | CLOSED | AD-11 Membership (`P::Membership`, `wanted`, `Option<Vec<Market>>` with a test, routers end paths only by emptying them). |
| P4 | CLOSED | AD-11 (pings and staleness never wait; `Bounded(n)`/`Unbounded`) and AD-24. |
| P5 | CLOSED | AD-15's eight classes, status before body, provided `is_retriable()`; AD-14 tags from class. Residual: N6. |
| P6 | CLOSED | AD-14. |
| P7 | CLOSED | AD-10 (`WindowQuotaTable`) and AD-12 (S1 makes suites public-API-only; S2 shows an empty normalized diff). |
| P8 | CLOSED | AD-4. |
| P9 | CLOSED | AD-19. |
| P10 | **OPEN (medium)** | Closed: the S3 records story, `Side`, `Level`, `Instrument` as the only tick and size carrier, `size`/`notional`, `UnixMillis::now()`. Missing: memlog 93's "Polymarket's `Trading::positions` reads the data API so that impl is gated `all(clob, data)`". |
| P11 | CLOSED | AD-13 (generated-region markers; the generator merges first in S1) and AD-21 ("inside its own `##` section"). |
| P12 | CLOSED | AD-3 and AD-15 `impl_ws_classification!`. |
| P13 | CLOSED | AD-13 `live.<target> = { suite, timeout, features }` from S1, rows grouped by (crate, suite). The issue-identity concern is moot: staging found the tracking issue is keyed on the `nightly-behavioral` label alone. |
| P14 | CLOSED | AD-17. |
| P15 | CLOSED | AD-4 (ids in metadata plus a unit test per venue crate). |

### staging

| ID | Status | Evidence |
|---|---|---|
| ST-F1 | REJECTED-BY-DECISION | Memlog 107: the user chose the strict reading of no shims. Consolidated items break in S1 with no interim re-export. The finding's gate (item 5) was adopted: a checked-in removal list plus `cargo semver-checks` (AD-16, AD-22). That gate has enforceability gaps (N5). |
| ST-F2 | **OPEN (low)** | Items 2 and 3 are closed by AD-25. Item 1 is not: S1 epics must branch from current `origin/main` (v0.38.1 per memlog 83; this worktree is at 0.37.1, `e3d8c3e`). Memlog 83 records it but the spine does not. |
| ST-F3 | CLOSED | AD-16: a draft PR at session start and nightly-behavioral dispatched on the integration ref. |
| ST-F4 | CLOSED | AD-22 dry-run; AD-25 (`publish_order.py` from the crates.io API, resumable, five new crates at most); AD-13 dev-dependency cycle check. The token-scope residual is tracked under S-L6. |
| ST-F5 | CLOSED | AD-16 (rename manifest from a public-API diff; tombstones) and conventions (specta, keychain and parquet keep their names; additive `test-server`; docs.rs features; `UsdmError`; umbrella). The tombstone mechanics are broken (N4). |
| ST-F6 | CLOSED | AD-16: prader migrates removals and call sites in S1; S1 is one release or few; notes carry a consumer-impact section. |
| ST-F7 | CLOSED | AD-22: gates are `ci.yml` jobs; MSRV runs without `-D warnings`. AD-25: release fails loudly on a reused version. |
| ST-F8 | CLOSED | AD-16 S1 order: the tag reporter and live-test migration land before any error reshape. |
| ST-F9 | CLOSED | AD-16 S3: the clob `Supervisor` is a new type; the Kalshi skeleton is `publish = false`. |
| ST-F10 | CLOSED | AD-2 (S1 per crate) and AD-18 (S1 `cargo test --workspace --all-features`). |
| ST-F11 | CLOSED | AD-25: "Versions and pins are read from `cargo metadata`, never by regex." |
| ST-F12 | CLOSED | AD-1 (inline helpers stay in-crate or in ws `test-server`); AD-12 ("test function names"). |

---

## 2. Spot-checks against the code (what holds)

- **AD-9, core.** Correct as stated:
  - `should_retry` retries exactly 429 and 425 (`client.rs:132`) with `retry_delay(attempt)`.
  - `note_rate_limited` is a no-op except for 429 and cools with `retry_delay(0, …)` (`client.rs:171-176`).
  - It is called before `should_retry` and unconditionally (`request.rs:155`).
  - The permit is dropped before the sleep (`request.rs:167`), and the log text matches (`request.rs:161`).
  - The core `Retry-After` clamp is `max_backoff_ms` (`client.rs:145-154`).
- **AD-9, Binance.** Correct as stated:
  - A 418 begins a cooldown of `Retry-After` or `DEFAULT_BAN` (120 s) and is not retried (`usdm/request.rs:111-126`, `weight.rs:26`).
  - A 429 holds `max(retry, Retry-After)`, or `hold_until_next_minute` only when `(None, None)` (`:127-151`).
  - The clamp is `MAX_COOLDOWN`, 3 days (`weight.rs:30`, `error.rs:128-137`).
  - Binance's 429 retry `continue`s without a sleep: the wait is served by the budget's cooldown. Under AD-8 the loop also sleeps, but the hold is at least the sleep, so the total wait is unchanged.
- **AD-12 counts.** All match the code (see RB-07). One unlisted inline suite also lives in a file the S1 migration deletes: `binance/usdm/ws/supervised.rs` (3 tests, Backoff and cadence defaults). The open list covers it.
- **AD-15 Binance facts.** 451 → `RegionBlocked`, 403 → `Forbidden`/WAF; DFR D14 names both, so the `Restricted` override has its DFR row.
- **Stack.** Every row matches `Cargo.toml` (alloy 1.1.2 is the caret pin; the lock holds 1.8.3).

---

## 3. Open prior findings: severity and minimal fix

| ID | Sev | Minimal fix text |
|---|---|---|
| S-L3 | high | Merged into N3 below. |
| M-F6 | medium | AD-16 S1 epic order, prepend: "`scripts/publish_order.py` and its `release.yml`/`finish_release.sh` wiring merge before, or with, the first new foundation crate; until then no release is cut from `main`." |
| P10 | medium | AD-3, add: "Polymarket's `Trading::positions` reads the data API, so the clob `Trading` impl is gated `all(clob, data)` and declared in the crate's Cargo metadata." |
| S-L6 | low | AD-25, add: "Before S1's release, confirm the crates.io token's crate scope covers `polyoxide*` and its expiry is past S3." |
| RB-19 | low | AD-16 S2: "The CAP-7 gate greps `polyoxide-{venue,core,ws,test-support}` case-insensitively for every venue and product id in the registration metadata, with a checked-in exception list." |
| RB-24 | low | AD-18: "From S2, the second build is `cargo test --workspace --all-features`, which includes the Kalshi skeleton; `polyoxide --features full` is an additional build." |
| RB-26 | low | Features row: "clob's gamma lookups are gated `all(clob, gamma)`; `clob` does not imply `gamma`", or the reverse, decided in the rename manifest. |
| ST-F2 | low | AD-16 S1, add: "S1 epics branch from current `origin/main` (≥ v0.38.1); the inventory's gamma and sports rows are re-audited there first." |

---

## 4. New problems introduced or exposed by the revision

### N1 — high — AD-5's `'static` supertrait does not compile with dynosaur 0.3.1

**Where:** AD-5 says "Each declares `Send + Sync + 'static` supertraits".

**Verified.** A scratch crate on rustc 1.95.0 with `dynosaur = "=0.3.1"` held one trait:
`#[dynosaur::dynosaur(pub DynThrottle = dyn(box) Throttle)] pub trait Throttle: Send + Sync + 'static { fn acquire(&self, n: u32) -> impl Future<Output = u32> + Send; }`.
- It fails with 14 errors (E0477, E0478, E0521, E0803, "`'dynosaur_struct` must outlive `'static`").
- Dropping `'static` compiles.
- The generated `DynX<'dynosaur_struct>` must implement the trait for every lifetime, which a
  `'static` supertrait forbids.

**No memlog basis.** Memlog 85 records "`Send + Sync` supertraits", and verify-current tested
exactly that. `'static` came from RB-12's suggested text.

**Why high.** It is the first trait the S1 core epic writes, and AD-5's fallback clause ("If
dynosaur fails the MSRV job … every trait moves to `async-trait` in one change") invites a
spurious switch of the whole workspace.

**Fix.** "Each declares `Send + Sync` supertraits (never `'static`: the held
`Arc<Dyn<Trait><'static>>` already makes the erased object `'static`) and no associated
consts."

### N2 — medium — AD-5's stream shape does not compile on a dyn-held trait

**Where:** AD-5 "Streams are written `-> impl Stream<Item = …> + Send + 'static`", applied to
`Trading::events()` (AD-24).

**Verified.**
- dynosaur boxes a non-`Future` RPIT as `Box<dyn Stream + Send>` with no `Pin`
  (`dynosaur_derive-0.3.1/src/expand.rs:63-66,190-192`).
- `Box<dyn Stream + Send>` is a `Stream` only when `Unpin`, so the generated blanket impl fails
  with E0277 "`dyn Stream<Item = u32> + Send` cannot be unpinned".
- Adding `+ Unpin` to the declaration compiles, and both static and dyn callers can then
  `.next().await`.

**Fix.** "Streams are written `-> impl Stream<Item = …> + Send + Unpin + 'static`;
implementations return `Box::pin(..)` or another `Unpin` stream." Alternatively, return a
concrete `Pin<Box<dyn Stream<Item = …> + Send>>`.

### N3 — high — The venue traits' error type is undecided, and AD-5's held form cannot hold it (absorbs S-L3)

**Where:**
- AD-5: dyn-held traits "held as `Arc<Dyn<Trait><'static>>`".
- AD-15: "No venue-wide catch-all exists" and one enum per module.
- Memlog 106: `Stream<Item = Result<TradingEvent, E>>`.
- Memlog 18 rejected per-venue associated types because "mixed-venue lists and the dyn path
  need one type".

**Problem.**
- verify-current showed that an associated `Error` becomes a type parameter on the wrapper
  (`DynMarketData<'static, E>`). The stated held form therefore exists only if the error is one
  concrete type, and AD-15 forbids that type.
- Nothing binds trait errors to `Classify`, so CAP-4's "enforced at compile time" has no
  mechanism. This was S-L3, still open.
- The S3 market-data and trading epics will choose independently: an associated type, a boxed
  `dyn Error`, or a new enum. A mixed-venue `Vec<Arc<DynMarketData>>` is impossible under the
  first.

**Fix.** Add to AD-5:
- "Every venue trait has `type Error: Classify + std::error::Error + Send + Sync + 'static`.
  Static callers use it as is, and the dyn form is `Dyn<Trait><'static, E>`."
- "For mixed-venue holding, `polyoxide-venue` provides an erasing adapter
  (`Erased<T>`, with `Error = ClassifiedError { class: Class, source: Box<dyn Error + Send + Sync> }`),
  which is an erasure, not a catch-all enum."

Record the choice in the memlog, since it amends memlog 18's "one type" rationale.

### N4 — high — The S2 tombstones break CI, the dry-run gate and docs.rs

**Where:**
- AD-16: the seven retired crates "each publish one tombstone at the S2 version: … a pointer and
  `compile_error!`".
- Memlog 108: they "stay workspace members for exactly the S2 release".

**Problem.**
- A workspace member whose lib root is an unconditional `compile_error!` fails every
  workspace-wide build:
  - clippy `--all-targets --all-features`;
  - nextest;
  - doctest;
  - `cargo doc --workspace`;
  - AD-22's `cargo publish --workspace --dry-run`, which verifies by building.
- CI goes red on the S2 merge and silently withholds the S2 release (CLAUDE.md, release.yml
  `workflow_run`).
- docs.rs also fails to build the tombstone, so its page shows a build failure instead of the
  pointer the tombstone exists to show.
- The generator (AD-13) would also list seven tombstones in the README and CLAUDE.md crate
  tables.

**Fix.** Add to AD-16:
- "Tombstones live under `tombstones/<crate>/`, outside the workspace (`[workspace] exclude`).
- They are published by a dedicated release step with `cargo publish --manifest-path … --no-verify`.
- Their `compile_error!` is gated `#[cfg(not(docsrs))]`, so docs.rs renders the pointer.
- CI checks only that each tombstone manifest has no dependencies and that the S2 version
  matches.
- `publish_order.py` lists tombstones separately, and the generator skips them."

### N5 — medium — The S1 removal gate cannot enforce the rule as written

**Where:** AD-16 says `cargo semver-checks` "fails CI on any other removed path"; AD-22 says
"`cargo semver-checks` (S1)".

**Problem.**
- **0.x minor bumps permit removals.** cargo-semver-checks requires a "major" bump for removal
  lints, and for 0.x crates "0.5.2 to 0.6.0" counts as major (Context7,
  `/obi1kenobi/cargo-semver-checks`, lint-level configuration). polyoxide's lockstep 0.x minor
  bump therefore permits every removal. The bump commit, which AD-25 makes the last commit
  before the tag and which CI gates, passes whatever was removed.
- **No per-item allowlist.** The tool cannot express "fail on any removal not on the checked-in
  list" without a wrapper.
- **No baseline for new crates.** `polyoxide-venue`, `-ws` and `-cli` (first publish) have
  nothing to compare against.
- **Hidden items.** Items marked `#[doc(hidden)]` (the `test_server` modules prader imports) are
  outside the tool's view of public API.
- **No stated end.** Nothing says whether the gate continues after S1. S3 is supposed to be
  additive.

**Fix.** Add to AD-22:
- "The removal gate runs `cargo semver-checks --baseline-rev <last release tag> --release-type patch`,
  so every removal is reported regardless of the 0.x bump. It skips crates absent from the
  baseline.
- `scripts/api_removals.py` fails on any reported removal not in the checked-in S1 list.
  Doc-hidden removals are listed by hand.
- It is report-only in the S2 integration session, where it feeds the rename manifest, and
  fail-on-any-removal from S3."

Spike the configuration in the first S1 PR.

### N6 — medium — "The status decides the class" has no status map beyond 408/425/5xx and 451

**Where:** AD-15. It defines `Unavailable` (408, 425, 5xx), `Restricted` (451, Binance WAF 403)
and `VenueRefusal` ("a 4xx caused by the request"). It gives `InvalidRequest` no meaning and
assigns no class to 400, 401, 403, 404, 409, 418 or 422.

**Problem.**
- Seven S1 error epics (AD-16 S1, "variants only") each choose between `InvalidRequest` and
  `VenueRefusal` for a 400, so prader's generic classifier sees different classes per venue.
- Binance's 418 matters most. Mapped to `RateLimited`, the provided `is_retriable()` becomes
  true and the nightly tag becomes `transient`, so `--retries 2` re-sends into a ban. Today
  `BinanceError::IpBanned` is not retriable (`binance/src/error.rs:97-104`), and Binance's own
  doc says re-sending lengthens the ban.

**Fix.** Add a status column to AD-15's table:
- 401 and 403 → `Unauthorized` (a venue override needs a DFR row; Binance 403 → `Restricted`, D14);
- 418 → `Restricted` (an IP-level refusal: not retriable, `environmental` in nightly);
- 429 → `RateLimited`;
- every other 4xx → `VenueRefusal { code }`.

Then define the remaining class: "`InvalidRequest` is a client-side refusal (`Refused`, an
undeclared extension, local validation) and never comes from a status."

### N7 — medium — AD-10's layer models omit Binance's weight minute

**Where:**
- AD-10: "a published window quota is depth 1 with a tenth reserved; a published capacity uses
  `allow_burst`".
- Shared-code homes H5: "reserve-a-tenth, paced slot → `polyoxide-core` … window-quota table".

**Problem.** Binance's 2400/min is a published window quota, but `WeightBudget` is a UTC-minute
counter, not a depth-1 GCRA bucket. It allows the full 2160 per minute and is raised by
`X-MBX-USED-WEIGHT-1M` for the charged minute only (`weight.rs:1-60`, `:286-359`). Read
literally, AD-10 turns it into a depth-1 bucket. AD-10 also says every non-request layer
"charges exactly its `costs` entry", so a weight-5 `klines` request against a depth-1 bucket is
a cost "the bucket can never hold" and is `Refused` forever. Only D3, bound through AD-12,
stands in the way.

**Fix.** Add to AD-10:
- "A third model, Binance's weight minute (a UTC-minute counter at 2160 of 2400, corrected by
  the server's count for the charged minute), stays in `polyoxide-binance` per D3. It uses only
  core's hold.
- Core's window-quota table carries depth-1 window quotas only (Polymarket's tables, Binance's
  funding bucket)."

Restrict the H5 home to the same.

### N8 — medium — No tag for the "server ended the connection" convention once the regexes go

**Where:** AD-14. It says the regex table is deleted "no later than S2"; its only explicit
reporter calls are `or_fail`, the credential loaders and `environmental(reason)`.

**Problem.**
- The binance, sports and rtds live tests panic with "server ended the connection" where the
  bare stream ends without a close code (`binance/tests/live_ws.rs`, `sports/tests/live_api.rs`,
  `rtds/tests/live_api.rs`).
- `classify_failures.py:149-152` classes that text as transient. CLAUDE.md lists it as a
  dropped-WebSocket signal.
- There is no `Result` to `or_fail` and no transient reporter, so after the deletion every
  routine socket drop in nightly is filed `real`.

**Fix.** Add to AD-14: "test-support provides `transient(reason)`, which prints `transient`.
Tests that observe a stream ending without a close code call it in place of the panic text."

### N9 — low — AD-8 does not mark which hooks are async

AD-5 fixes the async form but not which methods use it. Memlog 60 makes `Authenticator::sign`
async, and Polymarket's L1 EIP-712 signing is async through alloy (`clob/src/core/eip712.rs:419-428`).
A sync `sign` forces a `block_on`.

**Fix.** Add to AD-8: "`acquire` and `sign` are async (AD-5 form); `observe`, `decide` and
`hold` are synchronous."

### N10 — low — AD-14's `<VENUE>_<ENV>_*` naming collides with existing secret names

CLAUDE.md's `POLYMARKET_PRIVATE_KEY`/`POLYMARKET_API_*`, `BUILDER_*` and `RELAYER_*` do not fit
the pattern, and the clob live tests read the OS keyring. Read literally, AD-14 renames users'
environment variables and the repository secrets.

**Fix.** "applies to CI secrets for venues added from S3; existing Polymarket, builder and
relayer names are unchanged."

### N11 — low — AD-16's S1 coverage list reads as exhaustive

AD-16's S1 list reads "AD-8 to AD-11, AD-14 and AD-15". It omits AD-17, AD-18 and AD-23, which
are also S1 work (AD-17 and AD-18 say so themselves), and AD-2, AD-12, AD-22 and AD-25 apply
from S1.

**Fix.** "It covers, among others:"

### N12 — low — The mutant list lives in a file AD-21 says is regenerated from the spine

AD-12 records mutants in `docs/ARCHITECTURE.md`. AD-21 says that guide is "regenerated from"
the spine, so a regeneration can drop the hand-kept list (a second writer, as in P11).

**Fix.** "Mutants live in `docs/MUTANTS.md`, or in a section of `docs/ARCHITECTURE.md` that
regeneration preserves."

### N13 — low — Two checks have no named tool

- AD-12's "empty normalized diff of the test bodies".
- AD-14's "list of live tests that unwrap without `or_fail`".

Both are CI-shaped, but neither names a script, so each S2 epic would write its own
normalizer.

**Fix.** Name `scripts/test_body_diff.py` and `scripts/live_unwraps.py` in AD-12 and AD-14.

### N14 — low — Memlog decisions missing from the spine (beyond M-F6, P10, S-L6 and ST-F2)

- **Memlog 34.** "CI fails when a `tests/live_*.rs` lacks required-features". The conventions
  require `required-features`, but AD-13's CI-fails list has no such check.
- **Memlog 101.** "S2 moves entries, never the schema." The spine has only the S1 keying. The
  issue-renaming half is moot (see P13).
- **Memlog 37.** "main stays releasable". AD-25 covers the bump, but nothing states the
  invariant that M-F6's ordering protects.

**Fix.** Add the first two to AD-13 and the third to AD-25.

### N15 — low — Spine content with no memlog basis

All of it is benign except N1.

- **AD-5 `'static`.** Wrong; see N1.
- **AD-18 "`cargo test --workspace --all-features` (S1)".** This follows staging F10 and
  RB-24; memlog 66 names only `polyoxide --features full`.
- **The Stack versions for cargo-nextest and git-cliff.** These are recorded only as
  `(version)` entries, which is acceptable.

**Fix.** Add one memlog line ratifying AD-18's S1 form, and correct AD-5 per N1.

---

## 5. Contradictions checked and found consistent

These pairs were checked and agree:

- AD-1 ↔ AD-2 ↔ AD-3 ↔ AD-11 ↔ AD-15: ws has no venue edge; classification is injected and
  macro-expanded.
- AD-8 ↔ AD-9 ↔ AD-23: one `Decision`; the hold is called once; the floor is the loop's.
- AD-10 ↔ AD-15 ↔ AD-19: `Refused` and an undeclared extension both class as `InvalidRequest`.
- AD-11 ↔ AD-24: an `Unbounded` fills queue and `Resync` after `Reconnected`.
- AD-13 ↔ AD-21 ↔ AD-25: generated regions; cargo metadata as the source.
- AD-14 ↔ AD-15: tags from the class alone; kills are `VenueRefusal` and are filed as `real`
  only when a test unexpectedly hits one.
- AD-16 ↔ memlog 105-108: eight classes; unbounded plus `Resync`; strict S1 with no shims plus a
  removal list; tombstones; typed extensions; `Ok(Killed)`; the S2 integration session.
- AD-2's S2 deny-list (`hmac` for credential-free features) can only pass after Polymarket's
  `Signer` leaves core. Today gamma and binance build hmac, sha2 and governor through core.
  AD-2 already runs the per-feature check only from S2, so the sequencing holds.
