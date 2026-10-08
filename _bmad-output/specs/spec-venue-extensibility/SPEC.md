---
id: SPEC-venue-extensibility
companions:
  - glossary.md
  - venue-landscape.md
  - duplication-inventory.md
  - divergences.md
  - registration-points.md
  - ../../planning-artifacts/architecture/architecture-polyoxide-2026-10-08/ARCHITECTURE-SPINE.md
  - ../../../CLAUDE.md
sources: []
---

> **Canonical contract.** This SPEC and the files in `companions:` are the complete, preservation-validated contract for what to build, test, and validate. Source documents listed in frontmatter are for traceability — consult them only if you need narrative rationale or prose color this contract intentionally omits.

# Venue extensibility: one foundation, venue-first structure, venue traits

## Why

Pain and opportunity. Binance, polyoxide's second venue, was added by copying and adapting
code. There are now five TLS shims, four reconnect backoffs, seven HTTP retry loops and two
cooldown engines. About 3,500 lines are duplicated, plus about 2,000 lines of hand-written
query setters. Core still carries Polymarket's signing, tier headers and limiter tables. One
venue's `reqwest` feature changed every other client's wire behaviour (0.37.0 shipped the
regression; 0.37.1 fixed it). At least nine hand-kept registration lists exist, and they have
already drifted: the CLI the README says to `cargo install` has never reached crates.io.
Kalshi is next. As things stand it would add a sixth copy of every block, edit core again,
and bring a third credential scheme and a third limiter model. The consumer, prader-rs,
already normalises each venue itself and hand-classifies every polyoxide error enum. Now is
the cheapest moment to change this: two venues show what is common, the third is not yet
written, and breaking changes are accepted.

## Capabilities

- **CAP-1**
  - **intent:** Every HTTP request in every venue crate goes through one send-and-retry path that applies rate-limit feedback. Client configuration, namespaces, query parameters and wire enums share one vocabulary.
  - **success:** The retry loop has one definition. Each of these mutants fails a test:
    - one that drops the hold on a last-attempt 429;
    - one that skips the response observation on the last attempt;
    - a policy that returns a zero wait.

    `clob` ping and `gamma` `post_json` are gated, and tests pin both. No crate defines its own `open_enum!`, `wire_enum!`, `UnknownVariant` or builder knobs.
- **CAP-2**
  - **intent:** A venue supplies its own limiter model (window quotas, per-IP weight minutes, per-account token-cost buckets, per-signer buckets) through one interface that the HTTP path calls. The interface carries a per-request cost and refuses, client-side, any cost the bucket can never hold. One cooldown primitive is shared by all venues.
  - **success:** `RateLimiter` and `WeightBudget` both implement the interface. The HTTP client names no concrete limiter. Venue limiter tables live in venue crates. A test outside the foundation crate builds a Kalshi-style token-cost bucket without editing the foundation.
- **CAP-3**
  - **intent:** These socket building blocks exist once, and credential-free feeds can use them without pulling in signing dependencies: TLS provider install, reconnect backoff, connect with timeout, handshake-status and close classification, the supervised task shell, per-attempt handshake headers, and the scripted test server. Venues inject their ping schedule, liveness rule, backoff-reset input and auth, whether it goes in the handshake (Kalshi) or the subscribe frame (Polymarket user channel).
  - **success:** Each socket row in `duplication-inventory.md` has one definition. With only `rtds` or only `sports` enabled, the Polymarket crate's `cargo tree` contains neither `reqwest` nor `alloy`. Existing supervision tests pass with behaviour unchanged.
- **CAP-4**
  - **intent:** Every venue error maps, through one shared interface, to one of eight classes, with retriability and fault flags derived from the class. The classes are network, unavailable, rate-limited (with retry-after), unauthorized, invalid request, venue refusal (with venue code), restricted, and decode. One Retry-After parser and one retriable-status rule serve all venues.
  - **success:** Every venue error type implements the interface, enforced at compile time. Venue traits return one classified error type. A consumer classifier like prader's `perps_err` is written once, generic over the interface.
