---
stepsCompleted: ["step-01-validate-prerequisites", "step-02-design-epics", "step-03-create-stories", "step-04-final-validation"]
inputDocuments:
  - _bmad-output/specs/spec-venue-extensibility/SPEC.md
  - _bmad-output/specs/spec-venue-extensibility/glossary.md
  - _bmad-output/specs/spec-venue-extensibility/venue-landscape.md
  - _bmad-output/specs/spec-venue-extensibility/duplication-inventory.md
  - _bmad-output/specs/spec-venue-extensibility/divergences.md
  - _bmad-output/specs/spec-venue-extensibility/registration-points.md
  - _bmad-output/planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md
  - CLAUDE.md
---

# polyoxide - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for polyoxide. It decomposes the requirements of the venue-extensibility spec and the architecture spine into implementable stories. The spec stands in for a PRD, and there is no UX design document.

## Requirements Inventory

### Functional Requirements

FR1 (CAP-1): Every HTTP request in every venue crate goes through one send-and-retry path in `polyoxide-core`, which applies rate-limit feedback. Client configuration, namespaces, query setters and wire enums share one vocabulary. `clob` ping and `gamma` `post_json` are gated, with tests. These mutants each fail a test:
- dropping the hold on a last-attempt 429;
- skipping `observe` on the last attempt;
- a policy returning a zero wait.

No crate defines its own `open_enum!`, `wire_enum!`, `UnknownVariant` or builder knobs.

FR2 (CAP-2): A venue supplies its limiter model through one `Throttle` interface that takes a per-request cost, refuses impossible costs on the client side, and owns the shared hold. `RateLimiter` and `WeightBudget` implement it. The HTTP client names no concrete limiter, and venue limiter tables live in venue crates. A test outside the foundation builds a Kalshi-style token-cost bucket without editing the foundation.

FR3 (CAP-3): The socket building blocks exist once in `polyoxide-ws`:
- TLS provider install;
- backoff;
- connect with timeout;
- handshake and close classification;
- the `Supervisor` task shell;
- per-attempt handshake headers;
- the scripted test server.

Venues inject their ping schedule, liveness rule, backoff-reset input and auth. With only `rtds` or only `sports` enabled, the build contains neither `reqwest` nor `alloy`. Existing supervision suites pass with behaviour unchanged.

FR4 (CAP-4): Every venue error maps to one of eight classes (Network, Unavailable, RateLimited, Unauthorized, InvalidRequest, VenueRefusal, Restricted, Decode) through one shared interface, with derived retriability and fault flags. One `Retry-After` parser and one retriable-status rule serve all venues. This is enforced at compile time. Venue traits return `ClassifiedError`, so a consumer classifier is written once.

FR5 (CAP-5): Consumers read instruments or markets, quotes, books, trades and candles, plus funding for perps. They do so through one base `MarketData` trait and capability traits, with venue-tagged canonical keys, `Option` for unpublished fields, and venue-only data in typed extensions. The traits are implemented for the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M, and one generic best-bid/ask function runs against each.

FR6 (CAP-6): One normalized `Trading` interface covers place, cancel, cancel-all, open-order reconciliation, a lossless event stream of acks and fills, positions and balances. Kill outcomes are terminal statuses (`Killed`), not errors. It is implemented for the Polymarket CLOB. A recorded mapping table maps Kalshi's common order and fill fields to the trait, and lists Kalshi-only concepts as deferred.

FR7 (CAP-7):
- Each venue is one crate with feature-gated modules: `polyoxide-polymarket` holds clob, gamma, data, relay, perps, rtds and sports; `polyoxide-binance` holds usdm.
- The `polyoxide` umbrella is multi-venue.
- The CLI is `polyoxide <venue> <module> <verb…>`, with one streaming runner and one `OutputFormat`.
- Python exposes per-venue submodules.
- README, CLAUDE.md and `docs/specs/INDEX.md` describe a multi-venue toolkit.
- A grep gate finds no venue identifiers in the foundation crates.

FR8 (CAP-8): Adding a venue edits one registration source: Cargo metadata. Publish order, nightly rows, the schema watch list and exclusions, classifier inputs and docs tables derive from it, or CI checks them against it. `release.yml` and `finish_release.sh` share one publish-order script. `polyoxide-cli` publishes to crates.io.

FR9 (CAP-9): These exist once and every crate uses them:
- the spec- and wire-agreement helpers;
- fixture loading;
- the soak harness;
- the capture-script helpers.

The Kalshi skeleton copies none of them.

FR10 (CAP-10): One venue's dependency features cannot change another venue's wire behaviour. A per-module test pins request headers, including `Accept-Encoding`, in a minimal venue-only build and in an all-venues build.

FR11 (CAP-11): A venue-onboarding guide (`docs/ARCHITECTURE.md`) covers layout, registration, the drift-detector pattern, limiter measurement and trait implementation. The Kalshi skeleton is built by following it. Any step it misses is recorded as a spine amendment, and the guide is regenerated.

FR12 (CAP-12): The CLOB market and user sockets are supervised:
- reconnect with backoff;
- staleness detection;
- membership replay, including user-channel credentials;
- a `Disconnected` then `Reconnected` pair per outage.

The 10 s `PING` schedule is unchanged. The supervised socket is a new type.

FR13 (success signal): A Kalshi walking skeleton, `publish = false`, lands with no foundation edits and no copied infrastructure, touching only what AD-13 allows. It provides:
- exchange status;
- one `MarketData` implementation;
- one supervised socket with a signed handshake against Kalshi's demo host.

FR14 (success signal): Every row of `duplication-inventory.md` ends with exactly one definition, in the home the spine assigns.

FR15 (DRIFT R1–R10): The decided fixes are applied, each in its own commit naming its row:
- R1: socket reconnects retry 408/425/429/5xx;
- R2: 10 s connect timeout everywhere;
- R3: RFC 6455 close reply, verified on the wire first;
- R4: one `Retry-After` parser;
- R5: workspace `rustls` with `std`;
- R6: one `test-server` feature name;
- R7: relay on the shared loop with 429 tests;
- R8: the gating holes closed;
- R9: live tests use the shared TLS function;
- R10: one retry log line.

### NonFunctional Requirements

NFR1: Breaking changes ship with no shims or re-exports, in lockstep minor bumps over three stages: S1 internals, S2 renames, and S3 and later additions. prader-rs migrates only from crates.io releases.
NFR2: Enabling only a socket-only module builds only three sets of dependencies: `polyoxide-ws`'s, `polyoxide-venue`'s (rust_decimal, serde, thiserror, futures-core, dynosaur) and the module's own. No HTTP or signing stack is built.
NFR3: Socket handshake auth is computed per connection attempt.
NFR4: Every throttle charge carries a per-request cost, so batch and weighted requests are charged correctly.
NFR5: The DFR divergences in `divergences.md` stay per-venue. Only identical or parametric copies merge, and the branch that carries behaviour stays at the venue's call site.
NFR6: Credential storage is split. Shared code owns keyring access, secret redaction and the plumbing for CLI `credentials`. Each venue owns its credential types.
NFR7: Behaviour-carrying tests move; they are never deleted or weakened. Moves keep test names and assertions, and mutants are re-run after each move.
NFR8: Normalization loses no venue information: absent data is `Option`, ids are opaque, money and size are `Decimal`, and venue-only fields stay reachable.
NFR9: The CI gates hold: clippy `-D warnings`, rustdoc `-D warnings`, fmt, MSRV 1.91 and the new AD-22 gates. A red CI withholds the release tag.
NFR10: The retry `WARN` line `Retriable status <code> on <path>, retry <n> after <ms>ms`, under target prefix `polyoxide_core`, is kept for every venue, because the soak harnesses match it.
NFR11: Each stage's release notes carry a consumer-impact section. They name DRIFT R1, R2 and R4, and any moved log targets.
NFR12: `main` stays releasable after every merge. A version bump is a separate commit on `main` only.

### Additional Requirements

From the architecture spine and the brownfield code. There is no starter template, because this is a brownfield restructure.

**Crates and dependencies (AD-1, AD-2, AD-3, AD-18):**
- Create `polyoxide-venue` (vocabulary), `polyoxide-ws` (socket kit) and `polyoxide-test-support` (`publish = false`, a path-only dev-dependency).
- The dependency edges are exactly those of AD-1.
- `polyoxide-ws` depends on neither `polyoxide-core` nor `polyoxide-venue`.
- Only `polyoxide-core` depends directly on `reqwest`, and only `polyoxide-ws` on `tokio-tungstenite` and `rustls`.
- Per-module allowlist and deny-list checks run on `cargo tree`.
- The workspace uses `resolver = "3"`.

**Send loop and throttles (AD-8, AD-9, AD-10, AD-17, AD-23):**
- The hook contract: `Throttle::acquire` (async) returns a `Charge` or a `Refused`; `Authenticator::sign` (async); `observe`, `decide` and `hold` (sync).
- One `Decision { outcome, hold }`. The loop owns the backoff floor, releases the permit before sleeping, and emits one retry log line.
- The ratified hold behaviour: only 429 and venue bans set a hold; core's 429 hold is `retry_delay(0)`.
- Three bucket models: the window-quota table and the capacity bucket live in core; Binance's weight minute stays in `polyoxide-binance`.
- Polymarket's one `RetryPolicy` retries 429 and 425; core's default retries 429.

**Sockets (AD-11):**
- `Supervisor<P: Protocol>` with `P::Membership` and `wanted`, markers declared by the `Protocol`, pings that never wait on the consumer, and a queue declaration.
- perps and usdm migrate under AD-12's gate. Binance runs one supervisor per routed path.
- sports and rtds use only kit blocks.

**Errors (AD-15):**
- Eight classes, mapped from the status first: 401/403 Unauthorized; 418/451 Restricted; 429 RateLimited; 408/425/5xx Unavailable; any other 4xx VenueRefusal.
- InvalidRequest is client-side only.
- Enums are per module and per tier.
- `impl_ws_classification!` provides one WsError table.

**Traits, keys, records (AD-4–AD-7, AD-19, AD-20, AD-24):**
- dynosaur 0.3.1 with the form `#[dynosaur::dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]`, `Send + Sync` supertraits (never `'static`), and `Unpin` streams.
- Canonical `MarketKey`, with `RawKey::parse` and one constructor module per product.
- Outcome keys on event-contract venues.
- Typed `Extensions`.
- S3 opens with one records story.
- The trading stream is unbounded, with `Resync` after every reconnect.

**Registration and CI (AD-13, AD-14, AD-22, AD-25):**
- `[package.metadata.polyoxide]` holds venue and product ids and `live.<target>`; `[workspace.metadata.polyoxide.mirrors]` holds crate-less mirrors.
- `scripts/gen_registry.py` owns the generated regions.
- `scripts/publish_order.py` lists the unpublished set from crates.io, so a release can resume.
- Tags come from `or_fail`, the credential loaders, `environmental` and `transient`; `scripts/live_unwraps.py` must shrink to zero.
- CI gains these jobs in `ci.yml`: cargo-hack per feature, MSRV, publish dry-run, the removal gate (cargo-semver-checks 0.51.0 plus `scripts/api_removals.py`), the allowlist checks and the header builds.
- The release fails loudly when its version tag already exists.

