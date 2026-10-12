---
title: 'Story 5.2: polyoxide-polymarket and a venue-neutral core'
type: 'refactor'
created: '2026-10-12'
status: 'in-progress'
route: 'dispatch'
baseline_commit: '436ce762c6a4061dc6d5228c9d687fa8367c4b3d'
review_loop_iteration: 0
context:
  - '{project-root}/_bmad-output/implementation-artifacts/epic-5-context.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-5-1-open-the-s2-integration-session.md'
  - '{project-root}/docs/rename-manifest.md'
  - '{project-root}/ci/test-body-diff.toml'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `polyoxide-core`, the foundation every venue builds on, still carries Polymarket's code, so Binance, and next Kalshi, compile Polymarket's rules and can inherit its defaults. That code is the limit tables, the throttle, the 425 retry policy, the HMAC signer, the signer layer and the session-signer types. Nothing stops venue names creeping back into the foundation crates.

**Approach:**
- Create the venue crate `polyoxide-polymarket`. Move every Polymarket item out of core into its feature-gated `src/shared/`, with the tests that exercise them. Point the old Polymarket crates at it until 5.3–5.5 fold them in.
- Start the CAP-7 gate, which fails when a foundation crate (venue, core, ws, test-support) names a venue, a product, a `venue.product:` prefix or a venue identifier, beyond the exceptions each venue declares.

**Decisions:**
- **A public `shared`, permanent** (the user, 2026-10-12). `polyoxide_polymarket::shared` is a public module, feature-gated and stable from 5.2 on. It holds the limit tables, `ClobThrottle`, `clob_throttle`, `signer_cost`, `PolymarketRetryPolicy`, the signer layer (`SignerLimiter`, `Tier`, `TradingBucket`, `TradingRequest`, `RateLimitStatus`, `BurstCapacityExceeded`), the session-signer types and the HMAC `Signer` with `Base64Format` and `current_timestamp`. A module may re-export what its API exposes. This departs from the spine's "venue-private" wording for `src/shared/`, and 5.11 records that.
- **The transition.** Until each old crate is folded in, gamma, data, perps, relay and clob depend on `polyoxide-polymarket` with the matching feature. The branch never ships in this state; the edges go when 5.3–5.5 delete those crates.

## Boundaries & Constraints

**Always:**
- **What moves.** Everything core holds that is Polymarket's:
  - `src/polymarket/` (mod, limits, throttle);
  - `src/signer_limit.rs`;
  - `src/session_signer.rs`;
  - `src/auth.rs`.
  - Core then declares no `base64`, `hmac` or `sha2`.
  - The venue-neutral machinery stays: the send loop and hooks, `WindowQuotaTable`, `RateLimiter`, `RetryConfig`, `CapacityBucket`, `Hold`, `Refused`, the keychain and the `reqwest` re-export.
- **Gating.** Every `shared` item, and the optional `polyoxide-core`, `hmac`, `sha2` and `base64` dependencies, are gated by the features that use them. The features are `clob`, `gamma`, `data`, `relay` and `perps`, all off by default until their modules arrive. `cargo hack --each-feature` passes.
- **Tests move unchanged.** A test whose code moves, or whose body names a moved item, moves with it under its own name, and `scripts/test_body_diff.py` stays at 0 diffs and 0 lost. Each mutant in `docs/MUTANTS.md` whose cited test or line moves is re-cited and still fails its test.
- **The rename manifest.** It gains a `## Paths` row for every core path that goes, mapped into `polyoxide_polymarket::shared`. `api_removals.py manifest --strict` resolves every entry with no stale row.
- **The CAP-7 gate.** A pytest on the real tree, run in the scripts job.
  - **What it reads:** venue ids, products and each crate's new `identifiers` from cargo metadata.
  - **What it greps,** case-insensitively, in every file under `polyoxide-{venue,core,ws,test-support}`:
    - venue ids, as substrings;
    - product ids, as identifier segments split at `_`, `-`, `.`, `::` and case boundaries, minus a checked-in skip list (`data`, `events`);
    - `<venue>.<product>:` prefixes;
    - identifiers (Polymarket: `Poly-RateLimit`).
  - **Passing:** a hit passes only through a `gate_exceptions` entry in the hit venue's own crate metadata, of the shape `{ path, term, reason }`. An entry that matches nothing fails the gate.