- **CAP-5**
  - **intent:** Consumers read instruments or markets, tickers, order books, trades and candles (plus funding for perps). They do so through one base market-data trait that every venue product implements, plus capability traits for the abilities only some venues offer. Ids are tagged with their venue, fields a venue does not publish are `Option`, and venue-only facts stay reachable.
  - **success:** The traits are implemented for the Polymarket CLOB, Polymarket perps and Binance USDⓈ-M. One generic function, such as best bid/ask for a key, runs unchanged against each.
- **CAP-6**
  - **intent:** One normalized trading interface covers place, cancel, cancel-all, open-order reconciliation, order acks and fills as an async event stream, positions and balances. Venue-specific order options and kill outcomes (FAK/FOK) are preserved, not flattened.
  - **success:** The interface is implemented for the Polymarket CLOB. A recorded mapping table maps Kalshi's common order and fill fields to the trait. Order fields: ticker, side, count, price, time in force, client order id, post-only, reduce-only. Fill fields: trade and order ids, price, size, fee, taker flag. The table lists Kalshi-only concepts as deferred to Kalshi integration.
- **CAP-7**
  - **intent:** Each venue is one crate. Its hosts or products are feature-gated modules, each laid out as api/ws/types. The `polyoxide` umbrella re-exports each venue behind a venue feature, together with the shared traits. The CLI and Python bindings expose venues on one axis.
  - **success:**
    - `polyoxide-polymarket` holds clob, gamma, data, relay, rtds, sports and perps as modules, and `polyoxide-binance` holds usdm.
    - `polyoxide` with feature `binance` reaches Binance.
    - The perps paths `polymarket::perps` and `binance::usdm` cannot be confused.
    - The CLI groups commands by venue, with one shared streaming runner and one `OutputFormat`.
    - Python exposes one submodule per venue, with no collision prefixes.
    - README, CLAUDE.md and `docs/specs/INDEX.md` describe a multi-venue toolkit.
    - A CI gate finds no venue identifier in the venue-neutral crates. It matches venue names as substrings, product names as identifier parts (generic words skipped), key prefixes, and identifiers a venue declares. Each venue declares its own exceptions.
- **CAP-8**
  - **intent:** Adding a venue edits one registration source. Everything else either derives from it or is checked against it in CI: workspace members, publish order, nightly live rows, schema watch list and exclusions, classifier patterns, and the docs crate table.
  - **success:**
    - CI fails when a crate lacks a publish entry, when a `tests/live_*.rs` lacks a nightly row, or when publish order disagrees with the dependency graph.
    - `release.yml` and `finish_release.sh` read the same list.
    - The release publishes `polyoxide-cli`, so `cargo install polyoxide-cli` works.
- **CAP-9**
  - **intent:** These exist once: the spec- and wire-agreement helpers, fixture loading, the soak harness, and the capture-script helpers.
  - **success:** Each has one definition, used by every crate that needs it. The Kalshi skeleton's agreement tests copy no helper.
- **CAP-10**
  - **intent:** One venue's dependency features or settings cannot change another venue's wire behaviour.
  - **success:** A test pins each client's request headers, including `Accept-Encoding`, whichever venue features are enabled.
- **CAP-11**
  - **intent:** A written venue-onboarding guide covers:
    - crate layout
    - registration
    - the drift-detector pattern
    - limiter measurement
    - trait implementation
  - **success:** The Kalshi skeleton is built by following the guide. Every step the guide missed is recorded in the epic's spine-amendments file, one session merges that file into the architecture spine, and the guide is regenerated from the spine.
- **CAP-12**
  - **intent:** The CLOB market and user sockets get the same supervision as the other feeds: reconnect with backoff, staleness detection, membership replay (including user-channel credentials) and outage markers.
  - **success:** Against the shared scripted test server, the supervised CLOB socket:
    - reconnects after a server close and after a stall;
    - replays the market assets and the user markets;
    - yields `Disconnected` followed by `Reconnected` for each outage;
    - keeps its 10 s `PING` schedule unchanged.