**Release staging (AD-16):**
- S1 branches from current `origin/main` (at least v0.38.1), and the gamma and sports inventory rows are re-audited there.
- S1 order: the publish-order script first; then the generator; then the tag reporter with the live-test migration; then any error reshape.
- S1 ships a checked-in removal list.
- S2 runs as one loom integration session with a draft PR to `main` and a nightly run dispatched on it, plus a rename manifest from the public-API diff.
- S2 publishes tombstones from `tombstones/<crate>/`, outside the workspace.
- S3 adds a new supervised clob type, and the Kalshi skeleton stays `publish = false`.

**Custody and standing rules (AD-12, AD-21):**
- An open protected-suite list, with per-suite counts reported before and after.
- `docs/MUTANTS.md`, re-mutated after each move, and `scripts/test_body_diff.py` for S2.
- Every DFR row binds every epic.
- `docs/ARCHITECTURE.md` lands in S1, with a CLAUDE.md pointer.
- Superseded CLAUDE.md rules are edited in the same change, through the generator inside generated regions.

**Conventions:** naming, layout, features, test targets, the drift-detector layout, module README doctests, the CLI and Python shape, and the rustdoc private-link rule (spine §Consistency Conventions).

**Operations:**
- Confirm that the crates.io token covers `polyoxide*` and expires after S3.
- At most five new crates per release.
- Kalshi demo credentials for nightly; without them the run is `auth-gated`.
- Dependency pins are kept as they are, except dynosaur, which is added.

### UX Design Requirements

None. polyoxide is a library, CLI and Python bindings with no UI, and there is no UX design contract.

### FR Coverage Map

FR1: Epic 3 - one HTTP send loop, shared client vocabulary, gated ping and post_json, mutation tests
FR2: Epic 3 - Throttle interface with per-request cost, hold, venue tables, out-of-foundation bucket test
FR3: Epic 4 - socket kit and Supervisor in polyoxide-ws; credential-free builds; suites unchanged
FR4: Epic 2 - classification interface over existing enums; Epic 3 - HTTP enums reshaped; Epic 4 - socket enums reshaped
FR5: Epic 6 - MarketData and capability traits, keys, extensions, three implementations
FR6: Epic 7 - Trading trait, lossless events, Killed status, Kalshi mapping table
FR7: Epic 5 - one crate per venue, multi-venue umbrella, CLI and Python on the venue axis, grep gate
FR8: Epic 1 - Cargo-metadata registration, generator, publish-order script; Epic 2 - classifier reads tags; Epic 4 - S1 release puts the CLI on crates.io; Epic 5 - metadata entries move with consolidation
FR9: Epic 2 - polyoxide-test-support helpers and capture_common.py
FR10: Epic 3 - reqwest ownership and per-module header pins in two builds; Epic 4 - tungstenite/rustls ownership; Epic 5 - header pins under polyoxide --features full
FR11: Epic 1 - docs/ARCHITECTURE.md lands with a CLAUDE.md pointer; Epic 8 - validated by the Kalshi skeleton
FR12: Epic 7 - supervised CLOB market and user sockets
FR13: Epic 8 - Kalshi walking skeleton
FR14: Epic 3 - H rows; Epic 4 - W rows; Epic 2 - T rows; Epic 5 - C rows
FR15: Epic 1 - R5; Epic 3 - R4, R7, R8, R10; Epic 4 - R1, R2, R3, R9; Epic 5 - R6

## Epic List

### Epic 1: Reliable releases and one-place registration
Stage S1, first. Releases publish every crate, including `polyoxide-cli` (on crates.io for the first time), resume after a failure, and fail loudly on a reused version. A maintainer registers a crate or live test in that crate's own `Cargo.toml`; the README crate table, `docs/specs/INDEX.md`, CLAUDE.md's lists and the nightly rows are generated. The agent guide lands at `docs/ARCHITECTURE.md`.
**FRs covered:** FR8, FR11 (guide lands), FR15 (R5)

### Epic 2: Nightly failures classified by error class, one test toolkit
Stage S1; Epics 3 and 4 start after its live-test migration (Story 2.6). Nightly separates `auth-gated`, `environmental`, `transient` and `real` failures from tags rather than regexes. Every crate uses one set of agreement, fixture, soak and capture helpers. Creates `polyoxide-venue`'s classification interface and `polyoxide-test-support`, with the interface implemented over the existing error enums.
**FRs covered:** FR4 (interface), FR8 (classifier), FR9, FR14 (T rows)

### Epic 3: One HTTP path with venue-supplied throttles
Stage S1, after Story 2.6; runs in parallel with Epic 4. Every request on every venue is gated, retried and held the same way. A venue plugs in its limits without editing core, which a Kalshi-style bucket test proves. HTTP errors reshape to the eight classes, and one venue's features cannot change another's headers.
**FRs covered:** FR1, FR2, FR4 (HTTP enums), FR10, FR14 (H rows), FR15 (R4, R7, R8, R10)

### Epic 4: One socket kit and supervisor, and the S1 release
Stage S1, after Story 2.6; runs in parallel with Epic 3, and its last story cuts the S1 release once Epics 1–4 are complete. Every feed reconnects, stays alive and reports outages consistently, and credential-free feeds stay light. perps and usdm move onto `Supervisor` behind their unchanged suites; rtds and sports use the kit blocks.
**FRs covered:** FR3, FR4 (socket enums), FR8 (S1 release), FR10 (socket transport), FR14 (W rows), FR15 (R1, R2, R3, R9)

### Epic 5: One crate per venue, multi-venue facades
Stage S2, one loom integration session with a draft PR to `main`. Consumers find each venue under one name: `polyoxide-polymarket` and `polyoxide-binance`, a multi-venue `polyoxide`, `polyoxide <venue> <module> <verb>`, and `polyoxide.polymarket.*`. The epic also produces the rename manifest, the tombstone releases and the venue-identifier grep gate.
**FRs covered:** FR7, FR8 (metadata moves), FR14 (C rows), FR15 (R6)

### Epic 6: Venue-neutral market data
Stage S3. One generic function reads quotes and books from the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M, through canonical keys and typed extensions.
**FRs covered:** FR5

### Epic 7: Supervised CLOB sockets and normalized trading
Stage S3, after Epics 4 and 6. Consumers trade Polymarket through a lossless `Trading` interface (fills, `Resync`, `Killed`) over supervised CLOB sockets. The epic includes the Kalshi mapping table.
**FRs covered:** FR12, FR6

### Epic 8: Kalshi walking skeleton
Stage S3, after Epic 6. A third venue lands by following the guide and touches only its own files. It provides exchange status, `MarketData`, and a signed supervised socket against the demo host. Any gap in the guide becomes a spine amendment.
**FRs covered:** FR13, FR11 (validated)

## Epic 1: Reliable releases and one-place registration

Releases publish every crate, including `polyoxide-cli`. A failed release can be resumed, and reusing a version number fails loudly. Each crate is registered once, in its own `Cargo.toml`; the docs tables and nightly rows are generated from that. The agent guide lands in the repository. This is stage S1, and this epic merges first: the publish-order script must land before any new crate exists (AD-16).

### Story 1.1: Re-baseline on current main

As a polyoxide maintainer,
I want the restructure to start from the latest released code,
So that S1 cannot reuse a shipped version number or audit stale code.

**Implements:** NFR12, AD-16 (S1 baseline)

**Acceptance Criteria:**

**Given** `origin/main` at v0.38.1 or later
**When** the restructure branch is brought up to date through loom
**Then** the branch's workspace version equals the latest release on crates.io
**And** that release's tag is recorded as the S1 start tag, which becomes the removal gate's baseline (AD-22)
**And** the duplication-inventory rows for gamma and sports are re-checked against the merged code, and any difference is recorded through a `bmad-spec` update

### Story 1.2: Publish-order script and resumable releases

As a release operator,
I want releases to publish exactly the crate versions not yet on crates.io, in dependency order,
So that I can re-run a failed release to completion, and a version number can never be reused silently.

**Implements:** FR8, NFR12, AD-25

**Acceptance Criteria:**

**Given** `scripts/publish_order.py`
**When** it runs
**Then** it reads crates, versions and dependencies from `cargo metadata`, never by regex
**And** it orders crates topologically, ignoring path-only dev-dependencies, and fails on a cycle of versioned dev-dependencies
**And** it lists only the (crate, version) pairs that the crates.io API reports as absent, considering only members whose `publish` is not `false`
**And** it sends a User-Agent, because crates.io refuses requests without one
**And** when `tombstones/*/Cargo.toml` exist, it lists their absent pairs after every workspace crate

**Given** `release.yml` and `finish_release.sh`
**When** either one runs
**Then** it publishes only what the script lists
**And** the list includes `polyoxide-cli`, whose stale description is corrected
**And** the run fails if more than five publishable crate names are absent from crates.io

**Given** `release.yml`'s trigger
**When** a CI run completes
**Then** the release proceeds only if `workflow_run.event == 'push'`, `head_branch == 'main'` and `head_repository.full_name == github.repository`
**And** the `cargo` and `pypi` environments restrict deployments to `main`

**Given** a commit that changes the workspace version
**When** the tag for that version already exists at another SHA
**Then** `release.yml` fails with an `::error::` instead of skipping silently
**And** `test_changelog.py` reads the version with the same query
**And** the script has pytest coverage in the CI scripts job

### Story 1.3: Package every crate on every PR

As a release operator,
I want manifest faults caught on the pull request that introduces them,
So that none surfaces halfway through a publish.

**Implements:** FR8, AD-22

**Acceptance Criteria:**

**Given** a `ci.yml` job that runs `cargo publish --workspace --dry-run` or `cargo package`
**When** any crate has a manifest fault, such as a missing pin version, a regular path-only dependency or missing metadata
**Then** CI fails on that PR
**And** `publish = false` members (`polyoxide-py`, and later `polyoxide-test-support`) are excluded or skipped, as verified on the current stable toolchain
**And** the job passes on the current tree, including the never-published `polyoxide-cli`

### Story 1.4: Registration metadata and the generator

As a maintainer adding a crate,
I want to declare it once, in its own `Cargo.toml`,
So that the README, the spec index and CLAUDE.md's crate facts cannot drift.

**Implements:** FR8, AD-13, AD-21

**Acceptance Criteria:**

**Given** each crate's `[package.metadata.polyoxide]` table
**When** a crate is declared
**Then** the table holds the crate's README line, its covered mirrors, and venue and product ids where they apply
**And** `[workspace.metadata.polyoxide.mirrors]` holds the mirrors that belong to no crate (bridge, combos-rfq) and the deliberate exclusions

**Given** `scripts/gen_registry.py`
**When** it runs
**Then** it rewrites only the text between `generated:begin <id>` and `generated:end <id>` markers, written with each host file's comment leader (`#` in YAML or TOML, `<!-- -->` in markdown), and it preserves indentation
**And** those regions are the README crate table, `docs/specs/INDEX.md`, CLAUDE.md's crate list, dependency graph, publishing order, nightly list and schema exclusions, and SELF-HEALING.md's lists
**And** its output on the current tree matches today's content, apart from deliberate corrections

**Given** CI
**When** the generated output differs from what is committed, or two venue ids collide
**Then** CI fails

### Story 1.5: Nightly and schema rows derived from metadata