- **The foundation's own docs and comments name no venue.** Scrub them: core, venue and ws comments, READMEs, Cargo keywords and descriptions, and the keychain doc examples. A protected test name with a venue word gets a `gate_exception` instead of a rename.
- **AD-21.** Every CLAUDE.md rule this makes false is rewritten in the same change. Generated regions are regenerated with `gen_registry.py --write`.

**Never:**
- leave shims or re-exports in core;
- rename a moved test;
- change a moved test's assertion;
- weaken a CI-script anti-vacuity assertion;
- give `polyoxide-polymarket` a `ci/dep-allowlists` file in 5.2;
- add `[[drift]]` entries (S2's only DRIFT row is R6, and it is 5.5's);
- touch Lane B's work.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Gate clean | the tree after the move | the CAP-7 test passes, and each exception matches at least one hit | N/A |
| Venue word added | a foundation comment gains "binance" | the gate fails, naming the file, line and term | N/A |
| Product segment | an identifier `clob_default` or `PERPS_GENERAL` in core | caught as the segment `clob` or `perps` | N/A |
| Generic word | `data` or `events` in core | not a hit (skip list) | N/A |
| Stale exception | an exception whose term no longer appears in its path | the gate fails, naming it | N/A |
| Moved test | `polymarket_limits.rs` moved to the new crate | the body diff pairs it with 0 diffs | N/A |
| Manifest | the core paths gone | every entry resolves under `--strict` | N/A |

</frozen-after-approval>

## Code Map

- **`polyoxide-core/src/polymarket/{mod,limits,throttle}.rs`**:
  - `PolymarketRetryPolicy` (mod.rs:51);
  - `*_limits` (limits.rs:50, 142, 181, 205, 234);
  - `ClobThrottle` (throttle.rs:52), `clob_throttle` (:123) and `signer_cost` (:27);
  - the constants at :15-19.
- **`polyoxide-core/src/signer_limit.rs`**: `TradingBucket`, `Tier`, `TradingRequest`, `RateLimitStatus` (the `Poly-RateLimit-*` parsing, :194), `BurstCapacityExceeded` and `SignerLimiter`.
  - `pub(crate) charge` (:437) is called only by `ClobThrottle`, so the two files move together.
  - The `debug!` at :398 changes its tracing target; log that for 5.12's notes.
- **`polyoxide-core/src/{session_signer,auth}.rs`**: `DepositWalletRole`, `SessionSignerScope`, `Signer`, `Base64Format` and `current_timestamp`.
  - Root re-exports: lib.rs:93 and :113-116.
  - The `Classify` assertion: lib.rs:127, which names `BurstCapacityExceeded`.
- **Users to repoint at `polyoxide_polymarket::shared`:**
  - gamma: client.rs:1, :76, and api/health.rs:43, :63;
  - data: client.rs:1, :175, and examples/v2_soak/main.rs:57, :576;
  - perps: client.rs:3, :60;
  - relay: client.rs:21, :979, :1166, :2017, config.rs:4, lib.rs:133, session_signers.rs:15, :246, and 18 sites in tests/mock_api.rs;
  - clob: client.rs:2, :678, :709, :903, error.rs:1, :75, api/orders.rs:5, api/health.rs:104, account.rs:7, authenticator.rs:11, target.rs:14, lib.rs:191, and mock_api.rs:3568, :3578, :3864-3871, :4075, :4387-4388.
  - Binance, rtds, sports, the umbrella, the CLI, py and test-support use none of them.