## Constraints

- Breaking changes ship with no shims, in lockstep minor bumps over three stages:
  - S1, internals. S1 breaks only the paths it consolidates.
  - S2, renames. Every other rename lands here.
  - S3 and later, additions.

  prader-rs migrates only after each release is on crates.io, because it never takes a path or git dependency on polyoxide.
- Credential-free modules must not pull in HTTP or signing stacks.
  - Enabling only a socket-only module builds just three sets of dependencies: the shared socket crate's, the vocabulary crate's (`rust_decimal`, `serde`, `thiserror`, `futures-core`, `dynosaur`), and the module's own.
  - The shared socket crate may depend only on tokio, tokio-tungstenite, futures-util, thiserror, tracing, rustls (`ring`, `std`), and optionally serde/serde_json.
  - Venue auth enters by injection.
- Socket handshake auth is computed per connection attempt, because Kalshi's signature covers a millisecond timestamp.
- The throttle interface takes a per-request cost. Binance weights, Kalshi token costs and Polymarket batch orders all charge more than one unit per request.
- The divergences listed in `divergences.md` stay per-venue. DRY merges only identical or parametric copies, and the branch that carries behaviour stays at the venue's call site.
- Each DRIFT row is applied as decided in `divergences.md` when its copies merge. R1, R2 and R4 change behaviour users can see, so the release notes name them.
- Credential storage is split. Shared code owns the mechanism: keyring access, secret redaction, and the plumbing for CLI `credentials` store, show and delete. Each venue owns its credential type: fields, validation, service name.
- Tests that carry behaviour move; they are not deleted or weakened. That covers mutation-tested rate-limit rules, supervision invariants, kill-outcome classification, effective-quota agreement tests, spec and wire agreement tests, and live drift detectors.
- Normalization loses no venue information:
  - absent data is `Option`
  - ids are opaque or at least `u64`
  - money and size are `Decimal`
  - venue-only fields stay reachable
- The existing CI gates hold:
  - clippy `-D warnings`
  - rustdoc `-D warnings`
  - fmt
  - MSRV 1.91
- The soak harnesses match the retry log line `Retriable status 429` on target `polyoxide_core`. The merged loop either keeps that line for every venue, or the harnesses move to a structured signal in the same change.

## Non-goals

- Full Kalshi coverage, Kalshi-only order concepts (subaccounts, order groups, RFQ, self-trade prevention) and Kalshi credential storage. These are decided at Kalshi integration; the walking skeleton proves extensibility.
- New routes or features for existing venues: Binance signed routes, perps trading, new Polymarket routes.
- Aggregating, comparing or routing across venues.
- Compatibility shims or deprecated re-exports.
- Unifying the deliberate divergences.
- New Python coverage (Binance, perps, sockets).

## Success signal

- A Kalshi walking skeleton lands with no foundation edits and no copied infrastructure. It touches only:
  - its `members` line, its `[workspace.dependencies]` entries (new third-party pins included), and `Cargo.lock`;
  - its crate directory, `docs/specs/kalshi/` and its mirrors entry;
  - `scripts/capture_kalshi_*.py` and `ci/dep-allowlists/polyoxide-kalshi*.txt`;
  - its spine-amendments file and `_bmad-output/specs/**` records;
  - regenerated regions (spine AD-13).

  It provides exchange status, one market-data trait implementation, and one supervised socket with a signed handshake against the demo host.
- Every row in `duplication-inventory.md` has exactly one definition.

## Assumptions

- The market-data and trading traits sit on top of the native clients, which keep every route; the traits cover the common subset.
- The Kalshi walking skeleton is in this spec's scope. It loads its RSA key from a file or environment variable.
- "No shims" applies to the Python package and the CLI command tree as well as to Rust crates.
- The `polyoxide` brand stays; only crate, module and command names change.
- The CLOB's outage markers keep the `Disconnected`-then-`Reconnected` invariant, as Binance's do.