As a nightly operator,
I want the live-test rows, their secrets and the schema watch list derived from metadata,
So that a new live test or spec mirror can never be left unwatched by accident.

**Implements:** FR8, AD-13

**Acceptance Criteria:**

**Given** `live.<target> = { suite, timeout, features, secrets }` entries
**When** they are declared
**Then** nightly-behavioral's jobs are generated from them, one job per (crate, suite)
**And** each job's env carries only the exact secret names its targets declare, because secrets cannot be referenced from a matrix or `if:`
**And** the derived matrix equals today's hand-written rows, including clob's separate 40-minute session-keys row

**Given** nightly-schema
**When** it runs
**Then** its watch list and exclusions come from metadata, and the `spec:<id>` labels are unchanged

**Given** CI
**When** a test in `tests/live_*.rs` has no derived row, lacks `#[ignore]`, or lacks the `required-features` it needs
**Then** CI fails
**And** clob's `live_ws` declares `required-features = ["ws"]`
**And** CI fails when the env names a live test passes to the credential loaders differ from its target's `secrets`

### Story 1.6: Workspace hygiene gates

As a polyoxide maintainer,
I want the MSRV, per-feature builds and TLS dependencies enforced by CI,
So that a module that compiles only alongside others, or a break that shows up only on the MSRV toolchain, cannot reach a release.

**Implements:** NFR9, AD-22, DRIFT R5

**Acceptance Criteria:**

**Given** the workspace manifest
**When** it is updated
**Then** it sets `resolver = "3"`
**And** it declares `rustls` once in `[workspace.dependencies]` with `ring` and `std`
**And** every socket crate, clob included, uses that declaration (DRIFT R5, in its own commit)

**Given** `ci.yml`
**When** it runs
**Then** it has an MSRV job that runs `cargo +1.91 check --workspace --all-features` and `cargo +1.91 doc` without `-D warnings`
**And** it has a `cargo hack check --each-feature --no-dev-deps` job (cargo-hack 0.6.45) covering every crate that has features
**And** both jobs pass on the current tree

### Story 1.7: The S1 removal gate

As a prader-rs maintainer,
I want every public item that S1 removes to be listed deliberately,
So that I can plan my S1 migration and am never surprised by a removal.

**Implements:** NFR1, AD-16, AD-22

**Acceptance Criteria:**

**Given** a `ci.yml` job that runs `cargo semver-checks --baseline-rev <S1 start tag> --release-type patch`, with cargo-semver-checks and its toolchain pinned together
**When** it runs
**Then** it reports every removal, even across a 0.x minor bump
**And** the job checks out tags and passes `--exclude` for crates absent at the baseline
**And** `docs/s1-removals.md` stays cumulative until the S1 release

**Given** `release.yml`
**When** it runs `cargo semver-checks` against the previous tag and a removal is reported
**Then** the release fails unless the bump raises the 0.x minor

**Given** `scripts/api_removals.py` and a checked-in `docs/s1-removals.md`, which starts empty
**When** a PR removes a public item that is not on the list
**Then** CI fails
**And** doc-hidden removals, such as the `test_server` modules, are listed by hand and covered by a compile test that `use`s each listed path against the baseline
**And** the configuration is proven against a scratch removal in this story's own PR
**And** this story is a declared predecessor of Epics 2–4, so the gate lands before any story that removes a public item

### Story 1.8: The agent guide lands

As an agent working on polyoxide,
I want the architecture guide in the repository, with CLAUDE.md pointing to it,
So that I know where every change goes during the restructure.

**Implements:** FR11, AD-12, AD-21

**Acceptance Criteria:**

**Given** the spine's guide rendering
**When** it is placed at `docs/ARCHITECTURE.md`
**Then** it states the current stage, S1
**And** CLAUDE.md links to it from a generated section or a section scoped to its own `##` heading

**Given** `docs/MUTANTS.md`
**When** it is created
**Then** it has one row per mutation-tested rule that exists today: the 429 feedback ordering, Retry-After only extending a wait, cooldowns only extending, `quota()` without `allow_burst`, and `classify_order_kill`
**And** each row gives the file, line, mutation and the test that must fail
**And** running each mutant makes its test fail

## Epic 2: Nightly failures classified by error class, one test toolkit

The nightly run sorts failures into `auth-gated`, `environmental`, `transient` and `real` using tags printed where the failure happens, not regexes. Every crate uses one set of agreement, fixture, soak and capture helpers.

This is stage S1. Epics 3 and 4 start only after Story 2.6. By then the tag reporter and the live-test migration have both landed, so no error's variants, `Debug` or `Display` change before the classifier stops relying on them (AD-16).

### Story 2.1: The classification vocabulary

As a consumer of any venue,
I want one shared interface that classifies any polyoxide error,
So that I write my retry and alerting logic once.

**Implements:** FR4, NFR6, AD-3, AD-15

**Acceptance Criteria:**

**Given** a new crate `polyoxide-venue` that depends only on rust_decimal, serde, thiserror, futures-core and dynosaur
**When** it is published
**Then** it exports `#[non_exhaustive] Class` with eight variants: Network, Unavailable{code}, RateLimited{retry_after}, Unauthorized, InvalidRequest, VenueRefusal{code}, Restricted and Decode
**And** it exports a `Classify` trait with `class()`, `is_fault()` and `retry_after()`, plus a provided `is_retriable()` that is true for Network, Unavailable and RateLimited
**And** it exports `ClassifiedError { class, source }` with `From<E: Classify>`
**And** it exports `Secret<T>`, whose `Debug` output is redacted
**And** a status-to-class function implements AD-15's map: 401 and 403 give Unauthorized; 418 and 451 give Restricted; 429 gives RateLimited; 408, 425 and 5xx give Unavailable; every other 4xx gives VenueRefusal
**And** the one `Retry-After` parser takes its clamp as a parameter and only ever lengthens the client's own backoff
**And** a table test pins every status row and every parser case, including zero, negative, NaN and HTTP-date values

### Story 2.2: Classify today's error enums

As a consumer of any venue,
I want every existing polyoxide error to implement `Classify`,
So that classification works now, before any error type is reshaped.

**Implements:** FR4, AD-15, AD-21

**Acceptance Criteria:**

**Given** core's `ApiError` and `KeychainError`, `GammaError`, `DataApiError` (v1 and v2), `ClobError`, `RelayError`, `PerpsError`, `BinanceError`, `PerpsWsError`, `UsdmWsError`, `SportsError`, `RtdsError`, clob's `WebSocketError`, and the umbrella's `PolymarketError`
**When** `Classify` is implemented for each of them
**Then** each implementation maps its current variants to classes using the status-first rule
**And** Binance's 403 maps to Restricted through its documented D14 exception
**And** the FAK and FOK kill variants map to VenueRefusal with `is_fault() == false`
**And** no variant changes, and no `Debug` or `Display` output changes
**And** each crate has a table test covering every variant
**And** a compile-time assertion lists every public error type and fails to build if one does not implement `Classify`
**And** rtds and sports gain only the `polyoxide-venue` dependency, and CLAUDE.md's "depend on nothing in-workspace" rule is rewritten in the same change

### Story 2.3: The test toolkit crate and the failure-tag reporter

As a nightly operator,
I want every live-test failure to print what kind of failure it is,
So that the classifier never has to guess from panic text.

**Implements:** FR9, AD-1, AD-14

**Acceptance Criteria:**

**Given** a new crate `polyoxide-test-support`, which is `publish = false`, depends only on `polyoxide-core` (with keychain) and `polyoxide-venue`, and is used only as a path-only dev-dependency
**When** a test calls `.or_fail("ctx")` on a `Result<T, E: Classify>` that is an `Err`
**Then** it prints `polyoxide-class=<tag>` to stderr, with the tag mapped from the class alone
**And** it then panics with `ctx` and the error

**Given** the credential loaders, which take their env names from the test
**When** the credentials are absent or empty (an unset repository secret arrives as an empty string)
**Then** the loaders print `auth-gated`
**And** the existing `POLYMARKET_*`, `BUILDER_*` and `RELAYER_*` names keep working
**And** `environmental(reason)` and `transient(reason)` each print their own tag
**And** every entry point installs its hook through a chained `Once`, which a test proves under nextest's process-per-test model

### Story 2.4: The classifier reads tags

As a nightly operator,
I want `classify_failures.py` to decide verdicts from tags first,
So that adding a venue never means adding regexes.

**Implements:** FR8, AD-14

**Acceptance Criteria:**

**Given** a failure log
**When** it is classified
**Then** the last `polyoxide-class=` line wins: `auth-gated` is skipped silently, `environmental` is logged and skipped, `transient` is retried, and `real` is filed
**And** a log without a tag falls back to the existing regex table, and a log the table does not match is `real`
**And** `merge` still promotes a persistent transient to `real`
**And** all 27 existing tests pass unchanged, and new tests cover tag precedence

**Given** `scripts/live_unwraps.py` and its committed baseline
**When** a PR adds a live test that unwraps a polyoxide `Result` without `or_fail`, or adds a regex to the classifier
**Then** CI fails, so the baseline can only shrink

### Story 2.5: Migrate the HTTP live suites

As a nightly operator,
I want every HTTP live test to report through the toolkit,
So that its failures are tagged.

**Implements:** FR9, NFR7, AD-14

**Acceptance Criteria:**

**Given** the live suites of gamma, data, clob (`live_api` and `live_session_keys`), relay, perps (HTTP), binance (HTTP) and cli
**When** they are migrated
**Then** every unwrap or expect on a polyoxide call becomes `or_fail`
**And** credential checks go through the loaders
**And** conditions caused by the outside world, such as Binance's 451 or "no qualifying market", call `environmental(reason)`
**And** test names, assertions and per-suite counts are unchanged
**And** these suites leave `live_unwraps.py`'s baseline

### Story 2.6: Migrate the socket live suites

As a nightly operator,
I want every socket live test to report through the toolkit,
So that a routine disconnect is tagged transient instead of being filed as a fault.

**Implements:** FR9, NFR7, AD-14

**Acceptance Criteria:**

**Given** clob `live_ws`, perps `live_ws`, binance `live_ws`, rtds `live_api` and sports `live_api`
**When** they are migrated
**Then** "server ended the connection" becomes `transient(reason)`
**And** sports' "legitimately time out" becomes `environmental(reason)`
**And** every other polyoxide `Result` goes through `or_fail`
**And** test names, assertions and counts are unchanged
**And** `live_unwraps.py`'s baseline is empty

### Story 2.7: Retire the regex fallback

As a nightly operator,
I want the regex classifier removed once nothing needs it,
So that there is exactly one way a failure is classified.

**Implements:** AD-14, AD-21

**Acceptance Criteria:**

**Given** an empty `live_unwraps.py` baseline
**When** this story merges
**Then** `AUTH_GATED_RE`, `ENVIRONMENTAL_RE` and `TRANSIENT_RES` are deleted
**And** the behaviour of the 27 regex-era tests is preserved as tag-table tests, one per original case
**And** CLAUDE.md's text on `AUTH_GATED_RE` and on nightly classification is rewritten in the same change

### Story 2.8: Shared agreement helpers, adopted by data v2 and perps

As a maintainer writing wire or spec agreement tests,
I want one set of helpers,
So that every crate's drift tests check the same things in the same way.