- **Core tests that move whole to `polyoxide-polymarket`**, because they run on Polymarket's policy or tables:
  - tests/polymarket_limits.rs (48) and tests/polymarket_throttle.rs (6);
  - tests/send_loop.rs (21), since all of them go through `Deciding`, which defers to `PolymarketRetryPolicy`;
  - tests/health.rs: `a_429_on_health_holds_the_next_request` (:126);
  - client.rs `mod tests`: the 7 `test_should_retry_*` tests with the `retries` helper (:296-395), and `test_builder_with_rate_limiter` (:503);
  - rate_limit.rs `mod tests`: the 2 tests at :1124-1178;
  - the inline tests of signer_limit.rs (22), session_signer.rs (3) and auth.rs (7).
  - **How they pair:** inline tests go into `#[cfg(test)] mod tests` of a `shared` file, so they keep the module path `tests::<name>` that the body diff falls back on. A test split out of a file that stays pairs by its unique `(module path, kind, name)`. Use `[files]` only where git's 30% pairing fails.
  - **Literals:** client.rs:737-758 uses polymarket.com host literals; a scoped `[literals]` covers them.
  - **Features:** each moved integration target declares `required-features`. Clear them with `[gates]` entries; see the tool task below.
- **Body-diff rename behaviour** (`scripts/test_body_diff.py`): a row whose last segment changes (`polymarket` → `shared`) also renames a bare `polymarket::` path head in baseline files that name core. HEAD code must therefore write `shared::clob_limits()` where the baseline wrote `polymarket::clob_limits()`. `[gates]` drops only `cfg(feature = "f")` and a top-level `all(...)` operand, at :1643.
- **`scripts/gen_registry.py`**:
  - `CRATE_KEYS` (:102), with unknown keys refused (:296);
  - `venue` requires `products` (:336-338); relax this only for a crate named `polyoxide-<venue>`;
  - the (venue, product) pair is unique (:434);
  - the `notes` line of core goes stale.
- **`.github/scripts/tests/`**:
  - `test_ci_workflow.py:101-129` `http_modules`, which would demand a `tests/headers.rs` of a crate with a venue and no products;
  - `test_classify_coverage.py:228-229`, where core's row loses `BurstCapacityExceeded` and a polymarket row gains it;
  - `test_mutants_ledger.py` SNIPPETS (:28-32, :54, :64-70);
  - `test_gen_registry.py`'s `manifest(**meta)` fixture.
- **docs/MUTANTS.md rows to re-cite:**
  - (a) send_loop.rs:299, :580;
  - (c) polymarket_limits.rs:1035;
  - (d) polymarket_limits.rs:20;
  - (f) send_loop.rs:252;
  - (g) send_loop.rs:278, :322;
  - (i) signer_limit.rs:276, :523, :532, :548, :570, and its rule line;
  - the note at :99.
- **CAP-7 dry run today:** 518 hits, 245 of them in files that move.
  - **What is left:**
    - core: 81 (Cargo.toml:9-10, README ×16, the docs in lib.rs, rate_limit.rs, client.rs and keychain.rs, and rustdoc links rate_limit.rs:4, :116 and lib.rs:9 that will break);
    - venue: 6 (Cargo keywords, README.md:10, status.rs:22, ws_table.rs:43, class.rs:226);
    - ws: 186 (Cargo keywords include `polymarket`, plus 86 comment hits in src).
  - **Needing exceptions:** ws's protected test names `request.rs` `mod perps`/`mod binance`/`perps_*`/`binance_*`, `test_server/tests.rs` `as_*_will_use_it`/`struct Perps`, and example close_reply_probe.rs:49.
  - The scanner prototype is `<scratchpad>/cap7.py` (`/tmp/claude-1000/-tb-Source-DilettanteTrading-polyoxide--loom-worktrees-aidanb-restructure-3-18ddc6b32f684e0e/3cc99012-d044-4b36-b477-f37d9f3f0223/scratchpad/`), with inventories `inv-5-2-core.md` and `inv-5-2-crate.md` beside it.
