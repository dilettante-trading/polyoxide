---
review: release-staging feasibility (ad-hoc lens)
target: ../ARCHITECTURE-SPINE.md
focus: AD-16 (stages), AD-12 (test custody), AD-13 (registration, publish order), AD-21 (CLAUDE.md), AD-22 (CI gates)
inputs:
  - ../ARCHITECTURE-SPINE.md
  - ../.memlog.md
  - ../../../../specs/spec-venue-extensibility/SPEC.md
  - ../../../../specs/spec-venue-extensibility/divergences.md
  - ../../../../specs/spec-venue-extensibility/duplication-inventory.md
  - ../../../../specs/spec-venue-extensibility/registration-points.md
  - .github/workflows/{release,ci,nightly-behavioral}.yml
  - .github/scripts/{classify_failures.py,tests/test_changelog.py}
  - scripts/finish_release.sh
  - every crate's Cargo.toml and src/lib.rs
  - /tb/Source/DilettanteTrading/prader-rs (read-only, for the consumer's actual import surface)
  - crates.io API and `gh run list` (read-only, 2026-10-08)
date: 2026-10-08
---

# Release-staging feasibility review

**Verdict: deliverable, but not as written.** S3 is additive, with one caveat. S2 can be
atomic on `main` but not on crates.io, and it needs a CI path and a rename manifest that the
spine does not give it. S1 cannot keep its promise as worded. "Public paths do not change"
conflicts with five ADs that the spine schedules for S1 (AD-3, AD-8, AD-11, AD-15 and
AD-18). Each of those moves or deletes items that are `pub` today, and the consumer imports
some of them. The release mechanics can carry the new crates. The publish path, though, has
never packaged a crate before release time, and its recovery script cannot resume a partial
publish. The stale-baseline hazard is live on this branch today.

Code facts were checked on this worktree (`e3d8c3e`, 0.37.1), on `origin/main` (`fcadc90`,
0.38.0), on crates.io and in prader-rs (`8cfaaa3e7`).

---

## Answers to the six questions

### Q1. Which public items would S1 have to move or rename anyway?

S1 implements these ADs, and each one touches items that are `pub` today:

| S1 work | Public item today | Evidence | Used by prader? |
|---|---|---|---|
| AD-3: "No other crate defines" `UnknownVariant` or the `Retry-After` parser | `polyoxide_perps::types::UnknownVariant`; `polyoxide_binance::usdm::types::UnknownVariant`; `polyoxide_core::retry_after_header` (pub fn, re-exported at `polyoxide-core/src/lib.rs` `pub use client::{retry_after_header, …}`) | perps `types.rs:31`; binance `usdm/types.rs:85`; core `client.rs:14` | no |
| AD-8: one send loop | `HttpClient::{acquire_rate_limit, acquire_concurrency, should_retry, note_rate_limited}` (pub methods that exist to build hand-rolled loops); `polyoxide_clob::request` (pub mod); `polyoxide_binance::usdm::request::WeightedRequest` (re-exported at `usdm/mod.rs:19`); `RetryConfig` and `with_retry_config` on core, clob and binance builders | core `client.rs:78,89,126,171,304`; binance `usdm/mod.rs:118` | **yes**: `polyoxide_core::RetryConfig` (3 sites: `prader-app-core/src/perps/binance/source.rs:114`, `prader-server/src/infra/clob_executor.rs:332,1947`) |
| AD-10: `HttpClient` holds one `Arc<DynThrottle>` | `HttpClientBuilder::with_rate_limiter(RateLimiter)` | core `client.rs:298` | no (a builder knob, so it may change in S1) |
| AD-11: perps and binance onto `Supervisor` | `polyoxide_perps::ws::{supervised::*, MembershipHandle, PerpsWsBuilder, SupervisedPerpsWs, Recovery, Event}`; `polyoxide_binance::usdm::ws::{supervised::*, MembershipHandle, UsdmWsBuilder, SupervisedUsdmWs, DisconnectReason, Event, Recovery}` | perps `ws/mod.rs:17,24,27`; binance `usdm/ws/mod.rs:21,29,35` | **yes**: `MembershipHandle`, `PerpsWsBuilder`, `Recovery::{Retry,SkipFrame,Fatal}` (`prader-app-core/src/perps/polymarket/stream.rs:26,113-121`); `UsdmWsBuilder`, `Recovery`, `StreamPath` (`…/binance/stream.rs:27,160`) |
| W13: one scripted server in `polyoxide-ws` (spine "Shared-code homes", structural seed) | `#[doc(hidden)] pub mod test_server` in perps, binance, sports and rtds; `fixtures` in binance, sports and rtds | perps `ws/mod.rs:18-20`; binance `usdm/ws/mod.rs:17-24`; rtds `lib.rs:30-44` | **yes**: `polyoxide_perps::ws::test_server::{Script, ScriptedServer}` (2 files); `polyoxide_binance::usdm::ws::test_server::{Script, ScriptedServer}`; `polyoxide_rtds::fixtures` with feature `test-fixtures` (`prader-lock/Cargo.toml:36`) |
| AD-15: one classified enum per module per tier, no catch-all; AD-21 retires `impl_api_error_conversions!` in S1 | `#[macro_export] impl_api_error_conversions!` (path `polyoxide_core::impl_api_error_conversions`); the type names `polyoxide_clob::ws::WebSocketError` (convention says `ClobWsError`), `polyoxide_binance::BinanceError` (crate-wide name for the usdm HTTP tier), `polyoxide_perps::VenueError`, and the umbrella's `polyoxide::PolymarketError` (a catch-all) | core `macros.rs:33`; clob `ws/mod.rs:126`; perps `lib.rs:31`; umbrella `lib.rs:140` | **yes**: `polyoxide::prelude::ws::WebSocketError` (2), `BinanceError` (2), `ApiError` variants (~25 match sites), `RelayError::Core`, the never-constructed variant that R7 deletes |
| AD-18: only core depends on reqwest | `polyoxide_clob::…::send_raw(self) -> Result<reqwest::Response, ClobError>`; `DataApiError::from_response(reqwest::Response)` | clob `api/orders.rs:384` | `from_response`: yes |

Variant and field changes to error types are allowed by S1's rule. **Renames and removals
of the items above are not**, and the spine names no mechanism for keeping them: no interim
re-export, no API-diff gate, and no statement on whether `#[doc(hidden)]` test-server modules
count. See F1.

### Q2. rtds and sports gaining `polyoxide-ws` and `polyoxide-venue`

- **The paths are compatible.** Adding a dependency removes no path. Types that rtds and
  sports expose from tungstenite stay the same type as long as `polyoxide-ws` re-exports the
  same tokio-tungstenite 0.26.
- **prader's published graph** gains `polyoxide-ws`, `polyoxide-venue` and `dynosaur`
  (proc-macro; its syn and quote are already present) in `Cargo.lock`. Sports also gains
  `rust_decimal`. prader does not use sports, and it has no `deny.toml` and no
  `cargo-deny`/`udeps` step, so nothing fails. The claim in prader's `Cargo.toml:164-168`
  comment that rtds "depends on nothing else in polyoxide" goes stale. Its point about the
  signing stack still holds: there is no core, reqwest or alloy.
- **Caveat:** prader-lock builds `polyoxide-rtds` with `test-fixtures`. If rtds's scripted
  server moves to `polyoxide-ws` in S1, `test-fixtures` must keep compiling and keep exposing
  `polyoxide_rtds::fixtures` until R6 renames it in S2.
- **The AD-2 allowlist check is written for the S2 layout** ("`-F rtds` and `-F sports`
  alone"). S1 needs `cargo tree -p polyoxide-rtds -e normal` and the same for sports (F10).

### Q3. S2 for prader, and what cannot be atomic

- **prader's migration** covers about **495 path references in about 116 `.rs` files**
  (`polyoxide_{gamma,data,clob,relay,perps,rtds,sports}::` and `polyoxide::`). It also covers
  eight `[workspace.dependencies]` pins collapsing to `polyoxide-polymarket` plus
  `polyoxide-binance`, `polyoxide-core` and the umbrella, and these feature rewirings:
  - `specta` on gamma and data, for the Tauri bindings;
  - `ws` on clob, which becomes `clob-ws`;
  - `test-server` on perps and binance;
  - `test-fixtures` on rtds, which becomes `test-server`;
  - prader-core's `upstream` and `perps` features, which list `dep:polyoxide-clob`,
    `dep:polyoxide-relay` and `dep:polyoxide-perps`.

  Most of it is a mechanical `polyoxide_X::` → `polyoxide_polymarket::X::` rewrite. The parts
  that are not:
  - `polyoxide_core::SessionSignerScope` leaves core;
  - error type renames under AD-15;
  - inner path changes wherever the `src/<module>/{api,ws,types,error}` layout flattens
    today's modules;
  - the umbrella's `prelude` and `Polymarket` struct.

  prader cannot migrate incrementally. Two `polyoxide-core` majors in one graph give two
  incompatible `ApiError` types, which prader-core's `upstream.rs` matches on, so the
  migration is one PR, as "no shims" implies.
- **Atomic on `main`:** yes, given the loom integration session. That holds only if the
  session's branch actually runs CI (F3).
- **Not atomic on crates.io:**
  - Each release publishes crate by crate. That is not new, but S2 adds a first-time crate
    (`polyoxide-polymarket`) to the run.
  - Nothing tells users of the seven retired crates where the code went. crates.io and
    docs.rs keep showing 0.38 or 0.39 as the latest version with full docs. A 0.x caret
    requirement never moves to the S2 minor, so their builds keep working and they get no
    signal.
  - A final **tombstone** release is not a shim if it carries no re-exports. It is the only
    way to put "moved" on those pages (F5).

### Q4. Is S3 additive?

Yes, with one caveat and one technicality.

- **Caveat:** the clob `Supervisor` (CAP-12) is additive only if it is a **new** type
  alongside `WebSocketBuilder` and `WebSocketWithPing`. Replacing those, or changing their
  stream item to carry `Disconnected`/`Reconnected`, is a breaking change in S3, against
  AD-16's "every rename in S2".
- **Technicality:** under RFC 1105, adding trait impls to existing client types is a "minor"
  change, because method resolution can become ambiguous when a consumer's own trait with
  the same method name is in scope. prader has **no** trait impls on polyoxide types and
  **no** glob imports, so this is safe for the known consumer.
- The Kalshi skeleton's publish status is unspecified (F9).

### Q5. Release mechanics

- **What already works.** release.yml has published brand-new crates before:
  `polyoxide-binance` was created on crates.io at 2026-10-07 15:56 by Release run
  2026-10-07 15:48 (`4fe4490`, v0.37.0). So the token's endpoint scope includes
  `publish-new`, which mostly settles AD-16's "S1 checks the token" item. The single owner on
  every crate is the user `aidan-bailey`, with no team to add to new crates. Every planned
  name (`polyoxide-venue`, `-ws`, `-cli`, `-polymarket`, `-kalshi`, `-test-support`) is
  **unclaimed** on crates.io as of 2026-10-08.
- **What must change:**
  - The hand-written `CRATES` list at `release.yml:83` must gain `polyoxide-venue` before
    core, `polyoxide-ws` before every socket crate, and `polyoxide-cli` last. The
    publish-order script replaces the list, as AD-16 orders.
  - `finish_release.sh` must call the same script, and it must become resumable (F4).
- **What can bite:**
  - Nothing in CI ever packages a crate: release.yml uses `--no-verify`, so new-crate
    manifest errors first surface mid-release.
  - crates.io limits **new crates** to a burst of about 5, then 1 per 10 minutes. The retry
    loop gives up after 6 × 5 s.
  - `cargo search` is a fuzzy search used as a version probe.
  - Cargo itself now orders and waits: `cargo publish --workspace` uploads in dependency
    order and polls the index between uploads. The spine's `publish_order.py` can shrink to
    "which crates are not yet published" (F4).
- **Index propagation** is handled. Cargo polls the index for up to 60 s after each upload,
  and the existing 6 × 5 s retry covers the rest. The new crates do not change that.

### Q6. Red doc build and stale baseline

- **Stale baseline: live now.** This worktree is at 0.37.1 (`e3d8c3e`). `origin/main` is at
  0.38.0 (`fcadc90`, `feat(gamma)!`, released and on crates.io at 2026-10-08 08:18). The
  spine's audit commit, the task brief ("lockstep 0.37.1 today") and prader's pins all
  predate it. A long-lived S2 integration session multiplies the risk (F2).
- **Red build risks:**
  - S1 adds five new CI gates. Each one withholds the release silently when red, and only if
    it is a job in `ci.yml` (F7).
  - S2 moves every README doctest and creates many `pub(crate)` items in `src/shared/`,
    which is the rustdoc `private_intra_doc_links` trap. It does this on a branch that CI
    does not see (F3).

---

## Findings

### F1 — high — S1's path freeze contradicts five ADs scheduled for S1, and nothing enforces it

**Evidence.**
- AD-16 S1: "Error and builder types may change; public paths do not."
- The same stage carries:
  - AD-3: `UnknownVariant` and the `Retry-After` parser defined nowhere but `polyoxide-venue`;
  - AD-8: the only loop;
  - AD-11: perps and binance onto `Supervisor`;
  - AD-15: per-module-per-tier enums, no catch-all, and AD-21's retirement of
    `impl_api_error_conversions!` in S1;
  - AD-18: no direct reqwest in venue crates.

  The Q1 table lists the `pub` items each one moves or deletes.
- prader imports several of them:
  - `RetryConfig`;
  - perps `MembershipHandle` and `Recovery::Retry`;
  - binance `Recovery` and `UsdmWsBuilder`;
  - **the doc-hidden `test_server::{Script, ScriptedServer}` of both perps and binance** (3
    test modules);
  - `polyoxide_rtds::fixtures` behind `test-fixtures`.
- No AD says whether `#[doc(hidden)]` items count as public paths. Parallel epic agents will
  answer that differently. One deletes `polyoxide_perps::ws::supervised` along with the old
  loop (AD-12 orders the old loops deleted). Another keeps it.

**Fix.** Add an S1 path-freeze rule to AD-16 and gate it:
1. In S1 every existing `pub` path keeps resolving. Where the single definition moves, the
   old path becomes a `pub use` of the new one. Examples: `polyoxide_perps::types::UnknownVariant`
   becomes `pub use polyoxide_venue::UnknownVariant`, and `polyoxide_perps::ws::SupervisedPerpsWs`
   becomes a type alias over `Supervisor<PerpsProtocol>` with its inherent methods kept.
   These interim re-exports are deleted in S2 with every other rename. They are not "shims"
   in the spec's sense, because no release after S2 carries them. Say so explicitly, or the
   "no shims" constraint will be read as forbidding them.
2. `#[doc(hidden)]` test-server modules and the `test-fixtures` feature are covered by the
   freeze. They keep their names and their `Script`/`ScriptedServer` entry points as thin
   venue-local wrappers over `polyoxide-ws`'s server until S2.
3. Type **renames** under AD-15 (`WebSocketError` → `ClobWsError`, `BinanceError` →
   `UsdmError` if intended, `PolymarketError`'s fate) wait for S2. S1 reshapes variants only.
   `impl_api_error_conversions!` stays exported, even if unused, until S2.
4. `RetryConfig` and `with_retry_config` survive S1 as the knobs of the default
   `RetryPolicy`, since they are the only retry control prader uses.
5. Gate it: a CI job runs `cargo public-api diff` against the last release tag for each
   crate, or `cargo semver-checks` with only the "item removed / path changed" lints set to
   deny. During S1 it fails on any removed path. During the S2 integration session it is
   switched to report-only, and it feeds the rename manifest (F5).

### F2 — high — The stale-baseline hazard is live on this branch; a long S2 session compounds it

**Evidence.**
- This worktree is at `e3d8c3e` (0.37.1). `git ls-remote` shows `refs/heads/main` and
  `refs/tags/v0.38.0` at `fcadc90`. crates.io lists `polyoxide`, `polyoxide-core` and
  `polyoxide-rtds` at max version 0.38.0, published 2026-10-08 08:18. 0.38.0 is a
  `feat(gamma)!` with 20 files and ~1,100 lines changed under `polyoxide-gamma` and
  `polyoxide-sports`.
- The duplication inventory is "the 2026-10-08 audit of commit `e3d8c3e`", so its gamma line
  references may already be off. A bump from this baseline reuses 0.38.0. Then
  `release.yml:53` (`gh release view`) sets `should_release=false`, and **every publish job
  is skipped with no failing check**. The memory note `polyoxide-release-workflow` records
  the same thing happening on 2026-09-07.
- AD-16 makes S2 one loom integration session that merges to `main` "once complete", while
  "S1 patches can ship". Any version bump committed on that branch races `main`'s releases.

**Fix.**
1. Merge `origin/main` into `aidanb/restructure` before any S1 epic branches from it, and
   re-run the inventory's gamma rows.
2. Add to AD-16: no version-bump commit on any loom or integration branch. The bump is a
   separate commit on `main` after `git fetch` and a crates.io check, and it is the last
   commit before the tag.
3. Make the silent skip loud. In release.yml's `check` step, if
   `git diff HEAD^ HEAD -- Cargo.toml` changes the `[workspace.package] version` line **and**
   the tag already exists at a different SHA, fail with `::error::` instead of setting
   `should_release=false`. Ordinary non-release commits do not change that line, so they
   still skip quietly.

### F3 — medium — The S2 integration branch gets no CI, so the merge is the first time the renamed tree meets CI

**Evidence.**
- `ci.yml` triggers only on `push: branches: [main]` and `pull_request: branches: [main]`.
  Epic merges into a loom integration branch therefore run **no** CI. They run only local
  gates, and the memory note `polyoxide-ci-gates` records that local Rust is 1.95 while CI
  floats on stable 1.99, with new lints that "only fail on main".
- S2 is where the doc-gate risks concentrate:
  - every README doctest changes paths;
  - module READMEs become `cfg(all(doctest, feature = "<module>"))` includes;
  - `src/shared/` creates many `pub(crate)` items that `pub` docs must not link.
- `nightly-behavioral.yml` runs against `main` only. Its rows change shape in S2 (per crate
  becomes per module), so the derived rows and renamed live targets are first exercised after
  the merge.

**Fix.**
1. Open a **draft PR** from the integration branch to `main` when the session starts.
   `pull_request` `synchronize` then runs the full CI on every epic merge. Alternatively,
   add the integration branch pattern to `ci.yml`'s `push` trigger.
2. Before merging, dispatch `nightly-behavioral` (it has `workflow_dispatch`) on the
   integration ref.
3. Bump the version only after CI is green on the merge commit on `main` (see F2.2).

### F4 — medium — The publish path never packages before release, and the recovery script cannot resume

**Evidence.**
- `release.yml:94` runs `cargo publish -p "$crate" --no-verify`, and no CI job packages. A
  new crate's manifest faults surface only mid-release, after earlier crates of the same
  version are already up. Examples:
  - a missing `version` on a `[workspace.dependencies]` pin;
  - `polyoxide-test-support` added as a *normal* path-only dependency by mistake, which
    crates.io refuses;
  - a missing `description` or `license`.
- crates.io limits new crates to a burst of about 5, then one per 10 minutes. A refusal
  there is retried 6 × 5 s and then the release fails part-published. S1 adds three new
  crates (`polyoxide-venue`, `polyoxide-ws`, and `polyoxide-cli`, which has never been on
  crates.io). S2 adds one and S3 possibly one. That is within the burst, but only if no
  release adds more than five.
- `scripts/finish_release.sh` is `set -e` and starts again at `cargo publish -p
  polyoxide-core` (line 10). After a partial release, its first command fails with "already
  exists" and the script exits. It cannot finish the release it exists to finish.
- `release.yml:86` probes versions with `cargo search "$crate" --limit 1`. That is a fuzzy
  ranking that only works because of the `|| echo "0.0.0"` fallback and the "already
  exists" grep.
- Cargo already publishes multi-crate workspaces in dependency order and polls the index
  between uploads (`cargo publish --workspace`; `cargo_publish.rs` `take_ready` and
  `wait_for_any_publish_confirmation`, 60 s default). The workspace's MSRV of 1.91 and the
  stable toolchain on the release runner both have it.

**Fix.**
1. Add a CI step, `cargo publish --workspace --dry-run` or `cargo package --workspace`.
   Packaging inter-dependent unpublished crates resolves them through a local overlay, so it
   works before `polyoxide-venue` exists on crates.io. It catches every manifest fault above
   on the PR that introduces it. Confirm on the current stable that `--workspace` skips
   `publish = false` members (`polyoxide-py`, `polyoxide-test-support`). If it does not,
   pass `--exclude` for them.
2. Reduce `publish_order.py` to two outputs. One is the not-yet-published set, queried from
   `https://crates.io/api/v1/crates/<name>/<version>` rather than `cargo search`. The other is
   a CI check that the derived order matches `cargo metadata`. Both `release.yml` and
   `finish_release.sh` then run `cargo publish -p <each unpublished crate>` and let cargo
   order and wait. That makes both paths resumable.
3. AD-16's "S1 checks the token" item is mostly settled by the polyoxide-binance first
   publish on 2026-10-07. What remains is to read the token's **crate scope** on crates.io:
   a `polyoxide*` pattern, not an explicit list that happened to include `polyoxide-binance`,
   plus its expiry. Keep no more than five new crates per release.
4. For ordering, ignore only *path-only* dev-dependencies, as AD-13 says. Versioned
   dev-dependencies (clob → relay today, cli → sports/binance) still order the publish, and a
   versioned dev-dependency cycle would make the topological sort fail. Have the CI check
   reject such a cycle.

### F5 — medium — S2 has no rename manifest and no plan for the retired crates

**Evidence.**
- AD-16 S2 lists "every public rename or move" by area only. Gaps a parallel agent will fill
  inconsistently:
  - **Features.** The conventions name `<module>`, `<module>-ws` and one `test-server`, but
    say nothing of `specta`, which gamma and data have and which **prader uses** in four
    manifests. Nor do they cover `keychain` (core, clob, relay) or `parquet` (cli). A single
    `test-server` on `polyoxide-polymarket` must be additive (`cfg(all(feature =
    "test-server", feature = "perps-ws"))`). Otherwise prader-lock, which wants only rtds
    fixtures, builds clob and alloy in its tests.
  - **Binance.** Nothing says whether `BinanceError` becomes `UsdmError` (AD-15's
    "no venue-wide catch-all") or whether `weight::WeightBudget` moves under `usdm`.
    Anything undecided in S2 becomes a breaking change in S3 or later.
  - **The umbrella.** `polyoxide::Polymarket`, `PolymarketBuilder`, `PolymarketError` (a
    catch-all under AD-15) and `prelude`. prader uses `polyoxide::prelude::ws::*` and
    `prelude::Gamma`.
  - **docs.rs.** `polyoxide-polymarket` needs
    `[package.metadata.docs.rs] features = [<every module, every -ws>]`, without
    `test-server`. Otherwise docs.rs shows only clob, gamma and data, and the perps, relay,
    rtds and sports docs vanish. Binance hit this exact gap (commit `4aefa5e`,
    "docs.rs shows the streams").
- **Retired crates.** `polyoxide-{clob,gamma,data,relay,perps,rtds,sports}` simply stop
  receiving versions. Their crates.io and docs.rs pages keep advertising the last pre-S2
  version with no pointer.

**Fix.**
1. Add a **rename manifest** artifact to S2: old path → new path, old feature → new feature,
   deleted item and reason. Generate it from the F1.5 public-API diff and check it in. It is
   both the S2 definition of done and prader's migration guide.
2. **Tombstones.** Keep the seven retired crate directories as workspace members for exactly
   the S2 release. Each `lib.rs` has no dependencies and no re-exports, so it is not a shim:
   a crate doc plus `compile_error!("polyoxide-gamma moved to polyoxide-polymarket (feature
   `gamma`); see <manifest URL>")`, and a README saying the same. Publish them at the S2
   version. A 0.x caret requirement never resolves to them, so nobody's build breaks, but
   the crates.io and docs.rs pages now point the way. They are updates, not new crates, so
   no new-crate rate limit applies. Delete the directories in the following release.
   **Never yank** the old versions.

### F6 — medium — prader migrates in S1 too; AD-16's promise covers imports only

**Evidence.**
- "Error and builder types may change" in S1. prader matches `polyoxide_core::ApiError`
  variants (`Api`, `Authentication`, `Timeout`, `Validation`, `Network`, `RateLimit`) in
  about 25 places. It also matches `RelayError::Core` (which R7 deletes), perps and binance
  `Recovery`, and builds `RetryConfig` literals.
- AD-8 reshapes `ApiError` ("carrying status, headers, body and the parsed `Retry-After`"),
  and AD-15 adds `#[non_exhaustive]`. Both force edits to prader's `upstream.rs` and stream
  adapters in S1. The memlog also has releases as deliberate bumps with "epics merge to main
  whenever green", so S1 may span several minors, and prader edits its call sites at each
  one.

**Fix.**
- State in AD-16 that prader migrates **call sites** in S1 and **imports** in S2, and nothing
  in S3 if F9's caveat holds.
- Cut S1 as one release, or as few as practical, after the error reshape (AD-8 plus AD-15)
  is complete, so prader adapts its error handling once.
- Put a short "consumer impact" section in the release notes per stage. AD-16 already
  requires naming R1, R2 and R4 and the log targets.

### F7 — medium — The new CI gates withhold the release only if they live in `ci.yml`

**Evidence.**
- `release.yml:5` listens to `workflows: [CI]` only. AD-22 adds five gates and says "A red
  check withholds the release tag, as every CI check does". If any gate is added as its own
  workflow (an `msrv.yml`, say), it neither gates the release nor stops a bad tree from
  publishing.
- AD-22's MSRV job runs `cargo +1.91 … doc`. Under `RUSTDOCFLAGS=-D warnings` that is a
  second rustdoc toolchain whose lint set differs from floating stable's. It adds a
  silent-withhold path, and local runs on 1.95 reproduce neither toolchain.

**Fix.**
- Add AD-22's gates as jobs in `ci.yml`.
- The MSRV job runs `cargo +1.91 check --workspace --all-features` and `cargo +1.91 doc`
  **without** `-D warnings`. The deny-warnings doc gate stays on stable only.
- Record in AD-22 that each new gate is one more way for a release to be silently withheld,
  and point to the F2.3 loud-failure check.

### F8 — low-medium — The nightly classifier's regex fallback reads the error spellings that S1 reshapes

**Evidence.** `classify_failures.py` `TRANSIENT_RES` matches Debug and Display spellings:
`\bRateLimit\(`, `Api { status: 5xx`, `\bRateLimited \{`, `binance answered 5xx:`,
`\bConnectTimeout\(` and `Venue { status: …`. AD-8 and AD-15 reshape exactly those enums in
S1. AD-14 keeps the regexes "as a fallback until every live test reports through
test-support". If the reshape lands first, transient failures are filed as `real`. That is
the noisy direction, not a silent one, but it costs nightly issues during S1. The 27
classifier tests use frozen strings, so they stay green.

**Fix.** Order the S1 epics so that the AD-14 tag reporter, and the migration of live tests
onto it, merge **before** any epic that changes an error's Debug or Display. Failing that,
each reshape PR updates `TRANSIENT_RES` and its fixtures with strings captured from the new
types.

### F9 — low — S3 is additive only if the clob `Supervisor` is a new type; the Kalshi skeleton's publish status is unset

**Evidence.**
- The clob socket exports `WebSocket`, `WebSocketBuilder`, `WebSocketWithPing` and
  `MembershipHandle` (`polyoxide-clob/src/ws/mod.rs:125`).
- CAP-12 wants `Disconnected` then `Reconnected` markers. Retrofitting them onto
  `WebSocketWithPing`'s stream item, or replacing that type, breaks callers in S3.
- AD-11's "Prevents per-venue supervision loops" invites deleting the old ping loop.
- The structural seed has a nightly row against the "Kalshi demo host". Nothing says whether
  `polyoxide-kalshi` publishes. If it does, it is a new crate (the name is free today) that
  needs a nightly secret, and its absence must classify as `auth-gated` under AD-14.

**Fix.**
- AD-16 S3: the supervised clob socket is a new type (for example `SupervisedClobWs`, under
  the S2 naming). `WebSocketWithPing` stays until a later breaking release that is named as
  such.
- The Kalshi skeleton is `publish = false` until the Kalshi integration spec. This also keeps
  it out of the new-crate budget and the publish-order script.

### F10 — low — Two S1 checks are written for the S2 layout

**Evidence.**
- AD-2's allowlist is phrased as "`-F rtds` and `-F sports` alone", which are features of
  `polyoxide-polymarket`, a crate that S1 does not have.
- AD-18's cross-venue header build is `polyoxide --features full`. The umbrella's `full`
  excludes Binance until S2 (`polyoxide/Cargo.toml` features; CLAUDE.md "not in the
  umbrella"), so in S1 that build cannot reproduce the 0.37.0 gzip regression class.

**Fix.**
- S1 runs AD-2 as `cargo tree -p polyoxide-rtds -e normal` and the same for
  `polyoxide-sports`.
- S1 runs AD-18's "all venues" build as `cargo test --workspace --all-features`, which
  unifies Binance's reqwest features with everyone else's, beside the per-crate minimal
  build.
- AD-2 and AD-18 each state their S1 and S2 forms.

### F11 — low — Version and pin parsing are regexes that the new manifest shape can slip past

**Evidence.**
- `release.yml:44` takes the version with `grep -m1 '^version' Cargo.toml`, and
  `test_changelog.py:54` takes the first `^version = "…"`. AD-13 adds
  `[workspace.metadata.polyoxide.mirrors]` to the root manifest. A table placed above
  `[workspace.package]` with a `version` key hijacks both.
- `PATH_PIN_RE` (`test_changelog.py:55`) matches only `{ path = "…", version = "…" }`.
  A new pin with any extra key escapes the pin-moved-with-the-bump check, and then fails
  mid-publish. `polyoxide-polymarket` with `default-features = false`, as `polyoxide-py`
  uses for clob today, is one example.

**Fix.** Read both values from `cargo metadata --format-version 1 --no-deps` in the
publish-order script, and have `test_changelog.py` and release.yml use that script or the
same query.

### F12 — low — A path-only `polyoxide-test-support` cannot serve inline unit tests

**Evidence.**
- AD-12 protects "perps inline (17)" supervision tests and core's inline limiter, cooldown
  and `Retry-After` mutation tests. AD-1 makes `polyoxide-test-support` a dev-dependency
  that itself depends on core or venue.
- A `#[cfg(test)]` unit test in crate X that uses a helper crate depending on X links two
  copies of X: the test build and the lib the helper sees. Their types do not unify
  ("expected `polyoxide_core::ApiError`, found `polyoxide_core::ApiError`").
- Moving those tests to `tests/` to use the helper changes their nextest names. AD-12 says
  "A move keeps test names", and its before/after per-suite counts are keyed on names.

**Fix.**
- State in AD-12 that "names" means the test function names, so the binary path may change.
- Keep helpers that inline tests need inside the crate under test, or in `polyoxide-ws`'s
  `test-server` (W15 already lives there), not in `polyoxide-test-support`.

---

## Checked and fine

- **Crate sizes.** The seven Polymarket crates total about 3.2 MB uncompressed, fixtures
  included, well under crates.io's 10 MB package limit for `polyoxide-polymarket`.
- **Path-only dev-dependency on a `publish = false` crate.** It is stripped on publish (as
  the memlog verified), so `polyoxide-test-support` never blocks a release.
- **CLI first publish in S1.** The name is free. The binary is named `polyoxide` with
  `doc = false` already set. Its versioned dev-dependencies (sports and binance with
  `test-server`) are published earlier in the order. Its `description` ("CLI tool for
  querying Polymarket Gamma API") is stale, so fix it before the first crates.io page
  exists. It ships the old command tree once and is broken in S2. That is acceptable and
  worth one line in the S1 notes.
- **Changelog.** `cliff.toml` groups every conventional type and skips only
  `chore(release|deps|pr|pull)`. AD-12's "a DRIFT change is its own commit, naming its row"
  is therefore enough for AD-16's "release notes name DRIFT R1, R2 and R4", provided those
  commits are typed `fix`, `feat` or `refactor` and not `chore`.
- **The nightly tracking issue.** It is keyed on the single `nightly-behavioral` label, not
  on crate names, so S2's renames do not orphan it. `spec:<id>` labels are unchanged
  (AD-13).
- **Python.** The wheel and the crates publish from the same SHA in one workflow, so the
  `polyoxide.polymarket.*` move is atomic for Python users. PyPI uses `--skip-existing`, so
  a partial rerun is safe.