**Implements:** FR9, FR14 (T1–T6), NFR7, AD-12

**Acceptance Criteria:**

**Given** `polyoxide-test-support`
**When** the helpers move into it
**Then** it holds `key_paths`, `assert_values_agree`, the allow-lists with the stale-excuse check, the OpenAPI synthesiser, `query_keys_sent`/`Fire`, and the fixture loader
**And** the synthesiser is a superset of the perps fork: it covers `$ref`-nullable, enum and example handling, and `OBSERVED_EXTRA`
**And** data v2's suites and perps' HTTP and socket suites use these helpers, with no copy left in either crate
**And** each migrated suite keeps its test names, assertions and counts

### Story 2.9: Gamma and Binance on the shared agreement helpers

As a maintainer of the gamma and Binance drift tests,
I want them on the same helpers,
So that no agreement helper exists twice.

**Implements:** FR9, FR14 (T1–T6), NFR7, AD-12

**Acceptance Criteria:**

**Given** the shared helpers
**When** gamma and binance adopt them
**Then** the allow-lists support gamma's dotted paths, its `(key, reason)` tuples and its array-length assertion
**And** gamma's and binance's agreement suites use the shared helpers, with no copy left in either crate
**And** each migrated suite keeps its test names, assertions and counts

### Story 2.10: Shared soak harness and capture helpers

As a maintainer measuring a venue's limits,
I want one soak harness and one capture-script helper,
So that measuring a new venue copies nothing.

**Implements:** FR9, FR14 (T7–T9)

**Acceptance Criteria:**

**Given** `polyoxide-test-support`
**When** the soak pieces move into it
**Then** it holds `Pacer`, the throttle observer, `classify`/`judge`/`pin`, stage and route parsing, and the minute-boundary waits
**And** data's `v2_soak` and perps' `info_soak` use it without `#[path]` includes
**And** both soaks still detect the `Retriable status 429` line logged at `WARN` under the `polyoxide_core` target prefix

**Given** `scripts/capture_common.py`
**When** the capture scripts use it
**Then** it provides one HTTP `get` (with pause, user agent and error handling), one PROVENANCE writer and one WebSocket client
**And** the `get` and the WebSocket client both take per-request headers from a function the caller supplies, so a signing venue keeps its signing in its own capture script
**And** all six capture scripts use it, and Binance's hand-rolled RFC 6455 client is removed
**And** re-running each script reproduces its committed fixtures, apart from live-data differences

## Epic 3: One HTTP path with venue-supplied throttles

Every HTTP request on every venue is gated, retried and held the same way. A venue plugs in its limits without editing core. HTTP errors are reshaped to the eight classes, and no venue's features can change another venue's headers.

This is stage S1. It starts after Story 2.6 and runs in parallel with Epic 4. Throughout the epic:
- Behaviour is preserved unless a DRIFT row says otherwise, and each DRIFT row lands as its own commit naming its row.
- Every removed public item is listed in `docs/s1-removals.md`.
- Moved tests keep their names and assertions.

### Story 3.1: The one send loop and its hook contract

As a consumer of any Polymarket host,
I want every request retried and backed off by one tested loop,
So that a 429 on one request slows every request sharing the limit, exactly as today.

**Implements:** FR1, FR2, NFR10, AD-5, AD-8, AD-9, AD-21, AD-23, DRIFT R10

**Acceptance Criteria:**

**Given** `polyoxide-core`
**When** the loop lands
**Then** it defines three hook traits:
- `Throttle`, with async `acquire(&RequestMeta) -> Result<Charge, Refused>` and synchronous `observe` and `hold`;
- `RetryPolicy`, whose `decide` returns `Decision { outcome, hold }`;
- `Authenticator`, with async `sign(&mut RequestParts, attempt)`.

**And** the traits are `Send + Sync`, never `'static`, with `#[dynosaur(pub Dyn<Trait> = dyn(box) <Trait>)]` wrappers
**And** `HttpClient` holds exactly one `Arc<DynThrottle<'static>>`, a no-op when unthrottled, names no concrete limiter, and holds no cooldown
**And** clients created with `with_base_url` share that throttle and its hold
**And** each attempt runs in AD-8's order: permit, acquire, sign, send, observe, decide, hold, log
**And** the permit is released before sleeping for `max(floor, wait)`
**And** a transport error that gets no response skips observe and decide, is not retried, and is classed Network
**And** the four decode-and-log copies (H3) are replaced by the loop's

**Given** today's `RateLimiter`, wrapped as a `Throttle` that owns the hold
**And** Polymarket's one `RetryPolicy` for 429 and 425, in core's `polymarket` module
**When** gamma, data and perps run through the loop
**Then** they behave as today:
- only a 429 sets a hold;
- that hold is `retry_delay(0)`;
- a 425 waits per request;
- every 429 sets a hold, even on the last attempt;
- `Retry-After` only lengthens a wait.

**And** `get_bytes` uses the same loop
**And** each retry logs `Retriable status <code> on <path>, retry <n> after <ms>ms` at WARN under the `polyoxide_core` target prefix (DRIFT R10)
**And** each of these mutants fails a test: dropping the hold on a last-attempt 429, skipping observe on the last attempt, and a policy that returns a zero wait
**And** those mutants are recorded in `docs/MUTANTS.md`
**And** CLAUDE.md's "`note_rate_limited` before `should_retry`" text and its 429/425 retry-set text are rewritten in the same change

### Story 3.2: The public window-quota table

As a maintainer adding a venue with window quotas,
I want to build its limit table from core's public builder,
So that describing a venue's limits never needs an edit to core.

**Implements:** FR2, NFR7, AD-10, AD-12

**Acceptance Criteria:**

**Given** a public `WindowQuotaTable` builder in core
**When** a table is built
**Then** it supports buckets shared across routes, prefix and exact matching, method scoping, depth 1, and a reserved tenth
**And** a public `effective_quota(method, path)` returns every bucket a request awaits, including the general bucket

**Given** the five Polymarket tables
**When** they are rebuilt with the builder inside core's `polymarket` module
**Then** every `documented_*_limits` test asserts only through `effective_quota`, keeping its name and its expected quotas
**And** this rewrite is its own commit, with suite counts before and after and the mutants re-run, so the tests are ready to move in S2
**And** the shared ledger bucket still spans `/trades`, `/orders`, `/notifications` and `/order`

### Story 3.3: Capacity buckets and Polymarket's composed throttle

As a Polymarket trader,
I want order batches charged in requests on one layer and in orders on another,
So that a batch is neither refused forever nor sent over the signer's limit.

**Implements:** FR2, NFR4, AD-10, AD-23

**Acceptance Criteria:**

**Given** a public capacity bucket in core (capacity, refill, refusal of costs it can never hold) that shares the hold
**When** Polymarket's per-signer layer is rebuilt on it
**Then** it keeps `allow_burst(capacity)` (D1) and its Order and Cancel buckets
**And** Polymarket's composed throttle charges the Cloudflare layer 1 request and the signer layer N orders, taken from the request's `costs`
**And** a cost the signer bucket can never hold is refused before sending, as non-retriable
**And** a hold set on the composed throttle stops every layer
**And** the hold is a handle shared by every layer, and it survives replacing or resizing a layer
**And** the capacity bucket offers `resize(capacity, refill)`, which keeps its tokens (clamped) and its hold
**And** a provisionally sized throttle never returns `Refused`; it waits instead until its sizing is confirmed
**And** the hold's ceiling is a parameter of each throttle

**Given** a test outside `polyoxide-core`
**When** it builds a Kalshi-style throttle with separate read and write buckets and integer token costs
**Then** it compiles and passes without any edit to core (the CAP-2 gate)

### Story 3.4: clob on the one loop

As a Polymarket trader,
I want order requests signed fresh on every attempt and throttled through the shared loop,
So that retries never reuse a stale signature or bypass the signer limits.

**Implements:** FR1, FR2, NFR6, AD-8

**Acceptance Criteria:**

**Given** clob's request path
**When** it moves onto the core loop
**Then** an L2 `Authenticator` signs on every attempt, with L1 signing done asynchronously inside `sign`
**And** the composed throttle from Story 3.3 charges the per-signer costs
**And** `observe` adopts the `Poly-RateLimit-Tier` header on every status
**And** clob's API credentials are held in `Secret<T>`
**And** `classify_order_kill` behaves as before
**And** clob's own loop and its `request` module are removed and listed
**And** the clob suites, including the kill-outcome and burst-capacity tests, pass with names and assertions unchanged

### Story 3.5: relay on the one loop, gating holes closed

As a relay user,
I want relay's requests rate-limited and classified like every other host's,
So that a 429 from the relayer is retried correctly instead of surfacing as an opaque string.

**Implements:** FR1, NFR6, AD-8, DRIFT R7, DRIFT R8

**Acceptance Criteria:**

**Given** relay's three loops
**When** they move onto the core loop with relay's `Authenticator`
**Then** `RelayError` wraps `ApiError` and drops `Api(String)` and the variants that are never constructed (DRIFT R7)
**And** relay's builder credentials are held in `Secret<T>`
**And** new mock tests prove 429 retry and hold on each relay route
**And** on-chain gas estimation through alloy stays outside the loop, as the documented exception

**Given** clob's ping, gamma's `post_json`, and the gamma and data pings
**When** they go through core's `health(path)` or the loop
**Then** each is gated and feeds 429s back (DRIFT R8)
**And** a test pins each one

### Story 3.6: Binance on the one loop

As a Binance consumer running several clients on one IP,
I want a ban seen by one client to hold them all,
So that sending into a spent minute never turns a 429 into a 418.

**Implements:** FR2, AD-9, AD-10, AD-23

**Acceptance Criteria:**

**Given** Binance's throttle
**When** it is built
**Then** the weight minute stays in `polyoxide-binance`, and its `Charge` carries the UTC minute
**And** the funding bucket uses core's `WindowQuotaTable`
**And** the hold is core's primitive, with a 3-day ceiling, shared through the one `WeightBudget` passed to every `Usdm`
**And** no venue crate depends on governor directly, which a CI check enforces

**Given** Binance's `RetryPolicy`
**When** responses arrive
**Then** a 429 with a retry left holds `max(wait, Retry-After)`
**And** a 429 with no retry left and no `Retry-After` holds to the next UTC minute
**And** a 418 is `Fail`, holding for its `Retry-After`, or for 2 minutes when it has none
**And** a 425 is not retried
**And** `WeightedRequest` is removed and listed
**And** the existing 418, hold and weight mock tests pass unchanged

### Story 3.7: One client builder, one namespace pattern, one health ping

As a maintainer adding a host,
I want client config, namespaces and the health ping generated from one definition,
So that a new client takes a few lines rather than a copied builder.

**Implements:** FR1, FR14 (H6–H8)

**Acceptance Criteria:**

**Given** core's client-config type and builder macro
**When** every client adopts them
**Then** `base_url`, `timeout_ms`, `pool_size`, the retry config and `max_concurrent` come from one definition
**And** each client keeps its default concurrency of 2, 4 or 8, and leaves `gzip` unset
**And** the namespace accessors (H7) and `health(path)` (H8) come from core macros
**And** no crate defines its own builder knobs, accessors or ping
**And** every removed or changed public item is listed

### Story 3.8: One query-setter macro