- **Docs naming core's paths:** core README:20 and :64-72 (a doctest), relay README:227, relay lib.rs:46, :78 and :89, clob lib.rs:113. Also docs/s1-migration.md and s1-removals.md; these are historical, so leave them.

## Tasks & Acceptance

**Working rule.** Commit locally in logical steps as you go, and never push:
1. the crate and the move;
2. the repointed crates;
3. the moved tests;
4. the gate;
5. the tool and the registry;
6. the docs.

Use conventional messages ending `(Story 5.2)` and the repository's attribution lines:

```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01BjnNeirn7DAPvQNrs5khCr
```

Then a lost session loses nothing. Build with `CARGO_INCREMENTAL=0`. A rustc killed with signal 15 or exit 254 is earlyoom; re-run it. Run cargo-hack alone.

**Execution:**
- [ ] `polyoxide-polymarket/` (new)
  - **Manifest:**
    - Cargo.toml: description, licence, `[package.metadata.polyoxide]` with `venue = "polymarket"`, `readme`, `identifiers = ["Poly-RateLimit"]` and `gate_exceptions`;
    - docs.rs `all-features`;
    - features `clob`, `gamma`, `data`, `relay` and `perps`, gating optional `polyoxide-core`, `hmac`, `sha2` and `base64`.
  - **Source:**
    - `src/lib.rs`, which carries the gated `Classify` assertion;
    - `src/shared/{mod,limits,throttle,policy,signer_limit,session_signer,auth}.rs`, holding the moved code. `shared` re-exports what core's `polymarket` and root re-exports offered.
  - **Docs:** a README that is a doctest.
- [ ] Root `Cargo.toml`, `Cargo.lock`: add the member and the `polyoxide-polymarket` entry in `[workspace.dependencies]`.
- [ ] `polyoxide-core`: delete the moved files, the re-exports and the `polymarket` module. Drop `base64`, `hmac` and `sha2`. Scrub every venue word from src, README and Cargo.toml, rewriting the doc links it breaks and the keychain doc examples.
- [ ] `polyoxide-{gamma,data,perps,relay,clob}`: depend on `polyoxide-polymarket` with their feature, repoint every import and doc path at `polyoxide_polymarket::shared`, and keep their public re-exports.
- [ ] Moved tests (see Code Map): into `polyoxide-polymarket/tests/` (with `[[test]] required-features`) or into `shared` inline modules. Core's remaining tests stay unchanged.
- [ ] `polyoxide-{venue,ws}`: scrub venue words from comments, READMEs and Cargo keywords.
- [ ] `scripts/gen_registry.py` and `test_gen_registry.py`:
  - add `identifiers` (strings) and `gate_exceptions` (a list of `{path, term, reason}`, with the path in a foundation crate and a non-empty reason);
  - both keys require `venue`;
  - allow `venue` without `products` for a crate named `polyoxide-<venue>`;
  - update core's `notes`;
  - run `--write`.