As a maintainer adding a route,
I want query setters declared rather than hand-written,
So that about 2,000 lines of setters become one definition.

**Implements:** FR1, FR14 (H9)

**Acceptance Criteria:**

**Given** a query-setter macro in core, promoted from perps' `setter!`
**When** gamma, data, clob and binance adopt it
**Then** no hand-written setter remains
**And** every setter keeps its name, argument type and query key, as each crate's `query_keys_sent` spec-agreement tests prove

### Story 3.9: One wire-enum vocabulary

As a maintainer modelling a venue's enums,
I want one set of open and closed wire-enum macros,
So that four enum macros and two `UnknownVariant` copies become one definition each.

**Implements:** FR1, FR14 (H10, H11, H16)

**Acceptance Criteria:**

**Given** `polyoxide-venue`
**When** it receives `open_enum!`, `wire_enum!`, `UnknownVariant`, positional decimal serde and `UnixMillis::now()`
**Then** gamma, data, perps, binance and relay use them, and their own copies are removed
**And** every serde round-trip and wire-agreement test passes unchanged

### Story 3.10: One Retry-After parser and one retriable-status rule

As a consumer of any venue,
I want `Retry-After` read the same way everywhere,
So that the same header never gives different waits on different venues.

**Implements:** FR4, FR14 (H12, H13), DRIFT R4

**Acceptance Criteria:**

**Given** the four `Retry-After` parsers in core, binance, data v2 and perps
**When** they are replaced by `polyoxide-venue`'s one parser (DRIFT R4, in its own commit)
**Then** Binance passes its 3-day clamp as the parameter
**And** a zero, negative or unparseable value never shortens the client's own backoff
**And** the three copies of the 408/425/429/5xx retriable-status rule (core, perps, binance) are removed in favour of `polyoxide-venue`'s
**And** tests pin the cases where the parsers used to disagree, each with its new answer

### Story 3.11: Polymarket HTTP errors reshaped

As a consumer handling Polymarket errors,
I want every Polymarket HTTP error built the same way and classified by status first,
So that a 503 on gamma and a 503 on clob mean the same thing.

**Implements:** FR4, FR14 (H14, H15), AD-15, AD-21

**Acceptance Criteria:**

**Given** the HTTP-tier error enums of gamma, data, perps, clob and relay, whose names stay unchanged until S2
**When** they are reshaped
**Then** each wraps core's `ApiError`, which carries the status, headers, body, parsed `Retry-After` and `Refused`
**And** each decodes its venue's body in a single function
**And** each module classifies by AD-15's map, overriding it only where a DFR row says so
**And** `ApiError::Refused` maps to each module's existing variant, such as `ClobError::BurstCapacityExceeded`
**And** CLAUDE.md's `impl_api_error_conversions!` paragraph is rewritten in the same change
**And** the changed variants are listed for prader
**And** the Story 2.2 table tests still give every status the same class

### Story 3.12: Binance HTTP errors reshaped, and Python maps by class

As a consumer handling Binance errors, or calling polyoxide from Python,
I want Binance errors classified like Polymarket's, and Python exceptions chosen by class,
So that my error handling is the same across venues and languages.

**Implements:** FR4, AD-15

**Acceptance Criteria:**

**Given** `BinanceError`, whose name stays unchanged until S2
**When** it is reshaped
**Then** it wraps `ApiError` and decodes `{code, msg}` in one function
**And** its 403 (WAF), 418 and 451 map to Restricted, through D14
**And** the changed variants are listed

**Given** `polyoxide-py`
**When** it maps errors to exceptions
**Then** each exception follows the error's class one-to-one
**And** data v2 errors keep their mapping by code
**And** the offline Python suites and the stub-consistency check pass

### Story 3.13: Transport ownership for HTTP and venue isolation

As a consumer depending on two venues,
I want neither venue's dependency features to change the other's requests,
So that the 0.37.0 gzip regression cannot happen again.

**Implements:** FR10, AD-18

**Acceptance Criteria:**

**Given** the workspace
**When** this story lands
**Then** only `polyoxide-core` depends directly on reqwest
**And** a CI check fails on a direct reqwest dependency in any other crate (Epic 4 extends this check to tokio-tungstenite and rustls)

**Given** one mock test per HTTP module that pins the full header set of one request, including `Accept-Encoding`
**When** it runs in the venue crate alone with minimal features, and again under `cargo test --workspace --all-features`
**Then** both builds produce identical headers

## Epic 4: One socket kit and supervisor, and the S1 release

Every feed reconnects, stays alive and reports outages in the same way, and credential-free feeds stay light. perps and usdm move onto the generic `Supervisor`. rtds, sports and clob use the kit blocks. The epic ends by cutting the S1 release.

This is stage S1. It starts after Story 2.6 and runs in parallel with Epic 3, except Story 4.11, which needs Epics 1–4 complete. Throughout the epic:
- DFR rows D4–D12 stay at the venue's `Protocol` or call site.
- Each DRIFT row lands as its own commit naming its row.
- Removed public items are listed, with no re-exports.

### Story 4.1: The socket kit crate

As a maintainer of any socket feed,
I want TLS setup, backoff and timing defined once,
So that a fix to reconnect behaviour reaches every feed.

**Implements:** FR3, FR14 (W1, W3–W6), NFR2, AD-2

**Acceptance Criteria:**

**Given** a new crate `polyoxide-ws` that depends only on tokio, tokio-tungstenite, futures-util, thiserror, tracing and rustls (`ring`, `std`), plus optional serde and serde_json
**When** it is published
**Then** it defines each of these once:
- `ensure_crypto_provider` (W1);
- `Backoff` (W3), whose reset input stays a call-site `bool`, with rtds' 7-test backoff suite as the canonical tests;
- the timing setters and defaults (W4);
- the deadline arithmetic (W5);
- the boxed `WsError` wrapper (W6).

**And** a CI allowlist check on `cargo tree -p polyoxide-ws -e normal` fails on any other dependency
**And** core's jittered HTTP backoff stays a separate algorithm (D13)

### Story 4.2: Connect, classify and close, once

As a consumer of any feed,
I want every connect bounded by a timeout, and every refusal and close classified the same way,
So that a hung handshake or a 503 never stalls or silently kills a feed.

**Implements:** FR3, FR14 (W7–W9, W11, W12), DRIFT R3

**Acceptance Criteria:**

**Given** `polyoxide-ws`
**When** connect, handshake and close handling land
**Then** connect-with-timeout defaults to 10 s (W8)
**And** handshake-status and close classification take the retriable-status rule as an injected function (W7, W9)
**And** the bare-tier `poll_next` skeleton skips Ping, Pong and Frame for callers that ask, and passes every frame to callers that need all of them (W11)
**And** request/answer correlation is provided (W12)

**Given** a server that closes the connection
**When** the bare tier sees the close
**Then** it sends the RFC 6455 close reply (DRIFT R3)
**And** the reply's absence today is first confirmed on the wire against one live host, with the capture noted in the PR

### Story 4.3: One socket-error classification table

As a consumer handling socket errors,
I want a dropped connection classified the same way on every venue,
So that a 1011 close is transient everywhere, not on one venue only.

**Implements:** FR4, AD-15

**Acceptance Criteria:**

**Given** `impl_ws_classification!`, exported by `polyoxide-venue` and expanding against `::polyoxide_ws::WsError`
**When** a venue crate invokes it for its socket error type
**Then** `Io` (a TLS EOF included), `Protocol`, `ConnectionClosed`, and close codes 1000, 1001, 1006, 1011, 1012 and 1013 map to Network
**And** close codes 1002, 1003, 1007, 1008, 1009 and 1010, and 4000–4999, map to VenueRefusal{code}
**And** `Url`, `HttpFormat`, `Tls`, `AttackAttempt`, `Capacity` and `AlreadyClosed` map to InvalidRequest
**And** handshake statuses map through AD-15's status rule
**And** the decision to reconnect or stop belongs to the `Protocol`'s recovery hook, which by default reconnects only when the class is retriable
**And** neither `polyoxide-venue` nor `polyoxide-ws` depends on the other
**And** a table test in a crate that depends on both covers every row

### Story 4.4: One scripted test server

As a maintainer testing a socket feed,
I want one scripted server with a pluggable protocol answer,
So that a new venue's supervision tests copy no harness.

**Implements:** FR3, FR9, FR14 (W13, W15)

**Acceptance Criteria:**

**Given** `polyoxide-ws`'s `test-server` feature
**When** the server lands
**Then** it provides (W13):
- an accept loop with one script per connection, where the last script repeats;
- `connection_count` and `wait_for`;
- reject, stall, refuse-status and close-code actions;
- recorders for closes, pongs and frames;
- the interval timer idiom;
- a pluggable `answer`.

**And** it provides the shared test helpers `next_event`, `fast` and ping counting (W15)
**And** it is built only from published crates

**Given** perps, binance, sports and rtds
**When** their suites switch to the shared server
**Then** each keeps only its protocol `answer` and its fixtures
**And** the `test_server` paths that prader imports are removed and listed

### Story 4.5: The generic Supervisor

As a maintainer adding a socket venue,
I want to implement only a `Protocol`,
So that reconnect, keep-alive, staleness and outage markers come for free and behave identically on every venue.

**Implements:** FR3, NFR3, AD-11

**Acceptance Criteria:**

**Given** `Supervisor<P: Protocol>`
**When** a `Protocol` supplies its AD-11 hooks:
- a handshake request built on every attempt;
- decode, liveness and delivered predicates that see every frame type;
- a ping schedule;
- `P::Membership` with paced `replay`;
- `wanted`;
- an optional `max_connection_age`;
- recovery and protocol events;
- whether it declares `Disconnected`;
- its queue, `Bounded(n)` or `Unbounded`.

**Then** each outage yields at most one `Disconnected`, and only for a declaring `Protocol`
**And** each outage yields exactly one `Reconnected`, even when membership empties mid-outage
**And** pings stay on the wall clock while the consumer is slow: the Supervisor races `reserve()` against the ping timer and stops reading, and blocked time does not count as silence
**And** no decoded event is ever dropped
**And** when `wanted` turns false on a healthy connection, the connection closes politely with no marker
**And** a unit suite against the scripted server, using a test `Protocol`, covers every rule above

### Story 4.6: perps on the Supervisor

As a perps consumer,
I want the perps feed supervised by the shared Supervisor, with its behaviour unchanged,
So that perps gains every future Supervisor fix.

**Implements:** FR3, NFR7, AD-11, AD-12, DRIFT R1, DRIFT R2

**Acceptance Criteria:**

**Given** a `PerpsProtocol`
**When** perps moves onto the Supervisor
**Then** it sends JSON `post/ping` on the wall clock and counts an ok pong as liveness
**And** it maps `message_rate_limited` to retry-without-reconnect
**And** it emits `SequenceRegressed`
**And** it declares no `Disconnected`, stays wanted when its membership is empty, and uses a bounded queue
**And** handshake refusals with 408, 425, 429 or 5xx are retried (DRIFT R1), and connects time out (DRIFT R2)
**And** the 21 inline tests (17 supervision, 3 backoff, 1 defaults) move with names and assertions unchanged and pass, with per-suite counts reported
**And** perps' own loop is deleted, and the removed `supervised` module, `MembershipHandle` and `Recovery` paths are listed

### Story 4.7: Binance on the Supervisor

As a Binance consumer,
I want each routed path supervised by the shared Supervisor, with every Binance rule intact,
So that Binance's outage pairing, rotation and request pacing keep working.

**Implements:** FR3, NFR7, AD-11, AD-12, DRIFT R9

**Acceptance Criteria:**

**Given** a `UsdmProtocol` and a routing layer that runs one Supervisor per routed path
**When** Binance moves onto the Supervisor
**Then** an empty path is not wanted, and a router ends a path only by emptying its membership
**And** `Disconnected { path }` is always followed by `Reconnected { path }`, including when the path empties mid-outage
**And** connections rotate at 23 h 50 min through `max_connection_age`
**And** replay sends at most 200 names per request and one request per 200 ms, with the 1024-stream cap enforced first
**And** `tests/supervision.rs` (21 tests), `supervision_edges.rs` (7), the 3 inline tests in `usdm/ws/supervised.rs`, and the usdm ws client's inline tests all pass with names and assertions unchanged
**And** `live_ws` calls the shared TLS function (DRIFT R9)
**And** the removed paths are listed

### Story 4.8: rtds and sports on the kit

As a consumer of the price and sports feeds,
I want both feeds built on the shared kit and kept light,
So that a 503 at reconnect no longer ends the price feed, and neither feed builds HTTP or signing code.

**Implements:** FR3, NFR2, NFR7, DRIFT R1, DRIFT R2, DRIFT R9

**Acceptance Criteria:**

**Given** rtds, which keeps its `run(handler)` shape
**When** it moves onto the kit blocks (Backoff, connect-with-timeout, the handshake classifier, TLS, the close reply and the test-server core)
**Then** a 503 or 429 at reconnect is retried instead of ending `run()` (DRIFT R1), and connects time out (DRIFT R2)
**And** its 19 supervision tests pass unchanged
**And** `polyoxide_rtds::fixtures` and the `test-fixtures` feature keep their names until S2

**Given** sports, which keeps its `Stream` state machine, ping-as-liveness and its rule for unfixable refusals
**When** it moves onto the kit blocks
**Then** its 27 tests pass unchanged
**And** `live_api` calls the shared TLS function (DRIFT R9)
**And** the allowlist checks on `cargo tree -p polyoxide-rtds -e normal` and on `-p polyoxide-sports` pass

### Story 4.9: clob's sockets on the kit

As a clob socket consumer,
I want clob's market and user sockets built from the same kit, with their current behaviour kept,
So that S3's supervised clob socket starts from shared parts.

**Implements:** FR3, FR14 (W1, W13), AD-21, DRIFT R9

**Acceptance Criteria:**

**Given** clob's `ws` module
**When** it adopts the kit's TLS function, `WsError`, bare-tier skeleton and test server
**Then** Binary frames are still decoded as text (D12)
**And** the 10 s `PING` still runs
**And** `WebSocketWithPing` keeps its behaviour and public shape
**And** clob's per-address connect stays as clob's implementation of the connect step, wrapped by the shared timeout, and is recorded as a DFR row through `bmad-spec`
**And** `live_ws` calls the shared TLS function (DRIFT R9)
**And** clob's two ad-hoc loopback servers are replaced by the shared scripted server
**And** with the last copy gone, CLAUDE.md's text on per-crate `ensure_crypto_provider` copies is rewritten in the same change

### Story 4.10: Socket error enums reshaped, and socket transport ownership

As a consumer handling feed errors,
I want every socket error classified through the one table, and every feed's transport owned by the kit,
So that a dropped connection gets the same verdict on every venue and no feed's TLS can drift.

**Implements:** FR4, FR10, AD-15, AD-18

**Acceptance Criteria:**

**Given** `PerpsWsError`, `UsdmWsError`, `SportsError`, `RtdsError` and clob's `WebSocketError`, whose names stay unchanged until S2
**When** they are reshaped
**Then** each invokes `impl_ws_classification!`
**And** venue-specific recovery, such as perps' `Retry`, moves to its `Protocol`
**And** each venue keeps today's `AlreadyClosed` behaviour through its recovery hook: perps and rtds stop, while Binance and sports reconnect (a DFR row)
**And** the changed variants are listed
**And** the Story 2.2 table tests still give every case the same class, except where DRIFT R1 deliberately changes rtds' and perps' handshake statuses, each named in its commit

**Given** the venue crates
**When** this story lands
**Then** none depends directly on tokio-tungstenite or rustls
**And** `polyoxide-ws` re-exports the types that `Protocol`s need
**And** a CI check fails on a direct tokio-tungstenite or rustls dependency outside `polyoxide-ws`, folded into Story 3.13's transport-ownership check if that has landed, or added standalone if not

### Story 4.11: Cut the S1 release

As a prader-rs maintainer,
I want S1 shipped as one release with a complete migration list,
So that I adapt my error handling and removed imports once, before the S2 renames.

**Implements:** FR8, NFR1, NFR11, NFR12, AD-16, AD-25

**Acceptance Criteria:**

**Given** Epics 1–4 are complete and CI is green on `main`
**When** the S1 release is prepared
**Then** the crates.io token's crate scope is confirmed to cover `polyoxide*`, with an expiry after S3
**And** `docs/s1-removals.md` is final and gives prader the call-site changes alongside the removed paths
**And** the release notes carry a consumer-impact section naming DRIFT R1, R2 and R4 and the log targets that moved to `polyoxide_ws`
**And** the version bump is a separate commit on `main`, made after `git fetch` and a crates.io check

**Given** the release run
**When** it publishes
**Then** `polyoxide-venue`, `polyoxide-ws` and `polyoxide-cli` are published for the first time (three new crate names)
**And** `cargo install polyoxide-cli` works from crates.io, which a post-release smoke check proves
**And** the spine's stage status is updated through `bmad-architecture` to record that S1 has shipped and S2 is next, and `docs/ARCHITECTURE.md` is regenerated from it

## Epic 5: One crate per venue, multi-venue facades

Consumers find each venue under one name: `polyoxide-polymarket` and `polyoxide-binance`, a multi-venue `polyoxide`, `polyoxide <venue> <module> <verb>`, and `polyoxide.polymarket.*`. Retired crates point to their new home. This is stage S2. Every story merges into one loom integration session, and nothing reaches `main` until Story 5.12. Every moved test shows an empty normalized body diff, and every rename gets a row in the rename manifest.

### Story 5.1: Open the S2 integration session

As a maintainer coordinating S2,
I want CI on every S2 merge and a rename manifest that builds itself,
So that the renamed tree has passed CI many times before it reaches `main`.

**Implements:** NFR9, AD-16, AD-22

**Acceptance Criteria:**

**Given** a dedicated loom integration session
**When** it opens
**Then** a draft PR from its branch to `main` runs the full `ci.yml` on every merge into it
**And** the removal gate switches to report-only and writes its findings into `docs/rename-manifest.md` as old path → new path, or old path → reason
**And** `scripts/test_body_diff.py` prints the normalized diff of moved test bodies
**And** CI fails on a non-empty diff that has no DRIFT reason

### Story 5.2: polyoxide-polymarket and a venue-neutral core

As a maintainer adding a venue,
I want the foundation free of Polymarket code,
So that a new venue never inherits another venue's rules.

**Implements:** FR7, AD-1, AD-16

**Acceptance Criteria:**

**Given** a new crate `polyoxide-polymarket` with a `src/shared/` directory, where every item is gated by the features that use it
**When** core's `polymarket` module moves into it (the `Signer`, session signers, signer layer, limit tables, Polymarket's `RetryPolicy` and its 425 policy)
**Then** the `documented_*_limits` and signer tests move with empty body diffs
**And** the CAP-7 gate greps `polyoxide-{venue,core,ws,test-support}` case-insensitively for:
- venue ids, as substrings;
- product ids, as identifier segments, minus a checked-in skip list of generic words (`data`, `events`);
- `venue.product:` key prefixes;
- each venue's extra `identifiers` (Polymarket: `Poly-RateLimit`).

**And** today's Polymarket residue is caught, including `clob_default`, `PERPS_GENERAL` and the `Poly-RateLimit-*` headers
**And** the grep finds nothing beyond the `gate_exceptions` that each venue declares in its own metadata

### Story 5.3: Gamma and data become modules

As a consumer of Polymarket market and user data,
I want gamma and data under `polyoxide-polymarket`,
So that I depend on one Polymarket crate.

**Implements:** FR7, FR8, NFR7

**Acceptance Criteria:**

**Given** the `gamma` and `data` features
**When** the two crates move into `src/gamma/` and `src/data/`, laid out per the conventions
**Then** their error enums are renamed per the convention (for example `DataApiError` → `DataError`), and each rename is recorded in the manifest
**And** module READMEs become feature-gated doctests, and the README count check passes
**And** fixtures move to `tests/fixtures/<module>/`
**And** test targets are renamed per the convention, and their `live.<target>` entries move with them
**And** capture scripts become `scripts/capture_polymarket_<module>.py`
**And** `-F gamma` alone and `-F data` alone each build and pass `cargo hack`

### Story 5.4: clob and relay become modules

As a Polymarket trader,
I want clob and relay under `polyoxide-polymarket` with their signing intact,
So that orders and relay calls work exactly as before, under one crate.

**Implements:** FR7, NFR2, NFR7

**Acceptance Criteria:**

**Given** the `clob`, `clob-ws` and `relay` features
**When** the crates move into `src/clob/` and `src/relay/`
**Then** `clob` no longer implies `gamma`, and gamma-dependent helpers are gated `all(clob, gamma)`
**And** clob's `ws` feature becomes `clob-ws`, and its `WebSocketError` becomes `ClobWsError`
**And** `keychain` keeps its name
**And** the EIP-712 and session-key golden vectors, the kill-outcome tests and the relay suites move with empty body diffs
**And** the per-feature deny-list check proves `alloy` is built only with `clob` or `relay`

### Story 5.5: perps, rtds and sports become modules

As a consumer of the perps, price and sports feeds,
I want them under `polyoxide-polymarket`, with the credential-free feeds still light,
So that enabling only `rtds` or `sports` builds no HTTP or signing code.

**Implements:** FR3, FR7, NFR2, DRIFT R6

**Acceptance Criteria:**

**Given** the `perps`, `perps-ws`, `rtds` and `sports` features
**When** the crates move into their modules
**Then** `test-fixtures` becomes `test-server` (DRIFT R6, in its own commit), additive per module via `cfg(all(feature = "test-server", feature = "<module>"))`
**And** the per-feature allowlist and deny-list checks pass for `-F rtds` alone and for `-F sports` alone, with no reqwest, alloy, governor, hmac, sha2 or keyring
**And** every supervision suite moves with an empty body diff

### Story 5.6: Binance follows the conventions

As a Binance consumer,
I want Binance laid out like every other venue,
So that what I learn on one venue applies to the next.

**Implements:** FR7

**Acceptance Criteria:**

**Given** `polyoxide-binance`
**When** it adopts the conventions
**Then** `BinanceError` becomes `UsdmError`
**And** usdm follows `src/usdm/{api,ws,types,error,venue}`, with `usdm` and `usdm-ws` features
**And** its venue and product ids are declared in metadata
**And** its suites move with empty body diffs, and every rename is in the manifest