- [ ] `scripts/venue_gate.py` (new), `.github/scripts/tests/test_venue_gate.py` (new) and `ci/venue-gate-skip.txt` (new, holding `data` and `events`): implement the gate described in Boundaries, with unit tests on synthetic trees for every matrix row and a real-tree test. It reads terms from cargo metadata. The `gate_exceptions` for Binance's words go in `polyoxide-binance`'s metadata.
- [ ] `scripts/test_body_diff.py` and `test_test_body_diff.py`: let a `[gates]` value also be a full cfg predicate string, such as `'any(feature = "clob", feature = "relay")'`, which drops exactly that `cfg(...)`. It must fire, like every other entry. Document it in `ci/test-body-diff.toml`.
- [ ] `ci/test-body-diff.toml`: the `[files]`, `[literals]`, `[paths]` and `[gates]` entries the moves need, each one firing.
- [ ] `docs/rename-manifest.md`: rows for every gone core path, then `api_removals.py manifest --write`.
- [ ] `docs/MUTANTS.md`, `test_mutants_ledger.py`, `test_classify_coverage.py` and `test_ci_workflow.py`: re-cite the moved rows and re-prove each mutant. Move `BurstCapacityExceeded` to a polymarket row. Have `http_modules` require products.
- [ ] `CLAUDE.md`: rewrite each rule that names a moved path, in its own section: the rate-limit table's modules, the signer layer, the retry policies, the error hierarchy, the "One send loop" paragraph and the crate list via `--write`. Add the CAP-7 gate to the scripts job description.
- [ ] `deferred-work.md`: the `debug!` target moving from `polyoxide_core::signer_limit` to `polyoxide_polymarket::shared::signer_limit`, for 5.12's notes.

**Acceptance Criteria:**
- Given the moved tree, when `cargo clippy --all-targets --all-features -- -D warnings`, `cargo nextest run --all-features --workspace`, `cargo test --doc` and `RUSTDOCFLAGS=-D warnings cargo doc` run, then all pass. And the per-suite test count over the workspace equals the baseline's: the same number of tests, now partly in `polyoxide-polymarket`.
- Given the same tree, when `test_body_diff.py --baseline v0.39.0` runs, then it reports 0 diffs and 0 lost, and every config entry fires.
- Given the same tree, when `api_removals.py manifest --baseline v0.39.0 --strict` runs, then it exits 0.
- Given the same tree, when `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` runs, then it passes. And `cargo tree -p polyoxide-core -e normal` lists no `hmac`, `sha2` or `base64`.
- Given the scripts job, when its pytest runs, then it is green, including the real-tree CAP-7 test, whose findings name `clob_default`, `PERPS_GENERAL` and `Poly-RateLimit` when run on `v0.39.0`'s core.

## Spec Change Log

## Design Notes

**Where things go.** `shared` keeps core's names, so each path maps one to one:
- `polyoxide_core::polymarket::X` → `polyoxide_polymarket::shared::X`;
- `polyoxide_core::{signer_limit,session_signer,auth}::X` → `polyoxide_polymarket::shared::{…}::X`;
- `polyoxide_core::Tier`, and core's other root re-exports → `polyoxide_polymarket::shared::Tier` and so on.

**Gating.**
- `limits`, `policy` and `throttle` take the union of their users: the limit tables are `any(clob, gamma, data, relay, perps)`, and each table is gated by its own feature.
- The signer layer and `throttle` are `clob`.
- `auth` and `session_signer` are `any(clob, relay)`.

**The gate's exception shape:**

```toml
[[package.metadata.polyoxide.gate_exceptions]]
path = "polyoxide-ws/src/request.rs"
term = "binance"
reason = "protected test names from Story 4.x (mod binance, binance_*), which S2 may not rename"
```

## Verification

**Commands:**
- `CARGO_INCREMENTAL=0 cargo clippy --all-targets --all-features -- -D warnings` -- expected: clean
- `CARGO_INCREMENTAL=0 cargo nextest run --all-features --workspace` -- expected: all pass, count unchanged against `b7b3fed`
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features --workspace` -- expected: clean
- `cargo hack check --workspace --each-feature --no-dev-deps --ignore-private` -- expected: pass (run alone)
- `python3 scripts/test_body_diff.py --baseline v0.39.0` -- expected: 0 diffs, 0 lost, exit 0
- `python3 scripts/api_removals.py manifest --baseline v0.39.0 --strict --no-semver-checks` -- expected: exit 0
- `cd .github/scripts && uv run pytest tests/ -q` -- expected: all pass
- `python3 scripts/gen_registry.py --write && git diff --exit-code -- ':!*.rs'` after commit -- expected: no further change