### Story 5.7: The multi-venue umbrella

As a consumer of several venues,
I want one `polyoxide` crate that exposes every venue behind features,
So that I enable venues instead of hunting for crates.

**Implements:** FR7, FR10, AD-18

**Acceptance Criteria:**

**Given** `polyoxide`
**When** it is rebuilt
**Then** it exposes `polyoxide::{venue, polymarket, binance, prelude}` behind the features `polymarket` (the default), `polymarket-<module>`, `binance` and `full`
**And** `full` covers every module of every venue, including `-ws` and `keychain`
**And** the unified `Polymarket` client moves to `polyoxide_polymarket::Polymarket`, its builder returns a `BuildError`, and `PolymarketError` is removed
**And** the root README remains a doctest
**And** docs.rs lists every module and `-ws` feature for both venue crates, without `test-server`
**And** every venue module's header-pin test also passes under `polyoxide --features full` (AD-18's S2 build)

### Story 5.8: The CLI on the venue axis

As a CLI user,
I want commands grouped by venue, with one way to stream any feed,
So that every feed streams, formats and stops the same way.

**Implements:** FR7, FR14 (C1–C3)

**Acceptance Criteria:**

**Given** `polyoxide <venue> <module> <verb…>`
**When** any socket is streamed
**Then** it streams through `stream` with one runner and one `OutputFormat`, including market, user and prices, which drop their old ctrl-c + `select!` loops
**And** `clob prices download` lives under `polymarket clob prices download`
**And** every list flag uses `value_delimiter` and has a parse test that passes it
**And** the `ws_sports`, `ws_binance` and `data_v2` tests keep their assertions on the new paths
**And** no copy of `OutputFormat` or of the runner (C1–C3) remains

### Story 5.9: CLI credentials over a shared trait

As a CLI user storing credentials for a venue,
I want one credentials command, with each venue defining its own credential kinds,
So that a new venue's credentials need no new CLI plumbing.

**Implements:** NFR6, AD-21

**Acceptance Criteria:**

**Given** a `StoredCredential` trait in core's keychain, covering the service, field names, which fields are secret and validation, with secret fields held in `Secret<T>`
**When** the CLI runs `polyoxide <venue> credentials <store|show|delete> <kind>`
**Then** the command is generic over the trait, and Polymarket defines its `clob` and `builder` kinds
**And** `show` never prints a secret field in clear
**And** the existing keychain service names keep working
**And** parse tests cover every kind and verb

### Story 5.10: Python on the venue axis

As a Python user,
I want Polymarket's clients under `polyoxide.polymarket`,
So that the package can grow to more venues without name collisions.

**Implements:** FR7

**Acceptance Criteria:**

**Given** the Python package
**When** it is rebuilt
**Then** it exposes `polyoxide.polymarket.{clob, gamma, data}` and `polyoxide.polymarket.data.v2`, with no `Clob*` collision prefixes
**And** `test_stub_consistency.py`, `every_v2_getter_reads_its_own_key` and the offline suites pass on the new paths
**And** `pyproject.toml` describes a multi-venue package

### Story 5.11: Docs and standing rules for a multi-venue toolkit

As an agent working on polyoxide,
I want every document to describe the toolkit as it now is,
So that no standing rule sends me to code that no longer exists.

**Implements:** FR7, FR11, AD-21

**Acceptance Criteria:**

**Given** README, CLAUDE.md, `docs/specs/INDEX.md` and SELF-HEALING.md
**When** the generator runs and the prose is updated
**Then** they describe a multi-venue toolkit
**And** CLAUDE.md's rules superseded in S2 are rewritten in their own sections: Binance outside the umbrella, clob's `ws` feature, `polyoxide.v2`, the `live_api`/`mock_api` naming, and Module Organization
**And** `docs/ARCHITECTURE.md` is regenerated and states that the workspace is in stage S2
**And** nightly issues whose test identity changed are closed or renamed

### Story 5.12: The S2 release

As a prader-rs maintainer,
I want one release that carries every rename, with a complete migration map and pointers left on the old crates,
So that I migrate my imports once.

**Implements:** NFR1, NFR7, NFR11, NFR12, AD-12, AD-16, AD-22, AD-25

**Acceptance Criteria:**

**Given** the integration session
**When** every manifest entry has a row, CI is green on the draft PR, and nightly-behavioral dispatched on the integration ref is clean or only `auth-gated`/`environmental`
**And** every mutant in `docs/MUTANTS.md` still fails its moved test
**Then** the session merges to `main` in one merge
**And** the version bump is a separate commit on `main`

**Given** the seven retired crates
**When** the release runs
**Then** each publishes one tombstone from `tombstones/<crate>/`, outside the workspace
**And** each tombstone has no dependencies and no re-exports, only a pointer and a `compile_error!` gated `#[cfg(not(docsrs))]`
**And** AD-25's resumable publish loop publishes the tombstones after every workspace crate, with `--no-verify`
**And** CI checks only their manifests, and the generator skips them
**And** no old version is ever yanked
**And** the release notes carry the consumer-impact section, link the rename manifest, and list every moved tracing target (for example `polyoxide_clob` → `polyoxide_polymarket::clob`)
**And** the release publishes one new crate name, `polyoxide-polymarket`

**Given** the S2 release is out
**When** the next commit lands on `main`
**Then** it deletes `tombstones/`
**And** it switches the removal gate to fail on any removal, using the S2 tag as the baseline

## Epic 6: Venue-neutral market data

One generic function reads quotes and books from three venues: the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M. It works through canonical keys and typed extensions. This is stage S3, so every story adds public API and none removes any.

### Story 6.1: Shared records and typed extensions

As a consumer reading several venues,
I want one set of market-data records that still carries each venue's own facts,
So that I can compare venues without losing anything venue-specific.

**Implements:** FR5, NFR8, AD-3, AD-19

**Acceptance Criteria:**

**Given** `polyoxide-venue`
**When** the records land
**Then** it defines `Side { Buy, Sell }`, `Level`, and `Instrument`, which is the only type that carries tick size, minimum size and `SizeUnit`
**And** `size` is in the instrument's native quantity, with a separate `notional` for quote-currency amounts
**And** records derive `Clone`, `Debug` and `PartialEq`, and carry `Extensions`

**Given** `Extensions`
**When** it is used
**Then** it is `Clone + Debug + Default + PartialEq + Send + Sync`, with no serde support
**And** an inserted `T` must be `Clone + Debug + PartialEq + Send + Sync + 'static`
**And** equality compares contents
**And** inserting a type that is already present is an error
**And** it exposes the `TypeId`s it holds
**And** tests prove every rule above, including that two records differing only in one extension value are unequal

### Story 6.2: Canonical market keys

As a consumer parsing keys from config or the command line,
I want one canonical key per instrument,
So that `binance.usdm:btcusdt` and `binance.usdm:BTCUSDT` are the same key in my maps.

**Implements:** FR5, AD-4, AD-7

**Acceptance Criteria:**

**Given** `MarketKey`, which has no public constructor from text or from parts, with venue and product as `Cow<'static, str>` newtypes and the id as an opaque `Arc<str>`
**When** text is parsed with `RawKey::parse`
**Then** the text is split at the first `.` and the first `:`, and the id may itself contain `:`
**And** only a product's constructors produce a `MarketKey`
**And** Binance's constructors live in `usdm::key` and uppercase ASCII, while Polymarket's live in `src/shared/keys.rs` under `any(clob, gamma, data)`
**And** an event-contract key names an outcome token (AD-7)
**And** serde uses the text form
**And** `polyoxide::parse_key` dispatches to the enabled venues
**And** each venue crate tests that its id constants equal its metadata
**And** tests cover case canonicalization and a synthetic id that contains a second `:`

### Story 6.3: The market-data traits, implemented for Binance USDⓈ-M

As a consumer of Binance perps,
I want Binance readable through the shared market-data traits,
So that code I write for one venue works on the next.

**Implements:** FR5, AD-5, AD-6

**Acceptance Criteria:**

**Given** the traits `MarketData` (instruments, quote, book), `Trades`, `Candles`, `Funding` and `EventGroups` in `polyoxide-venue`
**When** they are defined
**Then** each has `Send + Sync` supertraits
**And** each method returns `impl Future<Output = Result<_, ClassifiedError>> + Send`
**And** each trait has a wrapper of the form `#[dynosaur::dynosaur(pub DynMarketData = dyn(box) MarketData)]`
**And** no method returns `Unsupported`

**Given** `polyoxide-binance`'s usdm module
**When** it implements `MarketData`, `Trades`, `Candles` and `Funding` in `src/usdm/venue.rs`
**Then** fields Binance does not publish are `Option`, and Binance-only facts sit in usdm's `<Record>Ext` types
**And** an `Arc<DynMarketData<'static>>` holding usdm compiles and returns the native client's values on the captured fixtures

### Story 6.4: Polymarket perps through the traits

As a consumer of Polymarket perps,
I want perps readable through the same traits as Binance,
So that a perps dashboard handles both venues with one code path.

**Implements:** FR5

**Acceptance Criteria:**

**Given** the perps module
**When** it implements `MarketData`, `Trades`, `Candles` and `Funding`
**Then** instrument ids map to keys through perps' constructor
**And** perps-only facts (fee tiers, risk tiers, price bounds) sit in perps' `<Record>Ext` types
**And** fixture-based tests show the trait results equal the native client's for every implemented method

### Story 6.5: The Polymarket CLOB through the traits

As a consumer of prediction markets,
I want CLOB outcomes readable through the same traits,
So that event contracts and perps share one market-data interface.

**Implements:** FR5, AD-7

**Acceptance Criteria:**

**Given** the clob module
**When** it implements `MarketData`, `Trades` and `Candles` (from prices-history)
**Then** each key names one outcome token, and that outcome's book holds bids and asks in its own price terms
**And** CLOB-only facts (neg-risk, tick changes) sit in clob's `<Record>Ext` types

**Given** both `clob` and `gamma` are enabled
**When** `EventGroups` is implemented
**Then** it groups outcomes into events through gamma, gated `all(clob, gamma)`
**And** fixture-based tests show the trait results equal the native clients'

### Story 6.6: One generic function across venues

As a consumer like prader-rs,
I want to write market-data and error-handling code once against the shared interfaces,
So that adding a venue means adding no code.

**Implements:** FR4, FR5, NFR11

**Acceptance Criteria:**

**Given** a generic `best_bid_ask(&DynMarketData, &MarketKey)` and a generic error classifier written only against `Classify`
**When** they run against mock servers for the clob, perps and usdm modules
**Then** one test proves both work unchanged against each module (the CAP-5 and CAP-4 success criteria)
**And** a `#[ignore]` live variant runs nightly through `or_fail`
**And** the umbrella exposes the traits under `polyoxide::venue`
**And** the link to the example is recorded in `spine-amendments/epic-6.md`, and `docs/ARCHITECTURE.md` is regenerated once that amendment is merged
**And** the first release to ship this epic's work carries a consumer-impact note stating that the work is additive

## Epic 7: Supervised CLOB sockets and normalized trading

Consumers trade Polymarket through a lossless `Trading` interface built on supervised CLOB sockets. Fills arrive on an unbounded stream, a reconnect emits `Resync`, and a killed order ends in a `Killed` status. This is stage S3, so the work adds a new supervised socket type and leaves `WebSocketWithPing` in place. The epic depends on Epics 4 and 6.

### Story 7.1: A supervised CLOB market socket

As a consumer streaming CLOB books,
I want a market socket that reconnects and replays my subscriptions by itself,
So that a dropped connection never silently freezes my books.

**Implements:** FR12, AD-11

**Acceptance Criteria:**

**Given** a new supervised clob market type that runs a `ClobMarketProtocol` on the `Supervisor`
**When** it runs against the scripted server
**Then** it reconnects after a server close and after a stall
**And** it replays its asset membership after each reconnect
**And** it yields one `Disconnected` and one `Reconnected` per outage
**And** it keeps the 10 s text `PING` and skips `PONG`
**And** it decodes Binary frames as text
**And** it stays wanted and connected while its membership is empty
**And** `WebSocketWithPing` is unchanged

### Story 7.2: A supervised CLOB user socket

As a Polymarket trader,
I want my order and fill feed supervised without widening or losing my subscription,
So that a reconnect never subscribes me to every market by accident and never drops my fills.

**Implements:** FR12, AD-11, AD-24

**Acceptance Criteria:**

**Given** a supervised clob user type whose membership is `Option<Vec<Market>>`
**When** it runs against the scripted server
**Then** `None` subscribes to every market, and an empty `Some` subscribes to none
**And** a test pins that an empty `Some` never collapses into `None`
**And** the credentials in the subscribe frame are rebuilt and replayed on every reconnect
**And** it declares `Disconnected` and an `Unbounded` queue
**And** fills sent while the consumer is blocked are all delivered, in order, once it resumes

### Story 7.3: The Trading trait

As a trader writing venue-neutral strategy code,
I want one complete trading interface,
So that my order, fill and inventory logic works on any venue that trades.

**Implements:** FR6, AD-5, AD-19, AD-20, AD-24

**Acceptance Criteria:**

**Given** a `Trading` trait in `polyoxide-venue`, shaped per AD-5 with a `DynTrading` wrapper
**When** it is defined
**Then** it covers place, cancel, cancel-all, `open_orders`, `positions`, `balances`, and `events()`, which returns an `impl Stream<…> + Send + Unpin + 'static`
**And** an order names an outcome key, `Side`, price, native size, time in force, client order id, post-only and reduce-only
**And** venue options travel in `Extensions`, and a request that carries an undeclared extension is refused as `InvalidRequest`
**And** order statuses include `Filled`, `Cancelled` and `Killed { reason }`
**And** events include acks, fills and `Resync`
**And** order, trade and fill ids are opaque `Arc<str>` newtypes
**And** the trait documents that the venue replays nothing, and that every fills-channel `Reconnected` is followed by `Resync` before any later fill
**And** a test implementation proves the trait compiles both statically and as `Arc<DynTrading<'static>>`

### Story 7.4: Trading on the Polymarket CLOB

As a Polymarket trader,
I want to place, cancel, reconcile and track fills, positions and balances through `Trading`,
So that my inventory never drifts from the venue's.

**Implements:** FR6, NFR8, AD-3, AD-20, AD-24

**Acceptance Criteria:**

**Given** the clob `Trading` implementation, gated `all(clob, data)` and declared as such in Cargo metadata
**When** a FAK or FOK order is killed
**Then** `place` returns `Ok` with the terminal status `Killed { reason }`
**And** the native `ClobError::FakUnmatched` and `FokUnfilled` remain for native callers
**And** `ClobOrderOptions` carries neg-risk, expiration and fee terms
**And** mock tests cover place, cancel, cancel-all and open-orders reconciliation

**Given** `events()`
**When** it is consumed
**Then** it is fed from the supervised user socket (Story 7.2) through an unbounded queue
**And** after every `Reconnected` it yields `TradingEvent::Resync` before any later fill
**And** a test with a slow consumer and a reconnect mid-stream shows every fill delivered and `Resync` in the right place

**Given** `positions` and `balances`
**When** they are called
**Then** `positions` reads the data API, `balances` reads the CLOB balance route, and fields the venue does not publish are `Option`

### Story 7.5: The Kalshi trading mapping

As a maintainer preparing Kalshi integration,
I want Kalshi's order and fill fields mapped against the `Trading` trait now,
So that the trait is proven to fit a second event-contract venue before Kalshi ships.

**Implements:** FR6, NFR11, AD-7

**Acceptance Criteria:**

**Given** Kalshi's OpenAPI 3.34.0, its `CreateOrderV2Request` and its fill message
**When** `docs/specs/kalshi/trading-mapping.md` is written
**Then** every common order field (ticker, side, count, price, time in force, client order id, post-only, reduce-only) maps to a trait field or to a named future Kalshi option type
**And** every common fill field (trade and order ids, price, size, fee, taker flag) maps the same way
**And** the YES-side-only conversion is spelled out per AD-7: buying NO at p is an ask on YES at 1 − p
**And** subaccounts, order groups, RFQ and self-trade prevention are listed as deferred to Kalshi integration
**And** any field that fits neither the trait nor a deferral is recorded in `spine-amendments/epic-7.md`
**And** the first release to ship this epic's work carries a consumer-impact note for the new supervised clob types and `Trading`

## Epic 8: Kalshi walking skeleton

A third venue lands by following the guide, and its changes touch only that venue's own files. It provides exchange status, `MarketData`, and a signed supervised socket against Kalshi's demo host. Any gap in the guide becomes a spine amendment. This is stage S3. It depends on Epics 1–6. Its permitted touch set is AD-13's new-venue list.

### Story 8.1: Register Kalshi by the recipe

As a maintainer adding a venue,
I want Kalshi registered by following the guide's steps alone,
So that the registration path is proven on a real third venue.

**Implements:** FR11, FR13, AD-13

**Acceptance Criteria:**

**Given** the guide's recipe, steps 1–2
**When** `polyoxide-kalshi` is created
**Then** it has a `members` line, a `[workspace.dependencies]` pin, and `[package.metadata.polyoxide]` declaring venue `kalshi` and product `events`
**And** a unit test checks that the crate's id constants match that metadata
**And** it is `publish = false`
**And** `docs/specs/kalshi/` mirrors Kalshi's OpenAPI 3.34.0, with a `mirrors` entry so nightly-schema watches it
**And** `OBSERVED.md` exists
**And** the generated regions are regenerated, not hand-edited

**Given** a touch-list check
**When** it runs on this epic's PRs
**Then** it fails on any path outside AD-13's new-venue set: `members` and `[workspace.dependencies]` entries, `Cargo.lock`, the crate directory (including its `gate_exceptions`), `docs/specs/kalshi/` and its mirrors entry, `scripts/capture_kalshi_*.py`, `ci/dep-allowlists/polyoxide-kalshi*.txt`, `spine-amendments/epic-8.md`, `_bmad-output/specs/**`, and regenerated regions

### Story 8.2: Signed Kalshi requests and exchange status

As a Kalshi user,
I want signed, throttled requests through the shared HTTP path,
So that Kalshi gets the same retry and hold behaviour as every other venue without copying it.

**Implements:** FR13, NFR3, NFR6, AD-8, AD-10, AD-15

**Acceptance Criteria:**

**Given** a Kalshi `Authenticator`
**When** a request is signed
**Then** it signs RSA-PSS/SHA-256 over `timestamp_ms + METHOD + path-without-query` on every attempt
**And** the key is loaded from a file or an environment variable into `Secret<T>`
**And** Ed25519 support is recorded as deferred

**Given** a Kalshi throttle composed from core capacity buckets
**When** requests are charged
**Then** the buckets start provisionally from the published Basic tier, recorded as provisional in `OBSERVED.md`; while provisional they wait rather than return `Refused`
**And** on first use they are confirmed by reading `/account/limits`, charged to the read bucket, and resized in place with core's `resize`
**And** each request builder takes its token costs from `/account/endpoint_costs` (default 10), and a drift test checks the route table against that endpoint
**And** the per-shard write buckets, and the 429s that carry no `Retry-After`, are recorded in `OBSERVED.md`
**And** `KalshiError` decodes Kalshi's error body in one function and classifies by status first

**Given** the exchange-status route
**When** it is tested
**Then** mock tests cover it
**And** a `#[ignore]` live test against the demo host runs through `or_fail`, with its secrets declared in its `live.<target>` entry
**And** that live test is `auth-gated` when the secrets are absent

### Story 8.3: Kalshi market data through the traits

As a consumer of prediction markets,
I want Kalshi's markets readable through the same `MarketData` interface as the Polymarket CLOB,
So that my event-contract code works on both venues.

**Implements:** FR5, FR13, AD-4, AD-7

**Acceptance Criteria:**

**Given** Kalshi's key constructor
**When** keys are built
**Then** each key names an outcome, as `kalshi.events:<TICKER>:yes|no`, with the ticker canonicalized

**Given** `MarketData` for Kalshi
**When** a book is read
**Then** the yes/no bid ladder converts deterministically: a NO bid at p is a YES ask at 1 − p
**And** fixed-point dollar and count strings become `Decimal`, with fractional counts kept
**And** Kalshi-only facts sit in `KalshiExt` types
**And** fixtures are captured by `scripts/capture_kalshi_events.py` through `capture_common.py`, passing a header function that signs each request
**And** the wire- and spec-agreement tests use the test-support helpers, with no copies
**And** Story 6.6's generic `best_bid_ask` passes against a Kalshi mock

### Story 8.4: A supervised Kalshi socket with a signed handshake

As a Kalshi user,
I want Kalshi's order-book feed supervised and re-signed on every reconnect,
So that a reconnect never fails on a stale signature.

**Implements:** FR13, NFR3, AD-11

**Acceptance Criteria:**

**Given** a `KalshiProtocol` on the `Supervisor`
**When** it connects, and on every reconnect attempt
**Then** the handshake signs `timestamp + "GET" + "/trade-api/ws/v2"` afresh

**Given** subscriptions to markets
**When** membership changes
**Then** `subscribe` and `update_subscription` (add or delete markets) carry the change, and replay restores it after a reconnect

**Given** `orderbook_snapshot` and `orderbook_delta` frames
**When** they are decoded
**Then** a sequence regression is reported as a protocol event
**And** maintaining a local book is deferred to Kalshi integration
**And** socket errors are classified with `impl_ws_classification!`
**And** scripted-server tests prove a fresh signature on every attempt and replay after a reconnect
**And** a `#[ignore]` `live_ws` test against the demo host is `auth-gated` when its declared secrets are absent

### Story 8.5: The guide, validated

As a maintainer planning the next venue,
I want proof that the skeleton needed nothing beyond the guide,
So that the success signal is demonstrated, not assumed.

**Implements:** FR11, FR13, FR14

**Acceptance Criteria:**

**Given** this epic's merged PRs
**When** they are audited
**Then** their combined diff touches only the amended AD-13 set
**And** it makes no edit to `polyoxide-{venue,core,ws,test-support}`
**And** `polyoxide-kalshi` contains no copied helper, loop, server or classifier, checked against the duplication inventory's helper names
**And** every step the guide missed is recorded in `spine-amendments/epic-8.md`, merged into the spine through `bmad-architecture`, and `docs/ARCHITECTURE.md` is regenerated
**And** the spec's success signal is recorded as met, or as amended, through `bmad-spec`
